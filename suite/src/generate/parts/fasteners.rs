//! Common metric fasteners: threads of the 60° basic profile at coarse and fine pitches, on hex
//! nuts, thin nuts, socket head cap screws, hex head screws and plain washers. They are shapes
//! for testing a kernel, and claim conformity to no standard.
//!
//! Nominal dimensions follow the common metric fastener standards (ISO 4032, 4035, 4762, 4017 and
//! 7089; threads ISO 68-1 and 261).
//!
//! Every cut a fastener takes about its axis (a countersink, a point's chamfer, the runout where
//! a thread leaves the shank) keeps an exact volume. A thread's ridge is the same at every height
//! turned, so each of its slices covers, at radius ρ, the fraction `w(ρ)/P` of the circle there,
//! `w` its profile's width along the axis at ρ and `P` its pitch. A slice through the ridge and
//! anything turned about the axis is a sum of annuli and such fractions, closed in form; the
//! volume is the slices' integral over height.

use super::*;
use crate::portable::Portable;

/// A thread of the metric basic profile: 60° flanks, the crest flat `P/8` wide on the major
/// diameter of an external thread and `P/4` on the minor of an internal one. Its ridge reaches
/// `embed` into the material it grows from, so a join of the two overlaps there.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Thread {
    pub p: f64,
    /// The radius of the crest, and of the root on the material's side.
    pub crest: f64,
    pub root: f64,
    pub embed: f64,
    pub external: bool,
}

/// `2/√3`: how fast a 60° thread's width changes with radius.
fn flank() -> f64 {
    2.0 / 3f64.sqrt()
}

impl Thread {
    /// The thread of nominal diameter `d` and pitch `p`, on a bolt or in a nut.
    pub fn metric(d: f64, p: f64, external: bool) -> Thread {
        // The basic profile's depth from major to minor: 5H/8, H = (√3/2)P.
        let depth = 5.0 / 8.0 * 3f64.sqrt() / 2.0 * p;
        let (major, minor) = (d / 2.0, d / 2.0 - depth);
        let (crest, root) = if external {
            (major, minor)
        } else {
            (minor, major)
        };
        Thread {
            p,
            crest,
            root,
            embed: 0.1 * p,
            external,
        }
    }

    /// The width along the axis of the ridge at radius `rho`.
    pub fn width(&self, rho: f64) -> f64 {
        let at_crest = if self.external {
            self.p / 8.0
        } else {
            self.p / 4.0
        };
        at_crest + (rho - self.crest).abs() * flank()
    }

    /// The radius of the ridge's base, `embed` into its material from the root.
    fn base(&self) -> f64 {
        if self.external {
            self.root - self.embed
        } else {
            self.root + self.embed
        }
    }

    /// The area of a slice of the ridge between radii `a` and `b`: `∫ 2πρ·w(ρ)/P dρ`.
    pub fn slice(&self, a: f64, b: f64) -> f64 {
        let (lo, hi) = (self.base().min(self.crest), self.base().max(self.crest));
        let (a, b) = (a.max(lo), b.min(hi));
        if b <= a {
            return 0.0;
        }
        // w = α + βρ on the ridge.
        let beta = if self.external { -flank() } else { flank() };
        let alpha = self.width(self.crest) - beta * self.crest;
        2.0 * PI / self.p * (alpha * (b * b - a * a) / 2.0 + beta * (b.ppowi(3) - a.ppowi(3)) / 3.0)
    }

    /// The ridge from a pitch below `lo` to a pitch above `hi`: a screw of the profile about z,
    /// one pitch a turn, so a cut at either height finds it whole.
    fn ridge(&self, lo: f64, hi: f64) -> Primitive {
        let (from, to) = (lo - self.p, hi + self.p);
        let base = self.base();
        let (wb, wc) = (self.width(base), self.width(self.crest));
        let pts = ordered(vec![
            [base, from - wb / 2.0],
            [self.crest, from - wc / 2.0],
            [self.crest, from + wc / 2.0],
            [base, from + wb / 2.0],
        ]);
        Primitive::Screw {
            frame: Frame::at([0.0; 3]),
            profile: Profile {
                loops: vec![poly(&pts)],
            },
            rise: self.p / TAU,
            angle: TAU * (to - from) / self.p,
        }
    }

