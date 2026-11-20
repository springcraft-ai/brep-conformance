//! Realistic parts, modelled as a designer would: a first solid, then the cuts, joins and blends
//! that make it a gear, a fastener, a bracket or a housing. Each step's tool either lies wholly in
//! the solid or empty space where it lands, or meets it in a region with a closed form (a strip of
//! a disc, two crossing cylinders, a hexagon outside a cone), so every part's volume is exact.

use super::*;

pub(crate) mod fasteners;
use crate::portable::Portable;
use brep_protocol::path::PathCurve;

const Z: Vec3 = [0.0, 0.0, 1.0];
const X: Vec3 = [1.0, 0.0, 0.0];
const Y: Vec3 = [0.0, 1.0, 0.0];

/// A part as it is built: the first solid, the steps so far, the shape they make and its volume.
struct Part {
    base: Shape,
    steps: Vec<Step>,
    want: Shape,
    volume: f64,
}

impl Part {
    fn new(p: Primitive) -> Part {
        Part {
            volume: p.volume(),
            base: p.clone().into(),
            steps: Vec::new(),
            want: p.into(),
        }
    }

    fn cut(mut self, tool: impl Into<Shape>, removed: f64) -> Part {
        let t: Shape = tool.into();
        self.steps.push(Step::Cut(t.clone()));
        self.want = Shape::bool(Op::Difference, self.want, t);
        self.volume -= removed;
        self
    }

    fn join(mut self, tool: impl Into<Shape>, added: f64) -> Part {
        let t: Shape = tool.into();
        self.steps.push(Step::Join(t.clone()));
        self.want = Shape::bool(Op::Union, self.want, t);
        self.volume += added;
        self
    }

    /// A blend at the points, taking `region` away (a convex edge) or adding it (a concave one).
    fn blend(mut self, act: Act, points: Vec<Vec3>, size: f64, region: Shape, add: bool) -> Part {
        let v = oracle_volume(&region);
        self.steps.push(Step::Act(Feature {
            act,
            points,
            size,
            pull: None,
            radii: Vec::new(),
            matrix: None,
        }));
        let op = if add { Op::Union } else { Op::Difference };
        self.want = Shape::bool(op, self.want, region);
        self.volume += if add { v } else { -v };
        self
    }

    fn done(self) -> Drawn {
        let exact = Exact {
            result_volume: Some(self.volume),
            chain: Some(self.steps),
            ..Default::default()
        };
        (self.base, self.want, exact)
    }
}

