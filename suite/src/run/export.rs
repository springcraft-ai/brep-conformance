//! A run's shape files: each case's views as one GLB, and its expected shape as a STEP file where
//! it has one, all drawn and written by the kernel under test. A view's subject is the kernel's own result
//! where its runner kept one, and otherwise the shape the view names, built by the kernel. A ghost
//! is the part of an operand outside the view's result, also by the kernel: their difference,
//! less every face and edge on the result's boundary.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use brep_protocol::{Display, Kernel, Refusal};
use serde_json::{Value, json};

use super::{attempt, build};
use crate::assets;
use crate::generate::Case;
use crate::model::{Op, Primitive, Shape, Side};
use crate::oracle;

/// A solid is drawn within this fraction of its diagonal, so one microns across and one metres
/// across both show their shape at a page's scale.
pub const DRAWN: f64 = 5e-4;

/// The chord tolerance for a solid whose size the case does not give, mm.
pub const EXPORT_TOLERANCE: f64 = 0.01;

/// Where a run writes its shape files; either may be absent.
#[derive(Clone, Debug, Default)]
pub struct Exports {
    pub glb: Option<PathBuf>,
    pub step: Option<PathBuf>,
}

impl Exports {
    pub fn any(&self) -> bool {
        self.glb.is_some() || self.step.is_some()
    }
}

/// The results a case's runner kept, by view and role.
type Kept = HashMap<(String, String), Box<dyn Any>>;

thread_local! {
    static KEPT: RefCell<Option<Kept>> =
        const { RefCell::new(None) };
}

/// Starts keeping results for the case this thread runs next.
pub(super) fn begin() {
    KEPT.with(|k| *k.borrow_mut() = Some(HashMap::new()));
}

/// Keeps `s` as what the view `view` shows in `role`, when the case exports. A handle costs
/// nothing to keep; the kernel draws it after the case is judged, outside every timed call.
pub(super) fn keep<S: Clone + 'static>(view: &str, role: &str, s: &S) {
    KEPT.with(|k| {
        if let Some(m) = k.borrow_mut().as_mut() {
            m.insert((view.into(), role.into()), Box::new(s.clone()));
        }
    });
}

fn take<S: 'static>() -> HashMap<(String, String), S> {
    let m = KEPT.with(|k| k.borrow_mut().take()).unwrap_or_default();
    m.into_iter()
        .filter_map(|(key, v)| v.downcast::<S>().ok().map(|s| (key, *s)))
        .collect()
}

/// A shape of the view table: a primitive, or a boolean of two shapes.
fn shape_of(v: &Value) -> Result<Shape, String> {
    if v["kind"] == "boolean" {
        let op = match v["op"].as_str() {
            Some("union") => Op::Union,
            Some("intersection") => Op::Intersection,
            Some("difference") => Op::Difference,
            _ => return Err(format!("no op {}", v["op"])),
        };
        return Ok(Shape::bool(op, shape_of(&v["a"])?, shape_of(&v["b"])?));
    }
    Primitive::from_json(v)
        .map(Shape::Prim)
        .map_err(|e| format!("{}: {e}", v["kind"]))
}

/// A solid of a view by its reference: a shape of the table by index, or `[op, i, j]`.
fn shape_at(shapes: &[Value], r: &Value) -> Result<Shape, String> {
    match r {
        Value::Array(v) => match v.as_slice() {
            [op, a, b] => shape_of(&json!({"kind": "boolean", "op": op,
                "a": shapes.get(a.as_u64().unwrap_or(u64::MAX) as usize).ok_or("no shape")?,
                "b": shapes.get(b.as_u64().unwrap_or(u64::MAX) as usize).ok_or("no shape")?})),
            _ => Err(format!("a solid of {r}")),
        },
        i => shapes
            .get(i.as_u64().unwrap_or(u64::MAX) as usize)
            .ok_or_else(|| format!("no shape {i}"))
            .and_then(shape_of),
    }
}

fn written(e: super::Ran) -> String {
    format!("{}: {}", e.outcome().word(), e.message())
}

