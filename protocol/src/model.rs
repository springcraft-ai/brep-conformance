//! Points, rigid motions and the primitive solids cases are made of, each placed by a frame so a
//! motion moves it without changing its kind.

use std::f64::consts::{PI, TAU};

use crate::profile::Profile;

pub type Vec3 = [f64; 3];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// A rotation (rows of an orthonormal matrix) then a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub r: [[f64; 3]; 3],
    pub t: Vec3,
}

impl Rigid {
    /// The right-handed turn by `angle` about the unit axis `k` through `c`.
    pub fn about(c: Vec3, k: Vec3, angle: f64) -> Rigid {
        let (s, co) = angle.sin_cos();
        let t = 1.0 - co;
        let r = [
            [
                co + k[0] * k[0] * t,
                k[0] * k[1] * t - k[2] * s,
                k[0] * k[2] * t + k[1] * s,
            ],
            [
                k[1] * k[0] * t + k[2] * s,
                co + k[1] * k[1] * t,
                k[1] * k[2] * t - k[0] * s,
            ],
            [
                k[2] * k[0] * t - k[1] * s,
                k[2] * k[1] * t + k[0] * s,
                co + k[2] * k[2] * t,
            ],
        ];
        let turned = Rigid { r, t: [0.0; 3] };
        let rc = turned.rotate(c);
        Rigid {
            r,
            t: [c[0] - rc[0], c[1] - rc[1], c[2] - rc[2]],
        }
    }

    pub const IDENTITY: Rigid = Rigid {
        r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    pub fn rotate(&self, v: Vec3) -> Vec3 {
        let r = &self.r;
        [
            r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
            r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
            r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
        ]
    }

    pub fn point(&self, p: Vec3) -> Vec3 {
        add(self.rotate(p), self.t)
    }

    /// A rotation drawn uniformly from three numbers in [0, 1), by unit quaternion.
    pub fn uniform(u: [f64; 3], t: Vec3) -> Rigid {
        let (a, b) = ((1.0 - u[0]).sqrt(), u[0].sqrt());
        let (t1, t2) = (2.0 * PI * u[1], 2.0 * PI * u[2]);
        let (w, x, y, z) = (a * t1.sin(), a * t1.cos(), b * t2.sin(), b * t2.cos());
        Rigid {
            r: [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y - w * z),
                    2.0 * (x * z + w * y),
                ],
                [
                    2.0 * (x * y + w * z),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z - w * x),
                ],
                [
                    2.0 * (x * z - w * y),
                    2.0 * (y * z + w * x),
                    1.0 - 2.0 * (x * x + y * y),
                ],
            ],
            t,
        }
    }
}

/// An orthonormal frame: an origin and its x, y and z axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame {
    pub fn at(origin: Vec3) -> Frame {
        Frame {
            origin,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        }
    }

    /// The frame with its z along the unit `z` and its x the first world axis not near `z`.
    pub fn along(origin: Vec3, z: Vec3) -> Frame {
        let hint = if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let d = hint[0] * z[0] + hint[1] * z[1] + hint[2] * z[2];
        let x = add(hint, scale(z, -d));
        let n = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let x = scale(x, 1.0 / n);
        let y = [
            z[1] * x[2] - z[2] * x[1],
            z[2] * x[0] - z[0] * x[2],
            z[0] * x[1] - z[1] * x[0],
        ];
        Frame { origin, x, y, z }
    }

    pub fn moved(&self, m: &Rigid) -> Frame {
        Frame {
            origin: m.point(self.origin),
            x: m.rotate(self.x),
            y: m.rotate(self.y),
            z: m.rotate(self.z),
        }
    }
}