/// A blend region's volume: every region `blend` is given is one primitive.
fn oracle_volume(s: &Shape) -> f64 {
    match s {
        Shape::Prim(p) => p.volume(),
        Shape::Bool(Op::Union, a, b) => oracle_volume(a) + oracle_volume(b),
        Shape::Bool(..) => unreachable!("a blend region is a primitive or disjoint ones"),
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn frame(origin: Vec3, x: Vec3, y: Vec3) -> Frame {
    Frame {
        origin,
        x,
        y,
        z: cross(x, y),
    }
}

/// The frame drawing in the world's xz plane at `origin`, extruding toward −y.
fn xz(origin: Vec3) -> Frame {
    frame(origin, X, Z)
}

/// The frame drawing in the world's yz plane at `origin`, extruding toward +x.
fn yz(origin: Vec3) -> Frame {
    frame(origin, Y, Z)
}

fn ext(frame: Frame, loops: Vec<Vec<Seg>>, height: f64) -> Primitive {
    Primitive::Extrude {
        frame,
        profile: Profile { loops },
        height,
    }
}

fn rev(origin: Vec3, lp: Vec<Seg>) -> Primitive {
    Primitive::Revolve {
        frame: Frame::at(origin),
        profile: Profile { loops: vec![lp] },
        angle: TAU,
    }
}

fn cyl(o: Vec3, axis: Vec3, radius: f64, height: f64) -> Primitive {
    Primitive::Cylinder {
        frame: Frame::along(o, axis),
        radius,
        height,
    }
}

fn poly(pts: &[[f64; 2]]) -> Vec<Seg> {
    polygon(pts).loops.remove(0)
}

fn area(lp: &[Seg]) -> f64 {
    Profile {
        loops: vec![lp.to_vec()],
    }
    .area()
}

fn union_of(tools: Vec<Primitive>) -> Shape {
    let mut it = tools.into_iter();
    let first: Shape = it.next().expect("a tool").into();
    it.fold(first, |acc, t| Shape::bool(Op::Union, acc, t))
}

fn rot(p: [f64; 2], a: f64) -> [f64; 2] {
    let (c, s) = (a.pcos(), a.psin());
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}

fn reversed(segs: Vec<Seg>) -> Vec<Seg> {
    segs.into_iter()
        .rev()
        .map(|s| match s {
            Seg::Line { a, b } => Seg::Line { a: b, b: a },
            Seg::Arc {
                center,
                radius,
                start,
                sweep,
            } => Seg::Arc {
                center,
                radius,
                start: start + sweep,
                sweep: -sweep,
            },
            Seg::Conic { a, apex, b, w } => Seg::Conic {
                a: b,
                apex,
                b: a,
                w,
            },
            other => other,
        })
        .collect()
}

/// ∫ f over [a, b] by 8-point Gauss–Legendre on 64 panels: to rounding for the smooth integrands
/// here, each written so its endpoints carry no square-root singularity.
fn quad(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    const G: [(f64, f64); 4] = [
        (0.183_434_642_495_649_8, 0.362_683_783_378_362),
        (0.525_532_409_916_329, 0.313_706_645_877_887_3),
        (0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
        (0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
    ];
    let n = 64;
    let h = (b - a) / n as f64;
    (0..n)
        .map(|i| {
            let m = a + h * (i as f64 + 0.5);
            G.iter()
                .map(|&(x, w)| w * (f(m - x * h / 2.0) + f(m + x * h / 2.0)))
                .sum::<f64>()
                * h
                / 2.0
        })
        .sum()
}

/// The volume two cylinders crossing square through each other's axes share, radii `r ≥ a`:
/// 4∫ √(a² − x²)·√(r² − x²) dx over |x| ≤ a, with x = a·sin θ.
fn crossing(r: f64, a: f64) -> f64 {
    4.0 * quad(
        |t| a * a * t.pcos().ppowi(2) * (r * r - (a * t.psin()).ppowi(2)).sqrt(),
        -PI / 2.0,
        PI / 2.0,
    )
}

/// The area of the disc of radius `r` within `|y| ≤ w/2` on the side x ≥ 0.
fn half_strip(r: f64, w: f64) -> f64 {
    let y = w / 2.0;
    y * (r * r - y * y).sqrt() + r * r * (y / r).pasin()
}

/// The volume a cone chamfer takes from a hexagonal prism's end: the hexagon (inradius `d`)
/// outside the circle whose radius grows from `rho0` at the face by 1/tan(a) per unit depth.
fn hex_chamfer(d: f64, rho0: f64, tan_a: f64) -> f64 {
    let a_hex = 2.0 * 3f64.sqrt() * d * d;
    let rc = 2.0 * d / 3f64.sqrt();
    let inner = a_hex * (d - rho0) - PI * (d.ppowi(3) - rho0.ppowi(3)) / 3.0;
    // Past the inradius the circle leaves six segments of itself beyond the flats; with r = d +
    // (rc − d)u² the segment's (r − d)^{3/2} start is smooth in u.
    let outer = quad(
        |u| {
            let r = d + (rc - d) * u * u;
            let seg = r * r * (d / r).pacos() - d * (r * r - d * d).sqrt();
            (a_hex - PI * r * r + 6.0 * seg) * 2.0 * (rc - d) * u
        },
        0.0,
        1.0,
    );
    tan_a * (inner + outer)
}

/// The revolved tool cutting a cone chamfer of angle `a` (to the face) into the end at height
/// `z` of a hexagon whose corners reach `rc`, the cone `rho0` from the axis at the face; `up` when
/// the material lies below the face.
fn chamfer_tool(z: f64, rho0: f64, rc: f64, tan_a: f64, up: bool) -> Primitive {
    let (e, far) = (0.2, rc + 1.0);
    let deep = (far - rho0) * tan_a;
    let mut pts = vec![[rho0 - e / tan_a, z + e], [far, z + e], [far, z - deep]];
    if !up {
        for p in &mut pts {
            p[1] = 2.0 * z - p[1];
        }
    }
    rev([0.0; 3], poly(&ordered(pts)))
}

fn hexagon(rc: f64) -> Vec<[f64; 2]> {
    (0..6)
        .map(|i| {
            let a = PI / 3.0 * i as f64;
            [rc * a.pcos(), rc * a.psin()]
        })
        .collect()
}

/// A slot of radius `rho` between centres `a` and `b`, counter-clockwise.
fn stadium(a: [f64; 2], b: [f64; 2], rho: f64) -> Vec<Seg> {
    let l = (b[0] - a[0]).phypot(b[1] - a[1]);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let n = [-u[1], u[0]];
    let off = |p: [f64; 2], s: f64| [p[0] + n[0] * rho * s, p[1] + n[1] * rho * s];
    let down = (-n[1]).patan2(-n[0]);
    vec![
        Seg::Line {
            a: off(a, -1.0),
            b: off(b, -1.0),
        },
        Seg::Arc {
            center: b,
            radius: rho,
            start: down,
            sweep: PI,
        },
        Seg::Line {
            a: off(b, 1.0),
            b: off(a, 1.0),
        },
        Seg::Arc {
            center: a,
            radius: rho,
            start: down + PI,
            sweep: PI,
        },
    ]
}

/// A bore of radius `r` with a keyway `2k` wide reaching `r + depth` along +x, counter-clockwise.
fn keyed_bore(r: f64, k: f64, depth: f64) -> Vec<Seg> {
    let b = (k / r).pasin();
    let s = [(r * r - k * k).sqrt(), k];
    vec![
        Seg::Arc {
            center: [0.0, 0.0],
            radius: r,
            start: b,
            sweep: TAU - 2.0 * b,
        },
        Seg::Line {
            a: [s[0], -k],
            b: [r + depth, -k],
        },
        Seg::Line {
            a: [r + depth, -k],
            b: [r + depth, k],
        },
        Seg::Line {
            a: [r + depth, k],
            b: s,
        },
    ]
}

/// The region a fillet (`round`) or chamfer of `s` takes at a right-angled corner `(x, y)` of a
/// meridian, the material toward `(sx, sy)`, turned about the axis.
fn ring(round: bool, x: f64, y: f64, s: f64, sx: f64, sy: f64) -> Shape {
    let profile = if round {
        spandrel(x, y, s, sx, sy)
    } else {
        corner_triangle(x, y, s, sx, sy)
    };
    Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile,
        angle: TAU,
    }
    .into()
}

/// The same corner region run straight along an edge `len` long, drawn in `frame`.
fn strip(frame: Frame, x: f64, y: f64, s: f64, sx: f64, sy: f64, len: f64) -> Shape {
    Primitive::Extrude {
        frame,
        profile: spandrel(x, y, s, sx, sy),
        height: len,
    }
    .into()
}

/// The points in counter-clockwise order.
fn ordered(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let n = pts.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum();
    if twice < 0.0 {
        pts.reverse();
    }
    pts
}

/// tan α − α at radius `rho` of an involute of the circle `rb`.
fn inv_at(rb: f64, rho: f64) -> f64 {
    let a = (rb / rho).min(1.0).pacos();
    a.ptan() - a
}

/// A conic piece `(a, apex, b, w)`: its ends, the apex of its tangents there, and its weight.
type Conic = ([f64; 2], [f64; 2], [f64; 2], f64);

/// A gear outline: `z` teeth, pitch radius `rp`, base `rb`, half a tooth's angle `psi` at the
/// pitch circle, tips at `ra` and roots at `rf`. Each flank is the conic tangent to the involute at
/// both its ends and through it in the middle, which every kernel builds exactly.
struct Gear {
    z: usize,
    rp: f64,
    rb: f64,
    psi: f64,
    ra: f64,
    rf: f64,
    /// Whether each root meets its flanks in a fillet of radius 0.38m, the tip radius of the
    /// common basic rack (ISO 53), as a hob cuts it; an internal gear's spaces, drawn as a gear
    /// turned inside out, have none at their tips.
    fillets: bool,
}

impl Gear {
    fn new(z: usize, m: f64, backlash: f64) -> Gear {
        let rp = m * z as f64 / 2.0;
        Gear {
            z,
            rp,
            rb: rp * 20f64.to_radians().pcos(),
            psi: PI / (2.0 * z as f64) - backlash / (2.0 * rp),
            ra: rp + m,
            rf: rp - 1.25 * m,
            fillets: true,
        }
    }

    /// Half the tooth's angular width at radius `rho`.
    fn half(&self, rho: f64) -> f64 {
        self.psi + inv_at(self.rb, self.rp) - inv_at(self.rb, rho)
    }

    fn r0(&self) -> f64 {
        self.rb.max(self.rf)
    }

    /// The tooth at angle 0's clockwise flank from its root to its tip as conics `(a, apex, b,
    /// w)`, each tangent to the involute at both ends and through it at its middle, split until
    /// none strays from the involute by more than a hundred-thousandth of the module.
    fn flank(&self) -> Vec<Conic> {
        let m = 2.0 * self.rp / self.z as f64;
        let mut out = Vec::new();
        let mut todo = vec![(self.r0(), self.ra)];
        while let Some((lo, hi)) = todo.pop() {
            let c = self.flank_piece(lo, hi);
            let (a, apex, b, w) = c;
            let at = |s: f64| {
                let (u, v) = (1.0 - s, s);
                let d = u * u + 2.0 * w * u * v + v * v;
                [0, 1].map(|k| (u * u * a[k] + 2.0 * w * u * v * apex[k] + v * v * b[k]) / d)
            };
            let stray = [0.25, 0.75]
                .iter()
                .map(|&s| {
                    let q = at(s);
                    let rho = q[0].phypot(q[1]);
                    (rho * (q[1].patan2(q[0]) + self.half(rho))).abs()
                })
                .fold(0.0, f64::max);
            if stray > 1e-5 * m && out.len() + todo.len() < 64 {
                let mid = (lo + hi) / 2.0;
                todo.push((mid, hi));
                todo.push((lo, mid));
            } else {
                out.push(c);
            }
        }
        out
    }

    /// The conic tangent to the clockwise flank's involute at radii `lo` and `hi` and through it
    /// at its middle.
    fn flank_piece(&self, lo: f64, hi: f64) -> Conic {
        let at = |rho: f64| {
            let a = -self.half(rho);
            [rho * a.pcos(), rho * a.psin()]
        };
        let tangent = |rho: f64| {
            let a = -self.half(rho);
            let t = ((rho / self.rb).ppowi(2) - 1.0).max(0.0).sqrt();
            [a.pcos() - t * a.psin(), a.psin() + t * a.pcos()]
        };
        let (p0, p1, t0, t1) = (at(lo), at(hi), tangent(lo), tangent(hi));
        let det = t0[0] * -t1[1] + t1[0] * t0[1];
        let d = [p1[0] - p0[0], p1[1] - p0[1]];
        let s = (d[0] * -t1[1] + t1[0] * d[1]) / det;
        let apex = [p0[0] + s * t0[0], p0[1] + s * t0[1]];
        let mid = [(p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0];
        let off = |s: f64| {
            let q = [
                mid[0] + s * (apex[0] - mid[0]),
                mid[1] + s * (apex[1] - mid[1]),
            ];
            q[1].patan2(q[0]) + self.half(q[0].phypot(q[1]))
        };
        let (mut lo, mut hi) = (0.0, 1.0);
        let up = off(hi) > off(lo);
        for _ in 0..100 {
            let m = (lo + hi) / 2.0;
            if (off(m) > 0.0) == up {
                hi = m;
            } else {
                lo = m;
            }
        }
        let s = (lo + hi) / 2.0;
        (p0, apex, p1, s / (1.0 - s))
    }

    fn outline(&self) -> Vec<Seg> {
        let flank = self.flank();
        let flip = |p: [f64; 2]| [p[0], -p[1]];
        let (r0, ra, rf) = (self.r0(), self.ra, self.rf);
        let (h0, ha) = (self.half(r0), self.half(ra));
        let pitch = TAU / self.z as f64;
        assert!(
            ha > 0.0 && pitch - 2.0 * h0 > 0.0,
            "a gear's teeth must neither point nor meet"
        );
        // The root fillet: tangent to the radial line below each flank and to the root circle,
        // its centre `rf + ρ` out and `δ` round from the line, touching the line at radius `s`.
        // No larger than the radial line below the involute holds.
        let m = 2.0 * self.rp / self.z as f64;
        let rho = if self.fillets && rf < r0 {
            (0.38 * m).min(0.9 * (r0 * r0 - rf * rf) / (2.0 * rf))
        } else {
            0.0
        };
        let delta = (rho / (rf + rho)).pasin();
        let s = (rf * rf + 2.0 * rf * rho).sqrt();
        assert!(
            pitch - 2.0 * h0 - 2.0 * delta > 0.0,
            "a gear's root fillets must not meet"
        );
        let polar = |r: f64, ang: f64| [r * ang.pcos(), r * ang.psin()];
        // The fillet about its centre at angle `c_ang`, from `p` to `q` the short way round.
        let fillet = |c_ang: f64, p: [f64; 2], q: [f64; 2]| {
            let c = polar(rf + rho, c_ang);
            let start = (p[1] - c[1]).patan2(p[0] - c[0]);
            let end = (q[1] - c[1]).patan2(q[0] - c[0]);
            let sweep = (end - start + PI).rem_euclid(TAU) - PI;
            Seg::Arc {
                center: c,
                radius: rho,
                start,
                sweep,
            }
        };
        let mut out = Vec::new();
        for k in 0..self.z {
            let t = pitch * k as f64;
            if rf < r0 {
                let (root, foot) = (polar(rf, t - h0 - delta), polar(s, t - h0));
                if rho > 0.0 {
                    out.push(fillet(t - h0 - delta, root, foot));
                }
                out.push(Seg::Line {
                    a: foot,
                    b: polar(r0, t - h0),
                });
            }
            for &(a, apex, b, w) in &flank {
                out.push(Seg::Conic {
                    a: rot(a, t),
                    apex: rot(apex, t),
                    b: rot(b, t),
                    w,
                });
            }
            out.push(Seg::Arc {
                center: [0.0, 0.0],
                radius: ra,
                start: t - ha,
                sweep: 2.0 * ha,
            });
            for &(a, apex, b, w) in flank.iter().rev() {
                out.push(Seg::Conic {
                    a: rot(flip(b), t),
                    apex: rot(flip(apex), t),
                    b: rot(flip(a), t),
                    w,
                });
            }
            if rf < r0 {
                let (foot, root) = (polar(s, t + h0), polar(rf, t + h0 + delta));
                out.push(Seg::Line {
                    a: polar(r0, t + h0),
                    b: foot,
                });
                if rho > 0.0 {
                    out.push(fillet(t + h0 + delta, foot, root));
                }
            }
            out.push(Seg::Arc {
                center: [0.0, 0.0],
                radius: rf,
                start: t + h0 + delta,
                sweep: pitch - 2.0 * h0 - 2.0 * delta,
            });
        }
        out
    }
}

fn count(r: &mut Rng, lo: usize, hi: usize) -> usize {
    lo + (r.next_u64() % (hi - lo + 1) as u64) as usize
}

/// `n` cylinders round the z axis at radius `rc`, from z0 up by `h`, starting at angle `a0`.
fn bolt_circle(n: usize, rc: f64, rh: f64, z0: f64, h: f64, a0: f64) -> Shape {
    union_of(
        (0..n)
            .map(|i| {
                let a = a0 + TAU * i as f64 / n as f64;
                cyl([rc * a.pcos(), rc * a.psin(), z0], Z, rh, h)
            })
            .collect(),
    )
}

// ---- Transmission ----

/// A spur gear of `z` teeth of module `m`, `h` wide: the toothed rim, a hub on it, a keyed bore
/// through both, and `holes` lightening holes when there is room between hub and root.
pub(super) fn spur_gear(z: usize, m: f64, h: f64, holes: usize) -> Drawn {
    let g = Gear::new(z, m, 0.0);
    let hub_r = (g.rf * 0.4).min(4.0 * m + 6.0);
    let hub_h = 0.6 * h;
    let bore_r = hub_r * 0.5;
    let bore = keyed_bore(bore_r, bore_r * 0.25, bore_r * 0.25);
    let part = Part::new(ext(Frame::at([0.0; 3]), vec![g.outline()], h))
        .join(
            cyl([0.0, 0.0, h], Z, hub_r, hub_h),
            PI * hub_r * hub_r * hub_h,
        )
        .cut(
            ext(
                Frame::at([0.0, 0.0, -1.0]),
                vec![bore.clone()],
                h + hub_h + 2.0,
            ),
            area(&bore) * (h + hub_h),
        );
    if holes == 0 {
        return part.done();
    }
    let rc = (hub_r + g.rf) / 2.0;
    let rh = ((g.rf - hub_r) * 0.3).min(rc * (PI / holes as f64).psin() * 0.6);
    part.cut(
        bolt_circle(holes, rc, rh, -1.0, h + 2.0, PI / holes as f64),
        holes as f64 * PI * rh * rh * h,
    )
    .done()
}

/// A helical gear: the tooth outline swept straight up while turning at the helix angle, then
/// bored. Every slice is the outline turned, so its volume is the outline's area times its height.
/// `hand` is 1 for a right-handed helix, -1 for a left; `beta` is the helix angle in degrees.
pub(super) fn helical_gear(z: usize, m: f64, h: f64, beta: f64, hand: f64) -> Drawn {
    let g = Gear::new(z, m, 0.0);
    let beta = beta.to_radians();
    let body = Primitive::SweepPath {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![g.outline()],
        },
        path: PathCurve::new(
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            (0..4).map(|i| [0.0, 0.0, h * i as f64 / 3.0]).collect(),
        ),
        twist: hand * h * beta.ptan() / g.rp,
    };
    let bore_r = g.rf * 0.35;
    Part::new(body)
        .cut(
            cyl([0.0, 0.0, -1.0], Z, bore_r, h + 2.0),
            PI * bore_r * bore_r * h,
        )
        .done()
}

/// An internal (ring) gear: teeth pointing in from a ring, its spaces shaped as an outer gear's
/// teeth, with a bolt circle through the ring.
pub(super) fn internal_gear(z: usize, m: f64, h: f64, holes: usize) -> Drawn {
    let rp = m * z as f64 / 2.0;
    let spaces = Gear {
        z,
        rp,
        rb: rp * 20f64.to_radians().pcos(),
        psi: PI / (2.0 * z as f64),
        ra: rp + 1.25 * m,
        rf: rp - m,
        fillets: false,
    };
    let ro = spaces.ra + 6.0 * m;
    let n = holes;
    let rc = (spaces.ra + ro) / 2.0;
    let rh = (ro - spaces.ra) * 0.25;
    Part::new(ext(
        Frame::at([0.0; 3]),
        vec![circle([0.0, 0.0], ro, true), reversed(spaces.outline())],
        h,
    ))
    .cut(
        bolt_circle(n, rc, rh, -1.0, h + 2.0, 0.0),
        n as f64 * PI * rh * rh * h,
    )
    .done()
}

/// A rack: straight-flanked teeth on a bar, with counterbored mounting holes from below.
/// `n` teeth of module `m`, `b` wide.
pub(super) fn rack(n: usize, m: f64, b: f64) -> Drawn {
    let p = PI * m;
    let tan = 20f64.to_radians().ptan();
    let bar = 6.0 * m;
    let (yb, yp) = (bar, bar + 1.25 * m);
    let yt = yp + m;
    let half = |y: f64| p / 4.0 - (y - yp) * tan;
    let len = n as f64 * p;
    let mut pts = vec![[0.0, 0.0], [len, 0.0], [len, yb]];
    for k in (0..n).rev() {
        let x = p * (k as f64 + 0.5);
        pts.extend([
            [x + half(yb), yb],
            [x + half(yt), yt],
            [x - half(yt), yt],
            [x - half(yb), yb],
        ]);
    }
    pts.push([0.0, yb]);
    let (rh, rcb) = (b * 0.15, b * 0.28);
    let (dh, dc) = (yb * 0.7, yb * 0.2);
    let at = [len * 0.2, len * 0.8];
    let holes = |rad: f64, d: f64| {
        union_of(
            at.iter()
                .map(|&x| cyl([x, -1.0, b / 2.0], Y, rad, 1.0 + d))
                .collect(),
        )
    };
    Part::new(ext(Frame::at([0.0; 3]), vec![poly(&pts)], b))
        .cut(holes(rh, dh), 2.0 * PI * rh * rh * dh)
        .cut(holes(rcb, dc), 2.0 * PI * (rcb * rcb - rh * rh) * dc)
        .done()
}

/// A timing pulley: a toothed drum between two flanges, bored through.
pub(super) fn timing_pulley(r: &mut Rng) -> Drawn {
    let z = count(r, 16, 40);
    let ro = r.range(8.0, 18.0);
    let g = TAU * ro / z as f64 * r.range(0.2, 0.3);
    let delta = (1.0 - g * g / (2.0 * ro * ro)).pacos();
    let pitch = TAU / z as f64;
    let mut lp = Vec::new();
    for k in 0..z {
        let t = pitch * k as f64;
        lp.push(Seg::Arc {
            center: [0.0, 0.0],
            radius: ro,
            start: t + delta,
            sweep: pitch - 2.0 * delta,
        });
        let tn = t + pitch;
        let c = [ro * tn.pcos(), ro * tn.psin()];
        let from = [
            ro * (tn - delta).pcos() - c[0],
            ro * (tn - delta).psin() - c[1],
        ];
        let to = [
            ro * (tn + delta).pcos() - c[0],
            ro * (tn + delta).psin() - c[1],
        ];
        let start = from[1].patan2(from[0]);
        let d = (to[1].patan2(to[0]) - start).rem_euclid(TAU);
        lp.push(Seg::Arc {
            center: c,
            radius: g,
            start,
            sweep: -(TAU - d),
        });
    }
    let (w, tf, fl) = (r.range(6.0, 12.0), r.range(0.8, 1.5), r.range(1.0, 2.0));
    let rf = ro + fl;
    let rb = (ro - g) * r.range(0.3, 0.5);
    let drum = ext(Frame::at([0.0, 0.0, tf]), vec![lp.clone()], w);
    let drum_v = area(&lp) * w;
    let flange_v = PI * rf * rf * tf;
    Part::new(cyl([0.0; 3], Z, rf, tf))
        .join(drum, drum_v)
        .join(cyl([0.0, 0.0, tf + w], Z, rf, tf), flange_v)
        .cut(
            cyl([0.0, 0.0, -1.0], Z, rb, w + 2.0 * tf + 2.0),
            PI * rb * rb * (w + 2.0 * tf),
        )
        .done()
}

/// A roller-chain sprocket's outline: seats for the rollers, straight flanks out to the tips.
fn sprocket_outline(z: usize, p: f64) -> (Vec<Seg>, f64, f64) {
    let rp = p / (2.0 * (PI / z as f64).psin());
    let rs = 0.32 * p;
    let beta = 20f64.to_radians();
    let pitch = TAU / z as f64;
    let seat = |k: usize| {
        let t = pitch * k as f64;
        let c = [rp * t.pcos(), rp * t.psin()];
        let (s, e) = (t - PI / 2.0 - beta, t + PI / 2.0 + beta);
        let on = |a: f64| [c[0] + rs * a.pcos(), c[1] + rs * a.psin()];
        // Leaving the seat clockwise at e, and arriving clockwise at s.
        (
            c,
            s,
            e,
            on(s),
            on(e),
            [e.psin(), -e.pcos()],
            [s.psin(), -s.pcos()],
        )
    };
    let hit = |p0: [f64; 2], d: [f64; 2], radius: f64, sign: f64| {
        let b = p0[0] * d[0] + p0[1] * d[1];
        let c = p0[0] * p0[0] + p0[1] * p0[1] - radius * radius;
        let s = -sign * b + (b * b - c).sqrt();
        [p0[0] + sign * s * d[0], p0[1] + sign * s * d[1]]
    };
    // The flanks of one gap meet where the line leaving seat 0 crosses the one into seat 1.
    let (_, _, _, _, e0, d0, _) = seat(0);
    let (_, _, _, s1, _, _, d1) = seat(1);
    let det = d0[0] * d1[1] - d0[1] * d1[0];
    let q = [s1[0] - e0[0], s1[1] - e0[1]];
    let u = (q[0] * d1[1] - q[1] * d1[0]) / det;
    let peak = [e0[0] + u * d0[0], e0[1] + u * d0[1]];
    let re = e0[0].phypot(e0[1]);
    let ra = re + 0.8 * (peak[0].phypot(peak[1]) - re);
    let mut out = Vec::new();
    for k in 0..z {
        let (c, s, e, _, ek, dk, _) = seat(k);
        let (_, _, _, sn, _, _, dn) = seat(k + 1);
        out.push(Seg::Arc {
            center: c,
            radius: rs,
            start: s,
            sweep: -(TAU - (e - s)),
        });
        let t0 = hit(ek, dk, ra, 1.0);
        let t1 = hit(sn, dn, ra, -1.0);
        out.push(Seg::Line { a: ek, b: t0 });
        let a0 = t0[1].patan2(t0[0]);
        let sweep = (t1[1].patan2(t1[0]) - a0).rem_euclid(TAU);
        out.push(Seg::Arc {
            center: [0.0, 0.0],
            radius: ra,
            start: a0,
            sweep,
        });
        out.push(Seg::Line { a: t1, b: sn });
    }
    // The loop starts at the first seat's arrival point; rotate so it closes there.
    (out, rp, rs)
}

/// A sprocket: the toothed plate, a hub on it, bored through.
pub(super) fn sprocket(r: &mut Rng) -> Drawn {
    let z = count(r, 12, 30);
    let p = r.range(4.0, 9.0);
    let (lp, rp, rs) = sprocket_outline(z, p);
    let t = p * r.range(0.4, 0.6);
    let hub_r = (rp - rs) * r.range(0.4, 0.6);
    let hub_h = r.range(3.0, 8.0);
    let bore_r = hub_r * r.range(0.4, 0.6);
    Part::new(ext(Frame::at([0.0; 3]), vec![lp], t))
        .join(
            cyl([0.0, 0.0, t], Z, hub_r, hub_h),
            PI * hub_r * hub_r * hub_h,
        )
        .cut(
            cyl([0.0, 0.0, -1.0], Z, bore_r, t + hub_h + 2.0),
            PI * bore_r * bore_r * (t + hub_h),
        )
        .done()
}

/// The outline of a splined shaft's section: `n` square splines `w` wide from radius `rm` to `ro`.
fn spline_section(n: usize, rm: f64, ro: f64, w: f64) -> Vec<Seg> {
    let (am, ao) = ((w / 2.0 / rm).pasin(), (w / 2.0 / ro).pasin());
    let pitch = TAU / n as f64;
    let at = |rad: f64, a: f64| [rad * a.pcos(), rad * a.psin()];
    let mut out = Vec::new();
    for k in 0..n {
        let t = pitch * k as f64;
        out.push(Seg::Line {
            a: at(rm, t - am),
            b: at(ro, t - ao),
        });
        out.push(Seg::Arc {
            center: [0.0, 0.0],
            radius: ro,
            start: t - ao,
            sweep: 2.0 * ao,
        });
        out.push(Seg::Line {
            a: at(ro, t + ao),
            b: at(rm, t + am),
        });
        out.push(Seg::Arc {
            center: [0.0, 0.0],
            radius: rm,
            start: t + am,
            sweep: pitch - 2.0 * am,
        });
    }
    out
}

/// A splined shaft: a stepped turned shaft, a splined end on it, a cross hole for a pin.
pub(super) fn splined_shaft(r: &mut Rng) -> Drawn {
    let (r1, l1) = (r.range(5.0, 9.0), r.range(10.0, 20.0));
    let (r2, l2) = (r1 * r.range(0.65, 0.85), r.range(8.0, 16.0));
    let n = count(r, 6, 10);
    let ro = r2 * r.range(0.9, 1.0);
    let rm = ro * r.range(0.78, 0.88);
    let w = (rm * (PI / n as f64).psin() * r.range(0.6, 0.9)).min(ro * 0.5);
    let ls = r.range(8.0, 14.0);
    let shaft = rev(
        [0.0; 3],
        poly(&[
            [0.0, 0.0],
            [r1, 0.0],
            [r1, l1],
            [r2, l1],
            [r2, l1 + l2],
            [0.0, l1 + l2],
        ]),
    );
    let sec = spline_section(n, rm, ro, w);
    let a = r1 * r.range(0.15, 0.3);
    let zh = l1 * r.range(0.3, 0.7);
    Part::new(shaft)
        .join(
            ext(Frame::at([0.0, 0.0, l1 + l2]), vec![sec.clone()], ls),
            area(&sec) * ls,
        )
        .cut(
            cyl([-r1 - 1.0, 0.0, zh], X, a, 2.0 * r1 + 2.0),
            crossing(r1, a),
        )
        .done()
}

/// Two spur gears of `z1` and `z2` teeth in mesh at their centre distance, with a backlash of
/// `bl` modules: they share no volume.
pub(super) fn meshing_pair(z1: usize, z2: usize, m: f64, bl: f64, h: f64) -> Drawn {
    let bl = m * bl;
    let (g1, g2) = (Gear::new(z1, m, bl), Gear::new(z2, m, bl));
    let a = g1.rp + g2.rp;
    let turn = PI + PI / z2 as f64;
    let second: Vec<Seg> = g2
        .outline()
        .into_iter()
        .map(|s| moved(s, turn, [a, 0.0]))
        .collect();
    let (p1, p2) = (
        ext(Frame::at([0.0; 3]), vec![g1.outline()], h),
        ext(Frame::at([0.0; 3]), vec![second], h),
    );
    let exact = Exact {
        intersection_volume: Some(0.0),
        ..Default::default()
    };
    (p1.into(), p2.into(), exact)
}

/// A segment turned by `a` about the origin, then shifted by `d`.
fn moved(s: Seg, a: f64, d: [f64; 2]) -> Seg {
    let m = |p: [f64; 2]| {
        let q = rot(p, a);
        [q[0] + d[0], q[1] + d[1]]
    };
    match s {
        Seg::Line { a: p, b } => Seg::Line { a: m(p), b: m(b) },
        Seg::Arc {
            center,
            radius,
            start,
            sweep,
        } => Seg::Arc {
            center: m(center),
            radius,
            start: start + a,
            sweep,
        },
        Seg::Conic { a: p, apex, b, w } => Seg::Conic {
            a: m(p),
            apex: m(apex),
            b: m(b),
            w,
        },
        other => other,
    }
}

// ---- Fasteners ----

/// A split lock washer: a rectangle screwed most of a turn, its ends a thickness apart.
pub(super) fn lock_washer(r: &mut Rng) -> Drawn {
    let ri = r.range(1.5, 6.0);
    let w = ri * r.range(0.3, 0.45);
    let t = w * r.range(0.6, 0.9);
    let gap = r.range(0.15, 0.35);
    let angle = TAU - gap;
    let body = Primitive::Screw {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&[[ri, 0.0], [ri + w, 0.0], [ri + w, t], [ri, t]]),
        rise: t * r.range(1.0, 1.5) / angle,
        angle,
    };
    Part::new(body).done()
}

/// A helical compression spring: a round wire screwed through several turns.
pub(super) fn compression_spring(r: &mut Rng) -> Drawn {
    let rm = r.range(3.0, 8.0);
    let a = rm * r.range(0.08, 0.15);
    let pitch = a * r.range(2.6, 4.0);
    let turns = count(r, 3, 8) as f64 + r.range(0.0, 1.0);
    Part::new(Primitive::Screw {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![circle([rm, a], a, true)],
        },
        rise: pitch / TAU,
        angle: TAU * turns,
    })
    .done()
}

// ---- Shafts and rotating parts ----

/// The removed volume of a round-ended keyway `w` wide and `len` between its end centres, cut `t`
/// deep into a shaft of radius `rad`, measured from the shaft's top line.
fn keyway(rad: f64, w: f64, len: f64, t: f64) -> f64 {
    let rho = w / 2.0;
    quad(
        |th| {
            let x = rho * th.psin();
            let l = len + 2.0 * rho * th.pcos();
            l * ((rad * rad - x * x).sqrt() - (rad - t)) * rho * th.pcos()
        },
        -PI / 2.0,
        PI / 2.0,
    )
}

/// A stepped shaft: turned diameters with rounded shoulders, both ends chamfered, a keyway milled
/// in the widest step and a cross hole through another.
pub(super) fn stepped_shaft(r: &mut Rng) -> Drawn {
    let r1 = r.range(4.0, 7.0);
    let r2 = r1 * r.range(1.25, 1.5);
    let r3 = r1 * r.range(0.75, 0.95);
    let (l1, l2, l3) = (r.range(8.0, 14.0), r.range(16.0, 26.0), r.range(8.0, 14.0));
    let f = (r2 - r1).min(r2 - r3) * r.range(0.3, 0.6);
    let len = l1 + l2 + l3;
    let pts = [
        [0.0, 0.0],
        [r1, 0.0],
        [r1, l1],
        [r2, l1],
        [r2, l1 + l2],
        [r3, l1 + l2],
        [r3, len],
        [0.0, len],
    ];
    let shaft = rev(
        [0.0; 3],
        rounded(&pts, &[0.0, 0.0, f, 0.0, 0.0, f, 0.0, 0.0]),
    );
    let c = r1.min(r3) * r.range(0.08, 0.15);
    let ends = Shape::bool(
        Op::Union,
        ring(false, r1, 0.0, c, -1.0, 1.0),
        ring(false, r3, len, c, -1.0, -1.0),
    );
    let w = r2 * r.range(0.35, 0.5);
    let t = w * r.range(0.4, 0.6);
    let kl = l2 * r.range(0.3, 0.45);
    let zc = l1 + l2 * 0.5;
    let slot = ext(
        frame([0.0, -(r2 - t), 0.0], X, Z),
        vec![stadium([0.0, zc - kl / 2.0], [0.0, zc + kl / 2.0], w / 2.0)],
        r2 + 1.0,
    );
    let a = r1 * r.range(0.15, 0.3);
    let zh = l1 * r.range(0.35, 0.6);
    Part::new(shaft)
        .blend(
            Act::Chamfer,
            vec![[r1, 0.0, 0.0], [-r3, 0.0, len]],
            c,
            ends,
            false,
        )
        .cut(slot, keyway(r2, w, kl, t))
        .cut(
            cyl([0.0, -r1 - 1.0, zh], Y, a, 2.0 * r1 + 2.0),
            crossing(r1, a),
        )
        .done()
}

/// A slip-on pipe flange: a turned disc with a raised face and a hub, the hub's root filleted, a
/// bolt circle through the disc.
pub(super) fn pipe_flange(r: &mut Rng) -> Drawn {
    let ri = r.range(4.0, 10.0);
    let rh = ri * r.range(1.3, 1.5);
    let rr = rh * r.range(1.15, 1.3);
    let rf = rr * r.range(1.7, 2.1);
    let (tf, hh, rfh) = (
        ri * r.range(0.3, 0.5),
        ri * r.range(0.6, 1.0),
        r.range(0.3, 0.8),
    );
    let f = (rh - ri).min(hh) * r.range(0.2, 0.4);
    let body = rev(
        [0.0; 3],
        poly(&[
            [ri, -rfh],
            [rr, -rfh],
            [rr, 0.0],
            [rf, 0.0],
            [rf, tf],
            [rh, tf],
            [rh, tf + hh],
            [ri, tf + hh],
        ]),
    );
    let n = count(r, 4, 8);
    let rc = (rr.max(rh + f) + rf) / 2.0;
    let rb = ((rf - rr.max(rh + f)) / 2.0 * 0.6).min(rc * (PI / n as f64).psin() * 0.6);
    Part::new(body)
        .blend(
            Act::Fillet,
            vec![[rh, 0.0, tf]],
            f,
            ring(true, rh, tf, f, 1.0, 1.0),
            true,
        )
        .cut(
            bolt_circle(n, rc, rb, -1.0, tf + 2.0, PI / n as f64),
            n as f64 * PI * rb * rb * tf,
        )
        .done()
}

/// A V-belt pulley: a turned hub, web and grooved rim, a keyway in its bore, holes in its web.
pub(super) fn v_pulley(r: &mut Rng) -> Drawn {
    let ri = r.range(3.0, 6.0);
    let rhub = ri * r.range(1.6, 2.0);
    let n_g = count(r, 1, 3);
    let gw = r.range(4.0, 7.0);
    let gd = gw * r.range(0.6, 0.8);
    let gb = gw - 2.0 * gd * 19f64.to_radians().ptan();
    let land = gw * 0.4;
    let w = n_g as f64 * (gw + land) + land;
    let ro = rhub + r.range(10.0, 18.0);
    let rin = ro - gd - r.range(2.0, 4.0);
    let (zw0, zw1) = (w * 0.35, w * 0.65);
    let mut pts = vec![
        [ri, 0.0],
        [rhub, 0.0],
        [rhub, zw0],
        [rin, zw0],
        [rin, 0.0],
        [ro, 0.0],
    ];
    for k in 0..n_g {
        let zc = land + gw / 2.0 + k as f64 * (gw + land);
        pts.extend([
            [ro, zc - gw / 2.0],
            [ro - gd, zc - gb / 2.0],
            [ro - gd, zc + gb / 2.0],
            [ro, zc + gw / 2.0],
        ]);
    }
    pts.extend([
        [ro, w],
        [rin, w],
        [rin, zw1],
        [rhub, zw1],
        [rhub, w],
        [ri, w],
    ]);
    let kw = ri * 0.5;
    let kd = ri * 0.3;
    let key = Primitive::Box {
        frame: Frame::at([0.0, -kw / 2.0, -1.0]),
        size: [ri + kd, kw, w + 2.0],
    };
    let n = count(r, 4, 6);
    let rc = (rhub + rin) / 2.0;
    let rh = ((rin - rhub) * 0.3).min(rc * (PI / n as f64).psin() * 0.6);
    Part::new(rev([0.0; 3], poly(&pts)))
        .cut(key, w * (kw * (ri + kd) - half_strip(ri, kw)))
        .cut(
            bolt_circle(n, rc, rh, -1.0, w + 2.0, PI / n as f64),
            n as f64 * PI * rh * rh * (zw1 - zw0),
        )
        .done()
}

/// A kidney-shaped window: an arc slot of half-width `wk` round radius `rc`, its ends' centres
/// `phi` either side of angle `t`, counter-clockwise.
fn kidney(rc: f64, wk: f64, phi: f64, t: f64) -> Vec<Seg> {
    let end = |a: f64| [rc * (t + a).pcos(), rc * (t + a).psin()];
    vec![
        Seg::Arc {
            center: [0.0, 0.0],
            radius: rc + wk,
            start: t - phi,
            sweep: 2.0 * phi,
        },
        Seg::Arc {
            center: end(phi),
            radius: wk,
            start: t + phi,
            sweep: PI,
        },
        Seg::Arc {
            center: [0.0, 0.0],
            radius: rc - wk,
            start: t + phi,
            sweep: -2.0 * phi,
        },
        Seg::Arc {
            center: end(-phi),
            radius: wk,
            start: t - phi + PI,
            sweep: PI,
        },
    ]
}

/// A wheel: a thin web, a rim and a hub joined across it, kidney windows through the web, a bore.
pub(super) fn windowed_wheel(r: &mut Rng) -> Drawn {
    let ro = r.range(20.0, 35.0);
    let ri = ro * r.range(0.8, 0.88);
    let rh = ro * r.range(0.18, 0.25);
    let tw = r.range(1.5, 3.0);
    let (hr, hh) = (tw * r.range(3.0, 5.0), tw * r.range(3.0, 6.0));
    let n = count(r, 4, 7);
    let rc = (rh + ri) / 2.0;
    let wk = (ri - rh) * r.range(0.2, 0.3);
    let phi = (PI / n as f64 - 2.0 * (wk / rc).pasin()).max(0.05) * r.range(0.5, 0.8);
    let windows = union_of(
        (0..n)
            .map(|i| {
                let t = TAU * i as f64 / n as f64;
                ext(
                    Frame::at([0.0, 0.0, -hh]),
                    vec![kidney(rc, wk, phi, t)],
                    2.0 * hh,
                )
            })
            .collect(),
    );
    let win_a = 2.0 * phi * rc * 2.0 * wk + PI * wk * wk;
    let rim = ext(
        Frame::at([0.0, 0.0, -(hr - tw) / 2.0]),
        vec![circle([0.0, 0.0], ro, true), circle([0.0, 0.0], ri, false)],
        hr,
    );
    let a_rim = PI * (ro * ro - ri * ri);
    let hub = cyl([0.0, 0.0, -(hh - tw) / 2.0], Z, rh, hh);
    let rb = rh * r.range(0.35, 0.55);
    Part::new(cyl([0.0; 3], Z, ro, tw))
        .join(rim, a_rim * (hr - tw))
        .join(hub, PI * rh * rh * (hh - tw))
        .cut(windows, n as f64 * win_a * tw)
        .cut(cyl([0.0, 0.0, -hh], Z, rb, 2.0 * hh), PI * rb * rb * hh)
        .done()
}

/// A turbine disc: a turned hub, web and rim with rounded corners, dovetail slots broached
/// axially through the rim.
pub(super) fn turbine_disc(r: &mut Rng) -> Drawn {
    let ri = r.range(3.0, 6.0);
    let rh = ri * r.range(1.8, 2.4);
    let rr = rh + r.range(10.0, 16.0);
    let ro = rr + r.range(5.0, 8.0);
    let (hh, tw, w) = (r.range(8.0, 12.0), r.range(2.0, 3.5), r.range(5.0, 8.0));
    let f = (tw * 0.5).min((w - tw) / 2.0 * 0.6);
    let pts = [
        [ri, -hh / 2.0],
        [rh, -hh / 2.0],
        [rh, -tw / 2.0],
        [rr, -tw / 2.0],
        [rr, -w / 2.0],
        [ro, -w / 2.0],
        [ro, w / 2.0],
        [rr, w / 2.0],
        [rr, tw / 2.0],
        [rh, tw / 2.0],
        [rh, hh / 2.0],
        [ri, hh / 2.0],
    ];
    let radii = [0.0, 0.0, f, f, 0.0, 0.0, 0.0, 0.0, f, f, 0.0, 0.0];
    let disc = rev([0.0; 3], rounded(&pts, &radii));
    let n = count(r, 12, 30);
    let rb = rr + f + (ro - rr - f) * r.range(0.3, 0.45);
    let pitch_w = TAU * rb / n as f64;
    let (wb, wt) = (pitch_w * r.range(0.4, 0.5), pitch_w * r.range(0.2, 0.3));
    // The slot's section within the rim: its sides from the floor out to the rim's circle.
    let reach = |y0: f64, y1: f64| {
        let (a, d) = ([rb, y0], [ro + 1.0 - rb, y1 - y0]);
        let (qa, qb, qc) = (
            d[0] * d[0] + d[1] * d[1],
            2.0 * (a[0] * d[0] + a[1] * d[1]),
            a[0] * a[0] + a[1] * a[1] - ro * ro,
        );
        let s = (-qb + (qb * qb - 4.0 * qa * qc).sqrt()) / (2.0 * qa);
        [a[0] + s * d[0], a[1] + s * d[1]]
    };
    let (lo, hi) = (reach(-wb / 2.0, -wt / 2.0), reach(wb / 2.0, wt / 2.0));
    let section = vec![
        Seg::Line {
            a: [rb, -wb / 2.0],
            b: lo,
        },
        Seg::Arc {
            center: [0.0, 0.0],
            radius: ro,
            start: lo[1].patan2(lo[0]),
            sweep: hi[1].patan2(hi[0]) - lo[1].patan2(lo[0]),
        },
        Seg::Line {
            a: hi,
            b: [rb, wb / 2.0],
        },
        Seg::Line {
            a: [rb, wb / 2.0],
            b: [rb, -wb / 2.0],
        },
    ];
    let slots = union_of(
        (0..n)
            .map(|i| {
                let t = TAU * i as f64 / n as f64;
                let (c, s) = (t.pcos(), t.psin());
                ext(
                    frame([0.0, 0.0, -w / 2.0 - 1.0], [c, s, 0.0], [-s, c, 0.0]),
                    vec![poly(&[
                        [rb, -wb / 2.0],
                        [ro + 1.0, -wt / 2.0],
                        [ro + 1.0, wt / 2.0],
                        [rb, wb / 2.0],
                    ])],
                    w + 2.0,
                )
            })
            .collect(),
    );
    Part::new(disc)
        .cut(slots, n as f64 * area(&section) * w)
        .done()
}

// ---- Brackets ----

/// An L bracket: a bent plate with concentric inside and outside radii, holes in both legs, a slot
/// in its base.
pub(super) fn l_bracket(r: &mut Rng) -> Drawn {
    let t = r.range(2.0, 4.0);
    let (a, b, w) = (
        r.range(20.0, 35.0),
        r.range(20.0, 35.0),
        r.range(15.0, 30.0),
    );
    let ri = t * r.range(0.5, 1.5);
    let pts = [[0.0, 0.0], [a, 0.0], [a, t], [t, t], [t, b], [0.0, b]];
    let lp = rounded(&pts, &[ri + t, 0.0, 0.0, ri, 0.0, 0.0]);
    let body = ext(xz([0.0, w, 0.0]), vec![lp], w);
    let rh = (w * 0.1).min(t * 1.5);
    let x0 = t + ri + rh + 1.0;
    let xs = (x0 + a) / 2.0;
    let base_holes = union_of(vec![
        cyl([xs, w * 0.25, -1.0], Z, rh, t + 2.0),
        cyl([xs, w * 0.75, -1.0], Z, rh, t + 2.0),
    ]);
    let z0 = t + ri + rh + 1.0;
    let zs = (z0 + b) / 2.0;
    let leg_holes = union_of(vec![
        cyl([-1.0, w * 0.25, zs], X, rh, t + 2.0),
        cyl([-1.0, w * 0.75, zs], X, rh, t + 2.0),
    ]);
    let sl = (a - x0) * 0.3;
    let slot = ext(
        Frame::at([0.0, 0.0, -1.0]),
        vec![stadium(
            [xs - sl / 2.0, w / 2.0],
            [xs + sl / 2.0, w / 2.0],
            rh * 0.8,
        )],
        t + 2.0,
    );
    let slot_a = 2.0 * rh * 0.8 * sl + PI * (rh * 0.8).ppowi(2);
    Part::new(body)
        .cut(base_holes, 2.0 * PI * rh * rh * t)
        .cut(leg_holes, 2.0 * PI * rh * rh * t)
        .cut(slot, slot_a * t)
        .done()
}

/// A mounting plate: a rounded rectangle with countersunk holes at its corners and a counterbored
/// hole in its middle.
pub(super) fn mounting_plate(r: &mut Rng) -> Drawn {
    let (w, d, t) = (r.range(30.0, 50.0), r.range(25.0, 40.0), r.range(3.0, 6.0));
    let rc = w.min(d) * r.range(0.08, 0.15);
    let body = ext(
        Frame::at([0.0; 3]),
        vec![rounded(&[[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], &[rc; 4])],
        t,
    );
    let rh = t * r.range(0.35, 0.5);
    let c = t * r.range(0.3, 0.5);
    let m = rc.max(rh + c) + 2.0;
    let corners = [[m, m], [w - m, m], [w - m, d - m], [m, d - m]];
    let holes = union_of(
        corners
            .iter()
            .map(|p| cyl([p[0], p[1], -1.0], Z, rh, t + 2.0))
            .collect(),
    );
    let e = 0.2;
    let sinks = union_of(
        corners
            .iter()
            .map(|p| Primitive::Cone {
                frame: Frame::at([p[0], p[1], t - c - e]),
                bottom: rh - e,
                top: rh + c + 1.0,
                height: c + e + 1.0,
            })
            .collect(),
    );
    let sink_v = PI * c / 3.0 * (rh * rh + rh * (rh + c) + (rh + c).ppowi(2)) - PI * rh * rh * c;
    let (rb, rcb, dc) = (t * 0.6, t * 1.1, t * r.range(0.3, 0.5));
    Part::new(body)
        .cut(holes, 4.0 * PI * rh * rh * t)
        .cut(sinks, 4.0 * sink_v)
        .cut(
            cyl([w / 2.0, d / 2.0, -1.0], Z, rb, t + 2.0),
            PI * rb * rb * t,
        )
        .cut(
            cyl([w / 2.0, d / 2.0, t - dc], Z, rcb, dc + 1.0),
            PI * (rcb * rcb - rb * rb) * dc,
        )
        .done()
}

/// A gusseted angle bracket: a sharp-cornered angle, two triangular gussets in its inside
/// corner, holes in each leg clear of them.
pub(super) fn gusset_bracket(r: &mut Rng) -> Drawn {
    let t = r.range(2.5, 4.0);
    let (a, b, w) = (
        r.range(25.0, 40.0),
        r.range(25.0, 40.0),
        r.range(25.0, 40.0),
    );
    let body = ext(
        xz([0.0, w, 0.0]),
        vec![poly(&[
            [0.0, 0.0],
            [a, 0.0],
            [a, t],
            [t, t],
            [t, b],
            [0.0, b],
        ])],
        w,
    );
    let g = t * r.range(0.8, 1.2);
    let gl = a.min(b) * r.range(0.45, 0.6);
    let tri = poly(&[[t, t], [t + gl, t], [t, t + gl]]);
    let ys = [w * 0.15, w * 0.85 - g];
    let gussets = union_of(
        ys.iter()
            .map(|&y| ext(xz([0.0, y + g, 0.0]), vec![tri.clone()], g))
            .collect(),
    );
    let rh = t * 1.2;
    let x = t + gl + (a - t - gl) / 2.0;
    let z = t + gl + (b - t - gl) / 2.0;
    Part::new(body)
        .join(gussets, 2.0 * g * gl * gl / 2.0)
        .cut(cyl([x, w / 2.0, -1.0], Z, rh, t + 2.0), PI * rh * rh * t)
        .cut(cyl([-1.0, w / 2.0, z], X, rh, t + 2.0), PI * rh * rh * t)
        .done()
}

/// A bent Z bracket: a strip of sheet bent up and over at two radii, holes in both flanges and a
/// slot in its web.
pub(super) fn bent_bracket(r: &mut Rng) -> Drawn {
    let t = r.range(1.0, 2.5);
    let ri = t * r.range(0.8, 2.0);
    let (a, b) = (r.range(12.0, 25.0), r.range(12.0, 25.0));
    let height = 2.0 * (ri + t) + r.range(10.0, 25.0);
    let w = r.range(15.0, 30.0);
    let xr = a + ri + t;
    let c1 = [a, ri + t];
    let c2 = [xr + ri, height - t - ri];
    let lp = vec![
        Seg::Line {
            a: [0.0, 0.0],
            b: [a, 0.0],
        },
        Seg::Arc {
            center: c1,
            radius: ri + t,
            start: -PI / 2.0,
            sweep: PI / 2.0,
        },
        Seg::Line {
            a: [xr, ri + t],
            b: [xr, c2[1]],
        },
        Seg::Arc {
            center: c2,
            radius: ri,
            start: PI,
            sweep: -PI / 2.0,
        },
        Seg::Line {
            a: [c2[0], height - t],
            b: [c2[0] + b, height - t],
        },
        Seg::Line {
            a: [c2[0] + b, height - t],
            b: [c2[0] + b, height],
        },
        Seg::Line {
            a: [c2[0] + b, height],
            b: [c2[0], height],
        },
        Seg::Arc {
            center: c2,
            radius: ri + t,
            start: PI / 2.0,
            sweep: PI / 2.0,
        },
        Seg::Line {
            a: [a + ri, c2[1]],
            b: [a + ri, ri + t],
        },
        Seg::Arc {
            center: c1,
            radius: ri,
            start: 0.0,
            sweep: -PI / 2.0,
        },
        Seg::Line {
            a: [a, t],
            b: [0.0, t],
        },
        Seg::Line {
            a: [0.0, t],
            b: [0.0, 0.0],
        },
    ];
    let body = ext(xz([0.0, w, 0.0]), vec![lp], w);
    let rh = (w * 0.12).min(a * 0.15).min(b * 0.15);
    let zm = (ri + t + c2[1]) / 2.0;
    let sl = (c2[1] - ri - t) * 0.3;
    let rs = rh.min(sl);
    Part::new(body)
        .cut(
            cyl([a * 0.5, w / 2.0, -1.0], Z, rh, t + 2.0),
            PI * rh * rh * t,
        )
        .cut(
            cyl([c2[0] + b * 0.5, w / 2.0, height - t - 1.0], Z, rh, t + 2.0),
            PI * rh * rh * t,
        )
        .cut(
            ext(
                yz([a + ri - 1.0, 0.0, 0.0]),
                vec![stadium(
                    [w / 2.0, zm - sl / 2.0],
                    [w / 2.0, zm + sl / 2.0],
                    rs * 0.8,
                )],
                t + 2.0,
            ),
            (2.0 * rs * 0.8 * sl + PI * (rs * 0.8).ppowi(2)) * t,
        )
        .done()
}

/// A clevis: a block with a round end and a pin hole, slotted between its prongs, a boss on its
/// back with a tapped hole.
pub(super) fn clevis(r: &mut Rng) -> Drawn {
    let hc = r.range(10.0, 16.0);
    let lb = hc * r.range(1.2, 1.8);
    let w = hc * r.range(1.0, 1.3);
    let rp = hc * r.range(0.15, 0.22);
    let lp = vec![
        Seg::Line {
            a: [0.0, -hc / 2.0],
            b: [lb, -hc / 2.0],
        },
        Seg::Arc {
            center: [lb, 0.0],
            radius: hc / 2.0,
            start: -PI / 2.0,
            sweep: PI,
        },
        Seg::Line {
            a: [lb, hc / 2.0],
            b: [0.0, hc / 2.0],
        },
        Seg::Line {
            a: [0.0, hc / 2.0],
            b: [0.0, -hc / 2.0],
        },
    ];
    let body = ext(
        xz([0.0, w / 2.0, 0.0]),
        vec![lp, circle([lb, 0.0], rp, false)],
        w,
    );
    let g = w * r.range(0.35, 0.5);
    let x0 = lb * r.range(0.3, 0.5);
    let slot = Primitive::Box {
        frame: Frame::at([x0, -g / 2.0, -hc]),
        size: [lb + hc, g, 2.0 * hc],
    };
    let slot_v = g * ((lb - x0) * hc + PI * hc * hc / 8.0 - PI * rp * rp);
    let rb = hc.min(w) * r.range(0.3, 0.42);
    let lboss = r.range(5.0, 10.0);
    let rt = rb * r.range(0.4, 0.6);
    let dt = lboss * r.range(0.5, 0.9);
    Part::new(body)
        .cut(slot, slot_v)
        .join(cyl([-lboss, 0.0, 0.0], X, rb, lboss), PI * rb * rb * lboss)
        .cut(
            cyl([-lboss - 1.0, 0.0, 0.0], X, rt, dt + 1.0),
            PI * rt * rt * dt,
        )
        .done()
}

// ---- Housings ----

/// An enclosure: a rounded box shelled open at the top, screw bosses standing on its floor, holes
/// in the bosses, a port through one wall.
pub(super) fn enclosure(r: &mut Rng) -> Drawn {
    let (w, d, h) = (
        r.range(40.0, 70.0),
        r.range(30.0, 50.0),
        r.range(15.0, 30.0),
    );
    let t = r.range(1.5, 3.0);
    let rb = r.range(2.0, 3.5);
    let rho = (t * r.range(1.5, 4.0)).min(d / 2.0 - t - 2.0 * rb - 2.0);
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64, c: f64| {
        rounded(&[[x0, y0], [x1, y0], [x1, y1], [x0, y1]], &[c; 4])
    };
    let outer = rect(0.0, 0.0, w, d, rho);
    let inner = rect(t, t, w - t, d - t, rho - t);
    let pocket = ext(Frame::at([0.0, 0.0, t]), vec![inner.clone()], h);
    let shell_v = area(&inner) * (h - t);
    let hb = (h - t) * r.range(0.4, 0.7);
    let m = t + rho + rb + 1.0;
    let at = [[m, m], [w - m, m], [w - m, d - m], [m, d - m]];
    let bosses = union_of(at.iter().map(|p| cyl([p[0], p[1], t], Z, rb, hb)).collect());
    let rs = rb * r.range(0.35, 0.5);
    let ds = hb * r.range(0.5, 0.8);
    let screw_holes = union_of(
        at.iter()
            .map(|p| cyl([p[0], p[1], t + hb - ds], Z, rs, ds + 1.0))
            .collect(),
    );
    let (pw, ph) = (
        (w - 2.0 * m) * r.range(0.2, 0.4),
        (h - t) * r.range(0.2, 0.4),
    );
    let pz = t + (h - t - ph) / 2.0 + (h - t) * 0.1;
    let port = Primitive::Box {
        frame: Frame::at([w / 2.0 - pw / 2.0, -1.0, pz]),
        size: [pw, t + 1.5, ph],
    };
    let shell = Feature {
        act: Act::Shell,
        points: vec![[w / 2.0, d / 2.0, h]],
        size: t,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let mut part = Part::new(ext(Frame::at([0.0; 3]), vec![outer], h));
    part.steps.push(Step::Act(shell));
    part.want = Shape::bool(Op::Difference, part.want, pocket);
    part.volume -= shell_v;
    part.join(bosses, 4.0 * PI * rb * rb * hb)
        .cut(screw_holes, 4.0 * PI * rs * rs * ds)
        .cut(port, pw * ph * t)
        .done()
}

/// A gearbox housing: a block hollowed open at the top, bearing bosses on two opposite walls,
/// two bores through bosses and walls, tapped holes in the top of the walls.
pub(super) fn gearbox_housing(r: &mut Rng) -> Drawn {
    let (lx, wy, hz) = (
        r.range(60.0, 90.0),
        r.range(35.0, 50.0),
        r.range(40.0, 60.0),
    );
    let tw = r.range(4.0, 6.0);
    let tb = tw * r.range(1.0, 1.5);
    let cavity = Primitive::Box {
        frame: Frame::at([tw, tw, tb]),
        size: [lx - 2.0 * tw, wy - 2.0 * tw, hz],
    };
    let cav_v = (lx - 2.0 * tw) * (wy - 2.0 * tw) * (hz - tb);
    let rbore = r.range(5.0, 8.0);
    let rboss = rbore * r.range(1.5, 1.8);
    let lboss = r.range(3.0, 6.0);
    let zc = tb + rbore + (hz - tb - rbore - rboss) * r.range(0.3, 0.6);
    let xs = [lx * 0.25, lx * 0.75];
    let mut bosses = Vec::new();
    for &x in &xs {
        bosses.push(cyl([x, -lboss, zc], Y, rboss, lboss));
        bosses.push(cyl([x, wy, zc], Y, rboss, lboss));
    }
    let bores = union_of(
        xs.iter()
            .map(|&x| cyl([x, -lboss - 1.0, zc], Y, rbore, wy + 2.0 * lboss + 2.0))
            .collect(),
    );
    let rt = tw * 0.25;
    let dt = r.range(5.0, 10.0);
    let taps = union_of(
        [
            [tw / 2.0, wy / 2.0],
            [lx - tw / 2.0, wy / 2.0],
            [lx / 2.0, tw / 2.0],
        ]
        .iter()
        .map(|p| cyl([p[0], p[1], hz - dt], Z, rt, dt + 1.0))
        .collect(),
    );
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [lx, wy, hz],
    })
    .cut(cavity, cav_v)
    .join(union_of(bosses), 4.0 * PI * rboss * rboss * lboss)
    .cut(bores, 2.0 * PI * rbore * rbore * (2.0 * tw + 2.0 * lboss))
    .cut(taps, 3.0 * PI * rt * rt * dt)
    .done()
}

/// A pillow block: feet and an arched pedestal round a bearing bore, the pedestal's roots
/// filleted, bolt holes through the feet, a grease hole down through the arch.
pub(super) fn pillow_block(r: &mut Rng) -> Drawn {
    let rbore = r.range(5.0, 10.0);
    let rh = rbore * r.range(1.5, 1.8);
    let tb = r.range(4.0, 7.0);
    let hc = tb + rh * r.range(0.6, 1.0);
    let wb = rh * r.range(4.0, 5.0);
    let lx = rh * r.range(1.0, 1.5);
    let lp = vec![
        Seg::Line {
            a: [-wb / 2.0, 0.0],
            b: [wb / 2.0, 0.0],
        },
        Seg::Line {
            a: [wb / 2.0, 0.0],
            b: [wb / 2.0, tb],
        },
        Seg::Line {
            a: [wb / 2.0, tb],
            b: [rh, tb],
        },
        Seg::Line {
            a: [rh, tb],
            b: [rh, hc],
        },
        Seg::Arc {
            center: [0.0, hc],
            radius: rh,
            start: 0.0,
            sweep: PI,
        },
        Seg::Line {
            a: [-rh, hc],
            b: [-rh, tb],
        },
        Seg::Line {
            a: [-rh, tb],
            b: [-wb / 2.0, tb],
        },
        Seg::Line {
            a: [-wb / 2.0, tb],
            b: [-wb / 2.0, 0.0],
        },
    ];
    let body = ext(yz([0.0; 3]), vec![lp, circle([0.0, hc], rbore, false)], lx);
    let f = (hc - tb).min(wb / 2.0 - rh) * r.range(0.15, 0.3);
    let roots = Shape::bool(
        Op::Union,
        strip(yz([0.0; 3]), rh, tb, f, 1.0, 1.0, lx),
        strip(yz([0.0; 3]), -rh, tb, f, -1.0, 1.0, lx),
    );
    let yh = (rh + f + wb / 2.0) / 2.0;
    let rbolt = ((wb / 2.0 - rh - f) / 2.0 * 0.6).min(lx * 0.3);
    let bolts = union_of(vec![
        cyl([lx / 2.0, yh, -1.0], Z, rbolt, tb + 2.0),
        cyl([lx / 2.0, -yh, -1.0], Z, rbolt, tb + 2.0),
    ]);
    let a = rbore * r.range(0.15, 0.25);
    let grease = quad(
        |th| {
            let y = a * th.psin();
            2.0 * a
                * th.pcos()
                * ((rh * rh - y * y).sqrt() - (rbore * rbore - y * y).sqrt())
                * a
                * th.pcos()
        },
        -PI / 2.0,
        PI / 2.0,
    );
    Part::new(body)
        .blend(
            Act::Fillet,
            vec![[lx / 2.0, rh, tb], [lx / 2.0, -rh, tb]],
            f,
            roots,
            true,
        )
        .cut(bolts, 2.0 * PI * rbolt * rbolt * tb)
        .cut(cyl([lx / 2.0, 0.0, hc], Z, a, rh + 1.0), grease)
        .done()
}

/// A motor mount: a face plate standing on a base, its inside corner filleted, a pilot bore and
/// four screw holes through the face, slots through the base.
pub(super) fn motor_mount(r: &mut Rng) -> Drawn {
    let s = r.range(30.0, 45.0);
    let (tp, tb) = (r.range(3.0, 6.0), r.range(3.0, 6.0));
    let lb = s * r.range(1.0, 1.4);
    let hp = s * r.range(1.0, 1.2);
    let face = Primitive::Box {
        frame: Frame::at([0.0, 0.0, tb]),
        size: [tp, s, hp],
    };
    let f = tp.min(tb) * r.range(0.5, 1.0);
    let zc = tb + hp / 2.0;
    let rp = s * r.range(0.25, 0.32);
    let sp = s * 0.35;
    let rs = s * 0.04;
    let screws = union_of(
        [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .iter()
            .map(|q| cyl([-1.0, s / 2.0 + q[0] * sp, zc + q[1] * sp], X, rs, tp + 2.0))
            .collect(),
    );
    let x0 = tp + f + 2.0;
    let sl = (lb - x0) * 0.4;
    let rsl = s * 0.06;
    let slots = union_of(
        [s * 0.25, s * 0.75]
            .iter()
            .map(|&y| {
                ext(
                    Frame::at([0.0, 0.0, -1.0]),
                    vec![stadium([x0 + rsl + 1.0, y], [x0 + rsl + 1.0 + sl, y], rsl)],
                    tb + 2.0,
                )
            })
            .collect(),
    );
    let slot_a = 2.0 * rsl * sl + PI * rsl * rsl;
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [lb, s, tb],
    })
    .join(face, tp * s * hp)
    .blend(
        Act::Fillet,
        vec![[tp, s / 2.0, tb]],
        f,
        strip(xz([0.0, s, 0.0]), tp, tb, f, 1.0, 1.0, s),
        true,
    )
    .cut(cyl([-1.0, s / 2.0, zc], X, rp, tp + 2.0), PI * rp * rp * tp)
    .cut(screws, 4.0 * PI * rs * rs * tp)
    .cut(slots, 2.0 * slot_a * tb)
    .done()
}

