//! Sweeps of a moving solid whose results are shapes the suite already holds exactly: a solid slid
//! along a direction in which it is a prism, turned about an axis round which it is a piece of a
//! body of revolution, or screwed along its own thread.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use brep_protocol::Motion;

use super::{Act, Exact, Feature, FeatureDraw, Rng, axis_box, polygon};
use crate::model::{Frame, Op, Primitive, Shape, Vec3};
use crate::portable::Portable;
use crate::profile::{Profile, Seg, circle};

const Z: Vec3 = [0.0, 0.0, 1.0];

fn swept(motion: Motion) -> Feature {
    Feature {
        act: Act::SweepSolid,
        points: Vec::new(),
        size: 0.0,
        pull: None,
        radii: Vec::new(),
        matrix: None,
        motion: Some(motion),
    }
}

fn draw(base: Primitive, motion: Motion, want: Primitive) -> FeatureDraw {
    (base, swept(motion), want.into(), Exact::default())
}

fn sign(r: &mut Rng) -> f64 {
    if r.next_u64().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// The world's frame turned by `phi` about z and raised by `lift`.
fn turned(phi: f64, lift: f64) -> Frame {
    let (s, c) = phi.psin_cos();
    Frame {
        origin: [0.0, 0.0, lift],
        x: [c, s, 0.0],
        y: [-s, c, 0.0],
        z: Z,
    }
}

fn rotated(p: [f64; 2], phi: f64) -> [f64; 2] {
    let (s, c) = phi.psin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}

fn line(a: [f64; 2], b: [f64; 2]) -> Seg {
    Seg::Line { a, b }
}

fn arc(center: [f64; 2], radius: f64, start: f64, sweep: f64) -> Seg {
    Seg::Arc {
        center,
        radius,
        start,
        sweep,
    }
}

/// The ball of `radius` about `c` carried by `v`: the capsule, turned about its axis.
fn capsule(c: Vec3, v: Vec3, radius: f64) -> Primitive {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let meridian = vec![
        arc([0.0, 0.0], radius, -FRAC_PI_2, FRAC_PI_2),
        line([radius, 0.0], [radius, len]),
        arc([0.0, len], radius, 0.0, FRAC_PI_2),
        line([0.0, len + radius], [0.0, -radius]),
    ];
    Primitive::Revolve {
        frame: Frame::along(c, v.map(|x| x / len)),
        profile: Profile {
            loops: vec![meridian],
        },
        angle: TAU,
    }
}

/// The frustum on the world's z axis from the origin, carried `d` along z: at each height the
/// widest of the sections that pass it.
fn slid_frustum(bottom: f64, top: f64, h: f64, d: f64) -> Primitive {
    let (lo, hi) = (d.min(0.0), d.max(0.0));
    let mut pts = if bottom > top {
        vec![
            [0.0, lo],
            [bottom, lo],
            [bottom, hi],
            [top, h + hi],
            [0.0, h + hi],
        ]
    } else {
        vec![
            [0.0, lo],
            [bottom, lo],
            [top, h + lo],
            [top, h + hi],
            [0.0, h + hi],
        ]
    };
    pts.dedup();
    if pts.first() == pts.last() {
        pts.pop();
    }
    Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&pts),
        angle: TAU,
    }
}

fn cylinder(lo: f64, radius: f64, height: f64) -> Primitive {
    Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, lo]),
        radius,
        height,
    }
}

/// A solid of revolution about the world's z axis and what it becomes carried `d` along that axis:
/// a cylinder, a frustum or a ball.
fn carried_along_its_axis(r: &mut Rng, d: f64) -> (Primitive, Primitive) {
    match r.next_u64() % 3 {
        0 => {
            let (rad, h) = (r.range(1.0, 4.0), r.range(1.0, 6.0));
            (
                cylinder(0.0, rad, h),
                cylinder(d.min(0.0), rad, h + d.abs()),
            )
        }
        1 => {
            let (wide, h) = (r.range(2.0, 4.0), r.range(2.0, 6.0));
            let narrow = if r.next_u64().is_multiple_of(3) {
                0.0
            } else {
                wide * r.range(0.2, 0.7)
            };
            let (bottom, top) = if r.next_u64().is_multiple_of(2) {
                (wide, narrow)
            } else {
                (narrow, wide)
            };
            let base = Primitive::Cone {
                frame: Frame::at([0.0; 3]),
                bottom,
                top,
                height: h,
            };
            (base, slid_frustum(bottom, top, h, d))
        }
        _ => {
            let rad = r.range(1.0, 4.0);
            let base = Primitive::Sphere {
                frame: Frame::at([0.0; 3]),
                radius: rad,
            };
            (base, capsule([0.0; 3], [0.0, 0.0, d], rad))
        }
    }
}

