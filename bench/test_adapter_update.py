"""Tests for adapter_update.py: python3 -m unittest discover -s bench"""

import json
import os
import tempfile
import unittest
import urllib.error
from unittest import mock

import adapter_update

from adapter_update import (check, conda_packages, kernel_crates, kernels, licences, load, pinned,
                            plan, read, registry, summary, target, write)

MANIFEST = """[package]
name = "kernel-adapter"
version = "0.1.0"

[dependencies]
brep-protocol = "=2.0.1"
kernel = { package = "Kernel_core", version = "=0.24.0" }
# The same release's blend crate.
blend = { package = "Kernel_blend", version = "=0.24.0" }
helper = "=1.0.0"
nalgebra = { version = "0.35", default-features = false }
fromgit = { git = "https://example.com/k", rev = "bb09c367" }

[dev-dependencies]
tested = "=9.9.9"
"""


def results(path, version, cases):
    write(path, json.dumps({"kernel": {"name": "kernel", "version": version},
                            "cases": [{"id": i, "test": t, "result": r} for i, t, r in cases]}))


GIT = """[dependencies]
brep-protocol = "2"
# https://github.com/owner/kernel/releases/tag/v0.2.8, with `VERSION` in src/lib.rs.
kernel_core = { git = "https://github.com/owner/kernel", rev = "07e083bb" }
"""

OLD, NEW = "07e083bb" + "1" * 32, "bb09c367" + "2" * 32


def fake_github(path):
    """The GitHub API of a repository with releases v0.2.8 and v0.2.9, and a pre-release after."""
    replies = {
        "repos/owner/kernel/commits/07e083bb": {"sha": OLD},
        "repos/owner/kernel/commits/v0.2.9": {"sha": NEW},
        "repos/owner/kernel/commits/deadbeef": {"sha": "4" * 40},
        "repos/owner/kernel/tags?per_page=100": [
            {"name": "v0.3.0-rc.1", "commit": {"sha": "3" * 40}},
            {"name": "v0.2.9", "commit": {"sha": NEW}},
            {"name": "v0.2.8", "commit": {"sha": OLD}},
        ],
        "repos/owner/kernel/releases/latest": {"tag_name": "v0.2.9"},
        f"repos/owner/kernel/license?ref={OLD}": {"license": {"spdx_id": "MIT"}},
        f"repos/owner/kernel/license?ref={NEW}": {"license": {"spdx_id": "MIT"}},
        "repos/owner/kernel/releases?per_page=100": [
            {"tag_name": "v0.3.0-rc.1", "draft": False, "prerelease": True},
            {"tag_name": "v0.2.9", "draft": False, "prerelease": False},
            {"tag_name": "v0.2.8", "draft": False, "prerelease": False},
        ],
    }
    return replies[path]


class Crates(unittest.TestCase):
    def test_exact_crates_io_pins_but_the_protocol_are_the_kernel(self):
        self.assertEqual(kernel_crates(MANIFEST),
                         [{"crate": "Kernel_core", "version": "0.24.0"},
                          {"crate": "Kernel_blend", "version": "0.24.0"},
                          {"crate": "helper", "version": "1.0.0"}])

    def test_a_github_dependency_pinned_by_rev_is_a_kernel_crate(self):
        self.assertEqual(kernel_crates(GIT), [{"crate": "kernel_core", "slug": "owner/kernel",
                                               "rev": "07e083bb"}])

    def test_a_git_dependency_on_a_branch_or_off_github_is_not(self):
        for line in ('k = { git = "https://example.com/k", rev = "1" }',
                     'k = { git = "https://github.com/owner/k", branch = "main" }'):
            self.assertEqual(kernel_crates(f"[dependencies]\n{line}\n"), [])


BUILD = """#!/bin/bash
# The release of conda-forge's kernelpkg package the adapter links.
kernelpkg=8.0.1
    ninja=1.12.1
"$mamba" create -c conda-forge "kernelpkg=$kernelpkg=*"
"""


def fake_fetch(url):
    """crates.io with Kernel_core and Kernel_blend 0.24.0 to 0.27.0, a yanked 0.28.0 and a
    pre-release, relicensed at 0.27.0; and conda-forge with kernelpkg 8.0.1 and 8.0.2 for linux-64, an
    8.1.0 for another platform, and a release named another way."""
    crates = {"0.24.0": "MIT", "0.26.0": "MIT", "0.27.0": "MIT OR Apache-2.0", "0.28.0": "MIT",
              "0.29.0-rc.1": "MIT"}
    for name in ("Kernel_core", "Kernel_blend"):
        if url == f"https://crates.io/api/v1/crates/{name}/versions":
            return {"versions": [{"num": v, "yanked": v == "0.28.0"} for v in crates]}
        for v, licence in crates.items():
            if url == f"https://crates.io/api/v1/crates/{name}/{v}":
                return {"version": {"license": licence}}
    if url == "https://api.anaconda.org/package/conda-forge/kernelpkg":
        file = lambda v, subdir="linux-64": {"version": v, "attrs": {
            "subdir": subdir, "license": "LGPL-2.1-only"}}
        return {"files": [file("8.0.1"), file("8.0.2"), file("8_0_3"), file("8.1.0", "osx-arm64")]}
    raise AssertionError(url)


