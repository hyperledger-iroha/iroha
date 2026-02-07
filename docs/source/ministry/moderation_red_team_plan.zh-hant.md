---
lang: zh-hant
direction: ltr
source: docs/source/ministry/moderation_red_team_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ecf637fa28f68a997367118163224caecc6c71c604d5e7ece409941d5374f44
source_last_modified: "2025-12-29T18:16:35.978869+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team & Chaos Drill Plan
summary: Execution plan for roadmap item MINFO-9 covering recurring adversarial campaigns, telemetry hooks, and reporting requirements.
translator: machine-google-reviewed
---

# 紅隊與混亂演習 (MINFO-9)

路線圖參考：**MINFO-9 — 審核紅隊和混亂演習**（目標為 2026 年第 3 季度）

信息部必須開展可重複的對抗性活動，強調人工智能審核管道、賞金計劃、網關和治理控制。該計劃通過定義直接與現有審核儀表板 (`dashboards/grafana/ministry_moderation_overview.json`) 和緊急規範工作流程相關的範圍、節奏、演練模板和證據要求，彌補了路線圖中指出的“🈳 未開始”差距。

## 目標和可交付成果

- 在將覆蓋範圍擴大到其他威脅類別之前，進行季度紅隊演習，涵蓋多部分走私企圖、賄賂/上訴篡改和網關旁道探測。
- 捕獲每次演練的確定性工件（CLI 日誌、Norito 清單、Grafana 導出、SoraFS CID），並通過 `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` 將其歸檔。
- 將演練輸出反饋到校準清單 (`docs/examples/ai_moderation_calibration_*.json`) 和拒絕名單策略中，以便修復任務成為可追踪的路線圖票證。
- 有線警報/運行手冊集成，以便故障與 MINFO-1 儀表板和 Alertmanager 包 (`dashboards/alerts/ministry_moderation_rules.yml`) 一起出現。

## 範圍和依賴關係

- **測試中的系統：** SoraFS 攝取/編排器路徑、`docs/source/sorafs_ai_moderation_plan.md` 中定義的 AI 審核運行器、拒絕名單/Merkle 執行、申訴金庫工具和網關速率限制。
- **先決條件：** 緊急規範和 TTL 策略 (`docs/source/ministry/emergency_canon_policy.md`)、調節校準裝置和 Torii 模擬線束奇偶校驗，以便在混亂運行期間可以重播可重現的有效負載。
- **超出範圍：** SoraDNS 政策、Kaigi 會議或非部委通信渠道（在 SNNet 和 DOCS-SORA 計劃下單獨跟踪）。

## 角色和職責

|角色 |職責|主要所有者 |備份|
|------|--------------------|----------------|--------|
|鑽探總監|批准場景列表、分配紅隊成員、簽署運行手冊 |部安全負責人|副主持人|
|對抗細胞 |製作有效負載、運行攻擊、記錄證據 |安防工程行業協會|志願者經營者|
|可觀察性主管 |監控儀表板/警報、捕獲 Grafana 導出、記錄事件時間表 | SRE / 可觀察性 TL |待命 SRE |
|值班主持人 |推動升級流程、驗證覆蓋請求、更新緊急規範記錄 |事件指揮官|預備役指揮官 |
|報告抄寫員|填充 `docs/source/ministry/reports/moderation_red_team_template.md` 下的模板、鏈接工件、打開後續問題 |文檔/開發版本 |產品聯絡 |

## 節奏和時間軸|相|目標窗口|主要活動|文物|
|--------|-------------|----------------|------------------------|
| **計劃** | T−4 週 |選擇場景，通過 `scripts/ministry/scaffold_red_team_drill.py` 刷新有效負載固定裝置、腳手架工件，並試運行鑽頭鏡像的遙測助手（`scripts/telemetry/check_redaction_status.py`、`ci/run_android_telemetry_chaos_prep.sh`）|場景簡報、票務跟踪器 |
| **準備好** | T−1 週 |鎖定參與者、階段 SoraFS/Torii 沙箱、凍結儀表板/警報哈希 |準備好清單、儀表板摘要 |
| **執行** |演習日（4 小時）|啟動對抗流程、收集 Alertmanager 通知、捕獲 Torii/CLI 跟踪、強制執行覆蓋批准 |實時日誌、Grafana 快照 |
| **恢復** | T+1 天 |將覆蓋恢復、清理數據集、將工件存檔至 `artifacts/ministry/red-team/<YYYY-MM>/` 和 SoraFS |證據包，清單 |
| **報告** | T+1週|從模板發布 Markdown 報告、記錄修復票證、更新 roadmap/status.md |報告文件、Jira/GitHub 鏈接 |

季度演習（三月/六月/九月/十二月）至少進行；高風險發現會觸發遵循相同證據工作流程的臨時運行。

## 場景庫（初始）

