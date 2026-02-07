---
lang: zh-hans
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS 培训幻灯片模板

此 Markdown 大纲反映了主持人应适应的幻灯片
他们的语言群体。将这些部分复制到 Keynote/PowerPoint/Google 中
根据需要滑动并本地化要点、屏幕截图和图表。

## 标题幻灯片
- 计划：“Sora 名称服务入门”
- 字幕：指定后缀+周期（例如，`.sora — 2026‑03`）
- 演讲者+隶属关系

## KPI导向
- `docs/portal/docs/sns/kpi-dashboard.md` 的屏幕截图或嵌入
- 解释后缀过滤器的项目符号列表、ARPU 表、冻结跟踪器
- 用于导出 PDF/CSV 的标注

## 清单生命周期
- 图表：注册商 → Torii → 治理 → DNS/网关
- 参考 `docs/source/sns/registry_schema.md` 的步骤
- 带注释的示例清单摘录

## 争议和冻结演习
- 监护人干预流程图
- 参考 `docs/source/sns/governance_playbook.md` 的清单
- 冻结工单时间线示例

## 附件捕获
- 显示 `cargo xtask sns-annex ... --portal-entry ...` 的命令片段
- 提醒将 Grafana JSON 归档到 `artifacts/sns/regulatory/<suffix>/<cycle>/` 下
- 链接至 `docs/source/sns/reports/.<suffix>/<cycle>.md`

## 后续步骤
- 培训反馈链接（参见 `docs/examples/sns_training_eval_template.md`）
- Slack/Matrix 通道手柄
- 即将到来的里程碑日期