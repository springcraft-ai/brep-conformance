//! Operations applied to what earlier operations made: fillets on edges a union or cut left, blends
//! beside other blends, edits after blends, and a round trip through STEP after all of it. Each
//! reaches a solid another family already has the closed form of, by a longer road.

use super::*;
use crate::portable::Portable;

fn drawn(base: Shape, want: Shape, steps: Vec<Step>, volume: f64, area: Option<f64>) -> Drawn {
    let exact = Exact {
        result_volume: Some(volume),
        result_area: area,
        chain: Some(steps),
        ..Default::default()
    };
    (base, want, exact)
}

fn fillet(points: Vec<Vec3>, size: f64) -> Step {
    Step::Act(Feature {
        act: Act::Fillet,
        points,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    })
}

fn act_of(act: Act, points: Vec<Vec3>, size: f64) -> Step {
    Step::Act(Feature {
        act,
        points,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    })
}

fn prism(pts: &[[f64; 2]], h: f64) -> Primitive {
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(pts),
        height: h,
    }
}

fn rounded_prism(pts: &[[f64; 2]], radii: &[f64], h: f64) -> Primitive {
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![rounded(pts, radii)],
        },
        height: h,
    }
}

/// A radius for each chosen corner that keeps its round within 0.45 of both its sides.
fn fitting_radius(pts: &[[f64; 2]], i: usize, f: f64) -> f64 {
    let (turn, side) = corners(pts)[i];
    0.45 * side * f / (turn.abs() / 2.0).ptan()
}

/// An L-prism made as the union of its two arm boxes, every convex edge then filleted: the edges
/// along the seam the union leaves must be single edges. The result is `opened-l-prisms`'s.
pub(super) fn after_union_l(r: &mut Rng) -> Drawn {
    let (base, f, want, exact) = opened_l_prism(r);
    let Primitive::Extrude {
        profile, height: h, ..
    } = &base
    else {
        unreachable!("an L-prism is an extrusion")
    };
    let p: Vec<[f64; 2]> = profile.loops[0]
        .iter()
        .filter_map(|s| match s {
            Seg::Line { a, .. } => Some(*a),
            _ => None,
        })
        .collect();
    let (w, t, d) = (p[1][0], p[2][1], p[4][1]);
    let arms = Shape::bool(
        Op::Union,
        axis_box([0.0; 3], [w, t, *h]),
        axis_box([0.0; 3], [t, d, *h]),
    );
    drawn(
        arms,
        want,
        vec![Step::Act(f)],
        exact.result_volume.unwrap_or(0.0),
        exact.result_area,
    )
}

/// A plate drilled by a cut, then the hole's rim filleted or chamfered where it meets the top: the
/// rim edge and the hole's face are what the cut made. The blend takes, by Pappus, its section's
/// area round the circle its centroid runs.
pub(super) fn rim_after_cut(r: &mut Rng) -> Drawn {
    let blend = if r.next_u64().is_multiple_of(2) {
        Act::Fillet
    } else {
        Act::Chamfer
    };
    let (w, d, t) = (r.range(10.0, 16.0), r.range(10.0, 16.0), r.range(3.0, 6.0));
    let rad = r.range(1.5, 3.5);
    let c = [w * r.range(0.4, 0.6), d * r.range(0.4, 0.6)];
    let size = t.min(rad) * r.range(0.15, 0.45);
    let plate = axis_box([0.0; 3], [w, d, t]);
    let hole = Primitive::Cylinder {
        frame: Frame::at([c[0], c[1], -1.0]),
        radius: rad,
        height: t + 2.0,
    };
    let drilled = Shape::bool(Op::Difference, plate.clone(), hole);
    let (cut, at, face, section) = rim_blend(blend, rad, t, size);
    let tool = Primitive::Revolve {
        frame: Frame::at([c[0], c[1], 0.0]),
        profile: section,
        angle: TAU,
    };
    let v0 = w * d * t - PI * rad * rad * t;
    let s0 = 2.0 * (w * d - PI * rad * rad) + 2.0 * (w + d) * t + TAU * rad * t;
    let volume = v0 - TAU * at * cut;
    let ring = PI * ((rad + size).ppowi(2) - rad * rad);
    let area = s0 - ring - TAU * rad * size + face;
    let rim = r.range(0.5, 2.5);
    let step = act_of(
        blend,
        vec![[c[0] + rad * rim.pcos(), c[1] + rad * rim.psin(), t]],
        size,
    );
    drawn(
        drilled.clone(),
        Shape::bool(Op::Difference, drilled, tool),
        vec![step],
        volume,
        Some(area),
    )
}

