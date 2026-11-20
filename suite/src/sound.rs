//! Checks on a sound case's result where its definition leaves it open: that it lies between its
//! limits point for point, and that across its blend the boundary is smooth. Each reads the
//! result through the kernel's own queries, at points the suite chooses.

use brep_protocol::{Kernel, Refusal};

use crate::generate::{Limits, Scan};
use crate::model::{Vec3, add, scale};
use crate::oracle;

/// Points classified against a case's limits.
const SAMPLES: usize = 8192;

/// Rays each way across a scan's grid.
const GRID: usize = 24;

/// The turn of the boundary's normal, in radians, that is a crease when it happens within
/// [`CREASE_WITHIN`] of the blend's size: a surface would bend a hundred times tighter than the
/// blend to turn that far that fast.
const TURN: f64 = 0.1;
const CREASE_WITHIN: f64 = 1e-3;

/// The change of a principal curvature, in units of one over the blend's size, that is a jump
/// when it happens within [`JUMP_WITHIN`] of that size.
const JUMP: f64 = 0.25;
const JUMP_WITHIN: f64 = 1e-2;

/// How far, in units of the blend's size, a result may differ from itself moved by a motion that
/// leaves the case as it was.
const LOPSIDED: f64 = 1e-3;

/// Rays a scan may cast before it is given up as not settling.
const MOST_RAYS: usize = 60_000;

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn mid(a: Vec3, b: Vec3) -> Vec3 {
    scale(add(a, b), 0.5)
}

/// The angle between two unit vectors.
fn angle(a: Vec3, b: Vec3) -> f64 {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sin = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    sin.atan2(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
}

/// Whether the result holds every sample all acceptable results hold and none that none does,
/// over `within`: how many it places wrongly, and the first.
pub fn between<K: Kernel>(
    k: &mut K,
    s: &K::Solid,
    limits: &Limits,
    within: oracle::Bounds,
) -> Result<(bool, String), Refusal> {
    let asked: Vec<(Vec3, bool)> = oracle::samples(within, SAMPLES)
        .into_iter()
        .filter_map(|p| limits.at(p).map(|held| (p, held)))
        .collect();
    let pts: Vec<Vec3> = asked.iter().map(|a| a.0).collect();
    let got = k.holds(s, &pts)?;
    let wrong: Vec<&(Vec3, bool)> = asked
        .iter()
        .zip(&got)
        .filter(|(a, g)| a.1 != **g)
        .map(|(a, _)| a)
        .collect();
    let first = wrong.first().map_or(String::new(), |(p, held)| {
        let must = if *held {
            "must be held"
        } else {
            "must be left"
        };
        format!(": {p:?} {must}")
    });
    // One point placed differently is allowed, as it is of an exact case's samples.
    Ok((
        wrong.len() <= 1,
        format!(
            "{} of {} points on the wrong side{first}",
            wrong.len(),
            asked.len()
        ),
    ))
}

/// What a scan found.
#[derive(Default)]
pub struct Found {
    /// Points where the boundary's tangent plane breaks.
    pub creases: Vec<Vec3>,
    /// Points where its curvature jumps.
    pub jumps: Vec<Vec3>,
    /// Rays that stop elsewhere once moved by a motion that leaves the case as it was, of those
    /// compared.
    pub lopsided: usize,
    pub compared: usize,
    /// Rays cast.
    pub rays: usize,
    /// Points of the grid where the kernel gave no normal, or no curvature where it was asked,
    /// of those the grid's rays met.
    pub blind: usize,
    pub seen: usize,
}

/// Where one ray meets the boundary first, and the boundary's normal and curvature there.
#[derive(Clone, Copy)]
struct Seen {
    from: Vec3,
    at: Vec3,
    normal: Option<Vec3>,
    bend: Option<[f64; 2]>,
}

struct Scanner<'a, K: Kernel> {
    k: &'a mut K,
    s: &'a K::Solid,
    scan: &'a Scan,
    rays: usize,
}

