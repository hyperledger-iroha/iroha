---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Reserve+Rent 政策（路線圖項目 **SFM-6**）現在提供 `sorafs reserve`
CLI 助手加上 `scripts/telemetry/reserve_ledger_digest.py` 轉換器
國債運行可以產生確定性的租金/儲備轉移。此頁面鏡像
`docs/source/sorafs_reserve_rent_plan.md` 中定義的工作流程並解釋
如何將新的傳輸源連接到 Grafana + Alertmanager 中，如此經濟且
治理審核員可以審核每個計費周期。

## 端到端工作流程

1. **報價+賬本投影**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account ih58... \
    --treasury-account ih58... \
    --reserve-account ih58... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   賬本助手附加一個 `ledger_projection` 塊（租金到期，保留
   短缺、充值增量、承保布爾值）加上 Norito `Transfer`
   ISI 需要在國庫賬戶和儲備賬戶之間進行異或運算。

2. **生成摘要 + Prometheus/NDJSON 輸出**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   摘要助手將微異或總計標準化為異或，記錄是否
   預測滿足承保要求，並發出**轉移源**指標
   `sorafs_reserve_ledger_transfer_xor` 和
   `sorafs_reserve_ledger_instruction_total`。當需要多個賬本時
   處理（例如，一批提供者），重複 `--ledger`/`--label` 對和
   幫助程序寫入一個包含每個摘要的 NDJSON/Prometheus 文件，以便
   儀表板吸收了整個週期，無需定製膠水。 `--out-prom`
   文件的目標是節點導出器文本文件收集器 - 將 `.prom` 文件放入
   導出者的監視目錄或將其上傳到遙測存儲桶
   由 Reserve 儀表板作業消耗 — 而 `--ndjson-out` 則提供相同的數據
   有效負載進入數據管道。

3. **發布文物+證據**
   - 將摘要存儲在 `artifacts/sorafs_reserve/ledger/<provider>/` 和鏈接下
     每周經濟報告中的 Markdown 摘要。
   - 將 JSON 摘要附加到租金燃盡表中（以便審核員可以重播
     math）並將校驗和包含在治理證據包中。
   - 如果摘要表明存在充值或承保違規，請參考警報
     ID（`SoraFSReserveLedgerTopUpRequired`，
     `SoraFSReserveLedgerUnderwritingBreach`）並記下哪些傳輸 ISI
     應用。

## 指標 → 儀表板 → 警報

|來源指標 | Grafana面板|警報/政策掛鉤 |筆記|
|----------------|---------------|---------------------|--------|
| `torii_da_rent_base_micro_total`、`torii_da_protocol_reserve_micro_total`、`torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` 中的“DA 租金分配（異或/小時）”|提供每週財政部摘要；儲備流量的峰值傳播到 `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`)。 |
| `torii_da_rent_gib_months_total` | “容量使用情況（GiB 月）”（同一儀表板）|與賬本摘要配對，證明發票存儲與 XOR 傳輸相匹配。 |
| `sorafs_reserve_ledger_rent_due_xor`、`sorafs_reserve_ledger_reserve_shortfall_xor`、`sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` 中的“保留快照 (XOR)”+ 狀態卡 | `SoraFSReserveLedgerTopUpRequired` 當 `requires_top_up=1` 時觸發； `SoraFSReserveLedgerUnderwritingBreach` 當 `meets_underwriting=0` 時觸發。 |
| `sorafs_reserve_ledger_transfer_xor`、`sorafs_reserve_ledger_instruction_total` | “按種類轉移”、“最新轉移明細”以及 `dashboards/grafana/sorafs_reserve_economics.json` 中的承保卡 |即使需要租金/充值，當傳輸饋送缺失或歸零時，`SoraFSReserveLedgerInstructionMissing`、`SoraFSReserveLedgerRentTransferMissing` 和 `SoraFSReserveLedgerTopUpTransferMissing` 也會發出警告；在相同情況下，承保卡會降至 0%。 |

租用周期完成後，刷新 Prometheus/NDJSON 快照，確認
Grafana 面板採用新的 `label`，並附上屏幕截圖 +
租金治理數據包的 Alertmanager ID。這證明了 CLI 投影，
遙測和治理工件都源於**相同的**傳輸源和
使路線圖的經濟儀表板與儲備+租金保持一致
自動化。覆蓋卡應顯示 100%（或 1.0）並且新警報
一旦租金和儲備金充值轉移出現在摘要中，就應該清除。