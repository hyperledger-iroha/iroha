---
lang: ja
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraGlobal Gateway 課金照合

- **ウィンドウ:** `<from>/<to>`
- **テナント:** `<tenant-id>`
- **カタログバージョン:** `<catalog-version>`
- **利用スナップショット:** `<path or hash>`
- **ガードレール:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, alert threshold `<alert-threshold>%`
- **支払者 -> トレジャリー:** `<payer>` -> `<treasury>` in `<asset-definition>`
- **合計請求額:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## 明細チェック
- [ ] 利用エントリはカタログの meter id と有効な課金リージョンのみを含む
- [ ] 数量単位がカタログ定義と一致 (requests, GiB, ms, etc.)
- [ ] リージョン乗数と割引ティアがカタログどおり適用
- [ ] CSV/Parquet のエクスポートが JSON 請求明細と一致

## ガードレール評価
- [ ] soft cap の alert threshold 到達? `<yes/no>` (yes の場合はアラート証跡を添付)
- [ ] hard cap 超過? `<yes/no>` (yes の場合は override 承認を添付)
- [ ] 最低請求額のフロアを満たしている

## 台帳プロジェクション
- [ ] 送金バッチ合計が請求書の `total_micros` と一致
- [ ] 資産定義が課金通貨と一致
- [ ] 支払者とトレジャリーのアカウントがテナントと記録上のオペレーターと一致
- [ ] 監査リプレイ用に Norito/JSON artefacts を添付

## 争議/調整メモ
- 観測された差異: `<variance detail>`
- 提案調整: `<delta and rationale>`
- 裏付け証跡: `<logs/dashboards/alerts>`

## 承認
- Billing analyst: `<name + signature>`
- Treasury reviewer: `<name + signature>`
- Governance packet hash: `<hash/reference>`
