//! The suite's own shapes as triangle meshes, for the report to draw. Each primitive's surfaces are
//! tessellated in their own parameters, so every vertex lies on the exact surface, and trimmed by
//! the shape's boolean tree: a point on leaf `L`'s surface is on the shape's boundary exactly when
//! the tree's membership differs with `L` taken as solid and as empty, and the material lies on
//! the side `L`'s inside does when it is solid there. Trim curves are found by bisection on that
//! test in the surface's parameters. Built to WebAssembly, it is called with a shape's JSON and
//! returns positions and triangles in a buffer the page reads.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use brep_conformance::model::{Frame, Op, Primitive, Shape, Vec3};
use brep_conformance::oracle;
use brep_conformance::profile::{Profile, Seg};
use brep_protocol::path::PathSweep;
use serde_json::Value;

/// The shapes balanced into a tree of `op`, so hundreds of them nest only as deep as their log.
fn balanced(op: Op, mut v: Vec<Shape>) -> Option<Shape> {
    match v.len() {
        0 => None,
        1 => v.pop(),
        n => {
            let right = v.split_off(n / 2);
            Some(Shape::Bool(
                op,
                Box::new(balanced(op, v)?),
                Box::new(balanced(op, right)?),
            ))
        }
    }
}

/// The shape a case's JSON describes: a primitive, a boolean of two shapes, or `many`, a target
/// less every tool or the union of the tools.
pub fn shape(v: &Value) -> Result<Shape, String> {
    if v["kind"] == "many" {
        let tools = v["tools"]
            .as_array()
            .ok_or("`tools` is not an array")?
            .iter()
            .map(shape)
            .collect::<Result<Vec<_>, _>>()?;
        let union = balanced(Op::Union, tools).ok_or("no tools")?;
        return Ok(if v["target"].is_null() {
            union
        } else {
            Shape::Bool(
                Op::Difference,
                Box::new(shape(&v["target"])?),
                Box::new(union),
            )
        });
    }
    if v["kind"] == "boolean" {
        let op = match v["op"].as_str() {
            Some("union") => Op::Union,
            Some("intersection") => Op::Intersection,
            Some("difference") => Op::Difference,
            o => return Err(format!("unknown op {o:?}")),
        };
        return Ok(Shape::Bool(
            op,
            Box::new(shape(&v["a"])?),
            Box::new(shape(&v["b"])?),
        ));
    }
    Primitive::from_json(v).map(Shape::Prim)
}

/// A mesh: vertex positions and triangles by vertex index, wound counter-clockwise from outside.
#[derive(Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    /// Each vertex's normal, from its surface, facing out of the shape.
    pub normals: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// The shape's edges as segments: wherever two faces meet, at an angle or tangentially, as
    /// CAD draws them; not where a surface meets itself at its own seam.
    pub edges: Vec<[[f32; 3]; 2]>,
    /// For each of `edges`, the signed angle between its two faces' normals there, in radians,
    /// positive where the edge is convex; NaN where the edge has not two faces to compare.
    pub turns: Vec<f32>,
    /// The face each triangle lies on, numbered by surface.
    pub faces: Vec<u32>,
    /// Mesh edges that bound a surface's kept part: a trim, or the surface's own border. Only
    /// these may be shape edges where one triangle uses them; the rest are the inner sides of a
    /// split triangle beside an unsplit one.
    pub bounds: Vec<[u32; 2]>,
}

/// The boolean tree over numbered leaves.
enum Node {
    Leaf(usize),
    Bool(Op, Box<Node>, Box<Node>),
}

/// The tree of `s` and its leaves, a sweep of straight runs and bends split into its pieces so the
/// caps where they join drop out as inside the union.
fn tree(s: &Shape, leaves: &mut Vec<Shape>) -> Node {
    match s {
        Shape::Bool(op, a, b) => {
            Node::Bool(*op, Box::new(tree(a, leaves)), Box::new(tree(b, leaves)))
        }
        Shape::Prim(p @ Primitive::Sweep { .. }) => {
            let mut pieces = p.pieces().into_iter();
            let first = pieces
                .next()
                .map(Shape::Prim)
                .unwrap_or_else(|| Shape::Prim(p.clone()));
            let mut node = tree(&first, leaves);
            for q in pieces {
                node = Node::Bool(
                    Op::Union,
                    Box::new(node),
                    Box::new(tree(&Shape::Prim(q), leaves)),
                );
            }
            node
        }
        Shape::Prim(_) => {
            leaves.push(s.clone());
            Node::Leaf(leaves.len() - 1)
        }
    }
}

/// Membership of `p` in the tree, leaf `forced.0` taken as solid or empty by `forced.1`; a point
/// outside a leaf's box is outside the leaf without asking.
fn inside(n: &Node, leaves: &[(Shape, oracle::Bounds)], p: Vec3, forced: (&[usize], bool)) -> bool {
    match n {
        Node::Leaf(i) if forced.0.contains(i) => forced.1,
        Node::Leaf(i) => {
            let (s, (lo, hi)) = &leaves[*i];
            (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]) && oracle::contains(s, p)
        }
        Node::Bool(op, a, b) => {
            let x = inside(a, leaves, p, forced);
            match op {
                Op::Union => x || inside(b, leaves, p, forced),
                Op::Intersection => x && inside(b, leaves, p, forced),
                Op::Difference => x && !inside(b, leaves, p, forced),
            }
        }
    }
}

type Map = Box<dyn Fn(f64, f64) -> Vec3>;
type Valid = Box<dyn Fn(f64, f64) -> bool>;

/// One surface of a leaf over `[0, 1]²`, with where it is part of the leaf when not all of it is.
struct Patch {
    at: Map,
    valid: Option<Valid>,
    /// The widest a grid cell may be, in the leaf's lengths, where the trim follows curves finer
    /// than the shape: a cell wider than a round in its outline would cover the round's corner.
    fine: Option<f64>,
}

fn world(f: &Frame, l: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| f.origin[k] + f.x[k] * l[0] + f.y[k] * l[1] + f.z[k] * l[2])
}

/// `pt` over only the part of its parameters near the box `lo`–`hi` the shape lies in, or `None`
/// where none of it comes near. A half-space's face, ten thousand across, would otherwise spread
/// its grid over all of that and leave the shape a cell or two. Sampled on a 64 × 64 grid,
/// narrowed to the samples within the box padded by `cell` and a sample step either side, and
/// sampled again inside that until it stops shrinking.
fn clipped(pt: Patch, (lo, hi): oracle::Bounds, cell: f64) -> Option<Patch> {
    const N: usize = 64;
    let pad = 2.0 * cell;
    let inside = |p: Vec3| (0..3).all(|k| p[k] >= lo[k] - pad && p[k] <= hi[k] + pad);
    let (mut u0, mut u1, mut v0, mut v1) = (0.0f64, 1.0f64, 0.0f64, 1.0f64);
    for _ in 0..6 {
        let (du, dv) = ((u1 - u0) / N as f64, (v1 - v0) / N as f64);
        let (mut i0, mut i1, mut j0, mut j1) = (N, 0, N, 0);
        for i in 0..=N {
            for j in 0..=N {
                if inside((pt.at)(u0 + du * i as f64, v0 + dv * j as f64)) {
                    (i0, i1, j0, j1) = (i0.min(i), i1.max(i), j0.min(j), j1.max(j));
                }
            }
        }
        if i0 > i1 {
            // Nothing sampled near the box: on the first pass the patch passes it by; later,
            // the box fell between samples of a range already narrowed round it, so keep that.
            if (u0, u1, v0, v1) == (0.0, 1.0, 0.0, 1.0) {
                return None;
            }
            break;
        }
        let (nu0, nu1) = (
            u0 + du * i0.saturating_sub(1) as f64,
            u0 + du * (i1 + 1).min(N) as f64,
        );
        let (nv0, nv1) = (
            v0 + dv * j0.saturating_sub(1) as f64,
            v0 + dv * (j1 + 1).min(N) as f64,
        );
        let shrunk = (nu1 - nu0) < 0.5 * (u1 - u0) || (nv1 - nv0) < 0.5 * (v1 - v0);
        (u0, u1, v0, v1) = (nu0, nu1, nv0, nv1);
        if !shrunk {
            break;
        }
    }
    if (u0, u1, v0, v1) == (0.0, 1.0, 0.0, 1.0) {
        return Some(pt);
    }
    let (du, dv) = (u1 - u0, v1 - v0);
    let at = pt.at;
    Some(Patch {
        at: Box::new(move |u, v| at(u0 + du * u, v0 + dv * v)),
        valid: pt
            .valid
            .map(|ok| Box::new(move |u, v| ok(u0 + du * u, v0 + dv * v)) as Valid),
        fine: pt.fine,
    })
}

fn patch(f: Frame, g: impl Fn(f64, f64) -> Vec3 + 'static) -> Patch {
    Patch {
        at: Box::new(move |u, v| world(&f, g(u, v))),
        valid: None,
        fine: None,
    }
}

fn trimmed(
    f: Frame,
    g: impl Fn(f64, f64) -> Vec3 + 'static,
    ok: impl Fn(f64, f64) -> bool + 'static,
) -> Patch {
    Patch {
        at: Box::new(move |u, v| world(&f, g(u, v))),
        valid: Some(Box::new(ok)),
        fine: None,
    }
}

