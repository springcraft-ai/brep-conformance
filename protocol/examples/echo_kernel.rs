//! A "kernel" that answers every function with the arguments it was handed, for the wire parity
//! test in `protocol/parity`. Its C++ twin, `protocol/cpp/tests/echo_kernel.cpp`, computes the
//! same answers, so any difference in their replies is a difference in how the two bindings read
//! a request or write a reply.
//!
//! A solid is the JSON of the call that made it, with the solids it was made from nested in it;
//! `write_step` returns that JSON as text. A query answers with numbers taken from its arguments:
//! `fold` adds up every number of a solid, and `count` counts them.

use brep_protocol::model::{Frame, Primitive, Side, Vec3};
use brep_protocol::profile::Profile;
use brep_protocol::{Display, DisplayFace, Mesh, Projection};
use brep_protocol::{FUNCTIONS, Kernel, Kind, Meeting, Refusal};
use serde_json::{Value, json};

struct Echo;

fn fold(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::Array(items) => items.iter().map(fold).sum(),
        Value::Object(map) => map.values().map(fold).sum(),
        _ => 0.0,
    }
}

fn count(v: &Value) -> u64 {
    match v {
        Value::Number(_) => 1,
        Value::Array(items) => items.iter().map(count).sum(),
        Value::Object(map) => map.values().map(count).sum(),
        _ => 0,
    }
}

fn frame(f: &Frame) -> Value {
    json!({"origin": f.origin, "x": f.x, "y": f.y, "z": f.z})
}

/// Points with a negative x stand for points off the boundary, so both arms of an optional reply
/// are written.
fn present(p: &Vec3) -> bool {
    p[0] >= 0.0
}

impl Kernel for Echo {
    type Solid = Value;

    fn name(&self) -> String {
        "echo".into()
    }
    fn version(&self) -> String {
        "1".into()
    }
    fn tolerance(&self) -> f64 {
        1e-7
    }
    fn configuration(&self) -> String {
        "echoes its arguments".into()
    }
    fn supports(&self) -> Vec<String> {
        let mut all: Vec<String> = FUNCTIONS.iter().map(|f| f.to_string()).collect();
        all.push("not_a_function".into());
        all
    }

