//! The kernels the suite scores officially: `kernels.json` at the repository's root, compiled in, so
//! an official run's adapters are the ones the suite's commit names. Each entry pins its adapter to
//! the commit the suite's maintainers reviewed as pass-through, says how to build and start it, and
//! names the kernel release the adapter must report.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

pub const KERNELS: &str = include_str!("../../kernels.json");

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The name the adapter's `hello` gives, which also names the kernel's reports.
    pub kernel: String,
    /// The release the adapter's `hello` must give.
    pub version: String,
    pub repo: String,
    pub commit: String,
    /// Shell commands run in the adapter's checkout, in order.
    pub build: Vec<String>,
    /// The command that starts the adapter, with `{dir}` for its checkout.
    pub run: String,
    /// The kernel's own maintainers who review its adapter.
    pub reviewers: Vec<String>,
}

/// The compiled-in registry.
pub fn entries() -> Vec<Entry> {
    parse(KERNELS).expect("kernels.json")
}

pub fn find(kernel: &str) -> Result<Entry, String> {
    let all = entries();
    let names: Vec<_> = all.iter().map(|e| e.kernel.clone()).collect();
    all.into_iter().find(|e| e.kernel == kernel).ok_or_else(|| {
        format!(
            "no kernel {kernel} in the registry; it lists {}",
            names.join(", ")
        )
    })
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = root["kernels"].as_array().ok_or("no `kernels` list")?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (i, k) in list.iter().enumerate() {
        let at = format!("kernels[{i}]");
        let a = &k["adapter"];
        let kernel = text_of(k, "kernel", &at)?;
        if !kernel
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(format!(
                "{at}: kernel `{kernel}` is not lower-case letters, digits and -"
            ));
        }
        if !seen.insert(kernel.clone()) {
            return Err(format!("{at}: kernel `{kernel}` is listed twice"));
        }
        let commit = text_of(a, "commit", &at)?;
        if commit.len() != 40
            || !commit
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(format!("{at}: commit `{commit}` is not a full commit hash"));
        }
        for field in ["license", "source"] {
            if !(k[field].is_string() || field == "source" && k[field].is_null()) {
                return Err(format!("{at}: no `{field}`"));
            }
        }
        if !k["maintained_by_suite"].is_boolean() || !a["license"].is_string() {
            return Err(format!(
                "{at}: no `maintained_by_suite` or adapter `license`"
            ));
        }
        out.push(Entry {
            version: text_of(k, "version", &at)?,
            repo: text_of(a, "repo", &at)?,
            build: list_of(a, "build", &at)?,
            run: text_of(a, "run", &at)?,
            reviewers: list_of(k, "reviewers", &at)?,
            kernel,
            commit,
        });
    }
    Ok(out)
}

fn text_of(v: &Value, field: &str, at: &str) -> Result<String, String> {
    match v[field].as_str() {
        Some(s) if !s.trim().is_empty() => Ok(s.to_string()),
        _ => Err(format!("{at}: no `{field}`")),
    }
}

fn list_of(v: &Value, field: &str, at: &str) -> Result<Vec<String>, String> {
    v[field]
        .as_array()
        .and_then(|l| l.iter().map(|s| s.as_str().map(String::from)).collect())
        .ok_or_else(|| format!("{at}: `{field}` is not a list of strings"))
}

impl Entry {
    /// Checks the adapter out under `work` at its pinned commit and builds it, then gives the
    /// command that starts it. A checkout with changes to tracked files is refused, as is one from
    /// another repository: the run would not be of the reviewed adapter.
    pub fn prepare(&self, work: &Path) -> Result<Vec<String>, String> {
        std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
        let work = work
            .canonicalize()
            .map_err(|e| format!("{}: {e}", work.display()))?;
        let dir = work.join(&self.kernel);
        let text = dir
            .to_str()
            .ok_or("the work directory's path is not UTF-8")?;
        if text.contains(char::is_whitespace) {
            return Err(format!("{text}: the adapter's command cannot hold a space"));
        }
        if !dir.join(".git").exists() {
            run(Command::new("git").args(["clone", "--quiet", &self.repo, text]))?;
        }
        let origin = output(&dir, &["remote", "get-url", "origin"])?;
        if origin != self.repo {
            return Err(format!(
                "{text} is a checkout of {origin}, not {}",
                self.repo
            ));
        }
        if output(&dir, &["cat-file", "-t", &self.commit]).is_err() {
            git(&dir, &["fetch", "--quiet", "origin", &self.commit])?;
        }
        git(&dir, &["checkout", "--quiet", "--detach", &self.commit])?;
        if output(&dir, &["rev-parse", "HEAD"])? != self.commit {
            return Err(format!("{text} is not at {}", self.commit));
        }
        let changed = output(&dir, &["status", "--porcelain", "--untracked-files=no"])?;
        if !changed.is_empty() {
            return Err(format!("{text} has changes to tracked files:\n{changed}"));
        }
        for step in &self.build {
            run(Command::new("sh").args(["-c", step]).current_dir(&dir))?;
        }
        Ok(self
            .run
            .replace("{dir}", text)
            .split_whitespace()
            .map(String::from)
            .collect())
    }

