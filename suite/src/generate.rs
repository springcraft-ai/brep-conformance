//! The case families: pairs of primitives drawn from a seed, each with what is known exactly about
//! their booleans. A family's version is part of every case id it makes, so changing what a family
//! draws for a seed means bumping it.

use std::f64::consts::{PI, TAU};

use crate::model::{Frame, Op, PathSeg, Primitive, Rigid, Shape, Side, Spline, Vec3};
use crate::portable::Portable;
use crate::profile::{Profile, Seg, chamfered, circle, rounded};

mod blends;
mod chains;
mod drawing;
mod hard;
mod parts;
mod paths;
mod robust;

pub use paths::{DIRS, test_path};

/// What the case knows exactly, beyond the identities every case checks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exact {
    pub intersection_volume: Option<f64>,
    pub intersection_area: Option<f64>,
    /// The least distance between the operands: zero where they overlap or touch.
    pub distance: Option<f64>,
    /// A feature case's result, where its expected shape is not one primitive.
    pub result_volume: Option<f64>,
    pub result_area: Option<f64>,
    /// Betti numbers (pieces, tunnels, voids) of the operands and results that have known ones.
    pub betti_a: Option<[u64; 3]>,
    pub betti_b: Option<[u64; 3]>,
    pub betti_union: Option<[u64; 3]>,
    pub betti_difference: Option<[u64; 3]>,
    /// The limits a sound case's result must keep, where the definition leaves it open.
    pub sound: Option<Sound>,
    /// Placed parts whose interference the case asks for, and the answer.
    pub assembly: Option<Assembly>,
    /// A boolean of many solids in one operation, and its result's volume and area.
    pub many: Option<Many>,
    /// A mirror of `a` to join to it, and the joined solid's volume and area.
    pub mirror: Option<Mirror>,
    /// Points of a solid's boundary and their outward normals, `None` at a crease.
    pub normals: Option<Normals>,
    /// A STEP case's file under another protocol than AP214, or written as triangles.
    pub step_as: Option<StepAs>,
    /// A smooth loft through sections, and what every acceptable one keeps.
    pub loft: Option<LoftThrough>,
    /// The least axis-aligned box round `a`, low corner then high.
    pub bbox: Option<[Vec3; 2]>,
    /// Points to find the nearest point of `a` to: each with its distance from `a`, and with the
    /// nearest point itself where there is only one and it is known.
    pub closest: Vec<(Vec3, f64, Option<Vec3>)>,
    /// Points, and where each lies against `a`: inside it, on its boundary or outside it.
    pub sides: Vec<(Vec3, Side)>,
    /// A plane to split `a` by: a point on it and its normal.
    pub split: Option<(Vec3, Vec3)>,
    /// A plane section of `a` and its closed form.
    pub section: Option<Section>,
    /// `a`, a prism, written as loose faces to be sewn back together.
    pub sew: Option<Sew>,
    /// Points about `a`'s boundary and the principal curvatures where it lies nearest each.
    pub curvature: Option<Curvatures>,
    /// For each of `curvature`'s points, where the boundary lies nearest it, the outward normal
    /// there and the direction of the first principal curvature. Drawn, never checked, and so left
    /// out of the case's id.
    pub curvature_frames: Vec<CurvatureFrame>,
    /// `a` drawn looking along a direction, with its hidden lines.
    pub drawing: Option<Drawing>,
    /// `a`'s creases, volume and area, which a display of it must draw and bound.
    pub display: Option<DisplayExact>,
    /// For a feature case, points by the result's seams and the curvature it must have there.
    pub probes: Vec<Probe>,
    /// For a loft, grids of rays across its side, where it must be smooth. They are no part of
    /// what tells one case from another, so they stand apart from the loft that names it.
    pub scans: Vec<Scan>,
    /// The test a family drawing several situations puts this case in, where its generator says.
    pub variant: Option<&'static str>,
    /// Steps applied in turn to `a`, built as given, whose result must be `b`: a chain of
    /// operations, or a part built step by step.
    pub chain: Option<Vec<Step>>,
    /// The case combines `a` with itself: `b` is `a` again.
    pub itself: bool,
    /// The loft's sections leave one answer, and it is `b`.
    pub loft_is_b: bool,
}

/// One step of a chain, applied to the solid so far.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// A blend or edit, as a feature case's act.
    Act(Feature),
    /// The solid less a tool.
    Cut(Shape),
    /// The solid with a tool added.
    Join(Shape),
    /// The solid moved by a rigid motion, rows `[R | t]`.
    Move([[f64; 4]; 3]),
    /// The solid joined to its image under a similarity, a mirror among them.
    MirrorJoin([[f64; 4]; 3]),
    /// The solid written as STEP and read back.
    StepRoundTrip,
}

/// A point by a blend's seam, the principal curvatures `want` at the boundary point nearest it,
/// and how far from them a curvature-continuous blend may be that close to the seam.
#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub point: Vec3,
    pub want: [f64; 2],
    pub tol: f64,
}

/// A boundary point, its outward unit normal, and the unit direction of its first principal
/// curvature; the second's is the normal crossed with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvatureFrame {
    pub foot: Vec3,
    pub normal: Vec3,
    pub d1: Vec3,
}

/// Points, and the principal curvatures `[k1, k2]`, `k1 ≥ k2`, at the boundary point nearest each.
#[derive(Clone, Debug, PartialEq)]
pub struct Curvatures {
    pub points: Vec<Vec3>,
    pub want: Vec<[f64; 2]>,
}

/// A solid's hidden-line drawing looking along `direction`: its edges and outlines cut into the
/// stretches the viewer sees and those the solid hides.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub direction: Vec3,
    pub pieces: Vec<Piece>,
}

/// A solid's surfaces, its creases, each a whole curve from parameter 0 to 1, and its volume and
/// area.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayExact {
    pub surfaces: Vec<Surface>,
    pub creases: Vec<Curve3>,
    pub volume: f64,
    pub area: f64,
}

/// A surface a solid's boundary lies on, with the way out of the solid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// The plane through `p`, `n` its outward unit normal.
    Plane { p: Vec3, n: Vec3 },
    /// The round surface about the line through `c` along the unit `axis`, `r` from it; `out`
    /// where the solid lies inside it, as for a cylinder, not a hole.
    Round {
        c: Vec3,
        axis: Vec3,
        r: f64,
        out: bool,
    },
    /// The ball's surface about `c`, `r` from it.
    Ball { c: Vec3, r: f64 },
}

impl Surface {
    /// How far `q` lies from the surface, and its outward normal at the nearest point.
    pub fn near(&self, q: Vec3) -> (f64, Vec3) {
        let d = |a: Vec3, b: Vec3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let unit = |v: Vec3| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            v.map(|x| x / l)
        };
        match *self {
            Surface::Plane { p, n } => {
                let v = d(q, p);
                ((v[0] * n[0] + v[1] * n[1] + v[2] * n[2]).abs(), n)
            }
            Surface::Round { c, axis, r, out } => {
                let v = d(q, c);
                let t = v[0] * axis[0] + v[1] * axis[1] + v[2] * axis[2];
                let radial = [0, 1, 2].map(|k| v[k] - t * axis[k]);
                let rho =
                    (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
                let n = unit(radial);
                ((rho - r).abs(), if out { n } else { n.map(|x| -x) })
            }
            Surface::Ball { c, r } => {
                let v = d(q, c);
                let rho = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                ((rho - r).abs(), unit(v))
            }
        }
    }
}

/// The stretch of `curve` from parameter `t0` to `t1`, seen or hidden.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub curve: Curve3,
    pub t0: f64,
    pub t1: f64,
    pub visible: bool,
}

/// A curve over the parameter 0 to 1: a straight run, or a whole circle about `c` from `x` toward
/// `y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve3 {
    Line { a: Vec3, b: Vec3 },
    Circle { c: Vec3, x: Vec3, y: Vec3, r: f64 },
}

impl Curve3 {
    pub fn at(&self, t: f64) -> Vec3 {
        match *self {
            Curve3::Line { a, b } => [0, 1, 2].map(|k| a[k] + t * (b[k] - a[k])),
            Curve3::Circle { c, x, y, r } => {
                let (s, co) = (TAU * t).psin_cos();
                [0, 1, 2].map(|k| c[k] + r * (co * x[k] + s * y[k]))
            }
        }
    }
}

/// The plane through `origin` square to `normal`, and the closed curves it cuts from a solid's
/// boundary: how many, their total length, and their least radius of curvature (infinite for a
/// polygon), which bounds how much shorter a chord polyline within a tolerance may be.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub origin: Vec3,
    pub normal: Vec3,
    pub loops: usize,
    pub length: f64,
    pub rho: f64,
}

/// How each loose face of a prism is written: shifted along its normal by `gaps[f]`, and inside
/// out where `flipped[f]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Sew {
    pub gaps: Vec<f64>,
    pub flipped: Vec<bool>,
}

/// The most a loose face is shifted off its prism's face, mm.
pub const SEW_GAP: f64 = 1e-4;

/// The tolerance a sew is asked for: ten times the widest gap two faces can leave.
pub const SEW_TOLERANCE: f64 = 20.0 * SEW_GAP;

/// Profiles drawn in the frame's xy plane at rising heights, to loft smoothly through. Points of
/// each inner section's outline, which the loft's boundary must pass through.
#[derive(Clone, Debug, PartialEq)]
pub struct LoftThrough {
    pub frame: Frame,
    pub profiles: Vec<Profile>,
    pub heights: Vec<f64>,
    pub outline: Vec<Vec3>,
}

/// How a STEP case's file is written, where not as AP214's B-rep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepAs {
    Schema(crate::step::Schema),
    Tessellated,
}

/// The solid `a`, or `a` combined with `b` by `op`, and points of its boundary with their outward
/// normals, `None` where faces meet at an angle.
#[derive(Clone, Debug, PartialEq)]
pub struct Normals {
    pub op: Option<Op>,
    pub points: Vec<(Vec3, Option<Vec3>)>,
}

/// Two placed parts, by their indices `i < j`, that share material or come within the clearance:
/// the volume they share, and the gap between them, zero where they touch or overlap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clash {
    pub i: usize,
    pub j: usize,
    pub volume: f64,
    pub gap: f64,
}

/// Parts placed rigidly, several of one shape: `parts` holds each one's shape index and placement.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembly {
    pub shapes: Vec<Primitive>,
    pub parts: Vec<(usize, brep_protocol::Placement)>,
    pub clearance: f64,
    /// Every pair that shares material or comes within the clearance, by indices `i < j`.
    pub clashes: Vec<Clash>,
}

/// The union of `tools`, or `target` less all of them when there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct Many {
    pub target: Option<Primitive>,
    pub tools: Vec<Primitive>,
    pub volume: f64,
    pub area: f64,
}

/// `a` joined to its image in a mirror: rows `[M | t]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mirror {
    pub matrix: [[f64; 4]; 3],
    pub volume: f64,
    pub area: f64,
}

/// What every acceptable result of a sound case has: its volume outside `zone` exact, and its
/// whole volume between the results at the extreme radii.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sound {
    /// Where the definition leaves the result open, if anywhere short of the whole blend.
    pub zone: Option<Primitive>,
    /// The result's volume outside `zone`.
    pub outside: f64,
    /// The volumes of the results at the largest radius and at the smallest.
    pub least: f64,
    pub most: f64,
    pub betti: [u64; 3],
    /// Points where the blend must meet a face tangentially, with that face's outward normal.
    pub contacts: Vec<(Vec3, Vec3)>,
    /// Points by the blend's seams where its curvature must carry on from what it meets.
    pub probes: Vec<Probe>,
    /// Points the result must not hold.
    pub away: Vec<Vec3>,
    /// What the result lies between, point for point.
    pub limits: Option<Limits>,
    /// Grids of rays across the blend, where the boundary must be smooth.
    pub scans: Vec<Scan>,
}

/// What a sound case's result lies between, point for point.
#[derive(Clone, Debug, PartialEq)]
pub enum Limits {
    /// Every acceptable result holds all of `inner` and nothing outside `outer`.
    Between {
        inner: Box<Shape>,
        outer: Box<Shape>,
    },
    /// The block from the origin to `block` with its upright edge at `(block[0], block[1])`
    /// rounded by a ball growing evenly from `from` at the bottom to `to` at the top. A ball
    /// reaches no farther up or down than its radius, so at each height the result lies between
    /// that edge rounded at the least and at the greatest radius the ball has within the largest
    /// radius of that height.
    Growing { block: Vec3, from: f64, to: f64 },
}

impl Limits {
    /// Whether every acceptable result holds `p`, or none does; `None` where they differ.
    pub fn at(&self, p: Vec3) -> Option<bool> {
        match self {
            Limits::Between { inner, outer } => {
                if crate::oracle::contains(inner, p) {
                    Some(true)
                } else if !crate::oracle::contains(outer, p) {
                    Some(false)
                } else {
                    None
                }
            }
            Limits::Growing { block, from, to } => {
                if !(0..3).all(|k| p[k] >= 0.0 && p[k] <= block[k]) {
                    return Some(false);
                }
                let (u, v) = (block[0] - p[0], block[1] - p[1]);
                let reach = from.max(*to);
                let at = |z: f64| from + (to - from) * (z / block[2]).clamp(0.0, 1.0);
                let (a, b) = (at(p[2] - reach), at(p[2] + reach));
                let taken = |r: f64| u < r && v < r && (r - u).ppowi(2) + (r - v).ppowi(2) > r * r;
                if taken(a.min(b)) {
                    Some(false)
                } else if !taken(a.max(b)) {
                    Some(true)
                } else {
                    None
                }
            }
        }
    }
}

/// A round grid of parallel rays across a blend. Where they meet the result its boundary must
/// have a tangent plane that turns without a break from each ray's point to the next.
#[derive(Clone, Debug, PartialEq)]
pub struct Scan {
    /// The grid's middle, where its rays start, and the unit direction they run along.
    pub from: Vec3,
    pub along: Vec3,
    /// Unit directions across the rays, square to them and to each other.
    pub across: [Vec3; 2],
    /// The grid's radius.
    pub half: f64,
    /// The blend's size, its smallest radius: what counts as near is measured against it.
    pub size: f64,
    /// Whether the boundary's curvature too must carry on from each ray's point to the next.
    pub curvature: bool,
    /// Motions, rows `[M | t]`, that leave the case as it was within the grid: the result must
    /// meet each ray moved by one where it met the ray.
    pub same: Vec<[[f64; 4]; 3]>,
}

/// A scan down the diagonal of a box's corner at `corner`, the box on the low side of it along
/// every axis; `alike` where the corner's three edges are blended alike, so that the result must
/// be unchanged by every swap of its axes.
fn corner_scan(corner: Vec3, half: f64, size: f64, curvature: bool, alike: bool) -> Scan {
    let s = 1.0 / 3f64.sqrt();
    let along = [-s, -s, -s];
    let mut same = Vec::new();
    if alike {
        for order in [[1, 2, 0], [2, 0, 1], [1, 0, 2], [0, 2, 1], [2, 1, 0]] {
            let mut m = [[0.0; 4]; 3];
            for (row, &from) in order.iter().enumerate() {
                m[row][from] = 1.0;
                m[row][3] = corner[row] - corner[from];
            }
            same.push(m);
        }
    }
    Scan {
        from: [
            corner[0] + 2.0 * s,
            corner[1] + 2.0 * s,
            corner[2] + 2.0 * s,
        ],
        along,
        across: [
            [0.5f64.sqrt(), -(0.5f64.sqrt()), 0.0],
            [1.0 / 6f64.sqrt(), 1.0 / 6f64.sqrt(), -2.0 / 6f64.sqrt()],
        ],
        half,
        size,
        curvature,
        same,
    }
}

/// The mirror in the upright plane through `at` along the level unit direction `along`, rows
/// `[M | t]`.
fn upright_mirror(at: [f64; 2], along: [f64; 2]) -> [[f64; 4]; 3] {
    let m = [
        [2.0 * along[0] * along[0] - 1.0, 2.0 * along[0] * along[1]],
        [2.0 * along[0] * along[1], 2.0 * along[1] * along[1] - 1.0],
    ];
    [
        [
            m[0][0],
            m[0][1],
            0.0,
            at[0] - m[0][0] * at[0] - m[0][1] * at[1],
        ],
        [
            m[1][0],
            m[1][1],
            0.0,
            at[1] - m[1][0] * at[0] - m[1][1] * at[1],
        ],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

/// The prism along the edge of a box's corner that runs back along `axis`, from `from` back from
/// the corner for `len`: between the edge and `section`, which runs from `[d, 0]` to `[0, d]`
/// in the distances in from the edge's two faces, or the straight line between them.
fn corner_sliver(
    corner: Vec3,
    axis: usize,
    from: f64,
    len: f64,
    d: f64,
    section: Option<Seg>,
) -> Primitive {
    let unit = |k: usize, sign: f64| {
        let mut v = [0.0; 3];
        v[k] = sign;
        v
    };
    let mut origin = corner;
    origin[axis] -= from;
    let section = section.unwrap_or(Seg::Line {
        a: [d, 0.0],
        b: [0.0, d],
    });
    Primitive::Extrude {
        frame: Frame {
            origin,
            x: unit((axis + 2) % 3, -1.0),
            y: unit((axis + 1) % 3, -1.0),
            z: unit(axis, -1.0),
        },
        profile: Profile {
            loops: vec![vec![
                Seg::Line {
                    a: [0.0, 0.0],
                    b: [d, 0.0],
                },
                section,
                Seg::Line {
                    a: [0.0, d],
                    b: [0.0, 0.0],
                },
            ]],
        },
        height: len,
    }
}

/// The quarter circle of radius `r` from `[r, 0]` to `[0, r]` that a ball in a square corner
/// leaves.
fn round_section(r: f64) -> Seg {
    Seg::Arc {
        center: [r, r],
        radius: r,
        start: -PI / 2.0,
        sweep: -PI / 2.0,
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub family: &'static str,
    /// The test it belongs to: a path naming what it checks, under the feature it exercises.
    pub test: &'static str,
    pub a: Shape,
    pub b: Shape,
    pub exact: Exact,
    /// The motion the frame-invariance check moves both operands by.
    pub motion: Rigid,
    /// An act on `a` whose result is `b`, for a feature case; booleans of `a` and `b` otherwise.
    pub feature: Option<Feature>,
    /// The case's input is invalid, and the kernel must refuse it: making `a`, or the act.
    pub refuse: bool,
    /// The case reads a STEP file the suite writes for `a`, instead of building `a`.
    pub step: bool,
    /// The case moves `a` by this similarity, rows `[M | t]`, instead of combining it with `b`.
    pub matrix: Option<[[f64; 4]; 3]>,
}

/// What a feature case does to its base solid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Fillet,
    FilletRadii,
    ChamferDistances,
    ChamferTwo,
    FilletVariable,
    Chamfer,
    Offset,
    Shell,
    Draft,
    OffsetFaces,
    DeleteFaces,
    MoveFaces,
    Thicken,
    FilletG2,
    FilletConic,
    FilletSetback,
    FilletSetbackG2,
    FilletFaces,
    FullRound,
}

impl Act {
    pub fn word(self) -> &'static str {
        match self {
            Act::Fillet => "fillet",
            Act::FilletRadii => "fillet_radii",
            Act::ChamferDistances => "chamfer_distances",
            Act::ChamferTwo => "chamfer_two",
            Act::FilletVariable => "fillet_variable",
            Act::Chamfer => "chamfer",
            Act::Offset => "offset",
            Act::Shell => "shell",
            Act::Draft => "draft",
            Act::OffsetFaces => "offset_faces",
            Act::DeleteFaces => "delete_faces",
            Act::MoveFaces => "move_faces",
            Act::Thicken => "thicken",
            Act::FilletG2 => "fillet_g2",
            Act::FilletConic => "fillet_conic",
            Act::FilletSetback => "fillet_setback",
            Act::FilletSetbackG2 => "fillet_setback_g2",
            Act::FilletFaces => "fillet_faces",
            Act::FullRound => "full_round",
        }
    }
}

/// An act on a solid: a fillet or chamfer of the edges the points lie on, by a radius or setback
/// `size`, or a fillet with each point's chain at its own radius in `radii`; an outward offset by `size`; a shell `size` thick open at the faces the points lie on;
/// or a draft of those faces by the angle `size` about the plane through `pull.0` square to
/// `pull.1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub act: Act,
    pub points: Vec<Vec3>,
    pub size: f64,
    pub pull: Option<(Vec3, Vec3)>,
    pub radii: Vec<f64>,
    /// The rigid motion a face move gives the faces, as rows `[R | t]`.
    pub matrix: Option<[[f64; 4]; 3]>,
}

impl Feature {
    pub fn moved(&self, m: &Rigid) -> Feature {
        Feature {
            points: self.points.iter().map(|&e| m.point(e)).collect(),
            pull: self.pull.map(|(o, d)| (m.point(o), m.rotate(d))),
            matrix: self.matrix.map(|a| conjugate(&a, m)),
            ..self.clone()
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({"act": self.act.word(), "points": self.points,
                                       "size": self.size});
        if let Some((o, d)) = self.pull {
            v["pull"] = serde_json::json!({"origin": o, "direction": d});
        }
        if !self.radii.is_empty() {
            v["radii"] = serde_json::json!(self.radii);
        }
        if let Some(m) = self.matrix {
            v["matrix"] = serde_json::json!(m);
        }
        v
    }
}

/// The motion `a` seen from the frame `m` moves the world to: `m ∘ a ∘ m⁻¹`.
fn conjugate(a: &[[f64; 4]; 3], m: &Rigid) -> [[f64; 4]; 3] {
    let r = &m.r;
    // R·A·Rᵀ, and the translation R·(b − A·Rᵀ·t) + t.
    let rt_t = [0, 1, 2].map(|i| (0..3).map(|k| r[k][i] * m.t[k]).sum::<f64>());
    let a_rt_t = [0, 1, 2].map(|i| (0..3).map(|k| a[i][k] * rt_t[k]).sum::<f64>());
    let inner = [0, 1, 2].map(|i| a[i][3] - a_rt_t[i]);
    [0, 1, 2].map(|i| {
        let row = [0, 1, 2].map(|j| {
            (0..3)
                .flat_map(|k| (0..3).map(move |l| (k, l)))
                .map(|(k, l)| r[i][k] * a[k][l] * r[j][l])
                .sum::<f64>()
        });
        let t = (0..3).map(|k| r[i][k] * inner[k]).sum::<f64>() + m.t[i];
        [row[0], row[1], row[2], t]
    })
}

/// What a feature family draws: the base primitive, the act, the shape it must give, and what is
/// known exactly about that shape when it is not one primitive.
pub type FeatureDraw = (Primitive, Feature, Shape, Exact);

/// A family of feature cases.
pub struct FeatureFamily {
    pub name: &'static str,
    pub version: u32,
    pub count: usize,
    pub draw: fn(&mut Rng) -> FeatureDraw,
}

fn as_draw(f: (Primitive, Feature, Primitive)) -> FeatureDraw {
    (f.0, f.1, f.2.into(), Exact::default())
}

/// Families of inputs the kernel must refuse.
pub const REFUSAL_FAMILIES: &[Family] = &[
    Family {
        name: "invalid-constructions",
        version: 2,
        count: 20,
        draw: invalid_construction,
    },
    Family {
        name: "invalid-calls",
        version: 2,
        count: 8,
        draw: invalid_call,
    },
    Family {
        name: "invalid-sizes",
        version: 1,
        count: 30,
        draw: invalid_size,
    },
];

/// How many transform cases there are: half rigid motions, a quarter scaled, a quarter mirrored.
pub const TRANSFORMS: usize = 40;

/// How many affine cases there are: a third stretched along axes, a third sheared, a third
/// stretched with a mirror.
pub const STRETCHES: usize = 30;

/// An invertible map that is no similarity, `m`'s rotation after it, as rows `[M | t]`: a stretch
/// along the axes by 0.5 to 2 (no two factors within 0.2 of each other), the same with one axis
/// sheared along another by 0.2 to 0.8, or with one axis turned over.
fn stretch(r: &mut Rng, m: &Rigid) -> [[f64; 4]; 3] {
    let kind = r.next_u64() % 3;
    let mut s = [1.0f64; 3];
    while (0..3).any(|i| (s[i] - s[(i + 1) % 3]).abs() < 0.2) {
        s = [0; 3].map(|_| r.range(0.5, 2.0));
    }
    let mut a = [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]];
    let i = (r.next_u64() % 3) as usize;
    match kind {
        1 => {
            let j = (i + 1 + (r.next_u64() % 2) as usize) % 3;
            a[i][j] = r.range(0.2, 0.8) * s[i];
        }
        2 => a[i][i] = -a[i][i],
        _ => {}
    }
    [0, 1, 2].map(|i| {
        let row = [0, 1, 2].map(|j| (0..3).map(|k| m.r[i][k] * a[k][j]).sum::<f64>());
        [row[0], row[1], row[2], m.t[i]]
    })
}

/// A similarity: `m`'s rotation and translation, scaled by 0.5 to 2 or mirrored in a quarter of
/// draws each; as rows `[M | t]`.
fn similarity(r: &mut Rng, m: &Rigid) -> [[f64; 4]; 3] {
    let (factor, mirror) = match r.next_u64() % 4 {
        2 => (r.range(0.5, 2.0), 1.0),
        3 => (1.0, -1.0),
        _ => (1.0, 1.0),
    };
    [0, 1, 2].map(|i| {
        [
            factor * m.r[i][0] * mirror,
            factor * m.r[i][1],
            factor * m.r[i][2],
            m.t[i],
        ]
    })
}

/// Families of STEP files the suite writes for a kernel to read.
pub const STEP_FAMILIES: &[Family] = &[
    Family {
        name: "step-corpus",
        version: 1,
        count: 30,
        draw: |r| pair(step_prism(r)),
    },
    Family {
        name: "step-revolved",
        version: 1,
        count: 30,
        draw: |r| pair(step_revolved(r)),
    },
    Family {
        name: "step-schemas",
        version: 1,
        count: 30,
        draw: |r| pair(step_schema(r)),
    },
    Family {
        name: "step-tessellated",
        version: 1,
        count: 20,
        draw: |r| pair(step_tessellated(r)),
    },
];

pub const FEATURE_REFUSALS: &[FeatureFamily] = &[
    FeatureFamily {
        name: "invalid-acts",
        version: 3,
        count: 18,
        draw: invalid_act,
    },
    FeatureFamily {
        name: "invalid-new-acts",
        version: 2,
        count: 30,
        draw: invalid_new_act,
    },
    FeatureFamily {
        name: "fillet-over-limit",
        version: 1,
        count: 16,
        draw: hard::fillet_over_limit,
    },
    FeatureFamily {
        name: "invalid-blend-sizes",
        version: 1,
        count: 12,
        draw: invalid_blend_size,
    },
    FeatureFamily {
        name: "invalid-points",
        version: 1,
        count: 18,
        draw: invalid_point,
    },
];

pub const FEATURE_FAMILIES: &[FeatureFamily] = &[
    FeatureFamily {
        name: "rounded-boxes",
        version: 1,
        count: 20,
        draw: rounded_box,
    },
    FeatureFamily {
        name: "offsets",
        version: 1,
        count: 30,
        draw: offsets,
    },
    FeatureFamily {
        name: "shells",
        version: 1,
        count: 24,
        draw: shells,
    },
    FeatureFamily {
        name: "drafts",
        version: 1,
        count: 24,
        draw: drafts,
    },
    FeatureFamily {
        name: "fillet-prisms",
        version: 1,
        count: 30,
        draw: |r| as_draw(blended_prism(r, Act::Fillet)),
    },
    FeatureFamily {
        name: "chamfer-prisms",
        version: 1,
        count: 30,
        draw: |r| as_draw(blended_prism(r, Act::Chamfer)),
    },
    FeatureFamily {
        name: "fillet-rims",
        version: 1,
        count: 30,
        draw: |r| as_draw(blended_rim(r, Act::Fillet)),
    },
    FeatureFamily {
        name: "chamfer-rims",
        version: 1,
        count: 30,
        draw: |r| as_draw(blended_rim(r, Act::Chamfer)),
    },
    FeatureFamily {
        name: "fillet-chains",
        version: 1,
        count: 24,
        draw: fillet_chain,
    },
    FeatureFamily {
        name: "rounded-prisms",
        version: 1,
        count: 24,
        draw: rounded_prism,
    },
    FeatureFamily {
        name: "opened-l-prisms",
        version: 2,
        count: 20,
        draw: opened_l_prism,
    },
    FeatureFamily {
        name: "offset-faces",
        version: 1,
        count: 30,
        draw: offset_face,
    },
    FeatureFamily {
        name: "hole-rims",
        version: 1,
        count: 30,
        draw: hole_rim,
    },
    FeatureFamily {
        name: "shelled-rounds",
        version: 1,
        count: 24,
        draw: shelled_round,
    },
    FeatureFamily {
        name: "closing-drafts",
        version: 1,
        count: 24,
        draw: closing_drafts,
    },
    FeatureFamily {
        name: "offset-l-prisms",
        version: 1,
        count: 20,
        draw: offset_l_prism,
    },
    FeatureFamily {
        name: "closing-offsets",
        version: 1,
        count: 24,
        draw: closing_offset,
    },
    FeatureFamily {
        name: "delete-faces",
        version: 1,
        count: 24,
        draw: delete_face,
    },
    FeatureFamily {
        name: "move-faces",
        version: 1,
        count: 24,
        draw: move_face,
    },
    FeatureFamily {
        name: "thicken",
        version: 1,
        count: 24,
        draw: thicken_face,
    },
    FeatureFamily {
        name: "chamfer-uneven-corners",
        version: 1,
        count: 20,
        draw: uneven_chamfer_corner,
    },
    FeatureFamily {
        name: "chamfer-uneven",
        version: 1,
        count: 30,
        draw: uneven_chamfer,
    },
    FeatureFamily {
        name: "fillet-variable",
        version: 1,
        count: 20,
        draw: variable_fillet,
    },
    FeatureFamily {
        name: "fillet-uneven-corners",
        version: 1,
        count: 20,
        draw: uneven_corner,
    },
    FeatureFamily {
        name: "fillet-extreme-angles",
        version: 1,
        count: 24,
        draw: hard::extreme_angle,
    },
    FeatureFamily {
        name: "fillet-limits",
        version: 1,
        count: 20,
        draw: hard::fillet_limit,
    },
    FeatureFamily {
        name: "fillet-short-edges",
        version: 2,
        count: 12,
        draw: hard::short_edges,
    },
    FeatureFamily {
        name: "fillet-swallowed-face",
        version: 1,
        count: 12,
        draw: hard::swallowed_face,
    },
    FeatureFamily {
        name: "offset-past-holes",
        version: 1,
        count: 20,
        draw: hard::offset_past_hole,
    },
    FeatureFamily {
        name: "fillet-chains-tight",
        version: 1,
        count: 20,
        draw: robust::tight_chain,
    },
    FeatureFamily {
        name: "fillet-mixed-corners",
        version: 1,
        count: 20,
        draw: robust::mixed_corner,
    },
    FeatureFamily {
        name: "fillet-conic",
        version: 1,
        count: 24,
        draw: blends::conic_prism,
    },
    FeatureFamily {
        name: "fillet-g2",
        version: 4,
        count: 24,
        draw: blends::g2_prism,
    },
    FeatureFamily {
        name: "fillet-setbacks",
        version: 1,
        count: 20,
        draw: |r| blends::setback_corner(r, Act::FilletSetback),
    },
    FeatureFamily {
        name: "fillet-setbacks-g2",
        version: 2,
        count: 20,
        draw: |r| blends::setback_corner(r, Act::FilletSetbackG2),
    },
    FeatureFamily {
        name: "fillet-faces",
        version: 1,
        count: 24,
        draw: blends::face_fillet,
    },
    FeatureFamily {
        name: "full-rounds",
        version: 1,
        count: 20,
        draw: blends::full_round,
    },
];

