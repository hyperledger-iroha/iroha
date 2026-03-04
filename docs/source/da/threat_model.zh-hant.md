---
lang: zh-hant
direction: ltr
source: docs/source/da/threat_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bff91e735291e82d0d50b5dad4dfbf2b57af68f2f7067760add5da81fc7f554
source_last_modified: "2026-01-18T15:31:35.203840+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus 數據可用性威脅模型

_上次審核：2026-01-19 — 下次預定審核：2026-04-19_

維護節奏：數據可用性工作組（<=90 天）。每次修訂必須
出現在 `status.md` 中，其中包含主動緩解票據和模擬工件的鏈接。

## 目的和範圍

數據可用性 (DA) 程序保留 Taikai 廣播、Nexus 通道 blob，以及
在拜占庭、網絡和運營商故障下可檢索的治理工件。
該威脅模型鞏固了 DA-1 的工程工作（架構和威脅模型）
並作為下游 DA 任務（DA-2 至 DA-10）的基線。

範圍內的組件：
- Torii DA 攝取擴展和 Norito 元數據編寫器。
- SoraFS 支持的 blob 存儲樹（熱/冷層）和復制策略。
- Nexus 區塊承諾（有線格式、證明、輕客戶端 API）。
- 特定於 DA 有效負載的 PDP/PoTR 執行掛鉤。
- 操作員工作流程（固定、驅逐、削減）和可觀察性管道。
- 接納或驅逐 DA 運營商和內容的治理批准。

超出本文檔範圍：
- 完整的經濟建模（在 DA-7 工作流中捕獲）。
- SoraFS 威脅模型已涵蓋 SoraFS 基本協議。
- 客戶端 SDK 人體工程學超出了威脅面考慮因素。

## 架構概述

1. **提交：** 客戶端通過 Torii DA 攝取 API 提交 blob。節點
   塊 blob，編碼 Norito 清單（blob 類型、通道、紀元、編解碼器標誌），
   並將塊存儲在熱 SoraFS 層中。
2. **廣告：** Pin 意圖和復制提示傳播到存儲
   通過註冊表（SoraFS 市場）提供的策略標籤
   規定熱/冷保留目標。
3. **承諾：** Nexus 定序器包括 blob 承諾（CID + 可選的 KZG
   根）在規範塊中。輕客戶端依賴於承諾哈希並且
   廣告元數據以驗證可用性。
4. **複製：** 存儲節點拉取分配的共享/塊，滿足 PDP/PoTR
   挑戰，並根據策略在熱層和冷層之間提升數據。
5. **Fetch：**消費者通過SoraFS或DA感知網關獲取數據，驗證
   證明並在副本消失時提出修復請求。
6. **治理：** 議會和 DA 監督委員會批准運營商，
   租金時間表和執法升級。存儲治理工件
   通過相同的 DA 路徑來確保流程透明度。租金參數
   DA-7 下的跟踪記錄在 `docs/source/da/rent_policy.md` 中，因此審核
   執行審查可以參考每個 blob 所應用的確切 XOR 金額。

## 資產和所有者

影響等級：**嚴重**破壞賬本安全性/活躍性； **高**阻止 DA
回填或客戶； **中度** 會降低質量，但仍可恢復；
**低**效果有限。|資產|描述 |誠信|可用性 |保密 |業主|
| ---| ---| ---| ---| ---| ---|
| DA blob（塊 + 清單）| Taikai、車道、治理 blob 存儲在 SoraFS |關鍵|關鍵|中等| DA 工作組/存儲團隊 |
| Norito DA 清單 |描述 Blob 的類型化元數據 |關鍵|高|中等|核心協議工作組 |
|區塊承諾 | Nexus 塊內的 CID + KZG 根 |關鍵|高|低|核心協議工作組 |
| PDP/PoTR 時間表 | DA 副本的執行節奏 |高|高|低|存儲團隊|
|運營商註冊表|批准的存儲提供商和政策 |高|高|低|治理委員會|
|租金和獎勵記錄| DA 租金和罰款的分類賬目 |高|中等|低|財政部工作組 |
|可觀察性儀表板 | DA SLO、複製深度、警報 |中等|高|低| SRE / 可觀察性 |
|維修意向 |請求補充缺失塊的水|中等|中等|低|存儲團隊|