    /// What a results file records of the adapter that ran.
    pub fn stamp(&self) -> Value {
        json!({"repo": self.repo, "commit": self.commit})
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    run(Command::new("git").arg("-C").arg(dir).args(args))
}

fn output(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run(c: &mut Command) -> Result<(), String> {
    let shown = format!("{c:?}");
    match c.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("{shown}: {s}")),
        Err(e) => Err(format!("{shown}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(commit: &str) -> String {
        format!(
            r#"{{"kernels": [{{"kernel": "k", "version": "1", "license": "MIT", "source": null,
               "maintained_by_suite": false, "reviewers": [],
               "adapter": {{"repo": "r", "commit": "{commit}", "license": "MIT",
                            "build": ["touch built"], "run": "{{dir}}/a --flag"}}}}]}}"#
        )
    }

    #[test]
    fn the_registry_lists_each_kernel_at_a_full_commit_from_its_own_repository() {
        let all = entries();
        let names: Vec<_> = all.iter().map(|e| e.kernel.as_str()).collect();
        assert_eq!(names, ["arris", "cadrs", "opencascade", "solidcore", "springrep"]);
        for e in &all {
            assert!(e.repo.starts_with("https://github.com/"), "{}", e.repo);
            assert!(
                e.run.contains("{dir}"),
                "{} starts no program in its checkout",
                e.kernel
            );
        }
    }

    #[test]
    fn an_entry_with_a_short_commit_or_listed_twice_is_refused() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(parse(&one(full)).unwrap()[0].commit, full);
        assert!(
            parse(&one("0123456"))
                .unwrap_err()
                .contains("full commit hash")
        );
        let mut v: Value = serde_json::from_str(&one(full)).unwrap();
        let copy = v["kernels"][0].clone();
        v["kernels"].as_array_mut().unwrap().push(copy);
        assert!(parse(&v.to_string()).unwrap_err().contains("listed twice"));
    }

    fn quiet_git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-C"])
            .arg(dir)
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn an_adapter_is_built_at_its_pinned_commit_and_refused_once_its_files_change() {
        let base = std::env::temp_dir().join(format!("brep-registry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("adapter");
        std::fs::create_dir_all(&repo).unwrap();
        quiet_git(&repo, &["init", "--quiet"]);
        std::fs::write(repo.join("a"), "first").unwrap();
        quiet_git(&repo, &["add", "a"]);
        quiet_git(&repo, &["commit", "--quiet", "-m", "first"]);
        let first = output(&repo, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(repo.join("a"), "second").unwrap();
        quiet_git(&repo, &["commit", "--quiet", "-am", "second"]);

        let mut e = parse(&one(&first)).unwrap().remove(0);
        e.repo = repo.to_str().unwrap().to_string();
        let work = base.join("work");
        let command = e.prepare(&work).unwrap();
        let dir = work.canonicalize().unwrap().join("k");
        assert_eq!(command, [format!("{}/a", dir.display()), "--flag".into()]);
        assert_eq!(std::fs::read_to_string(dir.join("a")).unwrap(), "first");
        assert!(dir.join("built").exists());

        std::fs::write(dir.join("a"), "edited").unwrap();
        assert!(
            e.prepare(&work)
                .unwrap_err()
                .contains("changes to tracked files")
        );
        std::fs::remove_dir_all(&base).unwrap();
    }
}
