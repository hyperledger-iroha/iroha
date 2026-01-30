---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c52b07d1bdeeb9db3015b5da3cbcc8326e3f23d1a360c65f2c31e872081eac6
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS 容量マーケットプレイス検証チェックリスト

**レビュー期間:** 2026-03-18 -> 2026-03-24  
**プログラム責任者:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**スコープ:** provider のオンボーディングパイプライン、dispute 裁定フロー、SF-2c GA に必要な treasury reconciliation プロセス。

以下のチェックリストは、外部オペレーター向けにマーケットプレイスを有効化する前にレビューする必要がある。各行は、監査担当が再生できる決定論的な証拠 (tests, fixtures, documentation) へのリンクを含む。

## 受け入れチェックリスト

### Provider オンボーディング

| チェック | 検証 | 証拠 |
|-------|------------|----------|
| Registry が正規の容量宣言を受理する | Integration test が app API 経由で `/v1/sorafs/capacity/declare` を実行し、署名処理、metadata の捕捉、ノード registry へのハンドオフを検証する。 | `crates/iroha_torii/src/routing.rs:7654` |
| Smart contract が不一致 payload を拒否する | Unit test が provider IDs と committed GiB フィールドが署名済み宣言と一致することを永続化前に保証する。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI が正規のオンボーディング artefacts を生成する | CLI harness が決定論的な Norito/JSON/Base64 出力を生成し、round-trip を検証してオペレーターが宣言を offline で準備できるようにする。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| オペレーターガイドが admission workflow と governance guardrails を記載する | ドキュメントが宣言スキーマ、policy defaults、council のレビュー手順を列挙する。 | `../storage-capacity-marketplace.md` |

### Dispute 解決

| チェック | 検証 | 証拠 |
|-------|------------|----------|
| Dispute レコードが正規 payload digest と共に永続化される | Unit test が dispute を登録し、保存された payload をデコードして pending 状態を検証し、ledger の決定論性を保証する。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI の dispute ジェネレータが正規スキーマに一致する | CLI テストが `CapacityDisputeV1` の Base64/Norito 出力と JSON サマリーをカバーし、evidence bundles が決定論的に hash されることを保証する。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay テストが dispute/penalty の決定論性を示す | proof-failure telemetry を2回再生して同一の ledger, credit, dispute snapshots を生成し、slashes が peers 間で決定論的になることを保証する。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook が escalation と revocation のフローを記載する | Operations ガイドが council のワークフロー、evidence 要件、rollback 手順を示す。 | `../dispute-revocation-runbook.md` |

### Treasury reconciliation

| チェック | 検証 | 証拠 |
|-------|------------|----------|
| Ledger accrual が 30 日 soak の予測と一致する | Soak テストが 5 providers で 30 の settlement ウィンドウを跨ぎ、ledger エントリを想定 payout 参照と比較する。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Ledger export の reconciliation が夜間に記録される | `capacity_reconcile.py` が fee ledger の期待値を実行済み XOR transfer exports と比較し、Prometheus メトリクスを出力し、Alertmanager 経由で treasury 承認を gate する。 | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Billing dashboards が penalties と accrual telemetry を可視化する | Grafana import が GiB-hour accrual、strike counters、bonded collateral をプロットし、オンコールの可視性を確保する。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| 公開レポートが soak 手順と replay コマンドを保存する | レポートが soak のスコープ、実行コマンド、監査向けの observability hooks を記載する。 | `./sf2c-capacity-soak.md` |

## 実行メモ

サインオフ前に検証スイートを再実行する:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

オペレーターは `sorafs_manifest_stub capacity {declaration,dispute}` でオンボーディング/ディスピュート要求の payloads を再生成し、生成された JSON/Norito bytes をガバナンスチケットと一緒に保管する。

## サインオフ artefacts

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| Provider オンボーディング承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Dispute 解決承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Treasury reconciliation 承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

署名済みコピーは release bundle と一緒に保管し、governance change record にリンクすること。

## 承認

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
