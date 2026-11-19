//! The run report as one static HTML page: each kernel's counts, then the tests by directory, each
//! test opening to its cases with every kernel's result and time, failures opening to say why. Case
//! rows are drawn from data in the page when their test opens, so the page stays light.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::Value;

/// The clauses' promises, as `doc/contract.md` states them.
const CLAUSES: &[(&str, &str)] = &[
    ("B0", "Each operand is the shape described"),
    (
        "B1",
        "Every result and built operand is valid by the kernel's own checker",
    ),
    ("B2", "The intersection meets its closed form"),
    ("B3", "|A ∪ B| + |A ∩ B| = |A| + |B|"),
    ("B4", "|A ∖ B| + |A ∩ B| = |A|"),
    ("B5", "|A ∩ B| = |B ∩ A|"),
    ("B6", "A rigid motion of both leaves |A ∩ B| unchanged"),
    ("B7", "The operations leave their operands unchanged"),
    ("B8", "|A ∩ B| meets the suite's own sampled estimate"),
    (
        "B9",
        "The kernel places sample points as the exact operands do",
    ),
    (
        "B10",
        "Each primitive operand's moments meet their closed forms",
    ),
    (
        "B11",
        "Moments add up across union, intersection and difference",
    ),
    (
        "B12",
        "A ∩ B's moments meet the sample's and move with a rigid motion",
    ),
    (
        "B13",
        "Distances meet exact values, agree both ways and survive a motion",
    ),
    (
        "B14",
        "A fillet or chamfer gives the blended solid's closed forms",
    ),
    (
        "B15",
        "The kernel places sample points as the blended solid does",
    ),
    ("B16", "Blending the moved solid gives the result moved"),
    (
        "B17",
        "A result written as STEP and read back is the same solid",
    ),
    ("B18", "An input the contract does not allow is refused"),
    ("B19", "Betti numbers: pieces, tunnels and voids"),
    (
        "B20",
        "The same input gives the same result to the last bit",
    ),
    ("B21", "A plane section's area meets the suite's sample"),
    (
        "B22",
        "A STEP file the suite writes reads as the solid it describes",
    ),
    (
        "B23",
        "A mesh is closed, faces outward and holds the volume",
    ),
    (
        "B24",
        "A solid with itself: union and intersection itself, difference empty",
    ),
    (
        "B25",
        "A similarity, mirrors included, moves the solid and scales its measures",
    ),
    ("B26", "A ray's crossings meet the suite's march along it"),
    (
        "B27",
        "Interference finds exactly an assembly's clashing pairs, their volumes and gaps",
    ),
    (
        "B28",
        "A boolean of many solids meets its closed form and the booleans one at a time",
    ),
    (
        "B29",
        "A part joined to its mirror image meets its closed form",
    ),
    (
        "B30",
        "Normals meet the exact normals, and none at a crease",
    ),
    (
        "B31",
        "A fillet meets its faces smoothly, a chamfer or corner at a crease",
    ),
    (
        "B32",
        "A stretch or shear maps the solid: volume by the determinant, area within bounds",
    ),
    ("B33", "A solid's bounding box is the least box round it"),
    (
        "B34",
        "A split's pieces are valid, hold its volume, and hold each point on its side",
    ),
    (
        "B35",
        "A plane section's curves lie on the plane and the boundary, with their length",
    ),
    ("B36", "Loose faces sew back into the solid they bound"),
    ("B37", "Principal curvatures meet their closed forms"),
    (
        "B38",
        "A hidden-line drawing lies on the seen and hidden edges and covers them",
    ),
    (
        "B39",
        "By its seams a curvature-continuous blend bends as its faces do",
    ),
    ("Q1", "Sound: the kernel completes the act"),
    ("Q2", "Sound: its own checker finds the result valid"),
    (
        "Q3",
        "Sound: the result keeps its pieces, tunnels and voids",
    ),
    ("Q4", "Sound: outside the open zone the result is exact"),
    (
        "Q5",
        "Sound: the result lies between the results at the extreme sizes",
    ),
    (
        "Q6",
        "Sound: the blend meets its faces tangentially where it touches them",
    ),
    ("Q7", "Sound: a loft passes through its sections"),
    (
        "Q8",
        "Sound: by its seams the blend bends as what it meets does",
    ),
    ("Q9", "Sound: the result leaves the points it must"),
    (
        "Q10",
        "Sound: point for point the result lies between its limits",
    ),
    ("Q11", "Sound: across the blend the boundary has no crease"),
    (
        "Q12",
        "Sound: across a curvature-continuous blend the curvature carries on",
    ),
    (
        "Q13",
        "Sound: the result is unchanged by what leaves the case unchanged",
    ),
];

/// How a test came out, as the page shows it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mark {
    Pass,
    /// Within a hundred times the tolerance: right in kind, short of the precision asked.
    Short,
    /// Further off, or wrong in a way no tolerance covers.
    Wrong,
    /// Refused, timed out or crashed: no answer to judge.
    NoAnswer,
    /// Read, with nothing stated to compare: interop files without a stated measure.
    Read,
    Unsupported,
}

impl Mark {
    fn code(self) -> &'static str {
        match self {
            Mark::Pass => "p",
            Mark::Short => "s",
            Mark::Wrong => "w",
            Mark::NoAnswer => "n",
            Mark::Read => "r",
            Mark::Unsupported => "u",
        }
    }
}

/// Clauses that compare a measure with its expected value within the tolerance.
const MEASURES: &[&str] = &[
    "B0", "B2", "B3", "B4", "B5", "B6", "B8", "B10", "B11", "B12", "B13", "B14", "B16", "B17",
    "B21", "B22", "B23", "B24", "B25", "B28", "B29", "B32", "Q4", "Q5", "Q7",
];

/// What a failed check that is not a measure means, in a few words.
fn plain(clause: &str) -> &'static str {
    match clause {
        "B1" | "Q2" => "invalid by its own checker",
        "B7" => "changed its input",
        "B9" | "B15" => "places points on the wrong side",
        "B18" => "built a solid from input the spec rules out",
        "B19" | "Q3" => "wrong number of pieces, tunnels or voids",
        "B20" => "a different answer the second time",
        "B26" => "ray crossings in the wrong places",
        "B27" => "wrong clashing pairs",
        "B30" => "wrong normals",
        "B33" => "a box that is not the least round the solid",
        "B34" => "pieces that miss part of the solid or hold it on the wrong side",
        "B35" => "section curves off the plane or the boundary, or the wrong length",
        "B31" | "Q6" => "a crease where it should be smooth, or the reverse",
        "Q1" => "did not complete",
        _ => "",
    }
}

fn failed_checks(c: &Value) -> Vec<&Value> {
    c["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|ch| ch["ok"] == false)
        .collect()
}

/// How far over the tolerance, readably.
fn times(over: f64) -> String {
    if over >= 1e6 {
        "over a million times the tolerance".into()
    } else if over >= 10.0 {
        format!("{over:.0}× the tolerance")
    } else {
        format!("{over:.1}× the tolerance")
    }
}

fn first_error(c: &Value) -> String {
    c["error"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            c["ops"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|o| o["error"].as_str().map(String::from))
        })
        .unwrap_or_default()
}

