//! Edits whose faces open, close or vanish against curved neighbours: features taken off turned
//! parts, bosses and concave rounds taken away, frusta, balls, cones and rounded pucks hollowed, a
//! step's face opened by a shell, a torus grown past its hole, a boss grown past the round at its
//! root, and a cone grown round its apex.

use super::*;

/// The point of a turned part's meridian at `p`, turned to `phi` about the z axis.
fn turned(p: [f64; 2], phi: f64) -> Vec3 {
    [p[0] * phi.pcos(), p[0] * phi.psin(), p[1]]
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

/// The turn of a meridian of one loop about the z axis.
fn revolve(lp: Vec<Seg>) -> Primitive {
    Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: Profile { loops: vec![lp] },
        angle: TAU,
    }
}

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = v[0].phypot(v[1]);
    [v[0] / l, v[1] / l]
}

/// A point inside the blend of corner `i` of the loop `pts`, blended by `size`: the middle of a
/// rounding's arc, convex or concave, or of a chamfer's cut.
fn on_blend(blend: Act, pts: &[[f64; 2]], i: usize, size: f64) -> [f64; 2] {
    let n = pts.len();
    let (p, q, o) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
    let (u, v) = (
        unit2([p[0] - q[0], p[1] - q[1]]),
        unit2([o[0] - q[0], o[1] - q[1]]),
    );
    match blend {
        Act::Fillet => {
            // The ball sits on the bisector of the two sides, size / sin(half their angle) from
            // the corner, inside a convex corner and outside a concave one.
            let half = (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0).pacos() / 2.0;
            let b = unit2([u[0] + v[0], u[1] + v[1]]);
            let to_arc = size / half.psin() - size;
            [q[0] + b[0] * to_arc, q[1] + b[1] * to_arc]
        }
        _ => [
            q[0] + size * (u[0] + v[0]) / 2.0,
            q[1] + size * (u[1] + v[1]) / 2.0,
        ],
    }
}

/// The loop `pts` with corner `i` blended by a size drawn to fit it, and a point in the blend.
fn blend_corner(r: &mut Rng, blend: Act, pts: &[[f64; 2]], i: usize) -> (Profile, [f64; 2]) {
    let size = blend_size(r, blend, pts, &[i]);
    let sizes: Vec<f64> = (0..pts.len())
        .map(|k| if k == i { size } else { 0.0 })
        .collect();
    (blended(blend, pts, &sizes), on_blend(blend, pts, i, size))
}

fn delete(points: Vec<Vec3>) -> Feature {
    face_act(Act::DeleteFaces, points, 0.0, None)
}

/// A feature taken off a turned part, the gap closed against what is round it. Each result is the
/// part without the feature, the faces round it run on to meet.
#[derive(Clone, Copy)]
pub(super) enum Turned {
    /// A cylinder's rounded rim: the gap closes against the cylinder and its end.
    RimRound,
    /// A frustum's bevelled rim: against the frustum's cone and its end.
    RimBevel,
    /// A cone's rounded foot: against the cone and its base.
    FootRound,
    /// The round where a cylinder meets the dome on it: against the cylinder and a sphere.
    DomeRound,
    /// A hemisphere's rounded rim: against the sphere and the flat.
    HemisphereRound,
    /// The rounded rim of a tube's bore: against the hollow bore and the end.
    BoreRound,
    /// A groove cut round a tube's bore: against the hollow bore.
    BoreGroove,
    /// A countersink at a tube's bore: against the hollow bore and the end.
    Countersink,
}