/// What a fillet or chamfer of `size` takes from a hole's rim of radius `rad` at the top of a plate
/// `t` thick: its section's area, the distance of the section's centroid from the axis, the area of
/// the blend's own face, and the section as a profile to revolve.
fn rim_blend(blend: Act, rad: f64, t: f64, size: f64) -> (f64, f64, f64, Profile) {
    let spandrel_x = |s: f64| s * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI);
    match blend {
        Act::Fillet => (
            (1.0 - PI / 4.0) * size * size,
            rad + spandrel_x(size),
            TAU * size * ((rad + size) * PI / 2.0 - size),
            spandrel(rad, t, size, 1.0, -1.0),
        ),
        _ => (
            size * size / 2.0,
            rad + size / 3.0,
            TAU * (rad + size / 2.0) * size * 2f64.sqrt(),
            corner_triangle(rad, t, size, 1.0, -1.0),
        ),
    }
}

/// A plate drilled by a row of cuts, every rim then filleted in one call: each hole's rim takes what
/// one alone does, the holes far enough apart that their blends never meet.
pub(super) fn rims_of_a_pattern(r: &mut Rng) -> Drawn {
    let n = 2 + (r.next_u64() % 5) as usize;
    let pitch = r.range(5.0, 8.0);
    let (w, d, t) = (pitch * n as f64, r.range(6.0, 10.0), r.range(3.0, 6.0));
    let rad = pitch * r.range(0.15, 0.3);
    let size = t.min(rad) * r.range(0.15, 0.45);
    let mut drilled: Shape = axis_box([0.0; 3], [w, d, t]).into();
    let mut want_tools = Vec::new();
    let mut points = Vec::new();
    let (cut, at, face, section) = rim_blend(Act::Fillet, rad, t, size);
    for i in 0..n {
        let c = [pitch * (i as f64 + 0.5), d / 2.0];
        drilled = Shape::bool(
            Op::Difference,
            drilled,
            Primitive::Cylinder {
                frame: Frame::at([c[0], c[1], -1.0]),
                radius: rad,
                height: t + 2.0,
            },
        );
        want_tools.push(Primitive::Revolve {
            frame: Frame::at([c[0], c[1], 0.0]),
            profile: section.clone(),
            angle: TAU,
        });
        points.push([c[0] + rad, c[1], t]);
    }
    let mut want = drilled.clone();
    for tool in want_tools {
        want = Shape::bool(Op::Difference, want, tool);
    }
    let nf = n as f64;
    let v0 = w * d * t - nf * PI * rad * rad * t;
    let s0 = 2.0 * (w * d - nf * PI * rad * rad) + 2.0 * (w + d) * t + nf * TAU * rad * t;
    let ring = PI * ((rad + size).ppowi(2) - rad * rad);
    let volume = v0 - nf * TAU * at * cut;
    let area = s0 + nf * (face - ring - TAU * rad * size);
    drawn(
        drilled,
        want,
        vec![fillet(points, size)],
        volume,
        Some(area),
    )
}

