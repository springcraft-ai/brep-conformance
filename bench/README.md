# bench

How an official benchmark session gets its machine: one bare-metal instance that is started for the
session, runs one job, and is terminated.

## Overview

The [Benchmark workflow](../.github/workflows/benchmark.yml) is started by hand on `main`
(`workflow_dispatch`). It has three jobs:

| Job     | Runs on         | What it does                                                         |
| ------- | --------------- | -------------------------------------------------------------------- |
| `start` | GitHub's runner | Registers a one-time runner, launches the machine, waits for it      |
| `bench` | The machine     | Checks it has no identity token, and records the machine's settings  |
| `stop`  | GitHub's runner | Terminates the machine, also when `bench` failed or was cancelled    |

Today `bench` only records the machine. Running the suite on each kernel is added to that job.

The machine ends in three independent ways: the `stop` job, a lifetime timer armed at first boot,
and a shutdown that terminates the instance.

## The machine's settings

[`first-boot.sh`](first-boot.sh) is the whole configuration of the machine, and it is what makes
timings comparable between sessions:

- One logical CPU per core: simultaneous multithreading is off.
- The clock is fixed at the processor's base frequency: turbo is off, and the governor is
  `performance`.
- The first 24 cores run the operating system and the suite. The others are isolated, for one
  adapter process each.
- Deep idle states and background package work are off.

It writes what it found to `/etc/benchmark-runner/environment`, which the `bench` job uploads as
the `machine` artifact.

## What the bench job may not do

The `bench` job runs kernel code, so it declares no environment and has no `id-token` permission.
It cannot read the App's key, and nothing it starts can ask for a token that a cloud role would
accept. The job fails if it finds that it can.

## Setup

The workflow reads these from the `benchmark` environment, which allows only `main`:

| Name                        | Kind     | What it is                                     |
| --------------------------- | -------- | ---------------------------------------------- |
| `BENCHMARK_APP_CLIENT_ID`   | Variable | The GitHub App that registers the runner       |
| `BENCHMARK_APP_PRIVATE_KEY` | Secret   | That App's key                                 |
| `BENCHMARK_ROLE_ARN`        | Variable | The role that may launch and terminate         |
| `BENCHMARK_AWS_REGION`      | Variable | The region of the machine                      |
| `BENCHMARK_LAUNCH_TEMPLATE` | Variable | The launch template's name                     |
| `BENCHMARK_SUBNET_IDS`      | Variable | Subnets to try, in order, separated by commas  |

[`app-token.sh`](app-token.sh) turns the App's key into a token that can only manage this
repository's runners.
