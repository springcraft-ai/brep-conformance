//! The classic ways kernels break: intersections through seams and poles, parts far from the
//! origin, surfaces touching along a circle, crossings next to a singular one, fillets at extreme
//! angles, at the limit of fitting and along short edges, and offsets past a hole's radius.

use super::robust::gauss;
use super::*;
use crate::portable::Portable;

/// The box from `lo` to `hi` in `frame`'s local coordinates.
fn framed_box(frame: &Frame, lo: Vec3, hi: Vec3) -> Primitive {
    Primitive::Box {
        frame: Frame {
            origin: framed(frame, lo).0,
            ..*frame
        },
        size: [0, 1, 2].map(|k| hi[k] - lo[k]),
    }
}

/// A cylinder of `radius` up `frame`'s z from local height `z0` to `z1`, about the local point
/// `(x, y)`.
fn framed_rod(frame: &Frame, x: f64, y: f64, z0: f64, z1: f64, radius: f64) -> Primitive {
    Primitive::Cylinder {
        frame: Frame {
            origin: framed(frame, [x, y, z0]).0,
            ..*frame
        },
        radius,
        height: z1 - z0,
    }
}

fn tagged(v: f64, test: &'static str) -> Exact {
    Exact {
        intersection_volume: Some(v),
        variant: Some(test),
        ..Default::default()
    }
}

fn exact_volume(v: f64) -> Exact {
    Exact {
        intersection_volume: Some(v),
        ..Default::default()
    }
}

/// A power of ten from `10^lo` to `10^hi`, even in its exponent.
fn decade(r: &mut Rng, lo: f64, hi: f64) -> f64 {
    10f64.ppowf(r.range(lo, hi))
}

/// Intersections aimed at the places parametrizations degenerate, in a frame turned anyhow: a rod
/// along a ball's polar axis through both poles, half-spaces whose planes hold a ball's, a
/// cylinder's or a ring's seam, a slice a hair deep shaved off a cylinder along its seam, and a rod
/// through a cone's apex. Each intersection is a closed form.
pub(super) fn seams_poles(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let f = turned_frame(r, origin);
    match r.next_u64() % 7 {
        0 => {
            let rad = r.range(1.0, 5.0);
            let rho = rad * r.range(0.1, 0.9);
            let ball = Primitive::Sphere {
                frame: f,
                radius: rad,
            };
            let rod = framed_rod(&f, 0.0, 0.0, -rad - 1.0, rad + 1.0, rho);
            let v = 4.0 / 3.0 * PI * (rad.ppowi(3) - (rad * rad - rho * rho).ppowf(1.5));
            (
                ball,
                rod,
                tagged(v, "booleans/degenerate/rod-through-sphere-poles"),
            )
        }
        1 => {
            let rad = r.range(1.0, 5.0);
            let ball = Primitive::Sphere {
                frame: f,
                radius: rad,
            };
            let e = rad + 1.0;
            let half = framed_box(&f, [-e, 0.0, -e], [e, e, e]);
            (
                ball,
                half,
                tagged(
                    2.0 / 3.0 * PI * rad.ppowi(3),
                    "booleans/degenerate/sphere-cut-along-its-seam",
                ),
            )
        }
        2 => {
            let (rad, h) = (r.range(1.0, 5.0), r.range(1.0, 8.0));
            let cyl = Primitive::Cylinder {
                frame: f,
                radius: rad,
                height: h,
            };
            let e = rad + 1.0;
            let half = framed_box(&f, [-e, 0.0, -1.0], [e, e, h + 1.0]);
            (
                cyl,
                half,
                tagged(
                    PI * rad * rad * h / 2.0,
                    "booleans/degenerate/cylinder-cut-along-its-seam",
                ),
            )
        }
        3 => {
            let (rad, h) = (r.range(1.0, 5.0), r.range(1.0, 8.0));
            let depth = rad * decade(r, -6.0, -1.0);
            let cyl = Primitive::Cylinder {
                frame: f,
                radius: rad,
                height: h,
            };
            let e = rad + 1.0;
            let slice = framed_box(&f, [rad - depth, -e, -1.0], [e, e, h + 1.0]);
            let a = rad - depth;
            let seg =
                rad * rad * (a / rad).pacos() - a * (2.0 * rad * depth - depth * depth).sqrt();
            (
                cyl,
                slice,
                tagged(seg * h, "booleans/degenerate/cylinder-shaved-at-its-seam"),
            )
        }
        k @ (4 | 5) => {
            let minor = r.range(0.3, 1.5);
            let major = minor + r.range(0.5, 4.0);
            let ring = Primitive::Torus {
                frame: f,
                major,
                minor,
            };
            let e = major + minor + 1.0;
            let half = if k == 4 {
                framed_box(&f, [-e, 0.0, -e], [e, e, e])
            } else {
                framed_box(&f, [-e, -e, 0.0], [e, e, e])
            };
            (
                ring,
                half,
                tagged(
                    PI * PI * major * minor * minor,
                    if k == 4 {
                        "booleans/degenerate/torus-cut-across-its-seam"
                    } else {
                        "booleans/degenerate/torus-cut-along-its-seam"
                    },
                ),
            )
        }
        _ => {
            let (bottom, h) = (r.range(1.0, 5.0), r.range(1.0, 8.0));
            let rho = bottom * r.range(0.1, 0.9);
            let point = cone(f, bottom, 0.0, h);
            let rod = framed_rod(&f, 0.0, 0.0, -1.0, h + 1.0, rho);
            let z = h * (1.0 - rho / bottom);
            let v = PI * rho * rho * z + PI / 3.0 * rho * rho * (h - z);
            (
                point,
                rod,
                tagged(v, "booleans/degenerate/rod-through-a-cone-apex"),
            )
        }
    }
}

