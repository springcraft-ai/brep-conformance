//! Answering the protocol with a [`Kernel`]: one JSON request per line on stdin, one reply per
//! line on stdout, solids kept here between requests under integer handles.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

use crate::model::{Op, Primitive};
use crate::{FUNCTIONS, Kernel, PROTOCOL_VERSION, Refusal};

fn op(v: &Value) -> Result<Op, Refusal> {
    match v.as_str() {
        Some("union") => Ok(Op::Union),
        Some("intersection") => Ok(Op::Intersection),
        Some("difference") => Ok(Op::Difference),
        _ => Err(Refusal::invalid(format!("unknown op {v}"))),
    }
}

/// The constructor commands, each a function of the spec.
const CONSTRUCTORS: [&str; 12] = [
    "box",
    "cylinder",
    "sphere",
    "torus",
    "cone",
    "extrude",
    "revolve",
    "screw",
    "loft",
    "sweep",
    "slab",
    "sweep_path",
];

fn handle(v: &Value) -> Result<u64, Refusal> {
    v.as_u64()
        .ok_or_else(|| Refusal::failed(format!("{v} is not a handle")))
}

/// Answers requests until stdin closes. A refusal replies `err`; a panic replies `crash`, and the
/// worker carries on.
pub fn serve<K: Kernel>(k: &K) -> std::io::Result<()> {
    let mut solids: HashMap<u64, K::Solid> = HashMap::new();
    let mut next = 1u64;
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let reply = match serde_json::from_str::<Value>(&line) {
            Err(e) => json!({"err": format!("not JSON: {e}")}),
            Ok(req) => {
                let r = catch_unwind(AssertUnwindSafe(|| answer(k, &req, &mut solids, &mut next)));
                match r {
                    Ok(Ok(v)) => json!({"ok": v}),
                    Ok(Err(e)) => json!({"err": e.reason, "kind": e.kind.word()}),
                    Err(p) => {
                        let msg = p
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "panic".into());
                        json!({"crash": msg})
                    }
                }
            }
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    std::panic::set_hook(quiet);
    Ok(())
}

