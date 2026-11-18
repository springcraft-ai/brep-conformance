//! What the report's viewer shows for a case: named views, each a few solids (a shape, maybe
//! moved by a matrix) drawn as the subject, a second subject, or a ghost behind them, with a plane
//! or a box laid over where the case is about one. The views follow what the case tests, so a
//! mirror join shows the part and its image, a split its two pieces, a loft its sections.

use serde_json::{Value, json};

use crate::generate::Case;
use crate::model::{Frame, Op, Primitive, Shape, Vec3};
use crate::oracle;

/// The keys of what a case's views lay over its drawing, which the export copies into the file.
pub const OVERLAY: &[&str] = &[
    "points", "box", "plane", "lines", "look", "clashes", "closest", "sides", "normals",
];

thread_local! {
    /// The shapes of the case whose views are being drawn up, each once, by its JSON's text.
    static TABLE: std::cell::RefCell<Vec<(String, Value)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The index of `s` in the case's table of shapes, adding it the first time.
fn index(s: &Shape) -> usize {
    let v = s.to_json();
    let key = v.to_string();
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(i) = t.iter().position(|(k, _)| *k == key) {
            return i;
        }
        t.push((key, v));
        t.len() - 1
    })
}

/// A solid in a view, as the subject (`main`), a second subject (`alt`) or a ghost behind them:
/// a shape of the table by index, or a boolean of two of them as `[op, i, j]`.
fn solid(role: &str, s: &Shape, matrix: Option<[[f64; 4]; 3]>) -> Value {
    let shape = match s {
        Shape::Bool(op, a, b) => json!([op.word(), index(a), index(b)]),
        _ => json!(index(s)),
    };
    let mut v = json!({"role": role, "s": shape});
    if let Some(m) = matrix {
        v["matrix"] = json!(m);
    }
    v
}

/// `a` as a ghost behind `b`, a result made from it: only where it lies outside `b`, so it ends at
/// the result's faces instead of lying on them, where two meshes of one curved surface would
/// show through each other.
fn ghost_outside(a: &Shape, b: &Shape) -> Value {
    ghost_as("ghost", a, b)
}

/// [`ghost_outside`] in the tone of `role`.
fn ghost_as(role: &str, a: &Shape, b: &Shape) -> Value {
    // A boolean result by its operands' indices, so the table holds no second copy of them.
    let b = match b {
        Shape::Bool(op, x, y) => json!([op.word(), index(x), index(y)]),
        _ => json!(index(b)),
    };
    json!({"role": role, "s": ["outside", index(a), b], "flush": true})
}

fn view(name: &str, solids: Vec<Value>) -> Value {
    json!({"name": name, "solids": solids})
}

/// A view with a line saying what it shows.
fn noted(name: &str, note: &str, solids: Vec<Value>) -> Value {
    json!({"name": name, "note": note, "solids": solids})
}

fn bool(op: Op, a: &Shape, b: &Shape) -> Shape {
    Shape::bool(op, a.clone(), b.clone())
}

