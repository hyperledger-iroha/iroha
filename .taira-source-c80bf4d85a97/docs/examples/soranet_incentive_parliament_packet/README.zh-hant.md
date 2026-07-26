---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet 中繼激勵議會包

該捆綁包包含索拉議會批准所需的文物
自動中繼支付（SNNet-7）：

- `reward_config.json` - Norito-可序列化獎勵引擎配置，準備就緒
  由 `iroha app sorafs incentives service init` 攝取。的
  `budget_approval_id` 與治理記錄中列出的哈希值匹配。
- `shadow_daemon.json` - 重放消耗的受益人和債券映射
  線束 (`shadow-run`) 和生產守護進程。
- `economic_analysis.md` - 2025-10 -> 2025-11 的公平性摘要
  陰影模擬。
- `rollback_plan.md` - 用於禁用自動支付的操作手冊。
- 支持文物：`docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`，
  `dashboards/grafana/soranet_incentives.json`，
  `dashboards/alerts/soranet_incentives_rules.yml`。

## 完整性檢查

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

將摘要與議會會議紀要中記錄的值進行比較。驗證
影子運行簽名，如中所述
`docs/source/soranet/reports/incentive_shadow_run.md`。

## 更新數據包

1. 每當獎勵權重、基本支出或
   批准哈希更改。
2. 重新運行 60 天陰影模擬，將 `economic_analysis.md` 更新為
   新發現，並提交 JSON + 分離簽名對。
3. 將更新後的捆綁包與天文台儀表板一起提交給議會
   尋求重新批准時的出口。