class Conda(unittest.TestCase):
    def test_a_line_of_its_own_in_a_script_that_installs_from_conda_forge_is_a_pin(self):
        self.assertEqual(conda_packages(BUILD), [("kernelpkg", "8.0.1"), ("ninja", "1.12.1")])

    def test_a_script_that_installs_nothing_from_conda_forge_pins_nothing(self):
        self.assertEqual(conda_packages("#!/bin/bash\nkernelpkg=8.0.1\n"), [])


class Target(unittest.TestCase):
    def target(self, files, version=None):
        with tempfile.TemporaryDirectory() as d:
            for name, text in files.items():
                write(os.path.join(d, name), text)
            with mock.patch("adapter_update.github", fake_github), \
                    mock.patch("adapter_update.fetch", fake_fetch):
                return target(d, version)

    def test_moves_each_crate_to_the_newest_stable_release_and_names_a_licence_change(self):
        manifest = MANIFEST.replace('helper = "=1.0.0"\n', "")
        moved = self.target({"Cargo.toml": manifest})
        self.assertEqual((moved["from"], moved["to"]), ("0.24.0", "0.27.0"))
        self.assertEqual(set(moved["pins"]), {"Kernel_core", "Kernel_blend"})
        self.assertEqual(moved["licence_changed"][0],
                         {"name": "Kernel_core", "from": "MIT", "to": "MIT OR Apache-2.0"})

    def test_a_given_release_must_be_published_and_stable(self):
        manifest = MANIFEST.replace('helper = "=1.0.0"\n', "")
        moved = self.target({"Cargo.toml": manifest}, "0.26.0")
        self.assertEqual((moved["to"], moved["licence_changed"]), ("0.26.0", []))
        for version in ("0.28.0", "0.29.0-rc.1", "0.25.0"):
            with self.assertRaises(SystemExit):
                self.target({"Cargo.toml": manifest}, version)

    def test_a_crate_from_git_moves_to_the_newest_release_and_its_tagged_commit(self):
        moved = self.target({"Cargo.toml": GIT})
        self.assertEqual((moved["from"], moved["to"]), ("0.2.8", "0.2.9"))
        self.assertEqual(moved["pins"]["kernel_core"]["to_commit"], NEW)
        self.assertEqual(moved["pins"]["kernel_core"]["repository"],
                         "https://github.com/owner/kernel")
        with self.assertRaises(SystemExit):
            self.target({"Cargo.toml": GIT}, "0.3.0-rc.1")

    def test_a_conda_forge_package_moves_to_its_newest_stable_release_for_linux(self):
        moved = self.target({"build.sh": BUILD.replace("    ninja=1.12.1\n", "")})
        self.assertEqual((moved["from"], moved["to"]), ("8.0.1", "8.0.2"))
        self.assertEqual(moved["pins"]["kernelpkg"]["licence"],
                         {"from": "LGPL-2.1-only", "to": "LGPL-2.1-only"})
        with self.assertRaises(SystemExit):
            self.target({"build.sh": BUILD}, "8.1.0")

    def test_a_release_older_than_the_pin_is_refused_and_never_the_newest(self):
        manifest = MANIFEST.replace('helper = "=1.0.0"\n', "").replace("0.24.0", "0.26.0")
        with self.assertRaises(SystemExit):
            self.target({"Cargo.toml": manifest}, "0.24.0")
        ahead = manifest.replace("0.26.0", "0.30.0")
        with mock.patch("adapter_update.crate_licence", lambda name, version: "MIT"):
            moved = self.target({"Cargo.toml": ahead})
        self.assertEqual((moved["from"], moved["to"]), ("0.30.0", "0.30.0"))

    def test_changes_nothing_in_the_checkout(self):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "Cargo.toml"), GIT)
            with mock.patch("adapter_update.github", fake_github):
                target(d)
            self.assertEqual(read(os.path.join(d, "Cargo.toml")), GIT)


