---
lang: zh-hans
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ `roadmap.md:M4` 引用的机密资产审计和运营手册。

# Confidential Assets Audit & Operations Runbook

本指南整合了审核员和操作员所依赖的证据表面
when validating confidential-asset flows. It complements the rotation playbook
(`docs/source/confidential_assets_rotation.md`) and the calibration ledger
（`docs/source/confidential_assets_calibration.md`）。

## 1. Selective Disclosure & Event Feeds

- 每条机密指令都会发出结构化的 `ConfidentialEvent` 有效负载
  （`Shielded`、`Transferred`、`Unshielded`）捕获于
  `crates/iroha_data_model/src/events/data/events.rs:198` and serialized by the
  executors (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  回归套件测试了具体的有效负载，以便审计人员可以信赖
  确定性 JSON 布局 (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`)。
- Torii 通过标准 SSE/WebSocket 管道公开这些事件；审计员
  subscribe using `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  可以选择将范围限定为单个资产定义。 CLI 示例：

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- 政策元数据和待定转换可通过
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)，由 Swift SDK 镜像
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) 并记录在
  机密资产设计和 SDK 指南
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. 遥测、仪表板和校准证据

- 运行时指标表面树深度、承诺/前沿历史、根驱逐
  计数器和验证者缓存命中率
  （`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`）。 Grafana dashboards in
  `dashboards/grafana/confidential_assets.json` ship the associated panels and
  警报，工作流程记录在 `docs/source/confidential_assets.md:401` 中。
- 带有签名日志的校准运行（NS/op、gas/op、ns/gas）
  `docs/source/confidential_assets_calibration.md`。最新的苹果芯片
  NEON 运行存档于
  `docs/source/confidential_assets_calibration_neon_20260428.log`，同样
  ledger 记录 SIMD 中性和 AVX2 配置文件的临时豁免，直到
  x86 主机上线。

## 3. 事件响应和操作员任务

- 轮换/升级程序位于
  `docs/source/confidential_assets_rotation.md`，涵盖如何上演新内容
  参数包、安排策略升级并通知钱包/审计员。的
  跟踪器 (`docs/source/project_tracker/confidential_assets_phase_c.md`) 列表
  操作手册所有者和排练期望。
- 对于生产排练或紧急窗口，操作员将证据附在
  `status.md` 条目（例如，多车道排练日志）并包括：
  `curl` 策略转换证明、Grafana 快照以及相关事件
  摘要，以便审计员可以重建铸币→转移→披露时间表。

## 4. 外部审核节奏

- 安全审查范围：机密电路、参数注册表、策略
  转换和遥测。本文件加上校准分类帐表格
  发送给供应商的证据包；审核安排通过以下方式跟踪
  `docs/source/project_tracker/confidential_assets_phase_c.md` 中的 M4。
- 运营商必须随时更新 `status.md` 的任何供应商调查结果或后续行动
  行动项目。在外部审核完成之前，本运行手册将作为
  运营基线审计员可以进行测试。