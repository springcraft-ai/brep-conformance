# Versioning

This document says how the suite's releases are numbered, what each number promises to someone
comparing results, and what a release records. It governs the releases and is not part of the specs.

## What a version names

A suite version names everything that decides a score: the tests and the cases they draw for the
default seed, the checks and the scoring in [contract.md](contract.md), and the protocol in
[api.md](api.md). Two runs on one suite version ran the same cases under the same rules, so their
results compare case for case.

The contract and the protocol each keep a version number of their own. That number says which text a
check or an adapter was written to. The protocol's rule is in [api.md](api.md#versions). The
contract's major number rises when a clause changes what it requires, its minor number rises when
clauses are added for new tests, and its patch number rises when its wording is corrected. The
suite's, the contract's and the protocol's versions all start together at 1.0.0 and move apart from
there. The suite version is the one that orders releases, and the only one a result is grouped by.

An adapter belongs to its kernel and has its kernel's version. A change that alters no case and no
verdict, such as the report's layout, a document's wording or the runner's speed, needs no release
of its own and ships with the next release.

## The three numbers

A version is `MAJOR.MINOR.PATCH`, as in semantic versioning. A change is breaking when a result
stops meaning what it meant.

| Number | Raised by                                  | Results on the version before                |
| ------ | ------------------------------------------ | -------------------------------------------- |
| Major  | A change to the protocol or to the scoring | Do not compare                               |
| Minor  | New tests                                  | Stand for the cases they hold; totals differ |
| Patch  | A fix to tests that were wrong, each named | Stand, except on the tests named             |

- **Major.** Either the protocol changes so that an adapter written to the old protocol no longer
  runs, which is a change that raises the protocol's own major number, or the scoring changes. A
  scoring change is a change to how an existing case is judged or how cases are counted, such as the
  tolerance, the tiers, a clause's bound, a clause added to existing cases, or what the headline
  counts. A score from before the change says nothing about a score after it.
- **Minor.** Tests are added, with any function they need, as [process.md](process.md) admits them.
  Existing cases keep their ids and are judged as before, so a kernel's result on each of them
  stands. The kernel's score still moves, because there are more cases, and a new function the
  kernel does not offer counts as unsupported.
- **Patch.** A test was wrong and is fixed. What was wrong is an expected value, a case outside what
  its test declares, or a check that did not implement its clause. A test that cannot be fixed is
  withdrawn the same way. The release names every affected test by its path, and only results on
  those tests change.

A fix that cannot name the tests it touches is not a patch. A fix to a check that every case shares
is a change to the scoring, and raises the major number.

A family that changes what it draws gets new case ids, as [formats.md](formats.md) says. That change
is a patch when it corrects the tests the family feeds, and a minor release when it widens them.

Published results use the default seed. A run on another seed drew other cases, so it can be
compared only with itself.

## What a release records

Every release has an entry in [`CHANGELOG.md`](../CHANGELOG.md), newest first, under its version and
date. An entry says what a reader with results on the version before must know:

- **Protocol**: what changed on the wire, and the protocol's new number.
- **Scoring**: what is judged or counted differently, and the contract's new number.
- **Tests added**: the new tests, by path or by directory.
- **Tests fixed**: each affected test by path, what was wrong, and which way results move.

A major or minor entry carries the sections that apply. A patch entry carries only **Tests fixed**.

The version itself is `SUITE_VERSION` in [`run.rs`](../suite/src/run.rs), which every results file
records as `suite`. A release raises it in the commit that adds the entry.

The changelog starts at 1.0.0, the first public release. The numbers before it were never published,
and no entry lists them.

## Results across versions

Results are grouped by suite version, and are compared only within one version.

- A results file and a flat report both carry the suite version they were made on.
- The report page compares the runs on the newest version it is given. Runs on earlier versions are
  listed apart, under their version, with their scores and no row in the comparison.
- The results site lists each kernel's latest release on the current suite version. A kernel not yet
  run on it stays under the version it last ran on, and does not appear beside the others until it
  is rerun.
- Reports on earlier versions stay published, so a link to a result keeps meaning what it meant.
