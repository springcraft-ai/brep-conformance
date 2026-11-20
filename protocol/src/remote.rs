//! A [`Kernel`] that forwards every call to a worker process speaking the protocol, which holds its
//! solids.

use std::io::{BufRead, BufReader, Write};
use std::panic::AssertUnwindSafe;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::model::{Op, Primitive, Side, Vec3};
use crate::{Display, DisplayFace, Kernel, Mesh, PROTOCOL_VERSION, Refusal, TimedOut, speaks};

struct Worker {
    child: Child,
    to: ChildStdin,
    /// The worker's reply lines, read on a thread of their own so a call can stop waiting.
    from: Receiver<String>,
}

/// How a call ended without a reply.
enum Lost {
    Died,
    TimedOut,
}

impl Drop for Worker {
    fn drop(&mut self) {
        // The whole group: a worker started through a launcher (`uv run …`) is a grandchild,
        // and killing the launcher alone would leave it running, holding memory and a core.
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(self.child.id()) {
            // SAFETY: a signal to the group the worker was started as the leader of.
            unsafe { libc::kill(-pid, libc::SIGKILL) };
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Worker {
    fn spawn(command: &[String]) -> Result<Worker, String> {
        let (program, args) = command.split_first().ok_or("the worker command is empty")?;
        let mut cmd = Command::new(program);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped());
        // Its own process group, so it and anything it starts go together.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("starting the worker `{}`: {e}", command.join(" ")))?;
        let to = child.stdin.take().ok_or("no stdin")?;
        let out = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        let (tx, from) = channel();
        std::thread::spawn(move || {
            for line in out.lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Worker { child, to, from })
    }

    /// One request and its reply, waiting at most `limit`.
    fn call(&mut self, req: &Value, limit: Duration) -> Result<Value, Lost> {
        writeln!(self.to, "{req}").map_err(|_| Lost::Died)?;
        self.to.flush().map_err(|_| Lost::Died)?;
        match self.from.recv_timeout(limit) {
            Ok(line) => serde_json::from_str(&line).map_err(|_| Lost::Died),
            Err(RecvTimeoutError::Timeout) => Err(Lost::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Err(Lost::Died),
        }
    }

    /// How the worker ended, waiting briefly for a worker that closed its output to exit.
    fn exit_status(&mut self) -> String {
        let deadline = Instant::now() + Duration::from_millis(200);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status.to_string(),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => return "still running, killed".into(),
                Err(e) => return format!("exit status unknown: {e}"),
            }
        }
    }
}

/// A solid: its handle in the worker.
#[derive(Clone, Copy, Debug)]
pub struct Handle {
    handle: u64,
}

/// How a worker was lost: what ran past the time limit, or what it died on.
#[derive(Clone, Debug)]
enum Loss {
    TimedOut(String),
    Died(String),
}

/// A refusal for a reply to `cmd` that does not decode as the protocol says.
fn malformed(cmd: &'static str) -> impl FnOnce(serde_json::Error) -> Refusal {
    move |e| Refusal::failed(format!("{cmd}: {e}"))
}

/// Ends a call the way its worker was lost: a timeout or a crash.
fn lose(loss: Loss) -> ! {
    match loss {
        Loss::TimedOut(what) => std::panic::panic_any(TimedOut(what)),
        Loss::Died(what) => panic!("{what}"),
    }
}

/// One session of an adapter's kernel, in a worker process of its own.
pub struct Remote {
    command: Vec<String>,
    /// None after a loss, until the reset that replaces it.
    worker: Option<Worker>,
    /// The worker's loss since the last reset. Its solids are gone, so every later call ends as
    /// the call that lost it did, until the reset.
    lost: Option<Loss>,
    hello: Value,
    limit: Duration,
}

impl Remote {
    /// Starts the worker and asks its name. A call that takes longer than `limit` is a crash: the
    /// worker is killed, and the next reset starts another. A worker that speaks another version
    /// of the protocol is an error.
    pub fn new(command: Vec<String>, limit: Duration) -> Result<Remote, String> {
        let mut worker = Worker::spawn(&command)?;
        let hello = worker
            .call(&json!({"cmd": "hello"}), limit)
            .ok()
            .and_then(|r| r.get("ok").cloned())
            .ok_or("the worker did not answer hello")?;
        let theirs = hello["protocol"].as_str().unwrap_or("none");
        if !speaks(PROTOCOL_VERSION, theirs) {
            return Err(format!(
                "the worker speaks protocol {theirs} and this client {PROTOCOL_VERSION}"
            ));
        }
        Ok(Remote {
            command,
            worker: Some(worker),
            lost: None,
            hello,
            limit,
        })
    }

    /// A request to the worker, or to a new one in place of a lost worker. A `crash` reply, a
    /// worker that dies, or one past the time limit panics: a crash of the operation it was
    /// running.
    fn call(&mut self, req: Value) -> Result<Value, Refusal> {
        if let Some(loss) = &self.lost {
            lose(loss.clone());
        }
        let limit = self.limit;
        if self.worker.is_none() {
            self.worker = Some(Worker::spawn(&self.command)?);
        }
        let reply = match self.worker.as_mut().map(|wk| wk.call(&req, limit)) {
            Some(Ok(reply)) => reply,
            Some(Err(Lost::TimedOut)) => {
                self.worker = None;
                let loss = Loss::TimedOut(format!("{} took longer than {limit:?}", req["cmd"]));
                self.lost = Some(loss.clone());
                lose(loss);
            }
            _ => {
                let status = self
                    .worker
                    .as_mut()
                    .map(Worker::exit_status)
                    .unwrap_or_default();
                self.worker = None;
                let loss = Loss::Died(format!("the worker died on {} ({status})", req["cmd"]));
                self.lost = Some(loss.clone());
                lose(loss);
            }
        };
        if let Some(c) = reply.get("crash") {
            panic!("{}", c.as_str().unwrap_or("crash"));
        }
        match reply.get("ok") {
            Some(v) => Ok(v.clone()),
            None => Err(Refusal {
                kind: crate::Kind::from_word(reply["kind"].as_str()),
                reason: reply["err"].as_str().unwrap_or("error").to_string(),
            }),
        }
    }

    /// A request whose reply is a new solid's handle.
    fn solid(&mut self, req: Value) -> Result<Handle, Refusal> {
        let v = self.call(req)?;
        Self::handle(v)
    }

    fn handle(v: Value) -> Result<Handle, Refusal> {
        let handle = crate::wire::integer(&v).ok_or("the worker returned no handle")?;
        Ok(Handle { handle })
    }
}

impl Kernel for Remote {
    type Solid = Handle;

    fn name(&self) -> String {
        self.hello["name"].as_str().unwrap_or("?").into()
    }

    fn version(&self) -> String {
        self.hello["version"].as_str().unwrap_or("?").into()
    }

    fn tolerance(&self) -> f64 {
        self.hello["tolerance"].as_f64().unwrap_or(f64::NAN)
    }

    fn configuration(&self) -> String {
        self.hello["configuration"].as_str().unwrap_or("").into()
    }

    fn make(&mut self, p: &Primitive) -> Result<Handle, Refusal> {
        self.solid(p.to_call())
    }

    fn union(&mut self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Union, a, b)
    }

    fn intersection(&mut self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Intersection, a, b)
    }

