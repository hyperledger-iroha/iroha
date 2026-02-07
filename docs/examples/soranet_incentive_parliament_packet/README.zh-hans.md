---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet 中继激励议会包

该捆绑包包含索拉议会批准所需的文物
自动中继支付（SNNet-7）：

- `reward_config.json` - Norito-可序列化奖励引擎配置，准备就绪
  由 `iroha app sorafs incentives service init` 摄取。的
  `budget_approval_id` 与治理记录中列出的哈希值匹配。
- `shadow_daemon.json` - 重放消耗的受益人和债券映射
  线束 (`shadow-run`) 和生产守护进程。
- `economic_analysis.md` - 2025-10 -> 2025-11 的公平性摘要
  阴影模拟。
- `rollback_plan.md` - 用于禁用自动支付的操作手册。
- 支持文物：`docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`，
  `dashboards/grafana/soranet_incentives.json`，
  `dashboards/alerts/soranet_incentives_rules.yml`。

## 完整性检查

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

将摘要与议会会议纪要中记录的值进行比较。验证
影子运行签名，如中所述
`docs/source/soranet/reports/incentive_shadow_run.md`。

## 更新数据包

1. 每当奖励权重、基本支出或
   批准哈希更改。
2. 重新运行 60 天阴影模拟，将 `economic_analysis.md` 更新为
   新发现，并提交 JSON + 分离签名对。
3. 将更新后的捆绑包与天文台仪表板一起提交给议会
   寻求重新批准时的出口。