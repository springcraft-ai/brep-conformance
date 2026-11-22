//! What a run's results say: a summary per test and per clause.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// Every case's test path by its id, for results written before cases carried their test.
fn tests_by_id() -> &'static BTreeMap<String, &'static str> {
    static MAP: std::sync::OnceLock<BTreeMap<String, &'static str>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        crate::generate::cases(1)
            .into_iter()
            .map(|c| (c.id, c.test))
            .collect()
    })
}

/// The test a result belongs to: as it says, or as its id's case says.
pub fn test_of(c: &Value) -> String {
    if let Some(t) = c["test"].as_str() {
        return t.to_string();
    }
    let id = c["id"].as_str().unwrap_or("?");
    tests_by_id()
        .get(id)
        .map_or_else(|| "unsorted".to_string(), |t| (*t).to_string())
}

/// The directory a test is reported under: the first part of its path.
pub fn area(test: &str) -> &str {
    test.split('/').next().unwrap_or("unsorted")
}

/// The suite version a run was made on, from a results file or a flat report.
pub fn suite_of(run: &Value) -> &str {
    let suite = &run["suite"];
    suite["version"].as_str().or(suite.as_str()).unwrap_or("?")
}

/// Runs by the suite version they were made on, newest version first: scores compare only within
/// one version. A version that is not three numbers sorts after those that are.
pub fn by_suite(runs: &[Value]) -> Vec<(&str, Vec<&Value>)> {
    let key = |v: &str| {
        let parts: Option<Vec<u64>> = v.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() == 3)
    };
    let mut groups: Vec<(&str, Vec<&Value>)> = Vec::new();
    for run in runs {
        let version = suite_of(run);
        match groups.iter_mut().find(|(v, _)| *v == version) {
            Some((_, group)) => group.push(run),
            None => groups.push((version, vec![run])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| key(b).cmp(&key(a)).then_with(|| b.cmp(a)));
    groups
}

/// A run's coverage: cases, the cases the kernel offers what they call, and those that pass.
pub fn coverage(results: &Value) -> (usize, usize, usize) {
    let cases = results["cases"]
        .as_array()
        .map_or(&[][..], |v| v.as_slice());
    let offered = cases
        .iter()
        .filter(|c| c["outcome"] != "unsupported")
        .count();
    let pass = cases.iter().filter(|c| c["outcome"] == "pass").count();
    (cases.len(), offered, pass)
}

/// Outcome counts per test, and failed checks per clause.
pub fn summary(results: &Value) -> String {
    let mut fam: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut clause: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for c in results["cases"].as_array().into_iter().flatten() {
        let f = test_of(c);
        let o = c["outcome"].as_str().unwrap_or("?").to_string();
        *fam.entry(f).or_default().entry(o).or_default() += 1;
        for ch in c["checks"].as_array().into_iter().flatten() {
            let e = clause
                .entry(ch["clause"].as_str().unwrap_or("?").into())
                .or_default();
            e.0 += 1;
            if ch["ok"] == json!(false) {
                e.1 += 1;
            }
        }
    }
    let mut out = String::new();
    let k = &results["kernel"];
    out += &format!(
        "suite {} · contract {} · kernel {} {} · judged to {} mm (it declares {} mm)\n\n",
        results["suite"].as_str().unwrap_or("?"),
        results["contract"].as_str().unwrap_or("?"),
        k["name"].as_str().unwrap_or("?"),
        k["version"].as_str().unwrap_or("?"),
        results["tolerance"],
        k["tolerance"]
    );
    out += &format!(
        "{:<64} {:>5} {:>5} {:>6} {:>7} {:>7} {:>5} {:>6}\n",
        "test", "cases", "pass", "unsupp", "refused", "timeout", "fail", "crash"
    );
    let mut total: BTreeMap<String, usize> = BTreeMap::new();
    let row = |name: &str, counts: &BTreeMap<String, usize>| {
        let n: usize = counts.values().sum();
        let g = |o: &str| counts.get(o).copied().unwrap_or(0);
        format!(
            "{name:<64} {n:>5} {:>5} {:>6} {:>7} {:>7} {:>5} {:>6}\n",
            g("pass"),
            g("unsupported"),
            g("refused"),
            g("timeout"),
            g("fail"),
            g("crash")
        )
    };
    let mut groups: Vec<&str> = crate::generate::DIRS.to_vec();
    groups.push("unsorted");
    for group in groups {
        let in_group: Vec<_> = fam.iter().filter(|(f, _)| area(f) == group).collect();
        if in_group.is_empty() {
            continue;
        }
        out += &format!("{group}\n");
        let mut sub: BTreeMap<String, usize> = BTreeMap::new();
        for (f, counts) in in_group {
            let name = f.split_once('/').map_or(f.as_str(), |(_, rest)| rest);
            out += &row(&format!("  {name}"), counts);
            for (o, v) in counts {
                *total.entry(o.clone()).or_default() += v;
                *sub.entry(o.clone()).or_default() += v;
            }
        }
        out += &row(&format!("  ({group})"), &sub);
    }
    out += &row("all", &total);
    let (n, offered, pass) = coverage(results);
    out += &format!(
        "offers what {offered} of {n} cases call; passes {pass}, {:.1}% of those\n\n",
        100.0 * pass as f64 / offered.max(1) as f64
    );
    out += &format!("{:<8} {:>7} {:>7}\n", "clause", "checks", "failed");
    for (c, (n, bad)) in &clause {
        out += &format!("{c:<8} {n:>7} {bad:>7}\n");
    }
    // Interop with a peer kernel scores neither kernel, so it is listed apart from the clauses.
    let interop: Vec<bool> = results["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["interop"]["agrees"].as_bool())
        .collect();
    if !interop.is_empty() {
        let agree = interop.iter().filter(|&&a| a).count();
        out += &format!(
            "\ninterop: the peer read {agree} of {} files as the solid they describe\n",
            interop.len()
        );
    }
    out += &timing(results);
    out
}

/// Each operation's time across the run as multiples of the calibration workload: the median, the
/// 95th percentile and the slowest, with the slowest call's case.
pub fn timing(results: &Value) -> String {
    let unit = results["calibration_ms"].as_f64().unwrap_or(f64::NAN);
    let mut by_op: BTreeMap<String, Vec<(f64, String)>> = BTreeMap::new();
    for c in results["cases"].as_array().into_iter().flatten() {
        for o in c["ops"].as_array().into_iter().flatten() {
            if let (Some(op), Some(ms)) = (o["op"].as_str(), o["ms"].as_f64()) {
                let id = c["id"].as_str().unwrap_or("?").to_string();
                by_op.entry(op.into()).or_default().push((ms, id));
            }
        }
    }
    let mut out = format!("\ntime in units of the calibration workload ({unit:.2} ms)\n");
    out += &format!(
        "{:<24} {:>6} {:>8} {:>8} {:>9}  slowest\n",
        "operation", "calls", "median", "p95", "max"
    );
    for (op, mut t) in by_op {
        t.sort_by(|a, b| a.0.total_cmp(&b.0));
        let at = |q: f64| t[((t.len() - 1) as f64 * q).round() as usize].0 / unit;
        let (max, id) = t.last().cloned().unwrap_or_default();
        out += &format!(
            "{op:<24} {:>6} {:>8.1} {:>8.1} {:>9.1}  {id}\n",
            t.len(),
            at(0.5),
            at(0.95),
            max / unit
        );
    }
    out
}

/// Each case's outcome by id.
pub fn outcomes(results: &Value) -> BTreeMap<String, String> {
    results["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let id = c["id"].as_str().unwrap_or("?").to_string();
            (id, c["outcome"].as_str().unwrap_or("?").to_string())
        })
        .collect()
}

/// The fewest timed cases a group needs for its median to be trusted.
const TIMED_ENOUGH: usize = 5;

/// What a benchmark session records beside one pass's results (bench/README.md): which run it was,
/// what it built, and the machine. Each field is absent for a run made anywhere else.
#[derive(Debug, Default, PartialEq)]
pub struct Session {
    /// The Benchmark workflow's run and attempt, such as `44984300614-1`.
    pub run: Option<String>,
    pub suite_commit: Option<String>,
    /// `{"repo", "commit"}` of the pass's adapter.
    pub adapter: Option<Value>,
    /// The machine's settings, from `environment.txt`.
    pub machine: Option<serde_json::Map<String, Value>>,
    /// The pass ran every test: no suite arguments, NIST's files where the suite had them, and a
    /// clean exit.
    pub complete: Option<bool>,
}

/// `key=value` lines; a line whose key is not a plain word, such as a pass's `name: exit=0`, is not
/// one.
fn key_values(text: &str) -> BTreeMap<&str, &str> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .collect()
}

