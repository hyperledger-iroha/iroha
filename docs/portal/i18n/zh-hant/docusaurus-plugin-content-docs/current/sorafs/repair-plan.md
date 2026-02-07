---
id: repair-plan
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
鏡子 `docs/source/sorafs_repair_plan.md`。保持兩個版本同步，直到 Sphinx 集退役。
:::

## 治理決策生命週期
1. 升級修復創建斜杠提案草案並打開爭議窗口。
2. 治理投票者在爭議窗口期間提交批准/拒絕投票。
3. 在 `escalated_at_unix + dispute_window_secs` 中，決策是確定性計算的：最少投票者、批准超過拒絕，並且批准率滿足法定人數閾值。
4. 批准的決定打開上訴窗口； `approved_at_unix + appeal_window_secs` 之前記錄的上訴將決定標記為已上訴。
5. 處罰上限適用於所有提案；超過上限的提交將被拒絕。

## 治理升級政策
升級策略源自 `iroha_config` 中的 `governance.sorafs_repair_escalation`，並對每個修復削減提案強制執行。

|設置|默認|意義|
|---------|---------|---------|
| `quorum_bps` | 6667 |計票中的最低支持率（基點）。 |
| `minimum_voters` | 3 |做出決定所需的最小數量的不同選民。 |
| `dispute_window_secs` | 86400 |升級後到投票最終確定之前的時間（秒）。 |
| `appeal_window_secs` | 604800 |批准後接受申訴的時間（秒）。 |
| `max_penalty_nano` | 1,000,000,000 |修復升級允許的最大斜線懲罰（納米異或）。 |

- 調度程序生成的提案上限為 `max_penalty_nano`；審核員提交的超出上限的意見將被拒絕。
- 投票記錄以確定性排序（`voter_id` 排序）存儲在 `repair_state.to` 中，因此所有節點都得出相同的決策時間戳和結果。