/// A case of an exact family turned anyhow and moved 10³ to 10⁶ mm from the origin, against the
/// same absolute tolerance: a part a few mm across there keeps ten fewer bits of its size.
pub(super) fn far_away(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let ((a, b, e), variant) = match r.next_u64() % 5 {
        0 => (sphere_sphere(r), "booleans/scale/far-from-origin-spheres"),
        1 => (
            steinmetz(r),
            "booleans/scale/far-from-origin-equal-cylinders",
        ),
        2 => (box_box(r), "booleans/scale/far-from-origin-boxes"),
        3 => (
            cone_cylinder_coaxial(r),
            "booleans/scale/far-from-origin-cone-and-cylinder",
        ),
        _ => (
            crossing_cylinders(r),
            "booleans/scale/far-from-origin-crossing-cylinders",
        ),
    };
    let dir = r.direction();
    let far = decade(r, 3.0, 6.0);
    let m = Rigid::uniform([r.unit(), r.unit(), r.unit()], dir.map(|x| x * far));
    let exact = Exact {
        intersection_volume: e.intersection_volume,
        intersection_area: e.intersection_area,
        distance: e.distance,
        variant: Some(variant),
        ..Default::default()
    };
    (a.moved(&m), b.moved(&m), exact)
}

/// Surfaces that touch along a circle instead of crossing, or miss or overlap it by a hair: a ball
/// in a cylinder of its radius, against it round its equator; a ball in a cone, against its side;
/// a ring round a cylinder, against it round its inner equator; a ball resting in a ring's hole.
/// Overlapping, a ball and cylinder share the ball less two caps, a ring and cylinder the
/// revolution of the tube's slice inside the cylinder, `2π(R·A − (2/3)(r² − a²)^(3/2))` by Pappus.
pub(super) fn circle_contact(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let f = turned_frame(r, origin);
    // Touching exactly, apart by a hair, or overlapping by one.
    let hair = match r.next_u64() % 3 {
        0 => 0.0,
        1 => decade(r, -6.0, -2.0),
        _ => -decade(r, -6.0, -2.0),
    };
    match r.next_u64() % 4 {
        0 => {
            let rad = r.range(1.0, 4.0);
            let h = 2.0 * rad + r.range(1.0, 4.0);
            let cyl = Primitive::Cylinder {
                frame: f,
                radius: rad,
                height: h,
            };
            let s = rad * (1.0 - hair);
            let ball = Primitive::Sphere {
                frame: Frame {
                    origin: framed(&f, [0.0, 0.0, h / 2.0]).0,
                    ..f
                },
                radius: s,
            };
            let v = if s <= rad {
                4.0 / 3.0 * PI * s.ppowi(3)
            } else {
                4.0 / 3.0 * PI * (s.ppowi(3) - (s * s - rad * rad).ppowf(1.5))
            };
            (cyl, ball, exact_volume(v))
        }
        1 => {
            let (bottom, h) = (r.range(2.0, 5.0), r.range(4.0, 9.0));
            let slant = (1.0 / (bottom * bottom) + 1.0 / (h * h)).sqrt();
            let s = (h / (1.0 + slant * h)) * r.range(0.3, 0.8);
            let z = h * (1.0 - s * slant);
            let point = cone(f, bottom, 0.0, h);
            // Only touching or apart: a ball pushed through a cone's side is not a closed form.
            let s = s * (1.0 - hair.abs());
            let ball = Primitive::Sphere {
                frame: Frame {
                    origin: framed(&f, [0.0, 0.0, z]).0,
                    ..f
                },
                radius: s,
            };
            (point, ball, exact_volume(4.0 / 3.0 * PI * s.ppowi(3)))
        }
        2 => {
            let minor = r.range(0.3, 1.5);
            let major = minor + r.range(1.0, 4.0);
            let ring = Primitive::Torus {
                frame: f,
                major,
                minor,
            };
            let c = (major - minor) * (1.0 - hair);
            let cyl = framed_rod(&f, 0.0, 0.0, -minor - 1.0, minor + 1.0, c);
            let a = major - c;
            let v = if a >= minor {
                0.0
            } else {
                let seg = minor * minor * (a / minor).pacos() - a * (minor * minor - a * a).sqrt();
                TAU * (major * seg - 2.0 / 3.0 * (minor * minor - a * a).ppowf(1.5))
            };
            (ring, cyl, exact_volume(v))
        }
        _ => {
            let minor = r.range(0.3, 1.5);
            let major = minor + r.range(1.0, 4.0);
            let ring = Primitive::Torus {
                frame: f,
                major,
                minor,
            };
            let s = (major - minor) * r.range(1.2, 2.0);
            let z = ((s + minor).ppowi(2) - major * major).sqrt();
            let s = s * (1.0 - hair.abs());
            let ball = Primitive::Sphere {
                frame: Frame {
                    origin: framed(&f, [0.0, 0.0, z]).0,
                    ..f
                },
                radius: s,
            };
            (ring, ball, exact_volume(0.0))
        }
    }
}

