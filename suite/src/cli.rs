//! The suite's command: generate the cases, run them on a kernel's adapter, and write the results.
//!
//! ```text
//! brep-conformance (--adapter "<command>" | --kernel NAME [--work DIR]) [--peer "<command>"]
//!                  [--nist DIR] [--family A,B] [--timeout S] [--seed N] [--threads N] [--out FILE]
//!                  [--glb DIR] [--step DIR] [--html FILE [--viewer viewer.wasm]]
//! ```
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
                 [--threads N] [--out FILE] [--glb DIR] [--step DIR] \
                 [--html FILE [--viewer viewer.wasm]]"
            );
            return ExitCode::from(2);
        }
    };
    let seed: u64 = value("--seed").map_or(1, |s| s.parse().expect("--seed takes a number"));
    let threads = value("--threads").map_or_else(
        || std::thread::available_parallelism().map_or(4, |n| n.get()),
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
    // One worker started here, to check it before the run, and dropped before the run starts its
    // own: a run holds no more workers than threads.
    let kernel = match Remote::new(adapter.clone(), limit) {
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
    // `--nist DIR` adds a case for each of NIST's STEP files there, as `nist/fetch.py` leaves them.
    let mut all = generate::cases(seed);
    if let Some(dir) = value("--nist") {
        match generate::nist_cases(std::path::Path::new(&dir)) {
            Ok(more) => all.extend(more),
            Err(e) => {
                eprintln!("--nist: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let cases: Vec<_> = all
        .into_iter()
        .filter(|c| {
            only.as_ref().is_none_or(|o| {
                o.iter()
                    .any(|f| f == c.family || c.test == f || c.test.starts_with(f.as_str()))
            })
        })
        .collect();
    // A second kernel's adapter, to read the STEP files this one writes.
    let peer: Option<Vec<String>> = match value("--peer") {
        Some(cmd) => {
            let cmd: Vec<String> = cmd.split_whitespace().map(String::from).collect();
            // Checked here, as the kernel's own worker is.
            match Remote::new(cmd.clone(), limit) {
                Ok(_) => Some(cmd),
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
    drop(kernel);
    let start = |cmd: &[String]| Remote::new(cmd.to_vec(), limit);
    let make_peer = peer.as_deref().map(|cmd| move || start(cmd));
    let run = run::run(
        || start(&adapter),
        &cases,
        threads,
        make_peer
            .as_ref()
            .map(|f| f as &(dyn Fn() -> Result<Remote, String> + Sync)),
        &exports,
    );
    let mut results = match run {
        Ok(results) => results,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if let Some(e) = &official {
        results["adapter"] = e.stamp();
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
