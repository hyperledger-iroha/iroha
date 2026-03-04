---
lang: ja
direction: ltr
source: docs/source/soranet_gateway_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab232fba530de14a767bf1832bc00e19a4c82e6ca431ef5c875a9f62c321509d
source_last_modified: "2025-11-21T12:20:45.435832+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet_gateway_billing_m0.md -->

# SoraGlobal ゲートウェイ課金 M0 パック

SN15-M0-9 は、初期の PoP ドリルで使用した課金プレビューのアーティファクトを
記録する。`soranet-gateway-billing-m0` ヘルパーは決定論的なバンドルを
生成し、財務/運用/ガバナンスの各チームが同じ証拠をチケットやリハーサルに
添付できるようにする。

## 使い方

```
cargo xtask soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir artifacts/soranet/gateway_m0/billing_demo
```

デフォルト:
- 請求期間: `2026-11`
- 出力ディレクトリ: `configs/soranet/gateway_m0/billing/`

## バンドル内容

- `billing_meter_catalog.json` — メーター id/単位、地域倍率、上限。
- `billing_rating_plan.toml` — レーティング/割引/階層ルール（bps, micros）。
- `billing_ledger_hooks.toml` — 売掛/収益/escrow/返金の計上ルール。
- `billing_guardrails.yaml` — 上限、異議、検証遅延に関するアラート閾値。
- テンプレート: `billing_invoice_template.md`, `billing_dispute_template.md`,
  `billing_reconciliation_template.md`。
- `billing_m0_summary.json` — すべてのファイルへの相対パス（迅速な添付用）。

M0 ドリルではバンドルを変更せず、`--output-dir` で再生成して
環境ごとのコピーを作成する。`configs/soranet/gateway_m0/billing/` にある
正準デフォルトには手を触れないこと。
