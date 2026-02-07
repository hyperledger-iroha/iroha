---
lang: zh-hant
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T14:35:37.551676+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 治理手冊

這本手冊記錄了維持 Sora Network 的日常儀式
治理委員會一致。它匯集了來自權威的參考文獻
存儲庫，以便各個儀式可以保持簡潔，而操作員始終
為更廣泛的流程提供單一入口點。

## 理事會儀式

- **裝置治理** – 請參閱[Sora 議會裝置批准](sorafs/signing_ceremony.md)
  對於議會基礎設施小組現在的鏈上審批流程
  查看 SoraFS 分塊器更新時如下。
- **計票結果發布** – 請參閱
  [治理投票統計](governance_vote_tally.md) 用於分步 CLI
  工作流程和報告模板。

## 操作手冊

- **API 集成** – [治理 API 參考](governance_api.md) 列出了
  理事會服務公開的 REST/gRPC 表面，包括身份驗證
  要求和分頁規則。
- **遙測儀表板** – Grafana JSON 定義
  `docs/source/grafana_*` 定義了“治理約束”和“調度程序”
  TEU”板。每次發布後將 JSON 導出到 Grafana 以保持一致
  與規範佈局。

## 數據可用性監督

### 保留類

批准 DA 清單的議會小組必須參考強制保留
投票前的政策。下表反映了通過強制執行的默認值
`torii.da_ingest.replication_policy` 因此審閱者無需
尋找源TOML。【docs/source/da/replication_policy.md:1】

|治理標籤|斑點類|熱保持|保冷|所需副本 |存儲類|
|----------------|------------|---------------|----------------|--------------------|----------------------------|
| `da.taikai.live` | `taikai_segment` | 24小時 | 14 天 | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6小時| 7 天 | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 小時 | 180天| 3 | `cold` |
| `da.default` | _所有其他類別_ | 6小時| 30 天 | 3 | `warm` |

基礎設施小組應附上來自
`docs/examples/da_manifest_review_template.md` 每張選票因此清單
摘要、保留標籤和 Norito 文物在治理中保持關聯
記錄。

### 簽名清單審計跟踪

在選票進入議程之前，議會工作人員必須證明清單
正在審查的字節與議會信封和 SoraFS 文物相符。使用
收集證據的現有工具：1. 從 Torii (`iroha app da get-blob --storage-ticket <hex>`
   或等效的 SDK 幫助程序），因此每個人都對到達的相同字節進行哈希處理
   網關。
2. 使用簽名的信封運行清單存根驗證程序：
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   這會重新計算 BLAKE3 清單摘要，驗證
   `chunk_digest_sha3_256`，並檢查嵌入的每個 Ed25519 簽名
   `manifest_signatures.json`。參見 `docs/source/sorafs/manifest_pipeline.md`
   有關其他 CLI 選項。
3. 將摘要、`chunk_digest_sha3_256`、配置文件句柄和簽名者列表複製到
   審核模板。注意：如果驗證者報告“配置文件不匹配”或
   缺少簽名，停止投票並要求提供更正的信封。
4. 存儲驗證器輸出（或來自的 CI 工件）
   `ci/check_sorafs_fixtures.sh`) 與 Norito `.to` 有效負載一起，以便審核員
   無需訪問內部網關即可重播證據。

由此產生的審計包應該讓議會重新創建每個哈希值和簽名
即使在清單從熱存儲中轉出後也要進行檢查。

### 審查清單

1. 取出議會批准的艙單信封（參見
   `docs/source/sorafs/signing_ceremony.md`）並記錄 BLAKE3 摘要。
2. 驗證清單的 `RetentionPolicy` 塊與表中的標記匹配
   上面； Torii 將拒絕不匹配，但理事會必須捕獲
   審計員的證據。 【docs/source/da/replication_policy.md:32】
3. 確認提交的 Norito 有效負載引用相同的保留標籤
   以及出現在入場券中的 blob 類。
4. 附上策略檢查證明（CLI 輸出，`torii.da_ingest.replication_policy`
   dump 或 CI artefact）到審核數據包，以便 SRE 可以重播決策。
5. 當提案取決於時，記錄計劃的補貼水龍頭或租金調整
   `docs/source/sorafs_reserve_rent_plan.md`。

### 升級矩陣