/// A sharp prism whose upright edges are filleted by ρ in one call, then its top loop, now one
/// tangent chain, by a smaller r in another: `fillet-chains`' solid, reached in two calls.
pub(super) fn top_loop_after_corners(r: &mut Rng) -> Drawn {
    let (_, f, want, exact) = fillet_chain(r);
    let Some(Shape::Prim(Primitive::Extrude {
        profile, height: h, ..
    })) = Some(&want).and_then(|s| first_leaf(s))
    else {
        unreachable!("a fillet chain's first leaf is its prism")
    };
    let rho = profile.loops[0]
        .iter()
        .find_map(|s| match s {
            Seg::Arc { radius, .. } => Some(*radius),
            _ => None,
        })
        .unwrap_or(0.0);
    // The sharp polygon the rounded one came from: each pair of lines' extensions meet at a corner.
    let lines: Vec<([f64; 2], [f64; 2])> = profile.loops[0]
        .iter()
        .filter_map(|s| match s {
            Seg::Line { a, b } if (b[0] - a[0]).phypot(b[1] - a[1]) > 1e-12 => Some((*a, *b)),
            _ => None,
        })
        .collect();
    let m = lines.len();
    let pts: Vec<[f64; 2]> = (0..m)
        .map(|i| {
            let ((p, q), (u, v)) = (lines[(i + m - 1) % m], lines[i]);
            let (d1, d2) = ([q[0] - p[0], q[1] - p[1]], [v[0] - u[0], v[1] - u[1]]);
            let den = d1[0] * d2[1] - d1[1] * d2[0];
            let s = ((u[0] - p[0]) * d2[1] - (u[1] - p[1]) * d2[0]) / den;
            [p[0] + s * d1[0], p[1] + s * d1[1]]
        })
        .collect();
    let height = *h + f.size;
    let corners: Vec<Vec3> = pts.iter().map(|p| [p[0], p[1], height / 2.0]).collect();
    drawn(
        prism(&pts, height).into(),
        want,
        vec![fillet(corners, rho), Step::Act(f)],
        exact.result_volume.unwrap_or(0.0),
        exact.result_area,
    )
}

fn first_leaf(s: &Shape) -> Option<&Shape> {
    match s {
        Shape::Prim(_) => Some(s),
        Shape::Bool(_, a, _) => first_leaf(a),
    }
}

/// A prism with one upright edge chamfered, then the two edges the chamfer made filleted: the prism
/// on the chamfered profile with those two corners rounded.
pub(super) fn edges_of_a_chamfer(r: &mut Rng) -> Drawn {
    let pts = convex_polygon(r);
    let n = pts.len();
    let i = (r.next_u64() % n as u64) as usize;
    let c = 0.3 * corners(&pts)[i].1 * r.range(0.3, 1.0);
    let h = r.range(2.0, 8.0);
    let mut cuts = vec![0.0; n];
    cuts[i] = c;
    let cut_pts: Vec<[f64; 2]> = chamfered(&pts, &cuts)
        .iter()
        .filter_map(|s| match s {
            Seg::Line { a, b } if (b[0] - a[0]).phypot(b[1] - a[1]) > 0.0 => Some(*a),
            _ => None,
        })
        .collect();
    let m = cut_pts.len();
    // The chamfer's two ends: the corners of the new profile nearest the old corner.
    let mut near: Vec<usize> = (0..m).collect();
    let dist = |j: usize| (cut_pts[j][0] - pts[i][0]).phypot(cut_pts[j][1] - pts[i][1]);
    near.sort_by(|&a, &b| dist(a).total_cmp(&dist(b)));
    let ends = [near[0], near[1]];
    let f = r.range(0.2, 1.0);
    let rad = fitting_radius(&cut_pts, ends[0], f).min(fitting_radius(&cut_pts, ends[1], f));
    let mut radii = vec![0.0; m];
    radii[ends[0]] = rad;
    radii[ends[1]] = rad;
    let z = h * r.range(0.3, 0.7);
    let want = rounded_prism(&cut_pts, &radii, h);
    let steps = vec![
        act_of(Act::Chamfer, vec![[pts[i][0], pts[i][1], z]], c),
        fillet(
            ends.iter()
                .map(|&j| [cut_pts[j][0], cut_pts[j][1], z])
                .collect(),
            rad,
        ),
    ];
    let v = want.volume();
    let a = want.area();
    drawn(prism(&pts, h).into(), want.into(), steps, v, Some(a))
}