/// The session the bench wrote in `dir` (`session.txt`, `build.txt`, `adapters.txt`,
/// `environment.txt`), for the pass of `kernel` in the run `run`.
pub fn session(dir: &std::path::Path, kernel: &str, run: &str) -> Result<Session, String> {
    let read =
        |name: &str| std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let (session, build, adapters, environment) = (
        read("session.txt")?,
        read("build.txt")?,
        read("adapters.txt")?,
        read("environment.txt")?,
    );
    let runs = key_values(&session);
    let exit = session
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{kernel}: exit=")))
        .and_then(|rest| rest.split_whitespace().next())
        .ok_or_else(|| format!("session.txt: no pass of {kernel}"))?;
    let adapter = adapters
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|w| w.len() == 3 && w[0] == kernel)
        .map(|w| json!({"repo": w[1], "commit": w[2]}))
        .ok_or_else(|| format!("adapters.txt: no adapter of {kernel}"))?;
    let machine = key_values(&environment)
        .into_iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect();
    // A session written before NIST's files existed has no `nist` line, and ran every test there
    // was.
    let complete = exit == "0"
        && runs.get("suite_args").is_some_and(|a| a.trim().is_empty())
        && runs.get("nist") != Some(&"false");
    Ok(Session {
        run: Some(run.to_string()),
        suite_commit: key_values(&build)
            .get("brep_conformance")
            .map(|c| c.to_string()),
        adapter: Some(adapter),
        machine: Some(machine),
        complete: Some(complete),
    })
}

