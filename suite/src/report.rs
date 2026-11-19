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
    if let Some(files) = results["nist"].as_array() {
        let read = files.iter().filter(|f| f["volume"].is_number()).count();
        let judged: Vec<bool> = files.iter().filter_map(|f| f["agrees"].as_bool()).collect();
        let agree = judged.iter().filter(|&&a| a).count();
        out += &format!(
            "\nNIST files (interop, not scored): read {read} of {}; {agree} of the {} that \
             state a volume or area agree with it\n",
            files.len(),
            judged.len()
        );
    }
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

/// One run as the results site reads it: who ran what and where, the totals, and one row per case
/// with its tier, the reason it is not a pass, its time and its group's median. `verified` says the
/// run came from the suite's own benchmark machine.
pub fn flat(results: &Value, verified: bool) -> Value {
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
    json!({
        "format": crate::run::RESULTS_FORMAT,
        "kernel": results["kernel"],
        "adapter": results["adapter"],
        "assets": assets,
        "suite": {"version": results["suite"], "contract": results["contract"]},
        "run": {"unix": results["unix"], "verified": verified},
        "environment": results["environment"],
        "summary": {
            "cases": rows.len(),
            "pass_1e7": if rows.is_empty() { 0.0 } else { passed as f64 / rows.len() as f64 },
            "results": counts,
        },
        "unsupported": results["unsupported"],
        "stand_ins": results["stand_ins"],
        "cases": rows,
        // Interop with NIST's files, as the run read them; no part of the summary above.
        "nist": results["nist"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    #[test]
    fn a_flat_report_counts_every_case_and_times_those_that_answered() {
        let test = generate::cases(1).into_iter().next().unwrap().test;
        let case = |result: &str, ms: f64| json!({"id": "x", "test": test, "result": result, "kind": "exact", "time_ms": ms});
        let results = json!({"cases": [
            case("1e-7", 3.0), case("1e-6", 5.0), case("1e-7", 4.0),
            case("refused", 70.0), case("unsupported", 0.0),
        ]});
        let flat = flat(&results, true);
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
    fn a_flat_report_keeps_the_nist_record_apart_from_the_score() {
        let test = generate::cases(1).into_iter().next().unwrap().test;
        let cases = json!([{"id": "x", "test": test, "result": "1e-7", "kind": "exact"}]);
        let nist = json!([
            {"file": "ap203/a.stp", "stated_volume": 2.0, "stated_area": null, "ms": 9.0,
             "volume": 2.0, "area": 7.0, "valid": true, "agrees": true},
            {"file": "ap203/b.stp", "stated_volume": null, "stated_area": null, "ms": 1.0,
             "outcome": "refused", "error": "no reader"},
        ]);
        let with = flat(&json!({"cases": cases, "nist": nist}), false);
        let without = flat(&json!({"cases": cases}), false);
        assert_eq!(with["nist"], nist);
        assert!(without["nist"].is_null());
        assert_eq!(with["summary"], without["summary"]);
    }

    #[test]
    fn a_flat_report_names_where_each_case_s_files_are_published() {
        let ex = json!({"glb": "t/a.h.glb", "step": ["t/a.h.step"], "errors": [], "skipped": []});
        let results = json!({"suite": "1.0.0", "kernel": {"name": "k", "version": "2.1"},
            "cases": [{"id": "a", "test": "t", "result": "1e-7", "exports": ex},
                      {"id": "b", "test": "t", "result": "1e-7"}]});
        let flat = flat(&results, false);
        assert_eq!(
            flat["assets"],
            format!("{}/1.0.0/k/2.1/", crate::assets::ASSET_BASE)
        );
        assert_eq!(
            flat["cases"][0]["exports"],
            json!({"glb": "t/a.h.glb", "step": ["t/a.h.step"]})
        );
        assert!(flat["cases"][1].get("exports").is_none());
        let none = super::flat(&json!({"cases": []}), false);
        assert!(none["assets"].is_null());
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