/// The kernel's solid for a reference, built once per case.
fn solid<K: Kernel<Solid: Clone>>(
    k: &K,
    shapes: &[Value],
    built: &mut HashMap<String, Result<K::Solid, String>>,
    r: &Value,
) -> Result<K::Solid, String> {
    let key = r.to_string();
    if let Some(s) = built.get(&key) {
        return s.clone();
    }
    let s = shape_at(shapes, r).and_then(|shape| attempt(|| build(k, &shape)).map_err(written));
    built.insert(key, s.clone());
    s
}

/// The part of `a` outside `b` as the kernel draws it: `a ∖ b`, less each face and edge that lies
/// on `b`'s boundary, so the ghost ends where the result's faces begin.
fn ghost<K: Kernel>(k: &K, a: &K::Solid, b: &K::Solid, tol: f64) -> Result<Display, String> {
    let rest = attempt(|| k.difference(a, b)).map_err(written)?;
    let d = attempt(|| k.display(&rest, tol)).map_err(written)?;
    // A point inside each face: the middle of its largest triangle, put back on the face.
    let middles: Vec<[f64; 3]> = d
        .faces
        .iter()
        .map(|f| {
            let area = |t: &[u32; 3]| {
                let [p, q, r] = t.map(|i| f.positions[i as usize]);
                let (u, v) = (
                    [0, 1, 2].map(|j| q[j] - p[j]),
                    [0, 1, 2].map(|j| r[j] - p[j]),
                );
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                n[0] * n[0] + n[1] * n[1] + n[2] * n[2]
            };
            f.triangles
                .iter()
                .max_by(|s, t| area(s).total_cmp(&area(t)))
                .map_or([0.0; 3], |t| {
                    let ps = t.map(|i| f.positions[i as usize]);
                    [0, 1, 2].map(|j| (ps[0][j] + ps[1][j] + ps[2][j]) / 3.0)
                })
        })
        .collect();
    let on_faces = attempt(|| k.closest(&rest, &middles))
        .ok()
        .and_then(|p| k.classify(b, &p).ok());
    let mids: Vec<[f64; 3]> = d
        .edges
        .iter()
        .map(|e| e.get(e.len() / 2).copied().unwrap_or([0.0; 3]))
        .collect();
    let on_edges = k.classify(b, &mids).ok();
    let keep = |sides: &Option<Vec<Side>>, i: usize| {
        sides
            .as_ref()
            .and_then(|s| s.get(i))
            .is_none_or(|s| *s != Side::On)
    };
    Ok(Display {
        faces: d
            .faces
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep(&on_faces, *i))
            .map(|(_, f)| f)
            .collect(),
        edges: d
            .edges
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep(&on_edges, *i))
            .map(|(_, e)| e)
            .collect(),
    })
}

fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path)
}

