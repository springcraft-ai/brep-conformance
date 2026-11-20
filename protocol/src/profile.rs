//! Planar profiles of lines, circular arcs and periodic B-splines: the sections the constructors
//! extrude, revolve, screw and loft, with their area, moments and lengths (in closed form for lines
//! and arcs) and a test of which points they hold (exact for lines and arcs).

use std::f64::consts::{PI, TAU};

use crate::model::Spline;
use crate::portable::Portable;

/// One segment of a loop, in sketch coordinates.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Seg {
    Line {
        a: [f64; 2],
        b: [f64; 2],
    },
    /// Counter-clockwise from `start` (radians) when `sweep` is positive, clockwise when negative;
    /// a sweep of ±2π is a whole circle.
    Arc {
        center: [f64; 2],
        radius: f64,
        start: f64,
        sweep: f64,
    },
    /// A closed periodic B-spline over `knots[degree] ..= knots[ctrl.len()]`: a whole loop.
    Spline(Spline),
    /// A conic arc from `a` to `b`, tangent there to the lines to `apex`: the rational quadratic
    /// Bézier with control points `a`, `apex`, `b` and weights 1, `w`, 1. Its shoulder, the point
    /// at its middle parameter, lies `w/(1 + w)` of the way from the chord's middle to the apex:
    /// an ellipse for `w` < 1, a parabola at 1, a hyperbola above.
    Conic {
        a: [f64; 2],
        apex: [f64; 2],
        b: [f64; 2],
        w: f64,
    },
    /// The blend section of order `order` and thumbweight `w` between the lines from `apex` to
    /// `a` and to `b`, from `a` to `b`: `apex + α(t)(a − apex) + β(t)(b − apex)` with
    /// `α = f·(1 − p₁)`, `β = (1 − f)·p₂`, `f = t̄ⁿ⁺¹/(t̄ⁿ⁺¹ + tⁿ⁺¹)`, `p₁ = t/(1 − w + wt)` and
    /// `p₂ = t(1 − w)/(1 − wt)`, `t̄ = 1 − t`. It meets each line with contact of order `order`:
    /// at 2, it starts as straight as the line. "Thumbweight" is this shape parameter's name
    /// in E. Hartmann, "Parametric Gⁿ blending of curves and surfaces", The Visual Computer 17,
    /// 2001, pp. 1–13.
    Gn {
        a: [f64; 2],
        apex: [f64; 2],
        b: [f64; 2],
        order: u32,
        w: f64,
    },
}

/// The weights `(α, β)` of a G^n section's legs at `t`, and their derivatives.
fn gn_weights(order: u32, w: f64, t: f64) -> ([f64; 2], [f64; 2]) {
    let m = order as i32 + 1;
    let (s, u) = (1.0 - t, t);
    let (big_a, big_b) = (s.ppowi(m), u.ppowi(m));
    let (da, db) = (-(m as f64) * s.ppowi(m - 1), m as f64 * u.ppowi(m - 1));
    let sum = big_a + big_b;
    let f = big_a / sum;
    let df = (da * big_b - big_a * db) / (sum * sum);
    let (d1, d2) = (1.0 - w + w * t, 1.0 - w * t);
    let (p1, p2) = (t / d1, t * (1.0 - w) / d2);
    let (dp1, dp2) = ((1.0 - w) / (d1 * d1), (1.0 - w) / (d2 * d2));
    (
        [f * (1.0 - p1), (1.0 - f) * p2],
        [df * (1.0 - p1) - f * dp1, -df * p2 + (1.0 - f) * dp2],
    )
}

/// The removed-area integral `I(w) = ∫ αβ' − α'β dt` of an order-`order` section: the share of
/// the triangle on its two legs that it takes off.
pub fn gn_area_share(order: u32, w: f64) -> f64 {
    // Composite five-point Gauss: the integrand is smooth and the share exact to rounding.
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
    let panels = 256;
    let h = 1.0 / panels as f64;
    let mut sum = 0.0;
    for k in 0..panels {
        let mid = (k as f64 + 0.5) * h;
        for (x, wt) in X.iter().zip(W) {
            let (v, d) = gn_weights(order, w, mid + 0.5 * h * x);
            sum += wt * 0.5 * h * (v[0] * d[1] - d[0] * v[1]);
        }
    }
    sum
}

/// The radius of curvature at the middle of an order-`order` section of thumbweight `w` whose legs
/// are `d` long and meet at the angle `alpha`. By symmetry the middle is `t = ½`.
pub fn gn_center_radius(order: u32, w: f64, alpha: f64, d: f64) -> f64 {
    gn_radius_at(order, w, alpha, d, 0.5)
}

/// The radius of curvature at `t` of an order-`order` section of thumbweight `w` whose legs are
/// `d` long and meet at the angle `alpha`. With the legs' weights `α`, `β`,
/// `R = d·(α'² + β'² + 2α'β' cos alpha)^(3/2) / (|α'β'' − α''β'| sin alpha)`, the second
/// derivatives by a five-point difference of the exact first ones (error near h⁴, 1e-12).
pub fn gn_radius_at(order: u32, w: f64, alpha: f64, d: f64, t: f64) -> f64 {
    let (_, d1) = gn_weights(order, w, t);
    let h = 1e-3 * (2.0 * t.min(1.0 - t)).min(1.0);
    let second = |k: usize| {
        let at = |s: f64| gn_weights(order, w, t + s * h).1[k];
        (at(-2.0) - 8.0 * at(-1.0) + 8.0 * at(1.0) - at(2.0)) / (12.0 * h)
    };
    let (a1, b1, a2, b2) = (d1[0], d1[1], second(0), second(1));
    let speed = a1 * a1 + b1 * b1 + 2.0 * a1 * b1 * alpha.pcos();
    d * speed.ppowf(1.5) / ((a1 * b2 - a2 * b1).abs() * alpha.psin())
}

/// Whether an order-`order` section of thumbweight `w` in a wedge of angle `alpha` curves most at
/// its middle: from each contact its curvature rises to the middle and nowhere passes it, with no
/// shoulders curving harder on the way. Sampled at 200 points a half.
pub fn gn_peaks_at_middle(order: u32, w: f64, alpha: f64) -> bool {
    let middle = gn_radius_at(order, w, alpha, 1.0, 0.5);
    (1..200).all(|k| {
        gn_radius_at(order, w, alpha, 1.0, 0.5 * k as f64 / 200.0) >= middle * (1.0 - 1e-9)
    })
}

