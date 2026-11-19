//! Edits whose faces open, close or vanish against curved neighbours: a step's face opened by a
//! shell, a frustum hollowed, a turned part's blend or boss taken away, and a torus grown past its
//! hole.

use super::*;

/// The point of a turned part's meridian at `p`, turned to `phi` about the z axis.
fn turned(p: [f64; 2], phi: f64) -> Vec3 {
    [p[0] * phi.pcos(), p[0] * phi.psin(), p[1]]
}

/// A point inside the blend of corner `i` of the meridian `pts`, blended by `size`: the middle of a
/// rounding's arc, or of a chamfer's cut.
fn on_blend(blend: Act, pts: &[[f64; 2]], i: usize, size: f64) -> [f64; 2] {
    let n = pts.len();
    let (p, q, o) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
    let unit = |v: [f64; 2]| {
        let l = v[0].phypot(v[1]);
        [v[0] / l, v[1] / l]
    };
    let (u, v) = (
        unit([p[0] - q[0], p[1] - q[1]]),
        unit([o[0] - q[0], o[1] - q[1]]),
    );
    match blend {
        Act::Fillet => {
            // The ball sits on the bisector, size / sin(half the corner) from the corner.
            let half = (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0).pacos() / 2.0;
            let b = unit([u[0] + v[0], u[1] + v[1]]);
            let to_arc = size / half.psin() - size;
            [q[0] + b[0] * to_arc, q[1] + b[1] * to_arc]
        }
        _ => [
            q[0] + size * (u[0] + v[0]) / 2.0,
            q[1] + size * (u[1] + v[1]) / 2.0,
        ],
    }
}

/// A turned part with one feature taken away, closing the gap against a curved face or a plane: a
/// cylinder's rounded rim, where a torus met the cylinder and the end; a frustum's bevelled rim,
/// where a cone met a cone and the end; a cone's rounded foot; or a boss on a disc, whose taking
/// away takes material with it. Each result is the plain primitive.
#[derive(Clone, Copy)]
pub(super) enum Turned {
    RimRound,
    RimBevel,
    FootRound,
    Boss,
}

pub(super) fn turned_delete(r: &mut Rng, which: Turned) -> FeatureDraw {
    let del = |points: Vec<Vec3>| face_act(Act::DeleteFaces, points, 0.0, None);
    let phi = r.range(0.0, TAU);
    let revolve = |profile: Profile| Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile,
        angle: TAU,
    };
    // A meridian with one corner blended, the faces round it able to close over it.
    let blended_corner = |blend: Act, pts: Vec<[f64; 2]>, i: usize, r: &mut Rng| {
        let size = blend_size(r, blend, &pts, &[i]);
        let sizes: Vec<f64> = (0..pts.len())
            .map(|k| if k == i { size } else { 0.0 })
            .collect();
        let base = revolve(blended(blend, &pts, &sizes));
        (base, del(vec![turned(on_blend(blend, &pts, i, size), phi)]))
    };
    match which {
        Turned::RimRound => {
            let (rad, h) = (r.range(2.0, 6.0), r.range(3.0, 9.0));
            let pts = vec![[0.0, 0.0], [rad, 0.0], [rad, h], [0.0, h]];
            let (base, feature) = blended_corner(Act::Fillet, pts, 2, r);
            let want = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rad,
                height: h,
            };
            (base, feature, want.into(), Exact::default())
        }
        Turned::RimBevel => {
            let (bottom, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
            let top = bottom * r.range(0.4, 0.8);
            let pts = vec![[0.0, 0.0], [bottom, 0.0], [top, h], [0.0, h]];
            let (base, feature) = blended_corner(Act::Chamfer, pts, 2, r);
            let want = cone(Frame::at([0.0; 3]), bottom, top, h);
            (base, feature, want.into(), Exact::default())
        }
        Turned::FootRound => {
            let (bottom, h) = (r.range(3.0, 7.0), r.range(3.0, 9.0));
            let pts = vec![[0.0, 0.0], [bottom, 0.0], [0.0, h]];
            let (base, feature) = blended_corner(Act::Fillet, pts, 1, r);
            let want = cone(Frame::at([0.0; 3]), bottom, 0.0, h);
            (base, feature, want.into(), Exact::default())
        }
        Turned::Boss => {
            let (rad, h) = (r.range(4.0, 8.0), r.range(1.0, 3.0));
            let (boss, tall) = (rad * r.range(0.2, 0.6), r.range(1.0, 4.0));
            let base = revolve(polygon(&[
                [0.0, 0.0],
                [rad, 0.0],
                [rad, h],
                [boss, h],
                [boss, h + tall],
                [0.0, h + tall],
            ]));
            let points = vec![
                turned([boss, h + tall / 2.0], phi),
                turned([boss / 2.0, h + tall], phi),
            ];
            let want = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rad,
                height: h,
            };
            (base, del(points), want.into(), Exact::default())
        }
    }
}

