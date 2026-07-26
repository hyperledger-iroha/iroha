IVM tests: dev-only property testing option

- Deterministic matrix tests
  - The file `merkle_super_hash.rs` contains deterministic RNG-based matrix tests that emulate property checks without adding dependencies.
  - Run: `cargo test -p ivm merkle_super_hash -- --nocapture`

- Deterministic regression tests replace local property-test scaffolding so the
  IVM test suite does not pull a property-testing dependency into the workspace.