/// The least thumbweight at which an order-`order` section in a wedge of angle `alpha` curves
/// most at its middle ([`gn_peaks_at_middle`]); above it the middle only sharpens. Bisected
/// between −1, where the shoulders curve hardest, and just under 1.
pub fn gn_peaked_w(order: u32, alpha: f64) -> f64 {
    let (mut lo, mut hi) = (-1.0f64, 1.0 - 1e-6);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if gn_peaks_at_middle(order, mid, alpha) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// The thumbweight at which an order-`order` section with legs `d` long meeting at `alpha` has
/// radius of curvature `center` at its middle. The middle radius falls from unbounded to zero as
/// `w` rises to 1, so the root is bisected on a bracket stepped down from −1.
pub fn gn_center_w(order: u32, alpha: f64, d: f64, center: f64) -> f64 {
    let (mut lo, mut hi) = (-1.0f64, 1.0 - 1e-9);
    while gn_center_radius(order, lo, alpha, d) < center {
        hi = lo;
        lo = 2.0 * lo - 1.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if gn_center_radius(order, mid, alpha, d) > center {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-15 * (1.0 - lo) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// The thumbweight at which an order-`order` section in a wedge of angle `alpha` takes off what
/// the circle tangent at the same contacts does: `I(w)` is the circle's share,
/// `(cot(α/2) − (π − α)/2) / (cot(α/2)·cos²(α/2))`. `I` falls as `w` rises, so the root is
/// bisected on a bracket stepped down from −1.
pub fn gn_equal_area_w(order: u32, alpha: f64) -> f64 {
    let c = 1.0 / (alpha / 2.0).ptan();
    let target = (c - (PI - alpha) / 2.0) / (c * (alpha / 2.0).pcos().ppowi(2));
    let (mut lo, mut hi) = (-1.0f64, 1.0 - 1e-9);
    while gn_area_share(order, lo) < target {
        hi = lo;
        lo = 2.0 * lo - 1.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if gn_area_share(order, mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-15 * (1.0 - lo) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Closed loops, the first the outer one counter-clockwise and any further ones holes, clockwise.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub loops: Vec<Vec<Seg>>,
}

/// Five-point Gauss–Legendre nodes and weights on [−1, 1].
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

/// A spline piece's parameters, its box grown by twice its chord's sag, `[x0, x1, y0, y1]`, and
/// its ends' heights.
type Piece = (f64, f64, [f64; 4], [f64; 2]);

/// A spline's pieces, by where its control points lie and what they are.
type Polygon = (usize, Vec<[f64; 2]>, std::rc::Rc<Vec<Piece>>);

thread_local! {
    /// The last splines' polygons, by where their control points lie and what they are.
    static POLYGONS: std::cell::RefCell<Vec<Polygon>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Seg {
    /// The point and derivative at `t` in [0, 1].
    pub fn eval(&self, t: f64) -> ([f64; 2], [f64; 2]) {
        if let Seg::Gn {
            a,
            apex,
            b,
            order,
            w,
        } = *self
        {
            let (v, d) = gn_weights(order, w, t);
            let leg = |p: [f64; 2], k: usize| p[k] - apex[k];
            return (
                [0, 1].map(|k| apex[k] + v[0] * leg(a, k) + v[1] * leg(b, k)),
                [0, 1].map(|k| d[0] * leg(a, k) + d[1] * leg(b, k)),
            );
        }
        if let Seg::Conic { a, apex, b, w } = *self {
            // N(t)/D(t) with N = (1−t)²a + 2wt(1−t)apex + t²b and D = (1−t)² + 2wt(1−t) + t².
            let (s, u) = (1.0 - t, t);
            let d = s * s + 2.0 * w * s * u + u * u;
            let dd = -2.0 * s + 2.0 * w * (s - u) + 2.0 * u;
            let n = [0, 1].map(|k| s * s * a[k] + 2.0 * w * s * u * apex[k] + u * u * b[k]);
            let dn = [0, 1].map(|k| -2.0 * s * a[k] + 2.0 * w * (s - u) * apex[k] + 2.0 * u * b[k]);
            let q = [n[0] / d, n[1] / d];
            return (q, [0, 1].map(|k| (dn[k] - q[k] * dd) / d));
        }
        if let Seg::Spline(sp) = self {
            let (lo, hi) = range(sp);
            let (q, d) = sp.eval(lo + t * (hi - lo));
            return (q, [d[0] * (hi - lo), d[1] * (hi - lo)]);
        }
        match *self {
            Seg::Line { a, b } => {
                let d = [b[0] - a[0], b[1] - a[1]];
                ([a[0] + t * d[0], a[1] + t * d[1]], d)
            }
            Seg::Arc {
                center: c,
                radius: r,
                start,
                sweep,
            } => {
                let (s, co) = (start + t * sweep).psin_cos();
                (
                    [c[0] + r * co, c[1] + r * s],
                    [-r * sweep * s, r * sweep * co],
                )
            }
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => unreachable!(),
        }
    }

    /// The winding a curve segment adds about `q` from the parameters `roots` where it is level
    /// with `q`: half-open in t as a line's crossing is in y, so a crossing at a shared end
    /// counts once, an end where the curve leaves going up, or arrives going down.
    fn crossings(&self, q: [f64; 2], roots: Vec<f64>) -> i32 {
        let mut n = 0;
        for t in roots {
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let (p, d) = self.eval(t);
            if p[0] <= q[0] || d[1] == 0.0 {
                continue;
            }
            let up = d[1] > 0.0;
            if (up && t < 1.0) || (!up && t > 0.0) {
                n += if up { 1 } else { -1 };
            }
        }
        n
    }

    pub fn length(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => (b[0] - a[0]).phypot(b[1] - a[1]),
            Seg::Arc { radius, sweep, .. } => radius * sweep.abs(),
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => {
                self.integrate(|_, d| d[0].phypot(d[1]))
            }
        }
    }

    /// ∮ x dy / 2 − y dx / 2 along the segment: its share of the enclosed area.
    fn area(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => 0.5 * (a[0] * b[1] - b[0] * a[1]),
            Seg::Arc {
                center: c,
                radius: r,
                start: s,
                sweep: w,
            } => {
                let e = s + w;
                0.5 * (r * c[0] * (e.psin() - s.psin()) - r * c[1] * (e.pcos() - s.pcos())
                    + r * r * w)
            }
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => {
                self.integrate(|q, d| 0.5 * (q[0] * d[1] - q[1] * d[0]))
            }
        }
    }

    /// ∮ x² dy / 2 along the segment: its share of ∫∫ x dA.
    fn moment(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                0.5 * dy * (a[0] * a[0] + a[0] * dx + dx * dx / 3.0)
            }
            Seg::Arc {
                center: c,
                radius: r,
                start: s,
                sweep: w,
            } => {
                // x = cx + r cos θ, dy = r cos θ dθ.
                let e = s + w;
                let i1 = e.psin() - s.psin();
                let i2 = 0.5 * w + 0.25 * ((2.0 * e).psin() - (2.0 * s).psin());
                let i3 =
                    (e.psin() - e.psin().ppowi(3) / 3.0) - (s.psin() - s.psin().ppowi(3) / 3.0);
                0.5 * r * (c[0] * c[0] * i1 + 2.0 * c[0] * r * i2 + r * r * i3)
            }
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => {
                self.integrate(|q, d| 0.5 * q[0] * q[0] * d[1])
            }
        }
    }

    /// ∫ x ds along the segment.
    fn x_length(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => self.length() * 0.5 * (a[0] + b[0]),
            Seg::Arc {
                center: c,
                radius: r,
                start: s,
                sweep: w,
            } => r * (c[0] * w.abs() + w.signum() * r * ((s + w).psin() - s.psin())),
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => {
                self.integrate(|q, d| q[0] * d[0].phypot(d[1]))
            }
        }
    }

    /// Where `y` is zero in `[lo, hi)`: at `lo`, where it changes sign by `hi`, or, where its ends
    /// agree, twice either side of where it turns between them if it crosses there. A zero at `hi`
    /// is the next step's, so steps sharing an end count it once.
    fn level_in(&self, lo: f64, hi: f64, y: &dyn Fn(f64) -> f64) -> Vec<f64> {
        let root = |mut a: f64, mut b: f64| {
            let below = y(a) < 0.0;
            for _ in 0..80 {
                let m = 0.5 * (a + b);
                if (y(m) < 0.0) == below {
                    a = m;
                } else {
                    b = m;
                }
            }
            0.5 * (a + b)
        };
        let (ylo, yhi) = (y(lo), y(hi));
        if ylo == 0.0 {
            return vec![lo];
        }
        if yhi == 0.0 {
            return Vec::new();
        }
        if (ylo < 0.0) != (yhi < 0.0) {
            return vec![root(lo, hi)];
        }
        let dy = |t: f64| self.eval(t).1[1];
        let (dlo, dhi) = (dy(lo), dy(hi));
        if (dlo < 0.0) == (dhi < 0.0) {
            return Vec::new();
        }
        let (mut a, mut b) = (lo, hi);
        for _ in 0..80 {
            let m = 0.5 * (a + b);
            if (dy(m) < 0.0) == (dlo < 0.0) {
                a = m;
            } else {
                b = m;
            }
        }
        let turn = 0.5 * (a + b);
        let yt = y(turn);
        if yt == 0.0 {
            vec![turn]
        } else if (yt < 0.0) != (ylo < 0.0) {
            vec![root(lo, turn), root(turn, hi)]
        } else {
            Vec::new()
        }
    }

    /// The segment's crossings of the ray from `q` along +x, each +1 where the segment runs up
    /// through it and −1 where down: its share of the winding number about `q`.
    fn winding(&self, q: [f64; 2]) -> i32 {
        if let Seg::Spline(sp) = self {
            // Pieces of 64 a span, built once per spline, as a grid of points asks again and
            // again: a piece whose box, grown by its chord's sag, the ray cannot reach is passed;
            // the rest are crossed exactly, as a point a hair off the curve must be told rightly.
            let pieces = POLYGONS.with(|c| {
                let mut c = c.borrow_mut();
                let key = sp.ctrl.as_ptr() as usize;
                if let Some((_, _, pieces)) =
                    c.iter().find(|(k, ctrl, _)| *k == key && *ctrl == sp.ctrl)
                {
                    return pieces.clone();
                }
                let pieces: std::rc::Rc<Vec<Piece>> = std::rc::Rc::new(
                    self.pieces(64)
                        .into_iter()
                        .map(|(ta, tb)| {
                            let (pa, pb) = (self.eval(ta).0, self.eval(tb).0);
                            let mid = self.eval(0.5 * (ta + tb)).0;
                            let sag = 2.0
                                * ((mid[0] - 0.5 * (pa[0] + pb[0])).ppowi(2)
                                    + (mid[1] - 0.5 * (pa[1] + pb[1])).ppowi(2))
                                .sqrt()
                                + 1e-12;
                            let bx = [
                                pa[0].min(pb[0]) - sag,
                                pa[0].max(pb[0]) + sag,
                                pa[1].min(pb[1]) - sag,
                                pa[1].max(pb[1]) + sag,
                            ];
                            (ta, tb, bx, [pa[1], pb[1]])
                        })
                        .collect(),
                );
                c.insert(0, (key, sp.ctrl.clone(), pieces.clone()));
                c.truncate(16);
                pieces
            });
            let y = |t: f64| self.eval(t).0[1] - q[1];
            let mut roots = Vec::new();
            // A piece wholly right of q crosses the ray however its crossing lies: only whether
            // and which way its ends straddle q's height counts, half-open as a line's does.
            let mut wound = 0;
            for &(ta, tb, [x0, x1, y0, y1], [ya, yb]) in pieces.iter() {
                if q[1] < y0 || q[1] > y1 || q[0] > x1 {
                    continue;
                }
                if q[0] < x0 {
                    let (a, b) = (ya - q[1], yb - q[1]);
                    if a <= 0.0 && b > 0.0 {
                        wound += 1;
                    } else if b <= 0.0 && a > 0.0 {
                        wound -= 1;
                    }
                    continue;
                }
                roots.extend(self.level_in(ta, tb, &y));
            }
            return wound + self.crossings(q, roots);
        }
        if let Seg::Gn { a, apex, b, .. } = *self {
            // The section lies in the triangle on its legs: a ray from q that misses that
            // triangle's span misses it.
            let (lo_y, hi_y) = (a[1].min(apex[1]).min(b[1]), a[1].max(apex[1]).max(b[1]));
            if q[1] < lo_y || q[1] > hi_y || q[0] > a[0].max(apex[0]).max(b[0]) {
                return 0;
            }
            // y(t) = qy where it changes sign between samples, bisected to rounding. The section
            // is convex, so a horizontal line crosses it at most twice. Both crossings can fall
            // between one pair of samples, by where y turns, one each side of q, so a step whose
            // ends agree in sign is split at the turn of y in it, found where y' changes sign.
            let n = 64;
            let y = |t: f64| self.eval(t).0[1] - q[1];
            let dy = |t: f64| self.eval(t).1[1];
            let root = |mut lo: f64, mut hi: f64| {
                let below = y(lo) < 0.0;
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    if (y(mid) < 0.0) == below {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                0.5 * (lo + hi)
            };
            let mut roots = Vec::new();
            for j in 0..n {
                let (lo, hi) = (j as f64 / n as f64, (j + 1) as f64 / n as f64);
                let (ylo, yhi) = (y(lo), y(hi));
                if ylo == 0.0 {
                    roots.push(lo);
                    continue;
                }
                if (ylo < 0.0) != (yhi < 0.0) {
                    roots.push(root(lo, hi));
                    continue;
                }
                let (dlo, dhi) = (dy(lo), dy(hi));
                if (dlo < 0.0) == (dhi < 0.0) {
                    continue;
                }
                let (mut a, mut b) = (lo, hi);
                for _ in 0..80 {
                    let mid = 0.5 * (a + b);
                    if (dy(mid) < 0.0) == (dlo < 0.0) {
                        a = mid;
                    } else {
                        b = mid;
                    }
                }
                let turn = 0.5 * (a + b);
                let yt = y(turn);
                if yt == 0.0 {
                    roots.push(turn);
                } else if (yt < 0.0) != (ylo < 0.0) {
                    roots.push(root(lo, turn));
                    roots.push(root(turn, hi));
                }
            }
            if y(1.0) == 0.0 {
                roots.push(1.0);
            }
            return self.crossings(q, roots);
        }
        if let Seg::Conic { a, apex, b, w } = *self {
            // (1−t)²(a.y − qy) + 2wt(1−t)(apex.y − qy) + t²(b.y − qy) = 0, a quadratic in t.
            let (y0, y1, y2) = (a[1] - q[1], w * (apex[1] - q[1]), b[1] - q[1]);
            let (ca, cb, cc) = (y0 - 2.0 * y1 + y2, 2.0 * (y1 - y0), y0);
            let roots: Vec<f64> = if ca.abs() < 1e-300 {
                if cb.abs() < 1e-300 {
                    vec![]
                } else {
                    vec![-cc / cb]
                }
            } else {
                let disc = cb * cb - 4.0 * ca * cc;
                if disc < 0.0 {
                    vec![]
                } else {
                    let r = disc.sqrt();
                    vec![(-cb - r) / (2.0 * ca), (-cb + r) / (2.0 * ca)]
                }
            };
            return self.crossings(q, roots);
        }
        match *self {
            Seg::Line { a, b } => {
                let cross = (b[0] - a[0]) * (q[1] - a[1]) - (q[0] - a[0]) * (b[1] - a[1]);
                if a[1] <= q[1] && b[1] > q[1] && cross > 0.0 {
                    1
                } else if a[1] > q[1] && b[1] <= q[1] && cross < 0.0 {
                    -1
                } else {
                    0
                }
            }
            Seg::Arc {
                center: c,
                radius: r,
                start,
                sweep,
            } => {
                let v = (q[1] - c[1]) / r;
                // Level with the circle's top or bottom the ray touches it, crossing nothing; a
                // rounded `v` a hair inside 1 would count one of the two roots there and not the
                // other where the arc starts at that extreme.
                if v.abs() >= 1.0 - 1e-12 {
                    return 0;
                }
                let t1 = v.pasin();
                let mut n = 0;
                for theta in [t1, PI - t1] {
                    if c[0] + r * theta.pcos() <= q[0] {
                        continue;
                    }
                    // How far along the arc, in its own direction, the crossing lies.
                    let along = ((theta - start) * sweep.signum()).rem_euclid(TAU);
                    if along < sweep.abs() || sweep.abs() >= TAU {
                        n += if theta.pcos() * sweep > 0.0 { 1 } else { -1 };
                    }
                }
                n
            }
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => unreachable!(),
        }
    }

    /// The parameter range [0, 1] in pieces: `n` equal ones, or for a spline `n` to each knot
    /// span, so no piece straddles a knot where the curve's polynomial changes.
    fn pieces(&self, n: usize) -> Vec<(f64, f64)> {
        let cuts: Vec<f64> = match self {
            Seg::Spline(sp) => {
                let (lo, hi) = range(sp);
                let ks = &sp.knots[sp.degree..=sp.ctrl.len()];
                ks.windows(2)
                    .filter(|w| w[1] > w[0])
                    .flat_map(|w| (0..n).map(move |j| w[0] + (w[1] - w[0]) * j as f64 / n as f64))
                    .map(|k| (k - lo) / (hi - lo))
                    .chain([1.0])
                    .collect()
            }
            _ => (0..=n).map(|j| j as f64 / n as f64).collect(),
        };
        cuts.windows(2).map(|w| (w[0], w[1])).collect()
    }

    /// ∫ f(point, derivative) dt over the segment, by Gauss on 16 pieces, or 8 a knot span.
    fn integrate(&self, f: impl Fn([f64; 2], [f64; 2]) -> f64) -> f64 {
        let n = if matches!(self, Seg::Spline(_)) {
            8
        } else {
            16
        };
        let mut sum = 0.0;
        for (a, b) in self.pieces(n) {
            for (x, w) in GX.iter().zip(GW) {
                let (p, d) = self.eval(0.5 * (a + b) + 0.5 * (b - a) * x);
                sum += 0.5 * (b - a) * w * f(p, d);
            }
        }
        sum
    }

    fn scaled(&self, s: f64) -> Seg {
        if let Seg::Gn {
            a,
            apex,
            b,
            order,
            w,
        } = *self
        {
            let k = |p: [f64; 2]| [p[0] * s, p[1] * s];
            return Seg::Gn {
                a: k(a),
                apex: k(apex),
                b: k(b),
                order,
                w,
            };
        }
        if let Seg::Conic { a, apex, b, w } = *self {
            let k = |p: [f64; 2]| [p[0] * s, p[1] * s];
            return Seg::Conic {
                a: k(a),
                apex: k(apex),
                b: k(b),
                w,
            };
        }
        if let Seg::Spline(sp) = self {
            let ctrl = sp.ctrl.iter().map(|c| [c[0] * s, c[1] * s]).collect();
            return Seg::Spline(Spline { ctrl, ..sp.clone() });
        }
        match *self {
            Seg::Line { a, b } => Seg::Line {
                a: [a[0] * s, a[1] * s],
                b: [b[0] * s, b[1] * s],
            },
            Seg::Arc {
                center,
                radius,
                start,
                sweep,
            } => Seg::Arc {
                center: [center[0] * s, center[1] * s],
                radius: radius * s,
                start,
                sweep,
            },
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => unreachable!(),
        }
    }

    fn numbers(&self) -> Vec<f64> {
        if let Seg::Gn {
            a,
            apex,
            b,
            order,
            w,
        } = *self
        {
            return vec![
                4.0,
                a[0],
                a[1],
                apex[0],
                apex[1],
                b[0],
                b[1],
                f64::from(order),
                w,
            ];
        }
        if let Seg::Conic { a, apex, b, w } = *self {
            return vec![3.0, a[0], a[1], apex[0], apex[1], b[0], b[1], w];
        }
        if let Seg::Spline(sp) = self {
            let mut v = vec![2.0, sp.degree as f64];
            v.extend(&sp.knots);
            v.extend(sp.ctrl.iter().flatten());
            return v;
        }
        match *self {
            Seg::Line { a, b } => vec![0.0, a[0], a[1], b[0], b[1]],
            Seg::Arc {
                center,
                radius,
                start,
                sweep,
            } => vec![1.0, center[0], center[1], radius, start, sweep],
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => unreachable!(),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        if let Seg::Gn {
            a,
            apex,
            b,
            order,
            w,
        } = *self
        {
            return serde_json::json!({"gn": {"a": a, "apex": apex, "b": b, "order": order,
                                             "w": w}});
        }
        if let Seg::Conic { a, apex, b, w } = *self {
            return serde_json::json!({"conic": {"a": a, "apex": apex, "b": b, "w": w}});
        }
        if let Seg::Spline(sp) = self {
            return serde_json::json!({"spline": {
                "degree": sp.degree, "knots": sp.knots, "ctrl": sp.ctrl
            }});
        }
        match *self {
            Seg::Line { a, b } => serde_json::json!({"line": [a, b]}),
            Seg::Arc {
                center,
                radius,
                start,
                sweep,
            } => serde_json::json!({"arc": {
                "center": center, "radius": radius, "start": start, "sweep": sweep
            }}),
            Seg::Spline(_) | Seg::Conic { .. } | Seg::Gn { .. } => unreachable!(),
        }
    }
}

impl Profile {
    fn segs(&self) -> impl Iterator<Item = &Seg> {
        self.loops.iter().flatten()
    }

    pub fn area(&self) -> f64 {
        self.segs().map(Seg::area).sum()
    }

    pub fn perimeter(&self) -> f64 {
        self.segs().map(Seg::length).sum()
    }

    /// ∮ f(point, derivative) dt round every loop, by Gauss quadrature on each segment.
    pub fn boundary_integral(&self, f: impl Fn([f64; 2], [f64; 2]) -> f64) -> f64 {
        self.segs().map(|s| s.integrate(&f)).sum()
    }

    /// ∫∫ xᵏ yᵐ dA over the region, by Green's theorem as ∮ xᵏ⁺¹ yᵐ / (k + 1) dy.
    pub fn area_moment(&self, k: i32, m: i32) -> f64 {
        self.boundary_integral(|q, d| q[0].ppowi(k + 1) * q[1].ppowi(m) / f64::from(k + 1) * d[1])
    }

    /// ∫∫ x dA: the first moment about the sketch's y axis.
    pub fn moment(&self) -> f64 {
        self.segs().map(Seg::moment).sum()
    }

    /// ∫ x ds round every loop.
    pub fn x_length(&self) -> f64 {
        self.segs().map(Seg::x_length).sum()
    }

    /// The side area per radian of a screw that rises `rise` per radian about the sketch's y axis:
    /// ∫ √(x² + rise²·(dx/ds)²) ds round every loop.
    pub fn helix_side(&self, rise: f64) -> f64 {
        self.segs()
            .map(|s| {
                s.integrate(|p, d| {
                    (p[0] * p[0] * (d[0] * d[0] + d[1] * d[1]) + rise * rise * d[0] * d[0]).sqrt()
                })
            })
            .sum()
    }

    /// The side area of the ruled loft from the profile to its copy scaled by `s` about the
    /// origin, `h` above: (1 + s)/2 · ∮ √(h²|q′|² + (s − 1)²(q × q′)²) dt.
    pub fn loft_side(&self, s: f64, h: f64) -> f64 {
        let k = s - 1.0;
        0.5 * (1.0 + s)
            * self
                .segs()
                .map(|g| {
                    g.integrate(|q, d| {
                        let cross = q[0] * d[1] - q[1] * d[0];
                        (h * h * (d[0] * d[0] + d[1] * d[1]) + k * k * cross * cross).sqrt()
                    })
                })
                .sum::<f64>()
    }

    /// Whether `q` lies in the region the loops bound, by the nonzero winding rule.
    pub fn contains(&self, q: [f64; 2]) -> bool {
        self.segs().map(|s| s.winding(q)).sum::<i32>() != 0
    }

    /// A box holding the profile, `lo` and `hi`: each arc's whole circle, and each spline's
    /// control points, whose hull holds it.
    pub fn bounds(&self) -> ([f64; 2], [f64; 2]) {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for s in self.segs() {
            let pts = match s {
                Seg::Line { a, b } => vec![*a, *b],
                // Its ends, and each of the circle's four extremes the arc passes: a nearly flat
                // arc's circle can be a thousand times the part.
                Seg::Arc {
                    center: c,
                    radius: r,
                    start,
                    sweep,
                } => {
                    let at = |t: f64| [c[0] + r * t.pcos(), c[1] + r * t.psin()];
                    let mut pts = vec![at(*start), at(start + sweep)];
                    for k in 0..4 {
                        let t = f64::from(k) * PI / 2.0;
                        let along = ((t - start) * sweep.signum()).rem_euclid(TAU);
                        if along <= sweep.abs() || sweep.abs() >= TAU {
                            pts.push(at(t));
                        }
                    }
                    pts
                }
                Seg::Spline(sp) => sp.ctrl.clone(),
                // A conic arc lies in the triangle of its control points.
                Seg::Conic { a, apex, b, .. } | Seg::Gn { a, apex, b, .. } => vec![*a, *apex, *b],
            };
            for p in pts {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        (lo, hi)
    }

    /// The profile mirrored in the line x = r/2, so x becomes r − x, with every loop reversed to
    /// keep its direction: how a sweep's bend sees its profile from the bend's axis. A spline's
    /// knots are taken as uniform.
    pub fn reflected(&self, r: f64) -> Profile {
        let m = |p: [f64; 2]| [r - p[0], p[1]];
        let flip = |g: &Seg| match g {
            Seg::Line { a, b } => Seg::Line { a: m(*b), b: m(*a) },
            Seg::Arc {
                center,
                radius,
                start,
                sweep,
            } => Seg::Arc {
                center: m(*center),
                radius: *radius,
                start: PI - start - sweep,
                sweep: *sweep,
            },
            Seg::Conic { a, apex, b, w } => Seg::Conic {
                a: m(*b),
                apex: m(*apex),
                b: m(*a),
                w: *w,
            },
            // Run backwards a section is the one from its other leg with the same w: α(1 − t) = β(t).
            Seg::Gn {
                a,
                apex,
                b,
                order,
                w,
            } => Seg::Gn {
                a: m(*b),
                apex: m(*apex),
                b: m(*a),
                order: *order,
                w: *w,
            },
            Seg::Spline(sp) => {
                let n = sp.ctrl.len() - sp.degree;
                let mut ctrl: Vec<[f64; 2]> = sp.ctrl[..n].iter().rev().map(|&c| m(c)).collect();
                ctrl.extend_from_within(..sp.degree);
                Seg::Spline(Spline { ctrl, ..sp.clone() })
            }
        };
        Profile {
            loops: self
                .loops
                .iter()
                .map(|l| l.iter().rev().map(flip).collect())
                .collect(),
        }
    }

    pub fn scaled(&self, s: f64) -> Profile {
        Profile {
            loops: self
                .loops
                .iter()
                .map(|l| l.iter().map(|g| g.scaled(s)).collect())
                .collect(),
        }
    }

    /// Every number, loop by loop, a −1 between loops, for case ids.
    pub fn numbers(&self) -> Vec<f64> {
        let mut out = Vec::new();
        for l in &self.loops {
            for s in l {
                out.extend(s.numbers());
            }
            out.push(-1.0);
        }
        out
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.loops
                .iter()
                .map(|l| serde_json::Value::Array(l.iter().map(Seg::to_json).collect()))
                .collect(),
        )
    }
}

/// The loop through `pts`, counter-clockwise and convex at every rounded corner, with each corner
/// `i` with `r[i]` above zero blended by an order-`order` section: its contacts where a circle of
/// radius `r[i]` tangent to both sides touches them, `r·cot(α/2)` from a corner of interior angle
/// `α` (its tangent offset), and its radius of curvature at its middle `center[i]`, or, where that
/// is zero, its thumbweight the equal-area one, taking off what the circle does.
pub fn gn_rounded(pts: &[[f64; 2]], r: &[f64], center: &[f64], order: u32) -> Vec<Seg> {
    let n = pts.len();
    let toward = |p: [f64; 2], q: [f64; 2], t: f64| {
        let u = unit([q[0] - p[0], q[1] - p[1]]);
        [p[0] + u[0] * t, p[1] + u[1] * t]
    };
    let interior = |i: usize| {
        let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
        let (u, v) = (
            unit([prev[0] - p[0], prev[1] - p[1]]),
            unit([next[0] - p[0], next[1] - p[1]]),
        );
        (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0).pacos()
    };
    let ends: Vec<([f64; 2], [f64; 2])> = (0..n)
        .map(|i| {
            let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
            if r[i] > 0.0 {
                let d = r[i] / (interior(i) / 2.0).ptan();
                (toward(p, prev, d), toward(p, next, d))
            } else {
                (p, p)
            }
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..n {
        if r[i] > 0.0 {
            let alpha = interior(i);
            let w = if center[i] > 0.0 {
                gn_center_w(order, alpha, r[i] / (alpha / 2.0).ptan(), center[i])
            } else {
                gn_equal_area_w(order, alpha)
            };
            out.push(Seg::Gn {
                a: ends[i].0,
                apex: pts[i],
                b: ends[i].1,
                order,
                w,
            });
        }
        out.push(Seg::Line {
            a: ends[i].1,
            b: ends[(i + 1) % n].0,
        });
    }
    out
}

/// The loop through `pts` with each corner `i` with `d[i]` above zero rounded by a conic arc that
/// meets both sides `d[i]` from the corner, tangent to them, its shoulder `rho` of the way from the
/// chord's middle to the corner.
pub fn conic_rounded(pts: &[[f64; 2]], d: &[f64], rho: f64) -> Vec<Seg> {
    let n = pts.len();
    let w = rho / (1.0 - rho);
    let toward = |p: [f64; 2], q: [f64; 2], t: f64| {
        let u = unit([q[0] - p[0], q[1] - p[1]]);
        [p[0] + u[0] * t, p[1] + u[1] * t]
    };
    // Where each corner's blend starts and ends, or the corner itself.
    let ends: Vec<([f64; 2], [f64; 2])> = (0..n)
        .map(|i| {
            let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
            if d[i] > 0.0 {
                (toward(p, prev, d[i]), toward(p, next, d[i]))
            } else {
                (p, p)
            }
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..n {
        if d[i] > 0.0 {
            out.push(Seg::Conic {
                a: ends[i].0,
                apex: pts[i],
                b: ends[i].1,
                w,
            });
        }
        let next = (i + 1) % n;
        out.push(Seg::Line {
            a: ends[i].1,
            b: ends[next].0,
        });
    }
    out
}

/// The loop through `pts` with each corner rounded by an arc of radius `r[i]` (zero for sharp),
/// counter-clockwise when the points are; each rounding must fit both of its corner's sides.
pub fn rounded(pts: &[[f64; 2]], r: &[f64]) -> Vec<Seg> {
    let n = pts.len();
    // Each corner's arc: where it leaves the incoming side and joins the outgoing one.
    let mut ends = Vec::with_capacity(n);
    for i in 0..n {
        let (p, q, o) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let u = unit([q[0] - p[0], q[1] - p[1]]);
        let v = unit([o[0] - q[0], o[1] - q[1]]);
        if r[i] == 0.0 {
            ends.push((q, q, None));
            continue;
        }
        let turn = (u[0] * v[1] - u[1] * v[0]).patan2(u[0] * v[0] + u[1] * v[1]);
        let t = r[i] * (turn.abs() / 2.0).ptan();
        let a = [q[0] - u[0] * t, q[1] - u[1] * t];
        let b = [q[0] + v[0] * t, q[1] + v[1] * t];
        // The centre is r to the left of a left turn's side, to the right of a right turn's.
        let side = turn.signum();
        let c = [a[0] - u[1] * r[i] * side, a[1] + u[0] * r[i] * side];
        let start = (a[1] - c[1]).patan2(a[0] - c[0]);
        ends.push((a, b, Some((c, start, turn))));
    }
    let mut out = Vec::new();
    for i in 0..n {
        let (_, b, arc) = ends[i];
        if let Some((c, start, sweep)) = arc {
            out.push(Seg::Arc {
                center: c,
                radius: r[i],
                start,
                sweep,
            });
        }
        let next = ends[(i + 1) % n].0;
        if b != next {
            out.push(Seg::Line { a: b, b: next });
        }
    }
    out
}

/// The loop through `pts` with each corner cut `d[i]` back along both its sides (zero for sharp):
/// a chamfer's section. Each cut must fit both of its corner's sides.
pub fn chamfered(pts: &[[f64; 2]], d: &[f64]) -> Vec<Seg> {
    let n = pts.len();
    let ends: Vec<([f64; 2], [f64; 2])> = (0..n)
        .map(|i| {
            let (p, q, o) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let u = unit([q[0] - p[0], q[1] - p[1]]);
            let v = unit([o[0] - q[0], o[1] - q[1]]);
            (
                [q[0] - u[0] * d[i], q[1] - u[1] * d[i]],
                [q[0] + v[0] * d[i], q[1] + v[1] * d[i]],
            )
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = ends[i];
        if a != b {
            out.push(Seg::Line { a, b });
        }
        let next = ends[(i + 1) % n].0;
        if b != next {
            out.push(Seg::Line { a: b, b: next });
        }
    }
    out
}

/// A whole circle, counter-clockwise, or clockwise for a hole.
pub fn circle(center: [f64; 2], radius: f64, ccw: bool) -> Vec<Seg> {
    vec![Seg::Arc {
        center,
        radius,
        start: 0.0,
        sweep: if ccw { TAU } else { -TAU },
    }]
}

/// A spline's parameter range: one period.
fn range(sp: &Spline) -> (f64, f64) {
    (sp.knots[sp.degree], sp.knots[sp.ctrl.len()])
}

fn unit(v: [f64; 2]) -> [f64; 2] {
    let n = v[0].phypot(v[1]);
    [v[0] / n, v[1] / n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_edge_s_g2_section_peaks_at_its_middle_from_a_third_up() {
        // Sampled by hand at a square edge: at w = 0 the shoulders curve 1.38 to the middle's
        // 1.06, at 0.3 they nearly match (1.51, 1.47), and from 0.5 the middle is the peak. The
        // equal-area section, below 0, has shoulders.
        let square = PI / 2.0;
        let w = gn_peaked_w(2, square);
        assert!(w > 0.3 && w < 0.5, "{w}");
        assert!(!gn_peaks_at_middle(2, 0.0, square));
        assert!(gn_peaks_at_middle(2, 0.5, square));
        assert!(!gn_peaks_at_middle(2, gn_equal_area_w(2, square), square));
        let middle = gn_center_radius(2, 0.0, square, 1.0);
        let shoulder = (1..200)
            .map(|k| gn_radius_at(2, 0.0, square, 1.0, 0.5 * k as f64 / 200.0))
            .fold(f64::INFINITY, f64::min);
        assert!((1.0 / middle - 1.06).abs() < 0.01, "{middle}");
        assert!((1.0 / shoulder - 1.38).abs() < 0.01, "{shoulder}");
    }

    #[test]
    fn a_point_a_hair_off_a_closed_spline_is_on_its_side() {
        // A closed cubic through 16 points round a lobed cam, as a case of the suite draws it.
        // A grid's tests a ten-millionth of the part off the curve must land on their side of
        // it; a polygon through the curve read both sides as out where it cut the corner.
        let n = 16;
        let mut ctrl: Vec<[f64; 2]> = (0..n)
            .map(|i| {
                let a = TAU * i as f64 / n as f64;
                let rad = 15.0 + 5.0 * (-(a - PI).ppowi(2) / 1.28).pexp();
                [rad * a.pcos(), rad * a.psin()]
            })
            .collect();
        ctrl.extend_from_within(..3);
        let cam = Seg::Spline(crate::model::Spline {
            degree: 3,
            knots: (0..n + 7).map(|k| k as f64).collect(),
            ctrl,
        });
        let p = Profile {
            loops: vec![vec![cam.clone()]],
        };
        for k in 0..997 {
            let t = (k as f64 + 0.37) / 997.0;
            let (q, d) = cam.eval(t);
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
            // The loop runs counter-clockwise: its inside is to the left.
            let left = [-d[1] / l, d[0] / l];
            let h = 2e-6;
            assert!(
                p.contains([q[0] + left[0] * h, q[1] + left[1] * h]),
                "in at {t}"
            );
            assert!(
                !p.contains([q[0] - left[0] * h, q[1] - left[1] * h]),
                "out at {t}"
            );
        }
    }

    #[test]
    fn a_point_just_under_a_flat_blend_s_top_is_inside() {
        // A hexagon with one corner blended by a wide G2 section, from a case of the suite. Near
        // its top the section is all but level, so a ray a hair under it crosses it twice within
        // one sample step, once each side of the point; skipping both put the point outside.
        let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
        let (a, b) = (
            [1.5071795084913695, 4.552269895252525],
            [-0.9129997780543011, 4.702039074178597],
        );
        let blend = Seg::Gn {
            a,
            apex: [0.3378102714667473, 5.2851716044481645],
            b,
            order: 2,
            w: -0.6306432734778642,
        };
        let p = Profile {
            loops: vec![vec![
                line([4.609258818610143, 2.608042908204973], a),
                blend.clone(),
                line(b, [-4.265281581590907, 3.1391922046467613]),
                line(
                    [-4.265281581590907, 3.1391922046467613],
                    [-4.374085831226504, -2.98572065154079],
                ),
                line(
                    [-4.374085831226504, -2.98572065154079],
                    [4.349649346752178, -3.0212092327860995],
                ),
                line(
                    [4.349649346752178, -3.0212092327860995],
                    [4.609258818610143, 2.608042908204973],
                ),
            ]],
        };
        for k in 0..=200 {
            let t = 0.4 + 0.2 * k as f64 / 200.0;
            let (q, d) = blend.eval(t);
            let n = (d[0] * d[0] + d[1] * d[1]).sqrt();
            for h in [1e-9, 1e-7, 1e-5] {
                // A hair inward along the section's left normal.
                let inner = [q[0] - d[1] / n * h, q[1] + d[0] / n * h];
                assert!(p.contains(inner), "t {t} h {h}");
            }
        }
    }

    #[test]
    fn a_point_level_with_rounds_tops_beside_them_is_outside() {
        // A ring's section with all four corners rounded, from a case of the suite: both top
        // rounds' circles top out at the top line, where a ray at that height only touches them.
        // Rounded a hair inside a circle's top, the ray found two roots there and counted the one
        // on the arc, which put the corner the inner round cuts away inside.
        let arc = |c: [f64; 2], r: f64, start: f64| Seg::Arc {
            center: c,
            radius: r,
            start,
            sweep: PI / 2.0,
        };
        let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
        let top = 9.153311178030236;
        let p = Profile {
            loops: vec![vec![
                arc(
                    [4.004916867382027, 0.8042041435158365],
                    0.8042041435158366,
                    PI,
                ),
                line([4.004916867382027, 0.0], [6.249802165585292, 0.0]),
                arc(
                    [6.249802165585292, 0.7963777109682916],
                    0.7963777109682916,
                    -PI / 2.0,
                ),
                line(
                    [7.046179876553584, 0.7963777109682915],
                    [7.046179876553584, 8.827829496639024],
                ),
                arc(
                    [6.720698195162371, 8.827829496639024],
                    0.3254816813912126,
                    0.0,
                ),
                line([6.720698195162371, top], [4.097231655801578, top]),
                arc(
                    [4.097231655801578, 8.256792246094848],
                    0.8965189319353872,
                    PI / 2.0,
                ),
                line(
                    [3.2007127238661908, 8.256792246094848],
                    [3.2007127238661908, 0.8042041435158365],
                ),
            ]],
        };
        // Where a grid over the profile's box puts its top row.
        let row = p.bounds().1[1];
        for x in [3.25, 3.3, 3.6] {
            assert!(!p.contains([x, row]), "{x}");
            assert!(!p.contains([x, top]), "{x}");
        }
        assert!(p.contains([3.6, 8.5]));
    }

    fn square_with_hole() -> Profile {
        let sq = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        Profile {
            loops: vec![rounded(&sq, &[0.0; 4]), circle([2.0, 2.0], 1.0, false)],
        }
    }

    #[test]
    fn a_square_less_a_unit_hole_holds_sixteen_less_pi() {
        // Area 16 − π; ∫∫x dA = 2·(16 − π) by symmetry about x = 2; perimeter 16 + 2π.
        let p = square_with_hole();
        assert!((p.area() - (16.0 - PI)).abs() < 1e-12);
        assert!((p.moment() - 2.0 * (16.0 - PI)).abs() < 1e-12);
        assert!((p.perimeter() - (16.0 + TAU)).abs() < 1e-12);
        assert!(p.contains([0.5, 0.5]) && !p.contains([2.0, 2.0]) && !p.contains([5.0, 2.0]));
    }

    #[test]
    fn a_polygon_spline_round_a_unit_square_holds_one_and_runs_four() {
        // Degree 1 on the square's corners: the spline is the square itself.
        let sq = Profile {
            loops: vec![vec![Seg::Spline(Spline {
                degree: 1,
                knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
                ctrl: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
            })]],
        };
        assert!((sq.area() - 1.0).abs() < 1e-12, "{}", sq.area());
        assert!((sq.perimeter() - 4.0).abs() < 1e-12);
        assert!((sq.moment() - 0.5).abs() < 1e-12);
        assert!(sq.contains([0.5, 0.5]) && !sq.contains([1.5, 0.5]));
    }

    #[test]
    fn a_rounded_square_loses_four_corners_of_one_less_a_quarter_circle() {
        // Each corner of radius 1 removes 1 − π/4; the side runs 4·(4 − 2) + 2π.
        let sq = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let p = Profile {
            loops: vec![rounded(&sq, &[1.0; 4])],
        };
        assert!((p.area() - (16.0 - 4.0 + PI)).abs() < 1e-12, "{}", p.area());
        assert!((p.perimeter() - (8.0 + TAU)).abs() < 1e-12);
        assert!(!p.contains([0.1, 0.1]) && p.contains([0.5, 0.5]));
    }

    #[test]
    fn a_g2_section_takes_off_what_its_circle_does_and_starts_straight() {
        let pts = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
        let r = [0.0, 0.0, 0.8, 0.0];
        let round = Profile {
            loops: vec![rounded(&pts, &r)],
        };
        let g2 = Profile {
            loops: vec![gn_rounded(&pts, &r, &[0.0; 4], 2)],
        };
        // Equal area with the circle on the same contacts, at a square corner and at others.
        assert!(
            (g2.area() - round.area()).abs() < 1e-12,
            "{} vs {}",
            g2.area(),
            round.area()
        );
        for alpha in [0.6f64, 1.2, PI / 2.0, 2.2, 2.8] {
            let w = gn_equal_area_w(2, alpha);
            let c = 1.0 / (alpha / 2.0).ptan();
            let want = (c - (PI - alpha) / 2.0) / (c * (alpha / 2.0).pcos().ppowi(2));
            assert!((gn_area_share(2, w) - want).abs() < 1e-13, "alpha {alpha}");
        }
        let Some(s) = g2.loops[0].iter().find(|s| matches!(s, Seg::Gn { .. })) else {
            panic!("a section");
        };
        // Run backwards it is the section from its other leg: α(1 − t) = β(t).
        let Seg::Gn { w, .. } = *s else {
            unreachable!()
        };
        for t in [0.1, 0.37, 0.5, 0.9] {
            let (v, _) = gn_weights(2, w, t);
            let (u, _) = gn_weights(2, w, 1.0 - t);
            assert!((v[0] - u[1]).abs() < 1e-14 && (v[1] - u[0]).abs() < 1e-14);
        }
        // Curvature from the first and second derivatives, by differences of eval's tangent:
        // order 2 starts and ends straight, where the circle bends by 1/r.
        let bend = |t: f64| {
            let h = 1e-6;
            let (_, d0) = s.eval(t);
            let (_, d1) = s.eval(t + h);
            let dd = [(d1[0] - d0[0]) / h, (d1[1] - d0[1]) / h];
            (d0[0] * dd[1] - d0[1] * dd[0]).abs() / d0[0].phypot(d0[1]).ppowi(3)
        };
        assert!(
            bend(1e-4) < 1e-3 / 0.8 && bend(0.5) > 0.5 / 0.8,
            "{} {}",
            bend(1e-4),
            bend(0.5)
        );
        // Its points: inside near the corner's chord, outside past the section toward the corner.
        let (mid, _) = s.eval(0.5);
        let inward = [mid[0] - 1e-6, mid[1] - 1e-6];
        let outward = [mid[0] + 1e-6, mid[1] + 1e-6];
        assert!(g2.contains(inward) && !g2.contains(outward));
        // Mirrored, it keeps its area.
        assert!((g2.reflected(10.0).area() - g2.area()).abs() < 1e-12);
        // Given a center radius, the section bends by its reciprocal at its middle, measured
        // apart from the closed form by the turn of eval's tangent across it.
        for (alpha, center) in [(PI / 2.0, 0.5), (PI / 2.0, 2.0), (1.2, 1.0), (2.4, 0.3)] {
            let d = 0.8;
            let w = gn_center_w(2, alpha, d, center);
            let s = Seg::Gn {
                a: [d, 0.0],
                apex: [0.0, 0.0],
                b: [d * alpha.pcos(), d * alpha.psin()],
                order: 2,
                w,
            };
            let h = 1e-5;
            let ((p0, t0), (p1, t1)) = (s.eval(0.5 - h), s.eval(0.5 + h));
            let turn = (t0[0] * t1[1] - t0[1] * t1[0]).patan2(t0[0] * t1[0] + t0[1] * t1[1]);
            let radius = (p1[0] - p0[0]).phypot(p1[1] - p0[1]) / turn.abs();
            assert!(
                (radius - center).abs() < 1e-6 * center,
                "{radius} for {center}"
            );
        }
    }

    #[test]
    fn a_conic_corner_holds_its_closed_form_area_and_its_points() {
        // A unit square's corner at (1, 1) cut by a parabola (w = 1) from (1, 0.5) to (0.5, 1):
        // the parabola keeps two thirds of its control triangle, 1/8, so 1 − 1/8 + 1/12 = 23/24.
        // A circular quarter is the conic of weight cos 45°: the corner loses 1/4 − π/16.
        let pts = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let d = [0.0, 0.0, 0.5, 0.0];
        let parabola = Profile {
            loops: vec![conic_rounded(&pts, &d, 0.5)],
        };
        assert!(
            (parabola.area() - 23.0 / 24.0).abs() < 1e-12,
            "{}",
            parabola.area()
        );
        let rho = (0.5f64.sqrt()) / (1.0 + 0.5f64.sqrt());
        let circle = Profile {
            loops: vec![conic_rounded(&pts, &d, rho)],
        };
        let want = 1.0 - 0.25 + PI / 16.0;
        assert!(
            (circle.area() - want).abs() < 1e-12,
            "{} vs {want}",
            circle.area()
        );
        // The circle through the corner's arc: centre (0.5, 0.5), radius 0.5.
        let r = 0.5 * (1.0 + 1e-9);
        let on = [0.5 + r * 0.6, 0.5 + r * 0.8];
        assert!(!circle.contains(on));
        assert!(circle.contains([0.5 + 0.49 * 0.6, 0.5 + 0.49 * 0.8]));
        assert!(circle.contains([0.9, 0.2]) && !circle.contains([0.99, 0.99]));
        // Its shoulder lies rho of the way from the chord's middle to the corner, whatever rho.
        for rho in [0.2, 0.5, 0.8] {
            let lp = conic_rounded(&pts, &d, rho);
            let Some(arc) = lp.iter().find(|s| matches!(s, Seg::Conic { .. })) else {
                panic!("a conic");
            };
            let (q, _) = arc.eval(0.5);
            let want = 0.75 + rho * 0.25;
            assert!(
                (q[0] - want).abs() < 1e-15 && (q[1] - want).abs() < 1e-15,
                "{q:?}"
            );
        }
    }

    #[test]
    fn a_unit_circle_at_two_turns_a_torus_by_pappus() {
        // ∫∫x dA = 2π for the unit disc about (2, 0); ∫x ds = 2·2π.
        let p = Profile {
            loops: vec![circle([2.0, 0.0], 1.0, true)],
        };
        assert!((p.moment() - 2.0 * PI).abs() < 1e-12);
        assert!((p.x_length() - 4.0 * PI).abs() < 1e-12);
        // A screw that does not rise sweeps the torus's skin.
        assert!((p.helix_side(0.0) - 4.0 * PI).abs() < 1e-9);
    }

    #[test]
    fn a_loft_of_a_unit_square_to_a_point_has_a_pyramids_sides() {
        // Scale 0 over height 1: four triangles of base 1 and slant √(1 + 1/4).
        let sq = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
        let p = Profile {
            loops: vec![rounded(&sq, &[0.0; 4])],
        };
        let want = 4.0 * 0.5 * (1.25f64).sqrt();
        assert!(
            (p.loft_side(0.0, 1.0) - want).abs() < 1e-9,
            "{}",
            p.loft_side(0.0, 1.0)
        );
    }
}
