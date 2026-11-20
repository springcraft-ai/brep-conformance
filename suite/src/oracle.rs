//! The suite's own answer to what a boolean should hold, sharing no code with any kernel: exact
//! membership in each primitive, and sample points spread evenly through a box.

use std::f64::consts::TAU;

use crate::model::{Frame, Op, Primitive, Shape, Vec3, add, scale};
use crate::portable::Portable;
use crate::profile::Profile;
use brep_protocol::path::{PathCurve, PathSweep};

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn local(f: &Frame, p: Vec3) -> Vec3 {
    let d = sub(p, f.origin);
    [dot(d, f.x), dot(d, f.y), dot(d, f.z)]
}

fn world(f: &Frame, l: Vec3) -> Vec3 {
    add(
        f.origin,
        add(scale(f.x, l[0]), add(scale(f.y, l[1]), scale(f.z, l[2]))),
    )
}

/// An axis-aligned box, `lo` and `hi` corners.
pub type Bounds = (Vec3, Vec3);

fn bounds_of(points: impl IntoIterator<Item = Vec3>) -> Bounds {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

/// The corners of a local box `lo..hi` in the frame, in world coordinates.
fn corners(f: &Frame, lo: Vec3, hi: Vec3) -> Vec<Vec3> {
    (0..8)
        .map(|i| {
            let l = [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ];
            world(f, l)
        })
        .collect()
}

/// A world box holding the shape, or `None` where it is certainly empty.
pub fn bounds(s: &Shape) -> Option<Bounds> {
    match s {
        Shape::Prim(p) => Some(prim_bounds(p)),
        Shape::Bool(Op::Union, a, b) => match (bounds(a), bounds(b)) {
            (Some(x), Some(y)) => Some(bounds_of([x.0, x.1, y.0, y.1])),
            (x, y) => x.or(y),
        },
        Shape::Bool(Op::Intersection, a, b) => overlap(bounds(a)?, bounds(b)?),
        Shape::Bool(Op::Difference, a, _) => bounds(a),
    }
}

/// Whether `p` lies in the shape, by its primitives' exact membership.
pub fn contains(s: &Shape, p: Vec3) -> bool {
    match s {
        Shape::Prim(q) => prim_contains(q, p),
        Shape::Bool(Op::Union, a, b) => contains(a, p) || contains(b, p),
        Shape::Bool(Op::Intersection, a, b) => contains(a, p) && contains(b, p),
        Shape::Bool(Op::Difference, a, b) => contains(a, p) && !contains(b, p),
    }
}

/// A world box holding the primitive.
fn prim_bounds(p: &Primitive) -> Bounds {
    let f = p.frame();
    match p {
        Primitive::Slab { size, heights, .. } => {
            // A B-spline lies in its coefficients' hull.
            let top = heights.iter().flatten().copied().fold(0.0, f64::max);
            bounds_of(corners(&f, [0.0; 3], [size[0], size[1], top]))
        }
        Primitive::SweepPath { profile, path, .. } => {
            // The path lies in its control points' hull; the profile reaches no further from it.
            let (lo, hi) = profile.bounds();
            let reach = [lo[0], lo[1], hi[0], hi[1]]
                .iter()
                .fold(0.0f64, |a, x| a.max(x.abs()));
            let reach = reach * std::f64::consts::SQRT_2;
            let pts: Vec<Vec3> = path
                .ctrl
                .iter()
                .flat_map(|c| corners(&f, c.map(|x| x - reach), c.map(|x| x + reach)))
                .collect();
            bounds_of(pts)
        }
        Primitive::Sweep { .. } => bounds_of(p.pieces().iter().flat_map(|q| {
            let (lo, hi) = prim_bounds(q);
            [lo, hi]
        })),
        Primitive::Box { size, .. } => bounds_of(corners(&f, [0.0; 3], *size)),
        Primitive::Cylinder { radius, height, .. } => bounds_of(corners(
            &f,
            [-radius, -radius, 0.0],
            [*radius, *radius, *height],
        )),
        Primitive::Sphere { radius, .. } => {
            (sub(f.origin, [*radius; 3]), add(f.origin, [*radius; 3]))
        }
        Primitive::Torus { major, minor, .. } => {
            let r = major + minor;
            bounds_of(corners(&f, [-r, -r, -minor], [r, r, *minor]))
        }
        Primitive::Cone {
            bottom,
            top,
            height,
            ..
        } => {
            let r = bottom.max(*top);
            bounds_of(corners(&f, [-r, -r, 0.0], [r, r, *height]))
        }
        Primitive::Extrude {
            profile, height, ..
        } => {
            let (lo, hi) = profile.bounds();
            bounds_of(corners(&f, [lo[0], lo[1], 0.0], [hi[0], hi[1], *height]))
        }
        Primitive::Revolve { profile, .. } => {
            let (lo, hi) = profile.bounds();
            let r = hi[0];
            bounds_of(corners(&f, [-r, -r, lo[1]], [r, r, hi[1]]))
        }
        Primitive::Screw {
            profile,
            rise,
            angle,
            ..
        } => {
            let (lo, hi) = profile.bounds();
            let r = hi[0];
            let risen = rise * angle;
            bounds_of(corners(
                &f,
                [-r, -r, lo[1] + risen.min(0.0)],
                [r, r, hi[1] + risen.max(0.0)],
            ))
        }
        Primitive::Loft {
            profile,
            scale,
            height,
            ..
        } => {
            let (lo, hi) = profile.bounds();
            let s = scale.max(1.0);
            let (lo, hi) = (
                [lo[0].min(lo[0] * s), lo[1].min(lo[1] * s)],
                [hi[0] * s, hi[1] * s],
            );
            bounds_of(corners(&f, [lo[0], lo[1], 0.0], [hi[0], hi[1], *height]))
        }
    }
}

thread_local! {
    /// The last sweeps along a path that membership was asked of, by where their path lies and
    /// their twist: building one carries its frame along the whole path, which a grid of points
    /// must not repeat at every point.
    static SWEEPS: std::cell::RefCell<Vec<(usize, u64, std::rc::Rc<PathSweep>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn swept(path: &PathCurve, profile: &Profile, twist: f64) -> std::rc::Rc<PathSweep> {
    let key = (path.ctrl.as_ptr() as usize, twist.to_bits());
    SWEEPS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, _, s)) = c
            .iter()
            .find(|(p, t, s)| (*p, *t) == key && s.path == *path)
        {
            return s.clone();
        }
        let s = std::rc::Rc::new(PathSweep::new(path, profile, twist));
        c.insert(0, (key.0, key.1, s.clone()));
        c.truncate(4);
        s
    })
}

