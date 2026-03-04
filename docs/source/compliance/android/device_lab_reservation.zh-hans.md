---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2025-12-29T18:16:35.924530+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 设备实验室预约流程 (AND6/AND7)

本手册介绍了 Android 团队如何预订、确认和审核设备
里程碑 **AND6**（CI 和合规性强化）和 **AND7** 的实验室时间
（可观察性准备）。它补充了应急登录
`docs/source/compliance/android/device_lab_contingency.md` 通过确保容量
首先就避免了不足。

## 1. 目标和范围

- 将 StrongBox + 通用设备池保持在路线图规定的 80% 以上
  整个冻结窗口的容量目标。
- 提供确定性日历，以便清除 CI、证明和混乱
  排练从来不会争夺相同的硬件。
- 捕获可审计的跟踪（请求、批准、运行后注释）
  AND6 合规性检查表和证据日志。

此过程涵盖专用像素通道、共享后备池和
路线图中引用的外部 StrongBox 实验室固定器。临时模拟器
使用超出范围。

## 2. 预约窗口

|泳池/泳道 |硬件|默认槽长度 |预订提前期 |业主|
|------------|----------|--------------------------------|--------------------|--------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4小时| 3 个工作日 |硬件实验室负责人 |
| `pixel8a-ci-b` | Pixel8a（CI 通用）| 2小时| 2 个工作日 | Android 基础 TL |
| `pixel7-fallback` | Pixel7共享池| 2小时| 1 个工作日 |发布工程|
| `firebase-burst` | Firebase 测试实验室烟雾队列 | 1小时| 1 个工作日 | Android 基础 TL |
| `strongbox-external` |外部 StrongBox 实验室固定器 | 8小时| 7 个日历日 |项目负责人 |

时段以 UTC 时间预订；重叠预订需要明确批准
来自硬件实验室负责人。

## 3. 请求工作流程

1. **准备上下文**
   - 更新 `docs/source/sdk/android/android_strongbox_device_matrix.md`
     您计划锻炼的设备和准备标签
     （`attestation`、`ci`、`chaos`、`partner`）。
   - 收集最新的容量快照
     `docs/source/sdk/android/android_strongbox_capture_status.md`。
2. **提交请求**
   - 使用以下模板在 `_android-device-lab` 队列中提交票证
     `docs/examples/android_device_lab_request.md`（所有者、日期、工作负载、
     后备要求）。
   - 附加任何监管依赖项（例如 AND6 证明扫描、AND7
     遥测演习）并链接到相关路线图条目。
3. **批准**
   - 硬件实验室负责人在一个工作日内进行审查，确认在
     共享日历 (`Android Device Lab – Reservations`)，并更新
     `device_lab_capacity_pct` 列
     `docs/source/compliance/android/evidence_log.csv`。
4. **执行**
   - 运行预定的作业；记录 Buildkite 运行 ID 或工具日志。
   - 注意任何偏差（硬件交换、超限）。
5. **关闭**
   - 使用文物/链接对票证进行评论。
   - 如果运行与合规性相关，则更新
     `docs/source/compliance/android/and6_compliance_checklist.md` 并添加一行
     至 `evidence_log.csv`。

影响合作伙伴演示 (AND8) 的请求必须抄送合作伙伴工程人员。

## 4. 变更与取消- **重新安排：**重新打开原始工单，提出新的时段，并更新
  日历条目。如果新插槽在 24 小时内，请 ping 硬件实验室负责人 + SRE
  直接。
- **紧急取消：**遵循应急计划
  (`device_lab_contingency.md`) 并记录触发器/操作/后续行。
- **溢出：** 如果运行超出其时间段 >15 分钟，则发布更新并确认
  是否可以进行下一次预订；否则移交给后备
  池或 Firebase 突发通道。

## 5. 证据与审计