/// A case's mark and its reason in a few words.
fn judge(c: &Value) -> (Mark, String) {
    let line = |s: &str| {
        s.lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(160)
            .collect::<String>()
    };
    match c["outcome"].as_str().unwrap_or("?") {
        "pass" => (Mark::Pass, String::new()),
        "unsupported" => (Mark::Unsupported, String::new()),
        o @ ("refused" | "timeout" | "crash") => {
            (Mark::NoAnswer, format!("{o}: {}", line(&first_error(c))))
        }
        _ => {
            let other: Vec<&str> = failed_checks(c)
                .iter()
                .filter_map(|ch| ch["clause"].as_str())
                .filter(|id| !MEASURES.contains(id))
                .map(plain)
                .filter(|p| !p.is_empty())
                .collect();
            if let Some(first) = other.first() {
                return (Mark::Wrong, (*first).to_string());
            }
            match c["over"].as_f64() {
                Some(over) if over <= 100.0 => (Mark::Short, format!("off by {}", times(over))),
                Some(over) => (Mark::Wrong, format!("off by {}", times(over))),
                None => (
                    Mark::Wrong,
                    "differs from the suite's sample of the expected shape".into(),
                ),
            }
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn cases(run: &Value) -> &[Value] {
    run["cases"].as_array().map_or(&[], |v| v.as_slice())
}

fn kernel(run: &Value) -> String {
    run["kernel"]["name"].as_str().unwrap_or("?").to_string()
}

fn case_ms(c: &Value) -> f64 {
    c["ops"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| o["ms"].as_f64())
        .sum()
}

/// A readable name for a test: what it builds and does.
fn label(c: &Value) -> String {
    let kind = |s: &Value| {
        s["kind"].as_str().map(String::from).unwrap_or_else(|| {
            // A built shape: its operation.
            s["op"].as_str().unwrap_or("shape").to_string()
        })
    };
    if !c["feature"].is_null() {
        let act = c["feature"]["act"]
            .as_str()
            .unwrap_or("act")
            .replace('_', " ");
        return format!("{act} on {}", kind(&c["a"]));
    }
    if let Some(schema) = c["schema"].as_str() {
        return format!("{} as {schema}", kind(&c["a"]));
    }
    format!("{} · {}", kind(&c["a"]), kind(&c["b"]))
}

/// The row a case is listed under: its test's path below its directory, marked where it is
/// judged within limits rather than against one answer.
fn category(c: &Value, test: &str) -> String {
    let name = test.split_once('/').map_or(test, |(_, rest)| rest);
    if c["sound"] == true {
        format!("{name} · sound")
    } else {
        name.to_string()
    }
}

/// One test's result for one kernel: mark, time, reason, and the lines that explain a failure.
struct Cell {
    mark: Mark,
    ms: f64,
    why: String,
    detail: Vec<String>,
}

fn cell_of(c: &Value) -> Cell {
    let (mark, why) = judge(c);
    let mut detail = Vec::new();
    if !matches!(mark, Mark::Pass | Mark::Unsupported) {
        for ch in failed_checks(c) {
            let clause = ch["clause"].as_str().unwrap_or("?");
            let promise = CLAUSES
                .iter()
                .find(|(k, _)| *k == clause)
                .map_or("", |(_, t)| t);
            detail.push(format!(
                "{clause} {} — {}",
                ch["detail"].as_str().unwrap_or(""),
                promise
            ));
        }
        let e = first_error(c);
        if !e.is_empty() {
            detail.push(e);
        }
    }
    Cell {
        mark,
        ms: case_ms(c),
        why,
        detail,
    }
}

/// One test row: its label, its id, its inputs, and one cell per kernel.
struct Test {
    label: String,
    id: String,
    inputs: String,
    /// What the viewer shows for the case: its views, from the suite's own definition of it.
    shapes: Value,
    cells: Vec<Cell>,
}

/// Counts and times of one kernel over a group of tests.
#[derive(Default)]
struct Stats {
    n: BTreeMap<Mark, usize>,
    answered_ms: Vec<f64>,
    timeouts: usize,
}

impl Stats {
    fn add(&mut self, c: &Cell) {
        *self.n.entry(c.mark).or_default() += 1;
        if matches!(c.mark, Mark::Pass | Mark::Short | Mark::Wrong | Mark::Read) {
            self.answered_ms.push(c.ms);
        }
        if c.why.starts_with("timeout") {
            self.timeouts += 1;
        }
    }
    fn get(&self, m: Mark) -> usize {
        self.n.get(&m).copied().unwrap_or(0)
    }
    fn total(&self) -> usize {
        self.n.values().sum()
    }
}

fn ms_text(ms: f64) -> String {
    if ms >= 10_000.0 {
        format!("{:.0} s", ms / 1000.0)
    } else if ms >= 1000.0 {
        format!("{:.1} s", ms / 1000.0)
    } else if ms >= 10.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{ms:.1} ms")
    }
}

/// A score as passes over tests, and its share.
fn score(pass: usize, total: usize) -> String {
    let pct = if total == 0 {
        0.0
    } else {
        100.0 * pass as f64 / total as f64
    };
    format!("{pass}/{total} <span class=\"pct\">{pct:.1}%</span>")
}

/// One kernel's cell on a directory or test row: a bar of its passes, its score over every case,
/// and its times.
fn stats_cell(s: &Stats) -> String {
    if s.total() == 0 {
        return "<div class=\"cell muted\">not run</div>".into();
    }
    let pass = s.get(Mark::Pass) + s.get(Mark::Read);
    let rest = s.total() - pass;
    // Median and slowest on the row; what they are, and any time-outs, in its title.
    let (mut time, mut title) = (
        String::new(),
        String::from("median · slowest of the answered"),
    );
    if !s.answered_ms.is_empty() {
        let mut v = s.answered_ms.clone();
        v.sort_by(f64::total_cmp);
        let _ = write!(
            time,
            "{} · {}",
            ms_text(v[v.len() / 2]),
            ms_text(v[v.len() - 1])
        );
    }
    if s.timeouts > 0 {
        let _ = write!(title, "; {} timed out", s.timeouts);
        time += " ⧗";
    }
    format!(
        "<div class=\"cell line\"><span class=\"bar\"><i class=\"pass\" style=\"flex:{pass}\"></i>\
         <i style=\"flex:{rest}\"></i></span><span class=\"n\">{}</span>\
         <span class=\"time\" title=\"{title}\">{time}</span></div>",
        score(pass, s.total())
    )
}

fn legend() -> String {
    let item = |cls: &str, text: &str| format!("<li><span class=\"tag {cls}\">{text}</span></li>");
    format!(
        "<ul class=\"legend\">{}{}{}{}{}</ul>",
        item("pass", "pass"),
        item("short", "fail · within 100× tolerance"),
        item("wrong", "fail · further off"),
        item("none", "no answer"),
        item("unsup", "unsupported"),
    )
}

/// Bytes as base64, for embedding the viewer's WebAssembly.
fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            out.push(if k <= c.len() {
                A[(n >> (18 - 6 * k) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// One kernel's counts over every case of its run.
fn stats_of(run: &Value) -> Stats {
    let mut s = Stats::default();
    for c in cases(run) {
        s.add(&cell_of(c));
    }
    s
}

fn card(run: &Value, s: &Stats) -> String {
    let k = &run["kernel"];
    format!(
        "<article class=\"card\"><h2>{}</h2><p class=\"meta\">{} · declares {} mm</p>\
         <p class=\"readout\">{}</p><p class=\"meta\">cases passed</p></article>\n",
        esc(&kernel(run)),
        esc(k["version"].as_str().unwrap_or("?")),
        k["tolerance"],
        score(s.get(Mark::Pass) + s.get(Mark::Read), s.total())
    )
}

/// The report page. With `viewer`, the suite's shapes meshed in WebAssembly, every test opens to
/// show the shape it should produce. Runs are grouped by suite version: the tests compare the runs
/// on the newest, and runs on earlier versions are listed apart with their scores.
pub fn page(runs: &[Value], viewer: Option<&[u8]>) -> String {
    let by_name = |r: &&Value| kernel(r).to_lowercase();
    let mut groups = crate::report::by_suite(runs).into_iter();
    let (_, mut runs) = groups.next().unwrap_or_default();
    runs.sort_by_key(by_name);
    let earlier: Vec<(&str, Vec<&Value>)> = groups.collect();
    let tol = runs
        .first()
        .and_then(|r| r["tolerance"].as_f64())
        .unwrap_or(1e-7);

    // Each case's views, from the cases the suite draws for the default seed: a results file
    // holds only its operands, not what the case is about.
    let views: std::collections::HashMap<String, Value> = if viewer.is_some() {
        crate::generate::cases(1)
            .iter()
            .map(|c| (c.id.clone(), crate::views::views(c)))
            .collect()
    } else {
        Default::default()
    };

    // Every case, by directory and test, with one cell per kernel.
    let mut tree: BTreeMap<(usize, u8, String), BTreeMap<String, Test>> = BTreeMap::new();
    let mut suites: Vec<&str> = crate::generate::DIRS.to_vec();
    suites.push("unsorted");
    let order = |s: &str| suites.iter().position(|x| *x == s).unwrap_or(suites.len());
    let blank = || Cell {
        mark: Mark::Unsupported,
        ms: 0.0,
        why: "not run".into(),
        detail: Vec::new(),
    };
    for (ki, r) in runs.iter().enumerate() {
        for c in cases(r) {
            let test = crate::report::test_of(c);
            let key = (order(crate::report::area(&test)), 0, category(c, &test));
            let id = c["id"].as_str().unwrap_or("?").to_string();
            let t = tree
                .entry(key)
                .or_default()
                .entry(id.clone())
                .or_insert_with(|| Test {
                    label: label(c),
                    id: id.clone(),
                    inputs: String::new(),
                    shapes: views.get(&id).cloned().unwrap_or(Value::Null),
                    cells: (0..runs.len()).map(|_| blank()).collect(),
                });
            let cell = cell_of(c);
            if !matches!(cell.mark, Mark::Pass | Mark::Unsupported) && t.inputs.is_empty() {
                let mut inputs = format!("a {}\nb {}", c["a"], c["b"]);
                if !c["feature"].is_null() {
                    let _ = write!(inputs, "\nact {}", c["feature"]);
                }
                t.inputs = inputs;
            }
            t.cells[ki] = cell;
        }
    }

    let mut h = String::new();
    h += "<title>B-rep Conformance</title>\n";
    h += "<meta name=\"claude:viewport-fit-sides\" content=\"cover\">\n";
    h += STYLE;
    h += "<main>\n<header>\n";
    let _ = write!(
        h,
        "<h1>B-rep kernel conformance</h1>\n<p class=\"lede\">Generated tests for exact B-rep \
         kernels, each answer checked against its exact value. A case passes when every measure \
         is within <b>{tol:e} mm</b> of exact, the same tolerance for every kernel. Every kernel \
         is scored on the same cases: its passes over all of them. A case it fails, refuses, does \
         not offer or runs out of time on is not a pass; the tags below say which, for finding \
         the cause. Each test is one situation, named by its path, run as many generated cases. \
         Times are the median and slowest of the cases a kernel answered.</p>\n"
    );
    h += &legend();
    if let Some(r) = runs.first() {
        let _ = writeln!(
            h,
            "<p class=\"fine\">Suite {} · contract {}. The suite and its adapters are written by the \
             authors of springrep. The other kernels' maintainers did not write these adapters \
             or choose these tests; each adapter's README lists how it drives its kernel. A \
             failure can be the adapter's: corrections are welcome.</p>",
            esc(r["suite"].as_str().unwrap_or("?")),
            esc(r["contract"].as_str().unwrap_or("?"))
        );
    }
    h += "</header>\n";

    // Kernel cards.
    let mut all: Vec<Stats> = (0..runs.len()).map(|_| Stats::default()).collect();
    for tests in tree.values() {
        for t in tests.values() {
            for (ki, c) in t.cells.iter().enumerate() {
                all[ki].add(c);
            }
        }
    }
    h += "<section class=\"cards\">\n";
    for (r, s) in runs.iter().zip(&all) {
        h += &card(r, s);
    }
    h += "</section>\n";
    if !earlier.is_empty() {
        h += "<section class=\"earlier\">\n<h2>Earlier suite versions</h2>\n<p class=\"muted\">\
              These runs were made on an earlier version of the suite, with other tests or other \
              scoring. Their scores do not compare with those above, and the tests below leave \
              them out.</p>\n";
        for (version, mut group) in earlier {
            group.sort_by_key(by_name);
            let _ = writeln!(
                h,
                "<p class=\"meta\">Suite {}</p>\n<div class=\"cards\">",
                esc(version)
            );
            for r in group {
                h += &card(r, &stats_of(r));
            }
            h += "</div>\n";
        }
        h += "</section>\n";
    }

    // The tree: directories, subdirectories, tests, and (drawn on opening) their cases.
    let cols = runs.len();
    let _ = write!(
        h,
        "<section>\n<h2>Tests</h2>\n<p class=\"muted\">Open a directory for its tests, and a test \
         for its cases; a failed case opens to say why.</p>\n<div class=\"scroll\">\
         <div class=\"tree\" style=\"--cols:{cols}\">\n<div class=\"row head\"><span></span>"
    );
    for r in &runs {
        let _ = write!(h, "<span>{}</span>", esc(&kernel(r)));
    }
    h += "</div>\n";
    let mut data_cats: Vec<Value> = Vec::new();
    let mut current: Option<usize> = None;
    let mut sub: Option<String> = None;
    let keys: Vec<&(usize, u8, String)> = tree.keys().collect();
    let dir_of = |cat: &str| cat.split_once('/').map(|(d, _)| d.to_string());
    for key in &keys {
        let (suite, _, cat) = key;
        if current != Some(*suite) {
            if sub.take().is_some() {
                h += "</details>\n";
            }
            if current.is_some() {
                h += "</details>\n";
            }
            current = Some(*suite);
            let mut st: Vec<Stats> = (0..cols).map(|_| Stats::default()).collect();
            for ((s, _, _), tests) in &tree {
                if s == suite {
                    for t in tests.values() {
                        for (ki, c) in t.cells.iter().enumerate() {
                            st[ki].add(c);
                        }
                    }
                }
            }
            let name = suites.get(*suite).copied().unwrap_or("unsorted");
            let _ = write!(
                h,
                "<details class=\"suite\"><summary class=\"row\"><span class=\"name\">{}/</span>",
                esc(name)
            );
            for s in &st {
                h += &stats_cell(s);
            }
            h += "</summary>\n";
        }
        let here = dir_of(cat);
        if here != sub {
            if sub.is_some() {
                h += "</details>\n";
            }
            if let Some(d) = &here {
                let mut st: Vec<Stats> = (0..cols).map(|_| Stats::default()).collect();
                for ((s, _, c), tests) in &tree {
                    if s == suite && dir_of(c).as_ref() == Some(d) {
                        for t in tests.values() {
                            for (ki, c) in t.cells.iter().enumerate() {
                                st[ki].add(c);
                            }
                        }
                    }
                }
                let _ = write!(
                    h,
                    "<details class=\"sub\"><summary class=\"row\"><span class=\"name\">{}/</span>",
                    esc(d)
                );
                for s in &st {
                    h += &stats_cell(s);
                }
                h += "</summary>\n";
            }
            sub = here.clone();
        }
        let shown = cat.split_once('/').map_or(cat.as_str(), |(_, rest)| rest);
        let tests = &tree[*key];
        let mut st: Vec<Stats> = (0..cols).map(|_| Stats::default()).collect();
        for t in tests.values() {
            for (ki, c) in t.cells.iter().enumerate() {
                st[ki].add(c);
            }
        }
        let _ = write!(
            h,
            "<details class=\"cat{}\" data-i=\"{}\"><summary class=\"row\"><span class=\"name\">{} \
             <span class=\"n\">{}</span></span>",
            if sub.is_some() { " deep" } else { "" },
            data_cats.len(),
            esc(shown),
            tests.len()
        );
        for s in &st {
            h += &stats_cell(s);
        }
        h += "</summary><div class=\"tests\"></div></details>\n";
        let rows: Vec<Value> = tests
            .values()
            .map(|t| {
                let cells: Vec<Value> = t
                    .cells
                    .iter()
                    .map(|c| {
                        serde_json::json!([
                            c.mark.code(),
                            (c.ms * 10.0).round() / 10.0,
                            c.why,
                            c.detail
                        ])
                    })
                    .collect();
                serde_json::json!({"l": t.label, "id": t.id, "in": t.inputs, "s": t.shapes,
                                   "c": cells})
            })
            .collect();
        data_cats.push(Value::Array(rows));
    }
    if sub.is_some() {
        h += "</details>\n";
    }
    if current.is_some() {
        h += "</details>\n";
    }
    h += "</div></div>\n</section>\n";

    h += "</main>\n";

    // The tests' data, drawn into a category when it is opened.
    let kernels: Vec<String> = runs.iter().map(|r| kernel(r)).collect();
    let data = serde_json::json!({"k": kernels, "cats": data_cats});
    let json = serde_json::to_string(&data)
        .unwrap_or_default()
        .replace("</", "<\\/");
    let _ = writeln!(
        h,
        "<script type=\"application/json\" id=\"data\">{json}</script>"
    );
    if let Some(wasm) = viewer {
        let _ = writeln!(
            h,
            "<script type=\"application/octet-stream\" id=\"oracle\">{}</script>",
            base64(wasm)
        );
        h += "<script>";
        h += THREE;
        h += "</script>\n";
        h += VIEWER;
    }
    h += SCRIPT;
    h
}

/// three.js, embedded so that a report opens without the network.
const THREE: &str = include_str!("../assets/three-r128.min.js");

const VIEWER: &str = r##"<script>
(() => {
  // The suite's shapes, meshed by its own membership test compiled to WebAssembly.
  let oracle = null;
  const load = async () => {
    if (oracle) return oracle;
    const b64 = document.getElementById("oracle").textContent.trim();
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const { instance } = await WebAssembly.instantiate(bytes, {});
    oracle = instance.exports;
    return oracle;
  };
  const meshOf = (shape, n) => {
    const text = new TextEncoder().encode(JSON.stringify(shape));
    const p = oracle.alloc(text.length);
    new Uint8Array(oracle.memory.buffer, p, text.length).set(text);
    const out = oracle.mesh_json(p, text.length, n);
    oracle.release(p, text.length);
    const head = new Uint32Array(oracle.memory.buffer, out, 4);
    if (head[0] !== 0) {
      const msg = new TextDecoder().decode(new Uint8Array(oracle.memory.buffer, out + 8, head[1]));
      return { error: msg };
    }
    const [nv, nt, ne] = [head[1], head[2], head[3]];
    const buf = oracle.memory.buffer;
    let at = out + 16;
    const take = (Kind, n) => { const v = new Kind(buf.slice(at, at + n * 4)); at += n * 4; return v; };
    const pos = take(Float32Array, nv * 3);
    const nrm = take(Float32Array, nv * 3);
    const idx = take(Uint32Array, nt * 3);
    const edges = take(Float32Array, ne * 6);
    return { pos, nrm, idx, edges };
  };
  const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const bool = (op, a, b) => ({ kind: "boolean", op, a, b });

  // A solid moved by rows [M | t]: points by M and t, normals by M's inverse transpose (the
  // cofactors, then made unit), and a mirror's triangles turned round so they still face out.
  const moved = (m, mat) => {
    if (!mat) return m;
    const point = (src) => {
      const out = new Float32Array(src.length);
      for (let i = 0; i < src.length; i += 3) {
        const [x, y, z] = [src[i], src[i + 1], src[i + 2]];
        for (let r = 0; r < 3; r++) out[i + r] = mat[r][0] * x + mat[r][1] * y + mat[r][2] * z + mat[r][3];
      }
      return out;
    };
    const a = mat;
    const cof = [
      [a[1][1] * a[2][2] - a[1][2] * a[2][1], a[1][2] * a[2][0] - a[1][0] * a[2][2], a[1][0] * a[2][1] - a[1][1] * a[2][0]],
      [a[0][2] * a[2][1] - a[0][1] * a[2][2], a[0][0] * a[2][2] - a[0][2] * a[2][0], a[0][1] * a[2][0] - a[0][0] * a[2][1]],
      [a[0][1] * a[1][2] - a[0][2] * a[1][1], a[0][2] * a[1][0] - a[0][0] * a[1][2], a[0][0] * a[1][1] - a[0][1] * a[1][0]],
    ];
    const det = a[0][0] * cof[0][0] + a[0][1] * cof[0][1] + a[0][2] * cof[0][2];
    const nrm = new Float32Array(m.nrm.length);
    for (let i = 0; i < m.nrm.length; i += 3) {
      const [x, y, z] = [m.nrm[i], m.nrm[i + 1], m.nrm[i + 2]];
      const v = [0, 1, 2].map((r) => (cof[r][0] * x + cof[r][1] * y + cof[r][2] * z) * Math.sign(det));
      const l = Math.hypot(...v) || 1;
      for (let r = 0; r < 3; r++) nrm[i + r] = v[r] / l;
    }
    let idx = m.idx;
    if (det < 0) {
      idx = new Uint32Array(m.idx.length);
      for (let i = 0; i < m.idx.length; i += 3) { idx[i] = m.idx[i]; idx[i + 1] = m.idx[i + 2]; idx[i + 2] = m.idx[i + 1]; }
    }
    return { pos: point(m.pos), nrm, idx, edges: point(m.edges) };
  };

  let ui = null;
  const build = () => {
    const root = document.createElement("div");
    root.id = "viewer";
    root.innerHTML = '<div class="panel" role="dialog" aria-modal="true">' +
      '<div class="bar2"><div class="title"></div><div class="modes"></div>' +
      '<label><input type="checkbox" class="ghost" checked> operands</label>' +
      '<button type="button" class="close">close</button></div>' +
      '<div class="stage"><canvas class="main"></canvas><div class="viewcube"><canvas class="cube" aria-label="View cube: click a face, edge or corner to look from there"></canvas>' +
      '<canvas class="axes" width="64" height="64" aria-hidden="true"></canvas></div>' +
      '<div class="note"></div><div class="status"></div>' +
      '<div class="busy" role="status" hidden><div class="ring"></div><div class="label"></div></div>' +
      '<div class="keys">drag orbit · right-drag or space-drag pan · scroll zoom · F frame · 1 front · 2 right · 3 top · 4 iso · ⇧ the opposite side</div></div></div>';
    document.body.append(root);
    const canvas = root.querySelector("canvas.main");
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(30, 1, 0.01, 1e6);
    camera.up.set(0, 0, 1);
    // The light: a sky over a dark ground, a key light and a weaker fill. Here
    // they ride with the camera, so a part turned any way stays lit.
    scene.add(new THREE.HemisphereLight(0xffffff, 0x444a50, 0.85));
    const sun = new THREE.DirectionalLight(0xffffff, 0.85);
    const fill = new THREE.DirectionalLight(0xffffff, 0.35);
    scene.add(fill, sun);
    const group = new THREE.Group();
    scene.add(group);
    // The ground: a faint grid in the plane under the part, so it stands on something.
    let ground = null;
    const view = { yaw: -0.6, pitch: 0.45, dist: 10, center: new THREE.Vector3() };
    const eyeDir = () => {
      const c = Math.cos(view.pitch);
      return new THREE.Vector3(c * Math.cos(view.yaw), c * Math.sin(view.yaw), Math.sin(view.pitch));
    };

    // The view cube: plate faces with a control rim and their
    // names, an amber face, edge or corner under the pointer, and a click on one looks from it.
    // Beside it, the axes as the camera sees them.
    const SC = { plate: "#0B0C0D", control: "#6A6C6E", ink: "#FFFFFF", page: "#000000", amber: "#F5A200" };
    const CUBE_SPAN = 1.33, CUBE_H = 0.55, CUBE_RIM = 0.62, RIM_PX = Math.round((1 - CUBE_RIM) / 2 * 128);
    const cubeCanvas = root.querySelector("canvas.cube");
    const cubeR = new THREE.WebGLRenderer({ canvas: cubeCanvas, antialias: true, alpha: true });
    cubeR.setPixelRatio(Math.min(devicePixelRatio, 2));
    const cubeScene = new THREE.Scene();
    const cubeCam = new THREE.OrthographicCamera(-CUBE_SPAN, CUBE_SPAN, CUBE_SPAN, -CUBE_SPAN, 0.1, 10);
    const cubeFace = (text, lit) => {
      const c = document.createElement("canvas");
      c.width = c.height = 128;
      const g = c.getContext("2d");
      g.fillStyle = SC.plate;
      g.fillRect(0, 0, 128, 128);
      if (lit) {
        g.fillStyle = SC.amber;
        g.fillRect(RIM_PX, RIM_PX, 128 - 2 * RIM_PX, 128 - 2 * RIM_PX);
      }
      g.strokeStyle = SC.control;
      g.lineWidth = 3;
      g.strokeRect(1.5, 1.5, 125, 125);
      g.fillStyle = lit ? SC.page : SC.ink;
      g.font = "600 24px 'Helvetica Neue', Helvetica, Arial, sans-serif";
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(text, 64, 66);
      const t = new THREE.CanvasTexture(c);
      t.anisotropy = 4;
      return t;
    };
    // BoxGeometry's faces in order +x, −x, +y, −y, +z, −z; z is up and −y the front.
    const SIDES = [["RIGHT", [1, 0, 0]], ["LEFT", [-1, 0, 0]], ["BACK", [0, 1, 0]],
      ["FRONT", [0, -1, 0]], ["TOP", [0, 0, 1]], ["BOTTOM", [0, 0, -1]]];
    const cubeMats = SIDES.map(([t]) => new THREE.MeshBasicMaterial({ map: cubeFace(t, false) }));
    const cube = new THREE.Mesh(new THREE.BoxGeometry(2 * CUBE_H, 2 * CUBE_H, 2 * CUBE_H), cubeMats);
    cubeScene.add(cube);
    // An edge or corner under the pointer lights as a block standing just proud of the faces.
    const cubeSpot = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial({ color: SC.amber }));
    cubeSpot.visible = false;
    cubeScene.add(cubeSpot);
    // The direction a point on the cube looks from: one axis for a face's middle, two for an
    // edge, three for a corner.
    const cubeDirAt = (point) => {
      const v = point.clone().divideScalar(CUBE_H).toArray().map((c) => (Math.abs(c) > CUBE_RIM ? Math.sign(c) : 0));
      return v.some((c) => c) ? v : null;
    };
    let cubeLit = -1, cubeLitKey = "";
    const lightCube = (dir) => {
      const key = dir ? dir.join() : "";
      if (key === cubeLitKey) return;
      cubeLitKey = key;
      const axes = dir ? dir.filter((c) => c).length : 0;
      const i = axes === 1 ? SIDES.findIndex(([, d]) => d.join() === key) : -1;
      if (i !== cubeLit) {
        if (cubeLit >= 0) { cubeMats[cubeLit].map.dispose(); cubeMats[cubeLit].map = cubeFace(SIDES[cubeLit][0], false); }
        if (i >= 0) { cubeMats[i].map.dispose(); cubeMats[i].map = cubeFace(SIDES[i][0], true); }
        cubeLit = i;
      }
      cubeSpot.visible = axes > 1;
      if (axes > 1) {
        const proud = 0.012, inner = CUBE_RIM * CUBE_H, outer = CUBE_H + proud, rim = outer - inner;
        cubeSpot.scale.set(...dir.map((k) => (k ? rim : 2 * inner)));
        cubeSpot.position.set(...dir.map((k) => k * (inner + rim / 2)));
      }
      draw();
    };
    const cubeAt = (e) => {
      const r = cubeCanvas.getBoundingClientRect();
      const p = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
      const ray = new THREE.Raycaster();
      ray.setFromCamera(p, cubeCam);
      const hit = ray.intersectObject(cube)[0];
      return hit ? cubeDirAt(hit.point) : null;
    };
    const axesCanvas = root.querySelector("canvas.axes");
    // The X, Y and Z, turned as the camera sees them: a short line each from the middle.
    const drawAxes = () => {
      const g = axesCanvas.getContext("2d");
      g.clearRect(0, 0, 64, 64);
      const inv = camera.quaternion.clone().invert();
      const axes = [[1, 0, 0, "#e5484d", "x"], [0, 1, 0, "#46a758", "y"], [0, 0, 1, "#3e7bfa", "z"]]
        .map(([x, y, z, col, t]) => ({ v: new THREE.Vector3(x, y, z).applyQuaternion(inv), col, t }))
        .sort((a, b) => a.v.z - b.v.z);
      g.lineWidth = 2.5;
      g.font = "600 13px 'Helvetica Neue', Helvetica, Arial, sans-serif";
      g.textAlign = "center";
      g.textBaseline = "middle";
      for (const { v, col, t } of axes) {
        g.strokeStyle = g.fillStyle = col;
        g.beginPath(); g.moveTo(32, 32); g.lineTo(32 + v.x * 20, 32 - v.y * 20); g.stroke();
        g.fillText(t, 32 + v.x * 28, 32 - v.y * 28);
      }
    };

    const draw = () => {
      const w = canvas.clientWidth, h = canvas.clientHeight;
      if (canvas.width !== Math.round(w * renderer.getPixelRatio())) renderer.setSize(w, h, false);
      camera.aspect = w / Math.max(h, 1);
      // Depth precision follows the ratio of far to near: kept to 10⁴ round the view's distance,
      // not the 10⁸ of fixed planes, so an edge and the faces it lies on keep their order.
      camera.near = view.dist / 100;
      camera.far = view.dist * 100;
      camera.updateProjectionMatrix();
      const d = eyeDir();
      const eye = d.clone().multiplyScalar(view.dist).add(view.center);
      camera.position.copy(eye);
      camera.lookAt(view.center);
      const side = new THREE.Vector3().crossVectors(d, camera.up).normalize();
      // The key light well off the line of sight, so a face square to it shows no glare.
      const screenUp = new THREE.Vector3().crossVectors(side, d).normalize();
      sun.position.copy(eye).add(side.clone().multiplyScalar(view.dist * 1.2)).add(screenUp.multiplyScalar(view.dist * 0.9));
      fill.position.copy(view.center).sub(d.clone().multiplyScalar(view.dist)).sub(side.multiplyScalar(view.dist * 0.5));
      scene.background = new THREE.Color(css("--bg") || "#12171c");
      renderer.render(scene, camera);
      const cw = cubeCanvas.clientWidth;
      if (cubeCanvas.width !== Math.round(cw * cubeR.getPixelRatio())) cubeR.setSize(cw, cw, false);
      cubeCam.position.copy(d.clone().multiplyScalar(4));
      cubeCam.up.copy(camera.up);
      cubeCam.lookAt(0, 0, 0);
      cubeCam.updateMatrixWorld();
      cubeR.render(cubeScene, cubeCam);
      drawAxes();
    };
    const look = (d) => {
      const v = new THREE.Vector3(...d).normalize();
      view.pitch = Math.max(-1.5703, Math.min(1.5703, Math.asin(v.z)));
      // Straight down or up, the view squares to the axes: +y up the screen from the top.
      view.yaw = Math.abs(v.z) < 0.9999 ? Math.atan2(v.y, v.x) : -Math.PI / 2;
      draw();
    };
    const frame = () => {
      const box = new THREE.Box3().setFromObject(group);
      if (box.isEmpty()) return;
      box.getCenter(view.center);
      view.dist = box.getSize(new THREE.Vector3()).length() * 1.9;
      draw();
    };
    // The grid: square, a round number of cells across, a little wider than the part, under it.
    const setGround = () => {
      if (ground) { scene.remove(ground); ground = null; }
      const box = new THREE.Box3().setFromObject(group);
      if (box.isEmpty()) return;
      const size = box.getSize(new THREE.Vector3());
      const span = Math.max(size.x, size.y, size.z) * 2.2;
      const step = Math.pow(10, Math.floor(Math.log10(span / 10)));
      const cells = Math.max(4, Math.ceil(span / step / 2) * 2);
      const line = css("--line") || "#2a343e";
      ground = new THREE.GridHelper(cells * step, cells, line, line);
      ground.rotation.x = Math.PI / 2;
      const c = box.getCenter(new THREE.Vector3());
      ground.position.set(c.x, c.y, box.min.z - 1e-3 * span);
      ground.material.transparent = true;
      ground.material.opacity = 0.7;
      scene.add(ground);
    };

    // The camera: a drag orbits; a right-drag, or a drag with space held, pans; the wheel
    // zooms.
    let drag = null, spaceHeld = false;
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    canvas.addEventListener("pointerdown", (e) => {
      drag = { x: e.clientX, y: e.clientY, pan: e.button !== 0 || spaceHeld || e.shiftKey };
      canvas.setPointerCapture(e.pointerId);
    });
    canvas.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      drag.x = e.clientX;
      drag.y = e.clientY;
      if (drag.pan) {
        // Move the target with the pointer, at the target's depth.
        const per = 2 * view.dist * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) / canvas.clientHeight;
        const d = eyeDir();
        const right = new THREE.Vector3().crossVectors(d, new THREE.Vector3(0, 0, 1)).normalize().negate();
        const up = new THREE.Vector3().crossVectors(right, d).normalize().negate();
        view.center.add(right.multiplyScalar(-dx * per)).add(up.multiplyScalar(dy * per));
      } else {
        view.yaw -= dx * 0.008;
        view.pitch = Math.max(-1.5703, Math.min(1.5703, view.pitch + dy * 0.008));
      }
      draw();
    });
    canvas.addEventListener("pointerup", () => { drag = null; });
    canvas.addEventListener("wheel", (e) => { e.preventDefault(); view.dist *= Math.exp(e.deltaY * 0.001); draw(); }, { passive: false });
    cubeCanvas.addEventListener("pointermove", (e) => lightCube(cubeAt(e)));
    cubeCanvas.addEventListener("pointerleave", () => lightCube(null));
    cubeCanvas.addEventListener("click", (e) => {
      const d = cubeAt(e);
      if (d) look(d);
    });
    addEventListener("resize", () => { if (root.classList.contains("on")) draw(); });
    const open = () => root.style.display !== "none" && root.classList.contains("on");
    const close = () => {
      root.classList.remove("on");
      setTimeout(() => { root.style.display = "none"; }, 120);
    };
    root.querySelector(".close").addEventListener("click", close);
    root.addEventListener("click", (e) => { if (e.target === root) close(); });
    // The keys: F frames, 1 front, 2 right, 3 top, 4 iso, shift for the opposite side.
    const VIEWS = { Digit1: [0, -1, 0], Digit2: [1, 0, 0], Digit3: [0, 0, 1], Digit4: [1, -1, 1] };
    addEventListener("keydown", (e) => {
      if (!open() || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "Escape") { close(); return; }
      if (e.code === "Space") { spaceHeld = true; e.preventDefault(); return; }
      if (e.key === "f" || e.key === "F") { frame(); return; }
      const d = VIEWS[e.code];
      if (d) look(e.shiftKey && e.code !== "Digit4" ? d.map((x) => -x) : d);
    });
    addEventListener("keyup", (e) => { if (e.code === "Space") spaceHeld = false; });
    const busyBox = root.querySelector(".busy");
    // Meshing blocks the page, so the spinner turns by a CSS animation, which the browser runs
    // on without it; `busy` clears the stage and returns once a frame shows it.
    const busy = (label) => new Promise((done) => {
      group.clear();
      draw();
      busyBox.querySelector(".label").textContent = label;
      busyBox.hidden = false;
      requestAnimationFrame(() => setTimeout(done, 0));
    });
    const idle = () => { busyBox.hidden = true; };
    ui = { root, group, view, draw, frame, look, setGround, status: root.querySelector(".status"),
      note: root.querySelector(".note"), busy, idle };
  };

  // A solid: its faces shaded by their surfaces' normals,
  // and its edges, where faces meet at a crease, as black lines over them. A ghost is faint,
  // and pushed back so it never fights the subject for depth where they share a face: by a few
  // depth steps, not by its slope, which seen edge-on pushed it behind the subject's far side.
  const solid = (m, color, opacity, flush) => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(m.pos, 3));
    g.setAttribute("normal", new THREE.BufferAttribute(m.nrm, 3));
    g.setIndex(new THREE.BufferAttribute(m.idx, 1));
    const ghost = opacity < 1;
    const mat = new THREE.MeshStandardMaterial({
      color, roughness: 0.5, metalness: 0.2,
      // A solid's triangles all face out, so its back faces never show: drawing them let a far
      // face's lit back peek through a hairline gap or past a face seen edge-on. A ghost is an
      // open shell, seen from either side.
      transparent: ghost, opacity, depthWrite: !ghost, side: ghost ? THREE.DoubleSide : THREE.FrontSide,
      // Faces sit a few depth steps back, so an edge drawn along them wins even where the next
      // face's chords bulge a hair past it. A flush ghost never lies on the subject, so it is
      // drawn a step forward to win where it leaves the subject's face at a tangent.
      polygonOffset: true, polygonOffsetFactor: flush ? 0 : 1,
      polygonOffsetUnits: flush ? -2 : ghost ? 10 : 4,
    });
    const part = new THREE.Group();
    part.add(new THREE.Mesh(g, mat));
    if (m.edges.length) {
      const e = new THREE.BufferGeometry();
      e.setAttribute("position", new THREE.BufferAttribute(m.edges, 3));
      const lineMat = new THREE.LineBasicMaterial({
        color: 0x000000, transparent: true, opacity: ghost ? 0.12 : 0.5 });
      // Each face is meshed on its own, so where two meet, one's chords can stand a hair in front
      // of the other's border and hide pixels of the edge along it. The edge is drawn a
      // thousandth of the way toward the eye, which moves no pixel and clears any chord.
      lineMat.onBeforeCompile = (sh) => {
        sh.vertexShader = sh.vertexShader.replace("#include <project_vertex>",
          "#include <project_vertex>\n  mvPosition.xyz *= 0.999;\n  gl_Position = projectionMatrix * mvPosition;");
      };
      part.add(new THREE.LineSegments(e, lineMat));
    }
    return part;
  };

  window.showShape = async (t) => {
    if (!ui) build();
    ui.root.style.display = "";
    requestAnimationFrame(() => ui.root.classList.add("on"));
    ui.root.querySelector(".title").innerHTML = "";
    const title = ui.root.querySelector(".title");
    title.append(t.l);
    const id = document.createElement("span");
    id.className = "id";
    id.textContent = t.id;
    title.append(id);
    const spec = t.s;
    const modes = spec.views;
    const bar = ui.root.querySelector(".modes");
    bar.innerHTML = "";
    const ghostBox = ui.root.querySelector(".ghost");
    const anyGhost = modes.some((v) => v.solids.some((s) => s.role.startsWith("ghost")));
    ghostBox.parentElement.style.display = anyGhost ? "" : "none";
    // The last test's solid goes at once, not when this one's is ready.
    ui.status.textContent = "";
    await ui.busy("loading the mesher…");
    await load();
    const cache = new Map();
    const get = (shape) => {
      const k = JSON.stringify(shape);
      if (!cache.has(k)) cache.set(k, meshOf(shape, spec.cell || 0));
      return cache.get(k);
    };
    let fitted = false;
    const tint = {
      // Plastic tones for the subjects; the ghosts in the page's accents.
      main: ["#7d93a4", 1], alt: ["#b8864f", 1],
      ghost: [css("--accent") || "#7fb6dc", 0.14], ghost2: [css("--pass") || "#5fb486", 0.14],
    };
    const show = (i, first) => {
      [...bar.children].forEach((b, j) => b.setAttribute("aria-pressed", String(i === j)));
      ui.note.textContent = modes[i].note || "";
      ui.status.textContent = "";
      ui.busy(`meshing ${modes[i].name}…`).then(() => {
        const t0 = performance.now();
        let nt = 0, error = null;
        for (const s of modes[i].solids) {
          if (s.role.startsWith("ghost") && !ghostBox.checked) continue;
          // A shape by its index in the table, or `[op, a, b]` of two such, nested once.
          const ref = (r) => typeof r === "number" ? spec.shapes[r]
            : { kind: "boolean", op: r[0], a: ref(r[1]), b: ref(r[2]) };
          const m0 = get(ref(s.s));
          if (m0.error) { error = m0.error; continue; }
          const m = moved(m0, s.matrix);
          if (!s.role.startsWith("ghost")) nt += m.idx.length / 3;
          const [color, opacity] = tint[s.role] || tint.main;
          ui.group.add(solid(m, color, opacity, !!s.flush));
        }
        // A view whose subject comes out empty, when the test first opens: show the next.
        if (first && nt === 0 && i + 1 < modes.length) { show(i + 1, true); return; }
        if (spec.points) {
          // The points that name the act's edges or faces, as small marks.
          const r = (spec.cell || 0.05) * 2.5;
          const g = new THREE.SphereGeometry(r, 12, 8);
          const m = new THREE.MeshBasicMaterial({ color: css("--accent") || "#7fb6dc", depthTest: false });
          for (const p of spec.points) {
            const dot = new THREE.Mesh(g, m);
            dot.position.set(p[0], p[1], p[2]);
            dot.renderOrder = 2;
            ui.group.add(dot);
          }
        }
        if (spec.box) {
          const [lo, hi] = spec.box;
          const g = new THREE.BoxGeometry(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
          const lines = new THREE.LineSegments(new THREE.EdgesGeometry(g),
            new THREE.LineBasicMaterial({ color: css("--accent") || "#7fb6dc" }));
          lines.position.set((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2);
          ui.group.add(lines);
        }
        if (spec.lines) {
          // A hidden-line drawing over the solid: seen stretches solid, hidden ones dashed, all
          // drawn through it. Transparent, so they draw after the solid's own edges.
          const dash = (spec.cell || 0.05) * 6;
          for (const l of spec.lines) {
            const g = new THREE.BufferGeometry().setFromPoints(l.pts.map((p) => new THREE.Vector3(...p)));
            const line = new THREE.Line(g, l.seen
              ? new THREE.LineBasicMaterial({ color: css("--accent") || "#7fb6dc", depthTest: false,
                transparent: true })
              : new THREE.LineDashedMaterial({ color: css("--short") || "#d9b25f", depthTest: false,
                transparent: true, dashSize: dash, gapSize: dash }));
            if (!l.seen) line.computeLineDistances();
            line.renderOrder = 3;
            ui.group.add(line);
          }
        }
        if (!fitted) {
          const box = new THREE.Box3().setFromObject(ui.group);
          if (!box.isEmpty()) {
            box.getCenter(ui.view.center);
            ui.view.dist = box.getSize(new THREE.Vector3()).length() * 1.9;
            ui.setGround();
            if (spec.look) ui.look(spec.look);
            fitted = true;
          }
        }
        if (spec.plane) {
          // The plane, as a square through its point as wide as the view, faint.
          const { origin, normal } = spec.plane;
          const size = ui.view.dist * 0.6;
          const g = new THREE.PlaneGeometry(size, size);
          const mesh = new THREE.Mesh(g, new THREE.MeshBasicMaterial({
            color: css("--accent") || "#7fb6dc", transparent: true, opacity: 0.12,
            side: THREE.DoubleSide, depthWrite: false }));
          mesh.position.set(...origin);
          mesh.lookAt(origin[0] + normal[0], origin[1] + normal[1], origin[2] + normal[2]);
          ui.group.add(mesh);
        }
        ui.status.textContent = error ? error
          : nt === 0 ? "empty" : `${nt.toLocaleString()} triangles · ${(performance.now() - t0).toFixed(0)} ms`;
        ui.idle();
        ui.draw();
      });
    };
    modes.forEach(({ name }, i) => {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = name;
      b.addEventListener("click", () => show(i, false));
      bar.append(b);
    });
    ghostBox.onchange = () => show([...bar.children].findIndex((b) => b.getAttribute("aria-pressed") === "true"), false);
    show(0, true);
  };
})();
</script>
"##;

const SCRIPT: &str = r#"<script>
(() => {
  const data = JSON.parse(document.getElementById("data").textContent);
  const word = { p: "pass", s: "fail", w: "fail", r: "read", u: "unsupported" };
  const cls = { p: "pass", s: "short", w: "wrong", n: "none", r: "read", u: "unsup" };
  const time = (ms) => ms >= 10000 ? (ms / 1000).toFixed(0) + " s"
    : ms >= 1000 ? (ms / 1000).toFixed(1) + " s"
    : ms >= 10 ? ms.toFixed(0) + " ms" : ms.toFixed(1) + " ms";
  const el = (tag, cl, text) => {
    const e = document.createElement(tag);
    if (cl) e.className = cl;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const cell = ([m, ms, why]) => {
    const d = el("div", "cell");
    const label = m === "n" ? why.split(":")[0] : word[m];
    const tag = el("span", "tag " + cls[m], label);
    if (why) tag.title = why;
    d.append(tag);
    if (m !== "u" && m !== "n") d.append(el("span", "time", time(ms)));
    return d;
  };
  const draw = (box, rows) => {
    for (const t of rows) {
      const failing = t.c.some(([m]) => m !== "p" && m !== "u");
      const head = el(failing ? "summary" : "div", "row test");
      const name = el("span", "name");
      name.append(el("span", "", t.l), el("span", "id", t.id));
      if (window.showShape && t.s) {
        const v = el("button", "view", "view");
        v.type = "button";
        v.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); window.showShape(t); });
        name.append(v);
      }
      head.append(name, ...t.c.map(cell));
      if (!failing) { box.append(head); continue; }
      const det = el("details", "test");
      const body = el("div", "why");
      t.c.forEach(([m, , why, lines], i) => {
        if (m === "p" || m === "u") return;
        const k = el("div", "kwhy");
        k.append(el("b", "", data.k[i] + ": "), el("span", "", why));
        if (lines && lines.length) {
          const ul = el("ul");
          for (const l of lines) ul.append(el("li", "", l));
          k.append(ul);
        }
        body.append(k);
      });
      if (t.in) body.append(el("pre", "", t.in));
      det.append(head, body);
      box.append(det);
    }
  };
  for (const d of document.querySelectorAll("details.cat")) {
    d.addEventListener("toggle", () => {
      const box = d.querySelector(".tests");
      if (d.open && !box.childElementCount) draw(box, data.cats[+d.dataset.i]);
    });
  }
})();
</script>
"#;

