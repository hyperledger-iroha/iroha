---
lang: ja
direction: ltr
source: docs/source/project_tracker/sorafs_pin_registry_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54f96d65069b15af3c1f48cc22d56efedddcaa098c91df1382007c47a3a6329f
source_last_modified: "2026-01-06T15:14:01.036336+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/project_tracker/sorafs_pin_registry_tracker.md -->

# SoraFS Pin Registry コントラクト・トラッカー

このトラッカーは SF-4 に向けた SoraFS Pin Registry コントラクトの実装作業を調整する。
[SoraFS Architecture RFC (SF-1)](../sorafs_architecture_rfc.md) で定義された
要件を継承し、正準マニフェストの digest フローとガバナンスエンベロープを含む。

| ID | マイルストーン | 担当 | 目標期間 | ステータス | 備考 |
|----|-----------|--------|---------------|--------|-------|
| PR-001 | Contract scaffolding (`RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`) | Storage Team; Nexus Core Infra TL | Q4 2025 | Complete | Data model, world-state storage, permissions, and ISI dispatch are implemented for manifest registration, approval, retirement, alias binding, and replication order issue/complete. |
| PR-002 | Governance signature plumbing | Governance Secretariat; Tooling WG | Q1 2026 | Complete locally | Core validates Ed25519 council envelopes during `ApprovePinManifest`; Torii/CLI surfaces carry manifest payloads and council-digest validation, while Dilithium/ML-DSA governance verification lives in the SF-11 reference validator and release-policy surface. |
| PR-003 | Alias + retention policy enforcement | Storage Team | Q1 2026 | Complete | Alias binding validation, uniqueness, retention windows, replica-count policy, and successor-chain cycle rejection live in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, with Torii DTO and listing support in `crates/iroha_torii/src/sorafs/api.rs`. |
| PR-004 | CI + fixture parity | Tooling WG | Q1 2026 | Complete | `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin-registry fixtures; core unit coverage includes contract-focused alias, successor, replication-order, and policy guard tests. |
| PR-005 | Rollout documentation & operator guide | Docs Team | Q1 2026 | Rollout evidence | `docs/source/sorafs/runbooks/pin_registry_ops.md`, migration docs, CLI docs, and API surfaces are published; live cutover packets and governance archive handoff are deployment evidence. |

## 参照

- [`docs/source/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../../ci/check_sorafs_fixtures.sh)