pub(super) fn turned_delete(r: &mut Rng, which: Turned) -> FeatureDraw {
    let phi = r.range(0.0, TAU);
    let at = |p: [f64; 2]| turned(p, phi);
    let cylinder = |radius: f64, height: f64| Primitive::Cylinder {
        frame: Frame::at([0.0; 3]),
        radius,
        height,
    };
    let revolve_profile = |profile: Profile| Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile,
        angle: TAU,
    };
    let (base, feature, want) = match which {
        Turned::RimRound => {
            let (rad, h) = (r.range(2.0, 6.0), r.range(3.0, 9.0));
            let pts = [[0.0, 0.0], [rad, 0.0], [rad, h], [0.0, h]];
            let (lp, p) = blend_corner(r, Act::Fillet, &pts, 2);
            (revolve_profile(lp), delete(vec![at(p)]), cylinder(rad, h))
        }
        Turned::RimBevel => {
            let (bottom, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
            let top = bottom * r.range(0.4, 0.8);
            let pts = [[0.0, 0.0], [bottom, 0.0], [top, h], [0.0, h]];
            let (lp, p) = blend_corner(r, Act::Chamfer, &pts, 2);
            let want = cone(Frame::at([0.0; 3]), bottom, top, h);
            (revolve_profile(lp), delete(vec![at(p)]), want)
        }
        Turned::FootRound => {
            let (bottom, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
            let pts = [[0.0, 0.0], [bottom, 0.0], [0.0, h]];
            let (lp, p) = blend_corner(r, Act::Fillet, &pts, 1);
            let want = cone(Frame::at([0.0; 3]), bottom, 0.0, h);
            (revolve_profile(lp), delete(vec![at(p)]), want)
        }
        Turned::DomeRound => {
            // A cylinder of radius `rad` to height `h`, capped by the sphere of radius `rho`
            // about (0, c) that meets it there.
            let rad = r.range(2.0, 5.0);
            let rho = rad * r.range(1.15, 2.0);
            let h = r.range(2.0, 6.0);
            let c = h - (rho * rho - rad * rad).sqrt();
            let s = rad.min(h) * r.range(0.08, 0.3);
            // The round: tangent to the cylinder, so its centre is `s` in, and inside the sphere,
            // so its centre is `rho − s` from the sphere's.
            let yc = c + ((rho - s).powi(2) - (rad - s).powi(2)).sqrt();
            let touch = (yc - c).patan2(rad - s);
            let base = revolve(vec![
                line([0.0, 0.0], [rad, 0.0]),
                line([rad, 0.0], [rad, yc]),
                arc([rad - s, yc], s, 0.0, touch),
                arc([0.0, c], rho, touch, PI / 2.0 - touch),
                line([0.0, c + rho], [0.0, 0.0]),
            ]);
            let mid = touch / 2.0;
            let p = [rad - s + s * mid.pcos(), yc + s * mid.psin()];
            let meet = (h - c).patan2(rad);
            let want = revolve(vec![
                line([0.0, 0.0], [rad, 0.0]),
                line([rad, 0.0], [rad, h]),
                arc([0.0, c], rho, meet, PI / 2.0 - meet),
                line([0.0, c + rho], [0.0, 0.0]),
            ]);
            (base, delete(vec![at(p)]), want)
        }
        Turned::HemisphereRound => {
            let rho = r.range(2.0, 6.0);
            let s = rho * r.range(0.08, 0.3);
            // The round: on the flat, so its centre is `s` up, and inside the sphere.
            let xc = ((rho - s).powi(2) - s * s).sqrt();
            let touch = s.patan2(xc);
            let base = revolve(vec![
                line([0.0, 0.0], [xc, 0.0]),
                arc([xc, s], s, -PI / 2.0, touch + PI / 2.0),
                arc([0.0, 0.0], rho, touch, PI / 2.0 - touch),
                line([0.0, rho], [0.0, 0.0]),
            ]);
            let mid = (touch - PI / 2.0) / 2.0;
            let p = [xc + s * mid.pcos(), s + s * mid.psin()];
            let want = revolve(vec![
                line([0.0, 0.0], [rho, 0.0]),
                arc([0.0, 0.0], rho, 0.0, PI / 2.0),
                line([0.0, rho], [0.0, 0.0]),
            ]);
            (base, delete(vec![at(p)]), want)
        }
        Turned::BoreRound | Turned::Countersink => {
            let bore = r.range(1.0, 3.0);
            let (rad, h) = (bore + r.range(2.0, 4.0), r.range(3.0, 8.0));
            let pts = [[bore, 0.0], [rad, 0.0], [rad, h], [bore, h]];
            let blend = match which {
                Turned::BoreRound => Act::Fillet,
                _ => Act::Chamfer,
            };
            let (lp, p) = blend_corner(r, blend, &pts, 3);
            (
                revolve_profile(lp),
                delete(vec![at(p)]),
                revolve_profile(polygon(&pts)),
            )
        }
        Turned::BoreGroove => {
            let bore = r.range(1.0, 3.0);
            let (rad, h) = (bore + r.range(2.0, 4.0), r.range(3.0, 8.0));
            let (z0, z1) = (h * r.range(0.2, 0.4), h * r.range(0.55, 0.8));
            let g = (rad - bore) * r.range(0.2, 0.5);
            let base = revolve_profile(polygon(&[
                [bore, 0.0],
                [rad, 0.0],
                [rad, h],
                [bore, h],
                [bore, z1],
                [bore + g, z1],
                [bore + g, z0],
                [bore, z0],
            ]));
            let points = vec![
                at([bore + g, (z0 + z1) / 2.0]),
                at([bore + g / 2.0, z1]),
                at([bore + g / 2.0, z0]),
            ];
            let want = revolve_profile(polygon(&[[bore, 0.0], [rad, 0.0], [rad, h], [bore, h]]));
            (base, delete(points), want)
        }
    };
    (base, feature, want.into(), Exact::default())
}

/// Faces taken away whose gap closes by taking material away: a boss on a disc, the concave round
/// at a post's root, or the round in an L prism's inside corner. Each result is the part without
/// them.
#[derive(Clone, Copy)]
pub(super) enum Removal {
    Boss,
    RootRound,
    InsideRound,
}

pub(super) fn removal_delete(r: &mut Rng, which: Removal) -> FeatureDraw {
    let phi = r.range(0.0, TAU);
    let revolve_profile = |profile: Profile| Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile,
        angle: TAU,
    };
    let (base, feature, want) = match which {
        Removal::Boss | Removal::RootRound => {
            let (rad, h) = (r.range(4.0, 8.0), r.range(1.0, 3.0));
            let (boss, tall) = (rad * r.range(0.2, 0.6), r.range(1.5, 4.0));
            let pts = [
                [0.0, 0.0],
                [rad, 0.0],
                [rad, h],
                [boss, h],
                [boss, h + tall],
                [0.0, h + tall],
            ];
            match which {
                Removal::Boss => {
                    let points = vec![
                        turned([boss, h + tall / 2.0], phi),
                        turned([boss / 2.0, h + tall], phi),
                    ];
                    let want = Primitive::Cylinder {
                        frame: Frame::at([0.0; 3]),
                        radius: rad,
                        height: h,
                    };
                    (revolve_profile(polygon(&pts)), delete(points), want)
                }
                _ => {
                    let (lp, p) = blend_corner(r, Act::Fillet, &pts, 3);
                    (
                        revolve_profile(lp),
                        delete(vec![turned(p, phi)]),
                        revolve_profile(polygon(&pts)),
                    )
                }
            }
        }
        Removal::InsideRound => {
            let a = r.range(8.0, 14.0);
            let c = a * r.range(0.3, 0.6);
            let h1 = r.range(2.0, 5.0);
            let h2 = h1 + r.range(2.0, 6.0);
            let w = r.range(3.0, 8.0);
            let pts = [[0.0, 0.0], [a, 0.0], [a, h1], [c, h1], [c, h2], [0.0, h2]];
            let (lp, p) = blend_corner(r, Act::Fillet, &pts, 3);
            (
                prism_xy(lp, w),
                delete(vec![[p[0], p[1], w / 2.0]]),
                prism_xy(polygon(&pts), w),
            )
        }
    };
    (base, feature, want.into(), Exact::default())
}