/// SplitMix64 (G. L. Steele, D. Lea and C. H. Flood, "Fast splittable pseudorandom number
/// generators", OOPSLA 2014): small, fast, and the same on every platform. The constants are
/// those of the generator as published; a different one would draw different cases.
pub struct Rng(u64);

impl Rng {
    const STEP: u64 = 0x9e37_79b9_7f4a_7c15;
    const MIXES: [(u32, u64); 2] = [(30, 0xbf58_476d_1ce4_e5b9), (27, 0x94d0_49bb_1331_11eb)];

    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(Self::STEP);
        let mixed = Self::MIXES.iter().fold(self.0, |z, &(shift, by)| {
            (z ^ (z >> shift)).wrapping_mul(by)
        });
        mixed ^ (mixed >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// A value on a grid of `step` in [lo, hi]: grids put faces on one plane exactly.
    pub fn grid(&mut self, lo: f64, hi: f64, step: f64) -> f64 {
        let n = ((hi - lo) / step).floor() as u64;
        lo + step * (self.next_u64() % (n + 1)) as f64
    }

    pub fn motion(&mut self) -> Rigid {
        let u = [self.unit(), self.unit(), self.unit()];
        let t = [
            self.range(-50.0, 50.0),
            self.range(-50.0, 50.0),
            self.range(-50.0, 50.0),
        ];
        Rigid::uniform(u, t)
    }

    pub fn direction(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let t = self.range(0.0, 2.0 * PI);
        let s = (1.0 - z * z).sqrt();
        [s * t.pcos(), s * t.psin(), z]
    }
}

/// `debug` with every float rounded to nine significant digits. The values a case holds come out of
/// `sin`, `cos` and the like, whose last bit differs between platforms' maths libraries; rounded,
/// they hash the same everywhere, and what cancellation leaves of a zero hashes as zero. A number
/// with no point and no exponent is an integer, and is kept.
fn rounded_floats(debug: &str) -> String {
    let b = debug.as_bytes();
    let mut out = String::with_capacity(debug.len());
    let mut i = 0;
    while i < b.len() {
        let starts =
            b[i].is_ascii_digit() || (b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit));
        let after_word = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if !starts || after_word {
            out.push(b[i] as char);
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < b.len()
            && (b[j].is_ascii_digit()
                || b[j] == b'.'
                || b[j] == b'e'
                || (b[j] == b'-' && b[j - 1] == b'e'))
        {
            j += 1;
        }
        let token = &debug[i..j];
        match token.parse::<f64>() {
            // Below a picometre a value is what cancellation left of a zero, whose bits and sign
            // differ between platforms.
            Ok(v) if token.contains(['.', 'e']) && v.abs() < 1e-12 => out.push_str("0e0"),
            Ok(v) if token.contains(['.', 'e']) => out.push_str(&format!("{v:.8e}")),
            _ => out.push_str(token),
        }
        i = j;
    }
    out
}

/// FNV-1a, for ids that are the same on every platform and Rust release.
pub(crate) fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A family: its name, its version, and how it draws one case.
pub struct Family {
    pub name: &'static str,
    pub version: u32,
    pub count: usize,
    pub draw: fn(&mut Rng) -> Drawn,
}

/// A family's two operands and what it knows of their booleans.
pub type Drawn = (Shape, Shape, Exact);

/// A pair of primitives as operands.
fn pair((a, b, e): (Primitive, Primitive, Exact)) -> Drawn {
    (a.into(), b.into(), e)
}

/// A pair whose union is one piece with no tunnel or void, however the two touch: pieces that only
/// touch are one piece, as their point set is connected.
fn connected_union((a, b, mut e): (Primitive, Primitive, Exact)) -> Drawn {
    e.betti_union.get_or_insert([1, 0, 0]);
    (a.into(), b.into(), e)
}

/// The families that test a primitive's constructor: `a` is made by the constructor itself, never
/// by its stand-in, so a kernel without one is unsupported here and nowhere else.
pub const CONSTRUCTORS: [&str; 5] = ["boxes", "cylinders", "spheres", "tori", "cones"];

pub const FAMILIES: &[Family] = &[
    Family {
        name: "box-box",
        version: 1,
        count: 60,
        draw: |r| pair(box_box(r)),
    },
    Family {
        name: "coaxial-cylinders",
        version: 1,
        count: 40,
        draw: |r| pair(coaxial_cylinders(r)),
    },
    Family {
        name: "steinmetz",
        version: 1,
        count: 20,
        draw: |r| pair(steinmetz(r)),
    },
    Family {
        name: "crossing-cylinders",
        version: 1,
        count: 40,
        draw: |r| pair(crossing_cylinders(r)),
    },
    Family {
        name: "sphere-sphere",
        version: 1,
        count: 40,
        draw: |r| pair(sphere_sphere(r)),
    },
    Family {
        name: "sphere-halfspace",
        version: 1,
        count: 30,
        draw: |r| pair(sphere_halfspace(r)),
    },
    Family {
        name: "torus-halfspace",
        version: 1,
        count: 20,
        draw: |r| pair(torus_halfspace(r)),
    },
    Family {
        name: "spheres-nearly-touching",
        version: 1,
        count: 40,
        draw: |r| pair(spheres_nearly_touching(r)),
    },
    Family {
        name: "seams-poles",
        version: 1,
        count: 35,
        draw: |r| pair(hard::seams_poles(r)),
    },
    Family {
        name: "far-from-origin",
        version: 1,
        count: 30,
        draw: |r| pair(hard::far_away(r)),
    },
    Family {
        name: "circle-contact",
        version: 1,
        count: 32,
        draw: |r| pair(hard::circle_contact(r)),
    },
    Family {
        name: "near-steinmetz",
        version: 1,
        count: 24,
        draw: |r| pair(hard::near_steinmetz(r)),
    },
    Family {
        name: "crossing-near-tangent",
        version: 1,
        count: 30,
        draw: |r| pair(robust::crossing_near_tangent(r)),
    },
    Family {
        name: "sphere-shaved",
        version: 1,
        count: 30,
        draw: |r| pair(sphere_shaved(r)),
    },
    Family {
        name: "parallel-cylinders",
        version: 1,
        count: 30,
        draw: |r| pair(parallel_cylinders(r)),
    },
    Family {
        name: "coaxial-nearly-equal",
        version: 1,
        count: 30,
        draw: |r| pair(coaxial_nearly_equal(r)),
    },
    Family {
        name: "boxes-touching",
        version: 1,
        count: 30,
        draw: |r| connected_union(boxes_touching(r)),
    },
    Family {
        name: "spheres-touching",
        version: 1,
        count: 20,
        draw: |r| connected_union(spheres_touching(r)),
    },
    Family {
        name: "spline-prisms",
        version: 2,
        count: 60,
        draw: |r| pair(spline_prisms(r)),
    },
    Family {
        name: "cone-cylinder-coaxial",
        version: 1,
        count: 30,
        draw: |r| pair(cone_cylinder_coaxial(r)),
    },
    Family {
        name: "cone-box",
        version: 1,
        count: 30,
        draw: |r| pair(cone_box(r)),
    },
    Family {
        name: "cones-touching",
        version: 1,
        count: 32,
        draw: |r| connected_union(cones_touching(r)),
    },
    Family {
        name: "inscribed",
        version: 1,
        count: 32,
        draw: |r| pair(inscribed(r)),
    },
    Family {
        name: "near-coincident-boxes",
        version: 1,
        count: 40,
        draw: |r| pair(near_coincident_boxes(r)),
    },
    Family {
        name: "scaled",
        version: 1,
        count: 40,
        draw: |r| pair(scaled(r)),
    },
    Family {
        name: "drilled-plates",
        version: 1,
        count: 20,
        draw: drilled_plates,
    },
    Family {
        name: "stacked-boxes",
        version: 1,
        count: 20,
        draw: stacked_boxes,
    },
    Family {
        name: "sphere-clusters",
        version: 1,
        count: 20,
        draw: sphere_clusters,
    },
    Family {
        name: "random-chains",
        version: 1,
        count: 40,
        draw: random_chains,
    },
    Family {
        name: "separated",
        version: 1,
        count: 40,
        draw: |r| pair(separated(r)),
    },
    Family {
        name: "voids",
        version: 1,
        count: 30,
        draw: |r| pair(voids(r)),
    },
    Family {
        name: "assemblies",
        version: 1,
        count: 30,
        draw: assembly,
    },
    Family {
        name: "patterns",
        version: 1,
        count: 30,
        draw: pattern,
    },
    Family {
        name: "normals",
        version: 1,
        count: 40,
        draw: normals_case,
    },
    Family {
        name: "lofts-smooth",
        version: 1,
        count: 24,
        draw: smooth_loft,
    },
    Family {
        name: "lofts-equal-sections",
        version: 1,
        count: 12,
        draw: equal_sections_loft,
    },
    Family {
        name: "mirror-joins",
        version: 1,
        count: 32,
        draw: mirror_join,
    },
    Family {
        name: "bounding-boxes",
        version: 1,
        count: 30,
        draw: bounding_case,
    },
    Family {
        name: "nearest-points",
        version: 1,
        count: 30,
        draw: nearest_case,
    },
    Family {
        name: "nearest-in-holes",
        version: 1,
        count: 12,
        draw: nearest_in_a_hole,
    },
    Family {
        name: "boundary-points",
        version: 1,
        count: 30,
        draw: boundary_case,
    },
    Family {
        name: "self-booleans",
        version: 1,
        count: 30,
        draw: itself_case,
    },
    Family {
        name: "splits",
        version: 1,
        count: 30,
        draw: split_case,
    },
    Family {
        name: "sections",
        version: 1,
        count: 30,
        draw: section_case,
    },
    Family {
        name: "sewing",
        version: 1,
        count: 24,
        draw: sew_case,
    },
    Family {
        name: "curvatures",
        version: 1,
        count: 30,
        draw: drawing::curvature_case,
    },
    Family {
        name: "projections",
        version: 1,
        count: 30,
        draw: drawing::projection_case,
    },
    Family {
        name: "displays",
        version: 1,
        count: 20,
        draw: drawing::display_case,
    },
    Family {
        name: "line-contact",
        version: 1,
        count: 32,
        draw: |r| connected_union(line_contact(r)),
    },
    Family {
        name: "flush-bores",
        version: 1,
        count: 20,
        draw: |r| pair(flush_bore(r)),
    },
    Family {
        name: "flush-pockets",
        version: 1,
        count: 30,
        draw: |r| pair(flush_pocket(r)),
    },
    Family {
        name: "thin-solids",
        version: 1,
        count: 30,
        draw: |r| pair(thin_solid(r)),
    },
    Family {
        name: "islands",
        version: 1,
        count: 20,
        draw: island,
    },
    Family {
        name: "long-chains",
        version: 1,
        count: 20,
        draw: long_chains,
    },
    Family {
        name: "many-holes",
        version: 1,
        count: 6,
        draw: many_holes,
    },
    Family {
        name: "dense-holes",
        version: 1,
        count: 24,
        draw: robust::dense_holes,
    },
    Family {
        name: "slabs",
        version: 1,
        count: 40,
        draw: |r| pair(slabs(r)),
    },
    Family {
        name: "sweeps",
        version: 1,
        count: 30,
        draw: |r| pair(sweeps(r)),
    },
    Family {
        name: "sweeps-spline",
        version: 1,
        count: 24,
        draw: |r| pair(path_sweeps(r)),
    },
    Family {
        name: "boxes",
        version: 1,
        count: 12,
        draw: |r| pair(constructed(r, 0)),
    },
    Family {
        name: "cylinders",
        version: 1,
        count: 12,
        draw: |r| pair(constructed(r, 1)),
    },
    Family {
        name: "spheres",
        version: 1,
        count: 12,
        draw: |r| pair(constructed(r, 2)),
    },
    Family {
        name: "tori",
        version: 1,
        count: 12,
        draw: |r| pair(constructed(r, 3)),
    },
    Family {
        name: "cones",
        version: 1,
        count: 12,
        draw: |r| pair(constructed(r, 4)),
    },
    Family {
        name: "extrusions",
        version: 1,
        count: 40,
        draw: |r| pair(extrusions(r)),
    },
    Family {
        name: "revolutions",
        version: 1,
        count: 40,
        draw: |r| pair(revolutions(r)),
    },
    Family {
        name: "helices",
        version: 1,
        count: 30,
        draw: |r| pair(helices(r)),
    },
    Family {
        name: "thread-runout-m8",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::runout_join(parts::fasteners::M8, 0.0),
    },
    Family {
        name: "thread-runout-m12",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::runout_join(parts::fasteners::M12, 0.0),
    },
    Family {
        name: "thread-runout-m8-on-the-shank",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::runout_join(parts::fasteners::M8, -1.0),
    },
    Family {
        name: "thread-runout-m12-on-the-cone",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::runout_join(parts::fasteners::M12, 0.4),
    },
    Family {
        name: "lofts",
        version: 1,
        count: 30,
        draw: |r| pair(lofts(r)),
    },
    Family {
        name: "chain-after-union-l",
        version: 2,
        count: 20,
        draw: chains::after_union_l,
    },
    Family {
        name: "chain-rim-after-cut",
        version: 1,
        count: 20,
        draw: chains::rim_after_cut,
    },
    Family {
        name: "chain-rims-of-a-pattern",
        version: 1,
        count: 20,
        draw: chains::rims_of_a_pattern,
    },
    Family {
        name: "chain-top-loop-after-corners",
        version: 1,
        count: 20,
        draw: chains::top_loop_after_corners,
    },
    Family {
        name: "chain-edges-of-a-chamfer",
        version: 1,
        count: 20,
        draw: chains::edges_of_a_chamfer,
    },
    Family {
        name: "chain-chamfers-beside-fillets",
        version: 1,
        count: 20,
        draw: chains::chamfers_beside_fillets,
    },
    Family {
        name: "chain-two-calls-as-one",
        version: 1,
        count: 20,
        draw: chains::two_calls_as_one,
    },
    Family {
        name: "chain-across-a-mirror-join",
        version: 1,
        count: 20,
        draw: chains::across_a_mirror_join,
    },
    Family {
        name: "chain-shell-after-fillet",
        version: 1,
        count: 20,
        draw: chains::shell_after_fillet,
    },
    Family {
        name: "chain-offset-after-cut",
        version: 1,
        count: 20,
        draw: chains::offset_after_cut,
    },
    Family {
        name: "chain-offset-twice",
        version: 1,
        count: 20,
        draw: chains::offset_twice,
    },
    Family {
        name: "chain-move-and-back",
        version: 1,
        count: 20,
        draw: chains::move_and_back,
    },
    Family {
        name: "chain-cut-through-fillets",
        version: 1,
        count: 20,
        draw: chains::cut_through_fillets,
    },
    Family {
        name: "chain-round-trip-after-chain",
        version: 1,
        count: 20,
        draw: chains::round_trip_after_chain,
    },
    Family {
        name: "part-gear-spur-pinion",
        version: 1,
        count: 1,
        draw: |_| parts::spur_gear(18, 1.5, 8.0, 0),
    },
    Family {
        name: "part-gear-spur-wheel",
        version: 1,
        count: 1,
        draw: |_| parts::spur_gear(40, 1.5, 8.0, 0),
    },
    Family {
        name: "part-gear-spur-lightened",
        version: 1,
        count: 1,
        draw: |_| parts::spur_gear(60, 1.0, 6.0, 6),
    },
    Family {
        name: "part-gear-helical-right-15",
        version: 1,
        count: 1,
        draw: |_| parts::helical_gear(24, 1.5, 10.0, 15.0, 1.0),
    },
    Family {
        name: "part-gear-helical-left-30",
        version: 1,
        count: 1,
        draw: |_| parts::helical_gear(20, 2.0, 12.0, 30.0, -1.0),
    },
    Family {
        name: "part-gear-internal",
        version: 1,
        count: 1,
        draw: |_| parts::internal_gear(48, 1.0, 8.0, 8),
    },
    Family {
        name: "part-gear-rack",
        version: 1,
        count: 1,
        draw: |_| parts::rack(10, 2.0, 12.0),
    },
    Family {
        name: "part-gears-timing-pulley",
        version: 1,
        count: 1,
        draw: parts::timing_pulley,
    },
    Family {
        name: "part-gears-sprocket",
        version: 1,
        count: 1,
        draw: parts::sprocket,
    },
    Family {
        name: "part-shafts-splined",
        version: 1,
        count: 1,
        draw: parts::splined_shaft,
    },
    // Fasteners at common metric sizes, one case each: the variants differ in what they make a
    // kernel do, not in random sizes.
    Family {
        name: "part-nut-m3",
        version: 1,
        count: 1,
        draw: |_| {
            parts::fasteners::hex_nut(parts::fasteners::M3, parts::fasteners::nut_dims(3.0).1)
        },
    },
    Family {
        name: "part-nut-m6",
        version: 1,
        count: 1,
        draw: |_| {
            parts::fasteners::hex_nut(parts::fasteners::M6, parts::fasteners::nut_dims(6.0).1)
        },
    },
    Family {
        name: "part-nut-m12",
        version: 1,
        count: 1,
        draw: |_| {
            parts::fasteners::hex_nut(parts::fasteners::M12, parts::fasteners::nut_dims(12.0).1)
        },
    },
    Family {
        // A thin nut on a fine pitch: the countersinks from both faces reach
        // most of the way along the few turns between them.
        name: "part-nut-thin-m10-fine",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::hex_nut(parts::fasteners::M10_FINE, 5.0),
    },
    Family {
        // Shorter than its thread length: threaded up to the runout under the head.
        name: "part-socket-screw-m3x8",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::socket_head_screw(parts::fasteners::M3, 8.0),
    },
    Family {
        name: "part-socket-screw-m8x40",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::socket_head_screw(parts::fasteners::M8, 40.0),
    },
    Family {
        name: "part-socket-screw-m12x60",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::socket_head_screw(parts::fasteners::M12, 60.0),
    },
    Family {
        name: "part-hex-screw-m5x10",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::hex_head_screw(parts::fasteners::M5, 10.0),
    },
    Family {
        name: "part-hex-screw-m6x16",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::hex_head_screw(parts::fasteners::M6, 16.0),
    },
    Family {
        name: "part-hex-screw-m10x30",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::hex_head_screw(parts::fasteners::M10, 30.0),
    },
    Family {
        name: "part-washer-m3",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::plain_washer(parts::fasteners::M3),
    },
    Family {
        name: "part-washer-m12",
        version: 2,
        count: 1,
        draw: |_| parts::fasteners::plain_washer(parts::fasteners::M12),
    },
    Family {
        name: "part-fasteners-lock-washer",
        version: 1,
        count: 1,
        draw: parts::lock_washer,
    },
    Family {
        name: "part-insert-m3",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::heat_set_insert(parts::fasteners::M3),
    },
    Family {
        name: "part-insert-m5",
        version: 1,
        count: 1,
        draw: |_| parts::fasteners::heat_set_insert(parts::fasteners::M5),
    },
    Family {
        name: "part-fasteners-compression-spring",
        version: 1,
        count: 2,
        draw: parts::compression_spring,
    },
    Family {
        name: "part-shafts-stepped",
        version: 1,
        count: 2,
        draw: parts::stepped_shaft,
    },
    Family {
        name: "part-shafts-pipe-flange",
        version: 1,
        count: 1,
        draw: parts::pipe_flange,
    },
    Family {
        name: "part-shafts-v-belt-pulley",
        version: 1,
        count: 1,
        draw: parts::v_pulley,
    },
    Family {
        name: "part-shafts-windowed-wheel",
        version: 1,
        count: 1,
        draw: parts::windowed_wheel,
    },
    Family {
        name: "part-shafts-turbine-disc",
        version: 1,
        count: 1,
        draw: parts::turbine_disc,
    },
    Family {
        name: "part-brackets-l-bracket",
        version: 1,
        count: 1,
        draw: parts::l_bracket,
    },
    Family {
        name: "part-brackets-mounting-plate",
        version: 1,
        count: 1,
        draw: parts::mounting_plate,
    },
    Family {
        name: "part-brackets-gusset",
        version: 1,
        count: 1,
        draw: parts::gusset_bracket,
    },
    Family {
        name: "part-brackets-bent-z",
        version: 1,
        count: 1,
        draw: parts::bent_bracket,
    },
    Family {
        name: "part-brackets-clevis",
        version: 1,
        count: 1,
        draw: parts::clevis,
    },
    Family {
        name: "part-housings-enclosure",
        version: 1,
        count: 1,
        draw: parts::enclosure,
    },
    Family {
        name: "part-housings-gearbox",
        version: 1,
        count: 1,
        draw: parts::gearbox_housing,
    },
    Family {
        name: "part-housings-pillow-block",
        version: 1,
        count: 1,
        draw: parts::pillow_block,
    },
    Family {
        name: "part-housings-motor-mount",
        version: 1,
        count: 1,
        draw: parts::motor_mount,
    },
    Family {
        name: "part-housings-valve-body",
        version: 1,
        count: 1,
        draw: parts::valve_body,
    },
    Family {
        name: "part-molded-drafted-boss-and-rib",
        version: 1,
        count: 1,
        draw: parts::drafted_boss_and_rib,
    },
    Family {
        name: "part-molded-snap-fit",
        version: 1,
        count: 1,
        draw: parts::snap_fit,
    },
    Family {
        name: "part-molded-bottle-cap",
        version: 2,
        count: 1,
        draw: |_| parts::bottle_cap(),
    },
    Family {
        name: "part-machined-pocketed-block",
        version: 1,
        count: 1,
        draw: parts::pocketed_block,
    },
    Family {
        name: "part-machined-t-slot-plate",
        version: 1,
        count: 1,
        draw: parts::t_slot_plate,
    },
    Family {
        name: "part-machined-dovetail-slide",
        version: 1,
        count: 1,
        draw: parts::dovetail_slide,
    },
    Family {
        name: "part-machined-plate-cam",
        version: 1,
        count: 1,
        draw: parts::plate_cam,
    },
    Family {
        name: "part-structural-i-beam",
        version: 1,
        count: 1,
        draw: parts::i_beam,
    },
    Family {
        name: "part-structural-channel",
        version: 1,
        count: 1,
        draw: parts::channel,
    },
    Family {
        name: "part-structural-angle",
        version: 1,
        count: 1,
        draw: parts::angle_section,
    },
    Family {
        name: "part-structural-flanged-tube",
        version: 1,
        count: 1,
        draw: parts::flanged_tube,
    },
    Family {
        name: "part-gears-meshing-pinion-and-wheel",
        version: 1,
        count: 1,
        draw: |_| parts::meshing_pair(18, 40, 1.5, 0.08, 6.0),
    },
    Family {
        name: "part-gears-meshing-equal",
        version: 1,
        count: 1,
        draw: |_| parts::meshing_pair(20, 20, 2.0, 0.05, 8.0),
    },
];

/// Families whose operands always touch without sharing volume.
const TOUCHING: [&str; 3] = ["boxes-touching", "spheres-touching", "cones-touching"];

/// Every family's cases for `seed`, in a fixed order.
pub fn cases(seed: u64) -> Vec<Case> {
    let mut out = Vec::new();
    for f in FAMILIES {
        let mut rng = Rng::new(seed ^ fnv(f.name));
        for _ in 0..f.count {
            let (a, b, mut exact) = (f.draw)(&mut rng);
            for (s, slot) in [(&a, &mut exact.betti_a), (&b, &mut exact.betti_b)] {
                if let (Shape::Prim(p), None) = (s, *slot) {
                    *slot = prim_betti(p);
                }
            }
            // Operands that share volume, or whose family has them touch, are no distance apart.
            let overlap = exact.intersection_volume.is_some_and(|v| v > 0.0);
            if exact.distance.is_none() && (overlap || TOUCHING.contains(&f.name)) {
                exact.distance = Some(0.0);
            }
            let motion = rng.motion();
            let mut key = format!("{}@{}|{}|{}", f.name, f.version, a.key(), b.key());
            // Cases whose operands are stand-ins are told apart by what they hold instead.
            let held = [
                exact.assembly.as_ref().map(|x| format!("{x:?}")),
                exact.many.as_ref().map(|x| format!("{x:?}")),
                exact.mirror.as_ref().map(|x| format!("{x:?}")),
                exact.normals.as_ref().map(|x| format!("{x:?}")),
                exact.loft.as_ref().map(|x| format!("{x:?}")),
                exact.curvature.as_ref().map(|x| format!("{x:?}")),
                exact.drawing.as_ref().map(|x| format!("{x:?}")),
                exact.chain.as_ref().map(|x| format!("{x:?}")),
            ];
            for h in held.into_iter().flatten() {
                key = format!("{key}|{:016x}", fnv(&rounded_floats(&h)));
            }
            out.push(Case {
                id: format!("{}-{:016x}", f.name, fnv(&rounded_floats(&key))),
                family: f.name,
                test: "",
                a,
                b,
                exact,
                motion,
                feature: None,
                refuse: false,
                step: false,
                matrix: None,
            });
        }
    }
    for f in STEP_FAMILIES {
        let mut rng = Rng::new(seed ^ fnv(f.name));
        for _ in 0..f.count {
            let (a, b, exact) = (f.draw)(&mut rng);
            let motion = rng.motion();
            let mut key = format!("{}@{}|{}", f.name, f.version, a.key());
            if let Some(how) = exact.step_as {
                key = format!("{key}|{how:?}");
            }
            out.push(Case {
                id: format!("{}-{:016x}", f.name, fnv(&rounded_floats(&key))),
                family: f.name,
                test: "",
                a,
                b,
                exact,
                motion,
                feature: None,
                refuse: false,
                step: true,
                matrix: None,
            });
        }
    }
    {
        let name = "transforms";
        let mut rng = Rng::new(seed ^ fnv(name));
        for _ in 0..TRANSFORMS {
            let a = any_primitive(&mut rng);
            let motion = rng.motion();
            let matrix = similarity(&mut rng, &motion);
            let nums: Vec<String> = matrix
                .iter()
                .flatten()
                .map(|v| format!("{v:.12e}"))
                .collect();
            let key = format!("{name}@1|{}|{}", a.key(), nums.join(","));
            out.push(Case {
                id: format!("{name}-{:016x}", fnv(&rounded_floats(&key))),
                family: name,
                test: "",
                a: a.into(),
                b: axis_box([0.0; 3], [1.0; 3]).into(),
                exact: Exact::default(),
                motion,
                feature: None,
                refuse: false,
                step: false,
                matrix: Some(matrix),
            });
        }
    }
    {
        let name = "stretches";
        let mut rng = Rng::new(seed ^ fnv(name));
        for _ in 0..STRETCHES {
            let a = any_primitive(&mut rng);
            let motion = rng.motion();
            let matrix = stretch(&mut rng, &motion);
            let nums: Vec<String> = matrix
                .iter()
                .flatten()
                .map(|v| format!("{v:.12e}"))
                .collect();
            let key = format!("{name}@1|{}|{}", a.key(), nums.join(","));
            out.push(Case {
                id: format!("{name}-{:016x}", fnv(&rounded_floats(&key))),
                family: name,
                test: "",
                a: a.into(),
                b: axis_box([0.0; 3], [1.0; 3]).into(),
                exact: Exact::default(),
                motion,
                feature: None,
                refuse: false,
                step: false,
                matrix: Some(matrix),
            });
        }
    }
    for f in REFUSAL_FAMILIES {
        let mut rng = Rng::new(seed ^ fnv(f.name));
        for _ in 0..f.count {
            let (a, b, exact) = (f.draw)(&mut rng);
            let motion = rng.motion();
            let mut key = format!("{}@{}|{}|{}", f.name, f.version, a.key(), b.key());
            let held = [
                exact.assembly.as_ref().map(|x| format!("{x:?}")),
                exact.loft.as_ref().map(|x| format!("{x:?}")),
            ];
            for h in held.into_iter().flatten() {
                key = format!("{key}|{:016x}", fnv(&rounded_floats(&h)));
            }
            out.push(Case {
                id: format!("{}-{:016x}", f.name, fnv(&rounded_floats(&key))),
                family: f.name,
                test: "",
                a,
                b,
                exact,
                motion,
                feature: None,
                refuse: true,
                step: false,
                matrix: None,
            });
        }
    }
    for (f, refuse) in FEATURE_FAMILIES
        .iter()
        .map(|f| (f, false))
        .chain(FEATURE_REFUSALS.iter().map(|f| (f, true)))
    {
        let mut rng = Rng::new(seed ^ fnv(f.name));
        for _ in 0..f.count {
            let (base, feature, want, exact) = (f.draw)(&mut rng);
            let motion = rng.motion();
            let nums: Vec<String> = feature
                .points
                .iter()
                .flatten()
                .chain([&feature.size])
                .chain(&feature.radii)
                .map(|v| format!("{v:.12e}"))
                .collect();
            let key = format!("{}@{}|{}|{}", f.name, f.version, base.key(), nums.join(","));
            out.push(Case {
                id: format!("{}-{:016x}", f.name, fnv(&rounded_floats(&key))),
                family: f.name,
                test: "",
                a: base.into(),
                b: want,
                exact,
                motion,
                feature: Some(feature),
                refuse,
                step: false,
                matrix: None,
            });
        }
    }
    for c in &mut out {
        c.test = test_path(c);
    }
    out
}

fn axis_box(lo: Vec3, hi: Vec3) -> Primitive {
    Primitive::Box {
        frame: Frame::at(lo),
        size: [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
    }
}

/// Two axis-aligned boxes on a 0.5 mm grid, so faces often lie on one plane; their intersection is
/// the box of the overlapping ranges.
fn box_box(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let mut corner = || {
        let lo = [
            r.grid(0.0, 10.0, 0.5),
            r.grid(0.0, 10.0, 0.5),
            r.grid(0.0, 10.0, 0.5),
        ];
        let hi = [
            lo[0] + r.grid(1.0, 10.0, 0.5),
            lo[1] + r.grid(1.0, 10.0, 0.5),
            lo[2] + r.grid(1.0, 10.0, 0.5),
        ];
        (lo, hi)
    };
    let ((la, ha), (lb, hb)) = (corner(), corner());
    let span: Vec<f64> = (0..3)
        .map(|k| (ha[k].min(hb[k]) - la[k].max(lb[k])).max(0.0))
        .collect();
    let (v, area) = if span.iter().all(|&s| s > 0.0) {
        let (x, y, z) = (span[0], span[1], span[2]);
        (x * y * z, 2.0 * (x * y + y * z + z * x))
    } else {
        (0.0, 0.0)
    };
    // Apart along each axis by the gap between their ranges, or not at all.
    let gap: f64 = (0..3)
        .map(|k| (la[k] - hb[k]).max(lb[k] - ha[k]).max(0.0).ppowi(2))
        .sum::<f64>()
        .sqrt();
    let exact = Exact {
        intersection_volume: Some(v),
        intersection_area: Some(area),
        distance: Some(gap),
        ..Default::default()
    };
    (axis_box(la, ha), axis_box(lb, hb), exact)
}

/// Two cylinders on one axis: the intersection is the narrower one over the shared length. Ends
/// are often level, so caps lie on one plane.
fn coaxial_cylinders(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (ra, rb) = (r.grid(1.0, 10.0, 0.5), r.grid(1.0, 10.0, 0.5));
    let (za, zb) = (r.grid(0.0, 5.0, 1.0), r.grid(0.0, 5.0, 1.0));
    let (ha, hb) = (r.grid(2.0, 12.0, 1.0), r.grid(2.0, 12.0, 1.0));
    let a = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, za]),
        radius: ra,
        height: ha,
    };
    let b = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, zb]),
        radius: rb,
        height: hb,
    };
    let len = ((za + ha).min(zb + hb) - za.max(zb)).max(0.0);
    let rr = ra.min(rb);
    let exact = if len > 0.0 {
        Exact {
            intersection_volume: Some(PI * rr * rr * len),
            intersection_area: Some(2.0 * PI * rr * (rr + len)),
            distance: None,
            ..Default::default()
        }
    } else {
        Exact {
            intersection_volume: Some(0.0),
            intersection_area: Some(0.0),
            distance: Some((za - (zb + hb)).max(zb - (za + ha)).max(0.0)),
            ..Default::default()
        }
    };
    (a, b, exact)
}