## 對手和能力

|演員 |能力|動機|筆記|
| ---| ---| ---| ---|
|惡意客戶端 |提交格式錯誤的 blob、重放過時的清單、嘗試在攝取時執行 DoS。 |擾亂 Taikai 廣播，注入無效數據。 |沒有特權密鑰。 |
|拜占庭存儲節點|丟棄分配的副本、偽造 PDP/PoTR 證明、與他人勾結。 |減少 DA 保留、避免租金、扣留數據。 |擁有有效的操作員憑證。 |
|受損的測序儀 |省略承諾、對塊模棱兩可、重新排序 blob 元數據。 |隱藏 DA 提交內容，造成不一致。 |受多數共識限制。 |
|內幕操作員|濫用治理訪問、篡改保留策略、洩露憑證。 |經濟利益，破壞行為。 |訪問熱/冷層基礎設施。 |
|網絡對手|分區節點、延遲複製、注入 MITM 流量。 |降低可用性，降低 SLO。 |無法破壞 TLS，但可以丟棄/減慢鏈接。 |
|可觀察性攻擊者 |篡改儀表板/警報，抑制事件。 |隱藏 DA 中斷。 |需要訪問遙測管道。 |

## 信任邊界

- **入口邊界：** Torii DA 擴展的客戶端。需要請求級身份驗證，
  速率限制和有效負載驗證。
- **複製邊界：** 存儲節點交換塊和證明。節點是
  相互驗證，但可能表現為拜占庭式。
- **賬本邊界：** 提交的塊數據與鏈外存儲。共識衛士
  完整性，但可用性需要鏈下執行。
- **治理邊界：** 理事會/議會決定批准運營商，
  預算和削減。這裡的中斷直接影響 DA 部署。
- **可觀察性邊界：** 導出到儀表板/警報的指標/日誌集合
  工具。篡改隱藏了中斷或攻擊。

## 威脅場景和控制

### 攝取路徑攻擊**場景：** 惡意客戶端提交格式錯誤的 Norito 有效負載或過大的負載
blob 會耗盡資源或走私無效元數據。

**控制**
- Norito 模式驗證，具有嚴格的版本協商；拒絕未知的標誌。
- Torii 攝取端點處的速率限制和身份驗證。
- 由 SoraFS 分塊器強制執行的塊大小界限和確定性編碼。
- 准入管道僅在完整性校驗和匹配後持續顯示。
- 確定性重播緩存 (`ReplayCache`) 跟踪 `(lane, epoch, sequence)` 窗口，在磁盤上保留高水位線，並拒絕重複/過時的重播；財產和模糊利用涵蓋不同的指紋和無序提交。 【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da/ingest.rs:1】

**剩餘間隙**
- Torii 攝取必須將重播緩存線程化到准入中，並在重新啟動時保留序列游標。
- Norito DA 模式現在有一個專用的模糊工具 (`fuzz/da_ingest_schema.rs`) 來強調編碼/解碼不變量；如果目標下降，覆蓋儀表板應該發出警報。

### 複製扣留

**場景：** 拜占庭存儲運算符接受 pin 分配但丟棄塊，
通過偽造響應或串通傳遞 PDP/PoTR 挑戰。

**控制**
- PDP/PoTR 挑戰計劃擴展到具有每個紀元覆蓋範圍的 DA 有效負載。
- 具有仲裁閾值的多源複製；獲取協調器檢測到
  丟失碎片並觸發修復。
- 與失敗的證明和丟失的副本相關的治理削減。

**剩餘間隙**
- `integration_tests/src/da/pdp_potr.rs` 中的模擬線束（由
  `integration_tests/tests/da/pdp_potr_simulation.rs`) 現在進行勾結
  和分區場景，驗證 PDP/PoTR 計劃檢測到
  拜占庭行為是確定性的。繼續將其與 DA-5 一起延伸至
  覆蓋新的校樣表面。
- 冷層驅逐政策需要簽署審計跟踪以防止秘密刪除。

### 承諾篡改

**場景：** 受損的定序器發布省略或更改 DA 的塊
承諾，導致獲取失敗或輕客戶端不一致。

