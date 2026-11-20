//! A profile swept along a space curve: the curve, its rotation-minimizing frame, and the closed
//! forms the sweep's measures reduce to once that frame is known.
//!
//! The profile's plane stays square to the curve's tangent `T`. Its axes `e₁`, `e₂` turn with the
//! rotation-minimizing frame `N₁`, `N₂` (`N′ = −(T′·N) T`, no turn about `T`) and a twist `φ` that
//! grows evenly with arc length. A profile point `(X, Y)` sits at `γ + X e₁ + Y e₂`, so with `K₁`,
//! `K₂` the curvature `T′` in the `e` axes the volume element is `(1 − K₁X − K₂Y) dA ds` and the
//! side element `√(a²|b′|² + φ′²(b·b′)²) dt ds`, where `a = 1 − K₁X − K₂Y` along the boundary `b`.

use crate::profile::Profile;

use crate::model::spline_point;
use crate::portable::Portable;

type V = [f64; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: V) -> V {
    mul(a, 1.0 / dot(a, a).sqrt())
}

/// A clamped B-spline in space over `knots[degree] ..= knots[ctrl.len()]`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PathCurve {
    pub degree: usize,
    pub knots: Vec<f64>,
    pub ctrl: Vec<V>,
}

impl PathCurve {
    pub fn new(degree: usize, knots: Vec<f64>, ctrl: Vec<V>) -> PathCurve {
        PathCurve {
            degree,
            knots,
            ctrl,
        }
    }
}

/// The derivative's degree, knots and control points.
fn derived(p: usize, knots: &[f64], ctrl: &[V]) -> (usize, Vec<f64>, Vec<V>) {
    let q = (0..ctrl.len() - 1)
        .map(|i| {
            let den = knots[i + p + 1] - knots[i + 1];
            let s = if den > 0.0 { p as f64 / den } else { 0.0 };
            mul(sub(ctrl[i + 1], ctrl[i]), s)
        })
        .collect();
    (p - 1, knots[1..knots.len() - 1].to_vec(), q)
}

impl PathCurve {
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.ctrl.len()])
    }

    /// The point and its first and second derivatives at `t`.
    pub fn eval(&self, t: f64) -> [V; 3] {
        let (p1, k1, c1) = derived(self.degree, &self.knots, &self.ctrl);
        let second = if p1 == 0 {
            [0.0; 3]
        } else {
            let (p2, k2, c2) = derived(p1, &k1, &c1);
            spline_point(p2, &k2, &c2, t)
        };
        [
            spline_point(self.degree, &self.knots, &self.ctrl, t),
            spline_point(p1, &k1, &c1, t),
            second,
        ]
    }

    /// The distinct knots bounding each span of the domain.
    fn breaks(&self) -> Vec<f64> {
        let (a, b) = self.domain();
        let mut v: Vec<f64> = self
            .knots
            .iter()
            .copied()
            .filter(|&k| k >= a && k <= b)
            .collect();
        v.dedup();
        v
    }
}

const GX: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683,
    0.0,
    0.538_469_310_105_683,
    0.906_179_845_938_664,
];
const GW: [f64; 5] = [
    0.236_926_885_056_189,
    0.478_628_670_499_366,
    0.568_888_888_888_889,
    0.478_628_670_499_366,
    0.236_926_885_056_189,
];

/// `v` mirrored in the plane square to `n`; `v` itself where `n` is zero.
fn mirrored(v: V, n: V) -> V {
    let nn = dot(n, n);
    if nn == 0.0 {
        return v;
    }
    sub(v, mul(n, 2.0 / nn * dot(n, v)))
}

/// The rotation-minimizing frame carried from one point to the next by two mirrors, fourth
/// order in the step (W. Wang, B. Jüttler, D. Zheng and Y. Liu, "Computation of rotation
/// minimizing frames", ACM Transactions on Graphics 27(1), 2008): the first, across the step,
/// takes the frame to the new point; the second takes the mirrored tangent onto the tangent
/// there.
fn reflect(x0: V, t0: V, r0: V, x1: V, t1: V) -> V {
    let step = sub(x1, x0);
    let (r_across, t_across) = (mirrored(r0, step), mirrored(t0, step));
    mirrored(r_across, sub(t1, t_across))
}

