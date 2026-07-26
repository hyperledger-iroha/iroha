---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-12-29T18:16:35.087502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 经济分析 - 2025-10 -> 2025-11 Shadow Run

来源文物：`docs/examples/soranet_incentive_shadow_run.json`（签名+
公钥在同一目录中）。每个中继重放 60 个 epoch 的模拟
奖励引擎固定到 `RewardConfig` 记录在
`reward_config.json`。

## 分布总结

- **总支出：** 5,160 XOR 超过 360 个奖励时期。
- **公平包络线：**基尼系数0.121；顶级继电器份额 23.26%
  （远低于 30% 的治理护栏）。
- **可用性：**机队平均96.97%，所有中继保持在94%以上。
- **带宽：** 机群平均 91.20%，表现最低的为 87.23%
  在计划维护期间；处罚是自动实施的。
- **合规噪音：** 观察到 9 次警告时期和 3 次暂停
  转化为支出减少；没有继电器超过 12 次警告上限。
- **操作卫生：**没有由于丢失而跳过指标快照
  配置、债券或重复项；没有发出任何计算器错误。

## 观察结果

- 暂停对应于继电器进入维护模式的时期。的
  支付引擎在这些时期发出零支付，同时保留
  影子运行 JSON 中的审计跟踪。
- 警告处罚使受影响的支出减少 2%；由此产生的
  由于正常运行时间/带宽权重（650/350
  每千）。
- 带宽方差跟踪匿名防护热图。表现最差的人
  (`6666...6666`) 在窗口上保留 620 XOR，高于 0.6x 下限。
- 延迟敏感警报 (`SoranetRelayLatencySpike`) 仍低于警告
  整个窗口的阈值；相关仪表板被捕获在
  `dashboards/grafana/soranet_incentives.json`。

## 正式发布前的建议操作

1. 继续运行每月的影子重播并更新工件集和此
   分析机队构成是否发生变化。
2. 在路线图中引用的 Grafana 警报套件上进行自动支付
   （`dashboards/alerts/soranet_incentives_rules.yml`）；将屏幕截图复制到
   寻求更新时的治理会议纪要。
3. 如果基本奖励、正常运行时间/带宽权重或
   合规处罚变化 >=10%。