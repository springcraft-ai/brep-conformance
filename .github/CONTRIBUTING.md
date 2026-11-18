# Contributing

How to propose a change to the suite, and what a change must be for a maintainer to merge it.

Issues and pull requests are welcome. A Springcraft maintainer reviews every pull request, ours
included, and nothing reaches `main` without that review.

## What belongs here

- **A wrong test.** A case whose expected value is wrong, or a check that judges a correct answer as
  a failure. Say which case, and why the expected value is not the exact one.
- **A new test or function.** [process.md](../doc/process.md) says when one enters the specs: a test
  needs it, its result is decidable, and two kernels implement it. Open an issue first.
- **The runner, the report and the documents.**

A problem with how a kernel is driven can be opened here: a maintainer moves it to that kernel's
adapter repository, which [kernels.json](../kernels.json) names, if the fault is there. A fix to an
adapter is a pull request in its repository. A problem in a kernel belongs with the kernel.

## Where the work comes from

The suite's value is that no kernel and no vendor decided its answers. Every contribution keeps that
true.

- **Your own work, or work you may submit.** You wrote the change, or you have the right to submit
  it under this repository's licence.
- **Nothing from proprietary sources.** No part of a change derives from proprietary source code,
  from decompiled or disassembled code, or from documentation you hold under a non-disclosure
  agreement or a licence that restricts its use. If you have seen such material for the thing you
  are changing, leave that change to someone who has not.
- **No expected value from a kernel.** An expected value is a closed form or the suite's own sample,
  as [contract.md](../doc/contract.md) requires. Running a kernel to find out what the answer
  "should" be makes that kernel the standard.
- **Cite what you used.** An algorithm from a paper or a book names it where the code uses it.
- **No proprietary kernel is run.** No workflow, test or benchmark in this repository runs one, and
  no result here comes from one.

## Names

Do not name a proprietary CAD kernel or CAD program, its vendor, or its API, in code, tests,
comments, commit messages or documents. Describe the behaviour: "a kernel that sews before it
checks", not the product that does it. The suite holds no list of such names, plain or hashed, and a
change must not add one.

The rule is about CAD software only. Open-source kernels the suite scores are named where the suite
needs to say what it ran, and the tools the project is built and hosted with are named as usual.

## Files

- No compiled binaries and no vendor SDK files.
- No CAD files outside a fixture folder. A fixture is one the suite wrote, or one whose terms allow
  it to be redistributed, with a notice beside it, as [NOTICE](../nist/NOTICE) is.
- No secrets. A key or a token in a commit stays in the history after it is removed.

## Dependencies

Dependencies are under permissive licences only: [`deny.toml`](../deny.toml) lists them, and
`cargo deny check licenses sources` checks every pull request. The GPL and the LGPL are refused. A
dependency comes from crates.io, not from a git repository.

## Licence of contributions

Contributions are accepted under this repository's [MIT licence](../LICENSE).
