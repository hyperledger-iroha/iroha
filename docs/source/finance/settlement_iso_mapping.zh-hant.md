---
lang: zh-hant
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T16:26:46.568382+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 結算 ↔ ISO 20022 字段映射

本註釋捕獲了 Iroha 結算指令之間的規範映射
（`DvpIsi`、`PvpIsi`、回購抵押品流量）和執行的 ISO 20022 消息
在橋邊。它反映了在中實現的消息腳手架
`crates/ivm/src/iso20022.rs` 並作為生產或時的參考
驗證 Norito 有效負載。

### 參考數據政策（標識符和驗證）

該策略打包了標識符首選項、驗證規則和參考數據
Norito ↔ ISO 20022 網橋在發出消息之前必須執行的義務。

**ISO 消息內的錨點：**
- **儀器標識符** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  （或同等儀器領域）。
- **當事方/代理人** → `DlvrgSttlmPties/Pty` 和 `RcvgSttlmPties/Pty` 為 `sese.*`，
  或 `pacs.009` 中的代理結構。
- **賬戶** → `…/Acct` 保管/現金賬戶要素；鏡像賬本
  `AccountId` 中的 `SupplementaryData`。
- **專有標識符** → `…/OthrId` 和 `Tp/Prtry` 並鏡像
  `SupplementaryData`。切勿用專有標識符替換受監管的標識符。

#### 消息系列的標識符首選項

##### `sese.023` / `.024` / `.025`（證券結算）

- **儀器 (`FinInstrmId`)**
  - 首選：`…/ISIN` 下的 **ISIN**。它是 CSD / T2S 的規範標識符。 [^anna]
  - 後備方案：
    - **CUSIP** 或 `…/OthrId/Id` 下的其他 NSIN，其中 `Tp/Cd` 從 ISO 外部設置
      代碼列表（例如，`CUSP`）；必要時將發行人包含在 `Issr` 中。 [^iso_mdr]
    - **Norito 資產 ID** 作為專有：`…/OthrId/Id`、`Tp/Prtry="NORITO_ASSET_ID"` 和
      在 `SupplementaryData` 中記錄相同的值。
  - 可選描述符：**CFI** (`ClssfctnTp`) 和 **FISN**（支持緩解）
    和解。 [^iso_cfi][^iso_fisn]