/// A profile's region in a plane: the rectangle round it, kept where the profile holds the point.
/// `place` puts a profile point `(x, y)` in the leaf's coordinates.
fn region(f: Frame, p: &Profile, place: impl Fn(f64, f64) -> Vec3 + 'static) -> Patch {
    let (lo, hi) = p.bounds();
    let q = p.clone();
    let xy = move |u: f64, v: f64| (lo[0] + u * (hi[0] - lo[0]), lo[1] + v * (hi[1] - lo[1]));
    // A quarter of the outline's tightest round, or a sixteenth of a curve's length: a grid that
    // fine cuts the trim at least a few times along every curve of it.
    let fine = p
        .loops
        .iter()
        .flatten()
        .filter_map(|s| match s {
            Seg::Line { .. } => None,
            Seg::Arc { radius, .. } => Some(0.25 * radius),
            other => Some(other.length() / 16.0),
        })
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))));
    let mut out = trimmed(
        f,
        move |u, v| {
            let (x, y) = xy(u, v);
            place(x, y)
        },
        move |u, v| {
            let (x, y) = xy(u, v);
            q.contains([x, y])
        },
    );
    out.fine = fine;
    out
}

/// Each segment of a profile swept by `g(point, v)`.
fn sides(f: Frame, p: &Profile, g: impl Fn([f64; 2], f64) -> Vec3 + Clone + 'static) -> Vec<Patch> {
    p.loops
        .iter()
        .flatten()
        .map(|s| {
            let (s, g) = (s.clone(), g.clone());
            patch(f, move |u, v| g(s.eval(u).0, v))
        })
        .collect()
}

/// The surfaces of a primitive.
fn patches(p: &Primitive) -> Vec<Patch> {
    let f = p.frame();
    match p.clone() {
        Primitive::Box { size: s, .. } => (0..6)
            .map(|k| {
                let (a, top) = (k / 2, k % 2 == 1);
                let (b, c) = ((a + 1) % 3, (a + 2) % 3);
                patch(f, move |u, v| {
                    let mut l = [0.0; 3];
                    l[a] = if top { s[a] } else { 0.0 };
                    l[b] = u * s[b];
                    l[c] = v * s[c];
                    l
                })
            })
            .collect(),
        Primitive::Cylinder { radius, height, .. } => frustum(f, radius, radius, height),
        Primitive::Cone {
            bottom,
            top,
            height,
            ..
        } => frustum(f, bottom, top, height),
        Primitive::Sphere { radius: r, .. } => vec![patch(f, move |u, v| {
            let (t, ph) = (TAU * u, PI * (v - 0.5));
            [r * ph.cos() * t.cos(), r * ph.cos() * t.sin(), r * ph.sin()]
        })],
        Primitive::Torus { major, minor, .. } => vec![patch(f, move |u, v| {
            let (t, ph) = (TAU * u, TAU * v);
            let rho = major + minor * ph.cos();
            [rho * t.cos(), rho * t.sin(), minor * ph.sin()]
        })],
        Primitive::Extrude {
            profile, height, ..
        } => {
            let mut v = sides(f, &profile, move |q, w| [q[0], q[1], w * height]);
            v.push(region(f, &profile, |x, y| [x, y, 0.0]));
            v.push(region(f, &profile, move |x, y| [x, y, height]));
            v
        }
        Primitive::Revolve { profile, angle, .. } => {
            let turn = move |x: f64, y: f64, t: f64| [x * t.cos(), x * t.sin(), y];
            let mut v = sides(f, &profile, move |q, w| turn(q[0], q[1], angle * w));
            if angle < TAU - 1e-9 {
                v.push(region(f, &profile, move |x, y| turn(x, y, 0.0)));
                v.push(region(f, &profile, move |x, y| turn(x, y, angle)));
            }
            v
        }
        Primitive::Screw {
            profile,
            rise,
            angle,
            ..
        } => {
            let turn = move |x: f64, y: f64, t: f64| [x * t.cos(), x * t.sin(), y + rise * t];
            let mut v = sides(f, &profile, move |q, w| turn(q[0], q[1], angle * w));
            v.push(region(f, &profile, move |x, y| turn(x, y, 0.0)));
            v.push(region(f, &profile, move |x, y| turn(x, y, angle)));
            v
        }
        Primitive::Loft {
            profile,
            scale,
            height,
            ..
        } => {
            let mut v = sides(f, &profile, move |q, w| {
                let k = 1.0 + (scale - 1.0) * w;
                [q[0] * k, q[1] * k, height * w]
            });
            v.push(region(f, &profile, |x, y| [x, y, 0.0]));
            v.push(region(f, &profile, move |x, y| {
                [x * scale, y * scale, height]
            }));
            v
        }
        Primitive::Slab { size, .. } => {
            let field =
                |p: &Primitive, u: f64, v: f64| p.height_field().expect("a slab").eval(u, v).0;
            let top = p.clone();
            let (a, b) = (size[0], size[1]);
            let tall = match p {
                Primitive::Slab { heights, .. } => {
                    heights.iter().flatten().copied().fold(0.0, f64::max)
                }
                _ => 0.0,
            };
            let mut v = vec![
                patch(f, move |u, w| [a * u, b * w, field(&top, u, w)]),
                patch(f, move |u, w| [a * u, b * w, 0.0]),
            ];
            // Each side up to the top's edge above it: (u along the side, w up to the tallest).
            for side in 0..4 {
                let z = p.clone();
                let at = move |u: f64| match side {
                    0 => (u, 0.0),
                    1 => (1.0, u),
                    2 => (u, 1.0),
                    _ => (0.0, u),
                };
                v.push(trimmed(
                    f,
                    move |u, w| {
                        let (x, y) = at(u);
                        [a * x, b * y, w * tall]
                    },
                    move |u, w| {
                        let (x, y) = at(u);
                        w * tall <= field(&z, x, y)
                    },
                ));
            }
            v
        }
        Primitive::SweepPath {
            profile,
            path,
            twist,
            ..
        } => {
            let sweep = std::rc::Rc::new(PathSweep::new(&path, &profile, twist));
            let (u0, u1) = path.domain();
            let along = move |w: f64| u0 + w * (u1 - u0);
            let mut v: Vec<Patch> = profile
                .loops
                .iter()
                .flatten()
                .map(|s| {
                    let (s, sw) = (s.clone(), sweep.clone());
                    patch(f, move |u, w| {
                        let q = s.eval(u).0;
                        let (p, e1, e2) = sw.frame_at(along(w));
                        [0, 1, 2].map(|k| p[k] + q[0] * e1[k] + q[1] * e2[k])
                    })
                })
                .collect();
            for end in [u0, u1] {
                let sw = sweep.clone();
                v.push(region(f, &profile, move |x, y| {
                    let (p, e1, e2) = sw.frame_at(end);
                    [0, 1, 2].map(|k| p[k] + x * e1[k] + y * e2[k])
                }));
            }
            v
        }
        Primitive::Sweep { .. } => Vec::new(),
    }
}

