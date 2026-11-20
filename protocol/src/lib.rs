//! The operations of [doc/api.md](../../doc/api.md) as a Rust API and as a protocol:
//! the shapes a kernel is asked to make ([`model`], [`profile`]), the [`Kernel`] trait a kernel
//! implements, [`serve`] to answer the protocol over stdin and stdout with any implementation, and
//! [`Remote`], an implementation that forwards each call to worker processes speaking it.

pub mod model;
pub mod path;
pub mod portable;
pub mod profile;
mod remote;
mod serve;
mod wire;

pub use model::Op;
pub use remote::Remote;

pub use serve::serve;

use model::{Frame, Primitive, Side, Vec3};

/// The protocol's version, which `hello` reports: requests and replies in
/// [doc/api.md](../../doc/api.md#calling-a-kernel).
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Every function of this version of the protocol, in the order of
/// [doc/api.md](../../doc/api.md). `hello` lists the ones a kernel offers out of these.
pub const FUNCTIONS: &[&str] = &[
    "box",
    "cylinder",
    "sphere",
    "torus",
    "cone",
    "extrude",
    "revolve",
    "screw",
    "loft",
    "loft_through",
    "sweep",
    "sweep_path",
    "slab",
    "union",
    "intersection",
    "difference",
    "union_all",
    "difference_all",
    "fillet",
    "fillet_radii",
    "fillet_variable",
    "fillet_conic",
    "fillet_g2",
    "fillet_setback",
    "fillet_setback_g2",
    "fillet_faces",
    "full_round",
    "chamfer",
    "chamfer_two",
    "chamfer_distances",
    "offset",
    "offset_faces",
    "delete_faces",
    "move_faces",
    "thicken",
    "shell",
    "draft",
    "write_step",
    "sew",
    "read_step",
    "volume",
    "area",
    "classify",
    "closest",
    "normal",
    "curvature",
    "check",
    "centroid",
    "inertia",
    "distance",
    "interference",
    "betti",
    "section_area",
    "section",
    "project",
    "bounding_box",
    "split",
    "mesh",
    "display",
    "raycast",
    "transform",
    "affine",
];

/// Whether a client speaking `ours` may drive an adapter speaking `theirs`: their major versions
/// agree.
pub(crate) fn speaks(ours: &str, theirs: &str) -> bool {
    let major = |v: &str| v.split('.').next().and_then(|n| n.parse::<u64>().ok());
    major(ours).is_some() && major(ours) == major(theirs)
}

/// The panic payload of a call that ran past its time limit, so a client can tell a timeout from a
/// crash.
#[derive(Clone, Debug)]
pub struct TimedOut(pub String);

/// A triangle mesh: positions, and triangles as three indices each.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// One face of a [`Display`]: its own vertices, the exact surface's outward normal at each, and
/// its triangles over them.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DisplayFace {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// A solid as a viewer draws it: each face meshed on its own, and the edges that are creases as
/// polylines.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Display {
    pub faces: Vec<DisplayFace>,
    pub edges: Vec<Vec<Vec3>>,
}

impl Mesh {
    pub fn new(positions: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Mesh {
        Mesh {
            positions,
            triangles,
        }
    }
}

impl DisplayFace {
    pub fn new(positions: Vec<Vec3>, normals: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> DisplayFace {
        DisplayFace {
            positions,
            normals,
            triangles,
        }
    }
}

impl Display {
    pub fn new(faces: Vec<DisplayFace>, edges: Vec<Vec<Vec3>>) -> Display {
        Display { faces, edges }
    }
}

/// A rigid motion placing a part: rows `[R | t]`, `R` a rotation.
pub type Placement = [[f64; 4]; 3];

/// Why a kernel declined a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// The kernel has no way to do what is asked: not this function, or not with these arguments.
    Unsupported,
    /// The request is at fault: it describes no solid, or names an edge or a face that is not
    /// there.
    Invalid,
    /// The kernel took the request as a fair one and could not carry it out.
    Failed,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Unsupported => "unsupported",
            Kind::Invalid => "invalid",
            Kind::Failed => "failed",
        }
    }

    /// The kind a word names; an unknown or missing word is a failure.
    pub fn from_word(word: Option<&str>) -> Kind {
        match word {
            Some("unsupported") => Kind::Unsupported,
            Some("invalid") => Kind::Invalid,
            _ => Kind::Failed,
        }
    }
}

