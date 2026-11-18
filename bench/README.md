# bench

How an official benchmark session runs: one bare-metal instance, started for the session, runs the
suite on every registered kernel and is terminated, and each kernel's shape files are published.

## Overview

The [Benchmark workflow](../.github/workflows/benchmark.yml) is started by hand on `main`
(`workflow_dispatch`). It has four jobs:

| Job       | Runs on         | What it does                                                                       |
| --------- | --------------- | ---------------------------------------------------------------------------------- |
| `start`   | GitHub's runner | Registers a one-time runner, launches the machine, waits for it                    |
| `bench`   | The machine     | Records the machine, builds, runs the session, keeps the results, stages the files |
| `stop`    | GitHub's runner | Terminates the machine, also when `bench` failed or was cancelled                  |
| `publish` | GitHub's runner | Copies each kernel's shape files the CDN lacks, when `bench` succeeded             |

Its inputs: `suite_args`, passed to every pass (such as `--family booleans/overlap/boxes` for a
short run), and `publish`, the CDN the files go to: `dev`, `prod` or `none`.

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
registry says. `results/build.txt` records every commit and tool version.

[`run-session.sh`](run-session.sh) then runs one pass per kernel with these settings:

| Setting  | Value                                                                                                                      |
| -------- | -------------------------------------------------------------------------------------------------------------------------- |
| Order    | springrep, arris, solidcore, OpenCascade: one pass each                                                                    |
| Workers  | 24, always passed                                                                                                          |
| Cores    | The suite's runner on the housekeeping cores; each adapter on an isolated core of its own, through [`pin.sh`](pin.sh)      |
| Adapters | Started by the registry's command, through [`adapters.py`](adapters.py)                                                    |
| Release  | A pass whose adapter reports another release than `kernels.json` pins is invalid                                           |
| Files    | Every pass runs with `--glb` and `--step` to a RAM disk; the suite draws a case after judging it, outside every timed call |

When the last pass ends, each kernel's files are moved to `assets/<kernel>/`. A pass that did not
finish cleanly fails the job, after its results are kept, so nothing is staged or published.
Otherwise `bench` sends the files to a private staging bucket, under a prefix that names the run.
`publish` copies each pass's files from there to `brep-conformance/<suite>/<kernel>/<version>/` on
the CDN, skipping those already there, with each file's type and an immutable cache lifetime. The
copy is made inside the storage service, so the files do not pass through GitHub. Each report names
that folder as `assets` ([formats.md](../doc/formats.md#assets)).

The machine runs kernel code, which must never be able to write to the CDN. So the machine holds one
cloud credential, and it can only write to staging: it cannot read, list or delete there, and it
cannot reach the CDN. The staging bucket deletes what it holds after a few days.

## What the bench job may not do

The `bench` job runs kernel code, so it declares no environment and has no `id-token` permission. It
cannot read the App's key, and nothing it starts can ask for a token that a cloud role would accept.
The job fails if it finds that it can.

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

[`app-token.sh`](app-token.sh) turns the App's key into a token that can only manage this
repository's runners.

`publish` reads these from the repository. It declares no environment, because the upload roles
trust this repository's `main` and `v*` tags with the default token subject. The buckets are
secrets, so the public logs mask them:

| Name                                | Kind     | What it is                 |
| ----------------------------------- | -------- | -------------------------- |
| `BREP_CONFORMANCE_ASSET_ROLE`       | Secret   | The prod CDN's upload role |
| `BREP_CONFORMANCE_ASSET_BUCKET`     | Secret   | The prod CDN's bucket      |
| `BREP_CONFORMANCE_ASSET_ROLE_DEV`   | Secret   | The dev CDN's upload role  |
| `BREP_CONFORMANCE_ASSET_BUCKET_DEV` | Secret   | The dev CDN's bucket       |
| `BENCHMARK_STAGING_BUCKET`          | Secret   | The staging bucket         |

`bench` reads the staging bucket's name too, for the one step that sends the files.