/// A valve body: a block with bosses on three faces, a bore straight through two of them and a
/// branch bore from the third ending at the first's axis.
pub(super) fn valve_body(r: &mut Rng) -> Drawn {
    let (lx, wy, hz) = (
        r.range(30.0, 45.0),
        r.range(25.0, 35.0),
        r.range(25.0, 35.0),
    );
    let r1 = wy.min(hz) * r.range(0.15, 0.22);
    let r2 = r1 * r.range(0.5, 0.9);
    let rboss = r1 * r.range(1.5, 1.8);
    let lboss = r.range(4.0, 8.0);
    let (yc, zc) = (wy / 2.0, hz / 2.0);
    let xb = lx / 2.0;
    let bosses = union_of(vec![
        cyl([-lboss, yc, zc], X, rboss, lboss),
        cyl([lx, yc, zc], X, rboss, lboss),
        cyl([xb, -lboss, zc], Y, rboss, lboss),
    ]);
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [lx, wy, hz],
    })
    .join(bosses, 3.0 * PI * rboss * rboss * lboss)
    .cut(
        cyl([-lboss - 1.0, yc, zc], X, r1, lx + 2.0 * lboss + 2.0),
        PI * r1 * r1 * (lx + 2.0 * lboss),
    )
    .cut(
        cyl([xb, -lboss - 1.0, zc], Y, r2, lboss + 1.0 + yc),
        PI * r2 * r2 * (lboss + yc) - crossing(r1, r2) / 2.0,
    )
    .done()
}

