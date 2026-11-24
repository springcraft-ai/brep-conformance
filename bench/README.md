# bench

How an official benchmark session runs: one bare-metal instance, started for the session, runs the
suite on the registered kernels and is terminated, and each kernel's shape files and report are
published. The results site changes only when a pull request that lists the run merges.

## Overview

The [Benchmark workflow](../.github/workflows/benchmark.yml) is started by hand on `main`
(`workflow_dispatch`). It has six jobs:

| Job       | Runs on         | What it does                                                                       |
| --------- | --------------- | ---------------------------------------------------------------------------------- |
| `plan`    | GitHub's runner | Checks the inputs and states what the run will do, before any machine starts       |
| `start`   | GitHub's runner | Registers a one-time runner, launches the machine, waits for it                    |
| `bench`   | The machine     | Records the machine, builds, runs the session, keeps the results, stages the files |
| `stop`    | GitHub's runner | Terminates the machine, also when `bench` failed or was cancelled                  |
| `publish` | GitHub's runner | Copies each kernel's shape files the CDN lacks, when `bench` succeeded             |
| `reports` | GitHub's runner | Publishes each pass's report and, for prod, proposes the run: [Reports](#reports)  |

Its inputs, which default to a full run that can go on the site:

| Input         | Default | What it does                                                                     |
| ------------- | ------- | -------------------------------------------------------------------------------- |
| `kernels`     | `all`   | `all`, or the kernels to run, comma-separated, as `kernels.json` names them      |
| `tests`       | `all`   | `all`; `smoke`, the few minutes' subset in [`smoke.txt`](smoke.txt); or `custom` |
| `custom_args` | empty   | The suite's arguments with `tests: custom`, such as `--family fillets/`          |
| `nist`        | on      | Runs NIST's STEP files as `step/read-nist` cases                                 |
| `publish`     | `prod`  | The CDN the files and reports go to: `prod`, `dev` or `none`                     |
| `fail_bench`  | off     | Fails `bench` on purpose, to test that the machine still ends                    |

The run is named for its kernels, tests and CDN in the list of runs. [`plan.py`](plan.py) fails the
run, before a machine starts, on a kernel `kernels.json` does not list, on `dev` or `prod` from a
branch other than `main` or a `v*` tag, and on `custom` with no arguments. Its summary names the
suite and commit, each kernel's adapter commit, whether the run can go on a leaderboard, and an
estimate from each kernel's last pass time in `reports.json`.

A run of `all` kernels starts or replaces its suite's leaderboard. A run of named kernels sets
only their rows, which is how a kernel joins the leaderboard after it registers, or is run again
after an anomaly; on a suite with no leaderboard yet it is listed with its pages and moves nothing
([formats.md](../doc/formats.md#published-runs)).

The machine ends in three independent ways: the `stop` job, a lifetime timer armed at first boot,
and a shutdown that terminates the instance.

## The machine

The machine is launched from one launch template, the one `BENCHMARK_LAUNCH_TEMPLATE` names. The
template is defined in our infrastructure code, which is not in this repository. These are its
settings:

| Setting       | Value                                                                       |
| ------------- | --------------------------------------------------------------------------- |
| Cloud, region | AWS, us-west-2                                                              |
| Instance type | `c7i.metal-24xl`: bare metal, with no hypervisor and no other tenant        |
| Processor     | Intel Xeon Platinum 8488C, 48 cores, base frequency 2400 MHz                |
| Image         | Canonical's Ubuntu 24.04 for amd64, build 20261108, `ami-0346507e743a5ffc6` |
| Root volume   | 100 GiB `gp3`, encrypted, deleted with the instance                         |
| Instance role | None: the machine holds no cloud credentials                                |
| On shutdown   | The instance is terminated, not stopped                                     |

The image is pinned to one build, because a newer one brings a newer kernel. The workflow's role can
launch only from this template, and only this instance type.

The template sets what the machine is. What happens on it after it boots is the next section.

## The machine's settings

[`first-boot.sh`](first-boot.sh) is the rest of the machine's configuration, and it is what makes
timings comparable between sessions:

- One logical CPU per core: simultaneous multithreading is off.
- The clock is fixed at the processor's base frequency: turbo is off, and the governor is
  `performance`.
- The first 24 cores run the operating system and the suite. The others are isolated, for one
  adapter process each.
- Deep idle states and background package work are off.

It writes what it found to `/etc/benchmark-runner/environment`, which the `bench` job uploads as the
`machine` artifact: the instance type, the image, the zone, the kernel and its command line, the
processor model and its microcode, the online and isolated cores, and the clock settings. So each
session records the machine it ran on, and a difference from the table above shows there.

## The session

[`setup.sh`](setup.sh) installs the toolchains, checks each kernel's adapter out under `adapters/`
at the commit [`kernels.json`](../kernels.json) pins, and builds the suite and each adapter as the
registry says. Unless the workflow's `nist` input is off, it also fetches NIST's STEP files with
[`nist/fetch.py`](../nist/fetch.py), which takes them from the CDN first and accepts only the archive
whose hash it pins. `results/build.txt`
records every commit and tool version.

[`run-session.sh`](run-session.sh) then runs one pass per kernel with these settings:

| Setting  | Value                                                                                                                      |
| -------- | -------------------------------------------------------------------------------------------------------------------------- |
| Order    | The run's kernels in the order `kernels.json` lists them: one pass each                                                    |
| Workers  | 24, always passed                                                                                                          |
| Cores    | The suite's runner on the housekeeping cores; each adapter on an isolated core of its own, through [`pin.sh`](pin.sh)      |
| Adapters | Started by the registry's command, through [`adapters.py`](adapters.py)                                                    |
| Release  | A pass whose adapter reports another release than `kernels.json` pins is invalid                                           |
| Files    | Every pass runs with `--glb` and `--step` to a RAM disk; the suite draws a case after judging it, outside every timed call |
| NIST     | On by default: every pass runs with `--nist`, a `step/read-nist` case per NIST file; off with the workflow's `nist` input  |

When the last pass ends, each kernel's files are moved to `assets/<kernel>/`. A pass that did not
finish cleanly fails the job, after its results are kept, so nothing is staged or published.
Otherwise, unless `publish` is `none`, `bench` sends the files to a private staging bucket, under a
prefix that names the run. `publish` copies each pass's files from there to
`brep-conformance/<suite>/<kernel>/<version>/` on the CDN, skipping those already there, with each
file's type and an immutable cache lifetime. The copy is made inside the storage service, so the
files do not pass through GitHub. Each report names that folder as `assets`
([formats.md](../doc/formats.md#assets)).

## Reports

The [Reports workflow](../.github/workflows/reports.yml) runs after `publish`, and can be started by
hand with a run's id to publish an earlier Benchmark run's reports again, such as after a failed
publish. It runs no kernel code. Its `publish` job writes each pass's report with
[`publish-reports.sh`](publish-reports.sh): `brep-conformance-report --flat --verified`, with the
session's folder, so the report records the run, its commits and the machine. Each report is named
by its hash and uploaded to `brep-conformance/reports/<run>-<attempt>/` with an immutable cache
lifetime ([formats.md](../doc/formats.md#published-runs)).

For `prod`, its `propose` job adds the run to [`reports.json`](../reports.json) as formats.md says
and opens a pull request with the benchmark's GitHub App. The pull request compares each kernel with its
row on the leaderboard now. Merging it is the approval: the Pages workflow builds the site from the
runs `reports.json` lists, and its check on the pull request fails if a listed report is missing or
altered. A run with no complete report, or one already listed, opens no pull request. To take a run
back, remove it from `reports.json` in a pull request.

The machine runs kernel code, which must never be able to write to the CDN. So the machine holds one
cloud credential, and it can only write to staging: it cannot read, list or delete there, and it
cannot reach the CDN. The staging bucket deletes what it holds after a few days.

## What the bench job may not do

The `bench` job runs kernel code, so it declares no environment and has no `id-token` permission. It
cannot read the App's key, and nothing it starts can ask GitHub for a token that a cloud role would
accept. The job fails if it finds that it can.

The machine does hold one cloud credential, its own role, and anything running on it can use that
role. The role can put files in the staging bucket and do nothing else. `bench` lists what it sent
in `results/staged.txt`, and `publish` refuses to copy unless staging holds exactly that list.

## Adapter update

The [Adapter update workflow](../.github/workflows/adapter-update.yml) has Claude move each adapter
the suite maintains to its kernel's newer release, and use what the release changed. It is started
by hand on `main`, and opens a pull request in each adapter's repository whose kernel has a newer
release; it merges nothing. An adapter its kernel's own maintainers keep, or one whose repository is
not the suite's, is not offered: its maintainers move it by pull request.

| Input     | Default | What it does                                                                     |
| --------- | ------- | -------------------------------------------------------------------------------- |
| `kernels` | `all`   | `all` or empty for every adapter it can update, or kernels separated by commas   |
| `version` | empty   | The release to move to, for one kernel; empty for each one's newest published    |
| `dry_run` | off     | Opens no pull request; each change and its report stay as the run's artifacts    |

Each kernel runs on its own, in
[`adapter-update-kernel.yml`](../.github/workflows/adapter-update-kernel.yml), so one adapter that
fails stops no other.

[`adapter_update.py`](adapter_update.py) finds the release to move to, and changes nothing. It moves
a kernel only between published releases, never to a branch, a draft or a pre-release, and the
adapter's own pin says where those come from:

| The adapter pins its kernel                 | Its newest release                         |
| ------------------------------------------- | ------------------------------------------ |
| Exactly, `=X.Y.Z`, from crates.io           | The newest on crates.io                    |
| By `rev` from GitHub, publishing no crate   | The tagged commit of its newest GitHub one |
| In `build.sh`, `package=X.Y.Z`, conda-forge | The newest on conda-forge for Linux        |

It also reads the licence each pin's two releases declare, from the same place, so a licence named
two ways in two places is no change.

Each kernel has three jobs:

| Job       | Holds          | What it does                                                             |
| --------- | -------------- | ------------------------------------------------------------------------ |
| `target`  | Nothing        | Finds the release to move to, and stops if the adapter is at it already  |
| `update`  | A Claude token | Claude moves the adapter to the release; the job checks it and runs some |
| `propose` | The App's key  | Opens the pull request in the adapter's repository, or updates its own   |

Claude follows [`adapter-update-prompt.md`](adapter-update-prompt.md): what the contract allows an
adapter and how it applies, the two releases' sources, and what to report. It builds the adapter and
runs the families it changes, never the whole suite. The job then builds the change once more and
runs the families in [`smoke.txt`](smoke.txt) and those Claude names, and the pull request shows
that run, not Claude's. The adapter's own CI runs on the pull request as on any other. The whole
suite runs in the adapter's weekly CI, and on the benchmark machine once Registry update moves
`kernels.json`.

Claude may change the adapter's source, build files, README and manifests, and nothing else. The
pull request opens as a draft when the adapter does not build, does not pin the release, reports
another, the kernel's licence changed, or Claude did not finish. Run again for the same release, the
workflow replaces its own branch and updates its pull request, unless someone else has pushed to it.

`target` runs no kernel code and holds nothing. `update` builds and runs kernel code, and holds only
a Claude token that expires in minutes and is good for one workspace. Only `propose`, which runs no
kernel code, holds the App's key.

## Registry update

Once adapters' pull requests merge, the [Registry update
workflow](../.github/workflows/registry-update.yml) moves their entries in
[`kernels.json`](../kernels.json). Started by hand on `main`, with the same `kernels` input and a
`dry_run`, it resolves each adapter's `main` and reads the kernel release its manifest pins,
building nothing: the adapter's own pull request checked that it reports that release, and an
official run refuses one that does not. It moves every entry whose adapter commit or release
changed, and opens one pull request with a row for each, linking the adapter's changes since its
last pin.

Each adapter's changes were reviewed in its own repository when they merged there, so the
registry's pull request is the record of the move more than a second review. The workflow waits for
its checks and then tries to merge it. With the App on the `main` ruleset's bypass list, for pull
requests only, it merges; without, it is refused, and the pull request waits for a maintainer as
[process.md](../doc/process.md#the-registry) says. An adapter whose `main` pins no release is left
where it is, and named.

A licence change always waits for a maintainer. For each entry it moves, the workflow compares the
licence each pin's release declares at the registry's adapter commit with the one at `main`, each
from the pin's own source, and diffs the adapter's licence files and its manifest's `license`
between the two. Sources spell licences their own ways, and `kernels.json` the suite's, so nothing
compares a source with `kernels.json`, and the workflow never writes its `license`. A pin it cannot
read on either side, such as one whose kernel now comes from elsewhere, counts as a change. Any
change is listed in the pull request, which is then left for review, for a maintainer to write the
licence `kernels.json` gives.

## Setup

The workflow reads these from the `benchmark` environment, which allows only `main`:

| Name                        | Kind     | What it is                                    |
| --------------------------- | -------- | --------------------------------------------- |
| `BENCHMARK_APP_CLIENT_ID`   | Variable | The GitHub App that registers the runner      |
| `BENCHMARK_APP_PRIVATE_KEY` | Secret   | That App's key                                |
| `BENCHMARK_ROLE_ARN`        | Secret   | The role that may launch and terminate        |
| `BENCHMARK_AWS_REGION`      | Variable | The region of the machine                     |
| `BENCHMARK_LAUNCH_TEMPLATE` | Variable | The launch template's name                    |
| `BENCHMARK_SUBNET_IDS`      | Secret   | Subnets to try, in order, separated by commas |

The role and the subnets are secrets so that the public logs mask them, and the account's ID with
them. The jobs also mask the machine's instance ID, and the machine names itself
`benchmark-runner` in place of its private IP, which the runner would print in the `bench` log.

[`app-token.sh`](app-token.sh) turns the App's key into a token limited to this repository and to
the permissions a job asks for: `start` asks for its runners only, and the Reports workflow's
`propose` job for contents and pull requests, to push its branch and open the pull request. The App
must hold Administration, Contents and Pull requests, each read and write, on this repository.

`publish` and the Reports workflow's `publish` job read these from the repository. Neither
declares an environment, because the upload roles trust this repository's `main` and `v*` tags with
the default token subject. The buckets are secrets, so the public logs mask them:

| Name                                | Kind     | What it is                 |
| ----------------------------------- | -------- | -------------------------- |
| `BREP_CONFORMANCE_ASSET_ROLE`       | Secret   | The prod CDN's upload role |
| `BREP_CONFORMANCE_ASSET_BUCKET`     | Secret   | The prod CDN's bucket      |
| `BREP_CONFORMANCE_ASSET_ROLE_DEV`   | Secret   | The dev CDN's upload role  |
| `BREP_CONFORMANCE_ASSET_BUCKET_DEV` | Secret   | The dev CDN's bucket       |
| `BENCHMARK_STAGING_BUCKET`          | Secret   | The staging bucket         |

`bench` reads the staging bucket's name too, for the one step that sends the files.

The Adapter update and Registry update workflows have their own App, which holds Contents and Pull
requests, read and write, and nothing else, on this repository and the adapter repositories the
suite maintains. Their `propose` jobs read it from the `adapter-update` environment, and the
`update` job reads the Claude workspace from the `adapter-claude` environment; both allow only
`main`:

| Name                           | Kind     | Environment      | What it is                         |
| ------------------------------ | -------- | ---------------- | ---------------------------------- |
| `ADAPTER_APP_CLIENT_ID`        | Variable | `adapter-update` | The adapter update App             |
| `ADAPTER_APP_PRIVATE_KEY`      | Secret   | `adapter-update` | That App's key                     |
| `ANTHROPIC_FEDERATION_RULE_ID` | Variable | `adapter-claude` | The rule that grants Claude tokens |
| `ANTHROPIC_ORGANIZATION_ID`    | Variable | `adapter-claude` | The Claude organization            |
| `ANTHROPIC_SERVICE_ACCOUNT_ID` | Variable | `adapter-claude` | The account the tokens act as      |
| `ANTHROPIC_WORKSPACE_ID`       | Variable | `adapter-claude` | The workspace they are good for    |

There is no Claude key: the `update` job trades GitHub's token for the run for a Claude token that
lasts minutes. The rule grants one only to a manual run of this repository's `main` in the
`adapter-claude` environment, and the workspace's spending limit caps what a run can cost.

The [Claude check workflow](../.github/workflows/claude-check.yml) asks Claude one question from
that environment and prints the answer, or the reason there is none. Run it after a change to the
rule, the workspace or the variables: it takes a minute, where an Adapter update reaches Claude
only after a suite run.
