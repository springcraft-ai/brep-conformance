//! The suite's command: generate the cases, run them on a kernel's adapter, and write the results.
//!
//! ```text
//! brep-conformance (--adapter "<command>" | --kernel NAME [--work DIR]) [--peer "<command>"]
//!                  (--nist DIR | --skip-nist) [--family A,B] [--timeout S] [--seed N] [--threads N]
//!                  [--out FILE] [--glb DIR] [--step DIR] [--html FILE [--viewer viewer.wasm]]
//! ```
//!
//! A complete run reads NIST's STEP files, so a run without them says so with `--skip-nist`.
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
use clap::{ArgGroup, Parser};

use crate::{generate, html, registry, report, run};

#[derive(Parser)]
#[command(
    name = "brep-conformance",
    about = "Run the conformance suite on a kernel's adapter"
)]
#[command(group(ArgGroup::new("source").required(true).args(["adapter", "kernel"])))]
#[command(group(ArgGroup::new("nist_files").required(true).args(["nist", "skip_nist"])))]
struct Args {
    /// The adapter's command, split on whitespace.
    #[arg(long, value_name = "COMMAND")]
    adapter: Option<String>,
    /// Make an official run with the adapter the registry pins for this kernel.
    #[arg(long, value_name = "NAME")]
    kernel: Option<String>,
    /// Where `--kernel` checks out and builds the adapter.
    #[arg(long, value_name = "DIR", default_value = "adapters")]
    work: PathBuf,
    /// A second kernel's adapter, to read the STEP files this one writes.
    #[arg(long, value_name = "COMMAND")]
    peer: Option<String>,
    /// Add a case for each of NIST's STEP files here, as `nist/fetch.py` leaves them.
    #[arg(long, value_name = "DIR")]
    nist: Option<PathBuf>,
    /// Run without NIST's STEP files.
    #[arg(long)]
    skip_nist: bool,
    /// Run only these tests: test paths, directories such as `fillets/`, or family names.
    #[arg(long, value_name = "A,B", value_delimiter = ',')]
    family: Option<Vec<String>>,
    /// The limit on one call, in seconds.
    #[arg(long, value_name = "S", default_value_t = 120.0)]
    timeout: f64,
    /// The seed that draws the cases.
    #[arg(long, value_name = "N", default_value_t = 1)]
    seed: u64,
    /// The number of workers [default: the available parallelism]
    #[arg(long, value_name = "N")]
    threads: Option<usize>,
    /// The results file.
    #[arg(long, value_name = "FILE", default_value = "conformance-results.json")]
    out: PathBuf,
    /// Write every case's shapes as GLB files under this folder.
    #[arg(long, value_name = "DIR")]
    glb: Option<PathBuf>,
    /// Write every case's shapes as STEP files under this folder.
    #[arg(long, value_name = "DIR")]
    step: Option<PathBuf>,
    /// Also write the run as a report page.
    #[arg(long, value_name = "FILE")]
    html: Option<PathBuf>,
    /// The shape viewer's WebAssembly, to embed in the report page.
    #[arg(long, value_name = "viewer.wasm")]
    viewer: Option<PathBuf>,
}

pub fn main() -> ExitCode {
    let args = Args::parse();
    let (adapter, official) = match (args.adapter, args.kernel) {
        (Some(adapter), None) => (adapter.split_whitespace().map(String::from).collect(), None),
        (None, Some(kernel)) => {
            match registry::find(&kernel).and_then(|e| Ok((e.prepare(&args.work)?, e))) {
                Ok((command, entry)) => (command, Some(entry)),
                Err(e) => {
                    eprintln!("--kernel {kernel}: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => unreachable!("clap requires exactly one of --adapter and --kernel"),
    };
    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));

    // Read before the run, so a wrong path costs no run.
    let viewer = match args
        .viewer
        .map(|path| std::fs::read(&path).map_err(|e| (path, e)))
    {
        Some(Ok(bytes)) => Some(bytes),
        Some(Err((path, e))) => {
            eprintln!("--viewer {}: {e}", path.display());
            return ExitCode::from(2);
        }
        None => None,
    };

    let limit = std::time::Duration::from_secs_f64(args.timeout);
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
    let only = args.family;
    // `--nist DIR` adds a case for each of NIST's STEP files there, as `nist/fetch.py` leaves them.
    let mut all = generate::cases(args.seed);
    if let Some(dir) = args.nist {
        match generate::nist_cases(&dir) {
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
    let peer: Option<Vec<String>> = match args.peer {
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
        glb: args.glb,
        step: args.step,
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
        Some(limit),
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
    std::fs::write(&args.out, serde_json::to_string_pretty(&results).unwrap())
        .expect("writing results");
    print!("{}", report::summary(&results));
    println!("\nresults: {}", args.out.display());
    if let Some(path) = args.html {
        let page = html::page(std::slice::from_ref(&results), viewer.as_deref());
        std::fs::write(&path, page).expect("writing the report");
        println!("report: {}", path.display());
    }

    ExitCode::SUCCESS
}