/// A primitive solid in its frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    /// The box from the frame's origin out along its axes by `size`.
    Box { frame: Frame, size: Vec3 },
    /// The cylinder about the frame's z axis from its origin up by `height`.
    Cylinder {
        frame: Frame,
        radius: f64,
        height: f64,
    },
    /// The ball about the frame's origin.
    Sphere { frame: Frame, radius: f64 },
    /// The ring about the frame's z axis, its tube's centre circle `major` from the axis.
    Torus {
        frame: Frame,
        major: f64,
        minor: f64,
    },
    /// The frustum about the frame's z axis from its origin up by `height`, its radius `bottom` at
    /// the origin and `top` at the far end; either may be zero, for a cone's apex.
    Cone {
        frame: Frame,
        bottom: f64,
        top: f64,
        height: f64,
    },
    /// The profile, drawn in the frame's xy plane, extruded along its z by `height`.
    Extrude {
        frame: Frame,
        profile: Profile,
        height: f64,
    },
    /// The profile, drawn with x out from the frame's z axis and y along it in the plane of its x,
    /// turned right-handed about z by `angle` (2π for a whole turn).
    Revolve {
        frame: Frame,
        profile: Profile,
        angle: f64,
    },
    /// The profile, drawn as for a revolution, screwed about the frame's z through `angle` while
    /// rising `rise` per radian: a thread.
    Screw {
        frame: Frame,
        profile: Profile,
        rise: f64,
        angle: f64,
    },
    /// The profile, drawn in the frame's xy plane, carried along `path` from the origin, starting
    /// along z, its plane kept square to the path.
    Sweep {
        frame: Frame,
        profile: Profile,
        path: Vec<PathSeg>,
    },
    /// The points over the rectangle `0 ≤ x ≤ size[0]`, `0 ≤ y ≤ size[1]` of the frame's xy plane
    /// from z = 0 up to the height field `z = f(x/size[0], y/size[1])`: a clamped bicubic B-spline
    /// over `[0, 1]²` with knots `knots_u`, `knots_v` and coefficients `heights[i][j]`, all
    /// positive.
    Slab {
        frame: Frame,
        size: [f64; 2],
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        heights: Vec<Vec<f64>>,
    },
    /// The profile, drawn in the frame's xy plane, carried along `path` (in the frame's coordinates,
    /// starting at the origin along z) with its plane square to the path, turned by the path's
    /// rotation-minimizing frame and by `twist` more, evenly in arc length, by the end.
    SweepPath {
        frame: Frame,
        profile: Profile,
        path: crate::path::PathCurve,
        twist: f64,
    },
    /// A ruled loft from the profile in the frame's xy plane to its copy scaled by `scale` about
    /// the origin, `height` up z. `top` is that copy, sent so that a kernel lofting between two
    /// sections is given both and scales nothing.
    Loft {
        frame: Frame,
        profile: Profile,
        scale: f64,
        height: f64,
        top: Profile,
    },
}

/// One piece of a sweep's path: straight on, or bending toward the current frame's x about an axis
/// along its y, `radius` from the path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathSeg {
    Line { length: f64 },
    Arc { radius: f64, angle: f64 },
}

impl PathSeg {
    fn numbers(&self) -> Vec<f64> {
        match *self {
            PathSeg::Line { length } => vec![0.0, length],
            PathSeg::Arc { radius, angle } => vec![1.0, radius, angle],
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match *self {
            PathSeg::Line { length } => serde_json::json!({"line": length}),
            PathSeg::Arc { radius, angle } => {
                serde_json::json!({"arc": {"radius": radius, "angle": angle}})
            }
        }
    }
}

impl Primitive {
    /// A sweep as the extrusions and revolutions it is made of, one per path piece, each in the
    /// world: a straight piece is the profile extruded along the path; a bend is the profile, seen
    /// from the bend's axis, revolved about it. Any other primitive is its own one piece.
    pub fn pieces(&self) -> Vec<Primitive> {
        let Primitive::Sweep {
            frame,
            profile,
            path,
        } = self
        else {
            return vec![self.clone()];
        };
        let mut f = *frame;
        let mut out = Vec::new();
        for seg in path {
            match *seg {
                PathSeg::Line { length } => {
                    out.push(Primitive::Extrude {
                        frame: f,
                        profile: profile.clone(),
                        height: length,
                    });
                    f.origin = add(f.origin, scale(f.z, length));
                }
                PathSeg::Arc { radius, angle } => {
                    // About the axis through `origin + radius·x` along y: a profile point at
                    // (u, v) lies radius − u out from it and v along it, a mirror image, so the
                    // profile is reflected to keep its loops counter-clockwise.
                    let centre = add(f.origin, scale(f.x, radius));
                    let rev = Frame {
                        origin: centre,
                        x: scale(f.x, -1.0),
                        y: f.z,
                        z: f.y,
                    };
                    out.push(Primitive::Revolve {
                        frame: rev,
                        profile: profile.reflected(radius),
                        angle,
                    });
                    let m = Rigid::about(centre, f.y, angle);
                    f = f.moved(&m);
                }
            }
        }
        out
    }
}

/// The cubic B-spline basis functions that are nonzero at `t` over clamped `knots`, with their
/// derivatives: the span's first index, the four values, and the four derivatives.
fn basis(knots: &[f64], t: f64) -> (usize, [f64; 4], [f64; 4]) {
    let n = knots.len() - 4;
    let t = t.clamp(knots[3], knots[n]);
    let mut k = 3;
    while k + 1 < n && knots[k + 1] <= t {
        k += 1;
    }
    // Cox–de Boor up to degree 2, then degree 3 with its derivative.
    let mut nb = [[0.0f64; 4]; 4];
    nb[0][0] = 1.0;
    for p in 1..=3 {
        for j in 0..=p {
            let i = k + j - p;
            let mut v = 0.0;
            if j > 0 {
                let den = knots[i + p] - knots[i];
                if den > 0.0 {
                    v += (t - knots[i]) / den * nb[p - 1][j - 1];
                }
            }
            if j < p {
                let den = knots[i + p + 1] - knots[i + 1];
                if den > 0.0 {
                    v += (knots[i + p + 1] - t) / den * nb[p - 1][j];
                }
            }
            nb[p][j] = v;
        }
    }
    let mut d = [0.0; 4];
    for (j, dj) in d.iter_mut().enumerate() {
        let i = k + j - 3;
        let mut v = 0.0;
        if j > 0 {
            let den = knots[i + 3] - knots[i];
            if den > 0.0 {
                v += 3.0 / den * nb[2][j - 1];
            }
        }
        if j < 3 {
            let den = knots[i + 4] - knots[i + 1];
            if den > 0.0 {
                v -= 3.0 / den * nb[2][j];
            }
        }
        *dj = v;
    }
    (k - 3, nb[3], d)
}

/// A slab's height field `f(u, v)` over `[0, 1]²`.
pub struct HeightField<'a> {
    pub knots_u: &'a [f64],
    pub knots_v: &'a [f64],
    pub heights: &'a [Vec<f64>],
}