    /// The ridge between heights `lo` and `hi`, cut square there.
    pub fn ridge_between(&self, lo: f64, hi: f64) -> Shape {
        let reach = self.base().max(self.crest) + 1.0;
        Shape::bool(
            Op::Intersection,
            self.ridge(lo, hi),
            cyl([0.0, 0.0, lo], Z, reach, hi - lo),
        )
    }
}

/// `∫ f` over [a, b], split at each of `cuts` inside it, where `f` has a corner.
fn quad_at(f: impl Fn(f64) -> f64, a: f64, b: f64, cuts: &[f64]) -> f64 {
    let mut ts = vec![a];
    ts.extend(cuts.iter().copied().filter(|&c| c > a && c < b));
    ts.push(b);
    ts.sort_by(f64::total_cmp);
    ts.windows(2).map(|w| quad(&f, w[0], w[1])).sum()
}

/// A metric size: nominal diameter and pitch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Size {
    pub d: f64,
    pub p: f64,
}

/// Metric coarse pitches.
pub(crate) const M3: Size = Size { d: 3.0, p: 0.5 };
pub(crate) const M5: Size = Size { d: 5.0, p: 0.8 };
pub(crate) const M6: Size = Size { d: 6.0, p: 1.0 };
pub(crate) const M8: Size = Size { d: 8.0, p: 1.25 };
pub(crate) const M10: Size = Size { d: 10.0, p: 1.5 };
pub(crate) const M12: Size = Size { d: 12.0, p: 1.75 };
/// A metric fine pitch.
pub(crate) const M10_FINE: Size = Size { d: 10.0, p: 1.25 };

/// A hex nut's width across flats `s` and height `m` by nominal diameter.
pub(crate) fn nut_dims(d: f64) -> (f64, f64) {
    match d as u32 {
        3 => (5.5, 2.4),
        5 => (8.0, 4.7),
        6 => (10.0, 5.2),
        8 => (13.0, 6.8),
        10 => (16.0, 8.4),
        _ => (18.0, 10.8),
    }
}

/// A hex nut `m` tall: the hexagon bored at the thread's major diameter with the internal thread
/// joined all through, both faces' corners chamfered at 30° from the bearing circle `0.9s`, and
/// both ends of the bore countersunk at 120° to `1.08d`.
pub(crate) fn hex_nut(size: Size, m: f64) -> Drawn {
    let (s, _) = nut_dims(size.d);
    let th = Thread::metric(size.d, size.p, false);
    let (a, rc) = (s / 2.0, s / 3f64.sqrt());
    let bore = th.root;
    let body = ext(
        Frame::at([0.0; 3]),
        vec![poly(&hexagon(rc)), circle([0.0, 0.0], bore, false)],
        m,
    );
    let tan30 = (PI / 6.0).ptan();
    let rho0 = 0.45 * s;
    let chamfer = hex_chamfer(a, rho0, tan30);
    let r0 = 1.08 * size.d / 2.0;
    let sunk = countersunk(&th, r0);
    let mut part = Part::new(body).join(th.ridge_between(0.0, m), m * th.slice(th.crest, bore));
    for (z, up) in [(m, true), (0.0, false)] {
        part = part
            .cut(chamfer_tool(z, rho0, rc, tan30, up), chamfer)
            .cut(sink_tool(r0, z, up), sunk);
    }
    part.done()
}

/// `tan 60°`: a 120° countersink's radius shrinks by it for each unit of depth.
fn sink_slope() -> f64 {
    (PI / 3.0).ptan()
}

/// What a 120° countersink of radius `r0` at the face takes from a bore at the root of internal
/// thread `th`: at depth δ its radius is `r0 − δ·tan 60°`, inside which it takes the annulus of
/// wall outside the bore and the ridge's slice inside the bore.
fn countersunk(th: &Thread, r0: f64) -> f64 {
    let t = sink_slope();
    let bore = th.root;
    let rc = |dep: f64| r0 - dep * t;
    let (d1, d2) = ((r0 - bore) / t, (r0 - th.crest) / t);
    let wall = quad(|dep| PI * (rc(dep).ppowi(2) - bore * bore), 0.0, d1);
    let ridge = quad_at(|dep| th.slice(th.crest, rc(dep).min(bore)), 0.0, d2, &[d1]);
    wall + ridge
}

/// The countersink's cone at a face at height `z`, the material below the face when `up`.
fn sink_tool(r0: f64, z: f64, up: bool) -> Primitive {
    let (t, s, lift) = (sink_slope(), if up { 1.0 } else { -1.0 }, 1.0);
    rev(
        [0.0; 3],
        poly(&ordered(vec![
            [0.0, z + s * lift],
            [r0 + lift * t, z + s * lift],
            [0.0, z - s * r0 / t],
        ])),
    )
}

