---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 设备实验室故障转移演练操作手册 (AND6/AND7)

该操作手册记录了程序、证据要求和联系矩阵
在执行中引用的**设备实验室应急计划**时使用
`roadmap.md`（§“监管人工制品批准和实验室应急情况”）。它补充了
预订工作流程 (`device_lab_reservation.md`) 和事件日志
(`device_lab_contingency.md`) 因此合规审查员、法律顾问和 SRE
对于如何验证故障转移准备情况有单一的事实来源。

## 目的和节奏

- 演示Android StrongBox +通用设备池可以故障转移
  到回退 Pixel 通道、共享池、Firebase 测试实验室突发队列，以及
  外部 StrongBox 固定器，不会丢失 AND6/AND7 SLA。
- 制作法律部门可以附加到 ETSI/FISC 提交材料中的证据包
  在二月份的合规审查之前。
- 每季度至少运行一次，以及实验室硬件名册发生变化时的任何时间
  （新设备、报废或维护时间超过 24 小时）。

|钻头 ID |日期 |场景 |证据包|状态 |
|----------|------|----------|------------------|--------|
| DR-2026-02-Q1 | 2026-02-20 |模拟 Pixel8Pro 车道中断 + AND7 遥测排练证明积压 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ 已完成 — 捆绑哈希记录在 `docs/source/compliance/android/evidence_log.csv` 中。 |
| DR-2026-05-Q2 | 2026-05-22（预定）| StrongBox 维护重叠 + Nexus 排练 | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *（待定）* — `_android-device-lab` 机票 **AND6-DR-202605** 保留预订；捆绑将在演习后填充。 | 🗓 已安排 — 按照 AND6 节奏将日历块添加到“Android 设备实验室 - 预订”。 |

## 程序

### 1. 演练前准备

1. 确认 `docs/source/sdk/android/android_strongbox_capture_status.md` 中的基线容量。
2. 通过以下方式导出目标 ISO 周的预订日历
   `python3 scripts/android_device_lab_export.py --week <ISO week>`。
3. 归档 `_android-device-lab` 票据
   `AND6-DR-<YYYYMM>` 包含范围（“故障转移演练”）、计划插槽和受影响的
   工作负载（证明、CI 烟雾、遥测混乱）。
4. 使用以下内容更新 `device_lab_contingency.md` 中的应急日志模板
   钻取日期的占位符行。

### 2. 模拟故障情况

1. 禁用或取消池化实验室内的主通道 (`pixel8pro-strongbox-a`)
   调度程序并将预留条目标记为“drill”。
2. 在 PagerDuty 中触发模拟中断警报（`AND6-device-lab` 服务）并
   捕获证据包的通知导出。
3. 注释通常消耗车道的 Buildkite 作业
   （`android-strongbox-attestation`、`android-ci-e2e`）以及钻头 ID。

### 3. 故障转移执行1. 将后备 Pixel7 通道提升到主要 CI 目标并安排
   针对它的计划工作负载。
2. 通过 `firebase-burst` 通道触发 Firebase 测试实验室突发套件
   零售钱包烟雾测试，而 StrongBox 覆盖范围转移到共享
   车道。捕获票证中的 CLI 调用（或控制台导出）以供审核
   平价。
3. 使用外部 StrongBox 实验室固定器进行短暂的认证扫描；
   如下所述记录联系确认。
4. 将所有 Buildkite 运行 ID、Firebase 作业 URL 和保留者记录记录在
   `_android-device-lab` 票据和证据包清单。

### 4. 验证和回滚

1. 将证明/CI 运行时间与基线进行比较；标记增量 >10%
   硬件实验室负责人。
2. 恢复主通道并更新容量快照和就绪情况
   验证通过后的矩阵。
3. 将最后一行附加到 `device_lab_contingency.md`，其中包含触发器、操作、
   和后续行动。
4. 将 `docs/source/compliance/android/evidence_log.csv` 更新为：
   捆绑包路径、SHA-256 清单、Buildkite 运行 ID、PagerDuty 导出哈希以及
   审稿人签字。

## 证据包布局

|文件|描述 |
|------|-------------|
| `README.md` |摘要（演习 ID、范围、所有者、时间表）。 |
| `bundle-manifest.json` |捆绑包中每个文件的 SHA-256 映射。 |
| `calendar-export.{ics,json}` |来自导出脚本的 ISO 周预留日历。 |
| `pagerduty/incident_<id>.json` | PagerDuty 事件导出显示警报 + 确认时间线。 |
| `buildkite/<job>.txt` | Buildkite 受影响作业的运行 URL 和日志。 |
| `firebase/burst_report.json` | Firebase 测试实验室突发执行摘要。 |
| `retainer/acknowledgement.eml` |来自外部 StrongBox 实验室的确认。 |
| `photos/` |如果重新连接硬件，则可以提供实验室拓扑的可选照片/屏幕截图。 |

将捆绑包存储在
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` 并记录
证据日志中的清单校验和加上 AND6 合规性检查表。

## 联系和升级矩阵

|角色 |主要联系人 |频道 |笔记|
|------|-----------------|------------|--------|
|硬件实验室负责人 |普里亚·拉马纳坦 | `@android-lab` 松弛 · +81-3-5550-1234 |拥有现场行动和日历更新。 |
|设备实验室运营|马特奥·克鲁兹 | `_android-device-lab` 队列 |协调预订门票 + 捆绑上传。 |
|发布工程|阿列克谢·莫罗佐夫 |发布英语 Slack · `release-eng@iroha.org` |验证 Buildkite 证据 + 发布哈希值。 |
|外部保险箱实验室 |樱花仪器 NOC | `noc@sakura.example` · +81-3-5550-9876 |保持器接触； 6 小时内确认可用性。 |
| Firebase 突发协调员 |泰莎·赖特 | `@android-ci` 松弛 |当需要回退时触发 Firebase 测试实验室自动化。 |

如果演练发现阻塞问题，请按以下顺序升级：
1. 硬件实验室负责人
2. Android 基础 TL
3. 项目负责人/发布工程
4. 合规主管+法律顾问（如果演习揭示监管风险）

## 报告和跟进- 每当引用时，将此操作手册与预订程序链接起来
  `roadmap.md`、`status.md` 和治理数据包中的故障转移准备情况。
- 将季度演习回顾连同证据包通过电子邮件发送给合规+法务部
  哈希表并附加 `_android-device-lab` 票据导出。
- 镜像关键指标（故障转移时间、恢复的工作负载、未完成的操作）
  在 `status.md` 和 AND7 热门列表跟踪器内，以便审阅者可以跟踪
  依赖于具体的排练。