    fn difference(&mut self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Difference, a, b)
    }

    fn volume(&mut self, s: &Handle) -> Result<f64, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "volume", "s": s.handle}))?)
            .map_err(malformed("volume"))
    }

    fn area(&mut self, s: &Handle) -> Result<f64, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "area", "s": s.handle}))?)
            .map_err(malformed("area"))
    }

    fn check(&mut self, s: &Handle) -> Result<Vec<String>, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "check", "s": s.handle}))?)
            .map_err(malformed("check"))
    }

    fn fillet(&mut self, s: &Handle, edges: &[Vec3], radius: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "fillet", "s": s.handle, "edges": edges, "radius": radius}))
    }

    fn fillet_radii(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        radii: &[f64],
    ) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "fillet_radii", "s": s.handle, "edges": edges, "radii": radii}))
    }

    fn normal(&mut self, s: &Handle, points: &[Vec3]) -> Result<Vec<Option<Vec3>>, Refusal> {
        serde_json::from_value(
            self.call(json!({"cmd": "normal", "s": s.handle, "points": points}))?,
        )
        .map_err(malformed("normal"))
    }

    fn fillet_g2(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        offset: f64,
        center: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "fillet_g2", "s": s.handle, "edges": edges, "offset": offset,
                   "center": center}),
        )
    }

    fn fillet_conic(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        setback: f64,
        rho: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "fillet_conic", "s": s.handle, "edges": edges,
                         "setback": setback, "rho": rho}))
    }

    fn fillet_setback(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "fillet_setback", "s": s.handle, "edges": edges,
                   "radius": radius, "setback": setback}),
        )
    }

    fn fillet_setback_g2(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "fillet_setback_g2", "s": s.handle, "edges": edges,
                   "radius": radius, "setback": setback}),
        )
    }

    fn fillet_faces(
        &mut self,
        s: &Handle,
        face_a: Vec3,
        face_b: Vec3,
        radius: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "fillet_faces", "s": s.handle, "face_a": face_a,
                   "face_b": face_b, "radius": radius}),
        )
    }

    fn full_round(
        &mut self,
        s: &Handle,
        side_a: Vec3,
        middle: Vec3,
        side_b: Vec3,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "full_round", "s": s.handle, "side_a": side_a, "middle": middle,
                   "side_b": side_b}),
        )
    }

    fn curvature(&mut self, s: &Handle, points: &[Vec3]) -> Result<Vec<Option<[f64; 2]>>, Refusal> {
        serde_json::from_value(
            self.call(json!({"cmd": "curvature", "s": s.handle, "points": points}))?,
        )
        .map_err(malformed("curvature"))
    }

    fn project(
        &mut self,
        s: &Handle,
        direction: Vec3,
        tolerance: f64,
    ) -> Result<crate::Projection, Refusal> {
        let req = json!({"cmd": "project", "s": s.handle, "direction": direction,
                         "tolerance": tolerance});
        let v = self.call(req)?;
        let part = |key: &str| {
            serde_json::from_value(v[key].clone())
                .map_err(|e| Refusal::failed(format!("project: {key}: {e}")))
        };
        Ok(crate::Projection {
            visible: part("visible")?,
            hidden: part("hidden")?,
        })
    }

    fn fillet_variable(
        &mut self,
        s: &Handle,
        edge: Vec3,
        from: Vec3,
        r_from: f64,
        r_to: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "fillet_variable", "s": s.handle, "edge": edge, "from": from,
                   "r_from": r_from, "r_to": r_to}),
        )
    }

    fn loft_through(
        &mut self,
        frame: &crate::model::Frame,
        profiles: &[crate::profile::Profile],
        heights: &[f64],
    ) -> Result<Handle, Refusal> {
        let f = json!({"origin": frame.origin, "x": frame.x, "y": frame.y, "z": frame.z});
        let ps: Vec<Value> = profiles.iter().map(|p| p.to_json()).collect();
        self.solid(json!({"cmd": "loft_through", "frame": f, "profiles": ps, "heights": heights}))
    }

    fn chamfer_two(
        &mut self,
        s: &Handle,
        edge: Vec3,
        face: Vec3,
        d1: f64,
        d2: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "chamfer_two", "s": s.handle, "edge": edge, "face": face,
                   "d1": d1, "d2": d2}),
        )
    }

    fn chamfer_distances(
        &mut self,
        s: &Handle,
        edges: &[Vec3],
        distances: &[f64],
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "chamfer_distances", "s": s.handle, "edges": edges,
                   "distances": distances}),
        )
    }

    fn chamfer(&mut self, s: &Handle, edges: &[Vec3], distance: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "chamfer", "s": s.handle, "edges": edges, "distance": distance}))
    }

    fn betti(&mut self, s: &Handle) -> Result<[u64; 3], Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "betti", "s": s.handle}))?)
            .map_err(malformed("betti"))
    }

    fn section_area(&mut self, s: &Handle, origin: Vec3, normal: Vec3) -> Result<f64, Refusal> {
        serde_json::from_value(self.call(
            json!({"cmd": "section_area", "s": s.handle, "origin": origin, "normal": normal}),
        )?)
        .map_err(malformed("section_area"))
    }

    fn mesh(&mut self, s: &Handle, tolerance: f64) -> Result<Mesh, Refusal> {
        let req = json!({"cmd": "mesh", "s": s.handle, "tolerance": tolerance});
        let v = self.call(req)?;
        Ok(Mesh {
            positions: serde_json::from_value(v["positions"].clone())
                .map_err(|e| format!("mesh positions: {e}"))?,
            triangles: serde_json::from_value(v["triangles"].clone())
                .map_err(|e| format!("mesh triangles: {e}"))?,
        })
    }

    fn display(&mut self, s: &Handle, tolerance: f64) -> Result<Display, Refusal> {
        let req = json!({"cmd": "display", "s": s.handle, "tolerance": tolerance});
        let v = self.call(req)?;
        let part = |v: &Value, key: &str| -> Result<Value, Refusal> {
            Ok(v.get(key)
                .ok_or_else(|| format!("display: no `{key}`"))?
                .clone())
        };
        let faces = part(&v, "faces")?
            .as_array()
            .ok_or("display: `faces` is not a list")?
            .iter()
            .map(|f| {
                let read = |key: &str| part(f, key);
                Ok(DisplayFace {
                    positions: serde_json::from_value(read("positions")?)
                        .map_err(|e| format!("display positions: {e}"))?,
                    normals: serde_json::from_value(read("normals")?)
                        .map_err(|e| format!("display normals: {e}"))?,
                    triangles: serde_json::from_value(read("triangles")?)
                        .map_err(|e| format!("display triangles: {e}"))?,
                })
            })
            .collect::<Result<Vec<_>, Refusal>>()?;
        let edges = serde_json::from_value(part(&v, "edges")?)
            .map_err(|e| format!("display edges: {e}"))?;
        Ok(Display { faces, edges })
    }

    fn offset(&mut self, s: &Handle, distance: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "offset", "s": s.handle, "distance": distance}))
    }

    fn shell(&mut self, s: &Handle, open: &[Vec3], thickness: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "shell", "s": s.handle, "open": open, "thickness": thickness}))
    }

    fn draft(
        &mut self,
        s: &Handle,
        faces: &[Vec3],
        origin: Vec3,
        pull: Vec3,
        angle: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(
            json!({"cmd": "draft", "s": s.handle, "faces": faces, "origin": origin,
                   "pull": pull, "angle": angle}),
        )
    }

    fn interference(&mut self, a: &Handle, b: &Handle) -> Result<crate::Meeting, Refusal> {
        let v = self.call(json!({"cmd": "interference", "a": a.handle, "b": b.handle}))?;
        let n = |k: &str| v[k].as_f64().ok_or(format!("interference: no `{k}`"));
        Ok(crate::Meeting {
            volume: n("volume")?,
            gap: n("gap")?,
        })
    }

    fn union_all(&mut self, solids: &[&Handle]) -> Result<Handle, Refusal> {
        if solids.is_empty() {
            return Err("a union of all needs solids".into());
        }
        let hs: Vec<u64> = solids.iter().map(|s| s.handle).collect();
        self.solid(json!({"cmd": "union_all", "solids": hs}))
    }

    fn difference_all(&mut self, target: &Handle, tools: &[&Handle]) -> Result<Handle, Refusal> {
        let hs: Vec<u64> = tools.iter().map(|s| s.handle).collect();
        self.solid(json!({"cmd": "difference_all", "target": target.handle, "tools": hs}))
    }

    fn delete_faces(&mut self, s: &Handle, faces: &[Vec3]) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "delete_faces", "s": s.handle, "faces": faces}))
    }

    fn move_faces(
        &mut self,
        s: &Handle,
        faces: &[Vec3],
        matrix: [[f64; 4]; 3],
    ) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "move_faces", "s": s.handle, "faces": faces, "matrix": matrix}))
    }

    fn thicken(&mut self, s: &Handle, faces: &[Vec3], thickness: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "thicken", "s": s.handle, "faces": faces,
                         "thickness": thickness}))
    }

    fn split(
        &mut self,
        s: &Handle,
        origin: Vec3,
        normal: Vec3,
    ) -> Result<(Handle, Handle), Refusal> {
        let req = json!({"cmd": "split", "s": s.handle, "origin": origin, "normal": normal});
        let v = self.call(req)?;
        let hs = v.as_array().ok_or("split: not a list")?;
        let [a, b] = hs.as_slice() else {
            return Err("split: not two handles".into());
        };
        Ok((Self::handle(a.clone())?, Self::handle(b.clone())?))
    }

    fn bounding_box(&mut self, s: &Handle) -> Result<[Vec3; 2], Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "bounding_box", "s": s.handle}))?)
            .map_err(malformed("bounding_box"))
    }

    fn section(
        &mut self,
        s: &Handle,
        origin: Vec3,
        normal: Vec3,
        tolerance: f64,
    ) -> Result<Vec<Vec<Vec3>>, Refusal> {
        serde_json::from_value(self.call(
            json!({"cmd": "section", "s": s.handle, "origin": origin, "normal": normal,
                   "tolerance": tolerance}),
        )?)
        .map_err(malformed("section"))
    }

    fn sew(&mut self, text: &str, tolerance: f64) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "sew", "text": text, "tolerance": tolerance}))
    }

    fn offset_faces(
        &mut self,
        s: &Handle,
        faces: &[Vec3],
        distance: f64,
    ) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "offset_faces", "s": s.handle, "faces": faces,
                         "distance": distance}))
    }

    fn transform(&mut self, s: &Handle, matrix: [[f64; 4]; 3]) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "transform", "s": s.handle, "matrix": matrix}))
    }

    fn affine(&mut self, s: &Handle, matrix: [[f64; 4]; 3]) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "affine", "s": s.handle, "matrix": matrix}))
    }

    fn raycast(&mut self, s: &Handle, origin: Vec3, direction: Vec3) -> Result<Vec<f64>, Refusal> {
        serde_json::from_value(self.call(
            json!({"cmd": "raycast", "s": s.handle, "origin": origin, "direction": direction}),
        )?)
        .map_err(malformed("raycast"))
    }

    fn raycasts(&mut self, s: &Handle, rays: &[(Vec3, Vec3)]) -> Result<Vec<Vec<f64>>, Refusal> {
        // A worker that reads only one ray a request answers the many-ray form with an error.
        let req = json!({"cmd": "raycast", "s": s.handle, "rays": rays});
        match self.call(req).map(serde_json::from_value::<Vec<Vec<f64>>>) {
            Ok(Ok(all)) if all.len() == rays.len() => Ok(all),
            _ => rays.iter().map(|&(o, d)| self.raycast(s, o, d)).collect(),
        }
    }

    fn reset(&mut self) {
        // A lost worker is replaced here, between cases, so a replacement that cannot start is
        // not charged to the next case.
        self.lost = None;
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| self.call(json!({"cmd": "reset"}))));
        if let Some(Loss::Died(what) | Loss::TimedOut(what)) = self.lost.take() {
            eprintln!("the worker was lost between cases: {what}");
        }
    }

    fn supports(&self) -> Vec<String> {
        serde_json::from_value(self.hello["supports"].clone()).unwrap_or_default()
    }

    fn write_step(&mut self, s: &Handle) -> Result<String, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "write_step", "s": s.handle}))?)
            .map_err(malformed("write_step"))
    }

    fn read_step(&mut self, text: &str) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": "read_step", "text": text}))
    }

    fn centroid(&mut self, s: &Handle) -> Result<Option<Vec3>, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "centroid", "s": s.handle}))?)
            .map_err(malformed("centroid"))
    }

    fn inertia(&mut self, s: &Handle) -> Result<[[f64; 3]; 3], Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "inertia", "s": s.handle}))?)
            .map_err(malformed("inertia"))
    }

    fn distance(&mut self, a: &Handle, b: &Handle) -> Result<f64, Refusal> {
        serde_json::from_value(self.call(json!({"cmd": "distance", "a": a.handle, "b": b.handle}))?)
            .map_err(malformed("distance"))
    }

    fn classify(&mut self, s: &Handle, points: &[Vec3]) -> Result<Vec<Side>, Refusal> {
        let req = json!({"cmd": "classify", "s": s.handle, "points": points});
        let v = self.call(req)?;
        v.as_array()
            .and_then(|words| {
                let side = |word: &Value| Side::from_word(word.as_str()?);
                words.iter().map(side).collect()
            })
            .ok_or_else(|| Refusal::failed(format!("classify: {v} is not a list of sides")))
    }

    fn closest(&mut self, s: &Handle, points: &[Vec3]) -> Result<Vec<Vec3>, Refusal> {
        serde_json::from_value(
            self.call(json!({"cmd": "closest", "s": s.handle, "points": points}))?,
        )
        .map_err(malformed("closest"))
    }
}

