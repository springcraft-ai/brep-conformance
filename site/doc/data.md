# Data

The report the site reads, and the numbers it derives from it. Both live in
[`results.ts`](../src/data/results.ts); this page states the rules, the code is exact.

## The report

The flat report `brep-conformance-report RESULTS.json --flat` writes, as
[formats.md](../../doc/formats.md) describes it. One file per kernel release and suite version, at
`<suite>/<kernel>/<version>/report.json`, named by the report's `suite.version`, `kernel.name` and
`kernel.version`. A release rerun on a new suite version gets a file of its own, so the report an
older link names stays as it was; a rerun on the same suite version replaces the file.

- `kernel`: its `name`, `version`, declared `tolerance` and `configuration`.
- `suite`: the suite's `version` and the `contract` version. `run`: when it ended (`unix`) and
  whether it ran on the suite's benchmark machine (`verified`). `environment`: OS, architecture,
  cores and threads.
- `unsupported`: the functions the kernel does not offer; `stand_ins`: the function sent in place of
  each primitive it has no constructor for.
- `cases`: one row per case: `id`, `test`, `operation` (the test path's first part, its area),
  `kind` (exact, sound or rejection), `result`, `why`, `failure` (geometry or measurement),
  `time_ms`, the test's `group_median_ms`, and `exports`, the shape files the kernel wrote for it.

A test's URL segment is its path without the area, joined by dashes: `booleans/overlap/spheres` is
`overlap-spheres` under `booleans`.

Every kernel draws its own shape files, and the site shows no others. A case's `exports` holds the
key of its drawing as `glb`, or null, and in the list `step` the key of a STEP file for each solid
the case shows: one, two, or none. A case with no `exports` has no files. A key is a path of plain
names ending in `.glb` or `.step`, such as `<test>/<id>.<hash>.glb`, by the rule
[`publish-assets.sh`](../../publish-assets.sh) holds it to; the site refuses a report with any
other.

A file's URL is the report's `assets`, its kernel's folder, followed by the key. `assets` must be an
https address ending `/<suite>/<kernel>/<version>/`, the path the report has in the reports
directory, so that no report names another's files; it is null where the run wrote none. A report
written before `assets` existed takes the suite's compiled default,
`https://cdn.springcraft.ai/brep-conformance`, with that path after it.

A test's page shows a case as the selected kernel drew it, and as the first kernel that drew it
where that one wrote no drawing. It offers each STEP file the kernel wrote, named by its solid's
role where there are two.

The report does not yet say who wrote a kernel's adapter, which functions a test calls, or a failed
check's numbers and failing points, so the site shows none of them.

## Grades

A case's grade is its `result`: the tier it reached, 1e-7, 1e-6, 1e-5 or 1e-3, the tightest ε at
which every check holds; or, where it reached none, its outcome: unsupported, refused, timeout, fail
or crash. Grades rank in that order, best first. A kernel's grade on a test is its worst case's.

On the page, tiers take the lidar ramp, brightest best; unsupported is hatched; fail and crash are
red type; refused and timeout are quiet ink.

## Pass rate

The headline is the share of all cases graded 1e-7. Every other grade counts against it, so a kernel
that does not offer a function loses those cases.

## Speed

Times are grouped by test. In each group the first case is a warmup and is dropped, and only the
cases every compared kernel reached at the same tier are kept. A kernel's factor in a group is its
median time over the fastest kernel's median; its speed is the geometric mean of those factors, 1.00
when it is fastest everywhere.

## Time

The leaderboard's median case time is taken over the cases every kernel answered, with a tier or a
fail, so each kernel is timed on the same work; its middle half runs from the quarter mark to the
three-quarter mark. The grid's time for a test is the report's `group_median_ms`, marked against the
fastest of the kernels shown. A time the report rounds to 0 is shown but not compared, as no ratio
can be taken against it. The grid's footer is the geometric mean of those ratios over the tests
every kernel shown passed in full.

## Leaderboard

One row per kernel: its latest release on the current suite version, the highest suite version among
the reports. Each report has its page at `/reports/<suite>/<kernel>/<version>/`, ranked or not. A
version that is not a release number, such as springrep's source hash, is ordered by when it ran.
Rows rank by pass rate, then by kernel id. A kernel's license comes from
[`kernels.ts`](../src/data/kernels.ts), not the report. No kernel is placed or coloured apart.
