//! Writes the HTML report for one or more results files, one per kernel.
//!
//! ```text
//! brep-conformance-report RESULTS.json... [--out report.html] [--viewer viewer.wasm]
//! brep-conformance-report RESULTS.json --flat report.json [--verified]
//! ```
//!
//! `--flat` writes one run's report as the results site reads it: a header and one row per case.
//!
//! `--viewer` embeds the shape viewer, built by
//! `cargo build --release --target wasm32-unknown-unknown -p brep-conformance-viewer`:
//!
//! ```text
//! ```

use std::path::PathBuf;

use brep_conformance::{html, report};
use serde_json::Value;

fn read(path: &str) -> Value {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parsing {path}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut runs = Vec::new();
    let (mut flat, mut verified) = (None, false);
    let mut out = PathBuf::from("conformance-report.html");
    let mut viewer: Option<Vec<u8>> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--flat" => {
                flat = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--verified" => verified = true,
            "--viewer" => {
                let path = &args[i + 1];
                viewer =
                    Some(std::fs::read(path).unwrap_or_else(|e| panic!("reading {path}: {e}")));
                i += 1;
            }
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            path => runs.push(read(path)),
        }
        i += 1;
    }
    assert!(!runs.is_empty(), "name at least one results file");
    if let Some(path) = flat {
        assert!(runs.len() == 1, "--flat takes one results file");
        let text = serde_json::to_string_pretty(&report::flat(&runs[0], verified)).unwrap();
        std::fs::write(&path, text + "\n").expect("writing the flat report");
        println!("{}", path.display());
        return;
    }
    std::fs::write(&out, html::page(&runs, viewer.as_deref())).expect("writing the report");
    println!("{}", out.display());
}
