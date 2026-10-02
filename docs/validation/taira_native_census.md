# Taira native validation census

The development gate in `scripts/taira_release_check.py` selects current executable tests. Its source check rejects absent declarations before Cargo; the compiled harness listing must then contain every exact selected name, and each selected test must actually pass. A source inventory pass is not release evidence.

`scripts/taira_native_test_inventory.py` pins 208 tests in 26 registered native source owners. Its independent census checks the complete direct test declarations and exact parent-to-source module registration. Removing a test, adding an unreviewed test, substituting a source path, or duplicating a selected name fails the check. The ordinary application, configuration, SDK and HTTP suites remain independently selected by the gate. Authenticated preparation retains and verifies the gate, inventory and Rust-source masking helper before executing any of them; later filesystem replacement cannot substitute executable helper code.

| Current owner | Required behavior |
| --- | --- |
| Native executor publication | Retain the original executed overlay, result, certificate and allocation owners across refusal; prevent reexecution after consuming publication failure. |
| Native archive completion | Retry the exact durable decision; reject substituted sources and insufficient quorum. |
| Native execution tip | Preserve current and undo identity, including the genesis predecessor; decode grants no authority. |
| Certified chain and prefix readers | Authenticate the exact network, frozen context, quorum, parent, complete execution result and original canonical frames. |
| Native driver | Bound custody and scheduling, retain witnesses, order persistence, and reproduce storage/crash/message faults through deterministic fixtures. |
| Lane merge | Admit current certified lane sources through the native execution owner. |
| Lane signer custody | Retain original-pool sparse backing across pinning, World clones and both snapshot generations; keep local resource refusal typed and retry from unchanged source. |
| Lane sample custody | Charge the exact retained sample suffix and shared control to the original pool; preserve current and undo owners through refusal, rollback, publication and snapshot retries. |
| Stored body and committed reads | Retain original certificate, availability, QC and witness backing through decoder or projection refusal; preserve retry classification while malformed storage remains terminal. |
| Beacon and epoch schedule | Authenticate current custody, executed controls, signed genesis registrations and frozen epoch authority; bind control retries to the original published tip without decoding historical bodies again. |

The deployment engine library, `iroha_deploy`, owns the eleven selected genesis-staging and localnet tests. Kagami retains its CLI, signing, bootstrap and key-custody tests. Source and compiled-harness checks enforce those owners without dropping selectors.

The retired lifecycle ledger, QueuePlan certificate authority, caller-authored execution seals, hash-only snapshot bootstrap and their source-contract asset are not implementation owners. The gate has no fallback to their selectors or layouts. Snapshot export does not qualify positive-height World restore; original native replay remains required until the complete World provenance work is finished.

Signed RS16 `PayloadManifest`/`PayloadChunk` acquisition is integrated in the Core/worker candidate. Current-source whole-node loss, withholding and restart qualification remains open under `specs/sumeragi_goals.md`. Passing this census or component tests does not establish release readiness.
