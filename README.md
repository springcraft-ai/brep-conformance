# brep-conformance

**Results and documentation: <https://springcraft-ai.github.io/brep-conformance/>**

A kernel-neutral conformance suite for B-rep operations: generated cases, checks drawn from a
written contract, and a runner that drives any kernel through its adapter, run as a separate
process.

## Overview

The suite measures how correct a kernel is across a declared space of cases, as a score per test and
per contract clause. It is not run on every pull request: it runs in release, by hand.

- [`protocol/`](protocol/) is the crate `brep-protocol`: the operations' shapes, the `Kernel` trait,
  and its binding to the line protocol in [doc/api.md](doc/api.md#calling-a-kernel), `serve` for an
  adapter and `Remote` for a client.
- [`suite/`](suite/) is the crate `brep-conformance`: the case families
  ([`generate.rs`](suite/src/generate.rs)), the checks and runner ([`run.rs`](suite/src/run.rs)),
  the summaries ([`report.rs`](suite/src/report.rs)), and the one command that runs them on any
  adapter. Neither crate depends on a kernel.
- The suite's adapters each have their own repository, and [`kernels.json`](kernels.json), the
  registry, pins each to a reviewed commit and one release of its kernel:
  [OpenCascade](https://github.com/springcraft-ai/brep-conformance-occt), in C++, and the Rust
  kernels [anvil-brep](https://github.com/gntech/brep-conformance-anvilcad),
  [arris](https://github.com/springcraft-ai/brep-conformance-arris),
  [cad-kernel](https://github.com/springcraft-ai/brep-conformance-cad-kernel),
  [cadrs](https://github.com/springcraft-ai/brep-conformance-cadrs),
  [kentron](https://github.com/rsz269/brep-conformance-kentron),
  [next.BREP](https://github.com/springcraft-ai/brep-conformance-next-brep),
  [solidcraft](https://github.com/springcraft-ai/brep-conformance-solidcraft) and
  [springrep](https://github.com/springcraft-ai/brep-conformance-springrep).
- [`site/`](site/) is the public results site, built from the kernels' reports for GitHub Pages.

The suite scores kernels that compute exact geometry and measure it exactly, at its tolerance of
1e-7 mm. Kernels that measure through meshes are out of its scope.

A kernel is scored through an adapter, which passes each request to the kernel's own call and adds
nothing, so a score is the kernel's and not the adapter's. A kernel's developers know best how to
drive it. If an adapter drives a kernel wrongly, that is a fault in the adapter and not in the
kernel, and a correction from the kernel's developers is the most welcome fix. So that every kernel
is measured the same way, each adapter in an official run is reviewed against
[one rule](doc/contract.md#what-an-adapter-may-do), and [the registry](doc/process.md#the-registry)
pins the commit reviewed. The tests are the suite's and not the adapter's, so a stale adapter binary
still runs the current tests.

The suite is under the MIT licence or the Apache License, Version 2.0, at your option. Each
adapter's licence is in its own repository; it covers the adapter and not the kernel it drives:
each kernel keeps its own licence.

Cases describe primitive solids by parameters in a frame, and each adapter builds them with its own
constructors; some operands are booleans of others, built with the kernel under test. Every case
checks the clauses in [doc/contract.md](doc/contract.md); families with a closed form check it too,
and the suite's own tests check every closed form against its sampled estimate.

## Setup

The workspace's Rust toolchain, and an adapter for the kernel to score, built as its repository
says.

## Usage

```bash
cargo build --release -p brep-conformance
python3 nist/fetch.py  # NIST's STEP files, into nist/
target/release/brep-conformance --kernel arris --nist nist --out arris.json  # official
target/release/brep-conformance --adapter path/to/arris-adapter --skip-nist --out arris.json \
    --html arris.html
target/release/brep-conformance --adapter path/to/occt/build/occt-adapter --skip-nist \
    --out occt.json
cargo build --release --target wasm32-unknown-unknown -p brep-conformance-viewer
target/release/brep-conformance-report arris.json occt.json \
    --viewer target/wasm32-unknown-unknown/release/brep_conformance_viewer.wasm --out report.html
cargo test -p brep-protocol -p brep-conformance -p brep-conformance-viewer  # the suite's own tests
```

The report compares runs made on one suite version. Given runs on several, it compares those on the
newest and lists the others apart, as [doc/versioning.md](doc/versioning.md) says.

`--viewer` embeds the [viewer](viewer/src/lib.rs): the suite's own membership test, compiled to
WebAssembly, meshes a test's expected shape in the page when its view opens. A boolean case shows
its four results with the operands behind them, and a feature its solid before and after.

`--glb DIR` and `--step DIR` write every case's views as a GLB and the solids it shows as STEP
files, as the kernel draws them, failed cases' included ([formats.md](doc/formats.md#assets)). The
[Benchmark workflow](bench/README.md) writes them in each kernel's official pass and publishes them
to the CDN.

A run prints outcomes per test and failed checks per clause and writes every case's result to
`--out`. `--html FILE` also writes the run as a report page: one file that holds its own data and
opens without the network, with the viewer when `--viewer` names its WebAssembly.
`brep-conformance-report` makes the same page from results files, for several kernels side by side.
`--seed N` draws another set of cases; `--threads N` sets the parallelism, one worker per thread;
`--timeout S` sets the limit on one call, 120 s by default. The summary ends with each operation's
time in units of the run's calibration workload. `--peer "<command>"` starts a second kernel's
adapter to read the STEP files this one writes, reported as interop apart from the score.
`--nist DIR` adds a case of `step/read-nist` for each of NIST's STEP files there, which
[`nist/fetch.py`](nist/fetch.py) downloads into [`nist/`](nist/NOTICE). A run without them must pass
`--skip-nist`.
`--family A,B` runs only those tests: each a test path, a directory such as `fillets/`, or a
family's name.

A kernel's results change only when the tests (the suite version or seed), its adapter, or the
kernel's release or configuration change. Run it once per such change, keep its results under
`target/brep-conformance/` named by what they came from
(`<kernel>-suite2.0.2-protocol1.1.0-<kernel release>.json`), and give the report that file.

## Additional Information

See [doc/intro.md](doc/intro.md).

The suite is under either the MIT licence ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option. The report embeds three.js, under
its own MIT licence: [suite/assets/NOTICE](suite/assets/NOTICE). NIST's STEP files are
downloaded, not held here: [nist/NOTICE](nist/NOTICE).
