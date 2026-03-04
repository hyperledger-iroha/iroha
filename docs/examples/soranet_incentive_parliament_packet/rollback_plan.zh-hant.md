---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 中繼激勵回滾計劃

如果治理要求，請使用此劇本禁用自動中繼支付
停止或遙測護欄失火。

1. **凍結自動化。 ** 停止每個 Orchestrator 主機上的激勵守護進程
   （`systemctl stop soranet-incentives.service` 或同等容器
   部署）並確認該進程不再運行。
2. **清空待處理指令。 ** 運行
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   以確保沒有未完成的付款指令。存檔結果
   Norito 用於審計的有效負載。
3. **撤銷治理批准。 ** 編輯`reward_config.json`，設置
   `"budget_approval_id": null`，並重新部署配置
   `iroha app sorafs incentives service init`（或 `update-config`，如果運行
   長壽命守護進程）。支付引擎現在無法關閉
   `MissingBudgetApprovalId`，因此守護進程拒絕鑄造支出，直到出現新的
   批准哈希值已恢復。記錄 git commit 和 SHA-256
   修改事件日誌中的配置。
4. **通知索拉議會。 ** 附上耗盡的支付賬本，影子運行
   報告和簡短的事件摘要。議會會議紀要必須註明哈希值
   已撤銷的配置和守護進程停止的時間。
5. **回滾驗證。 ** 保持守護進程禁用，直到：
   - 遙測警報 (`soranet_incentives_rules.yml`) 為綠色，持續時間 >=24 小時，
   - 國庫調節報告顯示零缺失轉移，以及
   - 議會批准新的預算哈希。

一旦治理重新發布預算批准哈希，請更新 `reward_config.json`
使用新的摘要，在最新的遙測數據上重新運行 `shadow-run` 命令，
並重新啟動獎勵守護進程。