/// Two equal cylinders crossing square with their axes missing each other by 10⁻⁶ to 10⁻² of the
/// radius: next to the Steinmetz pair, whose intersection curves cross each other at two points,
/// so the curves pass within a hair of crossing. The shared volume is
/// `∫ 4√(r² − z²)√(r² − (z − ε)²) dz` over `ε − r ≤ z ≤ r`.
pub(super) fn near_steinmetz(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rad = r.range(1.0, 6.0);
    let eps = rad * decade(r, -6.0, -2.0);
    let h = 4.0 * rad;
    let a = Primitive::Cylinder {
        frame: Frame::along([-h / 2.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        radius: rad,
        height: h,
    };
    let b = Primitive::Cylinder {
        frame: Frame::along([0.0, -h / 2.0, eps], [0.0, 1.0, 0.0]),
        radius: rad,
        height: h,
    };
    let (lo, hi) = (eps - rad, rad);
    let v = gauss(0.0, PI, 128, |theta, wt| {
        let z = lo + (hi - lo) * (1.0 - theta.pcos()) / 2.0;
        let wa = (rad * rad - z * z).max(0.0).sqrt();
        let wb = (rad * rad - (z - eps) * (z - eps)).max(0.0).sqrt();
        wt * 4.0 * wa * wb * (hi - lo) / 2.0 * theta.psin()
    });
    (a, b, exact_volume(v))
}

fn prism_of(pts: &[[f64; 2]], h: f64) -> Primitive {
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(pts),
        height: h,
    }
}

fn rounded_prism_of(pts: &[[f64; 2]], radii: &[f64], h: f64) -> Primitive {
    // A zero-length run between two rounds that meet is no edge.
    let lp = rounded(pts, radii)
        .into_iter()
        .filter(|s| match s {
            Seg::Line { a, b } => (b[0] - a[0]).phypot(b[1] - a[1]) > 0.0,
            _ => true,
        })
        .collect();
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile { loops: vec![lp] },
        height: h,
    }
}

