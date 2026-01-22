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
| PR-001 | コントラクト土台（`PinRegistry::register`, `approve`, `retire`） | Storage Team; Nexus Core Infra TL | Q4 2025 | 進行中 | `iroha_data_model` が SoraFS pin registry データ型と ISI stubs（`RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`）を公開。 |
| PR-002 | ガバナンス署名配線 | Governance Secretariat; Tooling WG | Q1 2026 | 進行中 | Iroha core が `ApprovePinManifest` 中に評議会エンベロープ（digest/manifest/profile/signatures）を検証。残作業: Torii/CLI の露出と多アルゴリズム対応。 |
| PR-003 | alias + retention ポリシーの強制 | Storage Team | Q1 2026 | 進行中 | alias バインド検証 + 一意性は `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` に実装され、Torii DTO は `crates/iroha_torii/src/routing.rs` で更新済み。retention ウィンドウとレプリカ数は未完。 |
| PR-004 | CI + フィクスチャ整合 | Tooling WG | Q1 2026 | 計画中 | `ci/check_sorafs_fixtures.sh` を拡張し、コントラクト向け golden テストを追加。 |
| PR-005 | ロールアウト文書 & 運用ガイド | Docs Team | Q1 2026 | 計画中 | ガバナンスエンベロープと移行計画を参照する運用 runbook を公開。 |

## 参照

- [`docs/source/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../../ci/check_sorafs_fixtures.sh)
