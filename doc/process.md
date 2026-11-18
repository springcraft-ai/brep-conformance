# Process

How the suite's specifications change: when a function enters [api.md](api.md), and where the ideas
for new ones come from. This document governs the specs; it is not part of them.

## Adding a function

1. **A test needs it.** The functions are the smallest set the tests call; a family that needs a new
   one brings its definition with it.
2. **Its result is decidable.** Its definition is exact enough for the suite to decide, for any
   point, whether the result holds it. A function without that is not yet specified.
3. **Two kernels implement it.** It is interoperable once two independent kernels meet the contract
   on its tests. Until then it is specified but not interoperable.

## The registry

[`kernels.json`](../kernels.json) lists the kernels the suite scores officially. Each entry names
the kernel as its adapter's `hello` does, the kernel release that adapter must report, the kernel's
licence and source, and its adapter: the repository, the commit the suite's maintainers reviewed as
pass-through against [the adapter rule](contract.md#what-an-adapter-may-do), the adapter's licence,
the commands that build it, and the command that starts it, with `{dir}` for its checkout.
`reviewers` names the kernel's own maintainers who review its adapter, where they do.

An official run is `brep-conformance --kernel NAME`. The runner reads the registry it was built
with, checks the adapter out at the pinned commit, refuses a checkout with changed files, builds it,
refuses an adapter that reports another kernel or release, and records the repository and commit in
the results. The site lists only the kernels in the registry.

A merge to an adapter's repository changes no official run until a pull request here moves its pin.
The suite's maintainers review that pull request against the adapter's changes since the last pinned
commit, with the kernel's reviewers where it has them. A new kernel release moves `version` with the
adapter commit that builds it.

## Two kinds of test

- **Exact.** The definition fixes the result as a set of points, and the test checks the kernel's
  answer against it: a closed form for its volume and area, or the suite's own sample. Most tests
  are exact.
- **Sound.** The definition leaves the result open within limits, where CAD tools legitimately
  differ: the corner where fillets of unequal radii meet is one. The test checks that the kernel
  completes, that its result is valid and covers what was asked, and that it keeps the properties
  every acceptable answer has; it does not check one answer. Sound cases count in the same total as
  exact ones.

Behaviour no definition limits, such as how a tool names faces or which of two valid corner patches
it prefers, is not tested.

## What is out of scope

- **Annotations.** STEP's product manufacturing information (dimensions, tolerances, datums) is data
  about a part, not a set of points, so no function here defines it and no test reads it.

## Where new functions come from

What to specify next comes from what CAD users expect: the features common to modelling tools and
the functions common to kernel APIs. Those are evidence of what is standard, never the definition;
every definition in the specs is the suite's own, written as a set of points.
