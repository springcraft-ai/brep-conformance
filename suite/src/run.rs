//! The runner: each case's operations through a kernel adapter, the contract's checks on what
//! came back, and the run's results as JSON.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

pub use brep_protocol::{Kernel, Kind, Measure, Op, Refusal, Remote, TimedOut};

use std::collections::BTreeMap;

use crate::generate::{
    Act, Assembly, Case, Clash, Feature, LoftThrough, Many, Mirror, Normals, Sound,
};
use crate::model::{Primitive, Shape, Vec3};
use crate::{mass, oracle};

mod export;

pub use export::{EXPORT_TOLERANCE, Exports};

/// The suite's release and the contract version its checks implement.
pub const SUITE_VERSION: &str = "2.0.2";
pub const CONTRACT_VERSION: &str = "2.0.0";
/// The version of the results file and of the flat report, as
/// [doc/formats.md](../../doc/formats.md) gives them: raised when a field is removed or changes
/// what it means, and not when one is added.
pub const RESULTS_FORMAT: u32 = 1;

/// Families whose features can be thinner than the ray march sees, or touch: B26 skips them.
const THIN: [&str; 10] = [
    "touching",
    "line-contact",
    "thin-solids",
    "nearly",
    "shaved",
    "near-coincident",
    "inscribed",
    "parallel-cylinders",
    "spline-prisms",
    "scaled",
];

/// How near two answers of a kernel lie and are one: two meetings of a ray with the faces at an
/// edge, or the ends of two pieces of a section where they join, mm.
pub const SAME: f64 = 10.0 * TOLERANCE;

/// A ray's crossings from what [`Kernel::raycast`] lists: in order, meetings within [`SAME`] of
/// each other one crossing, each as a multiple of the direction's length.
pub fn crossings(mut hits: Vec<f64>, direction: Vec3) -> Vec<f64> {
    let len = direction.iter().map(|x| x * x).sum::<f64>().sqrt();
    hits.sort_by(f64::total_cmp);
    hits.dedup_by(|later, kept| *later - *kept <= SAME);
    hits.into_iter().map(|t| t / len).collect()
}

/// A section's pieces joined end to end into closed curves, each without its last point's
/// return to the first. An end that no other end lies within [`SAME`] of is an error.
pub fn chained(pieces: Vec<Vec<Vec3>>) -> Result<Vec<Vec<Vec3>>, String> {
    let near = |p: Vec3, q: Vec3| (0..3).map(|i| (p[i] - q[i]).powi(2)).sum::<f64>().sqrt() <= SAME;
    if let Some(p) = pieces.iter().find(|p| p.len() < 2) {
        return Err(format!("a piece of {} points", p.len()));
    }
    let mut left: Vec<Vec<Vec3>> = pieces;
    let mut loops = Vec::new();
    while let Some(mut run) = left.pop() {
        while !(run.len() > 2 && near(run[0], run[run.len() - 1])) {
            let end = run[run.len() - 1];
            let Some(i) = left
                .iter()
                .position(|p| near(p[0], end) || near(p[p.len() - 1], end))
            else {
                return Err(format!("a piece ends at {end:?}, where no other begins"));
            };
            let mut next = left.swap_remove(i);
            if !near(next[0], end) {
                next.reverse();
            }
            run.extend(next.into_iter().skip(1));
        }
        run.pop();
        loops.push(run);
    }
    Ok(loops)
}

fn apply(m: &[[f64; 4]; 3], p: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
}

/// The chord tolerance a mesh is asked for, mm.
const MESH_TOLERANCE: f64 = 0.01;

/// Whether a mesh is closed with every edge used once each way round (so its triangles face
/// consistently), and the volume it bounds.
pub(crate) fn mesh_check(m: &brep_protocol::Mesh) -> (bool, f64) {
    let m = &welded(m);
    let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
    let mut volume = 0.0;
    // About a point of the mesh: about the origin each term of a part far from it is the small
    // difference of products of its coordinates, and rounding swamps the volume.
    let o = m.positions.first().copied().unwrap_or([0.0; 3]);
    for t in &m.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
        }
        let [p, q, r] = t.map(|i| {
            let x = m.positions[i as usize];
            [x[0] - o[0], x[1] - o[1], x[2] - o[2]]
        });
        volume += (p[0] * (q[1] * r[2] - q[2] * r[1]) - p[1] * (q[0] * r[2] - q[2] * r[0])
            + p[2] * (q[0] * r[1] - q[1] * r[0]))
            / 6.0;
    }
    let mut count: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
    for t in &m.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let closed = edges.values().all(|&n| n == 0) && count.values().all(|&n| n % 2 == 0);
    (closed, volume)
}

/// The mesh with positions within [`SAME`] of each other made one vertex, and the triangles that
/// leaves with two corners at one vertex taken out: a kernel may mesh each face on its own.
fn welded(m: &brep_protocol::Mesh) -> brep_protocol::Mesh {
    let cell = |p: Vec3| p.map(|x| (x / (4.0 * SAME)).floor() as i64);
    let mut grid: std::collections::HashMap<[i64; 3], Vec<u32>> = std::collections::HashMap::new();
    let mut positions: Vec<Vec3> = Vec::new();
    let mut to = Vec::with_capacity(m.positions.len());
    for &p in &m.positions {
        let c = cell(p);
        let mut found = None;
        'near: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = [c[0] + dx, c[1] + dy, c[2] + dz];
                    for &i in grid.get(&key).into_iter().flatten() {
                        let q = positions[i as usize];
                        if (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt() <= SAME {
                            found = Some(i);
                            break 'near;
                        }
                    }
                }
            }
        }
        to.push(found.unwrap_or_else(|| {
            positions.push(p);
            let i = positions.len() as u32 - 1;
            grid.entry(c).or_default().push(i);
            i
        }));
    }
    let triangles = m
        .triangles
        .iter()
        .map(|t| t.map(|i| to[i as usize]))
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0])
        .collect();
    brep_protocol::Mesh::new(positions, triangles)
}

/// Sample points a case's intersection is estimated and classified on.
const SAMPLES: usize = 4096;

/// How an operation that did not complete ended.
#[derive(Clone, Debug)]
enum Ran {
    Refused(Refusal),
    TimedOut(String),
    Crashed(String),
}

impl Ran {
    fn outcome(&self) -> Outcome {
        match self {
            // A kernel that cannot do what is asked is not judged on it.
            Ran::Refused(r) if r.kind == Kind::Unsupported => Outcome::Unsupported,
            Ran::Refused(_) => Outcome::Refused,
            Ran::TimedOut(_) => Outcome::Timeout,
            Ran::Crashed(_) => Outcome::Crash,
        }
    }

    /// What the kernel said: a refusal's kind, then its reason.
    fn message(&self) -> String {
        match self {
            Ran::Refused(r) => r.to_string(),
            Ran::TimedOut(m) | Ran::Crashed(m) => m.clone(),
        }
    }
}

/// Worst last: a refusal declines, a timeout never answers, a failed check misleads, and a crash
/// takes the operation down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Pass,
    /// The kernel does not offer a function the case needs: no judgement either way.
    Unsupported,
    Refused,
    Timeout,
    Fail,
    Crash,
}

impl Outcome {
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Unsupported => "unsupported",
            Outcome::Refused => "refused",
            Outcome::Timeout => "timeout",
            Outcome::Fail => "fail",
            Outcome::Crash => "crash",
        }
    }
}

fn attempt<T>(f: impl FnOnce() -> Result<T, Refusal>) -> Result<T, Ran> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(Ran::Refused(e)),
        Err(p) => {
            if let Some(t) = p.downcast_ref::<TimedOut>() {
                return Err(Ran::TimedOut(t.0.clone()));
            }
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            Err(Ran::Crashed(msg))
        }
    }
}

/// Whether `s` holds each point, by the kernel's classifier. `None` where the kernel has no
/// classifier for them, which leaves a check out. The reason where it has one that gave no answer:
/// a membership the kernel could not give is a check failed, not a check passed over.
fn placed<K: Kernel>(k: &mut K, s: &K::Solid, pts: &[Vec3]) -> Option<Result<Vec<bool>, String>> {
    match attempt(|| k.holds(s, pts)) {
        Ok(got) => Some(Ok(got)),
        Err(e) if e.outcome() == Outcome::Unsupported => None,
        Err(e) => Some(Err(unplaced(&e.message()))),
    }
}

fn unplaced(why: &str) -> String {
    format!("classify gave no answer: {why}")
}

/// What the suite sends in a primitive's place: its extrusion or revolution, when the kernel lists
/// the primitive's constructor as unsupported and offers the other.
fn stand_in(unsupported: &[String], p: &Primitive) -> Option<Primitive> {
    let lacks = |q: &Primitive| unsupported.iter().any(|u| u == q.kind());
    if !lacks(p) {
        return None;
    }
    p.stand_in().filter(|q| !lacks(q))
}

/// Whether the kernel can make the primitive, by its constructor or by its stand-in.
fn makes(unsupported: &[String], p: &Primitive) -> bool {
    !unsupported.iter().any(|u| u == p.kind()) || stand_in(unsupported, p).is_some()
}

/// The image of the primitive `p`, made as `a`, in the mirror `m`: by the kernel's `transform`,
/// or, where the kernel says it has no mirror, the mirrored primitive made in its place. A case
/// about a boolean or a fillet across a mirror does not turn on whether the kernel can mirror.
fn mirror_image<K: Kernel>(
    k: &mut K,
    p: &Primitive,
    a: &K::Solid,
    m: [[f64; 4]; 3],
) -> Result<K::Solid, Refusal> {
    match k.transform(a, m) {
        Err(r) if r.kind == Kind::Unsupported => match p.mirrored(&m) {
            Some(q) => construct(k, &q),
            None => Err(r),
        },
        made => made,
    }
}

/// Whether the kernel can make every primitive of the shape.
fn makes_all(unsupported: &[String], s: &Shape) -> bool {
    match s {
        Shape::Prim(p) => makes(unsupported, p),
        Shape::Bool(_, a, b) => makes_all(unsupported, a) && makes_all(unsupported, b),
    }
}

/// The functions of the protocol the kernel does not offer: every one its `supports` leaves out.
fn lacks<K: Kernel + ?Sized>(k: &K) -> Vec<String> {
    let offered = k.supports();
    brep_protocol::FUNCTIONS
        .iter()
        .filter(|&&f| !offered.iter().any(|s| s == f))
        .map(|&f| f.to_string())
        .collect()
}

/// Makes a primitive as an operand: a case about a boolean or a fillet does not turn on whether the
/// kernel has a constructor for the solid it starts from.
fn construct<K: Kernel>(k: &mut K, p: &Primitive) -> Result<K::Solid, Refusal> {
    match stand_in(&lacks(k), p) {
        Some(q) => k.make(&q),
        None => k.make(p),
    }
}

/// The constructors this kernel lacks and what the suite sends instead, for the results.
fn stand_ins(unsupported: &[String]) -> Value {
    let f = crate::model::Frame::at([0.0; 3]);
    let each = [
        Primitive::Box {
            frame: f,
            size: [1.0; 3],
        },
        Primitive::Cylinder {
            frame: f,
            radius: 1.0,
            height: 1.0,
        },
        Primitive::Sphere {
            frame: f,
            radius: 1.0,
        },
        Primitive::Torus {
            frame: f,
            major: 2.0,
            minor: 1.0,
        },
        Primitive::Cone {
            frame: f,
            bottom: 1.0,
            top: 0.0,
            height: 1.0,
        },
    ];
    let pairs = each.iter().filter_map(|p| {
        Some((
            p.kind().to_string(),
            json!(stand_in(unsupported, p)?.kind()),
        ))
    });
    Value::Object(pairs.collect())
}

/// Builds an operand with the kernel: a primitive by its constructor, a boolean from its operands.
fn build<K: Kernel>(k: &mut K, s: &Shape) -> Result<K::Solid, Refusal> {
    match s {
        Shape::Prim(p) => construct(k, p),
        Shape::Bool(op, a, b) => {
            let (a, b) = (build(k, a)?, build(k, b)?);
            k.boolean(*op, &a, &b)
        }
    }
}

/// The suite's estimate of how much of `bx` the predicate holds, sampled at `SAMPLES` points:
/// the estimate, the sampling allowance, and the points with their truth.
fn sampled(bx: oracle::Bounds, inside: impl Fn(Vec3) -> bool) -> (f64, f64, Vec<Vec3>, Vec<bool>) {
    sampled_n(bx, SAMPLES, inside)
}

fn sampled_n(
    bx: oracle::Bounds,
    n: usize,
    inside: impl Fn(Vec3) -> bool,
) -> (f64, f64, Vec<Vec3>, Vec<bool>) {
    let pts = oracle::samples(bx, n);
    let truth: Vec<bool> = pts.iter().map(|&p| inside(p)).collect();
    let n = pts.len() as f64;
    let f = truth.iter().filter(|&&t| t).count() as f64 / n;
    let bv = oracle::volume(bx);
    // Five standard errors of a random sample bound the even one's, with a floor of two points'
    // share for shapes the sample barely meets.
    let allow = 5.0 * bv * (f * (1.0 - f) / n).sqrt() + 2.0 * bv / n;
    (f * bv, allow, pts, truth)
}

/// What the runner keeps of a result: its measure, and its moments or why the kernel refused them.
struct Got {
    m: Measure,
    mass: Result<mass::Moments, Refusal>,
    /// The kernel offers centroid and inertia and still gave no moments: that refuses the case.
    mass_refused: bool,
}

impl std::ops::Deref for Got {
    type Target = Measure;
    fn deref(&self) -> &Measure {
        &self.m
    }
}

fn got<K: Kernel>(k: &mut K, s: &K::Solid) -> Got {
    let m = k.measure(s);
    let mass = (|| {
        let (c, j) = (k.centroid(s)?, k.inertia(s)?);
        Ok(mass::Moments::from_mass(m.volume, c, j))
    })();
    let offered = !lacks(k).iter().any(|u| u == "centroid" || u == "inertia");
    Got {
        m,
        mass_refused: offered && mass.is_err(),
        mass,
    }
}

struct Check {
    clause: &'static str,
    ok: bool,
    detail: String,
    tier: &'static str,
    measures: Value,
    counts: Value,
}

impl Check {
    /// A check of `clause` with verdict `ok`, taking what was weighed and counted since the last.
    fn taken(clause: &'static str, ok: bool, note: &str) -> Check {
        let tier = tier_of(ok);
        let (measures, counts, detail) = weighed(note);
        Check {
            clause,
            ok,
            detail,
            tier,
            measures,
            counts,
        }
    }

    fn to_json(&self) -> Value {
        json!({"clause": self.clause, "ok": self.ok, "tier": self.tier,
               "measures": self.measures, "counts": self.counts, "detail": self.detail})
    }
}

/// The bound a measured volume must meet: geometry within the tolerance moves volume by at most the
/// tolerance times the surface area, with a factor for measuring and a relative floor for rounding.
/// The suite's linear tolerance, mm: every kernel is judged against it, whatever it declares.
pub const TOLERANCE: f64 = 1e-7;

/// The tolerances a case is judged at, tightest first, mm. A case's tier is the first at which
/// every one of its checks holds.
pub const TIERS: [f64; 4] = [TOLERANCE, 1e-6, 1e-5, 1e-3];

thread_local! {
    /// The worst error the current case has shown, as a multiple of its bound.
    static WORST: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    /// The tolerance bounds are read at: the suite's, except inside [`tiers`].
    static EPS: std::cell::Cell<f64> = const { std::cell::Cell::new(TOLERANCE) };
    /// What the last [`tiers`] found, until the check it feeds is recorded.
    static JUDGED: std::cell::Cell<Option<[bool; 4]>> = const { std::cell::Cell::new(None) };
    /// What the current check has weighed at the suite's tolerance.
    static MEASURES: std::cell::RefCell<Vec<Weighed>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// What the current check has counted.
    static COUNTS: std::cell::RefCell<Vec<Counted>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// A quantity a check weighs: its name and its unit.
type Quantity = (&'static str, &'static str);
const VOLUME: Quantity = ("volume", "mm³");
const AREA: Quantity = ("area", "mm²");
const LENGTH: Quantity = ("distance", "mm");
const FIRST: Quantity = ("first moment", "mm⁴");
const SECOND: Quantity = ("second moment", "mm⁵");

/// One measure a check weighed: the error in a quantity against its bound, with the value the
/// kernel gave and the one it should have where the check has both.
#[derive(Clone, Copy, Debug)]
struct Weighed {
    quantity: Quantity,
    ends: Option<(f64, f64)>,
    error: f64,
    bound: f64,
}

impl Weighed {
    fn to_json(self) -> Value {
        let mut v = json!({"measure": self.quantity.0, "unit": self.quantity.1,
                           "error": self.error, "bound": self.bound});
        if let Some((got, want)) = self.ends {
            v["got"] = json!(got);
            v["want"] = json!(want);
        }
        v
    }

    fn sentence(self) -> String {
        let (name, unit) = self.quantity;
        let off = format!(
            "off by {:.3e} of {:.3e} {unit} allowed",
            self.error, self.bound
        );
        match self.ends {
            Some((got, want)) => format!("{name} {got:.9} against {want:.9}, {off}"),
            None => format!("{name} {off}"),
        }
    }
}

/// One count a check took: how many of something there were, and the most there may be, or,
/// where `exact`, how many there must be.
#[derive(Clone, Copy, Debug)]
struct Counted {
    what: &'static str,
    got: usize,
    most: usize,
    exact: bool,
}

impl Counted {
    fn to_json(self) -> Value {
        let limit = if self.exact { "want" } else { "most" };
        json!({"count": self.what, "got": self.got, limit: self.most})
    }

    fn sentence(self) -> String {
        match self.exact {
            true => format!("{} {}, {} expected", self.got, self.what, self.most),
            false => format!("{} {}, {} allowed", self.got, self.what, self.most),
        }
    }
}

/// A bound on an error that grows with the tolerance: `slope · ε + floor`.
#[derive(Clone, Copy, Debug)]
struct Bound {
    slope: f64,
    floor: f64,
}

impl Bound {
    /// A bound of `slope · ε + floor`.
    fn linear(slope: f64, floor: f64) -> Bound {
        Bound { slope, floor }
    }

    /// The bound at the tolerance now in force.
    fn now(self) -> f64 {
        self.slope * EPS.with(|e| e.get()) + self.floor
    }
}

impl From<f64> for Bound {
    fn from(floor: f64) -> Bound {
        Bound { slope: 0.0, floor }
    }
}

impl std::ops::Mul<f64> for Bound {
    type Output = Bound;
    fn mul(self, k: f64) -> Bound {
        Bound {
            slope: self.slope * k,
            floor: self.floor * k,
        }
    }
}

impl<B: Into<Bound>> std::ops::Add<B> for Bound {
    type Output = Bound;
    fn add(self, other: B) -> Bound {
        let other = other.into();
        Bound {
            slope: self.slope + other.slope,
            floor: self.floor + other.floor,
        }
    }
}

/// Whether `got` meets `want` within `bound`, both recorded with the error.
fn meets(quantity: Quantity, got: f64, want: f64, bound: impl Into<Bound>) -> bool {
    let ok = within(quantity, got - want, bound);
    if EPS.with(|e| e.get()) == TOLERANCE {
        MEASURES.with(|m| {
            if let Some(last) = m.borrow_mut().last_mut() {
                last.ends = Some((got, want));
            }
        });
    }
    ok
}

/// Whether there are at most `most` of `what`, the count recorded. A count is not a measure: a
/// check that fails on one judged the shape, not a number.
fn tally(what: &'static str, got: usize, most: usize) -> bool {
    if EPS.with(|e| e.get()) == TOLERANCE {
        COUNTS.with(|c| {
            c.borrow_mut().push(Counted {
                what,
                got,
                most,
                exact: false,
            })
        });
    }
    got <= most
}

/// Whether there are exactly `want` of `what`, the count recorded.
fn same(what: &'static str, got: usize, want: usize) -> bool {
    if EPS.with(|e| e.get()) == TOLERANCE {
        COUNTS.with(|c| {
            c.borrow_mut().push(Counted {
                what,
                got,
                most: want,
                exact: true,
            })
        });
    }
    got == want
}

/// Whether a solid's Betti numbers are the ones expected: its pieces, tunnels and voids, each
/// count recorded.
fn betti_same(got: [u64; 3], want: [u64; 3]) -> bool {
    let each = ["pieces", "tunnels", "voids"];
    // Every count is recorded, so none is skipped once one differs.
    let mut ok = true;
    for i in 0..3 {
        ok &= same(each[i], got[i] as usize, want[i] as usize);
    }
    ok
}

/// Whether the kernel's checker found nothing wrong, the number of its issues recorded.
fn valid(issues: &[String]) -> bool {
    tally("checker issues", issues.len(), 0)
}

/// Whether `error` in `quantity` is within `bound`, noting how far over it the case has gone.
fn within(quantity: Quantity, error: f64, bound: impl Into<Bound>) -> bool {
    let bound = bound.into().now();
    if EPS.with(|e| e.get()) == TOLERANCE {
        let ratio = error.abs() / bound;
        WORST.with(|w| {
            w.set(
                w.get()
                    .max(if ratio.is_nan() { f64::INFINITY } else { ratio }),
            )
        });
        MEASURES.with(|m| {
            m.borrow_mut().push(Weighed {
                quantity,
                ends: None,
                error,
                bound,
            })
        });
    }
    error.abs() <= bound
}

/// A check's verdict at each of [`TIERS`], and its verdict at the suite's tolerance as the value.
/// The check recorded next takes its tier from this; `ok` must read its bounds through [`within`]
/// and call no kernel.
fn tiers(ok: impl Fn() -> bool) -> bool {
    let mut at = [false; 4];
    for (i, eps) in TIERS.iter().enumerate().rev() {
        EPS.with(|e| e.set(*eps));
        at[i] = ok();
    }
    JUDGED.with(|j| {
        let before = j.get().unwrap_or([true; 4]);
        j.set(Some(std::array::from_fn(|i| before[i] && at[i])));
    });
    at[0]
}

/// The tier of the check being recorded with verdict `ok`: the tightest tolerance it holds at.
fn tier_of(ok: bool) -> &'static str {
    let at = JUDGED.with(|j| j.take()).unwrap_or([ok; 4]);
    // A verdict the tolerance did not decide holds at every tier or at none.
    let at = if at[0] == ok { at } else { [ok; 4] };
    match at.iter().position(|&held| held) {
        Some(0) => "1e-7",
        Some(1) => "1e-6",
        Some(2) => "1e-5",
        Some(3) => "1e-3",
        _ => "fail",
    }
}

