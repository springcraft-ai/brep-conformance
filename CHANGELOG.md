# Changelog

Every release of the suite, newest first. [doc/versioning.md](doc/versioning.md) says what each
number promises and what an entry records.

## 2.0.2 (unreleased)

Verdicts and scores on 2.0.1 stand, except on the three tests below, where a case can only move from
failed to passed.

### Tests fixed

- `queries/hidden-lines-of-a-cylinder`, `queries/hidden-lines-of-a-drilled-box` and
  `queries/hidden-lines-of-a-sphere`: B38 could measure a drawing's vertex just short of a whole
  circle's seam by its distance round the circle to the seam, about `r·2π·(1 − t)`, instead of 0.
  An exact drawing with a vertex there failed. The search for the nearest point now runs across the
  seam. Results move from failed to passed only.

## 2.0.1 (2026-11-22)

Verdicts and scores on 2.0.0 stand. Case times do not compare with 2.0.0's.

### Timing

- A case's `time_ms` leaves out the B17 round trip through STEP, which only kernels that read and
  write STEP run, so every kernel is timed on the same work; its time is the case's new `step_ms`,
  null where none ran. The site shows the median of `step_ms` beside the median case time. No
  verdict changes.

### Reporting

- With `--bench DIR --run ID`, the flat report records the Benchmark workflow's run (`run.id`),
  whether the pass ran every test (`run.complete`), the suite's and the adapter's commits, and the
  machine (`environment.machine`). Without the options these fields are null or absent. No verdict
  changes.

## 2.0.0 (2026-11-22)

Results on 1.0.0 do not compare with results on 2.0.0: every case has a new id, and two checks score
a refused blend differently.

### Protocol

- Protocol 1.1.0. [`display`](doc/api.md#display) gains four fields: `edge_faces`, the faces each
  edge bounds; `edge_turns`, the least and greatest signed angle between those faces' normals along
  it; `edge_split`, which flags an edge between two faces on one surface; and `seams`, a list of
  their own. Split edges, left out before, now come in `edges`, flagged. Adapters written to 1.0.0
  run on this release, and a client reads the fields they omit as not known.
- The Rust crate's `Display` holds its edges as `DisplayEdge` and has `seams`. The C++ binding's
  `Display` changes the same way, which makes that binding 2.0.0.
- [`display`](doc/api.md#display) now says what a vertex's normal is where the surface has no one
  normal, as at a cone's apex: each triangle that meets there has its own vertex there, with a
  normal that the surface has along that triangle. It also says that `edges` holds every edge
  between two faces, including an edge where two faces meet smoothly, and it leaves to the viewer
  which of them count as creases.
- [`shell`](doc/api.md#shell) is defined by faces moved inward instead of by distance to the
  boundary. A wall ends flush with each opening, and the cavity is sharp inside each concave edge.

### Scoring

- Contract 2.0.0. B42 adds a cone to the solids it displays. A vertex at the cone's apex must have,
  in each triangle that uses it, one of the normals that the cone has across that triangle, to
  within 1e-3.
- B42 has a fourth check, of what each edge says: the faces it names, the turn between them against
  its closed form to within 1e-3, and the flag on an edge a split made. Each of these is judged only
  where the display gives it, and seams are not scored. A display that gives none of them, as from
  an adapter written to protocol 1.0.0, passes this check as not reported. B42 also displays a box
  with its upright edges rounded, where faces meet smoothly, and a box with one side in two faces.
- B31 and B39 no longer run when the kernel refused the blend. Before, a kernel that offers `normal`
  or curvatures at probes got a failed case for a refused blend. Now the case is a refusal, as
  every other check of a blend case already scored it.
- Case ids changed for every case of the default seed. A case id now hashes its key rounded to nine
  significant digits, with any magnitude below a picometre as zero, and the generator uses the same
  maths on every platform. So the same seed draws the same cases and ids on every platform. The
  cases themselves are the same as before to within their last bits, and no verdict changes.

### Reporting

- The results file records when the run started (`started_unix`), how long it took (`wall_ms`) and
  the time limit of one call (`time_limit_ms`). Each case records its steps (`ops`) and whether a
  newly started kernel answered it (`cold`), so that its time includes that kernel's first calls.
  The flat report carries the same fields. No verdict changes.

### Tests added

- 3617 cases in 250 tests for the default seed: 130 cases in 12 tests more than 1.0.0.
- `queries/display-of-a-cone`, `queries/display-of-a-filleted-box` and
  `queries/display-of-a-split-box`: the cone, the rounded box and the box with a split side in B42.
- Edits against curved faces: `modify/delete-faces-of-turned-parts`,
  `modify/delete-faces-taking-material-away`, `modify/offset-boss-with-a-root-round`,
  `modify/offset-cone-to-its-apex`, `modify/offset-torus`, `modify/shell-ball-or-cone`,
  `modify/shell-frustum`, `modify/shell-l-prism-open-at-its-step` and `modify/shell-rounded-puck`.
- `step/read-nist`: one case for each of NIST's STEP files, with `--nist DIR`. These cases are not
  in the default seed, and the Benchmark workflow runs them by default.

### Tests fixed

- Every test with a blend whose kernel refused the blend, in B31 and B39, as **Scoring** says.
  Results move from failed to refused.

## 1.0.0 (2026-11-18)

The first public release, and the version every later entry is measured against.

### Protocol

- Protocol 1.0.0, as [doc/api.md](doc/api.md) defines it.

### Scoring

- Contract 1.0.0, as [doc/contract.md](doc/contract.md) defines it. A case passes when every check
  holds at the suite's tolerance of 1e-7 mm, and every case counts once in one total, whether it is
  exact, sound or a rejection.

### Tests added

- 3487 cases in 238 tests for the default seed, under `booleans/`, `constructions/`, `fillets/`,
  `chamfers/`, `modify/`, `queries/`, `step/` and `parts/`.