    fn make(&mut self, p: &Primitive) -> Result<Value, Refusal> {
        Ok(p.to_json())
    }
    fn union(&mut self, a: &Value, b: &Value) -> Result<Value, Refusal> {
        Ok(json!({"op": "union", "a": a, "b": b}))
    }
    fn intersection(&mut self, a: &Value, b: &Value) -> Result<Value, Refusal> {
        Ok(json!({"op": "intersection", "a": a, "b": b}))
    }
    fn difference(&mut self, a: &Value, b: &Value) -> Result<Value, Refusal> {
        Ok(json!({"op": "difference", "a": a, "b": b}))
    }
    fn volume(&mut self, s: &Value) -> Result<f64, Refusal> {
        Ok(fold(s))
    }
    fn area(&mut self, s: &Value) -> Result<f64, Refusal> {
        Ok(count(s) as f64)
    }
    fn check(&mut self, s: &Value) -> Result<Vec<String>, Refusal> {
        Ok(vec![format!("{} numbers", count(s)), "checked".into()])
    }
    fn classify(&mut self, _s: &Value, points: &[Vec3]) -> Result<Vec<Side>, Refusal> {
        Ok(points
            .iter()
            .map(|p| {
                if p[0] < 0.0 {
                    Side::Out
                } else if p[0] == 0.0 {
                    Side::On
                } else {
                    Side::In
                }
            })
            .collect())
    }
    fn closest(&mut self, s: &Value, points: &[Vec3]) -> Result<Vec<Vec3>, Refusal> {
        Ok(points.iter().map(|p| [p[0], p[1], fold(s)]).collect())
    }
    fn normal(&mut self, _s: &Value, points: &[Vec3]) -> Result<Vec<Option<Vec3>>, Refusal> {
        Ok(points.iter().map(|p| present(p).then_some(*p)).collect())
    }
    fn curvature(&mut self, _s: &Value, points: &[Vec3]) -> Result<Vec<Option<[f64; 2]>>, Refusal> {
        Ok(points
            .iter()
            .map(|p| present(p).then_some([p[1], p[2]]))
            .collect())
    }
    fn fillet(&mut self, s: &Value, edges: &[Vec3], radius: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "fillet", "s": s, "edges": edges, "radius": radius}))
    }
    fn fillet_radii(&mut self, s: &Value, edges: &[Vec3], radii: &[f64]) -> Result<Value, Refusal> {
        Ok(json!({"op": "fillet_radii", "s": s, "edges": edges, "radii": radii}))
    }
    fn chamfer_distances(
        &mut self,
        s: &Value,
        edges: &[Vec3],
        distances: &[f64],
    ) -> Result<Value, Refusal> {
        Ok(json!({"op": "chamfer_distances", "s": s, "edges": edges, "distances": distances}))
    }
    fn chamfer_two(
        &mut self,
        s: &Value,
        edge: Vec3,
        face: Vec3,
        d1: f64,
        d2: f64,
    ) -> Result<Value, Refusal> {
        Ok(json!({"op": "chamfer_two", "s": s, "edge": edge, "face": face, "d1": d1, "d2": d2}))
    }
    fn fillet_variable(
        &mut self,
        s: &Value,
        edge: Vec3,
        from: Vec3,
        r_from: f64,
        r_to: f64,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "fillet_variable", "s": s, "edge": edge, "from": from,
                  "r_from": r_from, "r_to": r_to}),
        )
    }
    fn fillet_g2(
        &mut self,
        s: &Value,
        edges: &[Vec3],
        offset: f64,
        center: f64,
    ) -> Result<Value, Refusal> {
        Ok(json!({"op": "fillet_g2", "s": s, "edges": edges, "offset": offset, "center": center}))
    }
    fn fillet_conic(
        &mut self,
        s: &Value,
        edges: &[Vec3],
        setback: f64,
        rho: f64,
    ) -> Result<Value, Refusal> {
        Ok(json!({"op": "fillet_conic", "s": s, "edges": edges, "setback": setback, "rho": rho}))
    }
    fn fillet_setback(
        &mut self,
        s: &Value,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "fillet_setback", "s": s, "edges": edges, "radius": radius,
                  "setback": setback}),
        )
    }
    fn fillet_setback_g2(
        &mut self,
        s: &Value,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "fillet_setback_g2", "s": s, "edges": edges, "radius": radius,
                  "setback": setback}),
        )
    }
    fn fillet_faces(
        &mut self,
        s: &Value,
        face_a: Vec3,
        face_b: Vec3,
        radius: f64,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "fillet_faces", "s": s, "face_a": face_a, "face_b": face_b,
                  "radius": radius}),
        )
    }
    fn full_round(
        &mut self,
        s: &Value,
        side_a: Vec3,
        middle: Vec3,
        side_b: Vec3,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "full_round", "s": s, "side_a": side_a, "middle": middle,
                  "side_b": side_b}),
        )
    }
    fn project(
        &mut self,
        s: &Value,
        direction: Vec3,
        tolerance: f64,
    ) -> Result<Projection, Refusal> {
        Ok(Projection::new(
            vec![vec![
                [direction[0], direction[1]],
                [direction[2], tolerance],
            ]],
            vec![vec![[fold(s), count(s) as f64]], vec![]],
        ))
    }
    fn chamfer(&mut self, s: &Value, edges: &[Vec3], distance: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "chamfer", "s": s, "edges": edges, "distance": distance}))
    }
    fn write_step(&mut self, s: &Value) -> Result<String, Refusal> {
        Ok(s.to_string())
    }
    /// Text `refuse <kind>` is refused with that kind, and `crash` panics, so each reply that is
    /// not `ok` is written too.
    fn read_step(&mut self, text: &str) -> Result<Value, Refusal> {
        if let Some(kind) = text.strip_prefix("refuse ") {
            let kind = match kind {
                "unsupported" => Kind::Unsupported,
                "invalid" => Kind::Invalid,
                _ => Kind::Failed,
            };
            return Err(Refusal::new(kind, format!("echo refuses \"{text}\"")));
        }
        if text == "crash" {
            panic!("echo crashes");
        }
        Ok(json!({"op": "read_step", "text": text}))
    }
    fn centroid(&mut self, s: &Value) -> Result<Option<Vec3>, Refusal> {
        let n = count(s);
        Ok((n > 0).then(|| [fold(s), n as f64, 0.0]))
    }
    fn inertia(&mut self, s: &Value) -> Result<[[f64; 3]; 3], Refusal> {
        Ok([
            [fold(s), 0.0, 0.0],
            [0.0, count(s) as f64, 0.0],
            [0.0, 0.0, 1.0],
        ])
    }
    fn distance(&mut self, a: &Value, b: &Value) -> Result<f64, Refusal> {
        Ok(fold(a) - fold(b))
    }
    fn betti(&mut self, s: &Value) -> Result<[u64; 3], Refusal> {
        Ok([count(s), 0, 1])
    }
    fn section_area(&mut self, s: &Value, origin: Vec3, normal: Vec3) -> Result<f64, Refusal> {
        Ok(fold(s) + fold(&json!([origin, normal])) * 1000.0)
    }
    fn mesh(&mut self, s: &Value, tolerance: f64) -> Result<Mesh, Refusal> {
        Ok(Mesh::new(
            vec![[tolerance, fold(s), 0.0], [0.0, 0.0, count(s) as f64]],
            vec![[0, 1, 0], [1, 0, u32::MAX]],
        ))
    }
    fn display(&mut self, s: &Value, tolerance: f64) -> Result<Display, Refusal> {
        let face = DisplayFace::new(
            vec![[tolerance, 0.0, 0.0], [0.0, fold(s), 0.0]],
            vec![[0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 1]],
        );
        Ok(Display::new(
            vec![face, DisplayFace::new(vec![], vec![], vec![])],
            vec![vec![[count(s) as f64, 0.0, 0.0], [0.0, 0.0, 0.0]], vec![]],
        ))
    }
    fn offset(&mut self, s: &Value, distance: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "offset", "s": s, "distance": distance}))
    }
    fn shell(&mut self, s: &Value, open: &[Vec3], thickness: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "shell", "s": s, "open": open, "thickness": thickness}))
    }
    fn draft(
        &mut self,
        s: &Value,
        faces: &[Vec3],
        origin: Vec3,
        pull: Vec3,
        angle: f64,
    ) -> Result<Value, Refusal> {
        Ok(
            json!({"op": "draft", "s": s, "faces": faces, "origin": origin, "pull": pull,
                  "angle": angle}),
        )
    }
    fn transform(&mut self, s: &Value, matrix: [[f64; 4]; 3]) -> Result<Value, Refusal> {
        Ok(json!({"op": "transform", "s": s, "matrix": matrix}))
    }
    fn affine(&mut self, s: &Value, matrix: [[f64; 4]; 3]) -> Result<Value, Refusal> {
        Ok(json!({"op": "affine", "s": s, "matrix": matrix}))
    }
    fn raycast(&mut self, s: &Value, origin: Vec3, direction: Vec3) -> Result<Vec<f64>, Refusal> {
        Ok(vec![fold(&json!(origin)), fold(&json!(direction)), fold(s)])
    }
    fn interference(&mut self, a: &Value, b: &Value) -> Result<Meeting, Refusal> {
        Ok(Meeting {
            volume: fold(a),
            gap: fold(b),
        })
    }
    fn union_all(&mut self, solids: &[&Value]) -> Result<Value, Refusal> {
        Ok(json!({"op": "union_all", "solids": solids}))
    }
    fn difference_all(&mut self, target: &Value, tools: &[&Value]) -> Result<Value, Refusal> {
        Ok(json!({"op": "difference_all", "target": target, "tools": tools}))
    }
    fn offset_faces(&mut self, s: &Value, faces: &[Vec3], distance: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "offset_faces", "s": s, "faces": faces, "distance": distance}))
    }
    fn loft_through(
        &mut self,
        f: &Frame,
        profiles: &[Profile],
        heights: &[f64],
    ) -> Result<Value, Refusal> {
        let profiles: Vec<Value> = profiles.iter().map(Profile::to_json).collect();
        Ok(
            json!({"op": "loft_through", "frame": frame(f), "profiles": profiles,
                  "heights": heights}),
        )
    }
    fn delete_faces(&mut self, s: &Value, faces: &[Vec3]) -> Result<Value, Refusal> {
        Ok(json!({"op": "delete_faces", "s": s, "faces": faces}))
    }
    fn move_faces(
        &mut self,
        s: &Value,
        faces: &[Vec3],
        matrix: [[f64; 4]; 3],
    ) -> Result<Value, Refusal> {
        Ok(json!({"op": "move_faces", "s": s, "faces": faces, "matrix": matrix}))
    }
    fn thicken(&mut self, s: &Value, faces: &[Vec3], thickness: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "thicken", "s": s, "faces": faces, "thickness": thickness}))
    }
    fn split(&mut self, s: &Value, origin: Vec3, normal: Vec3) -> Result<(Value, Value), Refusal> {
        let part = |side: &str| json!({"op": "split", "side": side, "s": s, "origin": origin, "normal": normal});
        Ok((part("above"), part("below")))
    }
    fn bounding_box(&mut self, s: &Value) -> Result<[Vec3; 2], Refusal> {
        Ok([[0.0, 0.0, 0.0], [fold(s), count(s) as f64, -1.0]])
    }
    fn section(
        &mut self,
        s: &Value,
        origin: Vec3,
        normal: Vec3,
        tolerance: f64,
    ) -> Result<Vec<Vec<Vec3>>, Refusal> {
        Ok(vec![
            vec![origin, normal, [tolerance, fold(s), 0.0]],
            vec![],
        ])
    }
    fn sew(&mut self, text: &str, tolerance: f64) -> Result<Value, Refusal> {
        Ok(json!({"op": "sew", "text": text, "tolerance": tolerance}))
    }
}

fn main() -> std::io::Result<()> {
    brep_protocol::serve(&mut Echo)
}
