---
lang: mn
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
---

//! Payload v1 rollout approval (SDK Council, 2026-04-28).
//!
//! Captures the SDK Council decision memo required by `roadmap.md:M1` so the
//! encrypted payload v1 rollout has an auditable record (deliverable M1.4).

# Payload v1 Rollout Decision (2026-04-28)

- **Chair:** SDK Council Lead (M. Takemiya)
- **Voting members:** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Observers:** Program Mgmt, Telemetry Ops

## Inputs Reviewed

1. **Swift bindings & submitters** — `ShieldRequest`/`UnshieldRequest`, async submitters, and Tx builder helpers landed with parity tests and docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ergonomics** — `iroha app zk envelope` helper covers encode/inspect workflows plus failure diagnostics, aligned with the roadmap ergonomics requirement.【crates/iroha_cli/src/zk.rs:1256】
3. **Deterministic fixtures & parity suites** — shared fixture + Rust/Swift validation to keep Norito bytes/error surfaces aligned.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Decision

- **Approve payload v1 rollout** for SDKs and CLI, enabling Swift wallets to originate confidential envelopes without bespoke plumbing.
- **Conditions:** 
  - Keep parity fixtures under CI drift alerts (tied to `scripts/check_norito_bindings_sync.py`).
  - Document the operational playbook in `docs/source/confidential_assets.md` (already updated via the Swift SDK PR).
  - Record calibration + telemetry evidence before flipping any production flags (tracked under M2).

## Action Items

| Owner | Item | Due |
|-------|------|-----|
| Swift Lead | Announce GA availability + README snippets | 2026-05-01 |
| CLI Maintainer | Add `iroha app zk envelope --from-fixture` helper (optional) | Backlog (not blocking) |
| DevRel WG | Update wallet quickstarts with payload v1 instructions | 2026-05-05 |

> **Note:** This memo supersedes the temporary “pending council approval” call-out in `roadmap.md:2426` and satisfies tracker item M1.4. Update `status.md` whenever follow-up action items close.
