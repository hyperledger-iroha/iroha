---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-12-29T18:16:35.201180+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SF-2c 容量累积浸泡报告

日期：2026-03-21

## 范围

该报告记录了确定性 SoraFS 容量累积和支付浸泡
SF-2c 路线图轨道下要求的测试。

- **30 天多提供商浸泡：** 执行者
  `capacity_fee_ledger_30_day_soak_deterministic` 中
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  该工具实例化了 5 个提供商，跨越 30 个结算窗口，并且
  验证分类总数是否与独立计算的参考相匹配
  投影。该测试发出 Blake3 摘要 (`capacity_soak_digest=...`)，因此
  CI 可以捕获并区分规范快照。
- **交付不足的处罚：** 执行者
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  （同一文件）。该测试确认了打击阈值、冷却时间、附带削减、
  账本计数器仍然是确定性的。

## 执行

使用以下命令在本地运行浸泡验证：

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

测试在标准笔记本电脑上在一秒内完成，并且不需要
外部固定装置。

## 可观察性

Torii 现在将提供商信用快照与费用分类账一起公开，以便仪表板
可以控制低余额和罚球：

- REST：`GET /v2/sorafs/capacity/state` 返回 `credit_ledger[*]` 条目
  镜像浸泡测试中验证的账本字段。参见
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana 导入：`dashboards/grafana/sorafs_capacity_penalties.json` 绘制
  出口罢工计数器、罚款总额和保税抵押品等
  工作人员可以将浸泡基线与现场环境进行比较。

## 后续行动

- 在 CI 中安排每周门运行以重播浸泡测试（烟雾层）。
- 一旦生产遥测，使用 Torii 刮擦目标扩展 Grafana 板
  出口上线。