// ---- Molded and machined parts ----

/// A molded plate with a drafted boss and a drafted rib standing on it, the boss cored.
pub(super) fn drafted_boss_and_rib(r: &mut Rng) -> Drawn {
    let (w, d, t) = (r.range(40.0, 60.0), r.range(30.0, 45.0), r.range(2.0, 3.5));
    let draft = r.range(1.0, 3.0).to_radians().ptan();
    let (rb, hb) = (r.range(4.0, 7.0), r.range(8.0, 15.0));
    let top = rb - hb * draft;
    let boss = Primitive::Cone {
        frame: Frame::at([w * 0.3, d / 2.0, t]),
        bottom: rb,
        top,
        height: hb,
    };
    let (wb, hr) = (t * r.range(0.8, 1.2), r.range(6.0, 12.0));
    let wt = wb - 2.0 * hr * draft;
    let lr = d * r.range(0.5, 0.8);
    let xr = w * 0.7;
    let rib = ext(
        xz([0.0, d / 2.0 + lr / 2.0, 0.0]),
        vec![poly(&[
            [xr - wb / 2.0, t],
            [xr + wb / 2.0, t],
            [xr + wt / 2.0, t + hr],
            [xr - wt / 2.0, t + hr],
        ])],
        lr,
    );
    let rc = top * r.range(0.4, 0.6);
    let dc = hb * r.range(0.5, 0.8);
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [w, d, t],
    })
    .join(boss.clone(), boss.volume())
    .join(rib, (wb + wt) / 2.0 * hr * lr)
    .cut(
        cyl([w * 0.3, d / 2.0, t + hb - dc], Z, rc, dc + 1.0),
        PI * rc * rc * dc,
    )
    .done()
}

