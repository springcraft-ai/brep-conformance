# Changelog

Every release of the suite, newest first. [doc/versioning.md](doc/versioning.md) says what each
number promises and what an entry records.

## 1.0.0 (unreleased)

The first public release, and the version every later entry is measured against.

### Protocol

- Protocol 1.0.0, as [doc/api.md](doc/api.md) defines it.
- Its bindings: the Rust crate [`brep-protocol`](protocol/README.md), and a C++ binding in
  [`protocol/cpp`](protocol/cpp/README.md).

### Scoring

- Contract 1.0.0, as [doc/contract.md](doc/contract.md) defines it. A case passes when every check
  holds at the suite's tolerance of 1e-7 mm, and every case counts once in one total, whether it is
  exact, sound or a rejection.

### Tests added

- 3487 cases in 238 tests for the default seed, under `booleans/`, `constructions/`, `fillets/`,
  `chamfers/`, `modify/`, `queries/`, `step/` and `parts/`.
