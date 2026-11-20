//! A [`Kernel`] that forwards every call to worker processes speaking the protocol: one worker
//! per calling thread, each solid living in the worker that made it.

use std::cell::Cell;
use std::io::{BufRead, BufReader, Write};
use std::panic::AssertUnwindSafe;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::model::{Op, Primitive, Side, Vec3};
use crate::{
    Display, DisplayFace, FUNCTIONS, Kernel, Mesh, PROTOCOL_VERSION, Refusal, TimedOut, speaks,
};

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

/// A solid: the worker holding it, and its handle there.
#[derive(Clone, Copy, Debug)]
pub struct Handle {
    worker: usize,
    handle: u64,
}

/// How a worker was lost: what ran past the time limit, or what it died on.
#[derive(Clone, Debug)]
enum Loss {
    TimedOut(String),
    Died(String),
}

pub struct Remote {
    command: Vec<String>,
    workers: Vec<Mutex<Option<Worker>>>,
    /// Each worker's loss since its caller last reset it. The solids of a lost worker are gone,
    /// so every later call for them ends as the call that lost it did, until the reset.
    lost: Vec<Mutex<Option<Loss>>>,
    next: AtomicUsize,
    hello: Value,
    limit: Duration,
}

thread_local! {
    static MINE: Cell<Option<usize>> = const { Cell::new(None) };
}

impl Remote {
    /// Starts one worker to ask its name; `threads` more start as threads first call. A call that
    /// takes longer than `limit` is a crash: its worker is killed and replaced. A worker that
    /// speaks another version of the protocol is an error.
    pub fn new(command: Vec<String>, threads: usize, limit: Duration) -> Result<Remote, String> {
        let mut first = Worker::spawn(&command)?;
        let hello = first
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
        let mut workers = vec![Mutex::new(Some(first))];
        workers.extend((1..threads.max(1)).map(|_| Mutex::new(None)));
        let lost = workers.iter().map(|_| Mutex::new(None)).collect();
        Ok(Remote {
            command,
            workers,
            lost,
            next: AtomicUsize::new(0),
            hello,
            limit,
        })
    }

    /// This thread's worker, the same for every call it makes.
    fn mine(&self) -> usize {
        MINE.with(|m| {
            *m.get().get_or_insert_with(|| {
                let i = self.next.fetch_add(1, Ordering::Relaxed) % self.workers.len();
                m.set(Some(i));
                i
            })
        })
    }

    /// A request to worker `w`. A `crash` reply, a worker that dies, or one past the time limit
    /// panics: a crash of the operation it was running. A dead or killed worker is replaced on the
    /// next request.
    fn call(&self, w: usize, req: Value) -> Result<Value, Refusal> {
        let lose = |loss: Loss| -> ! {
            *self.lost[w].lock().unwrap_or_else(|p| p.into_inner()) = Some(loss.clone());
            match loss {
                Loss::TimedOut(what) => std::panic::panic_any(TimedOut(what)),
                Loss::Died(what) => panic!("{what}"),
            }
        };
        let before = self.lost[w]
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(loss) = before {
            lose(loss);
        }
        let mut slot = self.workers[w].lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_none() {
            *slot = Some(Worker::spawn(&self.command)?);
        }
        let reply = match slot.as_mut().map(|wk| wk.call(&req, self.limit)) {
            Some(Ok(reply)) => reply,
            Some(Err(Lost::TimedOut)) => {
                *slot = None;
                drop(slot);
                lose(Loss::TimedOut(format!(
                    "{} took longer than {:?}",
                    req["cmd"], self.limit
                )));
            }
            _ => {
                let status = slot.as_mut().map(Worker::exit_status).unwrap_or_default();
                *slot = None;
                drop(slot);
                lose(Loss::Died(format!(
                    "the worker died on {} ({status})",
                    req["cmd"]
                )));
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

    fn handle(&self, w: usize, v: Value) -> Result<Handle, Refusal> {
        let handle = v.as_u64().ok_or("the worker returned no handle")?;
        Ok(Handle { worker: w, handle })
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

    fn make(&self, p: &Primitive) -> Result<Handle, Refusal> {
        let w = self.mine();
        let v = self.call(w, p.to_call())?;
        self.handle(w, v)
    }

    fn union(&self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Union, a, b)
    }

    fn intersection(&self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Intersection, a, b)
    }

    fn difference(&self, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        self.combine(Op::Difference, a, b)
    }

    fn volume(&self, s: &Handle) -> Result<f64, Refusal> {
        let v = self.call(s.worker, json!({"cmd": "volume", "s": s.handle}))?;
        v.as_f64().ok_or_else(|| "volume: not a number".into())
    }

    fn area(&self, s: &Handle) -> Result<f64, Refusal> {
        let v = self.call(s.worker, json!({"cmd": "area", "s": s.handle}))?;
        v.as_f64().ok_or_else(|| "area: not a number".into())
    }

    fn check(&self, s: &Handle) -> Result<Vec<String>, Refusal> {
        let v = self.call(s.worker, json!({"cmd": "check", "s": s.handle}))?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("check: {e}")))
    }

