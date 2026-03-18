---
lang: ja
direction: ltr
source: docs/source/soranet/snnet15_m2_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3ed72902eb7308805e391c1d03a36f88429093b4edf9f509b445dbb3253d9f6
source_last_modified: "2026-01-30T13:33:43.991472+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/snnet15_m2_runbook.md -->

# SNNet-15M2 — ゲートウェイ ベータランブック

M2 ベータバンドルは、SoraGlobal Gateway CDN を DoQ/ODoH プレビュー、
trustless CAR 検証、GAR コンプライアンス出力、ハードニング証拠付きで提供する。

## 手順
- ベータパック生成: `cargo xtask soranet-gateway-m2 --config configs/soranet/gateway_m2/beta_config.json --output-dir artifacts/soranet/gateway_m2/beta`
- アーティファクト確認:
  - `gateway_m2_summary.json` は、各 PoP のベータエッジ設定、trustless verifier パス、
    PQ 準備サマリ、ハードニングサマリ、ops バンドルを列挙する。
  - 課金セクションは `invoice`/`ledger_projection` を持ち、`allow_hard_cap=true` を強制する。
  - コンプライアンス出力は GAR の receipts/ACKs から生成された `compliance_summary.{json,md}` に格納。
- PoP ごとの PQ 準備: 設定で `--srcv2`, `--tls-bundle`, `--trustless-config` を指定し、
  各 `pops/<pop>/beta/` に `pq/` サマリを出力する。
- ハードニング: `sbom`, `vuln_report`, `hsm_policy`, `sandbox_profile` を指定すると、
  `gateway_hardening_summary.{json,md}` が出力され、保持期間シグナルを含む（30 日超で警告）。

## 出力
- `pops/<label>/gateway_edge_beta.yaml` — trustless verifier バインディング付きの H3 + DoQ + ODoH プレビュー設定。
- `billing/` — JSON/CSV/Parquet の請求書と ledger projection/合計。
- `compliance_summary.{json,md}` — GAR 執行のロールアップ。
- `gateway_m2_summary.json` — ガバナンスパケット向けの正準証拠ルート。