- **各方 (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - 首選：**BIC**（`AnyBIC/BICFI`，ISO 9362）。 [^swift_bic]
  - 後備：**LEI**，其中消息版本公開專用 LEI 字段；如果
    不存在，攜帶帶有清晰 `Prtry` 標籤的專有 ID，並在元數據中包含 BIC。 [^iso_cr]
- **定居點/場地** → **MIC** 代表場地，**BIC** 代表 CSD。 [^iso_mic]

##### `colr.010` / `.011` / `.012` 和 `colr.007`（抵押品管理）

- 遵循與 `sese.*` 相同的儀器規則（首選 ISIN）。
- 各方默認使用**BIC**； **LEI** 在架構公開的情況下是可接受的。 [^swift_bic]
- 現金金額必須使用 **ISO 4217** 貨幣代碼和正確的小單位。 [^iso_4217]

##### `pacs.009` / `camt.054`（PvP 資金和報表）- **代理人（`InstgAgt`、`InstdAgt`、債務人/債權人代理人）** → **BIC** 可選
  LEI 在允許的情況下。 [^swift_bic]
- **賬戶**
  - 銀行間：通過 **BIC** 和內部賬戶參考進行識別。
  - 面向客戶的聲明 (`camt.054`)：包括 **IBAN**（如果存在）並進行驗證
    （長度、國家/地區規則、mod-97 校驗和）。 [^swift_iban]
- **貨幣** → **ISO 4217** 3 個字母代碼，尊重小單位舍入。 [^iso_4217]
- **Torii 攝取** → 通過 `POST /v1/iso20022/pacs009` 提交 PvP 資金；橋
  需要 `Purp=SECU`，現在在配置參考數據時強制執行 BIC 人行橫道。

#### 驗證規則（在發射前應用）

|標識符 |驗證規則 |筆記|
|------------|-----------------|--------|
| **ISIN** |正則表達式 `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` 和 Luhn (mod-10) 校驗位符合 ISO 6166 附錄 C |橋接發射前拒絕；更喜歡上游富集。 [^anna_luhn] |
| **CUSIP** |正則表達式 `^[A-Z0-9]{9}$` 和模數 10，權重為 2（字符映射到數字）|僅當 ISIN 不可用時；獲取後通過 ANNA/CUSIP 人行橫道繪製地圖。 [^cusip] |
| **雷** |正則表達式 `^[A-Z0-9]{18}[0-9]{2}$` 和 mod-97 校驗位 (ISO 17442) |在接受之前對照 GLEIF 每日增量文件進行驗證。 [^gleif] |
| **BIC** |正則表達式 `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` |可選的分支代碼（最後三個字符）。確認 RA 文件中的活動狀態。 [^swift_bic] |
| **麥克風** |根據 ISO 10383 RA 文件進行維護；確保場館處於活動狀態（無 `!` 終止標誌）|在發射前標記退役的 MIC。 [^iso_mic] |
| **國際銀行賬號** |國家/地區特定長度，大寫字母數字，mod-97 = 1 |使用 SWIFT 維護的註冊表；拒絕結構上無效的 IBAN。 [^swift_iban] |
| **專有帳戶/方 ID** | `Max35Text`（UTF-8，≤35 個字符），帶有修剪的空格 |適用於 `GenericAccountIdentification1.Id` 和 `PartyIdentification135.Othr/Id` 字段。拒絕超過 35 個字符的條目，以便橋接有效負載符合 ISO 架構。 |
| **代理帳戶標識符** | `…/Prxy/Id` 下的非空 `Max2048Text`，`…/Prxy/Tp/{Cd,Prtry}` 中具有可選類型代碼 |與主要 IBAN 一起存儲；驗證仍然需要 IBAN，同時接受代理句柄（帶有可選類型代碼）以鏡像 PvP 軌道。 |
| **CFI** |六字符代碼，使用 ISO 10962 分類法的大寫字母 |可選的豐富；確保字符與儀器類別匹配。 [^iso_cfi] |
| **FISN** |最多 35 個字符，大寫字母數字加有限標點符號 |選修的;根據 ISO 18774 指南進行截斷/標準化。 [^iso_fisn] |
| **貨幣** | ISO 4217 3 字母代碼，由小單位確定的比例 |金額必須四捨五入到允許的小數位；在 Norito 端強制執行。 [^iso_4217] |

#### 人行橫道和數據維護義務- 維護 **ISIN ↔ Norito 資產 ID** 和 **CUSIP ↔ ISIN** 人行橫道。每晚更新自
  ANNA/DSB 提供 CI 使用的快照並進行版本控制。 [^anna_crosswalk]
- 刷新 GLEIF 公共關係文件中的 **BIC ↔ LEI** 映射，以便橋接器可以
  需要時同時發出。 [^bic_lei]
- 將 **MIC 定義** 與橋元數據一起存儲，以便場地驗證
  即使 RA 文件在中午發生變化，也具有確定性。 [^iso_mic]
- 在橋元數據中記錄數據來源（時間戳+來源）以供審核。堅持
  快照標識符與發出的指令一起。
- 配置 `iso_bridge.reference_data.cache_dir` 以保留每個加載的數據集的副本
  以及出處元數據（版本、來源、時間戳、校驗和）。這使得審核員
  即使在上游快照輪換之後，操作員也可以區分歷史源。
- ISO 人行橫道快照由 `iroha_core::iso_bridge::reference_data` 使用
  `iso_bridge.reference_data` 配置塊（路徑 + 刷新間隔）。儀表
  `iso_reference_status`、`iso_reference_age_seconds`、`iso_reference_records` 和
  `iso_reference_refresh_interval_secs` 公開運行時運行狀況以進行警報。 Torii
  網橋拒絕其代理 BIC 不存在於配置中的 `pacs.008` 提交
  人行橫道，當對手方處於
  未知。 【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN 和 ISO 4217 綁定在同一層強制執行：現在 pacs.008/pacs.009 流
  當債務人/債權人 IBAN 缺少配置的別名或當
  `currency_assets` 中缺少結算貨幣，防止橋接畸形
  到達分類賬的指令。 IBAN 驗證也適用於特定國家/地區
  ISO 7064 mod-97 通過之前的長度和數字校驗位，因此在結構上無效
  值被提前拒絕。 【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- CLI結算助手繼承相同的護欄：通過
  `--iso-reference-crosswalk <path>` 與 `--delivery-instrument-id` 一起獲得 DvP
  在發出 `sese.023` XML 快照之前預覽驗證儀器 ID。 【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint`（和 CI 包裝器 `ci/check_iso_reference_data.sh`）lint
  人行橫道快照和固定裝置。該命令接受 `--isin`、`--bic-lei`、`--mic` 和
  運行時，`--fixtures` 標記並回退到 `fixtures/iso_bridge/` 中的示例數據集
  不帶參數。 【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM 幫助程序現在攝取真正的 ISO 20022 XML 信封（head.001 + `DataPDU` + `Document`）
  並通過 `head.001` 架構驗證業務應用程序標頭，因此 `BizMsgIdr`，
  `MsgDefIdr`、`CreDt` 和 BIC/ClrSysMmbId 代理被確定性地保留； XMLDSig/XAdES
  塊仍然被故意跳過。回歸測試消耗樣本和新的用於保護映射的標頭信封固定裝置。 【crates/ivm/src/iso20022.rs:265】【crates/ivm/src/iso20022.rs:3301】【crates/ivm/src/iso20022.rs:3703】

#### 監管和市場結構考慮因素

- **T+1結算**：美國/加拿大股票市場於2024年轉向T+1；調整 Norito
  相應地安排和 SLA 警報。 [^sec_t1][^csa_t1]
- **CSDR 處罰**：和解紀律規則強制執行現金處罰；確保 Norito
  元數據捕獲用於調節的懲罰參考。 [^csdr]
- **當日結算試點**：印度監管機構正在逐步推行T0/T+0結算；保留
  隨著試點範圍的擴大，橋樑日曆也隨之更新。 [^india_t0]
- **抵押買入/持有**：監控 ESMA 關於買入時間表和可選持有的更新
  因此有條件交付 (`HldInd`) 與最新指南保持一致。 [^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### 貨到付款 → `sese.023`| DvP 領域 | ISO 20022 路徑 |筆記|
|--------------------------------------------------------------------|----------------------------------------|------|
| `settlement_id` | `TxId` |穩定的生命週期標識符 |
| `delivery_leg.asset_definition_id`（安全）| `SctiesLeg/FinInstrmId` |規範標識符（ISIN、CUSIP……）|
| `delivery_leg.quantity` | `SctiesLeg/Qty` |十進製字符串；榮譽資產精準 |
| `payment_leg.asset_definition_id`（貨幣）| `CashLeg/Ccy` | ISO 貨幣代碼 |
| `payment_leg.quantity` | `CashLeg/Amt` |十進製字符串；根據數字規范進行四捨五入 |
| `delivery_leg.from`（賣家/送貨方）| `DlvrgSttlmPties/Pty/Bic` |交付參與者的 BIC *（帳戶規範 ID 目前在元數據中導出）* |
| `delivery_leg.from` 帳戶標識符 | `DlvrgSttlmPties/Acct` |自由形式； Norito 元數據攜帶準確的帳戶 ID |
| `delivery_leg.to`（買方/接收方）| `RcvgSttlmPties/Pty/Bic` |接收參與者的 BIC |
| `delivery_leg.to` 帳戶標識符 | `RcvgSttlmPties/Acct` |自由形式；匹配接收帳戶 ID |
| `plan.order` | `Plan/ExecutionOrder` |枚舉：`DELIVERY_THEN_PAYMENT` 或 `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` |枚舉：`ALL_OR_NOTHING`、`COMMIT_FIRST_LEG`、`COMMIT_SECOND_LEG` |
| **留言目的** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI`（發送）或 `RECE`（接收）；反映提交方執行的分支。 |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT`（付款）或 `FREE`（免付款）。 |
| `delivery_leg.metadata`，`payment_leg.metadata` | `SctiesLeg/Metadata`、`CashLeg/Metadata` |可選 Norito JSON 編碼為 UTF-8 |

> **結算限定符** – 橋接器通過將結算條件代碼 (`SttlmTxCond`)、部分結算指標 (`PrtlSttlmInd`) 以及 Norito 元數據中的其他可選限定符複製到 `sese.023/025`（如果存在）來反映市場慣例。強制執行 ISO 外部代碼列表中發布的枚舉，以便目標 CSD 識別這些值。

### 支付與支付資金 → `pacs.009`

為 PvP 指令提供資金的現金換現金部分以 FI-to-FI 信用形式發放
轉移。該橋對這些付款進行了註釋，以便下游系統識別
他們為證券結算提供資金。| PvP 資金領域 | ISO 20022 路徑 |筆記|
|------------------------------------------------|--------------------------------------------------------|--------------------|
| `primary_leg.quantity` / {金額，貨幣} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` |從發起人處扣除的金額/貨幣。 |
|交易對手代理標識符 | `InstgAgt`，`InstdAgt` |發送和接收代理的 BIC/LEI。 |
|落戶目的 | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` |設置為 `SECU` 用於證券相關的 PvP 資金。 |
| Norito 元數據（賬戶 ID、FX 數據）| `CdtTrfTxInf/SplmtryData` |包含完整的 AccountId、FX 時間戳、執行計劃提示。 |
|指令標識符/生命週期鏈接| `CdtTrfTxInf/PmtId/InstrId`、`CdtTrfTxInf/RmtInf` |與 Norito `settlement_id` 相匹配，以便現金部分與證券方保持一致。 |

JavaScript SDK 的 ISO 橋通過默認的
`pacs.009` 類別目的為 `SECU`；調用者可以用另一個覆蓋它
發出非證券信用轉賬時有效的 ISO 代碼，但無效
值被預先拒絕。

如果基礎設施需要明確的證券確認，那麼橋樑
繼續發出 `sese.025`，但該確認反映了證券腿
狀態（例如，`ConfSts = ACCP`）而不是 PvP“目的”。

### 付款與付款確認 → `sese.025`

| PvP 領域 | ISO 20022 路徑 |筆記|
|------------------------------------------------------------|----------------------------------------|------|
| `settlement_id` | `TxId` |穩定的生命週期標識符 |
| `primary_leg.asset_definition_id` | `SttlmCcy` |主要支線的貨幣代碼 |
| `primary_leg.quantity` | `SttlmAmt` |發起人交付的金額 |
| `counter_leg.asset_definition_id` | `AddtlInf`（JSON 有效負載）|補充信息中嵌入的櫃檯貨幣代碼 |
| `counter_leg.quantity` | `SttlmQty` |櫃檯金額|
| `plan.order` | `Plan/ExecutionOrder` |與 DvP | 相同的枚舉集
| `plan.atomicity` | `Plan/Atomicity` |與 DvP | 相同的枚舉集
| `plan.atomicity` 狀態 (`ConfSts`) | `ConfSts` |匹配時為 `ACCP`；橋在拒絕​​時發出故障代碼 |
|交易對手標識符| `AddtlInf` JSON |當前橋在元數據中序列化完整的 AccountId/BIC 元組 |

示例（具有鏈接、保留和市場 MIC 的 CLI ISO 預覽）：

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from <i105-account-id> \
  --delivery-to <i105-account-id> \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from <i105-account-id> \
  --payment-to <i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### 回購抵押品替代 → `colr.007`|回購字段/上下文 | ISO 20022 路徑 |筆記|
|------------------------------------------------|------------------------------------------------|--------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` |回購合約標識符 |
|抵押品替代交易標識符 | `TxId` |每次替換生成 |
|原始抵押品數量 | `Substitution/OriginalAmt` |比賽在替換前承諾抵押品|
|原始抵押幣 | `Substitution/OriginalCcy` |貨幣代碼 |
|替代抵押品數量 | `Substitution/SubstituteAmt` |更換金額 |
|替代抵押貨幣 | `Substitution/SubstituteCcy` |貨幣代碼 |
|生效日期（治理保證金時間表）| `Substitution/EffectiveDt` | ISO 日期 (YYYY-MM-DD) |
|理髮分類| `Substitution/Type` |目前基於治理策略的 `FULL` 或 `PARTIAL` |
|治理/理髮注意| `Substitution/ReasonCd` |可選，承載治理原理|
|理髮尺寸| `Substitution/Haircut` |數字;映射替換期間應用的髮型 |
|原始/替代儀器 ID | `Substitution/OriginalFinInstrmId`、`Substitution/SubstituteFinInstrmId` |每條腿可選 ISIN/CUSIP |

### 資金和報表

| Iroha 上下文 | ISO 20022 消息 |地圖位置 |
|----------------------------------|--------------------------------|------------------|
|回購現金腿點火/解除| `pacs.009` | `IntrBkSttlmAmt`、`IntrBkSttlmCcy`、`IntrBkSttlmDt`、`InstgAgt`、`InstdAgt` 從 DvP/PvP 分支填充 |
|結算後報表| `camt.054` |支付腿移動記錄在 `Ntfctn/Ntry[*]` 下；橋在 `SplmtryData` 中註入賬本/賬戶元數據 |

### 使用說明* 所有金額均使用 Norito 數字助手 (`NumericSpec`) 進行序列化
  確保資產定義之間的規模一致性。
* `TxId` 值為 `Max35Text` — 強制 UTF-8 長度≤35 個字符
  導出為 ISO 20022 消息。
* BIC 必須是 8 或 11 個大寫字母數字字符 (ISO9362)；拒絕
  Norito 在發出付款或結算之前未通過此檢查的元數據
  確認。
* 賬戶標識符（AccountId / ChainId）導出到補充中
  元數據，以便接收參與者可以根據其本地分類賬進行核對。
* `SupplementaryData` 必須是規範的 JSON（UTF-8、排序鍵、JSON 原生
  逃跑）。 SDK 幫助程序強制執行此操作，以便簽名、遙測哈希和 ISO
  有效負載檔案在重建過程中保持確定性。
* 貨幣金額遵循 ISO4217 小數位（例如 JPY 為 0
  小數點，美元有 2);橋相應地箝位 Norito 數字精度。