/// How far `p` lies from the boundary of a box, cylinder, cone, sphere or torus, from inside as
/// from outside, in closed form; `None` for any other primitive.
pub fn boundary_distance(prim: &Primitive, p: Vec3) -> Option<f64> {
    let l = local(&prim.frame(), p);
    let rho = l[0].phypot(l[1]);
    // The distance from (rho, z) to a segment of a meridian.
    let to_segment = |a: [f64; 2], b: [f64; 2]| {
        let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = ux * ux + uy * uy;
        let t = if len2 > 0.0 {
            (((rho - a[0]) * ux + (l[2] - a[1]) * uy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (rho - a[0] - t * ux).phypot(l[2] - a[1] - t * uy)
    };
    // The boundary of a solid of revolution is its meridian's sides off the axis, turned.
    let meridian = |corners: &[[f64; 2]]| {
        corners
            .windows(2)
            .map(|w| to_segment(w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    };
    Some(match prim {
        Primitive::Box { size, .. } => {
            if (0..3).all(|k| l[k] >= 0.0 && l[k] <= size[k]) {
                (0..3)
                    .map(|k| l[k].min(size[k] - l[k]))
                    .fold(f64::INFINITY, f64::min)
            } else {
                let off = [0, 1, 2].map(|k| l[k] - l[k].clamp(0.0, size[k]));
                dot(off, off).sqrt()
            }
        }
        Primitive::Cylinder { radius, height, .. } => meridian(&[
            [0.0, 0.0],
            [*radius, 0.0],
            [*radius, *height],
            [0.0, *height],
        ]),
        Primitive::Cone {
            bottom,
            top,
            height,
            ..
        } => meridian(&[[0.0, 0.0], [*bottom, 0.0], [*top, *height], [0.0, *height]]),
        Primitive::Sphere { radius, .. } => (dot(l, l).sqrt() - radius).abs(),
        Primitive::Torus { major, minor, .. } => ((rho - major).phypot(l[2]) - minor).abs(),
        _ => return None,
    })
}

/// How far `p` lies from the primitive, its inside counted in: nothing where the primitive holds
/// it. `None` where [`boundary_distance`] has no closed form.
pub fn solid_distance(prim: &Primitive, p: Vec3) -> Option<f64> {
    let d = boundary_distance(prim, p)?;
    Some(if prim_contains(prim, p) { 0.0 } else { d })
}

/// Whether `p` lies in the primitive, on the exact geometry (a spline outline to its polygon).
fn prim_contains(prim: &Primitive, p: Vec3) -> bool {
    let l = local(&prim.frame(), p);
    match prim {
        Primitive::Slab { size, .. } => {
            let inside = (0..2).all(|k| l[k] >= 0.0 && l[k] <= size[k]) && l[2] >= 0.0;
            inside && {
                let f = prim.height_field().expect("a slab");
                l[2] <= f.eval(l[0] / size[0], l[1] / size[1]).0
            }
        }
        Primitive::Sweep { .. } => prim.pieces().iter().any(|q| prim_contains(q, p)),
        Primitive::SweepPath {
            profile,
            path,
            twist,
            ..
        } => swept(path, profile, *twist).contains(l),
        Primitive::Box { size, .. } => (0..3).all(|k| l[k] >= 0.0 && l[k] <= size[k]),
        Primitive::Cylinder { radius, height, .. } => {
            l[2] >= 0.0 && l[2] <= *height && l[0] * l[0] + l[1] * l[1] <= radius * radius
        }
        Primitive::Sphere { radius, .. } => dot(l, l) <= radius * radius,
        Primitive::Torus { major, minor, .. } => {
            let rho = l[0].phypot(l[1]) - major;
            rho * rho + l[2] * l[2] <= minor * minor
        }
        Primitive::Cone {
            bottom,
            top,
            height,
            ..
        } => {
            let r = bottom + (top - bottom) * l[2] / height;
            l[2] >= 0.0 && l[2] <= *height && l[0].phypot(l[1]) <= r
        }
        Primitive::Extrude {
            profile, height, ..
        } => l[2] >= 0.0 && l[2] <= *height && profile.contains([l[0], l[1]]),
        Primitive::Revolve { profile, angle, .. } => {
            let phi = l[1].patan2(l[0]).rem_euclid(TAU);
            (*angle >= TAU || phi <= *angle) && profile.contains([l[0].phypot(l[1]), l[2]])
        }
        Primitive::Screw {
            profile,
            rise,
            angle,
            ..
        } => {
            // The point lies on the meridian half-plane at every turn t ≡ its angle; it is in the
            // solid when the profile, risen to that turn, holds it there.
            let (rho, mut t) = (l[0].phypot(l[1]), l[1].patan2(l[0]).rem_euclid(TAU));
            while t <= *angle {
                if profile.contains([rho, l[2] - rise * t]) {
                    return true;
                }
                t += TAU;
            }
            false
        }
        Primitive::Loft {
            profile,
            scale,
            height,
            ..
        } => {
            let s = 1.0 + (scale - 1.0) * l[2] / height;
            l[2] >= 0.0 && l[2] <= *height && s > 0.0 && profile.contains([l[0] / s, l[1] / s])
        }
    }
}

fn radical_inverse(mut i: u64, base: u64) -> f64 {
    let (mut f, mut r) = (1.0, 0.0);
    while i > 0 {
        f /= base as f64;
        r += f * (i % base) as f64;
        i /= base;
    }
    r
}

/// `n` points of the Halton sequence in bases 2, 3 and 5, spread through the box. The sequence is
/// shifted by irrational amounts: unshifted, its points fall on simple fractions of the box, where
/// the cases put faces, and a point on a face is in or out by rounding.
pub fn samples((lo, hi): Bounds, n: usize) -> Vec<Vec3> {
    const SHIFT: [f64; 3] = [
        0.414_213_562_373_095,
        0.732_050_807_568_877,
        0.236_067_977_499_790,
    ];
    (1..=n as u64)
        .map(|i| {
            let u = [
                (radical_inverse(i, 2) + SHIFT[0]).fract(),
                (radical_inverse(i, 3) + SHIFT[1]).fract(),
                (radical_inverse(i, 5) + SHIFT[2]).fract(),
            ];
            [
                lo[0] + (hi[0] - lo[0]) * u[0],
                lo[1] + (hi[1] - lo[1]) * u[1],
                lo[2] + (hi[2] - lo[2]) * u[2],
            ]
        })
        .collect()
}

/// The suite's estimate of the area the shape cuts from the plane through `origin` square to
/// `normal`, and its allowance: `n` points of a shifted Halton sequence over a square of the plane
/// that covers the shape's box, five standard errors and two points' share.
pub fn section_area(s: &Shape, origin: Vec3, normal: Vec3, n: usize) -> (f64, f64) {
    let Some((lo, hi)) = bounds(s) else {
        return (0.0, 0.0);
    };
    let nn = dot(normal, normal).sqrt();
    let z = scale(normal, 1.0 / nn);
    let f = Frame::along(origin, z);
    let half = (0..3).map(|k| (hi[k] - lo[k]).ppowi(2)).sum::<f64>().sqrt()
        + (0..3)
            .map(|k| (0.5 * (lo[k] + hi[k]) - origin[k]).ppowi(2))
            .sum::<f64>()
            .sqrt();
    const SHIFT: [f64; 2] = [0.414_213_562_373_095, 0.732_050_807_568_877];
    let inside = (1..=n as u64)
        .filter(|&i| {
            let u = (radical_inverse(i, 2) + SHIFT[0]).fract() * 2.0 - 1.0;
            let v = (radical_inverse(i, 3) + SHIFT[1]).fract() * 2.0 - 1.0;
            contains(s, world(&f, [u * half, v * half, 0.0]))
        })
        .count() as f64;
    let square = 4.0 * half * half;
    let frac = inside / n as f64;
    let allow = 5.0 * square * (frac * (1.0 - frac) / n as f64).sqrt() + 2.0 * square / n as f64;
    (frac * square, allow)
}

/// Where the ray from `o` along unit `d` crosses the shape's boundary, out to `far`: membership
/// marched in `steps` and each change bisected to 1e-12 of `far`. Returns the crossings, and the
/// shortest run between them, so a caller can tell when a feature thinner than a step could hide.
pub fn crossings(s: &Shape, o: Vec3, d: Vec3, far: f64, steps: usize) -> (Vec<f64>, f64) {
    let at = |t: f64| contains(s, add(o, scale(d, t)));
    let h = far / steps as f64;
    let mut out = Vec::new();
    let mut was = at(0.0);
    for i in 1..=steps {
        let t = h * i as f64;
        let is = at(t);
        if is != was {
            let (mut lo, mut hi) = (t - h, t);
            while hi - lo > far * 1e-12 {
                let mid = 0.5 * (lo + hi);
                if at(mid) == was {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push(0.5 * (lo + hi));
            was = is;
        }
    }
    let gap = out
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(f64::INFINITY, f64::min);
    (out, gap)
}

/// The box both operands' boxes share, or `None` where they miss.
pub fn overlap(a: Bounds, b: Bounds) -> Option<Bounds> {
    let lo = [a.0[0].max(b.0[0]), a.0[1].max(b.0[1]), a.0[2].max(b.0[2])];
    let hi = [a.1[0].min(b.1[0]), a.1[1].min(b.1[1]), a.1[2].min(b.1[2])];
    (0..3).all(|k| hi[k] > lo[k]).then_some((lo, hi))
}

pub fn volume((lo, hi): Bounds) -> f64 {
    (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2])
}

/// Where a primitive's profile turns from one segment to the next, on its side: points of the
/// boundary with the outward normal where the turn is smooth, as a fillet meets a face, and none
/// at a corner. An extrusion's joints are taken halfway up, a revolution's off its seam.
pub fn joints(p: &Primitive) -> Vec<(Vec3, Option<Vec3>)> {
    let (f, profile, revolved) = match p {
        Primitive::Extrude { frame, profile, .. } => (frame, profile, None),
        Primitive::Revolve {
            frame,
            profile,
            angle,
        } if *angle > 1.2 => (frame, profile, Some(1.0f64)),
        _ => return Vec::new(),
    };
    let half = match p {
        Primitive::Extrude { height, .. } => height / 2.0,
        _ => 0.0,
    };
    let mut out = Vec::new();
    for lp in &profile.loops {
        let segs: Vec<_> = lp.iter().filter(|s| s.length() > 1e-9).collect();
        for i in 0..segs.len() {
            let (q, t_in) = segs[i].eval(1.0);
            let (_, t_out) = segs[(i + 1) % segs.len()].eval(0.0);
            let unit = |t: [f64; 2]| {
                let n = t[0].phypot(t[1]);
                [t[0] / n, t[1] / n]
            };
            let (a, b) = (unit(t_in), unit(t_out));
            let smooth =
                (a[0] * b[1] - a[1] * b[0]).abs() < 1e-9 && a[0] * b[0] + a[1] * b[1] > 0.0;
            let n2 = [a[1], -a[0]];
            let (at, n) = match revolved {
                None => (
                    world(f, [q[0], q[1], half]),
                    [0, 1, 2].map(|k| f.x[k] * n2[0] + f.y[k] * n2[1]),
                ),
                Some(phi) => {
                    if q[0] < 1e-9 {
                        continue;
                    }
                    let (s, c) = phi.psin_cos();
                    (
                        world(f, [q[0] * c, q[0] * s, q[1]]),
                        [0, 1, 2].map(|k| f.x[k] * n2[0] * c + f.y[k] * n2[0] * s + f.z[k] * n2[1]),
                    )
                }
            };
            out.push((at, smooth.then_some(n)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn halton_points_in_a_unit_sphere_box_find_its_volume() {
        // 4π/3 of the 8 the box holds, within the five standard errors of a random sample of
        // 20000 that the runner allows: 8·√(p(1−p)/n) with p = π/6 is 0.028.
        let s: Shape = Primitive::Sphere {
            frame: Frame::at([0.0; 3]),
            radius: 1.0,
        }
        .into();
        let b = bounds(&s).unwrap();
        let pts = samples(b, 20_000);
        let f = pts.iter().filter(|&&p| contains(&s, p)).count() as f64 / pts.len() as f64;
        let sigma = volume(b) * (f * (1.0 - f) / pts.len() as f64).sqrt();
        let got = f * volume(b);
        assert!(
            (got - 4.0 / 3.0 * PI).abs() < 5.0 * sigma,
            "{got} ± {sigma}"
        );
    }
}