fn shell(points: Vec<Vec3>, t: f64) -> Feature {
    face_act(Act::Shell, points, t, None)
}

/// A frustum narrowing upward hollowed to walls `t` thick: open through its wide bottom or its
/// narrow top, or closed round a void. Each result is one turned profile, the cone's wall moved in
/// by `t` along its normal and ending flush in the opening's plane.
#[derive(Clone, Copy)]
pub(super) enum Opened {
    WideBottom,
    NarrowTop,
    Closed,
}

pub(super) fn frustum_shell(r: &mut Rng, opened: Opened) -> FeatureDraw {
    let (bottom, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
    let top = bottom * r.range(0.3, 0.75);
    let slant = (bottom - top).phypot(h);
    // How far in a wall `t` thick reaches at a given height, measured across.
    let across = slant / h;
    let t = (top / across).min(h) * r.range(0.08, 0.25);
    let inner = |y: f64| bottom - (bottom - top) * y / h - t * across;
    let base = cone(Frame::at([0.0; 3]), bottom, top, h);
    let (want, points) = match opened {
        Opened::WideBottom => (
            revolve(vec![
                line([inner(0.0), 0.0], [bottom, 0.0]),
                line([bottom, 0.0], [top, h]),
                line([top, h], [0.0, h]),
                line([0.0, h], [0.0, h - t]),
                line([0.0, h - t], [inner(h - t), h - t]),
                line([inner(h - t), h - t], [inner(0.0), 0.0]),
            ]),
            vec![[0.0, 0.0, 0.0]],
        ),
        Opened::NarrowTop => (
            revolve(vec![
                line([0.0, 0.0], [bottom, 0.0]),
                line([bottom, 0.0], [top, h]),
                line([top, h], [inner(h), h]),
                line([inner(h), h], [inner(t), t]),
                line([inner(t), t], [0.0, t]),
                line([0.0, t], [0.0, 0.0]),
            ]),
            vec![[0.0, 0.0, h]],
        ),
        Opened::Closed => (
            revolve(vec![
                line([0.0, 0.0], [bottom, 0.0]),
                line([bottom, 0.0], [top, h]),
                line([top, h], [0.0, h]),
                line([0.0, h], [0.0, h - t]),
                line([0.0, h - t], [inner(h - t), h - t]),
                line([inner(h - t), h - t], [inner(t), t]),
                line([inner(t), t], [0.0, t]),
                line([0.0, t], [0.0, 0.0]),
            ]),
            vec![],
        ),
    };
    (base, shell(points, t), want.into(), Exact::default())
}

/// A ball hollowed round a void, or a cone to its apex hollowed open through its base or round a
/// void. The ball's void is the ball `t` smaller; the cone's wall moves in by `t` along its normal
/// and closes to an apex of its own below the cone's.
#[derive(Clone, Copy)]
pub(super) enum Curved {
    Ball,
    ConeOpen,
    ConeClosed,
}

pub(super) fn curved_shell(r: &mut Rng, which: Curved) -> FeatureDraw {
    match which {
        Curved::Ball => {
            let rad = r.range(2.0, 7.0);
            let t = rad * r.range(0.05, 0.3);
            let inner = rad - t;
            let want = revolve(vec![
                arc([0.0, 0.0], rad, -PI / 2.0, PI),
                line([0.0, rad], [0.0, inner]),
                arc([0.0, 0.0], inner, PI / 2.0, -PI),
                line([0.0, -inner], [0.0, -rad]),
            ]);
            (
                sphere([0.0; 3], rad),
                shell(vec![], t),
                want.into(),
                Exact::default(),
            )
        }
        Curved::ConeOpen | Curved::ConeClosed => {
            let (rad, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
            let slant = rad.phypot(h);
            let t = rad.min(h) * r.range(0.06, 0.18);
            let inner = |y: f64| rad * (1.0 - y / h) - t * slant / h;
            // The moved wall meets the axis this far up.
            let apex = h - t * slant / rad;
            let base = cone(Frame::at([0.0; 3]), rad, 0.0, h);
            let (want, points) = match which {
                Curved::ConeOpen => (
                    revolve_polygon(&[[inner(0.0), 0.0], [rad, 0.0], [0.0, h], [0.0, apex]]),
                    vec![[0.0, 0.0, 0.0]],
                ),
                _ => (
                    revolve_polygon(&[
                        [0.0, 0.0],
                        [rad, 0.0],
                        [0.0, h],
                        [0.0, apex],
                        [inner(t), t],
                        [0.0, t],
                    ]),
                    vec![],
                ),
            };
            (base, shell(points, t), want.into(), Exact::default())
        }
    }
}

fn revolve_polygon(pts: &[[f64; 2]]) -> Primitive {
    Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: polygon(pts),
        angle: TAU,
    }
}

/// A puck, a cylinder with both rims rounded by `rad`, hollowed round a void by walls `t` thick:
/// thinner than its rounds, when the void is the puck `t` smaller with rounds `rad − t`, or
/// thicker, when the void's rims are sharp, the rounds having closed to nothing inside.
pub(super) fn puck_shell(r: &mut Rng, past: bool) -> FeatureDraw {
    let (rad, h) = (r.range(3.0, 7.0), r.range(3.0, 8.0));
    let round = rad.min(h) * r.range(0.12, 0.3);
    let t = if past {
        round * r.range(1.3, 2.0)
    } else {
        round * r.range(0.3, 0.8)
    };
    let puck = |lo: f64, radius: f64, height: f64, rounds: f64| Primitive::Revolve {
        frame: Frame::at([0.0, 0.0, lo]),
        profile: Profile {
            loops: vec![rounded(
                &[[0.0, 0.0], [radius, 0.0], [radius, height], [0.0, height]],
                &[0.0, rounds, rounds, 0.0],
            )],
        },
        angle: TAU,
    };
    let base = puck(0.0, rad, h, round);
    let void = puck(t, rad - t, h - 2.0 * t, (round - t).max(0.0));
    let exact = Exact {
        result_volume: Some(base.volume() - void.volume()),
        result_area: Some(base.area() + void.area()),
        ..Default::default()
    };
    let want = Shape::bool(Op::Difference, base.clone(), void);
    (base, shell(vec![], t), want, exact)
}

/// The sine and cosine of the angle `lean` the wall above an L's step makes with the step, exact
/// for an upright wall.
fn wall_dir(lean: f64) -> (f64, f64) {
    if lean == FRAC_PI_2 {
        (1.0, 0.0)
    } else {
        (lean.psin(), lean.pcos())
    }
}

/// The cavity of an L prism shelled open at its step, in the L's plane: every side but the step
/// moved in by `t`, the moved sides extended or trimmed to meet each other and the step's line, so
/// the wall above the step ends flush with it. The wall meets the step at `lean`, a right angle for
/// an upright wall. With `past`, the cavity also runs out through the opening by that much, from
/// the step's edge into the open air, so that it meets the L's faces only along that edge.
fn step_cavity(a: f64, c: f64, h1: f64, h2: f64, t: f64, past: f64, lean: f64) -> Profile {
    let (sn, cs) = wall_dir(lean);
    // Where the wall's inner face crosses the step's line, and where it meets the top's.
    let low = c - t / sn;
    let top = low + (h2 - t - h1) * cs / sn;
    let mut lp = vec![line([t, t], [a - t, t])];
    if past > 0.0 {
        // Half way between the step and the wall, the open air's middle.
        let half = lean / 2.0;
        let edge = [c + past * half.pcos() / half.psin(), h1 + past];
        lp.extend([
            line([a - t, t], [a - t, h1 + past]),
            line([a - t, h1 + past], edge),
            line(edge, [c, h1]),
            line([c, h1], [low, h1]),
        ]);
    } else {
        lp.extend([line([a - t, t], [a - t, h1]), line([a - t, h1], [low, h1])]);
    }
    lp.extend([
        line([low, h1], [top, h2 - t]),
        line([top, h2 - t], [t, h2 - t]),
        line([t, h2 - t], [t, t]),
    ]);
    Profile { loops: vec![lp] }
}

/// The L prism `a` wide, `h1` high at its step and `h2` at its wall, `w` deep, shelled `t` thick
/// open at the step, its wall meeting the step at `lean`: the shape, its volume and its area.
fn step_shell(
    a: f64,
    c: f64,
    h1: f64,
    h2: f64,
    w: f64,
    t: f64,
    lean: f64,
) -> (Primitive, Shape, f64, f64) {
    let (sn, cs) = wall_dir(lean);
    let ell = prism_xy(
        polygon(&[
            [0.0, 0.0],
            [a, 0.0],
            [a, h1],
            [c, h1],
            [c + (h2 - h1) * cs / sn, h2],
            [0.0, h2],
        ]),
        w,
    );
    let in_frame = |profile: Profile| Primitive::Extrude {
        frame: Frame::at([0.0, 0.0, t]),
        profile,
        height: w - 2.0 * t,
    };
    let cavity = in_frame(step_cavity(a, c, h1, h2, t, 0.0, lean));
    // The opening, from the step's edge to the far wall, is in both the L's area and the cavity's
    // and in neither boundary. Under the wall, the cavity's side on the step's line is the end of
    // the wall, which stays.
    let opening = (a - c - t) * (w - 2.0 * t);
    let volume = ell.volume() - cavity.volume();
    let area = ell.area() + cavity.area() - 2.0 * opening;
    let through = in_frame(step_cavity(a, c, h1, h2, t, 0.5 * t, lean));
    let want = Shape::bool(Op::Difference, ell.clone(), through);
    (ell, want, volume, area)
}

/// An L prism shelled to walls `t` thick, open through the face of its step.
pub(super) fn l_shell(r: &mut Rng) -> FeatureDraw {
    let a = r.range(8.0, 14.0);
    let c = a * r.range(0.3, 0.6);
    let h1 = r.range(2.0, 5.0);
    let h2 = h1 + r.range(2.0, 6.0);
    let w = r.range(3.0, 8.0);
    let t = [c, a - c, h1, h2 - h1, w]
        .into_iter()
        .fold(f64::INFINITY, f64::min)
        * r.range(0.1, 0.25);
    let (base, want, volume, area) = step_shell(a, c, h1, h2, w, t, FRAC_PI_2);
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    let feature = shell(vec![[(a + c) / 2.0, h1, w / 2.0]], t);
    (base, feature, want, exact)
}

/// An L prism whose wall meets its step at `lean`, leaning back from the step past a right angle
/// or over it short of one, shelled to walls `t` thick open through the face of its step.
pub(super) fn leaning_l_shell(r: &mut Rng, lean: f64) -> FeatureDraw {
    let (sn, cs) = wall_dir(lean);
    let a = r.range(8.0, 14.0);
    let c = a * r.range(0.35, 0.6);
    let h1 = r.range(2.0, 5.0);
    let rise = r.range(2.0, 5.0);
    // The wall's top stays within the L: at most 0.6 of the way across what it leans over.
    let room = if lean < FRAC_PI_2 { a - c } else { c };
    let rise = rise.min(0.6 * room * sn / cs.abs());
    let h2 = h1 + rise;
    let w = r.range(3.0, 8.0);
    let shift = rise * cs.abs() / sn;
    let top = if lean > FRAC_PI_2 { c - shift } else { c };
    let t = [top, a - c, h1, rise, w]
        .into_iter()
        .fold(f64::INFINITY, f64::min)
        * r.range(0.1, 0.25);
    let (base, want, volume, area) = step_shell(a, c, h1, h2, w, t, lean);
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    let feature = shell(vec![[(a + c) / 2.0, h1, w / 2.0]], t);
    (base, feature, want, exact)
}

fn grow(d: f64) -> Feature {
    face_act(Act::Offset, vec![], d, None)
}

/// A torus grown by `d`: a torus still, while its hole stays open; past the hole, the turn of the
/// tube's circle cut off at the axis, where the two sides of the hole have met.
pub(super) fn torus_offset(r: &mut Rng, past: bool) -> FeatureDraw {
    let major = r.range(3.0, 7.0);
    let minor = major * r.range(0.2, 0.5);
    let hole = major - minor;
    let d = if past {
        hole * r.range(1.3, 2.5)
    } else {
        hole * r.range(0.2, 0.7)
    };
    let base = Primitive::Torus {
        frame: Frame::at([0.0; 3]),
        major,
        minor,
    };
    let tube = minor + d;
    let want = if tube < major {
        Primitive::Torus {
            frame: Frame::at([0.0; 3]),
            major,
            minor: tube,
        }
    } else {
        // Where the tube's circle crosses the axis, and the arc round the outside between.
        let reach = (tube * tube - major * major).sqrt();
        revolve(vec![
            arc(
                [major, 0.0],
                tube,
                (-reach).patan2(-major),
                TAU - 2.0 * reach.patan2(major),
            ),
            line([0.0, reach], [0.0, -reach]),
        ])
    };
    (base, grow(d), want.into(), Exact::default())
}

/// The meridian `pts` of a turned solid, counter-clockwise, its first and last points on the axis,
/// grown by `d`: each side moved out by `d`, a convex corner rounded by `d` about where it was, and
/// a concave corner rounded by what is left of its round `radii[i]`, about the same centre, sharp
/// once `d` reaches it. The ends stay on the axis, moved along it with the sides they end.
fn grown_meridian(pts: &[[f64; 2]], radii: &[f64], d: f64) -> Vec<Seg> {
    let n = pts.len();
    // The sides off the axis, from point k to k + 1 for k < n − 1, each moved out by d.
    let moved: Vec<([f64; 2], [f64; 2])> = (0..n - 1)
        .map(|k| {
            let (p, q) = (pts[k], pts[k + 1]);
            let u = unit2([q[0] - p[0], q[1] - p[1]]);
            let out = [u[1], -u[0]];
            ([p[0] + d * out[0], p[1] + d * out[1]], u)
        })
        .collect();
    let meet = |a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])| {
        let (p, u) = a;
        let (q, v) = b;
        let det = u[0] * v[1] - u[1] * v[0];
        let s = ((q[0] - p[0]) * v[1] - (q[1] - p[1]) * v[0]) / det;
        [p[0] + s * u[0], p[1] + s * u[1]]
    };
    // Where a moved side crosses the axis.
    let on_axis = |(p, u): ([f64; 2], [f64; 2])| [0.0, p[1] - p[0] * u[1] / u[0]];
    let mut corners = vec![on_axis(moved[0])];
    let mut sizes = vec![0.0];
    for k in 1..n - 1 {
        corners.push(meet(moved[k - 1], moved[k]));
        let (a, b) = (moved[k - 1].1, moved[k].1);
        let convex = a[0] * b[1] - a[1] * b[0] > 0.0;
        sizes.push(if convex { d } else { (radii[k] - d).max(0.0) });
    }
    corners.push(on_axis(moved[n - 2]));
    sizes.push(0.0);
    rounded(&corners, &sizes)
}

