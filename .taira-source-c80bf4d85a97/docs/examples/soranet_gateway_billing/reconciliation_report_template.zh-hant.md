---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraGlobal 網關賬單對賬

- **窗口：** `<from>/<to>`
- **租戶：** `<tenant-id>`
- **目錄版本：** `<catalog-version>`
- **使用快照：** `<path or hash>`
- **護欄：** 軟上限 `<soft-cap-xor> XOR`，硬上限 `<hard-cap-xor> XOR`，警報閾值 `<alert-threshold>%`
- **付款人 -> 財務：** `<payer>` -> `<asset-definition>` 中的 `<treasury>`
- **總應付款：** `<total-xor> XOR`（`<total-micros>` 微異或）

## 行項目檢查
- [ ] 使用條目僅涵蓋目錄計量表 ID 和有效的計費區域
- [ ] 數量單位與目錄定義匹配（請求、GiB、毫秒等）
- [ ] 根據目錄應用區域乘數和折扣等級
- [ ] CSV/Parquet 導出與 JSON 發票行項目匹配

## 護欄評估
- [ ] 達到軟上限警報閾值？ `<yes/no>`（如果是，請附上警報證據）
- [ ] 超出硬上限？ `<yes/no>`（如果是，請附上覆蓋批准）
- [ ] 滿足最低發票下限

## 賬本投影
- [ ] 發票中的轉移批次總計等於 `total_micros`
- [ ] 資產定義與計費貨幣匹配
- [ ] 付款人和財務賬戶與記錄的租戶和運營商相匹配
- [ ] Norito/JSON 工件附加用於審核重放

## 爭議/調整說明
- 觀察到的方差：`<variance detail>`
- 建議調整：`<delta and rationale>`
- 支持證據：`<logs/dashboards/alerts>`

## 批准
- 計費分析師：`<name + signature>`
- 財務審核員：`<name + signature>`
- 治理數據包哈希：`<hash/reference>`