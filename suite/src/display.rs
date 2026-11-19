//! B42: what a kernel's [`display`](brep_protocol::Kernel::display) of a solid must be. Its faces
//! together close the solid and bound its volume, every vertex lies on the solid's surface with
//! that surface's outward normal, and its edges are the solid's creases: every vertex on one, every
//! chord within the tolerance of one, every crease covered, and no more length than they have.

use brep_protocol::{Display, DisplayFace, Mesh};

use crate::generate::{Curve3, DisplayExact, Surface};
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

/// How far `q` lies from the whole curve: exact for a line, and for a circle exact too, from its
/// projection onto the circle's plane.
fn to_curve(q: Vec3, c: &Curve3) -> f64 {
    match *c {
        Curve3::Line { a, b } => to_segment(q, a, b),
        Curve3::Circle { c, x, y, r } => {
            let d = sub(q, c);
            let (u, v) = (dot(d, x), dot(d, y));
            let n = cross(x, y);
            let along = dot(d, n);
            let across = (u * u + v * v).sqrt() - r;
            across.hypot(along)
        }
    }
}

/// A curve's length.
fn length(c: &Curve3) -> f64 {
    match *c {
        Curve3::Line { a, b } => gap(a, b),
        Curve3::Circle { r, .. } => std::f64::consts::TAU * r,
    }
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

/// B42's three checks of a display drawn within `tol` of a solid `scale` across: each whether it
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

    // The edges as B38 holds a drawing, in space and with no side hidden.
    let near = |q: Vec3| {
        want.creases
            .iter()
            .map(|c| to_curve(q, c))
            .fold(f64::INFINITY, f64::min)
    };
    let (mut vertex, mut chord, mut drawn) = (0.0f64, 0.0f64, 0.0);
    for line in &d.edges {
        for (i, &q) in line.iter().enumerate() {
            vertex = vertex.max(near(q));
            if let Some(&next) = line.get(i + 1) {
                chord = chord.max(near([0, 1, 2].map(|k| (q[k] + next[k]) / 2.0)));
                drawn += gap(q, next);
            }
        }
    }
    let mut cover = 0.0f64;
    for c in &want.creases {
        let n = 64;
        for j in 0..=n {
            let q = c.at(j as f64 / n as f64);
            let g = d
                .edges
                .iter()
                .flat_map(|l| l.windows(2).map(|w| to_segment(q, w[0], w[1])))
                .fold(f64::INFINITY, f64::min);
            cover = cover.max(g);
        }
    }
    let total: f64 = want.creases.iter().map(length).sum();
    let reach = tol + on;
    let vertex = if d.edges.iter().all(Vec::is_empty) {
        0.0
    } else {
        vertex
    };
    out.push((
        vertex <= on && chord <= reach && cover <= reach && drawn <= total + on,
        format!(
            "edges: vertices off by up to {vertex:.3e}, chords {chord:.3e}, creases uncovered by \
             {cover:.3e}; length {drawn:.6} of {total:.6}"
        ),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Surface;
    use brep_protocol::DisplayFace;

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
            .map(|(q, n)| DisplayFace {
                positions: q.iter().map(|&i| corner(i)).collect(),
                normals: vec![*n; 4],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            })
            .collect();
        let mut creases = Vec::new();
        let mut edges = Vec::new();
        for n in 0..8 {
            for i in 0..3 {
                if n >> i & 1 == 0 {
                    let (a, b) = (corner(n), corner(n | 1 << i));
                    creases.push(Curve3::Line { a, b });
                    edges.push(vec![a, b]);
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
            creases,
            volume: 1.0,
            area: 6.0,
        };
        (Display { faces, edges }, want)
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
        let mut base = DisplayFace {
            positions: vec![[0.0; 3]],
            normals: vec![[0.0, 0.0, -1.0]],
            triangles: vec![],
        };
        let mut side = DisplayFace {
            positions: vec![],
            normals: vec![],
            triangles: vec![],
        };
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
            creases: vec![Curve3::Circle {
                c: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                r: 1.0,
            }],
            volume: std::f64::consts::PI * h / 3.0,
            area: std::f64::consts::PI * (1.0 + (1.0f64 + h * h).sqrt()),
        };
        (
            Display {
                faces: vec![base, side],
                edges: vec![rim],
            },
            want,
        )
    }

    #[test]
    fn a_cone_whose_apex_triangles_each_have_their_own_normal_passes() {
        let (d, want) = cone();
        assert_eq!(held(&d, &want), [true, true, true]);
    }

    #[test]
    fn a_cone_whose_apex_is_one_vertex_or_takes_a_normal_off_its_triangle_fails_the_vertices() {
        let (mut d, want) = cone();
        let side = &mut d.faces[1];
        for t in side.triangles.iter_mut() {
            t[2] = 0;
        }
        side.normals[0] = [0.0, 0.0, 1.0];
        assert_eq!(held(&d, &want), [true, false, true]);
        let (mut d, want) = cone();
        let side = &mut d.faces[1];
        side.normals[0] = side.normals[3 * 32];
        assert_eq!(held(&d, &want), [true, false, true]);
    }

    fn held(d: &Display, want: &DisplayExact) -> Vec<bool> {
        let closed = |m: &Mesh| crate::run::mesh_check(m);
        checks(d, want, 0.01, 1.7, closed)
            .into_iter()
            .map(|c| c.0)
            .collect()
    }

    #[test]
    fn a_true_cube_passes_all_three() {
        let (d, want) = cube();
        assert_eq!(held(&d, &want), [true, true, true]);
    }

    #[test]
    fn a_missing_face_opens_the_mesh() {
        let (mut d, want) = cube();
        d.faces.pop();
        assert_eq!(held(&d, &want), [false, true, true]);
    }

    #[test]
    fn a_normal_pointing_in_fails_the_normals() {
        let (mut d, want) = cube();
        d.faces[2].normals[1] = [0.0, 1.0, 0.0];
        assert_eq!(held(&d, &want), [true, false, true]);
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
        stray.edges.push(vec![[0.5, 0.5, 0.0], [0.5, 0.5, 1.0]]);
        assert!(!held(&stray, &want)[2]);
        // A chord across a crease's corner lies off it by more than the tolerance.
        let mut cut = d.clone();
        cut.edges[0] = vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.0]];
        cut.edges[1] = vec![[0.5, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(!held(&cut, &want)[2]);
    }

    #[test]
    fn a_circle_is_measured_from_its_plane_and_rim() {
        let c = Curve3::Circle {
            c: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            r: 2.0,
        };
        assert!((to_curve([3.0, 0.0, 0.0], &c) - 1.0).abs() < 1e-12);
        assert!((to_curve([0.0, 2.0, 1.0], &c) - 1.0).abs() < 1e-12);
        assert!((to_curve([0.0, 0.0, 0.0], &c) - 2.0).abs() < 1e-12);
    }
}
