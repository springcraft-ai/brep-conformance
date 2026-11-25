# Update an adapter to a new release of its kernel

You are moving one brep-conformance adapter, a small program that answers the suite's protocol with
its kernel's calls, to a newer release of that kernel. You do the whole update: the pins, the build,
and what the release changes. The workflow then builds your change itself, runs part of the suite on
it, and opens a pull request with your report and its own results.

## What you have

The working directory holds:

| Path               | What it is                                                               |
| ------------------ | ------------------------------------------------------------------------ |
| `adapter/`         | The adapter's repository at its `main`; the only files you may change    |
| `suite/`           | brep-conformance at the commit this workflow ran from, already built     |
| `work/plan.json`   | The adapter's build commands and binary, and the release to move to      |
| `work/kernel.json` | The kernel's registry entry: its licence, its source repository          |
| `work/old/`        | Each kernel crate's source at the release the adapter pins now           |
| `work/new/`        | The same at the release to move to                                       |

`work/plan.json` gives `from` and `to`, the two releases; `pins`, each of the kernel's pins with the
versions, and for one from git the commits, it moves between, and the licence each release
declares; and `licence_changed`, each pin whose licence differs between the two.

A kernel the adapter takes from conda-forge, as a line `package=X.Y.Z` of its `build.sh`, has
nothing in `work/old/` or `work/new/`: its source is too big to unpack whole. Start from its release
notes. Then clone each release's tag shallowly into `work/`, never inside `adapter/`, as
`git clone --depth 1 --branch <tag> <repository> work/<tag>`, with the repository from
`work/kernel.json`, and compare the two.

Read these first, in this order:

1. `suite/CLAUDE.md`, the rules an agent most often breaks. Every one of them applies to you.
2. In `suite/doc/contract.md`: [What an adapter may do](../doc/contract.md#what-an-adapter-may-do),
   [Outcomes](../doc/contract.md#outcomes) and [Tolerance](../doc/contract.md#tolerance). The rest of
   that file is the checks; read a clause only when a test of it changes.
3. `adapter/README.md`, which says what the adapter supports, and what it leaves out and why.
4. `adapter/src/`, or the adapter's C++ source, and its build files.

`suite/doc/api.md` gives each function's arguments and answer. Read a function there before you
answer it with a kernel call.

## What to do

You have 90 minutes, and then you are stopped wherever you are. Do the steps in this order, so
that a run cut short still leaves the pins moved and the adapter building, and write
`work/report.md` as you go, not at the end.

1. **Read what the release says it changed.** Start from the kernel's changelog or release notes, in
   `work/new/` or at its repository in `work/kernel.json`, for every release after `from` up to and
   including `to`. Then diff `work/old/` and `work/new/`: public items, their docs and their tests.
2. **Check the licence.** The workflow already reports what `licence_changed` lists. Look at the
   licence files in the release as well: if they changed in a way `licence_changed` does not show,
   such as a new file under another licence, say so under `## Licence`. Go on with the update.
3. **Move the pins**, each to the release `pins` gives it:
   - a crate from crates.io is pinned exactly, `=X.Y.Z`: set the version, then run
     `cargo update -p <crate> --precise <version>`;
   - a crate from git is pinned by `rev`: set it to the new commit, at least its first 8
     characters, then run `cargo update -p <crate>`, and move any link to the release's tag;
   - a package from conda-forge is the line `package=X.Y.Z` in `build.sh`: set the version.

   Move the version the adapter reports to `to`, where the adapter writes it out rather than reading
   its kernel's. Move no other dependency, except `brep-protocol` within its major version if the
   new release needs it. The workflow checks each pin against `work/plan.json`.
4. **Make it build.** Where the release changed a call the adapter uses, find the new call in
   `work/new/` and use it. Change what the adapter does only as far as the kernel's change forces.
5. **Use what the release adds.** For each function the adapter leaves out, check whether the
   release now has one public call of the protocol's shape for it. Where it does, answer the
   function with that call and add it to `supports`.
6. **Drop what the release fixed.** A refusal in the adapter that names a kernel limit the release
   no longer has goes, as long as one call now answers the function.
7. **Check the settings.** Where the release adds a tolerance setting, changes a default, or adds a
   more accurate way to integrate, follow [Tolerance and settings](#tolerance-and-settings).
8. **Check the refusals.** Where the release gives its errors new kinds, map them as
   [Refusals](#refusals) says.
9. **Update the README** so it describes the adapter as it now is: the kernel's version where it
   names one, what is supported, and what is left out and why.

## What an adapter may do

The contract is the rule. This is how it applies to code you are likely to write.

### Strict: whatever makes a solid or a number the suite scores

A constructor, a boolean, a blend, a face edit, a transform, a measurement and a query are each one
call of the kernel. Each of these breaks that rule, however reasonable it looks:

- **Building a function from several calls.** An inertia tensor put together from two mass
  property calls, an area summed face by face, a split made from a decomposition and a regrouping, a
  distance worked out from a projection and a classification.
- **A number of your own that decides something.** A threshold, a tolerance, an epsilon or a count
  that the kernel does not have, such as a law given as radii sampled at a count you chose, or a
  matrix taken apart with a cut-off of your own to choose between two kernel calls.
- **Choosing between answers.** A retry with other settings, a vote, and a fallback to another call
  are each a choice the kernel did not make. So is choosing between edges or faces for a point:
  see [Points](#points).
- **Repair.** A call after the kernel's call that fixes its result, such as turning a solid round
  or healing it.
- **Working around a kernel bug.** A kernel that gets a case wrong gets it wrong in its score.

Choosing a kernel call by the exact form of the argument is translation, not a decision: a negative
distance to one call and a positive one to another, or one size for every edge to a call that takes
one size. A cut-off is never exact form.

### Translation: putting the request into the kernel's form

The contract's first four allowed things: a profile's segments as the kernel's curves, a frame as
its placement, a path as its spine, a solid built at the origin and then placed with the kernel's
own transform, the solids of a split sorted by side, a point turned into the kernel's address of an
edge or a face through its distance query at 10·ε, the empty solid read from the kernel's own way of
saying a result is empty, and a solid made again where the kernel consumes its operand. The
protocol's crate already has helpers for some of this, such as `view_axes` and a profile's
`scaled`; use them rather than writing your own.

### Points

`api.md` says, function by function, what a point within 10·ε of the boundary means. Follow it, not
a rule of your own:

- A point names the edge, or the face, that passes within 10·ε of it; the tests place such points
  on one edge or face. Find it through the kernel's distance query, and refuse a point that names
  none as `invalid`.
- `normal` and `curvature` answer null at a crease and more than 10·ε off the boundary, and give
  the normal two faces share where they meet smoothly. Where the kernel says which, pass it on.
  Where the kernel answers on one face and the point lies on several, the call is `unsupported`:
  never take the nearest.
- `classify` may answer `in`, `out` or `on` for a point between ε and 10·ε of the boundary.
- The suite itself merges a mesh's vertices, a section's pieces and a ray's crossings within 10·ε.
  The adapter merges nothing.

### Looser: drawing what the kernel made

`mesh`, `display`, `section` and `project` draw a solid; they make nothing. For these the adapter may
make several calls that read the solid without changing it, and work out what follows from their
answers alone:

- the kernel's mesher, with each face given its own vertices and the kernel's normals at them, the
  triangles turned to face outward, and several bodies' meshes put into one list;
- the faces that use each edge, from the kernel's topology: a seam where one face uses the edge
  twice, and a split edge where the kernel says two faces lie on one surface;
- the turn at an edge, from the two normals the kernel gave at each of its vertices;
- a curve's points, by the kernel's own sampler where it has one.

The adapter still moves no point, makes no normal and compares no surfaces with a number of its own.
What the kernel cannot say, it leaves out: an edge with no turn has none, and a kernel with no
display leaves `display` unsupported. Never write a mesher or a sampler of your own.

### Refusals

- **`unsupported`** means the kernel has not built this, for any arguments or for arguments of this
  sort. It never means the adapter has not been written yet: where translation would answer it,
  write the translation.
- **`invalid`** is an input that has no answer: a left-handed frame, sizes that do not match, a
  point that names no edge.
- **`failed`** is a call that should have worked and did not.

Map the kernel's errors by their type or kind, never by matching their text. Where the release
gives an error a kind it did not have, such as a typed invalid input where there was a bare string,
use it.

### Tolerance and settings

Set every tolerance setting the kernel has to the suite's tolerance, 1e-7 mm, or to the tightest
value the kernel allows. `hello`'s tolerance is the loosest one that still governs the kernel's
results. Measure with the kernel's most accurate integration.

`configuration` names, in one short line, the settings that matter that the adapter changed from
the kernel's default: tolerances, the accuracy of integration, a mesh's angular deflection. It is not
a list of every option.

## Scope

Change what the release changes. Where you find code that already breaks the contract, and the
release does not touch it, leave it and say so under `## Contract notes` in your report. A person
decides those, apart from the release.

- **Change only `adapter/`, and only its source, build files, README and manifests.** Do not touch
  `.github/`, the licence, the toolchain file or the contributing guide, and add no file outside
  the source.
  Change `Cargo.lock` only through cargo.
- **Never edit the suite, and never take an expected value from a kernel.**
- **Name no proprietary CAD product** in code, comments, the README or your report.
- **No AI attribution** anywhere: no co-author lines, no "generated with" notes.
- **Do not commit or push.** The workflow takes your changes from the working tree.

## Checking your work

Build the adapter and run the suite as the workflow does:

```sh
suite/bench/adapter-build.sh adapter work/plan.json
suite/target/release/brep-conformance --adapter "adapter/$(jq -r .binary work/plan.json)" \
    --threads 4 --skip-nist --out work/check.json --family <family>,<family> > work/check.txt
```

Never run the whole suite: it takes hours and the workflow does not need it. Run the families in
`suite/bench/smoke.txt` and the families whose functions you changed, in one `--family`. A case that
fails is yours to explain: either the adapter drives the kernel wrongly, which you fix, or the
kernel gets it wrong, which you leave and report.

Write the families you changed, one per line, to `work/families.txt`. The workflow runs those and
the smoke families itself, and the pull request shows its run, not yours.

## Your report

Write `work/report.md` for the pull request's description. Keep it short, and leave out any heading
with nothing under it:

- `## Licence`: a licence change in the release that `licence_changed` does not show.
- `## Summary`: what you changed and why, one bullet per build fix, per function newly supported,
  per refusal dropped and per setting changed. Name each kernel call you used.
- `## Kernel regressions`: each case that got worse because of the kernel, and what it does wrong.
- `## Contract notes`: code the release does not touch that breaks the contract.
- `## Not done`: anything you could not finish, so a person can pick it up.

Do not include a results table: the workflow adds one from its own run.
