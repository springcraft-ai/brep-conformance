//! B42: what a kernel's [`display`](brep_protocol::Kernel::display) of a solid must be. Its faces
//! together close the solid and bound its volume, every vertex lies on the solid's surface with
//! that surface's outward normal, and its edges are those between the solid's surfaces: every
//! vertex on one, every chord within the tolerance of one, every one covered, and no more length
//! than they have. Each edge names the faces it lies on and the turn they make along it, and an edge
//! between two faces on one surface is flagged as a split. Seams are listed apart, and not scored.

use brep_protocol::{Display, DisplayFace, Mesh};

use crate::generate::{Curve3, DisplayExact, Meet, Surface};
use crate::model::Vec3;

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn gap(a: Vec3, b: Vec3) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

/// The middle of a line's middle chord: off its ends, where other edges meet it.
fn middle(line: &[Vec3]) -> Option<Vec3> {
    let i = line.len().checked_sub(1)? / 2;
    let (a, b) = (line[i], *line.get(i + 1).unwrap_or(&line[i]));
    Some([0, 1, 2].map(|k| (a[k] + b[k]) / 2.0))
}

/// Whether an edge lies along a split the case foresees, over its whole length, to within `near`. `edge_split` is optional,
/// so such an edge is judged as a split whether or not the display flags it.
fn on_split(points: &[Vec3], want: &DisplayExact, near: f64) -> bool {
    !points.is_empty()
        && want.splits.iter().any(|m| {
            points.iter().all(|&q| to_meet(q, m) <= near)
                && middle(points).is_some_and(|q| to_meet(q, m) <= near)
        })
}

/// How far `q` lies from the segment from `a` to `b`.
fn to_segment(q: Vec3, a: Vec3, b: Vec3) -> f64 {
    let (ab, aq) = (sub(b, a), sub(q, a));
    let len = dot(ab, ab);
    let t = if len == 0.0 {
        0.0
    } else {
        (dot(aq, ab) / len).clamp(0.0, 1.0)
    };
    gap(q, [0, 1, 2].map(|k| a[k] + t * ab[k]))
}

/// How far `q` lies from the stretch `m`: exact for a line, and for a circle exact too, from its
/// projection onto the circle's plane where that falls within the stretch and from the nearer end
/// where it does not.
fn to_meet(q: Vec3, m: &Meet) -> f64 {
    let ends = || gap(q, m.curve.at(m.t0)).min(gap(q, m.curve.at(m.t1)));
    match m.curve {
        Curve3::Line { .. } => to_segment(q, m.curve.at(m.t0), m.curve.at(m.t1)),
        Curve3::Circle { c, x, y, r } => {
            let d = sub(q, c);
            let (u, v) = (dot(d, x), dot(d, y));
            let t = v.atan2(u) / std::f64::consts::TAU;
            let t = t + (m.t0 - t).ceil();
            if m.t1 - m.t0 < 1.0 && t > m.t1 {
                return ends();
            }
            let along = dot(d, cross(x, y));
            let across = (u * u + v * v).sqrt() - r;
            across.hypot(along)
        }
    }
}

/// A stretch's length.
fn length(m: &Meet) -> f64 {
    (m.t1 - m.t0)
        * match m.curve {
            Curve3::Line { a, b } => gap(a, b),
            Curve3::Circle { r, .. } => std::f64::consts::TAU * r,
        }
}