/// What was weighed and counted since the last check, as its record carries them, and the
/// check's sentence: `note`, which says what the check was of, then every measure and count.
fn weighed(note: &str) -> (Value, Value, String) {
    let measures = MEASURES.with(|m| std::mem::take(&mut *m.borrow_mut()));
    let counts = COUNTS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let said = measures
        .iter()
        .map(|w| w.sentence())
        .chain(counts.iter().map(|c| c.sentence()));
    let note = note.trim_end_matches([' ', ':', ';']);
    let detail = std::iter::once(note.to_string())
        .filter(|n| !n.is_empty())
        .chain(said)
        .collect::<Vec<_>>()
        .join("; ");
    (
        measures.iter().map(|w| w.to_json()).collect(),
        counts.iter().map(|c| c.to_json()).collect(),
        detail,
    )
}

/// One check's record: its clause, verdict, tier, the measures it weighed and a sentence.
fn record(clause: &str, ok: bool, note: impl Into<String>) -> Value {
    let tier = tier_of(ok);
    let (measures, counts, detail) = weighed(&note.into());
    json!({"clause": clause, "ok": ok, "tier": tier, "measures": measures, "counts": counts,
           "detail": detail})
}

fn volume_bound(area: f64, volume: f64) -> Bound {
    Bound::linear(10.0 * area, 1e-9 * volume.abs())
}

fn area_bound(area: f64) -> f64 {
    1e-6 * area.abs() + 1e-9
}

/// [`area_bound`] of a solid of `volume` whose geometry may lie the tolerance off: moved out by
/// `ε` a convex body gains at most `(2/3)·A²/V·ε` of area (Minkowski's `A² ≥ 3V∫H dA`), which on
/// a part microns across is far above the relative bound.
fn area_bound_of(area: f64, volume: f64) -> Bound {
    let slope = if volume > 0.0 {
        10.0 * area * area / volume
    } else {
        0.0
    };
    Bound::linear(slope, area_bound(area))
}

fn b0_primitive(
    check: &mut impl FnMut(&'static str, bool, String),
    name: &str,
    got: &Measure,
    want: &Primitive,
    vb: Bound,
) {
    let (dv, da) = (got.volume - want.volume(), got.area - want.area());
    check(
        "B0",
        tiers(|| within(VOLUME, dv, vb) && within(AREA, da, area_bound(want.area()))),
        name.to_string(),
    );
}

/// Runs one feature case: the blend of `c.a`'s edges, judged against the solid `c.b` it must give.
/// Runs a sound case: the act's result is open within limits, so it is checked against those
/// (Q1–Q5) and not against one answer.
fn run_sound<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    f: &Feature,
    sound: &Sound,
) -> Value {
    let mut ops: Vec<Value> = Vec::new();
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(), "sound": true,
               "a": c.a.to_json(), "b": c.b.to_json(), "feature": f.to_json(),
               "checks": checks, "ops": ops})
    };
    let Shape::Prim(base) = &c.a else {
        return header(Outcome::Crash, ops, vec![]);
    };
    if lacks(k).iter().any(|u| u == f.act.word()) {
        return header(Outcome::Unsupported, ops, vec![]);
    }
    let mut checks: Vec<Value> = Vec::new();
    let mut check = |clause: &str, ok: bool, detail: String| {
        checks.push(record(clause, ok, detail));
    };
    let t = Instant::now();
    let made = attempt(|| {
        let a = construct(k, base)?;
        act(k, &a, f)
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    // Q1: the kernel completes the act.
    let s = match made {
        Ok(s) => s,
        Err(e) => {
            ops.push(
                json!({"op": f.act.word(), "ms": ms, "outcome": e.outcome().word(),
                            "error": e.message()}),
            );
            check("Q1", false, e.message().to_string());
            return header(e.outcome(), ops, checks);
        }
    };
    export::keep("result", "main", &s);
    let m = k.measure(&s);
    ops.push(
        json!({"op": f.act.word(), "ms": ms, "outcome": "done", "volume": m.volume,
                    "area": m.area}),
    );
    check("Q1", true, String::new());
    let vb = volume_bound(base.area() + m.area, base.volume());
    // Q2: its own checker finds the result valid.
    check("Q2", valid(&m.issues), m.issues.join("; "));
    // Q3: the result has the pieces, tunnels and voids it must.
    if !lacks(k).iter().any(|u| u == "betti") {
        match attempt(|| k.betti(&s)) {
            Ok(b) => check("Q3", betti_same(b, sound.betti), String::new()),
            // A query the kernel has not built for these points leaves its check out.
            Err(e) if e.outcome() == Outcome::Unsupported => {}
            Err(e) => check("Q3", false, e.message().to_string()),
        }
    }
    // Q4: outside the corner, each edge is as a fillet of its own radius makes it.
    if let Some(zone) = &sound.zone {
        let outside = attempt(|| {
            let zone = construct(k, zone)?;
            let outside = k.difference(&s, &zone)?;
            k.volume(&outside)
        });
        match outside {
            Ok(v) => check(
                "Q4",
                tiers(|| meets(VOLUME, v, sound.outside, vb)),
                format!("outside the corner {v:.9} against {:.9}", sound.outside),
            ),
            // A query the kernel has not built for these points leaves its check out.
            Err(e) if e.outcome() == Outcome::Unsupported => {}
            Err(e) => check("Q4", false, e.message().to_string()),
        }
    }
    // Q6: the blend meets its faces tangentially where it must touch them.
    if !sound.contacts.is_empty() && !lacks(k).iter().any(|u| u == "normal") {
        let pts: Vec<Vec3> = sound.contacts.iter().map(|c| c.0).collect();
        let want: Vec<(Vec3, Option<Vec3>)> =
            sound.contacts.iter().map(|&(p, n)| (p, Some(n))).collect();
        match attempt(|| k.normal(&s, &pts)) {
            Ok(got) => {
                let off = normals_off(&got, &want);
                check(
                    "Q6",
                    tally("places off", off.len(), 0),
                    format!(
                        "{} of {} contacts off: {}",
                        off.len(),
                        pts.len(),
                        off.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
                    ),
                );
            }
            // A query the kernel has not built for these points leaves its check out.
            Err(e) if e.outcome() == Outcome::Unsupported => {}
            Err(e) => check("Q6", false, e.message().to_string()),
        }
    }
    // Q8: by its seams the blend bends as what it meets does.
    if !sound.probes.is_empty()
        && let Some((ok, detail)) = probes_check(k, &s, &sound.probes)
    {
        check("Q8", ok, detail);
    }
    // Q9: the result holds none of the points it must leave.
    if !sound.away.is_empty() {
        match placed(k, &s, &sound.away) {
            Some(Ok(placed)) => {
                let held = placed.iter().filter(|&&b| b).count();
                check(
                    "Q9",
                    tally("points held that must not be", held, 0),
                    format!("{held} of {} points held", sound.away.len()),
                );
            }
            Some(Err(why)) => check("Q9", false, why),
            None => {}
        }
    }
    // Q10: point for point, the result holds what every acceptable one does and no more than
    // any does.
    if let Some(limits) = &sound.limits {
        let within = sound
            .zone
            .as_ref()
            .and_then(|z| oracle::bounds(&Shape::Prim(z.clone())))
            .or_else(|| oracle::bounds(&c.a));
        match within.map(|bx| attempt(|| crate::sound::between(k, &s, limits, bx))) {
            Some(Ok((ok, detail))) => check("Q10", ok, detail),
            Some(Err(e)) if e.outcome() == Outcome::Unsupported => {}
            Some(Err(e)) => check("Q10", false, e.message().to_string()),
            None => {}
        }
    }
    // Q11 to Q13: across the blend the boundary has no crease; its curvature carries on, where
    // the blend is to carry it; and it is unchanged by what leaves the case unchanged.
    if !sound.scans.is_empty() && offers(k, &["raycast", "normal"], &[]) {
        for (clause, ok, detail) in scan_checks(k, &s, &sound.scans) {
            check(clause, ok, detail);
        }
    }
    // Q5: the result lies between the fillets at the corner's largest radius and its smallest.
    check(
        "Q5",
        tiers(|| m.volume >= sound.least - vb.now() && m.volume <= sound.most + vb.now()),
        format!(
            "volume {:.9} against [{:.9}, {:.9}]",
            m.volume, sound.least, sound.most
        ),
    );
    let ok = checks.iter().all(|c| c["ok"] == json!(true));
    header(if ok { Outcome::Pass } else { Outcome::Fail }, ops, checks)
}

/// What the scans across a result's blend find: Q11, no crease; Q12, where a scan asks, no jump
/// in curvature; Q13, where a scan names motions, no difference under them.
fn scan_checks<K: Kernel>(
    k: &mut K,
    s: &K::Solid,
    scans: &[crate::generate::Scan],
) -> Vec<(&'static str, bool, String)> {
    let mut all = crate::sound::Found::default();
    for scan in scans {
        match attempt(|| crate::sound::scan(k, s, scan)) {
            Ok(f) => {
                all.creases.extend(f.creases);
                all.jumps.extend(f.jumps);
                all.lopsided += f.lopsided;
                all.compared += f.compared;
                all.rays += f.rays;
                all.blind += f.blind;
                all.seen += f.seen;
            }
            // Scans the kernel's queries do not reach, as a normal on an edge, are left out.
            Err(e) if e.outcome() == Outcome::Unsupported => return Vec::new(),
            Err(e) => return vec![("Q11", false, e.message().to_string())],
        }
    }
    let at = |p: &[Vec3]| p.first().map_or(String::new(), |p| format!(" at {p:?}"));
    // A ray seldom lands on an edge, where a kernel may have no answer: one that has none at
    // more than a fiftieth of the points cannot show its blend smooth.
    let shown = 50 * all.blind <= all.seen;
    let mut out = vec![(
        "Q11",
        all.creases.is_empty() && shown,
        format!(
            "{} creases{} by {} rays, {} of {} points without an answer",
            all.creases.len(),
            at(&all.creases),
            all.rays,
            all.blind,
            all.seen
        ),
    )];
    if scans.iter().any(|sc| sc.curvature) {
        out.push((
            "Q12",
            all.jumps.is_empty(),
            format!("{} jumps in curvature{}", all.jumps.len(), at(&all.jumps)),
        ));
    }
    if all.compared > 0 {
        out.push((
            "Q13",
            all.lopsided == 0,
            format!(
                "{} of {} rays stop elsewhere once moved",
                all.lopsided, all.compared
            ),
        ));
    }
    out
}

/// One act on a solid, as the feature it names asks.
fn act<K: Kernel>(k: &mut K, s: &K::Solid, f: &Feature) -> Result<K::Solid, Refusal> {
    match f.act {
        Act::Fillet => k.fillet(s, &f.points, f.size),
        Act::OffsetFaces => k.offset_faces(s, &f.points, f.size),
        Act::DeleteFaces => k.delete_faces(s, &f.points),
        Act::MoveFaces => {
            let m = f.matrix.ok_or("a face move needs its motion")?;
            k.move_faces(s, &f.points, m)
        }
        Act::Thicken => k.thicken(s, &f.points, f.size),
        Act::FilletRadii => k.fillet_radii(s, &f.points, &f.radii),
        Act::ChamferDistances => k.chamfer_distances(s, &f.points, &f.radii),
        Act::FilletVariable => {
            let (&[edge, from], &[r_to]) = (f.points.as_slice(), f.radii.as_slice()) else {
                return Err(Refusal::invalid(
                    "a variable fillet names an edge and its starting end",
                ));
            };
            k.fillet_variable(s, edge, from, f.size, r_to)
        }
        Act::ChamferTwo => {
            let (&[edge, face], &[d2]) = (f.points.as_slice(), f.radii.as_slice()) else {
                return Err(Refusal::invalid(
                    "a chamfer of two distances names an edge and a face",
                ));
            };
            k.chamfer_two(s, edge, face, f.size, d2)
        }
        Act::Chamfer => k.chamfer(s, &f.points, f.size),
        Act::FilletG2 => {
            let &[center] = f.radii.as_slice() else {
                return Err(Refusal::invalid("a G2 fillet names its center radius"));
            };
            k.fillet_g2(s, &f.points, f.size, center)
        }
        Act::FilletSetbackG2 => {
            let &[setback] = f.radii.as_slice() else {
                return Err(Refusal::invalid("a set-back fillet names its setback"));
            };
            k.fillet_setback_g2(s, &f.points, f.size, setback)
        }
        Act::FilletConic => {
            let &[rho] = f.radii.as_slice() else {
                return Err(Refusal::invalid("a conic fillet names its rho"));
            };
            k.fillet_conic(s, &f.points, f.size, rho)
        }
        Act::FilletSetback => {
            let &[setback] = f.radii.as_slice() else {
                return Err(Refusal::invalid("a set-back fillet names its setback"));
            };
            k.fillet_setback(s, &f.points, f.size, setback)
        }
        Act::FilletFaces => {
            let &[a, b] = f.points.as_slice() else {
                return Err(Refusal::invalid("a face fillet names two faces"));
            };
            k.fillet_faces(s, a, b, f.size)
        }
        Act::FullRound => {
            let &[a, m, b] = f.points.as_slice() else {
                return Err(Refusal::invalid(
                    "a full round names a side, the middle face and a side",
                ));
            };
            k.full_round(s, a, m, b)
        }
        Act::Offset => k.offset(s, f.size),
        Act::Shell => k.shell(s, &f.points, f.size),
        Act::Draft => {
            let (o, d) = f.pull.ok_or("a draft needs its neutral plane")?;
            k.draft(s, &f.points, o, d, f.size)
        }
    }
}

fn run_feature<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, f: &Feature) -> Value {
    if let Some(sound) = &c.exact.sound {
        return run_sound(k, c, f, sound);
    }
    let mut ops: Vec<Value> = Vec::new();
    let mut worst = Outcome::Pass;
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(),
               "a": c.a.to_json(), "b": c.b.to_json(), "feature": f.to_json(),
               "checks": checks, "ops": ops})
    };
    let Shape::Prim(base) = &c.a else {
        return header(Outcome::Crash, ops, vec![]);
    };
    if lacks(k).iter().any(|u| u == f.act.word()) {
        return header(Outcome::Unsupported, ops, vec![]);
    }
    let a = match attempt(|| construct(k, base)) {
        Ok(a) => a,
        Err(e) => {
            let mut v = header(e.outcome(), ops, vec![]);
            v["error"] = json!(format!("making the base: {}", e.message()));
            return v;
        }
    };
    if c.refuse {
        // B18: an act the input does not allow must be refused.
        let r = attempt(|| act(k, &a, f).map(|_| ()));
        let (outcome, detail) = match &r {
            Ok(()) => (
                Outcome::Fail,
                "accepted an act the input does not allow".to_string(),
            ),
            // A kernel that says it cannot do this has not judged the input.
            Err(Ran::Refused(e)) if e.kind != Kind::Unsupported => {
                (Outcome::Pass, format!("refused, {e}"))
            }
            Err(e) => (e.outcome(), e.message()),
        };
        let checks = vec![record("B18", outcome == Outcome::Pass, detail)];
        return header(outcome, ops, checks);
    }
    // The shape the act must give: one primitive, or a built shape with its closed forms stated.
    let want_prim = match &c.b {
        Shape::Prim(p) => Some(p),
        Shape::Bool(..) => None,
    };
    let want_volume = want_prim.map_or(c.exact.result_volume.unwrap_or(f64::NAN), |p| p.volume());
    let want_area = want_prim.map_or(c.exact.result_area.unwrap_or(f64::NAN), |p| p.area());
    let mut record = |name: &str, r: Result<Got, Ran>, ms: f64| -> Option<Got> {
        let mut o = json!({"op": name, "ms": ms});
        let out = match r {
            Ok(g) => {
                o["outcome"] = json!("done");
                o["volume"] = json!(g.volume);
                o["area"] = json!(g.area);
                if !g.issues.is_empty() {
                    o["issues"] = json!(g.issues);
                }
                if let Err(e) = &g.mass {
                    o["mass_error"] = json!(e.to_string());
                }
                if g.mass_refused {
                    worst = worst.max(Outcome::Refused);
                }
                Some(g)
            }
            Err(r) => {
                o["outcome"] = json!(r.outcome().word());
                o["error"] = json!(r.message());
                // The round trip through STEP is a check on the side: a kernel that cannot read
                // this file back is left out of that check, and judged on the rest.
                let aside = is_round_trip(name) && r.outcome() == Outcome::Unsupported;
                if !aside {
                    worst = worst.max(r.outcome());
                }
                None
            }
        };
        ops.push(o);
        out
    };
    let t = Instant::now();
    export::keep("before", "main", &a);
    let r = attempt(|| {
        act(k, &a, f).map(|s| {
            export::keep("after", "main", &s);
            got(k, &s)
        })
    });
    let res = record(f.act.word(), r, t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let r = attempt(|| {
        let moved = construct(k, &base.moved(&c.motion))?;
        act(k, &moved, &f.moved(&c.motion)).map(|s| got(k, &s))
    });
    let moved = record("moved", r, t.elapsed().as_secs_f64() * 1e3);
    let step = !lacks(k)
        .iter()
        .any(|u| u == "write_step" || u == "read_step");
    let through = step
        .then(|| {
            let t = Instant::now();
            let r = attempt(|| {
                let s = act(k, &a, f)?;
                let text = k.write_step(&s)?;
                let read = k.read_step(&text)?;
                Ok(got(k, &read))
            });
            record("through STEP", r, t.elapsed().as_secs_f64() * 1e3)
        })
        .flatten();
    let after = attempt(|| Ok(k.measure(&a)));

    let mut checks: Vec<Check> = Vec::new();
    let mut check = |clause: &'static str, ok: bool, note: String| {
        checks.push(Check::taken(clause, ok, &note));
    };
    let area = base.area() + want_area;
    let vb = volume_bound(area, base.volume() + want_volume);
    let bx = oracle::bounds(&c.a).zip(oracle::bounds(&c.b));
    let reach = bx
        .map(|(x, y)| {
            [x.0, x.1, y.0, y.1]
                .iter()
                .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0)
        * 3f64.sqrt();
    if let Some(g) = &res {
        check("B1", valid(&g.issues), g.issues.join("; "));
        // B14: the result is the blended solid.
        let (dv, da) = (g.volume - want_volume, g.area - want_area);
        check(
            "B14",
            tiers(|| within(VOLUME, dv, vb) && within(AREA, da, area_bound(want_area))),
            String::new(),
        );
        if let (Ok(m), Some(want)) = (&g.mass, want_prim) {
            let (d1, d2) = m.gap(&mass::of(want));
            check(
                "B14",
                tiers(|| within(FIRST, d1, vb * reach) && within(SECOND, d2, vb * reach * reach)),
                "moments".to_string(),
            );
        }
        // B15: the kernel places sample points as the blended solid holds them.
        if let Some((x, y)) = bx {
            let lo = [0, 1, 2].map(|i| x.0[i].min(y.0[i]));
            let hi = [0, 1, 2].map(|i| x.1[i].max(y.1[i]));
            let pts = oracle::samples((lo, hi), SAMPLES);
            let truth: Vec<bool> = pts.iter().map(|&p| oracle::contains(&c.b, p)).collect();
            let blended = attempt(|| act(k, &a, f)).ok();
            match blended.and_then(|s| placed(k, &s, &pts)) {
                Some(Ok(gotten)) => {
                    let off = gotten.iter().zip(&truth).filter(|(g, t)| g != t).count();
                    check(
                        "B15",
                        tally("sample points placed wrongly", off, 1),
                        format!(
                            "{off} of {} sample points against the blended solid",
                            pts.len()
                        ),
                    );
                }
                Some(Err(why)) => check("B15", false, why),
                None => {}
            }
        }
        // B16: blending the moved solid at the moved points gives the result moved.
        if let (Some(mg), Ok(m)) = (&moved, &g.mass) {
            check(
                "B16",
                tiers(|| meets(VOLUME, mg.volume, g.volume, vb)),
                "moved".to_string(),
            );
            if let Ok(mm) = &mg.mass {
                let t = c.motion.t;
                let far = reach + (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
                let (d1, d2) = mm.gap(&m.moved(&c.motion));
                check(
                    "B16",
                    tiers(|| within(FIRST, d1, vb * far) && within(SECOND, d2, vb * far * far)),
                    "moved moments".to_string(),
                );
            }
        }
    }
    // B17: the result written as STEP and read back is the same valid solid.
    if let (Some(g), Some(r)) = (&res, &through) {
        let (dv, da) = (r.volume - g.volume, r.area - g.area);
        let ok = tiers(|| {
            valid(&r.issues) && within(VOLUME, dv, vb) && within(AREA, da, area_bound(g.area))
        });
        check("B17", ok, "through STEP: ".to_string());
    }
    // B31: where a fillet meets a face the boundary is smooth, its normal the face's; where a
    // chamfer or an unblended corner meets one it is a crease, with none. A blend the kernel
    // refused has no boundary to read: its refusal is scored once, as the act's.
    let built = res.is_some();
    let blend = built
        && matches!(
            f.act,
            Act::Fillet
                | Act::Chamfer
                | Act::ChamferTwo
                | Act::FilletConic
                | Act::FilletFaces
                | Act::FullRound
        );
    let joints = want_prim.map(oracle::joints).unwrap_or_default();
    if blend && !joints.is_empty() && !lacks(k).iter().any(|u| u == "normal") {
        let pts: Vec<Vec3> = joints.iter().map(|j| j.0).collect();
        match attempt(|| {
            let s = act(k, &a, f)?;
            k.normal(&s, &pts)
        }) {
            Ok(got) => {
                let off = normals_off(&got, &joints);
                check(
                    "B31",
                    tally("places off", off.len(), 0),
                    format!(
                        "{} of {} joints off: {}",
                        off.len(),
                        pts.len(),
                        off.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
                    ),
                );
            }
            // A query the kernel has not built for these points leaves its check out.
            Err(e) if e.outcome() == Outcome::Unsupported => {}
            Err(e) => check("B31", false, e.message().to_string()),
        }
    }
    // B39: by its seams the blend bends as what it meets does.
    if built && !c.exact.probes.is_empty() {
        match attempt(|| act(k, &a, f)) {
            Ok(s) => {
                if let Some((ok, detail)) = probes_check(k, &s, &c.exact.probes) {
                    check("B39", ok, detail);
                }
            }
            // A query the kernel has not built for these points leaves its check out.
            Err(e) if e.outcome() == Outcome::Unsupported => {}
            Err(e) => check("B39", false, e.message().to_string()),
        }
    }
    // B7: the blend leaves its operand as it was.
    if let Ok(m) = &after {
        let ok = tiers(|| meets(VOLUME, m.volume, base.volume(), vb));
        check("B7", ok, "the base's volume after the blend".into());
    }
    if checks.iter().any(|c| !c.ok) {
        worst = worst.max(Outcome::Fail);
    }
    let checks = checks.iter().map(Check::to_json).collect();
    header(worst, ops, checks)
}

/// Runs one case and returns its result object.
/// The constructor names a shape calls.
fn kinds(s: &Shape) -> Vec<String> {
    match s {
        Shape::Prim(p) => vec![p.to_json()["kind"].as_str().unwrap_or("?").to_string()],
        Shape::Bool(_, a, b) => [kinds(a), kinds(b)].concat(),
    }
}

/// Runs one STEP-corpus case: the suite's file for `c.a`, read by the kernel, must be `c.a`.
fn run_step<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case) -> Value {
    let Shape::Prim(p) = &c.a else {
        return json!({"id": c.id, "family": c.family, "outcome": "crash", "checks": [], "ops": []});
    };
    let schema = match c.exact.step_as {
        None => "AP214",
        Some(crate::generate::StepAs::Schema(crate::step::Schema::Ap203)) => "AP203",
        Some(crate::generate::StepAs::Schema(crate::step::Schema::Ap242)) => "AP242",
        Some(crate::generate::StepAs::Tessellated) => "AP242 tessellated",
    };
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(), "a": c.a.to_json(),
               "b": c.b.to_json(), "checks": checks, "ops": ops, "schema": schema})
    };
    if lacks(k).iter().any(|u| u == "read_step") {
        return header(Outcome::Unsupported, vec![], vec![]);
    }
    let text = match c.exact.step_as {
        None => crate::step::write(p),
        Some(crate::generate::StepAs::Schema(s)) => crate::step::write(p).map(|t| s.apply(&t)),
        Some(crate::generate::StepAs::Tessellated) => crate::step::tessellated(p),
    };
    let Some(text) = text else {
        return header(Outcome::Crash, vec![], vec![]);
    };
    let t = Instant::now();
    let read = attempt(|| k.read_step(&text));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let s = match read {
        Ok(s) => s,
        Err(e) => {
            let op = json!({"op": "read_step", "outcome": e.outcome().word(), "ms": ms,
                            "error": e.message()});
            return header(e.outcome(), vec![op], vec![]);
        }
    };
    export::keep("read", "main", &s);
    let g = got(k, &s);
    let ops = vec![
        json!({"op": "read_step", "outcome": "done", "ms": ms, "volume": g.volume,
                          "area": g.area}),
    ];
    let vb = volume_bound(p.area(), p.volume());
    let (dv, da) = (g.volume - p.volume(), g.area - p.area());
    let mut checks = vec![record(
        "B22",
        tiers(|| {
            valid(&g.issues) && within(VOLUME, dv, vb) && within(AREA, da, area_bound(p.area()))
        }),
        format!("read: ; {}", g.issues.join("; ")),
    )];
    if let Some(bx) = oracle::bounds(&c.a) {
        let pts = oracle::samples(bx, SAMPLES);
        match placed(k, &s, &pts) {
            Some(Ok(placed)) => {
                let off = pts
                    .iter()
                    .zip(&placed)
                    .filter(|(q, g)| oracle::contains(&c.a, **q) != **g)
                    .count();
                checks.push(record(
                    "B22",
                    tally("sample points placed wrongly", off, 1),
                    format!("{off} sample points placed differently"),
                ));
            }
            Some(Err(why)) => checks.push(record("B22", false, why)),
            None => {}
        }
    }
    let outcome = if checks.iter().all(|c| c["ok"] == json!(true)) {
        Outcome::Pass
    } else {
        Outcome::Fail
    };
    header(outcome, ops, checks)
}

