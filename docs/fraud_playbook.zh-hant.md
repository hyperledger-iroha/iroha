---
lang: zh-hant
direction: ltr
source: docs/fraud_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ac4c98cc4aa6ab0c34e58e6428d0ee33eb9a0c3fdad9e6958bdc75f2a48dc66
source_last_modified: "2026-01-22T16:26:46.488648+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 欺詐治理手冊

本文檔總結了 PSP 欺詐堆棧所需的腳手架，同時
完整的微服務和 SDK 正在積極開發中。它捕捉到
對分析、審計工作流程和後備程序的期望，以便
即將推出的實施可以安全地插入分類帳。

## 服務概覽

1. **API網關** – 接收同步`RiskQuery`有效負載，將其轉發到
   功能聚合，並將 `FraudAssessment` 響應轉發回賬本
   流動。需要高可用性（雙活）；使用區域對
   確定性哈希以避免請求偏差。
2. **特徵聚合** – 組成用於評分的特徵向量。發出
   僅 `FeatureInput` 哈希值；敏感負載保持在鏈外。可觀測性
   必鬚髮布延遲直方圖、隊列深度計量器和重播計數器
   租戶。
3. **風險引擎** – 評估規則/模型並產生確定性
   `FraudAssessment` 輸出。確保規則執行順序穩定並捕獲
   每個評估 ID 的審核日誌。

## 分析和模型推廣

- **異常檢測**：維護一個標記偏差的流作業
  每個租戶的決策率。將警報輸入治理儀表板和商店
  季度審查摘要。
- **圖形分析**：每晚對關係導出運行圖形遍歷
  識別共謀集群。通過以下方式將結果導出到治理門戶
  `GovernanceExport` 參考支持證據。
- **反饋攝取**：整理手動審核結果和退款報告。
  將它們轉換為特徵增量並將它們合併到訓練數據集中。
  發布攝取狀態指標，以便風險團隊可以發現停滯的提要。
- **模型推廣管道**：自動化候選人評估（離線指標，
  金絲雀評分，回滾準備）。促銷活動應發出簽名
  `FraudAssessment` 樣本集並更新 `model_version` 字段
  `GovernanceExport`。

## 審核員工作流程

1. 快照最新的 `GovernanceExport` 並驗證 `policy_digest` 是否匹配
   風險團隊提供的清單。
2. 驗證規則聚合是否與賬本端決策總數一致
   採樣窗口。
3.審查異常檢測和圖形分析報告是否有突出問題
   問題。記錄升級情況和預期的修復所有者。
4. 簽署審核清單並存檔。將 Norito 編碼的工件存儲在
   可重複性的治理門戶。

## 後備劇本

- **引擎停機**：如果風險引擎不可用超過 60 秒，
  網關應切換到僅審查模式，發出 `AssessmentDecision::Review`
  用於所有請求和警報操作員。
- **遙測差距**：當指標或跟踪落後時（丟失 5 分鐘），
  停止自動型號促銷並通知值班工程師。
- **模型回歸**：如果部署後反饋表明欺詐行為增加
  損失，回滾到之前簽名的模型包並更新路線圖
  並採取糾正措施。

## 數據共享協議

- 維護特定管轄區的附錄，涵蓋保留、加密和
  違反通知 SLA。合作夥伴在接收前必須簽署附錄
  `FraudAssessment` 出口。
- 記錄每個集成的數據最小化實踐（例如，散列
  帳戶標識符、截斷卡號）。
- 每年或在監管要求發生變化時更新協議。

## API 模式

網關現在公開具體的 JSON 信封，這些信封一對一地映射到
`crates/iroha_data_model::fraud` 中實現的 Norito 類型：

- **風險攝入** – `POST /v2/fraud/query` 接受 `RiskQuery` 架構：
  - `query_id`（`[u8; 32]`，十六進制編碼）
  - `subject`（`AccountId`，規範 I105 文字；可選 `@<domain>` 提示或別名）
  - `operation`（標記枚舉匹配 `RiskOperation`；JSON `type`
    鑑別器鏡像枚舉變體）
  - `related_asset`（`AssetId`，可選）
  - `features`（`{ key: String, value_hash: hex32 }` 的數組映射自
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context`（`RiskContext`；攜帶`tenant_id`，可選`session_id`，
    可選 `reason`)
- **風險決策** – `POST /v2/fraud/assessment` 消耗
  `FraudAssessment` 有效負載（也反映在治理導出中）：
  - `query_id`、`engine_id`、`risk_score_bps`、`confidence_bps`、
    `decision`（`AssessmentDecision` 枚舉）、`rule_outcomes`
    （`{ rule_id, score_delta_bps, rationale? }` 數組）
  - `generated_at_ms`
  - `signature`（可選的 base64 包裝 Norito 編碼的評估）
- **治理導出** – `GET /v2/fraud/governance/export` 返回
  當 `governance` 功能啟用時，`GovernanceExport` 結構，捆綁
  活動參數、最新頒布、模型版本、政策摘要和
  `DecisionAggregate` 直方圖。

`crates/iroha_data_model/src/fraud/types.rs` 中的往返測試可確保這些
架構與 Norito 編解碼器保持二進制一致，並且
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` 練習
端到端的完整接收/決策管道。

## PSP SDK 參考

以下語言存根跟踪面向 PSP 的集成示例：

- **生鏽** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  使用工作區 `iroha` 客戶端製作 `RiskQuery` 元數據並驗證
  錄取失敗/成功。
- **TypeScript** – `docs/source/governance_api.md` 記錄 REST 表面
  由 PSP 演示儀表板中使用的輕量級 Torii 網關消耗；的
  腳本化客戶端住在 `scripts/ci/schedule_fraud_scoring.sh` 中，因為煙霧
  演習。
- **Swift 和 Kotlin** – 現有 SDK（`IrohaSwift` 和
  `crates/iroha_cli/docs/multisig.md` 引用）公開 Torii 元數據
  附加 `fraud_assessment_*` 字段所需的掛鉤。 PSP 特定的幫助程序是
  在“欺詐和遙測治理循環”里程碑下進行跟踪
  `status.md` 並重用這些 SDK 的交易構建器。

這些引用將與微服務網關保持同步，以便 PSP
實施者始終擁有最新的架構和每個示例代碼路徑
支持的語言。