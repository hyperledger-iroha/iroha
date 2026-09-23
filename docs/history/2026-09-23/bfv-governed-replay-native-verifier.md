# BFV governed replay and native verifier slice

The `optimizations` checkout keeps BFV production qualification closed. The
crypto witness regression now changes an unselected blind-rotation coefficient:
the self-contained selected-sample shape check accepts the changed trace, but
replay against the governed bootstrap artifacts rejects its output. Core also
exposes an artifact-aware native verifier that performs that full replay before
checking the native STARK envelope. The old self-contained verifier remains a
diagnostic for material supplied by its caller, not production authorization.

Completed checks on the current checkout:

- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_crypto full_bootstrap_execution_witness_digest_binds_governed_trace -- --nocapture`: one test passed (230.13 seconds).
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_crypto full_bootstrap_statement_hash_trace_limbs_are_injective -- --nocapture`: one test passed.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib bfv_full_bootstrap_air_prover_binds_statement_and_public_openings -- --nocapture`: one test passed (176.99 seconds). This binary compiled before the last Core edits and does not validate the new artifact-aware wrapper.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --features zk-stark --lib soracloud_fhe_full_bootstrap_execution_prover_emits_valid_native_air_proof -- --nocapture`: one test passed (252.37 seconds) on the final edited Core source, including a valid governed proof and a self-consistent drifted proof rejected only by the artifact-aware wrapper.

These tests do not establish hidden-trace low degree, secret-key consistency, eight-party
behavior, maximum-shape bounds, or audited parameter, lattice-security, noise,
and qROM evidence. The production qualification function must continue to
reject until those independent requirements are met.