/// How far `q` lies from the triangle `t`, by the regions of its plane the nearest point falls in
/// (Ericson, Real-Time Collision Detection, 2005, §5.1.5).
fn to_triangle(q: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    let (ab, ac, aq) = (sub(b, a), sub(c, a), sub(q, a));
    let (d1, d2) = (dot(ab, aq), dot(ac, aq));
    if d1 <= 0.0 && d2 <= 0.0 {
        return gap(q, a);
    }
    let bq = sub(q, b);
    let (d3, d4) = (dot(ab, bq), dot(ac, bq));
    if d3 >= 0.0 && d4 <= d3 {
        return gap(q, b);
    }
    let cq = sub(q, c);
    let (d5, d6) = (dot(ab, cq), dot(ac, cq));
    if d6 >= 0.0 && d5 <= d6 {
        return gap(q, c);
    }
    let (va, vb, vc) = (d3 * d6 - d5 * d4, d5 * d2 - d1 * d6, d1 * d4 - d3 * d2);
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return to_segment(q, a, b);
    }
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return to_segment(q, a, c);
    }
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return to_segment(q, b, c);
    }
    let n = cross(ab, ac);
    dot(aq, n).abs() / dot(n, n).sqrt()
}

/// Whether vertex `i` of face `f`, at a point where `s` has no normal, has in each triangle using
/// it a normal `s` has along that triangle: the nearest of the normals across the triangle, halfway
/// out from the vertex, within 1e-3. On a cone those are the normals of the rulings the triangle
/// spans. A triangle of no area runs along nothing, and is passed over.
fn along_each_triangle(f: &DisplayFace, i: usize, s: &Surface, on: f64) -> bool {
    const STEPS: usize = 1024;
    let (q, n) = (f.positions[i], f.normals[i]);
    let fits = |[b, c]: [Vec3; 2]| {
        (0..=STEPS).any(|j| {
            let t = j as f64 / STEPS as f64;
            let p = [0, 1, 2].map(|k| q[k] + ((1.0 - t) * (b[k] - q[k]) + t * (c[k] - q[k])) / 2.0);
            gap(s.near(p).1, n) <= 1e-3
        })
    };
    f.triangles
        .iter()
        .filter_map(|t| {
            let at = t.iter().position(|&j| j as usize == i)?;
            Some([t[(at + 1) % 3], t[(at + 2) % 3]].map(|j| f.positions[j as usize]))
        })
        .filter(|&[b, c]| {
            let n = cross(sub(b, q), sub(c, q));
            dot(n, n).sqrt() > on * on
        })
        .all(fits)
}

/// The faces as one mesh, each face's vertices its own.
fn joined(d: &Display) -> Mesh {
    let mut m = Mesh::default();
    for f in &d.faces {
        let base = m.positions.len() as u32;
        m.positions.extend(&f.positions);
        m.triangles
            .extend(f.triangles.iter().map(|t| t.map(|i| i + base)));
    }
    m
}

