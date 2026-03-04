---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
---

> 规范来源：`docs/source/sorafs/priority_snapshot_2025-03.md`
>
> 状态：**测试版/等待转向 ACK**（网络、存储、文档线索）。

## 概述

三月份的快照使文档/内容网络计划与
SoraFS 传送轨道（SF-3、SF-6b、SF-9）。一旦所有线索均确认
Nexus转向通道中的快照，删除上面的“Beta”注释。

### 焦点话题

1. **循环优先级快照** — 收集确认并将其记录在
   2025-03-05 理事会会议记录。
2. **网关/DNS 启动结束** — 排练新的辅助工具包（第 6 节
   在运行手册中）在 2025 年 3 月 3 日研讨会之前。
3. **操作员运行手册迁移** — 门户 `Runbook Index` 已上线；暴露测试版
   审阅者加入签字后预览 URL。
4. **SoraFS 交付线程** — 使 SF-3/6b/9 剩余工作与计划/路线图保持一致：
   - `sorafs-node` PoR 摄取工作线程 + 状态端点。
   - CLI/SDK 跨 Rust/JS/Swift 协调器集成的绑定完善。
   - PoR 协调器运行时接线和 GovernanceLog 事件。

请参阅源文件以获取完整的表、分发清单和日志条目。