fn fillet_of(points: Vec<Vec3>, size: f64) -> Feature {
    Feature {
        act: Act::Fillet,
        points,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
        motion: None,
    }
}

/// One side edge filleted where its faces meet nearly flat, 0.06° to 6° off, the round's radius
/// hundreds of times its reach along them; or at a knife edge of 2° to 15°, the round tiny against
/// the faces. Either way the result is the prism on the profile with that corner rounded.
pub(super) fn extreme_angle(r: &mut Rng) -> FeatureDraw {
    let h = r.range(2.0, 8.0);
    let z = h * r.range(0.2, 0.8);
    if r.next_u64().is_multiple_of(2) {
        let (w, top) = (r.range(4.0, 10.0), r.range(3.0, 8.0));
        let bend = decade(r, -3.0, -1.0);
        let peak = [w / 2.0, top + w / 2.0 * bend.ptan()];
        let pts = [[0.0, 0.0], [w, 0.0], [w, top], peak, [0.0, top]];
        let rad = 0.45 * (w / 2.0) * r.range(0.2, 1.0) / bend.ptan();
        let want = rounded_prism_of(&pts, &[0.0, 0.0, 0.0, rad, 0.0], h);
        let f = fillet_of(vec![[peak[0], peak[1], z]], rad);
        let exact = Exact {
            variant: Some("fillets/nearly-flat-edge"),
            ..Default::default()
        };
        (prism_of(&pts, h), f, want.into(), exact)
    } else {
        let len = r.range(4.0, 10.0);
        let alpha = r.range(2.0, 15.0).to_radians();
        let pts = [[0.0, 0.0], [len, 0.0], [len, len * alpha.ptan()]];
        let rad = 0.45 * len * r.range(0.2, 1.0) * (alpha / 2.0).ptan();
        let want = rounded_prism_of(&pts, &[rad, 0.0, 0.0], h);
        let f = fillet_of(vec![[0.0, 0.0, z]], rad);
        let exact = Exact {
            variant: Some("fillets/knife-edge"),
            ..Default::default()
        };
        (prism_of(&pts, h), f, want.into(), exact)
    }
}

/// A rib with both edges along its top filleted by half its width, so the two rounds meet and the
/// top face is gone, or by a hair less, a sliver of top 10⁻⁹ to 10⁻³ of the width left.
pub(super) fn fillet_limit(r: &mut Rng) -> FeatureDraw {
    let (w, top, h) = (r.range(1.0, 5.0), r.range(4.0, 10.0), r.range(2.0, 8.0));
    let (rad, test) = if r.next_u64().is_multiple_of(2) {
        (w / 2.0, "fillets/rib-radius-exactly-half-width")
    } else {
        (
            w / 2.0 * (1.0 - decade(r, -9.0, -3.0)),
            "fillets/rib-radius-just-under-half-width",
        )
    };
    let pts = [[0.0, 0.0], [w, 0.0], [w, top], [0.0, top]];
    let z = h * r.range(0.2, 0.8);
    let want = rounded_prism_of(&pts, &[0.0, 0.0, rad, rad], h);
    let f = fillet_of(vec![[w, top, z], [0.0, top, z]], rad);
    let exact = Exact {
        variant: Some(test),
        ..Default::default()
    };
    (prism_of(&pts, h), f, want.into(), exact)
}