/// A boss on a disc with a concave round of radius `round` at its root, grown by `d`: less than
/// the round, when the round shrinks by `d` about its centre, or more, when it has closed to a sharp
/// corner. The boss is a post or a frustum.
pub(super) fn root_round_offset(r: &mut Rng, frustum: bool, past: bool) -> FeatureDraw {
    let (rad, h) = (r.range(4.0, 8.0), r.range(1.5, 3.0));
    let (foot, tall) = (rad * r.range(0.3, 0.5), r.range(2.0, 4.0));
    let head = if frustum {
        foot * r.range(0.5, 0.8)
    } else {
        foot
    };
    let pts = [
        [0.0, 0.0],
        [rad, 0.0],
        [rad, h],
        [foot, h],
        [head, h + tall],
        [0.0, h + tall],
    ];
    let round = blend_size(r, Act::Fillet, &pts, &[3]);
    let mut radii = [0.0; 6];
    radii[3] = round;
    let d = if past {
        round * r.range(1.2, 2.0)
    } else {
        round * r.range(0.25, 0.75)
    };
    let base = Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: Profile {
            loops: vec![rounded(&pts, &radii)],
        },
        angle: TAU,
    };
    let want = revolve(grown_meridian(&pts, &radii, d));
    (base, grow(d), want.into(), Exact::default())
}