/// A threaded shank about z from the underside of a head at 0 to its end at `len`: a concave
/// fillet `rf` under the head, the plain shank at the major radius to `top`, the core falling to
/// the minor radius over the runout below `top`, and a 45° point from radius `tip` at the end.
/// The shank's meridian as a loop about the axis, and its ridge cut to the point's cone, with the
/// volume the ridge adds outside the core inside that cone.
fn threaded_shank(
    th: &Thread,
    top: f64,
    len: f64,
    runout: f64,
    tip: f64,
    rf: f64,
) -> (Primitive, Shape, f64) {
    let (major, minor) = (th.crest, th.root);
    let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
    let loop_ = vec![
        line([0.0, 0.0], [major + rf, 0.0]),
        // Concave: from under the head round to the shank, the material outside the arc.
        Seg::Arc {
            center: [major + rf, rf],
            radius: rf,
            start: -PI / 2.0,
            sweep: -PI / 2.0,
        },
        line([major, rf], [major, top]),
        line([major, top], [minor, top + runout]),
        line([minor, top + runout], [minor, len - (minor - tip)]),
        line([minor, len - (minor - tip)], [tip, len]),
        line([tip, len], [0.0, len]),
        line([0.0, len], [0.0, 0.0]),
    ];
    let shank = Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: Profile { loops: vec![loop_] },
        angle: TAU,
    };
    let core = |z: f64| {
        if z < top + runout {
            major - (major - minor) * (z - top) / runout
        } else {
            minor
        }
    };
    let cone = |z: f64| tip + (len - z);
    let added = quad_at(
        |z| th.slice(core(z), major.min(cone(z))),
        top,
        len,
        &[top + runout, len - (major - tip), len - (minor - tip)],
    );
    let point = rev(
        [0.0; 3],
        poly(&ordered(vec![
            [0.0, top - 1.0],
            [major + 1.0, top - 1.0],
            [major + 1.0, len - (major + 1.0 - tip)],
            [tip, len],
            [0.0, len],
        ])),
    );
    let ridge = Shape::bool(Op::Intersection, th.ridge_between(top, len), point);
    (shank, ridge, added)
}

/// A socket head cap screw's head diameter `dk`, head height `k`, key `s`, key depth `t`, the radius under the
/// head `r` and the reference thread length `b`, by nominal diameter.
fn socket_dims(d: f64) -> (f64, f64, f64, f64, f64, f64) {
    match d as u32 {
        3 => (5.5, 3.0, 2.5, 1.3, 0.1, 18.0),
        5 => (8.5, 5.0, 4.0, 2.5, 0.2, 22.0),
        6 => (10.0, 6.0, 5.0, 3.0, 0.25, 24.0),
        8 => (13.0, 8.0, 6.0, 4.0, 0.4, 28.0),
        10 => (16.0, 10.0, 8.0, 5.0, 0.4, 32.0),
        _ => (18.0, 12.0, 10.0, 6.0, 0.6, 36.0),
    }
}

/// The thread's runout, its point's radius at the end face, and where the plain shank ends on a
/// screw `len` long threaded `b`. The first full thread lies `len − b` under the head, the most
/// the grip length may be, and no farther than `a = 3P` on a screw threaded to the head; the
/// runout of 2.5 pitches ends there.
fn thread_layout(size: Size, th: &Thread, len: f64, b: f64) -> (f64, f64, f64) {
    let runout = 2.5 * size.p;
    let full = (len - b).max(3.0 * size.p);
    (runout, th.root - 0.2 * size.p, full - runout)
}

