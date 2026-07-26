---
lang: ja
direction: ltr
source: docs/source/soranet_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 62a3cc35e474922cd7c3ff8a23b89f776a49f5a0aa24fdad56890fd5f7d46a81
source_last_modified: "2026-01-03T18:07:58.038711+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/soranet_billing_m0.md -->

# SoraGlobal ゲートウェイ課金 (SN15-M0-9)

`soranet-gateway-billing-m0` ヘルパーは、SoraGlobal Gateway CDN の課金プレビュー
パックを提供する。メーターカタログと課金アーティファクトを決定的に保ち、
PoP 演習やガバナンスパケットが同じ証跡を参照できるようにする。

## ジェネレーターの実行

```bash
cargo run -p xtask --bin xtask -- soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir configs/soranet/gateway_m0/billing
```

出力:

- `billing_meter_catalog.json` + `billing_meter_catalog.{csv,parquet}` — 6 つの
  メーター (HTTP リクエスト/エグレス、DoH クエリ、WAF 判定、CAR 検証、ストレージ) を
  階層、割引、リージョン乗数、バースト上限付きで提供。
- `billing_rating_plan.toml` — 契約ノブ (コミット/前払い割引、トレジャリーの skim、
  紛争ホールド) とメーターごとのレートカード。
- `billing_ledger_hooks.toml` + `billing_ledger_projection.json` — receivable / revenue /
  escrow / treasury の仕訳ルールと、サンプル使用量バンドルに基づく投影。
- `billing_guardrails.yaml` — メーターごとの上限と promtool / Alertmanager 検証用アラートルート。
- 請求書、紛争、照合テンプレートはレーティング + レジャー成果物と同期維持。
- `billing_m0_summary.json` — ガバナンス証跡パケット向けのアーティファクト相対パス一覧。

## メモ

- リージョン乗数と割引は CSV/Parquet 出力に適用されるため、課金照合担当は専用スクリプト無しでレジャーエントリと突き合わせできる。
- レジャー投影は M0 サンプル使用量 (リクエスト、エグレス、DoH、WAF、CAR 検証、ストレージ) を用い、レーティングプランで定義されたコミット + 前払い割引、紛争ホールド、トレジャリー skim を適用する。
- Guardrails はメーターごとの `burst_cap` を反映し、使用量スパイク、エグレス成長、トラストレス検証の遅延、紛争バックログに対するアラートをカバーする。
