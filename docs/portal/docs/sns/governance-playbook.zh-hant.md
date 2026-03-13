---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa4560c51d066ce5c63581dd102aef4e70786d140790fb157323df2553b15f4b
source_last_modified: "2026-01-28T17:11:30.700959+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id：治理手冊
標題：Sora 名稱服務治理手冊
sidebar_label：治理手冊
描述：SN-1/SN-6 引用的理事會、監護人、管理員和註冊員工作流程運行手冊。
---

:::注意規範來源
此頁面鏡像 `docs/source/sns/governance_playbook.md`，現在用作
規範門戶副本。翻譯 PR 的源文件仍然存在。
:::

# Sora 名稱服務治理手冊 (SN-6)

**狀態：** 起草於 2026 年 3 月 24 日 — SN-1/SN-6 準備情況的實時參考  
**路線圖鏈接：** SN-6“合規性和爭議解決”、SN-7“解析器和網關同步”、ADDR-1/ADDR-5 地址政策  
**先決條件：** [`registry-schema.md`](./registry-schema.md) 中的註冊架構、[`registrar-api.md`](./registrar-api.md) 中的註冊商 API 合約、[`address-display-guidelines.md`](./address-display-guidelines.md) 中的地址用戶體驗指南以及賬戶結構[`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) 中的規則。

本手冊描述了 Sora 名稱服務 (SNS) 治理機構如何採用
章程、批准註冊、升級爭議並證明解決者和
網關狀態保持同步。它滿足了路線圖的要求
`sns governance ...` CLI、Norito 清單和審核工件共享一個
N1（公開發布）之前面向運營商的參考。

## 1. 範圍和受眾

該文件的目標是：

- 對章程、後綴政策和爭議進行投票的治理委員會成員
  結果。
- 發出緊急凍結和審查撤銷的監護委員會成員。
- 後綴管理員，負責管理註冊隊列、批准拍賣和管理
  收入分成。
- 解析器/網關運營商負責 SoraDNS 傳播、GAR 更新、
  和遙測護欄。
- 合規、財務和支持團隊必須證明每個
  治理行動留下了可審計的 Norito 文物。

它涵蓋內測 (N0)、公開發布 (N1) 和擴展 (N2) 階段
通過將每個工作流程鏈接到所需的證據，在 `roadmap.md` 中枚舉，
儀表板和升級路徑。

## 2. 角色和聯繫圖

|角色 |核心職責|主要文物和遙測|升級 |
|------|----------------------|--------------------------------|------------|
|治理委員會|起草和批准章程、後綴政策、爭議裁決和管家輪換。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`，通過 `sns governance charter submit` 存儲的議會選票。 |理事會主席+治理日程跟踪器。 |
|監護委員會|發佈軟/硬凍結、緊急規則和 72 小時審查。 | `sns governance freeze` 發出的監護人票證，覆蓋 `artifacts/sns/guardian/*` 下記錄的清單。 |監護人值班輪換（≤15 分鐘 ACK）。 |
|後綴管家|運行註冊商隊列、拍賣、定價等級和客戶通信；確認合規性。 | `SuffixPolicyV1` 中的管家政策、定價參考表、存儲在監管備忘錄旁邊的管家致謝。 | Steward 程序主線 + 後綴特定的 PagerDuty。 |
|註冊商和計費操作 |操作 `/v2/sns/*` 端點、協調付款、發出遙測數據並維護 CLI 快照。 |註冊商 API（[`registrar-api.md`](./registrar-api.md)）、`sns_registrar_status_total` 指標、在 `artifacts/sns/payments/*` 下存檔的付款證明。 |登記官值班經理和財務聯絡人。 |
|解析器和網關運營商 |保持 SoraDNS、GAR 和網關狀態與註冊商事件保持一致；流透明度指標。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 |解析器 SRE 待命 + 網關操作橋接。 |
|財務與金融 |應用 70/30 的收入分成、推薦分拆、稅務/財務申報和 SLA 證明。 |應計收入清單、Stripe/國庫出口、`docs/source/sns/regulatory/` 下的季度 KPI 附錄。 |財務總監+合規官。 |
|合規與監管聯絡|跟踪全球義務（EU DSA 等）、更新 KPI 契約並歸檔披露。 | `docs/source/sns/regulatory/` 中的監管備忘錄、參考資料、`ops/drill-log.md` 桌面排練條目。 |合規計劃負責人。 |
|支持/SRE 待命 |處理事件（衝突、計費偏差、解析器中斷）、協調客戶消息傳遞和自己的操作手冊。 |事件模板、`ops/drill-log.md`、上演的實驗室證據、在 `incident/` 下存檔的 Slack/作戰室記錄。 | SNS on-call 輪換+SRE 管理。 |

## 3. 規範工件和數據源

|文物|地點 |目的|
|----------|----------|---------|
|章程 + KPI 附錄 | `docs/source/sns/governance_addenda/` | CLI 投票引用的版本控制的簽署章程、KPI 契約和治理決策。 |
|註冊表架構| [`registry-schema.md`](./registry-schema.md) |規範 Norito 結構（`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`）。 |
|註冊商合同 | [`registrar-api.md`](./registrar-api.md) | REST/gRPC 負載、`sns_registrar_status_total` 指標和治理掛鉤期望。 |
|地址用戶體驗指南 | [`address-display-guidelines.md`](./address-display-guidelines.md) | Canonical I105（首選）+ 壓縮（`sora`，第二好）錢包/瀏覽器鏡像的渲染。 |
| SoraDNS / GAR 文檔 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |確定性主機派生、透明尾部工作流程和警報規則。 |
|監管備忘錄| `docs/source/sns/regulatory/` |司法管轄區接收說明（例如，EU DSA）、管理員確認、模板附件。 |
|鑽井日誌| `ops/drill-log.md` |階段退出前需要記錄混亂情況和 IR 排練。 |
|文物存儲 | `artifacts/sns/` |付款證明、監護人票證、解析器差異、KPI 導出以及 `sns governance ...` 生成的簽名 CLI 輸出。 |

所有治理行動必須至少引用上表中的一項工件
因此審計人員可以在 24 小時內重建決策軌跡。

## 4. 生命週期手冊

### 4.1 章程和管理員動議

|步驟|業主| CLI / 證據 |筆記|
|------|------|----------------|--------|
|附錄草案和 KPI 增量 |理事會報告員 + 管家領導 |存儲在 `docs/source/sns/governance_addenda/YY/` 下的 Markdown 模板 |包括 KPI 契約 ID、遙測掛鉤和激活條件。 |
|提交提案 |理事會主席 | `sns governance charter submit --input SN-CH-YYYY-NN.md`（生成 `CharterMotionV1`）| CLI 發出存儲在 `artifacts/sns/governance/<id>/charter_motion.json` 下的 Norito 清單。 |
|投票和監護人致謝|理事會+監護人| `sns governance ballot cast --proposal <id>` 和 `sns governance guardian-ack --proposal <id>` |附上哈希會議記錄和法定人數證明。 |
|管家驗收|管家計劃| `sns governance steward-ack --proposal <id> --signature <file>` |後綴策略更改前需要；將信封記錄在 `artifacts/sns/governance/<id>/steward_ack.json` 下。 |
|激活 |註冊商操作 |更新 `SuffixPolicyV1`，刷新註冊商緩存，在 `status.md` 中發布說明。 |激活時間戳記錄到 `sns_governance_activation_total`。 |
|審核日誌|合規|如果執行了桌面操作，則將條目附加到 `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` 和演練日誌中。 |包括對遙測儀表板和策略差異的引用。 |

### 4.2 註冊、拍賣和定價批准

1. **預檢：** 註冊商查詢 `SuffixPolicyV1` 以確認定價等級，
   可用條款和寬限/贖回窗口。保持定價表同步
   3/4/5/6–9/10+ 等級表（基本等級 + 後綴係數）記錄在
   路線圖。
2. **密封投標拍賣：** 對於高級池，運行 72 小時承諾/24 小時揭示
   通過 `sns governance auction commit` / `... reveal` 循環。發布提交
   在 `artifacts/sns/auctions/<name>/commit.json` 下列出（僅哈希）所以
   審計員可以驗證隨機性。
3. **付款驗證：** 註冊商驗證 `PaymentProofV1`
   財務分割（70% 財務/30% 管理人員，≤10% 推薦剝離）。
   將 Norito JSON 存儲在 `artifacts/sns/payments/<tx>.json` 下並將其鏈接到
   註冊商回复 (`RevenueAccrualEventV1`)。
4. **治理掛鉤：** 附加 `GovernanceHookV1` 以獲取高級/受保護的名稱
   參考理事會提案 ID 和管理員簽名。缺少掛鉤結果
   在 `sns_err_governance_missing` 中。
5. **激活+解析器同步：** Torii發出註冊表事件後，觸發
   解析器透明度尾部確認傳播的新 GAR/區域狀態
   （參見第 4.5 節）。
6. **客戶披露：** 更新面向客戶的賬本（錢包/資源管理器）
   通過[`address-display-guidelines.md`](./address-display-guidelines.md)中的共享夾具，確保I105和
   壓縮效果圖與文案/QR 指南相符。

### 4.3 續訂、計費和財務對賬- **續訂工作流程：** 註冊商強制執行 30 天寬限期 + 60 天贖回期
  `SuffixPolicyV1` 中指定的窗口。 60 天后荷蘭重新開放序列
  （7 天，10× 費用衰減 15%/天）通過 `sns 治理自動觸發
  重新打開`。
- **收入分配：** 每次續訂或轉讓都會創建一個
  `RevenueAccrualEventV1`。國庫出口（CSV/Parquet）必須與
  這些事件每天都有；將校樣附在 `artifacts/sns/treasury/<date>.json` 上。
- **推薦剝離：** 每個後綴跟踪可選的推薦百分比
  通過將 `referral_share` 添加到管理員策略中。註冊商發出最終的
  將推薦清單拆分並存儲在付款證明旁邊。
- **報告節奏：** 財務發布每月 KPI 附件（註冊、
  續訂、ARPU、爭議/債券利用）
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。儀表板應該從
  相同的導出表，因此 Grafana 數字與分類帳證據匹配。
- **每月 KPI 審核：** 第一個週二檢查點與財務主管配對，
  值班管家和項目 PM。打開[SNS KPI儀表板](./kpi-dashboard.md)
  （`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` 的門戶嵌入），
  導出附件中的註冊商吞吐量+收入表、日誌增量，
  並將文物附加到備忘錄中。如果審核發現，則觸發事件
  違反 SLA（凍結窗口 >72 小時、註冊商錯誤峰值、ARPU 漂移）。

### 4.4 凍結、爭議和上訴

|相|業主|行動與證據|服務水平協議 |
|--------|--------|--------------------|-----|
|軟凍結請求 |管家/支持|將票據 `SNS-DF-<id>` 歸檔，其中包含付款證明、爭議保證金參考和受影響的選擇器。 |攝入後≤4小時。 |
|監護人票|監護板| `sns governance freeze --selector <I105> --reason <text> --until <ts>` 生成簽名的 `GuardianFreezeTicketV1`。將票證 JSON 存儲在 `artifacts/sns/guardian/<id>.json` 下。 | ≤30min ACK，≤2h 執行。 |
|理事會批准|治理委員會|批准或拒絕凍結，將決定鏈接記錄到監護人票證和爭議債券摘要。 |下次理事會會議或異步投票。 |
|仲裁小組|合規+管家|召集 7 名陪審員小組（根據路線圖），並通過 `sns governance dispute ballot` 提交哈希選票。將匿名投票收據附加到事件數據包中。 |保證金存入後≤7 天作出判決。 |
|上訴 |監護人+理事會|上訴使保證金加倍並重複陪審員程序；記錄 Norito 清單 `DisputeAppealV1` 和參考主票證。 | ≤10天。 |
|解凍和修復|註冊商 + 解析器操作 |執行 `sns governance unfreeze --selector <I105> --ticket <id>`，更新註冊器狀態並傳播 GAR/解析器差異。 |判決後立即。 |

緊急大砲（監護人觸發的凍結≤72小時）遵循相同的流程，但
要求理事會進行追溯審查並提供透明度說明
`docs/source/sns/regulatory/`。

### 4.5 解析器和網關傳播

1. **事件掛鉤：** 每個註冊表事件都會發送到解析器事件流
   （`tools/soradns-resolver` SSE）。解析器操作通過以下方式訂閱和記錄差異
   透明度裁剪器 (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **GAR 模板更新：** 網關必須更新引用的 GAR 模板
   `canonical_gateway_suffix()` 並重新簽署 `host_pattern` 列表。存儲差異
   在 `artifacts/sns/gar/<date>.patch` 中。
3. **Zonefile發布：** 使用中描述的zonefile骨架
   `roadmap.md`（名稱、ttl、cid、證明）並將其推送到 Torii/SoraFS。歸檔
   `artifacts/sns/zonefiles/<name>/<version>.json` 下的 Norito JSON。
4. **透明度檢查：** 運行 `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   以確保警報保持綠色。將 Prometheus 文本輸出附加到
   每週透明度報告。
5. **網關審計：**記錄`Sora-*`標頭樣本（緩存策略、CSP、GAR
   摘要）並將它們附加到治理日誌中，以便操作員可以證明
   網關為新名稱提供了預期的護欄。

## 5. 遙測和報告

|信號|來源 |描述/行動|
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii 註冊商處理程序 |註冊、續訂、凍結、轉讓的成功/錯誤計數器；當每個後綴的 `result="error"` 峰值時發出警報。 |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Torii 指標 | API 處理程序的延遲 SLO；從 `torii_norito_rpc_observability.json` 構建的提要儀表板。 |
| `soradns_bundle_proof_age_seconds` 和 `soradns_bundle_cid_drift_total` |旋轉變壓器透明度裁剪器 |檢測過時的證明或 GAR 漂移； `dashboards/alerts/soradns_transparency_rules.yml` 中定義的護欄。 |
| `sns_governance_activation_total` |治理 CLI |每當章程/附錄激活時，計數器就會遞增；用於協調理事會決定與已發布的附錄。 |
| `guardian_freeze_active` 儀表 |守護者 CLI |跟踪每個選擇器的軟/硬凍結窗口；如果值保持 `1` 超出聲明的 SLA，則頁 SRE。 |
| KPI 附件儀表板 |財務/文件|每月匯總與監管備忘錄一起發布；該門戶通過 [SNS KPI 儀表板](./kpi-dashboard.md) 嵌入它們，以便管理員和監管者可以訪問相同的 Grafana 視圖。 |

## 6. 證據和審計要求

|行動|歸檔證據|存儲|
|--------|--------------------|---------|
|章程/政策變更|簽署的 Norito 清單、CLI 記錄、KPI 差異、管理員確認。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|註冊/續訂| `RegisterNameRequestV1` 有效負載，`RevenueAccrualEventV1`，付款證明。 | `artifacts/sns/payments/<tx>.json`，註冊商 API 日誌。 |
|拍賣|提交/揭示清單、隨機種子、獲勝者計算電子表格。 | `artifacts/sns/auctions/<name>/`。 |
|凍結/解凍 |監護人票證、理事會投票哈希值、事件日誌 URL、客戶通信模板。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|解析器傳播 | Zonefile/GAR diff、tailer JSONL 摘錄、Prometheus 快照。 | `artifacts/sns/resolver/<date>/` + 透明度報告。 |
|監管攝入|接收備忘錄、截止日期跟踪器、管理員確認、KPI 變更摘要。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 階段門檢查表

|相|退出標準|證據包|
|--------|-------------|-----------------|
| N0 — 內測 | SN-1/SN-2 註冊表架構、手動註冊器 CLI、監護演練完成。 |章程動議 + 管理員 ACK、註冊商試運行日誌、解析器透明度報告、`ops/drill-log.md` 中的演練條目。 |
| N1 — 公開發布 | `.sora`/`.nexus` 的拍賣 + 固定價格層級、自助服務註冊商、解析器自動同步、計費儀表板。 |定價表差異、註冊商 CI 結果、付款/KPI 附件、透明度定制輸出、事件排練記錄。 |
| N2 — 擴展 | `.dao`、經銷商 API、爭議門戶、管理員記分卡、分析儀表板。 |門戶屏幕截圖、爭議 SLA 指標、管家記分卡導出、參考經銷商政策的更新治理章程。 |

階段退出需要記錄桌面演練（註冊快樂路徑、凍結、
解析器中斷），文物附加到 `ops/drill-log.md`。

## 8. 事件響應和升級

|觸發|嚴重性 |直接所有者 |強制行動 |
|--------|----------|-----------------|--------------------|
|解析器/GAR 漂移或陳舊證明 |嚴重程度1 |解析器SRE+監護板|頁面解析器待命，捕獲尾部輸出，決定是否凍結受影響的名稱，每 30 分鐘發布狀態更新一次。 |
|註冊商中斷、計費失敗或廣泛的 API 錯誤 |嚴重程度1 |登記員值班經理 |停止新的拍賣，切換到手動 CLI，通知管理員/財務人員，將 Torii 日誌附加到事件文檔中。 |
|單名爭議、付款不匹配或客戶升級 |嚴重程度2 |管家 + 支持領導 |收集付款證明，確定是否需要軟凍結，在 SLA 範圍內響應請求者，在爭議跟踪器中記錄結果。 |
|合規審計結果|嚴重程度2 |合規聯絡|起草補救計劃，在 `docs/source/sns/regulatory/` 下提交備忘錄，安排後續理事會會議。 |
|演習或排練|嚴重程度3 |節目PM |執行 `ops/drill-log.md` 中的腳本場景、歸檔工件、將差距標記為路線圖任務。 |

所有事件都必須創建具有所有權的 `incident/YYYY-MM-DD-sns-<slug>.md`
表格、命令日誌以及對整個過程中產生的證據的引用
劇本。

## 9. 參考文獻

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md`（SNS、DG、ADDR 部分）

每當章程措辭、CLI 出現或遙測時，請隨時更新此手冊
合同變更；引用 `docs/source/sns/governance_playbook.md` 的路線圖條目
應始終匹配最新版本。