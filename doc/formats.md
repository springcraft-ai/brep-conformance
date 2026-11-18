# Formats

This document describes the files a run writes and reads, and how cases are named.

## Tests and case ids

Every case belongs to a test. A test is named by a path: a directory for the feature it exercises
and a name for the situation, such as `booleans/touching/sphere-in-a-cone` or
`fillets/rejects-radius-past-half-width`. The top-level directories are `booleans`, `constructions`,
`fillets`, `chamfers`, `modify`, `queries`, `step` and `parts`. `parts` holds whole parts built step
by step, such as gears. Only `booleans` has a second level, which says how its two solids meet. A
test whose name starts with `rejects-` passes only when the kernel refuses its input, and it sits
with the feature it refuses.

Cases are drawn by families. A family is a generator, and it may feed several tests. A case id is
its family's name followed by a hash of what the family drew, for example
`box-box-1a2b3c4d5e6f7a8b`. An id is opaque: it names one case of one suite release, and nothing
outside the suite can work out an id from a case's parameters. The same seed draws the same cases on
every platform. A family that changes what it draws for a seed bumps its version, which gives its
cases new ids. A change that keeps the draws keeps the ids, and moving a case to another test
changes neither.

## Results

A results file is one JSON object per run. This is format 1, which the object gives as `format`.
Adding a field later does not change the number, so a reader ignores fields it does not know.
Removing a field, or changing a field's meaning, raises the number.

The run's fields:

| Field            | Value                                                                                |
| ---------------- | ------------------------------------------------------------------------------------ |
| `format`         | The version of this format, an integer                                               |
| `suite`          | The suite's release, numbered as [versioning.md](versioning.md) says                 |
| `contract`       | The version of [contract.md](contract.md) its checks implement                       |
| `tolerance`      | The suite's linear tolerance in mm, which every check is judged to                   |
| `kernel`         | `name`, `version`, declared `tolerance` and `configuration`, as `hello` gave them    |
| `adapter`        | On an official run (`--kernel`): the adapter's `repo` and the `commit` it ran        |
| `unsupported`    | The functions of the suite's protocol version that the kernel's `hello` leaves out   |
| `stand_ins`      | For each primitive the kernel has no constructor for, the function sent in its place |
| `environment`    | The machine's `os`, `arch` and `cores`, and the run's `threads`                      |
| `unix`           | When the run ended, in seconds since 1970                                            |
| `calibration_ms` | The time of the calibration workload on this kernel and machine                      |
| `cases`          | One object per case, in generation order                                             |
| `nist`           | With `--nist`, one row per file read; apart from the score                           |

A kernel's `version` is a release number, or a digest of the source it was built from. `stand_ins`
looks like `{"sphere": "revolve"}`, and [contract.md](contract.md) gives the solids sent.

A case's fields:

