IVM tests

- Integration tests are split across the `ivm_group_*` targets declared in
  `Cargo.toml` so focused validation can reuse one warm build lane.
- Deterministic regression tests replace local property-test scaffolding; the
  suite does not pull a property-testing dependency into the workspace.
- Backend-heavy BN254, Poseidon, proof-envelope, and Merkle checks use the
  `ivm_zk_tests` feature.

Run a focused group with, for example,
`cargo test -p ivm --test ivm_group_07 --features ivm_zk_tests`.