/// Two equal cylinders whose axes cross square at their middles: the Steinmetz solid, 16r³/3 of
/// volume and 16r² of area.
fn steinmetz(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rad = r.range(1.0, 10.0);
    let h = 4.0 * rad;
    let a = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, -h / 2.0]),
        radius: rad,
        height: h,
    };
    let b = Primitive::Cylinder {
        frame: Frame::along([-h / 2.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        radius: rad,
        height: h,
    };
    let exact = Exact {
        intersection_volume: Some(16.0 * rad.ppowi(3) / 3.0),
        intersection_area: Some(16.0 * rad * rad),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// Two cylinders at any angle and offset: only the identities are known.
fn crossing_cylinders(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (ra, rb) = (r.range(1.0, 8.0), r.range(1.0, 8.0));
    let h = 30.0;
    let a = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, -h / 2.0]),
        radius: ra,
        height: h,
    };
    let d = r.direction();
    let off = [r.range(-ra, ra), r.range(-ra, ra), r.range(-5.0, 5.0)];
    let base = [
        off[0] - d[0] * h / 2.0,
        off[1] - d[1] * h / 2.0,
        off[2] - d[2] * h / 2.0,
    ];
    let b = Primitive::Cylinder {
        frame: Frame::along(base, d),
        radius: rb,
        height: h,
    };
    (a, b, Exact::default())
}

/// Two spheres overlapping in a lens: volume and the two caps' area in closed form.
fn sphere_sphere(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (r1, r2) = (r.range(1.0, 10.0), r.range(1.0, 10.0));
    // Clear of tangency, inside and out, so the lens is a plain transversal case.
    let lo = (r1 - r2).abs() + 0.05 * r1.min(r2);
    let hi = r1 + r2 - 0.05 * r1.min(r2);
    let d = r.range(lo, hi);
    let a = Primitive::Sphere {
        frame: Frame::at([0.0; 3]),
        radius: r1,
    };
    let b = Primitive::Sphere {
        frame: Frame::at([d, 0.0, 0.0]),
        radius: r2,
    };
    let v = PI
        * (r1 + r2 - d).ppowi(2)
        * (d * d + 2.0 * d * r2 - 3.0 * r2 * r2 + 2.0 * d * r1 + 6.0 * r1 * r2 - 3.0 * r1 * r1)
        / (12.0 * d);
    let h1 = (r2 - r1 + d) * (r2 + r1 - d) / (2.0 * d);
    let h2 = (r1 - r2 + d) * (r1 + r2 - d) / (2.0 * d);
    let area = 2.0 * PI * (r1 * h1 + r2 * h2);
    let exact = Exact {
        intersection_volume: Some(v),
        intersection_area: Some(area),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// A sphere cut by a box whose face lies `c` from its centre: a cap and its flat disc.
fn sphere_halfspace(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rad = r.range(1.0, 10.0);
    let c = r.range(-0.9, 0.9) * rad;
    let a = Primitive::Sphere {
        frame: Frame::at([0.0; 3]),
        radius: rad,
    };
    let w = 3.0 * rad;
    let b = axis_box([-w, -w, c], [w, w, w]);
    let h = rad - c;
    let exact = Exact {
        intersection_volume: Some(PI * h * h * (3.0 * rad - h) / 3.0),
        intersection_area: Some(2.0 * PI * rad * h + PI * (rad * rad - c * c)),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// A torus halved by a box whose face holds its axis: half the volume, half the skin and the two
/// circles of the cut.
fn torus_halfspace(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let minor = r.range(0.5, 4.0);
    let major = minor + r.range(0.5, 8.0);
    let a = Primitive::Torus {
        frame: Frame::at([0.0; 3]),
        major,
        minor,
    };
    let w = 2.0 * (major + minor);
    let b = axis_box([0.0, -w, -w], [w, w, w]);
    let exact = Exact {
        intersection_volume: Some(PI * PI * major * minor * minor),
        intersection_area: Some(2.0 * PI * PI * major * minor + 2.0 * PI * minor * minor),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// A depth of contact `r·10⁻ᵏ` for k from 1 to 6: from a clear overlap to a sliver a millionth of
/// the size, still well above a tolerance of 1e-7 mm.
fn depth(r: &mut Rng, size: f64) -> f64 {
    size * 10f64.ppowi(-(1 + (r.next_u64() % 6) as i32))
}

/// The lens two spheres of radii `r1`, `r2` with centres `d` apart share: its volume and the area
/// of its two caps.
fn sphere_lens(r1: f64, r2: f64, d: f64) -> (f64, f64) {
    let v = PI
        * (r1 + r2 - d).ppowi(2)
        * (d * d + 2.0 * d * r2 - 3.0 * r2 * r2 + 2.0 * d * r1 + 6.0 * r1 * r2 - 3.0 * r1 * r1)
        / (12.0 * d);
    let h1 = (r2 - r1 + d) * (r2 + r1 - d) / (2.0 * d);
    let h2 = (r1 - r2 + d) * (r1 + r2 - d) / (2.0 * d);
    (v, 2.0 * PI * (r1 * h1 + r2 * h2))
}

/// The lens two circles of radii `r1`, `r2` with centres `d` apart share: its area and the
/// lengths of its two arcs.
fn circle_lens(r1: f64, r2: f64, d: f64) -> (f64, f64) {
    let a1 = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1))
        .clamp(-1.0, 1.0)
        .pacos();
    let a2 = ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2))
        .clamp(-1.0, 1.0)
        .pacos();
    let area = r1 * r1 * (a1 - a1.psin() * a1.pcos()) + r2 * r2 * (a2 - a2.psin() * a2.pcos());
    (area, 2.0 * r1 * a1 + 2.0 * r2 * a2)
}

/// Two spheres overlapping by a depth from a tenth to a millionth of the smaller radius, outside
/// each other or the smaller inside the larger.
fn spheres_nearly_touching(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (r1, r2) = (r.range(1.0, 10.0), r.range(1.0, 10.0));
    let dep = depth(r, r1.min(r2));
    let inside = r.next_u64().is_multiple_of(2) && (r1 - r2).abs() > 0.1;
    let d = if inside {
        (r1 - r2).abs() + dep
    } else {
        r1 + r2 - dep
    };
    let a = Primitive::Sphere {
        frame: Frame::at([0.0; 3]),
        radius: r1,
    };
    let b = Primitive::Sphere {
        frame: Frame::at([d, 0.0, 0.0]),
        radius: r2,
    };
    let (v, area) = sphere_lens(r1, r2, d);
    let exact = Exact {
        intersection_volume: Some(v),
        intersection_area: Some(area),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// A sphere with a cap a tenth to a millionth of its radius deep cut off by a box face.
fn sphere_shaved(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rad = r.range(1.0, 10.0);
    let h = depth(r, rad);
    let c = rad - h;
    let a = Primitive::Sphere {
        frame: Frame::at([0.0; 3]),
        radius: rad,
    };
    let w = 3.0 * rad;
    let b = axis_box([-w, -w, c], [w, w, w]);
    let exact = Exact {
        intersection_volume: Some(PI * h * h * (3.0 * rad - h) / 3.0),
        intersection_area: Some(2.0 * PI * rad * h + PI * (rad * rad - c * c)),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// Two parallel cylinders of one length, their sides overlapping by a tenth to a millionth of the
/// smaller radius: a lens-shaped prism.
fn parallel_cylinders(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (r1, r2) = (r.range(1.0, 8.0), r.range(1.0, 8.0));
    let d = r1 + r2 - depth(r, r1.min(r2));
    let len = r.range(2.0, 20.0);
    let a = Primitive::Cylinder {
        frame: Frame::at([0.0; 3]),
        radius: r1,
        height: len,
    };
    let b = Primitive::Cylinder {
        frame: Frame::at([d, 0.0, 0.0]),
        radius: r2,
        height: len,
    };
    let (lens, arcs) = circle_lens(r1, r2, d);
    let exact = Exact {
        intersection_volume: Some(lens * len),
        intersection_area: Some(2.0 * lens + arcs * len),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// Two cylinders on one axis whose radii differ by a tenth to a millionth of the radius: sides
/// nearly on one surface. The intersection is the narrower over the shared length.
fn coaxial_nearly_equal(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let ra = r.range(1.0, 10.0);
    let rb = ra
        + depth(r, ra)
            * if r.next_u64().is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
    let (za, zb) = (r.range(0.0, 3.0), r.range(0.0, 3.0));
    let (ha, hb) = (r.range(4.0, 12.0), r.range(4.0, 12.0));
    let a = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, za]),
        radius: ra,
        height: ha,
    };
    let b = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, zb]),
        radius: rb,
        height: hb,
    };
    let len = (za + ha).min(zb + hb) - za.max(zb);
    let rr = ra.min(rb);
    let exact = Exact {
        intersection_volume: Some(PI * rr * rr * len),
        intersection_area: Some(2.0 * PI * rr * (rr + len)),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// Two boxes on a 1 mm grid that touch and share no volume: at a corner, along an edge, or across
/// part of a face. The intersection is empty and the union both boxes: one piece, touching itself
/// at a point or along an edge, or merged across the area the two share.
fn boxes_touching(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let size = |r: &mut Rng| {
        [
            r.grid(1.0, 8.0, 1.0),
            r.grid(1.0, 8.0, 1.0),
            r.grid(1.0, 8.0, 1.0),
        ]
    };
    let (sa, sb) = (size(r), size(r));
    // How many of the three axes the boxes meet on, end to end: 3 a corner, 2 an edge, 1 a face.
    let ends = 1 + (r.next_u64() % 3) as usize;
    let mut lo_b = [0.0; 3];
    for (k, l) in lo_b.iter_mut().enumerate() {
        *l = if k < ends {
            sa[k]
        } else {
            r.grid(0.0, sa[k] - 1.0, 1.0)
        };
    }
    let a = axis_box([0.0; 3], sa);
    let b = axis_box(lo_b, [lo_b[0] + sb[0], lo_b[1] + sb[1], lo_b[2] + sb[2]]);
    let exact = Exact {
        intersection_volume: Some(0.0),
        intersection_area: Some(0.0),
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// Two spheres exactly touching, outside each other or one inside the other: no shared volume.
fn spheres_touching(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (r1, r2) = (r.range(1.0, 10.0), r.range(1.0, 10.0));
    let inside = r.next_u64().is_multiple_of(2) && (r1 - r2).abs() > 0.1;
    let d = if inside { (r1 - r2).abs() } else { r1 + r2 };
    let a = Primitive::Sphere {
        frame: Frame::at([0.0; 3]),
        radius: r1,
    };
    let b = Primitive::Sphere {
        frame: Frame::at([d, 0.0, 0.0]),
        radius: r2,
    };
    let small = r1.min(r2);
    let exact = if inside {
        // The smaller lies inside the larger, touching it from within at one point.
        Exact {
            intersection_volume: Some(4.0 / 3.0 * PI * small.ppowi(3)),
            intersection_area: Some(4.0 * PI * small * small),
            distance: None,
            ..Default::default()
        }
    } else {
        Exact {
            intersection_volume: Some(0.0),
            intersection_area: Some(0.0),
            distance: None,
            ..Default::default()
        }
    };
    (a, b, exact)
}

/// A smooth closed outline: a periodic cubic B-spline over `n` control points round a circle of
/// radius `r`, each moved in or out by up to a quarter of it, on uniform knots.
fn blob(rng: &mut Rng, r: f64) -> Spline {
    let n = 8 + (rng.next_u64() % 5) as usize;
    let mut ctrl: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = 2.0 * PI * i as f64 / n as f64;
            let rr = r * rng.range(0.75, 1.25);
            [rr * t.pcos(), rr * t.psin()]
        })
        .collect();
    ctrl.extend_from_within(..3);
    Spline {
        degree: 3,
        knots: (0..n + 7).map(|k| k as f64).collect(),
        ctrl,
    }
}

fn turned(s: &Spline, angle: f64, shift: [f64; 2]) -> Spline {
    let (c, si) = (angle.pcos(), angle.psin());
    Spline {
        ctrl: s
            .ctrl
            .iter()
            .map(|p| {
                [
                    c * p[0] - si * p[1] + shift[0],
                    si * p[0] + c * p[1] + shift[1],
                ]
            })
            .collect(),
        ..s.clone()
    }
}

/// Two prisms on spline outlines: crossing at any angle, or one outline and a copy of it shifted
/// or turned by a tenth to a millionth of its size, so their sides nearly coincide along lines.
/// Only the identities are known, and each prism's volume.
fn spline_prisms(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let size = r.range(2.0, 6.0);
    let outline = blob(r, size);
    let h = r.range(4.0, 12.0);
    let a = spline_prism(Frame::at([0.0; 3]), outline.clone(), h);
    let b = match r.next_u64() % 3 {
        0 => {
            let d = r.direction();
            let len = 4.0 * size;
            let base = [
                -d[0] * len / 2.0,
                -d[1] * len / 2.0,
                h / 2.0 - d[2] * len / 2.0,
            ];
            spline_prism(
                Frame::along(base, d),
                {
                    let rb = r.range(1.0, size);
                    blob(r, rb)
                },
                len,
            )
        }
        1 => {
            let dep = depth(r, size);
            let t = r.range(0.0, 2.0 * PI);
            spline_prism(
                Frame::at([0.0, 0.0, r.range(-2.0, 2.0)]),
                turned(&outline, 0.0, [dep * t.pcos(), dep * t.psin()]),
                h,
            )
        }
        _ => {
            let angle = depth(r, 1.0);
            spline_prism(
                Frame::at([0.0, 0.0, r.range(-2.0, 2.0)]),
                turned(&outline, angle, [0.0, 0.0]),
                h,
            )
        }
    };
    (a, b, Exact::default())
}

/// Two solids apart by a known gap, from well clear down to a millionth of their size: spheres;
/// boxes apart along one to three axes; a sphere off a box's face, edge or corner; parallel
/// cylinders; a cone's apex above a box; a ball in a torus's hole. Nothing is shared.
fn separated(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let size = r.range(1.0, 8.0);
    let gap = depth(r, size) * 10.0;
    let unit = |v: Vec3| {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        scale3(v, 1.0 / n)
    };
    let (a, b, d) = match r.next_u64() % 6 {
        0 => {
            let r2 = r.range(1.0, 8.0);
            let c = scale3(r.direction(), size + r2 + gap);
            (sphere([0.0; 3], size), sphere(c, r2), gap)
        }
        1 => {
            // Each axis is apart by a gap or overlapping in range; at least one is apart.
            let s = [r.range(1.0, 8.0), r.range(1.0, 8.0), r.range(1.0, 8.0)];
            let apart = 1 + (r.next_u64() % 7) as usize;
            let mut lo = [0.0; 3];
            let mut d2 = 0.0;
            for (k, l) in lo.iter_mut().enumerate() {
                if apart & (1 << k) != 0 {
                    let g = gap * r.range(0.5, 1.5);
                    *l = size + g;
                    d2 += g * g;
                } else {
                    // Strictly inside (−s, size), so the ranges overlap on this axis.
                    *l = -s[k] + (size + s[k]) * r.range(0.1, 0.9);
                }
            }
            let b = axis_box(lo, [lo[0] + s[0], lo[1] + s[1], lo[2] + s[2]]);
            (axis_box([0.0; 3], [size; 3]), b, d2.sqrt())
        }
        2 => {
            // The sphere's centre lies off the cube along a direction in the cone of normals at
            // a face, edge or corner point, so that point is the cube's nearest to it.
            let rs = r.range(0.5, 4.0);
            let kind = (r.next_u64() % 3) as usize;
            let mut q = [r.range(0.2, 0.8) * size, r.range(0.2, 0.8) * size, size];
            let mut n = [0.0, 0.0, 1.0];
            if kind >= 1 {
                q[0] = size;
                n[0] = r.range(0.2, 1.0);
            }
            if kind == 2 {
                q[1] = size;
                n[1] = r.range(0.2, 1.0);
            }
            let n = unit(n);
            let c = [
                q[0] + n[0] * (rs + gap),
                q[1] + n[1] * (rs + gap),
                q[2] + n[2] * (rs + gap),
            ];
            (axis_box([0.0; 3], [size; 3]), sphere(c, rs), gap)
        }
        3 => {
            let r2 = r.range(1.0, 8.0);
            let len = r.range(2.0, 20.0);
            let cyl = |x: f64, rad: f64| Primitive::Cylinder {
                frame: Frame::at([x, 0.0, 0.0]),
                radius: rad,
                height: len,
            };
            (cyl(0.0, size), cyl(size + r2 + gap, r2), gap)
        }
        4 => {
            let h = r.range(3.0, 10.0);
            let alpha = (size / h).patan();
            let d = tilted(r, 1.0, alpha);
            let w = 2.0 * (h + size);
            let below = axis_box([-w, -w, -w], [w, w, 0.0]);
            (
                cone(Frame::along([0.0, 0.0, gap], d), 0.0, size, h),
                below,
                gap,
            )
        }
        _ => {
            let minor = r.range(0.5, 3.0);
            let major = minor + size + gap + r.range(0.5, 3.0);
            let torus = Primitive::Torus {
                frame: Frame::at([0.0; 3]),
                major,
                minor,
            };
            let rs = major - minor - gap;
            (torus, sphere([0.0; 3], rs), gap)
        }
    };
    let exact = Exact {
        intersection_volume: Some(0.0),
        intersection_area: Some(0.0),
        distance: Some(d),
        ..Default::default()
    };
    (a, b, exact)
}

/// The turn at each corner of a loop and the shorter of its two sides.
fn corners(pts: &[[f64; 2]]) -> Vec<(f64, f64)> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (p, q, o) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (u, v) = ([q[0] - p[0], q[1] - p[1]], [o[0] - q[0], o[1] - q[1]]);
            let turn = (u[0] * v[1] - u[1] * v[0]).patan2(u[0] * v[0] + u[1] * v[1]);
            (turn, u[0].phypot(u[1]).min(v[0].phypot(v[1])))
        })
        .collect()
}

/// One size for every chosen corner that keeps each blend within 0.45 of both its sides: a fillet
/// of radius r reaches r·tan(turn/2) along each, a chamfer its setback.
fn blend_size(r: &mut Rng, blend: Act, pts: &[[f64; 2]], chosen: &[usize]) -> f64 {
    let c = corners(pts);
    let most = chosen
        .iter()
        .map(|&i| {
            let (turn, side) = c[i];
            let reach = match blend {
                Act::Fillet => (turn.abs() / 2.0).ptan(),
                _ => 1.0,
            };
            0.45 * side / reach
        })
        .fold(f64::INFINITY, f64::min);
    most * r.range(0.2, 1.0)
}

fn blended(blend: Act, pts: &[[f64; 2]], sizes: &[f64]) -> Profile {
    let lp = match blend {
        Act::Fillet => rounded(pts, sizes),
        _ => chamfered(pts, sizes),
    };
    Profile { loops: vec![lp] }
}

/// A prism on a rectangle, an L (whose inner corner is concave, where a fillet adds material) or a
/// convex polygon, with one to all of its side edges blended by one size. The result is the prism
/// on the profile with those corners rounded or cut.
fn blended_prism(r: &mut Rng, blend: Act) -> (Primitive, Feature, Primitive) {
    let pts: Vec<[f64; 2]> = match r.next_u64() % 3 {
        0 => {
            let (w, h) = (r.range(3.0, 10.0), r.range(3.0, 10.0));
            vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
        }
        1 => {
            let (w, h, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(2.0, 3.5));
            vec![[0.0, 0.0], [w, 0.0], [w, t], [t, t], [t, h], [0.0, h]]
        }
        _ => {
            let n = 5 + (r.next_u64() % 4) as usize;
            let rad = r.range(3.0, 6.0);
            let turn = r.range(0.0, 1.0);
            (0..n)
                .map(|i| {
                    let t = turn + TAU * i as f64 / n as f64;
                    [rad * t.pcos(), rad * t.psin()]
                })
                .collect()
        }
    };
    let n = pts.len();
    let mut chosen: Vec<usize> = (0..n).filter(|_| r.next_u64().is_multiple_of(2)).collect();
    if chosen.is_empty() {
        chosen.push((r.next_u64() % n as u64) as usize);
    }
    let size = blend_size(r, blend, &pts, &chosen);
    let height = r.range(2.0, 10.0);
    let sharp = Profile {
        loops: vec![rounded(&pts, &vec![0.0; n])],
    };
    let sizes: Vec<f64> = (0..n)
        .map(|i| if chosen.contains(&i) { size } else { 0.0 })
        .collect();
    let edges = chosen
        .iter()
        .map(|&i| [pts[i][0], pts[i][1], height * r.range(0.2, 0.8)])
        .collect();
    let frame = Frame::at([0.0; 3]);
    let base = Primitive::Extrude {
        frame,
        profile: sharp,
        height,
    };
    let want = Primitive::Extrude {
        frame,
        profile: blended(blend, &pts, &sizes),
        height,
    };
    let feature = Feature {
        act: blend,
        points: edges,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    (base, feature, want)
}

/// A cylinder or a cone (sometimes to an apex) with its bottom rim, top rim or both blended by one
/// size. The result is the revolution of its meridian with those corners rounded or cut.
fn blended_rim(r: &mut Rng, blend: Act) -> (Primitive, Feature, Primitive) {
    let (bottom, height) = (r.range(2.0, 8.0), r.range(3.0, 10.0));
    let top = match r.next_u64() % 3 {
        0 => bottom,
        1 => 0.0,
        _ => r.range(0.5, 8.0),
    };
    let base = if top == bottom {
        Primitive::Cylinder {
            frame: Frame::at([0.0; 3]),
            radius: bottom,
            height,
        }
    } else {
        cone(Frame::at([0.0; 3]), bottom, top, height)
    };
    let mut pts = vec![[0.0, 0.0], [bottom, 0.0]];
    if top > 0.0 {
        pts.push([top, height]);
    }
    pts.push([0.0, height]);
    // The rims are the meridian's outer corners: index 1 at the bottom, 2 at the top.
    let rims: Vec<usize> = if top > 0.0 { vec![1, 2] } else { vec![1] };
    let chosen: Vec<usize> = match r.next_u64() % 3 {
        0 => vec![rims[0]],
        1 => vec![*rims.last().unwrap()],
        _ => rims.clone(),
    };
    let size = blend_size(r, blend, &pts, &chosen);
    let sizes: Vec<f64> = (0..pts.len())
        .map(|i| if chosen.contains(&i) { size } else { 0.0 })
        .collect();
    let edges = chosen
        .iter()
        .map(|&i| {
            let phi = r.range(0.0, TAU);
            [pts[i][0] * phi.pcos(), pts[i][0] * phi.psin(), pts[i][1]]
        })
        .collect();
    let want = Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: blended(blend, &pts, &sizes),
        angle: TAU,
    };
    let feature = Feature {
        act: blend,
        points: edges,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    (base, feature, want)
}

/// The extrusion of a one-spline outline.
fn spline_prism(frame: Frame, outline: Spline, height: f64) -> Primitive {
    Primitive::Extrude {
        frame,
        profile: Profile {
            loops: vec![vec![Seg::Spline(outline)]],
        },
        height,
    }
}

fn cone(frame: Frame, bottom: f64, top: f64, height: f64) -> Primitive {
    Primitive::Cone {
        frame,
        bottom,
        top,
        height,
    }
}

fn sphere(c: Vec3, radius: f64) -> Primitive {
    Primitive::Sphere {
        frame: Frame::at(c),
        radius,
    }
}

fn scale3(v: Vec3, s: f64) -> Vec3 {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// The volume of the frustum about z, radius `r0` at 0 and `r1` at `h`, that a cylinder about z of
/// radius `rc` keeps between heights `lo` and `hi`: π·min(r(z), rc)² integrated, in closed form on
/// each side of where the frustum's radius meets the cylinder's.
fn frustum_in_cylinder(r0: f64, r1: f64, h: f64, rc: f64, lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return 0.0;
    }
    let r = |z: f64| r0 + (r1 - r0) * z / h;
    let mut cuts = vec![lo, hi];
    if r1 != r0 {
        let z = (rc - r0) * h / (r1 - r0);
        if z > lo && z < hi {
            cuts.insert(1, z);
        }
    }
    cuts.windows(2)
        .map(|w| {
            let (ra, rb) = (r(w[0]), r(w[1]));
            let d = w[1] - w[0];
            if r(0.5 * (w[0] + w[1])) <= rc {
                PI * d / 3.0 * (ra * ra + ra * rb + rb * rb)
            } else {
                PI * rc * rc * d
            }
        })
        .sum()
}

/// A frustum or cone and a cylinder on one axis, ends often level and either cone end an apex:
/// the intersection's volume in closed form.
fn cone_cylinder_coaxial(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    // Equal radii are a cylinder, which is another test's shape.
    let (r0, r1) = loop {
        let drawn = (r.grid(0.0, 8.0, 1.0), r.grid(1.0, 8.0, 1.0));
        if drawn.0 != drawn.1 {
            break drawn;
        }
    };
    let (r0, r1) = if r.next_u64().is_multiple_of(2) {
        (r0, r1)
    } else {
        (r1, r0)
    };
    let h = r.grid(2.0, 12.0, 1.0);
    let rc = r.grid(0.5, 8.0, 0.5);
    let (zc, hc) = (r.grid(-4.0, 8.0, 1.0), r.grid(2.0, 12.0, 1.0));
    let a = cone(Frame::at([0.0; 3]), r0, r1, h);
    let b = Primitive::Cylinder {
        frame: Frame::at([0.0, 0.0, zc]),
        radius: rc,
        height: hc,
    };
    let v = frustum_in_cylinder(r0, r1, h, rc, zc.max(0.0), (zc + hc).min(h));
    let exact = Exact {
        intersection_volume: Some(v),
        intersection_area: None,
        distance: None,
        ..Default::default()
    };
    (a, b, exact)
}

/// A cone or frustum at any angle through a box: only the identities are known.
fn cone_box(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let w = [r.range(3.0, 10.0), r.range(3.0, 10.0), r.range(3.0, 10.0)];
    let a = axis_box(scale3(w, -0.5), scale3(w, 0.5));
    let d = r.direction();
    let h = r.range(6.0, 20.0);
    let bottom = r.range(1.0, 6.0);
    let top = if r.next_u64().is_multiple_of(4) {
        0.0
    } else {
        r.range(0.5, 5.0)
    };
    let off = [r.range(-2.0, 2.0), r.range(-2.0, 2.0), r.range(-2.0, 2.0)];
    let base = [
        off[0] - d[0] * h / 2.0,
        off[1] - d[1] * h / 2.0,
        off[2] - d[2] * h / 2.0,
    ];
    (
        a,
        cone(Frame::along(base, d), bottom, top, h),
        Exact::default(),
    )
}

/// An axis tilted from +z (`sign` 1) or −z (−1) by up to 0.8 of what keeps a cone of half-angle
/// `alpha` with its apex at the origin on that side of the plane z = 0.
fn tilted(r: &mut Rng, sign: f64, alpha: f64) -> Vec3 {
    let theta = r.range(0.0, 0.8) * (PI / 2.0 - alpha);
    let phi = r.range(0.0, 2.0 * PI);
    [
        theta.psin() * phi.pcos(),
        theta.psin() * phi.psin(),
        sign * theta.pcos(),
    ]
}

/// A cone that touches its partner and shares no volume with it: its apex on a box's face, on a
/// sphere, or on another cone's apex, each axis tilted short of letting the side cross; or its side
/// lying along a box face. The union is one piece touching itself at a point or along a line.
fn cones_touching(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let h = r.range(3.0, 10.0);
    let rad = r.range(1.0, 6.0);
    let alpha = (rad / h).patan();
    let w = 2.0 * (h + rad);
    let below = axis_box([-w, -w, -w], [w, w, 0.0]);
    let apex_cone = |d: Vec3| cone(Frame::along([0.0; 3], d), 0.0, rad, h);
    let (a, b) = match r.next_u64() % 4 {
        0 => (apex_cone(tilted(r, 1.0, alpha)), below),
        1 => {
            let (h2, r2) = (r.range(3.0, 10.0), r.range(1.0, 6.0));
            let d = tilted(r, -1.0, (r2 / h2).patan());
            (
                apex_cone(tilted(r, 1.0, alpha)),
                cone(Frame::along([0.0; 3], d), 0.0, r2, h2),
            )
        }
        2 => {
            let rs = r.range(1.0, 8.0);
            (
                apex_cone(tilted(r, 1.0, alpha)),
                sphere([0.0, 0.0, -rs], rs),
            )
        }
        _ => {
            // The side's lowest line runs along +x on the plane z = 0.
            let d = [alpha.pcos(), 0.0, alpha.psin()];
            (apex_cone(d), below)
        }
    };
    let empty = Exact {
        intersection_volume: Some(0.0),
        intersection_area: Some(0.0),
        distance: None,
        ..Default::default()
    };
    (a, b, empty)
}

/// A solid inside another, touching it along a curve or at points: a sphere in a cylinder round
/// its equator, a sphere in a cone round a circle (its top cut off by the cone's end or not), a
/// cylinder in a box along four lines, a sphere in a cube at six points. The intersection is the
/// inner solid, or the sphere less its cap.
fn inscribed(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rho = r.range(1.0, 6.0);
    let ball = |rho: f64| Exact {
        intersection_volume: Some(4.0 / 3.0 * PI * rho.ppowi(3)),
        intersection_area: Some(4.0 * PI * rho * rho),
        distance: None,
        ..Default::default()
    };
    match r.next_u64() % 4 {
        0 => {
            let h = rho * r.range(2.2, 4.0);
            // A third of them sit on the bottom cap as well.
            let c = if r.next_u64().is_multiple_of(3) {
                rho
            } else {
                r.range(rho, h - rho)
            };
            let cyl = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rho,
                height: h,
            };
            (cyl, sphere([0.0, 0.0, c], rho), ball(rho))
        }
        1 => {
            let alpha = r.range(0.2, 1.0);
            let h = r.range(6.0, 16.0);
            let c = h * r.range(0.3, 0.8);
            let rho = c * alpha.psin();
            let k = cone(Frame::at([0.0; 3]), 0.0, h * alpha.ptan(), h);
            let exact = if c + rho <= h {
                ball(rho)
            } else {
                let cap = c + rho - h;
                Exact {
                    intersection_volume: Some(
                        4.0 / 3.0 * PI * rho.ppowi(3) - PI * cap * cap * (3.0 * rho - cap) / 3.0,
                    ),
                    intersection_area: Some(
                        4.0 * PI * rho * rho - 2.0 * PI * rho * cap
                            + PI * (rho * rho - (h - c).ppowi(2)),
                    ),
                    distance: None,
                    ..Default::default()
                }
            };
            (k, sphere([0.0, 0.0, c], rho), exact)
        }
        2 => {
            let h = r.range(2.0, 12.0);
            let bx = axis_box([-rho, -rho, -1.0], [rho, rho, h + 1.0]);
            let cyl = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rho,
                height: h,
            };
            let exact = Exact {
                intersection_volume: Some(PI * rho * rho * h),
                intersection_area: Some(2.0 * PI * rho * (rho + h)),
                distance: None,
                ..Default::default()
            };
            (bx, cyl, exact)
        }
        _ => (
            axis_box([-rho; 3], [rho; 3]),
            sphere([0.0; 3], rho),
            ball(rho),
        ),
    }
}

