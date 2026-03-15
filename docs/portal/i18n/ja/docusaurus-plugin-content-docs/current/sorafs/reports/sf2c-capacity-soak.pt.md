---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 蓄積された容量との関係 SF-2c

データ: 2026-03-21

## エスコポ

精巣との関係を登録し、蓄積された容量を決定するための検査 SoraFS
SF-2c のロードマップを検討します。

- **マルチプロバイダーを 30 ディアスにソーク:** 実行
  `capacity_fee_ledger_30_day_soak_deterministic`em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  インスタンス シンコ プロバイダーをハーネスします。30 日以内に和解が完了します。
  参照用の元帳通信を検証します。
  独立した形式計算。あなたの精液はダイジェストを出します Blake3
  (`capacity_soak_digest=...`) CI のキャプチャ スナップショットの比較
  カノニコ。
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  （メスモ・アルキーヴォ）。ストライク、クールダウン、スラッシュの制限を確認してください
  担保と管理は台帳を永続的に決定します。

## 実行する

validacoes de soak localmente com として実行します。

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

オスの睾丸が完成し、メノスがセグンドし、ラップトップのパドラオとナオがエグゼムを実行します
備品の外部。

## 観察可能性

Torii アゴラは、OS ダッシュボードにある料金台帳とプロバイダーのクレジットのスナップショットを公開します
ポッサム・ファザー・ゲート・エム・サルドス・バイショスとペナルティーストライク:

- 残り: `GET /v2/sorafs/capacity/state` レトルナ エントラダス `credit_ledger[*]` クエリ
  refletem os Campos は元帳の検証を行いません。テストは行われません。ヴェジャ
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Importacao Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` プロット OS
  ストライキの輸出、罰金の支払い、および担保の準備を整える
  オンコール時間は、製品全体の環境を浸すOSのベースラインと比較します。

## フォローアップ

- 議題は、CI の再実行とテスト (スモーク層) の管理を実行します。
- Grafana は、Torii の収集、テレメトリの輸出、運用管理の作業として使用されます。