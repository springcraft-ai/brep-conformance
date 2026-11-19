//! Blends past the rolling ball: conic and curvature-continuous fillets, corners whose fillets stop
//! short and close with one patch, a fillet between two faces that need not meet, and a full round.

use super::*;
use crate::portable::Portable;
use crate::profile::{conic_rounded, gn_center_radius, gn_equal_area_w, gn_peaked_w, gn_rounded};

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = v[0].phypot(v[1]);
    [v[0] / l, v[1] / l]
}

/// The point `d` from `p` toward `q`.
fn toward(p: [f64; 2], q: [f64; 2], d: f64) -> [f64; 2] {
    let u = unit2([q[0] - p[0], q[1] - p[1]]);
    [p[0] + d * u[0], p[1] + d * u[1]]
}

fn prism(pts: &[[f64; 2]], height: f64) -> Primitive {
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(pts),
        height,
    }
}

fn act(act: Act, points: Vec<Vec3>, size: f64, radii: Vec<f64>) -> Feature {
    Feature {
        act,
        points,
        size,
        pull: None,
        radii,
        matrix: None,
    }
}

/// A convex prism with some of its side edges blended by conics meeting each face `d` from the
/// edge, shoulder `rho` of the way out: the prism on the profile with those corners cut by the
/// rational quadratic arcs through the two tangent points and the corner, the middle weight
/// `rho/(1 − rho)`.
pub(super) fn conic_prism(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let mut chosen: Vec<usize> = (0..n).filter(|_| r.next_u64().is_multiple_of(2)).collect();
    if chosen.is_empty() {
        chosen.push((r.next_u64() % n as u64) as usize);
    }
    let c = corners(&pts);
    let most = chosen
        .iter()
        .map(|&i| 0.45 * c[i].1)
        .fold(f64::INFINITY, f64::min);
    let d = most * r.range(0.2, 1.0);
    let rho = r.range(0.25, 0.75);
    let height = r.range(2.0, 10.0);
    let sizes: Vec<f64> = (0..n)
        .map(|i| if chosen.contains(&i) { d } else { 0.0 })
        .collect();
    let edges = chosen
        .iter()
        .map(|&i| [pts[i][0], pts[i][1], height * r.range(0.2, 0.8)])
        .collect();
    let want = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![conic_rounded(&pts, &sizes, rho)],
        },
        height,
    };
    let feature = act(Act::FilletConic, edges, d, vec![rho]);
    (prism(&pts, height), feature, want.into(), Exact::default())
}

/// A convex prism with some of its side edges blended by the curvature-continuous section of
/// order 2 at the equal-area thumbweight: the prism on the profile with those corners cut by the
/// section between the contacts a ball of `rad` makes, which takes off what the ball's round
/// does. Just inside each contact the blend is as flat as the face it leaves.
pub(super) fn g2_prism(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let mut chosen: Vec<usize> = (0..n).filter(|_| r.next_u64().is_multiple_of(2)).collect();
    if chosen.is_empty() {
        chosen.push((r.next_u64() % n as u64) as usize);
    }
    let c = corners(&pts);
    // A ball of radius r in a corner of interior angle α touches each side r/tan(α/2) from it.
    let half = |i: usize| (PI - c[i].0.abs()) / 2.0;
    let most = chosen
        .iter()
        .map(|&i| 0.45 * c[i].1 * half(i).ptan())
        .fold(f64::INFINITY, f64::min);
    let rad = most * r.range(0.2, 1.0);
    let height = r.range(2.0, 10.0);
    let sizes: Vec<f64> = (0..n)
        .map(|i| if chosen.contains(&i) { rad } else { 0.0 })
        .collect();
    // One center radius for every chosen corner, as one call gives: a third of the cases the
    // equal-area section's at the first corner; a third one that curves most at its middle at
    // every chosen corner, the soft corner a blend is mostly asked for, from 0.4 times up to the
    // largest such; the rest from 0.6 to 1.6 times the equal-area one, shoulders and all.
    let first = chosen[0];
    let (alpha, d0) = (2.0 * half(first), rad / half(first).ptan());
    let even = gn_center_radius(2, gn_equal_area_w(2, alpha), alpha, d0);
    let (center, test) = match r.next_u64() % 3 {
        0 => (even, "fillets/g2-equal-area"),
        1 => {
            let most = chosen
                .iter()
                .map(|&i| {
                    let (a, d) = (2.0 * half(i), rad / half(i).ptan());
                    gn_center_radius(2, gn_peaked_w(2, a), a, d)
                })
                .fold(f64::INFINITY, f64::min);
            (
                most * r.range(0.4, 1.0),
                "fillets/g2-curving-most-at-the-middle",
            )
        }
        _ => (even * r.range(0.6, 1.6), "fillets/g2-by-center-radius"),
    };
    let centers = vec![center; n];
    let z = height * r.range(0.3, 0.7);
    let mut probes = Vec::new();
    for &i in &chosen {
        let (prev, at, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let d = rad / half(i).ptan();
        for side in [prev, next] {
            let q = toward(at, side, d - 1e-3 * d);
            probes.push(Probe {
                point: [q[0], q[1], z],
                want: [0.0, 0.0],
                tol: 0.02 / rad.min(center),
            });
        }
    }
    let edges = chosen
        .iter()
        .map(|&i| [pts[i][0], pts[i][1], height * r.range(0.2, 0.8)])
        .collect();
    let want = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![gn_rounded(&pts, &sizes, &centers, 2)],
        },
        height,
    };
    let exact = Exact {
        probes,
        variant: Some(test),
        ..Default::default()
    };
    let feature = act(Act::FilletG2, edges, rad, vec![center]);
    (prism(&pts, height), feature, want.into(), exact)
}