/// A prism with some upright edges filleted, then others chamfered beside them: the prism on the
/// profile with each corner rounded or cut as it was blended.
pub(super) fn chamfers_beside_fillets(r: &mut Rng) -> Drawn {
    let pts = convex_polygon(r);
    let n = pts.len();
    let h = r.range(2.0, 8.0);
    let z = h * r.range(0.3, 0.7);
    // Alternate corners: rounded, then cut; every blend within 0.45 of its sides.
    let rad = (0..n)
        .step_by(2)
        .map(|i| fitting_radius(&pts, i, 0.8))
        .fold(f64::INFINITY, f64::min)
        * r.range(0.3, 1.0);
    let cut = (1..n)
        .step_by(2)
        .map(|i| 0.45 * corners(&pts)[i].1)
        .fold(f64::INFINITY, f64::min)
        * r.range(0.3, 1.0);
    let mut cuts = vec![0.0; n];
    let mut radii_at = vec![0.0; n];
    for i in 0..n {
        if i % 2 == 0 {
            radii_at[i] = rad;
        } else {
            cuts[i] = cut;
        }
    }
    // The cut corners become two points each; the rounded corners keep their radius.
    let mut new_pts = Vec::new();
    let mut radii = Vec::new();
    for i in 0..n {
        if cuts[i] > 0.0 {
            let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let toward = |q: [f64; 2]| {
                let l = (q[0] - p[0]).phypot(q[1] - p[1]);
                [
                    p[0] + (q[0] - p[0]) * cut / l,
                    p[1] + (q[1] - p[1]) * cut / l,
                ]
            };
            new_pts.push(toward(prev));
            new_pts.push(toward(next));
            radii.extend([0.0, 0.0]);
        } else {
            new_pts.push(pts[i]);
            radii.push(radii_at[i]);
        }
    }
    let want = rounded_prism(&new_pts, &radii, h);
    let round_pts: Vec<Vec3> = (0..n)
        .filter(|i| i % 2 == 0)
        .map(|i| [pts[i][0], pts[i][1], z])
        .collect();
    let cut_pts: Vec<Vec3> = (0..n)
        .filter(|i| i % 2 == 1)
        .map(|i| [pts[i][0], pts[i][1], z])
        .collect();
    let steps = vec![fillet(round_pts, rad), act_of(Act::Chamfer, cut_pts, cut)];
    let (v, a) = (want.volume(), want.area());
    drawn(prism(&pts, h).into(), want.into(), steps, v, Some(a))
}

/// A prism with half its upright edges filleted in one call and the rest in another: the same
/// solid as one call rounding them all.
pub(super) fn two_calls_as_one(r: &mut Rng) -> Drawn {
    let pts = convex_polygon(r);
    let n = pts.len();
    let h = r.range(2.0, 8.0);
    let z = h * r.range(0.3, 0.7);
    let rad = (0..n)
        .map(|i| fitting_radius(&pts, i, 0.9))
        .fold(f64::INFINITY, f64::min)
        * r.range(0.3, 1.0);
    let want = rounded_prism(&pts, &vec![rad; n], h);
    let at = |i: usize| [pts[i][0], pts[i][1], z];
    let steps = vec![
        fillet((0..n).filter(|i| i % 2 == 0).map(at).collect(), rad),
        fillet((0..n).filter(|i| i % 2 == 1).map(at).collect(), rad),
    ];
    let (v, a) = (want.volume(), want.area());
    drawn(prism(&pts, h).into(), want.into(), steps, v, Some(a))
}

/// A box joined to its mirror image across one end, then an edge running across the join filleted:
/// the joined box's edge must be one edge, so the whole of it rounds, not the half the point is on.
pub(super) fn across_a_mirror_join(r: &mut Rng) -> Drawn {
    let (a, b, c) = (r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0));
    let rad = b.min(c) * r.range(0.1, 0.4);
    let mirror = [
        [-1.0, 0.0, 0.0, 2.0 * a],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let len = 2.0 * a;
    let strip = axis_box([-1.0, b - rad, c - rad], [len + 1.0, b + 1.0, c + 1.0]);
    let rod = Primitive::Cylinder {
        frame: Frame::along([-1.0, b - rad, c - rad], [1.0, 0.0, 0.0]),
        radius: rad,
        height: len + 2.0,
    };
    let want = Shape::bool(
        Op::Difference,
        axis_box([0.0; 3], [len, b, c]),
        Shape::bool(Op::Difference, strip, rod),
    );
    let k = 1.0 - PI / 4.0;
    let volume = len * b * c - k * rad * rad * len;
    let area = 2.0 * (len * b + b * c + c * len) - 2.0 * rad * len + PI / 2.0 * rad * len
        - 2.0 * k * rad * rad;
    let steps = vec![
        Step::MirrorJoin(mirror),
        fillet(vec![[a * r.range(1.1, 1.9), b, c]], rad),
    ];
    drawn(
        axis_box([0.0; 3], [a, b, c]).into(),
        want,
        steps,
        volume,
        Some(area),
    )
}