/// A snap-fit clip: a cantilever with a ramped hook standing off a block, its root filleted.
pub(super) fn snap_fit(r: &mut Rng) -> Drawn {
    let (bx, wy, bz) = (r.range(8.0, 14.0), r.range(10.0, 20.0), r.range(10.0, 16.0));
    let lc = r.range(12.0, 22.0);
    let tc = r.range(1.2, 2.2);
    let hk = tc * r.range(0.8, 1.2);
    let lh = lc * r.range(0.15, 0.25);
    let lr = lh * r.range(0.4, 0.7);
    let wc = wy * r.range(0.4, 0.7);
    let z0 = bz * r.range(0.3, 0.5);
    let y0 = (wy - wc) / 2.0;
    let beam_lp = poly(&[
        [0.0, z0],
        [lc, z0],
        [lc, z0 + tc + 0.3 * hk],
        [lc - lr, z0 + tc + hk],
        [lc - lh, z0 + tc + hk],
        [lc - lh, z0 + tc],
        [0.0, z0 + tc],
    ]);
    let beam_v = area(&beam_lp) * wc;
    let beam = ext(xz([0.0, y0 + wc, 0.0]), vec![beam_lp], wc);
    let f = tc * r.range(0.3, 0.6);
    Part::new(Primitive::Box {
        frame: Frame::at([-bx, 0.0, 0.0]),
        size: [bx, wy, bz],
    })
    .join(beam, beam_v)
    .blend(
        Act::Fillet,
        vec![[0.0, wy / 2.0, z0]],
        f,
        strip(xz([0.0, y0 + wc, 0.0]), 0.0, z0, f, 1.0, -1.0, wc),
        true,
    )
    .done()
}