| Field        | Value                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`         | The case's id                                                                                                                                                                                                 |
| `test`       | The path of the test it belongs to                                                                                                                                                                            |
| `family`     | The family that drew it                                                                                                                                                                                       |
| `kind`       | `exact`, `sound` or `rejection`: how it is judged, as [process.md](process.md) describes                                                                                                                      |
| `outcome`    | `pass`, `fail`, `unsupported`, `refused`, `timeout` or `crash`                                                                                                                                                |
| `result`     | The tier it passes at, `1e-7`, `1e-6`, `1e-5` or `1e-3`; else its outcome                                                                                                                                     |
| `why`        | Where the result is not `1e-7`: the failed clause and its detail, or the error                                                                                                                                |
| `failure`    | Where a check failed: `geometry`, or `measurement` when every failed check weighed one                                                                                                                        |
| `over`       | On a failed case: its worst measure's error as a multiple of its bound                                                                                                                                        |
| `a`, `b`     | The operands, as [Operands](#operands) writes them                                                                                                                                                            |
| `feature`    | On a blend or an edit: the `act`, the `points` naming edges or faces, the `size`                                                                                                                              |
| `checks`     | The checks made, below                                                                                                                                                                                        |
| `ops`        | The steps run, below                                                                                                                                                                                          |
| `error`      | Where the case's own solids could not be made: the kernel's reason                                                                                                                                            |
| `interop`    | With `--peer`: whether the second kernel read this one's STEP file as the same solid                                                                                                                          |
| `time_ms`    | The sum of its steps' times                                                                                                                                                                                   |
| `asset`      | The name of its shape files                                                                                                                                                                                   |
| `sound`      | `true` on a case judged as sound                                                                                                                                                                              |
| `asset_step` | `true` when the case has one expected shape, which a `.step` file holds ([Assets](#assets))                                                                                                                   |
| `exports`    | With `--glb` or `--step`: the keys written as `glb` and `step`, what could not be drawn or written as `errors`, and as `skipped` a ghost the kernel could not cut or a refused case's input it would not make |

In a feature, `b` is the shape the act must give, and a draft also has its `pull`. A case that must
be refused passes only when it is refused.

A step is one entry of `ops`. Its fields:

| Field     | Value                                                                       |
| --------- | --------------------------------------------------------------------------- |
| `op`      | The step's label, for a reader: a function's name, or words such as `a ∪ b` |
| `outcome` | `done`, `refused`, `unsupported`, `timeout` or `crash`                      |
| `ms`      | Its time                                                                    |
| `error`   | Where it did not finish: for a refusal its kind, then the kernel's reason   |

A step may time several calls, such as a boolean and the measures taken of its result. `op` is
therefore a label and not always one function's name, and no reader should parse it. A step that
finished may also give what it found: the result's `volume`, `area`, `centroid` and checker
`issues`, a `distance`, or the number of `clashes` in an assembly. For `classify a ∩ b` it gives, as
`mismatched`, the number of sample points the kernel placed differently from the exact operands, and
the `first` few of them, each with its `point` and whether it is `inside`.

A check has a `clause`, whether it held as `ok`, the `tier` it holds at, and what it measured and
counted:

- `measures`: each entry has a `measure` (`volume`, `area`, `distance`, `first moment` or
  `second moment`) in its `unit`, the `error`, and the `bound` the error had to stay within. Where
  the check compares two values, the entry also has the kernel's value as `got` and the expected
  value as `want`.
- `counts`: each entry has a `count` of something (`checker issues`, `sample points placed wrongly`,
  `pieces`, `closed curves`) and how many there were as `got`. It also has either the `most` there
  may be, or the number there must be as `want`.
- `detail`: a few words saying what the check was of, followed by a sentence made from those
  numbers. A check that is only a yes or no, such as whether an input was refused, has the sentence
  only.

## Reports

A run's `--html FILE` option, and the command `brep-conformance-report RESULTS.json... --out FILE`,
each write a report page for reading on the machine that ran the suite. The page is one HTML file
that holds its results, its styles and its scripts, and it asks the network for nothing. It is a
view of the results file, not a format to read back.

`brep-conformance-report RESULTS.json --flat report.json` writes one run in the form the results
site reads. It holds `format` (the same number as the results file's), `kernel`, `adapter`, `suite`,
`run` (`unix`, and `verified` when the suite's own benchmark machine ran it), `environment`,
`unsupported`, `stand_ins`, a `summary` (the number of `cases`, the headline `pass_1e7` and the
count of each result) and `cases`. `cases` has one row per case, with `id`, `test`, `operation`,
`kind`, `result`, `why`, `failure`, `time_ms`, `asset`, its test's `group_median_ms`, and
`group_low_confidence`.

## Operands

Each operand is a constructor call written as data: the request
[api.md](api.md#requests-and-replies) sends for it, with the constructor's name as `kind` in place
of `cmd`, for example `{"kind": "extrude", "frame": …, "profile": …, "height": …}`. Its fields are
the function's parameters in [api.md](api.md).

An operand may instead be built from other operands. A built operand has kind `boolean`, an `op` of
`union`, `intersection` or `difference`, and operands `a` and `b`, each of which is a primitive or
another boolean. The runner builds it with the kernel under test, innermost first, so a case feeds
results into the next operation as modelling does. Its id hashes the operation round its operands'
parameters.

## Assets

A run with `--glb DIR` or `--step DIR` writes each case's shape files there, as the kernel under
test draws them. Failed cases' files are included ([`export.rs`](../suite/src/run/export.rs)):

| Key                        | Holds                                                 |
| -------------------------- | ----------------------------------------------------- |
| `<test>/<id>.<asset>.glb`  | The case's views                                      |
| `<test>/<id>.<asset>.step` | The case's expected shape, where `asset_step` is true |

A case has one expected shape when it is neither refused nor sound and its first view has one
subject, not counting ghosts. Such a subject is a boolean's intersection, a feature's result or a
queried solid. A view of two subjects, such as a mirror join's part and its image, has no expected
shape. `asset` is a 64-bit FNV-1a of the case id and `ASSET_GENERATOR` in
[`run.rs`](../suite/src/run.rs). `ASSET_GENERATOR` changes when what draws the files changes. A test
in springrep's adapter pins it to a fingerprint of what springrep's `display` draws, and fails when
that drifts.

A view's subject is the kernel's own result where the case's runner kept one. The runner keeps a
boolean's four results and operands, a feature's or chain's result, a moved solid, a split's pieces,
and a boolean of many. Any other solid the view names is built by the kernel from the case's shapes
and moved by the kernel's [`transform`](api.md#transform), or by its [`affine`](api.md#affine) where
`transform` refuses. A ghost is the part of an operand that lies outside the view's subject. It is
the kernel's difference of the two, less each face and each edge whose middle the kernel
[classifies](api.md#classify) as on the subject's boundary. Subjects are drawn first and ghosts
last. An operand's ghost behind a union of that operand is empty, so it is not cut. Each solid is
drawn with [`display`](api.md#display) within `DRAWN` (5e-4) of its diagonal, and the expected shape
is written with [`write_step`](api.md#write_step).

The GLB holds one scene per view, named as the view. Each scene has one node per solid. A node is
named by its role (`main`, `alt`, `ghost`, `ghost2`) and also carries the role as `extras.role`. Its
mesh has the display's faces as triangles and the display's edges as lines. Positions are in
millimetres, under a root node that scales them to metres. The file's `extras` carry the case's
`id`, `test` and `hash`, the `kernel` that drew it, the case's `outcome` and `result`, and the
views' `points`, `box` and `plane`. The file requires `KHR_mesh_quantization`, which three.js,
Blender and `<model-viewer>` read. Positions are `SHORT` over the solid's box, and the node's
`translation` and uniform `scale` take them back to millimetres. Normals are normalized `BYTE`. Both
primitives are indexed, with `UNSIGNED_SHORT` where the vertices fit.

The expected shapes are published under a base URL that is compiled in from
`BREP_CONFORMANCE_ASSET_BASE`. The default is `https://cdn.springcraft.ai/brep-conformance`.
[`assets.rs`](../suite/src/assets.rs) gives the URLs of a case or of a result.
[`publish-assets.sh`](../publish-assets.sh) runs the suite on springrep with both exports. It
publishes nothing unless every case passed at 1e-7 and every file was drawn. Each published shape is
then a kernel's result that the suite has verified against its own exact values, and no kernel
produced those values. The script uploads only the files the bucket lacks. A file is never
rewritten. A change to what a file shows, or to what draws it, gives the file a new name, so a file
can be cached immutably.