/// One run as the results site reads it: who ran what and where, the totals, and one row per case
/// with its tier, the reason it is not a pass, its time and its group's median. `verified` says the
/// run came from the suite's own benchmark machine; `session` is what that machine recorded.
pub fn flat(results: &Value, verified: bool, session: &Session) -> Value {
    let cases = results["cases"]
        .as_array()
        .map_or(&[][..], |v| v.as_slice());
    // A group is a test: the median of its cases that gave an answer.
    let mut times: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for c in cases {
        let answered = crate::run::TIER_WORDS.iter().any(|t| c["result"] == *t);
        if let (true, Some(ms)) = (answered, c["time_ms"].as_f64()) {
            times.entry(test_of(c)).or_default().push(ms);
        }
    }
    let medians: BTreeMap<String, (f64, usize)> = times
        .into_iter()
        .map(|(test, mut ms)| {
            ms.sort_by(f64::total_cmp);
            (test, (ms[ms.len() / 2], ms.len()))
        })
        .collect();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let rows: Vec<Value> = cases
        .iter()
        .map(|c| {
            let test = test_of(c);
            let result = c["result"].as_str().unwrap_or("crash");
            *counts.entry(result.to_string()).or_default() += 1;
            let median = medians.get(&test);
            let mut row = json!({
                "id": c["id"],
                "test": test,
                "operation": area(&test),
                "kind": c["kind"],
                "result": result,
                "why": c["why"],
                "failure": c["failure"],
                "time_ms": c["time_ms"],
                // The B17 round trip's time, which `time_ms` leaves out; null where none ran.
                "step_ms": c["step_ms"],
                // Each step's time, so a reader can tell making the operands from the operation.
                "ops": c["ops"].as_array().map(|ops| {
                    ops.iter()
                        .map(|o| json!({"op": o["op"], "outcome": o["outcome"], "ms": o["ms"]}))
                        .collect::<Vec<_>>()
                }),
                // Null in a results file written before a case's start was recorded.
                "cold": c["cold"],
                "group_median_ms": median.map(|m| m.0),
                "group_low_confidence": median.is_none_or(|m| m.1 < TIMED_ENOUGH),
                "asset": c["asset"],
                "asset_step": c["asset_step"],
            });
            // The files written, below the kernel's folder; a case with none has no `exports`.
            let ex = &c["exports"];
            if ex.is_object() {
                row["exports"] = json!({"glb": ex["glb"], "step": ex["step"]});
            }
            row
        })
        .collect();
    let passed = counts.get("1e-7").copied().unwrap_or(0);
    // Where this run's files are published, when it wrote any: a case's `exports` keys follow it.
    let wrote = cases.iter().any(|c| c["exports"].is_object());
    let assets = match (
        wrote,
        results["suite"].as_str(),
        results["kernel"]["name"].as_str(),
        results["kernel"]["version"].as_str(),
    ) {
        (true, Some(suite), Some(kernel), Some(version)) => {
            json!(crate::assets::url(suite, kernel, version, ""))
        }
        _ => Value::Null,
    };
    // An official run's results name the adapter; the bench's passes record it beside them.
    let adapter = match &results["adapter"] {
        Value::Null => session.adapter.clone().unwrap_or(Value::Null),
        a => a.clone(),
    };
    let mut environment = results["environment"].clone();
    if let (Some(env), Some(machine)) = (environment.as_object_mut(), &session.machine) {
        env.insert("machine".into(), Value::Object(machine.clone()));
    }
    json!({
        "format": crate::run::RESULTS_FORMAT,
        "kernel": results["kernel"],
        "adapter": adapter,
        "assets": assets,
        "suite": {
            "version": results["suite"],
            "contract": results["contract"],
            "commit": session.suite_commit,
        },
        "run": {
            "id": session.run,
            "unix": results["unix"],
            "started_unix": results["started_unix"],
            "wall_ms": results["wall_ms"],
            "calibration_ms": results["calibration_ms"],
            "time_limit_ms": results["time_limit_ms"],
            "verified": verified,
            "complete": session.complete,
        },
        "environment": environment,
        "summary": {
            "cases": rows.len(),
            "pass_1e7": if rows.is_empty() { 0.0 } else { passed as f64 / rows.len() as f64 },
            "results": counts,
        },
        "unsupported": results["unsupported"],
        "stand_ins": results["stand_ins"],
        "cases": rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    #[test]
    fn a_flat_report_carries_the_run_s_timings_and_each_case_s_steps() {
        let test = generate::cases(1).into_iter().next().unwrap().test;
        let results = json!({
            "unix": 200, "started_unix": 100, "wall_ms": 100000.0, "calibration_ms": 7.5,
            "time_limit_ms": 120000.0,
            "cases": [
                {"id": "a", "test": test, "result": "1e-7", "kind": "exact", "time_ms": 3.0,
                 "step_ms": 8.0, "cold": true, "ops": [
                    {"op": "a", "outcome": "done", "ms": 1.0, "volume": 2.0},
                    {"op": "a ∪ b", "outcome": "done", "ms": 2.0}]},
                {"id": "b", "test": test, "result": "timeout", "kind": "exact",
                 "time_ms": 120000.0, "ops": [
                    {"op": "a ∪ b", "outcome": "timeout", "ms": 120000.0, "error": "late"}]},
            ],
        });
        let flat = flat(&results, false, &Session::default());
        assert_eq!(
            flat["run"],
            json!({"id": null, "unix": 200, "started_unix": 100, "wall_ms": 100000.0,
                   "calibration_ms": 7.5, "time_limit_ms": 120000.0, "verified": false,
                   "complete": null})
        );
        let [a, b] = [&flat["cases"][0], &flat["cases"][1]];
        // A step keeps its label, outcome and time, and nothing it found.
        assert_eq!(
            a["ops"],
            json!([{"op": "a", "outcome": "done", "ms": 1.0},
                   {"op": "a ∪ b", "outcome": "done", "ms": 2.0}])
        );
        assert_eq!(
            (a["cold"].clone(), b["cold"].clone()),
            (json!(true), Value::Null)
        );
        assert_eq!(b["ops"][0]["outcome"], "timeout");
        // The round trip's time travels beside the case's, null where none ran.
        assert_eq!(
            (a["step_ms"].clone(), b["step_ms"].clone()),
            (json!(8.0), Value::Null)
        );
    }

    #[test]
    fn a_flat_report_counts_every_case_and_times_those_that_answered() {
        let test = generate::cases(1).into_iter().next().unwrap().test;
        let case = |result: &str, ms: f64| json!({"id": "x", "test": test, "result": result, "kind": "exact", "time_ms": ms});
        let results = json!({"cases": [
            case("1e-7", 3.0), case("1e-6", 5.0), case("1e-7", 4.0),
            case("refused", 70.0), case("unsupported", 0.0),
        ]});
        let flat = flat(&results, true, &Session::default());
        assert_eq!(flat["format"], crate::run::RESULTS_FORMAT);
        assert_eq!(flat["summary"]["cases"], 5);
        assert_eq!(flat["summary"]["pass_1e7"], 0.4);
        assert_eq!(flat["summary"]["results"]["refused"], 1);
        // The refused case and the unsupported one gave no answer: 3, 4 and 5 are left.
        assert_eq!(flat["cases"][1]["group_median_ms"], 4.0);
        assert_eq!(flat["cases"][1]["group_low_confidence"], true);
        assert_eq!(flat["run"]["verified"], true);
        assert!(flat["cases"][0].get("exports").is_none());
    }

    #[test]
    fn a_flat_report_names_where_each_case_s_files_are_published() {
        let ex = json!({"glb": "t/a.h.glb", "step": ["t/a.h.step"], "errors": [], "skipped": []});
        let results = json!({"suite": "1.0.0", "kernel": {"name": "k", "version": "2.1"},
            "cases": [{"id": "a", "test": "t", "result": "1e-7", "exports": ex},
                      {"id": "b", "test": "t", "result": "1e-7"}]});
        let flat = flat(&results, false, &Session::default());
        assert_eq!(
            flat["assets"],
            format!("{}/1.0.0/k/2.1/", crate::assets::ASSET_BASE)
        );
        assert_eq!(
            flat["cases"][0]["exports"],
            json!({"glb": "t/a.h.glb", "step": ["t/a.h.step"]})
        );
        assert!(flat["cases"][1].get("exports").is_none());
        let none = super::flat(&json!({"cases": []}), false, &Session::default());
        assert!(none["assets"].is_null());
    }

    /// A session folder as the bench leaves it, with `session.txt` as given.
    fn session_dir(session: &str) -> tempfile_dir::Dir {
        let dir = tempfile_dir::Dir::new();
        let write = |name: &str, text: &str| std::fs::write(dir.0.join(name), text).unwrap();
        write("session.txt", session);
        write(
            "build.txt",
            "brep_conformance=29e5bcb\nadapter_k=bf3b059\nrustc=rustc 1.99.0\n",
        );
        write(
            "adapters.txt",
            "k https://example.com/k-adapter bf3b059\nj https://e.com/j 1\n",
        );
        write("environment.txt", "instance_type=c7i.metal-24xl\nsmt=off\n");
        dir
    }

    mod tempfile_dir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Self {
                use std::sync::atomic::{AtomicUsize, Ordering};
                static NEXT: AtomicUsize = AtomicUsize::new(0);
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                let dir =
                    std::env::temp_dir().join(format!("brep-session-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&dir).unwrap();
                Dir(dir)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn a_session_names_the_run_the_commits_and_the_machine() {
        let dir = session_dir("threads=24\nsuite_args=\nnist=true\nk: exit=0 seconds=176\n");
        let s = session(&dir.0, "k", "44984300614-1").unwrap();
        assert_eq!(s.run.as_deref(), Some("44984300614-1"));
        assert_eq!(s.suite_commit.as_deref(), Some("29e5bcb"));
        assert_eq!(
            s.adapter,
            Some(json!({"repo": "https://example.com/k-adapter", "commit": "bf3b059"}))
        );
        let machine = s.machine.as_ref().unwrap();
        assert_eq!(machine["instance_type"], "c7i.metal-24xl");
        assert_eq!(machine["smt"], "off");
        assert_eq!(s.complete, Some(true));
    }

    #[test]
    fn a_pass_is_complete_only_with_every_test_and_a_clean_exit() {
        let complete = |text: &str| session(&session_dir(text).0, "k", "1-1").unwrap().complete;
        assert_eq!(
            complete("suite_args=--family boxes\nnist=true\nk: exit=0\n"),
            Some(false)
        );
        assert_eq!(
            complete("suite_args=\nnist=false\nk: exit=0\n"),
            Some(false)
        );
        assert_eq!(
            complete("suite_args=\nnist=true\nk: exit=invalid(1 unplaced)\n"),
            Some(false)
        );
        // Before NIST's files, a session wrote no `nist` line and ran every test there was.
        assert_eq!(complete("suite_args=\nk: exit=0 seconds=182\n"), Some(true));
        assert!(session(&session_dir("suite_args=\nj: exit=0\n").0, "k", "1-1").is_err());
    }

    #[test]
    fn a_flat_report_carries_what_the_session_recorded() {
        let dir = session_dir("suite_args=\nnist=true\nk: exit=0\n");
        let s = session(&dir.0, "k", "7-2").unwrap();
        let results = json!({"suite": "2.0.0", "contract": "2.0.0", "cases": [],
            "kernel": {"name": "k", "version": "0.2.9"}, "environment": {"os": "linux"}});
        let flat = flat(&results, true, &s);
        assert_eq!(flat["run"]["id"], "7-2");
        assert_eq!(flat["run"]["complete"], true);
        assert_eq!(flat["suite"]["commit"], "29e5bcb");
        assert_eq!(flat["adapter"]["commit"], "bf3b059");
        assert_eq!(flat["environment"]["os"], "linux");
        assert_eq!(
            flat["environment"]["machine"]["instance_type"],
            "c7i.metal-24xl"
        );
        // An official run's own record of its adapter is kept.
        let official = json!({"adapter": {"repo": "r", "commit": "c"}, "environment": {}});
        assert_eq!(super::flat(&official, true, &s)["adapter"]["commit"], "c");
    }

    #[test]
    fn runs_group_by_suite_version_newest_first() {
        let run = |suite: Value, kernel: &str| json!({"suite": suite, "kernel": {"name": kernel}});
        let runs = [
            run(json!("1.2.0"), "a"),
            run(json!("1.10.0"), "b"),
            run(json!({"version": "1.2.0"}), "c"),
            run(json!("draft"), "d"),
            run(json!("1.10.1"), "e"),
        ];
        let groups: Vec<(&str, Vec<&str>)> = by_suite(&runs)
            .into_iter()
            .map(|(v, group)| {
                let names = group.iter().map(|r| r["kernel"]["name"].as_str().unwrap());
                (v, names.collect())
            })
            .collect();
        assert_eq!(
            groups,
            [
                ("1.10.1", vec!["e"]),
                ("1.10.0", vec!["b"]),
                ("1.2.0", vec!["a", "c"]),
                ("draft", vec!["d"]),
            ]
        );
    }

    #[test]
    fn the_changelog_opens_with_the_suite_version() {
        let changelog = include_str!("../../CHANGELOG.md");
        let newest = changelog.lines().find_map(|l| l.strip_prefix("## "));
        let version = newest.and_then(|l| l.split_whitespace().next());
        assert_eq!(version, Some(crate::run::SUITE_VERSION));
    }

    #[test]
    fn a_result_without_its_test_finds_it_by_id() {
        let c = generate::cases(1).into_iter().next().unwrap();
        let old = json!({"id": c.id, "family": c.family, "outcome": "pass"});
        assert_eq!(test_of(&old), c.test);
        assert_eq!(area(&test_of(&old)), c.test.split('/').next().unwrap());
    }
}
