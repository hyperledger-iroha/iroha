# AXT redacted-amount admission boundary

The shared V1 amount resolver now refuses a `RemoteSpendIntent` with no signed
clear amount even if the proof envelope exposes a public `committed_amount`
scalar. The host, Core block validator and persisted fragment budget path use
that boundary. A rejected record does not stage family budget consumption.
Clear-amount controls still check the proof scalar, commitment mirror,
cross-block family budget, asset policy, and unanchored spend rejection.

The affected source and fixtures are `crates/ivm_abi/src/axt.rs`,
`crates/iroha_core/src/block.rs`, `crates/iroha_core/src/state/tests.rs`,
`crates/iroha_core/src/block/axt_shared_budget_across_envelopes_test.rs`, and
`crates/iroha_core/tests/ivm_corehost_axt/remote_spend_proof_tests.rs`.

Focused locked offline validation in the shared `optimizations` checkout:

- `cargo test -p ivm_abi --lib axt::tests::`: 47 passed.
- `cargo test -p iroha_core --lib axt_validation_`: 50 passed.
- Core state redacted-refusal and cross-block budget selectors: 1 passed each.
- `cargo test -p iroha_core --features iroha-core-tests --test iroha_core_group_03 core_host_rejects_redacted_amount_even_with_verified_dataspace_proof`: 1 passed.

These were separate focused runs while other source in the dirty checkout could
change. They are not a sealed-candidate test receipt. The signed anchored-spend
model's clear/hidden shape preflight is a separate structural boundary; it does
not prove a private amount. TODO: implement the proof-bound confidential value,
conservation and budget equations, complete successful-execution and finalized
state binding, and an atomic durable spend nonce before private AXT admission.
FASTPQ compact proof size and independent qualification remain open.