/// A socket head cap screw `len` long under the head: the round head with its top edge chamfered
/// 45°, a hexagonal socket with a 118° drill point at its floor, the fillet under the head, a plain
/// shank, the thread's runout over 2.5 pitches ending at the first full thread `len − b` under the
/// head, and a 45° point. A screw shorter than its thread length has its first full thread 3P
/// under the head.
pub(crate) fn socket_head_screw(size: Size, len: f64) -> Drawn {
    let (dk, k, s, t, rf, b) = socket_dims(size.d);
    let th = Thread::metric(size.d, size.p, true);
    let (runout, tip, top) = thread_layout(size, &th, len, b);
    let (shank, ridge, added) = threaded_shank(&th, top, len, runout, tip, rf);
    let (r, c) = (dk / 2.0, 0.06 * dk);
    let head = rev(
        [0.0; 3],
        poly(&ordered(vec![
            [0.0, -k],
            [r - c, -k],
            [r, -k + c],
            [r, 0.0],
            [0.0, 0.0],
        ])),
    );
    let socket = ext(
        Frame::at([0.0, 0.0, -k - 1.0]),
        vec![poly(&hexagon(s / 3f64.sqrt()))],
        t + 1.0,
    );
    let point_h = s / 2.0 / 59f64.to_radians().ptan();
    let point = Primitive::Cone {
        frame: Frame::at([0.0, 0.0, -k + t]),
        bottom: s / 2.0,
        top: 0.0,
        height: point_h,
    };
    let shank_v = Primitive::volume(&shank);
    Part::new(head)
        .join(shank, shank_v)
        .join(ridge, added)
        .cut(socket, 3f64.sqrt() / 2.0 * s * s * t)
        .cut(point, PI / 3.0 * (s / 2.0).ppowi(2) * point_h)
        .done()
}

/// A hex head screw's width across flats `s`, head height `k`, washer face diameter `dw` and its height
/// `c`, by nominal diameter.
fn hex_head_dims(d: f64) -> (f64, f64, f64, f64) {
    match d as u32 {
        3 => (5.5, 2.0, 4.6, 0.4),
        5 => (8.0, 3.5, 6.9, 0.5),
        6 => (10.0, 4.0, 8.9, 0.5),
        8 => (13.0, 5.3, 11.6, 0.6),
        10 => (16.0, 6.4, 14.6, 0.6),
        _ => (18.0, 7.5, 16.6, 0.6),
    }
}

/// A hex head screw threaded to the head, `len` long under it: the hexagon's top corners
/// chamfered at 30° from the bearing circle `0.9s`, the round washer face under the head, the
/// fillet under that, and the thread from its runout under the head to a 45° point.
pub(crate) fn hex_head_screw(size: Size, len: f64) -> Drawn {
    let (s, k, dw, c) = hex_head_dims(size.d);
    let th = Thread::metric(size.d, size.p, true);
    let (runout, tip, top) = thread_layout(size, &th, len, len);
    let (shank, ridge, added) = threaded_shank(&th, top, len, runout, tip, 0.1 * size.d);
    let (a, rc) = (s / 2.0, s / 3f64.sqrt());
    let head = ext(Frame::at([0.0, 0.0, -k]), vec![poly(&hexagon(rc))], k - c);
    let face = cyl([0.0, 0.0, -c], Z, dw / 2.0, c);
    let tan30 = (PI / 6.0).ptan();
    let rho0 = 0.45 * s;
    let shank_v = Primitive::volume(&shank);
    Part::new(head)
        .cut(
            chamfer_tool(-k, rho0, rc, tan30, false),
            hex_chamfer(a, rho0, tan30),
        )
        .join(face, PI * (dw / 2.0).ppowi(2) * c)
        .join(shank, shank_v)
        .join(ridge, added)
        .done()
}

/// A plain washer's inner and outer diameters and thickness by nominal diameter.
fn washer_dims(d: f64) -> (f64, f64, f64) {
    match d as u32 {
        3 => (3.2, 7.0, 0.5),
        5 => (5.3, 10.0, 1.0),
        6 => (6.4, 12.0, 1.6),
        8 => (8.4, 16.0, 1.6),
        10 => (10.5, 20.0, 2.0),
        _ => (13.0, 24.0, 2.5),
    }
}

/// A plain washer: a flat ring, its edges square.
pub(crate) fn plain_washer(size: Size) -> Drawn {
    let (d1, d2, h) = washer_dims(size.d);
    let (ri, ro) = (d1 / 2.0, d2 / 2.0);
    let body = ext(
        Frame::at([0.0; 3]),
        vec![circle([0.0, 0.0], ro, true), circle([0.0, 0.0], ri, false)],
        h,
    );
    Part::new(body).done()
}