/// A slide across the axis of the rod of `rad` and `h` on the world's z axis, and the prism on a
/// stadium the rod sweeps by it.
fn slid_rod(r: &mut Rng, rad: f64, h: f64) -> (Vec3, Primitive) {
    let phi = r.range(0.0, TAU);
    let len = r.range(1.0, 8.0);
    let (s, c) = phi.psin_cos();
    let (u, n) = ([c, s], [-s, c]);
    let far = [len * u[0], len * u[1]];
    let stadium = vec![
        line(
            [-rad * n[0], -rad * n[1]],
            [far[0] - rad * n[0], far[1] - rad * n[1]],
        ),
        arc(far, rad, phi - FRAC_PI_2, PI),
        line(
            [far[0] + rad * n[0], far[1] + rad * n[1]],
            [rad * n[0], rad * n[1]],
        ),
        arc([0.0, 0.0], rad, phi + FRAC_PI_2, PI),
    ];
    let want = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![stadium],
        },
        height: h,
    };
    ([far[0], far[1], 0.0], want)
}

fn cross(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// The convex hull of the points, counter-clockwise, by Andrew's monotone chain.
fn hull(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    pts.sort_by(|a, b| a.partial_cmp(b).expect("finite points"));
    let chain = |pts: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut out: Vec<[f64; 2]> = Vec::new();
        for p in pts {
            while out.len() >= 2 && cross(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
        out
    };
    let mut out = chain(&mut pts.iter().copied());
    out.extend(chain(&mut pts.iter().rev().copied()));
    out
}

/// A block carried along one of its edges or across one of its faces, a rod along or across its
/// axis, a frustum along its axis, or a ball any way: each a prism or a body of revolution still.
pub fn slides(r: &mut Rng) -> FeatureDraw {
    let block = [r.range(2.0, 8.0), r.range(2.0, 8.0), r.range(2.0, 8.0)];
    match r.next_u64() % 6 {
        0 => {
            let axis = (r.next_u64() % 3) as usize;
            let d = sign(r) * r.range(1.0, 6.0);
            let mut vector = [0.0; 3];
            vector[axis] = d;
            let (mut lo, mut hi) = ([0.0; 3], block);
            lo[axis] = d.min(0.0);
            hi[axis] += d.max(0.0);
            draw(
                axis_box([0.0; 3], block),
                Motion::Slide { vector },
                axis_box(lo, hi),
            )
        }
        1 => {
            // The frame's axes are the world's in a cyclic order, so any face may be slid in.
            let k = (r.next_u64() % 3) as usize;
            let (i, j) = ((k + 1) % 3, (k + 2) % 3);
            let v = [sign(r) * r.range(1.0, 6.0), sign(r) * r.range(1.0, 6.0)];
            let mut vector = [0.0; 3];
            (vector[i], vector[j]) = (v[0], v[1]);
            let corners = [
                [0.0, 0.0],
                [block[i], 0.0],
                [block[i], block[j]],
                [0.0, block[j]],
            ];
            let both = corners
                .iter()
                .flat_map(|c| [*c, [c[0] + v[0], c[1] + v[1]]])
                .collect();
            let axis = |n: usize| {
                let mut e = [0.0; 3];
                e[n] = 1.0;
                e
            };
            let want = Primitive::Extrude {
                frame: Frame {
                    origin: [0.0; 3],
                    x: axis(i),
                    y: axis(j),
                    z: axis(k),
                },
                profile: polygon(&hull(both)),
                height: block[k],
            };
            draw(axis_box([0.0; 3], block), Motion::Slide { vector }, want)
        }
        2 => {
            let (rad, h) = (r.range(1.0, 4.0), r.range(1.0, 6.0));
            let (vector, want) = slid_rod(r, rad, h);
            draw(cylinder(0.0, rad, h), Motion::Slide { vector }, want)
        }
        3 => {
            let rad = r.range(1.0, 4.0);
            let v = r.direction().map(|x| x * rad * 3.0);
            let len = r.range(0.3, 1.0);
            let vector = v.map(|x| x * len);
            let base = Primitive::Sphere {
                frame: Frame::at([0.0; 3]),
                radius: rad,
            };
            draw(
                base,
                Motion::Slide { vector },
                capsule([0.0; 3], vector, rad),
            )
        }
        _ => {
            let d = sign(r) * r.range(1.0, 6.0);
            let (base, want) = carried_along_its_axis(r, d);
            let vector = [0.0, 0.0, d];
            draw(base, Motion::Slide { vector }, want)
        }
    }
}

/// A meridian section clear of the axis: a rectangle, a disc or a triangle.
fn section(r: &mut Rng, tallest: f64) -> Profile {
    let inner = r.range(1.0, 4.0);
    let wide = r.range(1.0, 3.0);
    let tall = tallest.min(r.range(1.0, 4.0));
    let low = r.range(-2.0, 2.0);
    match r.next_u64() % 3 {
        0 => polygon(&[
            [inner, low],
            [inner + wide, low],
            [inner + wide, low + tall],
            [inner, low + tall],
        ]),
        1 => {
            let rad = 0.5 * wide.min(tall);
            Profile {
                loops: vec![circle([inner + rad, low + rad], rad, true)],
            }
        }
        _ => polygon(&[[inner, low], [inner + wide, low], [inner, low + tall]]),
    }
}

/// A turn about the world's z axis by `angle`, its axis given at any point of it and any length.
fn turn(r: &mut Rng, angle: f64) -> Motion {
    Motion::Turn {
        origin: [0.0, 0.0, r.range(-5.0, 5.0)],
        axis: [0.0, 0.0, r.range(0.5, 3.0)],
        angle,
    }
}

fn whole_turn(r: &mut Rng) -> Motion {
    let angle = sign(r) * TAU;
    turn(r, angle)
}

/// The block over `x0 ≤ x ≤ x1`, `|y| ≤ w/2` turned by `theta` about the world's z: its outer
/// corners sweep one arc, the middle of its near side another, and its sides close the ends.
fn turned_block(x0: f64, x1: f64, w: f64, h: f64, theta: f64) -> Primitive {
    let t = theta.abs();
    let half = 0.5 * w;
    let outer = x1.phypot(half);
    let alpha = half.patan2(x1);
    let outline = vec![
        line([x0, -half], [x1, -half]),
        arc([0.0, 0.0], outer, -alpha, t + 2.0 * alpha),
        line(rotated([x1, half], t), rotated([x0, half], t)),
        line(rotated([x0, half], t), rotated([x0, 0.0], t)),
        arc([0.0, 0.0], x0, t, -t),
        line([x0, 0.0], [x0, -half]),
    ];
    Primitive::Extrude {
        frame: turned(theta.min(0.0), 0.0),
        profile: Profile {
            loops: vec![outline],
        },
        height: h,
    }
}

/// The ball of radius `rad` about a point `major` from the world's z axis, turned by `theta`:
/// the ring's piece between the ball where it starts and the ball where it ends.
fn turned_ball(major: f64, rad: f64, height: f64, theta: f64) -> (Shape, Exact) {
    let ball = |phi: f64| {
        let (s, c) = phi.psin_cos();
        Primitive::Sphere {
            frame: Frame::at([major * c, major * s, height]),
            radius: rad,
        }
    };
    let ring = Primitive::Revolve {
        frame: turned(theta.min(0.0), 0.0),
        profile: Profile {
            loops: vec![circle([major, height], rad, true)],
        },
        angle: theta.abs(),
    };
    let ends = Shape::bool(Op::Union, ball(0.0), ball(theta));
    let exact = Exact {
        result_volume: Some(PI * rad * rad * major * theta.abs() + 4.0 / 3.0 * PI * rad.ppowi(3)),
        result_area: Some(TAU * rad * major * theta.abs() + 4.0 * PI * rad * rad),
        ..Exact::default()
    };
    (Shape::bool(Op::Union, ring, ends), exact)
}

/// A piece of a ring turned further round or all the way, a ball turned into a ring or a piece of
/// one, and a block turned about an axis along its edges: through it, beside it, or part way.
pub fn turns(r: &mut Rng) -> FeatureDraw {
    let h = r.range(1.0, 6.0);
    match r.next_u64() % 7 {
        0 | 1 => {
            let whole = r.next_u64().is_multiple_of(3);
            let profile = section(r, f64::INFINITY);
            let had = r.range(0.3, 1.5);
            let theta = sign(r) * if whole { TAU } else { r.range(0.4, 4.0) };
            let base = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile: profile.clone(),
                angle: had,
            };
            let want = Primitive::Revolve {
                frame: turned(if whole { 0.0 } else { theta.min(0.0) }, 0.0),
                profile,
                angle: if whole { TAU } else { had + theta.abs() },
            };
            draw(base, turn(r, theta), want)
        }
        2 => {
            let major = r.range(3.0, 8.0);
            let rad = major * r.range(0.15, 0.5);
            let base = Primitive::Sphere {
                frame: Frame::at([major, 0.0, h]),
                radius: rad,
            };
            let want = Primitive::Torus {
                frame: Frame::at([0.0, 0.0, h]),
                major,
                minor: rad,
            };
            draw(base, whole_turn(r), want)
        }
        3 => {
            let major = r.range(3.0, 8.0);
            let rad = major * r.range(0.15, 0.5);
            let theta = sign(r) * r.range(0.8, 4.5);
            let base = Primitive::Sphere {
                frame: Frame::at([major, 0.0, h]),
                radius: rad,
            };
            let (want, exact) = turned_ball(major, rad, h, theta);
            (base, swept(turn(r, theta)), want, exact)
        }
        4 => {
            let lo = [-r.range(0.5, 4.0), -r.range(0.5, 4.0), 0.0];
            let hi = [r.range(0.5, 4.0), r.range(0.5, 4.0), h];
            let reach = lo[0].abs().max(hi[0]).phypot(lo[1].abs().max(hi[1]));
            draw(axis_box(lo, hi), whole_turn(r), cylinder(0.0, reach, h))
        }
        5 => {
            let x0 = r.range(0.5, 4.0);
            let x1 = x0 + r.range(1.0, 4.0);
            let y0 = r.range(-4.0, 2.0);
            let y1 = y0 + r.range(1.0, 4.0);
            let nearest = x0.phypot(0.0_f64.clamp(y0, y1));
            let reach = x1.phypot(y0.abs().max(y1.abs()));
            let mut ring = Profile {
                loops: vec![circle([0.0; 2], reach, true)],
            };
            ring.loops.push(circle([0.0; 2], nearest, false));
            let want = Primitive::Extrude {
                frame: Frame::at([0.0; 3]),
                profile: ring,
                height: h,
            };
            draw(axis_box([x0, y0, 0.0], [x1, y1, h]), whole_turn(r), want)
        }
        _ => {
            let x0 = r.range(1.0, 4.0);
            let x1 = x0 + r.range(1.0, 4.0);
            let w = r.range(1.0, 2.0) * x0.min(2.0);
            // Far enough for the outer corners' arcs to join into one, and short of meeting itself.
            let least = 2.0 * (0.5 * w).patan2(x1) + 0.2;
            let most = TAU - 2.0 * (0.5 * w).patan2(x0) - 0.4;
            let theta = sign(r) * r.range(least, most);
            draw(
                axis_box([x0, -0.5 * w, 0.0], [x1, 0.5 * w, h]),
                turn(r, theta),
                turned_block(x0, x1, w, h, theta),
            )
        }
    }
}

/// A piece of thread about the world's z axis, and the longer piece it becomes screwed on by
/// `theta` along its own helix.
fn thread(r: &mut Rng, theta: f64) -> (Primitive, f64, Primitive) {
    let rise = sign(r) * r.range(0.3, 1.0);
    let profile = section(r, 0.8 * TAU * rise.abs());
    let had = r.range(0.3, 1.5);
    let piece = |frame: Frame, angle: f64| Primitive::Screw {
        frame,
        profile: profile.clone(),
        rise,
        angle,
    };
    let back = theta.min(0.0);
    (
        piece(Frame::at([0.0; 3]), had),
        rise,
        piece(turned(back, rise * back), had + theta.abs()),
    )
}

fn screw(r: &mut Rng, rise: f64, angle: f64) -> Motion {
    Motion::Screw {
        origin: [0.0, 0.0, r.range(-5.0, 5.0)],
        axis: [0.0, 0.0, r.range(0.5, 3.0)],
        rise,
        angle,
    }
}

/// A piece of thread screwed on along its own helix, and a rod, a frustum or a ball screwed along
/// its own axis, where the turning moves nothing.
pub fn screws(r: &mut Rng) -> FeatureDraw {
    let theta = sign(r) * r.range(0.5, 7.0);
    if r.next_u64().is_multiple_of(2) {
        let (base, rise, want) = thread(r, theta);
        return draw(base, screw(r, rise, theta), want);
    }
    let rise = sign(r) * r.range(0.3, 1.0);
    let (base, want) = carried_along_its_axis(r, rise * theta);
    draw(base, screw(r, rise, theta), want)
}

fn then(first: Motion, second: Motion) -> Motion {
    Motion::Then(Box::new(first), Box::new(second))
}

/// A rod or a ball turned about its own axis, which moves nothing, and then slid across that axis.
/// The other way round the turn would swing the slid solid about where it started, so only the
/// order `then` states gives the slide's prism or capsule.
fn turned_in_place_then_slid(r: &mut Rng) -> FeatureDraw {
    let theta = sign(r) * r.range(1.0, 5.0);
    let rad = r.range(1.0, 4.0);
    if r.next_u64().is_multiple_of(2) {
        let h = r.range(1.0, 6.0);
        let (vector, want) = slid_rod(r, rad, h);
        let motion = then(turn(r, theta), Motion::Slide { vector });
        return draw(cylinder(0.0, rad, h), motion, want);
    }
    let centre = [0.0, 0.0, r.range(-3.0, 3.0)];
    let phi = r.range(0.0, TAU);
    let len = rad * r.range(1.0, 3.0);
    let vector = [len * phi.pcos(), len * phi.psin(), 0.0];
    let base = Primitive::Sphere {
        frame: Frame::at(centre),
        radius: rad,
    };
    let motion = then(turn(r, theta), Motion::Slide { vector });
    draw(base, motion, capsule(centre, vector, rad))
}

/// Two motions at once that come to one the suite knows: two slides to a slide along a block's
/// edge, a turn and a slide along its axis to a screw, two turns about one axis to a turn, a
/// screw with the slide that undoes its rise to a turn, and a turn in place before a slide.
pub fn composed(r: &mut Rng) -> FeatureDraw {
    match r.next_u64() % 5 {
        4 => turned_in_place_then_slid(r),
        0 => {
            let block = [r.range(2.0, 8.0), r.range(2.0, 8.0), r.range(2.0, 8.0)];
            let axis = (r.next_u64() % 3) as usize;
            let d = sign(r) * r.range(1.0, 6.0);
            let first = [r.range(-3.0, 3.0), r.range(-3.0, 3.0), r.range(-3.0, 3.0)];
            let mut second = first.map(|x| -x);
            second[axis] += d;
            let (mut lo, mut hi) = ([0.0; 3], block);
            lo[axis] = d.min(0.0);
            hi[axis] += d.max(0.0);
            draw(
                axis_box([0.0; 3], block),
                then(
                    Motion::Slide { vector: first },
                    Motion::Slide { vector: second },
                ),
                axis_box(lo, hi),
            )
        }
        1 => {
            // A turn goes no further than once round.
            let theta = sign(r) * r.range(0.5, 6.0);
            let (base, rise, want) = thread(r, theta);
            let slide = Motion::Slide {
                vector: [0.0, 0.0, rise * theta],
            };
            let turn = turn(r, theta);
            let motion = if r.next_u64().is_multiple_of(2) {
                then(turn, slide)
            } else {
                then(slide, turn)
            };
            draw(base, motion, want)
        }
        kind => {
            let profile = section(r, f64::INFINITY);
            let had = r.range(0.3, 1.5);
            let theta = sign(r) * r.range(0.4, 4.0);
            let base = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile: profile.clone(),
                angle: had,
            };
            let want = Primitive::Revolve {
                frame: turned(theta.min(0.0), 0.0),
                profile,
                angle: had + theta.abs(),
            };
            let motion = if kind == 2 {
                let part = theta * r.range(0.2, 0.8);
                then(turn(r, part), turn(r, theta - part))
            } else {
                let rise = sign(r) * r.range(0.3, 1.0);
                let undo = Motion::Slide {
                    vector: [0.0, 0.0, -rise * theta],
                };
                then(screw(r, rise, theta), undo)
            };
            draw(base, motion, want)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rigid;
    use crate::oracle;

    const STEPS: usize = 720;

    fn undone(m: &Rigid, p: Vec3) -> Vec3 {
        let q = [p[0] - m.t[0], p[1] - m.t[1], p[2] - m.t[2]];
        [0, 1, 2].map(|j| (0..3).map(|i| m.r[i][j] * q[i]).sum())
    }

    /// Whether the base holds the point at one of `STEPS` evenly spaced poses: a point this finds
    /// is in the sweep, and one it misses is outside it or in a sliver between two poses, as a
    /// corner leaves just inside the arc it sweeps.
    fn reached(base: &Shape, poses: &[Rigid], p: Vec3) -> bool {
        poses.iter().any(|m| oracle::contains(base, undone(m, p)))
    }

    #[test]
    fn every_swept_solid_is_what_its_base_passes_through() {
        let families: [(&str, fn(&mut Rng) -> FeatureDraw); 4] = [
            ("slides", slides),
            ("turns", turns),
            ("screws", screws),
            ("composed", composed),
        ];
        for (name, family) in families {
            let mut r = Rng::new(0x5eed ^ name.len() as u64);
            for case in 0..12 {
                let (base, feature, want, _) = family(&mut r);
                let motion = feature.motion.expect("a sweep has a motion");
                let poses: Vec<Rigid> = (0..=STEPS)
                    .map(|k| motion.at(k as f64 / STEPS as f64))
                    .collect();
                let base = Shape::Prim(base);
                let (lo, hi) = oracle::bounds(&want).expect("the result has a box");
                let pad = [0, 1, 2].map(|i| 0.1 * (hi[i] - lo[i]) + 0.1);
                let wide = (
                    [0, 1, 2].map(|i| lo[i] - pad[i]),
                    [0, 1, 2].map(|i| hi[i] + pad[i]),
                );
                let pts = oracle::samples(wide, 1000);
                let (mut beyond, mut missed, mut inside) = (0, 0, 0);
                for &p in &pts {
                    let (held, swept) = (oracle::contains(&want, p), reached(&base, &poses, p));
                    inside += usize::from(held);
                    beyond += usize::from(swept && !held);
                    missed += usize::from(held && !swept);
                }
                assert!(
                    inside > pts.len() / 200,
                    "{name} {case}: {inside} points inside"
                );
                assert_eq!(beyond, 0, "{name} {case}: {motion:?} reaches past {want:?}");
                assert!(
                    missed * 50 <= inside,
                    "{name} {case}: {missed} of {inside} points never reached by {motion:?}"
                );
            }
        }
    }

    #[test]
    fn a_hull_is_the_corners_counter_clockwise() {
        let square = vec![[1.0, 1.0], [0.0, 0.0], [0.5, 0.5], [1.0, 0.0], [0.0, 1.0]];
        assert_eq!(
            hull(square),
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        );
    }
}