/// One quadrature point along the path: its weight in arc length, the point, the tangent, the
/// profile's turned axes, and the curvature in those axes.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub ds: f64,
    pub s: f64,
    pub u: f64,
    pub at: V,
    pub t: V,
    pub n1: V,
    pub e1: V,
    pub e2: V,
    pub k: [f64; 2],
}

/// The sweep of `profile` along `path` turned through `twist` by its end, the path starting at the
/// origin along z with the profile in the xy plane, x along the frame's first axis.
pub struct PathSweep {
    pub path: PathCurve,
    pub profile: Profile,
    pub twist: f64,
    pub length: f64,
    pub stations: Vec<Station>,
    /// How far the profile reaches from the path.
    pub reach: f64,
}

/// Arc length from `a` to `b` along the path.
fn arc(path: &PathCurve, a: f64, b: f64) -> f64 {
    let (m, h) = (0.5 * (a + b), 0.5 * (b - a));
    (0..5)
        .map(|i| {
            let d = path.eval(m + h * GX[i])[1];
            GW[i] * h * dot(d, d).sqrt()
        })
        .sum()
}

impl PathSweep {
    /// The stations, 64 Gauss pieces to each span, and the frame carried through them in order.
    pub fn new(path: &PathCurve, profile: &Profile, twist: f64) -> PathSweep {
        let breaks = path.breaks();
        let mut raw: Vec<(f64, f64, f64)> = Vec::new();
        let mut s0 = 0.0;
        for w in breaks.windows(2) {
            let pieces = 64;
            for k in 0..pieces {
                let a = w[0] + (w[1] - w[0]) * k as f64 / pieces as f64;
                let b = w[0] + (w[1] - w[0]) * (k + 1) as f64 / pieces as f64;
                let (m, h) = (0.5 * (a + b), 0.5 * (b - a));
                for i in 0..5 {
                    let u = m + h * GX[i];
                    raw.push((u, GW[i] * h, s0 + arc(path, a, u)));
                }
                s0 += arc(path, a, b);
            }
        }
        let length = s0;
        let (u0, _) = path.domain();
        let (mut x, mut t, mut r) = (path.eval(u0)[0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        let mut stations = Vec::with_capacity(raw.len());
        for (u, w, s) in raw {
            let [p, d1, d2] = path.eval(u);
            let speed = dot(d1, d1).sqrt();
            let tn = mul(d1, 1.0 / speed);
            r = unit(reflect(x, t, r, p, tn));
            (x, t) = (p, tn);
            let n2 = cross(tn, r);
            // T′ in arc length: the second derivative's part square to T, over speed².
            let curv = mul(sub(d2, mul(tn, dot(d2, tn))), 1.0 / (speed * speed));
            let phi = twist * s / length;
            let (c, sn) = (phi.pcos(), phi.psin());
            let e1 = add(mul(r, c), mul(n2, sn));
            let e2 = add(mul(r, -sn), mul(n2, c));
            stations.push(Station {
                ds: w * speed,
                s,
                u,
                at: p,
                t: tn,
                n1: r,
                e1,
                e2,
                k: [dot(curv, e1), dot(curv, e2)],
            });
        }
        let (lo, hi) = profile.bounds();
        let reach = [
            lo[0].phypot(lo[1]),
            hi[0].phypot(hi[1]),
            lo[0].phypot(hi[1]),
            hi[0].phypot(lo[1]),
        ]
        .into_iter()
        .fold(0.0, f64::max);
        PathSweep {
            path: path.clone(),
            profile: profile.clone(),
            reach,
            twist,
            length,
            stations,
        }
    }

    pub fn volume(&self) -> f64 {
        let (a, m10, m01) = (
            self.profile.area(),
            self.profile.area_moment(1, 0),
            self.profile.area_moment(0, 1),
        );
        self.stations
            .iter()
            .map(|st| st.ds * (a - st.k[0] * m10 - st.k[1] * m01))
            .sum()
    }

    /// The side, and the two end caps.
    pub fn area(&self) -> f64 {
        let dphi = self.twist / self.length;
        let side: f64 = self
            .stations
            .iter()
            .map(|st| {
                st.ds
                    * self.profile.boundary_integral(|b, d| {
                        let a = 1.0 - st.k[0] * b[0] - st.k[1] * b[1];
                        let bb = b[0] * d[0] + b[1] * d[1];
                        (a * a * (d[0] * d[0] + d[1] * d[1]) + dphi * dphi * bb * bb).sqrt()
                    })
            })
            .sum();
        side + 2.0 * self.profile.area()
    }

    /// The volume, `∭ r dV` and `∭ r rᵀ dV`, in the path's own coordinates.
    pub fn moments(&self) -> (f64, V, [[f64; 3]; 3]) {
        let m = |k: i32, l: i32| self.profile.area_moment(k, l);
        let (a, m10, m01) = (self.profile.area(), m(1, 0), m(0, 1));
        let (m20, m11, m02) = (m(2, 0), m(1, 1), m(0, 2));
        let (m30, m21, m12, m03) = (m(3, 0), m(2, 1), m(1, 2), m(0, 3));
        let (mut v, mut f, mut s) = (0.0, [0.0; 3], [[0.0; 3]; 3]);
        for st in &self.stations {
            let [k1, k2] = st.k;
            let vs = a - k1 * m10 - k2 * m01;
            let q = add(
                mul(st.e1, m10 - k1 * m20 - k2 * m11),
                mul(st.e2, m01 - k1 * m11 - k2 * m02),
            );
            let qq = [
                [m20 - k1 * m30 - k2 * m21, m11 - k1 * m21 - k2 * m12],
                [m11 - k1 * m21 - k2 * m12, m02 - k1 * m12 - k2 * m03],
            ];
            let e = [st.e1, st.e2];
            v += st.ds * vs;
            f = add(f, mul(add(mul(st.at, vs), q), st.ds));
            for i in 0..3 {
                for j in 0..3 {
                    let mut x = vs * st.at[i] * st.at[j] + st.at[i] * q[j] + q[i] * st.at[j];
                    for (p, ep) in e.iter().enumerate() {
                        for (r, er) in e.iter().enumerate() {
                            x += ep[i] * er[j] * qq[p][r];
                        }
                    }
                    s[i][j] += st.ds * x;
                }
            }
        }
        (v, f, s)
    }

    /// The point at parameter `u` and the profile's turned axes there: the frame carried from the
    /// station before `u` to `u` by one more reflection, turned by the twist at its arc length.
    pub fn frame_at(&self, u: f64) -> (V, V, V) {
        let [p, d1, _] = self.path.eval(u);
        let tn = unit(d1);
        let before = self
            .stations
            .iter()
            .rev()
            .find(|st| st.u <= u)
            .unwrap_or(&self.stations[0]);
        let r = unit(reflect(before.at, before.t, before.n1, p, tn));
        let n2 = cross(tn, r);
        let s = before.s
            + if u >= before.u {
                arc(&self.path, before.u, u)
            } else {
                -arc(&self.path, u, before.u)
            };
        let phi = self.twist * s / self.length;
        let (c, sn) = (phi.pcos(), phi.psin());
        (p, add(mul(r, c), mul(n2, sn)), add(mul(r, -sn), mul(n2, c)))
    }

    /// Whether `q`, in the path's coordinates, lies in the sweep: the foot of `q` on the path,
    /// found from the nearest station by Newton steps on `(q − γ)·γ′ = 0`, must have `q` in its
    /// normal plane, and `q`'s place in the profile's turned axes there inside the profile.
    pub fn contains(&self, q: V) -> bool {
        // The nearest station: every 16th first, then those round the best of them. A point
        // farther from every station looked at than the profile reaches, plus the gap between
        // those stations, lies outside.
        const STRIDE: usize = 16;
        let d2 = |st: &Station| dot(sub(q, st.at), sub(q, st.at));
        let Some((coarse, best)) = self
            .stations
            .iter()
            .enumerate()
            .step_by(STRIDE)
            .map(|(i, st)| (i, d2(st)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            return false;
        };
        let gap = self.length * STRIDE as f64 / self.stations.len() as f64;
        if best.sqrt() > self.reach + gap {
            return false;
        }
        let lo_i = coarse.saturating_sub(STRIDE);
        let hi_i = (coarse + STRIDE + 1).min(self.stations.len());
        let near = self.stations[lo_i..hi_i]
            .iter()
            .min_by(|a, b| d2(a).total_cmp(&d2(b)))
            .expect("a station");
        let (lo, hi) = self.path.domain();
        let mut u = near.u;
        for _ in 0..30 {
            let [p, d1, d2] = self.path.eval(u);
            let g = dot(sub(q, p), d1);
            let dg = dot(sub(q, p), d2) - dot(d1, d1);
            let next = (u - g / dg).clamp(lo, hi);
            if (next - u).abs() <= 1e-15 * (hi - lo) {
                u = next;
                break;
            }
            u = next;
        }
        let [p, d1, _] = self.path.eval(u);
        let tn = unit(d1);
        let off = sub(q, p);
        if dot(off, tn).abs() > 1e-9 * (1.0 + dot(off, off).sqrt()) {
            return false;
        }
        let (_, e1, e2) = self.frame_at(u);
        self.profile.contains([dot(off, e1), dot(off, e2)])
    }
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Seg, circle};

    fn straight(len: f64) -> PathCurve {
        PathCurve {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            ctrl: (0..4).map(|i| [0.0, 0.0, len * i as f64 / 3.0]).collect(),
        }
    }

    #[test]
    fn a_disc_along_a_straight_path_is_a_cylinder_whatever_its_twist() {
        // Radius 1 along 5: volume 5π, area 2π·5 + 2π; a twist turns the disc in itself.
        let disc = Profile {
            loops: vec![circle([0.0, 0.0], 1.0, true)],
        };
        let path = straight(5.0);
        for twist in [0.0, 3.0] {
            let w = PathSweep::new(&path, &disc, twist);
            assert!((w.volume() - 5.0 * std::f64::consts::PI).abs() < 1e-10);
            assert!(
                (w.area() - 12.0 * std::f64::consts::PI).abs() < 1e-9,
                "{}",
                w.area()
            );
        }
    }

    #[test]
    fn a_disc_along_a_bent_path_keeps_pappus_and_its_points() {
        // A disc of radius ½ centred on a curving path of length L: volume πL/4 and side πL by
        // Pappus, whatever the curvature, as long as it stays under 2.
        let path = PathCurve {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            ctrl: vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 2.0],
                [1.5, 0.5, 4.0],
                [3.0, -1.0, 5.0],
                [4.0, -1.5, 7.0],
            ],
        };
        let disc = Profile {
            loops: vec![circle([0.0, 0.0], 0.5, true)],
        };
        let w = PathSweep::new(&path, &disc, 0.0);
        let pi = std::f64::consts::PI;
        assert!((w.volume() - pi * 0.25 * w.length).abs() < 1e-9 * w.length);
        assert!((w.area() - (pi * w.length + 2.0 * pi * 0.25)).abs() < 1e-9 * w.length);
        // Points a little off the path square to it, inside or out by 0.4 or 0.6.
        for st in w.stations.iter().step_by(37).skip(1) {
            if st.s < 0.01 * w.length || st.s > 0.99 * w.length {
                continue;
            }
            assert!(w.contains(add(st.at, mul(st.e1, 0.4))));
            assert!(!w.contains(add(st.at, mul(st.e2, 0.6))));
        }
    }

