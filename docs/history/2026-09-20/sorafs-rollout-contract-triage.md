# SoraFS rollout contract triage

This scoped record covers two stale assertions and the negative-promotion
specification. It does not establish production qualification. The active
implementation and promotion blockers remain open.

## Changes

`scripts/tests/check_sorafs_rollout_gate_contract_test.py` now checks the actual
hardened shell call label, `authenticated external Ed25519 signer adapter`, and
the reference SDK plan's current native operation-receipt requirement. The
executable validation, pinned native verifier, policy/key/revision/finalized
anchor and separate governance-approval assertions remain required.

`specs/sorafs/negative_promotion_archive.md` now describes authenticated software
and optional hardware custody without an HSM prerequisite. It matches the sole
24-field provenance statement, nine-field authentication object and 25-field
native verification result. Removed backend/qualification claims remain unknown
fields. All custody, completed-operation, cosign, exact-subject, independent
trust and open promotion requirements remain. The current
`validate_inner_approval_chain` block is unchanged.

## Scoped validation

Using `target/first-release-tools-venv/bin/python`:

```text
-m pytest -q scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_shell_helpers_use_hardened_release_and_no_follow_io scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_reference_sdk_release_distribution_work_stays_open_in_docs --tb=short
2 passed in 3.13s

-m pytest -q scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_active_sorafs_todo_inventory_has_only_contract_negative_controls scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_active_sorafs_source_todos_stay_closed --tb=short
2 failed in 11.11s
```

The failing closure checks are unchanged. Their full diagnostics are retained in
`target/first-release-sorafs-rollout-open-todos-tests.log`; the complete inventory
is in `target/first-release-sorafs-rollout-open-todos.json`. No marker was removed,
renamed, exempted or converted into a claimed completion.

The tracked inventory has four matches:

| Source | Outstanding requirement or scope |
| --- | --- |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/release_evidence.rs` | Authenticated proof-size, SoraFS transport, work and resident-memory reports together. |
| `crates/sorafs_manifest/src/signer/custody.rs` | Remaining runtime/receipt consumers and real production state adapters. |
| `docs/history/2026-09-11/version-json-context.md` | Preserved historical consumer-migration and combined-candidate qualification status; this is not a fresh implementation finding. |
| `specs/sorafs/negative_promotion_archive.md` | Actual production signing workflow and signed subject qualification. |

The active source scan has seven matches:

| Source | Outstanding requirement |
| --- | --- |
| `crates/iroha_torii/src/mcp.rs` | Explicit audited mutation declarations in curated capability entries. |
| `crates/iroha_torii/src/routing.rs` | Bounded multi-page shard prefixes before wider global fanout. |
| `crates/sorafs_manifest/src/signer/custody.rs` | Remaining consumers and finalized production state adapters. |
| `crates/sorafs_manifest/src/signer/mod.rs` | Production consumer replacement and real signer/state adapters. |
| `crates/sorafs_manifest/src/signer/stream_token/subject_tests.rs` | Complete receipt/coordinator recovery refusal across custody renewal and control changes, without reserving the original operation again. |
| `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` | Authenticated finalized registration/provider completion and publisher-source availability. |
| `scripts/check_sorafs_production_promotion_bundle.py` | Independently verified signer custody and completed-operation proofs for foundational, topology, resilience and lane-inventory approvals. |

Direct source introspection confirmed the 24/9/25 field counts and that the
inner approval checker still returns its production-blocking error. Scoped
`git diff --check` passed. No Cargo command, daemon/Core source edit, deployment
or signing operation was performed for this repair.