**控制**
- 共識交叉檢查區塊提案與 DA 提交隊列；同行拒絕
  提案缺少必要的承諾。
- 輕客戶端在顯示獲取句柄之前驗證承諾包含證明。
- 審計跟踪將提交收據與集體承諾進行比較。
- 自動調節作業 (`cargo xtask da-commitment-reconcile`) 比較
  攝取帶有 DA 承諾的收據（SignedBlockWire、`.norito` 或 JSON），
  發出用於治理的 JSON 證據包，並且在丟失或失敗時失敗
  不匹配的票證，以便 Alertmanager 可以尋呼遺漏/篡改的信息。

**剩餘間隙**
- 由對賬作業+Alertmanager鉤子覆蓋；現在治理包
  默認情況下攝取 JSON 證據包。

### 網絡分區和審查**場景：** 對手分割複製網絡，阻止節點
獲取分配的塊或響應 PDP/PoTR 挑戰。

**控制**
- 多區域提供商要求確保多樣化的網絡路徑。
- 挑戰窗口包括抖動和回退到帶外修復通道。
- 可觀察性儀表板監控複製深度、挑戰成功以及
  獲取帶有警報閾值的延遲。

**剩餘間隙**
- Taikai 現場活動的分區模擬仍然缺失；需要浸泡測試。
- 修復尚未編碼的帶寬預留策略。

### 內部濫用

**場景：** 具有註冊表訪問權限的操作員操縱保留策略，
將惡意提供商列入白名單，或抑制警報。

**控制**
- 治理行動需要多方簽名和 Norito 公證記錄。
- 策略更改將事件發送到監控和歸檔日誌。
- 可觀測性管道強制使用散列鏈接僅附加 Norito 日誌。
- 季度訪問審查自動化 (`cargo xtask da-privilege-audit`) 步行
  DA 清單/重播目錄（加上操作員提供的路徑）、標誌
  缺少/非目錄/世界可寫條目，並發出簽名的 JSON 包
  用於治理儀表板。

**剩餘間隙**
- 儀表板防篡改需要簽名快照。

## 殘餘風險登記冊

|風險|可能性|影響 |業主|緩解計劃|
| ---| ---| ---| ---| ---|
| DA-2 序列緩存登陸之前重播 DA 清單 |可能 |中等|核心協議工作組 |在DA-2中實現序列緩存+nonce驗證；添加回歸測試。 |
|當 >f 個節點妥協時，PDP/PoTR 共謀 |不太可能 |高|存儲團隊|通過跨供應商抽樣得出新的挑戰時間表；通過模擬線束進行驗證。 |
|冷層驅逐審計差距|可能 |高| SRE/存儲團隊 |附上已簽名的審計日誌和驅逐的鏈上收據；通過儀表板進行監控。 |
|定序器遺漏檢測延遲 |可能 |高|核心協議工作組 |每晚 `cargo xtask da-commitment-reconcile` 會比較收據與承諾 (SignedBlockWire/`.norito`/JSON) 以及針對丟失或不匹配票證的頁面治理。 |
| Taikai 直播的分區彈性 |可能 |關鍵|網絡 TL |執行分區演練；預留修復帶寬；文檔故障轉移 SOP。 |
|治理特權漂移|不太可能 |高|治理委員會|每季度 `cargo xtask da-privilege-audit` 運行（清單/重播目錄 + 額外路徑），帶有簽名 JSON + 儀表板門；將審計工件錨定在鏈上。 |

## 所需的後續行動1. 發布 DA 攝取 Norito 模式和示例向量（納入 DA-2）。
2. 通過 Torii DA 攝取重播緩存並在節點重新啟動時保留序列游標。
3. **已完成 (2026-02-05)：** PDP/PoTR 模擬工具現在通過 QoS 積壓建模來練習共謀 + 分區場景；請參閱 [`integration_tests/src/da/pdp_potr.rs`](/integration_tests/src/da/pdp_potr.rs)（在 `integration_tests/tests/da/pdp_potr_simulation.rs` 下進行測試），了解下面捕獲的實現和確定性摘要。
4. **已完成 (2026-05-29)：** `cargo xtask da-commitment-reconcile` 將攝取收據與 DA 承諾進行比較 (SignedBlockWire/`.norito`/JSON)，發出 `artifacts/da/commitment_reconciliation.json`，並連接到 Alertmanager/治理數據包中以發出遺漏/篡改警報（`xtask/src/da.rs`）。
5. **已完成 (2026-05-29)：** `cargo xtask da-privilege-audit` 遍歷清單/重播假脫機（加上操作員提供的路徑），標記缺失/非目錄/全局可寫條目，並生成用於儀表板/治理審查的簽名 JSON 包 (`artifacts/da/privilege_audit.json`)，從而縮小訪問審查自動化差距。