/// A frustum narrowing upward hollowed to walls `t` thick: open through its wide bottom, or closed
/// round a void. Each result is one turned profile, the cone's wall moved in by `t` along its
/// normal. At the wide bottom the nearest point of that wall to a point within it never lies past
/// the rim, so the wall ends flat in the opening with nothing measured to the rim's edge.
#[derive(Clone, Copy)]
pub(super) enum Opened {
    WideBottom,
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
    let feature = |points: Vec<Vec3>| face_act(Act::Shell, points, t, None);
    let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
    let revolve = |lp: Vec<Seg>| Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: Profile { loops: vec![lp] },
        angle: TAU,
    };
    let (want, points) = match opened {
        Opened::WideBottom => {
            let lp = vec![
                line([inner(0.0), 0.0], [bottom, 0.0]),
                line([bottom, 0.0], [top, h]),
                line([top, h], [0.0, h]),
                line([0.0, h], [0.0, h - t]),
                line([0.0, h - t], [inner(h - t), h - t]),
                line([inner(h - t), h - t], [inner(0.0), 0.0]),
            ];
            (revolve(lp), vec![[0.0, 0.0, 0.0]])
        }
        Opened::Closed => {
            let lp = vec![
                line([0.0, 0.0], [bottom, 0.0]),
                line([bottom, 0.0], [top, h]),
                line([top, h], [0.0, h]),
                line([0.0, h], [0.0, h - t]),
                line([0.0, h - t], [inner(h - t), h - t]),
                line([inner(h - t), h - t], [inner(t), t]),
                line([inner(t), t], [0.0, t]),
                line([0.0, t], [0.0, 0.0]),
            ];
            (revolve(lp), vec![])
        }
    };
    (base, feature(points), want.into(), Exact::default())
}

/// The cavity of an L prism shelled open at its step, in the L's plane: the L shrunk by `t` from
/// every face but the step, less the disc of radius `t` about the concave edge where the step meets
/// the wall above it. Running that wall on past the edge would carry it into the L, so a point below
/// the edge is measured to the edge. With `past`, the cavity runs on out through the opening by that
/// much.
fn step_cavity(a: f64, c: f64, h1: f64, h2: f64, t: f64, past: f64) -> Profile {
    let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
    let mut lp = vec![
        line([t, t], [a - t, t]),
        line([a - t, t], [a - t, h1 + past]),
    ];
    if past > 0.0 {
        lp.push(line([a - t, h1 + past], [c + t, h1 + past]));
        lp.push(line([c + t, h1 + past], [c + t, h1]));
    } else {
        lp.push(line([a - t, h1], [c + t, h1]));
    }
    lp.extend([
        Seg::Arc {
            center: [c, h1],
            radius: t,
            start: 0.0,
            sweep: -PI,
        },
        line([c - t, h1], [c - t, h2 - t]),
        line([c - t, h2 - t], [t, h2 - t]),
        line([t, h2 - t], [t, t]),
    ]);
    Profile { loops: vec![lp] }
}

/// The L prism `a` wide, `h1` high at its step and `h2` at its wall, `w` deep, shelled `t` thick
/// open at the step: the shape, its volume and its area.
fn step_shell(a: f64, c: f64, h1: f64, h2: f64, w: f64, t: f64) -> (Primitive, Shape, f64, f64) {
    let ell = prism_xy(
        polygon(&[[0.0, 0.0], [a, 0.0], [a, h1], [c, h1], [c, h2], [0.0, h2]]),
        w,
    );
    let in_frame = |profile: Profile| Primitive::Extrude {
        frame: Frame::at([0.0, 0.0, t]),
        profile,
        height: w - 2.0 * t,
    };
    let cavity = in_frame(step_cavity(a, c, h1, h2, t, 0.0));
    // The opening is in both the L's area and the cavity's, and in neither boundary.
    let opening = (a - c - 2.0 * t) * (w - 2.0 * t);
    let volume = ell.volume() - cavity.volume();
    let area = ell.area() + cavity.area() - 2.0 * opening;
    let through = in_frame(step_cavity(a, c, h1, h2, t, 1.0));
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
    let (base, want, volume, area) = step_shell(a, c, h1, h2, w, t);
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    let feature = face_act(Act::Shell, vec![[(a + c) / 2.0, h1, w / 2.0]], t, None);
    (base, feature, want, exact)
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
    let feature = face_act(Act::Offset, vec![], d, None);
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
        let from = (-reach).patan2(-major);
        Primitive::Revolve {
            frame: Frame::at([0.0; 3]),
            profile: Profile {
                loops: vec![vec![
                    Seg::Arc {
                        center: [major, 0.0],
                        radius: tube,
                        start: from,
                        sweep: TAU - 2.0 * reach.patan2(major),
                    },
                    Seg::Line {
                        a: [0.0, reach],
                        b: [0.0, -reach],
                    },
                ]],
            },
            angle: TAU,
        }
    };
    (base, feature, want.into(), Exact::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_l_prism_shelled_open_at_its_step_has_its_worked_volume_and_area() {
        // 10 wide, the wall 4 wide and 8 high, the step 3 high, 6 deep, walls 1 thick. The cavity
        // is 8 by 2 below the step and 2 by 6 up the wall, 24 less half the disc about the concave
        // edge, and 4 deep; the opening is 4 by 4.
        let (_, _, volume, area) = step_shell(10.0, 4.0, 3.0, 8.0, 6.0, 1.0);
        assert!((volume - (204.0 + 2.0 * PI)).abs() < 1e-9, "{volume}");
        // Outside, 316 less the opening; inside, floor 32, ceiling 8, walls 24, 8 and 16, the
        // lip's half cylinder 4π, and the cavity's two ends 48 − π.
        assert!((area - (436.0 + 3.0 * PI)).abs() < 1e-9, "{area}");
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
    }
}