/// B42's four checks of a display drawn within `tol` of a solid `scale` across: each whether it
/// held and what it found. `closed` tells whether a mesh closes with every edge used once each way
/// round, and the volume it bounds.
pub(crate) fn checks(
    d: &Display,
    want: &DisplayExact,
    tol: f64,
    scale: f64,
    closed: impl Fn(&Mesh) -> (bool, f64),
) -> Vec<(bool, String)> {
    let on = 1e-6 * scale.max(1.0);
    let mut out = Vec::new();

    let bad_index = d.faces.iter().any(|f| {
        f.normals.len() != f.positions.len()
            || f.triangles
                .iter()
                .flatten()
                .any(|&i| i as usize >= f.positions.len())
    });
    if bad_index {
        out.push((false, "faces: a triangle or normal names no vertex".into()));
        out.push((false, "normals: not checked".into()));
    } else {
        let (shut, volume) = closed(&joined(d));
        let allow = 2.0 * tol * want.area + on;
        let empty = d.faces.iter().filter(|f| f.triangles.is_empty()).count();
        out.push((
            shut && empty == 0 && (volume - want.volume).abs() <= allow,
            format!(
                "faces: {} faces, {empty} bare, mesh {}, volume {volume:.6} against {:.6} ± {allow:.2e}",
                d.faces.len(),
                if shut { "closed" } else { "open" },
                want.volume
            ),
        ));
        // Every vertex lies on a surface of the solid with that surface's normal there, and the
        // normal points the way its triangles face. Single precision is allowed for both.
        let (mut off, mut wrong, mut inward) = (0usize, 0usize, 0usize);
        let near = 1e-5 * scale.max(1.0);
        for f in &d.faces {
            for (i, (q, n)) in f.positions.iter().zip(&f.normals).enumerate() {
                let hits: Vec<_> = want
                    .surfaces
                    .iter()
                    .filter(|s| s.near(*q).0 <= near)
                    .collect();
                let fits = |s: &Surface| match s.apex() {
                    Some(apex) if gap(apex, *q) <= near => along_each_triangle(f, i, s, on),
                    _ => gap(s.near(*q).1, *n) <= 1e-3,
                };
                if hits.is_empty() {
                    off += 1;
                } else if !hits.into_iter().any(fits) {
                    wrong += 1;
                }
            }
            for t in &f.triangles {
                let [a, b, c] = t.map(|i| f.positions[i as usize]);
                let n = cross(sub(b, a), sub(c, a));
                if dot(n, n).sqrt() <= on * on {
                    continue;
                }
                inward += t
                    .iter()
                    .filter(|&&i| dot(n, f.normals[i as usize]) <= 0.0)
                    .count();
            }
        }
        out.push((
            off == 0 && wrong == 0 && inward == 0,
            format!(
                "vertices: {off} off the solid's surfaces, {wrong} with another normal than the \
                 surface's, {inward} normals against their triangles"
            ),
        ));
    }

    // The edges as B38 holds a drawing, in space and with no side hidden. A split, flagged or along
    // one the case foresees, lies between no two surfaces, and is judged with the topology.
    let near = |q: Vec3| {
        want.edges
            .iter()
            .map(|m| to_meet(q, m))
            .fold(f64::INFINITY, f64::min)
    };
    let split_near = 1e-5 * scale.max(1.0);
    let between = || {
        d.edges
            .iter()
            .filter(|e| !e.split && !on_split(&e.points, want, split_near))
            .map(|e| &e.points)
    };
    let (mut vertex, mut chord, mut drawn) = (0.0f64, 0.0f64, 0.0);
    for line in between() {
        for (i, &q) in line.iter().enumerate() {
            vertex = vertex.max(near(q));
            if let Some(&next) = line.get(i + 1) {
                chord = chord.max(near([0, 1, 2].map(|k| (q[k] + next[k]) / 2.0)));
                drawn += gap(q, next);
            }
        }
    }
    let mut cover = 0.0f64;
    for m in &want.edges {
        let n = 64;
        for j in 0..=n {
            let q = m.curve.at(m.t0 + (m.t1 - m.t0) * j as f64 / n as f64);
            let g = between()
                .flat_map(|l| l.windows(2).map(|w| to_segment(q, w[0], w[1])))
                .fold(f64::INFINITY, f64::min);
            cover = cover.max(g);
        }
    }
    let total: f64 = want.edges.iter().map(length).sum();
    let reach = tol + on;
    out.push((
        vertex <= on && chord <= reach && cover <= reach && drawn <= total + on,
        format!(
            "edges: vertices off by up to {vertex:.3e}, chords {chord:.3e}, edges uncovered by \
             {cover:.3e}; length {drawn:.6} of {total:.6}"
        ),
    ));
    out.push(if bad_index {
        (false, "topology: not checked".into())
    } else {
        topology(d, want, tol, scale)
    });
    out
}

