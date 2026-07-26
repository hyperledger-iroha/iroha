---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c の容量の蓄積についての情報

フェチャ: 2026-03-21

## アルカンス

エステは、容量 SoraFS およびパゴスに蓄積されたプルエバスの登録を決定することを通知します。
SF-2c を要求します。

- **マルチプロバイダーを 30 日間浸します:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` ja
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  エル ハーネス インスタンス シンコ プロバイダー、アバルカ 30 ベンタナ デ 和解 y
  参照の合計と元帳の一致を検証します。
  独立した計算方法。ラ・プルエバ・エミット・アン・ダイジェスト Blake3
  (`capacity_soak_digest=...`) CI プエダのキャプチャと比較スナップショット
  カノニコ。
- **罰則:** 罰則:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  （ミスモアルキーボ）。ラ・プルエバは、ストライキ、クールダウン、
  担保と帳簿の永久的な確定性をスラッシュします。

## 排出

ローカル情報を浸すための有効性の確認:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

必要な準備を必要とせずに、安全な管理を完了するための準備を完了します。
備品の外部。

## 観察可能性

Torii アホラは、ダッシュボードにある料金台帳とプロバイダーのクレジットのスナップショットを公開します
プエダン ガタール ソブレ サルドス バホス y ペナルティ ストライク:

- REST: `GET /v1/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` クエリ
  リフレジャン・ロス・カンポス・デル・レジャー・ベリフィカドス・アン・ラ・プルエバ・デ・ソーク。バージョン
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana のインポート: `dashboards/grafana/sorafs_capacity_penalties.json` グラフィックス
  輸出ストライキの管理、罰金および保証金の総額
  オンコールプエダは、ベースラインを生体内で浸漬して比較します。

## セギミエント

- CI パラ Reejecutar la prueba de soak (smoke-tier) のプログラムの出力。
- Grafana オブジェクトのスクレイピングを行うエクステンダー、Torii のエクスポート機能
  生産管理の遠隔測定。