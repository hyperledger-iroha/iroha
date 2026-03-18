---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c キャパシティ獲得ソーク

日付: 2026-03-21

## ありがとう

SF-2c の評価 高い評価 SoraFS の生産能力の発生率 支払額の確定的ソーク テスト❁❁❁❁

- **30 ユーロのマルチプロバイダー ソーク:**
  `capacity_fee_ledger_30_day_soak_deterministic` ذریعے
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  ハーネス プロバイダー بناتا ہے، 30 決済 ونڈوز کا احاطہ کرتا ہے، اور
  元帳合計数 آزادانہ طور پر حساب شدہ 参照投影 سے ملتے ہیں۔
  Blake3 ダイジェスト (`capacity_soak_digest=...`) と CI 正規スナップショット
  差分をキャプチャする
- **配信不足のペナルティ:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔閾値に達する、クールダウン、担保スラッシュ
  台帳カウンター決定論的 ہیں۔

## 実行

ソーク検証の結果:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

یہ ٹیسٹ ایک عام لیپ ٹاپ پر ایک سیکنڈ سے کم وقت میں مکمل ہو جاتے ہیں اور بیرونی試合日程

## 可観測性

Torii プロバイダーのクレジット スナップショット、料金台帳、ダッシュボード、残高、ペナルティ ストライク、ゲート、および料金台帳:

- REST: `GET /v1/sorafs/capacity/state` `credit_ledger[*]` エントリ واپس کرتا ہے جو 浸漬テスト میں verify ہونے والے
  元帳フィールド کی عکاسی کرتے ہیں۔ और देखें
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana インポート: `dashboards/grafana/sorafs_capacity_penalties.json` エクスポートされたストライク カウンター、ペナルティの合計
  保税担保のプロット オンコール ベースラインの浸漬 ライブ環境の監視

## フォローアップ

- CI ゲートは、浸漬テスト (煙層) を実行します。
- 生産テレメトリのライブエクスポート Grafana ボード Torii ターゲットのスクレイピング