/// A rectangular block, its upright edges filleted, then hollowed open at the top: `shelled-rounds`'
/// solid, with its rounds made by the fillet instead of given.
pub(super) fn shell_after_fillet(r: &mut Rng) -> Drawn {
    let (_, f, want, exact) = shelled_round(r);
    let Some(Shape::Prim(Primitive::Extrude {
        profile, height: h, ..
    })) = first_leaf(&want).cloned()
    else {
        unreachable!("a shelled round's first leaf is its block")
    };
    let rho = profile.loops[0]
        .iter()
        .find_map(|s| match s {
            Seg::Arc { radius, .. } => Some(*radius),
            _ => None,
        })
        .unwrap_or(0.0);
    let (lo, hi) = profile.bounds();
    let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
    let mut steps = Vec::new();
    if rho > 0.0 {
        let corners = vec![
            [0.0, 0.0, h / 2.0],
            [w, 0.0, h / 2.0],
            [w, d, h / 2.0],
            [0.0, d, h / 2.0],
        ];
        steps.push(fillet(corners, rho));
    }
    steps.push(Step::Act(f));
    drawn(
        axis_box([0.0; 3], [w, d, h]).into(),
        want,
        steps,
        exact.result_volume.unwrap_or(0.0),
        exact.result_area,
    )
}

/// A block drilled through by a cut, then offset outward, below or past the hole's radius:
/// `offset-past-holes`' solids, the hole made by a boolean instead of the profile.
pub(super) fn offset_after_cut(r: &mut Rng) -> Drawn {
    let (base, f, want, exact) = super::hard::offset_past_hole(r);
    let Primitive::Extrude {
        profile, height: c, ..
    } = &base
    else {
        unreachable!("a drilled block is an extrusion")
    };
    let (lo, hi) = profile.loops[0].iter().fold(
        ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
        |(lo, hi), s| match s {
            Seg::Line { a, .. } => (
                [lo[0].min(a[0]), lo[1].min(a[1])],
                [hi[0].max(a[0]), hi[1].max(a[1])],
            ),
            _ => (lo, hi),
        },
    );
    let Seg::Arc { center, radius, .. } = profile.loops[1][0] else {
        unreachable!("a drilled block's hole is a circle")
    };
    let drilled = Shape::bool(
        Op::Difference,
        axis_box([lo[0], lo[1], 0.0], [hi[0], hi[1], *c]),
        Primitive::Cylinder {
            frame: Frame::at([center[0], center[1], -1.0]),
            radius,
            height: c + 2.0,
        },
    );
    drawn(
        drilled,
        want,
        vec![Step::Act(f)],
        exact.result_volume.unwrap_or(0.0),
        exact.result_area,
    )
}

/// A box or ball offset by a, then by b: the same as offsetting it once by a + b.
pub(super) fn offset_twice(r: &mut Rng) -> Drawn {
    let (a, b) = (r.range(0.2, 1.5), r.range(0.2, 1.5));
    let offset = |d: f64| act_of(Act::Offset, Vec::new(), d);
    if r.next_u64().is_multiple_of(2) {
        let w = [r.range(2.0, 8.0), r.range(2.0, 8.0), r.range(2.0, 8.0)];
        let (want, v, s) = rounded_block([0.0; 3], w, a + b);
        drawn(
            axis_box([0.0; 3], w).into(),
            want,
            vec![offset(a), offset(b)],
            v,
            Some(s),
        )
    } else {
        let rad = r.range(1.0, 5.0);
        let (big, all) = (rad + a + b, sphere([0.0; 3], rad + a + b));
        drawn(
            sphere([0.0; 3], rad).into(),
            all.into(),
            vec![offset(a), offset(b)],
            4.0 / 3.0 * PI * big.ppowi(3),
            Some(4.0 * PI * big * big),
        )
    }
}