fn frustum(f: Frame, bottom: f64, top: f64, height: f64) -> Vec<Patch> {
    let mut v = vec![patch(f, move |u, w| {
        let (t, r) = (TAU * u, bottom + (top - bottom) * w);
        [r * t.cos(), r * t.sin(), height * w]
    })];
    for (r, z) in [(bottom, 0.0), (top, height)] {
        if r > 0.0 {
            v.push(patch(f, move |u, w| {
                let t = TAU * w;
                [r * u * t.cos(), r * u * t.sin(), z]
            }));
        }
    }
    v
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `s`'s boundary, each surface tessellated into cells about `cell` across.
pub fn mesh(s: &Shape, cell: f64) -> Mesh {
    mesh_of(s, cell, |_| true)
}

/// The mesh of a shape's JSON, where a boolean of op `outside` is [`mesh_outside`] of its operands.
pub fn mesh_value(v: &Value, cell: f64) -> Result<Mesh, String> {
    if v["kind"] == "boolean" && v["op"] == "outside" {
        Ok(mesh_outside(&shape(&v["a"])?, &shape(&v["b"])?, cell))
    } else {
        shape(v).map(|s| mesh(&s, cell))
    }
}

/// The part of `a`'s boundary outside `b`: what `b` takes away from `a`, without `b`'s own faces.
/// A ghost of a shape before an act drawn this way ends where the result's faces leave it, so
/// nothing of it lies on the result's surface to fight it for depth.
pub fn mesh_outside(a: &Shape, b: &Shape, cell: f64) -> Mesh {
    let mut own = Vec::new();
    tree(a, &mut own);
    let own: std::collections::HashSet<_> = own.iter().map(Shape::key).collect();
    let d = Shape::Bool(Op::Difference, Box::new(a.clone()), Box::new(b.clone()));
    mesh_of(&d, cell, |leaf| own.contains(&leaf.key()))
}

/// `s`'s boundary on the leaves `keep` picks.
fn mesh_of(s: &Shape, cell: f64, keep: impl Fn(&Shape) -> bool) -> Mesh {
    let mut shapes = Vec::new();
    let root = tree(s, &mut shapes);
    let Some((lo, hi)) = oracle::bounds(s) else {
        return Mesh::default();
    };
    let diag = dist(lo, hi).max(1e-9);
    let cell = if cell > 0.0 { cell } else { diag / 160.0 };
    // Each leaf with its box, padded a little so a point on its boundary is not cut off.
    let leaves: Vec<(Shape, oracle::Bounds)> = shapes
        .into_iter()
        .map(|l| {
            let (lo, hi) =
                oracle::bounds(&l).unwrap_or(([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]));
            let pad = 1e-9 * diag;
            (l, (lo.map(|x| x - pad), hi.map(|x| x + pad)))
        })
        .collect();
    let mut out = Mesh::default();
    // A leaf the tree holds twice bounds the shape once: meshing both copies doubles every
    // triangle, and each edge they share reads as a crease.
    let mut seen = std::collections::HashSet::new();
    let mut face = 0u32;
    for (li, (leaf, _)) in leaves.iter().enumerate() {
        let Shape::Prim(prim) = leaf else { continue };
        if !seen.insert(leaf.key()) || !keep(leaf) {
            continue;
        }
        for pt in patches(prim) {
            let Some(pt) = clipped(pt, (lo, hi), cell) else {
                continue;
            };
            tessellate(&pt, li, leaf, &root, &leaves, cell, diag, &mut out);
            out.faces.resize(out.triangles.len(), face);
            face += 1;
        }
    }
    finish(&mut out, cell);
    out
}

/// Fills the normals a surface left zero (a pole, a disc's centre) from the triangles round them,
/// and finds the edges: welding vertices that coincide, a mesh edge is a shape edge where it
/// bounds one triangle (a trim, or where two surfaces are sampled apart) or where the surface
/// normals of its two triangles part by more than 25°. A seam or a tangent joint, whose normals
/// agree, is no edge.
fn finish(m: &mut Mesh, cell: f64) {
    let q = (cell * 1e-3).max(1e-12);
    let mut weld: HashMap<[i64; 3], u32> = HashMap::new();
    let ids: Vec<u32> = m
        .positions
        .iter()
        .map(|x| {
            let key = x.map(|c| (f64::from(c) / q).round() as i64);
            let next = weld.len() as u32;
            *weld.entry(key).or_insert(next)
        })
        .collect();
    // A vanishing normal comes from every triangle round its point, so the copies of a pole or
    // a disc's centre agree.
    let p = |i: u32| m.positions[i as usize].map(f64::from);
    let mut acc = vec![[0.0f64; 3]; weld.len()];
    for t in &m.triangles {
        let [a, b, c] = t.map(p);
        let (u, v) = (
            [0, 1, 2].map(|k| b[k] - a[k]),
            [0, 1, 2].map(|k| c[k] - a[k]),
        );
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &i in t {
            for k in 0..3 {
                acc[ids[i as usize] as usize][k] += n[k];
            }
        }
    }
    for (n, &id) in m.normals.iter_mut().zip(&ids) {
        if *n == [0.0; 3] {
            let a = acc[id as usize];
            let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
            *n = a.map(|x| (x / l) as f32);
        }
    }
    // Each welded side's uses: its two vertices as the triangle has them, and the face.
    type Uses = Vec<(u32, u32, u32)>;
    let mut sides: HashMap<(u32, u32), Uses> = HashMap::new();
    // The third corner of the triangle each use belongs to, which lies on the use's face.
    let mut third: HashMap<(u32, u32, u32), u32> = HashMap::new();
    for (t, face) in m.triangles.iter().zip(&m.faces) {
        // A triangle two of whose corners weld into one, as at a pole, has no area and no say.
        let w = t.map(|i| ids[i as usize]);
        if w[0] == w[1] || w[1] == w[2] || w[2] == w[0] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let (wa, wb) = (ids[a as usize], ids[b as usize]);
            if wa == wb {
                continue;
            }
            let key = if wa < wb { (wa, wb) } else { (wb, wa) };
            let (ea, eb) = if wa < wb { (a, b) } else { (b, a) };
            sides.entry(key).or_default().push((ea, eb, *face));
            third.insert((ea, eb, *face), t[(k + 2) % 3]);
        }
    }
    let pair = |a: u32, b: u32| {
        let (wa, wb) = (ids[a as usize], ids[b as usize]);
        if wa < wb { (wa, wb) } else { (wb, wa) }
    };
    let marked: std::collections::HashSet<(u32, u32)> =
        m.bounds.iter().map(|&[a, b]| pair(a, b)).collect();
    // Where two faces meet is an edge, at any angle or none, as CAD draws one. Within one face a
    // crease is the mesh's, never the shape's, unless it is sharp; and a face meeting itself at a
    // seam is no edge where it is smooth there, its normals the same to the precision they are
    // stored at.
    let cos = 25f64.to_radians().cos() as f32;
    let tangent = 1f64.to_radians().cos() as f32;
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut keys: Vec<&(u32, u32)> = sides.keys().collect();
    keys.sort_unstable();
    // Two faces trimmed along the same curve, or sampled apart along the curve they meet on, as
    // a cone and a cylinder whose grids step differently round their shared circle, each bound
    // the other's border with no mesh edge shared. Such a border lying on another face's is one
    // edge, drawn once, from the lower-numbered face.
    let singles: Vec<(u32, u32, u32)> = keys
        .iter()
        .filter(|k| sides[k].len() == 1 && marked.contains(k))
        .map(|k| sides[k][0])
        .collect();
    let index: HashMap<(u32, u32, u32), usize> =
        singles.iter().enumerate().map(|(i, &t)| (t, i)).collect();
    let h = cell.max(1e-9);
    let at = |x: [f64; 3]| x.map(|c| (c / h).floor() as i64);
    let mid = |a: u32, b: u32| {
        let (pa, pb) = (p(a), p(b));
        [0, 1, 2].map(|k| 0.5 * (pa[k] + pb[k]))
    };
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, &(a, b, _)) in singles.iter().enumerate() {
        for x in [p(a), p(b), mid(a, b)] {
            let cell = grid.entry(at(x)).or_default();
            if cell.last() != Some(&i) {
                cell.push(i);
            }
        }
    }
    // How far `x` lies from the segment `a`–`b`.
    let off = |x: [f64; 3], a: [f64; 3], b: [f64; 3]| {
        let d = [0, 1, 2].map(|k| b[k] - a[k]);
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let t = if dd > 0.0 {
            ([0, 1, 2].map(|k| (x[k] - a[k]) * d[k]).iter().sum::<f64>() / dd).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let y = [0, 1, 2].map(|k| a[k] + t * d[k] - x[k]);
        (y[0] * y[0] + y[1] * y[1] + y[2] * y[2]).sqrt()
    };
    let len = |e: [f64; 3]| (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
    // Each single border's partner on another face, if it has one.
    let partner = |i: usize| -> Option<usize> {
        let (ia, ib, iface) = singles[i];
        let x = mid(ia, ib);
        let ei = [0, 1, 2].map(|k| p(ib)[k] - p(ia)[k]);
        let c = at(x);
        (-1..=1)
            .flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [dx, dy, dz])))
            .filter_map(|d| grid.get(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]]))
            .flatten()
            .copied()
            .filter(|&j| {
                // The two borders are chords of one curve, each sagging off it by up to its
                // turn per step: a twentieth of the longer chord covers both. Either one's middle
                // lying on the other pairs them, as a long chord's middle sags off a short one.
                let (ja, jb, jface) = singles[j];
                let ej = [0, 1, 2].map(|k| p(jb)[k] - p(ja)[k]);
                let along = (ei[0] * ej[0] + ei[1] * ej[1] + ei[2] * ej[2]).abs()
                    / (len(ei) * len(ej)).max(1e-300);
                let tol = 10.0 * q + 0.05 * len(ei).max(len(ej));
                j != i
                    && (jface != iface || (ja, jb) != (ia, ib))
                    && along > 10f64.to_radians().cos()
                    && (off(x, p(ja), p(jb)) < tol || off(mid(ja, jb), p(ia), p(ib)) < tol)
            })
            // Another face's border before the face's own; of those, the lowest face's.
            .min_by_key(|&j| (singles[j].2 == iface, singles[j].2))
    };
    // The angle from the normal `n1` of the face that `use1` lies on to `n2`, the other face's:
    // convex where the first face falls below the second's tangent plane.
    let turn = |use1: (u32, u32, u32), n1: [f32; 3], n2: [f32; 3]| -> f32 {
        let angle = dot(n1, n2).clamp(-1.0, 1.0).acos();
        let Some(&c) = third.get(&use1) else {
            return f32::NAN;
        };
        let (a, c) = (p(use1.0), p(c));
        let into: f64 = (0..3).map(|k| (c[k] - a[k]) * f64::from(n2[k])).sum();
        if into > 0.0 { -angle } else { angle }
    };
    for key in keys {
        let s = &sides[key];
        let n = |i: &u32| m.normals[*i as usize];
        let (a, b, t) = match s.as_slice() {
            [(a1, b1, f1), (a2, b2, f2)] => {
                let crease = dot(n(a1), n(a2)) < cos || dot(n(b1), n(b2)) < cos;
                if f1 == f2 && !crease {
                    continue;
                }
                (*a1, *b1, turn((*a1, *b1, *f1), n(a1), n(a2)))
            }
            [(a, b, face)] => {
                if !marked.contains(key) {
                    continue;
                }
                let i = index[&(*a, *b, *face)];
                // The partner is sampled apart from this border, so its normal here is read
                // between its two ends, where this border's start falls along it.
                let with = |j: usize| {
                    let (ja, jb, _) = singles[j];
                    let (o, d) = (p(ja), [0, 1, 2].map(|k| p(jb)[k] - p(ja)[k]));
                    let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                    let along: f64 = (0..3).map(|k| (p(*a)[k] - o[k]) * d[k]).sum();
                    let t = if dd > 0.0 {
                        (along / dd).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };
                    let (n0, n1) = (m.normals[ja as usize], m.normals[jb as usize]);
                    let mix = [0, 1, 2].map(|k| n0[k] + t * (n1[k] - n0[k]));
                    let len = dot(mix, mix).sqrt().max(1e-30);
                    turn((*a, *b, *face), n(a), mix.map(|x| x / len))
                };
                match partner(i) {
                    // A surface meeting itself, at a seam its two sides were sampled apart
                    // along: no edge where it is smooth, a crease drawn once where not.
                    Some(j) if singles[j].2 == *face => {
                        if dot(n(a), m.normals[singles[j].0 as usize]) >= tangent || j < i {
                            continue;
                        }
                        (*a, *b, with(j))
                    }
                    Some(j) if singles[j].2 < *face => continue,
                    Some(j) => (*a, *b, with(j)),
                    None => (*a, *b, f32::NAN),
                }
            }
            _ => (s[0].0, s[0].1, f32::NAN),
        };
        m.edges
            .push([m.positions[a as usize], m.positions[b as usize]]);
        m.turns.push(t);
    }
}

