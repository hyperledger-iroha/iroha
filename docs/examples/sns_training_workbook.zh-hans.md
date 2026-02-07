---
lang: zh-hans
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS 培训工作簿模板

使用本工作簿作为每个培训队列的规范讲义。更换
在分发给与会者之前添加占位符 (`<...>`)。

## 会议详细信息
- 后缀：`<.sora | .nexus | .dao>`
- 周期：`<YYYY-MM>`
- 语言：`<ar/es/fr/ja/pt/ru/ur>`
- 协调员：`<name>`

## 实验 1 — KPI 导出
1. 打开门户 KPI 仪表板 (`docs/portal/docs/sns/kpi-dashboard.md`)。
2. 按后缀 `<suffix>` 和时间范围 `<window>` 过滤。
3. 导出 PDF + CSV 快照。
4. 在此处记录导出的 JSON/PDF 的 SHA-256：`______________________`。

## 实验 2 — 清单演习
1. 从 `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` 获取示例清单。
2. 使用 `cargo run --bin sns_manifest_check -- --input <file>` 进行验证。
3. 使用 `scripts/sns_zonefile_skeleton.py` 生成解析器骨架。
4. 粘贴差异摘要：
   ```
   <git diff output>
   ```

## 实验 3 — 争议模拟
1. 使用监护人 CLI 启动冻结（案例 ID `<case-id>`）。
2. 记录争议哈希：`______________________`。
3. 将证据日志上传至`artifacts/sns/training/<suffix>/<cycle>/logs/`。

## 实验室 4 — 附件自动化
1. 导出 Grafana 仪表板 JSON 并将其复制到 `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`。
2. 运行：
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
3.粘贴附件路径+SHA-256输出：`________________________________`。

## 反馈说明
- 什么是不清楚的？
- 哪些实验室随着时间的推移而运行？
- 观察到工具错误吗？

将完成的工作簿返还给辅导员；他们属于
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`。