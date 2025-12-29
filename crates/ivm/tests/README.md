IVM tests: dev-only property testing option

- Deterministic matrix tests
  - The file `merkle_super_hash.rs` contains deterministic RNG-based matrix tests that emulate property checks without adding dependencies.
  - Run: `cargo test -p ivm merkle_super_hash -- --nocapture`

- Optional local proptests (do NOT commit dependency changes)
  - If you want to try real proptest locally without affecting the repo:
    - Add under `[dev-dependencies]` in `crates/ivm/Cargo.toml` (locally): `proptest = "1"`.
    - The feature `ivm_prop` gates a placeholder file `prop_skeleton.rs`.
    - Run: `cargo test -p ivm --features ivm_prop --test prop_skeleton`.
  - Please do not commit the `proptest` dependency or any lockfile changes. This repository prefers no new crates by default; keep property tests local until promoted to CI.