/// Runs one transform case (B25): `c.a` moved by `m` holds the moved sample points, with its volume
/// and area scaled.
fn run_transform<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    m: [[f64; 4]; 3],
) -> Value {
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(), "a": c.a.to_json(),
               "matrix": m, "checks": checks, "ops": ops})
    };
    let Shape::Prim(p) = &c.a else {
        return header(Outcome::Crash, vec![], vec![]);
    };
    if lacks(k).iter().any(|u| u == "transform") {
        return header(Outcome::Unsupported, vec![], vec![]);
    }
    let factor = (m[0][0] * m[0][0] + m[1][0] * m[1][0] + m[2][0] * m[2][0]).sqrt();
    let pts = oracle::bounds(&c.a).map(|bx| oracle::samples(bx, 1024));
    let t = Instant::now();
    let r = attempt(|| {
        let a = construct(k, p)?;
        let s = k.transform(&a, m)?;
        export::keep("moved", "main", &s);
        let placed = pts.as_ref().map(|pts| {
            let qs: Vec<Vec3> = pts.iter().map(|&q| apply(&m, q)).collect();
            k.holds(&s, &qs)
        });
        Ok((k.measure(&s), placed))
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (got, placed) = match r {
        Ok(x) => x,
        Err(e) => {
            let op = json!({"op": "transform", "outcome": e.outcome().word(), "ms": ms,
                            "error": e.message()});
            return header(e.outcome(), vec![op], vec![]);
        }
    };
    let ops = vec![
        json!({"op": "transform", "outcome": "done", "ms": ms, "volume": got.volume,
                          "area": got.area}),
    ];
    let f3 = factor.powi(3);
    let vb = volume_bound(p.area(), p.volume()) * f3.max(1.0);
    let (dv, da) = (
        got.volume - f3 * p.volume(),
        got.area - factor * factor * p.area(),
    );
    let mut checks = vec![record(
        "B25",
        tiers(|| {
            valid(&got.issues)
                && within(VOLUME, dv, vb)
                && within(AREA, da, area_bound(factor * factor * p.area()))
        }),
        format!("×{factor}: ; {}", got.issues.join("; ")),
    )];
    match (placed, &pts) {
        (Some(Ok(placed)), Some(pts)) => {
            let off = pts
                .iter()
                .zip(&placed)
                .filter(|(q, g)| oracle::contains(&c.a, **q) != **g)
                .count();
            checks.push(record(
                "B25",
                tally("sample points placed wrongly", off, 1),
                format!("{off} of {} moved points placed differently", pts.len()),
            ));
        }
        (Some(Err(e)), _) if e.kind != Kind::Unsupported => {
            checks.push(record("B25", false, unplaced(&e.to_string())));
        }
        _ => {}
    }
    let outcome = if checks.iter().all(|c| c["ok"] == json!(true)) {
        Outcome::Pass
    } else {
        Outcome::Fail
    };
    header(outcome, ops, checks)
}

/// The singular values of `m`'s linear part, largest first: the square roots of `MᵀM`'s
/// eigenvalues, found by Jacobi rotations.
fn singular_values(m: &[[f64; 4]; 3]) -> [f64; 3] {
    let mut a = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            a[i][j] = (0..3).map(|k| m[k][i] * m[k][j]).sum();
        }
    }
    for _ in 0..50 {
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
            let (c, s) = (theta.cos(), theta.sin());
            let mut g = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            g[p][p] = c;
            g[q][q] = c;
            g[p][q] = s;
            g[q][p] = -s;
            let mut ga = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    ga[i][j] = (0..3).map(|k| g[k][i] * a[k][j]).sum();
                }
            }
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] = (0..3).map(|k| ga[i][k] * g[k][j]).sum();
                }
            }
        }
    }
    let mut e = [0, 1, 2].map(|i| a[i][i].max(0.0).sqrt());
    e.sort_by(|x, y| y.total_cmp(x));
    e
}

