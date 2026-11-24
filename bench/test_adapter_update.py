"""Tests for adapter_update.py: python3 -m unittest discover -s bench"""

import json
import os
import tempfile
import unittest
from unittest import mock

from adapter_update import (bump, compare, kernel_crates, kernel_packages, kernels, load, pin,
                            pinned, plan, read, registry, report, write)

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

SOURCE = """impl brep_protocol::Kernel for K {
    fn name(&self) -> String {
        "kernel".into()
    }

    fn version(&self) -> String {
        "0.24.0".into()
    }
}
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

    def test_pinning_moves_only_the_named_crates_and_keeps_the_rest(self):
        moved = pin(MANIFEST, {"Kernel_core": ("0.24.0", "0.27.0"),
                               "Kernel_blend": ("0.24.0", "0.27.0")})
        self.assertIn('kernel = { package = "Kernel_core", version = "=0.27.0" }', moved)
        self.assertIn('blend = { package = "Kernel_blend", version = "=0.27.0" }', moved)
        self.assertIn('brep-protocol = "=2.0.1"', moved)
        self.assertIn('helper = "=1.0.0"', moved)
        self.assertIn('tested = "=9.9.9"', moved)
        self.assertIn("# The same release's blend crate.", moved)

    def test_a_plain_pin_moves(self):
        self.assertEqual(pin('[dependencies]\nspringrep = "=2.0.0"\n',
                             {"springrep": ("2.0.0", "2.1.0")}),
                         '[dependencies]\nspringrep = "=2.1.0"\n')


class Reported(unittest.TestCase):
    def test_the_literal_version_moves_with_the_kernel(self):
        text, changed = report(SOURCE, "0.24.0", "0.27.0")
        self.assertTrue(changed)
        self.assertIn('"0.27.0".into()', text)
        self.assertIn('"kernel".into()', text)

    def test_a_constant_or_another_version_is_left_alone(self):
        constant = SOURCE.replace('"0.24.0".into()', "kernel::VERSION.into()")
        self.assertEqual(report(constant, "0.24.0", "0.27.0"), (constant, False))
        self.assertEqual(report(SOURCE, "0.23.0", "0.27.0"), (SOURCE, False))

    def test_a_bump_to_a_given_version_moves_pins_and_report_together(self):
        with tempfile.TemporaryDirectory() as d:
            os.mkdir(os.path.join(d, "src"))
            write(os.path.join(d, "Cargo.toml"), MANIFEST.replace('helper = "=1.0.0"\n', ""))
            write(os.path.join(d, "src", "main.rs"), SOURCE)
            moved = bump(d, "0.26.0")
            self.assertEqual(moved["from"], "0.24.0")
            self.assertEqual(moved["to"], "0.26.0")
            self.assertEqual(moved["reported_in"], ["src/main.rs"])
            self.assertEqual(set(moved["crates"]), {"Kernel_core", "Kernel_blend"})
            self.assertIn('"0.26.0".into()', read(os.path.join(d, "src", "main.rs")))


class Git(unittest.TestCase):
    def bump(self, version=None):
        with tempfile.TemporaryDirectory() as d:
            os.mkdir(os.path.join(d, "src"))
            write(os.path.join(d, "Cargo.toml"), GIT)
            write(os.path.join(d, "src", "main.rs"), SOURCE.replace(
                '"0.24.0".into()', "kernel_core::VERSION.into()"))
            with mock.patch("adapter_update.github", fake_github):
                return bump(d, version), read(os.path.join(d, "Cargo.toml"))

    def test_moves_to_the_newest_release_and_its_tagged_commit(self):
        moved, manifest = self.bump()
        self.assertEqual((moved["from"], moved["to"]), ("0.2.8", "0.2.9"))
        self.assertEqual(moved["crates"]["kernel_core"]["to_commit"], NEW)
        self.assertIn('kernel_core = { git = "https://github.com/owner/kernel", rev = "bb09c367" }',
                      manifest)
        self.assertIn("/releases/tag/v0.2.9, with", manifest)
        self.assertEqual(moved["reported_in"], [])

    def test_a_pre_release_is_not_a_release(self):
        with self.assertRaises(SystemExit):
            self.bump("0.3.0-rc.1")
        self.assertEqual(self.bump("0.2.9")[0]["to"], "0.2.9")


class Compare(unittest.TestCase):
    def test_tallies_and_each_test_whose_result_moved(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = os.path.join(d, "a.json"), os.path.join(d, "b.json")
            results(a, "0.24.0", [("1", "fillets/hole-rim", "unsupported"),
                                  ("2", "fillets/hole-rim", "unsupported"),
                                  ("3", "booleans/boxes", "1e-7")])
            results(b, "0.27.0", [("1", "fillets/hole-rim", "1e-7"),
                                  ("2", "fillets/hole-rim", "1e-7"),
                                  ("3", "booleans/boxes", "1e-7")])
            table = compare(a, b)
            self.assertIn("| 1e-7 | 1 | 3 |", table)
            self.assertIn("| unsupported | 2 | 0 |", table)
            self.assertIn("| `fillets/hole-rim` | unsupported | 1e-7 | 2 |", table)
            self.assertNotIn("booleans/boxes", table)

    def test_no_change_says_so(self):
        with tempfile.TemporaryDirectory() as d:
            a = os.path.join(d, "a.json")
            results(a, "1.0.0", [("1", "t", "1e-7")])
            self.assertIn("No case changed its result.", compare(a, a))


PYPROJECT = """[project]
name = "kernel-adapter"
version = "0.1.0"
requires-python = ">=3.12,<3.13"
dependencies = ["kernel-libs==8.0.1.1.0", "helper>=1"]
"""


def fake_pypi(url):
    """PyPI's record of a package with final releases up to 8.0.2.0.0, a yanked 8.0.3.0.0, a
    pre-release, and a release with no files."""
    file, yanked = [{"yanked": False}], [{"yanked": True}]
    return {"releases": {"8.0.1.0.0": file, "8.0.1.1.0": file, "8.0.2.0.0": file,
                         "8.0.3.0.0": yanked, "8.1.0.0.0rc1": file, "9.0.0.0.0": []}}


class Packages(unittest.TestCase):
    def bump(self, version=None):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "pyproject.toml"), PYPROJECT)
            with mock.patch("adapter_update.fetch", fake_pypi):
                return bump(d, version), read(os.path.join(d, "pyproject.toml"))

    def test_only_exact_pins_in_the_dependencies_are_the_kernel(self):
        self.assertEqual(kernel_packages(PYPROJECT), [("kernel-libs", "8.0.1.1.0")])

    def test_moves_to_the_newest_final_release_with_files(self):
        moved, pyproject = self.bump()
        self.assertEqual((moved["from"], moved["to"]), ("8.0.1.1.0", "8.0.2.0.0"))
        self.assertEqual(moved["crates"]["kernel-libs"]["pypi"], True)
        self.assertIn('dependencies = ["kernel-libs==8.0.2.0.0", "helper>=1"]', pyproject)
        self.assertIn('version = "0.1.0"', pyproject)

    def test_a_named_release_must_be_final_and_published(self):
        self.assertEqual(self.bump("8.0.1.0.0")[0]["to"], "8.0.1.0.0")
        for version in ("8.0.3.0.0", "8.1.0.0.0rc1", "9.0.0.0.0", "7.0"):
            with self.assertRaises(SystemExit):
                self.bump(version)


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
        self.assertEqual(self.pinned_in({"pyproject.toml": PYPROJECT}), "8.0.1.1.0")

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