/// A request declined: of which kind, and the kernel's reason in its own words.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Refusal {
    pub kind: Kind,
    pub reason: String,
}

impl Refusal {
    pub fn new(kind: Kind, reason: impl Into<String>) -> Refusal {
        Refusal {
            kind,
            reason: reason.into(),
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Refusal {
        Refusal {
            kind: Kind::Unsupported,
            reason: reason.into(),
        }
    }

    pub fn invalid(reason: impl Into<String>) -> Refusal {
        Refusal {
            kind: Kind::Invalid,
            reason: reason.into(),
        }
    }

    pub fn failed(reason: impl Into<String>) -> Refusal {
        Refusal {
            kind: Kind::Failed,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind.word(), self.reason)
    }
}

/// A reason with no kind given is a failure.
impl From<String> for Refusal {
    fn from(reason: String) -> Refusal {
        Refusal::failed(reason)
    }
}

impl From<&str> for Refusal {
    fn from(reason: &str) -> Refusal {
        Refusal::failed(reason)
    }
}

/// How two solids meet: the volume they share, and the gap between them, zero where they touch or
/// overlap. Two that only touch have neither.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meeting {
    pub volume: f64,
    pub gap: f64,
}

/// A solid's edges and outlines drawn as the viewer sees them looking along a direction `d`: each
/// curve a polyline in the view plane, split into what the viewer sees and what the solid hides.
/// The plane's axes are `u`, `d × z` made unit (`d × x` when `d` runs along z), across, and
/// `v = −d × u`, up; a point `p` draws at `(p·u, p·v)`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Projection {
    pub visible: Vec<Vec<[f64; 2]>>,
    pub hidden: Vec<Vec<[f64; 2]>>,
}

impl Projection {
    pub fn new(visible: Vec<Vec<[f64; 2]>>, hidden: Vec<Vec<[f64; 2]>>) -> Projection {
        Projection { visible, hidden }
    }
}

/// The axes of the view plane looking along `d`, as [`Projection`] defines them.
pub fn view_axes(d: Vec3) -> (Vec3, Vec3) {
    let cross = |a: Vec3, b: Vec3| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |a: Vec3| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let d = unit(d);
    let across = cross(d, [0.0, 0.0, 1.0]);
    let u = if across.iter().map(|x| x * x).sum::<f64>() < 1e-12 {
        unit(cross(d, [1.0, 0.0, 0.0]))
    } else {
        unit(across)
    };
    (u, cross([-d[0], -d[1], -d[2]], u))
}

/// A solid's volume, area and validity problems, gathered by [`Kernel::measure`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Measure {
    pub volume: f64,
    pub area: f64,
    /// Validity problems its own checker finds; empty when valid.
    pub issues: Vec<String>,
}

impl Measure {
    pub fn new(volume: f64, area: f64, issues: Vec<String>) -> Measure {
        Measure {
            volume,
            area,
            issues,
        }
    }
}