class GitHub(unittest.TestCase):
    def test_a_token_in_the_environment_goes_with_each_call(self):
        seen = {}

        def fetch(url, headers=None):
            seen["headers"] = headers
            return {}
        with mock.patch("adapter_update.fetch", fetch), \
                mock.patch.dict(os.environ, {"GITHUB_TOKEN": "t"}):
            adapter_update.github("repos/owner/kernel")
        self.assertEqual(seen["headers"], {"Authorization": "Bearer t"})
        with mock.patch("adapter_update.fetch", fetch), mock.patch.dict(os.environ, {}, clear=True):
            adapter_update.github("repos/owner/kernel")
        self.assertIsNone(seen["headers"])

    def test_a_repository_with_no_licence_file_has_none(self):
        def missing(path):
            raise urllib.error.HTTPError(path, 404, "Not Found", None, None)
        with mock.patch("adapter_update.github", missing):
            self.assertIsNone(adapter_update.git_licence("owner/kernel", NEW))


class Check(unittest.TestCase):
    WANTED = {"pins": {"kernel_core": {"source": "git", "to": "0.2.9", "to_commit": NEW},
                       "kernelpkg": {"source": "conda-forge", "to": "8.0.2"}}}

    def check(self, manifest, script):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "Cargo.toml"), manifest)
            write(os.path.join(d, "build.sh"), script)
            return check(d, self.WANTED)

    def test_a_checkout_that_pins_each_release_passes(self):
        manifest = GIT.replace("07e083bb", NEW[:8])
        self.assertEqual(self.check(manifest, "conda-forge\nkernelpkg=8.0.2\n"), [])
        self.assertEqual(self.check(GIT.replace("07e083bb", NEW), "conda-forge\nkernelpkg=8.0.2\n"), [])

    def test_each_pin_left_behind_dropped_cut_short_or_added_is_named(self):
        problems = self.check(GIT.replace("07e083bb", NEW[:6]),
                              "conda-forge\nkernelpkg=8.0.1\nninja=1.12.1\n")
        self.assertEqual(len(problems), 3)
        self.assertEqual(len(self.check(GIT, "")), 2)


class Licences(unittest.TestCase):
    def compare(self, old, new):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            for d, files in ((a, old), (b, new)):
                for name, text in files.items():
                    write(os.path.join(d, name), text)
            with mock.patch("adapter_update.github", fake_github), \
                    mock.patch("adapter_update.fetch", fake_fetch):
                return licences(a, b)

    def test_releases_under_one_licence_are_alike_however_a_registry_spells_it(self):
        old = {"Cargo.toml": '[dependencies]\nkernel = { package = "Kernel_core", version = "=0.24.0" }\n'}
        new = {"Cargo.toml": old["Cargo.toml"].replace("0.24.0", "0.26.0")}
        self.assertEqual(self.compare(old, new), [])
        self.assertEqual(self.compare({"build.sh": BUILD.replace("    ninja=1.12.1\n", "")},
                                      {"build.sh": BUILD.replace("8.0.1", "8.0.2")
                                       .replace("    ninja=1.12.1\n", "")}), [])

    def test_a_relicensed_release_is_named_with_both_licences(self):
        old = {"Cargo.toml": '[dependencies]\nkernel = { package = "Kernel_core", version = "=0.24.0" }\n'}
        new = {"Cargo.toml": old["Cargo.toml"].replace("0.24.0", "0.27.0")}
        self.assertEqual(self.compare(old, new), ["Kernel_core: MIT → MIT OR Apache-2.0"])

    def test_a_kernel_from_another_source_or_pins_that_cannot_be_read_go_to_a_person(self):
        crates = {"Cargo.toml": '[dependencies]\nkernel = { package = "Kernel_core", version = "=0.24.0" }\n'}
        conda = {"build.sh": BUILD.replace("    ninja=1.12.1\n", "")}
        self.assertEqual(len(self.compare(crates, conda)), 2)
        self.assertEqual(self.compare({"pyproject.toml": "[project]\n"}, conda),
                         ["the old adapter pins no release this can read"])


class Summary(unittest.TestCase):
    def test_tallies_and_each_test_with_a_case_that_did_not_answer(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "r.json")
            results(path, "0.27.0", [("1", "fillets/hole-rim", "1e-5"),
                                     ("2", "fillets/hole-rim", "fail"),
                                     ("3", "booleans/boxes", "1e-7"),
                                     ("4", "slabs/box", "unsupported"),
                                     ("5", "step/read", "crash")])
            text = summary(path)
            self.assertIn("kernel 0.27.0, 5 cases", text)
            self.assertLess(text.index("| 1e-7 | 1 |"), text.index("| 1e-5 | 1 |"))
            self.assertIn("| `fillets/hole-rim` | fail | 1 |", text)
            self.assertIn("| `step/read` | crash | 1 |", text)
            self.assertNotIn("`slabs/box`", text)

    def test_a_run_with_every_supported_case_answered_says_so(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "r.json")
            results(path, "1.0.0", [("1", "t", "1e-7"), ("2", "u", "unsupported")])
            self.assertIn("Every case it supports answered.", summary(path))