/// A box corner where three edges meet, filleted at radius `rad` with every fillet stopping `s`
/// short of the corner and the corner closed by one patch. Past `s` each fillet is the ordinary
/// one, so outside a cube round the corner the result is exact; it lies between the box with the
/// cube of side `s` at the corner taken away and the box with that cube left sharp. On each face
/// the flat stops short of where its two fillets' contact lines would cross, which the ordinary
/// rolling-ball corner reaches. With [`Act::FilletSetbackG2`] each fillet is the
/// curvature-continuous section of order 2 at the equal-area thumbweight, between the contacts the
/// ball of `rad` makes, so it meets its faces with their curvature and takes off what the round
/// does; the patch carries each fillet's curvature across their seam, the section square to the
/// edge `s` from the corner: just inside it the boundary still bends by the section's centre
/// radius round the edge and not along it. A circular fillet would not do: it meets its faces with
/// a jump in curvature, and no patch can match both at the points where its end section meets a
/// face.
pub(super) fn setback_corner(r: &mut Rng, blend: Act) -> FeatureDraw {
    let k = 1.0 - PI / 4.0;
    let w = [
        r.range(12.0, 18.0),
        r.range(12.0, 18.0),
        r.range(12.0, 18.0),
    ];
    let rad = r.range(0.8, 1.6);
    let s = rad * r.range(2.5, 3.5);
    let mut points = Vec::new();
    for axis in 0..3 {
        let mut p = w;
        p[axis] = w[axis] * r.range(0.3, 0.7);
        points.push(p);
    }
    let full = w[0] * w[1] * w[2];
    let side = 1.5 * s;
    let zone = axis_box(w.map(|x| x - side), w.map(|x| x + 1.0));
    let along = |from: f64| (0..3).map(|i| k * rad * rad * (w[i] - from)).sum::<f64>();
    let mut contacts = Vec::new();
    let mut away = Vec::new();
    let mut probes = Vec::new();
    for i in 0..3 {
        let (j, l) = ((i + 1) % 3, (i + 2) % 3);
        // The fillet along axis i touches the face square to j at depth `rad` along l, and the
        // other way about.
        for (face, across) in [(j, l), (l, j)] {
            let mut p = w;
            p[i] = w[i] - 1.2 * s;
            p[across] = w[across] - rad;
            let mut n = [0.0; 3];
            n[face] = 1.0;
            contacts.push((p, n));
        }
        // On the face square to i, the fillets along j and l would cross at `rad` in from both.
        let mut q = w;
        q[j] = w[j] - rad;
        q[l] = w[l] - rad;
        q[i] = w[i] - 1e-5 * rad;
        away.push(q);
        // The middle of fillet i's end section, a little into the patch: the section's own
        // middle, bending by its centre radius round the edge and not along it.
        if blend == Act::FilletSetbackG2 {
            let section = Seg::Gn {
                a: [0.0, rad],
                apex: [0.0, 0.0],
                b: [rad, 0.0],
                order: 2,
                w: gn_equal_area_w(2, PI / 2.0),
            };
            let (mid, _) = section.eval(0.5);
            let center = gn_center_radius(2, gn_equal_area_w(2, PI / 2.0), PI / 2.0, rad);
            let mut m = w;
            m[i] = w[i] - s + 1e-3 * s;
            m[j] = w[j] - mid[0];
            m[l] = w[l] - mid[1];
            probes.push(Probe {
                point: m,
                want: [1.0 / center, 0.0],
                tol: 0.05 / center,
            });
        }
    }
    let feature = act(blend, points, rad, vec![s]);
    let base = axis_box([0.0; 3], w);
    // The box with each fillet cut as far as the corner's cube, and that with the cube gone too.
    let section = || match blend {
        Act::FilletSetbackG2 => Seg::Gn {
            a: [rad, 0.0],
            apex: [0.0, 0.0],
            b: [0.0, rad],
            order: 2,
            w: gn_equal_area_w(2, PI / 2.0),
        },
        _ => round_section(rad),
    };
    let outer = (0..3).fold(base.clone().into(), |solid: Shape, axis| {
        Shape::bool(
            Op::Difference,
            solid,
            corner_sliver(w, axis, s, w[axis], rad, Some(section())),
        )
    });
    let inner = Shape::bool(
        Op::Difference,
        outer.clone(),
        axis_box(w.map(|x| x - s), w.map(|x| x + 1.0)),
    );
    let exact = Exact {
        sound: Some(Sound {
            zone: Some(zone),
            outside: full - side.ppowi(3) - along(side),
            least: full - along(s) - s.ppowi(3),
            most: full - along(s),
            betti: [1, 0, 0],
            contacts,
            away,
            probes,
            limits: Some(Limits::Between {
                inner: Box::new(inner),
                outer: Box::new(outer),
            }),
            scans: vec![corner_scan(
                w,
                0.9 * s,
                rad,
                blend == Act::FilletSetbackG2,
                true,
            )],
        }),
        ..Default::default()
    };
    (base.clone(), feature, base.into(), exact)
}