|文物 |地点 |笔记|
|----------|----------|--------|
|订票 | `_android-device-lab` 队列 (Jira) |导出每周汇总；证据日志中的链接票证 ID。 |
|日历导出| `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` |每周五运行 `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>`；该帮助程序保存过滤后的 `.ics` 文件以及 ISO 周的 JSON 摘要，以便审核可以附加这两个工件，而无需手动下载。 |
|容量快照 | `docs/source/compliance/android/evidence_log.csv` |每次预订/关闭后更新。 |
|运行后笔记 | `docs/source/compliance/android/device_lab_contingency.md`（如果有意外情况）或票证评论 |审核所需。 |

在季度合规审查期间，附上日历导出、票据摘要、
AND6 清单提交的证据日志摘录。

### 日历导出自动化

1. 获取“Android Device Lab – Reservations”的 ICS feed URL（或下载 `.ics` 文件）。
2. 执行

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   该脚本写入 `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   和 `...-calendar.json`，捕获选定的 ISO 周。
3. 上传生成的文件和每周证据包并引用
   JSON摘要在`docs/source/compliance/android/evidence_log.csv`时
   记录设备实验室容量。

## 6. 升级阶梯

1. 硬件实验室负责人（初级）
2. Android 基础 TL
3. 项目主管/发布工程（用于冻结窗口）
4. 外部 StrongBox 实验室联系人（调用保留器时）

升级必须记录在票证中并反映在每周的 Android 中
状态邮件。

## 7. 相关文档

- `docs/source/compliance/android/device_lab_contingency.md` — 事件日志
  能力不足。
- `docs/source/compliance/android/and6_compliance_checklist.md` — 主控
  可交付成果清单。
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — 硬件
  覆盖跟踪器。
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  AND6/AND7 引用的 StrongBox 证明证据。

维持此预留程序满足路线图行动项目“定义
设备实验室预留程序”并保留面向合作伙伴的合规工件
与 Android 准备计划的其余部分同步。

## 8. 故障转移演练程序和联系方式

路线图项目 AND6 还需要每季度进行一次故障转移演练。完整的、
分步说明位于
`docs/source/compliance/android/device_lab_failover_runbook.md`，但是高
级别工作流程总结如下，以便请求者可以同时计划演练
常规预订。1. **安排演习：** 封锁受影响的车道（`pixel8pro-strongbox-a`，
   共享中的后备池、`firebase-burst`、外部 StrongBox 保留器）
   日历和 `_android-device-lab` 至少在演习前 7 天排队。
2. **模拟中断：** Depool主通道，触发PagerDuty
   (`AND6-device-lab`) 事件，并注释依赖的 Buildkite 作业
   操作手册中注明的演练 ID。
3. **故障转移：** 提升 Pixel7 后备通道，启动 Firebase 突发
   套件，并在 6 小时内与外部 StrongBox 合作伙伴合作。捕捉
   Buildkite 运行 URL、Firebase 导出和保留确认。
4. **验证和恢复：**验证证明+ CI运行时，恢复
   原始车道，并更新 `device_lab_contingency.md` 加上证据日志
   与捆绑路径+校验和。

### 联系和升级参考

|角色 |主要联系人 |频道 |升级命令 |
|------|-----------------|------------------------|--------------------|
|硬件实验室负责人 |普里亚·拉马纳坦 | `@android-lab` 松弛 · +81-3-5550-1234 | 1 |
|设备实验室运营|马特奥·克鲁兹 | `_android-device-lab` 队列 | 2 |
| Android 基础 TL |埃琳娜·沃罗贝娃 | `@android-foundations` 松弛 | 3 |
|发布工程|阿列克谢·莫罗佐夫 | `release-eng@iroha.org` | 4 |
|外部保险箱实验室 |樱花仪器 NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

如果演练发现阻塞问题或有任何后备措施，则按顺序升级
Lane无法在30分钟内上线。始终记录升级情况
在 `_android-device-lab` 票证中注明并将其镜像到应急日志中。