const GX5: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683,
    0.0,
    0.538_469_310_105_683,
    0.906_179_845_938_664,
];
const GW5: [f64; 5] = [
    0.236_926_885_056_189,
    0.478_628_670_499_366,
    0.568_888_888_888_889,
    0.478_628_670_499_366,
    0.236_926_885_056_189,
];

/// Gauss points and weights on each span of `knots`, split in `pieces`.
fn spans(knots: &[f64], pieces: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for w in knots.windows(2).filter(|w| w[1] > w[0]) {
        for p in 0..pieces {
            let a = w[0] + (w[1] - w[0]) * p as f64 / pieces as f64;
            let b = w[0] + (w[1] - w[0]) * (p + 1) as f64 / pieces as f64;
            for (x, wt) in GX5.iter().zip(GW5) {
                out.push((0.5 * (a + b) + 0.5 * (b - a) * x, 0.5 * (b - a) * wt));
            }
        }
    }
    out
}

impl HeightField<'_> {
    /// `f`, `∂f/∂u` and `∂f/∂v` at `(u, v)`.
    pub fn eval(&self, u: f64, v: f64) -> (f64, f64, f64) {
        let (iu, bu, du) = basis(self.knots_u, u);
        let (iv, bv, dv) = basis(self.knots_v, v);
        let (mut f, mut fu, mut fv) = (0.0, 0.0, 0.0);
        for a in 0..4 {
            for b in 0..4 {
                let h = self.heights[iu + a][iv + b];
                f += bu[a] * bv[b] * h;
                fu += du[a] * bv[b] * h;
                fv += bu[a] * dv[b] * h;
            }
        }
        (f, fu, fv)
    }

    /// `∫∫ g(f, ∂f/∂u, ∂f/∂v, u, v) du dv` over `[0, 1]²`, by Gauss on every knot span: exact
    /// for polynomials in `f`, `u`, `v` of degree up to 9 a span in each.
    pub fn integrate(&self, g: impl Fn(f64, f64, f64, f64, f64) -> f64) -> f64 {
        self.integrate_in(2, g)
    }

    /// [`Self::integrate`] with `pieces` Gauss rules a knot span: for integrands that are not
    /// polynomials, such as the top's area element.
    pub fn integrate_in(&self, pieces: usize, g: impl Fn(f64, f64, f64, f64, f64) -> f64) -> f64 {
        let (pu, pv) = (spans(self.knots_u, pieces), spans(self.knots_v, pieces));

        let mut sum = 0.0;
        for &(u, wu) in &pu {
            for &(v, wv) in &pv {
                let (f, fu, fv) = self.eval(u, v);
                sum += wu * wv * g(f, fu, fv, u, v);
            }
        }
        sum
    }

    /// `∫ f` along the edge `v = at` (`along_u`) or `u = at`, over `[0, 1]`.
    pub fn edge_integral(&self, along_u: bool, at: f64) -> f64 {
        let knots = if along_u { self.knots_u } else { self.knots_v };
        spans(knots, 2)
            .iter()
            .map(|&(t, w)| {
                let (f, _, _) = if along_u {
                    self.eval(t, at)
                } else {
                    self.eval(at, t)
                };
                w * f
            })
            .sum()
    }
}