* CLI 結算助手 (`iroha app settlement ... --atomicity ...`) 現在發出
  Norito 指令，其執行計劃以 1:1 映射到 `Plan/ExecutionOrder`，並且
  `Plan/Atomicity` 以上。
* ISO 助手 (`ivm::iso20022`) 驗證上面列出的字段並拒絕
  DvP/PvP 分支違反數字規範或交易對手互惠的消息。

### SDK 構建器助手

- JavaScript SDK 現在公開 `buildPacs008Message` /
  `buildPacs009Message`（參見 `javascript/iroha_js/src/isoBridge.js`）所以客戶端
  自動化可以轉換結構化結算元數據（BIC/LEI、IBAN、
  目的代碼、補充 Norito 字段）轉換為確定性 pacs XML
  無需重新實現本指南中的映射規則。
- 兩個助手都需要明確的 `creationDateTime`（帶時區的 ISO-8601）
  因此操作員必須從其工作流程中線程化確定性時間戳
  讓 SDK 默認為掛鐘時間。
- `recipes/iso_bridge_builder.mjs` 演示瞭如何將這些助手連接到
  合併環境變量或 JSON 配置文件的 CLI，打印
  生成的 XML，並可選擇將其提交給 Torii (`ISO_SUBMIT=1`)，重用
  與 ISO 橋配方相同的等待節奏。


