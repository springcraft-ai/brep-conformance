//! Where kernels are known to break: holes packed until the webs between them vanish, curved faces
//! that only just cross, and fillets running into other blends at corners they cannot round as one
//! ball rolls.

use super::*;
use crate::portable::Portable;

/// The area two discs of radius `rho` share with their centres `d` apart.
fn lens(rho: f64, d: f64) -> f64 {
    if d >= 2.0 * rho {
        return 0.0;
    }
    2.0 * rho * rho * (d / (2.0 * rho)).pacos() - d / 2.0 * (4.0 * rho * rho - d * d).sqrt()
}

/// A length from a twentieth to a ten-thousandth of `scale`, even in its exponent.
fn thin(r: &mut Rng, scale: f64) -> f64 {
    scale * 10f64.ppowf(r.range(-4.0, -1.3))
}

/// A plate drilled with a grid of holes packed until the web between neighbours is thin, gone
/// (each hole tangent to the next along a line) or less than nothing (each overlapping the next in
/// a lens), the outer holes a thin web from the plate's edges. One boolean of many: the plate keeps
/// its area less the holes' union, whose shared lenses the closed form adds back.
pub(super) fn dense_holes(r: &mut Rng) -> Drawn {
    let (nx, ny) = [(3, 3), (6, 4), (10, 10)][(r.next_u64() % 3) as usize];
    let rho = r.range(0.5, 2.0);
    let web = match r.next_u64() % 3 {
        0 => thin(r, rho),
        1 => 0.0,
        _ => -thin(r, rho),
    };
    let pitch = 2.0 * rho + web;
    let first = rho + thin(r, rho);
    let (w, d) = (
        2.0 * first + (nx - 1) as f64 * pitch,
        2.0 * first + (ny - 1) as f64 * pitch,
    );
    let t = r.range(1.0, 3.0);
    let mut tools = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            tools.push(Primitive::Cylinder {
                frame: Frame::at([first + i as f64 * pitch, first + j as f64 * pitch, -1.0]),
                radius: rho,
                height: t + 2.0,
            });
        }
    }
    let n = (nx * ny) as f64;
    // Neighbours along a row or a column overlap; diagonal ones, √2 pitches apart, never do.
    let pairs = ((nx - 1) * ny + nx * (ny - 1)) as f64;
    let holes = n * PI * rho * rho - pairs * lens(rho, pitch);
    let lost_arc = if pitch < 2.0 * rho {
        4.0 * rho * (pitch / (2.0 * rho)).pacos()
    } else {
        0.0
    };
    let top = w * d - holes;
    let many = Many {
        target: Some(axis_box([0.0; 3], [w, d, t])),
        tools,
        volume: top * t,
        area: 2.0 * top + 2.0 * (w + d) * t + (n * TAU * rho - pairs * lost_arc) * t,
    };
    let a = many.tools[0].clone();
    let exact = Exact {
        many: Some(many),
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// Two cylinders crossing square, their axes `ra + rb − δ` apart for δ from a tenth to a millionth
/// of the smaller radius: they share a sliver whose boundary is one small closed curve, the
/// surfaces all but tangent along it. Its volume is `∫ 2√(ra² − z²)·2√(rb² − (z − c)²) dz` over the
/// heights both reach, integrated after `z = lo + δ(1 − cos θ)/2`, which leaves the integrand
/// smooth at both ends.
pub(super) fn crossing_near_tangent(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (ra, rb) = (r.range(1.0, 6.0), r.range(1.0, 6.0));
    let delta = ra.min(rb) * 10f64.ppowf(r.range(-6.0, -1.0));
    let c = ra + rb - delta;
    let len = 4.0 * ra.max(rb) + 4.0;
    let a = Primitive::Cylinder {
        frame: Frame::along([-len / 2.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        radius: ra,
        height: len,
    };
    let b = Primitive::Cylinder {
        frame: Frame::along([0.0, -len / 2.0, c], [0.0, 1.0, 0.0]),
        radius: rb,
        height: len,
    };
    let lo = c - rb;
    let f = |theta: f64| {
        let z = lo + delta * (1.0 - theta.pcos()) / 2.0;
        let wa = (ra * ra - z * z).max(0.0).sqrt();
        let wb = (rb * rb - (z - c) * (z - c)).max(0.0).sqrt();
        4.0 * wa * wb * delta / 2.0 * theta.psin()
    };
    let volume = gauss(0.0, PI, 64, |t, wt| wt * f(t));
    let exact = Exact {
        intersection_volume: Some(volume),
        ..Default::default()
    };
    (a, b, exact)
}

/// `∫ g over [a, b]` in `n` panels of five-point Gauss, `g(x, weight)` returning its weighted term.
pub(super) fn gauss(a: f64, b: f64, n: usize, g: impl Fn(f64, f64) -> f64) -> f64 {
    const X: [f64; 5] = [
        0.0,
        -0.538_469_310_105_683_1,
        0.538_469_310_105_683_1,
        -0.906_179_845_938_664,
        0.906_179_845_938_664,
    ];
    const W: [f64; 5] = [
        0.568_888_888_888_888_9,
        0.478_628_670_499_366_5,
        0.478_628_670_499_366_5,
        0.236_926_885_056_189_1,
        0.236_926_885_056_189_1,
    ];
    let h = (b - a) / n as f64;
    (0..n)
        .map(|k| {
            let mid = a + (k as f64 + 0.5) * h;
            X.iter()
                .zip(W)
                .map(|(x, w)| g(mid + 0.5 * h * x, 0.5 * h * w))
                .sum::<f64>()
        })
        .sum()
}

/// As `fillet-chains`, a prism on a convex polygon with every corner rounded by ρ, filleted round
/// its top loop, but by a radius `r` above ρ: the fillet runs into rounds tighter than its own
/// ball, which cannot roll round them touching both faces, and kernels finish those corners each
/// their own way. Whatever they do there, a blend made by balls inside the prism keeps every ball
/// of radius `r` inside it: the result holds their union, the prism to `h − r`, the polygon inset
/// by `r` (sharp, since the inset swallows the rounds) to `h`, and the balls along that inset loop
/// at `h − r`, whose volume Steiner's formula gives over the top slab,
/// `A(h − r) + A'r + πr²L'/4 + 2πr³/3`. Along each straight run, 2r clear of its rounds, the fillet
/// is the ordinary one, taking `(1 − π/4)r²` a unit of length and meeting both faces tangentially.
pub(super) fn tight_chain(r: &mut Rng) -> FeatureDraw {
    tight_parts(r).0
}

/// A tight chain's case and the union of balls its result must hold.
fn tight_parts(r: &mut Rng) -> (FeatureDraw, Shape) {
    let pts = convex_polygon(r);
    let n = pts.len();
    let limit = round_limit(&pts);
    let rho = limit * r.range(0.1, 0.3);
    let mut rad = rho * r.range(1.2, 3.0);
    // The inset must stay a polygon of the same sides: each side keeps its direction.
    let keeps = |d: f64| {
        let q = inset(&pts, d);
        (0..n).all(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let (c, e) = (q[i], q[(i + 1) % n]);
            let along = (b[0] - a[0]) * (e[0] - c[0]) + (b[1] - a[1]) * (e[1] - c[1]);
            let len = (b[0] - a[0]).phypot(b[1] - a[1]);
            along > 0.2 * len * len
        })
    };
    while !keeps(rad) {
        rad *= 0.8;
    }
    let rad = rad.max(1.1 * rho);
    let h = r.range(2.0, 10.0).max(3.0 * rad);
    let outer = Profile {
        loops: vec![rounded(&pts, &vec![rho; n])],
    };
    let core = inset(&pts, rad);
    let inner = polygon(&core);
    let i = (r.next_u64() % n as u64) as usize;
    let (p, q) = (pts[i], pts[(i + 1) % n]);
    let feature = Feature {
        act: Act::Fillet,
        points: vec![[(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, h]],
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: outer.clone(),
        height: h,
    };
    let z = h - rad;
    let mut shape = Shape::bool(
        Op::Union,
        Primitive::Extrude {
            frame: Frame::at([0.0; 3]),
            profile: outer.clone(),
            height: z,
        },
        Primitive::Extrude {
            frame: Frame::at([0.0; 3]),
            profile: inner.clone(),
            height: h,
        },
    );
    for k in 0..n {
        let (a, b) = (core[k], core[(k + 1) % n]);
        shape = Shape::bool(Op::Union, shape, rod([a[0], a[1], z], [b[0], b[1], z], rad));
        shape = Shape::bool(Op::Union, shape, sphere([a[0], a[1], z], rad));
    }
    let (a, ai, li) = (outer.area(), inner.area(), inner.perimeter());
    let balls = a * z + ai * rad + PI * rad * rad / 4.0 * li + 2.0 * PI / 3.0 * rad.ppowi(3);
    let k = 1.0 - PI / 4.0;
    let mut certain = 0.0;
    let mut contacts = Vec::new();
    // The prism less the ordinary fillet along each straight run, 2r clear of its rounds.
    let mut most: Shape = base.clone().into();
    for seg in &outer.loops[0] {
        let Seg::Line { a: p, b: q } = *seg else {
            continue;
        };
        let len = (q[0] - p[0]).phypot(q[1] - p[1]);
        certain += k * rad * rad * (len - 4.0 * rad).max(0.0);
        if len > 4.0 * rad {
            let along = [(q[0] - p[0]) / len, (q[1] - p[1]) / len];
            let sliver = Primitive::Extrude {
                frame: Frame {
                    origin: [p[0] + 2.0 * rad * along[0], p[1] + 2.0 * rad * along[1], h],
                    x: [0.0, 0.0, -1.0],
                    y: [-along[1], along[0], 0.0],
                    z: [along[0], along[1], 0.0],
                },
                profile: Profile {
                    loops: vec![vec![
                        Seg::Line {
                            a: [0.0, 0.0],
                            b: [rad, 0.0],
                        },
                        round_section(rad),
                        Seg::Line {
                            a: [0.0, rad],
                            b: [0.0, 0.0],
                        },
                    ]],
                },
                height: len - 4.0 * rad,
            };
            most = Shape::bool(Op::Difference, most, sliver);
        }
        // At the run's middle the ball touches the top `r` in from the edge and the side `r` down.
        let n = [(q[1] - p[1]) / len, (p[0] - q[0]) / len];
        let m = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
        contacts.push(([m[0] - rad * n[0], m[1] - rad * n[1], h], [0.0, 0.0, 1.0]));
        contacts.push(([m[0], m[1], h - rad], [n[0], n[1], 0.0]));
    }
    // Down onto each corner of the loop, where the fillet runs into a round tighter than its ball.
    // A corner whose two sides run straight past the scan, clear of the next corners' rounds and
    // of every other side's fillet, is its own mirror in the upright plane that halves it.
    let turns = corners(&pts);
    let reach = 4.0 * rad;
    let off = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (d, q) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
        let t = ((q[0] * d[0] + q[1] * d[1]) / (d[0] * d[0] + d[1] * d[1])).clamp(0.0, 1.0);
        (q[0] - t * d[0]).phypot(q[1] - t * d[1])
    };
    let scans = (0..n)
        .map(|i| {
            let c = pts[i];
            let (before, after) = ((i + n - 1) % n, (i + 1) % n);
            let unit = |q: [f64; 2]| {
                let l = (q[0] - c[0]).phypot(q[1] - c[1]);
                ([(q[0] - c[0]) / l, (q[1] - c[1]) / l], l)
            };
            let ((u, lu), (v, lv)) = (unit(pts[before]), unit(pts[after]));
            let straight =
                |len: f64, next: usize| len >= reach + rho * (turns[next].0.abs() / 2.0).ptan();
            let clear = (0..n)
                .filter(|&k| k != i && (k + 1) % n != i)
                .all(|k| off(c, pts[k], pts[(k + 1) % n]) >= reach);
            let halves = (u[0] + v[0]).phypot(u[1] + v[1]);
            let same = if straight(lu, before) && straight(lv, after) && clear && halves > 1e-9 {
                vec![upright_mirror(
                    c,
                    [(u[0] + v[0]) / halves, (u[1] + v[1]) / halves],
                )]
            } else {
                Vec::new()
            };
            Scan {
                from: [c[0], c[1], h + 1.0],
                along: [0.0, 0.0, -1.0],
                across: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                half: 2.0 * rad,
                size: rho,
                curvature: false,
                same,
            }
        })
        .collect();
    let exact = Exact {
        sound: Some(Sound {
            least: balls,
            most: a * h - certain,
            betti: [1, 0, 0],
            contacts,
            limits: Some(Limits::Between {
                inner: Box::new(shape.clone()),
                outer: Box::new(most),
            }),
            scans,
            ..Default::default()
        }),
        ..Default::default()
    };
    ((base.clone(), feature, base.into(), exact), shape)
}

/// An L-prism with its concave upright edge and the two convex top edges meeting it filleted by
/// one radius: at the top of the concave edge a concave fillet meets two convex ones, a corner no
/// ball rolls round. Outside a box of half-width 3r round that corner each edge is as `fillet`
/// makes it alone: a convex edge loses `(1 − π/4)r²` a unit of length, the concave one gains as
/// much, each cut square where it runs out. Inside the box the corner is open, the blends changing
/// nothing farther than 2r from their edges: the result lies between the box's share of the prism
/// less the convex edges' 2r-square strips there and plus the concave edge's.
pub(super) fn mixed_corner(r: &mut Rng) -> FeatureDraw {
    let k = 1.0 - PI / 4.0;
    let (a, b) = (r.range(8.0, 14.0), r.range(8.0, 14.0));
    let t = r.range(3.0, 5.0);
    let rad = t * r.range(0.08, 0.25);
    let h = r.range(4.0, 10.0);
    let pts = [[0.0, 0.0], [a, 0.0], [a, t], [t, t], [t, b], [0.0, b]];
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&pts),
        height: h,
    };
    let half = 3.0 * rad;
    let zone = axis_box(
        [t - half, t - half, h - half],
        [t + half, t + half, h + 1.0],
    );
    let prism = (a * t + t * (b - t)) * h;
    // The L holds three of the zone's four quarters, over its height `half`.
    let inside = 3.0 * half * half * half;
    let outside = prism - inside - k * rad * rad * ((a - t - half) + (b - t - half))
        + k * rad * rad * (h - half);
    let strip = 4.0 * rad * rad * half;
    let points = vec![
        [t + (a - t) * r.range(0.4, 0.7), t, h],
        [t, t + (b - t) * r.range(0.4, 0.7), h],
        [t, t, h * r.range(0.2, 0.5)],
    ];
    let feature = Feature {
        act: Act::Fillet,
        points,
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    // A blend changes nothing farther than 2r from its edge: the convex edges take from within
    // that of them, and the concave edge adds within that of it.
    let reach = 2.0 * rad;
    let inner = Shape::bool(
        Op::Difference,
        Shape::bool(
            Op::Difference,
            base.clone(),
            axis_box(
                [t - reach, t - reach, h - reach],
                [a + 1.0, t + 1.0, h + 1.0],
            ),
        ),
        axis_box(
            [t - reach, t - reach, h - reach],
            [t + 1.0, b + 1.0, h + 1.0],
        ),
    );
    let outer = Shape::bool(
        Op::Union,
        base.clone(),
        axis_box([t, t, 0.0], [t + reach, t + reach, h]),
    );
    // Down the corner's diagonal, short of where either arm ends. The arms are alike within it,
    // so the result is its own mirror in the upright plane between them.
    let mut scan = corner_scan(
        [t, t, h],
        (2.0 * rad).min(0.3 * (a.min(b) - t)),
        rad,
        false,
        false,
    );
    scan.same
        .push(upright_mirror([t, t], [0.5f64.sqrt(), 0.5f64.sqrt()]));
    let exact = Exact {
        sound: Some(Sound {
            zone: Some(zone),
            outside,
            least: outside + inside - 2.0 * strip,
            most: outside + inside + strip,
            betti: [1, 0, 0],
            limits: Some(Limits::Between {
                inner: Box::new(inner),
                outer: Box::new(outer),
            }),
            scans: vec![scan],
            ..Default::default()
        }),
        ..Default::default()
    };
    (base.clone(), feature, base.into(), exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_cylinders_share_what_a_fine_sample_finds() {
        // At a tenth of the radius the sliver is big enough to sample: 200k points in its box.
        let ra = 2.0;
        let rb = 3.0;
        let delta = 0.2;
        let c = ra + rb - delta;
        let lo = c - rb;
        let f = |theta: f64| {
            let z = lo + delta * (1.0 - theta.pcos()) / 2.0;
            let wa = (ra * ra - z * z).max(0.0).sqrt();
            let wb = (rb * rb - (z - c) * (z - c)).max(0.0).sqrt();
            4.0 * wa * wb * delta / 2.0 * theta.psin()
        };
        let v = gauss(0.0, PI, 64, |t, wt| wt * f(t));
        let (hx, hy) = (
            (rb * rb - (rb - delta).ppowi(2)).sqrt(),
            (2.0 * ra * delta).sqrt(),
        );
        let n = 200_000;
        let mut hit = 0;
        let mut rng = Rng::new(3);
        for _ in 0..n {
            let p = [rng.range(-hx, hx), rng.range(-hy, hy), rng.range(lo, ra)];
            if p[1] * p[1] + p[2] * p[2] <= ra * ra && p[0] * p[0] + (p[2] - c).ppowi(2) <= rb * rb
            {
                hit += 1;
            }
        }
        let sampled = hit as f64 / n as f64 * (2.0 * hx) * (2.0 * hy) * delta;
        assert!((v - sampled).abs() < 0.02 * v, "{v} against {sampled}");
    }

    #[test]
    fn a_tight_chain_s_least_is_its_union_of_balls() {
        let mut r = Rng::new(5);
        for _ in 0..8 {
            let ((_, _, _, exact), shape) = tight_parts(&mut r);
            let (lo, hi) = crate::oracle::bounds(&shape).unwrap();
            let n = 60_000;
            let held = crate::oracle::samples((lo, hi), n)
                .iter()
                .filter(|p| crate::oracle::contains(&shape, **p))
                .count();
            let boxed = (0..3).map(|k| hi[k] - lo[k]).product::<f64>();
            let sampled = held as f64 / n as f64 * boxed;
            let want = exact.sound.unwrap().least;
            assert!(
                (sampled - want).abs() < 0.01 * want,
                "{sampled} against {want}"
            );
        }
    }

    #[test]
    fn dense_holes_keep_the_plate_less_their_union() {
        let mut r = Rng::new(9);
        for _ in 0..9 {
            let (_, _, exact) = dense_holes(&mut r);
            let m = exact.many.unwrap();
            let target: Shape = m.target.clone().unwrap().into();
            let mut s = target.clone();
            for tool in &m.tools {
                s = Shape::bool(Op::Difference, s, tool.clone());
            }
            let (lo, hi) = crate::oracle::bounds(&target).unwrap();
            let n = 60_000;
            let held = crate::oracle::samples((lo, hi), n)
                .iter()
                .filter(|p| crate::oracle::contains(&s, **p))
                .count();
            let boxed = (0..3).map(|k| hi[k] - lo[k]).product::<f64>();
            let sampled = held as f64 / n as f64 * boxed;
            assert!(
                (sampled - m.volume).abs() < 0.01 * m.volume,
                "{sampled} against {}",
                m.volume
            );
        }
    }
}
