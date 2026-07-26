---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 容量と蓄積の関係 SF-2c

日付: 2026-03-21

## ポルテ

Ce rapport consigne les testing deterministes de soak d'accumulation et de paiement de capacité SoraFS demés
ダン・ラ・フィーユ・ド・ルートSF-2c。

- **マルチプロバイダーを 30 時間浸漬します:** 実行パー
  `capacity_fee_ledger_30_day_soak_deterministic` ダンス
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  プロバイダーのインスタンスを利用し、決済などで 30 フェネトルを支払う
  レジャー特派員の有効な投影と参照
  独立した計算。 Blake3 をダイジェストでテストする (`capacity_soak_digest=...`)
  CI のキャプチャーと比較ファイルのスナップショットを正規化します。
- **Pénalités de sous-livraison:** アップリケ パー
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  （メームフィシエ）。ストライキのテスト確認、クールダウン、
  担保をスラッシュし、元帳の保存期間を決定します。

## 実行

ソーク ロケールの平均的な検証結果:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

ラップトップの標準規格などを含めて 2 秒間でテストが終了します
外部に必要な Aucun フィクスチャ。

## 可観測性

Torii ダッシュボード上でクレジット プロバイダーのメンテナンス スナップショットと手数料台帳を公開します
puissent ゲート sur les faibles ソルデスとペナルティストライク:

- REST: `GET /v1/sorafs/capacity/state` renvoie des entrées `credit_ledger[*]` qui
  シャンプ・デュ・レジャー・ベリフィエをテスト・ド・ソークに反映します。ヴォワール
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` トレース ファイルをインポートします
  輸出ストライキの計算、罰金の計算、および担保関係の監視
  オンコールで環境をライブで確認し、ベースラインを比較します。

##スイビ

- Planifier des exécutions hebdomadaires de Gate en CI pour rejouer le test de soak (smoke-tier)。
- テーブルの説明 Grafana 収集可能な情報の収集 Torii 生産用テレメトリの輸出
  セロンアンリーニュ。