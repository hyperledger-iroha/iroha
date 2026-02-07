---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI 审核校准报告 - 2026 年 2 月

本报告打包了 **MINFO-1** 的首次校准工件。的
数据集、清单和记分板于 2026 年 2 月 5 日制作，并由
2026年2月10日召开部委理事会，并扎根于治理DAG的高度
`912044`。

## 数据集清单

- **数据集参考：** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **子弹：** `ai-moderation-calibration-202602`
- **条目：** 清单 480、块 12,800、元数据 920、音频 160
- **标签组合：**安全 68%，可疑 19%，升级 13%
- **文物摘要：** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **分布：** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

完整清单位于 `docs/examples/ai_moderation_calibration_manifest_202602.json` 中
并包含治理签名以及发布时捕获的运行者哈希值
时间。

## 记分板摘要

校准使用 opset 17 和确定性种子管道运行。的
完整记分板 JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
记录哈希值和遥测摘要；下表突出显示了最
重要指标。

|模特（家庭）|荆棘|欧洲经委会 |奥罗克 |精准@检疫|召回@升级 |
| -------------- | -----| ---| -----| -------------------- | ---------------- |
| ViT-H/14 安全（视觉）| 0.141 | 0.141 0.031 | 0.031 0.987 | 0.987 0.964 | 0.964 0.912 | 0.912
| LLaVA-1.6 34B 安全（多式联运）| 0.118 | 0.118 0.028 | 0.028 0.978 | 0.978 0.942 | 0.942 0.904 | 0.904
|知觉整体（知觉） | 0.162 | 0.162 0.047 | 0.047 0.953 | 0.953 0.883 | 0.883 0.861 | 0.861

组合指标：`Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。判决结果
整个校准窗口的分布通过率为 91.2%，隔离率为 6.8%，
升级2.0%，符合清单中记录的政策预期
总结。假阳性积压保持为零，漂移分数 (7.1%)
远低于 20% 的警报阈值。

## 阈值和签核

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 治理动议：`MINFO-2026-02-07`
- 由 `ministry-council-seat-03` 在 `2026-02-10T11:33:12Z` 签名

CI 将签名包存储在 `artifacts/ministry/ai_moderation/2026-02/` 中
与审核运行程序二进制文件一起。清单摘要和记分板
在审核和上诉期间必须引用上述哈希值。

## 仪表板和警报

审核 SRE 应导入 Grafana 仪表板：
`dashboards/grafana/ministry_moderation_overview.json` 及其匹配
`dashboards/alerts/ministry_moderation_rules.yml` 中的 Prometheus 警报规则
（测试覆盖范围位于 `dashboards/alerts/tests/ministry_moderation_rules.test.yml` 下）。
这些工件会针对摄取停滞、漂移尖峰和隔离发出警报
队列增长，满足中提出的监控要求
[AI 审核运行器规范](../../ministry/ai-moderation-runner.md)。