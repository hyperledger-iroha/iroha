# Taira native validation census

The development gate in `scripts/taira_release_check.py` selects current executable tests. Its source check rejects absent declarations before Cargo; the compiled harness listing must then contain every exact selected name, and each selected test must actually pass. A source inventory pass is not release evidence.

`scripts/taira_native_test_inventory.py` pins 155 tests in 18 registered native source owners. Its independent census checks the complete direct test declarations and exact parent-to-source module registration. Removing a test, adding an unreviewed test, substituting a source path, or duplicating a selected name fails the check. The ordinary application, configuration, SDK and HTTP suites remain independently selected by the gate. Authenticated preparation retains and verifies the gate, inventory and Rust-source masking helper before executing any of them; later filesystem replacement cannot substitute executable helper code.

| Current owner | Required behavior |
| --- | --- |
| Native executor publication | Retain the original executed overlay, result, certificate and allocation owners across refusal; prevent reexecution after consuming publication failure. |
| Native archive completion | Retry the exact durable decision; reject substituted sources and insufficient quorum. |
| Native execution tip | Preserve current and undo identity, including the genesis predecessor; decode grants no authority. |
| Certified chain and prefix readers | Authenticate the exact network, frozen context, quorum, parent, complete execution result and original canonical frames. |
| Native driver | Bound custody and scheduling, retain witnesses, order persistence, and reproduce storage/crash/message faults through deterministic fixtures. |
| Lane merge | Admit current certified lane sources through the native execution owner. |
| Beacon and epoch schedule | Authenticate current custody, executed controls, signed genesis registrations and frozen epoch authority. |

The retired lifecycle ledger, QueuePlan certificate authority, caller-authored execution seals, hash-only snapshot bootstrap and their source-contract asset are not implementation owners. The gate has no fallback to their selectors or layouts. Snapshot export does not qualify positive-height World restore; original native replay remains required until the complete World provenance work is finished.

Signed RS16 `PayloadManifest`/`PayloadChunk` availability remains an open first-release requirement in `specs/sumeragi_goals.md`, question 8. Passing native full-body transport tests does not qualify that missing integration. This census change makes no release-readiness claim.