/// The half-space on `n`'s side of the plane through `o`, as a block reaching well past `s`.
fn half_space(s: &Shape, o: Vec3, n: Vec3) -> Shape {
    let reach = oracle::bounds(s).map_or(100.0, |(lo, hi)| {
        let d = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt();
        let far = (0..3)
            .map(|k| (o[k] - 0.5 * (lo[k] + hi[k])).powi(2))
            .sum::<f64>()
            .sqrt();
        4.0 * d + 2.0 * far + 1.0
    });
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let z = n.map(|x| x / len);
    let x0 = if z[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let d = x0[0] * z[0] + x0[1] * z[1] + x0[2] * z[2];
    let x = [0, 1, 2].map(|k| x0[k] - d * z[k]);
    let xl = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let x = x.map(|v| v / xl);
    let y = [
        z[1] * x[2] - z[2] * x[1],
        z[2] * x[0] - z[0] * x[2],
        z[0] * x[1] - z[1] * x[0],
    ];
    let origin = [0, 1, 2].map(|k| o[k] - reach * (x[k] + y[k]));
    Shape::Prim(Primitive::Box {
        frame: Frame { origin, x, y, z },
        size: [2.0 * reach, 2.0 * reach, reach],
    })
}

/// The four booleans of `a` and `b`, behind each what of the operands it leaves out, and each alone.
fn booleans(a: &Shape, b: &Shape, operands_first: bool) -> Vec<Value> {
    // Each operand where it is not the result: in a union, nothing.
    let with = |s: Shape| {
        vec![
            solid("main", &s, None),
            ghost_as("ghost", a, &s),
            ghost_as("ghost2", b, &s),
        ]
    };
    let results = vec![
        view("a ∩ b", with(bool(Op::Intersection, a, b))),
        view("a ∪ b", with(bool(Op::Union, a, b))),
        view("a ∖ b", with(bool(Op::Difference, a, b))),
        view("b ∖ a", with(bool(Op::Difference, b, a))),
    ];
    let alone = vec![
        view("a", vec![solid("main", a, None)]),
        view("b", vec![solid("main", b, None)]),
    ];
    if operands_first {
        [alone, results].concat()
    } else {
        [results, alone].concat()
    }
}

/// A triangle's size for every solid of the case: a 160th of the box round its operands, the
/// parts of an assembly or a pattern, and their moved copies.
fn cell(a: &Shape, b: &Shape, c: &Case) -> f64 {
    let mut boxes: Vec<oracle::Bounds> = [a, b].iter().filter_map(|s| oracle::bounds(s)).collect();
    let placed = |bx: oracle::Bounds, m: &[[f64; 4]; 3]| -> oracle::Bounds {
        let corners: Vec<Vec3> = (0..8)
            .map(|n| {
                let p = [0, 1, 2].map(|k| if n >> k & 1 == 1 { bx.1[k] } else { bx.0[k] });
                [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
            })
            .collect();
        let lo = [0, 1, 2].map(|k| corners.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min));
        let hi = [0, 1, 2].map(|k| {
            corners
                .iter()
                .map(|p| p[k])
                .fold(f64::NEG_INFINITY, f64::max)
        });
        (lo, hi)
    };
    if let Some(m) = c.matrix.or(c.exact.mirror.as_ref().map(|m| m.matrix))
        && let Some(bx) = oracle::bounds(a)
    {
        boxes.push(placed(bx, &m));
    }
    if let Some(asm) = &c.exact.assembly {
        for (i, place) in &asm.parts {
            if let Some(bx) = oracle::bounds(&Shape::Prim(asm.shapes[*i].clone())) {
                boxes.push(placed(bx, place));
            }
        }
    }
    if let Some(m) = &c.exact.many {
        for p in m.target.iter().chain(&m.tools) {
            boxes.extend(oracle::bounds(&Shape::Prim(p.clone())));
        }
    }
    let lo = [0, 1, 2].map(|k| boxes.iter().map(|b| b.0[k]).fold(f64::INFINITY, f64::min));
    let hi = [0, 1, 2].map(|k| {
        boxes
            .iter()
            .map(|b| b.1[k])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let d = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt();
    if d.is_finite() && d > 0.0 {
        d / 160.0
    } else {
        0.05
    }
}

/// A prism's faces as thin plates, bottom, top, then the sides in profile order as the sewing
/// case writes them, each with how far to pull it out along its normal to show it apart.
fn loose_faces(s: &Shape) -> Option<Vec<(Shape, Vec3)>> {
    let Shape::Prim(Primitive::Extrude {
        frame: f,
        profile,
        height,
    }) = s
    else {
        return None;
    };
    let pts: Vec<[f64; 2]> = profile
        .loops
        .first()?
        .iter()
        .map(|seg| match seg {
            crate::profile::Seg::Line { a, .. } => Some(*a),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (lo, hi) = profile.bounds();
    let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(*height);
    let (thick, pull) = (0.02 * size, 0.18 * size);
    let world = |x: f64, y: f64, z: f64| {
        [0, 1, 2].map(|k| f.origin[k] + f.x[k] * x + f.y[k] * y + f.z[k] * z)
    };
    let along = |d: Vec3, t: f64| d.map(|x| x * t);
    let plate = |z: f64| {
        Shape::Prim(Primitive::Extrude {
            frame: Frame {
                origin: world(0.0, 0.0, z - 0.5 * thick),
                ..*f
            },
            profile: profile.clone(),
            height: thick,
        })
    };
    let mut out = vec![
        (plate(0.0), along(f.z, -pull)),
        (plate(*height), along(f.z, pull)),
    ];
    let n = pts.len();
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        let (a, b) = (world(p[0], p[1], 0.0), world(q[0], q[1], 0.0));
        let len = (0..3).map(|k| (b[k] - a[k]).powi(2)).sum::<f64>().sqrt();
        let x = [0, 1, 2].map(|k| (b[k] - a[k]) / len);
        let y = f.z;
        // Out of a counter-clockwise profile: the edge's direction across the axis.
        let z = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        let origin = [0, 1, 2].map(|k| a[k] - z[k] * 0.5 * thick);
        let side = Shape::Prim(Primitive::Box {
            frame: Frame { origin, x, y, z },
            size: [len, *height, thick],
        });
        out.push((side, along(z, pull)));
    }
    Some(out)
}

/// The views of a case, the first shown when it opens; the viewer moves on past a first view that
/// comes out empty.
/// A mixed corner case's L-prism with each named edge filleted the ordinary way on its own: each
/// convex top edge less its square strip less the rod rounding it, the concave upright edge with
/// its square filled less the cylinder rounding it. Only outside the case's zone is this the answer.
fn mixed_fillets(c: &Case, f: &crate::generate::Feature) -> Option<Shape> {
    if c.family != "fillet-mixed-corners" {
        return None;
    }
    let Shape::Prim(Primitive::Extrude {
        profile, height: h, ..
    }) = &c.a
    else {
        return None;
    };
    let pts: Vec<[f64; 2]> = profile.loops[0]
        .iter()
        .map(|s| match s {
            crate::profile::Seg::Line { a, .. } => Some(*a),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (a, t, b, r, h) = (pts[1][0], pts[2][1], pts[4][1], f.size, *h);
    let block = |lo: Vec3, hi: Vec3| Primitive::Box {
        frame: Frame::at(lo),
        size: [0, 1, 2].map(|k| hi[k] - lo[k]),
    };
    let rod = |at: Vec3, axis: Vec3, len: f64| Primitive::Cylinder {
        frame: Frame::along(at, axis),
        radius: r,
        height: len,
    };
    let along_x = bool(
        Op::Difference,
        &block([t, t - r, h - r], [a + 1.0, t + 1.0, h + 1.0]).into(),
        &rod([t, t - r, h - r], [1.0, 0.0, 0.0], a + 1.0 - t).into(),
    );
    let along_y = bool(
        Op::Difference,
        &block([t - r, t, h - r], [t + 1.0, b + 1.0, h + 1.0]).into(),
        &rod([t - r, t, h - r], [0.0, 1.0, 0.0], b + 1.0 - t).into(),
    );
    let fill = bool(
        Op::Difference,
        &block([t, t, 0.0], [t + r, t + r, h]).into(),
        &rod([t + r, t + r, -1.0], [0.0, 0.0, 1.0], h + 2.0).into(),
    );
    let cut = bool(
        Op::Difference,
        &bool(Op::Difference, &c.a, &along_x),
        &along_y,
    );
    Some(bool(Op::Union, &cut, &fill))
}

/// A box corner case's three edges at the far corner filleted the ordinary way, each at its own
/// radius: the box less, along each edge, the square strip there less the cylinder rounding it.
/// Inside the case's zone the corner is open, so only the fillets outside it are the answer.
fn corner_fillets(a: &Shape, f: &crate::generate::Feature) -> Option<Shape> {
    use crate::generate::Act;
    let Shape::Prim(Primitive::Box { size: w, .. }) = a else {
        return None;
    };
    let radii: Vec<f64> = match f.act {
        Act::FilletSetback | Act::FilletSetbackG2 => vec![f.size; 3],
        Act::FilletRadii if f.radii.len() == 3 => f.radii.clone(),
        _ => return None,
    };
    let mut s = a.clone();
    for (i, &r) in radii.iter().enumerate() {
        let (j, l) = ((i + 1) % 3, (i + 2) % 3);
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        (lo[i], hi[i]) = (-1.0, w[i] + 1.0);
        (lo[j], hi[j]) = (w[j] - r, w[j] + 1.0);
        (lo[l], hi[l]) = (w[l] - r, w[l] + 1.0);
        let strip = Primitive::Box {
            frame: Frame::at(lo),
            size: [0, 1, 2].map(|k| hi[k] - lo[k]),
        };
        let mut axis = [0.0; 3];
        axis[i] = 1.0;
        let mut at = [0.0; 3];
        (at[i], at[j], at[l]) = (-1.0, w[j] - r, w[l] - r);
        let rod = Primitive::Cylinder {
            frame: Frame::along(at, axis),
            radius: r,
            height: w[i] + 2.0,
        };
        s = bool(
            Op::Difference,
            &s,
            &bool(Op::Difference, &strip.into(), &rod.into()),
        );
    }
    Some(s)
}

pub fn views(c: &Case) -> Value {
    TABLE.with(|t| t.borrow_mut().clear());
    let (a, b) = (&c.a, &c.b);
    let mut extra = json!({});
    let v: Vec<Value> = if let Some(f) = &c.feature {
        // The points naming the edges or faces the act is asked of.
        extra["points"] = json!(f.points);
        if c.refuse {
            vec![noted(
                "input (refused)",
                "The kernel must refuse this act on the marked edges or faces.",
                vec![solid("main", a, None)],
            )]
        } else if let Some(sound) = &c.exact.sound {
            // No one shape is right: draw the limits. The zone where the result is left open as a
            // box, the points it must touch or leave as dots, and, where it is a shape, the least
            // result every acceptable one holds.
            if let Some(zone) = &sound.zone
                && let Some(bx) = oracle::bounds(&zone.clone().into())
            {
                extra["box"] = json!([bx.0, bx.1]);
            }
            let mut pts = f.points.clone();
            pts.extend(sound.contacts.iter().map(|c| c.0));
            pts.extend(sound.probes.iter().map(|p| p.point));
            pts.extend(&sound.away);
            extra["points"] = json!(pts);
            let clear = if sound.away.is_empty() {
                ""
            } else {
                ", and points just under each face that the corner must clear"
            };
            if let Some(fillets) = corner_fillets(a, f).or_else(|| mixed_fillets(c, f)) {
                return finish_views(
                    c,
                    extra,
                    vec![
                        noted(
                            "limits",
                            &format!(
                                "No one corner is expected. Outside the box the result must be these ordinary fillets exactly; inside it the corner is open, its volume between bounds. Dots mark the edges named, where the fillets must touch their faces{clear}."
                            ),
                            vec![solid("main", &fillets, None), ghost_outside(a, &fillets)],
                        ),
                        view("input", vec![solid("main", a, None)]),
                    ],
                );
            }
            vec![noted(
                "limits",
                &format!(
                    "No one shape is expected: inside the box the corner is open, outside it the result must be the ordinary fillet exactly, and its volume lies between bounds. Dots mark the edges named, where the fillets must touch their faces{clear}."
                ),
                vec![solid("main", a, None)],
            )]
        } else {
            vec![
                noted(
                    "after",
                    "The expected result, the input faint behind it; dots mark the edges or faces the act names.",
                    vec![solid("main", b, None), ghost_outside(a, b)],
                ),
                view("before", vec![solid("main", a, None)]),
            ]
        }
    } else if let Some(steps) = &c.exact.chain {
        use crate::generate::Step;
        let pts: Vec<Vec3> = steps
            .iter()
            .flat_map(|s| match s {
                Step::Act(f) => f.points.clone(),
                _ => Vec::new(),
            })
            .collect();
        extra["points"] = json!(pts);
        vec![
            noted(
                "after",
                "The expected result of every step, the starting solid faint behind it; dots mark the edges or faces the steps name.",
                vec![solid("main", b, None), ghost_outside(a, b)],
            ),
            view("before", vec![solid("main", a, None)]),
        ]
    } else if c.refuse && c.exact.loft.is_none() {
        // A constructor's input the contract rules out: the input itself, as the suite reads it.
        vec![noted(
            "input (refused)",
            "An input the contract rules out: the kernel must refuse it.",
            vec![solid("main", a, None)],
        )]
    } else if let Some(bx) = c.exact.bbox {
        extra["box"] = json!(bx);
        vec![view("solid", vec![solid("main", a, None)])]
    } else if let Some((o, n)) = c.exact.split {
        let half = half_space(a, o, n);
        extra["plane"] = json!({"origin": o, "normal": n});
        vec![
            noted(
                "pieces",
                "The two pieces the plane cuts the solid into, one in each tone.",
                vec![
                    solid("main", &bool(Op::Intersection, a, &half), None),
                    solid("alt", &bool(Op::Difference, a, &half), None),
                ],
            ),
            view("solid", vec![solid("main", a, None)]),
        ]
    } else if let Some(s) = &c.exact.section {
        let half = half_space(a, s.origin, s.normal);
        extra["plane"] = json!({"origin": s.origin, "normal": s.normal});
        let cut = bool(Op::Difference, a, &half);
        vec![
            noted(
                "cut",
                "The solid cut open at the plane: the section is the cut face's outline.",
                vec![solid("main", &cut, None), ghost_outside(a, &cut)],
            ),
            view("solid", vec![solid("main", a, None)]),
        ]
    } else if let Some(m) = c.matrix {
        vec![
            noted(
                "moved",
                "The solid moved by the case's matrix, the original faint behind it.",
                vec![solid("main", a, Some(m)), solid("ghost", a, None)],
            ),
            view("solid", vec![solid("main", a, None)]),
        ]
    } else if let Some(m) = &c.exact.mirror {
        vec![
            noted(
                "joined",
                "The part and its mirror image, which the kernel must join.",
                vec![solid("main", a, None), solid("alt", a, Some(m.matrix))],
            ),
            view("part", vec![solid("main", a, None)]),
        ]
    } else if let Some(m) = &c.exact.many {
        // Hundreds of tools nest too deep as a chain of booleans: the result goes in flat.
        let mut flat = json!({"kind": "many",
            "tools": m.tools.iter().map(Primitive::to_json).collect::<Vec<_>>()});
        if let Some(t) = &m.target {
            flat["target"] = t.to_json();
        }
        let result = TABLE.with(|t| {
            let mut t = t.borrow_mut();
            t.push((flat.to_string(), flat));
            t.len() - 1
        });
        let pieces: Vec<Value> = m
            .target
            .iter()
            .map(|t| solid("main", &Shape::Prim(t.clone()), None))
            .chain(
                m.tools
                    .iter()
                    .map(|t| solid("alt", &Shape::Prim(t.clone()), None)),
            )
            .collect();
        vec![
            view("result", vec![json!({"role": "main", "s": result})]),
            noted(
                "pieces",
                "Every solid of the pattern or the many-solid boolean, before they are combined.",
                pieces,
            ),
        ]
    } else if let Some(asm) = &c.exact.assembly {
        let pairs: Vec<[usize; 2]> = asm.clashes.iter().map(|x| [x.i, x.j]).collect();
        let parts: Vec<Value> = asm
            .parts
            .iter()
            .enumerate()
            .map(|(k, (i, place))| {
                let clashes = pairs.iter().any(|p| p.contains(&k));
                let role = if clashes { "alt" } else { "main" };
                solid(role, &Shape::Prim(asm.shapes[*i].clone()), Some(*place))
            })
            .collect();
        extra["clashes"] = json!(pairs);
        vec![noted(
            "parts",
            "The parts at their placements; those in the second tone clash with another, which the kernel must find.",
            parts,
        )]
    } else if let Some(l) = &c.exact.loft {
        // The smooth loft has no closed form to draw: its sections, each a thin plate where it
        // lies.
        let thick = 0.01
            * l.profiles
                .iter()
                .map(|p| {
                    let (lo, hi) = p.bounds();
                    (hi[0] - lo[0]).max(hi[1] - lo[1])
                })
                .fold(0.0, f64::max);
        let plates: Vec<Value> = l
            .profiles
            .iter()
            .zip(&l.heights)
            .map(|(p, &h)| {
                let f = &l.frame;
                let frame = Frame {
                    origin: [0, 1, 2].map(|k| f.origin[k] + f.z[k] * (h - 0.5 * thick)),
                    ..*f
                };
                solid(
                    "main",
                    &Shape::Prim(Primitive::Extrude {
                        frame,
                        profile: p.clone(),
                        height: thick,
                    }),
                    None,
                )
            })
            .collect();
        vec![noted(
            "sections",
            "The sections the loft must pass through, in order; the smooth loft has no closed form to draw.",
            plates,
        )]
    } else if let Some(cv) = &c.exact.curvature {
        extra["points"] = json!(cv.points);
        vec![noted(
            "solid",
            "Dots mark the points: each asks for the principal curvatures where the boundary lies nearest it.",
            vec![solid("main", a, None)],
        )]
    } else if let Some(dr) = &c.exact.drawing {
        // Each stretch of the drawing as points along it, open looking along its direction.
        let lines: Vec<Value> = dr
            .pieces
            .iter()
            .filter(|p| p.t1 > p.t0)
            .map(|p| {
                let n = match p.curve {
                    crate::generate::Curve3::Line { .. } => 1,
                    crate::generate::Curve3::Circle { .. } => {
                        (96.0 * (p.t1 - p.t0)).ceil().max(4.0) as usize
                    }
                };
                let pts: Vec<Vec3> = (0..=n)
                    .map(|j| p.curve.at(p.t0 + (p.t1 - p.t0) * j as f64 / n as f64))
                    .collect();
                json!({"seen": p.visible, "pts": pts})
            })
            .collect();
        extra["lines"] = Value::Array(lines);
        extra["look"] = json!(dr.direction.map(|x| -x));
        vec![noted(
            "drawing",
            "Seen along the case's direction: the edges and outlines the viewer sees in blue, those the solid hides dashed.",
            vec![solid("main", a, None)],
        )]
    } else if let Some(n) = &c.exact.normals {
        let s = match n.op {
            Some(op) => bool(op, a, b),
            None => a.clone(),
        };
        vec![view("solid", vec![solid("main", &s, None)])]
    } else if let (Some(sew), Some(faces)) = (&c.exact.sew, loose_faces(a)) {
        // Each face as a thin plate pulled out from the prism along its normal, as an exploded
        // drawing would; the faces written inside out in the second tone.
        let plates = faces
            .into_iter()
            .enumerate()
            .map(|(k, (plate, out))| {
                let role = if sew.flipped.get(k).copied().unwrap_or(false) {
                    "alt"
                } else {
                    "main"
                };
                let m = [0, 1, 2].map(|r| {
                    let mut row = [0.0; 4];
                    row[r] = 1.0;
                    row[3] = out[r];
                    row
                });
                solid(role, &plate, Some(m))
            })
            .collect();
        vec![
            noted(
                "loose faces",
                "Each face is written on its own, apart from its neighbours by up to 0.0001 mm; faces in the second tone are written inside out. Drawn pulled apart.",
                plates,
            ),
            noted(
                "sewn",
                "The solid the kernel must sew the faces into.",
                vec![solid("main", a, None)],
            ),
        ]
    } else if !c.exact.sides.is_empty() {
        vec![noted(
            "solid",
            "Each probe is a point on the surface and, where the surface has a normal, points 0.01 mm inside and outside it, drawn spread apart; the kernel must say which side each is on.",
            vec![solid("main", a, None)],
        )]
    } else if c.step || c.exact.sew.is_some() || a == b {
        vec![view("solid", vec![solid("main", a, None)])]
    } else {
        booleans(a, b, c.test.starts_with("constructions/"))
    };
    finish_views(c, extra, v)
}

fn finish_views(c: &Case, mut extra: Value, v: Vec<Value>) -> Value {
    // The answers a point query is checked against, laid over whichever view the case draws.
    let e = &c.exact;
    if !e.closest.is_empty() {
        let at = |(p, d, foot): &(Vec3, f64, Option<Vec3>)| json!({"at": p, "distance": d, "foot": foot});
        extra["closest"] = e.closest.iter().map(at).collect();
    }
    if !e.sides.is_empty() {
        let at = |(p, side): &(Vec3, crate::model::Side)| json!({"at": p, "side": side.word()});
        extra["sides"] = e.sides.iter().map(at).collect();
    }
    if let Some(n) = &e.normals {
        let at = |(p, normal): &(Vec3, Option<Vec3>)| json!({"at": p, "n": normal});
        extra["normals"] = n.points.iter().map(at).collect();
    }
    extra["cell"] = json!(cell(&c.a, &c.b, c));
    extra["views"] = Value::Array(v);
    extra["shapes"] =
        TABLE.with(|t| Value::Array(t.borrow_mut().drain(..).map(|(_, v)| v).collect()));
    extra
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_lay_over_every_query_answer_the_case_holds() {
        let mut seen = [0; 4];
        for c in crate::generate::cases(1) {
            let spec = views(&c);
            let e = &c.exact;
            if !e.closest.is_empty() {
                seen[0] += 1;
                let got = spec["closest"].as_array().expect(&c.id);
                assert_eq!(got.len(), e.closest.len(), "{}", c.id);
                for (g, (at, d, foot)) in got.iter().zip(&e.closest) {
                    assert_eq!(g["at"], json!(at), "{}", c.id);
                    assert_eq!(g["distance"], json!(d), "{}", c.id);
                    assert_eq!(g["foot"], json!(foot), "{}", c.id);
                }
            }
            if !e.sides.is_empty() {
                seen[1] += 1;
                let got = spec["sides"].as_array().expect(&c.id);
                assert_eq!(got.len(), e.sides.len(), "{}", c.id);
                for (g, (at, side)) in got.iter().zip(&e.sides) {
                    assert_eq!(g["at"], json!(at), "{}", c.id);
                    assert_eq!(g["side"], side.word(), "{}", c.id);
                }
            }
            if let Some(n) = &e.normals {
                seen[2] += 1;
                let got = spec["normals"].as_array().expect(&c.id);
                assert_eq!(got.len(), n.points.len(), "{}", c.id);
                for (g, (at, normal)) in got.iter().zip(&n.points) {
                    assert_eq!(g["at"], json!(at), "{}", c.id);
                    assert_eq!(g["n"], json!(normal), "{}", c.id);
                }
            }
            if let Some(asm) = &e.assembly {
                seen[3] += 1;
                let pairs: Vec<[usize; 2]> = asm.clashes.iter().map(|x| [x.i, x.j]).collect();
                assert_eq!(spec["clashes"], json!(pairs), "{}", c.id);
                // Only a part that clashes with another is drawn in the second tone.
                let parts = spec["views"][0]["solids"].as_array().unwrap();
                for (k, s) in parts.iter().enumerate() {
                    let clashes = pairs.iter().any(|p| p.contains(&k));
                    assert_eq!(
                        s["role"],
                        if clashes { "alt" } else { "main" },
                        "{} {k}",
                        c.id
                    );
                }
            }
        }
        assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
    }
}