/// As [`fillet_limit`], a hair over half the width: no ball of that radius fits across the rib.
pub(super) fn fillet_over_limit(r: &mut Rng) -> FeatureDraw {
    let (w, top, h) = (r.range(1.0, 5.0), r.range(4.0, 10.0), r.range(2.0, 8.0));
    let rad = w / 2.0 * (1.0 + decade(r, -6.0, -1.0));
    let pts = [[0.0, 0.0], [w, 0.0], [w, top], [0.0, top]];
    let z = h * r.range(0.2, 0.8);
    let base = prism_of(&pts, h);
    let f = fillet_of(vec![[w, top, z], [0.0, top, z]], rad);
    (base.clone(), f, base.into(), Exact::default())
}

/// A convex prism with one corner cut into a face 10⁻⁵ to 10⁻² of its size, every side edge
/// filleted at its own radius in one operation, the two at the ends of that face by rounds small
/// enough to fit it. The result is the prism on the profile with each corner rounded at its radius.
pub(super) fn short_edges(r: &mut Rng) -> FeatureDraw {
    let base_pts = convex_polygon(r);
    let n = base_pts.len();
    let i = (r.next_u64() % n as u64) as usize;
    let size = base_pts
        .iter()
        .flat_map(|p| p.map(f64::abs))
        .fold(0.0, f64::max);
    let cut = size * decade(r, -5.0, -2.0);
    let mut cuts = vec![0.0; n];
    cuts[i] = cut;
    // The corner replaced by the two ends of a cut `cut` back along each side.
    let pts: Vec<[f64; 2]> = chamfered(&base_pts, &cuts)
        .iter()
        .filter_map(|s| match s {
            Seg::Line { a, b } if (b[0] - a[0]).phypot(b[1] - a[1]) > 0.0 => Some(*a),
            _ => None,
        })
        .collect();
    let m = pts.len();
    let c = corners(&pts);
    let radii: Vec<f64> = (0..m)
        .map(|j| {
            let (turn, side) = c[j];
            0.45 * side * r.range(0.2, 1.0) / (turn.abs() / 2.0).ptan()
        })
        .collect();
    let h = r.range(2.0, 8.0);
    let z = h * r.range(0.2, 0.8);
    let chosen: Vec<usize> = (0..m).filter(|&j| radii[j] > 0.0).collect();
    let want = rounded_prism_of(&pts, &radii, h);
    let f = Feature {
        act: Act::FilletRadii,
        points: chosen.iter().map(|&j| [pts[j][0], pts[j][1], z]).collect(),
        size: radii[chosen[0]],
        pull: None,
        radii: chosen.iter().map(|&j| radii[j]).collect(),
        matrix: None,
        motion: None,
    };
    (prism_of(&pts, h), f, want.into(), Exact::default())
}