/// One patch's kept part: a grid over its parameters, triangles where the patch lies on the
/// shape's boundary, cut along the trim by bisection where a triangle straddles it.
#[allow(clippy::too_many_arguments)]
fn tessellate(
    pt: &Patch,
    li: usize,
    leaf: &Shape,
    root: &Node,
    leaves: &[(Shape, oracle::Bounds)],
    cell: f64,
    diag: f64,
    out: &mut Mesh,
) {
    // Grid lines placed by the patch's length and turning along each parameter, sampled on a few
    // lines: a cell at most `cell` long, and at most `TURN` of turning to a step, so a fillet small
    // beside the part still reads round. Spacing them evenly in that measure, not in the
    // parameter, keeps a blend whose curvature ramps up fast from its contact from crowding its
    // whole ramp into one row, where normals blended across the row would show a crease.
    const TURN: f64 = 3.0 * PI / 180.0;
    const SAMPLES: usize = 256;
    let cell = pt.fine.map_or(cell, |f| cell.min(f));
    let grid = |along_u: bool| -> Vec<f64> {
        let mut best: Option<Vec<f64>> = None;
        for &c in &[0.0, 0.5, 1.0] {
            let mut acc = vec![0.0f64; SAMPLES + 1];
            let (mut prev, mut dir): (Option<Vec3>, Option<Vec3>) = (None, None);
            for k in 0..=SAMPLES {
                let t = k as f64 / SAMPLES as f64;
                let p = if along_u {
                    (pt.at)(t, c)
                } else {
                    (pt.at)(c, t)
                };
                // Each step weighs the larger of its length in cells and its turn in steps of
                // `TURN`, not their sum, which would row a round twice over.
                let mut w = 0.0;
                if let Some(q) = prev {
                    let step = dist(p, q);
                    w = step / cell;
                    if step > 0.0 {
                        let d = [0, 1, 2].map(|k| (p[k] - q[k]) / step);
                        if let Some(e) = dir {
                            let cos = (d[0] * e[0] + d[1] * e[1] + d[2] * e[2]).clamp(-1.0, 1.0);
                            w = w.max(cos.acos() / TURN);
                        }
                        dir = Some(d);
                    }
                }
                if k > 0 {
                    acc[k] = acc[k - 1] + w;
                }
                prev = Some(p);
            }
            if best.as_ref().is_none_or(|b| acc[SAMPLES] > b[SAMPLES]) {
                best = Some(acc);
            }
        }
        let acc = best.unwrap();
        let total = acc[SAMPLES];
        let n = (total.ceil() as usize).clamp(2, 600);
        if !total.is_finite() || total <= 0.0 {
            return (0..=n).map(|i| i as f64 / n as f64).collect();
        }
        // Each measure's step mixed half and half with an even one, so no row is starved.
        let mixed: Vec<f64> = acc
            .iter()
            .enumerate()
            .map(|(k, a)| 0.5 * a / total + 0.5 * k as f64 / SAMPLES as f64)
            .collect();
        let mut out = Vec::with_capacity(n + 1);
        let mut k = 0;
        for i in 0..=n {
            let goal = i as f64 / n as f64;
            while k + 1 < SAMPLES && mixed[k + 1] < goal {
                k += 1;
            }
            let (a, b) = (mixed[k], mixed[k + 1]);
            let f = if b > a {
                ((goal - a) / (b - a)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.push((k as f64 + f) / SAMPLES as f64);
        }
        out[0] = 0.0;
        out[n] = 1.0;
        out
    };
    let (us, vs) = (grid(true), grid(false));
    let (nu, nv) = (us.len() - 1, vs.len() - 1);
    // Which way the patch's natural normal points: out of the leaf, or into it.
    let eps = diag * 1e-5;
    let normal_at = |u: f64, v: f64| {
        let (h, k) = (1e-4, 1e-4);
        let p = (pt.at)(u, v);
        let du =
            [0, 1, 2].map(|c| (pt.at)((u + h).min(1.0), v)[c] - (pt.at)((u - h).max(0.0), v)[c]);
        let dv =
            [0, 1, 2].map(|c| (pt.at)(u, (v + k).min(1.0))[c] - (pt.at)(u, (v - k).max(0.0))[c]);
        let nrm = [
            du[1] * dv[2] - du[2] * dv[1],
            du[2] * dv[0] - du[0] * dv[2],
            du[0] * dv[1] - du[1] * dv[0],
        ];
        (p, nrm)
    };
    let mut flip = None;
    'probe: for &u in &[0.37, 0.61, 0.23, 0.81, 0.5] {
        for &v in &[0.41, 0.67, 0.29, 0.53] {
            if pt.valid.as_ref().is_some_and(|ok| !ok(u, v)) {
                continue;
            }
            let (p, nrm) = normal_at(u, v);
            let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
            if l < 1e-12 {
                continue;
            }
            let q = [0, 1, 2].map(|c| p[c] + nrm[c] / l * eps);
            flip = Some(oracle::contains(leaf, q));
            break 'probe;
        }
    }
    let Some(inward) = flip else { return };
    // On the boundary, and with the material on the leaf's inside side, at (u, v): the shape's
    // membership a hair off the surface on each side differs. Testing off the surface, not on it,
    // keeps faces that lie on another leaf's face from flickering with that leaf's test exactly on
    // its boundary. Where the normal vanishes (a pole, a disc's centre), the leaf is taken as solid
    // and as empty instead.
    let off = diag * 1e-7;
    // Every copy of this leaf in the tree, switched together.
    let twins: Vec<usize> = (0..leaves.len())
        .filter(|&j| leaves[j].0.key() == leaf.key())
        .collect();
    let state = |u: f64, v: f64| -> (bool, bool) {
        if pt.valid.as_ref().is_some_and(|ok| !ok(u, v)) {
            return (false, false);
        }
        let (p, mut nrm) = normal_at(u, v);
        let mut l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if l < 1e-14 {
            // A pole, a disc's centre: the normal a hair in toward the patch's middle, so the
            // point is tested off the surface as every other is. On the surface itself another
            // leaf's face through it, as a coaxial disc's, makes the test there a coin toss.
            let n = normal_at(u + (0.5 - u) * 1e-6, v + (0.5 - v) * 1e-6).1;
            let ln = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if ln < 1e-14 {
                let solid = inside(root, leaves, p, (&twins, true));
                let empty = inside(root, leaves, p, (&twins, false));
                return (solid != empty, solid);
            }
            (nrm, l) = (n, ln);
        }
        // Out of the leaf along its surface's normal.
        let s = if inward { -off / l } else { off / l };
        let (q_out, q_in) = (
            [0, 1, 2].map(|c| p[c] + nrm[c] * s),
            [0, 1, 2].map(|c| p[c] - nrm[c] * s),
        );
        let outer = inside(root, leaves, q_out, (&[], false));
        let inner = inside(root, leaves, q_in, (&[], false));
        if outer == inner {
            return (false, inner);
        }
        // The boundary passes here, but it may be another leaf's surface lying on this one, as a
        // thread's crest cut away from its ridge lies on the shank it runs along. A leaf matters
        // here where switching it changes the shape beside it.
        let matters = |ids: &[usize]| {
            inside(root, leaves, q_in, (ids, false)) != inner
                || inside(root, leaves, q_out, (ids, true)) != outer
        };
        // Where several leaves' surfaces lie here, as where two operands share a face, one keeps
        // it: the lowest that matters, or the lowest of them where none does.
        let here = |j: usize| {
            let (shape, (lo, hi)) = &leaves[j];
            let near = |q: Vec3| (0..3).all(|k| q[k] >= lo[k] && q[k] <= hi[k]);
            (near(q_in) || near(q_out))
                && oracle::contains(shape, q_in) != oracle::contains(shape, q_out)
        };
        let keep = if matters(&twins) {
            !(0..li).any(|j| !twins.contains(&j) && here(j) && matters(&[j]))
        } else {
            !(0..leaves.len()).any(|j| !twins.contains(&j) && here(j) && (j < li || matters(&[j])))
        };
        (keep, inner)
    };
    // The surface's normal at (u, v), out of the shape: out of the leaf where the material is on
    // the leaf's inside, into it otherwise; zero where it vanishes, filled in from the triangles.
    let facing = |u: f64, v: f64, solid: bool| -> [f32; 3] {
        let n = normal_at(u, v).1;
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l < 1e-14 {
            return [0.0; 3];
        }
        let s = if inward == solid { -1.0 / l } else { 1.0 / l };
        n.map(|x| (x * s) as f32)
    };
    let mut t = Trim {
        state: &state,
        at: &*pt.at,
        facing: &facing,
        inward,
        tol: 0.05 * cell,
        states: HashMap::new(),
        verts: HashMap::new(),
        cuts: HashMap::new(),
        local: Vec::new(),
        normals: Vec::new(),
        params: Vec::new(),
        tris: Vec::new(),
        bounds: Vec::new(),
        probe: false,
    };
    let at = |i: usize, j: usize| [us[i], vs[j]];
    // Probing every triangle for slivers between its corners costs several times the corners
    // alone. A sliver runs on from a trim, so the cells probed are those a trim crosses, and then,
    // cell by cell, each neighbour of a probed cell whose probes find something its corners miss.
    let kept: Vec<Vec<bool>> = (0..=nu)
        .map(|i| (0..=nv).map(|j| t.state(at(i, j)).0).collect())
        .collect();
    let mut probe = vec![false; nu * nv];
    let mut queue = std::collections::VecDeque::new();
    for i in 0..nu {
        for j in 0..nv {
            let c = kept[i][j];
            if kept[i + 1][j] != c || kept[i][j + 1] != c || kept[i + 1][j + 1] != c {
                probe[i * nv + j] = true;
                queue.push_back((i, j));
            }
        }
    }
    // Whether a cell whose corners agree hides something else between them, by the probes
    // `triangle` makes in each of its halves.
    let hides = |t: &mut Trim, i: usize, j: usize| {
        let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
        let c = kept[i][j];
        [[q[0], q[1], q[2]], [q[0], q[2], q[3]]].iter().any(|p| {
            let mid = |a: Uv, b: Uv| [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
            let centre = [
                (p[0][0] + p[1][0] + p[2][0]) / 3.0,
                (p[0][1] + p[1][1] + p[2][1]) / 3.0,
            ];
            let near = |k: usize| {
                [
                    p[k][0] + 0.02 * (centre[0] - p[k][0]),
                    p[k][1] + 0.02 * (centre[1] - p[k][1]),
                ]
            };
            [
                mid(p[0], p[1]),
                mid(p[1], p[2]),
                mid(p[2], p[0]),
                centre,
                near(0),
                near(1),
                near(2),
            ]
            .iter()
            .any(|&x| t.state(x).0 != c)
        })
    };
    // A band thin all along, as one solid's face runs under a thin collar of another, crosses no
    // trim on this patch to start from: each cell's halves are checked at their centres too, two
    // looks a cell where the full probes take eleven.
    for i in 0..nu {
        for j in 0..nv {
            if probe[i * nv + j] {
                continue;
            }
            let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let centre =
                |a: Uv, b: Uv, c: Uv| [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0];
            let c = kept[i][j];
            if t.state(centre(q[0], q[1], q[2])).0 != c || t.state(centre(q[0], q[2], q[3])).0 != c
            {
                probe[i * nv + j] = true;
                queue.push_back((i, j));
            }
        }
    }
    while let Some((i, j)) = queue.pop_front() {
        for (a, b) in [
            (i.wrapping_sub(1), j),
            (i + 1, j),
            (i, j.wrapping_sub(1)),
            (i, j + 1),
        ] {
            if a >= nu || b >= nv || probe[a * nv + b] {
                continue;
            }
            if hides(&mut t, a, b) {
                probe[a * nv + b] = true;
                queue.push_back((a, b));
            }
        }
    }
    for i in 0..nu {
        for j in 0..nv {
            t.probe = probe[i * nv + j];
            let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            t.triangle([q[0], q[1], q[2]], 0);
            t.triangle([q[0], q[2], q[3]], 0);
        }
    }
    // A triangle split beside one that was not leaves a vertex at the middle of their shared
    // side, on the surface, while the unsplit side is a chord short of it: a crack, open to what
    // lies behind. The unsplit triangle is split at that vertex too, and so is a border along
    // the side. Splits make their middles by the same sums, so the middle is found by its bits.
    let half = |a: Uv, b: Uv| [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    let mut i = 0;
    while i < t.tris.len() {
        let tri = t.tris[i];
        let hang = (0..3).find_map(|k| {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let m = *t
                .verts
                .get(&key(half(t.params[a as usize], t.params[b as usize])))?;
            (m != a && m != b).then_some((k, m))
        });
        match hang {
            Some((k, m)) => {
                let (a, b, c) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
                t.tris[i] = [a, m, c];
                t.tris.push([m, b, c]);
            }
            None => i += 1,
        }
    }
    let mut i = 0;
    while i < t.bounds.len() {
        let [a, b] = t.bounds[i];
        match t
            .verts
            .get(&key(half(t.params[a as usize], t.params[b as usize])))
        {
            Some(&m) if m != a && m != b => {
                t.bounds[i] = [a, m];
                t.bounds.push([m, b]);
            }
            _ => i += 1,
        }
    }
    // Only the vertices the kept triangles use: the rest of the grid lies off the shape, and a
    // half-space's grid reaches far past it.
    let Trim {
        local,
        normals,
        mut tris,
        bounds,
        ..
    } = t;
    let mut used: HashMap<u32, u32> = HashMap::new();
    for tri in &mut tris {
        for v in tri.iter_mut() {
            let next = out.positions.len() as u32;
            *v = *used.entry(*v).or_insert_with(|| {
                out.positions.push(local[*v as usize]);
                out.normals.push(normals[*v as usize]);
                next
            });
        }
    }
    out.triangles.extend(tris);
    // A bound beside no kept triangle names a vertex dropped with the rest of the grid.
    out.bounds.extend(
        bounds
            .iter()
            .filter_map(|e| Some([*used.get(&e[0])?, *used.get(&e[1])?])),
    );
}

type Uv = [f64; 2];

/// A point of the parameter plane by its coordinates' bits, and a segment by its ends'.
type Key = (u64, u64);

fn key(p: Uv) -> Key {
    (p[0].to_bits(), p[1].to_bits())
}

/// One patch's trimming: marching triangles over its parameters, each straddling triangle split
/// in four where the trim through it is no straight chord, as at a corner where two trims meet,
/// down to a thirty-second of a cell.
struct Trim<'a> {
    state: &'a dyn Fn(f64, f64) -> (bool, bool),
    at: &'a dyn Fn(f64, f64) -> Vec3,
    facing: &'a dyn Fn(f64, f64, bool) -> [f32; 3],
    inward: bool,
    tol: f64,
    states: HashMap<Key, (bool, bool)>,
    verts: HashMap<Key, u32>,
    cuts: HashMap<(Key, Key), u32>,
    local: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    params: Vec<Uv>,
    tris: Vec<[u32; 3]>,
    bounds: Vec<[u32; 2]>,
    /// Whether the cell being meshed lies by a trim, where a sliver can pass between corners
    /// that agree: only there are its triangles probed inside.
    probe: bool,
}

impl Trim<'_> {
    fn state(&mut self, p: Uv) -> (bool, bool) {
        if let Some(&s) = self.states.get(&key(p)) {
            return s;
        }
        let s = (self.state)(p[0], p[1]);
        self.states.insert(key(p), s);
        s
    }

    fn vertex(&mut self, p: Uv, solid: bool) -> u32 {
        if let Some(&id) = self.verts.get(&key(p)) {
            return id;
        }
        let id = self.local.len() as u32;
        self.local.push((self.at)(p[0], p[1]).map(|x| x as f32));
        self.normals.push((self.facing)(p[0], p[1], solid));
        self.params.push(p);
        self.verts.insert(key(p), id);
        id
    }

    /// Where the trim crosses from `a` to `b`, by bisection on the parameters.
    fn crossing_uv(&mut self, a: Uv, b: Uv) -> Uv {
        let ka = self.state(a).0;
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..18 {
            let m = 0.5 * (lo + hi);
            if (self.state)(a[0] + (b[0] - a[0]) * m, a[1] + (b[1] - a[1]) * m).0 == ka {
                lo = m;
            } else {
                hi = m;
            }
        }
        let m = 0.5 * (lo + hi);
        [a[0] + (b[0] - a[0]) * m, a[1] + (b[1] - a[1]) * m]
    }

    fn crossing(&mut self, a: Uv, b: Uv, solid: bool) -> (u32, Uv) {
        let k = if key(a) < key(b) {
            (key(a), key(b))
        } else {
            (key(b), key(a))
        };
        let at = self.crossing_uv(a, b);
        if let Some(&id) = self.cuts.get(&k) {
            return (id, at);
        }
        let id = self.local.len() as u32;
        self.local.push((self.at)(at[0], at[1]).map(|x| x as f32));
        self.normals.push((self.facing)(at[0], at[1], solid));
        self.params.push(at);
        self.cuts.insert(k, id);
        (id, at)
    }

    fn emit(&mut self, t: [u32; 3], solid: bool) {
        // A sliver a fiftieth of the tolerance high, as a face keeps along a curve another face
        // lies on exactly, is no face to see, only a strip to fight the other face for depth.
        let [a, b, c] = t.map(|i| self.local[i as usize].map(f64::from));
        let (u, v) = (
            [0, 1, 2].map(|k| b[k] - a[k]),
            [0, 1, 2].map(|k| c[k] - a[k]),
        );
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let twice = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let longest = [dist(a, b), dist(b, c), dist(c, a)]
            .into_iter()
            .fold(0.0, f64::max);
        if twice < 0.02 * self.tol * longest {
            return;
        }
        // The material lies on the leaf's inside where the leaf is solid: face out of the leaf.
        let reverse = solid == self.inward;
        self.tris.push(if reverse { [t[0], t[2], t[1]] } else { t });
        // A side along the surface's own border bounds it.
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let (p, q) = (self.params[a as usize], self.params[b as usize]);
            let on = |c: usize, x: f64| p[c] == x && q[c] == x;
            if on(0, 0.0) || on(0, 1.0) || on(1, 0.0) || on(1, 1.0) {
                self.bounds.push([a, b]);
            }
        }
    }

    fn triangle(&mut self, p: [Uv; 3], depth: u32) {
        let s = p.map(|q| self.state(q));
        let keep = s.map(|x| x.0);
        let n_keep = keep.iter().filter(|&&k| k).count();
        let mid = |a: Uv, b: Uv| [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
        let split = |me: &mut Self| {
            let (m01, m12, m20) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[0]));
            me.triangle([p[0], m01, m20], depth + 1);
            me.triangle([m01, p[1], m12], depth + 1);
            me.triangle([m20, m12, p[2]], depth + 1);
            me.triangle([m01, m12, m20], depth + 1);
        };
        if n_keep == 0 {
            // A sliver can pass between all three corners, as where a trim leaves the patch's
            // edge at a tangent and thins to nothing along it, where the corners on that edge
            // read as outside: probed at the sides' middles, the centre and just inside each
            // corner, and split down to a 256th of a cell while any probe finds it.
            let centre = [
                (p[0][0] + p[1][0] + p[2][0]) / 3.0,
                (p[0][1] + p[1][1] + p[2][1]) / 3.0,
            ];
            let near = |k: usize| {
                let q = p[k];
                [
                    q[0] + 0.02 * (centre[0] - q[0]),
                    q[1] + 0.02 * (centre[1] - q[1]),
                ]
            };
            let probes = [
                mid(p[0], p[1]),
                mid(p[1], p[2]),
                mid(p[2], p[0]),
                centre,
                near(0),
                near(1),
                near(2),
            ];
            if self.probe && depth < 8 && probes.iter().any(|&q| self.state(q).0) {
                split(self);
            }
            return;
        }
        let solid = s.iter().find(|x| x.0).is_none_or(|x| x.1);
        if n_keep == 3 && self.probe && depth < 8 {
            // The same the other way: a band another solid hides, thinner than a cell, can pass
            // between three kept corners, as a cone runs on under a thread's crest it barely
            // clears. Probed at the sides' middles and the centre, split while any is hidden.
            let centre = [
                (p[0][0] + p[1][0] + p[2][0]) / 3.0,
                (p[0][1] + p[1][1] + p[2][1]) / 3.0,
            ];
            let probes = [mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[0]), centre];
            if probes.iter().any(|&q| !self.state(q).0) {
                split(self);
                return;
            }
        }
        if n_keep == 3 {
            let ids = [0, 1, 2].map(|k| self.vertex(p[k], s[k].1));
            self.emit(ids, solid);
            return;
        }
        let r = (0..3)
            .find(|&k| keep[k] != keep[(k + 1) % 3] && keep[k] == keep[(k + 2) % 3])
            .unwrap_or(0);
        // `p[r]` and `p[r + 2]` agree; `p[r + 1]` is the odd one.
        let (a, b, c) = (p[r], p[(r + 1) % 3], p[(r + 2) % 3]);
        let (sa, sb, sc) = (s[r].1, s[(r + 1) % 3].1, s[(r + 2) % 3].1);
        let side = if keep[(r + 1) % 3] { sb } else { sa };
        let (ab, ab_uv) = self.crossing(a, b, side);
        let (bc, bc_uv) = self.crossing(b, c, side);
        // The trim is a straight chord between its two crossings when, from the odd corner
        // through the chord's middle, it crosses near that middle; at a corner of two trims, or
        // where one bends inside the triangle, it does not, and the triangle is split.
        if depth < 5 {
            let m = mid(ab_uv, bc_uv);
            let far = [b[0] + 2.0 * (m[0] - b[0]), b[1] + 2.0 * (m[1] - b[1])];
            let straight = self.state(far).0 != keep[(r + 1) % 3] && {
                let q = self.crossing_uv(b, far);
                let (x, y) = ((self.at)(q[0], q[1]), (self.at)(m[0], m[1]));
                let d =
                    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt();
                d <= self.tol
            };
            if !straight {
                split(self);
                return;
            }
        }
        // The chord between the two crossings is the trim.
        self.bounds.push([ab, bc]);
        if keep[(r + 1) % 3] {
            let vb = self.vertex(b, sb);
            self.emit([ab, vb, bc], solid);
        } else {
            let (va, vc) = (self.vertex(a, sa), self.vertex(c, sc));
            self.emit([va, ab, bc], solid);
            self.emit([va, bc, vc], solid);
        }
    }
}

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Room for `len` bytes of input, for the page to write a shape's JSON into.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Releases what [`alloc`] gave.
///
/// # Safety
///
/// `p` and `len` must be a pointer and length [`alloc`] returned, released once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn release(p: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(p, len, len) });
}

