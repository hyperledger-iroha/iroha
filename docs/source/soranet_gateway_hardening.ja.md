---
lang: ja
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

# SoraGlobal Gateway ハードニング（SNNet-15H）

ハードニング用ヘルパーは、Gateway ビルドを昇格させる前にセキュリティ/プライバシーの証跡を収集します。

## コマンド
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## 出力
- `gateway_hardening_summary.json` — 入力（SBOM、脆弱性レポート、HSM ポリシー、sandbox プロファイル）ごとのステータスと保管シグナル。入力が欠ける場合は `warn` または `error`。
- `gateway_hardening_summary.md` — ガバナンスパケット向けの人間可読サマリ。

## 受け入れメモ
- SBOM と脆弱性レポートは必須。入力が欠けるとステータスが低下します。
- 30 日超の保管はレビュー用に `warn` を付与。GA 前により厳しいデフォルトを設定してください。
- サマリアーティファクトを GAR/SOC レビューやインシデント運用手順の添付として使用してください。