/// A convex prism with its sharpest corner cut into a face 10⁻⁵ to 10⁻² of its size, the edges at
/// both ends of that face filleted at one radius far wider than it. A ball that size cannot touch
/// the face, so the round rolls on the two faces beyond it and the face goes: the result is the
/// prism on the uncut profile with that corner rounded. The radius keeps the round's arc inside the
/// cut and its contacts past the cut's ends, so nothing of the face is left.
pub(super) fn swallowed_face(r: &mut Rng) -> FeatureDraw {
    let base_pts = convex_polygon(r);
    let n = base_pts.len();
    let c = corners(&base_pts);
    let i = (0..n)
        .max_by(|&a, &b| c[a].0.abs().total_cmp(&c[b].0.abs()))
        .expect("a polygon has corners");
    let size = base_pts
        .iter()
        .flat_map(|p| p.map(f64::abs))
        .fold(0.0, f64::max);
    let (turn, side) = (c[i].0.abs(), c[i].1);
    let (sin, cos, tan) = (
        (turn / 2.0).psin(),
        (turn / 2.0).pcos(),
        (turn / 2.0).ptan(),
    );
    let most = 0.45 * side / tan;
    // The cut's chord lies cut·sin(θ/2) in from the corner and the round's arc r(1/cos(θ/2) − 1);
    // its contacts reach r·tan(θ/2) along each side. Both at least twice the cut's.
    let need = |cut: f64| (2.0 * cut * sin * cos / (1.0 - cos)).max(2.0 * cut / tan);
    let mut cut = size * decade(r, -5.0, -2.0);
    while need(cut) > 0.5 * most {
        cut /= 10.0;
    }
    let rad = need(cut) * (most / need(cut)).ppowf(r.range(0.3, 1.0));
    let mut cuts = vec![0.0; n];
    cuts[i] = cut;
    let pts: Vec<[f64; 2]> = chamfered(&base_pts, &cuts)
        .iter()
        .filter_map(|s| match s {
            Seg::Line { a, b } if (b[0] - a[0]).phypot(b[1] - a[1]) > 0.0 => Some(*a),
            _ => None,
        })
        .collect();
    let h = r.range(2.0, 8.0);
    let z = h * r.range(0.2, 0.8);
    // The cut face's two ends: the points of the cut profile within the cut of the old corner.
    let near: Vec<Vec3> = pts
        .iter()
        .filter(|p| (p[0] - base_pts[i][0]).phypot(p[1] - base_pts[i][1]) <= 1.5 * cut)
        .map(|p| [p[0], p[1], z])
        .collect();
    assert_eq!(near.len(), 2, "the cut leaves two ends");
    let mut radii = vec![0.0; n];
    radii[i] = rad;
    let want = rounded_prism_of(&base_pts, &radii, h);
    (
        prism_of(&pts, h),
        fillet_of(near, rad),
        want.into(),
        Exact::default(),
    )
}

/// The volume and area a block drilled through by a hole of radius ρ along z gains offset by `d`.
/// Below ρ, Steiner–Federer: with mean-curvature measure `M = π(a + b + c) + π²ρ − πc` (box
/// edges, the hole's two convex rims, its concave wall) and Euler characteristic 0,
/// `V = V₀ + S₀d + Md²`, `S = S₀ + 2Md`. Past ρ the hole fills, but over each end the offset dips
/// where the rim is nearer than the top would have been: the rounded block less two dimples, each
/// `2π∫₀^ρ r(d − √(d² − (ρ − r)²)) dr`.
fn drilled_offset(a: f64, b: f64, c: f64, rho: f64, d: f64) -> (f64, f64) {
    if d < rho {
        let v0 = a * b * c - PI * rho * rho * c;
        let s0 = 2.0 * (a * b + b * c + c * a) - 2.0 * PI * rho * rho + TAU * rho * c;
        let m = PI * (a + b + c) + PI * PI * rho - PI * c;
        return (v0 + s0 * d + m * d * d, s0 + 2.0 * m * d);
    }
    let (_, vb, sb) = rounded_block([0.0; 3], [a, b, c], d);
    let q = (d * d - rho * rho).sqrt();
    let dimple = TAU
        * (d * rho * rho / 2.0 - rho / 2.0 * (rho * q + d * d * (rho / d).pasin())
            + (d.ppowi(3) - q.ppowi(3)) / 3.0);
    let phi = (rho / d).pasin();
    let dimple_area = TAU * d * (rho * phi - d * (1.0 - phi.pcos()));
    (
        vb - 2.0 * dimple,
        sb - 2.0 * PI * rho * rho + 2.0 * dimple_area,
    )
}