/// A knurl's outline round a body of radius `rv`: `n` flat-topped teeth out to `peak`, each on
/// half its pitch between arcs of the body's own circle, so the outline is the disc with the teeth
/// added and its area less the disc's is the teeth's alone.
pub(crate) fn knurl_outline(rv: f64, peak: f64, n: usize) -> Vec<Seg> {
    let step = TAU / n as f64;
    let at = |r: f64, a: f64| [r * a.pcos(), r * a.psin()];
    let mut out = Vec::with_capacity(4 * n);
    for k in 0..n {
        let a = step * k as f64;
        out.push(Seg::Arc {
            center: [0.0, 0.0],
            radius: rv,
            start: a,
            sweep: step / 2.0,
        });
        let pts = [
            at(rv, a + 0.5 * step),
            at(peak, a + 0.625 * step),
            at(peak, a + 0.875 * step),
            at(rv, a + step),
        ];
        out.extend(pts.windows(2).map(|w| Seg::Line { a: w[0], b: w[1] }));
    }
    out
}

/// A band of knurl from `z0` up `h`, its teeth turning `twist` over the band, as a twisted
/// extrusion: every slice is the outline turned, so it adds its teeth's area times `h` to a body
/// of radius `rv`. It reaches 0.15 into the body.
pub(crate) fn knurl_band(
    rv: f64,
    peak: f64,
    n: usize,
    z0: f64,
    h: f64,
    twist: f64,
) -> (Primitive, f64) {
    let outline = knurl_outline(rv, peak, n);
    let added = (area(&outline) - PI * rv * rv) * h;
    let band = Primitive::SweepPath {
        frame: Frame::at([0.0, 0.0, z0]),
        profile: Profile {
            loops: vec![outline, circle([0.0, 0.0], rv - 0.15, false)],
        },
        path: PathCurve::new(
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            (0..4).map(|i| [0.0, 0.0, h * i as f64 / 3.0]).collect(),
        ),
        twist,
    };
    (band, added)
}

/// A heat-set insert's sizes by thread: its length, the knurl's peak and body radii, and the
/// radii of the pilot below the knurl and the groove between its bands. Typical of the inserts
/// sold for printed parts; there is no standard.
fn insert_dims(d: f64) -> (f64, f64, f64, f64, f64) {
    match d as u32 {
        3 => (5.7, 2.3, 2.05, 1.95, 1.9),
        _ => (9.5, 3.5, 3.2, 3.05, 2.95),
    }
}

/// A heat-set insert: a turned sleeve bored at the thread's major diameter, a plain pilot below
/// chamfered at its end, two bands of 30° knurl of opposite hand with a groove between them, a
/// plain collar above chamfered at its top, and the internal thread all through, its bore
/// countersunk at 120° at both ends.
pub(crate) fn heat_set_insert(size: Size) -> Drawn {
    let (len, peak, rv, rp, rg) = insert_dims(size.d);
    let th = Thread::metric(size.d, size.p, false);
    let bore = th.root;
    // Proportioned on the M3's 5.7: the pilot its first 1, a groove 0.6 wide between the bands,
    // the collar its last 0.6, the edges chamfered 0.15, short of the countersinks.
    let u = len / 5.7;
    let (c1, c2) = (0.15 * u, 0.15 * u);
    let (zp, zg0, zg1, zc) = (1.0 * u, 2.55 * u, 3.15 * u, 5.1 * u);
    let sleeve = rev(
        [0.0; 3],
        poly(&ordered(vec![
            [bore, 0.0],
            [rp - c1, 0.0],
            [rp, c1],
            [rp, zp],
            [rv, zp],
            [rv, zg0],
            [rg, zg0],
            [rg, zg1],
            [rv, zg1],
            [rv, len - c2],
            [rv - c2, len],
            [bore, len],
        ])),
    );
    let n = 18;
    let tan30 = (PI / 6.0).ptan();
    let (lower, lower_v) = knurl_band(rv, peak, n, zp, zg0 - zp, (zg0 - zp) * tan30 / rv);
    let (upper, upper_v) = knurl_band(rv, peak, n, zg1, zc - zg1, -(zc - zg1) * tan30 / rv);
    let r0 = 1.08 * size.d / 2.0;
    let sunk = countersunk(&th, r0);
    let mut part = Part::new(sleeve)
        .join(lower, lower_v)
        .join(upper, upper_v)
        .join(th.ridge_between(0.0, len), len * th.slice(th.crest, bore));
    for (z, up) in [(len, true), (0.0, false)] {
        part = part.cut(sink_tool(r0, z, up), sunk);
    }
    part.done()
}

