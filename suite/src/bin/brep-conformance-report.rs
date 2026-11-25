//! Writes the HTML report for one or more results files, one per kernel.
//!
//! ```text
//! brep-conformance-report RESULTS.json... [--out report.html] [--viewer viewer.wasm]
//! brep-conformance-report RESULTS.json --flat report.json [--verified] [--bench DIR --run ID]
//! ```
//!
//! `--flat` writes one run's report as the results site reads it: a header and one row per case.
//! `--bench` names the folder a benchmark session left beside the results, and `--run` the
//! workflow run and attempt: the report then records the run, the commits it built and the machine.
//!
//! `--viewer` embeds the shape viewer, built by
//! `cargo build --release --target wasm32-unknown-unknown -p brep-conformance-viewer`:
//!
//! ```text
//! ```

use std::path::{Path, PathBuf};

use brep_conformance::{html, report};
use clap::Parser;
use serde_json::Value;

#[derive(Parser)]
#[command(
    name = "brep-conformance-report",
    about = "Write the report page for results files"
)]
struct Args {
    /// The results files, one per kernel.
    #[arg(required = true, value_name = "RESULTS.json")]
    results: Vec<PathBuf>,
    /// The report page.
    #[arg(long, value_name = "FILE", default_value = "conformance-report.html")]
    out: PathBuf,
    /// The shape viewer's WebAssembly, to embed in the page.
    #[arg(long, value_name = "viewer.wasm")]
    viewer: Option<PathBuf>,
    /// Write one run's report as the results site reads it, instead of the page.
    #[arg(long, value_name = "FILE")]
    flat: Option<PathBuf>,
    /// Mark the flat report as made by the benchmark workflow.
    #[arg(long, requires = "flat")]
    verified: bool,
    /// The folder a benchmark session left beside the results.
    #[arg(long, value_name = "DIR", requires_all = ["flat", "run"])]
    bench: Option<PathBuf>,
    /// The workflow run and attempt the benchmark session belongs to.
    #[arg(long, value_name = "ID", requires = "bench")]
    run: Option<String>,
}

fn read(path: &Path) -> Value {
    let p = path.display();
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("reading {p}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parsing {p}: {e}"))
}

fn main() {
    let args = Args::parse();
    let runs: Vec<Value> = args.results.iter().map(|p| read(p)).collect();
    if let Some(path) = args.flat {
        assert!(runs.len() == 1, "--flat takes one results file");
        let session = match (args.bench, args.run) {
            (Some(dir), Some(run)) => {
                let kernel = runs[0]["kernel"]["name"]
                    .as_str()
                    .expect("the results name no kernel");
                report::session(&dir, kernel, &run)
                    .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            }
            _ => report::Session::default(),
        };
        let text =
            serde_json::to_string_pretty(&report::flat(&runs[0], args.verified, &session)).unwrap();
        std::fs::write(&path, text + "\n").expect("writing the flat report");
        println!("{}", path.display());
        return;
    }
    let viewer = args
        .viewer
        .map(|p| std::fs::read(&p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display())));
    std::fs::write(&args.out, html::page(&runs, viewer.as_deref())).expect("writing the report");
    println!("{}", args.out.display());
}