|請求類型 |擁有面板|附上證據 |截止日期和遙測|參考文獻 |
|--------------|--------------|--------------------|------------------------|------------|
|補貼/租金調整|基礎設施+財政|填充 DA 數據包、`reserve_rentd` 的租金增量、更新的儲備金預測 CSV、理事會投票記錄 |在提交財務更新之前註意租金影響；包括滾動 30 天緩衝遙測，以便財務部門可以在下一個結算窗口內進行調節 | `docs/source/sorafs_reserve_rent_plan.md`、`docs/examples/da_manifest_review_template.md` |
|審核刪除/合規行動 |適度+合規|合規票證 (`ComplianceUpdateV1`)、證明令牌、簽名清單摘要、上訴狀態 |遵循網關合規性 SLA（24 小時內確認，完全刪除≤72 小時）。附上顯示該操作的 `TransparencyReportV1` 摘錄。 | `docs/source/sorafs_gateway_compliance_plan.md`、`docs/source/sorafs_moderation_panel_plan.md` |
|緊急凍結/回滾|議會調解小組|事先批准包、新凍結令、回滾清單摘要、事件日誌 |立即發布凍結通知，並在下一個治理時段內安排回滾公投；包括緩衝區飽和+ DA 複製遙測來證明緊急情況的合理性。 | `docs/source/sorafs/signing_ceremony.md`、`docs/source/sorafs_moderation_panel_plan.md` |在對入場券進行分類時使用該表，以便每個小組都收到準確的
執行任務所需的文物。

### 報告可交付成果

每個 DA-10 決策都必須附帶以下工件（將它們附加到
投票中引用的治理 DAG 條目）：

- 完整的 Markdown 數據包來自
  `docs/examples/da_manifest_review_template.md`（現在包括簽名和
  升級部分）。
- 已簽名的 Norito 清單 (`.to`) 以及 `manifest_signatures.json` 信封
  或證明提取摘要的 CI 驗證程序日誌。
- 由該操作觸發的任何透明度更新：
  - `TransparencyReportV1` 刪除或合規驅動凍結的增量。
  - 租金/儲備賬本增量或 `ReserveSummaryV1` 補貼快照。
- 審查期間收集的遙測快照的鏈接（複製深度、
  緩衝餘量、審核積壓），以便觀察者可以交叉檢查條件
  事後。

## 審核和升級

合規後將關閉網關、收回補貼或凍結 DA
`docs/source/sorafs_gateway_compliance_plan.md` 中描述的管道和
`docs/source/sorafs_moderation_panel_plan.md` 中的上訴工具。面板應：

1. 記錄原始合規票證（`ComplianceUpdateV1` 或
   `ModerationAppealV1`）並附上相關的證明令牌。 【docs/source/sorafs_gateway_compliance_plan.md:20】
2. 確認請求是否調用審核上訴路徑（公民小組
   投票）或議會緊急凍結；兩個流程都必須引用清單
   新模板中捕獲的摘要和保留標籤。 【docs/source/sorafs_moderation_panel_plan.md:1】
3. 列舉升級期限（上訴提交/披露窗口、緊急情況
   凍結期限）並說明哪個理事會或小組擁有後續行動。
4. 捕獲用於的遙測快照（緩衝區餘量、審核積壓）
   證明該行動的合理性，以便下游審核可以將決策與實際情況相匹配
   狀態。

合規和審核小組必須同步其每週透明度報告
與路由器運營商結算，因此下架和補貼影響相同
清單集。

## 報告模板

所有 DA-10 評論現在都需要簽名的 Markdown 數據包。複製
`docs/examples/da_manifest_review_template.md`，填充清單元數據，
保留驗證表和小組投票摘要，然後固定已完成的內容
文檔（加上引用的 Norito/JSON 工件）到治理 DAG 條目。
專家組應在治理會議紀要中鏈接該數據包，以便將來刪除或
補貼續訂可以引用原始清單摘要，而無需重新運行
整個儀式。

## 事件和撤銷工作流程

緊急行動現在發生在鏈上。當需要發布夾具時
回滾，提交治理票並提出議會恢復提案
指向先前批准的清單摘要。基礎設施小組
處理投票，一旦最終確定，Nexus 運行時就會發布回滾
下游客戶端消費的事件。不需要本地 JSON 工件。

## 保持劇本最新- 每當新的面向治理的操作手冊登陸時更新此文件
  存儲庫。
- 在這裡交叉鏈接新的儀式，以便理事會索引仍然可以被發現。
- 如果引用的文檔發生移動（例如，新的 SDK 路徑），請更新鏈接
  作為同一拉取請求的一部分，以避免過時的指針。