/// `v` turned by `angle` about the unit axis `k` (Rodrigues).
fn turn(v: Vec3, k: Vec3, angle: f64) -> Vec3 {
    let (s, co) = (angle.psin(), angle.pcos());
    let kv = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let x = [
        k[1] * v[2] - k[2] * v[1],
        k[2] * v[0] - k[0] * v[2],
        k[0] * v[1] - k[1] * v[0],
    ];
    [
        v[0] * co + x[0] * s + k[0] * kv * (1.0 - co),
        v[1] * co + x[1] * s + k[1] * kv * (1.0 - co),
        v[2] * co + x[2] * s + k[2] * kv * (1.0 - co),
    ]
}

/// Offsets about a declared tolerance of 1e-7 mm, from well inside it to well outside.
const NEAR: [f64; 9] = [0.0, 1e-10, 1e-9, 1e-8, 5e-8, 1e-7, 2e-7, 1e-6, 1e-5];

/// A box and a copy of it with each face moved in or out by an offset from well inside a 1e-7 mm
/// tolerance to well outside it, or turned about its centre by 1e-10 to 1e-4 rad. The moved copy's
/// intersection is the box of the shared ranges; the turned one's is only sampled.
fn near_coincident_boxes(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let s = [r.range(1.0, 10.0), r.range(1.0, 10.0), r.range(1.0, 10.0)];
    let a = axis_box([0.0; 3], s);
    if r.next_u64().is_multiple_of(2) {
        let mut off = || {
            let d = NEAR[(r.next_u64() % NEAR.len() as u64) as usize];
            if r.next_u64().is_multiple_of(2) {
                d
            } else {
                -d
            }
        };
        let lo = [off(), off(), off()];
        let hi = [s[0] + off(), s[1] + off(), s[2] + off()];
        let span: Vec<f64> = (0..3).map(|k| hi[k].min(s[k]) - lo[k].max(0.0)).collect();
        let (x, y, z) = (span[0], span[1], span[2]);
        let exact = Exact {
            intersection_volume: Some(x * y * z),
            intersection_area: Some(2.0 * (x * y + y * z + z * x)),
            distance: None,
            ..Default::default()
        };
        (a, axis_box(lo, hi), exact)
    } else {
        let angle = 10f64.ppowi(-(4 + (r.next_u64() % 7) as i32));
        let k = r.direction();
        let c = scale3(s, 0.5);
        let o = turn(scale3(c, -1.0), k, angle);
        let b = Primitive::Box {
            frame: Frame {
                origin: [o[0] + c[0], o[1] + c[1], o[2] + c[2]],
                x: turn([1.0, 0.0, 0.0], k, angle),
                y: turn([0.0, 1.0, 0.0], k, angle),
                z: turn([0.0, 0.0, 1.0], k, angle),
            },
            size: s,
        };
        (a, b, Exact::default())
    }
}

/// The primitive with every length times `s`, about the world origin.
fn scaled_prim(p: &Primitive, s: f64) -> Primitive {
    let mut q = p.clone();
    let f = p.frame();
    let frame = Frame {
        origin: scale3(f.origin, s),
        ..f
    };
    match &mut q {
        Primitive::Slab {
            frame: fr,
            size,
            heights,
            ..
        } => {
            *fr = frame;
            *size = [size[0] * s, size[1] * s];
            for h in heights.iter_mut().flatten() {
                *h *= s;
            }
        }
        Primitive::SweepPath {
            frame: fr,
            profile,
            path,
            ..
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
            for c in &mut path.ctrl {
                *c = scale3(*c, s);
            }
        }
        Primitive::Sweep {
            frame: fr,
            profile,
            path,
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
            for seg in path {
                match seg {
                    PathSeg::Line { length } => *length *= s,
                    PathSeg::Arc { radius, .. } => *radius *= s,
                }
            }
        }
        Primitive::Box { frame: fr, size } => {
            *fr = frame;
            *size = scale3(*size, s);
        }
        Primitive::Cylinder {
            frame: fr,
            radius,
            height,
        } => {
            *fr = frame;
            *radius *= s;
            *height *= s;
        }
        Primitive::Sphere { frame: fr, radius } => {
            *fr = frame;
            *radius *= s;
        }
        Primitive::Torus {
            frame: fr,
            major,
            minor,
        } => {
            *fr = frame;
            *major *= s;
            *minor *= s;
        }
        Primitive::Cone {
            frame: fr,
            bottom,
            top,
            height,
        } => {
            *fr = frame;
            *bottom *= s;
            *top *= s;
            *height *= s;
        }
        Primitive::Extrude {
            frame: fr,
            profile,
            height,
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
            *height *= s;
        }
        Primitive::Revolve {
            frame: fr, profile, ..
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
        }
        Primitive::Screw {
            frame: fr,
            profile,
            rise,
            ..
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
            *rise *= s;
        }
        Primitive::Loft {
            frame: fr,
            profile,
            height,
            ..
        } => {
            *fr = frame;
            *profile = profile.scaled(s);
            *height *= s;
        }
    }
    q
}

/// The corners of the box `lo`–`hi`, counter-clockwise.
fn rect(lo: [f64; 2], hi: [f64; 2]) -> Vec<[f64; 2]> {
    vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]]
}

/// A star of `n` points between radii `r_in` and `r_out` about `c`, counter-clockwise.
fn star(c: [f64; 2], n: usize, r_in: f64, r_out: f64, turn: f64) -> Vec<[f64; 2]> {
    (0..2 * n)
        .map(|i| {
            let t = turn + PI * i as f64 / n as f64;
            let r = if i % 2 == 0 { r_out } else { r_in };
            [c[0] + r * t.pcos(), c[1] + r * t.psin()]
        })
        .collect()
}

/// A slot: two semicircles of radius `r` joined by sides `len` long, along x from `c`.
fn stadium(c: [f64; 2], len: f64, r: f64) -> Vec<Seg> {
    let (x0, x1) = (c[0], c[0] + len);
    vec![
        Seg::Line {
            a: [x0, c[1] - r],
            b: [x1, c[1] - r],
        },
        Seg::Arc {
            center: [x1, c[1]],
            radius: r,
            start: -PI / 2.0,
            sweep: PI,
        },
        Seg::Line {
            a: [x1, c[1] + r],
            b: [x0, c[1] + r],
        },
        Seg::Arc {
            center: [x0, c[1]],
            radius: r,
            start: PI / 2.0,
            sweep: PI,
        },
    ]
}

/// Corner radii for a loop of `n` corners whose shortest side is `side`: each sharp or up to
/// 0.4 of it, so neighbouring roundings never meet.
fn radii(r: &mut Rng, n: usize, side: f64) -> Vec<f64> {
    (0..n)
        .map(|_| {
            if r.next_u64().is_multiple_of(3) {
                0.0
            } else {
                r.range(0.05, 0.4) * side
            }
        })
        .collect()
}

/// A profile of one of four kinds about the sketch origin: a rounded rectangle with up to two
/// round holes, a slot, a star, or an L with its corners rounded.
fn any_profile(r: &mut Rng, holes: bool) -> Profile {
    match r.next_u64() % 4 {
        0 => {
            let (w, h) = (r.range(3.0, 10.0), r.range(3.0, 10.0));
            let lo = [-w / 2.0, -h / 2.0];
            let corners = radii(r, 4, w.min(h));
            let mut loops = vec![rounded(&rect(lo, [w / 2.0, h / 2.0]), &corners)];
            if holes {
                // Holes on the long axis, each in its own half, clear of the rounded corners.
                let n = (r.next_u64() % 3) as usize;
                let rad = 0.15 * w.min(h);
                for k in 0..n {
                    let side = if k == 0 { -1.0 } else { 1.0 };
                    let c = if w >= h {
                        [side * w / 4.0, 0.0]
                    } else {
                        [0.0, side * h / 4.0]
                    };
                    loops.push(circle(c, rad * r.range(0.5, 1.0), false));
                }
            }
            Profile { loops }
        }
        1 => {
            let rad = r.range(1.0, 3.0);
            let len = r.range(1.0, 8.0);
            Profile {
                loops: vec![stadium([-len / 2.0, 0.0], len, rad)],
            }
        }
        2 => {
            let n = 5 + (r.next_u64() % 5) as usize;
            let r_out = r.range(3.0, 6.0);
            let pts = star(
                [0.0; 2],
                n,
                r_out * r.range(0.4, 0.8),
                r_out,
                r.range(0.0, 1.0),
            );
            Profile {
                loops: vec![rounded(&pts, &vec![0.0; pts.len()])],
            }
        }
        _ => {
            let (w, h, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(1.5, 3.0));
            let pts = [[0.0, 0.0], [w, 0.0], [w, t], [t, t], [t, h], [0.0, h]];
            let corners = radii(r, 6, (t * 0.9).min(w - t).min(h - t));
            let shifted: Vec<[f64; 2]> = pts
                .iter()
                .map(|p| [p[0] - w / 2.0, p[1] - h / 2.0])
                .collect();
            Profile {
                loops: vec![rounded(&shifted, &corners)],
            }
        }
    }
}

/// A box through the middle of a part `size` across, at any angle, covering about half of it; or
/// a cylinder drilled across it.
fn crossing(r: &mut Rng, centre: Vec3, size: f64) -> Primitive {
    let d = r.direction();
    if r.next_u64().is_multiple_of(2) {
        let base = [
            centre[0] - d[0] * size,
            centre[1] - d[1] * size,
            centre[2] - d[2] * size,
        ];
        Primitive::Cylinder {
            frame: Frame::along(base, d),
            radius: size * r.range(0.1, 0.3),
            height: 2.0 * size,
        }
    } else {
        let w = 2.0 * size;
        let f = Frame::along(centre, d);
        let off = r.range(-0.2, 0.2) * size;
        // The box's lower face passes `off` from the centre, square to `d`.
        let o = [
            f.origin[0] - f.x[0] * w / 2.0 - f.y[0] * w / 2.0 + f.z[0] * off,
            f.origin[1] - f.x[1] * w / 2.0 - f.y[1] * w / 2.0 + f.z[1] * off,
            f.origin[2] - f.x[2] * w / 2.0 - f.y[2] * w / 2.0 + f.z[2] * off,
        ];
        Primitive::Box {
            frame: Frame { origin: o, ..f },
            size: [w, w, w],
        }
    }
}

/// A profile of any kind extruded 2 to 12 mm, crossed by a box or a cylinder. B0 checks the
/// extrusion's closed form: area times height, and twice the area plus perimeter times height.
fn extrusions(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let profile = any_profile(r, true);
    let height = r.range(2.0, 12.0);
    let a = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile,
        height,
    };
    let b = crossing(r, [0.0, 0.0, height / 2.0], 6.0);
    (a, b, Exact::default())
}

/// One primitive of the kind asked for, anywhere and at any tilt, crossed by a second solid: the
/// constructor's own test. A cone is a frustum, or comes to an apex one time in three.
fn constructed(r: &mut Rng, which: usize) -> (Primitive, Primitive, Exact) {
    let c = [r.range(-3.0, 3.0), r.range(-3.0, 3.0), r.range(-3.0, 3.0)];
    let d = r.direction();
    let size = r.range(2.0, 8.0);
    let h = size * r.range(0.8, 2.0);
    let base = [0, 1, 2].map(|k| c[k] - d[k] * h / 2.0);
    let a = match which {
        0 => Primitive::Box {
            frame: Frame::along(base, d),
            size: [size, size * r.range(0.5, 1.5), h],
        },
        1 => Primitive::Cylinder {
            frame: Frame::along(base, d),
            radius: size / 2.0,
            height: h,
        },
        2 => sphere(c, size / 2.0),
        3 => Primitive::Torus {
            frame: Frame::along(c, d),
            major: size / 2.0,
            minor: size * r.range(0.1, 0.2),
        },
        _ => {
            let top = if r.next_u64().is_multiple_of(3) {
                0.0
            } else {
                size * r.range(0.1, 0.4)
            };
            cone(Frame::along(base, d), size / 2.0, top, h)
        }
    };
    (a, crossing(r, c, size), Exact::default())
}

/// A profile in the half-plane right of the axis, turned about it whole or part way: a tube
/// with rounded ends, a ring of round section, or a vase that closes on the axis.
fn revolutions(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let h = r.range(3.0, 10.0);
    let profile = match r.next_u64() % 3 {
        0 => {
            let r0 = r.range(1.0, 4.0);
            let r1 = r0 + r.range(1.0, 4.0);
            let corners = radii(r, 4, (r1 - r0).min(h));
            Profile {
                loops: vec![rounded(&rect([r0, 0.0], [r1, h]), &corners)],
            }
        }
        1 => {
            let rad = r.range(0.5, 2.5);
            Profile {
                loops: vec![circle([rad + r.range(0.5, 4.0), h / 2.0], rad, true)],
            }
        }
        _ => {
            // Sharp where the loop meets the axis; the outside rounded.
            let (r0, r1, r2) = (r.range(2.0, 5.0), r.range(1.0, 6.0), r.range(1.0, 5.0));
            let pts = [[0.0, 0.0], [r0, 0.0], [r1, h / 2.0], [r2, h], [0.0, h]];
            let side = r0.min(r2).min(h / 2.0);
            let mut corners = radii(r, 5, side);
            corners[0] = 0.0;
            corners[4] = 0.0;
            Profile {
                loops: vec![rounded(&pts, &corners)],
            }
        }
    };
    let angle = if r.next_u64().is_multiple_of(2) {
        TAU
    } else {
        r.range(0.5, 5.5)
    };
    let a = Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile,
        angle,
    };
    let b = crossing(r, [0.0, 0.0, h / 2.0], 6.0);
    (a, b, Exact::default())
}

/// A thread: a square, round or vee section out at 3 to 8 mm from the axis, screwed through half
/// a turn to three turns, its pitch clear of the section by at least a fifth of its height. It is
/// crossed by a box or a cylinder, or crossed by a rod on its axis that the section bites into.
fn helices(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rad = r.range(3.0, 8.0);
    let (w, h) = (r.range(0.5, 2.0), r.range(0.5, 2.0));
    let profile = match r.next_u64() % 3 {
        0 => Profile {
            loops: vec![rounded(&rect([rad, 0.0], [rad + w, h]), &[0.0; 4])],
        },
        1 => Profile {
            loops: vec![circle([rad + w / 2.0, h / 2.0], w.min(h) / 2.0, true)],
        },
        _ => Profile {
            loops: vec![rounded(
                &[[rad, 0.0], [rad + w, h / 2.0], [rad, h]],
                &[0.0; 3],
            )],
        },
    };
    let pitch = h * r.range(1.2, 3.0);
    let angle = TAU * r.range(0.5, 3.0);
    let a = Primitive::Screw {
        frame: Frame::at([0.0; 3]),
        profile,
        rise: pitch / TAU,
        angle,
    };
    let top = h + pitch * angle / TAU;
    let b = if r.next_u64().is_multiple_of(2) {
        Primitive::Cylinder {
            frame: Frame::at([0.0, 0.0, -1.0]),
            radius: rad + w * r.range(0.2, 0.6),
            height: top + 2.0,
        }
    } else {
        crossing(r, [0.0, 0.0, top / 2.0], rad + w)
    };
    (a, b, Exact::default())
}

/// A ruled loft from a profile about the origin to its copy scaled 0.2 to 1.8 (sometimes exactly
/// 1, a prism), 3 to 12 mm up, crossed by a box or a cylinder.
fn lofts(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let profile = any_profile(r, false);
    let scale = if r.next_u64().is_multiple_of(5) {
        1.0
    } else {
        r.range(0.2, 1.8)
    };
    let height = r.range(3.0, 12.0);
    let a = Primitive::Loft {
        frame: Frame::at([0.0; 3]),
        profile,
        scale,
        height,
    };
    let b = crossing(r, [0.0, 0.0, height / 2.0], 6.0);
    (a, b, Exact::default())
}

/// A case of another family with every length times 10ᵏ, k from −3 to 4: from parts a few microns
/// across to ones of tens of metres, against one absolute tolerance. Closed forms scale with it.
fn scaled(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let s = 10f64.ppowi(-3 + (r.next_u64() % 8) as i32);
    let ((a, b, e), variant) = match r.next_u64() % 5 {
        0 => (sphere_sphere(r), "booleans/scale/microns-to-metres-spheres"),
        1 => (
            steinmetz(r),
            "booleans/scale/microns-to-metres-equal-cylinders",
        ),
        2 => (box_box(r), "booleans/scale/microns-to-metres-boxes"),
        3 => (
            cone_cylinder_coaxial(r),
            "booleans/scale/microns-to-metres-cone-and-cylinder",
        ),
        _ => (
            crossing_cylinders(r),
            "booleans/scale/microns-to-metres-crossing-cylinders",
        ),
    };
    let exact = Exact {
        intersection_volume: e.intersection_volume.map(|v| v * s.ppowi(3)),
        intersection_area: e.intersection_area.map(|v| v * s * s),
        distance: None,
        variant: Some(variant),
        ..Default::default()
    };
    (scaled_prim(&a, s), scaled_prim(&b, s), exact)
}

/// A plate drilled through by two to six holes, one cut after another, and crossed by a box over
/// part of it or a cross-drilled hole along it at mid-thickness that meets the holes.
fn drilled_plates(r: &mut Rng) -> Drawn {
    let n = 2 + (r.next_u64() % 5) as usize;
    let pitch = r.range(4.0, 8.0);
    let (len, wid, t) = (pitch * n as f64, r.range(6.0, 12.0), r.range(2.0, 6.0));
    let mut plate: Shape = axis_box([0.0; 3], [len, wid, t]).into();
    let hole_r = pitch * r.range(0.15, 0.4);
    let y = wid / 2.0;
    for i in 0..n {
        let hole = Primitive::Cylinder {
            frame: Frame::at([pitch * (i as f64 + 0.5), y, -1.0]),
            radius: hole_r,
            height: t + 2.0,
        };
        plate = Shape::bool(Op::Difference, plate, hole);
    }
    let b: Shape = if r.next_u64().is_multiple_of(2) {
        let x = r.range(0.2, 0.8) * len;
        let z = r.range(-1.0, t / 2.0);
        axis_box([x, -1.0, z], [len + 1.0, wid + 1.0, t + 1.0]).into()
    } else {
        let dy = r.range(-0.5, 0.5) * hole_r;
        Primitive::Cylinder {
            frame: Frame::along([-1.0, y + dy, t / 2.0], [1.0, 0.0, 0.0]),
            radius: t * r.range(0.1, 0.4),
            height: len + 2.0,
        }
        .into()
    };
    (plate, b, Exact::default())
}

/// Two to five boxes on a 1 mm grid, each standing on the one below and united in turn, so the
/// result shares faces' planes at every step; crossed by a sphere or a cylinder.
fn stacked_boxes(r: &mut Rng) -> Drawn {
    let n = 2 + (r.next_u64() % 4) as usize;
    let mut z = 0.0;
    let mut stack: Option<Shape> = None;
    for _ in 0..n {
        let lo = [r.grid(0.0, 4.0, 1.0), r.grid(0.0, 4.0, 1.0), z];
        let hi = [
            lo[0] + r.grid(2.0, 6.0, 1.0),
            lo[1] + r.grid(2.0, 6.0, 1.0),
            z + r.grid(1.0, 3.0, 1.0),
        ];
        let b = axis_box(lo, hi);
        stack = Some(match stack {
            None => b.into(),
            Some(s) => Shape::bool(Op::Union, s, b),
        });
        z = hi[2];
    }
    let c = [r.range(1.0, 6.0), r.range(1.0, 6.0), r.range(0.0, z)];
    let b: Shape = if r.next_u64().is_multiple_of(2) {
        sphere(c, r.range(1.0, 4.0)).into()
    } else {
        Primitive::Cylinder {
            frame: Frame::at([c[0], c[1], -1.0]),
            radius: r.range(0.5, 3.0),
            height: z + 2.0,
        }
        .into()
    };
    (stack.unwrap(), b, Exact::default())
}

/// Three spheres overlapping one another, united in turn, cut by a box through near their first
/// centre or crossed by a fourth sphere.
fn sphere_clusters(r: &mut Rng) -> Drawn {
    let r0 = r.range(2.0, 5.0);
    let mut cluster: Shape = sphere([0.0; 3], r0).into();
    for _ in 0..2 {
        let rr = r.range(2.0, 5.0);
        let d = r.direction();
        let dist = r.range(0.4, 0.9) * (r0 + rr);
        cluster = Shape::bool(Op::Union, cluster, sphere(scale3(d, dist), rr));
    }
    let b: Shape = if r.next_u64().is_multiple_of(2) {
        let w = 20.0;
        axis_box([-w, -w, r.range(-2.0, 2.0)], [w, w, w]).into()
    } else {
        let d = r.direction();
        sphere(scale3(d, r.range(2.0, 6.0)), r.range(2.0, 5.0)).into()
    };
    (cluster, b, Exact::default())
}

/// A primitive of any kind, size 2 to 8 mm, near the origin at any angle.
fn any_primitive(r: &mut Rng) -> Primitive {
    let c = [r.range(-3.0, 3.0), r.range(-3.0, 3.0), r.range(-3.0, 3.0)];
    let d = r.direction();
    let size = r.range(2.0, 8.0);
    let h = size * r.range(0.8, 2.0);
    let base = [
        c[0] - d[0] * h / 2.0,
        c[1] - d[1] * h / 2.0,
        c[2] - d[2] * h / 2.0,
    ];
    match r.next_u64() % 5 {
        0 => Primitive::Box {
            frame: Frame::along(base, d),
            size: [size, size * r.range(0.5, 1.5), h],
        },
        1 => Primitive::Cylinder {
            frame: Frame::along(base, d),
            radius: size / 2.0,
            height: h,
        },
        2 => sphere(c, size / 2.0),
        3 => {
            let top = size * r.range(0.0, 0.3);
            cone(Frame::along(base, d), size / 2.0, top, h)
        }
        _ => Primitive::Torus {
            frame: Frame::along(c, d),
            major: size / 2.0,
            minor: size * r.range(0.1, 0.2),
        },
    }
}

/// Three primitives of any kind combined by two random operations, crossed by a fourth: results
/// fed back as operands, the way a part is modelled.
fn random_chains(r: &mut Rng) -> Drawn {
    let mut a: Shape = any_primitive(r).into();
    for _ in 0..2 {
        // Intersections are rarer: two in a row often leave nothing.
        let op = match r.next_u64() % 5 {
            0 | 1 => Op::Union,
            2 | 3 => Op::Difference,
            _ => Op::Intersection,
        };
        a = Shape::bool(op, a, any_primitive(r));
    }
    (a, any_primitive(r).into(), Exact::default())
}

/// The Betti numbers a primitive's kind gives it, where they follow from the kind alone: one
/// piece, and a tunnel for each hole a profile carries or for a ring turned clear of its axis.
fn prim_betti(p: &Primitive) -> Option<[u64; 3]> {
    if let Primitive::Slab { .. } = p {
        return Some([1, 0, 0]);
    }
    Some(match p {
        Primitive::Torus { .. } => [1, 1, 0],
        Primitive::Extrude { profile, .. } | Primitive::Sweep { profile, .. } => {
            [1, profile.loops.len() as u64 - 1, 0]
        }
        Primitive::Revolve { profile, angle, .. } => {
            if profile.loops.len() != 1 {
                return None;
            }
            let clear = profile.bounds().0[0] > 0.0;
            [1, u64::from(clear && *angle >= TAU), 0]
        }
        Primitive::Loft { profile, .. } | Primitive::Screw { profile, .. }
            if profile.loops.len() != 1 =>
        {
            return None;
        }
        _ => [1, 0, 0],
    })
}

/// A box with a solid strictly inside it, clear of its walls: a ball, a smaller box, or a ring.
/// The difference has a void, and a ring's void a tunnel as well; the intersection is the inner
/// solid.
fn voids(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let w = [r.range(6.0, 12.0), r.range(6.0, 12.0), r.range(6.0, 12.0)];
    let outer = axis_box([0.0; 3], w);
    let c = scale3(w, 0.5);
    let room = w.iter().copied().fold(f64::INFINITY, f64::min) / 2.0;
    let (inner, ring) = match r.next_u64() % 3 {
        0 => (sphere(c, room * r.range(0.2, 0.8)), false),
        1 => {
            let h = room * r.range(0.2, 0.8);
            (
                axis_box(
                    [c[0] - h, c[1] - h, c[2] - h],
                    [c[0] + h, c[1] + h, c[2] + h],
                ),
                false,
            )
        }
        _ => {
            let minor = room * r.range(0.1, 0.25);
            let major = minor + room * r.range(0.2, 0.6);
            let t = Primitive::Torus {
                frame: Frame::at(c),
                major,
                minor,
            };
            (t, true)
        }
    };
    let exact = Exact {
        intersection_volume: Some(inner.volume()),
        intersection_area: Some(inner.area()),
        distance: Some(0.0),
        betti_union: Some([1, 0, 0]),
        betti_difference: Some([1, u64::from(ring), 1]),
        ..Default::default()
    };
    (outer, inner, exact)
}

/// A bore through a block, along x, its side exactly tangent to the block's top or bottom face: the
/// block less the bore is one piece that touches itself along that line, its material round the
/// bore thinning to nothing there, and still a tunnel. The bore stands out past both ends.
fn flush_bore(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (w, d, h) = (r.range(6.0, 12.0), r.range(5.0, 10.0), r.range(4.0, 8.0));
    let rad = r.range(0.8, 0.4 * d.min(h));
    let y = r.range(rad + 0.5, d - rad - 0.5);
    let z = if r.next_u64().is_multiple_of(2) {
        h - rad
    } else {
        rad
    };
    let out = r.range(0.5, 2.0);
    let block = axis_box([0.0; 3], [w, d, h]);
    let bore = Primitive::Cylinder {
        frame: Frame {
            origin: [-out, y, z],
            x: [0.0, 1.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [1.0, 0.0, 0.0],
        },
        radius: rad,
        height: w + 2.0 * out,
    };
    let (disc, wall) = (PI * rad * rad, 2.0 * PI * rad * w);
    let exact = Exact {
        intersection_volume: Some(disc * w),
        intersection_area: Some(2.0 * disc + wall),
        distance: Some(0.0),
        betti_union: Some([1, 0, 0]),
        betti_difference: Some([1, 1, 0]),
        ..Default::default()
    };
    (block, bore, exact)
}

/// Two solids touching along a line: a cylinder lying on a box's top face, two cylinders side by
/// side, a cylinder inside another against its wall, or a cone lying on a box's top face along one
/// of its lines. Lying outside, they share no volume; inside, the intersection is the inner one.
fn line_contact(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let w = [r.range(8.0, 14.0), r.range(8.0, 14.0), r.range(3.0, 8.0)];
    let outside = |a: Primitive, b: Primitive| {
        let exact = Exact {
            intersection_volume: Some(0.0),
            intersection_area: Some(0.0),
            distance: Some(0.0),
            ..Default::default()
        };
        (a, b, exact)
    };
    match r.next_u64() % 4 {
        0 => {
            let rad = r.range(0.5, 3.0);
            let (x0, x1) = (w[0] * r.range(0.0, 0.3), w[0] * r.range(0.7, 1.2));
            let y = w[1] * r.range(0.3, 0.7);
            let cyl = rod([x0, y, w[2] + rad], [x1, y, w[2] + rad], rad);
            outside(axis_box([0.0; 3], w), cyl)
        }
        1 => {
            let (r1, r2) = (r.range(1.0, 4.0), r.range(0.5, 4.0));
            let turn = r.range(0.0, TAU);
            let (h1, h2) = (r.range(4.0, 10.0), r.range(4.0, 10.0));
            let c = [
                (r1 + r2) * turn.pcos(),
                (r1 + r2) * turn.psin(),
                r.range(-2.0, 2.0),
            ];
            let a = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: r1,
                height: h1,
            };
            let b = Primitive::Cylinder {
                frame: Frame::at(c),
                radius: r2,
                height: h2,
            };
            outside(a, b)
        }
        2 => {
            let r1 = r.range(2.0, 5.0);
            let r2 = r1 * r.range(0.2, 0.8);
            let (h1, turn) = (r.range(5.0, 10.0), r.range(0.0, TAU));
            let lift = h1 * r.range(0.1, 0.3);
            let h2 = (h1 - lift) * r.range(0.3, 0.9);
            let off = r1 - r2;
            let a = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: r1,
                height: h1,
            };
            let b = Primitive::Cylinder {
                frame: Frame::at([off * turn.pcos(), off * turn.psin(), lift]),
                radius: r2,
                height: h2,
            };
            let exact = Exact {
                intersection_volume: Some(b.volume()),
                intersection_area: Some(b.area()),
                distance: Some(0.0),
                ..Default::default()
            };
            (a, b, exact)
        }
        _ => {
            // Tipped up by its half-angle α, the cone's lowest line is level, b·cos α below the
            // centre of its base.
            let (b, h) = (r.range(1.0, 3.0), r.range(4.0, 8.0));
            let alpha = (b / h).patan();
            let d = [alpha.pcos(), 0.0, -alpha.psin()];
            let o = [
                w[0] * 0.5 - h * alpha.pcos() * 0.5,
                w[1] * r.range(0.3, 0.7),
                w[2] + b * alpha.pcos(),
            ];
            let cone = Primitive::Cone {
                frame: Frame::along(o, d),
                bottom: b,
                top: 0.0,
                height: h,
            };
            outside(axis_box([0.0; 3], [w[0].max(h + 2.0), w[1], w[2]]), cone)
        }
    }
}

/// A box less a pocket box flush with one, two or three of its faces, on a 0.5 mm grid, so the
/// pocket's faces share those planes exactly: a slot, a step or a corner notch.
fn flush_pocket(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let grid = |r: &mut Rng, lo: f64, hi: f64| (r.range(lo, hi) * 2.0).round() / 2.0;
    let w = [grid(r, 6.0, 12.0), grid(r, 6.0, 12.0), grid(r, 6.0, 12.0)];
    let flush = 1 + (r.next_u64() % 3) as usize;
    let first = (r.next_u64() % 3) as usize;
    let (mut lo, mut hi) = ([0.0; 3], [0.0; 3]);
    for i in 0..3 {
        let depth = grid(r, 1.0, w[i] - 2.0);
        if (i + 3 - first) % 3 < flush {
            if r.next_u64().is_multiple_of(2) {
                (lo[i], hi[i]) = (w[i] - depth, w[i]);
            } else {
                (lo[i], hi[i]) = (0.0, depth);
            }
        } else {
            let start = grid(r, 0.5, w[i] - depth - 0.5).max(0.5);
            (lo[i], hi[i]) = (start, (start + depth).min(w[i] - 0.5));
        }
    }
    let pocket = axis_box(lo, hi);
    let exact = Exact {
        intersection_volume: Some(pocket.volume()),
        intersection_area: Some(pocket.area()),
        distance: Some(0.0),
        ..Default::default()
    };
    (axis_box([0.0; 3], w), pocket, exact)
}