/// B42's fourth check: what each edge says of the faces it bounds. Each of these facts is optional,
/// and judged only where the display gives it. An edge lies on each face it names; one between two
/// faces gives the turn the solid's surfaces make there, within 1e-3; an edge flagged as a split
/// lies on one surface, off every edge between two, along a line the solid may be cut on. Seams are
/// not scored. A display that says none of this passes, as not reported.
fn topology(d: &Display, want: &DisplayExact, tol: f64, scale: f64) -> (bool, String) {
    let on = 1e-6 * scale.max(1.0);
    let near = 1e-5 * scale.max(1.0);
    let silent = d
        .edges
        .iter()
        .all(|e| e.faces.is_empty() && e.turn.is_none() && !e.split);
    if silent && !d.edges.is_empty() {
        return (
            true,
            "topology: not reported: no edge names its faces, its turn or a split".into(),
        );
    }
    let surface = |q: Vec3| {
        want.surfaces
            .iter()
            .map(|s| s.near(q).0)
            .fold(f64::INFINITY, f64::min)
    };
    let off = |want: &[Meet], q: Vec3| {
        want.iter()
            .map(|m| to_meet(q, m))
            .fold(f64::INFINITY, f64::min)
    };
    let (mut unnamed, mut astray, mut turns, mut worst) = (0usize, 0usize, 0usize, 0.0f64);
    let (mut splits, mut cuts) = (0usize, 0usize);
    for e in &d.edges {
        let distinct = e.faces.len() == 2 && e.faces[0] != e.faces[1];
        // One face named twice is a seam, which has its own list.
        let itself = e.faces.len() == 2 && !distinct;
        // An edge that names no faces has not said, and is not judged on them.
        let named = !e.faces.is_empty();
        let unknown = e.faces.iter().any(|&f| f as usize >= d.faces.len());
        if named && (e.faces.len() < 2 || itself || unknown) {
            unnamed += 1;
        } else if named {
            let lies = e.faces.iter().all(|&f| {
                let f = &d.faces[f as usize];
                e.points.iter().all(|&q| {
                    f.triangles
                        .iter()
                        .map(|t| to_triangle(q, t.map(|i| f.positions[i as usize])))
                        .fold(f64::INFINITY, f64::min)
                        <= 2.0 * tol + near
                })
            });
            astray += usize::from(!lies);
        }
        let foreseen = on_split(&e.points, want, near);
        if e.split || foreseen {
            cuts += usize::from(e.split);
            let flat = e.turn.is_none_or(|t| t.iter().all(|a| a.abs() <= 1e-3));
            let placed = e.points.iter().all(|&q| surface(q) <= near)
                && middle(&e.points).is_none_or(|q| off(&want.edges, q) > tol + on);
            splits += usize::from(!(flat && placed && (distinct || !named)));
            continue;
        }
        let Some(mid) = middle(&e.points) else {
            continue;
        };
        let meet = want
            .edges
            .iter()
            .min_by(|a, b| to_meet(mid, a).total_cmp(&to_meet(mid, b)));
        // An edge that gives no turn has not said, and is not judged on it.
        match (e.turn, meet) {
            (None, _) => {}
            (Some(t), Some(m)) if distinct || !named => {
                let gap = t.iter().map(|a| (a - m.turn).abs()).fold(0.0, f64::max);
                worst = worst.max(gap);
                turns += usize::from(gap > 1e-3 || gap.is_nan());
            }
            _ => turns += 1,
        }
    }
    (
        unnamed == 0 && astray == 0 && turns == 0 && splits == 0,
        format!(
            "topology: {} edges, {unnamed} naming no two faces, {astray} off a face they name, \
             {turns} with another turn than the surfaces', off by up to {worst:.3e}; {cuts} \
             flagged as splits, {splits} of them wrongly",
            d.edges.len()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Surface;
    use brep_protocol::{DisplayEdge, DisplayFace};
    use std::f64::consts::FRAC_PI_2;

    /// The unit cube's six faces, two triangles each, outward, and its twelve edges.
    fn cube() -> (Display, DisplayExact) {
        let corner = |n: usize| [0, 1, 2].map(|i| (n >> i & 1) as f64);
        // Each face as its four corners counter-clockwise from outside, and its normal.
        let quads: [([usize; 4], Vec3); 6] = [
            ([0, 2, 3, 1], [0.0, 0.0, -1.0]),
            ([4, 5, 7, 6], [0.0, 0.0, 1.0]),
            ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
            ([2, 6, 7, 3], [0.0, 1.0, 0.0]),
            ([0, 4, 6, 2], [-1.0, 0.0, 0.0]),
            ([1, 3, 7, 5], [1.0, 0.0, 0.0]),
        ];
        let faces = quads
            .iter()
            .map(|(q, n)| {
                DisplayFace::new(
                    q.iter().map(|&i| corner(i)).collect(),
                    vec![*n; 4],
                    vec![[0, 1, 2], [0, 2, 3]],
                )
            })
            .collect();
        // Each edge's two faces, by which coordinates stay fixed along it.
        let face_at = |axis: usize, high: bool| (2 * (2 - axis) + usize::from(high)) as u32;
        let mut creases = Vec::new();
        let mut edges = Vec::new();
        for n in 0..8 {
            for i in 0..3 {
                if n >> i & 1 == 0 {
                    let (a, b) = (corner(n), corner(n | 1 << i));
                    creases.push(Meet::whole(Curve3::Line { a, b }, FRAC_PI_2));
                    let [j, k] = [(i + 1) % 3, (i + 2) % 3];
                    let faces = [j, k].map(|x| face_at(x, n >> x & 1 == 1)).to_vec();
                    edges.push(DisplayEdge::new(
                        vec![a, b],
                        faces,
                        Some([FRAC_PI_2; 2]),
                        false,
                    ));
                }
            }
        }
        let surfaces = (0..3)
            .flat_map(|k| {
                let n = |s: f64| {
                    let mut n = [0.0; 3];
                    n[k] = s;
                    n
                };
                [
                    Surface::Plane {
                        p: [0.0; 3],
                        n: n(-1.0),
                    },
                    Surface::Plane {
                        p: [1.0; 3],
                        n: n(1.0),
                    },
                ]
            })
            .collect();
        let want = DisplayExact {
            surfaces,
            edges: creases,
            splits: Vec::new(),
            volume: 1.0,
            area: 6.0,
        };
        (Display::new(faces, edges, Vec::new()), want)
    }

    /// A cone 2 high on the unit disc about the origin, its apex up, in 64 slices, each slice's
    /// triangle with its own vertex at the apex whose normal is the normal along the slice's middle.
    fn cone() -> (Display, DisplayExact) {
        let (n, h) = (64, 2.0);
        let (cos, sin) = (h / (1.0f64 + h * h).sqrt(), 1.0 / (1.0f64 + h * h).sqrt());
        let at = |i: usize| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [a.cos(), a.sin()]
        };
        let ruling = |a: f64| [cos * a.cos(), cos * a.sin(), sin];
        let mut base = DisplayFace::new(vec![[0.0; 3]], vec![[0.0, 0.0, -1.0]], vec![]);
        let mut side = DisplayFace::new(vec![], vec![], vec![]);
        for i in 0..n {
            let ([x0, y0], [x1, y1]) = (at(i), at(i + 1));
            base.positions.push([x0, y0, 0.0]);
            base.normals.push([0.0, 0.0, -1.0]);
            base.triangles
                .push([0, (i + 1) as u32 % n as u32 + 1, i as u32 + 1]);
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            let step = std::f64::consts::TAU / n as f64;
            let k = side.positions.len() as u32;
            side.positions
                .extend([[0.0, 0.0, h], [x0, y0, 0.0], [x1, y1, 0.0]]);
            side.normals
                .extend([ruling(a + step / 2.0), ruling(a), ruling(a + step)]);
            side.triangles.push([k + 1, k + 2, k]);
        }
        let rim: Vec<Vec3> = (0..=n).map(at).map(|[x, y]| [x, y, 0.0]).collect();
        // The base faces down and the side leans in by the cone's half angle past upright.
        let turn = FRAC_PI_2 + (1.0f64 / h).atan();
        let want = DisplayExact {
            surfaces: vec![
                Surface::Plane {
                    p: [0.0; 3],
                    n: [0.0, 0.0, -1.0],
                },
                Surface::Cone {
                    apex: [0.0, 0.0, h],
                    axis: [0.0, 0.0, -1.0],
                    slope: 1.0 / h,
                },
            ],
            edges: vec![Meet::whole(
                Curve3::Circle {
                    c: [0.0; 3],
                    x: [1.0, 0.0, 0.0],
                    y: [0.0, 1.0, 0.0],
                    r: 1.0,
                },
                turn,
            )],
            splits: Vec::new(),
            volume: std::f64::consts::PI * h / 3.0,
            area: std::f64::consts::PI * (1.0 + (1.0f64 + h * h).sqrt()),
        };
        let rim = DisplayEdge::new(rim, vec![0, 1], Some([turn; 2]), false);
        let seam = vec![[1.0, 0.0, 0.0], [0.0, 0.0, h]];
        (Display::new(vec![base, side], vec![rim], vec![seam]), want)
    }

    #[test]
    fn a_cone_whose_apex_triangles_each_have_their_own_normal_passes() {
        let (d, want) = cone();
        assert_eq!(held(&d, &want), [true, true, true, true]);
    }

    #[test]
    fn a_cone_whose_apex_is_one_vertex_or_takes_a_normal_off_its_triangle_fails_the_vertices() {
        let (mut d, want) = cone();
        let side = &mut d.faces[1];
        for t in side.triangles.iter_mut() {
            t[2] = 0;
        }
        side.normals[0] = [0.0, 0.0, 1.0];
        assert_eq!(held(&d, &want), [true, false, true, true]);
        let (mut d, want) = cone();
        let side = &mut d.faces[1];
        side.normals[0] = side.normals[3 * 32];
        assert_eq!(held(&d, &want), [true, false, true, true]);
    }

    fn held(d: &Display, want: &DisplayExact) -> Vec<bool> {
        let closed = |m: &Mesh| crate::run::mesh_check(m);
        checks(d, want, 0.01, 1.7, closed)
            .into_iter()
            .map(|c| c.0)
            .collect()
    }

    #[test]
    fn a_true_cube_passes_all_four() {
        let (d, want) = cube();
        assert_eq!(held(&d, &want), [true, true, true, true]);
    }

    #[test]
    fn a_missing_face_opens_the_mesh() {
        let (mut d, want) = cube();
        d.faces.pop();
        // Its four edges then name a face that is not there.
        assert_eq!(held(&d, &want), [false, true, true, false]);
    }

    #[test]
    fn a_triangle_naming_no_vertex_fails_every_check_of_the_faces_and_is_not_read_further() {
        let (mut d, want) = cube();
        d.faces[0].triangles[0] = [0, 1, 7];
        assert_eq!(held(&d, &want), [false, false, true, false]);
    }

    #[test]
    fn a_normal_pointing_in_fails_the_normals() {
        let (mut d, want) = cube();
        d.faces[2].normals[1] = [0.0, 1.0, 0.0];
        assert_eq!(held(&d, &want), [true, false, true, true]);
    }

    #[test]
    fn a_vertex_off_the_surface_or_a_tilted_normal_fails_the_vertices() {
        let (mut d, want) = cube();
        d.faces[0].positions[0] = [0.0, 0.0, -0.001];
        assert!(!held(&d, &want)[1]);
        let (mut d, want) = cube();
        let s = 0.5f64.sqrt();
        d.faces[0].normals[0] = [0.0, s, -s];
        assert!(!held(&d, &want)[1]);
    }

    #[test]
    fn a_missing_doubled_or_stray_edge_fails_the_edges() {
        let (d, want) = cube();
        let mut missing = d.clone();
        missing.edges.pop();
        assert!(!held(&missing, &want)[2]);
        let mut doubled = d.clone();
        doubled.edges.push(d.edges[0].clone());
        assert!(!held(&doubled, &want)[2]);
        let mut stray = d.clone();
        stray
            .edges
            .push(DisplayEdge::bare(vec![[0.5, 0.5, 0.0], [0.5, 0.5, 1.0]]));
        assert!(!held(&stray, &want)[2]);
        // A chord across a crease's corner lies off it by more than the tolerance.
        let mut cut = d.clone();
        cut.edges[0].points = vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.0]];
        cut.edges[1].points = vec![[0.5, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(!held(&cut, &want)[2]);
    }

    #[test]
    fn a_display_that_says_nothing_of_its_edges_passes_as_not_reported() {
        let (mut d, want) = cube();
        for e in &mut d.edges {
            *e = DisplayEdge::bare(e.points.clone());
        }
        let checks = checks(&d, &want, 0.01, 1.7, crate::run::mesh_check);
        assert_eq!(
            checks.iter().map(|c| c.0).collect::<Vec<_>>(),
            [true, true, true, true]
        );
        assert!(checks[3].1.starts_with("topology: not reported"));
    }

    #[test]
    fn an_edge_that_names_its_faces_and_gives_no_turn_is_judged_on_its_faces_alone() {
        let (mut d, want) = cube();
        for e in &mut d.edges {
            e.turn = None;
        }
        assert_eq!(held(&d, &want), [true, true, true, true]);
        d.edges[0].faces[0] = 5;
        assert!(!held(&d, &want)[3]);
    }

    #[test]
    fn a_wrong_face_a_wrong_turn_or_a_turn_of_the_wrong_sign_fails_the_topology() {
        let (d, want) = cube();
        // The first edge runs along x at y = z = 0: the face at x = 1 only touches its end.
        let mut far = d.clone();
        far.edges[0].faces[0] = 5;
        assert_eq!(held(&far, &want), [true, true, true, false]);
        // One face named twice fails with a turn and without one.
        let mut twice = d.clone();
        twice.edges[0].faces = vec![0, 0];
        assert!(!held(&twice, &want)[3]);
        twice.edges[0].turn = None;
        assert!(!held(&twice, &want)[3]);
        let mut none = d.clone();
        none.edges[0].faces = vec![99, 0];
        assert!(!held(&none, &want)[3]);
        for turn in [Some([FRAC_PI_2, 2.0]), Some([-FRAC_PI_2; 2])] {
            let mut bent = d.clone();
            bent.edges[3].turn = turn;
            assert!(!held(&bent, &want)[3], "{turn:?}");
        }
    }

    /// The cube with its bottom face in two halves along the line x = ½, each half's corners
    /// counter-clockwise from below, and the edge between them.
    fn halved() -> (Display, DisplayExact) {
        let (mut d, mut want) = cube();
        let down = [0.0, 0.0, -1.0];
        let half = |x: f64| {
            let quad = [[x, 0.0], [x, 1.0], [x + 0.5, 1.0], [x + 0.5, 0.0]];
            DisplayFace::new(
                quad.iter().map(|p| [p[0], p[1], 0.0]).collect(),
                vec![down; 4],
                vec![[0, 1, 2], [0, 2, 3]],
            )
        };
        d.faces[0] = half(0.0);
        d.faces.push(half(0.5));
        // The bottom's two edges along x each end at the cut, one piece on each half.
        let (cut, kept): (Vec<_>, Vec<_>) = d.edges.into_iter().partition(|e| {
            let (p, q) = (e.points[0], e.points[1]);
            p[0] != q[0] && p[2] == 0.0
        });
        d.edges = kept;
        // The bottom's edge at x = 1 bounds the far half.
        for e in &mut d.edges {
            if e.points.iter().all(|p| p[0] == 1.0 && p[2] == 0.0) {
                e.faces = e
                    .faces
                    .iter()
                    .map(|&f| if f == 0 { 6 } else { f })
                    .collect();
            }
        }
        for e in cut {
            let (p, q) = (e.points[0], e.points[1]);
            let mid = [0.5, p[1], 0.0];
            for (ends, bottom) in [([p, mid], 0), ([mid, q], 6)] {
                let faces = e.faces.iter().map(|&f| if f == 0 { bottom } else { f });
                d.edges.push(DisplayEdge::new(
                    ends.to_vec(),
                    faces.collect(),
                    e.turn,
                    false,
                ));
            }
        }
        let (a, b) = ([0.5, 0.0, 0.0], [0.5, 1.0, 0.0]);
        d.edges.push(DisplayEdge::new(
            vec![a, b],
            vec![0, 6],
            Some([0.0; 2]),
            true,
        ));
        want.splits.push(Meet::whole(Curve3::Line { a, b }, 0.0));
        (d, want)
    }

    #[test]
    fn an_edge_a_split_made_passes_flagged_or_not_and_fails_misplaced() {
        let (d, want) = halved();
        // The halves' corners at the cut are no corners of the front and back faces, so the joined
        // mesh is open there; the fixture is for the other three checks.
        assert_eq!(held(&d, &want)[1..], [true, true, true]);
        let at = d.edges.len() - 1;
        // `edge_split` is optional: the same edge unflagged is still judged as a split.
        let mut unflagged = d.clone();
        unflagged.edges[at].split = false;
        assert_eq!(held(&unflagged, &want)[2..], [true, true]);
        // A crease is no split, and a split has no turn.
        let mut crease = d.clone();
        crease.edges[0].split = true;
        assert!(!held(&crease, &want)[3]);
        let mut bent = d.clone();
        bent.edges[at].turn = Some([0.0, 0.1]);
        assert!(!held(&bent, &want)[3]);
        // A kernel that merges the halves draws no such edge, and passes.
        let (whole, _) = cube();
        assert_eq!(held(&whole, &want), [true, true, true, true]);
    }

    #[test]
    fn seams_are_not_scored_and_one_drawn_among_the_edges_is_a_stray_edge() {
        let (d, want) = cone();
        let mut bare = d.clone();
        bare.seams.clear();
        assert_eq!(held(&bare, &want), [true, true, true, true]);
        let mut doubled = d.clone();
        doubled.seams.push(d.seams[0].clone());
        assert_eq!(held(&doubled, &want), [true, true, true, true]);
        let mut off = d.clone();
        off.seams[0][1] = [0.0, 0.0, 3.0];
        assert_eq!(held(&off, &want), [true, true, true, true]);
        // A seam drawn among the edges is a stray edge.
        let mut drawn = d.clone();
        drawn.edges.push(DisplayEdge::bare(d.seams[0].clone()));
        assert!(!held(&drawn, &want)[2]);
    }

    #[test]
    fn a_stretch_of_a_circle_is_measured_to_its_ends_beyond_them() {
        let quarter = Meet {
            curve: Curve3::Circle {
                c: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                r: 2.0,
            },
            t0: 1.0,
            t1: 1.25,
            turn: 0.0,
        };
        let s = 2f64.sqrt();
        assert!(to_meet([s, s, 0.0], &quarter).abs() < 1e-12);
        assert!((to_meet([-2.0, 0.0, 0.0], &quarter) - 2.0 * s).abs() < 1e-12);
        assert!((to_meet([2.0, -1.0, 0.0], &quarter) - 1.0).abs() < 1e-12);
        assert!((length(&quarter) - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn a_point_is_measured_to_a_triangle_s_face_side_or_corner() {
        let t = [[0.0; 3], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        assert!((to_triangle([0.5, 0.5, 3.0], t) - 3.0).abs() < 1e-12);
        assert!((to_triangle([1.0, -1.0, 0.0], t) - 1.0).abs() < 1e-12);
        assert!((to_triangle([3.0, 0.0, 4.0], t) - 17f64.sqrt()).abs() < 1e-12);
        assert!((to_triangle([2.0, 2.0, 0.0], t) - 2f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn a_circle_is_measured_from_its_plane_and_rim() {
        let c = Meet::whole(
            Curve3::Circle {
                c: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                r: 2.0,
            },
            0.0,
        );
        assert!((to_meet([3.0, 0.0, 0.0], &c) - 1.0).abs() < 1e-12);
        assert!((to_meet([0.0, 2.0, 1.0], &c) - 1.0).abs() < 1e-12);
        assert!((to_meet([0.0, 0.0, 0.0], &c) - 2.0).abs() < 1e-12);
    }
}