/// A convex prism with one side edge chamfered `c` back, filleted between the two faces either side
/// of the chamfer by a ball of radius `rad` big enough that its path clears the chamfer: the
/// chamfer face goes, and the result is the sharp prism with that corner rounded at `rad`.
pub(super) fn face_fillet(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let i = (r.next_u64() % n as u64) as usize;
    let (prev, at, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
    let (turn, shortest) = corners(&pts)[i];
    let half = turn.abs() / 2.0;
    // A ball of radius R touches the sides R·tan(half) from the corner, and passes it
    // R·(1/cos(half) − 1) away; the chamfer's line passes it c·sin(half) away.
    let reach = |rad: f64| rad * (1.0 / half.pcos() - 1.0);
    let rad_most = 0.45 * shortest / half.ptan();
    let c = reach(rad_most) / (1.3 * half.psin()) * r.range(0.3, 1.0);
    let rad_least = 1.3 * c * half.psin() / (1.0 / half.pcos() - 1.0);
    let rad = rad_least + (rad_most - rad_least) * r.range(0.0, 1.0);
    let height = r.range(2.0, 10.0);
    let mut cuts = vec![0.0; n];
    cuts[i] = c;
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![chamfered(&pts, &cuts)],
        },
        height,
    };
    let mut radii = vec![0.0; n];
    radii[i] = rad;
    let want = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![rounded(&pts, &radii)],
        },
        height,
    };
    let z = height * r.range(0.2, 0.8);
    let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, z];
    let feature = act(
        Act::FilletFaces,
        vec![mid(prev, at), mid(at, next)],
        rad,
        Vec::new(),
    );
    (base, feature, want.into(), Exact::default())
}

