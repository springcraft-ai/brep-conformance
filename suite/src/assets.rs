//! Where a case's shape files go: `<test>/<case>.<hash>.glb` for its views, and a STEP file beside
//! it for each solid its first view shows, as the kernel under test made it. Each kernel's files are
//! published under `<base>/<suite>/<kernel>/<version>/`, the base compiled in from
//! `BREP_CONFORMANCE_ASSET_BASE`; a published file never changes, so it is cached immutably.

use serde_json::Value;

use crate::generate::{Case, fnv};
use crate::run::ASSET_GENERATOR;

pub const ASSET_BASE: &str = match option_env!("BREP_CONFORMANCE_ASSET_BASE") {
    Some(base) => base,
    None => "https://cdn.springcraft.ai/brep-conformance",
};

/// A view of more solids than this, such as an assembly of a hundred parts, has no STEP files: its
/// GLB shows it, and a file per part would be a hundred files for one case.
pub const MOST_STEP_FILES: usize = 2;

/// A case's files: their hash and the key of each, below its kernel's folder; `step` holds one key
/// per solid of the first view, and none for a case that must be refused or for a view of more
/// than [`MOST_STEP_FILES`] solids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assets {
    pub hash: String,
    pub glb: String,
    pub step: Vec<String>,
}

/// The case id carries its definition; [`ASSET_GENERATOR`] changes when what draws the files does.
pub fn hash(c: &Case) -> String {
    hash_versioned(&c.id, ASSET_GENERATOR)
}

fn hash_versioned(id: &str, generator: &str) -> String {
    format!("{:016x}", fnv(&format!("{id}|{generator}")))
}

/// A file's path below its kernel's folder, `format` being `glb` or `step`.
pub fn key(test: &str, id: &str, hash: &str, format: &str) -> String {
    format!("{test}/{id}.{hash}.{format}")
}

/// A published file's URL: `key` below the folder of one kernel release on one suite release.
pub fn url(suite: &str, kernel: &str, version: &str, key: &str) -> String {
    format!(
        "{}/{suite}/{kernel}/{version}/{key}",
        ASSET_BASE.trim_end_matches('/')
    )
}

/// The STEP files' keys for the first view's subjects, named by `roles` in the view's order: one
/// subject's file is `<id>.<hash>.step`, and with two each is `<id>.<hash>.<role>.step`.
pub fn step_keys(test: &str, id: &str, hash: &str, roles: &[&str]) -> Vec<String> {
    match roles {
        _ if roles.len() > MOST_STEP_FILES => Vec::new(),
        [_] => vec![key(test, id, hash, "step")],
        _ => roles
            .iter()
            .map(|r| format!("{test}/{id}.{hash}.{r}.step"))
            .collect(),
    }
}

pub fn of_case(c: &Case) -> Assets {
    let views = crate::views::views(c);
    let h = hash(c);
    let roles = if c.refuse {
        Vec::new()
    } else {
        subjects(&views)
    };
    let roles: Vec<&str> = roles.iter().map(|(_, r)| r.as_str()).collect();
    Assets {
        glb: key(c.test, &c.id, &h, "glb"),
        step: step_keys(c.test, &c.id, &h, &roles),
        hash: h,
    }
}

/// The first view's subjects, ghosts aside: each one's index in the view's solids and the name its
/// STEP file carries, its role, or `<role>-<n>` where two share a role.
pub fn subjects(views: &Value) -> Vec<(usize, String)> {
    let solids = views["views"][0]["solids"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let found: Vec<(usize, &str)> = solids
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s["role"].as_str().unwrap_or("main")))
        .filter(|(_, r)| !matches!(*r, "ghost" | "ghost2"))
        .collect();
    found
        .iter()
        .enumerate()
        .map(|(n, (i, r))| {
            let shared = found.iter().filter(|(_, q)| q == r).count() > 1;
            (
                *i,
                if shared {
                    format!("{r}-{}", n + 1)
                } else {
                    r.to_string()
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::cases;

    #[test]
    fn every_case_hashes_the_same_each_time_it_is_drawn() {
        let (a, b) = (cases(1), cases(1));
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(hash(x), hash(y), "{}", x.id);
        }
    }

    #[test]
    fn a_changed_case_or_generator_changes_the_hash() {
        let c = cases(1).into_iter().next().unwrap();
        assert_ne!(hash_versioned(&c.id, "next"), hash(&c));
        assert_ne!(
            hash_versioned(&format!("{}0", c.id), ASSET_GENERATOR),
            hash(&c)
        );
    }

    #[test]
    fn a_file_is_published_below_its_kernel_release_on_its_suite_release() {
        let k = key(
            "booleans/overlap/boxes",
            "box-box-00ff",
            "0123456789abcdef",
            "glb",
        );
        assert_eq!(
            url("1.0.0", "arris", "0.10.0", &k),
            format!(
                "{ASSET_BASE}/1.0.0/arris/0.10.0/booleans/overlap/boxes/\
                 box-box-00ff.0123456789abcdef.glb"
            )
        );
        assert_eq!(
            step_keys("t", "c", "h", &["main", "alt"]),
            ["t/c.h.main.step", "t/c.h.alt.step"]
        );
        assert_eq!(step_keys("t", "c", "h", &["main"]), ["t/c.h.step"]);
        assert!(step_keys("t", "c", "h", &["main-1", "main-2", "main-3"]).is_empty());
    }

    #[test]
    fn a_view_of_one_or_two_solids_has_a_step_file_for_each_and_an_assembly_none() {
        let all = cases(1);
        let steps = |c: &Case| of_case(c).step.len();
        // A boolean's first view is its intersection; a mirror join's is the part and its image.
        assert!(
            all.iter()
                .filter(|c| c.test.starts_with("booleans/overlap/boxes"))
                .all(|c| steps(c) == 1)
        );
        let mirrored: Vec<_> = all.iter().filter(|c| c.exact.mirror.is_some()).collect();
        assert!(!mirrored.is_empty());
        assert!(mirrored.iter().all(|c| steps(c) == 2));
        assert!(all.iter().filter(|c| c.refuse).all(|c| steps(c) == 0));
        let sound: Vec<_> = all.iter().filter(|c| c.exact.sound.is_some()).collect();
        assert!(!sound.is_empty());
        assert!(sound.iter().all(|c| steps(c) >= 1));
        let assemblies = all
            .iter()
            .filter(|c| c.test == "queries/assembly-clash-detection");
        assert!(assemblies.clone().count() > 0 && assemblies.clone().all(|c| steps(c) == 0));
        let with = all.iter().filter(|c| steps(c) > 0).count();
        assert!(with * 100 >= all.len() * 90, "{with} of {}", all.len());
    }
}