/// Writes the case's files and records them in its result `v` as `exports`: the GLB's key, the
/// expected STEP file's key, what could not be drawn or written as `errors`, and as `skipped` a ghost the
/// kernel could not cut or a refused case's input it would not make.
pub(super) fn export_case<K: Kernel<Solid: Clone + 'static>>(
    k: &K,
    c: &Case,
    ex: &Exports,
    v: &mut Value,
) {
    let kept: HashMap<(String, String), K::Solid> = take();
    let spec = crate::views::views(c);
    let shapes = spec["shapes"].as_array().map_or(&[][..], |v| v.as_slice());
    let hash = assets::hash(c);
    let subjects = if c.refuse {
        Vec::new()
    } else {
        assets::subjects(&spec)
    };
    let roles: Vec<&str> = subjects.iter().map(|(_, r)| r.as_str()).collect();
    let step_keys = assets::step_keys(c.test, &c.id, &hash, &roles);
    let whole = oracle::bounds(&Shape::bool(Op::Union, c.a.clone(), c.b.clone()));
    let tolerance = |r: &Value| {
        let bx = shape_at(shapes, r)
            .ok()
            .and_then(|s| oracle::bounds(&s))
            .or(whole);
        bx.map_or(EXPORT_TOLERANCE, |(lo, hi)| {
            let d = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt();
            (DRAWN * d).max(1e-9)
        })
    };
    let mut built: HashMap<String, Result<K::Solid, String>> = HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut steps: Vec<String> = Vec::new();
    let views = spec["views"].as_array().cloned().unwrap_or_default();
    // Each view's solids as drawn, in the view's order: (key, role, display).
    let mut drawn: Vec<Vec<Option<(String, String, Display)>>> = views
        .iter()
        .map(|v| vec![None; v["solids"].as_array().map_or(0, Vec::len)])
        .collect();
    let solids = |view: &Value| view["solids"].as_array().cloned().unwrap_or_default();
    let outside = |r: &Value| -> Option<(Value, Value)> {
        match r.as_array().map(|v| v.as_slice()) {
            Some([tag, a, b]) if tag == "outside" => Some((a.clone(), b.clone())),
            _ => None,
        }
    };
    // Subjects first, so a ghost that runs past its limit and loses the worker's solids costs only
    // the ghosts.
    for (vi, view) in views.iter().enumerate() {
        let name = view["name"].as_str().unwrap_or("view");
        for (si, s) in solids(view).iter().enumerate() {
            let role = s["role"].as_str().unwrap_or("main");
            let what = format!("{name} {role}");
            let r = &s["s"];
            if outside(r).is_some() {
                continue;
            }
            let kept_here = kept.get(&(name.to_string(), role.to_string()));
            let own = view["own"] == true;
            let (key, made) = match kept_here {
                Some(x) => (format!("kept {name} {role}"), Ok(x.clone())),
                // A view of the kernel's own result: what the kernel did not make is not drawn.
                None if own => continue,
                None => {
                    let base = solid(k, shapes, &mut built, r);
                    let m: Option<[[f64; 4]; 3]> = serde_json::from_value(s["matrix"].clone()).ok();
                    let placed = match (base, m) {
                        (Ok(x), Some(m)) => {
                            attempt(|| k.transform(&x, m).or_else(|_: Refusal| k.affine(&x, m)))
                                .map_err(written)
                        }
                        (b, _) => b,
                    };
                    (format!("{r}|{}", s["matrix"]), placed)
                }
            };
            let made = match made {
                Ok(x) => x,
                // A case that must be refused shows an input the kernel rightly will not make.
                Err(e) if c.refuse => {
                    skipped.push(format!("{what}: {e}"));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("{what}: {e}"));
                    continue;
                }
            };
            if ex.glb.is_some() {
                // A mesher can fail at one tolerance and not another: try once more at half,
                // and keep the first failure on record. A kernel's own result is measured by its
                // own box, as the suite's shape for the view only stands in for its size.
                let tol = if own {
                    attempt(|| k.bounding_box(&made)).map_or(EXPORT_TOLERANCE, |[lo, hi]| {
                        let d = (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f64>().sqrt();
                        (DRAWN * d).max(1e-9)
                    })
                } else {
                    tolerance(r)
                };
                let d = attempt(|| k.display(&made, tol)).or_else(|e| {
                    skipped.push(format!("{what}: display at {tol:.2e} {}", written(e)));
                    attempt(|| k.display(&made, tol / 2.0))
                });
                match d {
                    Ok(d) => drawn[vi][si] = Some((key, role.to_string(), d)),
                    Err(e) => errors.push(format!("{what}: display {}", written(e))),
                }
            }
            if let Some(dir) = &ex.step
                && vi == 0
                && let Some(key) = subjects
                    .iter()
                    .position(|(i, _)| *i == si)
                    .and_then(|n| step_keys.get(n))
            {
                let key = key.clone();
                match attempt(|| k.write_step(&made)) {
                    Ok(text) => match write(&dir.join(&key), text.as_bytes()) {
                        Ok(()) => steps.push(key),
                        Err(e) => errors.push(format!("{key}: {e}")),
                    },
                    Err(e) => errors.push(format!("{what}: write_step {}", written(e))),
                }
            }
        }
    }
    // Then the ghosts, which are context: one the kernel cannot cut is left out of its view.
    if ex.glb.is_some() {
        for (vi, view) in views.iter().enumerate() {
            let name = view["name"].as_str().unwrap_or("view");
            for (si, s) in solids(view).iter().enumerate() {
                let role = s["role"].as_str().unwrap_or("main");
                let r = &s["s"];
                let Some((a, b)) = outside(r) else { continue };
                // An operand lies wholly inside a union of it: nothing of it is outside.
                if let Some([op, i, j]) = b.as_array().map(|v| v.as_slice())
                    && op == "union"
                    && (*i == a || *j == a)
                {
                    continue;
                }
                // Cut against what the view shows: the kernel's own result where it kept one.
                let shown = match kept.get(&(name.to_string(), "main".to_string())) {
                    Some(x) => Ok(x.clone()),
                    None => solid(k, shapes, &mut built, &b),
                };
                let tol = tolerance(&a);
                let g = solid(k, shapes, &mut built, &a)
                    .and_then(|x| Ok((x, shown?)))
                    .and_then(|(x, y)| ghost(k, &x, &y, tol));
                match g {
                    Ok(d) => drawn[vi][si] = Some((format!("ghost {r}"), role.to_string(), d)),
                    Err(e) => skipped.push(format!("{name} {role}: {e}")),
                }
            }
        }
    }
    let drawn: Vec<Vec<(String, String, Display)>> = drawn
        .into_iter()
        .map(|row| row.into_iter().flatten().collect())
        .collect();
    let mut glb_key = Value::Null;
    // A kernel without `display` draws nothing: it gets no GLB, not an empty one.
    let anything = drawn.iter().flatten().any(|(_, _, d)| !d.faces.is_empty());
    if let Some(dir) = &ex.glb
        && anything
    {
        let mut extras = json!({"id": c.id, "test": c.test, "hash": hash, "unit": "mm",
            "kernel": {"name": k.name(), "version": k.version()},
            "outcome": v["outcome"], "result": v["result"]});
        for &key in crate::views::OVERLAY {
            if !spec[key].is_null() {
                extras[key] = spec[key].clone();
            }
        }
        let gv: Vec<crate::glb::View> = views
            .iter()
            .zip(&drawn)
            .map(|(view, row)| crate::glb::View {
                name: view["name"].as_str().unwrap_or("view"),
                note: view["note"].as_str(),
                alt: view["alt"].as_str(),
                solids: row
                    .iter()
                    .map(|(key, role, d)| crate::glb::Solid {
                        key: key.clone(),
                        role,
                        display: d,
                    })
                    .collect(),
            })
            .collect();
        let generator = format!("brep-conformance {}", super::ASSET_GENERATOR);
        let bytes = crate::glb::glb(&gv, extras, &generator);
        let key = assets::key(c.test, &c.id, &hash, "glb");
        match write(&dir.join(&key), &bytes) {
            Ok(()) => glb_key = json!(key),
            Err(e) => errors.push(format!("{key}: {e}")),
        }
    }
    v["exports"] = json!({"glb": glb_key, "step": steps, "errors": errors, "skipped": skipped});
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_solid_a_view_names_resolves_to_a_shape_or_is_kept() {
        let mut checked = 0;
        for c in crate::generate::cases(1) {
            let spec = crate::views::views(&c);
            let shapes = spec["shapes"].as_array().unwrap();
            for v in spec["views"].as_array().unwrap() {
                for s in v["solids"].as_array().unwrap() {
                    let r = &s["s"];
                    let refs: Vec<&Value> = match r.as_array().map(|v| v.as_slice()) {
                        Some([tag, a, b]) if tag == "outside" => vec![a, b],
                        _ => vec![r],
                    };
                    for r in refs {
                        let flat = r
                            .as_u64()
                            .is_some_and(|i| shapes[i as usize]["kind"] == "many");
                        // The flat many-solid boolean is only ever the kept result of its runner.
                        if flat {
                            assert_eq!(v["name"], "result", "{}", c.id);
                            continue;
                        }
                        if let Err(e) = shape_at(shapes, r) {
                            panic!("{} {}: {e}", c.id, v["name"]);
                        }
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 10_000, "{checked}");
    }

    #[test]
    fn every_extra_a_view_lays_over_its_drawing_reaches_the_file() {
        for c in crate::generate::cases(1) {
            let spec = crate::views::views(&c);
            for key in spec.as_object().unwrap().keys() {
                if ["views", "cell", "shapes"].contains(&key.as_str()) {
                    continue;
                }
                assert!(
                    crate::views::OVERLAY.contains(&key.as_str()),
                    "{}: {key}",
                    c.id
                );
            }
        }
    }
}