const STYLE: &str = r#"<style>
:root {
  --bg: #f6f7f8; --surface: #ffffff; --fg: #1d242b; --muted: #61707e; --line: #dde2e7;
  --accent: #2b5d82;
  --pass: #3c8a5f; --short: #d7a64a; --wrong: #b4523f; --none: #8b93a6; --read: #7aa5c4; --unsup: #e3e7eb;
  --display: "Helvetica Neue", Helvetica, Arial, "Liberation Sans", sans-serif;
  --sans: "Helvetica Neue", Helvetica, Arial, "Liberation Sans", sans-serif;
  --mono: ui-monospace, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #12171c; --surface: #192028; --fg: #e4e9ee; --muted: #93a1ae; --line: #2a343e;
  --accent: #7fb6dc;
  --pass: #5fb486; --short: #d9b25f; --wrong: #d47462; --none: #7d8597; --read: #6f9cbc; --unsup: #2c353f;
  color-scheme: dark; } }
:root[data-theme="dark"] {
  --bg: #12171c; --surface: #192028; --fg: #e4e9ee; --muted: #93a1ae; --line: #2a343e;
  --accent: #7fb6dc;
  --pass: #5fb486; --short: #d9b25f; --wrong: #d47462; --none: #7d8597; --read: #6f9cbc; --unsup: #2c353f;
  color-scheme: dark; }
