---
lang: ka
direction: ltr
source: docs/source/project_tracker/sorafs_pin_registry_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2064054a43ba536ae4cde4f90e1911a9b8df4ce77c431fc05b34ee9529f07736
source_last_modified: "2025-12-29T18:16:36.015068+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Pin Registry Contract Tracker

This tracker coordinates the implementation work for the SoraFS Pin Registry
contract ahead of SF-4. It inherits the requirements defined in the
[SoraFS Architecture RFC (SF-1)](../sorafs_architecture_rfc.md), including the
canonical manifest digest flow and governance envelopes.

| ID | Milestone | Owners | Target Window | Status | Notes |
|----|-----------|--------|---------------|--------|-------|
| PR-001 | Contract scaffolding (`PinRegistry::register`, `approve`, `retire`) | Storage Team; Nexus Core Infra TL | Q4 2025 | In progress | `iroha_data_model` now exposes SoraFS pin registry data types plus ISI stubs for `RegisterPinManifest`, `ApprovePinManifest`, and `RetirePinManifest`. |
| PR-002 | Governance signature plumbing | Governance Secretariat; Tooling WG | Q1 2026 | In progress | Iroha core now validates council envelopes (digest/manifest/profile/signatures) during `ApprovePinManifest`; remaining work: Torii/CLI surfacing and multi-algorithm support. |
| PR-003 | Alias + retention policy enforcement | Storage Team | Q1 2026 | In progress | Alias binding validation + uniqueness now lives in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, with Torii DTO updates in `crates/iroha_torii/src/routing.rs`; retention windows and replica counts remain. |
| PR-004 | CI + fixture parity | Tooling WG | Q1 2026 | Planned | Extend `ci/check_sorafs_fixtures.sh` and add contract-focused golden tests. |
| PR-005 | Rollout documentation & operator guide | Docs Team | Q1 2026 | Planned | Publish operator runbook referencing governance envelopes and migration plan. |

## References

- [`docs/source/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../../ci/check_sorafs_fixtures.sh)