/// Any primitive moved by a rigid motion and then by its inverse: the solid it started as.
pub(super) fn move_and_back(r: &mut Rng) -> Drawn {
    let p = any_primitive(r);
    let m = Rigid::uniform(
        [r.unit(), r.unit(), r.unit()],
        [
            r.range(-20.0, 20.0),
            r.range(-20.0, 20.0),
            r.range(-20.0, 20.0),
        ],
    );
    let there = [0, 1, 2].map(|i| [m.r[i][0], m.r[i][1], m.r[i][2], m.t[i]]);
    // The inverse: Rᵀ, and −Rᵀt.
    let back = [0, 1, 2].map(|i| {
        let t = -(0..3).map(|k| m.r[k][i] * m.t[k]).sum::<f64>();
        [m.r[0][i], m.r[1][i], m.r[2][i], t]
    });
    let (v, a) = (p.volume(), p.area());
    drawn(
        p.clone().into(),
        p.into(),
        vec![Step::Move(there), Step::Move(back)],
        v,
        Some(a),
    )
}

/// A prism with some upright edges filleted, then cut by a plane through the fillets: the rounded
/// prism, shorter.
pub(super) fn cut_through_fillets(r: &mut Rng) -> Drawn {
    let pts = convex_polygon(r);
    let n = pts.len();
    let h = r.range(4.0, 10.0);
    let rad = (0..n)
        .map(|i| fitting_radius(&pts, i, 0.9))
        .fold(f64::INFINITY, f64::min)
        * r.range(0.3, 1.0);
    let keep = h * r.range(0.3, 0.8);
    let want = rounded_prism(&pts, &vec![rad; n], keep);
    let (lo, hi) = polygon(&pts).bounds();
    let above = axis_box(
        [lo[0] - 1.0, lo[1] - 1.0, keep],
        [hi[0] + 1.0, hi[1] + 1.0, h + 1.0],
    );
    let z = h * r.range(0.1, 0.25);
    let steps = vec![
        fillet(pts.iter().map(|p| [p[0], p[1], z]).collect(), rad),
        Step::Cut(above.into()),
    ];
    let (v, a) = (want.volume(), want.area());
    drawn(prism(&pts, h).into(), want.into(), steps, v, Some(a))
}

/// A prism filleted, written as STEP and read back: the rounded prism, through exchange.
pub(super) fn round_trip_after_chain(r: &mut Rng) -> Drawn {
    let pts = convex_polygon(r);
    let n = pts.len();
    let h = r.range(2.0, 8.0);
    let rad = (0..n)
        .map(|i| fitting_radius(&pts, i, 0.9))
        .fold(f64::INFINITY, f64::min)
        * r.range(0.3, 1.0);
    let want = rounded_prism(&pts, &vec![rad; n], h);
    let z = h * r.range(0.3, 0.7);
    let steps = vec![
        fillet(pts.iter().map(|p| [p[0], p[1], z]).collect(), rad),
        Step::StepRoundTrip,
    ];
    let (v, a) = (want.volume(), want.area());
    drawn(prism(&pts, h).into(), want.into(), steps, v, Some(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chain_s_closed_form_meets_a_sample_of_its_shape() {
        let mut r = Rng::new(31);
        let draws: [fn(&mut Rng) -> Drawn; 14] = [
            after_union_l,
            rim_after_cut,
            rims_of_a_pattern,
            top_loop_after_corners,
            edges_of_a_chamfer,
            chamfers_beside_fillets,
            two_calls_as_one,
            across_a_mirror_join,
            shell_after_fillet,
            offset_after_cut,
            offset_twice,
            move_and_back,
            cut_through_fillets,
            round_trip_after_chain,
        ];
        for (k, draw) in draws.iter().enumerate() {
            for _ in 0..3 {
                let (_, want, exact) = draw(&mut r);
                let v = exact.result_volume.unwrap();
                let (lo, hi) = crate::oracle::bounds(&want).unwrap();
                let n = 40_000;
                let held = crate::oracle::samples((lo, hi), n)
                    .iter()
                    .filter(|p| crate::oracle::contains(&want, **p))
                    .count();
                let got = held as f64 / n as f64 * (0..3).map(|i| hi[i] - lo[i]).product::<f64>();
                assert!((got - v).abs() < 0.02 * v, "chain {k}: {got} against {v}");
            }
        }
    }
}