|場景|描述 |成功信號|證據輸入|
|----------|-------------|-----------------|------------------|
|多部分走私 |區塊鏈遍布 SoraFS 提供商，其多態有效負載試圖隨著時間的推移繞過 AI 過濾器。練習 Orchestrator 範圍獲取、審核 TTL 和拒絕列表傳播。 |用戶交付前檢測到走私；發出拒絕名單增量； `ministry_moderation_overview` 在 SLA 範圍內發出火災警報。 | CLI 重放日誌、塊清單、拒絕列表差異、來自 `sorafs.fetch.*` 儀表板的跟踪 ID。 |
|賄賂和上訴篡改|成對的惡意版主試圖批准因賄賂而導致的越權行為；測試資金流量、推翻批准和審計日誌記錄。 |使用強制性證據記錄覆蓋，標記國庫轉移，記錄治理投票。 | Norito 覆蓋記錄、`docs/source/ministry/volunteer_brief_template.md` 更新、國庫分類賬條目。 |
|網關旁路探測|模擬流氓發布商測量緩存時間和 TTL 以推斷經過審核的內容。在 SNNet-15 之前練習 CDN/網關強化。 |速率限制和異常儀表板突出顯示探針；管理 CLI 顯示策略執行情況；沒有內容洩露。 |網關訪問日誌、Grafana `ministry_gateway_observability` 面板的抓取、捕獲數據包跟踪 (pcap) 以供離線查看。 |

一旦最初的三個場景從學習節奏中畢業，未來的迭代將添加 `honey-payload beacons`、`AI adversarial prompt floods` 和 `SoraFS metadata poisoning`。

## 執行清單1. **預鑽孔**
   - 確認運行手冊+場景文檔已發布並獲得批准。
   - 快照儀表板 (`dashboards/grafana/ministry_moderation_overview.json`) 和 `artifacts/ministry/red-team/<YYYY-MM>/dashboards/` 的警報規則。
   - 使用 `scripts/ministry/scaffold_red_team_drill.py` 創建報告 + 工件目錄，然後記錄為演練準備的任何夾具包的 SHA256 摘要。
   - 在註入對抗性有效負載之前驗證拒絕名單 Merkle 根源和緊急規範註釋。
2. **演習期間**
   - 將每個操作（時間戳、操作員、命令）記錄到實時日誌（共享文檔或 `docs/source/ministry/reports/tmp/<timestamp>.md`）中。
   - 捕獲 Torii 響應和 AI 審核裁決，包括請求 ID、模型名稱和風險評分。
   - 執行升級工作流程（覆蓋請求 → 指揮官批准 → `emergency_canon_policy` 更新）。
   - 觸發至少一項警報清除活動和文檔響應延遲。
3. **演習後**
   - 清除覆蓋，將拒絕列表條目回滾到生產值，並驗證警報是否安靜。
   - 導出 Grafana/Alertmanager 歷史記錄、CLI 日誌、Norito 清單，並將它們附加到證據包。
   - 向所有者提出補救問題（高/中/低）並註明截止日期；最終報告的鏈接。

## 遙測、指標和證據

- **儀表板：** `dashboards/grafana/ministry_moderation_overview.json` + 未來的 `ministry_red_team_heatmap.json`（佔位符）捕獲實時信號。每次鑽取導出 JSON 快照。
- **警報：** `dashboards/alerts/ministry_moderation_rules.yml` 以及即將推出的 `ministry_red_team_rules.yml` 必須包含引用演練 ID 和場景的註釋，以簡化審核。
- **Norito 工件：** 將每個鑽探運行編碼為 `RedTeamDrillV1` 事件（規範即將推出），因此 Torii/CLI 導出是確定性的，並且可以與治理共享。
- **報告模板：** 複製 `docs/source/ministry/reports/moderation_red_team_template.md` 並填寫場景、指標、證據摘要、補救狀態和治理簽核。
- **存檔：** 將工件存儲在 `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/` 中（日誌、CLI 輸出、Norito 捆綁包、儀表板導出），並在治理批准後發布匹配的 SoraFS CAR 清單以供公眾審查。

## 自動化和後續步驟1. 在 `scripts/ministry/` 下實施幫助程序腳本來播種有效負載固定裝置、切換拒絕列表條目並收集 CLI/Grafana 導出。 （`scaffold_red_team_drill.py`、`moderation_payload_tool.py` 和 `check_red_team_reports.py` 現在涵蓋腳手架、有效負載捆綁、拒絕名單修補和占位符強制執行；`export_red_team_evidence.py` 通過 Grafana API 支持添加了缺少的儀表板/日誌導出，因此證據清單保持確定性。）
2. 使用 `ci/check_ministry_red_team.sh` 擴展 CI，以在合併報告之前驗證模板完整性和證據摘要。 ✅（`scripts/ministry/check_red_team_reports.py` 強制刪除所有已提交的鑽探報告中的佔位符。）
3. 將 `ministry_red_team_status` 部分添加到 `status.md`，以顯示即將進行的演練、未完成的修復項目和上次運行指標。
4. 將鑽探元數據集成到透明度管道中，以便季度報告可以參考最新的混亂結果。
5. 將演練報告直接輸入 `cargo xtask ministry-transparency ingest --red-team-report <path>...`，​​​​以便經過淨化的季度指標和治理清單將演練 ID、證據包和儀表板 SHA 與現有賬本/申訴/拒絕列表源一起攜帶。

一旦這些步驟落地，MINFO-9 就會從🈳 未開始轉變為 🈺 進行中，並具有可追踪的工件和可衡量的成功標準。