/// A block drilled through along z, offset outward by a distance below the hole's radius, the hole
/// narrowing to ρ − d with its rims rounded, or past it, the hole filled save a dimple at each end.
pub(super) fn offset_past_hole(r: &mut Rng) -> FeatureDraw {
    let (a, b, c) = (r.range(4.0, 9.0), r.range(4.0, 9.0), r.range(2.0, 6.0));
    let rho = a.min(b) * r.range(0.1, 0.3);
    let d = if r.next_u64().is_multiple_of(2) {
        rho * r.range(0.2, 0.95)
    } else {
        rho * r.range(1.05, 2.0)
    };
    let (mx, my) = (a / 2.0, b / 2.0);
    let (block, _, _) = rounded_block([0.0; 3], [a, b, c], d);
    // What the offset leaves open: the narrowed hole, and over each end what lies farther than d
    // from the rim.
    let dimple = |z0: f64, up: bool| {
        let (lo, hi) = if up {
            (z0, z0 + d + 1.0)
        } else {
            (z0 - d - 1.0, z0)
        };
        Shape::bool(
            Op::Difference,
            Primitive::Cylinder {
                frame: Frame::at([mx, my, lo]),
                radius: rho,
                height: hi - lo,
            },
            Primitive::Torus {
                frame: Frame::at([mx, my, z0]),
                major: rho,
                minor: d,
            },
        )
    };
    let mut open = Shape::bool(Op::Union, dimple(c, true), dimple(0.0, false));
    if d < rho {
        open = Shape::bool(
            Op::Union,
            open,
            Primitive::Cylinder {
                frame: Frame::at([mx, my, -d - 1.0]),
                radius: rho - d,
                height: c + 2.0 * d + 2.0,
            },
        );
    }
    let want = Shape::bool(Op::Difference, block, open);
    let (v, s) = drilled_offset(a, b, c, rho, d);
    let f = Feature {
        act: Act::Offset,
        points: Vec::new(),
        size: d,
        pull: None,
        radii: Vec::new(),
        matrix: None,
        motion: None,
    };
    let exact = Exact {
        result_volume: Some(v),
        result_area: Some(s),
        ..Default::default()
    };
    // The drilled block as one extrusion: a rectangle with a round hole.
    let profile = Profile {
        loops: vec![
            rounded(&[[0.0, 0.0], [a, 0.0], [a, b], [0.0, b]], &[0.0; 4]),
            vec![Seg::Arc {
                center: [mx, my],
                radius: rho,
                start: 0.0,
                sweep: -TAU,
            }],
        ],
    };
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile,
        height: c,
    };
    (base, f, want, exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sampled(s: &Shape, n: usize) -> f64 {
        let (lo, hi) = crate::oracle::bounds(s).unwrap();
        let held = crate::oracle::samples((lo, hi), n)
            .iter()
            .filter(|p| crate::oracle::contains(s, **p))
            .count();
        held as f64 / n as f64 * (0..3).map(|k| hi[k] - lo[k]).product::<f64>()
    }

    #[test]
    fn a_swallowing_round_takes_the_whole_tiny_face_and_stays_inside() {
        // The rounded corner lies within the cut prism, and the cut face's middle and both its
        // ends lie outside it: the round passes inside all of that face.
        let mut r = Rng::new(11);
        for _ in 0..200 {
            let (a, f, want, _) = swallowed_face(&mut r);
            let Primitive::Extrude {
                profile, height, ..
            } = &a
            else {
                unreachable!()
            };
            let Shape::Prim(Primitive::Extrude {
                profile: rounded, ..
            }) = &want
            else {
                unreachable!()
            };
            let (p, q) = (f.points[0], f.points[1]);
            for t in [0.0, 0.5, 1.0] {
                let m = [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])];
                assert!(!rounded.contains(m), "{m:?}");
            }
            // A hair in from each point of the rounded outline, toward the convex profile's middle.
            let pts: Vec<[f64; 2]> = rounded.loops[0]
                .iter()
                .flat_map(|seg| (0..=16).map(|k| seg.eval(k as f64 / 16.0).0))
                .collect();
            let mid = [0, 1].map(|k| pts.iter().map(|p| p[k]).sum::<f64>() / pts.len() as f64);
            for x in pts {
                let y = [0, 1].map(|k| x[k] + (mid[k] - x[k]) * 1e-7);
                assert!(profile.contains(y), "{x:?} {height}");
            }
        }
    }

    #[test]
    fn an_offset_drilled_block_holds_its_closed_form_either_side_of_the_hole() {
        let mut r = Rng::new(17);
        let (mut below, mut above) = (0, 0);
        for _ in 0..12 {
            let (base, f, want, exact) = offset_past_hole(&mut r);
            let v = exact.result_volume.unwrap();
            let got = sampled(&want, 80_000);
            assert!(
                (got - v).abs() < 0.01 * v,
                "d {}: {got} against {v}",
                f.size
            );
            let Primitive::Extrude { profile, .. } = base else {
                panic!("an extrusion")
            };
            let Seg::Arc { radius: rho, .. } = profile.loops[1][0] else {
                panic!("a hole")
            };
            if f.size < rho {
                below += 1;
            } else {
                above += 1;
            }
        }
        assert!(below > 0 && above > 0);
        // The two closed forms meet where the hole closes.
        let (a, b, c, rho) = (6.0, 5.0, 3.0, 1.2);
        let (v1, s1) = drilled_offset(a, b, c, rho, rho * (1.0 - 1e-9));
        let (v2, s2) = drilled_offset(a, b, c, rho, rho * (1.0 + 1e-9));
        assert!(
            (v1 - v2).abs() < 1e-6 && (s1 - s2).abs() < 1e-6,
            "{v1} {v2} {s1} {s2}"
        );
    }

    #[test]
    fn near_steinmetz_meets_steinmetz_as_the_axes_meet() {
        let rad = 2.0;
        let v = |eps: f64| {
            let (lo, hi) = (eps - rad, rad);
            gauss(0.0, PI, 128, |theta, wt| {
                let z = lo + (hi - lo) * (1.0 - theta.pcos()) / 2.0;
                let wa = (rad * rad - z * z).max(0.0).sqrt();
                let wb = (rad * rad - (z - eps) * (z - eps)).max(0.0).sqrt();
                wt * 4.0 * wa * wb * (hi - lo) / 2.0 * theta.psin()
            })
        };
        let exact = 16.0 * rad.ppowi(3) / 3.0;
        assert!(
            (v(1e-12) - exact).abs() < 1e-9 * exact,
            "{} against {exact}",
            v(1e-12)
        );
        assert!(v(1e-3) < exact);
    }

    #[test]
    fn every_hard_pair_s_closed_form_meets_a_sample() {
        let mut r = Rng::new(23);
        for draw in [seams_poles, circle_contact, far_away, near_steinmetz] {
            for _ in 0..12 {
                let (a, b, e) = draw(&mut r);
                let both = Shape::bool(Op::Intersection, a, b);
                // A far pair drawn from crossing cylinders states no volume to check.
                let Some(want) = e.intersection_volume else {
                    continue;
                };
                let Some(_) = crate::oracle::bounds(&both) else {
                    assert!(want.abs() < 1e-9);
                    continue;
                };
                let got = sampled(&both, 60_000);
                let scale = crate::oracle::bounds(&both)
                    .map(|(lo, hi)| (0..3).map(|k| hi[k] - lo[k]).product::<f64>())
                    .unwrap();
                assert!(
                    (got - want).abs() < 0.01 * want.max(0.0) + 2e-3 * scale,
                    "{got} against {want}"
                );
            }
        }
    }
}