/// Runs one affine case (B32): `c.a` mapped by a stretch or shear `m` holds the mapped sample
/// points, with its volume and moments mapped exactly and its area between σ₂σ₃ and σ₁σ₂ times the
/// solid's, the least and most any surface element can grow by.
fn run_affine<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, m: [[f64; 4]; 3]) -> Value {
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(), "a": c.a.to_json(),
               "matrix": m, "checks": checks, "ops": ops})
    };
    let Shape::Prim(p) = &c.a else {
        return header(Outcome::Crash, vec![], vec![]);
    };
    if lacks(k).iter().any(|u| u == "affine") {
        return header(Outcome::Unsupported, vec![], vec![]);
    }
    let [s1, s2, s3] = singular_values(&m);
    let det = s1 * s2 * s3;
    let bx = oracle::bounds(&c.a);
    let pts = bx.map(|bx| oracle::samples(bx, 1024));
    let t = Instant::now();
    let r = attempt(|| {
        let a = construct(k, p)?;
        let s = k.affine(&a, m)?;
        export::keep("moved", "main", &s);
        let placed = pts.as_ref().map(|pts| {
            let qs: Vec<Vec3> = pts.iter().map(|&q| apply(&m, q)).collect();
            k.holds(&s, &qs)
        });
        Ok((got(k, &s), placed))
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (g, placed) = match r {
        Ok(x) => x,
        Err(e) => {
            let op = json!({"op": "affine", "outcome": e.outcome().word(), "ms": ms,
                            "error": e.message()});
            return header(e.outcome(), vec![op], vec![]);
        }
    };
    let ops = vec![
        json!({"op": "affine", "outcome": "done", "ms": ms, "volume": g.volume,
                          "area": g.area}),
    ];
    let (area, volume) = (p.area(), p.volume());
    let vb = volume_bound(s1 * s2 * area, det * volume);
    let dv = g.volume - det * volume;
    let (lo, hi) = (s2 * s3 * area, s1 * s2 * area);
    let slack = area_bound(hi);
    let mut checks = vec![record(
        "B32",
        tiers(|| valid(&g.issues) && within(VOLUME, dv, vb))
            && g.area >= lo - slack
            && g.area <= hi + slack,
        format!(
            "σ {s1:.3}, {s2:.3}, {s3:.3}: volume off by {dv:.3e}, area {:.6} \
                           against {lo:.6} to {hi:.6}; {}",
            g.area,
            g.issues.join("; ")
        ),
    )];
    if let (Ok(got_m), Some((lo, hi))) = (&g.mass, bx) {
        // The mapped box's corners bound how far the solid reaches from the origin.
        let reach = (0..8)
            .map(|n| {
                let q = [0, 1, 2].map(|i| if n >> i & 1 == 1 { hi[i] } else { lo[i] });
                let p = apply(&m, q);
                (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
            })
            .fold(0.0, f64::max);
        let (d1, d2) = got_m.gap(&mass::of(p).mapped(&m));
        checks.push(record(
            "B32",
            tiers(|| within(FIRST, d1, vb * reach) && within(SECOND, d2, vb * reach * reach)),
            "moments".to_string(),
        ));
    } else if g.mass_refused {
        checks.push(record(
            "B32",
            false,
            "offers centroid and inertia but gave none",
        ));
    }
    match (placed, &pts) {
        (Some(Ok(placed)), Some(pts)) => {
            let off = pts
                .iter()
                .zip(&placed)
                .filter(|(q, g)| oracle::contains(&c.a, **q) != **g)
                .count();
            checks.push(record(
                "B32",
                tally("sample points placed wrongly", off, 1),
                format!("{off} of {} mapped points placed differently", pts.len()),
            ));
        }
        (Some(Err(e)), _) if e.kind != Kind::Unsupported => {
            checks.push(record("B32", false, unplaced(&e.to_string())));
        }
        _ => {}
    }
    let outcome = if checks.iter().all(|c| c["ok"] == json!(true)) {
        Outcome::Pass
    } else {
        Outcome::Fail
    };
    header(outcome, ops, checks)
}

/// Makes `c.a`, or reports why it could not, after checking the kernel offers `function`.
fn made<K: Kernel>(k: &mut K, c: &Case, function: &str) -> Result<K::Solid, Value> {
    if lacks(k).iter().any(|u| u == function) {
        return Err(finish(c, Outcome::Unsupported, vec![], vec![]));
    }
    attempt(|| build(k, &c.a)).map_err(|e| {
        let mut v = finish(c, e.outcome(), vec![], vec![]);
        v["error"] = json!(format!("making the solid: {}", e.message()));
        v
    })
}

/// One op's record, and the case's result when it failed.
fn timed<T>(
    c: &Case,
    name: &str,
    f: impl FnOnce() -> Result<T, Refusal>,
) -> Result<(T, Value), Value> {
    let t = Instant::now();
    let r = attempt(f);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Ok(x) => Ok((x, json!({"op": name, "outcome": "done", "ms": ms}))),
        Err(e) => {
            let op = json!({"op": name, "outcome": e.outcome().word(), "ms": ms,
                            "error": e.message()});
            Err(finish(c, e.outcome(), vec![op], vec![]))
        }
    }
}

/// B24: a solid with itself. Its union and intersection are itself, its difference empty. The
/// one handle is both operands of each call.
fn run_itself<K: Kernel>(k: &mut K, c: &Case) -> Value {
    let a = match made(k, c, "union") {
        Ok(a) => a,
        Err(v) => return v,
    };
    let (whole, op) = match timed(c, "a", || Ok(k.measure(&a))) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let volume = match &c.a {
        Shape::Prim(p) => p.volume(),
        Shape::Bool(..) => whole.volume,
    };
    let vb = volume_bound(2.0 * whole.area, 2.0 * volume);
    let (mut ops, mut checks) = (vec![op], Vec::new());
    for (name, o, want) in [
        ("a ∪ a", Op::Union, volume),
        ("a ∩ a", Op::Intersection, volume),
        ("a ∖ a", Op::Difference, 0.0),
    ] {
        match timed(c, name, || k.boolean(o, &a, &a).map(|s| k.measure(&s))) {
            Ok((m, op)) => {
                ops.push(op);
                let dv = m.volume - want;
                checks.push(record(
                    "B24",
                    tiers(|| valid(&m.issues) && within(VOLUME, dv, vb)),
                    format!("{name}: volume off by {dv:.3e}; {}", m.issues.join("; ")),
                ));
            }
            Err(mut v) => {
                ops.extend(v["ops"].as_array().cloned().unwrap_or_default());
                v["ops"] = json!(ops);
                return v;
            }
        }
    }
    finish(c, Outcome::Pass, ops, checks)
}

/// B33: the box the kernel gives is the least round the solid, to within the tolerance.
fn run_bbox<K: Kernel>(k: &mut K, c: &Case, want: [Vec3; 2]) -> Value {
    let s = match made(k, c, "bounding_box") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let (got, op) = match timed(c, "bounding_box", || k.bounding_box(&s)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let off = (0..2)
        .flat_map(|j| (0..3).map(move |i| (j, i)))
        .map(|(j, i)| (got[j][i] - want[j][i]).abs())
        .fold(0.0, f64::max);
    let ok = tiers(|| within(LENGTH, off, Bound::linear(10.0, 0.0)));
    let checks = vec![record(
        "B33",
        ok,
        format!("{got:?} against {want:?}: off by {off:.3e}"),
    )];
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        checks,
    )
}

/// B40: the point of the solid the kernel gives as nearest each point is as far from it as the
/// closed form says, and is a point of the solid: the one nearest point where there is only one.
fn run_closest<K: Kernel>(k: &mut K, c: &Case) -> Value {
    let s = match made(k, c, "closest") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let asked: Vec<Vec3> = c.exact.closest.iter().map(|x| x.0).collect();
    let (got, op) = match timed(c, "closest", || k.closest(&s, &asked)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    if got.len() != asked.len() {
        let detail = format!("{} points for {} asked", got.len(), asked.len());
        return finish(
            c,
            Outcome::Fail,
            vec![op],
            vec![record("B40", false, detail)],
        );
    }
    let apart = |a: Vec3, b: Vec3| {
        let d = [0, 1, 2].map(|i| a[i] - b[i]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    // The worst of: a distance off the closed form's, and a point off the solid or off the one
    // nearest point.
    let mut worst = (0.0f64, String::new());
    for (&(q, d, only), &g) in c.exact.closest.iter().zip(&got) {
        let off_distance = (apart(q, g) - d).abs();
        let off_solid = match (only, &c.a) {
            (Some(e), _) => apart(g, e),
            (None, Shape::Prim(p)) => oracle::solid_distance(p, g).unwrap_or(f64::INFINITY),
            (None, _) => 0.0,
        };
        let off = off_distance.max(off_solid);
        if off >= worst.0 {
            worst = (
                off,
                format!("from {q:?}: {g:?}, {:.9} away against {d:.9}", apart(q, g)),
            );
        }
    }
    let ok = tiers(|| within(LENGTH, worst.0, Bound::linear(10.0, 0.0)));
    let detail = format!(
        "{} points, the worst off by {:.3e}: {}",
        got.len(),
        worst.0,
        worst.1
    );
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        vec![record("B40", ok, detail)],
    )
}

/// B41: a point of the boundary is on it, and a point clear of the boundary is in or out as the
/// solid holds it.
fn run_sides<K: Kernel>(k: &mut K, c: &Case) -> Value {
    let s = match made(k, c, "classify") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let asked: Vec<Vec3> = c.exact.sides.iter().map(|x| x.0).collect();
    let (got, op) = match timed(c, "classify", || k.classify(&s, &asked)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let wrong: Vec<String> = c
        .exact
        .sides
        .iter()
        .zip(&got)
        .filter(|((_, want), got)| want != *got)
        .map(|((q, want), got)| format!("{q:?} is {}, not {}", want.word(), got.word()))
        .collect();
    let ok = tiers(|| wrong.is_empty() && got.len() == asked.len());
    let detail = format!(
        "{} of {} points placed otherwise; {}",
        wrong.len(),
        asked.len(),
        wrong.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
    );
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        vec![record("B41", ok, detail)],
    )
}

/// B34: the pieces of a split are valid, their volumes add up to the solid's, and each holds the
/// sample points of the solid on its side of the plane and no others.
fn run_split<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, o: Vec3, n: Vec3) -> Value {
    let s = match made(k, c, "split") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let ((up, down), op) = match timed(c, "split", || k.split(&s, o, n)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    export::keep("pieces", "main", &up);
    export::keep("pieces", "alt", &down);
    let (whole, gu, gd) = (k.measure(&s), k.measure(&up), k.measure(&down));
    let mut checks = Vec::new();
    let issues: Vec<String> = gu.issues.iter().chain(&gd.issues).cloned().collect();
    let vb = volume_bound(whole.area, whole.volume);
    let dv = gu.volume + gd.volume - whole.volume;
    checks.push(record(
        "B34",
        tiers(|| issues.is_empty() && within(VOLUME, dv, vb)),
        format!(
            "pieces {:.9} and {:.9} of {:.9}: off by {dv:.3e}; {}",
            gu.volume,
            gd.volume,
            whole.volume,
            issues.join("; ")
        ),
    ));
    if let Some(bx) = oracle::bounds(&c.a) {
        let pts = oracle::samples(bx, SAMPLES);
        let side = |p: &Vec3| (0..3).map(|i| (p[i] - o[i]) * n[i]).sum::<f64>() > 0.0;
        for (piece, above, name) in [
            (&up, true, "the normal's side"),
            (&down, false, "the other"),
        ] {
            match placed(k, piece, &pts) {
                Some(Ok(placed)) => {
                    let off = pts
                        .iter()
                        .zip(&placed)
                        .filter(|(p, g)| (oracle::contains(&c.a, **p) && side(p) == above) != **g)
                        .count();
                    checks.push(record(
                        "B34",
                        tally("sample points placed wrongly", off, 1),
                        format!("{off} of {} points placed differently on {name}", pts.len()),
                    ));
                }
                Some(Err(why)) => checks.push(record("B34", false, format!("{name}: {why}"))),
                None => {}
            }
        }
    }
    let ok = checks.iter().all(|c| c["ok"] == json!(true));
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        checks,
    )
}

/// How far `q` lies from the boundary of a ball, cylinder side, box or ring.
fn boundary_gap(p: &Primitive, q: Vec3) -> f64 {
    let f = p.frame();
    let d = [q[0] - f.origin[0], q[1] - f.origin[1], q[2] - f.origin[2]];
    let dot = |a: Vec3| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
    let l = [dot(f.x), dot(f.y), dot(f.z)];
    match p {
        Primitive::Sphere { radius, .. } => {
            ((l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() - radius).abs()
        }
        Primitive::Cylinder { radius, .. } => (l[0].hypot(l[1]) - radius).abs(),
        Primitive::Torus { major, minor, .. } => {
            ((l[0].hypot(l[1]) - major).hypot(l[2]) - minor).abs()
        }
        Primitive::Box { size, .. } => {
            let outside = (0..3)
                .map(|i| (-l[i]).max(l[i] - size[i]).max(0.0))
                .fold(0.0, f64::max);
            let inside = (0..3)
                .map(|i| l[i].abs().min((l[i] - size[i]).abs()))
                .fold(f64::INFINITY, f64::min);
            outside.max(inside)
        }
        _ => f64::NAN,
    }
}

/// The chord tolerance a section is asked for, mm.
const SECTION_TOLERANCE: f64 = 0.01;

/// B35: a plane section gives as many closed curves as the closed form, every point on the plane
/// and the solid's boundary, and a total length short of the curves' by no more than chords within
/// the tolerance can lose, s/(3ρ) of each arc where s is the sagitta.
fn run_section<K: Kernel>(k: &mut K, c: &Case, sec: &crate::generate::Section) -> Value {
    let s = match made(k, c, "section") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let Shape::Prim(p) = &c.a else {
        return finish(c, Outcome::Crash, vec![], vec![]);
    };
    let (pieces, op) = match timed(c, "section", || {
        k.section(&s, sec.origin, sec.normal, SECTION_TOLERANCE)
    }) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let loops = match chained(pieces) {
        Ok(l) => l,
        Err(e) => {
            let checks = vec![record(
                "B35",
                false,
                format!("the pieces do not close: {e}"),
            )];
            return finish(c, Outcome::Fail, vec![op], checks);
        }
    };
    let on = 10.0 * TOLERANCE.max(1e-7);
    let mut worst_plane = 0.0f64;
    let mut worst_gap = 0.0f64;
    let mut length = 0.0;
    for lp in &loops {
        for (i, q) in lp.iter().enumerate() {
            let h = (0..3)
                .map(|j| (q[j] - sec.origin[j]) * sec.normal[j])
                .sum::<f64>();
            worst_plane = worst_plane.max(h.abs());
            worst_gap = worst_gap.max(boundary_gap(p, *q));
            let next = lp[(i + 1) % lp.len()];
            length +=
                ((q[0] - next[0]).powi(2) + (q[1] - next[1]).powi(2) + (q[2] - next[2]).powi(2))
                    .sqrt();
        }
    }
    let short = sec.length * SECTION_TOLERANCE / sec.rho + 10.0 * on;
    let long = 1e-7 * sec.length + 10.0 * on;
    let dl = length - sec.length;
    let checks = vec![
        record(
            "B35",
            same("closed curves", loops.len(), sec.loops),
            String::new(),
        ),
        record(
            "B35",
            worst_plane <= on && worst_gap <= on,
            format!(
                "points off the plane by up to {worst_plane:.3e}, off the \
                                  boundary by {worst_gap:.3e}"
            ),
        ),
        record(
            "B35",
            dl >= -short && dl <= long,
            format!(
                "length {length:.9} against {:.9}: off by {dl:.3e}",
                sec.length
            ),
        ),
    ];
    let ok = checks.iter().all(|c| c["ok"] == json!(true));
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        checks,
    )
}

/// Whether the boundary nearest each probe bends as the probe says: the kernel's curvature query
/// at every point, each principal curvature within the probe's tolerance. A kernel without the
/// query cannot show a curvature-continuous blend, and fails.
fn probes_check<K: Kernel>(
    k: &mut K,
    s: &K::Solid,
    probes: &[crate::generate::Probe],
) -> Option<(bool, String)> {
    let pts: Vec<Vec3> = probes.iter().map(|p| p.point).collect();
    Some(match attempt(|| k.curvature(s, &pts)) {
        Ok(got) if got.len() == probes.len() => {
            let mut off = Vec::new();
            for (i, (g, p)) in got.iter().zip(probes).enumerate() {
                match g {
                    Some(g) if (0..2).all(|j| (g[j] - p.want[j]).abs() <= p.tol) => {}
                    Some(g) => off.push(format!(
                        "probe {i}: [{:.4}, {:.4}] against [{:.4}, {:.4}] ± {:.4}",
                        g[0], g[1], p.want[0], p.want[1], p.tol
                    )),
                    None => off.push(format!("probe {i}: a crease")),
                }
            }
            let detail = format!(
                "{} of {} probes off: {}",
                off.len(),
                probes.len(),
                off.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
            );
            (off.is_empty(), detail)
        }
        Ok(got) => (
            false,
            format!("{} answers for {} probes", got.len(), probes.len()),
        ),
        // A query the kernel has not built for these points leaves the check out.
        Err(e) if e.outcome() == Outcome::Unsupported => return None,
        Err(e) => (
            false,
            format!("the curvature query shows the blend: {}", e.message()),
        ),
    })
}

/// The functions a chain's steps call, for telling which a kernel does not offer.
fn step_words(steps: &[crate::generate::Step]) -> Vec<&'static str> {
    use crate::generate::Step;
    steps
        .iter()
        .flat_map(|st| match st {
            Step::Act(f) => vec![f.act.word()],
            Step::Cut(_) => vec!["difference"],
            Step::Join(_) => vec!["union"],
            Step::Move(_) | Step::MirrorJoin(_) => vec!["transform"],
            Step::StepRoundTrip => vec!["write_step", "read_step"],
        })
        .collect()
}

fn apply_step<K: Kernel>(
    k: &mut K,
    s: &K::Solid,
    st: &crate::generate::Step,
) -> Result<K::Solid, Refusal> {
    use crate::generate::Step;
    match st {
        Step::Act(f) => act(k, s, f),
        Step::Cut(tool) => {
            let tool = build(k, tool)?;
            k.difference(s, &tool)
        }
        Step::Join(tool) => {
            let tool = build(k, tool)?;
            k.union(s, &tool)
        }
        Step::Move(m) => k.transform(s, *m),
        Step::MirrorJoin(m) => {
            let image = k.transform(s, *m)?;
            k.union(s, &image)
        }
        Step::StepRoundTrip => {
            let text = k.write_step(s)?;
            k.read_step(&text)
        }
    }
}

/// A chain or a part: `c.a` built, then each step applied in turn to what the last gave. The
/// result must be valid (B1), meet the closed form's volume and area (B14), and hold the sample
/// points as the expected shape `c.b` does (B15). Each step's time and outcome is recorded, so a
/// failure says which step broke.
fn run_chain<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    steps: &[crate::generate::Step],
) -> Value {
    let unsupported = lacks(k);
    if step_words(steps)
        .iter()
        .any(|w| unsupported.iter().any(|u| u == w))
        || !makes_all(&unsupported, &c.a)
    {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    let mut ops = Vec::new();
    let t = Instant::now();
    let mut s = match attempt(|| build(k, &c.a)) {
        Ok(s) => s,
        Err(e) => {
            let mut v = finish(c, e.outcome(), vec![], vec![]);
            v["error"] = json!(format!("making the base: {}", e.message()));
            return v;
        }
    };
    ops.push(json!({"op": "base", "outcome": "done", "ms": t.elapsed().as_secs_f64() * 1e3}));
    for (i, st) in steps.iter().enumerate() {
        let name = format!(
            "{} {}",
            i + 1,
            step_words(std::slice::from_ref(st)).join("+")
        );
        let t = Instant::now();
        let r = attempt(|| match (i, st, &c.a) {
            // The base itself joined to its mirror image: the image may be a stand-in.
            (0, crate::generate::Step::MirrorJoin(m), Shape::Prim(p)) => {
                let image = mirror_image(k, p, &s, *m)?;
                k.union(&s, &image)
            }
            _ => apply_step(k, &s, st),
        });
        let ms = t.elapsed().as_secs_f64() * 1e3;
        match r {
            Ok(next) => {
                ops.push(json!({"op": name, "outcome": "done", "ms": ms}));
                s = next;
            }
            Err(e) => {
                ops.push(json!({"op": name, "outcome": e.outcome().word(), "ms": ms,
                                "error": e.message()}));
                return finish(c, e.outcome(), ops, vec![]);
            }
        }
    }
    export::keep("after", "main", &s);
    let g = match attempt(|| Ok(got(k, &s))) {
        Ok(g) => g,
        Err(e) => {
            ops.push(
                json!({"op": "measure", "outcome": e.outcome().word(), "ms": 0.0,
                            "error": e.message()}),
            );
            return finish(c, e.outcome(), ops, vec![]);
        }
    };
    let mut checks = vec![record("B1", valid(&g.issues), g.issues.join("; "))];
    if let Some(v) = c.exact.result_volume {
        let area = c.exact.result_area.unwrap_or(g.area);
        let (dv, vb) = (g.volume - v, volume_bound(area, v));
        let mut ok = tiers(|| within(VOLUME, dv, vb));
        let mut detail = format!("volume {:.9} against {v:.9}: off by {dv:.3e}", g.volume);
        if let Some(a) = c.exact.result_area {
            let da = g.area - a;
            ok &= tiers(|| within(AREA, da, area_bound(a)));
            detail += &format!("; area {:.9} against {a:.9}: off by {da:.3e}", g.area);
        }
        checks.push(record("B14", ok, detail));
    }
    if let Some(bx) = oracle::bounds(&c.b) {
        let pts = oracle::samples(bx, SAMPLES);
        match placed(k, &s, &pts) {
            Some(Ok(placed)) => {
                let off = pts
                    .iter()
                    .zip(&placed)
                    .filter(|(q, g)| oracle::contains(&c.b, **q) != **g)
                    .count();
                checks.push(record(
                    "B15",
                    tally("sample points placed wrongly", off, 1),
                    format!("{off} of {} sample points placed differently", pts.len()),
                ));
            }
            Some(Err(why)) => checks.push(record("B15", false, why)),
            None => {}
        }
    }
    let mut v = finish(c, Outcome::Pass, ops, checks);
    v["steps"] = json!(steps.len());
    v
}

/// The diagonal of the box round a shape, the length its tolerances scale with.
fn diagonal(s: &Shape) -> f64 {
    oracle::bounds(s).map_or(1.0, |(lo, hi)| {
        (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt()
    })
}

/// B37: the principal curvatures at the boundary point nearest each point, to a millionth of the
/// larger of their size and the reciprocal of the solid's size.
fn run_curvature<K: Kernel>(k: &mut K, c: &Case, cv: &crate::generate::Curvatures) -> Value {
    let s = match made(k, c, "curvature") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let (got, op) = match timed(c, "curvature", || k.curvature(&s, &cv.points)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let floor = 1.0 / diagonal(&c.a);
    let mut off = Vec::new();
    if got.len() != cv.want.len() {
        off.push(format!(
            "{} answers for {} points",
            got.len(),
            cv.want.len()
        ));
    }
    for (i, (g, w)) in got.iter().zip(&cv.want).enumerate() {
        let tol = 1e-6 * (w[0].abs().max(w[1].abs()) + floor);
        match g {
            Some(g) if (g[0] - w[0]).abs() <= tol && (g[1] - w[1]).abs() <= tol => {}
            Some(g) => off.push(format!(
                "point {i}: [{:.9}, {:.9}] against [{:.9}, {:.9}]",
                g[0], g[1], w[0], w[1]
            )),
            None => off.push(format!(
                "point {i}: a crease, against [{:.9}, {:.9}]",
                w[0], w[1]
            )),
        }
    }
    let checks = vec![record(
        "B37",
        tally("places off", off.len(), 0),
        format!(
            "{} of {} off: {}",
            off.len(),
            cv.points.len(),
            off.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
        ),
    )];
    finish(c, Outcome::Pass, vec![op], checks)
}

/// The chord tolerance a projection is asked for, mm.
const PROJECTION_TOLERANCE: f64 = 0.01;

fn gap2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// How far `q` lies from the segment from `a` to `b`.
fn to_segment(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ab, aq) = ([b[0] - a[0], b[1] - a[1]], [q[0] - a[0], q[1] - a[1]]);
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len == 0.0 {
        0.0
    } else {
        ((aq[0] * ab[0] + aq[1] * ab[1]) / len).clamp(0.0, 1.0)
    };
    gap2(q, [a[0] + t * ab[0], a[1] + t * ab[1]])
}

/// How far `q` lies from a piece of the drawing in the view plane: exact for a straight run, and
/// for an arc the nearest of 128 samples refined by a ternary search between its neighbours.
fn to_piece(q: [f64; 2], p: &crate::generate::Piece, flat: &dyn Fn(Vec3) -> [f64; 2]) -> f64 {
    use crate::generate::Curve3;
    let at = |t: f64| flat(p.curve.at(t));
    if let Curve3::Line { .. } = p.curve {
        return to_segment(q, at(p.t0), at(p.t1));
    }
    let n = 128;
    let t = |j: usize| p.t0 + (p.t1 - p.t0) * j as f64 / n as f64;
    let whole = matches!(p.curve, Curve3::Circle { .. }) && (p.t1 - p.t0).abs() >= 1.0;
    let best = (0..if whole { n } else { n + 1 })
        .min_by(|&i, &j| gap2(q, at(t(i))).total_cmp(&gap2(q, at(t(j)))))
        .unwrap_or(0);
    // A whole circle is periodic: its bracket runs past the seam instead of stopping at it.
    let (mut lo, mut hi) = if whole {
        let h = (p.t1 - p.t0) / n as f64;
        (t(best) - h, t(best) + h)
    } else {
        (t(best.saturating_sub(1)), t((best + 1).min(n)))
    };
    for _ in 0..60 {
        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if gap2(q, at(m1)) < gap2(q, at(m2)) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    gap2(q, at((lo + hi) / 2.0))
}

/// B38: a hidden-line projection draws, seen and hidden apart, every vertex on a curve of its kind,
/// every chord within the tolerance of one, every stretch of those curves within the tolerance of
/// its chords, and no more length than the curves have.
fn run_projection<K: Kernel>(k: &mut K, c: &Case, dr: &crate::generate::Drawing) -> Value {
    let s = match made(k, c, "project") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let (drawn, op) = match timed(c, "project", || {
        k.project(&s, dr.direction, PROJECTION_TOLERANCE)
    }) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let (u, v) = brep_protocol::view_axes(dr.direction);
    let flat = |p: Vec3| {
        [
            p[0] * u[0] + p[1] * u[1] + p[2] * u[2],
            p[0] * v[0] + p[1] * v[1] + p[2] * v[2],
        ]
    };
    let scale = diagonal(&c.a);
    let on = 1e-6 * scale.max(1.0);
    let tol = PROJECTION_TOLERANCE + on;
    let mut checks = Vec::new();
    for (seen, lines, name) in [
        (true, &drawn.visible, "seen"),
        (false, &drawn.hidden, "hidden"),
    ] {
        let pieces: Vec<_> = dr
            .pieces
            .iter()
            .filter(|p| p.visible == seen && p.t1 > p.t0)
            .collect();
        let near = |q: [f64; 2]| {
            pieces
                .iter()
                .map(|p| to_piece(q, p, &flat))
                .fold(f64::INFINITY, f64::min)
        };
        let (mut vertex, mut chord, mut length) = (0.0f64, 0.0f64, 0.0);
        let mut worst = [0.0; 2];
        for line in lines {
            for (i, &q) in line.iter().enumerate() {
                let gap = near(q);
                if gap > vertex {
                    (vertex, worst) = (gap, q);
                }
                if let Some(&next) = line.get(i + 1) {
                    chord = chord.max(near([(q[0] + next[0]) / 2.0, (q[1] + next[1]) / 2.0]));
                    length += gap2(q, next);
                }
            }
        }
        let mut cover = 0.0f64;
        let mut want = 0.0;
        for p in &pieces {
            let n = 64;
            let at = |j: usize| flat(p.curve.at(p.t0 + (p.t1 - p.t0) * j as f64 / n as f64));
            for j in 0..=n {
                let q = at(j);
                let gap = lines
                    .iter()
                    .flat_map(|l| l.windows(2).map(|w| to_segment(q, w[0], w[1])))
                    .fold(f64::INFINITY, f64::min);
                cover = cover.max(gap);
            }
            let fine = 2048;
            let at = |j: usize| flat(p.curve.at(p.t0 + (p.t1 - p.t0) * j as f64 / fine as f64));
            want += (0..fine).map(|j| gap2(at(j), at(j + 1))).sum::<f64>();
        }
        let ok = vertex <= on && chord <= tol && cover <= tol && length <= want + on;
        checks.push(record(
            "B38",
            ok,
            format!(
                "{name}: vertices off by up to {vertex:.3e} (at [{:.6}, {:.6}]), \
                               chords {chord:.3e}, curves uncovered by {cover:.3e}; length \
                               {length:.6} of {want:.6}",
                worst[0], worst[1]
            ),
        ));
    }
    let mut v = finish(c, Outcome::Pass, vec![op], checks);
    v["direction"] = json!(dr.direction);
    v
}

/// The chord tolerance a display is asked for, mm.
pub const DISPLAY_TOLERANCE: f64 = 0.01;

/// B42: a display's faces close the solid and bound its volume, its normals are units pointing out,
/// its edges are those between the solid's surfaces, and each says what it bounds.
fn run_display<K: Kernel>(k: &mut K, c: &Case, want: &crate::generate::DisplayExact) -> Value {
    let s = match made(k, c, "display") {
        Ok(s) => s,
        Err(v) => return v,
    };
    let (d, op) = match timed(c, "display", || k.display(&s, DISPLAY_TOLERANCE)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    let checks = crate::display::checks(&d, want, DISPLAY_TOLERANCE, diagonal(&c.a), mesh_check)
        .into_iter()
        .map(|(ok, note)| record("B42", ok, note))
        .collect();
    finish(c, Outcome::Pass, vec![op], checks)
}

/// B36: a prism's loose faces, shifted apart by up to `SEW_GAP` and some inside out, sew into a
/// valid solid holding the prism's volume and area to what the shifts allow, and its points.
fn run_sew<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    sew: &crate::generate::Sew,
) -> Value {
    use crate::generate::{SEW_GAP, SEW_TOLERANCE};
    let Shape::Prim(p) = &c.a else {
        return finish(c, Outcome::Crash, vec![], vec![]);
    };
    if lacks(k).iter().any(|u| u == "sew") {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    let Some(text) = crate::step::loose(p, &sew.gaps, &sew.flipped) else {
        return finish(c, Outcome::Crash, vec![], vec![]);
    };
    let (s, op) = match timed(c, "sew", || k.sew(&text, SEW_TOLERANCE)) {
        Ok(x) => x,
        Err(v) => return v,
    };
    export::keep("sewn", "main", &s);
    let g = got(k, &s);
    let (area, volume) = (p.area(), p.volume());
    // Each face shifted by at most the gap moves the volume by at most gap·area; an edge's two
    // faces meet within a few gaps of it, so the area moves by at most that times the edges.
    let edges = match p {
        Primitive::Extrude {
            profile, height, ..
        } => 2.0 * profile.perimeter() + profile.loops[0].len() as f64 * height,
        _ => 0.0,
    };
    let vb = volume_bound(area, volume) + SEW_GAP * area;
    let ab = area_bound(area) + 4.0 * SEW_GAP * edges;
    let (dv, da) = (g.volume - volume, g.area - area);
    let mut checks = vec![record(
        "B36",
        tiers(|| valid(&g.issues) && within(VOLUME, dv, vb) && within(AREA, da, ab)),
        format!("sewn: ; {}", g.issues.join("; ")),
    )];
    if let Some(bx) = oracle::bounds(&c.a) {
        let pts = oracle::samples(bx, SAMPLES);
        match placed(k, &s, &pts) {
            Some(Ok(placed)) => {
                let off = pts
                    .iter()
                    .zip(&placed)
                    .filter(|(q, g)| oracle::contains(&c.a, **q) != **g)
                    .count();
                checks.push(record(
                    "B36",
                    tally("sample points placed wrongly", off, 1),
                    format!("{off} of {} points placed differently", pts.len()),
                ));
            }
            Some(Err(why)) => checks.push(record("B36", false, why)),
            None => {}
        }
    }
    let ok = checks.iter().all(|c| c["ok"] == json!(true));
    finish(
        c,
        if ok { Outcome::Pass } else { Outcome::Fail },
        vec![op],
        checks,
    )
}

/// B18 for a call that must be refused: a refusal passes, an answer fails.
fn refusal(c: &Case, name: &str, r: Result<(), Ran>) -> Value {
    let (outcome, detail) = match &r {
        Ok(()) => (
            Outcome::Fail,
            "accepted an input it must refuse".to_string(),
        ),
        // A kernel that says it cannot do this has not judged the input.
        Err(Ran::Refused(e)) if e.kind != Kind::Unsupported => {
            (Outcome::Pass, format!("refused, {e}"))
        }
        Err(e) => (e.outcome(), e.message()),
    };
    let ops = vec![json!({"op": name, "outcome": if r.is_ok() { "done" } else { "refused" }})];
    let checks = vec![record("B18", outcome == Outcome::Pass, detail)];
    finish(c, outcome, ops, checks)
}

/// The result object of a case that checks one call's answer under one clause.
fn finish(c: &Case, outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>) -> Value {
    let worst = if checks.iter().any(|ch| ch["ok"] == json!(false)) {
        outcome.max(Outcome::Fail)
    } else {
        outcome
    };
    json!({"id": c.id, "family": c.family, "outcome": worst.word(),
           "a": c.a.to_json(), "b": c.b.to_json(), "checks": checks, "ops": ops})
}

/// Whether the kernel offers every function named, and every constructor the primitives call.
fn offers<K: Kernel>(k: &mut K, functions: &[&str], prims: &[&Primitive]) -> bool {
    let unsupported = lacks(k);
    !functions.iter().any(|f| unsupported.iter().any(|u| u == f))
        && prims.iter().all(|p| makes(&unsupported, p))
}

/// Runs an assembly case: B27, the pairs that interfere, with their shared volumes and gaps. The
/// suite makes each part where it stands and asks the kernel about one pair at a time, leaving out
/// the pairs whose bounding balls are farther apart than the clearance.
fn run_assembly<K: Kernel>(k: &mut K, c: &Case, a: &Assembly) -> Value {
    let prims: Vec<&Primitive> = a.shapes.iter().collect();
    if !offers(k, &["interference"], &prims) {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    // Each part is its shape made on its frame moved by the placement, so that a case about
    // interference does not turn on whether the kernel can move a solid.
    let made = attempt(|| {
        a.parts
            .iter()
            .map(|&(i, m)| {
                let motion = crate::model::Rigid {
                    r: [0, 1, 2].map(|j| [m[j][0], m[j][1], m[j][2]]),
                    t: [m[0][3], m[1][3], m[2][3]],
                };
                construct(k, &a.shapes[i].moved(&motion))
            })
            .collect::<Result<Vec<_>, _>>()
    });
    let placed = match made {
        Ok(s) => s,
        Err(e) => {
            let ops = vec![
                json!({"op": "making the parts", "outcome": e.outcome().word(),
                                  "error": e.message()}),
            ];
            return finish(c, e.outcome(), ops, vec![]);
        }
    };
    // A ball round each placed part: its shape's box turns with the part, the ball does not.
    let balls: Vec<(Vec3, f64)> = a
        .parts
        .iter()
        .map(|&(i, m)| {
            let (lo, hi) =
                oracle::bounds(&Shape::Prim(a.shapes[i].clone())).expect("a primitive has a box");
            let mid = [0, 1, 2].map(|j| 0.5 * (lo[j] + hi[j]));
            let r = 0.5 * (0..3).map(|j| (hi[j] - lo[j]).powi(2)).sum::<f64>().sqrt();
            (apply(&m, mid), r)
        })
        .collect();
    // Timed by size class, so assemblies of about one size share a row of the report.
    let class = match placed.len() {
        0..=15 => 10,
        16..=45 => 30,
        _ => 100,
    };
    let name = format!("interference ≈{class} parts");
    let t = Instant::now();
    let r = attempt(|| {
        let mut got = Vec::new();
        for i in 0..placed.len() {
            for j in i + 1..placed.len() {
                let apart = (0..3)
                    .map(|x| (balls[i].0[x] - balls[j].0[x]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if apart - balls[i].1 - balls[j].1 > a.clearance {
                    continue;
                }
                let m = k.interference(&placed[i], &placed[j])?;
                if m.volume > 0.0 || m.gap < a.clearance {
                    got.push(Clash {
                        i,
                        j,
                        volume: m.volume,
                        gap: m.gap,
                    });
                }
            }
        }
        Ok(got)
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let got = match r {
        Ok(g) => g,
        Err(e) => {
            let ops = vec![json!({"op": name, "ms": ms, "outcome": e.outcome().word(),
                                  "error": e.message()})];
            return finish(c, e.outcome(), ops, vec![]);
        }
    };
    let ops = vec![json!({"op": name, "ms": ms, "outcome": "done", "clashes": got.len()})];
    let key = |x: &Clash| (x.i.min(x.j), x.i.max(x.j));
    let found: BTreeMap<(usize, usize), &Clash> = got.iter().map(|x| (key(x), x)).collect();
    let want: BTreeMap<(usize, usize), &Clash> = a.clashes.iter().map(|x| (key(x), x)).collect();
    let missed: Vec<_> = want.keys().filter(|p| !found.contains_key(p)).collect();
    let extra: Vec<_> = found.keys().filter(|p| !want.contains_key(p)).collect();
    let mut checks = vec![record(
        "B27",
        missed.is_empty() && extra.is_empty(),
        format!(
            "{} pairs missed {:?}, {} pairs extra {:?}",
            missed.len(),
            missed.iter().take(4).collect::<Vec<_>>(),
            extra.len(),
            extra.iter().take(4).collect::<Vec<_>>()
        ),
    )];
    let off = || {
        let mut off = Vec::new();
        for (p, w) in &want {
            let Some(g) = found.get(p) else { continue };
            let (si, sj) = (a.parts[p.0].0, a.parts[p.1].0);
            let area = a.shapes[si].area() + a.shapes[sj].area();
            let vb = volume_bound(area, w.volume);
            let gb = Bound::linear(10.0, 1e-9 * 1e3);
            if !meets(VOLUME, g.volume, w.volume, vb) || !meets(LENGTH, g.gap, w.gap, gb) {
                off.push(format!(
                    "{p:?}: volume {:.3e} against {:.3e}, gap {:.3e} against {:.3e}",
                    g.volume, w.volume, g.gap, w.gap
                ));
            }
        }
        off
    };
    checks.push(record(
        "B27",
        tiers(|| off().is_empty()),
        off().iter().take(4).cloned().collect::<Vec<_>>().join("; "),
    ));
    finish(c, Outcome::Pass, ops, checks)
}

/// Runs a pattern case: B28, a boolean of many solids in one call, against its closed form and
/// against the same booleans one at a time.
fn run_many<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, m: &Many) -> Value {
    let function = if m.target.is_some() {
        "difference_all"
    } else {
        "union_all"
    };
    let prims: Vec<&Primitive> = m.target.iter().chain(&m.tools).collect();
    if !offers(k, &[function], &prims) {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    let made = attempt(|| {
        let target = m.target.as_ref().map(|t| construct(k, t)).transpose()?;
        let tools = m
            .tools
            .iter()
            .map(|t| construct(k, t))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((target, tools))
    });
    let (target, tools) = match made {
        Ok(x) => x,
        Err(e) => {
            let ops = vec![
                json!({"op": "making the solids", "outcome": e.outcome().word(),
                                  "error": e.message()}),
            ];
            return finish(c, e.outcome(), ops, vec![]);
        }
    };
    let refs: Vec<&K::Solid> = tools.iter().collect();
    let n = refs.len();
    let mut ops = Vec::new();
    let mut worst = Outcome::Pass;
    let mut timed = |k: &mut K, name: String, f: &dyn Fn(&mut K) -> Result<K::Solid, Refusal>| {
        let t = Instant::now();
        let r = attempt(|| {
            f(k).map(|s| {
                if name.starts_with("all at once") {
                    export::keep("result", "main", &s);
                }
                k.measure(&s)
            })
        });
        let ms = t.elapsed().as_secs_f64() * 1e3;
        match r {
            Ok(g) => {
                ops.push(
                    json!({"op": name, "ms": ms, "outcome": "done", "volume": g.volume,
                                "area": g.area}),
                );
                Some(g)
            }
            Err(e) => {
                ops.push(json!({"op": name, "ms": ms, "outcome": e.outcome().word(),
                                "error": e.message()}));
                worst = worst.max(e.outcome());
                None
            }
        }
    };
    let all = timed(k, format!("all at once ×{n}"), &|k| match &target {
        Some(t) => k.difference_all(t, &refs),
        None => k.union_all(&refs),
    });
    let each = timed(k, format!("one at a time ×{n}"), &|k| {
        let (mut acc, rest) = match &target {
            Some(t) => (k.difference(t, refs[0])?, &refs[1..]),
            None => (k.union(refs[0], refs[1])?, &refs[2..]),
        };
        for s in rest {
            acc = if target.is_some() {
                k.difference(&acc, s)?
            } else {
                k.union(&acc, s)?
            };
        }
        Ok(acc)
    });
    let vb = volume_bound(m.area, m.volume);
    let mut checks = Vec::new();
    if let Some(g) = &all {
        checks.push(record("B1", valid(&g.issues), g.issues.join("; ")));
        let (dv, da) = (g.volume - m.volume, g.area - m.area);
        checks.push(record(
            "B28",
            tiers(|| within(VOLUME, dv, vb) && within(AREA, da, area_bound(m.area))),
            String::new(),
        ));
        if let Some(e) = &each {
            let d = e.volume - g.volume;
            checks.push(record(
                "B28",
                tiers(|| within(VOLUME, d, vb)),
                format!("one at a time off by {d:.3e}"),
            ));
        }
    }
    finish(c, worst, ops, checks)
}

/// Runs a mirror case: B29, the part joined to its image in a mirror.
fn run_mirror<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, m: &Mirror) -> Value {
    let Shape::Prim(p) = &c.a else {
        return finish(c, Outcome::Crash, vec![], vec![]);
    };
    if !offers(k, &["transform"], &[p]) {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    let t = Instant::now();
    let r = attempt(|| {
        let a = construct(k, p)?;
        let image = mirror_image(k, p, &a, m.matrix)?;
        let joined = k.union(&a, &image)?;
        export::keep("result", "main", &joined);
        Ok(k.measure(&joined))
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let g = match r {
        Ok(g) => g,
        Err(e) => {
            let ops = vec![json!({"op": "a ∪ mirrored a", "ms": ms,
                                  "outcome": e.outcome().word(), "error": e.message()})];
            return finish(c, e.outcome(), ops, vec![]);
        }
    };
    let ops = vec![json!({"op": "a ∪ mirrored a", "ms": ms, "outcome": "done",
                          "volume": g.volume, "area": g.area})];
    let vb = volume_bound(m.area, m.volume);
    let (dv, da) = (g.volume - m.volume, g.area - m.area);
    let checks = vec![
        record("B1", valid(&g.issues), g.issues.join("; ")),
        record(
            "B29",
            tiers(|| within(VOLUME, dv, vb) && within(AREA, da, area_bound(m.area))),
            String::new(),
        ),
    ];
    finish(c, Outcome::Pass, ops, checks)
}

/// How the kernel's normals at points meet the expected: a normal within 1e-6 of each expected one,
/// and none where none is expected. Returns the mismatches.
fn normals_off(got: &[Option<Vec3>], want: &[(Vec3, Option<Vec3>)]) -> Vec<String> {
    let mut off = Vec::new();
    for (i, (g, (p, w))) in got.iter().zip(want).enumerate() {
        let ok = match (g, w) {
            (Some(g), Some(w)) => (0..3).map(|k| (g[k] - w[k]).powi(2)).sum::<f64>().sqrt() <= 1e-6,
            (None, None) => true,
            _ => false,
        };
        if !ok {
            off.push(format!("point {i} {p:?}: {g:?} against {w:?}"));
        }
    }
    if got.len() != want.len() {
        off.push(format!("{} answers for {} points", got.len(), want.len()));
    }
    off
}

/// Runs a normals case: B30, the outward normals at points of a solid's boundary.
fn run_normals<K: Kernel>(k: &mut K, c: &Case, n: &Normals) -> Value {
    let prims: Vec<&Primitive> = [&c.a, &c.b]
        .into_iter()
        .filter_map(|s| match s {
            Shape::Prim(p) => Some(p),
            Shape::Bool(..) => None,
        })
        .collect();
    if !offers(k, &["normal"], &prims) {
        return finish(c, Outcome::Unsupported, vec![], vec![]);
    }
    let pts: Vec<Vec3> = n.points.iter().map(|x| x.0).collect();
    let t = Instant::now();
    let r = attempt(|| {
        let a = build(k, &c.a)?;
        let s = match n.op {
            None => a,
            Some(op) => {
                let b = build(k, &c.b)?;
                k.boolean(op, &a, &b)?
            }
        };
        k.normal(&s, &pts)
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Ok(got) => {
            let off = normals_off(&got, &n.points);
            let ops = vec![json!({"op": "normal", "ms": ms, "outcome": "done"})];
            let checks = vec![record(
                "B30",
                tally("places off", off.len(), 0),
                format!(
                    "{} of {} off: {}",
                    off.len(),
                    pts.len(),
                    off.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
                ),
            )];
            finish(c, Outcome::Pass, ops, checks)
        }
        Err(e) => {
            let ops = vec![
                json!({"op": "normal", "ms": ms, "outcome": e.outcome().word(),
                                  "error": e.message()}),
            ];
            finish(c, e.outcome(), ops, vec![])
        }
    }
}

/// Runs a smooth loft: sound, Q1–Q3, and Q7, that it passes through its inner sections.
fn run_loft<K: Kernel<Solid: Clone + 'static>>(k: &mut K, c: &Case, l: &LoftThrough) -> Value {
    if !offers(k, &["loft_through"], &[]) {
        let mut v = finish(c, Outcome::Unsupported, vec![], vec![]);
        v["sound"] = json!(true);
        return v;
    }
    if c.refuse {
        let r = attempt(|| {
            k.loft_through(&l.frame, &l.profiles, &l.heights)
                .map(|_| ())
        });
        return refusal(c, "loft_through", r);
    }
    let mut checks = Vec::new();
    let t = Instant::now();
    let made = attempt(|| k.loft_through(&l.frame, &l.profiles, &l.heights));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let s = match made {
        Ok(s) => s,
        Err(e) => {
            let ops = vec![json!({"op": "loft_through", "ms": ms,
                                  "outcome": e.outcome().word(), "error": e.message()})];
            checks.push(record("Q1", false, e.message()));
            let mut v = finish(c, e.outcome(), ops, checks);
            v["sound"] = json!(true);
            return v;
        }
    };
    export::keep("result", "main", &s);
    let m = k.measure(&s);
    let ops = vec![json!({"op": "loft_through", "ms": ms, "outcome": "done",
                          "volume": m.volume, "area": m.area})];
    checks.push(record("Q1", true, ""));
    checks.push(record("Q2", valid(&m.issues), m.issues.join("; ")));
    if offers(k, &["betti"], &[]) {
        let b = attempt(|| k.betti(&s));
        checks.push(record(
            "Q3",
            b.as_ref().is_ok_and(|&b| betti_same(b, [1, 0, 0])),
            b.err().map(|e| e.message()).unwrap_or_default(),
        ));
    }
    // Q7: each inner section's outline lies on the loft's boundary, and its cut is the profile.
    if offers(k, &["normal"], &[]) {
        let on = attempt(|| k.normal(&s, &l.outline));
        let off = match &on {
            Ok(got) => Some(got.iter().filter(|n| n.is_none()).count()),
            Err(e) if e.outcome() == Outcome::Unsupported => None,
            Err(_) => Some(l.outline.len()),
        };
        if let Some(off) = off {
            checks.push(record(
                "Q7",
                tally("outline points off the boundary", off, 0),
                String::new(),
            ));
        }
    }
    if offers(k, &["section_area"], &[]) {
        for (p, &h) in l.profiles.iter().zip(&l.heights).skip(1) {
            if h >= *l.heights.last().unwrap_or(&0.0) {
                continue;
            }
            let o = [0, 1, 2].map(|j| l.frame.origin[j] + l.frame.z[j] * h);
            let want = p.area();
            let got = attempt(|| k.section_area(&s, o, l.frame.z));
            let ok = tiers(|| matches!(got, Ok(a) if meets(AREA, a, want, area_bound(want))));
            checks.push(record(
                "Q7",
                ok,
                format!(
                    "section at {h:.3}: {:?} against {want:.9}",
                    got.map_err(|e| e.message().to_string())
                ),
            ));
        }
    }
    if !c.exact.scans.is_empty() && offers(k, &["raycast", "normal"], &[]) {
        for (clause, ok, detail) in scan_checks(k, &s, &c.exact.scans) {
            checks.push(record(clause, ok, detail));
        }
    }
    // B0: where the sections leave one answer, the loft is that solid.
    if let (true, Shape::Prim(p)) = (c.exact.loft_is_b, &c.b) {
        let vb = volume_bound(p.area(), p.volume());
        let (dv, da) = (m.volume - p.volume(), m.area - p.area());
        checks.push(record(
            "B0",
            tiers(|| within(VOLUME, dv, vb) && within(AREA, da, area_bound(p.area()))),
            "the loft against the prism of its sections",
        ));
        if let Some(bx) = oracle::bounds(&c.b) {
            let pts = oracle::samples(bx, SAMPLES);
            match placed(k, &s, &pts) {
                Some(Ok(placed)) => {
                    let off = pts
                        .iter()
                        .zip(&placed)
                        .filter(|(q, g)| oracle::contains(&c.b, **q) != **g)
                        .count();
                    checks.push(record(
                        "B0",
                        tally("sample points placed wrongly", off, 1),
                        format!("{off} of {} points placed differently", pts.len()),
                    ));
                }
                Some(Err(why)) => checks.push(record("B0", false, why)),
                None => {}
            }
        }
    }
    let mut v = finish(c, Outcome::Pass, ops, checks);
    v["sound"] = json!(true);
    v
}

fn run_case<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    peer: Option<&mut Remote>,
) -> Value {
    if c.exact.itself {
        return run_itself(k, c);
    }
    if let Some(bx) = c.exact.bbox {
        return run_bbox(k, c, bx);
    }
    if !c.exact.closest.is_empty() {
        return run_closest(k, c);
    }
    if !c.exact.sides.is_empty() {
        return run_sides(k, c);
    }
    if let Some((o, n)) = c.exact.split {
        return run_split(k, c, o, n);
    }
    if let Some(s) = &c.exact.section {
        return run_section(k, c, s);
    }
    if let Some(s) = &c.exact.sew {
        return run_sew(k, c, s);
    }
    if let Some(cv) = &c.exact.curvature {
        return run_curvature(k, c, cv);
    }
    if let Some(d) = &c.exact.drawing {
        return run_projection(k, c, d);
    }
    if let Some(d) = &c.exact.display {
        return run_display(k, c, d);
    }
    if let Some(steps) = &c.exact.chain {
        return run_chain(k, c, steps);
    }
    if let Some(f) = &c.feature {
        return run_feature(k, c, f);
    }
    if let Some(a) = &c.exact.assembly {
        return run_assembly(k, c, a);
    }
    if let Some(m) = &c.exact.many {
        return run_many(k, c, m);
    }
    if let Some(m) = &c.exact.mirror {
        return run_mirror(k, c, m);
    }
    if let Some(n) = &c.exact.normals {
        return run_normals(k, c, n);
    }
    if let Some(l) = &c.exact.loft {
        return run_loft(k, c, l);
    }
    if let Some(f) = &c.nist {
        return run_nist(k, c, f);
    }
    if c.step {
        return run_step(k, c);
    }
    if let Some(m) = c.matrix {
        if c.family == "stretches" {
            return run_affine(k, c, m);
        }
        return run_transform(k, c, m);
    }
    let unsupported = lacks(k);
    let short = |outcome: Outcome, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(),
               "a": c.a.to_json(), "b": c.b.to_json(), "checks": checks, "ops": []})
    };
    // A constructor's own test asks for the constructor, to make its solid or to refuse a bad
    // one; every other case takes its stand-in.
    let direct = (c.refuse || crate::generate::CONSTRUCTORS.contains(&c.family))
        && kinds(&c.a).iter().any(|kd| unsupported.contains(kd));
    if direct || !makes_all(&unsupported, &c.a) || !makes_all(&unsupported, &c.b) {
        return short(Outcome::Unsupported, vec![]);
    }
    if c.refuse {
        // B18: an invalid input must be refused.
        let r = attempt(|| build(k, &c.a).map(|_| ()));
        let (outcome, detail) = match &r {
            Ok(()) => (Outcome::Fail, "accepted an invalid input".to_string()),
            // A kernel that says it cannot do this has not judged the input.
            Err(Ran::Refused(e)) if e.kind != Kind::Unsupported => {
                (Outcome::Pass, format!("refused, {e}"))
            }
            Err(e) => (e.outcome(), e.message()),
        };
        let ok = outcome == Outcome::Pass;
        return short(outcome, vec![record("B18", ok, detail)]);
    }
    let supports = |f: &str| !unsupported.iter().any(|u| u == f);
    let mut ops: Vec<Value> = Vec::new();
    let mut worst = Outcome::Pass;
    // Runs and measures one operation, records it, and gives back its measure if it completed.
    let mut ran = |name: &str, f: &mut dyn FnMut() -> Result<Got, Refusal>| -> Option<Got> {
        let t = Instant::now();
        let r = attempt(f);
        let mut o = json!({"op": name, "ms": t.elapsed().as_secs_f64() * 1e3});
        let out = match r {
            Ok(m) => {
                o["outcome"] = json!("done");
                o["volume"] = json!(m.volume);
                o["area"] = json!(m.area);
                if !m.issues.is_empty() {
                    o["issues"] = json!(m.issues);
                }
                match &m.mass {
                    Ok(mm) if mm.v.abs() > 0.0 => {
                        o["centroid"] = json!(mm.f.map(|x| x / mm.v));
                    }
                    Ok(_) => {}
                    Err(e) => o["mass_error"] = json!(e.to_string()),
                }
                // A refused query refuses the case, as a refused operation does.
                if m.mass_refused {
                    worst = worst.max(Outcome::Refused);
                }
                Some(m)
            }
            Err(r) => {
                o["outcome"] = json!(r.outcome().word());
                o["error"] = json!(r.message());
                // The round trip through STEP is a check on the side: a kernel that cannot read
                // this file back is left out of that check, and judged on the rest.
                let aside = is_round_trip(name) && r.outcome() == Outcome::Unsupported;
                if !aside {
                    worst = worst.max(r.outcome());
                }
                None
            }
        };
        ops.push(o);
        out
    };

    let (a, b) = match (attempt(|| build(k, &c.a)), attempt(|| build(k, &c.b))) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            let (outcome, msg) = (e.outcome(), e.message().to_string());
            return json!({"id": c.id, "family": c.family, "outcome": outcome.word(),
                          "a": c.a.to_json(), "b": c.b.to_json(),
                          "error": format!("making an operand: {msg}"), "checks": [], "ops": []});
        }
    };
    let op = |k: &mut K, name: &str, o: Op, x: &K::Solid, y: &K::Solid| {
        k.boolean(o, x, y).map(|s| {
            export::keep(name, "main", &s);
            got(k, &s)
        })
    };
    export::keep("a", "main", &a);
    export::keep("b", "main", &b);
    let ma = ran("a", &mut || Ok(got(k, &a)));
    let mb = ran("b", &mut || Ok(got(k, &b)));
    let u = ran("a ∪ b", &mut || op(k, "a ∪ b", Op::Union, &a, &b));
    let i = ran("a ∩ b", &mut || op(k, "a ∩ b", Op::Intersection, &a, &b));
    let ib = ran("b ∩ a", &mut || op(k, "b ∩ a", Op::Intersection, &b, &a));
    let dab = ran("a ∖ b", &mut || op(k, "a ∖ b", Op::Difference, &a, &b));
    let dba = ran("b ∖ a", &mut || op(k, "b ∖ a", Op::Difference, &b, &a));
    let (am, bm) = (c.a.moved(&c.motion), c.b.moved(&c.motion));
    let im = ran("moved a ∩ moved b", &mut || {
        let (x, y) = (build(k, &am)?, build(k, &bm)?);
        op(k, "moved a ∩ moved b", Op::Intersection, &x, &y)
    });
    let step = supports("write_step") && supports("read_step");
    let text = step
        .then(|| {
            attempt(|| {
                let u = k.union(&a, &b)?;
                k.write_step(&u)
            })
            .ok()
        })
        .flatten();
    let rt = step
        .then(|| {
            ran("a ∪ b through STEP", &mut || {
                let text = text.as_ref().ok_or("writing a ∪ b as STEP failed")?;
                let read = k.read_step(text)?;
                Ok(got(k, &read))
            })
        })
        .flatten();
    // The same file read by another kernel: what one writes, the other must read.
    let exchanged = match (peer, &text) {
        (Some(p), Some(t)) => Some(attempt(|| {
            let s = p.read_step(t)?;
            Ok(p.measure(&s))
        })),
        _ => None,
    };
    // B20 asks for the intersection again: the same input must give the same answer.
    let again = ran("a ∩ b again", &mut || {
        op(k, "a ∩ b again", Op::Intersection, &a, &b)
    });
    // B26: a ray through a ∪ b, from outside its box, toward a point near its middle.
    let union_shape = Shape::bool(Op::Union, c.a.clone(), c.b.clone());
    let ray = oracle::bounds(&union_shape).map(|(lo, hi)| {
        let mid = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
        let half = (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f64>().sqrt();
        let d = c.motion.r[1];
        let aside = c.motion.r[2].map(|x| x * 0.1 * half);
        let o = [0, 1, 2].map(|i| mid[i] + aside[i] - d[i] * 1.5 * half);
        (o, d, 3.0 * half)
    });
    let ray_hits = match (ray, supports("raycast")) {
        (Some((o, d, _)), true) => Some(attempt(|| {
            let u = k.union(&a, &b)?;
            Ok(crossings(k.raycast(&u, o, d)?, d))
        })),
        _ => None,
    };
    // The Betti numbers of the operands and results whose ones are known.
    let topo = |k: &mut K, x: &K::Solid| attempt(|| k.betti(x)).ok();
    let betti_a = c.exact.betti_a.and(topo(k, &a));
    let betti_b = c.exact.betti_b.and(topo(k, &b));
    let betti_u = c
        .exact
        .betti_union
        .and_then(|_| attempt(|| k.union(&a, &b)).ok())
        .and_then(|s| topo(k, &s));
    let betti_d = c
        .exact
        .betti_difference
        .and_then(|_| attempt(|| k.difference(&a, &b)).ok())
        .and_then(|s| topo(k, &s));
    // The intersection's section by a plane through its sample box, square to a direction drawn
    // from the case's motion, and its mesh.
    let inter = attempt(|| k.intersection(&a, &b)).ok();
    let plane =
        oracle::bounds(&Shape::bool(Op::Intersection, c.a.clone(), c.b.clone())).map(|(lo, hi)| {
            let mid = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
            (mid, c.motion.r[0])
        });
    let section = match (&inter, plane, supports("section_area")) {
        (Some(s), Some((o, n)), true) => Some(attempt(|| k.section_area(s, o, n))),
        _ => None,
    };
    let meshed = match (&inter, supports("mesh")) {
        (Some(s), true) => Some(attempt(|| k.mesh(s, MESH_TOLERANCE))),
        _ => None,
    };
    let after = (attempt(|| Ok(k.measure(&a))), attempt(|| Ok(k.measure(&b))));
    // Distances: both ways round, and after the rigid motion.
    let mut far = |name: &str, f: &mut dyn FnMut() -> Result<f64, Refusal>| -> Option<f64> {
        let t = Instant::now();
        let r = attempt(f);
        let mut o = json!({"op": name, "ms": t.elapsed().as_secs_f64() * 1e3});
        let out = match r {
            Ok(d) => {
                o["outcome"] = json!("done");
                o["distance"] = json!(d);
                Some(d)
            }
            Err(r) => {
                o["outcome"] = json!(r.outcome().word());
                o["error"] = json!(r.message());
                // The round trip through STEP is a check on the side: a kernel that cannot read
                // this file back is left out of that check, and judged on the rest.
                let aside = is_round_trip(name) && r.outcome() == Outcome::Unsupported;
                if !aside {
                    worst = worst.max(r.outcome());
                }
                None
            }
        };
        ops.push(o);
        out
    };
    // Distance to an empty solid is undefined, so a built operand that came out empty is not asked.
    let empty = |g: &Option<Got>| g.as_ref().is_some_and(|g| g.volume.abs() < 1e-12);
    let (d_ab, d_ba, d_moved) = if empty(&ma) || empty(&mb) || !supports("distance") {
        (None, None, None)
    } else {
        (
            far("distance a b", &mut || k.distance(&a, &b)),
            far("distance b a", &mut || k.distance(&b, &a)),
            far("distance moved a b", &mut || {
                let (x, y) = (build(k, &am)?, build(k, &bm)?);
                k.distance(&x, &y)
            }),
        )
    };

    let mut interop: Option<Value> = None;
    if let Some(Err(e)) = &exchanged {
        interop = Some(json!({"agrees": false, "error": e.message()}));
    }
    for (name, r) in [
        ("section", section.as_ref().map(|r| r.as_ref().map(|_| ()))),
        ("mesh", meshed.as_ref().map(|r| r.as_ref().map(|_| ()))),
        ("raycast", ray_hits.as_ref().map(|r| r.as_ref().map(|_| ()))),
    ] {
        if let Some(Err(e)) = r {
            ops.push(json!({"op": name, "outcome": e.outcome().word(), "error": e.message()}));
            worst = worst.max(e.outcome());
        }
    }
    let mut checks: Vec<Check> = Vec::new();
    let mut check = |clause: &'static str, ok: bool, note: String| {
        checks.push(Check::taken(clause, ok, &note));
    };
    // B1: every result is valid, built operands among them.
    for (name, r) in [
        ("a", &ma),
        ("b", &mb),
        ("a ∪ b", &u),
        ("a ∩ b", &i),
        ("b ∩ a", &ib),
        ("a ∖ b", &dab),
        ("b ∖ a", &dba),
    ] {
        if let Some(r) = r {
            check(
                "B1",
                valid(&r.issues),
                format!("{name}: {}", r.issues.join("; ")),
            );
        }
    }
    if let (Some(ma), Some(mb)) = (&ma, &mb) {
        let area = ma.area + mb.area;
        let vb = volume_bound(area, ma.volume + mb.volume);
        // B0: the operands are the shapes asked for: a primitive by its closed forms, a built
        // one by the suite's sampled estimate of its volume.
        for (name, got, want) in [("a", ma, &c.a), ("b", mb, &c.b)] {
            match want {
                Shape::Prim(p) => b0_primitive(&mut check, name, got, p, vb),
                Shape::Bool(..) => {
                    let Some(bx) = oracle::bounds(want) else {
                        continue;
                    };
                    let (est, allow, _, _) = sampled(bx, |p| oracle::contains(want, p));
                    check(
                        "B0",
                        tiers(|| meets(VOLUME, got.volume, est, vb + allow)),
                        format!(
                            "{name}: volume {:.6} against a sampled {est:.6} ± {:.2e}",
                            got.volume,
                            (vb + allow).now()
                        ),
                    );
                }
            }
        }
        // B2: closed forms.
        if let (Some(i), Some(want)) = (&i, c.exact.intersection_volume) {
            let ok = tiers(|| meets(VOLUME, i.volume, want, vb));
            check(
                "B2",
                ok,
                format!("a ∩ b volume {:.9} against {want:.9}", i.volume),
            );
        }
        if let (Some(i), Some(want)) = (&i, c.exact.intersection_area) {
            let v = c.exact.intersection_volume.unwrap_or(i.volume);
            let ok = tiers(|| meets(AREA, i.area, want, area_bound_of(want, v)));
            check(
                "B2",
                ok,
                format!("a ∩ b area {:.9} against {want:.9}", i.area),
            );
        }
        // B3: inclusion–exclusion.
        if let (Some(u), Some(i)) = (&u, &i) {
            let d = u.volume + i.volume - ma.volume - mb.volume;
            check(
                "B3",
                tiers(|| within(VOLUME, d, vb)),
                format!("|a∪b| + |a∩b| − |a| − |b| = {d:.3e}"),
            );
        }
        // B4: each operand is its difference and the intersection.
        if let (Some(d1), Some(d2), Some(i)) = (&dab, &dba, &i) {
            let (e1, e2) = (
                d1.volume + i.volume - ma.volume,
                d2.volume + i.volume - mb.volume,
            );
            check(
                "B4",
                tiers(|| within(VOLUME, e1, vb)),
                format!("|a∖b| + |a∩b| − |a| = {e1:.3e}"),
            );
            check(
                "B4",
                tiers(|| within(VOLUME, e2, vb)),
                format!("|b∖a| + |a∩b| − |b| = {e2:.3e}"),
            );
        }
        // B5: intersection is symmetric.
        if let (Some(i), Some(ib)) = (&i, &ib) {
            let d = i.volume - ib.volume;
            check(
                "B5",
                tiers(|| within(VOLUME, d, vb)),
                format!("|a∩b| − |b∩a| = {d:.3e}"),
            );
        }
        // B6: moving both operands rigidly changes nothing.
        if let (Some(i), Some(im)) = (&i, &im) {
            let d = im.volume - i.volume;
            check(
                "B6",
                tiers(|| within(VOLUME, d, vb)),
                format!("|a∩b| moved − unmoved = {d:.3e}"),
            );
        }
        // B7: the operations leave their inputs as they were.
        if let (Ok(a2), Ok(b2)) = &after {
            let same = a2.volume == ma.volume && b2.volume == mb.volume;
            check("B7", same, "operand volumes after the operations".into());
        }
        // Moments are taken about the world origin, so their bounds scale with the farthest a
        // point of either operand lies from it.
        let reach = [&c.a, &c.b]
            .iter()
            .filter_map(|s| oracle::bounds(s))
            .flat_map(|(lo, hi)| {
                (0..8).map(move |i| {
                    let p = [0, 1, 2].map(|k| if i & (1 << k) == 0 { lo[k] } else { hi[k] });
                    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
                })
            })
            .fold(0.0, f64::max);
        let (fb, sb) = (vb * reach, vb * reach * reach);
        let moments = |g: &Option<Got>| g.as_ref().and_then(|g| g.mass.as_ref().ok().copied());
        let mut shared = false;
        // B8 and B9: the suite's own sample of the intersection, from the exact operands.
        let both = Shape::bool(Op::Intersection, c.a.clone(), c.b.clone());
        if let Some(bx) = oracle::bounds(&both) {
            let (est, allow, pts, truth) = sampled(bx, |p| oracle::contains(&both, p));
            shared = truth.iter().any(|&t| t);
            let n = pts.len();
            let allow = vb + allow;
            if let Some(i) = &i {
                check(
                    "B8",
                    tiers(|| meets(VOLUME, i.volume, est, allow)),
                    format!(
                        "|a∩b| {:.6} against a sampled {est:.6} ± {:.2e}",
                        i.volume,
                        allow.now()
                    ),
                );
            }
            // B12: the intersection's moments against the sample's.
            if let Some(mi) = moments(&i) {
                let (est, tol) = mass::sampled(oracle::volume(bx), &pts, &truth);
                let worst_off = || {
                    (0..3)
                        .map(|a| (mi.f[a] - est.f[a]).abs() / (tol.f[a] + fb.now()))
                        .chain((0..9).map(|x| {
                            let (a, b) = (x / 3, x % 3);
                            (mi.s[a][b] - est.s[a][b]).abs() / (tol.s[a][b] + sb.now())
                        }))
                        .fold(0.0, f64::max)
                };
                check(
                    "B12",
                    tiers(|| worst_off() <= 1.0),
                    format!(
                        "a∩b moments against the sample's: {:.2} of the allowance",
                        worst_off()
                    ),
                );
            }
            let t = Instant::now();
            let classified = attempt(|| {
                let s = k.boolean(Op::Intersection, &a, &b)?;
                Ok(k.holds(&s, &pts))
            });
            let ms = t.elapsed().as_secs_f64() * 1e3;
            match classified {
                Ok(Ok(got)) => {
                    let off = got.iter().zip(&truth).filter(|(g, t)| g != t).count();
                    let shown: Vec<Value> = pts
                        .iter()
                        .zip(got.iter().zip(&truth))
                        .filter(|(_, (g, t))| g != t)
                        .take(5)
                        .map(|(p, (_, t))| json!({"point": p, "inside": t}))
                        .collect();
                    check(
                        "B9",
                        tally("sample points placed wrongly", off, 1),
                        format!("{off} of {n} sample points classified against the operands"),
                    );
                    ops.push(json!({"op": "classify a ∩ b", "outcome": "done", "ms": ms,
                                    "mismatched": off, "first": shown}));
                }
                Ok(Err(e)) if e.kind == Kind::Unsupported => {}
                Ok(Err(e)) => {
                    check("B9", false, unplaced(&e.to_string()));
                    ops.push(
                        json!({"op": "classify a ∩ b", "outcome": "refused", "ms": ms,
                                    "error": e.to_string()}),
                    );
                }
                Err(r) => {
                    // An intersection that declines is judged by its own checks, not here.
                    if !matches!(r, Ran::Refused(_)) {
                        worst = worst.max(r.outcome());
                    }
                    ops.push(
                        json!({"op": "classify a ∩ b", "outcome": r.outcome().word(),
                                    "ms": ms, "error": r.message()}),
                    );
                }
            }
        }
        let fits = |x: mass::Moments, y: mass::Moments, fb: Bound, sb: Bound| {
            let (d1, d2) = x.gap(&y);
            (
                tiers(|| within(FIRST, d1, fb) && within(SECOND, d2, sb)),
                String::new(),
            )
        };
        // B10: each primitive operand's moments meet their closed forms.
        let own = |g: &Got| g.mass.as_ref().ok().copied();
        for (name, g, want) in [("a", ma, &c.a), ("b", mb, &c.b)] {
            if let (Some(m), Shape::Prim(p)) = (own(g), want) {
                let (ok, how) = fits(m, mass::of(p), fb, sb);
                check("B10", ok, format!("{name} moments: {how}"));
            }
        }
        // B11: moments add up as volumes do.
        if let (Some(ma), Some(mb)) = (own(ma), own(mb)) {
            if let (Some(mu), Some(mi)) = (moments(&u), moments(&i)) {
                let (ok, how) = fits(mu.plus(&mi, 1.0), ma.plus(&mb, 1.0), fb, sb);
                check("B11", ok, format!("a∪b + a∩b against a + b: {how}"));
            }
            for (name, d, whole) in [("a∖b", &dab, ma), ("b∖a", &dba, mb)] {
                if let (Some(md), Some(mi)) = (moments(d), moments(&i)) {
                    let (ok, how) = fits(md.plus(&mi, 1.0), whole, fb, sb);
                    check(
                        "B11",
                        ok,
                        format!("{name} + a∩b against its operand: {how}"),
                    );
                }
            }
        }
        // B12: moving both operands moves the intersection's moments with them.
        if let (Some(mi), Some(mim)) = (moments(&i), moments(&im)) {
            let t = c.motion.t;
            let far_reach = reach + (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            let (fb, sb) = (vb * far_reach, vb * far_reach * far_reach);
            let (ok, how) = fits(mim, mi.moved(&c.motion), fb, sb);
            check("B12", ok, format!("moved a∩b against a∩b moved: {how}"));
        }
        // B19: the Betti numbers known for operands and results.
        for (name, want, gotten) in [
            ("a", c.exact.betti_a, betti_a),
            ("b", c.exact.betti_b, betti_b),
            ("a ∪ b", c.exact.betti_union, betti_u),
            ("a ∖ b", c.exact.betti_difference, betti_d),
        ] {
            if let (Some(w), Some(g)) = (want, gotten) {
                check("B19", betti_same(g, w), name.to_string());
            }
        }
        // B20: the intersection asked for twice is the same to the last bit.
        if let (Some(x), Some(y)) = (&i, &again) {
            let same =
                x.volume.to_bits() == y.volume.to_bits() && x.area.to_bits() == y.area.to_bits();
            check(
                "B20",
                same,
                format!(
                    "a ∩ b twice: volumes {} and {}, areas {} and {}",
                    x.volume, y.volume, x.area, y.area
                ),
            );
        }
        // B21: the section's area against the suite's sample of the plane.
        if let (Some(Ok(area)), Some((o, n))) = (&section, plane) {
            let (est, allow) = oracle::section_area(
                &Shape::bool(Op::Intersection, c.a.clone(), c.b.clone()),
                o,
                n,
                SAMPLES,
            );
            let tol = Bound::linear(10.0 * reach, allow + 1e-9 * est.abs());
            check(
                "B21",
                tiers(|| meets(AREA, *area, est, tol)),
                format!(
                    "section area {area:.6} against a sampled {est:.6} ± {:.2e}",
                    tol.now()
                ),
            );
        }
        // Interop, not conformance: a ∪ b written by this kernel and read by the peer, recorded
        // as agreeing when the peer's solid is valid with the volume the suite's sample gives.
        // It scores neither kernel, since a mismatch may be either's.
        let both = Shape::bool(Op::Union, c.a.clone(), c.b.clone());
        if let (Some(Ok(m)), Some(bx)) = (&exchanged, oracle::bounds(&both)) {
            let (est, allow, _, _) = sampled(bx, |p| oracle::contains(&both, p));
            let agrees = m.issues.is_empty() && (m.volume - est).abs() <= allow + vb.now();
            interop = Some(json!({"agrees": agrees, "volume": m.volume, "sampled": est,
                                  "issues": m.issues}));
        }
        // B23: the intersection's mesh is closed, faces outward, and holds its volume.
        if let (Some(Ok(mesh)), Some(i)) = (&meshed, &i)
            && i.volume > vb.now()
        {
            let (closed, volume) = mesh_check(mesh);
            let tol = vb + 2.0 * MESH_TOLERANCE * i.area;
            check(
                "B23",
                closed && tiers(|| meets(VOLUME, volume, i.volume, tol)),
                format!(
                    "mesh {}, volume {volume:.6} against {:.6} ± {:.2e}",
                    if closed { "closed" } else { "open" },
                    i.volume,
                    tol.now()
                ),
            );
        }
        // B26: the ray's crossings, where the suite's march along it can see every feature.
        if let (Some(Ok(hits)), Some((o, d, far))) = (&ray_hits, ray) {
            let steps = 8192;
            let (want, gap) = oracle::crossings(&union_shape, o, d, far, steps);
            if gap > 4.0 * far / steps as f64 && !THIN.iter().any(|t| c.family.contains(t)) {
                let tol = Bound::linear(10.0, 1e-9 * far);
                let worst = if hits.len() == want.len() {
                    hits.iter()
                        .zip(&want)
                        .map(|(g, w)| (g - w).abs())
                        .fold(0.0, f64::max)
                } else {
                    f64::INFINITY
                };
                check(
                    "B26",
                    tiers(|| within(LENGTH, worst, tol)),
                    format!(
                        "{} crossings against {}, off by up to {worst:.3e}, along {o:?} + t {d:?}",
                        hits.len(),
                        want.len()
                    ),
                );
            }
        }
        // B17: a ∪ b written as STEP and read back is the same valid solid. A union that is not
        // valid itself fails B1; what a reader makes of it is no measure of the round trip.
        if let (Some(u), Some(r)) = (&u, &rt)
            && u.issues.is_empty()
        {
            let (dv, da) = (r.volume - u.volume, r.area - u.area);
            let mut ok = tiers(|| {
                r.issues.is_empty()
                    && within(VOLUME, dv, vb)
                    && within(AREA, da, area_bound_of(u.area, u.volume))
            });
            let mut how = String::new();
            if let (Ok(mu), Ok(mr)) = (&u.mass, &r.mass) {
                let (fits_, moved) = fits(*mr, *mu, fb, sb);
                ok &= fits_;
                how += &format!("; moments {moved}");
            }
            if !r.issues.is_empty() {
                how += &format!("; {}", r.issues.join("; "));
            }
            check("B17", ok, format!("a ∪ b through STEP: {how}"));
        }
        // B13: the distance meets its exact value, is the same both ways, survives a motion, and is
        // zero where the sample found a shared point.
        let db = Bound::linear(10.0, 1e-9 * reach);
        if let (Some(want), Some(d)) = (c.exact.distance, d_ab) {
            check(
                "B13",
                tiers(|| meets(LENGTH, d, want, db)),
                format!("distance {d:.9} against {want:.9}"),
            );
        }
        if let (Some(x), Some(y)) = (d_ab, d_ba) {
            check(
                "B13",
                tiers(|| meets(LENGTH, x, y, db)),
                format!("distance a b {x:.9}, b a {y:.9}"),
            );
        }
        if let (Some(x), Some(y)) = (d_ab, d_moved) {
            check(
                "B13",
                tiers(|| meets(LENGTH, x, y, db)),
                format!("distance {x:.9}, moved {y:.9}"),
            );
        }
        if let (true, Some(d)) = (shared, d_ab) {
            check(
                "B13",
                tiers(|| within(LENGTH, d, db)),
                format!("distance {d:.3e} where the sample found a shared point"),
            );
        }
    }
    if checks.iter().any(|c| !c.ok) {
        worst = worst.max(Outcome::Fail);
    }
    let checks: Vec<Value> = checks.iter().map(Check::to_json).collect();
    json!({
        "id": c.id, "family": c.family, "outcome": worst.word(),
        "a": c.a.to_json(), "b": c.b.to_json(), "checks": checks, "ops": ops, "interop": interop,
    })
}

/// Runs `cases` on `threads` threads, in case order.
/// How far a file's measure may be from what its exporter states: exporters round them.
const STATED_REL: f64 = 1e-3;

/// Runs one of NIST's STEP files (Q1, Q2): the kernel must read it and find what it read valid. The
/// volume and area its exporter states are set beside the kernel's own, for the reader, and no check
/// weighs them: they are another program's numbers, not the suite's.
fn run_nist<K: Kernel<Solid: Clone + 'static>>(
    k: &mut K,
    c: &Case,
    f: &crate::generate::NistFile,
) -> Value {
    let header = |outcome: Outcome, ops: Vec<Value>, checks: Vec<Value>| {
        json!({"id": c.id, "family": c.family, "outcome": outcome.word(), "file": f.name,
               "checks": checks, "ops": ops})
    };
    if lacks(k).iter().any(|u| u == "read_step") {
        return header(Outcome::Unsupported, vec![], vec![]);
    }
    let text = match std::fs::read_to_string(&f.path) {
        Ok(t) => t,
        Err(e) => {
            let mut v = header(Outcome::Crash, vec![], vec![]);
            v["error"] = json!(format!("{}: {e}", f.path.display()));
            return v;
        }
    };
    let t = Instant::now();
    let read = attempt(|| {
        let s = k.read_step(&text)?;
        export::keep("read", "main", &s);
        Ok(k.measure(&s))
    });
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let m = match read {
        Ok(m) => m,
        Err(e) => {
            let op = json!({"op": "read_step", "outcome": e.outcome().word(), "ms": ms,
                            "error": e.message()});
            return header(e.outcome(), vec![op], vec![]);
        }
    };
    let near =
        |stated: Option<f64>, got: f64| stated.map(|w| (got - w).abs() <= STATED_REL * w.abs());
    let judged: Vec<bool> = [near(f.stated_volume, m.volume), near(f.stated_area, m.area)]
        .into_iter()
        .flatten()
        .collect();
    let mut op = json!({"op": "read_step", "outcome": "done", "ms": ms, "volume": m.volume,
                        "area": m.area, "stated_volume": f.stated_volume,
                        "stated_area": f.stated_area});
    if !judged.is_empty() {
        op["agrees"] = json!(judged.iter().all(|&a| a));
    }
    let checks = vec![
        record("Q1", true, "read the file"),
        record("Q2", valid(&m.issues), m.issues.join("; ")),
    ];
    let outcome = if valid(&m.issues) {
        Outcome::Pass
    } else {
        Outcome::Fail
    };
    header(outcome, vec![op], checks)
}

/// Runs `cases` on `threads` threads, each thread on a kernel of its own from `make`, reset after
/// each case. `peer`, when given, makes each thread a second kernel, which reads the STEP files the
/// first writes. `limit` is the longest one call of the kernel may take, which the results record.
pub fn run<K: Kernel<Solid: Clone + 'static>>(
    make: impl Fn() -> Result<K, String> + Sync,
    cases: &[Case],
    threads: usize,
    peer: Option<&(dyn Fn() -> Result<Remote, String> + Sync)>,
    exports: &Exports,
    limit: Option<std::time::Duration>,
) -> Result<Value, String> {
    let started = (std::time::SystemTime::now(), Instant::now());
    // The header's kernel first, gone before the threads start theirs, so a run never holds more
    // kernels than threads.
    let (kernel, unsupported, calibration) = {
        let mut k = make()?;
        let kernel = json!({"name": k.name(), "version": k.version(), "tolerance": k.tolerance(),
                            "configuration": k.configuration()});
        let unsupported = lacks(&k);
        (kernel, unsupported, calibrate(&mut k))
    };
    let next = Mutex::new(0usize);
    let out: Mutex<Vec<(usize, Value)>> = Mutex::new(Vec::new());
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                let (mut k, mut p): (Option<K>, Option<Remote>) = (None, None);
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        *n += 1;
                        *n - 1
                    };
                    let Some(c) = cases.get(i) else { break };
                    WORST.with(|w| w.set(0.0));
                    if exports.any() {
                        export::begin();
                    }
                    // A kernel that cannot start refuses the case it was to run, and the next case
                    // starts another. A peer that cannot start leaves the case without interop.
                    let fresh = k.is_none();
                    let mut made = match k.take() {
                        Some(k) => Ok(k),
                        None => make(),
                    };
                    if p.is_none() {
                        p = peer.and_then(|make| {
                            make()
                                .map_err(|e| eprintln!("the peer did not start: {e}"))
                                .ok()
                        });
                    }
                    let mut v = match &mut made {
                        Ok(k) => run_case(k, c, p.as_mut()),
                        Err(e) => {
                            let mut v = finish(c, Outcome::Refused, vec![], vec![]);
                            v["error"] = json!(format!("starting the kernel: {e}"));
                            v
                        }
                    };
                    v["test"] = json!(c.test);
                    // The first case on a kernel the runner started for it pays that kernel's first
                    // calls, which a reader may leave out of a time. A kernel that did not start ran
                    // nothing. A worker the client replaces inside a kernel is not seen here.
                    if made.is_ok() {
                        v["cold"] = json!(fresh);
                    }
                    judge(&mut v, c);
                    v["asset_step"] = json!(!crate::assets::of_case(c).step.is_empty());
                    if exports.any()
                        && let Ok(k) = &mut made
                    {
                        export::export_case(k, c, exports, &mut v);
                    }
                    // How far over the suite's tolerance its worst measure went, for a case that failed.
                    let worst = WORST.with(|w| w.get());
                    if v["outcome"] == "fail" && worst > 1.0 {
                        v["over"] = json!(if worst.is_finite() { worst } else { f64::MAX });
                    }
                    // A run keeps no solid past its case.
                    let _ = catch_unwind(AssertUnwindSafe(|| {
                        if let Ok(k) = &mut made {
                            k.reset();
                        }
                        if let Some(p) = &mut p {
                            p.reset();
                        }
                    }));
                    k = made.ok();
                    out.lock().unwrap().push((i, v));
                }
            });
        }
    });
    std::panic::set_hook(quiet);
    let mut out = out.into_inner().unwrap();
    out.sort_by_key(|x| x.0);
    Ok(json!({
        "format": RESULTS_FORMAT,
        "suite": SUITE_VERSION,
        "contract": CONTRACT_VERSION,
        "tolerance": TOLERANCE,
        "kernel": kernel,
        "stand_ins": stand_ins(&unsupported),
        "unsupported": unsupported,
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cores": std::thread::available_parallelism().map_or(0, |n| n.get()),
            "threads": threads.max(1),
        },
        "started_unix": started
            .0
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        "unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        "wall_ms": started.1.elapsed().as_secs_f64() * 1e3,
        "time_limit_ms": limit.map(|l| l.as_secs_f64() * 1e3),
        "calibration_ms": calibration,
        "cases": out.into_iter().map(|x| x.1).collect::<Vec<_>>(),
    }))
}

