# Update an adapter to a new release of its kernel

You are updating one brep-conformance adapter, a small Rust program that answers the suite's
protocol with its kernel's calls, to a newer release of that kernel. The workflow has already
moved the kernel's pins and the version the adapter reports. Your job is everything that move does
not do by itself.

## What you have

The working directory holds:

| Path               | What it is                                                             |
| ------------------ | ---------------------------------------------------------------------- |
| `adapter/`         | The adapter's repository, pins moved; the only files you may change    |
| `suite/`           | brep-conformance at the commit this workflow ran from, already built   |
| `work/bump.json`   | The kernel crates moved, from which version to which                   |
| `work/before.json` | The suite's results for the adapter at the old release                 |
| `work/after.json`  | The results with the pins moved, if the adapter built; absent if not   |
| `work/changes.md`  | What changed between the two, if both exist                            |
| `work/old/`        | Each kernel crate's source at the old release, as published            |
| `work/new/`        | The same at the new release; dependencies are in cargo's registry      |
| `work/kernel.json` | The kernel's registry entry, with its source repository                |
| `work/plan.json`   | The commands that build the adapter, and the binary they make          |

A kernel the adapter links from a Python package, as `pyproject.toml` pins it, has nothing in
`work/old/` or `work/new/`: its source is too big to unpack whole. Clone its repository, from
`work/kernel.json`, and compare the two releases' tags there, starting from the release notes.

Read these first:

- `suite/CLAUDE.md`, the rules an agent most often breaks. Every rule there applies to you.
- `suite/doc/contract.md`, what an adapter may and may not do.
- `adapter/README.md`, which says what this adapter supports, and what it leaves out and why.
- `adapter/src/`, the adapter.

## What to do

1. **Make it build.** If `work/after.json` is absent, the new release changed an API the adapter
   uses. Find the new call in `work/new/` and use it. Change what the adapter does only as far as
   the kernel's change forces you to.
2. **Use what the release adds.** Diff `work/old/` and `work/new/`: public items, their docs and
   their tests. For each function the adapter leaves out (its README lists them, and `supports`
   omits them), check whether the new release has one public call of the protocol's shape for it.
   Where it does, answer the function with that call and add it to `supports`.
3. **Drop what the release fixed.** A refusal or a workaround in the adapter that names a kernel
   limit the new release no longer has goes, as long as one call now answers the function.
4. **Check what got worse.** For each test in `work/changes.md` whose result got worse, find out
   whether the adapter drives the new release wrongly, and fix that if so. A kernel regression is
   the kernel's: leave it, and say so in your report.
5. **Update the README** so it describes the adapter as it now is: the kernel's version where it
   names one, what is supported, and what is left out and why.

## Rules

- **Pass-through only.** Each function is one call of the kernel. Never build a function out of
  several calls, never work around a kernel bug, and never add a tolerance or a retry the kernel
  does not have. Anything the kernel has no one call for stays unsupported.
- **Change only `adapter/`, and only its source, build files, README and manifests.** Do not touch
  `.github/`, the licence, the toolchain file or the contributing guide. Change `Cargo.lock` only
  through cargo, and `uv.lock` only through uv.
- **Keep the pins exact.** Do not move a dependency the workflow did not move, except
  `brep-protocol` within its current major version if the new kernel needs it.
- **No expected value from a kernel.** Never edit the suite.
- **Name no proprietary CAD product** in code, comments, the README or your report.
- **No AI attribution** anywhere: no co-author lines, no "generated with" notes.
- **Do not commit or push.** The workflow takes your changes from the working tree, builds them and
  runs the whole suite on them itself.

## Checking your work

Build and run the suite as the workflow does:

```sh
suite/bench/adapter-build.sh adapter work/plan.json
suite/target/release/brep-conformance --adapter "adapter/$(jq -r .binary work/plan.json)" \
    --threads 4 --out work/check.json --family <prefix>[,<prefix>...] > work/check.txt
```

A whole run takes a long time, and the workflow makes one after you finish, so run only the
families you changed.

## Your report

Write `work/report.md` for the pull request's description. Keep it short:

- `## Summary`: what you changed, and why, as bullets: one per function newly supported, per
  build fix, per refusal dropped. Name each kernel call you used.
- `## Regressions`: each test that got worse, and whether it is the kernel or the adapter.
- `## Not done`: anything you could not finish, so a person can pick it up. Leave the heading out
  if there is nothing.

Do not include a results table: the workflow adds one from its own run.