/// A box against a solid thin in one direction or two, from 1e-3 to 1e-2 mm: a foil through part
/// of it, a needle right through it, or a coin across its top face.
fn thin_solid(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let w = [r.range(6.0, 12.0), r.range(6.0, 12.0), r.range(6.0, 12.0)];
    let t = 10f64.ppowf(r.range(-3.0, -2.0));
    let block = axis_box([0.0; 3], w);
    let (thin, inside) = match r.next_u64() % 3 {
        0 => {
            let z = w[2] * r.range(0.2, 0.8);
            let (y0, y1) = (w[1] * r.range(0.1, 0.4), w[1] * r.range(0.6, 0.9));
            let foil = axis_box([-1.0, y0, z], [w[0] * r.range(0.3, 0.7), y1, z + t]);
            let inside = axis_box([0.0, y0, z], [foil_end(&foil), y1, z + t]);
            (foil, inside)
        }
        1 => {
            let (y, z) = (w[1] * r.range(0.2, 0.8), w[2] * r.range(0.2, 0.8));
            let needle = rod([-1.0, y, z], [w[0] + 1.0, y, z], t);
            (needle, rod([0.0, y, z], [w[0], y, z], t))
        }
        _ => {
            let rad = r.range(0.5, 1.5);
            let c = [w[0] * r.range(0.3, 0.7), w[1] * r.range(0.3, 0.7)];
            let coin = Primitive::Cylinder {
                frame: Frame::at([c[0], c[1], w[2] - t / 2.0]),
                radius: rad,
                height: t,
            };
            let half = Primitive::Cylinder {
                frame: Frame::at([c[0], c[1], w[2] - t / 2.0]),
                radius: rad,
                height: t / 2.0,
            };
            (coin, half)
        }
    };
    let exact = Exact {
        intersection_volume: Some(inside.volume()),
        intersection_area: Some(inside.area()),
        distance: Some(0.0),
        ..Default::default()
    };
    (block, thin, exact)
}

fn foil_end(foil: &Primitive) -> f64 {
    match foil {
        Primitive::Box { frame, size } => frame.origin[0] + size[0],
        _ => 0.0,
    }
}

/// A box with a ball-shaped cavity, and a smaller ball floating in it clear of the walls. They
/// share nothing and are the cavity's radius less the island's and its offset apart; their union
/// is two pieces, one round a void.
fn island(r: &mut Rng) -> Drawn {
    let w = r.range(8.0, 14.0);
    let c = [w / 2.0; 3];
    let cavity = w / 2.0 * r.range(0.4, 0.8);
    let rad = cavity * r.range(0.2, 0.7);
    let room = cavity - rad;
    let e = room * r.range(0.0, 0.8);
    let turn = r.range(0.0, TAU);
    let at = [c[0] + e * turn.pcos(), c[1] + e * turn.psin(), c[2]];
    let shell = Shape::bool(
        Op::Difference,
        axis_box([0.0; 3], [w; 3]),
        sphere(c, cavity),
    );
    let exact = Exact {
        intersection_volume: Some(0.0),
        intersection_area: Some(0.0),
        distance: Some(room - e),
        betti_a: Some([1, 0, 1]),
        betti_b: Some([1, 0, 0]),
        betti_union: Some([2, 0, 1]),
        betti_difference: Some([1, 0, 1]),
        ..Default::default()
    };
    (shell, sphere(at, rad).into(), exact)
}

/// A block worked by ten to thirty cuts and additions of small primitives in turn, as a part is
/// modelled, crossed by a box or a cylinder.
fn long_chains(r: &mut Rng) -> Drawn {
    let w = 20.0;
    let mut part: Shape = axis_box([0.0; 3], [w; 3]).into();
    let n = 10 + (r.next_u64() % 21) as usize;
    for _ in 0..n {
        let c = [r.range(0.0, w), r.range(0.0, w), r.range(0.0, w)];
        let size = r.range(1.0, 5.0);
        let tool = match r.next_u64() % 3 {
            0 => sphere(c, size / 2.0),
            1 => Primitive::Cylinder {
                frame: Frame::at(c),
                radius: size / 2.0,
                height: size,
            },
            _ => {
                let h = size / 2.0;
                axis_box(
                    [c[0] - h, c[1] - h, c[2] - h],
                    [c[0] + h, c[1] + h, c[2] + h],
                )
            }
        };
        let op = if r.next_u64().is_multiple_of(3) {
            Op::Union
        } else {
            Op::Difference
        };
        part = Shape::bool(op, part, tool);
    }
    let b = crossing(r, [w / 2.0; 3], w);
    (part, b.into(), Exact::default())
}

/// A plate drilled with a square grid of 25, 49 or 100 holes, cut one after another, crossed by a
/// box over part of it: work that grows with the part, for timing as much as for correctness.
fn many_holes(r: &mut Rng) -> Drawn {
    let n = [5usize, 7, 10][(r.next_u64() % 3) as usize];
    let pitch = r.range(3.0, 5.0);
    let (side, t) = (pitch * n as f64, r.range(2.0, 4.0));
    let mut plate: Shape = axis_box([0.0; 3], [side, side, t]).into();
    let hole = pitch * r.range(0.15, 0.35);
    for i in 0..n {
        for j in 0..n {
            let at = [pitch * (i as f64 + 0.5), pitch * (j as f64 + 0.5), -1.0];
            let c = Primitive::Cylinder {
                frame: Frame::at(at),
                radius: hole,
                height: t + 2.0,
            };
            plate = Shape::bool(Op::Difference, plate, c);
        }
    }
    let x = side * r.range(0.3, 0.7);
    let b = axis_box([x, -1.0, -1.0], [side + 1.0, side + 1.0, t / 2.0]);
    let exact = Exact {
        betti_a: Some([1, (n * n) as u64, 0]),
        ..Default::default()
    };
    (plate, b.into(), exact)
}

/// A pipe or bar: a round or rounded-square section carried along a straight run, a bend, and
/// sometimes another run or a second bend, crossed by a box or a cylinder.
fn sweeps(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let rho = r.range(0.5, 2.0);
    let profile = if r.next_u64().is_multiple_of(2) {
        Profile {
            loops: vec![circle([0.0; 2], rho, true)],
        }
    } else {
        let sq = [[-rho, -rho], [rho, -rho], [rho, rho], [-rho, rho]];
        Profile {
            loops: vec![rounded(&sq, &[rho * r.range(0.0, 0.5); 4])],
        }
    };
    let mut path = vec![PathSeg::Line {
        length: r.range(2.0, 8.0),
    }];
    let turns = 1 + (r.next_u64() % 2) as usize;
    for _ in 0..turns {
        path.push(PathSeg::Arc {
            radius: rho * r.range(1.6, 5.0),
            angle: r.range(0.3, 2.0),
        });
        if r.next_u64().is_multiple_of(2) {
            path.push(PathSeg::Line {
                length: r.range(1.0, 6.0),
            });
        }
    }
    let a = Primitive::Sweep {
        frame: Frame::at([0.0; 3]),
        profile,
        path,
    };
    let (lo, hi) = crate::oracle::bounds(&Shape::Prim(a.clone())).expect("a sweep has a box");
    let mid = [0, 1, 2].map(|k| 0.5 * (lo[k] + hi[k]));
    let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max) / 2.0;
    let b = crossing(r, mid, size);
    (a, b, Exact::default())
}

/// A disc, a rounded square or an off-centre disc swept along a cubic B-spline climbing in z with
/// a sideways wander, half the time twisted through up to half a turn; crossed by a ball or block
/// through its middle. The profile reaches no more than a third of the least radius of curvature
/// from the path, so the sweep never folds over itself.
fn path_sweeps(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    use brep_protocol::path::PathCurve;
    let n = 5 + (r.next_u64() % 3) as usize;
    let mut ctrl = vec![[0.0; 3], [0.0, 0.0, r.range(1.5, 3.0)]];
    while ctrl.len() < n {
        let last = ctrl[ctrl.len() - 1];
        ctrl.push([
            last[0] + r.range(-1.5, 1.5),
            last[1] + r.range(-1.5, 1.5),
            last[2] + r.range(1.5, 3.0),
        ]);
    }
    let inner = n - 4;
    let mut knots = vec![0.0; 4];
    knots.extend((1..=inner).map(|i| i as f64 / (inner + 1) as f64));
    knots.extend([1.0; 4]);
    let path = PathCurve {
        degree: 3,
        knots,
        ctrl,
    };
    // The least radius of curvature, sampled finely.
    let rho_min = (0..=2000)
        .map(|i| {
            let [_, d1, d2] = path.eval(i as f64 / 2000.0);
            let c = cross3(d1, d2);
            let speed = (d1[0] * d1[0] + d1[1] * d1[1] + d1[2] * d1[2]).sqrt();
            speed.ppowi(3) / (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().max(1e-300)
        })
        .fold(f64::INFINITY, f64::min);
    let reach = (rho_min / 3.0).min(1.5);
    let profile = match r.next_u64() % 3 {
        0 => Profile {
            loops: vec![circle([0.0; 2], reach * r.range(0.5, 1.0), true)],
        },
        1 => {
            let h = reach * r.range(0.4, 0.7);
            let sq = [[-h, -h], [h, -h], [h, h], [-h, h]];
            Profile {
                loops: vec![rounded(&sq, &[h * r.range(0.0, 0.5); 4])],
            }
        }
        _ => {
            let rad = reach * r.range(0.3, 0.5);
            Profile {
                loops: vec![circle([reach - rad, 0.0], rad, true)],
            }
        }
    };
    let twist = if r.next_u64().is_multiple_of(2) {
        0.0
    } else {
        r.range(-PI, PI)
    };
    let a = Primitive::SweepPath {
        frame: Frame::at([0.0; 3]),
        profile,
        path,
        twist,
    };
    let (lo, hi) = crate::oracle::bounds(&Shape::Prim(a.clone())).expect("a sweep has a box");
    let mid = [0, 1, 2].map(|k| 0.5 * (lo[k] + hi[k]));
    let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max) / 2.0;
    let b = crossing(r, mid, size);
    (a, b, Exact::default())
}

/// A prism, a cylinder or a frustum written under AP203 or AP242 instead of AP214: the same
/// entities, under another schema and application context.
fn step_schema(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let (a, b, mut exact) = if r.next_u64().is_multiple_of(2) {
        step_prism(r)
    } else {
        step_revolved(r)
    };
    let schema = if r.next_u64().is_multiple_of(2) {
        crate::step::Schema::Ap203
    } else {
        crate::step::Schema::Ap242
    };
    exact.step_as = Some(StepAs::Schema(schema));
    (a, b, exact)
}

/// A prism on a convex polygon written as an AP242 tessellated solid: triangles with no surfaces.
/// A reader gives a polyhedron, whose volume and area are the prism's exactly.
fn step_tessellated(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let n = 3 + (r.next_u64() % 6) as usize;
    let rad = r.range(2.0, 6.0);
    let pts: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = TAU * i as f64 / n as f64;
            [rad * t.pcos(), rad * t.psin()]
        })
        .collect();
    let origin = [
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
    ];
    let a = Primitive::Extrude {
        frame: Frame::along(origin, r.direction()),
        profile: polygon(&pts),
        height: r.range(1.0, 10.0),
    };
    let exact = Exact {
        step_as: Some(StepAs::Tessellated),
        ..Default::default()
    };
    (a, axis_box([0.0; 3], [1.0; 3]), exact)
}

/// A cylinder or a frustum widening or narrowing, at any angle, for a STEP file to describe: a
/// cylindrical or conical face closed by its seam.
fn step_revolved(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let origin = [
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
    ];
    let frame = Frame::along(origin, r.direction());
    let (bottom, height) = (r.range(1.0, 6.0), r.range(1.0, 10.0));
    let a = if r.next_u64().is_multiple_of(2) {
        Primitive::Cylinder {
            frame,
            radius: bottom,
            height,
        }
    } else {
        Primitive::Cone {
            frame,
            bottom,
            top: r.range(0.5, 6.0),
            height,
        }
    };
    (a, axis_box([0.0; 3], [1.0; 3]), Exact::default())
}

/// A prism on a rectangle, an L or a convex polygon, at any angle, for a STEP file to describe.
fn step_prism(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let pts: Vec<[f64; 2]> = match r.next_u64() % 3 {
        0 => {
            let (w, h) = (r.range(2.0, 10.0), r.range(2.0, 10.0));
            vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
        }
        1 => {
            let (w, h, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(1.5, 3.5));
            vec![[0.0, 0.0], [w, 0.0], [w, t], [t, t], [t, h], [0.0, h]]
        }
        _ => {
            let n = 3 + (r.next_u64() % 6) as usize;
            let rad = r.range(2.0, 6.0);
            (0..n)
                .map(|i| {
                    let t = TAU * i as f64 / n as f64;
                    [rad * t.pcos(), rad * t.psin()]
                })
                .collect()
        }
    };
    let origin = [
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
        r.range(-20.0, 20.0),
    ];
    let a = Primitive::Extrude {
        frame: Frame::along(origin, r.direction()),
        profile: Profile {
            loops: vec![rounded(&pts, &vec![0.0; pts.len()])],
        },
        height: r.range(1.0, 10.0),
    };
    (a, axis_box([0.0; 3], [1.0; 3]), Exact::default())
}

/// A slab under a bicubic height field on a 4 × 4 to 7 × 7 grid of coefficients, uniform clamped
/// knots, heights 1 to 9 mm.
fn slab(r: &mut Rng, size: [f64; 2], frame: Frame) -> Primitive {
    let knots = |n: usize| -> Vec<f64> {
        let inner = n - 4;
        let mut k = vec![0.0; 4];
        k.extend((1..=inner).map(|i| i as f64 / (inner + 1) as f64));
        k.extend([1.0; 4]);
        k
    };
    let (nu, nv) = (
        4 + (r.next_u64() % 4) as usize,
        4 + (r.next_u64() % 4) as usize,
    );
    let base = r.range(2.0, 6.0);
    let heights = (0..nu)
        .map(|_| {
            (0..nv)
                .map(|_| (base + r.range(-1.5, 3.0)).max(1.0))
                .collect()
        })
        .collect();
    Primitive::Slab {
        frame,
        size,
        knots_u: knots(nu),
        knots_v: knots(nv),
        heights,
    }
}

/// A freeform slab crossed by a ball or cylinder through its top surface, a box shaving it, or a
/// second slab turned a quarter about z and shifted, so two freeform tops meet.
fn slabs(r: &mut Rng) -> (Primitive, Primitive, Exact) {
    let size = [r.range(4.0, 10.0), r.range(4.0, 10.0)];
    let a = slab(r, size, Frame::at([0.0; 3]));
    let c = [r.range(0.2, 0.8) * size[0], r.range(0.2, 0.8) * size[1]];
    let top = a
        .height_field()
        .expect("a slab")
        .eval(c[0] / size[0], c[1] / size[1])
        .0;
    let b = match r.next_u64() % 4 {
        0 => sphere([c[0], c[1], top], r.range(1.0, 3.0)),
        1 => Primitive::Cylinder {
            frame: Frame::at([c[0], c[1], -1.0]),
            radius: r.range(0.5, 2.0),
            height: top + 2.0 + r.range(-0.5 * top, 0.0),
        },
        2 => {
            let cut = top * r.range(0.6, 0.95);
            axis_box(
                [-1.0, -1.0, cut],
                [size[0] + 1.0, size[1] + 1.0, cut + 20.0],
            )
        }
        _ => {
            let other = [r.range(4.0, 10.0), r.range(4.0, 10.0)];
            let f = Frame {
                origin: [
                    c[0] + other[1] / 2.0,
                    c[1] - other[0] / 2.0,
                    r.range(-1.0, 1.0),
                ],
                x: [0.0, 1.0, 0.0],
                y: [-1.0, 0.0, 0.0],
                z: [0.0, 0.0, 1.0],
            };
            slab(r, other, f)
        }
    };
    (a, b, Exact::default())
}

/// The points within `r` of the axis box `lo`–`hi`: three slabs, twelve edge cylinders and eight
/// corner balls, with Steiner's formula for its volume and area.
fn rounded_block(lo: Vec3, hi: Vec3, r: f64) -> (Shape, f64, f64) {
    let d = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let mut s: Shape = axis_box([lo[0] - r, lo[1], lo[2]], [hi[0] + r, hi[1], hi[2]]).into();
    s = Shape::bool(
        Op::Union,
        s,
        axis_box([lo[0], lo[1] - r, lo[2]], [hi[0], hi[1] + r, hi[2]]),
    );
    s = Shape::bool(
        Op::Union,
        s,
        axis_box([lo[0], lo[1], lo[2] - r], [hi[0], hi[1], hi[2] + r]),
    );
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for (cu, cv) in [
            (lo[u], lo[v]),
            (hi[u], lo[v]),
            (lo[u], hi[v]),
            (hi[u], hi[v]),
        ] {
            let mut start = [0.0; 3];
            start[axis] = lo[axis];
            start[u] = cu;
            start[v] = cv;
            let mut dir = [0.0; 3];
            dir[axis] = 1.0;
            let c = Primitive::Cylinder {
                frame: Frame::along(start, dir),
                radius: r,
                height: d[axis],
            };
            s = Shape::bool(Op::Union, s, c);
        }
    }
    for i in 0..8 {
        let c = [0, 1, 2].map(|k| if i & (1 << k) == 0 { lo[k] } else { hi[k] });
        s = Shape::bool(Op::Union, s, sphere(c, r));
    }
    let (pq, qs, sp) = (d[0] * d[1], d[1] * d[2], d[2] * d[0]);
    let sum = d[0] + d[1] + d[2];
    let volume = d[0] * d[1] * d[2]
        + 2.0 * r * (pq + qs + sp)
        + PI * r * r * sum
        + 4.0 / 3.0 * PI * r.ppowi(3);
    let area = 2.0 * (pq + qs + sp) + 2.0 * PI * r * sum + 4.0 * PI * r * r;
    (s, volume, area)
}

/// A box with all twelve edges filleted by one radius: three fillets meet at every corner, closed
/// by a ball. The result is the inner box swelled by the radius.
fn rounded_box(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(3.0, 10.0), r.range(3.0, 10.0), r.range(3.0, 10.0)];
    let rad = w.iter().copied().fold(f64::INFINITY, f64::min) * r.range(0.05, 0.45);
    let base = axis_box([0.0; 3], w);
    let mut points = Vec::new();
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for (cu, cv) in [(0.0, 0.0), (w[u], 0.0), (0.0, w[v]), (w[u], w[v])] {
            let mut p = [0.0; 3];
            p[axis] = w[axis] * r.range(0.3, 0.7);
            p[u] = cu;
            p[v] = cv;
            points.push(p);
        }
    }
    let (shape, volume, area) = rounded_block([rad; 3], [w[0] - rad, w[1] - rad, w[2] - rad], rad);
    let feature = Feature {
        act: Act::Fillet,
        points,
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    (base, feature, shape, exact)
}

/// A rectangle, or a convex polygon of five to eight corners on a circle, counter-clockwise.
fn convex_polygon(r: &mut Rng) -> Vec<[f64; 2]> {
    if r.next_u64().is_multiple_of(3) {
        let (w, h) = (r.range(4.0, 12.0), r.range(4.0, 12.0));
        return vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    }
    let n = 5 + (r.next_u64() % 4) as usize;
    let rad = r.range(4.0, 8.0);
    let turn = r.range(0.0, 1.0);
    (0..n)
        .map(|i| {
            let t = turn + TAU * (i as f64 + r.range(-0.15, 0.15)) / n as f64;
            [rad * t.pcos(), rad * t.psin()]
        })
        .collect()
}

/// The convex counter-clockwise polygon with each side moved `d` inward.
fn inset(pts: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let n = pts.len();
    let normal = |i: usize| {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        let len = (q[0] - p[0]).phypot(q[1] - p[1]);
        [-(q[1] - p[1]) / len, (q[0] - p[0]) / len]
    };
    (0..n)
        .map(|i| {
            let (a, b) = (normal((i + n - 1) % n), normal(i));
            let k = d / (1.0 + a[0] * b[0] + a[1] * b[1]);
            [pts[i][0] + k * (a[0] + b[0]), pts[i][1] + k * (a[1] + b[1])]
        })
        .collect()
}

/// The largest corner radius every corner of a polygon takes while its rounds keep within 0.45
/// of each side.
fn round_limit(pts: &[[f64; 2]]) -> f64 {
    corners(pts)
        .iter()
        .map(|&(turn, side)| 0.45 * side / (turn.abs() / 2.0).ptan())
        .fold(f64::INFINITY, f64::min)
}

fn polygon(pts: &[[f64; 2]]) -> Profile {
    Profile {
        loops: vec![rounded(pts, &vec![0.0; pts.len()])],
    }
}

/// The cylinder of `radius` along the segment `a`–`b`.
fn rod(a: Vec3, b: Vec3, radius: f64) -> Primitive {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    Primitive::Cylinder {
        frame: Frame::along(a, d.map(|x| x / len)),
        radius,
        height: len,
    }
}

/// A prism on a convex polygon with every corner rounded by ρ, filleted by r at one point of one
/// straight top edge. That edge's tangent chain is the whole top loop, so the loop rounds all the
/// way round. With the profile inset by r, of area A' and perimeter L', the result is the prism to
/// h − r, the inset prism to h, and a quarter tube along the inset loop:
/// `V = A(h − r) + A'r + πr²L'/4 + 2πr³/3`, `S = A + L(h − r) + A' + πrL'/2 + 2πr²`.
fn fillet_chain(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let rho = round_limit(&pts) * r.range(0.4, 1.0);
    let rad = rho * r.range(0.15, 0.45);
    let h = r.range(2.0, 10.0).max(3.0 * rad);
    let outer = Profile {
        loops: vec![rounded(&pts, &vec![rho; n])],
    };
    let inner_loop = rounded(&inset(&pts, rad), &vec![rho - rad; n]);
    let inner = Profile {
        loops: vec![inner_loop.clone()],
    };
    let i = (r.next_u64() % n as u64) as usize;
    let (p, q) = (pts[i], pts[(i + 1) % n]);
    let feature = Feature {
        act: Act::Fillet,
        points: vec![[(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, h]],
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: outer.clone(),
        height: h,
    };
    let z = h - rad;
    let mut shape = Shape::bool(
        Op::Union,
        Primitive::Extrude {
            frame: Frame::at([0.0; 3]),
            profile: outer.clone(),
            height: z,
        },
        Primitive::Extrude {
            frame: Frame::at([0.0; 3]),
            profile: inner.clone(),
            height: h,
        },
    );
    for seg in &inner_loop {
        let tube = match *seg {
            Seg::Line { a, b } if (b[0] - a[0]).phypot(b[1] - a[1]) > 1e-9 => {
                rod([a[0], a[1], z], [b[0], b[1], z], rad)
            }
            Seg::Arc { center, radius, .. } => Primitive::Torus {
                frame: Frame::at([center[0], center[1], z]),
                major: radius,
                minor: rad,
            },
            _ => continue,
        };
        shape = Shape::bool(Op::Union, shape, tube);
    }
    let (a, l) = (outer.area(), outer.perimeter());
    let (ai, li) = (inner.area(), inner.perimeter());
    let exact = Exact {
        result_volume: Some(
            a * z + ai * rad + PI * rad * rad / 4.0 * li + 2.0 * PI / 3.0 * rad.ppowi(3),
        ),
        result_area: Some(a + l * z + ai + PI / 2.0 * rad * li + 2.0 * PI * rad * rad),
        ..Default::default()
    };
    (base, feature, shape, exact)
}

/// A prism on a convex polygon with every edge filleted by r: the prism Q on the polygon inset by
/// r, between r and h − r, swelled by r. By Steiner's formula, with Q's base of area a and
/// perimeter l and its height H = h − 2r,
/// `V = aH + (2a + lH)r + (πH + πl/2)r² + 4πr³/3` and `S = 2a + lH + 2(πH + πl/2)r + 4πr²`.
fn rounded_prism(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let h = r.range(2.0, 10.0);
    let rad = round_limit(&pts).min(0.45 * h) * r.range(0.2, 0.9);
    let q = inset(&pts, rad);
    let mut points = Vec::new();
    for i in 0..n {
        let (p, o) = (pts[i], pts[(i + 1) % n]);
        let mid = [(p[0] + o[0]) / 2.0, (p[1] + o[1]) / 2.0];
        points.push([p[0], p[1], h * r.range(0.3, 0.7)]);
        points.push([mid[0], mid[1], h]);
        points.push([mid[0], mid[1], 0.0]);
    }
    let feature = Feature {
        act: Act::Fillet,
        points,
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&pts),
        height: h,
    };
    let big_h = h - 2.0 * rad;
    let mut shape = Shape::bool(
        Op::Union,
        Primitive::Extrude {
            frame: Frame::at([0.0, 0.0, rad]),
            profile: Profile {
                loops: vec![rounded(&pts, &vec![rad; n])],
            },
            height: big_h,
        },
        Primitive::Extrude {
            frame: Frame::at([0.0; 3]),
            profile: polygon(&q),
            height: h,
        },
    );
    for z in [rad, h - rad] {
        for i in 0..n {
            let (p, o) = (q[i], q[(i + 1) % n]);
            shape = Shape::bool(Op::Union, shape, rod([p[0], p[1], z], [o[0], o[1], z], rad));
            shape = Shape::bool(Op::Union, shape, sphere([p[0], p[1], z], rad));
        }
    }
    let base_q = polygon(&q);
    let (a, l) = (base_q.area(), base_q.perimeter());
    let m = PI * big_h + PI * l / 2.0;
    let s = 2.0 * a + l * big_h;
    let exact = Exact {
        result_volume: Some(a * big_h + s * rad + m * rad * rad + 4.0 / 3.0 * PI * rad.ppowi(3)),
        result_area: Some(s + 2.0 * m * rad + 4.0 * PI * rad * rad),
        ..Default::default()
    };
    (base, feature, shape, exact)
}

/// An L-prism with every convex edge filleted by r; the concave edge stays sharp. The result is
/// the union of the balls of radius r inside the prism. Away from the concave edge's ends that is
/// the L-shaped core A' ∪ B' of its two arm boxes shrunk by r, swelled by r; at each end the ball
/// also rolls round the concave edge, which the core misses (below). The core's volume is the two
/// rounded arms' less their overlap, which
/// is the rounded corner column C' plus, in each slice of swell ρ, a square of ρ² less a quarter
/// disk facing the concave edge: `V = V_A + V_B − V_C − (1 − π/4)(H'r² + 4r³/3)`, H' = h − 2r.
/// Its area is the volume's derivative in the swell: `S = S_A + S_B − S_C − (1 − π/4)(2H'r + 4r²)`.
fn opened_l_prism(r: &mut Rng) -> FeatureDraw {
    let (w, d, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(2.0, 3.5));
    let h = r.range(2.0, 8.0);
    let rad = t.min(h) * r.range(0.1, 0.45);
    let pts = vec![[0.0, 0.0], [w, 0.0], [w, t], [t, t], [t, d], [0.0, d]];
    let n = pts.len();
    let mut points = Vec::new();
    for i in 0..n {
        let (p, o) = (pts[i], pts[(i + 1) % n]);
        let mid = [(p[0] + o[0]) / 2.0, (p[1] + o[1]) / 2.0];
        if i != 3 {
            points.push([p[0], p[1], h * r.range(0.3, 0.7)]);
        }
        points.push([mid[0], mid[1], h]);
        points.push([mid[0], mid[1], 0.0]);
    }
    let feature = Feature {
        act: Act::Fillet,
        points,
        size: rad,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&pts),
        height: h,
    };
    let lo = [rad; 3];
    let (arm_a, va, sa) = rounded_block(lo, [w - rad, t - rad, h - rad], rad);
    let (arm_b, vb, sb) = rounded_block(lo, [t - rad, d - rad, h - rad], rad);
    let (_, vc, sc) = rounded_block(lo, [t - rad, t - rad, h - rad], rad);
    let (k, core_h) = (1.0 - PI / 4.0, h - 2.0 * rad);
    // Where the top or bottom fillets meet over the concave edge, the ball swings round that edge
    // on a quarter circle of radius r: a horn torus, not the mitre the swelled core makes. In each
    // slice s from the ball centres' plane the corner keeps the square of side δ = r − √(r² − s²)
    // less its quarter disk, k·δ², which over the corner's height is k·r³(5/3 − π/2); the torus,
    // (π/2)·r²·(π/2 − 1) of area, stands for the two mitred cylinder pieces, r²(π/2 − 1) each,
    // and the flat face keeps the square of side r at the corner less its quarter disk, k·r².
    let corner = |zc: f64, lo: f64| {
        let column = axis_box([t - rad, t - rad, lo], [t, t, lo + rad]);
        let rod = Primitive::Cylinder {
            frame: Frame::at([t, t, lo - 1.0]),
            radius: rad,
            height: rad + 2.0,
        };
        let horn = Primitive::Torus {
            frame: Frame::at([t, t, zc]),
            major: rad,
            minor: rad,
        };
        Shape::bool(
            Op::Difference,
            column,
            Shape::bool(Op::Difference, rod, horn),
        )
    };
    let corners = Shape::bool(Op::Union, corner(h - rad, h - rad), corner(rad, 0.0));
    let swing = 2.0 * k * (5.0 / 3.0 - PI / 2.0) * rad.ppowi(3);
    let swing_area = 2.0 * ((2.0 - PI / 2.0) * (PI / 2.0 - 1.0) - k) * rad * rad;
    let exact = Exact {
        result_volume: Some(
            va + vb - vc - k * (core_h * rad * rad + 4.0 / 3.0 * rad.ppowi(3)) + swing,
        ),
        result_area: Some(sa + sb - sc - k * (2.0 * core_h * rad + 4.0 * rad * rad) - swing_area),
        ..Default::default()
    };
    let arms = Shape::bool(Op::Union, arm_a, arm_b);
    (base, feature, Shape::bool(Op::Union, arms, corners), exact)
}

/// A box with the three edges at one corner filleted in one operation, at radii not all equal. The
/// corner where they meet is open; each edge alone, cut square by the box's far faces, takes
/// `(1 − π/4)r²` from every unit of its length, and three fillets of one radius r meeting at a
/// corner take `r³(1 − π/6)` more from the corner's cube than their lengths outside it.
fn uneven_corner(r: &mut Rng) -> FeatureDraw {
    uneven_blend(r, Act::FilletRadii)
}

/// As [`uneven_corner`], with chamfers: each edge alone takes `d²/2` from every unit of its
/// length, and three chamfers of one setback d meeting at a corner keep a quarter of its cube.
fn uneven_chamfer_corner(r: &mut Rng) -> FeatureDraw {
    uneven_blend(r, Act::ChamferDistances)
}

fn uneven_blend(r: &mut Rng, act: Act) -> FeatureDraw {
    // What a blend of size s takes per unit of edge, and from a corner cube where three meet.
    let (k, cube) = match act {
        Act::ChamferDistances => (0.5, 0.75),
        _ => (1.0 - PI / 4.0, 1.0 - PI / 6.0),
    };
    let w = [r.range(8.0, 15.0), r.range(8.0, 15.0), r.range(8.0, 15.0)];
    let mut radii = [r.range(0.8, 1.6), r.range(0.8, 1.6), r.range(0.8, 1.6)];
    radii[(r.next_u64() % 3) as usize] *= 1.6;
    let corner = w;
    let mut points = Vec::new();
    for axis in 0..3 {
        let mut p = corner;
        p[axis] = w[axis] * r.range(0.3, 0.7);
        points.push(p);
    }
    let base = axis_box([0.0; 3], w);
    let full = w[0] * w[1] * w[2];
    let at = |rad: f64| {
        full - w.iter().map(|l| k * rad * rad * (l - rad)).sum::<f64>() - rad.ppowi(3) * cube
    };
    let (lo, hi) = radii
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
    let side = 3.0 * hi;
    let zone = axis_box(corner.map(|x| x - side), corner.map(|x| x + 1.0));
    // The box with all three edges blended at one size: chamfered, less each edge's wedge;
    // filleted, what the balls of that radius inside it reach.
    let far = 2.0 * (w[0] + w[1] + w[2]);
    let at_one = |size: f64| -> Shape {
        if act == Act::ChamferDistances {
            (0..3).fold(base.clone().into(), |s: Shape, axis| {
                Shape::bool(
                    Op::Difference,
                    s,
                    corner_sliver(corner, axis, -1.0, far, size, None),
                )
            })
        } else {
            Shape::bool(
                Op::Intersection,
                base.clone(),
                rounded_block([-far; 3], corner.map(|x| x - size), size).0,
            )
        }
    };
    let scans = match act {
        Act::ChamferDistances => Vec::new(),
        _ => vec![corner_scan(corner, 2.0 * hi, lo, false, false)],
    };
    // Three chamfers' planes meet at a point, and tools differ on whether the result runs out to
    // it or is cut flat across short of it, within the largest distance of the corner every way:
    // there a chamfered corner is held to nothing.
    let (inner, outer) = match act {
        Act::ChamferDistances => (
            Shape::bool(
                Op::Difference,
                at_one(hi),
                axis_box(corner.map(|x| x - hi), corner.map(|x| x + 1.0)),
            ),
            at_one(lo),
        ),
        _ => (at_one(hi), at_one(lo)),
    };
    let outside = full
        - side.ppowi(3)
        - (0..3)
            .map(|i| k * radii[i] * radii[i] * (w[i] - side))
            .sum::<f64>();
    let feature = Feature {
        act,
        points,
        size: hi,
        pull: None,
        radii: radii.to_vec(),
        matrix: None,
    };
    let exact = Exact {
        sound: Some(Sound {
            zone: Some(zone),
            outside,
            least: at(hi),
            most: at(lo),
            betti: [1, 0, 0],
            contacts: Vec::new(),
            limits: Some(Limits::Between {
                inner: Box::new(inner),
                outer: Box::new(outer),
            }),
            scans,
            ..Default::default()
        }),
        ..Default::default()
    };
    (base.clone(), feature, base.into(), exact)
}