def adapter(repo, build="cargo build --release --locked", run="{dir}/target/release/k-adapter"):
    return {"kernel": "kernel", "version": "1.0.0",
            "adapter": {"repo": repo, "commit": "a" * 40, "build": [build], "run": run}}


class Plan(unittest.TestCase):
    def check(self, entry):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "kernels.json"), json.dumps({"kernels": [entry]}))
            return plan(d, "kernel")

    def test_an_adapter_the_suite_maintains_names_its_repository_build_and_binary(self):
        repo = "https://github.com/springcraft-ai/brep-conformance-kernel"
        self.assertEqual(self.check(adapter(repo)),
                         {"repo": repo, "build": ["cargo build --release --locked"],
                          "binary": "target/release/k-adapter"})
        self.assertEqual(self.check(adapter(repo, "./build.sh", "{dir}/build/k-adapter")),
                         {"repo": repo, "build": ["./build.sh"], "binary": "build/k-adapter"})

    def test_an_outside_adapter_another_build_or_a_binary_elsewhere_is_refused(self):
        for entry in (adapter("https://github.com/someone/brep-conformance-kernel"),
                      adapter("https://github.com/springcraft-ai/x", build="make"),
                      adapter("https://github.com/springcraft-ai/x", run="/usr/bin/k"),
                      adapter("https://github.com/springcraft-ai/x", run="{dir}/../k")):
            with self.assertRaises(SystemExit):
                self.check(entry)


class Kernels(unittest.TestCase):
    OURS = "https://github.com/springcraft-ai/brep-conformance-"

    def run_with(self, asked):
        entries = [dict(adapter(self.OURS + "a"), kernel="a"),
                   dict(adapter("https://github.com/someone/b"), kernel="b"),
                   dict(adapter(self.OURS + "c", "./build.sh", "{dir}/build/c"), kernel="c"),
                   dict(adapter(self.OURS + "d", build="make"), kernel="d")]
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "kernels.json"), json.dumps({"kernels": entries}))
            return kernels(d, asked)

    def test_all_or_nothing_is_every_kernel_the_workflows_can_update(self):
        self.assertEqual(self.run_with("all"), ["a", "c"])
        self.assertEqual(self.run_with(" "), ["a", "c"])

    def test_named_kernels_run_alone_in_the_registry_order_whatever_the_spacing(self):
        self.assertEqual(self.run_with("c, a"), ["a", "c"])
        self.assertEqual(self.run_with("c,,a ,"), ["a", "c"])

    def test_a_kernel_that_cannot_be_run_or_is_unknown_is_refused(self):
        for asked in ("a,b", "d", "a,zz"):
            with self.assertRaises(SystemExit):
                self.run_with(asked)


class Pinned(unittest.TestCase):
    def pinned_in(self, files):
        with tempfile.TemporaryDirectory() as d:
            for name, text in files.items():
                write(os.path.join(d, name), text)
            with mock.patch("adapter_update.github", fake_github):
                return pinned(d)

    def test_the_first_kernel_crate_or_package_names_the_release(self):
        self.assertEqual(self.pinned_in({"Cargo.toml": MANIFEST}), "0.24.0")
        self.assertEqual(self.pinned_in({"build.sh": BUILD}), "8.0.1")
        self.assertEqual(self.pinned_in({"Cargo.toml": MANIFEST, "build.sh": BUILD}), "0.24.0")

    def test_a_rev_names_the_release_tagged_at_it(self):
        self.assertEqual(self.pinned_in({"Cargo.toml": GIT}), "0.2.8")

    def test_a_rev_no_release_is_tagged_at_is_refused(self):
        with self.assertRaises(SystemExit):
            self.pinned_in({"Cargo.toml": GIT.replace("07e083bb", "deadbeef")})

    def test_an_adapter_that_pins_no_kernel_is_refused(self):
        with self.assertRaises(SystemExit):
            self.pinned_in({"Cargo.toml": '[dependencies]\nbrep-protocol = "2"\n'})


class Registry(unittest.TestCase):
    def test_moves_version_and_commit_of_the_one_entry(self):
        old = "a" * 40
        reg = {"kernels": [
            {"kernel": "other", "version": "0.24.0", "adapter": {"commit": "b" * 40}},
            {"kernel": "kernel", "version": "0.24.0", "adapter": {"commit": old}},
        ]}
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "kernels.json"), json.dumps(reg, indent=2))
            self.assertTrue(registry(d, "kernel", "0.27.0", "c" * 40))
            got = load(os.path.join(d, "kernels.json"))["kernels"]
            self.assertEqual(got[0], reg["kernels"][0])
            self.assertEqual((got[1]["version"], got[1]["adapter"]["commit"]),
                             ("0.27.0", "c" * 40))
            self.assertFalse(registry(d, "kernel", "0.27.0", "c" * 40))


if __name__ == "__main__":
    unittest.main()