### 參考文獻

- LuxCSD / Clearstream ISO 20022 結算示例顯示 `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) 和 `Pmt` (`APMT`/`FREE`)。 [1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Clearstream DCP 規範涵蓋結算限定符（`SttlmTxCond`、`PrtlSttlmInd`）。 [2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- SWIFT PMPG 指南建議將 `pacs.009` 和 `CtgyPurp/Cd = SECU` 用於證券相關的 PvP 融資。 [3](https://www.swift.com/swift-resource/251897/download)
- 針對標識符長度限制的 ISO 20022 消息定義報告（BIC、Max35Text）。 [4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- ANNA DSB 有關 ISIN 格式和校驗和規則的指南。 [5](https://www.anna-dsb.com/isin/)

### 使用技巧- 始終粘貼相關的 Norito 片段或 CLI 命令，以便 LLM 可以檢查
  準確的字段名稱和數字比例。
- 請求引用 (`provide clause references`) 以保留書面記錄
  合規性和審計師審查。
- 在 `docs/source/finance/settlement_iso_mapping.md` 中捕獲答案摘要
  （或鏈接的附錄），這樣未來的工程師就不需要重複查詢。

## 事件排序手冊（ISO 20022 ↔ Norito Bridge）

### 場景 A — 抵押品替代（回購/質押）

**參與者：** 抵押品給予者/接受者（和/或代理人）、託管人、CSD/T2S  
**時間：** 每個市場截止時間和 T2S 日/夜週期；協調兩條腿，使它們在同一個結算窗口內完成。

#### 消息編排
1. `colr.010` 抵押品替代請求 → 抵押品給予者/接受者或代理人。  
2. `colr.011` 抵押替代響應 → 接受/拒絕（可選拒絕原因）。  
3. `colr.012` 抵押品替代確認 → 確認替代協議。  
4、`sese.023`指令（兩條腿）：  
   - 返還原始抵押品（`SctiesMvmntTp=DELI`、`Pmt=FREE`、`SctiesTxTp=COLO`）。  
   - 交付替代抵押品（`SctiesMvmntTp=RECE`、`Pmt=FREE`、`SctiesTxTp=COLI`）。  
   鏈接該對（見下文）。  
5. `sese.024` 狀態建議（已接受、匹配、待處理、失敗、拒絕）。  
6. `sese.025` 預訂後確認。  
7. 可選現金增量（費用/折扣） → `pacs.009` 通過 `CtgyPurp/Cd = SECU` 進行 FI 到 FI 信用轉賬；狀態通過 `pacs.002`，通過 `pacs.004` 返回。

#### 所需的確認/狀態
- 傳輸層：網關可能會在業務處理之前發出 `admi.007` 或拒絕。  
- 結算生命週期：`sese.024`（處理狀態+原因代碼）、`sese.025`（最終）。  
- 現金方面：`pacs.002`（`PDNG`、`ACSC`、`RJCT` 等）、`pacs.004` 用於退貨。

#### 條件/展開字段
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) 鏈接兩條指令。  
- `SttlmParams/HldInd` 保留直至滿足標準；通過 `sese.030` 發布（`sese.031` 狀態）。  
- `SttlmParams/PrtlSttlmInd` 控制部分沉降（`NPAR`、`PART`、`PARC`、`PARQ`）。  
- `SttlmParams/SttlmTxCond/Cd` 適用於市場特定條件（`NOMC` 等）。  
- 可選的 T2S 有條件證券交割 (CoSD) 規則（如果支持）。

#### 參考文獻
- SWIFT 抵押品管理 MDR (`colr.010/011/012`)。  
- 用於鏈接和狀態的 CSD/T2S 使用指南（例如 DNB、ECB Insights）。  
- SMPG 結算實踐、Clearstream DCP 手冊、ASX ISO 研討會。

### 場景 B — 外匯窗口違規（PvP 資金失敗）

**參與者：** 交易對手和現金代理人、證券託管人、CSD/T2S  
**時間安排：** FX PvP 窗口（CLS/雙邊）和 CSD 截止；在現金確認之前，保持證券交易狀態。#### 消息編排
1. `pacs.009` 通過 `CtgyPurp/Cd = SECU` 每種貨幣進行 FI 到 FI 信用轉賬；通過 `pacs.002` 獲取狀態；通過 `camt.056`/`camt.029` 召回/取消；如果已經結算，則返回 `pacs.004`。  
2. `sese.023` 與 `HldInd=true` 的 DvP 指令，因此證券分支等待現金確認。  
3. 生命週期 `sese.024` 通知（已接受/已匹配/待定）。  
4. 如果 `pacs.009` 的兩條腿在窗口到期前均達到 `ACSC` → 以 `sese.030` 釋放 → `sese.031` (mod 狀態) → `sese.025` (確認)。  
5. 如果外匯窗口被突破 → 取消/召回現金（`camt.056/029` 或 `pacs.004`）並取消證券（`sese.020` + `sese.027`，或 `sese.026` 逆轉（如果已根據市場規則確認）。

#### 所需的確認/狀態
- 現金：`pacs.002`（`PDNG`、`ACSC`、`RJCT`）、`pacs.004` 用於退貨。  
- 證券：`sese.024`（待決/失敗原因，如 `NORE`、`ADEA`）、`sese.025`。  
- 傳輸：`admi.007`/網關在業務處理之前拒絕。

#### 條件/展開字段
- `SttlmParams/HldInd` + `sese.030` 成功/失敗時釋放/取消。  
- `Lnkgs` 將證券指令與現金部分聯繫起來。  
- T2S CoSD 規則（如果使用有條件交付）。  
- `PrtlSttlmInd` 以防止意外部分。  
- 在 `pacs.009` 上，`CtgyPurp/Cd = SECU` 標記與證券相關的資金。

#### 參考文獻
- PMPG / CBPR+ 證券流程支付指南。  
- SMPG 結算實踐、關於鏈接/保留的 T2S 見解。  
- Clearstream DCP 手冊、維護消息的 ECMS 文檔。

### pacs.004 返回映射註釋

- 退貨固定裝置現在規範化 `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) 和公開為 `TxInf[*]/RtrdRsn/Prtry` 的專有退貨原因，因此橋樑消費者可以重播費用歸屬和運營商代碼，而無需重新解析XML 信封。
- `DataPDU` 信封內的 AppHdr 簽名塊在攝取時仍然被忽略；審計應依賴於渠道來源而不是嵌入的 XMLDSIG 字段。