/// A 28 mm screw cap as a soft-drink bottle takes, its sizes typical rather than a standard's:
/// - the shell turned, its top panel's outer edge rounded 1 and its inside corner filleted 0.5;
/// - a straight knurl of 90 flat-topped teeth round the skirt;
/// - the internal thread, 2.7 pitch, 1.05 deep, its crest flat, running inside the skirt;
/// - a bore seal hanging from the top panel to plug the neck, its tip rounded;
/// - the tamper band below a 0.5 slit, held by 8 bridges, its bead turned in to catch under the
///   neck's ring.
///
/// The z axis runs up from the skirt's bottom at 0; the band hangs below it.
pub(super) fn bottle_cap() -> Drawn {
    let (ri, ro, h, tp) = (13.85, 14.75, 11.6, 1.2);
    let (round, fillet) = (1.0, 0.5);
    let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
    let under = h - tp;
    let shell = rev(
        [0.0; 3],
        vec![
            line([0.0, under], [ri - fillet, under]),
            // Concave: the corner between the panel's underside and the skirt's inside filled.
            Seg::Arc {
                center: [ri - fillet, under - fillet],
                radius: fillet,
                start: PI / 2.0,
                sweep: -PI / 2.0,
            },
            line([ri, under - fillet], [ri, 0.0]),
            line([ri, 0.0], [ro, 0.0]),
            line([ro, 0.0], [ro, h - round]),
            Seg::Arc {
                center: [ro - round, h - round],
                radius: round,
                start: 0.0,
                sweep: PI / 2.0,
            },
            line([ro - round, h], [0.0, h]),
            line([0.0, h], [0.0, under]),
        ],
    );
    // The knurl on the skirt's straight outside, clear of the round at the top.
    let (k0, k1) = (1.0, h - round - 0.4);
    let outline = fasteners::knurl_outline(ro, ro + 0.3, 90);
    let knurl_v = (area(&outline) - PI * ro * ro) * (k1 - k0);
    let knurl = ext(
        Frame::at([0.0, 0.0, k0]),
        vec![outline, circle([0.0, 0.0], ro - 0.15, false)],
        k1 - k0,
    );
    let th = fasteners::Thread {
        p: 2.7,
        crest: ri - 1.05,
        root: ri,
        embed: 0.25,
        external: false,
    };
    let (t0, t1) = (2.0, 8.8);
    let thread_v = (t1 - t0) * th.slice(th.crest, ri);
    // The bore seal: a ring 0.8 thick from 0.2 into the panel down 2, its tip a half round.
    let (s0, s1) = (10.7, 11.5);
    let seal = rev(
        [0.0; 3],
        vec![
            line([s0, under + 0.2], [s0, under - 1.8]),
            Seg::Arc {
                center: [(s0 + s1) / 2.0, under - 1.8],
                radius: (s1 - s0) / 2.0,
                start: PI,
                sweep: PI,
            },
            line([s1, under - 1.8], [s1, under + 0.2]),
            line([s1, under + 0.2], [s0, under + 0.2]),
        ],
    );
    let seal_v = seal.volume() - PI * (s1 * s1 - s0 * s0) * 0.2;
    // The band 0.7 thick from 0.5 to 3.3 below, its bead 0.9 in over its lower half.
    let (b0, b1) = (13.95, 14.65);
    let band = rev(
        [0.0; 3],
        poly(&ordered(vec![
            [b0, -3.3],
            [b1, -3.3],
            [b1, -0.5],
            [b0, -0.5],
            [b0, -1.7],
            [b0 - 0.9, -2.4],
            [b0, -2.9],
        ])),
    );
    let band_v = band.volume();
    // Bridges across the slit, 0.1 into the skirt and the band, their corners inside both rings.
    let (n, w, (r0, r1)) = (8, 0.8, (14.0, 14.4));
    let bridges = union_of(
        (0..n)
            .map(|i| {
                let a = TAU * (i as f64 + 0.5) / n as f64;
                let (c, s) = (a.pcos(), a.psin());
                let o = rot([r0, -w / 2.0], a);
                Primitive::Box {
                    frame: frame([o[0], o[1], -0.6], [c, s, 0.0], [-s, c, 0.0]),
                    size: [r1 - r0, w, 0.7],
                }
            })
            .collect(),
    );
    let bridges_v = n as f64 * w * (r1 - r0) * 0.5;
    Part::new(shell)
        .join(knurl, knurl_v)
        .join(th.ridge_between(t0, t1), thread_v)
        .join(seal, seal_v)
        .join(band, band_v)
        .join(bridges, bridges_v)
        .done()
}