impl Primitive {
    /// A slab's height field; `None` for any other kind.
    pub fn height_field(&self) -> Option<HeightField<'_>> {
        match self {
            Primitive::Slab {
                knots_u,
                knots_v,
                heights,
                ..
            } => Some(HeightField {
                knots_u,
                knots_v,
                heights,
            }),
            _ => None,
        }
    }
}

/// A B-spline in a plane over the parameters `knots[degree] ..= knots[ctrl.len()]`: clamped, or
/// periodic with its knots running on past both ends and its first `degree` points repeated last.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    pub degree: usize,
    pub knots: Vec<f64>,
    pub ctrl: Vec<[f64; 2]>,
}

/// The point at `t` of the degree-`p` B-spline over `knots` and `ctrl`, by de Boor's algorithm:
/// the `p + 1` control points of `t`'s span are cut back a level at a time, each level's points
/// lying between neighbours of the level before, until one is left.
pub(crate) fn spline_point<const D: usize>(
    p: usize,
    knots: &[f64],
    ctrl: &[[f64; D]],
    t: f64,
) -> [f64; D] {
    let t = t.clamp(knots[p], knots[ctrl.len()]);
    let span = (p..ctrl.len()).rev().find(|&k| knots[k] <= t).unwrap_or(p);
    let first = span - p;
    let mut level: Vec<[f64; D]> = ctrl[first..=span].to_vec();
    for cut in 1..=p {
        // Point `j` of this level lies between points `j` and `j + 1` of the last, by where
        // `t` falls between the knots that bound both.
        for j in 0..=p - cut {
            let (from, to) = (knots[first + cut + j], knots[span + 1 + j]);
            let share = if to > from {
                (t - from) / (to - from)
            } else {
                0.0
            };
            level[j] =
                std::array::from_fn(|d| (1.0 - share) * level[j][d] + share * level[j + 1][d]);
        }
    }
    level[0]
}

impl Spline {
    /// The point and first derivative at `t`: de Boor's algorithm on the curve, and on its
    /// derivative, a spline one degree lower over the differenced control points and the knots
    /// without their first and last.
    pub fn eval(&self, t: f64) -> ([f64; 2], [f64; 2]) {
        let p = self.degree;
        let k = &self.knots;
        let q: Vec<[f64; 2]> = (0..self.ctrl.len() - 1)
            .map(|i| {
                let den = k[i + p + 1] - k[i + 1];
                let s = if den > 0.0 { p as f64 / den } else { 0.0 };
                let (a, b) = (self.ctrl[i], self.ctrl[i + 1]);
                [s * (b[0] - a[0]), s * (b[1] - a[1])]
            })
            .collect();
        (
            spline_point(p, k, &self.ctrl, t),
            spline_point(p - 1, &k[1..k.len() - 1], &q, t),
        )
    }

    /// The enclosed area, by Green's theorem, and the perimeter: five-point Gauss–Legendre on
    /// eight pieces of every knot span, exact for the area's polynomial integrand.
    pub fn area_and_length(&self) -> (f64, f64) {
        const X: [f64; 5] = [
            -0.906_179_845_938_664,
            -0.538_469_310_105_683,
            0.0,
            0.538_469_310_105_683,
            0.906_179_845_938_664,
        ];
        const W: [f64; 5] = [
            0.236_926_885_056_189,
            0.478_628_670_499_366,
            0.568_888_888_888_889,
            0.478_628_670_499_366,
            0.236_926_885_056_189,
        ];
        let (mut area, mut len) = (0.0, 0.0);
        let n = self.ctrl.len();
        for w in self.knots[self.degree..=n].windows(2) {
            if w[1] <= w[0] {
                continue;
            }
            for piece in 0..8 {
                let a = w[0] + (w[1] - w[0]) * piece as f64 / 8.0;
                let b = w[0] + (w[1] - w[0]) * (piece + 1) as f64 / 8.0;
                for (x, wt) in X.iter().zip(W) {
                    let (q, d) = self.eval(0.5 * (a + b) + 0.5 * (b - a) * x);
                    let h = 0.5 * (b - a) * wt;
                    area += 0.5 * (q[0] * d[1] - q[1] * d[0]) * h;
                    len += d[0].hypot(d[1]) * h;
                }
            }
        }
        (area.abs(), len)
    }
}