/// Convex primitives offset outward: a box (a rounded block), a ball (a larger ball), and a
/// cylinder or frustum (the revolution of its meridian moved out, its rims rounded).
fn offsets(r: &mut Rng) -> FeatureDraw {
    let d = r.range(0.2, 2.0);
    let feature = Feature {
        act: Act::Offset,
        points: vec![],
        size: d,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    match r.next_u64() % 3 {
        0 => {
            let w = [r.range(2.0, 8.0), r.range(2.0, 8.0), r.range(2.0, 8.0)];
            let (shape, volume, area) = rounded_block([0.0; 3], w, d);
            let exact = Exact {
                result_volume: Some(volume),
                result_area: Some(area),
                ..Default::default()
            };
            (axis_box([0.0; 3], w), feature, shape, exact)
        }
        1 => {
            let rad = r.range(1.0, 6.0);
            let want = sphere([0.0; 3], rad + d);
            (
                sphere([0.0; 3], rad),
                feature,
                want.into(),
                Exact::default(),
            )
        }
        _ => {
            let (bottom, height) = (r.range(2.0, 6.0), r.range(2.0, 8.0));
            let top = if r.next_u64().is_multiple_of(2) {
                bottom
            } else {
                r.range(0.5, 6.0)
            };
            let base = if top == bottom {
                Primitive::Cylinder {
                    frame: Frame::at([0.0; 3]),
                    radius: bottom,
                    height,
                }
            } else {
                cone(Frame::at([0.0; 3]), bottom, top, height)
            };
            // The slant moved out by d along its normal, met by the bottom and top moved by d.
            let len = (bottom - top).phypot(height);
            let x_at = |y: f64| bottom + (d * len - y * (bottom - top)) / height;
            let pts = [
                [0.0, -d],
                [x_at(-d), -d],
                [x_at(height + d), height + d],
                [0.0, height + d],
            ];
            let want = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile: Profile {
                    loops: vec![rounded(&pts, &[0.0, d, d, 0.0])],
                },
                angle: TAU,
            };
            (base, feature, want.into(), Exact::default())
        }
    }
}

/// A box or cylinder hollowed to walls `t` thick: open through its top face, or closed round a
/// void.
fn shells(r: &mut Rng) -> FeatureDraw {
    let open = !r.next_u64().is_multiple_of(3);
    if r.next_u64().is_multiple_of(2) {
        let w = [r.range(3.0, 10.0), r.range(3.0, 10.0), r.range(3.0, 10.0)];
        let t = w.iter().copied().fold(f64::INFINITY, f64::min) * r.range(0.05, 0.3);
        let base = axis_box([0.0; 3], w);
        let (a, b, c) = (
            w[0] - 2.0 * t,
            w[1] - 2.0 * t,
            w[2] - if open { t } else { 2.0 * t },
        );
        let top = if open { w[2] + 1.0 } else { w[2] - t };
        let inner = axis_box([t; 3], [w[0] - t, w[1] - t, top]);
        let outer_area = 2.0 * (w[0] * w[1] + w[1] * w[2] + w[2] * w[0]);
        let inner_area = if open {
            a * b + 2.0 * (a + b) * c - a * b
        } else {
            2.0 * (a * b + b * c + c * a)
        };
        let exact = Exact {
            result_volume: Some(w[0] * w[1] * w[2] - a * b * c),
            result_area: Some(outer_area + inner_area),
            ..Default::default()
        };
        let points = if open {
            vec![[w[0] / 2.0, w[1] / 2.0, w[2]]]
        } else {
            vec![]
        };
        let feature = Feature {
            act: Act::Shell,
            points,
            size: t,
            pull: None,
            radii: Vec::new(),
            matrix: None,
        };
        let want = Shape::bool(Op::Difference, base.clone(), inner);
        (base, feature, want, exact)
    } else {
        let (rad, h) = (r.range(2.0, 6.0), r.range(3.0, 10.0));
        let t = rad.min(h) * r.range(0.05, 0.3);
        let base = Primitive::Cylinder {
            frame: Frame::at([0.0; 3]),
            radius: rad,
            height: h,
        };
        let (ri, hi) = (rad - t, h - if open { t } else { 2.0 * t });
        let inner = Primitive::Cylinder {
            frame: Frame::at([0.0, 0.0, t]),
            radius: ri,
            height: if open { h } else { hi },
        };
        let outer_area = 2.0 * PI * rad * (rad + h);
        let inner_area = if open {
            2.0 * PI * ri * hi
        } else {
            2.0 * PI * ri * (ri + hi)
        };
        let exact = Exact {
            result_volume: Some(PI * rad * rad * h - PI * ri * ri * hi),
            result_area: Some(outer_area + inner_area),
            ..Default::default()
        };
        let points = if open { vec![[0.0, 0.0, h]] } else { vec![] };
        let feature = Feature {
            act: Act::Shell,
            points,
            size: t,
            pull: None,
            radii: Vec::new(),
            matrix: None,
        };
        let want = Shape::bool(Op::Difference, base.clone(), inner);
        (base, feature, want, exact)
    }
}

/// The half-space of points on the side of the plane through `p` that `n` points into, as a box
/// 10⁴ mm across; `y` is a unit direction in the plane.
fn halfspace(p: Vec3, n: Vec3, y: Vec3) -> Primitive {
    let big = 1e4;
    let z = [
        n[1] * y[2] - n[2] * y[1],
        n[2] * y[0] - n[0] * y[2],
        n[0] * y[1] - n[1] * y[0],
    ];
    let origin = [0, 1, 2].map(|k| p[k] - y[k] * big / 2.0 - z[k] * big / 2.0);
    Primitive::Box {
        frame: Frame { origin, x: n, y, z },
        size: [big; 3],
    }
}

/// A box with one to four of its sides drafted by one angle about its base, narrowing it upward.
/// The result is the box cut by the tilted planes; its volume and area follow from its sections,
/// whose sides shrink linearly with height.
fn drafts(r: &mut Rng) -> FeatureDraw {
    drafted_box(r, false)
}

/// As [`drafts`], at an angle steep enough that the drafted walls meet below the top: the part
/// closes along a ridge, at an apex or against an undrafted wall, and the top face is gone. The
/// sections still shrink linearly, to nothing at the height z* where the walls meet, so the same
/// closed forms hold with z* for the height.
fn closing_drafts(r: &mut Rng) -> FeatureDraw {
    drafted_box(r, true)
}

fn drafted_box(r: &mut Rng, closing: bool) -> FeatureDraw {
    let (w, d, h) = (r.range(4.0, 10.0), r.range(4.0, 10.0), r.range(2.0, 6.0));
    let mut angle = r.range(0.02, 0.3);
    let mut sides: Vec<usize> = (0..4).filter(|_| r.next_u64().is_multiple_of(2)).collect();
    if sides.is_empty() {
        sides.push((r.next_u64() % 4) as usize);
    }
    // Sides: 0 at x = 0, 1 at x = w, 2 at y = 0, 3 at y = d.
    let nx = sides.iter().filter(|&&i| i < 2).count() as f64;
    let ny = sides.iter().filter(|&&i| i >= 2).count() as f64;
    // The run each tilt must close, per unit of tan(angle) times height.
    let run = |n: f64, len: f64| if n > 0.0 { len / n } else { f64::INFINITY };
    if closing {
        let meet = h * r.range(0.3, 0.8);
        angle = (run(nx, w).min(run(ny, d)) / meet).patan();
    }
    let tau = angle.ptan();
    let top = h.min(run(nx, w) / tau).min(run(ny, d) / tau);
    let base = axis_box([0.0; 3], [w, d, h]);
    let (s, c) = angle.psin_cos();
    let mut want: Shape = base.clone().into();
    let mut points = Vec::new();
    for &i in &sides {
        let (p, n, y, at) = match i {
            0 => (
                [0.0; 3],
                [c, 0.0, -s],
                [0.0, 1.0, 0.0],
                [0.0, d / 2.0, h / 2.0],
            ),
            1 => (
                [w, 0.0, 0.0],
                [-c, 0.0, -s],
                [0.0, -1.0, 0.0],
                [w, d / 2.0, h / 2.0],
            ),
            2 => (
                [0.0; 3],
                [0.0, c, -s],
                [-1.0, 0.0, 0.0],
                [w / 2.0, 0.0, h / 2.0],
            ),
            _ => (
                [0.0, d, 0.0],
                [0.0, -c, -s],
                [1.0, 0.0, 0.0],
                [w / 2.0, d, h / 2.0],
            ),
        };
        want = Shape::bool(Op::Intersection, want, halfspace(p, n, y));
        points.push(at);
    }
    let z = top;
    let volume =
        w * d * z - (w * ny + d * nx) * tau * z * z / 2.0 + nx * ny * tau * tau * z.ppowi(3) / 3.0;
    let (w1, d1) = ((w - nx * tau * z).max(0.0), (d - ny * tau * z).max(0.0));
    let x_face = d * z - ny * tau * z * z / 2.0;
    let y_face = w * z - nx * tau * z * z / 2.0;
    let mut area = w * d + w1 * d1;
    for i in 0..4 {
        let drafted = sides.contains(&i);
        let face = if i < 2 { x_face } else { y_face };
        area += if drafted { face / c } else { face };
    }
    let feature = Feature {
        act: Act::Draft,
        points,
        size: angle,
        pull: Some(([0.0; 3], [0.0, 0.0, 1.0])),
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    (base, feature, want, exact)
}

/// The region between a right-angled corner at `(x, y)` of a revolution's meridian and the arc of
/// radius `r` tangent to both its sides, toward `(sx, sy)` (each ±1): a fillet's section there.
fn spandrel(x: f64, y: f64, r: f64, sx: f64, sy: f64) -> Profile {
    let c = [x + sx * r, y + sy * r];
    // The arc's ends on the side along y and on the side along x; the short way between them
    // passes the corner. Corner, then the y side's end, runs counter-clockwise when sx·sy < 0.
    let (on_y, on_x) = ([x, c[1]], [c[0], y]);
    let (a_y, a_x) = ((0.0f64).patan2(-sx), (-sy).patan2(0.0));
    let mut turn = a_x - a_y;
    if turn > PI {
        turn -= TAU;
    } else if turn <= -PI {
        turn += TAU;
    }
    let (first, second, start, sweep) = if sx * sy < 0.0 {
        (on_y, on_x, a_y, turn)
    } else {
        (on_x, on_y, a_x, -turn)
    };
    let lp = vec![
        Seg::Line {
            a: [x, y],
            b: first,
        },
        Seg::Arc {
            center: c,
            radius: r,
            start,
            sweep,
        },
        Seg::Line {
            a: second,
            b: [x, y],
        },
    ];
    Profile { loops: vec![lp] }
}

/// The triangle a chamfer of setback `d` cuts or fills at a right-angled corner `(x, y)` of a
/// meridian, toward `(sx, sy)`, counter-clockwise.
fn corner_triangle(x: f64, y: f64, d: f64, sx: f64, sy: f64) -> Profile {
    let (p, q) = ([x + sx * d, y], [x, y + sy * d]);
    let pts = if sx * sy > 0.0 {
        vec![[x, y], p, q]
    } else {
        vec![[x, y], q, p]
    };
    polygon(&pts)
}

/// The rim of a hole through a plate, filleted or chamfered where it meets the top face, which
/// takes material away; or the foot of a boss on a round plate, where the blend fills material in.
/// The section the blend takes or adds is a spandrel of area `(1 − π/4)r²`, its centroid
/// `r(10 − 3π)/(12 − 3π)` out from the corner, or a triangle of `d²/2` a third of the way out; by
/// Pappus the volume is that area times the circle its centroid runs round, and the blend's face
/// is its arc or slant line round its own.
fn hole_rim(r: &mut Rng) -> FeatureDraw {
    let blend = if r.next_u64().is_multiple_of(2) {
        Act::Fillet
    } else {
        Act::Chamfer
    };
    let spandrel_x = |s: f64| s * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI);
    // A blend at a corner of radius `at` from the axis, of size `s`: the area it takes or adds,
    // that area's distance from the axis, the band of the walls it covers and its own face.
    let piece = |at: f64, s: f64, out: f64| match blend {
        Act::Fillet => (
            (1.0 - PI / 4.0) * s * s,
            at + out * spandrel_x(s),
            2.0 * PI * s * ((at + out * s) * PI / 2.0 - out * s),
        ),
        _ => (
            s * s / 2.0,
            at + out * s / 3.0,
            2.0 * PI * (at + out * s / 2.0) * s * 2f64.sqrt(),
        ),
    };
    let frame = Frame::at([0.0; 3]);
    // Off the seam, where a revolution's rim edge meets the seam at a vertex.
    let rim = r.range(0.5, 2.5);
    if r.next_u64().is_multiple_of(2) {
        let (w, d, t) = (r.range(12.0, 20.0), r.range(12.0, 20.0), r.range(3.0, 6.0));
        let rad = r.range(1.5, 3.5);
        let c = [w * r.range(0.4, 0.6), d * r.range(0.4, 0.6)];
        let size = t.min(rad) * r.range(0.15, 0.45);
        let plate = Profile {
            loops: vec![
                rounded(&[[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], &[0.0; 4]),
                circle(c, rad, false),
            ],
        };
        let base = Primitive::Extrude {
            frame,
            profile: plate,
            height: t,
        };
        let (cut, at, face) = piece(rad, size, 1.0);
        let section = match blend {
            Act::Fillet => spandrel(rad, t, size, 1.0, -1.0),
            _ => corner_triangle(rad, t, size, 1.0, -1.0),
        };
        let tool = Primitive::Revolve {
            frame: Frame::at([c[0], c[1], 0.0]),
            profile: section,
            angle: TAU,
        };
        let volume = base.volume() - TAU * at * cut;
        let ring = PI * ((rad + size).ppowi(2) - rad * rad);
        let area = base.area() - ring - TAU * rad * size + face;
        let feature = Feature {
            act: blend,
            points: vec![[c[0] + rad * rim.pcos(), c[1] + rad * rim.psin(), t]],
            size,
            pull: None,
            radii: Vec::new(),
            matrix: None,
        };
        let exact = Exact {
            result_volume: Some(volume),
            result_area: Some(area),
            ..Default::default()
        };
        return (
            base.clone(),
            feature,
            Shape::bool(Op::Difference, base, tool),
            exact,
        );
    }
    let (plate_r, t) = (r.range(8.0, 14.0), r.range(2.0, 5.0));
    let (rad, boss) = (plate_r * r.range(0.2, 0.5), r.range(3.0, 8.0));
    let size = boss.min(plate_r - rad) * r.range(0.15, 0.45);
    let meridian = Profile {
        loops: vec![rounded(
            &[
                [0.0, 0.0],
                [plate_r, 0.0],
                [plate_r, t],
                [rad, t],
                [rad, t + boss],
                [0.0, t + boss],
            ],
            &[0.0; 6],
        )],
    };
    let base = Primitive::Revolve {
        frame,
        profile: meridian,
        angle: TAU,
    };
    let (fill, at, face) = piece(rad, size, 1.0);
    let section = match blend {
        Act::Fillet => spandrel(rad, t, size, 1.0, 1.0),
        _ => corner_triangle(rad, t, size, 1.0, 1.0),
    };
    let tool = Primitive::Revolve {
        frame,
        profile: section,
        angle: TAU,
    };
    let volume = base.volume() + TAU * at * fill;
    let ring = PI * ((rad + size).ppowi(2) - rad * rad);
    let area = base.area() - ring - TAU * rad * size + face;
    let feature = Feature {
        act: blend,
        points: vec![[rad * rim.pcos(), rad * rim.psin(), t]],
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    (
        base.clone(),
        feature,
        Shape::bool(Op::Union, base, tool),
        exact,
    )
}

/// A prism on a rounded rectangle, corners ρ, hollowed to walls t thick and open through its top.
/// The pocket is the rectangle inset by t with corners ρ − t, or sharp where t ≥ ρ, from t up.
fn shelled_round(r: &mut Rng) -> FeatureDraw {
    let (w, d, h) = (r.range(8.0, 14.0), r.range(8.0, 14.0), r.range(4.0, 10.0));
    let t = r.range(0.5, 1.5);
    let rho = (t * r.range(0.3, 3.0)).min(0.45 * w.min(d));
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64, c: f64| Profile {
        loops: vec![rounded(
            &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            &[c.max(0.0); 4],
        )],
    };
    let outer = rect(0.0, 0.0, w, d, rho);
    let inner = rect(t, t, w - t, d - t, rho - t);
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: outer.clone(),
        height: h,
    };
    let pocket = Primitive::Extrude {
        frame: Frame::at([0.0, 0.0, t]),
        profile: inner.clone(),
        height: h,
    };
    let (ao, lo) = (outer.area(), outer.perimeter());
    let (ai, li) = (inner.area(), inner.perimeter());
    let volume = ao * h - ai * (h - t);
    let area = ao + lo * h + (ao - ai) + li * (h - t) + ai;
    let feature = Feature {
        act: Act::Shell,
        points: vec![[w / 2.0, d / 2.0, h]],
        size: t,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        result_volume: Some(volume),
        result_area: Some(area),
        ..Default::default()
    };
    (
        base.clone(),
        feature,
        Shape::bool(Op::Difference, base, pocket),
        exact,
    )
}

/// The rows `[R | t]` of the motion turning by `rot` and then moving by `t`.
fn placement(rot: &[[f64; 3]; 3], t: Vec3) -> brep_protocol::Placement {
    [0, 1, 2].map(|i| [rot[i][0], rot[i][1], rot[i][2], t[i]])
}

fn rotate(rot: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| rot[i][0] * v[0] + rot[i][1] * v[1] + rot[i][2] * v[2])
}

/// The volume two balls of radii `a` and `b` share with their centres `d` apart.
fn lens(a: f64, b: f64, d: f64) -> f64 {
    PI * (a + b - d).ppowi(2) * (d * d + 2.0 * d * (a + b) - 3.0 * (a - b).ppowi(2)) / (12.0 * d)
}

/// An assembly of 6 to 60 slots 30 mm apart, each turned its own way, holding one part or a pair:
/// boxes overlapping, balls overlapping, a shaft pressed into a tube, a ball inside a box, boxes
/// touching face to face, or boxes a hair apart. A few shapes serve every slot, each placed many
/// times. Boxes and balls a hair apart, from 1e-6 to 1e-3 mm, test the gap a kernel can resolve.
/// Only the pairs that share material, or come within the clearance, interfere.
fn assembly(r: &mut Rng) -> Drawn {
    let slots = [6, 20, 60][(r.next_u64() % 3) as usize];
    let clearance = [0.0, 0.01, 0.1][(r.next_u64() % 3) as usize];
    let press = 1.0 + r.range(0.005, 0.05);
    let tube = Primitive::Revolve {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 4.0], [1.0, 4.0]]),
        angle: TAU,
    };
    let shapes = vec![
        axis_box([0.0; 3], [4.0, 3.0, 2.0]),
        axis_box([0.0; 3], [3.0; 3]),
        sphere([0.0; 3], 1.5),
        sphere([0.0; 3], 2.0),
        tube,
        Primitive::Cylinder {
            frame: Frame::at([0.0, 0.0, -1.0]),
            radius: press,
            height: 6.0,
        },
        axis_box([0.0; 3], [6.0; 3]),
        sphere([0.0; 3], 1.0),
    ];
    let mut parts = Vec::new();
    let mut clashes = Vec::new();
    let side = (slots as f64).pcbrt().ceil() as usize;
    for k in 0..slots {
        let c = [
            (k % side) as f64 * 30.0,
            (k / side % side) as f64 * 30.0,
            (k / side / side) as f64 * 30.0,
        ];
        let rot = r.motion().r;
        let mut put = |shape: usize, o: Vec3| {
            let at = rotate(&rot, o);
            parts.push((shape, placement(&rot, [0, 1, 2].map(|i| c[i] + at[i]))));
            parts.len() - 1
        };
        let mut pair = |a: (usize, Vec3), b: (usize, Vec3), volume: f64, gap: f64| {
            let (i, j) = (put(a.0, a.1), put(b.0, b.1));
            if volume > 0.0 || gap < clearance {
                clashes.push(Clash { i, j, volume, gap });
            }
        };
        match r.next_u64() % 9 {
            0 => {
                put(0, [0.0; 3]);
            }
            1 => {
                let o = [r.range(1.0, 3.5), r.range(0.5, 2.5), r.range(-2.5, 1.5)];
                let span = |lo: f64, len_a: f64| (len_a.min(lo + 3.0) - lo.max(0.0)).max(0.0);
                let v = span(o[0], 4.0) * span(o[1], 3.0) * span(o[2], 2.0);
                pair((0, [0.0; 3]), (1, o), v, 0.0);
            }
            2 => {
                let d = r.range(0.8, 3.3);
                pair((2, [0.0; 3]), (3, [d, 0.0, 0.0]), lens(1.5, 2.0, d), 0.0);
            }
            3 => {
                let v = PI * (press * press - 1.0) * 4.0;
                pair((4, [0.0; 3]), (5, [0.0; 3]), v, 0.0);
            }
            4 => {
                pair((6, [0.0; 3]), (7, [3.0; 3]), 4.0 / 3.0 * PI, 0.0);
            }
            5 => pair((1, [0.0; 3]), (1, [3.0, 0.0, 0.0]), 0.0, 0.0),
            6 => {
                let g = 10f64.ppowf(r.range(-6.0, -3.0));
                pair((1, [0.0; 3]), (1, [3.0 + g, 0.0, 0.0]), 0.0, g);
            }
            7 => {
                let g = 10f64.ppowf(r.range(-6.0, -3.0));
                pair((2, [0.0; 3]), (3, [3.5 + g, 0.0, 0.0]), 0.0, g);
            }
            _ => {
                let g = 10f64.ppowf(r.range(-6.0, -3.0));
                pair((1, [0.0; 3]), (2, [-1.5 - g, 1.5, 1.5]), 0.0, g);
            }
        }
    }
    let exact = Exact {
        assembly: Some(Assembly {
            shapes: shapes.clone(),
            parts,
            clearance,
            clashes,
        }),
        ..Default::default()
    };
    (shapes[0].clone().into(), shapes[1].clone().into(), exact)
}

/// A pattern, as one boolean of many: a plate drilled by a grid of 9 to 400 holes, a grid of boxes
/// each touching its neighbours face to face, or a ring of pins each overlapping the next, the
/// last the first.
fn pattern(r: &mut Rng) -> Drawn {
    let size = (r.next_u64() % 3) as usize;
    let many = match r.next_u64() % 3 {
        0 => {
            let (nx, ny) = [(3, 3), (8, 5), (20, 20)][size];
            let s = 2.0;
            let rad = s * r.range(0.2, 0.4);
            let t = r.range(1.0, 3.0);
            let (w, d) = (nx as f64 * s, ny as f64 * s);
            let mut tools = Vec::new();
            for i in 0..nx {
                for j in 0..ny {
                    tools.push(Primitive::Cylinder {
                        frame: Frame::at([(i as f64 + 0.5) * s, (j as f64 + 0.5) * s, -1.0]),
                        radius: rad,
                        height: t + 2.0,
                    });
                }
            }
            let n = (nx * ny) as f64;
            let disc = PI * rad * rad;
            Many {
                target: Some(axis_box([0.0; 3], [w, d, t])),
                tools,
                volume: (w * d - n * disc) * t,
                area: 2.0 * (w * d - n * disc) + 2.0 * (w + d) * t + n * TAU * rad * t,
            }
        }
        1 => {
            let (nx, ny) = [(3, 1), (5, 4), (12, 10)][size];
            let grid = |r: &mut Rng| (r.range(0.5, 3.0) * 2.0).round() / 2.0;
            let (a, b, c) = (grid(r), grid(r), grid(r));
            let mut tools = Vec::new();
            for i in 0..nx {
                for j in 0..ny {
                    let lo = [i as f64 * a, j as f64 * b, 0.0];
                    tools.push(axis_box(lo, [lo[0] + a, lo[1] + b, c]));
                }
            }
            let (w, d) = (nx as f64 * a, ny as f64 * b);
            Many {
                target: None,
                tools,
                volume: w * d * c,
                area: 2.0 * (w * d + d * c + c * w),
            }
        }
        _ => {
            let n = [8, 24, 60][size];
            let ring = n as f64 * 0.5;
            let near = 2.0 * ring * (PI / n as f64).psin();
            let next = ring * (TAU / n as f64).psin();
            let p = near / 2.0 + (next - near / 2.0) * r.range(0.2, 0.8);
            let h = r.range(1.0, 4.0);
            let tools = (0..n)
                .map(|k| {
                    let a = TAU * k as f64 / n as f64;
                    Primitive::Cylinder {
                        frame: Frame::at([ring * a.pcos(), ring * a.psin(), 0.0]),
                        radius: p,
                        height: h,
                    }
                })
                .collect();
            let half = (near / (2.0 * p)).pacos();
            let shared = 2.0 * p * p * half - near / 2.0 * (4.0 * p * p - near * near).sqrt();
            let top = n as f64 * (PI * p * p - shared);
            Many {
                target: None,
                tools,
                volume: top * h,
                area: 2.0 * top + h * n as f64 * p * (TAU - 4.0 * half),
            }
        }
    };
    let a = many.tools[0].clone();
    let exact = Exact {
        many: Some(many),
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// A part with a face on a mirror plane at any angle, or straddling it, joined to its image there:
/// a box, a cylinder or an L-prism. Joined face to face the result is the part doubled, less the
/// two faces that meet; straddling, the box spans the overlap's far sides.
fn mirror_join(r: &mut Rng) -> Drawn {
    let n = r.direction();
    let p = [
        r.range(-10.0, 10.0),
        r.range(-10.0, 10.0),
        r.range(-10.0, 10.0),
    ];
    let dot = p[0] * n[0] + p[1] * n[1] + p[2] * n[2];
    let matrix = [0, 1, 2].map(|i| {
        let mut row = [0.0; 4];
        for j in 0..3 {
            row[j] = f64::from(u8::from(i == j)) - 2.0 * n[i] * n[j];
        }
        row[3] = 2.0 * dot * n[i];
        row
    });
    let on = Frame::along(p, n);
    let (part, volume, area) = match r.next_u64() % 4 {
        0 => {
            let s = [r.range(1.0, 6.0), r.range(1.0, 6.0), r.range(1.0, 6.0)];
            let b = Primitive::Box { frame: on, size: s };
            let (v, a) = (b.volume(), b.area());
            (b, 2.0 * v, 2.0 * a - 2.0 * s[0] * s[1])
        }
        1 => {
            let s = [r.range(1.0, 6.0), r.range(1.0, 6.0), r.range(2.0, 6.0)];
            let e = s[2] * r.range(0.1, 0.4);
            let frame = Frame {
                origin: [0, 1, 2].map(|i| p[i] - e * n[i]),
                ..on
            };
            let len = 2.0 * (s[2] - e);
            (
                Primitive::Box { frame, size: s },
                len * s[0] * s[1],
                2.0 * (s[0] * s[1] + len * (s[0] + s[1])),
            )
        }
        2 => {
            let (rad, h) = (r.range(0.5, 4.0), r.range(1.0, 6.0));
            let c = Primitive::Cylinder {
                frame: on,
                radius: rad,
                height: h,
            };
            let (v, a) = (c.volume(), c.area());
            (c, 2.0 * v, 2.0 * a - 2.0 * PI * rad * rad)
        }
        _ => {
            let (w, d, t) = (r.range(4.0, 8.0), r.range(4.0, 8.0), r.range(1.0, 2.5));
            let profile = polygon(&[[0.0, 0.0], [w, 0.0], [w, t], [t, t], [t, d], [0.0, d]]);
            let base = profile.area();
            let e = Primitive::Extrude {
                frame: on,
                profile,
                height: r.range(1.0, 5.0),
            };
            let (v, a) = (e.volume(), e.area());
            (e, 2.0 * v, 2.0 * a - 2.0 * base)
        }
    };
    let exact = Exact {
        mirror: Some(Mirror {
            matrix,
            volume,
            area,
        }),
        ..Default::default()
    };
    (part.clone().into(), part.into(), exact)
}

/// Faces pushed along their normals: one to three faces of a box, out or in; the inner face of an
/// L-prism's corner, widening or narrowing its leg; or a cylinder's side, changing its radius.
fn offset_face(r: &mut Rng) -> FeatureDraw {
    let feature = |points: Vec<Vec3>, d: f64| Feature {
        act: Act::OffsetFaces,
        points,
        size: d,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    match r.next_u64() % 3 {
        0 => {
            let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
            let least = w.iter().copied().fold(f64::INFINITY, f64::min);
            let d = if r.next_u64().is_multiple_of(2) {
                r.range(0.2, 2.0)
            } else {
                -least * r.range(0.1, 0.4)
            };
            let (mut lo, mut hi) = ([0.0; 3], w);
            let mut points = Vec::new();
            let mut faces: Vec<usize> = (0..6).filter(|_| r.next_u64().is_multiple_of(3)).collect();
            if faces.is_empty() {
                faces.push((r.next_u64() % 6) as usize);
            }
            faces.truncate(3);
            for &f in &faces {
                let (axis, top) = (f / 2, f % 2 == 1);
                let mut at = scale3(w, 0.5);
                at[axis] = if top { w[axis] } else { 0.0 };
                points.push(at);
                if top {
                    hi[axis] += d;
                } else {
                    lo[axis] -= d;
                }
            }
            (
                axis_box([0.0; 3], w),
                feature(points, d),
                axis_box(lo, hi).into(),
                Exact::default(),
            )
        }
        1 => {
            let (w, h, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(1.5, 3.0));
            let d = r.range(-0.4 * t, 0.4 * (w - t));
            let height = r.range(1.0, 5.0);
            let l = |x: f64| polygon(&[[0.0, 0.0], [w, 0.0], [w, t], [x, t], [x, h], [0.0, h]]);
            let at = |p: Profile| Primitive::Extrude {
                frame: Frame::at([0.0; 3]),
                profile: p,
                height,
            };
            let point = [t, (t + h) / 2.0, height / 2.0];
            (
                at(l(t)),
                feature(vec![point], d),
                at(l(t + d)).into(),
                Exact::default(),
            )
        }
        _ => {
            let (rad, h) = (r.range(1.0, 4.0), r.range(1.0, 6.0));
            let d = r.range(-0.4 * rad, 2.0);
            let cyl = |radius: f64| Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius,
                height: h,
            };
            (
                cyl(rad),
                feature(vec![[rad * 1f64.pcos(), rad * 1f64.psin(), h / 2.0]], d),
                cyl(rad + d).into(),
                Exact::default(),
            )
        }
    }
}

/// A feature of `act` on the faces the points lie on.
fn face_act(act: Act, points: Vec<Vec3>, size: f64, matrix: Option<[[f64; 4]; 3]>) -> Feature {
    Feature {
        act,
        points,
        size,
        pull: None,
        radii: Vec::new(),
        matrix,
    }
}

/// The prism of `profile` on the world's xy plane, `h` tall.
fn prism_xy(profile: Profile, h: f64) -> Primitive {
    Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile,
        height: h,
    }
}

/// A `w` × `d` plate with round holes of `r[k]` at `x[k]` along its middle.
fn holed_plate(w: f64, d: f64, h: f64, holes: &[(f64, f64)]) -> Primitive {
    let mut p = polygon(&[[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]]);
    for &(x, r) in holes {
        p.loops.push(circle([x, d / 2.0], r, false));
    }
    prism_xy(p, h)
}

/// A point on the side of the hole of radius `r` about `(x, y)`, halfway up `h`.
fn on_hole(x: f64, y: f64, r: f64, h: f64) -> Vec3 {
    [x + r * 1f64.pcos(), y + r * 1f64.psin(), h / 2.0]
}

/// A `w` × `d` block with a notch `n` deep from `x0` to `x1` in its far side.
fn notched(w: f64, d: f64, h: f64, x0: f64, x1: f64, n: f64) -> Primitive {
    let pts = [
        [0.0, 0.0],
        [w, 0.0],
        [w, d],
        [x1, d],
        [x1, d - n],
        [x0, d - n],
        [x0, d],
        [0.0, d],
    ];
    prism_xy(polygon(&pts), h)
}

/// Faces taken out and their neighbours closed over the gap: some holes of a plate, a notch's
/// three faces, a chamfer on a block's corner, or a groove's three faces round a shaft. Each
/// result is the solid without that feature.
fn delete_face(r: &mut Rng) -> FeatureDraw {
    let del = |points: Vec<Vec3>| face_act(Act::DeleteFaces, points, 0.0, None);
    let (w, d, h) = (r.range(6.0, 12.0), r.range(4.0, 8.0), r.range(1.0, 4.0));
    match r.next_u64() % 4 {
        0 => {
            let n = 1 + (r.next_u64() % 3) as usize;
            let pitch = w / (n + 1) as f64;
            let holes: Vec<(f64, f64)> = (1..=n)
                .map(|k| (k as f64 * pitch, r.range(0.2, 0.4) * pitch.min(d)))
                .collect();
            let gone: Vec<bool> = (0..n)
                .map(|k| k == 0 || r.next_u64().is_multiple_of(2))
                .collect();
            let points = holes
                .iter()
                .zip(&gone)
                .filter(|(_, g)| **g)
                .map(|(&(x, rad), _)| on_hole(x, d / 2.0, rad, h))
                .collect();
            let kept: Vec<(f64, f64)> = holes
                .iter()
                .zip(&gone)
                .filter(|(_, g)| !**g)
                .map(|(x, _)| *x)
                .collect();
            (
                holed_plate(w, d, h, &holes),
                del(points),
                holed_plate(w, d, h, &kept).into(),
                Exact::default(),
            )
        }
        1 => {
            let (x0, x1, n) = (
                w * r.range(0.15, 0.4),
                w * r.range(0.55, 0.85),
                d * r.range(0.2, 0.6),
            );
            let points = vec![
                [x0, d - n / 2.0, h / 2.0],
                [x1, d - n / 2.0, h / 2.0],
                [(x0 + x1) / 2.0, d - n, h / 2.0],
            ];
            (
                notched(w, d, h, x0, x1, n),
                del(points),
                axis_box([0.0; 3], [w, d, h]).into(),
                Exact::default(),
            )
        }
        2 => {
            let c = d.min(w) * r.range(0.15, 0.5);
            let base = prism_xy(
                polygon(&[[0.0, 0.0], [w, 0.0], [w, d - c], [w - c, d], [0.0, d]]),
                h,
            );
            let points = vec![[w - c / 2.0, d - c / 2.0, h / 2.0]];
            (
                base,
                del(points),
                axis_box([0.0; 3], [w, d, h]).into(),
                Exact::default(),
            )
        }
        _ => {
            let (rad, len) = (r.range(1.5, 4.0), r.range(6.0, 12.0));
            let (z0, z1, g) = (
                len * r.range(0.2, 0.45),
                len * r.range(0.55, 0.8),
                rad * r.range(0.2, 0.6),
            );
            let profile = polygon(&[
                [0.0, 0.0],
                [rad, 0.0],
                [rad, z0],
                [rad - g, z0],
                [rad - g, z1],
                [rad, z1],
                [rad, len],
                [0.0, len],
            ]);
            let base = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile,
                angle: TAU,
            };
            let at = |x: f64, z: f64| [x * 1f64.pcos(), x * 1f64.psin(), z];
            let points = vec![
                at(rad - g, (z0 + z1) / 2.0),
                at(rad - g / 2.0, z0),
                at(rad - g / 2.0, z1),
            ];
            let shaft = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rad,
                height: len,
            };
            (base, del(points), shaft.into(), Exact::default())
        }
    }
}