    fn fillet(&self, s: &Handle, edges: &[Vec3], radius: f64) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet", "s": s.handle, "edges": edges, "radius": radius});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn fillet_radii(&self, s: &Handle, edges: &[Vec3], radii: &[f64]) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_radii", "s": s.handle, "edges": edges, "radii": radii});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn normal(&self, s: &Handle, points: &[Vec3]) -> Result<Vec<Option<Vec3>>, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "normal", "s": s.handle, "points": points}),
        )?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("normal: {e}")))
    }

    fn fillet_g2(
        &self,
        s: &Handle,
        edges: &[Vec3],
        offset: f64,
        center: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_g2", "s": s.handle, "edges": edges, "offset": offset,
                         "center": center});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn fillet_conic(
        &self,
        s: &Handle,
        edges: &[Vec3],
        setback: f64,
        rho: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_conic", "s": s.handle, "edges": edges,
                         "setback": setback, "rho": rho});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn fillet_setback(
        &self,
        s: &Handle,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_setback", "s": s.handle, "edges": edges,
                         "radius": radius, "setback": setback});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn fillet_setback_g2(
        &self,
        s: &Handle,
        edges: &[Vec3],
        radius: f64,
        setback: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_setback_g2", "s": s.handle, "edges": edges,
                         "radius": radius, "setback": setback});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn fillet_faces(
        &self,
        s: &Handle,
        face_a: Vec3,
        face_b: Vec3,
        radius: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_faces", "s": s.handle, "face_a": face_a,
                         "face_b": face_b, "radius": radius});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn full_round(
        &self,
        s: &Handle,
        side_a: Vec3,
        middle: Vec3,
        side_b: Vec3,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "full_round", "s": s.handle, "side_a": side_a, "middle": middle,
                         "side_b": side_b});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn curvature(&self, s: &Handle, points: &[Vec3]) -> Result<Vec<Option<[f64; 2]>>, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "curvature", "s": s.handle, "points": points}),
        )?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("curvature: {e}")))
    }

    fn project(
        &self,
        s: &Handle,
        direction: Vec3,
        tolerance: f64,
    ) -> Result<crate::Projection, Refusal> {
        let req = json!({"cmd": "project", "s": s.handle, "direction": direction,
                         "tolerance": tolerance});
        let v = self.call(s.worker, req)?;
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
        &self,
        s: &Handle,
        edge: Vec3,
        from: Vec3,
        r_from: f64,
        r_to: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "fillet_variable", "s": s.handle, "edge": edge, "from": from,
                         "r_from": r_from, "r_to": r_to});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn loft_through(
        &self,
        frame: &crate::model::Frame,
        profiles: &[crate::profile::Profile],
        heights: &[f64],
    ) -> Result<Handle, Refusal> {
        let f = json!({"origin": frame.origin, "x": frame.x, "y": frame.y, "z": frame.z});
        let ps: Vec<Value> = profiles.iter().map(|p| p.to_json()).collect();
        let req = json!({"cmd": "loft_through", "frame": f, "profiles": ps, "heights": heights});
        let w = self.mine();
        let v = self.call(w, req)?;
        self.handle(w, v)
    }

    fn chamfer_two(
        &self,
        s: &Handle,
        edge: Vec3,
        face: Vec3,
        d1: f64,
        d2: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "chamfer_two", "s": s.handle, "edge": edge, "face": face,
                         "d1": d1, "d2": d2});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn chamfer_distances(
        &self,
        s: &Handle,
        edges: &[Vec3],
        distances: &[f64],
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "chamfer_distances", "s": s.handle, "edges": edges,
                         "distances": distances});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn chamfer(&self, s: &Handle, edges: &[Vec3], distance: f64) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "chamfer", "s": s.handle, "edges": edges, "distance": distance});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn betti(&self, s: &Handle) -> Result<[u64; 3], Refusal> {
        let v = self.call(s.worker, json!({"cmd": "betti", "s": s.handle}))?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("betti: {e}")))
    }

    fn section_area(&self, s: &Handle, origin: Vec3, normal: Vec3) -> Result<f64, Refusal> {
        let req = json!({"cmd": "section_area", "s": s.handle, "origin": origin, "normal": normal});
        let v = self.call(s.worker, req)?;
        v.as_f64()
            .ok_or_else(|| Refusal::failed(format!("section_area: {v} is not a number")))
    }

    fn mesh(&self, s: &Handle, tolerance: f64) -> Result<Mesh, Refusal> {
        let req = json!({"cmd": "mesh", "s": s.handle, "tolerance": tolerance});
        let v = self.call(s.worker, req)?;
        Ok(Mesh {
            positions: serde_json::from_value(v["positions"].clone())
                .map_err(|e| format!("mesh positions: {e}"))?,
            triangles: serde_json::from_value(v["triangles"].clone())
                .map_err(|e| format!("mesh triangles: {e}"))?,
        })
    }

    fn display(&self, s: &Handle, tolerance: f64) -> Result<Display, Refusal> {
        let req = json!({"cmd": "display", "s": s.handle, "tolerance": tolerance});
        let v = self.call(s.worker, req)?;
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

    fn offset(&self, s: &Handle, distance: f64) -> Result<Handle, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "offset", "s": s.handle, "distance": distance}),
        )?;
        self.handle(s.worker, v)
    }

    fn shell(&self, s: &Handle, open: &[Vec3], thickness: f64) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "shell", "s": s.handle, "open": open, "thickness": thickness});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn draft(
        &self,
        s: &Handle,
        faces: &[Vec3],
        origin: Vec3,
        pull: Vec3,
        angle: f64,
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "draft", "s": s.handle, "faces": faces, "origin": origin,
                         "pull": pull, "angle": angle});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn interference(&self, a: &Handle, b: &Handle) -> Result<crate::Meeting, Refusal> {
        let v = self.call(
            a.worker,
            json!({"cmd": "interference", "a": a.handle, "b": b.handle}),
        )?;
        let n = |k: &str| v[k].as_f64().ok_or(format!("interference: no `{k}`"));
        Ok(crate::Meeting {
            volume: n("volume")?,
            gap: n("gap")?,
        })
    }

    fn union_all(&self, solids: &[&Handle]) -> Result<Handle, Refusal> {
        let w = solids.first().ok_or("a union of all needs solids")?.worker;
        let hs: Vec<u64> = solids.iter().map(|s| s.handle).collect();
        let v = self.call(w, json!({"cmd": "union_all", "solids": hs}))?;
        self.handle(w, v)
    }

    fn difference_all(&self, target: &Handle, tools: &[&Handle]) -> Result<Handle, Refusal> {
        let hs: Vec<u64> = tools.iter().map(|s| s.handle).collect();
        let req = json!({"cmd": "difference_all", "target": target.handle, "tools": hs});
        let v = self.call(target.worker, req)?;
        self.handle(target.worker, v)
    }

    fn delete_faces(&self, s: &Handle, faces: &[Vec3]) -> Result<Handle, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "delete_faces", "s": s.handle, "faces": faces}),
        )?;
        self.handle(s.worker, v)
    }

    fn move_faces(
        &self,
        s: &Handle,
        faces: &[Vec3],
        matrix: [[f64; 4]; 3],
    ) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "move_faces", "s": s.handle, "faces": faces, "matrix": matrix});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn thicken(&self, s: &Handle, faces: &[Vec3], thickness: f64) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "thicken", "s": s.handle, "faces": faces,
                         "thickness": thickness});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn split(&self, s: &Handle, origin: Vec3, normal: Vec3) -> Result<(Handle, Handle), Refusal> {
        let req = json!({"cmd": "split", "s": s.handle, "origin": origin, "normal": normal});
        let v = self.call(s.worker, req)?;
        let hs = v.as_array().ok_or("split: not a list")?;
        let [a, b] = hs.as_slice() else {
            return Err("split: not two handles".into());
        };
        Ok((
            self.handle(s.worker, a.clone())?,
            self.handle(s.worker, b.clone())?,
        ))
    }

    fn bounding_box(&self, s: &Handle) -> Result<[Vec3; 2], Refusal> {
        let v = self.call(s.worker, json!({"cmd": "bounding_box", "s": s.handle}))?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("bounding_box: {e}")))
    }

    fn section(
        &self,
        s: &Handle,
        origin: Vec3,
        normal: Vec3,
        tolerance: f64,
    ) -> Result<Vec<Vec<Vec3>>, Refusal> {
        let req = json!({"cmd": "section", "s": s.handle, "origin": origin, "normal": normal,
                         "tolerance": tolerance});
        let v = self.call(s.worker, req)?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("section: {e}")))
    }

    fn sew(&self, text: &str, tolerance: f64) -> Result<Handle, Refusal> {
        let w = self.mine();
        let v = self.call(
            w,
            json!({"cmd": "sew", "text": text, "tolerance": tolerance}),
        )?;
        self.handle(w, v)
    }

    fn offset_faces(&self, s: &Handle, faces: &[Vec3], distance: f64) -> Result<Handle, Refusal> {
        let req = json!({"cmd": "offset_faces", "s": s.handle, "faces": faces,
                         "distance": distance});
        let v = self.call(s.worker, req)?;
        self.handle(s.worker, v)
    }

    fn transform(&self, s: &Handle, matrix: [[f64; 4]; 3]) -> Result<Handle, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "transform", "s": s.handle, "matrix": matrix}),
        )?;
        self.handle(s.worker, v)
    }

    fn affine(&self, s: &Handle, matrix: [[f64; 4]; 3]) -> Result<Handle, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "affine", "s": s.handle, "matrix": matrix}),
        )?;
        self.handle(s.worker, v)
    }

    fn raycast(&self, s: &Handle, origin: Vec3, direction: Vec3) -> Result<Vec<f64>, Refusal> {
        let req =
            json!({"cmd": "raycast", "s": s.handle, "origin": origin, "direction": direction});
        let v = self.call(s.worker, req)?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("raycast: {e}")))
    }

    fn raycasts(&self, s: &Handle, rays: &[(Vec3, Vec3)]) -> Result<Vec<Vec<f64>>, Refusal> {
        // A worker that reads only one ray a request answers the many-ray form with an error.
        let req = json!({"cmd": "raycast", "s": s.handle, "rays": rays});
        match self
            .call(s.worker, req)
            .map(serde_json::from_value::<Vec<Vec<f64>>>)
        {
            Ok(Ok(all)) if all.len() == rays.len() => Ok(all),
            _ => rays.iter().map(|&(o, d)| self.raycast(s, o, d)).collect(),
        }
    }

    fn reset(&self) {
        // Only this thread's worker: the others hold other threads' cases.
        let w = self.mine();
        let lost = || self.lost[w].lock().unwrap_or_else(|p| p.into_inner());
        *lost() = None;
        let _ =
            std::panic::catch_unwind(AssertUnwindSafe(|| self.call(w, json!({"cmd": "reset"}))));
        // A worker lost on the reset holds no case's solids, so its loss is not the next case's:
        // that case starts on a new worker, and the loss is reported here.
        if let Some(Loss::Died(what) | Loss::TimedOut(what)) = lost().take() {
            eprintln!("worker {w} lost between cases: {what}");
        }
    }

    fn unsupported(&self) -> Vec<String> {
        let supports: Vec<String> =
            serde_json::from_value(self.hello["supports"].clone()).unwrap_or_default();
        FUNCTIONS
            .iter()
            .filter(|&&f| !supports.iter().any(|s| s == f))
            .map(|&f| String::from(f))
            .collect()
    }

    fn write_step(&self, s: &Handle) -> Result<String, Refusal> {
        let v = self.call(s.worker, json!({"cmd": "write_step", "s": s.handle}))?;
        v.as_str()
            .map(String::from)
            .ok_or_else(|| "write_step returned no text".into())
    }

    fn read_step(&self, text: &str) -> Result<Handle, Refusal> {
        let w = self.mine();
        let v = self.call(w, json!({"cmd": "read_step", "text": text}))?;
        self.handle(w, v)
    }

    fn centroid(&self, s: &Handle) -> Result<Option<Vec3>, Refusal> {
        let v = self.call(s.worker, json!({"cmd": "centroid", "s": s.handle}))?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("centroid: {e}")))
    }

    fn inertia(&self, s: &Handle) -> Result<[[f64; 3]; 3], Refusal> {
        let v = self.call(s.worker, json!({"cmd": "inertia", "s": s.handle}))?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("inertia: {e}")))
    }

    fn distance(&self, a: &Handle, b: &Handle) -> Result<f64, Refusal> {
        assert_eq!(a.worker, b.worker, "both operands live in one worker");
        let v = self.call(
            a.worker,
            json!({"cmd": "distance", "a": a.handle, "b": b.handle}),
        )?;
        v.as_f64()
            .ok_or_else(|| Refusal::failed(format!("distance: {v} is not a number")))
    }

    fn classify(&self, s: &Handle, points: &[Vec3]) -> Result<Vec<Side>, Refusal> {
        let req = json!({"cmd": "classify", "s": s.handle, "points": points});
        let v = self.call(s.worker, req)?;
        v.as_array()
            .and_then(|words| {
                let side = |word: &Value| Side::from_word(word.as_str()?);
                words.iter().map(side).collect()
            })
            .ok_or_else(|| Refusal::failed(format!("classify: {v} is not a list of sides")))
    }

    fn closest(&self, s: &Handle, points: &[Vec3]) -> Result<Vec<Vec3>, Refusal> {
        let v = self.call(
            s.worker,
            json!({"cmd": "closest", "s": s.handle, "points": points}),
        )?;
        serde_json::from_value(v).map_err(|e| Refusal::failed(format!("closest: {e}")))
    }
}

impl Remote {
    fn combine(&self, op: Op, a: &Handle, b: &Handle) -> Result<Handle, Refusal> {
        assert_eq!(a.worker, b.worker, "both operands live in one worker");
        let req = json!({"cmd": op.word(), "a": a.handle, "b": b.handle});
        let v = self.call(a.worker, req)?;
        self.handle(a.worker, v)
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
            Remote::new(cmd, 1, Duration::from_secs(10))
        };
        let ours =
            format!(r#"{{"protocol": "{PROTOCOL_VERSION}", "supports": ["box", "volume"]}}"#);
        let k = worker(&ours).unwrap_or_else(|e| panic!("{e}"));
        // What the worker does not list, of this version's functions, is what it lacks.
        let lacks = k.unsupported();
        assert_eq!(lacks.len(), FUNCTIONS.len() - 2);
        assert!(!lacks.iter().any(|f| f == "box" || f == "volume"));
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
        let k = Remote::new(cmd, 1, Duration::from_secs(10)).unwrap();
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
        let k = Remote::new(cmd, 1, Duration::from_secs(10)).unwrap();
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
