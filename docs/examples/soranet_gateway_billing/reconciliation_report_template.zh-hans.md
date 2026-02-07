---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraGlobal 网关账单对账

- **窗口：** `<from>/<to>`
- **租户：** `<tenant-id>`
- **目录版本：** `<catalog-version>`
- **使用快照：** `<path or hash>`
- **护栏：** 软上限 `<soft-cap-xor> XOR`，硬上限 `<hard-cap-xor> XOR`，警报阈值 `<alert-threshold>%`
- **付款人 -> 财务：** `<payer>` -> `<asset-definition>` 中的 `<treasury>`
- **总应付款：** `<total-xor> XOR`（`<total-micros>` 微异或）

## 行项目检查
- [ ] 使用条目仅涵盖目录计量表 ID 和有效的计费区域
- [ ] 数量单位与目录定义匹配（请求、GiB、毫秒等）
- [ ] 根据目录应用区域乘数和折扣等级
- [ ] CSV/Parquet 导出与 JSON 发票行项目匹配

## 护栏评估
- [ ] 达到软上限警报阈值？ `<yes/no>`（如果是，请附上警报证据）
- [ ] 超出硬上限？ `<yes/no>`（如果是，请附上覆盖批准）
- [ ] 满足最低发票下限

## 账本投影
- [ ] 发票中的转移批次总计等于 `total_micros`
- [ ] 资产定义与计费货币匹配
- [ ] 付款人和财务账户与记录的租户和运营商相匹配
- [ ] Norito/JSON 工件附加用于审核重放

## 争议/调整说明
- 观察到的方差：`<variance detail>`
- 建议调整：`<delta and rationale>`
- 支持证据：`<logs/dashboards/alerts>`

## 批准
- 计费分析师：`<name + signature>`
- 财务审核员：`<name + signature>`
- 治理数据包哈希：`<hash/reference>`