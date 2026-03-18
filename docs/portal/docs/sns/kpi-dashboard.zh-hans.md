---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-12-29T18:16:35.173090+00:00"
translation_last_reviewed: 2026-02-07
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
translator: machine-google-reviewed
---

# Sora 名称服务 KPI 仪表板

KPI 仪表板为管理者、监护人和监管者提供了一个统一的位置
在每月附件节奏之前审查采用情况、错误和收入信号
(SN-8a)。 Grafana 定义位于存储库中
`dashboards/grafana/sns_suffix_analytics.json` 与门户镜像相同
通过嵌入式 iframe 进行面板，因此体验与内部 Grafana 相匹配
实例。

## 过滤器和数据源

- **后缀过滤器** – 驱动 `sns_registrar_status_total{suffix}` 查询，以便
  `.sora`、`.nexus`和`.dao`可以独立检查。
- **批量发布过滤器** – 限制 `sns_bulk_release_payment_*` 指标的范围，以便
  财务部门可以核对特定注册商清单。
- **指标** – 取自 Torii (`sns_registrar_status_total`,
  `torii_request_duration_seconds`), 监护人 CLI (`guardian_freeze_active`),
  `sns_governance_activation_total`，以及批量入职帮助程序指标。

## 面板

1. **注册（过去 24 小时）** – 成功注册商活动的数量
   选定的后缀。
2. **治理激活（30天）** – 章程/附录动议记录
   命令行界面。
3. **注册商吞吐量** – 每个后缀的注册商成功操作率。
4. **注册商错误模式** – 5 分钟错误率标记
   `sns_registrar_status_total` 计数器。
5. **Guardian 冻结窗口** – 实时选择器，其中 `guardian_freeze_active`
   报告未结冻结票证。
6. **按资产划分的净支付单位** – 按资产报告的总计
   每项资产 `sns_bulk_release_payment_net_units`。
7. **每个后缀的批量请求** – 每个后缀 ID 的清单卷。
8. **每个请求的净单位** – 来自发布的 ARPU 样式计算
   指标。

## 每月 KPI 审核清单

财务负责人在每个月的第一个星期二进行定期审核：

1. 打开门户的 **分析 → SNS KPI** 页面（或 Grafana 仪表板 `sns-kpis`）。
2. 捕获注册商吞吐量和收入表的 PDF/CSV 导出。
3. 比较 SLA 违规的后缀（错误率峰值、冻结选择器 >72 小时、
   ARPU 增量 >10%）。
4. 日志摘要 + 相关附件条目中的行动项目
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。
5. 将导出的仪表板工件附加到附件提交并将它们链接到
   理事会议程。

如果审核发现存在 SLA 违规行为，请为受影响的用户提交 PagerDuty 事件
所有者（登记员值班经理、值班监护人或管家计划负责人）以及
在附件日志中跟踪修复情况。