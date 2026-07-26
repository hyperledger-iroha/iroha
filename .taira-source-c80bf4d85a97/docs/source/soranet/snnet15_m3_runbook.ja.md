---
lang: ja
direction: ltr
source: docs/source/soranet/snnet15_m3_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e16ecf29a5c6178b63a5e347de748a83e53a550f4a902c4709d9c5dd8be6c65c
source_last_modified: "2026-01-30T13:33:43.991472+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/snnet15_m3_runbook.md -->

# SNNet-15M3 — ゲートウェイ GA 準備

このランブックは M2 ベータ証拠を消費し、ガバナンス向け GA バンドルを生成する:
autoscale/worker のダイジェスト、SLA 目標、およびベータ証拠へのリンク。

## 手順
- GA パックの生成:
  - `cargo xtask soranet-gateway-m3 --m2-summary artifacts/soranet/gateway_m2/beta/gateway_m2_summary.json --autoscale-plan <plan.json> --worker-pack <bundle.tgz> --out artifacts/soranet/gateway_m3 --sla-target 99.95%`
- GA 出力の検証:
  - `gateway_m3_summary.json` / `.md` に autoscale プランと worker パックの BLAKE3 ダイジェストが含まれる。
  - `m2_summary` フィールドがベータ証拠の正確なルートを参照している。
  - `sla_target` に SRE と合意した本番 SLO が記録される。

## 終了チェックリスト
- autoscale プランと worker パックのダイジェストがサマリに埋め込まれている。
- M2 サマリが参照され固定されている。
- SLA 目標が記録され、ダッシュボード/ゲートが更新されている。
