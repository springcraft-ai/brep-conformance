//! The suite's command: generate the cases, run them on a kernel's adapter, and write the results.
//!
//! ```text
//! brep-conformance (--adapter "<command>" | --kernel NAME [--work DIR]) [--peer "<command>"]
//!                  [--nist DIR] [--family A,B] [--timeout S] [--seed N] [--threads N] [--pin LIST]
//!                  [--out FILE] [--glb DIR] [--step DIR] [--html FILE [--viewer viewer.wasm]]
//! ```
//!
//! `--pin 24-47` runs each worker of the kernel's adapter on one of the listed cores alone, for
//! timings on cores the system keeps for them; the results record the list. Linux only.
//!
//! `--kernel` makes an official run: the adapter the [registry](crate::registry) pins for that
//! kernel, checked out and built under `--work` (`adapters` by default), which must report the
//! kernel release the registry names. The results record the adapter's repository and commit.
//!
//! `--html` also writes the run as a report page, one file that holds its own data and opens
//! without the network.
//!
//! The adapter's command is split on whitespace and started once per thread; it answers the protocol
//! on stdin and stdout. `--glb` and `--step` write every case's shape files as the kernel draws
//! them, failed cases' included, under the keys of [`crate::assets`].

use std::path::PathBuf;
use std::process::ExitCode;

use brep_protocol::{Kernel, Remote};

use crate::{generate, html, registry, report, run};

pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (adapter, official) = match (value("--adapter"), value("--kernel")) {
        (Some(adapter), None) => (adapter.split_whitespace().map(String::from).collect(), None),
        (None, Some(kernel)) => {
            let work = PathBuf::from(value("--work").unwrap_or_else(|| "adapters".into()));
            match registry::find(&kernel).and_then(|e| Ok((e.prepare(&work)?, e))) {
                Ok((command, entry)) => (command, Some(entry)),
                Err(e) => {
                    eprintln!("--kernel {kernel}: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => {
            eprintln!(
                "usage: brep-conformance (--adapter \"<command>\" | --kernel NAME [--work DIR]) \
                 [--peer \"<command>\"] [--nist DIR] [--family A,B] [--timeout S] [--seed N] \
                 [--threads N] [--pin LIST] [--out FILE] [--glb DIR] [--step DIR] \
                 [--html FILE [--viewer viewer.wasm]]"
            );
            return ExitCode::from(2);
        }
    };
    let seed: u64 = value("--seed").map_or(1, |s| s.parse().expect("--seed takes a number"));
    let pin = match value("--pin").map(|list| brep_protocol::cores(&list)) {
        Some(Ok(cores)) => Some(cores),
        Some(Err(e)) => {
            eprintln!("--pin: {e}");
            return ExitCode::from(2);
        }
        None => None,
    };
    let threads = value("--threads").map_or_else(
        || match &pin {
            Some(cores) => cores.len(),
            None => std::thread::available_parallelism().map_or(4, |n| n.get()),
        },
        |s| s.parse().expect("--threads takes a number"),
    );
    let out = PathBuf::from(value("--out").unwrap_or_else(|| "conformance-results.json".into()));

    // Read before the run, so a wrong path costs no run.
    let viewer = match value("--viewer").map(|path| std::fs::read(&path).map_err(|e| (path, e))) {
        Some(Ok(bytes)) => Some(bytes),
        Some(Err((path, e))) => {
            eprintln!("--viewer {path}: {e}");
            return ExitCode::from(2);
        }
        None => None,
    };

    let limit = value("--timeout").map_or(120.0, |s| s.parse().expect("--timeout takes seconds"));
    let limit = std::time::Duration::from_secs_f64(limit);
    let kernel = match Remote::pinned(adapter, threads, limit, pin.clone().unwrap_or_default()) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if let Some(e) = &official {
        let (name, version) = (kernel.name(), kernel.version());
        if name != e.kernel || version != e.version {
            eprintln!(
                "the adapter at {} reports {name} {version}; the registry pins {} {}",
                e.commit, e.kernel, e.version
            );
            return ExitCode::from(2);
        }
    }
    // `--family` runs only the named families, comma-separated, for a quick look at one area.
    let only: Option<Vec<String>> =
        value("--family").map(|f| f.split(',').map(String::from).collect());
    let cases: Vec<_> = generate::cases(seed)
        .into_iter()
        .filter(|c| {
            only.as_ref().is_none_or(|o| {
                o.iter()
                    .any(|f| f == c.family || c.test == f || c.test.starts_with(f.as_str()))
            })
        })
        .collect();
    // A second kernel's adapter, to read the STEP files this one writes.
    let peer = match value("--peer") {
        Some(cmd) => {
            let cmd = cmd.split_whitespace().map(String::from).collect();
            match Remote::new(cmd, threads, limit) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("the peer: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        None => None,
    };
    let exports = run::Exports {
        glb: value("--glb").map(PathBuf::from),
        step: value("--step").map(PathBuf::from),
    };
    let mut results = run::run(&kernel, &cases, threads, peer.as_ref(), &exports);
    results["environment"]["pinned"] = serde_json::json!(pin);
    if let Some(e) = &official {
        results["adapter"] = e.stamp();
    }
    // `--nist DIR` reads NIST's STEP files there as an interop record, apart from the score.
    if let Some(dir) = value("--nist") {
        match run::nist(&kernel, std::path::Path::new(&dir)) {
            Ok(v) => results["nist"] = v,
            Err(e) => {
                eprintln!("--nist: {e}");
                return ExitCode::from(2);
            }
        }
    }
    std::fs::write(&out, serde_json::to_string_pretty(&results).unwrap()).expect("writing results");
    print!("{}", report::summary(&results));
    println!("\nresults: {}", out.display());
    if let Some(path) = value("--html") {
        let page = html::page(std::slice::from_ref(&results), viewer.as_deref());
        std::fs::write(&path, page).expect("writing the report");
        println!("report: {path}");
    }

    ExitCode::SUCCESS
}
