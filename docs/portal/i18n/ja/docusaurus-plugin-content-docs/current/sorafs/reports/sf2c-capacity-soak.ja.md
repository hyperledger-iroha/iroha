---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4bbcbdb9cff2a1b723ae8dbb4ce15784c59d11c506ce09c408666377549efedf
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SF-2c 容量積算ソークレポート

日付: 2026-03-21

## スコープ

このレポートは、SF-2c ロードマップで要求された SoraFS 容量の決定論的な積算および支払いの
soak テストを記録する。

- **30日 multi-provider soak:**
  `capacity_fee_ledger_30_day_soak_deterministic` を
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` で実行する。
  harness は 5 つの providers を作成し、30 の settlement ウィンドウを跨ぎ、
  ledger 合計が独立に算出された参照プロジェクションと一致することを検証する。
  テストは Blake3 digest (`capacity_soak_digest=...`) を出力し、CI が canonical snapshot を
  取得して比較できるようにする。
- **過少提供ペナルティ:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (同じファイル) で強制される。テストは strikes、cooldowns、collateral の slashes、
  ledger カウンタが決定論的に維持されることを確認する。

## 実行

次で soak 検証をローカル実行する:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

テストは標準的なラップトップで 1 秒未満で完了し、外部 fixtures は不要。

## 観測性

Torii は provider credit snapshots を fee ledgers と並べて公開し、ダッシュボードが低残高と
penalty strikes を gate できるようにする:

- REST: `GET /v1/sorafs/capacity/state` は `credit_ledger[*]` エントリを返し、
  soak テストで検証された ledger フィールドを反映する。参照:
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana import: `dashboards/grafana/sorafs_capacity_penalties.json` は
  エクスポートされた strikes カウンタ、ペナルティ合計、bonded collateral を描画し、
  オンコールが soak ベースラインとライブ環境を比較できるようにする。

## フォローアップ

- CI で週次の gate 実行をスケジュールし、soak テストを再生する (smoke-tier)。
- 本番の telemetry エクスポートが稼働したら、Grafana ボードに Torii の scrape ターゲットを追加する。