/// The words a check's or a case's tier takes, tightest first.
pub const TIER_WORDS: [&str; 5] = ["1e-7", "1e-6", "1e-5", "1e-3", "fail"];

/// The version of what draws a test's shape files: part of each file's name, so a change here
/// names new files. springrep-adapter's tests fail when springrep's `display` changes until it does.
pub const ASSET_GENERATOR: &str = "9";

/// Adds a case's verdict to its result: its `kind`; its `result`, the loosest tier among its
/// checks or how it ended without an answer; for one that is not a pass at the suite's tolerance,
/// `why` and whether the `failure` is of geometry or of measurement; its time; and the name of its
/// shape files.
/// Whether a step is the B17 round trip: the case's operation again, written as STEP and read back.
fn is_round_trip(op: &str) -> bool {
    op.ends_with("through STEP")
}

fn judge(v: &mut Value, c: &Case) {
    let kind = if c.refuse {
        "rejection"
    } else if c.exact.sound.is_some() || c.nist.is_some() {
        "sound"
    } else {
        "exact"
    };
    let outcome = v["outcome"].as_str().unwrap_or("crash").to_string();
    let checks = v["checks"].as_array().cloned().unwrap_or_default();
    let rank = |ch: &Value| {
        TIER_WORDS
            .iter()
            .position(|t| ch["tier"] == *t)
            .unwrap_or(4)
    };
    let loosest = checks.iter().map(rank).max();
    let result = match (outcome.as_str(), loosest) {
        ("pass", _) => TIER_WORDS[0],
        ("fail", Some(r)) if r > 0 => TIER_WORDS[r],
        ("fail", _) => "fail",
        (other, _) => other,
    };
    if result != TIER_WORDS[0] {
        let failed: Vec<&Value> = checks.iter().filter(|ch| ch["ok"] == false).collect();
        let why = match failed.iter().max_by_key(|ch| rank(ch)) {
            Some(ch) if matches!(outcome.as_str(), "pass" | "fail") => format!(
                "{}: {}",
                ch["clause"].as_str().unwrap_or("?"),
                ch["detail"].as_str().unwrap_or("")
            ),
            _ => v["error"].as_str().map(String::from).unwrap_or_else(|| {
                v["ops"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find_map(|o| {
                        let e = o["error"].as_str()?;
                        Some(format!("{}: {e}", o["op"].as_str().unwrap_or("?")))
                    })
                    .unwrap_or_default()
            }),
        };
        v["why"] = json!(why);
        // A check that weighed no measure judged the shape itself: points, validity, topology.
        if !failed.is_empty() {
            let shape = failed
                .iter()
                .any(|ch| ch["measures"].as_array().is_none_or(|m| m.is_empty()));
            v["failure"] = json!(if shape { "geometry" } else { "measurement" });
        }
    }
    // The B17 round trip checks interchange, not modelling, and only kernels that read and write
    // STEP run it, so its time is kept apart from the case's.
    let (mut ms, mut step_ms, mut round_trip) = (0.0, 0.0, false);
    for o in v["ops"].as_array().into_iter().flatten() {
        let Some(t) = o["ms"].as_f64() else { continue };
        if is_round_trip(o["op"].as_str().unwrap_or("")) {
            step_ms += t;
            round_trip = true;
        } else {
            ms += t;
        }
    }
    v["kind"] = json!(kind);
    v["result"] = json!(result);
    v["time_ms"] = json!(ms);
    v["step_ms"] = if round_trip {
        json!(step_ms)
    } else {
        Value::Null
    };
    v["asset"] = json!(crate::assets::hash(c));
}

/// The time of a reference workload on this kernel and machine, ms: the median of 15 rounds of
/// two crossing cylinders, their intersection and its volume. Reports divide times by it, so they
/// compare across machines as multiples of one kernel doing one simple thing.
pub fn calibrate<K: Kernel>(k: &mut K) -> f64 {
    let mut round = || {
        let t = Instant::now();
        let a = construct(
            k,
            &Primitive::Cylinder {
                frame: crate::model::Frame::at([0.0, 0.0, -5.0]),
                radius: 2.0,
                height: 10.0,
            },
        )?;
        let b = construct(
            k,
            &Primitive::Cylinder {
                frame: crate::model::Frame::along([-5.0, 0.5, 0.0], [1.0, 0.0, 0.0]),
                radius: 1.5,
                height: 10.0,
            },
        )?;
        let i = k.intersection(&a, &b)?;
        k.volume(&i)?;
        Ok::<f64, Refusal>(t.elapsed().as_secs_f64() * 1e3)
    };
    let mut times: Vec<f64> = (0..15).filter_map(|_| round().ok()).collect();
    times.sort_by(f64::total_cmp);
    times.get(times.len() / 2).copied().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::cases;
    use crate::profile::Seg;

    /// A point on a whole circle just short of its seam (t in (127.5/128, 1)) lies on the piece. Its
    /// nearest samples are 0 and 128, the same point to the last bit or so; when rounding keeps
    /// sample 0, the search must still look on the side of the seam the point is on. Circles in many
    /// frames, so the rounding falls both ways.
    #[test]
    fn a_point_just_short_of_a_whole_circle_s_seam_lies_on_it() {
        use crate::generate::{Curve3, Piece};
        let flat = |p: Vec3| [p[0], p[1]];
        let mut worst = (0.0_f64, 0usize, 0.0_f64);
        for k in 0..64 {
            let (a, b) = (0.37 + 0.731 * k as f64, 0.11 + 0.293 * k as f64);
            let (sa, ca) = a.sin_cos();
            let (sb, cb) = b.sin_cos();
            // an orthonormal frame turned about z by a and tilted about x by b
            let x = [ca, sa * cb, sa * sb];
            let y = [-sa, ca * cb, ca * sb];
            let curve = Curve3::Circle { c: [0.3 * k as f64, -1.7, 2.0], x, y, r: 1.547 };
            let piece = Piece { curve, t0: 0.0, t1: 1.0, visible: true };
            for t in [0.99630, 0.9975, 0.999, 0.9999] {
                let d = to_piece(flat(curve.at(t)), &piece, &flat);
                if d > worst.0 {
                    worst = (d, k, t);
                }
            }
        }
        assert!(
            worst.0 < 1e-9,
            "frame {} at t = {}: a point on the circle reads {:e} from it",
            worst.1,
            worst.2,
            worst.0
        );
    }

    #[test]
    fn a_mirrored_primitive_holds_the_mirror_image_of_every_point() {
        // The mirror in the plane x + y = 3, and each kind on a tilted frame: a point is in the
        // primitive exactly when its image is in the mirrored one.
        let s = 0.5f64.sqrt();
        let n = [s, s, 0.0];
        let d = 3.0 * s;
        let m: [[f64; 4]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| match j {
                3 => 2.0 * d * n[i],
                _ => f64::from(u8::from(i == j)) - 2.0 * n[i] * n[j],
            })
        });
        let frame = crate::model::Frame::along([1.0, -2.0, 0.5], [0.6, 0.0, 0.8]);
        let ell = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 1.0],
            [1.0, 1.0],
            [1.0, 3.0],
            [0.0, 3.0],
        ];
        let lines = (0..6).map(|i| Seg::Line {
            a: ell[i],
            b: ell[(i + 1) % 6],
        });
        let kinds = [
            Primitive::Box {
                frame,
                size: [4.0, 3.0, 2.0],
            },
            Primitive::Cone {
                frame,
                bottom: 2.0,
                top: 1.0,
                height: 3.0,
            },
            Primitive::Extrude {
                frame,
                profile: crate::profile::Profile {
                    loops: vec![lines.collect()],
                },
                height: 2.0,
            },
        ];
        for p in kinds {
            let q = p.mirrored(&m).expect("the kind has a mirror image");
            let (lo, hi) = oracle::bounds(&Shape::Prim(p.clone())).expect("a box");
            let pts = oracle::samples((lo, hi), 2000);
            let inside = pts
                .iter()
                .filter(|&&x| oracle::contains(&Shape::Prim(p.clone()), x));
            assert!(inside.count() > 200, "too few points inside {p:?}");
            for x in pts {
                let was = oracle::contains(&Shape::Prim(p.clone()), x);
                let is = oracle::contains(&Shape::Prim(q.clone()), apply(&m, x));
                assert_eq!(was, is, "{x:?} of {p:?}");
            }
        }
    }

    fn five_primitives() -> Vec<Primitive> {
        let f = crate::model::Frame::along([1.0, -2.0, 0.5], [0.3, -0.5, 0.8]);
        vec![
            Primitive::Box {
                frame: f,
                size: [3.0, 2.0, 4.0],
            },
            Primitive::Cylinder {
                frame: f,
                radius: 1.5,
                height: 4.0,
            },
            Primitive::Sphere {
                frame: f,
                radius: 2.0,
            },
            Primitive::Torus {
                frame: f,
                major: 3.0,
                minor: 0.75,
            },
            Primitive::Cone {
                frame: f,
                bottom: 2.0,
                top: 0.5,
                height: 3.0,
            },
            Primitive::Cone {
                frame: f,
                bottom: 2.0,
                top: 0.0,
                height: 3.0,
            },
            Primitive::Cone {
                frame: f,
                bottom: 0.0,
                top: 1.5,
                height: 3.0,
            },
        ]
    }

    #[test]
    fn a_stand_in_is_the_same_solid_as_its_primitive() {
        for p in five_primitives() {
            let q = p.stand_in().expect("each of the five has one");
            let (m, n) = (mass::of(&p), mass::of(&q));
            let (dv, df) = m.gap(&n);
            assert!(dv < 1e-9 && df < 1e-9, "{}: {dv} {df}", p.kind());
            let (sp, sq): (Shape, Shape) = (p.clone().into(), q.into());
            let pts = oracle::samples(oracle::bounds(&sp).unwrap(), 4000);
            let differ = pts
                .iter()
                .filter(|&&x| oracle::contains(&sp, x) != oracle::contains(&sq, x))
                .count();
            assert_eq!(differ, 0, "{}", p.kind());
        }
    }

    #[test]
    fn a_stand_in_is_sent_only_for_a_constructor_the_kernel_lacks() {
        let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let sphere = &five_primitives()[2];
        assert!(stand_in(&words(&[]), sphere).is_none());
        assert!(makes(&words(&[]), sphere));
        let sent = stand_in(&words(&["sphere"]), sphere).unwrap();
        assert_eq!(sent.kind(), "revolve");
        assert!(makes(&words(&["sphere"]), sphere));
        // With no revolution either, there is nothing to send.
        assert!(stand_in(&words(&["sphere", "revolve"]), sphere).is_none());
        assert!(!makes(&words(&["sphere", "revolve"]), sphere));
        assert_eq!(
            stand_ins(&words(&["sphere", "box"])),
            json!({"box": "extrude", "sphere": "revolve"})
        );
    }

    #[test]
    fn a_check_takes_the_tightest_tier_its_bound_holds_at() {
        // A volume off by 3e-5 against a bound of 10·ε·1 passes from ε = 1e-5 on.
        let bound = volume_bound(1.0, 0.0);
        assert!(!tiers(|| within(LENGTH, 3e-5, bound)));
        assert_eq!(tier_of(false), "1e-5");
        assert!(tiers(|| within(LENGTH, 3e-7, bound)));
        assert_eq!(tier_of(true), "1e-7");
        assert!(!tiers(|| within(LENGTH, 1.0, bound)));
        assert_eq!(tier_of(false), "fail");
        // A verdict no tolerance decides holds at every tier or at none.
        assert_eq!(tier_of(true), "1e-7");
        assert_eq!(tier_of(false), "fail");
        assert!(tiers(|| within(LENGTH, 3e-7, bound)));
        assert_eq!(tier_of(false), "fail");
        MEASURES.with(|m| m.borrow_mut().clear());
    }

    #[test]
    fn a_case_is_judged_by_its_loosest_check_or_by_how_it_ended() {
        let c = cases(1)
            .into_iter()
            .find(|c| !c.refuse && c.exact.sound.is_none())
            .unwrap();
        let check = |tier: &str, measured: bool| {
            json!({"clause": "B2", "ok": tier == "1e-7", "tier": tier, "detail": "off",
                   "measures": if measured { json!([{"error": 1.0, "bound": 0.5}]) } else { json!([]) }})
        };
        let mut loose = json!({"outcome": "fail", "ops": [{"ms": 2.0}, {"ms": 3.0}],
                               "checks": [check("1e-7", true), check("1e-5", true)]});
        judge(&mut loose, &c);
        assert_eq!(loose["result"], "1e-5");
        assert_eq!(loose["failure"], "measurement");
        assert_eq!(loose["why"], "B2: off");
        assert_eq!(loose["time_ms"], 5.0);
        assert_eq!(loose["kind"], "exact");

        let mut wrong = json!({"outcome": "fail", "ops": [],
                               "checks": [check("fail", false), check("1e-5", true)]});
        judge(&mut wrong, &c);
        assert_eq!(wrong["result"], "fail");
        assert_eq!(wrong["failure"], "geometry");

        let mut refused = json!({"outcome": "refused", "checks": [],
                                 "ops": [{"op": "a ∪ b", "error": "no"}]});
        judge(&mut refused, &c);
        assert_eq!(refused["result"], "refused");
        assert_eq!(refused["why"], "a ∪ b: no");

        let mut pass = json!({"outcome": "pass", "ops": [], "checks": [check("1e-7", true)]});
        judge(&mut pass, &c);
        assert_eq!(pass["result"], "1e-7");
        assert!(pass["why"].is_null());
    }

    #[test]
    fn a_cases_time_leaves_out_its_round_trip_through_step_which_is_kept_apart() {
        let c = cases(1)
            .into_iter()
            .find(|c| !c.refuse && c.exact.sound.is_none())
            .unwrap();
        let pass = json!([{"clause": "B1", "ok": true, "tier": "1e-7", "detail": "",
                           "measures": [], "counts": []}]);
        let mut boolean = json!({"outcome": "pass", "checks": pass.clone(), "ops": [
            {"op": "a", "ms": 1.0}, {"op": "a ∪ b", "ms": 2.0},
            {"op": "a ∪ b through STEP", "ms": 40.0}, {"op": "classify a ∩ b", "ms": 4.0}]});
        judge(&mut boolean, &c);
        assert_eq!(boolean["time_ms"], 7.0);
        assert_eq!(boolean["step_ms"], 40.0);

        let mut blend = json!({"outcome": "pass", "checks": pass.clone(), "ops": [
            {"op": "fillet", "ms": 3.0}, {"op": "moved", "ms": 5.0},
            {"op": "through STEP", "ms": 20.0}]});
        judge(&mut blend, &c);
        assert_eq!(blend["time_ms"], 8.0);
        assert_eq!(blend["step_ms"], 20.0);

        // A kernel that does not read or write STEP runs no round trip: there is no STEP time.
        let mut none = json!({"outcome": "pass", "checks": pass.clone(), "ops": [
            {"op": "fillet", "ms": 3.0}, {"op": "moved", "ms": 5.0}]});
        judge(&mut none, &c);
        assert_eq!(none["time_ms"], 8.0);
        assert!(none["step_ms"].is_null());

        // A test of reading and writing STEP is timed in full: its STEP step is the operation.
        let mut step = json!({"outcome": "pass", "checks": pass, "ops": [
            {"op": "base", "ms": 1.0}, {"op": "2 write_step+read_step", "ms": 9.0}]});
        judge(&mut step, &c);
        assert_eq!(step["time_ms"], 10.0);
        assert!(step["step_ms"].is_null());
    }

    /// A kernel whose STEP reader runs past the time limit on a file named slow, and which, as a
    /// lost worker does, ends every later call the same way until it is reset.
    struct Sticky(std::cell::Cell<bool>);

    impl Kernel for Sticky {
        type Solid = ();
        fn supports(&self) -> Vec<String> {
            brep_protocol::FUNCTIONS
                .iter()
                .map(|&f| f.to_string())
                .collect()
        }
        fn name(&self) -> String {
            "sticky".into()
        }
        fn version(&self) -> String {
            "0".into()
        }
        fn tolerance(&self) -> f64 {
            TOLERANCE
        }
        fn make(&mut self, _p: &Primitive) -> Result<(), Refusal> {
            Ok(())
        }
        fn union(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn intersection(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn difference(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn volume(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(1.0)
        }
        fn area(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(6.0)
        }
        fn check(&mut self, _s: &()) -> Result<Vec<String>, Refusal> {
            Ok(Vec::new())
        }
        fn transform(&mut self, _s: &(), _m: [[f64; 4]; 3]) -> Result<(), Refusal> {
            Ok(())
        }
        fn read_step(&mut self, text: &str) -> Result<(), Refusal> {
            if text.contains("slow") || self.0.get() {
                self.0.set(true);
                std::panic::panic_any(brep_protocol::TimedOut("read_step".into()));
            }
            Ok(())
        }
        fn reset(&mut self) {
            self.0.set(false);
        }
        fn display(&mut self, _s: &(), _tolerance: f64) -> Result<brep_protocol::Display, Refusal> {
            Ok(brep_protocol::Display::new(
                vec![brep_protocol::DisplayFace::new(
                    vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    vec![[0.0, 0.0, 1.0]; 3],
                    vec![[0, 1, 2]],
                )],
                Vec::new(),
                Vec::new(),
            ))
        }
    }

    #[test]
    fn a_kernel_that_cannot_start_refuses_its_case_and_the_next_case_starts_another() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let starts = AtomicUsize::new(0);
        // The header's kernel starts, the first case's does not, and the second case's does.
        let make = || match starts.fetch_add(1, Ordering::SeqCst) {
            1 => Err("no free core".to_string()),
            _ => Ok(Sticky(Default::default())),
        };
        let cases: Vec<Case> = crate::generate::cases(1)
            .into_iter()
            .filter(|c| c.test.starts_with("booleans/overlap/boxes"))
            .take(2)
            .collect();
        let r = run(make, &cases, 1, None, &Exports::default(), None).unwrap();
        assert_eq!(r["cases"][0]["outcome"], "refused");
        assert_eq!(r["cases"][0]["error"], "starting the kernel: no free core");
        // A kernel that did not start ran nothing, so the case says nothing of a start.
        assert!(r["cases"][0].get("cold").is_none());
        assert_eq!(r["cases"][1]["cold"], true);
        assert_ne!(r["cases"][1]["outcome"], "refused", "{}", r["cases"][1]);
        assert_eq!(starts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn a_case_is_cold_on_a_kernel_started_for_it_and_records_the_time_limit() {
        let cases: Vec<Case> = crate::generate::cases(1)
            .into_iter()
            .filter(|c| c.test.starts_with("booleans/overlap/boxes"))
            .take(4)
            .collect();
        let cold = |r: &Value| -> Vec<Value> {
            r["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["cold"].clone())
                .collect()
        };
        let r = run(
            || Ok::<_, String>(Sticky(Default::default())),
            &cases,
            1,
            None,
            &Exports::default(),
            Some(std::time::Duration::from_secs(9)),
        )
        .unwrap();
        assert_eq!(
            cold(&r),
            [json!(true), json!(false), json!(false), json!(false)]
        );
        assert_eq!(r["time_limit_ms"], 9000.0);
        assert!(r["wall_ms"].as_f64().unwrap() >= 0.0);
        let r = run(
            || Ok::<_, String>(Sticky(Default::default())),
            &cases,
            1,
            None,
            &Exports::default(),
            None,
        )
        .unwrap();
        assert_eq!(r["time_limit_ms"], Value::Null);
    }

    /// Two NIST files in a folder of their own, the first one the test kernel hangs on.
    fn nist_folder(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nist-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ap203")).unwrap();
        std::fs::write(
            dir.join("validation.json"),
            r#"{"ap203/a.stp": {}, "ap203/b.stp": {"volume": 1.0, "area": 7.0}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("ap203/a.stp"), "slow").unwrap();
        std::fs::write(dir.join("ap203/b.stp"), "quick").unwrap();
        dir
    }

    #[test]
    fn a_nist_case_is_sound_and_passes_when_the_kernel_reads_a_valid_solid() {
        let dir = nist_folder("read");
        let cases = crate::generate::nist_cases(&dir).unwrap();
        let r = run(
            || Ok::<_, String>(Sticky(Default::default())),
            &cases,
            1,
            None,
            &Exports::default(),
            None,
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let read = &r["cases"][1];
        assert_eq!(read["test"], "step/read-nist");
        assert_eq!(read["kind"], "sound");
        assert_eq!(read["result"], "1e-7", "{read}");
        let clauses: Vec<&str> = read["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["clause"].as_str().unwrap())
            .collect();
        assert_eq!(clauses, ["Q1", "Q2"]);
        // What the file states is shown beside what the kernel measured, and judged by no check.
        let op = &read["ops"][0];
        assert_eq!(op["op"], "read_step");
        assert_eq!(
            (op["volume"].as_f64(), op["stated_volume"].as_f64()),
            (Some(1.0), Some(1.0))
        );
        assert_eq!(op["agrees"], false);
        assert_eq!(read["file"], "ap203/b.stp");
    }

    #[test]
    fn a_nist_case_draws_the_solid_the_kernel_read_and_nothing_when_it_read_none() {
        let dir = nist_folder("drawn");
        let out = dir.join("files");
        let cases = crate::generate::nist_cases(&dir).unwrap();
        let ex = Exports {
            glb: Some(out.clone()),
            step: None,
        };
        let r = run(
            || Ok::<_, String>(Sticky(Default::default())),
            &cases,
            1,
            None,
            &ex,
            None,
        )
        .unwrap();
        let read = &r["cases"][1]["exports"];
        let key = read["glb"].as_str().unwrap_or_default().to_string();
        let bytes = std::fs::read(out.join(&key)).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(key.starts_with("step/read-nist/"), "{read}");
        let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let gltf: Value = serde_json::from_slice(&bytes[20..20 + n]).unwrap();
        let scenes: Vec<&str> = gltf["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap())
            .collect();
        assert_eq!(scenes, ["read"]);
        let roles: Vec<&Value> = gltf["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["extras"].get("role"))
            .collect();
        assert_eq!(roles, [&json!("main")]);
        // The file the kernel hung on has nothing of the kernel's to draw.
        assert!(
            r["cases"][0]["exports"]["glb"].is_null(),
            "{}",
            r["cases"][0]["exports"]
        );
    }

    #[test]
    fn a_nist_file_that_times_out_is_not_charged_to_the_files_after_it() {
        let dir = nist_folder("slow");
        let cases = crate::generate::nist_cases(&dir).unwrap();
        let r = run(
            || Ok::<_, String>(Sticky(Default::default())),
            &cases,
            1,
            None,
            &Exports::default(),
            None,
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(r["cases"][0]["result"], "timeout");
        assert_eq!(r["cases"][1]["result"], "1e-7", "{}", r["cases"][1]);
    }

    /// A kernel that makes and moves anything, and declines to place points in the way given.
    struct Declines(Kind);

    impl Kernel for Declines {
        type Solid = ();
        fn supports(&self) -> Vec<String> {
            brep_protocol::FUNCTIONS
                .iter()
                .map(|&f| f.to_string())
                .collect()
        }
        fn name(&self) -> String {
            "declines".into()
        }
        fn version(&self) -> String {
            "0".into()
        }
        fn tolerance(&self) -> f64 {
            TOLERANCE
        }
        fn make(&mut self, _p: &Primitive) -> Result<(), Refusal> {
            Ok(())
        }
        fn union(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn intersection(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn difference(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn volume(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn area(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn check(&mut self, _s: &()) -> Result<Vec<String>, Refusal> {
            Ok(Vec::new())
        }
        fn transform(&mut self, _s: &(), _m: [[f64; 4]; 3]) -> Result<(), Refusal> {
            Ok(())
        }
        fn classify(
            &mut self,
            _s: &(),
            _points: &[Vec3],
        ) -> Result<Vec<crate::model::Side>, Refusal> {
            Err(Refusal::new(self.0, "could not place the points"))
        }
    }

    #[test]
    fn a_classifier_that_fails_fails_the_membership_check_and_one_not_offered_leaves_it_out() {
        let all = cases(1);
        let c = all
            .iter()
            .find(|c| c.family == "transforms" && oracle::bounds(&c.a).is_some())
            .unwrap();
        let placing = |kind: Kind| -> Vec<Value> {
            let v = run_transform(&mut Declines(kind), c, c.matrix.unwrap());
            let checks = v["checks"].as_array().unwrap();
            let of_points = |ch: &&Value| ch["detail"].as_str().unwrap_or("").contains("classify");
            checks.iter().filter(of_points).cloned().collect()
        };
        for kind in [Kind::Failed, Kind::Invalid] {
            let found = placing(kind);
            assert_eq!(found.len(), 1, "{kind:?}");
            assert_eq!(found[0]["ok"], false);
            assert_eq!(found[0]["clause"], "B25");
        }
        assert!(placing(Kind::Unsupported).is_empty());
    }

    /// A kernel that makes anything, answers normals, and refuses every fillet.
    struct RefusesBlends;

    impl Kernel for RefusesBlends {
        type Solid = ();
        fn supports(&self) -> Vec<String> {
            brep_protocol::FUNCTIONS
                .iter()
                .map(|&f| f.to_string())
                .collect()
        }
        fn name(&self) -> String {
            "refuses-blends".into()
        }
        fn version(&self) -> String {
            "0".into()
        }
        fn tolerance(&self) -> f64 {
            TOLERANCE
        }
        fn make(&mut self, _p: &Primitive) -> Result<(), Refusal> {
            Ok(())
        }
        fn union(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn intersection(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn difference(&mut self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn volume(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn area(&mut self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn check(&mut self, _s: &()) -> Result<Vec<String>, Refusal> {
            Ok(Vec::new())
        }
        fn transform(&mut self, _s: &(), _m: [[f64; 4]; 3]) -> Result<(), Refusal> {
            Ok(())
        }
        fn normal(&mut self, _s: &(), points: &[Vec3]) -> Result<Vec<Option<Vec3>>, Refusal> {
            Ok(vec![None; points.len()])
        }
        fn fillet(&mut self, _s: &(), _edges: &[Vec3], _radius: f64) -> Result<(), Refusal> {
            Err(Refusal::failed("the ball is larger than its face can hold"))
        }
    }

    #[test]
    fn a_refused_fillet_leaves_its_smoothness_unchecked() {
        let all = cases(1);
        let c = all
            .iter()
            .find(|c| {
                matches!(&c.feature, Some(f) if f.act == Act::Fillet)
                    && c.exact.chain.is_none()
                    && matches!(&c.b, Shape::Prim(p) if !oracle::joints(p).is_empty())
            })
            .unwrap();
        let v = run_feature(&mut RefusesBlends, c, c.feature.as_ref().unwrap());
        assert_eq!(v["ops"][0]["outcome"], "refused", "{v}");
        let checks = v["checks"].as_array().unwrap();
        assert!(
            !checks
                .iter()
                .any(|ch| ch["clause"] == "B31" || ch["clause"] == "B39"),
            "{v}"
        );
    }

    #[test]
    fn singular_values_of_a_turned_stretch_and_a_shear() {
        // A stretch by 3, 2, 1 turned a quarter about x; the shear x ↦ x + y has the golden ratio
        // φ and 1/φ, and keeps z.
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12);
        let turned = [
            [3.0, 0.0, 0.0, 5.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
        ];
        assert!(near(singular_values(&turned), [3.0, 2.0, 1.0]));
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        let shear = [
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        assert!(near(singular_values(&shear), [phi, 1.0, 1.0 / phi]));
    }

    #[test]
    fn every_closed_form_meets_the_suites_own_sample() {
        // The families' formulas and the oracle share no code, so each checks the other: every
        // primitive's volume and every stated intersection volume must meet the sampled estimate.
        let mut bad = Vec::new();
        // Spline outlines' membership is slow to sample and their area has its own test.
        let spline = |s: &Shape| {
            matches!(s, Shape::Prim(Primitive::Extrude { profile, .. })
                if profile.loops.iter().flatten().any(|g| matches!(g, Seg::Spline(_))))
        };
        for c in cases(1)
            .into_iter()
            .filter(|c| !c.refuse && !spline(&c.a) && !spline(&c.b))
        {
            for (name, s) in [("a", &c.a), ("b", &c.b)] {
                if let (Shape::Prim(p), Some(bx)) = (s, oracle::bounds(s)) {
                    let (est, allow, pts, truth) = sampled_n(bx, 1024, |q| oracle::contains(s, q));
                    if (est - p.volume()).abs() > allow {
                        let v = p.volume();
                        bad.push(format!("{} {name}: {v} against {est} ± {allow}", c.id));
                    }
                    // Its exact moments, against the sample's.
                    let exact = crate::mass::of(p);
                    let (m, tol) = crate::mass::sampled(oracle::volume(bx), &pts, &truth);
                    for i in 0..3 {
                        let off = (exact.f[i] - m.f[i]).abs();
                        if off > tol.f[i] + 1e-9 * exact.f[i].abs() {
                            bad.push(format!("{} {name}: first moment {i} off by {off}", c.id));
                        }
                        for j in 0..3 {
                            let off = (exact.s[i][j] - m.s[i][j]).abs();
                            if off > tol.s[i][j] + 1e-9 * exact.s[i][j].abs() {
                                let id = &c.id;
                                bad.push(format!("{id} {name}: second moment {i}{j} off by {off}"));
                            }
                        }
                    }
                }
            }
            // A feature's stated result volume, against the sample of the shape it names.
            if let (Some(want), Some(bx)) = (c.exact.result_volume, oracle::bounds(&c.b)) {
                let (est, allow, _, _) = sampled_n(bx, 4096, |q| oracle::contains(&c.b, q));
                if (est - want).abs() > allow + 1e-9 * want {
                    bad.push(format!("{} result: {want} against {est} ± {allow}", c.id));
                }
            }
            let both = Shape::bool(Op::Intersection, c.a.clone(), c.b.clone());
            if let Some(want) = c.exact.intersection_volume {
                let (est, allow) = oracle::bounds(&both)
                    .map(|bx| sampled_n(bx, 1024, |q| oracle::contains(&both, q)))
                    .map_or((0.0, 0.0), |(e, a, _, _)| (e, a));
                if (est - want).abs() > allow + 1e-9 * want {
                    bad.push(format!("{} a ∩ b: {want} against {est} ± {allow}", c.id));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