/// Faces moved rigidly: a plate's hole slid across it, a notch slid along its side, a block's top
/// moved up or down and sideways along itself, or a block's side tilted about its bottom edge.
fn move_face(r: &mut Rng) -> FeatureDraw {
    let mv = |points: Vec<Vec3>, m: [[f64; 4]; 3]| face_act(Act::MoveFaces, points, 0.0, Some(m));
    let shift = |t: Vec3| {
        [
            [1.0, 0.0, 0.0, t[0]],
            [0.0, 1.0, 0.0, t[1]],
            [0.0, 0.0, 1.0, t[2]],
        ]
    };
    let (w, d, h) = (r.range(6.0, 12.0), r.range(4.0, 8.0), r.range(1.0, 4.0));
    match r.next_u64() % 4 {
        0 => {
            let rad = r.range(0.5, 0.25 * d);
            let (x, y) = (w * r.range(0.3, 0.7), d / 2.0);
            let (nx, ny) = (
                r.range(rad + 0.5, w - rad - 0.5),
                r.range(rad + 0.3, d - rad - 0.3),
            );
            let plate = |cx: f64, cy: f64| {
                let mut p = polygon(&[[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]]);
                p.loops.push(circle([cx, cy], rad, false));
                prism_xy(p, h)
            };
            let m = shift([nx - x, ny - y, 0.0]);
            (
                plate(x, y),
                mv(vec![on_hole(x, y, rad, h)], m),
                plate(nx, ny).into(),
                Exact::default(),
            )
        }
        1 => {
            let (len, n) = (w * r.range(0.15, 0.3), d * r.range(0.2, 0.6));
            let x0 = w * r.range(0.1, 0.4);
            let to = r.range(0.5, w - len - 0.5);
            let points = vec![
                [x0, d - n / 2.0, h / 2.0],
                [x0 + len, d - n / 2.0, h / 2.0],
                [x0 + len / 2.0, d - n, h / 2.0],
            ];
            let m = shift([to - x0, 0.0, 0.0]);
            let want = notched(w, d, h, to, to + len, n);
            (
                notched(w, d, h, x0, x0 + len, n),
                mv(points, m),
                want.into(),
                Exact::default(),
            )
        }
        2 => {
            let dz = r.range(-0.5 * h, 2.0);
            let m = shift([r.range(-2.0, 2.0), r.range(-2.0, 2.0), dz]);
            let points = vec![[w / 2.0, d / 2.0, h]];
            let want = axis_box([0.0; 3], [w, d, h + dz]);
            (
                axis_box([0.0; 3], [w, d, h]),
                mv(points, m),
                want.into(),
                Exact::default(),
            )
        }
        _ => {
            // The side x = w turned by θ about its bottom edge, the line x = w, z = 0 along y: its
            // top edge lands at x = w + h·tan θ. Profiles in the xz plane, extruded toward −y.
            let theta = r.range(-0.3, 0.3);
            let (c, s) = (theta.pcos(), theta.psin());
            let frame = Frame {
                origin: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            };
            let quad = |top: f64| Primitive::Extrude {
                frame,
                profile: polygon(&[[0.0, 0.0], [w, 0.0], [top, h], [0.0, h]]),
                height: d,
            };
            let m = [
                [c, 0.0, s, w - c * w],
                [0.0, 1.0, 0.0, 0.0],
                [-s, 0.0, c, s * w],
            ];
            let points = vec![[w, -d / 2.0, h / 2.0]];
            (
                quad(w),
                mv(points, m),
                quad(w + h * theta.ptan()).into(),
                Exact::default(),
            )
        }
    }
}

/// A face thickened into a solid of its own: a block's face out or in, a cylinder's side into a
/// tube, a ball's face into a spherical shell, or a ring's face into a hollow ring.
fn thicken_face(r: &mut Rng) -> FeatureDraw {
    let th = |point: Vec3, t: f64| face_act(Act::Thicken, vec![point], t, None);
    match r.next_u64() % 4 {
        0 => {
            let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
            let f = (r.next_u64() % 6) as usize;
            let (axis, top) = (f / 2, f % 2 == 1);
            let t = if r.next_u64().is_multiple_of(2) {
                r.range(0.3, 2.0)
            } else {
                -w[axis] * r.range(0.1, 0.6)
            };
            let mut at = scale3(w, 0.5);
            at[axis] = if top { w[axis] } else { 0.0 };
            let (mut lo, mut hi) = ([0.0; 3], w);
            // Outward from the top face is up its axis, from the bottom face down it.
            let (a, b) = if top {
                (w[axis], w[axis] + t)
            } else {
                (0.0, -t)
            };
            lo[axis] = a.min(b);
            hi[axis] = a.max(b);
            (
                axis_box([0.0; 3], w),
                th(at, t),
                axis_box(lo, hi).into(),
                Exact::default(),
            )
        }
        1 => {
            let (rad, h) = (r.range(1.0, 4.0), r.range(1.0, 6.0));
            let t = if r.next_u64().is_multiple_of(2) {
                r.range(0.2, 2.0)
            } else {
                -rad * r.range(0.1, 0.8)
            };
            let (r0, r1) = (rad.min(rad + t), rad.max(rad + t));
            let mut ring = Profile {
                loops: vec![circle([0.0, 0.0], r1, true)],
            };
            ring.loops.push(circle([0.0, 0.0], r0, false));
            let cyl = Primitive::Cylinder {
                frame: Frame::at([0.0; 3]),
                radius: rad,
                height: h,
            };
            let point = [rad * 1f64.pcos(), rad * 1f64.psin(), h / 2.0];
            (
                cyl,
                th(point, t),
                prism_xy(ring, h).into(),
                Exact::default(),
            )
        }
        2 => {
            let rad = r.range(1.0, 4.0);
            let t = if r.next_u64().is_multiple_of(2) {
                r.range(0.2, 2.0)
            } else {
                -rad * r.range(0.1, 0.8)
            };
            let (r0, r1) = (rad.min(rad + t), rad.max(rad + t));
            let profile = Profile {
                loops: vec![vec![
                    Seg::Arc {
                        center: [0.0, 0.0],
                        radius: r1,
                        start: -PI / 2.0,
                        sweep: PI,
                    },
                    Seg::Line {
                        a: [0.0, r1],
                        b: [0.0, r0],
                    },
                    Seg::Arc {
                        center: [0.0, 0.0],
                        radius: r0,
                        start: PI / 2.0,
                        sweep: -PI,
                    },
                    Seg::Line {
                        a: [0.0, -r0],
                        b: [0.0, -r1],
                    },
                ]],
            };
            let shell = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile,
                angle: TAU,
            };
            let ball = Primitive::Sphere {
                frame: Frame::at([0.0; 3]),
                radius: rad,
            };
            let point = [rad * 0.6, rad * 0.48, rad * 0.64];
            (ball, th(point, t), shell.into(), Exact::default())
        }
        _ => {
            let (major, minor) = (r.range(3.0, 6.0), r.range(0.6, 2.0));
            let t = if r.next_u64().is_multiple_of(2) {
                r.range(0.2, 0.8)
            } else {
                -minor * r.range(0.1, 0.8)
            };
            let (r0, r1) = (minor.min(minor + t), minor.max(minor + t));
            let profile = Profile {
                loops: vec![
                    circle([major, 0.0], r1, true),
                    circle([major, 0.0], r0, false),
                ],
            };
            let hollow = Primitive::Revolve {
                frame: Frame::at([0.0; 3]),
                profile,
                angle: TAU,
            };
            let ring = Primitive::Torus {
                frame: Frame::at([0.0; 3]),
                major,
                minor,
            };
            let point = [
                (major + minor * 0.6) * 1f64.pcos(),
                (major + minor * 0.6) * 1f64.psin(),
                minor * 0.8,
            ];
            (ring, th(point, t), hollow.into(), Exact::default())
        }
    }
}

/// A frame turned at random about `origin`.
fn turned_frame(r: &mut Rng, origin: Vec3) -> Frame {
    let m = Rigid::uniform([r.unit(), r.unit(), r.unit()], [0.0; 3]);
    let col = |j: usize| [m.r[0][j], m.r[1][j], m.r[2][j]];
    Frame {
        origin,
        x: col(0),
        y: col(1),
        z: col(2),
    }
}

fn at_frame(f: &Frame, l: Vec3) -> Vec3 {
    framed(f, l).0
}

/// The least axis-aligned box round a disc of `radius` about `c` square to the unit `axis`: it
/// reaches `radius·√(1 − axisᵢ²)` along each axis.
fn disc_box(c: Vec3, axis: Vec3, radius: f64) -> [Vec3; 2] {
    let reach = axis.map(|a| radius * (1.0 - a * a).max(0.0).sqrt());
    [
        [0, 1, 2].map(|i| c[i] - reach[i]),
        [0, 1, 2].map(|i| c[i] + reach[i]),
    ]
}

fn box_union(a: [Vec3; 2], b: [Vec3; 2]) -> [Vec3; 2] {
    [
        [0, 1, 2].map(|i| a[0][i].min(b[0][i])),
        [0, 1, 2].map(|i| a[1][i].max(b[1][i])),
    ]
}

/// A primitive's least axis-aligned box, in closed form: a box's corners, a sphere's centre less
/// and plus its radius, the discs at a cylinder's or frustum's ends, a ring's centre circle widened
/// by its tube.
pub fn tight_box(p: &Primitive) -> Option<[Vec3; 2]> {
    Some(match p {
        Primitive::Box { frame, size } => {
            let corners = (0..8).map(|n| {
                let l = [0, 1, 2].map(|i| if n >> i & 1 == 1 { size[i] } else { 0.0 });
                at_frame(frame, l)
            });
            corners.fold([[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]], |b, q| {
                box_union(b, [q, q])
            })
        }
        Primitive::Sphere { frame, radius } => [
            frame.origin.map(|x| x - radius),
            frame.origin.map(|x| x + radius),
        ],
        Primitive::Cylinder {
            frame,
            radius,
            height,
        } => box_union(
            disc_box(frame.origin, frame.z, *radius),
            disc_box(at_frame(frame, [0.0, 0.0, *height]), frame.z, *radius),
        ),
        Primitive::Cone {
            frame,
            bottom,
            top,
            height,
        } => box_union(
            disc_box(frame.origin, frame.z, *bottom),
            disc_box(at_frame(frame, [0.0, 0.0, *height]), frame.z, *top),
        ),
        Primitive::Torus {
            frame,
            major,
            minor,
        } => {
            let [lo, hi] = disc_box(frame.origin, frame.z, *major);
            [lo.map(|x| x - minor), hi.map(|x| x + minor)]
        }
        _ => return None,
    })
}