/// Meshes the shape whose JSON is at `p`: a buffer of a zero status word, the vertex, triangle
/// and edge counts, the positions then the normals as f32 triples, the triangles as u32 triples,
/// the edges as pairs of f32 triples and each edge's turn as an f32, all little-endian; or a status
/// of one, the error's length and its text.
///
/// # Safety
///
/// `p` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mesh_json(p: *const u8, len: usize, cell: f64) -> *const u8 {
    let text = unsafe { std::slice::from_raw_parts(p, len) };
    let result = serde_json::from_slice::<Value>(text)
        .map_err(|e| e.to_string())
        .and_then(|v| mesh_value(&v, cell));
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.clear();
        // A status word first: 0 and the mesh, or 1 and the error's text.
        match result {
            Ok(m) => {
                o.extend(0u32.to_le_bytes());
                o.extend((m.positions.len() as u32).to_le_bytes());
                o.extend((m.triangles.len() as u32).to_le_bytes());
                o.extend((m.edges.len() as u32).to_le_bytes());
                for x in m.positions.iter().chain(&m.normals).flatten() {
                    o.extend(x.to_le_bytes());
                }
                for x in m.triangles.iter().flatten() {
                    o.extend(x.to_le_bytes());
                }
                for x in m.edges.iter().flatten().flatten() {
                    o.extend(x.to_le_bytes());
                }
                for x in &m.turns {
                    o.extend(x.to_le_bytes());
                }
            }
            Err(e) => {
                o.extend(1u32.to_le_bytes());
                o.extend((e.len() as u32).to_le_bytes());
                o.extend(e.as_bytes());
            }
        }
        o.as_ptr()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Volume by the divergence theorem over the triangles: right only when they face out.
    fn volume(m: &Mesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }

    fn frame(o: Vec3) -> Value {
        serde_json::json!({"origin": o, "x": [1.0, 0.0, 0.0], "y": [0.0, 1.0, 0.0],
                           "z": [0.0, 0.0, 1.0]})
    }

    fn close(got: f64, want: f64, rel: f64) {
        assert!((got - want).abs() < rel * want, "{got} vs {want}");
    }

    #[test]
    fn a_ghost_keeps_the_thin_ends_of_a_rounded_corner_s_crescent() {
        // A 10 square prism 4 tall outside the same prism with its corner at (10, 10) rounded 2:
        // on each cap the crescent between the round and the corner, (1 − π/4)·2². Toward each
        // contact it thins as s²/4, far below a cell, and must still be there.
        use brep_conformance::profile::{Profile, Seg};
        let line = |a: [f64; 2], b: [f64; 2]| Seg::Line { a, b };
        let prism = |lp: Vec<Seg>| {
            Shape::Prim(Primitive::Extrude {
                frame: Frame::at([0.0; 3]),
                profile: Profile { loops: vec![lp] },
                height: 4.0,
            })
        };
        let square = prism(vec![
            line([0.0, 0.0], [10.0, 0.0]),
            line([10.0, 0.0], [10.0, 10.0]),
            line([10.0, 10.0], [0.0, 10.0]),
            line([0.0, 10.0], [0.0, 0.0]),
        ]);
        let rounded = prism(vec![
            line([0.0, 0.0], [10.0, 0.0]),
            line([10.0, 0.0], [10.0, 8.0]),
            Seg::Arc {
                center: [8.0, 8.0],
                radius: 2.0,
                start: 0.0,
                sweep: PI / 2.0,
            },
            line([8.0, 10.0], [0.0, 10.0]),
            line([0.0, 10.0], [0.0, 0.0]),
        ]);
        let m = mesh_outside(&square, &rounded, 0.0);
        let mut top = 0.0;
        for t in &m.triangles {
            let [a, b, c] = t.map(|i| m.positions[i as usize].map(f64::from));
            if [a, b, c].iter().all(|p| (p[2] - 4.0).abs() < 1e-6) {
                let (u, v) = ([0, 1].map(|k| b[k] - a[k]), [0, 1].map(|k| c[k] - a[k]));
                top += 0.5 * (u[0] * v[1] - u[1] * v[0]).abs();
            }
        }
        close(top, (1.0 - PI / 4.0) * 4.0, 1e-3);
    }

    #[test]
    fn a_surface_cut_away_where_it_lies_on_another_is_not_drawn_twice() {
        // A rod of radius 1 from 0 to 4, joined with one from 0 to 8 cut to 6..8 by a box: the
        // second rod's side, cut away below 6, lies on the first's from 0 to 4. The union is two
        // rods, 2π·4 + 2π and 2π·2 + 2π, 16π; drawing the cut-away side as well adds 8π.
        let rod = |h: f64| {
            shape(
                &serde_json::json!({"kind": "cylinder", "radius": 1.0, "height": h,
                "frame": frame([0.0; 3])}),
            )
            .unwrap()
        };
        let block = shape(&serde_json::json!({"kind": "box", "size": [4.0, 4.0, 2.0],
            "frame": frame([-2.0, -2.0, 6.0])}))
        .unwrap();
        let top = Shape::Bool(Op::Intersection, Box::new(rod(8.0)), Box::new(block));
        let both = Shape::Bool(Op::Union, Box::new(rod(4.0)), Box::new(top));
        let m = mesh(&both, 0.02);
        let area: f64 = m
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].map(f64::from));
                let (u, v) = (
                    [0, 1, 2].map(|k| b[k] - a[k]),
                    [0, 1, 2].map(|k| c[k] - a[k]),
                );
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
            })
            .sum();
        close(area, 16.0 * PI, 2e-3);
    }

    /// How much of `lines` lies on the circle of radius 1 at z = 2.
    fn on_unit_circle(lines: &[[[f32; 3]; 2]]) -> f64 {
        lines
            .iter()
            .map(|[a, b]| (a.map(f64::from), b.map(f64::from)))
            .filter(|(a, b)| {
                [a, b]
                    .iter()
                    .all(|x| (x[0].hypot(x[1]) - 1.0).abs() < 1e-3 && (x[2] - 2.0).abs() < 1e-3)
            })
            .map(|(a, b)| dist(a, b))
            .sum()
    }

    #[test]
    fn a_gentle_crease_between_surfaces_sampled_apart_is_one_edge() {
        // A turned rod, a cylinder of radius 1 to z = 2 and then a cone widening to 1.2 at z = 4,
        // meeting at 5.7°. The cone's grid along the circle is sized by its wide end, so its
        // points there fall between the cylinder's and neither border is shared; the circle is
        // still an edge, however gentle, and drawn once.
        let rod = shape(
            &serde_json::json!({"kind": "revolve", "angle": TAU, "frame": frame([0.0; 3]),
            "profile": [[
                {"line": [[0.0, 0.0], [1.0, 0.0]]},
                {"line": [[1.0, 0.0], [1.0, 2.0]]},
                {"line": [[1.0, 2.0], [1.2, 4.0]]},
                {"line": [[1.2, 4.0], [0.0, 4.0]]},
                {"line": [[0.0, 4.0], [0.0, 0.0]]}]]}),
        )
        .unwrap();
        let m = mesh(&rod, 0.02);
        close(on_unit_circle(&m.edges), TAU, 1e-3);
    }

    #[test]
    fn a_tangent_join_between_faces_is_an_edge_drawn_once() {
        // The rod's cylinder running at z = 2 into a quarter round of radius 0.5 tangent to it:
        // the circle where they meet is an edge between two faces, tangent or not.
        let rod = shape(
            &serde_json::json!({"kind": "revolve", "angle": TAU, "frame": frame([0.0; 3]),
            "profile": [[
                {"line": [[0.0, 0.0], [1.0, 0.0]]},
                {"line": [[1.0, 0.0], [1.0, 2.0]]},
                {"arc": {"center": [0.5, 2.0], "radius": 0.5, "start": 0.0, "sweep": PI / 2.0}},
                {"line": [[0.5, 2.5], [0.0, 2.5]]},
                {"line": [[0.0, 2.5], [0.0, 0.0]]}]]}),
        )
        .unwrap();
        let m = mesh(&rod, 0.02);
        close(on_unit_circle(&m.edges), TAU, 1e-3);
    }

    /// The area of `m`'s triangles lying in the plane z = `z`.
    fn area_at(m: &Mesh, z: f64) -> f64 {
        m.triangles
            .iter()
            .map(|t| t.map(|i| m.positions[i as usize].map(f64::from)))
            .filter(|t| t.iter().all(|p| (p[2] - z).abs() < 1e-6))
            .map(|[a, b, c]| {
                0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs()
            })
            .sum()
    }

    #[test]
    fn discs_two_coaxial_solids_share_are_drawn_once_centre_and_all() {
        // A rod of radius 1 and a cone from 1.05 down to 0.99 over the same 4, joined: each end
        // is one disc, the rod's at the top with the cone's lying on it, π. At a disc's centre,
        // where its own normal vanishes, the other disc made the test a coin toss, and both
        // were drawn round it.
        let rod = shape(
            &serde_json::json!({"kind": "cylinder", "radius": 1.0, "height": 4.0,
            "frame": frame([0.0; 3])}),
        )
        .unwrap();
        let cone = shape(
            &serde_json::json!({"kind": "cone", "bottom": 1.05, "top": 0.99,
            "height": 4.0, "frame": frame([0.0; 3])}),
        )
        .unwrap();
        let m = mesh(&Shape::Bool(Op::Union, Box::new(rod), Box::new(cone)), 0.1);
        close(area_at(&m, 4.0), PI, 2e-3);
    }

    #[test]
    fn a_strip_thinning_to_nothing_is_drawn_to_its_tip() {
        // A 2 cube with blocks over its top either side of a wedge-shaped gap: from x = 1.03 on
        // one side, and on the other along a line leaning 0.15 in x for each 1 in y. The top is
        // kept only in the gap, 0.15·y wide, 0.3 in all; below y = 0.67 it is thinner than a
        // cell and misses every corner of the cells it crosses.
        let cube = shape(&serde_json::json!({"kind": "box", "size": [2.0, 2.0, 2.0],
            "frame": frame([0.0; 3])}))
        .unwrap();
        let left = shape(&serde_json::json!({"kind": "box", "size": [3.03, 4.0, 1.5],
            "frame": frame([-2.0, -1.0, 1.5])}))
        .unwrap();
        let l = 1.0f64.hypot(0.15);
        let right = shape(&serde_json::json!({"kind": "box", "size": [3.0, 4.0, 1.5],
            "frame": {"origin": [1.03, 0.0, 1.5], "x": [1.0 / l, -0.15 / l, 0.0],
                      "y": [0.15 / l, 1.0 / l, 0.0], "z": [0.0, 0.0, 1.0]}}))
        .unwrap();
        let over = Shape::Bool(Op::Union, Box::new(left), Box::new(right));
        let m = mesh(&Shape::Bool(Op::Union, Box::new(cube), Box::new(over)), 0.1);
        close(area_at(&m, 2.0), 0.3, 5e-3);
    }

    #[test]
    fn a_band_hidden_under_another_solid_thinner_than_a_cell_is_left_out() {
        // A rod of radius 1, 4 long, under a collar of radius 1.01 from z = 2.04 to 2.07, a third
        // of a cell tall and between two rows of the grid. The rod's side within the collar is
        // inside the union and is no face: the side drawn is 2π·(4 − 0.03), not the whole 8π.
        let rod = shape(
            &serde_json::json!({"kind": "cylinder", "radius": 1.0, "height": 4.0,
            "frame": frame([0.0; 3])}),
        )
        .unwrap();
        let collar = shape(&serde_json::json!({"kind": "cylinder", "radius": 1.01,
            "height": 0.03, "frame": frame([0.0, 0.0, 2.04])}))
        .unwrap();
        let m = mesh(
            &Shape::Bool(Op::Union, Box::new(rod), Box::new(collar)),
            0.1,
        );
        let side: f64 = m
            .triangles
            .iter()
            .map(|t| t.map(|i| m.positions[i as usize].map(f64::from)))
            .filter(|t| t.iter().all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-3))
            .map(|[a, b, c]| {
                let (u, v) = (
                    [0, 1, 2].map(|k| b[k] - a[k]),
                    [0, 1, 2].map(|k| c[k] - a[k]),
                );
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
            })
            .sum();
        close(side, TAU * (4.0 - 0.03), 2e-3);
    }

    #[test]
    fn a_box_outside_its_half_keeps_only_its_faces_past_it() {
        // A 10 mm cube and its half below x = 5, which shares five of its faces in part: outside
        // the half lie the face at x = 10 and the four sides' halves past x = 5, 300 mm², and
        // nothing on the faces they share.
        let cube = |sx: f64| {
            shape(&serde_json::json!({"kind": "box", "size": [sx, 10.0, 10.0],
                "frame": frame([0.0; 3])}))
            .unwrap()
        };
        let m = mesh_outside(&cube(10.0), &cube(5.0), 0.25);
        let mut area = 0.0;
        for t in &m.triangles {
            let [a, b, c] = t.map(|i| m.positions[i as usize].map(f64::from));
            let (u, v) = (
                [0, 1, 2].map(|k| b[k] - a[k]),
                [0, 1, 2].map(|k| c[k] - a[k]),
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((a[0] + b[0] + c[0]) / 3.0 > 5.0, "{a:?} {b:?} {c:?}");
        }
        close(area, 300.0, 1e-6);
    }

    #[test]
    fn a_ball_meshes_on_its_sphere_facing_out() {
        // A ball of radius 2 off the origin: 32π/3, and every vertex on the sphere.
        let s = shape(&serde_json::json!({"kind": "sphere", "radius": 2.0,
            "frame": frame([1.0, 2.0, 3.0])}))
        .unwrap();
        let m = mesh(&s, 4.0 * 3f64.sqrt() / 120.0);
        close(volume(&m), 32.0 * PI / 3.0, 0.002);
        for p in &m.positions {
            let r = dist(p.map(f64::from), [1.0, 2.0, 3.0]);
            assert!((r - 2.0).abs() < 1e-5, "{r}");
        }
    }

    #[test]
    fn a_box_less_a_rod_keeps_the_rod_s_hole() {
        // A 4 × 4 × 2 box less a rod of radius 1 through it: 32 − 2π.
        let s = shape(&serde_json::json!({"kind": "boolean", "op": "difference",
            "a": {"kind": "box", "frame": frame([0.0, 0.0, 0.0]), "size": [4.0, 4.0, 2.0]},
            "b": {"kind": "cylinder", "frame": frame([2.0, 2.0, -1.0]), "radius": 1.0,
                  "height": 4.0}}))
        .unwrap();
        close(volume(&mesh(&s, 0.0)), 32.0 - 2.0 * PI, 0.002);
    }

    #[test]
    fn a_chamfered_hole_keeps_its_cone_where_the_ring_shares_the_plate_s_faces() {
        // A 10 × 10 × 4 plate with a hole of radius 2, less a ring whose triangle sits on the
        // hole's wall and the top face, 1 on a side: the ring's volume is 2π·(r + c/3)·c²/2 by
        // Pappus, so 400 − 16π − π·(2 + 1/3).
        let (r, c, h) = (2.0, 1.0, 4.0);
        let plate = serde_json::json!({"kind": "extrude", "frame": frame([0.0, 0.0, 0.0]),
            "height": h, "profile": [
                [{"line": [[0.0, 0.0], [10.0, 0.0]]}, {"line": [[10.0, 0.0], [10.0, 10.0]]},
                 {"line": [[10.0, 10.0], [0.0, 10.0]]}, {"line": [[0.0, 10.0], [0.0, 0.0]]}],
                [{"arc": {"center": [5.0, 5.0], "radius": r, "start": 0.0, "sweep": -TAU}}]]});
        let ring = serde_json::json!({"kind": "revolve", "frame": frame([5.0, 5.0, 0.0]),
            "angle": TAU, "profile": [[
                {"line": [[r, h], [r, h - c]]}, {"line": [[r, h - c], [r + c, h]]},
                {"line": [[r + c, h], [r, h]]}]]});
        let s = shape(&serde_json::json!({"kind": "boolean", "op": "difference",
            "a": plate, "b": ring}))
        .unwrap();
        let want = 400.0 - 16.0 * PI - PI * (r + c / 3.0) * c * c;
        close(volume(&mesh(&s, 0.0)), want, 0.002);
    }

    fn edge_length(m: &Mesh) -> f64 {
        m.edges
            .iter()
            .map(|[a, b]| dist(a.map(f64::from), b.map(f64::from)))
            .sum()
    }

    #[test]
    fn a_box_s_edges_are_its_twelve_and_a_ball_has_none() {
        // A 3 × 4 × 5 box: 4·(3 + 4 + 5) = 48 of edges. A ball's seam and poles are no edges.
        let boxed = shape(
            &serde_json::json!({"kind": "box", "frame": frame([1.0, 2.0, 3.0]),
            "size": [3.0, 4.0, 5.0]}),
        )
        .unwrap();
        let length = edge_length(&mesh(&boxed, 0.0));
        assert!((length - 48.0).abs() < 1e-3, "{length}");
        let ball = shape(&serde_json::json!({"kind": "sphere", "radius": 2.0,
            "frame": frame([0.0, 0.0, 0.0])}))
        .unwrap();
        let bm = mesh(&ball, 0.0);
        assert!(bm.edges.is_empty(), "{} edges", bm.edges.len());
    }

    /// The length of `m`'s edges whose turn lies within 1e-3 of `turn`.
    fn length_turning(m: &Mesh, turn: f64) -> f64 {
        m.edges
            .iter()
            .zip(&m.turns)
            .filter(|(_, t)| (f64::from(**t) - turn).abs() < 1e-3)
            .map(|([a, b], _)| dist(a.map(f64::from), b.map(f64::from)))
            .sum()
    }

    #[test]
    fn each_edge_turns_as_its_faces_do_convex_concave_or_not_at_all() {
        // A 4 × 4 × 2 box less a 2 × 2 corner through it: an L, whose one inner edge, 2 long,
        // is concave, and whose other edges, 2·16 + 5·2 long, are convex right angles.
        let block = |at: [f64; 3], size: [f64; 3]| serde_json::json!({"kind": "box", "frame": frame(at), "size": size});
        let l = shape(&serde_json::json!({"kind": "boolean", "op": "difference",
            "a": block([0.0; 3], [4.0, 4.0, 2.0]), "b": block([2.0, 2.0, -1.0], [3.0, 3.0, 4.0])}))
        .unwrap();
        let m = mesh(&l, 0.0);
        assert_eq!(m.turns.len(), m.edges.len());
        close(length_turning(&m, PI / 2.0), 42.0, 1e-3);
        close(length_turning(&m, -PI / 2.0), 2.0, 1e-3);
        // A rod whose side runs smoothly into a quarter round: that circle turns by nothing,
        // and the rod's base rim by a right angle.
        let rod = shape(
            &serde_json::json!({"kind": "revolve", "angle": TAU, "frame": frame([0.0; 3]),
            "profile": [[
                {"line": [[0.0, 0.0], [1.0, 0.0]]},
                {"line": [[1.0, 0.0], [1.0, 2.0]]},
                {"arc": {"center": [0.5, 2.0], "radius": 0.5, "start": 0.0, "sweep": PI / 2.0}},
                {"line": [[0.5, 2.5], [0.0, 2.5]]},
                {"line": [[0.0, 2.5], [0.0, 0.0]]}]]}),
        )
        .unwrap();
        let m = mesh(&rod, 0.02);
        for (z, turn) in [(0.0, PI / 2.0), (2.0, 0.0)] {
            let at: Vec<f64> = m
                .edges
                .iter()
                .zip(&m.turns)
                .filter(|([a, b], _)| [a, b].iter().all(|p| (f64::from(p[2]) - z).abs() < 1e-6))
                .map(|(_, t)| f64::from(*t))
                .collect();
            assert!(at.len() >= 8, "{} edges at z = {z}", at.len());
            assert!(
                at.iter().all(|t| (t - turn).abs() < 1e-2),
                "z = {z}: {at:?}"
            );
        }
    }

    #[test]
    fn a_leaf_given_twice_draws_as_once() {
        // A filleted L's two rounded arms both hold the corner's cylinder: the union is the
        // cylinder, edged as one is, round its two rims, not along every triangle.
        let rod = serde_json::json!({"kind": "cylinder", "radius": 1.0, "height": 3.0,
            "frame": frame([0.0, 0.0, 0.0])});
        let twice = shape(&serde_json::json!({"kind": "boolean", "op": "union",
            "a": rod, "b": rod}))
        .unwrap();
        let length = edge_length(&mesh(&twice, 0.0));
        let once = edge_length(&mesh(&shape(&rod).unwrap(), 0.0));
        assert!(
            (length - once).abs() < 0.01 * once,
            "{length} twice, {once} once"
        );
    }

    #[test]
    fn where_a_box_edge_pierces_a_cone_every_triangle_lies_on_the_boundary() {
        // A cone of radius 3 to 0 over 6 crossed by a block whose edges run through its side:
        // every triangle's middle has the solid just behind it and nothing just in front, so no
        // triangle cuts across a corner where two trims meet.
        let cone = serde_json::json!({"kind": "cone", "frame": frame([0.0, 0.0, 0.0]),
            "bottom": 3.0, "top": 0.0, "height": 6.0});
        let block = serde_json::json!({"kind": "box", "frame": frame([-0.7, -4.0, 1.3]),
            "size": [4.5, 3.6, 3.1]});
        let s = shape(&serde_json::json!({"kind": "boolean", "op": "intersection",
            "a": cone, "b": block}))
        .unwrap();
        let m = mesh(&s, 0.05);
        let eps = 2e-3;
        let mut bad = 0.0;
        let mut total = 0.0;
        for t in &m.triangles {
            let [a, b, c] = t.map(|i| m.positions[i as usize].map(f64::from));
            let (u, v) = (
                [0, 1, 2].map(|k| b[k] - a[k]),
                [0, 1, 2].map(|k| c[k] - a[k]),
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let area = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if area < 1e-12 {
                continue;
            }
            let mid = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
            let at = |s: f64| [0, 1, 2].map(|k| mid[k] + n[k] / area * eps * s);
            total += area;
            if !oracle::contains(&s, at(-1.0)) || oracle::contains(&s, at(1.0)) {
                bad += area;
            }
        }
        assert!(
            bad / total < 1e-4,
            "{:.2e} of the area off the boundary",
            bad / total
        );
    }

    #[test]
    fn a_drilled_box_draws_its_edges_and_no_seams_of_split_triangles() {
        // A 4 × 4 × 2 box less a rod of radius 1: its twelve edges, 40 in all, and the hole's two
        // circles, each drawn once though both faces trim to it, 4π: nothing along the grid where
        // a triangle split beside the trim meets one that did not.
        let s = shape(&serde_json::json!({"kind": "boolean", "op": "difference",
            "a": {"kind": "box", "frame": frame([0.0, 0.0, 0.0]), "size": [4.0, 4.0, 2.0]},
            "b": {"kind": "cylinder", "frame": frame([2.0, 2.0, -1.0]), "radius": 1.0,
                  "height": 4.0}}))
        .unwrap();
        let length = edge_length(&mesh(&s, 0.0));
        close(length, 40.0 + 4.0 * PI, 0.01);
    }

    #[test]
    fn crossing_cylinders_meet_in_their_steinmetz_solid() {
        // Two rods of radius 1 crossing square through each other's middle: 16/3.
        let rod = |o: Vec3, z: Vec3| {
            serde_json::json!({"kind": "cylinder", "radius": 1.0,
            "height": 6.0, "frame": {"origin": o, "x": [0.0, 1.0, 0.0],
            "y": [z[1] * 0.0 - z[2] * 1.0, z[2] * 0.0 - z[0] * 0.0, z[0] * 1.0 - z[1] * 0.0], "z": z}})
        };
        let s = shape(&serde_json::json!({"kind": "boolean", "op": "intersection",
            "a": rod([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), "b": rod([-3.0, 0.0, 0.0], [1.0, 0.0, 0.0])}))
        .unwrap();
        close(volume(&mesh(&s, 0.0)), 16.0 / 3.0, 0.005);
    }
}