/// A rib, a prism on a rectangle or a trapezoid narrowing upward, whose top face becomes a round
/// tangent to both sides and to the top: on the rectangle a half cylinder of radius half its width,
/// on the trapezoid the arc of the circle touching all three lines.
pub(super) fn full_round(r: &mut Rng) -> FeatureDraw {
    let bottom = r.range(1.5, 5.0);
    let top = if r.next_u64().is_multiple_of(2) {
        bottom
    } else {
        bottom * r.range(0.4, 0.9)
    };
    let h = r.range(1.2 * bottom, 10.0);
    let length = r.range(2.0, 10.0);
    let (b, t) = (bottom / 2.0, top / 2.0);
    let pts = [[-b, 0.0], [b, 0.0], [t, h], [-t, h]];
    let lean = b - t;
    let side = lean.phypot(h);
    // The circle touching the top at (0, h) has its centre at (0, h − ρ), ρ from each side.
    let rho = h * t / (side - lean);
    let n = [h / side, lean / side];
    let centre = [0.0, h - rho];
    let touch = [centre[0] + rho * n[0], centre[1] + rho * n[1]];
    let start = n[1].patan2(n[0]);
    let lp = vec![
        Seg::Line {
            a: [-b, 0.0],
            b: [b, 0.0],
        },
        Seg::Line {
            a: [b, 0.0],
            b: touch,
        },
        Seg::Arc {
            center: centre,
            radius: rho,
            start,
            sweep: PI - 2.0 * start,
        },
        Seg::Line {
            a: [-touch[0], touch[1]],
            b: [-b, 0.0],
        },
    ];
    let want = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile { loops: vec![lp] },
        height: length,
    };
    let z = length * r.range(0.2, 0.8);
    let up = r.range(0.2, 0.5);
    let on_side = |s: f64| [s * (b + up * (t - b)), up * h, z];
    let feature = act(
        Act::FullRound,
        vec![on_side(1.0), [t * r.range(-0.5, 0.5), h, z], on_side(-1.0)],
        0.0,
        Vec::new(),
    );
    (prism(&pts, length), feature, want.into(), Exact::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_round_touches_the_top_and_both_sides() {
        let mut r = Rng::new(7);
        let mut rectangles = 0;
        for _ in 0..40 {
            let (base, _, want, _) = full_round(&mut r);
            let (
                Primitive::Extrude { profile: p, .. },
                Shape::Prim(Primitive::Extrude { profile: q, .. }),
            ) = (&base, &want)
            else {
                panic!("prisms");
            };
            let sides = &p.loops[0];
            let (&Seg::Line { a: foot, b: corner }, &Seg::Arc { center, radius, .. }) =
                (&sides[1], &q.loops[0][2])
            else {
                panic!("a side and the round");
            };
            let h = corner[1];
            // Up to the top, and as far from the right side's line as from the top.
            assert!((center[1] + radius - h).abs() < 1e-12);
            let (e, f) = (
                [corner[0] - foot[0], corner[1] - foot[1]],
                [center[0] - foot[0], center[1] - foot[1]],
            );
            let off = (e[0] * f[1] - e[1] * f[0]).abs() / e[0].phypot(e[1]);
            assert!((off - radius).abs() < 1e-12, "{off} against {radius}");
            // A rectangle of width w and height h keeps w·(h − w/2) and the half disc π·w²/8.
            if (corner[0] - foot[0]).abs() < 1e-15 {
                let w = 2.0 * corner[0];
                let want = w * (h - w / 2.0) + PI * w * w / 8.0;
                assert!((q.area() - want).abs() < 1e-12 * want);
                rectangles += 1;
            }
        }
        assert!(rectangles > 5 && rectangles < 35);
    }

    #[test]
    fn a_face_fillet_clears_its_chamfer() {
        let mut r = Rng::new(11);
        for _ in 0..60 {
            let (base, _, want, _) = face_fillet(&mut r);
            let Shape::Prim(want) = want else { panic!() };
            // The round lies inside the chamfered prism: everything the result holds, the base does.
            let (lo, hi) = crate::oracle::bounds(&want.clone().into()).unwrap();
            for p in crate::oracle::samples((lo, hi), 4000) {
                let base = Shape::from(base.clone());
                if crate::oracle::contains(&want.clone().into(), p) {
                    assert!(crate::oracle::contains(&base, p), "{p:?}");
                }
            }
        }
    }
}