impl<K: Kernel> Scanner<'_, K> {
    /// How far along each ray the boundary is first met.
    fn first(&mut self, rays: &[(Vec3, Vec3)]) -> Result<Vec<Option<f64>>, Refusal> {
        self.rays += rays.len();
        if self.rays > MOST_RAYS {
            return Err("the scan did not settle".into());
        }
        let all = self.k.raycasts(self.s, rays)?;
        if all.len() != rays.len() {
            return Err(Refusal::failed(format!(
                "{} answers for {} rays",
                all.len(),
                rays.len()
            )));
        }
        Ok(all
            .into_iter()
            .zip(rays)
            .map(|(ts, &(_, d))| crate::run::crossings(ts, d).into_iter().find(|&t| t > 0.0))
            .collect())
    }

    /// What the scan's rays from each of `from` see, in one question of each kind.
    fn see(&mut self, from: &[Vec3]) -> Result<Vec<Option<Seen>>, Refusal> {
        let rays: Vec<(Vec3, Vec3)> = from.iter().map(|&o| (o, self.scan.along)).collect();
        let reach = self.first(&rays)?;
        let mut seen: Vec<Option<Seen>> = from
            .iter()
            .zip(reach)
            .map(|(&from, t)| {
                t.map(|t| Seen {
                    from,
                    at: add(from, scale(self.scan.along, t)),
                    normal: None,
                    bend: None,
                })
            })
            .collect();
        let at: Vec<Vec3> = seen.iter().flatten().map(|g| g.at).collect();
        let normals = self.k.normal(self.s, &at)?;
        let bends = match self.scan.curvature {
            true => self.k.curvature(self.s, &at)?,
            false => vec![None; at.len()],
        };
        if normals.len() != at.len() || bends.len() != at.len() {
            return Err(Refusal::failed(format!(
                "{} answers for {} points",
                normals.len(),
                at.len()
            )));
        }
        for (g, (n, b)) in seen
            .iter_mut()
            .flatten()
            .zip(normals.into_iter().zip(bends))
        {
            (g.normal, g.bend) = (n, b);
        }
        Ok(seen)
    }

    /// A point between two neighbouring rays where `apart` says the boundary breaks. Each pair
    /// that disagrees is halved, all of them a round, until its two ends agree or lie within
    /// `within` and still do not. A pair with an end the kernel has no answer at, as it has none
    /// on an edge, is halved the same way and is no break by itself. Rays that close up while
    /// their points stay apart look past an outline, which is no break either.
    fn broken(
        &mut self,
        mut pairs: Vec<(Seen, Seen)>,
        apart: &dyn Fn(&Seen, &Seen) -> Option<bool>,
        within: f64,
    ) -> Result<Option<Vec3>, Refusal> {
        while !pairs.is_empty() {
            let mut open = Vec::new();
            for (a, b) in pairs {
                let differ = apart(&a, &b);
                if differ == Some(false) {
                    continue;
                }
                if dist(a.at, b.at) <= within {
                    if differ == Some(true) {
                        return Ok(Some(mid(a.at, b.at)));
                    }
                } else if dist(a.from, b.from) > 1e-3 * within {
                    open.push((a, b));
                }
            }
            let middles: Vec<Vec3> = open.iter().map(|(a, b)| mid(a.from, b.from)).collect();
            let seen = self.see(&middles)?;
            pairs = open
                .into_iter()
                .zip(seen)
                .filter_map(|((a, b), m)| m.map(|m| [(a, m), (m, b)]))
                .flatten()
                .collect();
        }
        Ok(None)
    }
}

/// Scans the result across one grid of rays: for a break in its tangent plane, for a jump in its
/// curvature where the blend is to carry it, and for a difference from itself moved.
pub fn scan<K: Kernel>(k: &mut K, s: &K::Solid, scan: &Scan) -> Result<Found, Refusal> {
    let mut sc = Scanner {
        k,
        s,
        scan,
        rays: 0,
    };
    let step = |i: usize| scan.half * (2.0 * i as f64 / (GRID - 1) as f64 - 1.0);
    // The grid is round: its corners would reach half as far again along the boundary.
    let node = |i: usize, j: usize| {
        (step(i).hypot(step(j)) <= scan.half).then(|| {
            add(
                scan.from,
                add(
                    scale(scan.across[0], step(i)),
                    scale(scan.across[1], step(j)),
                ),
            )
        })
    };
    let nodes: Vec<(usize, Vec3)> = (0..GRID * GRID)
        .filter_map(|n| node(n / GRID, n % GRID).map(|from| (n, from)))
        .collect();
    let from: Vec<Vec3> = nodes.iter().map(|n| n.1).collect();
    let mut grid: Vec<Option<Seen>> = vec![None; GRID * GRID];
    for (&(n, _), seen) in nodes.iter().zip(sc.see(&from)?) {
        grid[n] = seen;
    }
    let mut found = Found::default();
    for g in grid.iter().flatten() {
        found.seen += 1;
        if g.normal.is_none() || (scan.curvature && g.bend.is_none()) {
            found.blind += 1;
        }
    }
    let mut pairs = Vec::new();
    for i in 0..GRID {
        for j in 0..GRID {
            for (di, dj) in [(1, 0), (0, 1)] {
                if i + di < GRID
                    && j + dj < GRID
                    && let (Some(a), Some(b)) = (grid[i * GRID + j], grid[(i + di) * GRID + j + dj])
                {
                    pairs.push((a, b));
                }
            }
        }
    }
    let turned = |a: &Seen, b: &Seen| Some(angle(a.normal?, b.normal?) > TURN);
    let jumped = |a: &Seen, b: &Seen| {
        let (x, y) = (a.bend?, b.bend?);
        Some((0..2).any(|i| (x[i] - y[i]).abs() > JUMP / scan.size))
    };
    found
        .creases
        .extend(sc.broken(pairs.clone(), &turned, CREASE_WITHIN * scan.size)?);
    if scan.curvature {
        found
            .jumps
            .extend(sc.broken(pairs, &jumped, JUMP_WITHIN * scan.size)?);
    }
    let seen: Vec<Seen> = grid.into_iter().flatten().collect();
    for m in &scan.same {
        let turn = |v: Vec3| [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2]);
        let along = turn(scan.along);
        let moved: Vec<(Vec3, Vec3)> = seen
            .iter()
            .map(|g| (add(turn(g.from), [m[0][3], m[1][3], m[2][3]]), along))
            .collect();
        for (g, other) in seen.iter().zip(sc.first(&moved)?) {
            found.compared += 1;
            let same =
                other.is_some_and(|o| (dist(g.from, g.at) - o).abs() <= LOPSIDED * scan.size);
            if !same {
                found.lopsided += 1;
            }
        }
    }
    found.rays = sc.rays;
    Ok(found)
}