### 橋樑操作檢查表
- 執行上述編排（抵押品：`colr.010/011/012 → sese.023/024/025`；外匯違規：`pacs.009 (+pacs.002) → sese.023 held → release/cancel`）。  
- 將 `sese.024`/`sese.025` 狀態和 `pacs.002` 結果視為門控信號； `ACSC` 觸發釋放，`RJCT` 強制釋放。  
- 通過 `HldInd`、`Lnkgs`、`PrtlSttlmInd`、`SttlmTxCond` 和可選 CoSD 規則對有條件交付進行編碼。  
- 需要時，使用 `SupplementaryData` 關聯外部 ID（例如 `pacs.009` 的 UETR）。  
- 通過市場日曆/截止時間參數化持有/平倉時間；在取消截止日期之前發出 `sese.030`/`camt.056`，必要時退回退貨。

### ISO 20022 有效負載示例（帶註釋）

#### 具有指令鏈接的附帶替換對 (`sese.023`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```提交鏈接指令 `SUBST-2025-04-001-B`（替代抵押品的 FoP 接收），其中包含 `SctiesMvmntTp=RECE`、`Pmt=FREE` 和指向回 `SUBST-2025-04-001-A` 的 `WITH` 鏈接。一旦替換獲得批准，用匹配的 `sese.030` 釋放兩條腿。

#### 證券腿處於等待外匯確認狀態 (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

一旦 `pacs.009` 腿到達 `ACSC` 就釋放：

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` 確認解除持有，一旦證券腿被預訂，`sese.025` 隨後確認。

#### PvP 資金來源（`pacs.009` 具有證券用途）

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` 跟踪付款狀態（`ACSC` = 已確認，`RJCT` = 拒絕）。如果窗口被突破，請通過 `camt.056`/`camt.029` 調用或發送 `pacs.004` 以返還已結算資金。