/// A box, cylinder, frustum, ball or ring turned at random, and points all about it, inside and
/// out: the solid's nearest point to each is as far as the closed form says, and lies on the
/// solid. Among them, points on the solid's own axis beyond each end, where a ball has its poles
/// and a cone its apex.
fn nearest_case(r: &mut Rng) -> Drawn {
    let a = placed_primitive(r);
    let f = a.frame();
    let mut local: Vec<Vec3> = (0..24)
        .map(|_| [r.range(-7.0, 7.0), r.range(-7.0, 7.0), r.range(-7.0, 9.0)])
        .collect();
    local.extend([
        [0.0, 0.0, -4.0],
        [0.0, 0.0, 11.0],
        [0.0, 0.0, 1e-3],
        [9.0, 0.0, 0.0],
    ]);
    let closest = local
        .into_iter()
        .map(|l| {
            let q = at_frame(&f, l);
            let d = crate::oracle::solid_distance(&a, q).expect("a primitive with a closed form");
            (q, d, None)
        })
        .collect();
    let exact = Exact {
        closest,
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// A block drilled through, turned at random, and points in the hole: none of the block holds
/// them, and the nearest of it to each is the one point of the hole's wall straight out from the
/// axis. The hole is four times as long as it is wide, and the points lie in its middle half, so
/// neither rim is as near.
fn nearest_in_a_hole(r: &mut Rng) -> Drawn {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let f = turned_frame(r, origin);
    let radius = r.range(0.5, 1.5);
    let (w, h) = (r.range(4.0, 8.0) * radius, r.range(8.0, 12.0) * radius);
    let centre = [w * r.range(0.4, 0.6), w * r.range(0.4, 0.6)];
    let block = Primitive::Box {
        frame: f,
        size: [w, w, h],
    };
    let mut drill = f;
    drill.origin = at_frame(&f, [centre[0], centre[1], -1.0]);
    let hole = Primitive::Cylinder {
        frame: drill,
        radius,
        height: h + 2.0,
    };
    let closest = (0..16)
        .map(|_| {
            let (rho, phi) = (radius * r.range(0.2, 0.9), r.range(0.0, TAU));
            let z = h * r.range(0.25, 0.75);
            let at = |rho: f64| {
                at_frame(
                    &f,
                    [
                        centre[0] + rho * phi.pcos(),
                        centre[1] + rho * phi.psin(),
                        z,
                    ],
                )
            };
            (at(rho), radius - rho, Some(at(radius)))
        })
        .collect();
    let exact = Exact {
        closest,
        ..Default::default()
    };
    let drilled = Shape::bool(Op::Difference, block, hole);
    (drilled.clone(), drilled, exact)
}

/// A box, cylinder, frustum, ball or ring turned at random, and points of its boundary: on its
/// faces, along its edges and rims. Each is on the boundary, and the points a hundredth of a
/// millimetre to either side of a face are inside and outside, where they stand that far clear of
/// every part of the boundary.
fn boundary_case(r: &mut Rng) -> Drawn {
    let a = placed_primitive(r);
    let f = a.frame();
    let step = 0.01;
    let mut sides = Vec::new();
    for (l, normal) in surface_points(r, &a) {
        sides.push((at_frame(&f, l), Side::On));
        let Some(n) = normal else { continue };
        for (sign, side) in [(-1.0, Side::In), (1.0, Side::Out)] {
            let q = at_frame(&f, [0, 1, 2].map(|k| l[k] + sign * step * n[k]));
            // Near an apex or across a thin wall, a step off one face comes close to another.
            let clear = crate::oracle::boundary_distance(&a, q).is_some_and(|d| d > 0.5 * step);
            let held = crate::oracle::solid_distance(&a, q) == Some(0.0);
            if clear && held == (side == Side::In) {
                sides.push((q, side));
            }
        }
    }
    let exact = Exact {
        sides,
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// A box, cylinder, frustum, ball or ring turned at random, alone or joined to a second: the box
/// of a union is the box round both boxes.
fn bounding_case(r: &mut Rng) -> Drawn {
    let one = placed_primitive;
    let a = one(r);
    let want = tight_box(&a).expect("a primitive with a closed-form box");
    if r.next_u64().is_multiple_of(2) {
        let exact = Exact {
            bbox: Some(want),
            ..Default::default()
        };
        return (a.clone().into(), a.into(), exact);
    }
    let b = one(r);
    let want = box_union(
        want,
        tight_box(&b).expect("a primitive with a closed-form box"),
    );
    let exact = Exact {
        bbox: Some(want),
        ..Default::default()
    };
    let joined = Shape::bool(Op::Union, a, b);
    (joined.clone(), joined, exact)
}

/// A box, cylinder, frustum, ball or ring turned at random, as both operands of each boolean.
fn itself_case(r: &mut Rng) -> Drawn {
    let a = any_primitive(r);
    let exact = Exact {
        itself: true,
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// A box, cylinder, frustum, ball or ring, placed and turned at random.
fn placed_primitive(r: &mut Rng) -> Primitive {
    {
        let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
        let frame = turned_frame(r, origin);
        match r.next_u64() % 5 {
            0 => Primitive::Box {
                frame,
                size: [r.range(1.0, 6.0), r.range(1.0, 6.0), r.range(1.0, 6.0)],
            },
            1 => Primitive::Cylinder {
                frame,
                radius: r.range(0.5, 3.0),
                height: r.range(1.0, 6.0),
            },
            2 => Primitive::Cone {
                frame,
                bottom: r.range(0.5, 3.0),
                top: r.range(0.0, 2.0),
                height: r.range(1.0, 6.0),
            },
            3 => Primitive::Sphere {
                frame,
                radius: r.range(0.5, 3.0),
            },
            _ => {
                let major = r.range(2.0, 4.0);
                Primitive::Torus {
                    frame,
                    major,
                    minor: major * r.range(0.15, 0.5),
                }
            }
        }
    }
}

/// A solid of any kind cut by a plane through the middle of its box, at a random slant.
fn split_case(r: &mut Rng) -> Drawn {
    let a = any_primitive(r);
    let (lo, hi) = crate::oracle::bounds(&Shape::Prim(a.clone())).expect("a primitive's bounds");
    let origin = [0, 1, 2].map(|i| lo[i] + (hi[i] - lo[i]) * r.range(0.2, 0.8));
    let exact = Exact {
        split: Some((origin, r.direction())),
        ..Default::default()
    };
    (a.clone().into(), a.into(), exact)
}

/// The length of an ellipse of semi-axes `a` and `b`: the trapezoid rule on a periodic integrand
/// converges faster than any power, so 4096 steps leave only rounding.
fn ellipse_length(a: f64, b: f64) -> f64 {
    let n = 4096;
    (0..n)
        .map(|k| {
            let t = TAU * k as f64 / n as f64;
            (a * a * t.psin().ppowi(2) + b * b * t.pcos().ppowi(2)).sqrt()
        })
        .sum::<f64>()
        * TAU
        / n as f64
}

/// A plane section with a closed form: a ball cut off its centre (a circle), a cylinder cut slant
/// across its side alone (an ellipse), a box cut anywhere through it (a convex polygon), or a ring
/// cut level with or parallel to its centre plane, or through its axis (two circles).
fn section_case(r: &mut Rng) -> Drawn {
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let frame = turned_frame(r, origin);
    let (p, at, nl, loops, length, rho) = match r.next_u64() % 4 {
        0 => {
            let radius = r.range(1.0, 4.0);
            let d = radius * r.range(0.0, 0.9);
            let nl = r.direction();
            let rho = (radius * radius - d * d).sqrt();
            (
                Primitive::Sphere { frame, radius },
                scale3(nl, d),
                nl,
                1,
                TAU * rho,
                rho,
            )
        }
        1 => {
            let radius = r.range(0.5, 3.0);
            let phi: f64 = r.range(0.0, 1.0);
            let half = radius * phi.ptan() + r.range(1.0, 3.0);
            let turn = r.range(0.0, TAU);
            let nl = [
                phi.psin() * turn.pcos(),
                phi.psin() * turn.psin(),
                phi.pcos(),
            ];
            let (a, b) = (radius / phi.pcos(), radius);
            let cyl = Primitive::Cylinder {
                frame,
                radius,
                height: 2.0 * half,
            };
            (
                cyl,
                [0.0, 0.0, half],
                nl,
                1,
                ellipse_length(a, b),
                b * b / a,
            )
        }
        2 => {
            let size = [r.range(1.0, 6.0), r.range(1.0, 6.0), r.range(1.0, 6.0)];
            let at = [0, 1, 2].map(|i| size[i] * r.range(0.2, 0.8));
            let nl = r.direction();
            let corner = |n: usize| [0, 1, 2].map(|i| if n >> i & 1 == 1 { size[i] } else { 0.0 });
            let mut pts: Vec<Vec3> = Vec::new();
            for n in 0..8 {
                for i in 0..3 {
                    if n >> i & 1 == 1 {
                        continue;
                    }
                    let (p0, p1) = (corner(n), corner(n | 1 << i));
                    let side = |q: Vec3| (0..3).map(|k| (q[k] - at[k]) * nl[k]).sum::<f64>();
                    let (s0, s1) = (side(p0), side(p1));
                    if s0 * s1 < 0.0 {
                        let t = s0 / (s0 - s1);
                        pts.push([0, 1, 2].map(|k| p0[k] + t * (p1[k] - p0[k])));
                    }
                }
            }
            let c = [0, 1, 2].map(|k| pts.iter().map(|q| q[k]).sum::<f64>() / pts.len() as f64);
            // Sorted by angle about their centre in the plane.
            let v = unit3([pts[0][0] - c[0], pts[0][1] - c[1], pts[0][2] - c[2]]);
            let u = unit3(cross3(nl, v));
            let dot = |a: Vec3, b: Vec3| (0..3).map(|k| a[k] * b[k]).sum::<f64>();
            let angle = |q: &Vec3| {
                let d = [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
                dot(d, u).patan2(dot(d, v))
            };
            pts.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
            let len = (0..pts.len())
                .map(|k| {
                    let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                    ((a[0] - b[0]).ppowi(2) + (a[1] - b[1]).ppowi(2) + (a[2] - b[2]).ppowi(2))
                        .sqrt()
                })
                .sum();
            (
                Primitive::Box { frame, size },
                at,
                nl,
                1,
                len,
                f64::INFINITY,
            )
        }
        _ => {
            let major = r.range(2.0, 4.0);
            let minor = major * r.range(0.15, 0.5);
            let ring = Primitive::Torus {
                frame,
                major,
                minor,
            };
            if r.next_u64().is_multiple_of(2) {
                // Through the axis: two circles of the tube's radius.
                let turn = r.range(0.0, TAU);
                (
                    ring,
                    [0.0; 3],
                    [turn.pcos(), turn.psin(), 0.0],
                    2,
                    2.0 * TAU * minor,
                    minor,
                )
            } else {
                // Parallel to the centre plane at height z: circles of major ± √(minor² − z²).
                let z = minor * r.range(-0.8, 0.8);
                let w = (minor * minor - z * z).sqrt();
                (
                    ring,
                    [0.0, 0.0, z],
                    [0.0, 0.0, 1.0],
                    2,
                    2.0 * TAU * major,
                    major - w,
                )
            }
        }
    };
    let f = p.frame();
    let (origin, normal) = (framed(&f, at).0, framed(&f, nl).1);
    let exact = Exact {
        section: Some(Section {
            origin,
            normal,
            loops,
            length,
            rho,
        }),
        ..Default::default()
    };
    (p.clone().into(), p.into(), exact)
}

fn cross3(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A prism on a straight-sided profile, written as loose planar faces: each face its own, shifted
/// along its normal by up to `GAP`, and one in four turned inside out, to be sewn back to the prism.
fn sew_case(r: &mut Rng) -> Drawn {
    let n = 3 + (r.next_u64() % 4) as usize;
    let rad = r.range(2.0, 5.0);
    let pts: Vec<[f64; 2]> = (0..n)
        .map(|k| {
            let t = TAU * (k as f64 + r.range(-0.3, 0.3)) / n as f64;
            let s = rad * r.range(0.7, 1.0);
            [s * t.pcos(), s * t.psin()]
        })
        .collect();
    let origin = [r.range(-5.0, 5.0), r.range(-5.0, 5.0), r.range(-5.0, 5.0)];
    let p = Primitive::Extrude {
        frame: turned_frame(r, origin),
        profile: polygon(&pts),
        height: r.range(1.0, 6.0),
    };
    let faces = n + 2;
    let exact = Exact {
        sew: Some(Sew {
            gaps: (0..faces).map(|_| SEW_GAP * r.range(-1.0, 1.0)).collect(),
            flipped: (0..faces).map(|_| r.next_u64().is_multiple_of(4)).collect(),
        }),
        ..Default::default()
    };
    (p.clone().into(), p.into(), exact)
}

/// Faces pushed until they meet another: a step's lower top raised level with the upper, or the
/// upper lowered to the lower, so the two tops merge and the riser between them is gone; or one
/// wall of a slot pushed across to the other, closing it. Each result is a box.
fn closing_offset(r: &mut Rng) -> FeatureDraw {
    let (w, depth) = (r.range(6.0, 12.0), r.range(3.0, 8.0));
    // Profiles drawn in the xz plane, extruded toward −y.
    let frame = Frame {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [0.0, -1.0, 0.0],
    };
    let prism = |pts: &[[f64; 2]]| Primitive::Extrude {
        frame,
        profile: polygon(pts),
        height: depth,
    };
    let feature = |point: Vec3, d: f64| Feature {
        act: Act::OffsetFaces,
        points: vec![point],
        size: d,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let block = |h: f64| axis_box([0.0, -depth, 0.0], [w, 0.0, h]);
    let mid = -depth / 2.0;
    match r.next_u64() % 3 {
        0 | 1 => {
            let (low, high) = (r.range(1.0, 4.0), r.range(5.0, 9.0));
            let s = w * r.range(0.3, 0.7);
            let base = prism(&[
                [0.0, 0.0],
                [w, 0.0],
                [w, low],
                [s, low],
                [s, high],
                [0.0, high],
            ]);
            if r.next_u64().is_multiple_of(2) {
                let at = [(s + w) / 2.0, mid, low];
                (
                    base,
                    feature(at, high - low),
                    block(high).into(),
                    Exact::default(),
                )
            } else {
                let at = [s / 2.0, mid, high];
                (
                    base,
                    feature(at, low - high),
                    block(low).into(),
                    Exact::default(),
                )
            }
        }
        _ => {
            let h = r.range(4.0, 9.0);
            let (a, b) = (w * r.range(0.2, 0.4), w * r.range(0.6, 0.8));
            let floor = h * r.range(0.2, 0.6);
            let base = prism(&[
                [0.0, 0.0],
                [w, 0.0],
                [w, h],
                [b, h],
                [b, floor],
                [a, floor],
                [a, h],
                [0.0, h],
            ]);
            let at = [a, mid, (floor + h) / 2.0];
            (base, feature(at, b - a), block(h).into(), Exact::default())
        }
    }
}

/// The point and the direction `l` given in the frame, in world coordinates.
fn framed(f: &Frame, l: Vec3) -> (Vec3, Vec3) {
    let d = [0, 1, 2].map(|k| f.x[k] * l[0] + f.y[k] * l[1] + f.z[k] * l[2]);
    ([0, 1, 2].map(|k| f.origin[k] + d[k]), d)
}

fn unit3(v: Vec3) -> Vec3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / n)
}

/// Points of a primitive's boundary in its frame's coordinates, with their outward normals: faces
/// inside, and creases (a box's edges, a cylinder's rims) with none.
fn surface_points(r: &mut Rng, p: &Primitive) -> Vec<(Vec3, Option<Vec3>)> {
    let mut out = Vec::new();
    let mut ring = |r: &mut Rng, n: usize, f: &mut dyn FnMut(&mut Rng) -> (Vec3, Option<Vec3>)| {
        for _ in 0..n {
            out.push(f(r));
        }
    };
    match *p {
        Primitive::Box { size: s, .. } => {
            ring(r, 12, &mut |r| {
                let axis = (r.next_u64() % 3) as usize;
                let top = r.next_u64().is_multiple_of(2);
                let mut q = [0, 1, 2].map(|k| s[k] * r.range(0.1, 0.9));
                q[axis] = if top { s[axis] } else { 0.0 };
                let mut n = [0.0; 3];
                n[axis] = if top { 1.0 } else { -1.0 };
                (q, Some(n))
            });
            ring(r, 4, &mut |r| {
                let axis = (r.next_u64() % 3) as usize;
                let mut q = [0, 1, 2].map(|k| {
                    if r.next_u64().is_multiple_of(2) {
                        0.0
                    } else {
                        s[k]
                    }
                });
                q[axis] = s[axis] * r.range(0.1, 0.9);
                (q, None)
            });
        }
        Primitive::Cylinder { radius, height, .. } => {
            ring(r, 8, &mut |r| {
                let a = r.range(0.0, TAU);
                let z = height * r.range(0.1, 0.9);
                (
                    [radius * a.pcos(), radius * a.psin(), z],
                    Some([a.pcos(), a.psin(), 0.0]),
                )
            });
            ring(r, 4, &mut |r| {
                let (a, rho) = (r.range(0.0, TAU), radius * r.range(0.0, 0.9));
                let top = r.next_u64().is_multiple_of(2);
                let z = if top { height } else { 0.0 };
                let n = [0.0, 0.0, if top { 1.0 } else { -1.0 }];
                ([rho * a.pcos(), rho * a.psin(), z], Some(n))
            });
            ring(r, 3, &mut |r| {
                let a = r.range(0.0, TAU);
                let z = if r.next_u64().is_multiple_of(2) {
                    height
                } else {
                    0.0
                };
                ([radius * a.pcos(), radius * a.psin(), z], None)
            });
        }
        Primitive::Sphere { radius, .. } => {
            ring(r, 12, &mut |r| {
                let d = r.direction();
                (d.map(|x| x * radius), Some(d))
            });
        }
        Primitive::Torus { major, minor, .. } => {
            ring(r, 12, &mut |r| {
                let (a, b) = (r.range(0.0, TAU), r.range(0.0, TAU));
                let n = [b.pcos() * a.pcos(), b.pcos() * a.psin(), b.psin()];
                let q = [
                    (major + minor * b.pcos()) * a.pcos(),
                    (major + minor * b.pcos()) * a.psin(),
                    minor * b.psin(),
                ];
                (q, Some(n))
            });
        }
        Primitive::Cone {
            bottom,
            top,
            height,
            ..
        } => {
            ring(r, 10, &mut |r| {
                let (a, t) = (r.range(0.0, TAU), r.range(0.1, 0.9));
                let rho = bottom + (top - bottom) * t;
                let n = unit3([a.pcos(), a.psin(), (bottom - top) / height]);
                ([rho * a.pcos(), rho * a.psin(), t * height], Some(n))
            });
            ring(r, 2, &mut |r| {
                let a = r.range(0.0, TAU);
                ([bottom * a.pcos(), bottom * a.psin(), 0.0], None)
            });
        }
        _ => {}
    }
    out
}

/// Normals at points of a primitive's boundary at any pose, or of a box with a ball added or cut
/// away at one corner: on the ball's face the normal is the ball's, inward where it was cut.
/// Points where a face meets another at an angle have none.
fn normals_case(r: &mut Rng) -> Drawn {
    let frame = Frame::along(
        [
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
        ],
        r.direction(),
    );
    let kind = r.next_u64() % 7;
    if kind < 5 {
        let p = match kind {
            0 => Primitive::Box {
                frame,
                size: [r.range(1.0, 8.0), r.range(1.0, 8.0), r.range(1.0, 8.0)],
            },
            1 => Primitive::Cylinder {
                frame,
                radius: r.range(0.5, 4.0),
                height: r.range(1.0, 8.0),
            },
            2 => Primitive::Sphere {
                frame,
                radius: r.range(0.5, 5.0),
            },
            3 => {
                let minor = r.range(0.3, 1.5);
                Primitive::Torus {
                    frame,
                    major: minor + r.range(0.5, 4.0),
                    minor,
                }
            }
            _ => Primitive::Cone {
                frame,
                bottom: r.range(1.0, 4.0),
                top: r.range(0.5, 4.0),
                height: r.range(1.0, 8.0),
            },
        };
        let points = surface_points(r, &p)
            .into_iter()
            .map(|(q, n)| (framed(&frame, q).0, n.map(|n| framed(&frame, n).1)))
            .collect();
        let exact = Exact {
            normals: Some(Normals { op: None, points }),
            ..Default::default()
        };
        return (p.clone().into(), p.into(), exact);
    }
    let w = r.range(4.0, 8.0);
    let block = axis_box([0.0; 3], [w; 3]);
    let rad = w * r.range(0.3, 0.6);
    let c = [w, w, w];
    let ball = sphere(c, rad);
    let op = if kind == 5 { Op::Union } else { Op::Difference };
    let shape = Shape::bool(op, block.clone(), ball.clone());
    let mut points = Vec::new();
    for (prim, sign) in [
        (&block, 1.0),
        (&ball, if op == Op::Union { 1.0 } else { -1.0 }),
    ] {
        for (q, n) in surface_points(r, prim) {
            let (Some(n), q) = (n, q) else { continue };
            let q = if matches!(prim, Primitive::Sphere { .. }) {
                [0, 1, 2].map(|k| q[k] + c[k])
            } else {
                q
            };
            let n = n.map(|x| x * sign);
            // Keep a point only where this face alone bounds the result about it.
            let step = |s: f64| [0, 1, 2].map(|k| q[k] + s * n[k]);
            let probe = |dq: Vec3| crate::oracle::contains(&shape, dq);
            let mut clear = !probe(step(1e-3)) && probe(step(-1e-3));
            for t in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                for s in [-2e-2, 2e-2] {
                    let side = [0, 1, 2].map(|k| q[k] + s * t[k]);
                    let off = [0, 1, 2].map(|k| side[k] - 1e-3 * n[k]);
                    let on = [0, 1, 2].map(|k| side[k] + 1e-3 * n[k]);
                    let tangentish = (t[0] * n[0] + t[1] * n[1] + t[2] * n[2]).abs() < 0.9;
                    if tangentish && (!probe(off) || probe(on)) {
                        clear = false;
                    }
                }
            }
            if clear {
                points.push((q, Some(n)));
            }
        }
    }
    let exact = Exact {
        normals: Some(Normals {
            op: Some(op),
            points,
        }),
        ..Default::default()
    };
    (block.into(), ball.into(), exact)
}

/// One vertical edge of a prism on a rectangle, an L or a convex polygon chamfered unevenly: `d1`
/// along the face before the corner, `d2` along the face after. The result is the prism on the
/// profile with that corner cut by the line through the two points.
fn uneven_chamfer(r: &mut Rng) -> FeatureDraw {
    let pts = convex_polygon(r);
    let n = pts.len();
    let i = (r.next_u64() % n as u64) as usize;
    let (prev, at, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
    let len = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]).phypot(b[1] - a[1]);
    let (la, lb) = (len(prev, at), len(at, next));
    let d1 = la * r.range(0.1, 0.45);
    let mut d2 = lb * r.range(0.1, 0.45);
    if (d2 - d1).abs() < 0.05 * d1 {
        d2 *= 0.7;
    }
    let toward = |a: [f64; 2], b: [f64; 2], d: f64, l: f64| {
        [a[0] + (b[0] - a[0]) * d / l, a[1] + (b[1] - a[1]) * d / l]
    };
    let mut cut = Vec::new();
    for (k, &p) in pts.iter().enumerate() {
        if k == i {
            cut.push(toward(at, prev, d1, la));
            cut.push(toward(at, next, d2, lb));
        } else {
            cut.push(p);
        }
    }
    let h = r.range(2.0, 8.0);
    let prism = |profile: Profile| Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile,
        height: h,
    };
    let face = [(prev[0] + at[0]) / 2.0, (prev[1] + at[1]) / 2.0, h / 2.0];
    let feature = Feature {
        act: Act::ChamferTwo,
        points: vec![[at[0], at[1], h * r.range(0.3, 0.7)], face],
        size: d1,
        pull: None,
        radii: vec![d2],
        matrix: None,
    };
    (
        prism(polygon(&pts)),
        feature,
        prism(polygon(&cut)).into(),
        Exact::default(),
    )
}

/// One vertical edge of a box rounded by a ball growing or shrinking evenly along it. The ball
/// touches each face where its centre's foot falls, `r(t)` from the edge, and the blend must be
/// tangent there; the volume lies between the edge filleted at the smaller radius and the larger,
/// `(1 − π/4)·r²` taken from each unit of its length.
fn variable_fillet(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(6.0, 12.0), r.range(6.0, 12.0), r.range(4.0, 10.0)];
    let least = w[0].min(w[1]);
    let (r0, r1) = (least * r.range(0.05, 0.2), least * r.range(0.2, 0.4));
    let (r_from, r_to) = if r.next_u64().is_multiple_of(2) {
        (r0, r1)
    } else {
        (r1, r0)
    };
    // The edge at x = w0, y = w1, from z = 0 (where the radius is r_from) up.
    let edge = [w[0], w[1], w[2] * r.range(0.3, 0.7)];
    let from = [w[0], w[1], 0.0];
    let full = w[0] * w[1] * w[2];
    let k = 1.0 - PI / 4.0;
    let at = |rad: f64| full - k * rad * rad * w[2];
    let (lo, hi) = (r_from.min(r_to), r_from.max(r_to));
    let mut contacts = Vec::new();
    for i in 1..7 {
        let t = i as f64 / 7.0;
        let rad = r_from + (r_to - r_from) * t;
        contacts.push(([w[0], w[1] - rad, t * w[2]], [1.0, 0.0, 0.0]));
        contacts.push(([w[0] - rad, w[1], t * w[2]], [0.0, 1.0, 0.0]));
    }
    let feature = Feature {
        act: Act::FilletVariable,
        points: vec![edge, from],
        size: r_from,
        pull: None,
        radii: vec![r_to],
        matrix: None,
    };
    // Across the edge from outside it, clear of the block's top and bottom and of its far edges.
    // The edge's two faces are alike there, so the result is its own mirror in the plane between
    // them.
    let s = 0.5f64.sqrt();
    let scan = Scan {
        from: [w[0] + 1.0, w[1] + 1.0, w[2] / 2.0],
        along: [-s, -s, 0.0],
        across: [[0.0, 0.0, 1.0], [s, -s, 0.0]],
        half: (0.4 * w[2]).min(0.5 * least),
        size: lo,
        curvature: false,
        same: vec![upright_mirror([w[0], w[1]], [s, s])],
    };
    let exact = Exact {
        sound: Some(Sound {
            zone: None,
            outside: 0.0,
            least: at(hi),
            most: at(lo),
            betti: [1, 0, 0],
            contacts,
            limits: Some(Limits::Growing {
                block: w,
                from: r_from,
                to: r_to,
            }),
            scans: vec![scan],
            ..Default::default()
        }),
        ..Default::default()
    };
    let base = axis_box([0.0; 3], w);
    (base.clone(), feature, base.into(), exact)
}

/// A smooth loft through three to five circles of differing radii, each shifted a little off the
/// axis, at any pose. How it runs between them is the kernel's; through them it must pass.
fn smooth_loft(r: &mut Rng) -> Drawn {
    let frame = Frame::along(
        [
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
        ],
        r.direction(),
    );
    let n = 3 + (r.next_u64() % 3) as usize;
    let mut heights = Vec::new();
    let mut h = 0.0;
    let mut profiles = Vec::new();
    let mut outline = Vec::new();
    let mut least = f64::INFINITY;
    for k in 0..n {
        let rad = r.range(1.0, 4.0);
        least = least.min(rad);
        let c = [r.range(-0.5, 0.5), r.range(-0.5, 0.5)];
        profiles.push(Profile {
            loops: vec![circle(c, rad, true)],
        });
        heights.push(h);
        if k > 0 && k + 1 < n {
            for i in 0..8 {
                let a = 0.4 + TAU * i as f64 / 8.0;
                let l = [c[0] + rad * a.pcos(), c[1] + rad * a.psin(), h];
                outline.push([0, 1, 2].map(|j| {
                    frame.origin[j] + frame.x[j] * l[0] + frame.y[j] * l[1] + frame.z[j] * l[2]
                }));
            }
        }
        h += r.range(2.0, 5.0);
    }
    let stand_in = Primitive::Extrude {
        frame,
        profile: profiles[0].clone(),
        height: 1.0,
    };
    // Across the side from four ways round, clear of both ends and of the side's outline.
    let top = heights[n - 1];
    let world = |l: Vec3| {
        [0, 1, 2]
            .map(|j| frame.origin[j] + frame.x[j] * l[0] + frame.y[j] * l[1] + frame.z[j] * l[2])
    };
    let turned = |l: Vec3| [0, 1, 2].map(|j| frame.x[j] * l[0] + frame.y[j] * l[1]);
    let scans = (0..4)
        .map(|q| {
            let a = 0.3 + TAU * q as f64 / 4.0;
            let out = [a.pcos(), a.psin(), 0.0];
            Scan {
                from: world([6.0 * out[0], 6.0 * out[1], top / 2.0]),
                along: turned([-out[0], -out[1], 0.0]),
                across: [frame.z, turned([-out[1], out[0], 0.0])],
                half: (0.4 * top).min(0.5 * least),
                size: least,
                curvature: false,
                same: Vec::new(),
            }
        })
        .collect();
    let exact = Exact {
        loft: Some(LoftThrough {
            frame,
            profiles,
            heights,
            outline,
        }),
        scans,
        ..Default::default()
    };
    (stand_in.clone().into(), stand_in.into(), exact)
}

/// A smooth loft through three or four sections that are one rounded rectangle, each loop cut
/// to start on a different side or corner. A section is its region wherever its loop starts, so
/// the loft is the rectangle's prism: a kernel that joins the loops start to start twists it.
fn equal_sections_loft(r: &mut Rng) -> Drawn {
    let frame = Frame::along(
        [
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
            r.range(-10.0, 10.0),
        ],
        r.direction(),
    );
    let (w, d) = (r.range(3.0, 8.0), r.range(3.0, 8.0));
    let rad = 0.5 * w.min(d) * r.range(0.2, 0.8);
    let corners = [
        [-w / 2.0, -d / 2.0],
        [w / 2.0, -d / 2.0],
        [w / 2.0, d / 2.0],
        [-w / 2.0, d / 2.0],
    ];
    let outer = rounded(&corners, &[rad; 4]);
    let n = 3 + (r.next_u64() % 2) as usize;
    // Each section starts a different number of segments round the loop from the one below.
    let step = 1 + (r.next_u64() % (outer.len() as u64 - 1)) as usize;
    let mut heights = Vec::new();
    let mut profiles = Vec::new();
    let mut h = 0.0;
    for k in 0..n {
        let mut segs = outer.clone();
        segs.rotate_left(k * step % outer.len());
        profiles.push(Profile { loops: vec![segs] });
        heights.push(h);
        if k + 1 < n {
            h += r.range(2.0, 5.0);
        }
    }
    let prism = Primitive::Extrude {
        frame,
        profile: Profile { loops: vec![outer] },
        height: h,
    };
    let exact = Exact {
        loft: Some(LoftThrough {
            frame,
            profiles,
            heights,
            outline: Vec::new(),
        }),
        loft_is_b: true,
        ..Default::default()
    };
    (prism.clone().into(), prism.into(), exact)
}

/// An L-prism offset outward by d: the points within d of it, the L swelled. As for
/// [`opened_l_prism`], it is its two arm boxes each swelled, less their overlap: the corner column
/// swelled, with a square of ρ² less a quarter disk in each slice facing the concave edge, which
/// stays sharp. `V = V_A + V_B − V_C − (1 − π/4)(hd² + 4d³/3)`, and the area the volume's
/// derivative in d.
fn offset_l_prism(r: &mut Rng) -> FeatureDraw {
    let (w, depth, t) = (r.range(5.0, 10.0), r.range(5.0, 10.0), r.range(2.0, 3.5));
    let h = r.range(2.0, 8.0);
    let d = r.range(0.2, 2.0);
    let pts = vec![
        [0.0, 0.0],
        [w, 0.0],
        [w, t],
        [t, t],
        [t, depth],
        [0.0, depth],
    ];
    let base = Primitive::Extrude {
        frame: Frame::at([0.0; 3]),
        profile: polygon(&pts),
        height: h,
    };
    let (arm_a, va, sa) = rounded_block([0.0; 3], [w, t, h], d);
    let (arm_b, vb, sb) = rounded_block([0.0; 3], [t, depth, h], d);
    let (_, vc, sc) = rounded_block([0.0; 3], [t, t, h], d);
    let k = 1.0 - PI / 4.0;
    let feature = Feature {
        act: Act::Offset,
        points: vec![],
        size: d,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        result_volume: Some(va + vb - vc - k * (h * d * d + 4.0 / 3.0 * d.ppowi(3))),
        result_area: Some(sa + sb - sc - k * (2.0 * h * d + 4.0 * d * d)),
        ..Default::default()
    };
    (base, feature, Shape::bool(Op::Union, arm_a, arm_b), exact)
}

/// A constructor call whose shape the contract does not allow: a profile that crosses itself or
/// has a hole outside it, a revolution across its axis, a screw whose turns collide, a torus whose
/// tube crosses its axis. Each fault is in the shape the kernel receives, however an adapter builds
/// it; a bad number in the suite's own terms, a negative height or radius, never reaches a kernel.
fn invalid_construction(r: &mut Rng) -> Drawn {
    let f = Frame::at([0.0; 3]);
    let sq = |lo: [f64; 2], hi: [f64; 2]| Profile {
        loops: vec![rounded(
            &[lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]],
            &[0.0; 4],
        )],
    };
    let s = r.range(2.0, 6.0);
    let a = match r.next_u64() % 5 {
        0 => {
            let bow = [[0.0, 0.0], [s, 0.0], [0.0, s], [s, s]];
            Primitive::Extrude {
                frame: f,
                profile: Profile {
                    loops: vec![rounded(&bow, &[0.0; 4])],
                },
                height: s,
            }
        }
        1 => Primitive::Revolve {
            frame: f,
            profile: sq([-1.0, 0.0], [s, s]),
            angle: TAU,
        },
        2 => {
            let h = r.range(1.0, 3.0);
            Primitive::Screw {
                frame: f,
                profile: sq([s, 0.0], [s + 1.0, h]),
                rise: h / TAU * r.range(0.2, 0.8),
                angle: 2.0 * TAU,
            }
        }
        3 => Primitive::Torus {
            frame: f,
            major: s / 2.0,
            minor: s,
        },
        _ => {
            let mut p = sq([0.0; 2], [s, s]);
            p.loops.push(circle([3.0 * s, 3.0 * s], 1.0, false));
            Primitive::Extrude {
                frame: f,
                profile: p,
                height: s,
            }
        }
    };
    (
        a.into(),
        axis_box([0.0; 3], [1.0; 3]).into(),
        Exact::default(),
    )
}

/// An act the solid does not allow: a fillet or chamfer too big for the faces it must fit on, a
/// shell as thick as the part.
fn invalid_act(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
    let base = axis_box([0.0; 3], w);
    // The edge runs along z, between faces w[0] and w[1] wide: a blend must fit both. A shell open
    // at the top must leave a hollow across both widths and above the floor.
    let edge = vec![[w[0], w[1], w[2] / 2.0]];
    let faces = w[0].min(w[1]);
    let (act, points, size) = match r.next_u64() % 3 {
        0 => (Act::Fillet, edge, faces * r.range(1.1, 2.0)),
        1 => (Act::Chamfer, edge, faces * r.range(1.1, 2.0)),
        _ => (
            Act::Shell,
            vec![[w[0] / 2.0, w[1] / 2.0, w[2]]],
            (0.5 * faces).min(w[2]) * r.range(1.1, 2.0),
        ),
    };
    let feature = Feature {
        act,
        points,
        size,
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    (base.clone(), feature, base.into(), Exact::default())
}

/// An act of the newer functions the solid does not allow: a two-distance chamfer deeper than its
/// face, a variable fillet whose ball cannot fit, one radius or distance of several too big, a face
/// pushed in through the whole part, or a step's lower top pushed up past the upper.
fn invalid_new_act(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
    let small = w.iter().copied().fold(f64::INFINITY, f64::min);
    let base = axis_box([0.0; 3], w);
    let edge = [w[0], w[1], w[2] / 2.0];
    let corner = |axis: usize| {
        let mut p = w;
        p[axis] = w[axis] / 2.0;
        p
    };
    let feature = |act, points, size, radii| Feature {
        act,
        points,
        size,
        pull: None,
        radii,
        matrix: None,
    };
    let (base, feature) = match r.next_u64() % 6 {
        0 => {
            let face = [w[0], w[1] / 2.0, w[2] / 2.0];
            let f = feature(
                Act::ChamferTwo,
                vec![edge, face],
                small * r.range(1.1, 1.8),
                vec![small * 0.3],
            );
            (base, f)
        }
        1 => {
            let f = feature(
                Act::FilletVariable,
                vec![edge, [w[0], w[1], 0.0]],
                small * 0.2,
                vec![w[0].min(w[1]) * r.range(1.1, 1.8)],
            );
            (base, f)
        }
        2 => {
            let mut radii = vec![small * 0.2; 3];
            // The edge along axis `a` is blended across the faces as wide as the other two.
            let a = (r.next_u64() % 3) as usize;
            radii[a] = w[(a + 1) % 3].min(w[(a + 2) % 3]) * r.range(1.1, 1.8);
            let f = feature(
                Act::FilletRadii,
                (0..3).map(corner).collect(),
                small * 0.2,
                radii,
            );
            (base, f)
        }
        3 => {
            let mut sizes = vec![small * 0.2; 3];
            // The edge along axis `a` is blended across the faces as wide as the other two.
            let a = (r.next_u64() % 3) as usize;
            sizes[a] = w[(a + 1) % 3].min(w[(a + 2) % 3]) * r.range(1.1, 1.8);
            let f = feature(
                Act::ChamferDistances,
                (0..3).map(corner).collect(),
                small * 0.2,
                sizes,
            );
            (base, f)
        }
        4 => {
            let top = [w[0] / 2.0, w[1] / 2.0, w[2]];
            let f = feature(
                Act::OffsetFaces,
                vec![top],
                -w[2] * r.range(1.1, 1.6),
                Vec::new(),
            );
            (base, f)
        }
        _ => {
            let (low, high, s) = (r.range(1.0, 3.0), r.range(4.0, 7.0), w[0] * 0.5);
            let frame = Frame {
                origin: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            };
            let step = Primitive::Extrude {
                frame,
                profile: polygon(&[
                    [0.0, 0.0],
                    [w[0], 0.0],
                    [w[0], low],
                    [s, low],
                    [s, high],
                    [0.0, high],
                ]),
                height: w[1],
            };
            let at = [(s + w[0]) / 2.0, -w[1] / 2.0, low];
            let f = feature(
                Act::OffsetFaces,
                vec![at],
                (high - low) * r.range(1.2, 1.6),
                Vec::new(),
            );
            (step, f)
        }
    };
    (base.clone(), feature, base.into(), Exact::default())
}

/// A call to a newer function with an input it does not allow: a loft whose heights do not rise,
/// so its sections reach the kernel out of order. A placement that is not rigid is left out: the
/// adapter, not the kernel, turns the suite's matrix into a motion.
fn invalid_call(r: &mut Rng) -> Drawn {
    let (drawn_a, drawn_b, mut exact) = smooth_loft(r);
    if let Some(l) = exact.loft.as_mut() {
        let k = 1 + (r.next_u64() % (l.heights.len() as u64 - 1)) as usize;
        l.heights[k] = l.heights[k - 1] - r.range(0.0, 1.0);
        l.outline.clear();
    }
    (drawn_a, drawn_b, exact)
}

/// A size no solid has: zero, or as far below it as a dropped sign puts it.
fn no_size(r: &mut Rng) -> f64 {
    if r.next_u64().is_multiple_of(2) {
        0.0
    } else {
        -r.range(1.0, 6.0)
    }
}

/// A constructor handed one size that is not positive, the rest of the call sound: a box's side
/// or a radius of zero or below it, or a height of zero. Or a cone with no radius at either end,
/// which is a line. A height below zero is not drawn: a kernel may read it as the other way.
fn invalid_size(r: &mut Rng) -> Drawn {
    const ZERO: &str = "constructions/rejects-size-of-zero";
    let frame = Frame::at([0; 3].map(|_| r.range(-10.0, 10.0)));
    let s = r.range(2.0, 6.0);
    let bad = no_size(r);
    let mut test = if bad == 0.0 {
        ZERO
    } else {
        "constructions/rejects-size-below-zero"
    };
    let a = match r.next_u64() % 6 {
        0 => {
            let mut size = [s, r.range(2.0, 6.0), r.range(2.0, 6.0)];
            size[(r.next_u64() % 3) as usize] = bad;
            Primitive::Box { frame, size }
        }
        1 => Primitive::Cylinder {
            frame,
            radius: bad,
            height: s,
        },
        2 => {
            test = ZERO;
            Primitive::Cylinder {
                frame,
                radius: s,
                height: 0.0,
            }
        }
        3 => Primitive::Sphere { frame, radius: bad },
        4 => {
            test = ZERO;
            Primitive::Extrude {
                frame,
                profile: polygon(&[[0.0, 0.0], [s, 0.0], [s, s], [0.0, s]]),
                height: 0.0,
            }
        }
        _ => {
            test = "constructions/rejects-cone-with-no-radius";
            Primitive::Cone {
                frame,
                bottom: 0.0,
                top: 0.0,
                height: s,
            }
        }
    };
    let exact = Exact {
        variant: Some(test),
        ..Default::default()
    };
    (a.into(), axis_box([0.0; 3], [1.0; 3]).into(), exact)
}

/// A fillet or a chamfer of a box's edge by a size that is not positive: a ball of no radius
/// rounds nothing, and a cut of no distance cuts nothing.
fn invalid_blend_size(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
    let base = axis_box([0.0; 3], w);
    let (act, test) = if r.next_u64().is_multiple_of(2) {
        (Act::Fillet, "fillets/rejects-radius-of-zero-or-less")
    } else {
        (Act::Chamfer, "chamfers/rejects-distance-of-zero-or-less")
    };
    let feature = Feature {
        act,
        points: vec![[w[0], w[1], w[2] / 2.0]],
        size: no_size(r),
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        variant: Some(test),
        ..Default::default()
    };
    (base.clone(), feature, base.into(), exact)
}

/// An act of a sound size at a point that names nothing: a fillet or a chamfer at the middle of a
/// box's face, which no edge passes within a millimetre of, or a face pushed out at a point off
/// the box, which no face holds.
fn invalid_point(r: &mut Rng) -> FeatureDraw {
    let w = [r.range(3.0, 8.0), r.range(3.0, 8.0), r.range(3.0, 8.0)];
    let base = axis_box([0.0; 3], w);
    let mid = [w[0] / 2.0, w[1] / 2.0, w[2]];
    let (act, point, test) = match r.next_u64() % 3 {
        0 => (Act::Fillet, mid, "fillets/rejects-point-on-no-edge"),
        1 => (Act::Chamfer, mid, "chamfers/rejects-point-on-no-edge"),
        _ => (
            Act::OffsetFaces,
            [mid[0], mid[1], w[2] + r.range(0.5, 2.0)],
            "modify/rejects-point-on-no-face",
        ),
    };
    let feature = Feature {
        act,
        points: vec![point],
        size: r.range(0.2, 0.6),
        pull: None,
        radii: Vec::new(),
        matrix: None,
    };
    let exact = Exact {
        variant: Some(test),
        ..Default::default()
    };
    (base.clone(), feature, base.into(), exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every id of the default seed, as one hash. Every platform must draw these ids; CI runs on
    // another platform than most development, so a value that hashes differently fails here. A
    // change that draws other cases updates the hash with its family's version.
    #[test]
    fn the_default_seed_draws_the_same_ids_on_every_platform() {
        let ids: Vec<String> = cases(1).into_iter().map(|c| c.id).collect();
        let got = format!("{:016x}", fnv(&ids.join("\n")));
        if got != "4095bb09151ed05f" {
            // Every id, so a run elsewhere can be compared with this one case by case.
            for id in &ids {
                eprintln!("id {id}");
            }
        }
        assert_eq!(got, "4095bb09151ed05f");
    }

    #[test]
    fn held_values_hash_the_same_whatever_their_last_bit() {
        let a = 0.1_f64 + 0.2;
        let b = f64::from_bits(a.to_bits() + 1);
        assert_ne!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(
            rounded_floats(&format!("{a:?}")),
            rounded_floats(&format!("{b:?}"))
        );
        assert_eq!(
            rounded_floats("Clash { i: 3, j: 12, volume: 0.30000000000000004, gap: -1.5e-7 }"),
            "Clash { i: 3, j: 12, volume: 3.00000000e-1, gap: -1.50000000e-7 }"
        );
        assert_eq!(
            rounded_floats("[[1.0, -2.5], Some(4e3)]"),
            "[[1.00000000e0, -2.50000000e0], Some(4.00000000e3)]"
        );
        // What cancellation leaves of a zero, whatever its sign, is a zero.
        assert_eq!(
            rounded_floats("-2.168404344971e-19,1.2e-19,-0.0,0.0"),
            "0e0,0e0,0e0,0e0"
        );
    }

    #[test]
    fn a_polygon_spline_round_a_unit_square_holds_one_and_runs_four() {
        let sq = Spline {
            degree: 1,
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
            ctrl: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
        };
        let (a, l) = sq.area_and_length();
        assert!(
            (a - 1.0).abs() < 1e-12 && (l - 4.0).abs() < 1e-12,
            "{a} {l}"
        );
    }

    #[test]
    fn a_seed_draws_the_same_cases_with_unique_ids() {
        let (x, y) = (cases(1), cases(1));
        let all = FAMILIES.iter().map(|f| f.count).sum::<usize>()
            + FEATURE_FAMILIES.iter().map(|f| f.count).sum::<usize>()
            + REFUSAL_FAMILIES.iter().map(|f| f.count).sum::<usize>()
            + FEATURE_REFUSALS.iter().map(|f| f.count).sum::<usize>()
            + STEP_FAMILIES.iter().map(|f| f.count).sum::<usize>()
            + TRANSFORMS
            + STRETCHES;
        assert_eq!(x.len(), all);
        assert!(x.iter().zip(&y).all(|(p, q)| p.id == q.id));
        let mut ids: Vec<&str> = x.iter().map(|c| c.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), x.len());
    }

    #[test]
    fn a_cylinder_through_a_cones_middle_keeps_the_cone_below_and_its_own_core_above() {
        // A cone of radius 0 to 3 over height 6 in a cylinder of radius 1.5: below z = 3 the cone,
        // π·1.5²·3/3; above it the cylinder, π·1.5²·3.
        let v = frustum_in_cylinder(0.0, 3.0, 6.0, 1.5, 0.0, 6.0);
        assert!((v - PI * 2.25 * 4.0).abs() < 1e-12, "{v}");
    }

    #[test]
    fn spheres_of_one_radius_one_radius_apart_share_five_twelfths_pi() {
        // Two unit spheres with centres 1 apart: the lens is 5π/12 (two caps of height 1/2).
        let (r1, r2, d): (f64, f64, f64) = (1.0, 1.0, 1.0);
        let v = PI
            * (r1 + r2 - d).ppowi(2)
            * (d * d + 2.0 * d * r2 - 3.0 * r2 * r2 + 2.0 * d * r1 + 6.0 * r1 * r2 - 3.0 * r1 * r1)
            / (12.0 * d);
        assert!((v - 5.0 * PI / 12.0).abs() < 1e-12);
    }
}
