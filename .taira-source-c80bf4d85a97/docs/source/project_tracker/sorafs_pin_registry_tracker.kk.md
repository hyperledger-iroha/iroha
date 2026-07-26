---
lang: kk
direction: ltr
source: docs/source/project_tracker/sorafs_pin_registry_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2064054a43ba536ae4cde4f90e1911a9b8df4ce77c431fc05b34ee9529f07736
source_last_modified: "2025-12-29T18:16:36.015068+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Pin Registry Contract Tracker

This tracker coordinates local implementation and rollout evidence for the SoraFS
Pin Registry contract under SF-4. It inherits the requirements defined in the
[SoraFS Architecture RFC (SF-1)](../sorafs_architecture_rfc.md), including the
canonical manifest digest flow and governance envelopes.

| ID | Milestone | Owners | Target Window | Status | Notes |
|----|-----------|--------|---------------|--------|-------|
| PR-001 | Contract scaffolding (`RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`) | Storage Team; Nexus Core Infra TL | Q4 2025 | Complete | Data model, world-state storage, permissions, and ISI dispatch are implemented for manifest registration, approval, retirement, alias binding, and replication order issue/complete. |
| PR-002 | Governance signature plumbing | Governance Secretariat; Tooling WG | Q1 2026 | Complete locally | Core validates Ed25519 council envelopes during `ApprovePinManifest`; Torii/CLI surfaces carry manifest payloads and council-digest validation, while Dilithium/ML-DSA governance verification lives in the SF-11 reference validator and release-policy surface. |
| PR-003 | Alias + retention policy enforcement | Storage Team | Q1 2026 | Complete | Alias binding validation, uniqueness, retention windows, replica-count policy, and successor-chain cycle rejection live in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, with Torii DTO and listing support in `crates/iroha_torii/src/sorafs/api.rs`. |
| PR-004 | CI + fixture parity | Tooling WG | Q1 2026 | Complete | `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin-registry fixtures; core unit coverage includes contract-focused alias, successor, replication-order, and policy guard tests. |
| PR-005 | Rollout documentation & operator guide | Docs Team | Q1 2026 | Rollout evidence | `docs/source/sorafs/runbooks/pin_registry_ops.md`, migration docs, CLI docs, and API surfaces are published; live cutover packets and governance archive handoff are deployment evidence. |

## References

- [`docs/source/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../../ci/check_sorafs_fixtures.sh)
