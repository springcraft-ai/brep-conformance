//! What a drawing asks of a kernel: the principal curvatures about a solid's boundary, and its
//! hidden-line projection, whose seen and hidden stretches come from exact ray intervals.

use super::*;
use crate::portable::Portable;

/// Points about a box, cylinder, cone, ball, ring or drilled box, each off the boundary along its
/// normal by up to a fraction of the local radius, and the principal curvatures where the boundary
/// lies nearest: zero on a plane, 1/ρ round a cylinder, `cos α/ρ` round a cone whose side leans
/// `α`, 1/R on a ball, `1/r` and `cos φ/(R + r cos φ)` on a ring, and −1/ρ round a hole.
pub(super) fn curvature_case(r: &mut Rng) -> Drawn {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let frame = turned_frame(r, origin);
    // Each point as (on the boundary, outward normal, offset along it, curvatures, the first's
    // direction), local.
    let mut at: Vec<(Vec3, Vec3, f64, [f64; 2], Vec3)> = Vec::new();
    let face = |r: &mut Rng, s: Vec3, axis: usize| {
        let top = r.next_u64().is_multiple_of(2);
        let mut q = [0, 1, 2].map(|k| s[k] * r.range(0.25, 0.75));
        q[axis] = if top { s[axis] } else { 0.0 };
        let mut n = [0.0; 3];
        n[axis] = if top { 1.0 } else { -1.0 };
        let small = s.iter().copied().fold(f64::INFINITY, f64::min);
        let mut d = [0.0; 3];
        d[(axis + 1) % 3] = 1.0;
        (q, n, small * r.range(-0.1, 0.1), [0.0, 0.0], d)
    };
    let shape: Shape = match r.next_u64() % 6 {
        0 => {
            let size = [r.range(2.0, 8.0), r.range(2.0, 8.0), r.range(2.0, 8.0)];
            for _ in 0..8 {
                let axis = (r.next_u64() % 3) as usize;
                at.push(face(r, size, axis));
            }
            Primitive::Box { frame, size }.into()
        }
        1 => {
            let radius = r.range(0.5, 3.0);
            let height = r.range((2.0 * radius).max(2.0), 9.0);
            for _ in 0..6 {
                let (a, z) = (r.range(0.0, TAU), height * r.range(0.3, 0.7));
                let n = [a.pcos(), a.psin(), 0.0];
                let q = [radius * n[0], radius * n[1], z];
                let around = [-a.psin(), a.pcos(), 0.0];
                at.push((
                    q,
                    n,
                    radius * r.range(-0.2, 0.2),
                    [1.0 / radius, 0.0],
                    around,
                ));
            }
            for _ in 0..2 {
                let (a, rho) = (r.range(0.0, TAU), radius * r.range(0.0, 0.5));
                let top = r.next_u64().is_multiple_of(2);
                let q = [
                    rho * a.pcos(),
                    rho * a.psin(),
                    if top { height } else { 0.0 },
                ];
                let n = [0.0, 0.0, if top { 1.0 } else { -1.0 }];
                at.push((
                    q,
                    n,
                    radius * r.range(-0.1, 0.1),
                    [0.0, 0.0],
                    [1.0, 0.0, 0.0],
                ));
            }
            Primitive::Cylinder {
                frame,
                radius,
                height,
            }
            .into()
        }
        2 => {
            let bottom = r.range(1.0, 4.0);
            let mut top = r.range(0.3, 4.0);
            if (top - bottom).abs() < 0.2 {
                top = bottom + 0.5;
            }
            let height = r.range(2.0, 8.0);
            let slope = (bottom - top) / height;
            let lean = (1.0 + slope * slope).sqrt();
            for _ in 0..8 {
                let (a, z) = (r.range(0.0, TAU), height * r.range(0.3, 0.7));
                let rho = bottom - slope * z;
                let n = [a.pcos() / lean, a.psin() / lean, slope / lean];
                let q = [rho * a.pcos(), rho * a.psin(), z];
                let around = [-a.psin(), a.pcos(), 0.0];
                at.push((
                    q,
                    n,
                    rho * r.range(-0.1, 0.1),
                    [1.0 / (rho * lean), 0.0],
                    around,
                ));
            }
            cone(frame, bottom, top, height).into()
        }
        3 => {
            let radius = r.range(0.5, 5.0);
            for _ in 0..8 {
                let n = r.direction();
                let q = n.map(|x| x * radius);
                let k = 1.0 / radius;
                // Umbilic: every tangent is principal.
                let seed = if n[0].abs() < 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let s = seed[0] * n[0] + seed[1] * n[1] + seed[2] * n[2];
                let d = unit3([0, 1, 2].map(|i| seed[i] - s * n[i]));
                at.push((q, n, radius * r.range(-0.3, 0.3), [k, k], d));
            }
            Primitive::Sphere { frame, radius }.into()
        }
        4 => {
            let minor = r.range(0.3, 1.5);
            let major = minor + r.range(0.5, 4.0);
            for _ in 0..8 {
                let (a, b) = (r.range(0.0, TAU), r.range(0.0, TAU));
                let n = [b.pcos() * a.pcos(), b.pcos() * a.psin(), b.psin()];
                let ring = major + minor * b.pcos();
                let q = [ring * a.pcos(), ring * a.psin(), minor * b.psin()];
                let want = [1.0 / minor, b.pcos() / ring];
                // 1/r is the tube's own curvature, along its meridian.
                let meridian = [-b.psin() * a.pcos(), -b.psin() * a.psin(), b.pcos()];
                at.push((q, n, minor * r.range(-0.3, 0.3), want, meridian));
            }
            Primitive::Torus {
                frame,
                major,
                minor,
            }
            .into()
        }
        _ => {
            let size = [r.range(4.0, 8.0), r.range(4.0, 8.0), r.range(3.0, 8.0)];
            let c = [size[0] / 2.0, size[1] / 2.0];
            let radius = size[0].min(size[1]) * r.range(0.15, 0.3);
            for _ in 0..5 {
                let (a, z) = (r.range(0.0, TAU), size[2] * r.range(0.3, 0.7));
                let n = [-a.pcos(), -a.psin(), 0.0];
                let q = [c[0] + radius * a.pcos(), c[1] + radius * a.psin(), z];
                // The first curvature, zero, runs along the hole's axis.
                at.push((
                    q,
                    n,
                    radius * r.range(-0.3, 0.3),
                    [0.0, -1.0 / radius],
                    [0.0, 0.0, 1.0],
                ));
            }
            // Only the four sides: the top and bottom have the hole in them.
            for _ in 0..3 {
                let axis = (r.next_u64() % 2) as usize;
                at.push(face(r, size, axis));
            }
            let world = |l: Vec3| framed(&frame, l).0;
            let hole = rod(
                world([c[0], c[1], -1.0]),
                world([c[0], c[1], size[2] + 1.0]),
                radius,
            );
            Shape::bool(Op::Difference, Primitive::Box { frame, size }, hole)
        }
    };
    let points = at
        .iter()
        .map(|&(q, n, off, _, _)| framed(&frame, [0, 1, 2].map(|k| q[k] + off * n[k])).0)
        .collect();
    let want = at.iter().map(|x| x.3).collect();
    let curvature_frames = at
        .iter()
        .map(|&(q, n, _, _, d)| CurvatureFrame {
            foot: framed(&frame, q).0,
            normal: framed(&frame, n).1,
            d1: framed(&frame, d).1,
        })
        .collect();
    let exact = Exact {
        curvature: Some(Curvatures { points, want }),
        curvature_frames,
        ..Default::default()
    };
    (shape.clone(), shape, exact)
}

