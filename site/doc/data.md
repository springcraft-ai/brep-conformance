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
- `suite`: the suite's `version` and the `contract` version. `run`: when it started (`started_unix`)
  and ended (`unix`), how long it took (`wall_ms`), the calibration workload's time
  (`calibration_ms`), the limit on one call (`time_limit_ms`), and whether it ran on the suite's
  benchmark machine (`verified`). `environment`: OS, architecture, cores and threads.
- `unsupported`: the functions the kernel does not offer; `stand_ins`: the function sent in place of
  each primitive it has no constructor for.
- `cases`: one row per case: `id`, `test`, `operation` (the test path's first part, its area),
  `kind` (exact, sound or rejection), `result`, `why`, `failure` (geometry or measurement),
  `time_ms`, `step_ms` (the B17 round trip through STEP, which `time_ms` leaves out; null where none
  ran or in a report from before it was recorded), each step's time (`ops`: `op`, `outcome`, `ms`),
  `cold`, whether the runner started a kernel for the case (null in a report from before it was
  recorded), the test's `group_median_ms` (cold cases included), and `exports`, the shape files the
  kernel wrote for it.

A test's URL segment is its path without the area, joined by dashes: `booleans/overlap/spheres` is
`overlap-spheres` under `booleans`.

Every kernel draws its own shape files, and the site shows no others. A case's `exports` holds the
key of its drawing as `glb`, or null, and in the list `step` the key of a STEP file for each solid
the case shows: one, two, or none. A case with no `exports` has no files. A key is a path of plain
names ending in `.glb` or `.step`, such as `<test>/<id>.<hash>.glb`; the site refuses a report with
any other.

A file's URL is the report's `assets`, its kernel's folder, followed by the key. `assets` must be
`https://cdn.springcraft.ai/brep-conformance/<suite>/<kernel>/<version>/`: the suite's CDN, then the
path the report has in the reports directory. The site refuses any other, so that no report names
another host or another kernel's files. It is null where the run wrote none, and a report written
before `assets` existed takes that same address.

A test's page shows a case as the selected kernel drew it: the kernel the address names, or with
none named, the kernel that passed it at the best tier, a tie going to the higher on the
leaderboard. A kernel that refused, crashed, timed out or does not offer the operation made no
result, and its drawing holds only what it made before it stopped, such as one operand; the page
shows it on its first view with a shape, and says it is partial. Where the selected kernel wrote no
drawing at all, the best other drawing stands in, and the page says so. One address always shows one
drawing. A table under the case has a row for each kernel: the files it wrote, which are its drawing
and each STEP file, named by its solid's role where there are two; its result; and why it failed. A
kernel's name shows its drawing. A kernel that wrote a STEP file and no drawing still has its STEP
file offered.

A file a report names is on the CDN: the run's publish step puts every file there before the report
is published. Every file is a link in the page's own HTML, so it works before the page's script
runs. A click fetches the file and saves it under its own name, since a browser will not download a
file from another origin by a link alone; where the fetch fails, the file opens in a new tab.

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

A case's time leaves out the B17 round trip through STEP: only kernels that read and write STEP run
it, and it times interchange, not the operation. Its median, over the passed cases that ran it, is
shown beside the case time as the kernel's STEP time. Reports written before `step_ms` was recorded
count the round trip in the case time, so their times are not comparable with later ones.

## Leaderboard

One row per kernel: its latest release on the current suite version, the highest suite version among
the reports. A version that is not a release number, such as springrep's source hash, is ordered by
when it ran. Rows rank by pass rate, then by kernel id. A kernel's license comes from
[`kernels.ts`](../src/data/kernels.ts), not the report. No kernel is placed or coloured apart.

## Pages

Each suite version is a report of the site, and every page but the home page sits inside one, so a
link to it keeps naming the same results once a newer suite is published:

| Path                                    | Page                                                   |
| --------------------------------------- | ------------------------------------------------------ |
| `/`                                     | The current suite's leaderboard                        |
| `/reports/<suite>/`                     | That suite's leaderboard                               |
| `/reports/<suite>/<kernel>/`            | One kernel's row on that suite's leaderboard           |
| `/reports/<suite>/compare/<test path>/` | One test, every kernel; the path as the suite names it |
| `/reports/<suite>/compare/<level>/`     | The table of the tests under a level of that path      |

A kernel's page has three segments. Its name is its row on the leaderboard and never moves;
`<kernel>@<version>` is that release's own page, the leaderboard's run of it where there is one and
its newest run otherwise; `<kernel>@<version>~<run>` is one run, for good. Each page lists the
kernel's other runs, and what another run of the same release did differently. A level is any part
of a test's path above the test, such as `booleans` or `booleans/degenerate`. A compare page's
breadcrumb leads up through the page of each level to the report. `/reports/` has no page.