/// One kernel: the functions of [doc/api.md](../../doc/api.md). Errors are refusals:
/// the kernel declined, with a reason. A panic is a crash.
///
/// A function added to the protocol comes with a default body, refusing it as unsupported, so an
/// adapter written before it still builds; it is offered only once the adapter lists it.
pub trait Kernel: Sync {
    type Solid;
    fn name(&self) -> String;
    fn version(&self) -> String;
    /// The functions of [`FUNCTIONS`] this kernel offers, by name: `hello` lists these, and the
    /// suite runs no case that needs another. A function not listed is not offered, whatever its
    /// method does, and a name that is no function is left out.
    fn supports(&self) -> Vec<String>;
    /// The linear tolerance the kernel declares, mm: the loosest still governing its results.
    fn tolerance(&self) -> f64;
    /// What the adapter set differently from the kernel's defaults; empty when nothing.
    fn configuration(&self) -> String {
        String::new()
    }
    /// Every constructor, each a variant of [`Primitive`].
    fn make(&self, p: &Primitive) -> Result<Self::Solid, Refusal>;
    fn union(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal>;
    fn intersection(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal>;
    fn difference(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal>;
    /// The solid's volume, mm³: an error where the kernel could not measure it.
    fn volume(&self, s: &Self::Solid) -> Result<f64, Refusal>;
    /// The area of the solid's boundary, mm²: an error where the kernel could not measure it.
    fn area(&self, s: &Self::Solid) -> Result<f64, Refusal>;
    /// Validity problems the kernel's own checker finds; empty when valid. An error where the
    /// checker declined the solid, which is not a finding that it is valid.
    fn check(&self, s: &Self::Solid) -> Result<Vec<String>, Refusal>;
    /// Where each point lies against the solid, by the kernel's own classifier: inside it, on its
    /// boundary or outside it.
    fn classify(&self, _s: &Self::Solid, _points: &[Vec3]) -> Result<Vec<Side>, Refusal> {
        Err(Refusal::unsupported("classify is not supported"))
    }
    /// Whether the solid holds each point, its boundary counted in: [`Kernel::classify`] read as
    /// yes or no.
    fn holds(&self, s: &Self::Solid, points: &[Vec3]) -> Result<Vec<bool>, Refusal> {
        let sides = self.classify(s, points)?;
        Ok(sides.into_iter().map(|side| side != Side::Out).collect())
    }
    /// The point of the solid nearest each point: the point itself where the solid holds it.
    fn closest(&self, _s: &Self::Solid, _points: &[Vec3]) -> Result<Vec<Vec3>, Refusal> {
        Err(Refusal::unsupported("closest is not supported"))
    }
    /// The outward unit normal at each point of the solid's boundary, where the boundary has a
    /// tangent plane there; `None` at a crease, where faces meet at an angle, or off the boundary.
    fn normal(&self, _s: &Self::Solid, _points: &[Vec3]) -> Result<Vec<Option<Vec3>>, Refusal> {
        Err(Refusal::unsupported("normal is not supported"))
    }
    /// Rounds the edges of `s` that the points lie on, each by a ball of `radius` rolling along it
    /// while touching both faces that meet there.
    fn fillet(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _radius: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet is not supported"))
    }
    /// [`Kernel::fillet`] with each point's edge chain at its own radius, in one call, so that
    /// unequal radii meet at shared corners.
    fn fillet_radii(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _radii: &[f64],
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_radii is not supported"))
    }
    /// [`Kernel::chamfer`] with each point's edge chain at its own distance, in one call, so that
    /// unequal chamfers meet at shared corners.
    fn chamfer_distances(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _distances: &[f64],
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("chamfer_distances is not supported"))
    }
    /// Bevels the edge `edge` lies on by a cut `d1` back along the face `face` lies on and `d2` back
    /// along the other face there. A chamfer by a distance and an angle is this, `d2 = d1·tan(angle)`.
    fn chamfer_two(
        &self,
        _s: &Self::Solid,
        _edge: Vec3,
        _face: Vec3,
        _d1: f64,
        _d2: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("chamfer_two is not supported"))
    }
    /// Rounds the edge `edge` lies on by a ball whose radius runs evenly with length along the edge
    /// from `r_from` at its end nearest `from` to `r_to` at its other end.
    fn fillet_variable(
        &self,
        _s: &Self::Solid,
        _edge: Vec3,
        _from: Vec3,
        _r_from: f64,
        _r_to: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_variable is not supported"))
    }
    /// Rounds the edges the points lie on by the curvature-continuous section of order 2: it meets
    /// each face where a ball of radius `offset` would (its tangent offset), with the face's
    /// curvature as well as its tangent plane, and bends with radius `center` at its middle.
    fn fillet_g2(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _offset: f64,
        _center: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_g2 is not supported"))
    }
    /// Rounds the edges the points lie on by a conic blend meeting each face `setback` from the edge,
    /// its section the conic tangent to both faces there whose shoulder lies `rho` of the way from
    /// the chord's middle to the edge.
    fn fillet_conic(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _setback: f64,
        _rho: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_conic is not supported"))
    }
    /// [`Kernel::fillet`] of `radius` whose fillets stop `setback` short of each corner where three
    /// of the edges meet, the corner closed by one smooth patch meeting the three fillets and faces.
    fn fillet_setback(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _radius: f64,
        _setback: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_setback is not supported"))
    }
    /// [`Kernel::fillet_setback`] curvature-continuous throughout: each fillet the
    /// [`Kernel::fillet_g2`] section of tangent offset `radius` at the equal-area thumbweight, and
    /// the corner patch meeting the fillets and faces with their curvature as well as their
    /// tangent planes.
    fn fillet_setback_g2(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _radius: f64,
        _setback: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_setback_g2 is not supported"))
    }
    /// Rounds between the faces `face_a` and `face_b` lie on, which need not share an edge, by a ball
    /// of `radius` rolling while touching both: what lies outside the ball's path between its
    /// contacts goes, faces between them included.
    fn fillet_faces(
        &self,
        _s: &Self::Solid,
        _face_a: Vec3,
        _face_b: Vec3,
        _radius: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("fillet_faces is not supported"))
    }
    /// Replaces the face `middle` lies on by a round tangent to it and to the faces `side_a` and
    /// `side_b` lie on, which stay where they are.
    fn full_round(
        &self,
        _s: &Self::Solid,
        _side_a: Vec3,
        _middle: Vec3,
        _side_b: Vec3,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("full_round is not supported"))
    }
    /// The principal curvatures `[k1, k2]`, `k1 ≥ k2`, at the point of the boundary nearest each
    /// point: positive where the boundary bends away from its outward normal, as on a ball; `None`
    /// where that point lies on a crease.
    fn curvature(
        &self,
        _s: &Self::Solid,
        _points: &[Vec3],
    ) -> Result<Vec<Option<[f64; 2]>>, Refusal> {
        Err(Refusal::unsupported("curvature is not supported"))
    }
    /// The solid's edges and outlines seen looking along `direction`, each chord within `tolerance`
    /// of its curve: a hidden-line drawing.
    fn project(
        &self,
        _s: &Self::Solid,
        _direction: Vec3,
        _tolerance: f64,
    ) -> Result<Projection, Refusal> {
        Err(Refusal::unsupported("project is not supported"))
    }
    /// Bevels the edges of `s` that the points lie on, cutting `distance` back along both faces.
    fn chamfer(
        &self,
        _s: &Self::Solid,
        _edges: &[Vec3],
        _distance: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("chamfer is not supported"))
    }
    /// The solid as the text of a STEP file (ISO 10303-21).
    fn write_step(&self, _s: &Self::Solid) -> Result<String, Refusal> {
        Err(Refusal::unsupported("write_step is not supported"))
    }
    /// The solids a STEP file's text holds, as one solid set.
    fn read_step(&self, _text: &str) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("read_step is not supported"))
    }
    /// The centre of mass at unit density, mm; `None` for an empty solid.
    fn centroid(&self, _s: &Self::Solid) -> Result<Option<Vec3>, Refusal> {
        Err(Refusal::unsupported("centroid is not supported"))
    }
    /// `∭ (|r|²·I − r rᵀ) dV` about the centroid at unit density, mm⁵.
    fn inertia(&self, _s: &Self::Solid) -> Result<[[f64; 3]; 3], Refusal> {
        Err(Refusal::unsupported("inertia is not supported"))
    }
    /// The least distance between a point of `a` and a point of `b`, insides included: zero where
    /// they touch or overlap.
    fn distance(&self, _a: &Self::Solid, _b: &Self::Solid) -> Result<f64, Refusal> {
        Err(Refusal::unsupported("distance is not supported"))
    }
    /// The Betti numbers of the solid set: its separate pieces, its tunnels and its voids.
    fn betti(&self, _s: &Self::Solid) -> Result<[u64; 3], Refusal> {
        Err(Refusal::unsupported("betti is not supported"))
    }
    /// The area of the solid's cut by the plane through `origin` square to `normal`, mm².
    fn section_area(&self, _s: &Self::Solid, _origin: Vec3, _normal: Vec3) -> Result<f64, Refusal> {
        Err(Refusal::unsupported("section_area is not supported"))
    }
    /// A triangle mesh of the solid's boundary within `tolerance` of it: positions, and triangles
    /// of three indices each, counter-clockwise seen from outside.
    fn mesh(&self, _s: &Self::Solid, _tolerance: f64) -> Result<Mesh, Refusal> {
        Err(Refusal::unsupported("mesh is not supported"))
    }
    /// The solid as a viewer draws it, within `tolerance`: each face's triangles with the exact
    /// normal at every vertex, and its creases as polylines.
    fn display(&self, _s: &Self::Solid, _tolerance: f64) -> Result<Display, Refusal> {
        Err(Refusal::unsupported("display is not supported"))
    }
    /// The points within `distance` of the solid: its outward offset.
    fn offset(&self, _s: &Self::Solid, _distance: f64) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("offset is not supported"))
    }
    /// The solid hollowed to walls `thickness` thick inside it, open through the faces the points
    /// lie on.
    fn shell(
        &self,
        _s: &Self::Solid,
        _open: &[Vec3],
        _thickness: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("shell is not supported"))
    }
    /// The faces the points lie on, tilted by `angle` about where they meet the plane through
    /// `origin` square to `pull`, narrowing the part along `pull`.
    fn draft(
        &self,
        _s: &Self::Solid,
        _faces: &[Vec3],
        _origin: Vec3,
        _pull: Vec3,
        _angle: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("draft is not supported"))
    }
    /// The solid moved by `p ↦ M·p + t`, the rows of `matrix` being `[M | t]`, `M` a rotation or
    /// mirror times a nonzero scale.
    fn transform(&self, _s: &Self::Solid, _matrix: [[f64; 4]; 3]) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("transform is not supported"))
    }
    /// The solid mapped by `p ↦ M·p + t` for any invertible `M`: stretches and shears included.
    fn affine(&self, _s: &Self::Solid, _matrix: [[f64; 4]; 3]) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("affine is not supported"))
    }
    /// Where the ray from `origin` along `direction` meets the solid's faces: the distance from
    /// `origin` to each meeting, mm, in any order. A ray through an edge may list the point once
    /// for each face there: the caller reads meetings closer than its tolerance as one.
    fn raycast(
        &self,
        _s: &Self::Solid,
        _origin: Vec3,
        _direction: Vec3,
    ) -> Result<Vec<f64>, Refusal> {
        Err(Refusal::unsupported("raycast is not supported"))
    }
    /// [`Kernel::raycast`] for many rays, each an origin and a direction: every ray's meetings,
    /// in the rays' order. A kernel that prepares a solid for casting overrides it to prepare
    /// once.
    fn raycasts(&self, s: &Self::Solid, rays: &[(Vec3, Vec3)]) -> Result<Vec<Vec<f64>>, Refusal> {
        rays.iter().map(|&(o, d)| self.raycast(s, o, d)).collect()
    }
    /// How two solids meet: the volume they share and the gap between them.
    fn interference(&self, _a: &Self::Solid, _b: &Self::Solid) -> Result<Meeting, Refusal> {
        Err(Refusal::unsupported("interference is not supported"))
    }
    /// The union of all the solids, two or more, in one call. The kernel's own, or not offered: a
    /// run of pairwise unions in its place would be the suite's work under the kernel's name.
    fn union_all(&self, _solids: &[&Self::Solid]) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("union_all is not supported"))
    }
    /// `target` less every tool, one or more, in one call.
    fn difference_all(
        &self,
        _target: &Self::Solid,
        _tools: &[&Self::Solid],
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("difference_all is not supported"))
    }
    /// The faces the points lie on, each moved `distance` along its outward normal onto its offset
    /// surface, the faces round them extended or trimmed to meet them.
    fn offset_faces(
        &self,
        _s: &Self::Solid,
        _faces: &[Vec3],
        _distance: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("offset_faces is not supported"))
    }
    /// The smooth solid through the sections: profile `k` drawn in the frame's xy plane moved
    /// `heights[k]` up its z, the heights rising.
    fn loft_through(
        &self,
        _frame: &Frame,
        _profiles: &[profile::Profile],
        _heights: &[f64],
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("loft_through is not supported"))
    }
    /// The solid with the faces the points lie on taken out, the faces round them extended or
    /// trimmed to close the gap.
    fn delete_faces(&self, _s: &Self::Solid, _faces: &[Vec3]) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("delete_faces is not supported"))
    }
    /// The faces the points lie on moved rigidly by the rows `[R | t]`, the faces round them
    /// extended or trimmed to meet them.
    fn move_faces(
        &self,
        _s: &Self::Solid,
        _faces: &[Vec3],
        _matrix: [[f64; 4]; 3],
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("move_faces is not supported"))
    }
    /// The solid the faces the points lie on sweep moving from 0 to `thickness` along their
    /// outward normals, inward where it is negative; the input is not part of it.
    fn thicken(
        &self,
        _s: &Self::Solid,
        _faces: &[Vec3],
        _thickness: f64,
    ) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("thicken is not supported"))
    }
    /// The solid cut by the plane through `origin` square to `normal`: the part on the normal's
    /// side, then the part on the other, either of which may be empty.
    fn split(
        &self,
        _s: &Self::Solid,
        _origin: Vec3,
        _normal: Vec3,
    ) -> Result<(Self::Solid, Self::Solid), Refusal> {
        Err(Refusal::unsupported("split is not supported"))
    }
    /// The least box with sides square to the axes that holds the solid, as its low and high
    /// corners.
    fn bounding_box(&self, _s: &Self::Solid) -> Result<[Vec3; 2], Refusal> {
        Err(Refusal::unsupported("bounding_box is not supported"))
    }
    /// Where the plane through `origin` square to `normal` cuts the solid's boundary, in pieces
    /// in any order: each a stretch of one curve as points along it from end to end, every chord
    /// within `tolerance` of the curve. The pieces join end to end into closed curves, and a
    /// piece that is a whole closed curve ends on its first point.
    fn section(
        &self,
        _s: &Self::Solid,
        _origin: Vec3,
        _normal: Vec3,
        _tolerance: f64,
    ) -> Result<Vec<Vec<Vec3>>, Refusal> {
        Err(Refusal::unsupported("section is not supported"))
    }
    /// The faces of a STEP file, each its own, sewn into a solid where their edges lie within
    /// `tolerance` of each other.
    fn sew(&self, _text: &str, _tolerance: f64) -> Result<Self::Solid, Refusal> {
        Err(Refusal::unsupported("sew is not supported"))
    }
    /// Releases every solid this caller made, once it is done with them.
    fn reset(&self) {}

    fn boolean(&self, op: Op, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal> {
        match op {
            Op::Union => self.union(a, b),
            Op::Intersection => self.intersection(a, b),
            Op::Difference => self.difference(a, b),
        }
    }

    fn measure(&self, s: &Self::Solid) -> Measure {
        // A checker that declines a solid has not found it valid: its reason is an issue, unless
        // the kernel has no checker to ask. A measure the kernel could not take is no number, and
        // its reason an issue.
        let mut issues = match self.check(s) {
            Ok(found) => found,
            Err(e) if e.kind == Kind::Unsupported => Vec::new(),
            Err(e) => vec![format!("the checker declined the solid, {e}")],
        };
        let mut taken = |name: &str, r: Result<f64, Refusal>| {
            r.unwrap_or_else(|e| {
                issues.push(format!("{name}: {e}"));
                f64::NAN
            })
        };
        let (volume, area) = (taken("volume", self.volume(s)), taken("area", self.area(s)));
        Measure {
            volume,
            area,
            issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_function_list_is_the_reference_s_tables() {
        let reference = include_str!("../../doc/api.md");
        let documented: Vec<&str> = reference
            .lines()
            .filter_map(|line| line.strip_prefix("| [`")?.split('`').next())
            .collect();
        assert_eq!(documented, FUNCTIONS);
        assert!(reference.contains(&format!("version {PROTOCOL_VERSION} of the protocol")));
    }

    #[test]
    fn versions_agree_on_the_major() {
        assert!(speaks("1.0.0", "1.0.0"));
        assert!(speaks("1.2.0", "1.0.3"));
        assert!(!speaks("1.2.0", "2.2.0"));
        assert!(!speaks("1.0.0", "0.10"));
        assert!(!speaks("1.0.0", "none"));
    }
}