    #[test]
    fn a_twisted_square_along_a_straight_path_is_a_twisted_prism() {
        // A square of side 2 about the path turned through θ over length h, k = θ/h: a side at
        // distance 1 from the axis sweeps a strip of element √(1 + k²y²), so its area is
        // h·∫₋₁¹ √(1 + k²y²) dy = h·(√(1 + k²) + asinh(k)/k).
        let sq = Profile {
            loops: vec![vec![
                Seg::Line {
                    a: [-1.0, -1.0],
                    b: [1.0, -1.0],
                },
                Seg::Line {
                    a: [1.0, -1.0],
                    b: [1.0, 1.0],
                },
                Seg::Line {
                    a: [1.0, 1.0],
                    b: [-1.0, 1.0],
                },
                Seg::Line {
                    a: [-1.0, 1.0],
                    b: [-1.0, -1.0],
                },
            ]],
        };
        let (h, theta) = (4.0, 1.5);
        let line = straight(h);
        let w = PathSweep::new(&line, &sq, theta);
        let k = theta / h;
        let strip = h * ((1.0 + k * k).sqrt() + k.asinh() / k);
        assert!((w.volume() - 4.0 * h).abs() < 1e-10);
        assert!(
            (w.area() - (4.0 * strip + 8.0)).abs() < 1e-8,
            "{} vs {}",
            w.area(),
            4.0 * strip + 8.0
        );
    }
}