/// A ray whose chord through a round face is shorter than `2·√GRAZE` of its radius grazes it: a
/// point on an outline is on it only to rounding, and a tangent ray from it cuts a chord near 1e-8.
/// It moves where a rim starts to hide by about √GRAZE.
const GRAZE: f64 = 1e-14;

/// A body whose stretches of material along any ray have a closed form.
enum Body {
    Boxes(Vec<(Vec3, Vec3)>),
    Cylinder {
        c: Vec3,
        r: f64,
        h: f64,
    },
    Ball {
        c: Vec3,
        r: f64,
    },
    /// A box drilled through along z about its middle.
    Drilled {
        lo: Vec3,
        hi: Vec3,
        r: f64,
    },
}

fn slab(lo: Vec3, hi: Vec3, o: Vec3, w: Vec3) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for k in 0..3 {
        if w[k] == 0.0 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / w[k], (hi[k] - o[k]) / w[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0 < t1).then_some((t0, t1))
}

/// Where the ray runs inside the endless cylinder of radius `r` along z through `c`.
fn tube(c: [f64; 2], r: f64, o: Vec3, w: Vec3) -> Option<(f64, f64)> {
    let (px, py) = (o[0] - c[0], o[1] - c[1]);
    let a = w[0] * w[0] + w[1] * w[1];
    let cc = px * px + py * py - r * r;
    if a == 0.0 {
        return (cc < 0.0).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let b = px * w[0] + py * w[1];
    let disc = b * b - a * cc;
    if disc <= GRAZE * a * r * r {
        return None;
    }
    let s = disc.sqrt();
    Some(((-b - s) / a, (-b + s) / a))
}

impl Body {
    /// The stretches of `o + t·w` inside the body.
    fn along(&self, o: Vec3, w: Vec3) -> Vec<(f64, f64)> {
        match self {
            Body::Boxes(boxes) => boxes
                .iter()
                .filter_map(|&(lo, hi)| slab(lo, hi, o, w))
                .collect(),
            &Body::Cylinder { c, r, h } => {
                let Some((a, b)) = tube([c[0], c[1]], r, o, w) else {
                    return vec![];
                };
                let lo = [f64::NEG_INFINITY, f64::NEG_INFINITY, c[2]];
                let hi = [f64::INFINITY, f64::INFINITY, c[2] + h];
                slab(lo, hi, o, w)
                    .map(|(s0, s1)| (a.max(s0), b.min(s1)))
                    .filter(|x| x.0 < x.1)
                    .into_iter()
                    .collect()
            }
            &Body::Ball { c, r } => {
                let p = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
                let a = w[0] * w[0] + w[1] * w[1] + w[2] * w[2];
                let b = p[0] * w[0] + p[1] * w[1] + p[2] * w[2];
                let cc = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - r * r;
                let disc = b * b - a * cc;
                if disc <= GRAZE * a * r * r {
                    return vec![];
                }
                let s = disc.sqrt();
                vec![((-b - s) / a, (-b + s) / a)]
            }
            &Body::Drilled { lo, hi, r } => {
                let Some((a, b)) = slab(lo, hi, o, w) else {
                    return vec![];
                };
                let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
                match tube(mid, r, o, w) {
                    None => vec![(a, b)],
                    Some((h0, h1)) => [(a, b.min(h0)), (a.max(h1), b)]
                        .into_iter()
                        .filter(|x| x.0 < x.1)
                        .collect(),
                }
            }
        }
    }

    /// Whether the ray from `p` toward the viewer passes through more than `eps` of material. A
    /// point on a round face is on it only to rounding, which near an outline turns into a sliver
    /// of material as long as that rounding over how squarely the ray leaves.
    fn hides(&self, p: Vec3, toward: Vec3, eps: f64) -> bool {
        self.along(p, toward)
            .iter()
            .any(|&(a, b)| b - a.max(0.0) > eps)
    }
}

fn box_edges(lo: Vec3, hi: Vec3) -> Vec<Curve3> {
    let corner = |n: usize| [0, 1, 2].map(|i| if n >> i & 1 == 1 { hi[i] } else { lo[i] });
    let mut out = Vec::new();
    for n in 0..8 {
        for i in 0..3 {
            if n >> i & 1 == 0 {
                out.push(Curve3::Line {
                    a: corner(n),
                    b: corner(n | 1 << i),
                });
            }
        }
    }
    out
}

/// The rims and the two outline runs of a cylinder about z through `c`, `h` tall, seen along `d`.
fn cylinder_curves(c: Vec3, r: f64, h: f64, d: Vec3) -> Vec<Curve3> {
    let rim = |z: f64| Curve3::Circle {
        c: [c[0], c[1], c[2] + z],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        r,
    };
    let mut out = vec![rim(0.0), rim(h)];
    let a0 = d[1].patan2(d[0]);
    for a in [a0 + PI / 2.0, a0 - PI / 2.0] {
        let p = [c[0] + r * a.pcos(), c[1] + r * a.psin(), c[2]];
        out.push(Curve3::Line {
            a: p,
            b: [p[0], p[1], p[2] + h],
        });
    }
    out
}

/// A direction with every component at least `least` across, and z within `z`.
fn view(r: &mut Rng, least: f64, z: (f64, f64)) -> Vec3 {
    loop {
        let d = r.direction();
        let zz = d[2].abs();
        if d.iter().all(|x| x.abs() >= least) && zz >= z.0 && zz <= z.1 {
            return d;
        }
    }
}

/// Each curve cut where the ray toward the viewer starts or stops passing through material.
fn pieces(body: &Body, curves: &[Curve3], d: Vec3) -> Vec<Piece> {
    let toward = d.map(|x| -x);
    let span = curves
        .iter()
        .flat_map(|c| [c.at(0.0), c.at(0.5)])
        .flat_map(|p| p.map(f64::abs))
        .fold(1.0, f64::max);
    // It moves where a run starts to hide by about as much: well inside what the judge allows.
    let eps = 1e-9 * span;
    let seen = |c: &Curve3, t: f64| !body.hides(c.at(t), toward, eps);
    // Sampled inside, never at an end: an edge's end is also the next face's, and takes its side.
    let n = 512;
    let mid = |j: usize| (j as f64 + 0.5) / n as f64;
    let mut out = Vec::new();
    for c in curves {
        let mut t0 = 0.0;
        let mut state = seen(c, mid(0));
        for j in 1..n {
            if seen(c, mid(j)) == state {
                continue;
            }
            let (mut lo, mut hi) = (mid(j - 1), mid(j));
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if seen(c, mid) == state {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let cut = (lo + hi) / 2.0;
            out.push(Piece {
                curve: *c,
                t0,
                t1: cut,
                visible: state,
            });
            (t0, state) = (cut, !state);
        }
        out.push(Piece {
            curve: *c,
            t0,
            t1: 1.0,
            visible: state,
        });
    }
    out
}

/// A box, a cylinder, a ball, a box behind another or a drilled box, drawn looking along a
/// direction no face is seen edge-on from: every sharp edge and every outline (where the line of
/// sight grazes a curved face) cut into what the viewer sees and what the solid hides, the cut
/// points found by bisection on where the ray toward the viewer first meets material.
pub(super) fn projection_case(r: &mut Rng) -> Drawn {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let add = |a: Vec3, b: Vec3| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let size = |r: &mut Rng, lo: f64, hi: f64| [r.range(lo, hi), r.range(lo, hi), r.range(lo, hi)];
    let (shape, body, curves, d): (Shape, Body, Vec<Curve3>, Vec3) = match r.next_u64() % 5 {
        0 => {
            let hi = add(origin, size(r, 1.5, 7.0));
            let d = view(r, 0.15, (0.0, 1.0));
            let b = Body::Boxes(vec![(origin, hi)]);
            (axis_box(origin, hi).into(), b, box_edges(origin, hi), d)
        }
        1 => {
            let (rad, h) = (r.range(0.5, 3.0), r.range(1.0, 8.0));
            let d = view(r, 0.0, (0.15, 0.95));
            let cyl = Primitive::Cylinder {
                frame: Frame::at(origin),
                radius: rad,
                height: h,
            };
            let b = Body::Cylinder {
                c: origin,
                r: rad,
                h,
            };
            (cyl.into(), b, cylinder_curves(origin, rad, h, d), d)
        }
        2 => {
            let rad = r.range(0.5, 5.0);
            let d = r.direction();
            let (u, v) = brep_protocol::view_axes(d);
            let outline = Curve3::Circle {
                c: origin,
                x: u,
                y: v,
                r: rad,
            };
            let b = Body::Ball { c: origin, r: rad };
            (sphere(origin, rad).into(), b, vec![outline], d)
        }
        3 => {
            let (sa, sb) = (size(r, 2.0, 5.0), size(r, 1.5, 4.0));
            let d = view(r, 0.15, (0.0, 1.0));
            let half = |s: Vec3| (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() / 2.0;
            let gap = half(sa) + half(sb) + r.range(0.5, 2.0);
            // Across the line of sight by up to 0.4 of the back box's half-diagonal.
            let raw = r.direction();
            let along = raw[0] * d[0] + raw[1] * d[1] + raw[2] * d[2];
            let side = unit3([0, 1, 2].map(|k| raw[k] - along * d[k]));
            let shift = r.range(0.0, 0.4) * half(sa);
            let ca = add(origin, sa.map(|x| x / 2.0));
            let cb = [0, 1, 2].map(|k| ca[k] - gap * d[k] + shift * side[k]);
            let lb = [0, 1, 2].map(|k| cb[k] - sb[k] / 2.0);
            let (ha, hb) = (add(origin, sa), add(lb, sb));
            let shape = Shape::bool(Op::Union, axis_box(origin, ha), axis_box(lb, hb));
            let curves = [box_edges(origin, ha), box_edges(lb, hb)].concat();
            (shape, Body::Boxes(vec![(origin, ha), (lb, hb)]), curves, d)
        }
        _ => {
            let s = [r.range(3.0, 7.0), r.range(3.0, 7.0), r.range(1.5, 6.0)];
            let hi = add(origin, s);
            let rad = s[0].min(s[1]) * r.range(0.15, 0.35);
            let d = view(r, 0.15, (0.4, 0.95));
            let mid = [origin[0] + s[0] / 2.0, origin[1] + s[1] / 2.0];
            let hole = rod(
                [mid[0], mid[1], origin[2] - 1.0],
                [mid[0], mid[1], hi[2] + 1.0],
                rad,
            );
            let shape = Shape::bool(Op::Difference, axis_box(origin, hi), hole);
            let curves = [
                box_edges(origin, hi),
                cylinder_curves([mid[0], mid[1], origin[2]], rad, s[2], d),
            ]
            .concat();
            let b = Body::Drilled {
                lo: origin,
                hi,
                r: rad,
            };
            (shape, b, curves, d)
        }
    };
    let exact = Exact {
        drawing: Some(Drawing {
            direction: d,
            pieces: pieces(&body, &curves, d),
        }),
        ..Default::default()
    };
    (shape.clone(), shape, exact)
}

/// A box, a cylinder, a ball, a cone, a drilled box, a box with its four upright edges rounded or
/// a box with one side in two faces, with its surfaces, the edges between them and its closed-form
/// volume and area: a display must cover its boundary and draw exactly those edges, each with the
/// turn its faces make. Every edge is a convex right angle but two kinds: a cone's rim, which
/// turns by the cone's half angle more, and the eight lines where a rounded box's rounds meet its
/// sides, which turn by nothing. A ball has no edge, and a seam is none.
pub(super) fn display_case(r: &mut Rng) -> Drawn {
    use crate::model::add;
    use brep_protocol::profile::rounded;
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let rim = |c: Vec3, r: f64| Curve3::Circle {
        c,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        r,
    };
    let square = |curves: Vec<Curve3>| -> Vec<Meet> {
        curves
            .into_iter()
            .map(|c| Meet::whole(c, PI / 2.0))
            .collect()
    };
    // An axis box's six planes.
    let sides = |lo: Vec3, hi: Vec3| -> Vec<Surface> {
        (0..3)
            .flat_map(|k| {
                let n = |s: f64| {
                    let mut n = [0.0; 3];
                    n[k] = s;
                    n
                };
                [
                    Surface::Plane { p: lo, n: n(-1.0) },
                    Surface::Plane { p: hi, n: n(1.0) },
                ]
            })
            .collect()
    };
    let up = [0.0, 0.0, 1.0];
    let prism = |pts: &[[f64; 2]], radii: &[f64], height: f64| Primitive::Extrude {
        frame: Frame::at(origin),
        profile: Profile {
            loops: vec![rounded(pts, radii)],
        },
        height,
    };
    let mut splits = Vec::new();
    let (shape, surfaces, edges, volume, area): (Shape, Vec<Surface>, Vec<Meet>, f64, f64) =
        match r.next_u64() % 7 {
            0 => {
                let s = [r.range(1.5, 7.0), r.range(1.5, 7.0), r.range(1.5, 7.0)];
                let hi = add(origin, s);
                let area = 2.0 * (s[0] * s[1] + s[1] * s[2] + s[2] * s[0]);
                let volume = s[0] * s[1] * s[2];
                let edges = square(box_edges(origin, hi));
                (
                    axis_box(origin, hi).into(),
                    sides(origin, hi),
                    edges,
                    volume,
                    area,
                )
            }
            1 => {
                let (rad, h) = (r.range(0.5, 3.0), r.range(1.0, 8.0));
                let cyl = Primitive::Cylinder {
                    frame: Frame::at(origin),
                    radius: rad,
                    height: h,
                };
                let top = [origin[0], origin[1], origin[2] + h];
                let edges = square(vec![rim(origin, rad), rim(top, rad)]);
                let surfaces = vec![
                    Surface::Plane {
                        p: origin,
                        n: [0.0, 0.0, -1.0],
                    },
                    Surface::Plane { p: top, n: up },
                    Surface::Round {
                        c: origin,
                        axis: up,
                        r: rad,
                        out: true,
                    },
                ];
                let area = 2.0 * PI * rad * (rad + h);
                (cyl.into(), surfaces, edges, PI * rad * rad * h, area)
            }
            2 => {
                let rad = r.range(0.5, 5.0);
                let volume = 4.0 / 3.0 * PI * rad.ppowi(3);
                let surfaces = vec![Surface::Ball { c: origin, r: rad }];
                let area = 4.0 * PI * rad * rad;
                (sphere(origin, rad).into(), surfaces, vec![], volume, area)
            }
            3 => {
                let (rad, h) = (r.range(0.5, 3.0), r.range(1.0, 8.0));
                let apex = [origin[0], origin[1], origin[2] + h];
                let surfaces = vec![
                    Surface::Plane {
                        p: origin,
                        n: [0.0, 0.0, -1.0],
                    },
                    Surface::Cone {
                        apex,
                        axis: [0.0, 0.0, -1.0],
                        slope: rad / h,
                    },
                ];
                let volume = PI * rad * rad * h / 3.0;
                let area = PI * rad * (rad + (rad * rad + h * h).sqrt());
                let shape = cone(Frame::at(origin), rad, 0.0, h);
                // The rim's faces turn by a right angle and the cone's lean: π/2 + atan(rad/h).
                let edges = vec![Meet::whole(rim(origin, rad), PI / 2.0 + (rad / h).patan())];
                (shape.into(), surfaces, edges, volume, area)
            }
            4 => {
                let s = [r.range(3.0, 7.0), r.range(3.0, 7.0), r.range(1.5, 6.0)];
                let hi = add(origin, s);
                let rad = s[0].min(s[1]) * r.range(0.15, 0.35);
                let mid = [origin[0] + s[0] / 2.0, origin[1] + s[1] / 2.0];
                let hole = rod(
                    [mid[0], mid[1], origin[2] - 1.0],
                    [mid[0], mid[1], hi[2] + 1.0],
                    rad,
                );
                let shape = Shape::bool(Op::Difference, axis_box(origin, hi), hole);
                let mut creases = box_edges(origin, hi);
                creases.push(rim([mid[0], mid[1], origin[2]], rad));
                creases.push(rim([mid[0], mid[1], hi[2]], rad));
                let mut surfaces = sides(origin, hi);
                surfaces.push(Surface::Round {
                    c: [mid[0], mid[1], origin[2]],
                    axis: up,
                    r: rad,
                    out: false,
                });
                let disc = PI * rad * rad;
                let volume = s[0] * s[1] * s[2] - disc * s[2];
                let area = 2.0 * (s[0] * s[1] + s[1] * s[2] + s[2] * s[0]) - 2.0 * disc
                    + 2.0 * PI * rad * s[2];
                (shape, surfaces, square(creases), volume, area)
            }
            5 => {
                // A w × d × h box with its four upright edges rounded by `rad`: each round is a
                // quarter cylinder about a line `rad` in from two sides, meeting both smoothly.
                let s = [r.range(3.0, 7.0), r.range(3.0, 7.0), r.range(1.5, 6.0)];
                let hi = add(origin, s);
                let rad = s[0].min(s[1]) * r.range(0.1, 0.4);
                let pts = [[0.0, 0.0], [s[0], 0.0], [s[0], s[1]], [0.0, s[1]]];
                let mut surfaces = sides(origin, hi);
                let mut edges = Vec::new();
                for (k, corner) in pts.iter().enumerate() {
                    // The round's centre, and the quarter turn it spans, counter-clockwise from
                    // the side it leaves: corner k's starts at 180° + 90°·k.
                    let c = [
                        origin[0] + if corner[0] == 0.0 { rad } else { s[0] - rad },
                        origin[1] + if corner[1] == 0.0 { rad } else { s[1] - rad },
                    ];
                    let t0 = 0.5 + 0.25 * k as f64;
                    surfaces.push(Surface::Round {
                        c: [c[0], c[1], origin[2]],
                        axis: up,
                        r: rad,
                        out: true,
                    });
                    for z in [origin[2], hi[2]] {
                        edges.push(Meet {
                            curve: rim([c[0], c[1], z], rad),
                            t0,
                            t1: t0 + 0.25,
                            turn: PI / 2.0,
                        });
                    }
                    for t in [t0, t0 + 0.25] {
                        let a = rim([c[0], c[1], origin[2]], rad).at(t);
                        let b = [a[0], a[1], hi[2]];
                        edges.push(Meet::whole(Curve3::Line { a, b }, 0.0));
                    }
                    // The straight run from this round to the next, below and above.
                    let next = pts[(k + 1) % 4];
                    let along = [(next[0] - corner[0]) / 2.0, (next[1] - corner[1]) / 2.0];
                    let len = along[0].abs() + along[1].abs();
                    let inset = |p: [f64; 2], sign: f64| {
                        [
                            origin[0] + p[0] + sign * rad * along[0] / len,
                            origin[1] + p[1] + sign * rad * along[1] / len,
                        ]
                    };
                    let (a, b) = (inset(*corner, 1.0), inset(next, -1.0));
                    for z in [origin[2], hi[2]] {
                        let line = Curve3::Line {
                            a: [a[0], a[1], z],
                            b: [b[0], b[1], z],
                        };
                        edges.push(Meet::whole(line, PI / 2.0));
                    }
                }
                let floor = s[0] * s[1] - (4.0 - PI) * rad * rad;
                let round = 2.0 * (s[0] + s[1]) - 8.0 * rad + 2.0 * PI * rad;
                let shape = prism(&pts, &[rad; 4], s[2]);
                (
                    shape.into(),
                    surfaces,
                    edges,
                    floor * s[2],
                    2.0 * floor + round * s[2],
                )
            }
            _ => {
                // A box drawn as a prism whose outline has a fifth corner partway along one side:
                // a kernel that keeps it makes two faces of that side, on one plane.
                let s = [r.range(3.0, 7.0), r.range(3.0, 7.0), r.range(1.5, 6.0)];
                let hi = add(origin, s);
                let m = s[0] * r.range(0.3, 0.7);
                let pts = [[0.0, 0.0], [m, 0.0], [s[0], 0.0], [s[0], s[1]], [0.0, s[1]]];
                let a = [origin[0] + m, origin[1], origin[2]];
                let b = [a[0], a[1], hi[2]];
                splits.push(Meet::whole(Curve3::Line { a, b }, 0.0));
                let area = 2.0 * (s[0] * s[1] + s[1] * s[2] + s[2] * s[0]);
                let shape = prism(&pts, &[0.0; 5], s[2]);
                (
                    shape.into(),
                    sides(origin, hi),
                    square(box_edges(origin, hi)),
                    s[0] * s[1] * s[2],
                    area,
                )
            }
        };
    let exact = Exact {
        display: Some(DisplayExact {
            surfaces,
            edges,
            splits,
            volume,
            area,
        }),
        ..Default::default()
    };
    (shape.clone(), shape, exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_seen_from_a_corner_shows_nine_edges_and_hides_three() {
        let (lo, hi) = ([0.0; 3], [1.0, 2.0, 3.0]);
        let d = unit3([-1.0, -1.0, -1.0]);
        let ps = pieces(&Body::Boxes(vec![(lo, hi)]), &box_edges(lo, hi), d);
        let seen = ps.iter().filter(|p| p.visible).count();
        assert_eq!((ps.len(), seen), (12, 9));
        // The hidden three meet at the far corner, the one the viewer looks toward.
        for p in ps.iter().filter(|p| !p.visible) {
            assert!(p.curve.at(0.0) == lo);
        }
    }

    #[test]
    fn a_box_behind_another_is_hidden_from_exactly_where_their_edges_cross() {
        // Seen nearly along -x, the back box's drawing lands half over the front box's: each of
        // its edges that crosses the front box's outline is cut there.
        let d = unit3([-1.0, -0.2, -0.1]);
        let (front, back) = (([0.0; 3], [1.0; 3]), ([-4.0, -0.1, 0.2], [-3.0, 0.9, 0.8]));
        let body = Body::Boxes(vec![front, back]);
        let edges = box_edges(back.0, back.1);
        let (u, v) = brep_protocol::view_axes(d);
        let flat = |p: Vec3| {
            [
                (0..3).map(|k| p[k] * u[k]).sum::<f64>(),
                (0..3).map(|k| p[k] * v[k]).sum::<f64>(),
            ]
        };
        let fronts = box_edges(front.0, front.1);
        let mut worst = 0.0f64;
        let mut cuts = 0;
        for p in pieces(&body, &edges, d) {
            for t in [p.t0, p.t1] {
                if t == 0.0 || t == 1.0 {
                    continue;
                }
                cuts += 1;
                // Each cut lies where the back edge's drawing crosses a front edge's.
                let q = flat(p.curve.at(t));
                let gap = fronts
                    .iter()
                    .map(|e| {
                        let Curve3::Line { a, b } = *e else {
                            unreachable!()
                        };
                        let (a, b) = (flat(a), flat(b));
                        let ab = [b[0] - a[0], b[1] - a[1]];
                        let aq = [q[0] - a[0], q[1] - a[1]];
                        (ab[0] * aq[1] - ab[1] * aq[0]).abs() / ab[0].phypot(ab[1])
                    })
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(gap);
            }
        }
        assert!(
            cuts >= 4 && worst < 1e-8,
            "{cuts} cuts, one {worst:e} off every front edge"
        );
    }

    #[test]
    fn a_cylinder_seen_from_above_hides_half_its_bottom_rim() {
        let d = unit3([0.3, 0.2, -1.0]);
        let body = Body::Cylinder {
            c: [0.0; 3],
            r: 1.0,
            h: 2.0,
        };
        let ps = pieces(&body, &cylinder_curves([0.0; 3], 1.0, 2.0, d), d);
        let length = |seen: bool| {
            ps.iter()
                .filter(|p| p.visible == seen)
                .map(|p| match p.curve {
                    Curve3::Circle { r, .. } => TAU * r * (p.t1 - p.t0),
                    Curve3::Line { a, b } => {
                        (p.t1 - p.t0) * (0..3).map(|k| (b[k] - a[k]).ppowi(2)).sum::<f64>().sqrt()
                    }
                })
                .sum::<f64>()
        };
        // Top rim whole, the bottom's near half, both outline runs; the bottom's far half hidden.
        assert!(
            (length(true) - (TAU + PI + 4.0)).abs() < 1e-6,
            "{}",
            length(true)
        );
        assert!((length(false) - PI).abs() < 1e-6, "{}", length(false));
    }
}