body { background: var(--bg); color: var(--fg); font: 400 13px/1.38 var(--sans); letter-spacing: 0.005em; }
main { max-width: 100rem; margin: 0 auto; padding-block: 48px 96px;
  padding-left: max(16px, env(safe-area-inset-left, 0px));
  padding-right: max(16px, env(safe-area-inset-right, 0px));
  display: grid; gap: 48px; }
h1 { font: 700 56px/0.88 var(--display); letter-spacing: -0.025em; text-transform: uppercase; margin: 0; text-wrap: balance; }
h2 { font: 700 24px/1.05 var(--display); letter-spacing: -0.025em; text-transform: uppercase; margin: 0 0 8px; }
header { display: grid; gap: 16px; }
p { margin: 0; }
.lede { max-width: 64ch; font-size: 14px; font-weight: 300; }
.lede b { font-weight: 400; }
.fine { font-size: 13px; color: var(--muted); max-width: 76ch; }
.muted { color: var(--muted); }
.mono { font: 400 11px/1.2 var(--mono); letter-spacing: 0.02em; }
.n, .readout, .time, .id, pre { font-family: var(--mono); font-variant-numeric: tabular-nums; letter-spacing: 0.02em; }
.n { font-size: 12px; }
.pct { font-size: 10px; color: var(--muted); }
.legend { list-style: none; padding: 0; margin: 0; display: flex; flex-wrap: wrap; gap: 8px 16px; }
.legend li { display: flex; align-items: center; }
.bar i { display: block; }
i.pass { background: var(--pass); }
.cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr)); gap: 16px; }
.card { background: var(--surface); border: 1px solid var(--line); padding: 16px; display: grid; gap: 8px; min-width: 0; align-content: start; }
.card h2 { margin: 0; }
.earlier { display: grid; gap: 8px; }
.earlier .cards { margin-bottom: 8px; }
.meta { font: 600 9px/1.25 var(--display); letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); overflow-wrap: anywhere; }
.readout { font-size: 22px; line-height: 1.2; white-space: nowrap; }
.readout .pct { font-size: 11px; }
.scroll { overflow-x: auto; }
.tree { display: grid; min-width: calc(14rem + var(--cols) * 17rem); }
.row { display: grid; grid-template-columns: 24rem repeat(var(--cols), minmax(17rem, 1fr)); gap: 16px; align-items: center; padding: 6px 8px; border-bottom: 1px solid var(--line); }
.head { font: 600 10px/1.25 var(--display); text-transform: uppercase; letter-spacing: 0.18em; color: var(--muted); }
summary.row { cursor: pointer; list-style: none; }
summary.row::-webkit-details-marker { display: none; }
summary:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
.name { min-width: 0; overflow-wrap: anywhere; }
details.suite > summary .name { font: 600 12px/1.4 var(--mono); letter-spacing: 0.02em; white-space: nowrap; }
details.sub > summary { background: var(--surface); }
details.sub > summary .name { padding-left: 16px; font: 400 12px/1.4 var(--mono); white-space: nowrap; }
details > summary .name::before { content: "▸ "; color: var(--muted); font-size: 14px; }
details[open] > summary .name::before { content: "▾ "; }
details.cat > summary { background: var(--surface); }
details.cat > summary .name { padding-left: 16px; font-size: 14px; font-weight: 300; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
details.cat.deep > summary .name { padding-left: 32px; }
details.cat > summary .name .n { font-size: 11px; color: var(--muted); }
.tests { background: var(--bg); }
.row.test { padding-block: 4px; }
.row.test .name { padding-left: 32px; display: grid; }
details.cat.deep .row.test .name { padding-left: 48px; }
div.row.test .name::before { content: none; }
.row.test .id { font-size: 11px; color: var(--muted); }
.cell { display: grid; gap: 4px; min-width: 0; }
.cell.line { display: flex; align-items: baseline; gap: 8px; white-space: nowrap; overflow: hidden; }
.row.test .cell { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.bar { display: flex; flex: none; width: 2.5rem; height: 6px; overflow: hidden; background: var(--unsup); align-self: center; }
.cell .n { color: var(--fg); }
.time { font-size: 10px; color: var(--muted); }
.tag { font: 600 9px/1.25 var(--display); letter-spacing: 0.12em; text-transform: uppercase; padding: 2px 6px; border: 1px solid; white-space: nowrap; }
.tag.pass { color: var(--pass); } .tag.short { color: var(--short); } .tag.wrong { color: var(--wrong); }
.tag.none { color: var(--none); } .tag.read { color: var(--read); } .tag.unsup { color: var(--muted); border-color: var(--line); }
.why { padding: 8px 8px 12px 40px; display: grid; gap: 8px; border-bottom: 1px solid var(--line); }
.kwhy b { font-weight: 400; }
.kwhy ul { margin: 4px 0 0; padding-left: 16px; color: var(--muted); }
.kwhy li { overflow-wrap: anywhere; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; color: var(--muted); font-size: 11px; line-height: 1.45; }
table { border-collapse: collapse; width: 100%; }
button.view { justify-self: start; font: 600 9px/1.25 var(--display); letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); background: none; border: 1px solid var(--line); padding: 2px 6px; margin-top: 2px; cursor: pointer; }
button.view:hover, button.view:focus-visible { color: var(--accent); border-color: var(--accent); outline: none; }
#viewer { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; padding: 24px max(16px, env(safe-area-inset-right, 0px)) 24px max(16px, env(safe-area-inset-left, 0px)); background: color-mix(in srgb, var(--bg) 72%, transparent); opacity: 0; transition: opacity 120ms linear; }
#viewer .panel { width: min(1100px, 100%); height: min(780px, 100%); display: grid; grid-template-rows: auto 1fr; background: var(--bg); border: 1px solid var(--line); min-height: 0; }
#viewer.on { opacity: 1; }
#viewer .bar2 { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; padding: 12px 16px; border-bottom: 1px solid var(--line); }
#viewer .title { font-size: 14px; font-weight: 300; min-width: 0; overflow-wrap: anywhere; }
#viewer .title .id { display: block; }
#viewer .modes { display: flex; gap: 0; margin-left: auto; }
#viewer .modes button, #viewer .close, #viewer label { font: 600 9px/1.25 var(--display); letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); background: none; border: 1px solid var(--line); padding: 6px 10px; cursor: pointer; }
#viewer .modes button { position: relative; }
#viewer .modes button + button { margin-left: -1px; }
#viewer .modes button[aria-pressed="true"] { color: var(--accent); border-color: var(--accent); z-index: 1; }
#viewer label { display: flex; align-items: center; gap: 6px; }
#viewer canvas.main { width: 100%; height: 100%; display: block; touch-action: none; }
#viewer .viewcube { position: absolute; right: 8px; top: 8px; width: 116px; height: 116px; }
#viewer canvas.cube { width: 116px; height: 116px; display: block; cursor: pointer; }
#viewer canvas.axes { position: absolute; top: 2px; right: 2px; width: 32px; height: 32px; pointer-events: none; }
#viewer .keys { position: absolute; right: 16px; bottom: 16px; font: 400 10px/1.2 var(--mono); color: var(--muted); text-align: right; max-width: 60%; }
#viewer .stage { position: relative; min-height: 0; }
#viewer .status { position: absolute; left: 16px; bottom: 16px; font: 400 11px/1.2 var(--mono); color: var(--muted); }
#viewer .busy { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: 12px; pointer-events: none; }
#viewer .busy[hidden] { display: none; }
#viewer .busy .ring { width: 36px; height: 36px; border-radius: 50%; border: 3px solid var(--line); border-top-color: var(--accent); animation: busy-spin 0.8s linear infinite; }
#viewer .busy .label { font: 400 12px/1.2 var(--mono); color: var(--fg); background: var(--surface); border: 1px solid var(--line); padding: 5px 9px; border-radius: 4px; }
@keyframes busy-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { #viewer .busy .ring { animation-duration: 2.4s; } }
#viewer .note { position: absolute; left: 16px; top: 12px; max-width: 56ch; font-size: 13px; color: var(--muted); pointer-events: none; }
th { font: 600 9px/1.25 var(--display); letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--line); }
</style>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_tests_compare_only_runs_on_the_newest_suite_version() {
        let run = |suite: &str, kernel: &str| json!({"suite": suite, "contract": "1.0.0", "kernel": {"name": kernel}, "cases": []});
        let runs = [
            run("1.0.0", "old"),
            run("1.1.0", "zeta"),
            run("1.1.0", "alpha"),
        ];
        let page = page(&runs, None);
        assert!(page.contains(r#""k":["alpha","zeta"]"#));
        assert!(page.contains("Suite 1.1.0 · contract"));
        let earlier = page.split("Earlier suite versions").nth(1).unwrap();
        let earlier = earlier.split("</section>").next().unwrap();
        assert!(earlier.contains("Suite 1.0.0") && earlier.contains("<h2>old</h2>"));
        assert!(!earlier.contains("alpha"));
    }

    #[test]
    fn runs_on_one_suite_version_list_no_earlier_ones() {
        let run = json!({"suite": "1.0.0", "kernel": {"name": "a"}, "cases": []});
        assert!(!page(&[run], None).contains("Earlier suite versions"));
    }
}