impl Remote {
    fn combine(&mut self, op: Op, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.solid(json!({"cmd": op.word(), "a": a.handle, "b": b.handle}))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_worker_of_another_protocol_version_is_turned_away() {
        let worker = |hello: &str| {
            let script = format!("read line; echo '{{\"ok\": {hello}}}'; cat");
            let cmd = ["sh", "-c", &script].map(String::from).to_vec();
            Remote::new(cmd, Duration::from_secs(10))
        };
        let ours =
            format!(r#"{{"protocol": "{PROTOCOL_VERSION}", "supports": ["box", "volume"]}}"#);
        let k = worker(&ours).unwrap_or_else(|e| panic!("{e}"));
        // What the worker lists is what it offers.
        assert_eq!(k.supports(), ["box", "volume"]);
        for other in [r#"{"protocol": "0.10"}"#, r#"{"protocol": "7.0.0"}"#, "{}"] {
            let refused = worker(other).err().unwrap_or_default();
            assert!(refused.contains("speaks protocol"), "{other}: {refused}");
        }
    }

    #[test]
    fn a_dropped_worker_takes_the_processes_it_started_with_it() {
        // A worker run through a launcher (`uv run …`, here `sh`) is a grandchild: the shell
        // reports the pid of what it started, then waits as the worker would.
        let cmd = ["sh", "-c", "sleep 600 & echo $!; cat"].map(String::from);
        let w = Worker::spawn(&cmd).unwrap();
        let pid: i32 = w.from.recv().unwrap().trim().parse().unwrap();
        // SAFETY: signal 0 only asks whether the process exists.
        let alive = || unsafe { libc::kill(pid, 0) } == 0;
        assert!(alive());
        drop(w);
        // A tight poll for it to go, with a deadline only as a hang guard.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while alive() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "process {pid} outlived its worker");
    }

    /// A worker, run as `sh -c`, that answers hello and gives handle 1 to every other request.
    /// `arms` are `case` arms tried ahead of those two.
    fn scripted(arms: &str) -> Vec<String> {
        let hello = format!(r#"{{"ok": {{"protocol": "{PROTOCOL_VERSION}"}}}}"#);
        let script = format!(
            "while read line; do case \"$line\" in {arms} *hello*) echo '{hello}';; \
             *) echo '{{\"ok\": 1}}';; esac; done"
        );
        ["sh", "-c", &script].map(String::from).to_vec()
    }

    #[test]
    fn a_worker_that_dies_on_reset_is_not_charged_to_the_next_case() {
        let cmd = scripted(r#"*'"reset"'*) exit 3;;"#);
        let mut k = Remote::new(cmd, Duration::from_secs(10)).unwrap();
        k.read_step("first case").unwrap();
        k.reset();
        assert_eq!(k.read_step("next case").unwrap().handle, 1);
    }

    #[test]
    fn a_worker_that_cannot_start_after_a_loss_is_not_charged_to_the_next_case() {
        // Each start adds a line to a file; the second start exits before it reads a request.
        let starts = std::env::temp_dir().join(format!("remote-starts-{}", std::process::id()));
        let _ = std::fs::remove_file(&starts);
        let mut cmd = scripted(r#"*'"volume"'*) exit 5;;"#);
        cmd[2] = format!(
            "echo >> '{0}'; [ \"$(wc -l < '{0}')\" -eq 2 ] && exit 4; {1}",
            starts.display(),
            cmd[2]
        );
        let mut k = Remote::new(cmd, Duration::from_secs(10)).unwrap();
        let s = k.read_step("first case").unwrap();
        let died = std::panic::catch_unwind(AssertUnwindSafe(|| k.volume(&s))).unwrap_err();
        let died = died.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(died.starts_with(r#"the worker died on "volume""#), "{died}");
        k.reset();
        let next = k.read_step("next case");
        let started = std::fs::read_to_string(&starts).unwrap().lines().count();
        let _ = std::fs::remove_file(&starts);
        assert_eq!(next.unwrap().handle, 1);
        assert_eq!(started, 3);
    }
}