/// A shank and a thread's ridge, as a screw's runout joins them: a plain cylinder at the major
/// radius, then a cone down to the minor over 2.5 pitches, then the minor cylinder. The ridge's
/// crest lies on the plain cylinder, and the ridge runs from `start` pitches above the
/// cylinder's edge with the cone (negative: below it, on the plain shank) to 4 pitches past the
/// cone. Where they overlap, each height's slice is the ridge's from its base to the shank's
/// radius there.
pub(crate) fn runout_join(size: Size, start: f64) -> Drawn {
    let th = Thread::metric(size.d, size.p, true);
    let (major, minor, p) = (th.crest, th.root, size.p);
    let (top, runout) = (6.0 * p, 2.5 * p);
    let len = top + runout + 6.0 * p;
    let shank = rev(
        [0.0; 3],
        poly(&ordered(vec![
            [0.0, 0.0],
            [major, 0.0],
            [major, top],
            [minor, top + runout],
            [minor, len],
            [0.0, len],
        ])),
    );
    let (lo, hi) = (top + start * p, top + runout + 4.0 * p);
    let core = |z: f64| major - (major - minor) * ((z - top) / runout).clamp(0.0, 1.0);
    let overlap = quad_at(|z| th.slice(0.0, core(z)), lo, hi, &[top, top + runout]);
    let exact = Exact {
        intersection_volume: Some(overlap),
        ..Default::default()
    };
    (shank.into(), th.ridge_between(lo, hi), exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sample's estimate of a shape's volume within the box `lo..hi`, and its standard error.
    fn sampled(want: &Shape, lo: Vec3, hi: Vec3, n: usize) -> (f64, f64) {
        let box_v: f64 = (0..3).map(|i| hi[i] - lo[i]).product();
        let held = crate::oracle::samples((lo, hi), n)
            .iter()
            .filter(|p| crate::oracle::contains(want, **p))
            .count() as f64;
        let f = held / n as f64;
        (f * box_v, box_v * (f * (1.0 - f) / n as f64).sqrt())
    }

    #[test]
    fn a_thread_s_slice_covers_its_width_over_its_pitch() {
        // Across the whole ridge an external slice is the profile's area times 2π times its
        // centroid's radius over the pitch (Pappus over one pitch of height).
        let th = Thread::metric(10.0, 1.5, true);
        let (lo, hi) = (th.base(), th.crest);
        let n = 20000;
        let dr = (hi - lo) / n as f64;
        let (mut area, mut moment) = (0.0, 0.0);
        for i in 0..n {
            let r = lo + dr * (i as f64 + 0.5);
            area += th.width(r) * dr;
            moment += r * th.width(r) * dr;
        }
        let want = 2.0 * PI * moment / th.p;
        assert!((th.slice(lo, hi) - want).abs() < 1e-9 * want);
        assert!(area > 0.0);
    }

    #[test]
    fn every_fastener_s_closed_form_meets_a_sample_of_its_thread() {
        // Sampled in the band the threads and cuts lie in, where the volume is most sensitive:
        // within the major radius plus a margin, along the whole part.
        let parts: Vec<(&str, Drawn)> = vec![
            ("nut M6", hex_nut(M6, nut_dims(6.0).1)),
            ("thin nut M10 fine", hex_nut(M10_FINE, 5.0)),
            ("socket screw M5x12", socket_head_screw(M5, 12.0)),
            ("socket screw M8x40", socket_head_screw(M8, 40.0)),
            ("hex screw M6x16", hex_head_screw(M6, 16.0)),
            ("washer M8", plain_washer(M8)),
        ];
        for (name, (_, want, exact)) in parts {
            let v = exact.result_volume.unwrap();
            let (lo, hi) = crate::oracle::bounds(&want).unwrap();
            let (got, se) = sampled(&want, lo, hi, 2_000_000);
            assert!(
                (got - v).abs() < 5.0 * se + 1e-9 * v,
                "{name}: {got} ± {se} against {v}"
            );
        }
    }

    #[test]
    fn a_runout_s_overlap_with_its_ridge_meets_a_sample() {
        for (size, start) in [(M8, 0.0), (M8, -1.0), (M12, 0.0), (M12, 0.4)] {
            let (a, b, exact) = runout_join(size, start);
            let v = exact.intersection_volume.unwrap();
            let both = Shape::bool(Op::Intersection, a, b);
            let (lo, hi) = crate::oracle::bounds(&both).unwrap();
            let (got, se) = sampled(&both, lo, hi, 2_000_000);
            assert!(
                (got - v).abs() < 5.0 * se,
                "M{} from {start}: {got} ± {se} against {v}",
                size.d
            );
        }
    }
}
