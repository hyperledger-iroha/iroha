---
id: training-collateral
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 镜子 `docs/source/sns/training_collateral.md`。简报时使用此页面
> 每个后缀发布之前的注册商、DNS、监护人和财务团队。

## 1. 课程概览

|轨道 |目标 |预读|
|--------|------------|------------|
|注册商操作 |提交清单、监控 KPI 仪表板、升级错误。 | `sns/onboarding-kit`、`sns/kpi-dashboard`。 |
| DNS 和网关 |应用解析器骨架，排练冻结/回滚。 | `sorafs/gateway-dns-runbook`，直接模式策略示例。 |
|监护人和理事会|执行争议、更新治理附录、记录附件。 | `sns/governance-playbook`，管家记分卡。 |
|金融与分析 |捕获 ARPU/批量指标，发布附件包。 | `finance/settlement-iso-mapping`，KPI 仪表板 JSON。 |

### 模块流程

1. **M1 — KPI 定位（30 分钟）：** 步行后缀过滤器、出口和逃犯
   冻结计数器。可交付成果：带有 SHA-256 摘要的 PDF/CSV 快照。
2. **M2 — 清单生命周期（45 分钟）：** 构建并验证注册商清单，
   通过 `scripts/sns_zonefile_skeleton.py` 生成解析器骨架。可交付成果：
   git diff 显示骨架 + GAR 证据。
3. **M3——纠纷演练（40分钟）：** 模拟监护人冻结+申诉、抓捕
   监护人 CLI 日志位于 `artifacts/sns/training/<suffix>/<cycle>/logs/` 下方。
4. **M4 — 附件捕获（25 分钟）：** 导出仪表板 JSON 并运行：

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   可交付成果：更新附件 Markdown + 监管 + 门户备忘录块。

## 2. 本地化工作流程

- 语言：`ar`、`es`、`fr`、`ja`、`pt`、`ru`、`ur`。
- 每个翻译都位于源文件旁边
  （`docs/source/sns/training_collateral.<lang>.md`）。更新 `status` +
  刷新后为`translation_last_reviewed`。
- 每种语言的资产属于
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/`（幻灯片/、工作簿/、
  录音/、日志/)。
- 编辑英文后运行`python3 scripts/sync_docs_i18n.py --lang <code>`
  源以便翻译者看到新的哈希值。

### 交货清单

1. 本地化后更新翻译存根 (`status: complete`)。
2. 将幻灯片导出为 PDF 并上传到每种语言的 `slides/` 目录。
3. 记录≤10分钟的KPI演练；来自语言存根的链接。
4. 标记为 `sns-training` 的文件治理票证，包含幻灯片/工作簿
   摘要、记录链接和附件证据。

## 3. 培训资产

- 幻灯片大纲：`docs/examples/sns_training_template.md`。
- 工作簿模板：`docs/examples/sns_training_workbook.md`（每个与会者一份）。
- 邀请+提醒：`docs/examples/sns_training_invite_email.md`。
- 评估表：`docs/examples/sns_training_eval_template.md`（回复
  存档于 `artifacts/sns/training/<suffix>/<cycle>/feedback/` 下）。

## 4. 调度和指标

|循环 |窗口|指标|笔记|
|--------|--------|---------|--------|
| 2026-03 | KPI 审核后 |出席率%，附件摘要已记录| `.sora` + `.nexus` 队列 |
| 2026-06 |预 `.dao` GA |财务准备度≥90% |包括政策更新|
| 2026-09 |扩展|争议演练<20分钟，附件SLA≤2天 |与 SN-7 激励措施保持一致 |

在 `docs/source/sns/reports/sns_training_feedback.md` 中捕获匿名反馈
因此后续团队可以改进本地化和实验室。