impl Primitive {
    /// The function that makes this primitive, by its protocol name.
    pub fn kind(&self) -> &'static str {
        match self {
            Primitive::Box { .. } => "box",
            Primitive::Cylinder { .. } => "cylinder",
            Primitive::Sphere { .. } => "sphere",
            Primitive::Torus { .. } => "torus",
            Primitive::Cone { .. } => "cone",
            Primitive::Extrude { .. } => "extrude",
            Primitive::Revolve { .. } => "revolve",
            Primitive::Screw { .. } => "screw",
            Primitive::Loft { .. } => "loft",
            Primitive::Slab { .. } => "slab",
            Primitive::SweepPath { .. } => "sweep_path",
            Primitive::Sweep { .. } => "sweep",
        }
    }

    /// The same solid as an extrusion or a revolution, for a kernel with no constructor of this
    /// kind: a box and a cylinder extruded, a sphere, a torus and a cone revolved a whole turn.
    pub fn stand_in(&self) -> Option<Primitive> {
        use crate::profile::{Seg, circle};
        let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
        let one = |segs: Vec<Seg>| Profile { loops: vec![segs] };
        Some(match *self {
            Primitive::Box { frame, size } => {
                let (w, d) = (size[0], size[1]);
                let c = [[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]];
                Primitive::Extrude {
                    frame,
                    profile: one((0..4).map(|i| line(c[i], c[(i + 1) % 4])).collect()),
                    height: size[2],
                }
            }
            Primitive::Cylinder {
                frame,
                radius,
                height,
            } => Primitive::Extrude {
                frame,
                profile: one(circle([0.0, 0.0], radius, true)),
                height,
            },
            Primitive::Sphere { frame, radius } => Primitive::Revolve {
                frame,
                profile: one(vec![
                    Seg::Arc {
                        center: [0.0, 0.0],
                        radius,
                        start: -PI / 2.0,
                        sweep: PI,
                    },
                    line([0.0, radius], [0.0, -radius]),
                ]),
                angle: TAU,
            },
            Primitive::Torus {
                frame,
                major,
                minor,
            } => Primitive::Revolve {
                frame,
                profile: one(circle([major, 0.0], minor, true)),
                angle: TAU,
            },
            Primitive::Cone {
                frame,
                bottom,
                top,
                height,
            } => {
                // An apex is one point of the meridian, not a side of no length.
                let mut c = vec![[0.0, 0.0], [bottom, 0.0], [top, height], [0.0, height]];
                c.dedup();
                let n = c.len();
                Primitive::Revolve {
                    frame,
                    profile: one((0..n).map(|i| line(c[i], c[(i + 1) % n])).collect()),
                    angle: TAU,
                }
            }
            _ => return None,
        })
    }

    pub fn frame(&self) -> Frame {
        match self {
            Primitive::Box { frame, .. }
            | Primitive::Cylinder { frame, .. }
            | Primitive::Sphere { frame, .. }
            | Primitive::Torus { frame, .. }
            | Primitive::Cone { frame, .. }
            | Primitive::Extrude { frame, .. }
            | Primitive::Revolve { frame, .. }
            | Primitive::Screw { frame, .. }
            | Primitive::Loft { frame, .. }
            | Primitive::Slab { frame, .. }
            | Primitive::SweepPath { frame, .. }
            | Primitive::Sweep { frame, .. } => *frame,
        }
    }

    /// The primitive's image in the mirror `m`, rows `[M | t]` with `M` a reflection, as a
    /// primitive of the same kind: its frame mirrored and its x turned back, which keeps the frame
    /// right-handed, with what it holds mirrored across that x to match. `None` for the kinds this
    /// is not worked out for.
    pub fn mirrored(&self, m: &[[f64; 4]; 3]) -> Option<Primitive> {
        let turn = |v: Vec3| [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]);
        let f = self.frame();
        let at = turn(f.origin);
        let frame = Frame {
            origin: [0, 1, 2].map(|i| at[i] + m[i][3]),
            x: scale(turn(f.x), -1.0),
            y: turn(f.y),
            z: turn(f.z),
        };
        Some(match self {
            // A box runs from its origin along +x: mirrored, from its far side back.
            Primitive::Box { size, .. } => Primitive::Box {
                frame: Frame {
                    origin: add(frame.origin, scale(frame.x, -size[0])),
                    ..frame
                },
                size: *size,
            },
            // A solid of revolution about its z is its own mirror across x.
            Primitive::Cylinder { radius, height, .. } => Primitive::Cylinder {
                frame,
                radius: *radius,
                height: *height,
            },
            Primitive::Sphere { radius, .. } => Primitive::Sphere {
                frame,
                radius: *radius,
            },
            Primitive::Torus { major, minor, .. } => Primitive::Torus {
                frame,
                major: *major,
                minor: *minor,
            },
            Primitive::Cone {
                bottom,
                top,
                height,
                ..
            } => Primitive::Cone {
                frame,
                bottom: *bottom,
                top: *top,
                height: *height,
            },
            Primitive::Extrude {
                profile, height, ..
            } => Primitive::Extrude {
                frame,
                profile: profile.reflected(0.0),
                height: *height,
            },
            _ => return None,
        })
    }

    pub fn moved(&self, m: &Rigid) -> Primitive {
        let mut p = self.clone();
        let f = self.frame().moved(m);
        match &mut p {
            Primitive::Box { frame, .. }
            | Primitive::Cylinder { frame, .. }
            | Primitive::Sphere { frame, .. }
            | Primitive::Torus { frame, .. }
            | Primitive::Cone { frame, .. }
            | Primitive::Extrude { frame, .. }
            | Primitive::Revolve { frame, .. }
            | Primitive::Screw { frame, .. }
            | Primitive::Loft { frame, .. }
            | Primitive::Slab { frame, .. }
            | Primitive::SweepPath { frame, .. }
            | Primitive::Sweep { frame, .. } => *frame = f,
        }
        p
    }

    pub fn volume(&self) -> f64 {
        match self {
            Primitive::Box { size, .. } => size[0] * size[1] * size[2],
            Primitive::Cylinder { radius, height, .. } => PI * radius * radius * height,
            Primitive::Sphere { radius, .. } => 4.0 / 3.0 * PI * radius.powi(3),
            Primitive::Torus { major, minor, .. } => 2.0 * PI * PI * major * minor * minor,
            Primitive::Cone {
                bottom,
                top,
                height,
                ..
            } => PI * height / 3.0 * (bottom * bottom + bottom * top + top * top),
            Primitive::Extrude {
                profile, height, ..
            } => profile.area() * height,
            // Pappus: each area element travels round at its distance from the axis; a screw's
            // rise moves it within its own plane, which sweeps no volume.
            Primitive::Revolve { profile, angle, .. } | Primitive::Screw { profile, angle, .. } => {
                profile.moment() * angle
            }
            Primitive::Loft {
                profile,
                scale,
                height,
                ..
            } => profile.area() * height * (1.0 + scale + scale * scale) / 3.0,
            Primitive::Sweep { .. } => self.pieces().iter().map(Primitive::volume).sum(),
            Primitive::SweepPath {
                profile,
                path,
                twist,
                ..
            } => crate::path::PathSweep::new(path, profile, *twist).volume(),
            Primitive::Slab { size, .. } => {
                let f = self.height_field().expect("a slab");
                size[0] * size[1] * f.integrate(|z, _, _, _, _| z)
            }
        }
    }

    pub fn area(&self) -> f64 {
        match self {
            Primitive::Box { size: s, .. } => 2.0 * (s[0] * s[1] + s[1] * s[2] + s[2] * s[0]),
            Primitive::Cylinder { radius, height, .. } => 2.0 * PI * radius * (radius + height),
            Primitive::Sphere { radius, .. } => 4.0 * PI * radius * radius,
            Primitive::Torus { major, minor, .. } => 4.0 * PI * PI * major * minor,
            Primitive::Cone {
                bottom,
                top,
                height,
                ..
            } => {
                PI * (bottom + top) * (bottom - top).hypot(*height)
                    + PI * (bottom * bottom + top * top)
            }
            Primitive::Extrude {
                profile, height, ..
            } => 2.0 * profile.area() + profile.perimeter() * height,
            Primitive::Revolve { profile, angle, .. } => {
                let caps = if *angle < TAU {
                    2.0 * profile.area()
                } else {
                    0.0
                };
                profile.x_length() * angle + caps
            }
            Primitive::Screw {
                profile,
                rise,
                angle,
                ..
            } => profile.helix_side(*rise) * angle + 2.0 * profile.area(),
            Primitive::Loft {
                profile,
                scale,
                height,
                ..
            } => profile.area() * (1.0 + scale * scale) + profile.loft_side(*scale, *height),
            Primitive::Slab { size, .. } => {
                let f = self.height_field().expect("a slab");
                let (a, b) = (size[0], size[1]);
                let top = a
                    * b
                    * f.integrate_in(16, |_, fu, fv, _, _| {
                        (1.0 + (fu / a).powi(2) + (fv / b).powi(2)).sqrt()
                    });
                let sides = a * (f.edge_integral(true, 0.0) + f.edge_integral(true, 1.0))
                    + b * (f.edge_integral(false, 0.0) + f.edge_integral(false, 1.0));
                top + sides + a * b
            }
            Primitive::SweepPath {
                profile,
                path,
                twist,
                ..
            } => crate::path::PathSweep::new(path, profile, *twist).area(),
            // Each piece has both end caps; inside the sweep, each joint's two caps are not skin.
            Primitive::Sweep { profile, path, .. } => {
                let joints = path.len().saturating_sub(1) as f64;
                self.pieces().iter().map(Primitive::area).sum::<f64>()
                    - 2.0 * joints * profile.area()
            }
        }
    }

    /// The parameters as one canonical string, for case ids. Each kind keeps the tag its ids were
    /// first made with.
    pub fn key(&self) -> String {
        let f = self.frame();
        let mut parts = vec![f.origin, f.x, f.y, f.z];
        let (kind, extra) = match self {
            Primitive::Box { size, .. } => ("box", size.to_vec()),
            Primitive::Cylinder { radius, height, .. } => ("cyl", vec![*radius, *height]),
            Primitive::Sphere { radius, .. } => ("sph", vec![*radius]),
            Primitive::Torus { major, minor, .. } => ("tor", vec![*major, *minor]),
            Primitive::Cone {
                bottom,
                top,
                height,
                ..
            } => ("cone", vec![*bottom, *top, *height]),
            Primitive::Extrude {
                profile, height, ..
            } => ("ext", [vec![*height], profile.numbers()].concat()),
            Primitive::Revolve { profile, angle, .. } => {
                ("rev", [vec![*angle], profile.numbers()].concat())
            }
            Primitive::Screw {
                profile,
                rise,
                angle,
                ..
            } => ("hel", [vec![*rise, *angle], profile.numbers()].concat()),
            Primitive::Loft {
                profile,
                scale,
                height,
                ..
            } => ("loft", [vec![*scale, *height], profile.numbers()].concat()),
            Primitive::Slab {
                size,
                knots_u,
                knots_v,
                heights,
                ..
            } => {
                let mut v = vec![size[0], size[1]];
                v.extend(knots_u);
                v.push(-1.0);
                v.extend(knots_v);
                v.push(-1.0);
                v.extend(heights.iter().flatten());
                ("slab", v)
            }
            Primitive::SweepPath {
                profile,
                path,
                twist,
                ..
            } => {
                let mut v = vec![path.degree as f64, *twist];
                v.extend(&path.knots);
                v.push(-1.0);
                v.extend(path.ctrl.iter().flatten());
                v.push(-2.0);
                v.extend(profile.numbers());
                ("swpp", v)
            }
            Primitive::Sweep { profile, path, .. } => {
                let mut v: Vec<f64> = path.iter().flat_map(PathSeg::numbers).collect();
                v.push(-2.0);
                v.extend(profile.numbers());
                ("swp", v)
            }
        };
        for c in extra.chunks(3) {
            parts.push([c[0], *c.get(1).unwrap_or(&0.0), *c.get(2).unwrap_or(&0.0)]);
        }
        let nums: Vec<String> = parts
            .iter()
            .flatten()
            .map(|v| format!("{v:.12e}"))
            .collect();
        format!("{kind}:{}", nums.join(","))
    }

    pub fn to_json(&self) -> serde_json::Value {
        let f = self.frame();
        let frame = serde_json::json!({"origin": f.origin, "x": f.x, "y": f.y, "z": f.z});
        match self {
            Primitive::Box { size, .. } => {
                serde_json::json!({"kind": "box", "frame": frame, "size": size})
            }
            Primitive::Cylinder { radius, height, .. } => serde_json::json!({
                "kind": "cylinder", "frame": frame, "radius": radius, "height": height
            }),
            Primitive::Sphere { radius, .. } => {
                serde_json::json!({"kind": "sphere", "frame": frame, "radius": radius})
            }
            Primitive::Torus { major, minor, .. } => serde_json::json!({
                "kind": "torus", "frame": frame, "major": major, "minor": minor
            }),
            Primitive::Cone {
                bottom,
                top,
                height,
                ..
            } => serde_json::json!({
                "kind": "cone", "frame": frame, "bottom": bottom, "top": top, "height": height
            }),
            Primitive::Extrude {
                profile, height, ..
            } => serde_json::json!({
                "kind": "extrude", "frame": frame, "profile": profile.to_json(), "height": height
            }),
            Primitive::Revolve { profile, angle, .. } => serde_json::json!({
                "kind": "revolve", "frame": frame, "profile": profile.to_json(), "angle": angle
            }),
            Primitive::Screw {
                profile,
                rise,
                angle,
                ..
            } => serde_json::json!({
                "kind": "screw", "frame": frame, "profile": profile.to_json(), "rise": rise,
                "angle": angle
            }),
            Primitive::Slab {
                size,
                knots_u,
                knots_v,
                heights,
                ..
            } => serde_json::json!({
                "kind": "slab", "frame": frame, "size": size, "knots_u": knots_u,
                "knots_v": knots_v, "heights": heights
            }),
            Primitive::SweepPath {
                profile,
                path,
                twist,
                ..
            } => serde_json::json!({
                "kind": "sweep_path", "frame": frame, "profile": profile.to_json(),
                "path": {"degree": path.degree, "knots": path.knots, "ctrl": path.ctrl},
                "twist": twist
            }),
            Primitive::Sweep { profile, path, .. } => serde_json::json!({
                "kind": "sweep", "frame": frame, "profile": profile.to_json(),
                "path": path.iter().map(PathSeg::to_json).collect::<Vec<_>>()
            }),
            Primitive::Loft {
                profile,
                scale,
                height,
                top,
                ..
            } => serde_json::json!({
                "kind": "loft", "frame": frame, "profile": profile.to_json(), "scale": scale,
                "height": height, "top": top.to_json()
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Union,
    Intersection,
    Difference,
}

impl Op {
    pub fn word(self) -> &'static str {
        match self {
            Op::Union => "union",
            Op::Intersection => "intersection",
            Op::Difference => "difference",
        }
    }
}

/// Where a point lies against a solid: inside it, on its boundary, or outside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    In,
    On,
    Out,
}

impl Side {
    pub fn word(self) -> &'static str {
        match self {
            Side::In => "in",
            Side::On => "on",
            Side::Out => "out",
        }
    }

    pub fn from_word(word: &str) -> Option<Side> {
        [Side::In, Side::On, Side::Out]
            .into_iter()
            .find(|s| s.word() == word)
    }
}