/// A pocketed block: rounded pockets milled into the top, a through hole in each pocket's floor.
pub(super) fn pocketed_block(r: &mut Rng) -> Drawn {
    let (w, d, h) = (
        r.range(50.0, 80.0),
        r.range(30.0, 50.0),
        r.range(12.0, 20.0),
    );
    let n = count(r, 2, 3);
    let gap = r.range(4.0, 6.0);
    let pw = (w - gap * (n as f64 + 1.0)) / n as f64;
    let pd = d - 2.0 * gap;
    let rc = pw.min(pd) * r.range(0.1, 0.25);
    let dp = h * r.range(0.3, 0.6);
    let mut pockets = Vec::new();
    let mut holes = Vec::new();
    let rh = pw.min(pd) * 0.15;
    for i in 0..n {
        let x0 = gap + i as f64 * (pw + gap);
        pockets.push(ext(
            Frame::at([0.0, 0.0, h - dp]),
            vec![rounded(
                &[
                    [x0, gap],
                    [x0 + pw, gap],
                    [x0 + pw, gap + pd],
                    [x0, gap + pd],
                ],
                &[rc; 4],
            )],
            dp + 1.0,
        ));
        holes.push(cyl([x0 + pw / 2.0, d / 2.0, -1.0], Z, rh, h + 2.0));
    }
    let pa = pw * pd - (4.0 - PI) * rc * rc;
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [w, d, h],
    })
    .cut(union_of(pockets), n as f64 * pa * dp)
    .cut(union_of(holes), n as f64 * PI * rh * rh * (h - dp))
    .done()
}

/// A T-slot table: T-slots milled along a plate, counterbored holes at its corners.
pub(super) fn t_slot_plate(r: &mut Rng) -> Drawn {
    let (l, w, t) = (
        r.range(60.0, 100.0),
        r.range(40.0, 70.0),
        r.range(12.0, 20.0),
    );
    let n = count(r, 2, 4);
    let wn = r.range(5.0, 8.0);
    let wu = wn * r.range(1.6, 2.0);
    let (hn, hu) = (t * r.range(0.3, 0.4), t * r.range(0.25, 0.35));
    let zb = t - hn - hu;
    let section = |yc: f64| {
        poly(&[
            [yc - wu / 2.0, zb],
            [yc + wu / 2.0, zb],
            [yc + wu / 2.0, zb + hu],
            [yc + wn / 2.0, zb + hu],
            [yc + wn / 2.0, t + 1.0],
            [yc - wn / 2.0, t + 1.0],
            [yc - wn / 2.0, zb + hu],
            [yc - wu / 2.0, zb + hu],
        ])
    };
    let margin = 11.5 + wu / 2.0;
    let n = n
        .min(((w - 2.0 * margin) / (wu + 4.0)).floor() as usize + 1)
        .max(1);
    let spacing = (w - 2.0 * margin) / (n as f64 - 1.0).max(1.0);
    let slots = union_of(
        (0..n)
            .map(|i| {
                let yc = margin + spacing * i as f64;
                ext(yz([-1.0, 0.0, 0.0]), vec![section(yc)], l + 2.0)
            })
            .collect(),
    );
    let (rh, rcb, dc) = (2.5, 4.5, t * 0.3);
    let corners = [
        [5.0, 5.0],
        [l - 5.0, 5.0],
        [l - 5.0, w - 5.0],
        [5.0, w - 5.0],
    ];
    let holes = union_of(
        corners
            .iter()
            .map(|p| cyl([p[0], p[1], -1.0], Z, rh, t + 2.0))
            .collect(),
    );
    let bores = union_of(
        corners
            .iter()
            .map(|p| cyl([p[0], p[1], t - dc], Z, rcb, dc + 1.0))
            .collect(),
    );
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [l, w, t],
    })
    .cut(slots, n as f64 * l * (wu * hu + wn * hn))
    .cut(holes, 4.0 * PI * rh * rh * t)
    .cut(bores, 4.0 * PI * (rcb * rcb - rh * rh) * dc)
    .done()
}

/// A dovetail slide: a dovetail groove broached along a block, tapped holes beside it.
pub(super) fn dovetail_slide(r: &mut Rng) -> Drawn {
    let (l, w, t) = (
        r.range(50.0, 80.0),
        r.range(30.0, 50.0),
        r.range(12.0, 20.0),
    );
    let dd = t * r.range(0.3, 0.45);
    let wt = w * r.range(0.25, 0.35);
    let wb = wt + 2.0 * dd / 60f64.to_radians().ptan();
    let slope = (wb - wt) / (2.0 * dd);
    let e = 1.0;
    let yc = w / 2.0;
    let groove = ext(
        yz([-1.0, 0.0, 0.0]),
        vec![poly(&[
            [yc - wb / 2.0, t - dd],
            [yc + wb / 2.0, t - dd],
            [yc + wt / 2.0 - e * slope, t + e],
            [yc - wt / 2.0 + e * slope, t + e],
        ])],
        l + 2.0,
    );
    let rt = 1.5;
    let dt = t * 0.5;
    let yt = (yc + wb / 2.0 + w) / 2.0;
    let taps = union_of(
        [l * 0.25, l * 0.75]
            .iter()
            .flat_map(|&x| {
                [
                    cyl([x, yt, t - dt], Z, rt, dt + 1.0),
                    cyl([x, w - yt, t - dt], Z, rt, dt + 1.0),
                ]
            })
            .collect(),
    );
    Part::new(Primitive::Box {
        frame: Frame::at([0.0; 3]),
        size: [l, w, t],
    })
    .cut(groove, l * (wb + wt) / 2.0 * dd)
    .cut(taps, 4.0 * PI * rt * rt * dt)
    .done()
}

/// A plate cam: a smooth spline outline with one lobe, a hub on it, a keyed bore through both.
pub(super) fn plate_cam(r: &mut Rng) -> Drawn {
    let rb = r.range(12.0, 20.0);
    let lift = rb * r.range(0.25, 0.5);
    let n = 16;
    let spread = r.range(0.6, 1.0);
    let mut ctrl: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            let bump = (-(a - PI).ppowi(2) / (2.0 * spread * spread)).pexp();
            let rad = rb + lift * bump;
            [rad * a.pcos(), rad * a.psin()]
        })
        .collect();
    ctrl.extend_from_within(..3);
    let outline = vec![Seg::Spline(Spline::new(
        3,
        (0..n + 7).map(|k| k as f64).collect(),
        ctrl,
    ))];
    let t = r.range(5.0, 10.0);
    let hub_r = rb * r.range(0.4, 0.55);
    let hub_h = r.range(4.0, 8.0);
    let bore_r = hub_r * r.range(0.4, 0.55);
    let bore = keyed_bore(bore_r, bore_r * 0.25, bore_r * 0.25);
    let body = ext(Frame::at([0.0; 3]), vec![outline], t);
    Part::new(body)
        .join(
            cyl([0.0, 0.0, t], Z, hub_r, hub_h),
            PI * hub_r * hub_r * hub_h,
        )
        .cut(
            ext(
                Frame::at([0.0, 0.0, -1.0]),
                vec![bore.clone()],
                t + hub_h + 2.0,
            ),
            area(&bore) * (t + hub_h),
        )
        .done()
}

