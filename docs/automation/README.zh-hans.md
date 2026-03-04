---
lang: zh-hans
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-12-29T18:16:35.060432+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 文档自动化基线

该目录捕获路线图项目的自动化表面，例如
AND5/AND6（Android 开发者体验 + 发布准备）和 DA-1
（数据可用性威胁模型自动化）指的是当他们要求
可审计的文件证据。暂存命令参考和预期
artefacts in-tree 甚至可以保留合规审查的先决条件
当 CI 管道或仪表板离线时。

## 目录布局

|路径|目的|
|------|---------|
| `docs/automation/android/` | Android 文档和本地化自动化基线 (AND5)，包括 AND6 签署之前所需的 i18n 存根同步日志、奇偶校验摘要和 SDK 发布证据。 |
| `docs/automation/da/` | `cargo xtask da-threat-model-report` 和夜间文档刷新引用的数据可用性威胁模型自动化输出。 |

每个子目录都记录了产生证据的命令以及
我们希望签入的文件布局（通常是 JSON 摘要、运行日志或
体现）。每当
自动化运行实质上更改了已发布的文档，然后链接到提交
来自相关状态/路线图条目。

## 用法

1. **使用子目录中描述的命令运行自动化**
   自述文件（例如，`ci/check_android_fixtures.sh` 或
   `cargo xtask da-threat-model-report`）。
2. **将生成的 JSON/日志工件**从 `artifacts/…` 复制到
   将 `docs/automation/<program>/…` 文件夹与 ISO-8601 时间戳相匹配
   文件名，以便审计员可以将证据与治理会议记录关联起来。
3. **关闭路线图时参考 `status.md`/`roadmap.md` 中的提交**
   门，以便审核者可以确认用于该决策的自动化基线。
4. **保持文件轻量级**。期望是结构化元数据，
   清单或摘要，而不是批量二进制 blob。较大的垃圾场应保留在
   带有此处记录的签名引用的对象存储。

通过集中这些自动化注释，我们解锁了“文档/自动化基线”
AND6 提出并发出 DA 威胁的先决条件“可供审计”
模型流是夜间报告和手动抽查的确定性主页。