/// An operand: a primitive, or a boolean of two operands, which the runner builds with the kernel
/// under test, so a case can feed one result into the next operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Prim(Primitive),
    Bool(Op, Box<Shape>, Box<Shape>),
}

impl From<Primitive> for Shape {
    fn from(p: Primitive) -> Shape {
        Shape::Prim(p)
    }
}

impl Shape {
    pub fn bool(op: Op, a: impl Into<Shape>, b: impl Into<Shape>) -> Shape {
        Shape::Bool(op, Box::new(a.into()), Box::new(b.into()))
    }

    pub fn moved(&self, m: &Rigid) -> Shape {
        match self {
            Shape::Prim(p) => Shape::Prim(p.moved(m)),
            Shape::Bool(op, a, b) => Shape::Bool(*op, Box::new(a.moved(m)), Box::new(b.moved(m))),
        }
    }

    /// A primitive's key as it is; a boolean's its operation round its operands' keys.
    pub fn key(&self) -> String {
        match self {
            Shape::Prim(p) => p.key(),
            Shape::Bool(op, a, b) => format!("{}({};{})", op.word(), a.key(), b.key()),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Shape::Prim(p) => p.to_json(),
            Shape::Bool(op, a, b) => serde_json::json!({
                "kind": "boolean", "op": op.word(), "a": a.to_json(), "b": b.to_json()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_rotation_is_orthonormal_and_keeps_handedness() {
        let m = Rigid::uniform([0.3, 0.7, 0.1], [1.0, 2.0, 3.0]);
        for i in 0..3 {
            for j in 0..3 {
                let d: f64 = (0..3).map(|k| m.r[i][k] * m.r[j][k]).sum();
                assert!((d - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        let r = &m.r;
        let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
            - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
            + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
        assert!((det - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_cone_of_radius_three_and_height_four_holds_twelve_pi_in_twenty_four_pi() {
        // A 3-4-5 cone: πr²h/3 = 12π, its side πr·5 = 15π and its base 9π.
        let c = Primitive::Cone {
            frame: Frame::at([0.0; 3]),
            bottom: 0.0,
            top: 3.0,
            height: 4.0,
        };
        assert!((c.volume() - 12.0 * PI).abs() < 1e-12);
        assert!((c.area() - 24.0 * PI).abs() < 1e-12);
    }

    #[test]
    fn a_torus_holds_two_pi_squared_r_rho_squared() {
        // Pappus: the tube's area π·1² swept round a circle of radius 3.
        let t = Primitive::Torus {
            frame: Frame::at([0.0; 3]),
            major: 3.0,
            minor: 1.0,
        };
        assert!((t.volume() - 2.0 * PI * 3.0 * PI).abs() < 1e-12);
    }
}