// ---- Structural sections ----

/// An I-beam: a rolled section with root radii, holes through its web and its top flange.
pub(super) fn i_beam(r: &mut Rng) -> Drawn {
    let d = r.range(40.0, 80.0);
    let bf = d * r.range(0.5, 0.7);
    let tf = d * r.range(0.07, 0.1);
    let tw = tf * r.range(0.55, 0.7);
    let rr = tw * r.range(1.0, 1.6);
    let len = r.range(60.0, 120.0);
    let pts = [
        [-bf / 2.0, 0.0],
        [bf / 2.0, 0.0],
        [bf / 2.0, tf],
        [tw / 2.0, tf],
        [tw / 2.0, d - tf],
        [bf / 2.0, d - tf],
        [bf / 2.0, d],
        [-bf / 2.0, d],
        [-bf / 2.0, d - tf],
        [-tw / 2.0, d - tf],
        [-tw / 2.0, tf],
        [-bf / 2.0, tf],
    ];
    let radii = [0.0, 0.0, 0.0, rr, rr, 0.0, 0.0, 0.0, 0.0, rr, rr, 0.0];
    let section = rounded(&pts, &radii);
    let body = ext(Frame::at([0.0; 3]), vec![section], len);
    let rh = (d - 2.0 * tf - 2.0 * rr) * r.range(0.15, 0.3);
    let web = union_of(
        [len * 0.25, len * 0.75]
            .iter()
            .map(|&z| cyl([-tw / 2.0 - 1.0, d / 2.0, z], X, rh, tw + 2.0))
            .collect(),
    );
    let rf = ((bf / 2.0 - tw / 2.0 - rr) / 2.0 * 0.6).min(tf * 1.5);
    let xf = (tw / 2.0 + rr + bf / 2.0) / 2.0;
    let flange = union_of(
        [-xf, xf]
            .iter()
            .map(|&x| cyl([x, d - tf - 1.0, len * 0.5], Y, rf, tf + 2.0))
            .collect(),
    );
    Part::new(body)
        .cut(web, 2.0 * PI * rh * rh * tw)
        .cut(flange, 2.0 * PI * rf * rf * tf)
        .done()
}

/// A channel: a rolled C section with root radii, holes through its web.
pub(super) fn channel(r: &mut Rng) -> Drawn {
    let d = r.range(40.0, 70.0);
    let bf = d * r.range(0.35, 0.5);
    let tf = d * r.range(0.07, 0.1);
    let tw = tf * r.range(0.6, 0.8);
    let rr = tw * r.range(1.0, 1.5);
    let len = r.range(60.0, 100.0);
    let pts = [
        [0.0, 0.0],
        [bf, 0.0],
        [bf, tf],
        [tw, tf],
        [tw, d - tf],
        [bf, d - tf],
        [bf, d],
        [0.0, d],
    ];
    let section = rounded(&pts, &[0.0, 0.0, 0.0, rr, rr, 0.0, 0.0, 0.0]);
    let rh = (d - 2.0 * tf - 2.0 * rr) * r.range(0.12, 0.25);
    let n = count(r, 2, 4);
    let holes = union_of(
        (0..n)
            .map(|i| {
                let z = len * (i as f64 + 0.5) / n as f64;
                cyl([-1.0, d / 2.0, z], X, rh, tw + 2.0)
            })
            .collect(),
    );
    Part::new(ext(Frame::at([0.0; 3]), vec![section], len))
        .cut(holes, n as f64 * PI * rh * rh * tw)
        .done()
}

/// An equal angle: a rolled L section with a root radius and rounded toes, holes in both legs.
pub(super) fn angle_section(r: &mut Rng) -> Drawn {
    let a = r.range(30.0, 60.0);
    let t = a * r.range(0.08, 0.12);
    let rr = t * r.range(1.0, 1.5);
    let toe = t * r.range(0.3, 0.5);
    let len = r.range(50.0, 100.0);
    let pts = [[0.0, 0.0], [a, 0.0], [a, t], [t, t], [t, a], [0.0, a]];
    let section = rounded(&pts, &[0.0, 0.0, toe, rr, toe, 0.0]);
    let rh = (a - t - rr - toe) * r.range(0.12, 0.2);
    let c = (t + rr + a - toe) / 2.0;
    let holes = union_of(vec![
        cyl([c, -1.0, len * 0.3], Y, rh, t + 2.0),
        cyl([-1.0, c, len * 0.7], X, rh, t + 2.0),
    ]);
    Part::new(ext(Frame::at([0.0; 3]), vec![section], len))
        .cut(holes, 2.0 * PI * rh * rh * t)
        .done()
}

/// A flanged tube: a tube with a flange on each end, the tube's roots filleted, bolt circles
/// through both flanges.
pub(super) fn flanged_tube(r: &mut Rng) -> Drawn {
    let ri = r.range(5.0, 12.0);
    let ro = ri * r.range(1.15, 1.3);
    let len = r.range(30.0, 60.0);
    let tf = (ro - ri) * r.range(1.5, 2.5);
    let rf = ro * r.range(2.0, 2.6);
    let f = (ro - ri) * r.range(0.4, 0.8);
    let tube = rev(
        [0.0; 3],
        poly(&[[ri, 0.0], [ro, 0.0], [ro, len], [ri, len]]),
    );
    let flange = |z: f64| {
        rev(
            [0.0; 3],
            poly(&[[ri, z], [rf, z], [rf, z + tf], [ri, z + tf]]),
        )
    };
    let fv = PI * (rf * rf - ri * ri) * tf;
    let roots = Shape::bool(
        Op::Union,
        ring(true, ro, 0.0, f, 1.0, 1.0),
        ring(true, ro, len, f, 1.0, -1.0),
    );
    let n = count(r, 4, 8);
    let rc = (ro + f + rf) / 2.0;
    let rb = ((rf - ro - f) / 2.0 * 0.6).min(rc * (PI / n as f64).psin() * 0.6);
    let holes = Shape::bool(
        Op::Union,
        bolt_circle(n, rc, rb, -tf - 1.0, tf + 1.5, 0.0),
        bolt_circle(n, rc, rb, len - 0.5, tf + 1.5, 0.0),
    );
    Part::new(tube)
        .join(flange(-tf), fv)
        .join(flange(len), fv)
        .blend(
            Act::Fillet,
            vec![[ro, 0.0, 0.0], [-ro, 0.0, len]],
            f,
            roots,
            true,
        )
        .cut(holes, 2.0 * n as f64 * PI * rb * rb * tf)
        .done()
}

#[cfg(test)]
mod tests {
    use super::*;

    type Draw = fn(&mut Rng) -> Drawn;

    const PARTS: [(&str, Draw); 38] = [
        ("spur", |_| spur_gear(18, 1.5, 8.0, 0)),
        ("lightened spur", |_| spur_gear(60, 1.0, 6.0, 6)),
        ("helical", |_| helical_gear(24, 1.5, 10.0, 15.0, 1.0)),
        ("internal", |_| internal_gear(48, 1.0, 8.0, 8)),
        ("rack", |_| rack(10, 2.0, 12.0)),
        ("timing pulley", timing_pulley),
        ("sprocket", sprocket),
        ("splined shaft", splined_shaft),
        ("lock washer", lock_washer),
        ("heat-set insert m3", |_| {
            fasteners::heat_set_insert(fasteners::M3)
        }),
        ("heat-set insert m5", |_| {
            fasteners::heat_set_insert(fasteners::M5)
        }),
        ("spring", compression_spring),
        ("stepped shaft", stepped_shaft),
        ("pipe flange", pipe_flange),
        ("v pulley", v_pulley),
        ("windowed wheel", windowed_wheel),
        ("turbine disc", turbine_disc),
        ("l bracket", l_bracket),
        ("mounting plate", mounting_plate),
        ("gusset bracket", gusset_bracket),
        ("bent bracket", bent_bracket),
        ("clevis", clevis),
        ("enclosure", enclosure),
        ("gearbox housing", gearbox_housing),
        ("pillow block", pillow_block),
        ("motor mount", motor_mount),
        ("valve body", valve_body),
        ("drafted boss and rib", drafted_boss_and_rib),
        ("snap fit", snap_fit),
        ("bottle cap", |_| bottle_cap()),
        ("pocketed block", pocketed_block),
        ("t-slot plate", t_slot_plate),
        ("dovetail slide", dovetail_slide),
        ("plate cam", plate_cam),
        ("i-beam", i_beam),
        ("channel", channel),
        ("angle", angle_section),
        ("flanged tube", flanged_tube),
    ];

    #[test]
    fn every_part_s_closed_form_meets_a_sample_of_its_shape() {
        let mut r = Rng::new(7);
        for (name, draw) in PARTS {
            for _ in 0..2 {
                let (_, want, exact) = draw(&mut r);
                let v = exact.result_volume.unwrap();
                let (lo, hi) = crate::oracle::bounds(&want).unwrap();
                let n = 200_000;
                let held = crate::oracle::samples((lo, hi), n)
                    .iter()
                    .filter(|p| crate::oracle::contains(&want, **p))
                    .count();
                let got = held as f64 / n as f64 * (0..3).map(|i| hi[i] - lo[i]).product::<f64>();
                assert!((got - v).abs() < 0.015 * v, "{name}: {got} against {v}");
            }
        }
    }

    #[test]
    fn gears_in_mesh_share_no_point() {
        for (z1, z2, bl) in [(18, 40, 0.08), (20, 20, 0.05), (17, 60, 0.1)] {
            let (a, b, _) = meshing_pair(z1, z2, 1.5, bl, 6.0);
            let (
                Shape::Prim(Primitive::Extrude { profile: pa, .. }),
                Shape::Prim(Primitive::Extrude { profile: pb, .. }),
            ) = (&a, &b)
            else {
                unreachable!()
            };
            let (lo, hi) = pb.bounds();
            let (alo, ahi) = pa.bounds();
            let (x0, x1) = (lo[0].max(alo[0]), hi[0].min(ahi[0]));
            let (y0, y1) = (lo[1].max(alo[1]), hi[1].min(ahi[1]));
            let n = 600;
            for i in 0..n {
                for j in 0..n {
                    let q = [
                        x0 + (x1 - x0) * i as f64 / n as f64,
                        y0 + (y1 - y0) * j as f64 / n as f64,
                    ];
                    assert!(!(pa.contains(q) && pb.contains(q)), "both hold {q:?}");
                }
            }
        }
    }

    #[test]
    fn a_crossing_of_equal_cylinders_is_steinmetz() {
        let r: f64 = 1.7;
        assert!((crossing(r, r) - 16.0 / 3.0 * r.ppowi(3)).abs() < 1e-12);
    }

    #[test]
    fn a_hexagon_chamfered_from_its_inradius_loses_its_corners_outside_the_cone() {
        // From a disc that holds the hexagon, the cone takes nothing below the flats' level only
        // at the corners: check against a direct sum of thin slices.
        let (d, rho0, tan_a) = (2.0, 1.8, (PI / 6.0).ptan());
        let rc = 2.0 * d / 3f64.sqrt();
        let a_hex = 2.0 * 3f64.sqrt() * d * d;
        let slices = 200_000;
        let dr = (rc - rho0) / slices as f64;
        let mut sum = 0.0;
        for i in 0..slices {
            let rr = rho0 + dr * (i as f64 + 0.5);
            let inside = if rr <= d {
                PI * rr * rr
            } else {
                PI * rr * rr - 6.0 * (rr * rr * (d / rr).pacos() - d * (rr * rr - d * d).sqrt())
            };
            sum += (a_hex - inside) * dr;
        }
        assert!((hex_chamfer(d, rho0, tan_a) - tan_a * sum).abs() < 1e-8);
    }
}