/// A cone to its apex grown by `d`: its base moved down, its side moved out, the rim rounded by
/// `d`, and the apex by a ball of radius `d`.
pub(super) fn apex_offset(r: &mut Rng) -> FeatureDraw {
    let (rad, h) = (r.range(2.0, 6.0), r.range(3.0, 9.0));
    let d = rad.min(h) * r.range(0.1, 0.6);
    let slant = rad.phypot(h);
    let normal = [h / slant, rad / slant];
    let turn = rad.patan2(h);
    let want = revolve(vec![
        line([0.0, -d], [rad, -d]),
        arc([rad, 0.0], d, -PI / 2.0, turn + PI / 2.0),
        line(
            [rad + d * normal[0], d * normal[1]],
            [d * normal[0], h + d * normal[1]],
        ),
        arc([0.0, h], d, turn, PI / 2.0 - turn),
        line([0.0, h + d], [0.0, -d]),
    ]);
    (
        cone(Frame::at([0.0; 3]), rad, 0.0, h),
        grow(d),
        want.into(),
        Exact::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_l_prism_shelled_open_at_its_step_has_its_worked_volume_and_area() {
        // 10 wide, the wall 4 wide and 8 high, the step 3 high, 6 deep, walls 1 thick: the section
        // is 50. The cavity is 8 by 2 below the step and 2 by 4 above it, the wall ending flush
        // with the step, so 24, 4 deep.
        let (_, _, volume, area) = step_shell(10.0, 4.0, 3.0, 8.0, 6.0, 1.0, FRAC_PI_2);
        assert!((volume - 204.0).abs() < 1e-9, "{volume}");
        // The L: two ends 100, sides 6 × 36. The cavity: ends 48, sides 4 × 28. Less the opening,
        // 5 by 4, from each.
        assert!((area - 436.0).abs() < 1e-9, "{area}");
    }

    #[test]
    fn an_l_prism_whose_wall_leans_back_has_its_worked_volume_and_area() {
        // 10 wide, the step 3 high from x = 5, the wall leaning back at 135° to 6 high at x = 2, 6
        // deep, walls 1 thick: the section is 40.5. The wall's inner face is x + y = 8 − √2 and
        // ends on the step's line at x = 5 − √2. The cavity is 8 by 2 below the step and, above
        // it, up to that face and the top's at y = 5, 6 − 2√2: 22 − 2√2, 4 deep.
        let lean = 3.0 * FRAC_PI_4;
        let (_, _, volume, area) = step_shell(10.0, 5.0, 3.0, 6.0, 6.0, 1.0, lean);
        let r2 = 2f64.sqrt();
        assert!((volume - (155.0 + 8.0 * r2)).abs() < 1e-9, "{volume}");
        // The L: two ends 81, sides 6 × (26 + 3√2). The cavity: ends 44 − 4√2, sides
        // 4 × (20 + 2√2). Less the opening, 4 by 4, from each.
        assert!((area - (329.0 + 22.0 * r2)).abs() < 1e-9, "{area}");
    }

    #[test]
    fn an_l_prism_whose_wall_leans_over_its_step_has_its_worked_volume_and_area() {
        // 10 wide, the step 3 high from x = 4, the wall leaning over it at 45° to 6 high at x = 7,
        // 6 deep, walls 1 thick: the section is 46.5. The wall's inner face is y − x = √2 − 1 and
        // ends on the step's line at x = 4 − √2. The cavity is 8 by 2 below the step and, above
        // it, up to that face and the top's at y = 5, 8 − 2√2: 24 − 2√2, 4 deep.
        let (_, _, volume, area) = step_shell(10.0, 4.0, 3.0, 6.0, 6.0, 1.0, FRAC_PI_4);
        let r2 = 2f64.sqrt();
        assert!((volume - (183.0 + 8.0 * r2)).abs() < 1e-9, "{volume}");
        // The L: two ends 93, sides 6 × (32 + 3√2). The cavity: ends 48 − 4√2, sides
        // 4 × (24 + 2√2). Less the opening, 5 by 4, from each.
        assert!((area - (389.0 + 22.0 * r2)).abs() < 1e-9, "{area}");
    }

    #[test]
    fn a_blend_s_middle_lies_on_its_profile() {
        let pts = [[0.0, 0.0], [5.0, 0.0], [5.0, 4.0], [0.0, 4.0]];
        let p = on_blend(Act::Fillet, &pts, 2, 1.0);
        assert!((p[0] - (4.0 + 0.5f64.sqrt())).abs() < 1e-12, "{p:?}");
        assert!((p[1] - (3.0 + 0.5f64.sqrt())).abs() < 1e-12, "{p:?}");
        let q = on_blend(Act::Chamfer, &pts, 2, 1.0);
        assert!(
            (q[0] - 4.5).abs() < 1e-12 && (q[1] - 3.5).abs() < 1e-12,
            "{q:?}"
        );
        // A concave corner's round lies outside the corner: a post 2 wide on a disc, rounded 1.
        let post = [
            [0.0, 0.0],
            [5.0, 0.0],
            [5.0, 1.0],
            [2.0, 1.0],
            [2.0, 4.0],
            [0.0, 4.0],
        ];
        let c = on_blend(Act::Fillet, &post, 3, 1.0);
        let s = 0.5f64.sqrt();
        assert!(
            (c[0] - (3.0 - s)).abs() < 1e-12 && (c[1] - (2.0 - s)).abs() < 1e-12,
            "{c:?}"
        );
    }

    #[test]
    fn a_grown_post_s_meridian_has_its_worked_corners() {
        // A post 2 wide and 3 tall on a disc 5 wide and 1 thick, a round of 1 at its root, grown
        // by 0.5: the disc's rim and the post's top rounded by 0.5, the root's round 0.5 about its
        // old centre (3, 2).
        let post = [
            [0.0, 0.0],
            [5.0, 0.0],
            [5.0, 1.0],
            [2.0, 1.0],
            [2.0, 4.0],
            [0.0, 4.0],
        ];
        let segs = grown_meridian(&post, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.5);
        let arcs: Vec<([f64; 2], f64)> = segs
            .iter()
            .filter_map(|s| match s {
                Seg::Arc { center, radius, .. } => Some((*center, *radius)),
                _ => None,
            })
            .collect();
        let near =
            |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12;
        assert_eq!(arcs.len(), 4, "{arcs:?}");
        for (c, rad) in [
            ([5.0, 0.0], 0.5),
            ([5.0, 1.0], 0.5),
            ([3.0, 2.0], 0.5),
            ([2.0, 4.0], 0.5),
        ] {
            assert!(
                arcs.iter()
                    .any(|(a, r)| near(*a, c) && (r - rad).abs() < 1e-12),
                "{c:?}"
            );
        }
        // Grown by 1.5, past the round: the root is sharp, where the moved disc top and post side
        // meet at (3.5, 2.5).
        let past = grown_meridian(&post, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.5);
        let ends: Vec<[f64; 2]> = past
            .iter()
            .filter_map(|s| match s {
                Seg::Line { b, .. } => Some(*b),
                _ => None,
            })
            .collect();
        assert!(ends.iter().any(|&e| near(e, [3.5, 2.5])), "{ends:?}");
    }
}
