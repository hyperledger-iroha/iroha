---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 中继激励回滚计划

如果治理要求，请使用此剧本禁用自动中继支付
停止或遥测护栏失火。

1. **冻结自动化。** 停止每个 Orchestrator 主机上的激励守护进程
   （`systemctl stop soranet-incentives.service` 或同等容器
   部署）并确认该进程不再运行。
2. **清空待处理指令。** 运行
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   以确保没有未完成的付款指示。存档结果
   Norito 用于审计的有效负载。
3. **撤销治理批准。** 编辑`reward_config.json`，设置
   `"budget_approval_id": null`，并重新部署配置
   `iroha app sorafs incentives service init`（或 `update-config`，如果运行
   长寿命守护进程）。支付引擎现在无法关闭
   `MissingBudgetApprovalId`，因此守护进程拒绝铸造支出，直到出现新的
   批准哈希值已恢复。记录 git commit 和 SHA-256
   修改事件日志中的配置。
4. **通知索拉议会。** 附上耗尽的支付账本，影子运行
   报告和简短的事件摘要。议会会议纪要必须注明哈希值
   已撤销的配置和守护进程停止的时间。
5. **回滚验证。** 保持守护进程禁用，直到：
   - 遥测警报 (`soranet_incentives_rules.yml`) 为绿色，持续时间 >=24 小时，
   - 国库调节报告显示零缺失转移，以及
   - 议会批准新的预算哈希。

一旦治理重新发布预算批准哈希，请更新 `reward_config.json`
使用新的摘要，在最新的遥测数据上重新运行 `shadow-run` 命令，
并重新启动奖励守护进程。