**下一步看哪裡：**

- DA 重播緩存和光標持久性登陸 DA-2。請參閱
  `crates/iroha_core/src/da/replay_cache.rs`（緩存邏輯）中的實現和
  Torii 集成在 `crates/iroha_torii/src/da/ingest.rs` 中，該線程
  通過 `/v1/da/ingest` 進行指紋檢查。
- PDP/PoTR 流模擬通過驗證流工具進行
  `crates/sorafs_car/tests/sorafs_cli.rs`，涵蓋 PoR/PDP/PoTR 請求流程
  以及威脅模型中動畫的故障場景。
- 容量和修復浸泡結果如下
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`，而更廣泛
  Sumeragi 浸泡矩陣在 `docs/source/sumeragi_soak_matrix.md` 中跟踪
  （包括本地化變體）。這些文物記錄了長時間運行的演練
  殘留風險登記冊中引用。
- 協調+特權審計自動化存在
  `docs/automation/da/README.md` 和新的 `cargo xtask da-commitment-reconcile`
  /`cargo xtask da-privilege-audit` 命令；使用默認輸出
  `artifacts/da/` 將證據附加到治理數據包時。

## 模擬證據和 QoS 建模 (2026-02)

為了結束 DA-1 後續#3，我們編寫了確定性 PDP/PoTR 模擬
`integration_tests/src/da/pdp_potr.rs` 下的線束（由
`integration_tests/tests/da/pdp_potr_simulation.rs`）。安全帶
跨三個區域分配節點，根據以下注入分區/共謀
路線圖概率、跟踪 PoTR 延遲並提供維修積壓
反映熱門維修預算的模型。運行默認場景
（12 個 epoch，18 個 PDP 挑戰 + 每個 epoch 2 個 PoTR 窗口）產生了
以下指標：<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
|公制|價值|筆記|
| ---| ---| ---|
|檢測到 PDP 故障 | 48 / 49 (98.0%) |分區仍然觸發檢測；單個未檢測到的故障來自誠實的抖動。 |
| PDP 平均檢測延遲 | 0.0 紀元 |失敗是在原始紀元內浮現出來的。 |
|檢測到 PoTR 故障 | 28 / 77 (36.4%) |一旦節點錯過 ≥2 個 PoTR 窗口，就會觸發檢測，將大多數事件留在殘留風險寄存器中。 |
| PoTR 平均檢測延遲 | 2.0時代|與歸檔升級中的兩個紀元遲到閾值相匹配。 |
|搶修排隊高峰| 38 份清單 |當分區堆疊速度快於每個時期可用的四次修復時，積壓就會激增。 |
|響應延遲 p95 | 30,068 毫秒 |鏡像 30 秒質詢窗口，並應用 ±75 毫秒抖動進行 QoS 採樣。 |
<!-- END_DA_SIM_TABLE -->

這些輸出現在驅動 DA 儀表板原型並滿足“模擬
路線圖中引用的“利用+ QoS 建模”驗收標準。

自動化現在位於 `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]` 後面，它調用共享線束和
默認情況下，將 Norito JSON 發送到 `artifacts/da/threat_model_report.json`。每晚
作業使用此文件來刷新本文檔中的矩陣並發出警報
檢測率、修復隊列或 QoS 樣本的漂移。

要刷新上表的文檔，請運行 `make docs-da-threat-model`，其中
調用 `cargo xtask da-threat-model-report`，重新生成
`docs/source/da/_generated/threat_model_report.json`，並重寫本節
通過 `scripts/docs/render_da_threat_model_tables.py`。 `docs/portal` 鏡子
(`docs/portal/docs/da/threat-model.md`) 在同一遍中更新，因此兩者
副本保持同步。