fn answer<K: Kernel>(
    k: &K,
    req: &Value,
    solids: &mut HashMap<u64, K::Solid>,
    next: &mut u64,
) -> Result<Value, Refusal> {
    let solid = |key: &str| {
        let h = handle(&req[key])?;
        solids
            .get(&h)
            .ok_or_else(|| Refusal::failed(format!("no solid {h}")))
    };
    let made = match req["cmd"].as_str().unwrap_or("") {
        "hello" => {
            let without = k.unsupported();
            let supports: Vec<&str> = FUNCTIONS
                .iter()
                .copied()
                .filter(|f| !without.iter().any(|u| u == f))
                .collect();
            return Ok(json!({
                "name": k.name(), "version": k.version(), "tolerance": k.tolerance(),
                "protocol": PROTOCOL_VERSION, "supports": supports,
                "configuration": k.configuration(),
            }));
        }
        "betti" => return Ok(json!(k.betti(solid("s")?)?)),
        "normal" => {
            let points: Vec<[f64; 3]> = serde_json::from_value(req["points"].clone())
                .map_err(|e| format!("points: {e}"))?;
            return Ok(json!(k.normal(solid("s")?, &points)?));
        }
        "interference" => {
            let m = k.interference(solid("a")?, solid("b")?)?;
            return Ok(json!({"volume": m.volume, "gap": m.gap}));
        }
        c @ ("union_all" | "difference_all") => {
            let list = |key: &str| -> Result<Vec<&K::Solid>, Refusal> {
                let hs = req[key]
                    .as_array()
                    .ok_or_else(|| format!("`{key}` is not a list"))?;
                hs.iter()
                    .map(|v| {
                        let h = handle(v)?;
                        solids
                            .get(&h)
                            .ok_or_else(|| Refusal::failed(format!("no solid {h}")))
                    })
                    .collect()
            };
            let made = if c == "union_all" {
                k.union_all(&list("solids")?)?
            } else {
                k.difference_all(solid("target")?, &list("tools")?)?
            };
            let h = *next;
            *next += 1;
            solids.insert(h, made);
            return Ok(json!(h));
        }
        "offset_faces" => {
            let faces: Vec<[f64; 3]> =
                serde_json::from_value(req["faces"].clone()).map_err(|e| format!("faces: {e}"))?;
            let d = req["distance"]
                .as_f64()
                .ok_or("`distance` is not a number")?;
            let made = k.offset_faces(solid("s")?, &faces, d)?;
            let h = *next;
            *next += 1;
            solids.insert(h, made);
            return Ok(json!(h));
        }
        "raycast" => {
            if !req["rays"].is_null() {
                let rays: Vec<([f64; 3], [f64; 3])> = serde_json::from_value(req["rays"].clone())
                    .map_err(|e| format!("rays: {e}"))?;
                return Ok(json!(k.raycasts(solid("s")?, &rays)?));
            }
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            return Ok(json!(k.raycast(
                solid("s")?,
                v("origin")?,
                v("direction")?
            )?));
        }
        "transform" => {
            let m: [[f64; 4]; 3] = serde_json::from_value(req["matrix"].clone())
                .map_err(|e| format!("matrix: {e}"))?;
            k.transform(solid("s")?, m)?
        }
        "affine" => {
            let m: [[f64; 4]; 3] = serde_json::from_value(req["matrix"].clone())
                .map_err(|e| format!("matrix: {e}"))?;
            k.affine(solid("s")?, m)?
        }
        "section_area" => {
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            return Ok(json!(k.section_area(
                solid("s")?,
                v("origin")?,
                v("normal")?
            )?));
        }
        "mesh" => {
            let tol = req["tolerance"]
                .as_f64()
                .ok_or("`tolerance` is not a number")?;
            let m = k.mesh(solid("s")?, tol)?;
            return Ok(json!({"positions": m.positions, "triangles": m.triangles}));
        }
        "display" => {
            let tol = req["tolerance"]
                .as_f64()
                .ok_or("`tolerance` is not a number")?;
            let d = k.display(solid("s")?, tol)?;
            let faces: Vec<Value> = d
                .faces
                .iter()
                .map(|f| {
                    json!({"positions": f.positions, "normals": f.normals,
                           "triangles": f.triangles})
                })
                .collect();
            return Ok(json!({"faces": faces, "edges": d.edges}));
        }
        c @ ("delete_faces" | "move_faces" | "thicken") => {
            let faces: Vec<[f64; 3]> =
                serde_json::from_value(req["faces"].clone()).map_err(|e| format!("faces: {e}"))?;
            match c {
                "delete_faces" => k.delete_faces(solid("s")?, &faces)?,
                "move_faces" => {
                    let m: [[f64; 4]; 3] = serde_json::from_value(req["matrix"].clone())
                        .map_err(|e| format!("matrix: {e}"))?;
                    k.move_faces(solid("s")?, &faces, m)?
                }
                _ => {
                    let t = req["thickness"]
                        .as_f64()
                        .ok_or("`thickness` is not a number")?;
                    k.thicken(solid("s")?, &faces, t)?
                }
            }
        }
        c @ ("split" | "section") => {
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            if c == "section" {
                let tol = req["tolerance"]
                    .as_f64()
                    .ok_or("`tolerance` is not a number")?;
                return Ok(json!(k.section(
                    solid("s")?,
                    v("origin")?,
                    v("normal")?,
                    tol
                )?));
            }
            let (above, below) = k.split(solid("s")?, v("origin")?, v("normal")?)?;
            let mut out = Vec::new();
            for s in [above, below] {
                solids.insert(*next, s);
                out.push(*next);
                *next += 1;
            }
            return Ok(json!(out));
        }
        "bounding_box" => return Ok(json!(k.bounding_box(solid("s")?)?)),
        "sew" => {
            let text = req["text"].as_str().ok_or("`text` is not a string")?;
            let tol = req["tolerance"]
                .as_f64()
                .ok_or("`tolerance` is not a number")?;
            k.sew(text, tol)?
        }
        "offset" => {
            let d = req["distance"]
                .as_f64()
                .ok_or("`distance` is not a number")?;
            k.offset(solid("s")?, d)?
        }
        "shell" => {
            let open: Vec<[f64; 3]> =
                serde_json::from_value(req["open"].clone()).map_err(|e| format!("open: {e}"))?;
            let t = req["thickness"]
                .as_f64()
                .ok_or("`thickness` is not a number")?;
            k.shell(solid("s")?, &open, t)?
        }
        "draft" => {
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            let faces: Vec<[f64; 3]> =
                serde_json::from_value(req["faces"].clone()).map_err(|e| format!("faces: {e}"))?;
            let angle = req["angle"].as_f64().ok_or("`angle` is not a number")?;
            k.draft(solid("s")?, &faces, v("origin")?, v("pull")?, angle)?
        }
        "reset" => {
            solids.clear();
            return Ok(Value::Null);
        }
        "volume" => return Ok(json!(k.volume(solid("s")?)?)),
        "area" => return Ok(json!(k.area(solid("s")?)?)),
        "check" => return Ok(json!(k.check(solid("s")?)?)),
        "write_step" => return Ok(json!(k.write_step(solid("s")?)?)),
        "read_step" => k.read_step(req["text"].as_str().ok_or("`text` is not a string")?)?,
        "centroid" => return Ok(json!(k.centroid(solid("s")?)?)),
        "inertia" => return Ok(json!(k.inertia(solid("s")?)?)),
        "distance" => return Ok(json!(k.distance(solid("a")?, solid("b")?)?)),
        "classify" => {
            let points: Vec<[f64; 3]> = serde_json::from_value(req["points"].clone())
                .map_err(|e| format!("points: {e}"))?;
            let sides = k.classify(solid("s")?, &points)?;
            let words: Vec<&str> = sides.into_iter().map(crate::model::Side::word).collect();
            return Ok(json!(words));
        }
        "closest" => {
            let points: Vec<[f64; 3]> = serde_json::from_value(req["points"].clone())
                .map_err(|e| format!("points: {e}"))?;
            return Ok(json!(k.closest(solid("s")?, &points)?));
        }
        c @ ("union" | "intersection" | "difference") => {
            k.boolean(op(&json!(c))?, solid("a")?, solid("b")?)?
        }
        "curvature" => {
            let points: Vec<[f64; 3]> = serde_json::from_value(req["points"].clone())
                .map_err(|e| format!("points: {e}"))?;
            return Ok(json!(k.curvature(solid("s")?, &points)?));
        }
        "project" => {
            let d: [f64; 3] = serde_json::from_value(req["direction"].clone())
                .map_err(|e| format!("direction: {e}"))?;
            let tol = req["tolerance"]
                .as_f64()
                .ok_or("`tolerance` is not a number")?;
            let p = k.project(solid("s")?, d, tol)?;
            return Ok(json!({"visible": p.visible, "hidden": p.hidden}));
        }
        c @ ("fillet_g2" | "fillet_conic" | "fillet_setback" | "fillet_setback_g2") => {
            let edges: Vec<[f64; 3]> =
                serde_json::from_value(req["edges"].clone()).map_err(|e| format!("edges: {e}"))?;
            let d = |key: &str| {
                req[key]
                    .as_f64()
                    .ok_or_else(|| Refusal::failed(format!("`{key}` is not a number")))
            };
            match c {
                "fillet_g2" => k.fillet_g2(solid("s")?, &edges, d("offset")?, d("center")?)?,
                "fillet_conic" => k.fillet_conic(solid("s")?, &edges, d("setback")?, d("rho")?)?,
                "fillet_setback" => {
                    k.fillet_setback(solid("s")?, &edges, d("radius")?, d("setback")?)?
                }
                _ => k.fillet_setback_g2(solid("s")?, &edges, d("radius")?, d("setback")?)?,
            }
        }
        c @ ("fillet_faces" | "full_round") => {
            let face = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            if c == "fillet_faces" {
                let radius = req["radius"].as_f64().ok_or("`radius` is not a number")?;
                k.fillet_faces(solid("s")?, face("face_a")?, face("face_b")?, radius)?
            } else {
                k.full_round(
                    solid("s")?,
                    face("side_a")?,
                    face("middle")?,
                    face("side_b")?,
                )?
            }
        }
        "fillet_variable" => {
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            let d = |key: &str| {
                req[key]
                    .as_f64()
                    .ok_or_else(|| Refusal::failed(format!("`{key}` is not a number")))
            };
            k.fillet_variable(
                solid("s")?,
                v("edge")?,
                v("from")?,
                d("r_from")?,
                d("r_to")?,
            )?
        }
        "loft_through" => {
            let frame = crate::wire::frame(&req["frame"])?;
            let profiles = req["profiles"]
                .as_array()
                .ok_or("`profiles` is not a list")?
                .iter()
                .map(crate::profile::Profile::from_json)
                .collect::<Result<Vec<_>, _>>()?;
            let heights: Vec<f64> = serde_json::from_value(req["heights"].clone())
                .map_err(|e| format!("heights: {e}"))?;
            if heights.len() != profiles.len() {
                return Err(Refusal::invalid(format!(
                    "{} profiles but {} heights",
                    profiles.len(),
                    heights.len()
                )));
            }
            k.loft_through(&frame, &profiles, &heights)?
        }
        "chamfer_two" => {
            let v = |key: &str| -> Result<[f64; 3], Refusal> {
                serde_json::from_value(req[key].clone())
                    .map_err(|e| Refusal::failed(format!("{key}: {e}")))
            };
            let d = |key: &str| {
                req[key]
                    .as_f64()
                    .ok_or_else(|| Refusal::failed(format!("`{key}` is not a number")))
            };
            k.chamfer_two(solid("s")?, v("edge")?, v("face")?, d("d1")?, d("d2")?)?
        }
        c @ ("fillet_radii" | "chamfer_distances") => {
            let key = if c == "fillet_radii" {
                "radii"
            } else {
                "distances"
            };
            let edges: Vec<[f64; 3]> =
                serde_json::from_value(req["edges"].clone()).map_err(|e| format!("edges: {e}"))?;
            let sizes: Vec<f64> =
                serde_json::from_value(req[key].clone()).map_err(|e| format!("{key}: {e}"))?;
            if sizes.len() != edges.len() {
                return Err(Refusal::failed(format!(
                    "{} edges but {} {key}",
                    edges.len(),
                    sizes.len()
                )));
            }
            if c == "fillet_radii" {
                k.fillet_radii(solid("s")?, &edges, &sizes)?
            } else {
                k.chamfer_distances(solid("s")?, &edges, &sizes)?
            }
        }
        c @ ("fillet" | "chamfer") => {
            let edges: Vec<[f64; 3]> =
                serde_json::from_value(req["edges"].clone()).map_err(|e| format!("edges: {e}"))?;
            let size = |key: &str| {
                req[key]
                    .as_f64()
                    .ok_or_else(|| Refusal::failed(format!("`{key}` is not a number")))
            };
            if c == "fillet" {
                k.fillet(solid("s")?, &edges, size("radius")?)?
            } else {
                k.chamfer(solid("s")?, &edges, size("distance")?)?
            }
        }
        c if CONSTRUCTORS.contains(&c) => k.make(&Primitive::from_json(req)?)?,
        c => return Err(Refusal::unsupported(format!("unknown command `{c}`"))),
    };
    let h = *next;
    *next += 1;
    solids.insert(h, made);
    Ok(json!(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kernel whose checker declines every solid, in the way given.
    struct Unchecked(crate::Kind);

    impl Kernel for Unchecked {
        type Solid = ();
        fn name(&self) -> String {
            "unchecked".into()
        }
        fn version(&self) -> String {
            "0".into()
        }
        fn tolerance(&self) -> f64 {
            1e-7
        }
        fn make(&self, _p: &Primitive) -> Result<(), Refusal> {
            Ok(())
        }
        fn union(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn intersection(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn difference(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn volume(&self, _s: &()) -> Result<f64, Refusal> {
            Ok(1.0)
        }
        fn area(&self, _s: &()) -> Result<f64, Refusal> {
            Ok(6.0)
        }
        fn check(&self, _s: &()) -> Result<Vec<String>, Refusal> {
            Err(Refusal {
                kind: self.0,
                reason: "no verdict".into(),
            })
        }
    }

    #[test]
    fn a_checker_that_declines_is_an_issue_and_a_kernel_without_one_is_not_judged() {
        let mut solids = HashMap::from([(1, ())]);
        let asked = json!({"cmd": "check", "s": 1});
        let e = answer(&Unchecked(crate::Kind::Failed), &asked, &mut solids, &mut 2).unwrap_err();
        assert_eq!(
            (e.kind, e.reason.as_str()),
            (crate::Kind::Failed, "no verdict")
        );
        let declined = Unchecked(crate::Kind::Failed).measure(&());
        assert_eq!(declined.issues.len(), 1);
        assert!(declined.issues[0].contains("no verdict"));
        assert!(
            Unchecked(crate::Kind::Unsupported)
                .measure(&())
                .issues
                .is_empty()
        );
    }

    /// A kernel that offers nothing beyond what the trait makes it answer.
    struct Bare;

    impl Kernel for Bare {
        type Solid = ();
        fn name(&self) -> String {
            "bare".into()
        }
        fn version(&self) -> String {
            "0".into()
        }
        fn tolerance(&self) -> f64 {
            1e-7
        }
        fn make(&self, _p: &Primitive) -> Result<(), Refusal> {
            Ok(())
        }
        fn union(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn intersection(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn difference(&self, _a: &(), _b: &()) -> Result<(), Refusal> {
            Ok(())
        }
        fn volume(&self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn area(&self, _s: &()) -> Result<f64, Refusal> {
            Ok(0.0)
        }
        fn check(&self, _s: &()) -> Result<Vec<String>, Refusal> {
            Ok(Vec::new())
        }
        fn unsupported(&self) -> Vec<String> {
            vec!["slab".into(), "sew".into()]
        }
    }

    #[test]
    fn every_listed_function_is_a_command_serve_knows() {
        for f in FUNCTIONS {
            let (mut solids, mut next) = (HashMap::new(), 1);
            let unknown = match answer(&Bare, &json!({"cmd": f}), &mut solids, &mut next) {
                Err(e) => e.reason.starts_with("unknown command"),
                Ok(_) => false,
            };
            assert!(!unknown, "{f}");
        }
        let (mut solids, mut next) = (HashMap::new(), 1);
        let e = answer(&Bare, &json!({"cmd": "later"}), &mut solids, &mut next).unwrap_err();
        assert_eq!(e.kind, crate::Kind::Unsupported);
    }

    #[test]
    fn hello_lists_what_the_kernel_offers_and_classify_refuses_where_it_has_none() {
        let (mut solids, mut next) = (HashMap::new(), 1);
        let hello = answer(&Bare, &json!({"cmd": "hello"}), &mut solids, &mut next).unwrap();
        let supports: Vec<&str> = hello["supports"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(supports.len(), FUNCTIONS.len() - 2);
        assert!(supports.contains(&"box") && !supports.contains(&"slab"));
        assert!(hello.get("unsupported").is_none());
        solids.insert(1, ());
        let asked = json!({"cmd": "classify", "s": 1, "points": [[0.0, 0.0, 0.0]]});
        let e = answer(&Bare, &asked, &mut solids, &mut next).unwrap_err();
        assert_eq!(e.kind, crate::Kind::Unsupported);
    }
}
