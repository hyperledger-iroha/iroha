<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 通用帳戶指南

本指南從以下內容中提取了 UAID（通用帳戶 ID）部署要求：
Nexus 路線圖並將其打包成以操作員 + SDK 為重點的演練。
它涵蓋 UAID 推導、投資組合/清單檢查、監管範本、
以及每個「iroha 應用程式空間目錄清單」必須隨附的證據
發布` run (roadmap reference: `roadmap.md:2209`)。

## 1.UAID快速參考- UAID 是 `uaid:<hex>` 文字，其中 `<hex>` 是 Blake2b-256 摘要，其
  LSB 設定為 `1`。規範類型位於
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`。
- 帳戶記錄（`Account` 和 `AccountDetails`）現在附有可選的 `uaid`
  字段，以便應用程式無需自訂哈希即可了解標識符。
- 隱藏函數標識符策略可以綁定任意規範化輸入
  （電話號碼、電子郵件、帳號、合作夥伴字串）到 `opaque:` ID
  在 UAID 命名空間下。鏈上的碎片是`IdentifierPolicy`，
  `IdentifierClaimRecord` 和 `opaque_id -> uaid` 索引。
- 空間目錄維護一個 `World::uaid_dataspaces` 映射來綁定每個 UAID
  活動清單引用的資料空間帳戶。 Torii 重複使用該
  `/portfolio` 和 `/uaids/*` API 的對應。
- `POST /v1/accounts/onboard` 發布預設空間目錄清單
  當不存在時全域資料空間，因此 UAID 立即綁定。
  入職機構必須持有 `CanPublishSpaceDirectoryManifest{dataspace=0}`。
- 所有 SDK 都公開了用於規範 UAID 文字的幫助程式（例如，
  Android SDK 中的 `UaidLiteral`）。助手接受原始 64 十六進位摘要
  (LSB=1) 或 `uaid:<hex>` 文字並重新使用相同的 Norito 編解碼器，以便
  摘要不能跨語言漂移。

## 1.1 隱藏識別符策略

UAID 現在是第二個身分圖層的錨點：- 全域 `IdentifierPolicyId` (`<kind>#<business_rule>`) 定義
  命名空間、公共承諾元資料、解析器驗證金鑰以及
  規範輸入標準化模式（`Exact`、`LowercaseTrimmed`、
  `PhoneE164`、`EmailAddress` 或 `AccountNumber`）。
- 一項聲明將一個衍生的 `opaque:` 識別碼恰好綁定到一個 UAID 和一個
  該政策下的規範 `AccountId`，但鏈只接受
  索賠時附有簽署的 `IdentifierResolutionReceipt`。
- 解析度仍然是 `resolve -> transfer` 流。 Torii 解決了不透明問題
  處理並返回規範的 `AccountId`；轉移目標仍然是
  規範帳戶，而不是直接 `uaid:` 或 `opaque:` 文字。
- 策略現在可以透過以下方式發布 BFV 輸入加密參數
  `PolicyCommitment.public_parameters`。當存在時，Torii 在
  `GET /v1/identifier-policies`，客戶端可以提交 BFV 包裝的輸入
  而不是明文。程式策略將 BFV 參數包裝在
  規範的 `BfvProgrammedPublicParameters` 捆綁包也發布了
  公共 `ram_fhe_profile`；傳統的原始 BFV 有效負載已升級到
  重建承諾時的規範包。
- 標識符路由經過相同的 Torii 存取權杖和速率限制
  作為其他面向應用程式的端點進行檢查。它們不是繞過正常的
  API 政策。

## 1.2 術語

命名拆分是有意的：- `ram_lfe` 是外部隱藏函數抽象化。涵蓋保單
  註冊、承諾、公共元資料、執行收據，以及
  驗證模式。
- `BFV` 是 Brakerski/Fan-Vercauteren 使用的同態加密方案
  一些 `ram_lfe` 後端來評估加密輸入。
- `ram_fhe_profile` 是 BFV 特定的元數據，而不是整個元數據的第二個名稱
  功能。它描述了錢包和
  驗證者必須針對策略使用程式設計後端的情況。

具體來說：

- `RamLfeProgramPolicy` 和 `RamLfeExecutionReceipt` 是 LFE 層類型。
- `BfvParameters`、`BfvCiphertext`、`BfvProgrammedPublicParameters` 和
  `BfvRamProgramProfile` 是 FHE 層型。
- `HiddenRamFheProgram` 和 `HiddenRamFheInstruction` 是內部名稱
  由程式設計後端執行的隱藏 BFV 程式。他們留在
  FHE 方面，因為它們描述的是加密執行機製而不是
  外部保單或收據抽象。

## 1.3 帳戶身分與別名

通用帳戶的推出不會改變規範的帳戶身分模型：- `AccountId` 仍然是規範的無網域帳戶主題。
- `AccountAlias` 值是該主題之上的單獨 SNS 綁定。一個
  域限定別名，例如 `merchant@banka.sbp` 和資料空間根別名
  例如 `merchant@sbp` 都可以解析為相同的規範 `AccountId`。
- 規範帳戶註冊始終為 `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`；沒有領域限定或領域具體化
  註冊路徑。
- 網域所有權、別名權限和其他網域範圍內的行為即時
  他們自己的狀態和 API，而不是帳戶身分本身。
- 公共帳戶查找遵循該分割：別名查詢保持公開，而
  規範帳戶身分仍然是純粹的 `AccountId`。

算子、SDK、測試的實作規則：從規範開始
`AccountId`，然後新增別名租約、資料空間/網域權限以及任何
域擁有狀態單獨。不要合成假的別名衍生帳戶
或僅僅因為別名或期望帳戶記錄上有任何連結網域字段
路由攜帶域段。

目前 Torii 路由：|路線 |目的|
|--------|---------|
| `GET /v1/ram-lfe/program-policies` |列出活動和非活動 RAM-LFE 程式策略及其公共執行元數據，包括可選 BFV `input_encryption` 參數和程式設計後端 `ram_fhe_profile`。 |
| `POST /v1/ram-lfe/programs/{program_id}/execute` |只接受 `{ input_hex }` 或 `{ encrypted_input }` 之一，並傳回所選程式的無狀態 `RamLfeExecutionReceipt` 和 `{ output_hex, output_hash, receipt_hash }`。目前的 Torii 運作時為已編程的 BFV 後端發出收據。 |
| `POST /v1/ram-lfe/receipts/verify` |根據已發布的鏈上程序策略無狀態地驗證 `RamLfeExecutionReceipt`，並可選擇檢查呼叫者提供的 `output_hex` 是否與收據 `output_hash` 相符。 |
| `GET /v1/identifier-policies` |列出活動和非活動隱藏功能策略命名空間及其公共元數據，包括可選的 BFV `input_encryption` 參數、加密客戶端輸入所需的 `normalization` 模式以及編程 BFV 策略的 `ram_fhe_profile`。 |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` |剛好接受 `{ input }` 或 `{ encrypted_input }` 之一。明文 `input` 是伺服器端規範化的； BFV `encrypted_input` 必須已根據已發佈的策略模式進行標準化。然後，端點派生 `opaque:` 句柄並傳回 `ClaimIdentifier` 可以在鏈上提交的簽章收據，包括原始 `signature_payload_hex` 和解析後的 `signature_payload`。 || `POST /v1/identifiers/resolve` |剛好接受 `{ input }` 或 `{ encrypted_input }` 之一。明文 `input` 是伺服器端規範化的； BFV `encrypted_input` 必須已根據已發佈的策略模式進行標準化。當存在有效聲明時，端點將標識符解析為 `{ opaque_id, receipt_hash, uaid, account_id, signature }`，並且也將規範簽署的有效負載傳回為 `{ signature_payload_hex, signature_payload }`。 |
| `GET /v1/identifiers/receipts/{receipt_hash}` |尋找與確定性收據雜湊綁定的持久 `IdentifierClaimRecord`，以便操作員和 SDK 可以審核聲明所有權或診斷重播/不匹配失敗，而無需掃描完整標識符索引。 |

Torii的進程內執行運行時配置在
`torii.ram_lfe.programs[*]`，由 `program_id` 鍵入。標識符現在路由
重複使用相同的 RAM-LFE 運行時而不是單獨的 `identifier_resolver`
配置表面。

目前的SDK支援：- `normalizeIdentifierInput(value, normalization)` 與 Rust 匹配
  `exact`、`lowercase_trimmed`、`phone_e164` 的規格化器，
  `email_address` 和 `account_number`。
- `ToriiClient.listIdentifierPolicies()` 列出策略元數據，包括 BFV
  策略發佈時的輸入加密元數據，加上解碼的
  透過 `input_encryption_public_parameters_decoded` 的 BFV 參數物件。
  程式設計策略也公開解碼後的 `ram_fhe_profile`。該字段是
  有意 BFV 範圍：它讓錢包驗證預期的暫存器
  計數、通道計數、規範化模式和最小密文模數
  在加密客戶端輸入之前編程的 FHE 後端。
- `getIdentifierBfvPublicParameters(policy)` 和
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` 幫助
  JS 呼叫者使用已發佈的 BFV 元資料並建立策略感知請求
  機構無需重新實施策略 ID 和規範化規則。
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` 和
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` 現在讓
  JS 錢包在本地建造完整的 BFV Norito 密文信封
  發布策略參數而不是發送預先建置的密文十六進位。
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  解析隱藏標識符並傳回簽署的收據有效負載，
  包括 `receipt_hash`、`signature_payload_hex` 和
  `signature_payload`。
-`ToriiClient.issueIdentifierClaimReceipt（accountId，{policyId，輸入|
  加密輸入})` issues the signed receipt needed by `ClaimIdentifier`。
- `verifyIdentifierResolutionReceipt(receipt, policy)` 驗證回傳的
  針對客戶端策略解析器金鑰的收據，以及`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` 獲取
  為以後的審計/調試流程保留索賠記錄。
- `IrohaSwift.ToriiClient` 現在公開 `listIdentifierPolicies()`，
  `resolveIdentifier(policyId:input:encryptedInputHex:)`，
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`，
  和 `getIdentifierClaimByReceiptHash(_)`，加上
  `ToriiIdentifierNormalization` 同一電話/電子郵件/帳號
  規範化模式。
- `ToriiIdentifierLookupRequest` 和
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` 幫助程式提供類型化的 Swift 請求表面
  解決和索賠接收調用，Swift 策略現在可以導出 BFV
  透過 `encryptInput(...)` / `encryptedRequest(input:...)` 本地密文。
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` 驗證
  頂級收據欄位與簽名的有效負載相符並驗證
  提交前解析器簽章客戶端。
- Android SDK 中的 `HttpClientTransport` 現在公開
  `listIdentifierPolicies()`，`resolveIdentifier（policyId，輸入，
  加密輸入十六進位）`, `issueIdentifierClaimReceipt（accountId，policyId，
  輸入，加密的InputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  加上 `IdentifierNormalization` 以獲得相同的規範化規則。
- `IdentifierResolveRequest` 和
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` 幫助程式提供類型化的 Android 請求表面，
  而 `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` 推導出BFV密文信封
  從本地發布的策略參數。
  `IdentifierResolutionReceipt.verifySignature(policy)` 驗證回傳的
  解析器簽章客戶端。

目前指令集：- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier`（綁定收據；原始 `opaque_id` 索賠被拒絕）
- `RevokeIdentifier`

`iroha_crypto::ram_lfe` 中現在存在三個後端：

- 歷史承諾約束的 `HKDF-SHA3-512` PRF，以及
- BFV 支援的秘密仿射評估器，使用 BFV 加密的識別符
  直接插槽。當使用預設建置 `iroha_crypto` 時
  `bfv-accel` 特徵，BFV 環乘法使用精確的確定性
  內建CRT-NTT後端；停用該功能會回到
  具有相同輸出的標量教科書路徑，以及
- BFV 支援的秘密程式評估器，可匯出指令驅動的
  對加密寄存器和密文內存進行 RAM 式執行跟踪
  派生不透明標識符和收據哈希之前的通道。已編程的
  後端現在需要比仿射路徑更強的 BFV 模數底限，並且
  它的公共參數發佈在一個規範包中，其中包括
  錢包和驗證者使用的 RAM-FHE 執行設定檔。

這裡 BFV 表示 Brakerski/Fan-Vercauteren FHE 方案實作於
`crates/iroha_crypto/src/fhe_bfv.rs`。這是加密執行機制
由仿射和編程後端使用，而不是外部隱藏的名稱
函數抽象。Torii使用策略承諾發布的後端。當 BFV 後端
處於活動狀態，明文請求先標準化，然後在伺服器端加密
評價。評估仿射後端的 BFV `encrypted_input` 請求
直接且必須已標準化客戶端；已編程的後端
將加密輸入規範化回解析器的確定性 BFV
執行秘密 RAM 程式之前的信封，以便保留收據哈希值
在語意等效的密文中保持穩定。

## 2. 匯出並驗證 UAID

支援三種取得 UAID 的方式：

1. **從世界狀態或 SDK 模型讀取。 ** 任何 `Account`/`AccountDetails`
   透過 Torii 查詢的有效負載現在填充了 `uaid` 字段
   參與者選擇使用通用帳戶。
2. **查詢 UAID 註冊表。 ** Torii 公開
   `GET /v1/space-directory/uaids/{uaid}` 返回資料空間綁定
   以及空間目錄主機保留的清單元資料（請參閱
   `docs/space-directory.md` §3 用於有效負載樣本）。
3. **確定性地推導它。 ** 當離線引導新的 UAID 時，雜湊
   規範參與者種子為 Blake2b-256 並在結果前加上前綴
   `uaid:`。下面的程式碼片段反映了中記錄的幫助程序
   `docs/space-directory.md` §3.3：

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```始終以小寫形式儲存文字，並在雜湊之前標準化空格。
CLI 幫助程序，例如 `iroha app space-directory manifest scaffold` 和 Android
`UaidLiteral` 解析器應用相同的修剪規則，因此治理審查可以
無需臨時腳本即可交叉檢查值。

## 3. 檢查 UAID 持有量和清單

`iroha_core::nexus::portfolio` 中的確定性投資組合聚合器
顯示引用 UAID 的每個資產/資料空間對。營運商和 SDK
可以透過以下表面消費數據：

|表面|用途 |
|--------|--------|
| `GET /v1/accounts/{uaid}/portfolio` |返回資料空間→資產→餘額摘要； `docs/source/torii/portfolio_api.md` 中描述。 |
| `GET /v1/space-directory/uaids/{uaid}` |列出與 UAID 關聯的資料空間 ID + 帳戶文字。 |
| `GET /v1/space-directory/uaids/{uaid}/manifests` |提供完整的 `AssetPermissionManifest` 歷史記錄以供審核。 |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI 捷徑包裝綁定端點並可選擇將 JSON 寫入磁碟 (`--json-out`)。 |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` |取得證據包的清單 JSON 套件。 |

CLI 會話範例（透過 `iroha.json` 中的 `torii_api_url` 設定的 Torii URL）：

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

將 JSON 快照與審核期間使用的清單雜湊一起儲存；的
每當出現時，空間目錄觀察器就會重建 `uaid_dataspaces` 映射
啟動、過期或撤銷，因此這些快照是證明的最快方法
在給定的時期哪些綁定是活躍的。## 4. 出版能力有據可依

每當推出新配額時，請使用下方的 CLI 流程。每一步都必須
記錄在用於治理簽署的證據包中的土地。

1. **對清單 JSON 進行編碼**，以便審閱者可以在之前看到確定性哈希
   提交：

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **使用 Norito 有效負載 (`--manifest`) 或
   JSON 描述 (`--manifest-json`)。記錄 Torii/CLI 收據以及
   `PublishSpaceDirectoryManifest` 指令哈希：

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **捕獲SpaceDirectory事件證據。 ** 訂閱
   `SpaceDirectoryEvent::ManifestActivated` 並將事件有效負載包含在
   以便審計人員可以確認變更何時生效。

4. **產生審核包** 將清單與其資料空間設定檔連結起來，並
   遙測掛鉤：

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **透過 Torii**（`bindings fetch` 和 `manifests fetch`）驗證綁定
   使用上面的雜湊 + 捆綁包歸檔這些 JSON 檔案。

證據清單：

- [ ] 由變更審核者簽署的清單雜湊 (`*.manifest.hash`)。
- [ ] CLI/Torii 發布調用的收據（stdout 或 `--json-out` 工件）。
- [ ] `SpaceDirectoryEvent` 有效負載證明啟動。
- [ ] 審核包含資料空間設定檔、掛鉤和清單副本的捆綁包目錄。
- [ ] 綁定 + 從 Torii 啟動後取得的清單快照。這反映了 `docs/space-directory.md` §3.2 中的要求，同時提供 SDK
擁有在發布審核期間可指向的單一頁面。

## 5. 監管機構/區域清單模板

當製作能力顯現時，使用回購中的固定裝置作為起點
對於監管機構或地區監管機構。他們示範如何確定允許/拒絕的範圍
規則並解釋審查者期望的政策說明。

|夾具|目的|亮點|
|--------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB 稽核來源。 | `compliance.audit::{stream_reports, request_snapshot}` 的唯讀津貼，並拒絕零售轉賬，以保持監管機構 UAID 的被動。 |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA 監管車道。 |增加有上限的 `cbdc.supervision.issue_stop_order` 限額（每日窗口 + `max_amount`）和對 `force_liquidation` 的明確拒絕，以實施雙重控制。 |

克隆這些裝置時，更新：

1. `uaid` 和 `dataspace` id 與您啟用的參與者和通道相符。
2. `activation_epoch`/`expiry_epoch` 基於治理時間表的視窗。
3. `notes` 欄位以及監管機構的政策參考（MiCA 文章，JFSA
   圓形等）。
4. 津貼窗口（`PerSlot`、`PerMinute`、`PerDay`）和可選
   `max_amount` 上限，因此 SDK 強制執行與主機相同的限制。

## 6. SDK 用戶的遷移說明引用每個網域帳戶 ID 的現有 SDK 整合必須遷移到
上面描述的以 UAID 為中心的表面。在升級期間使用此清單：

  帳戶 ID。對於 Rust/JS/Swift/Android，這意味著升級到最新版本
  工作區板條箱或重新產生 Norito 綁定。
- **API 呼叫：** 將網域範圍的投資組合查詢替換為
  `GET /v1/accounts/{uaid}/portfolio` 和清單/綁定端點。
  `GET /v1/accounts/{uaid}/portfolio` 接受可選的 `asset_id` 查詢
  當錢包只需要單一資產實例時的參數。客戶幫手如
  如 `ToriiClient.getUaidPortfolio` (JS) 和 Android
  `SpaceDirectoryClient` 已經包裝了這些路由；比起定制更喜歡它們
  HTTP 程式碼。
- **快取和遙測：** 透過 UAID + 資料空間而不是原始快取條目
  帳戶 ID，並發出顯示 UAID 文字的遙測數據，以便操作可以
  將日誌與空間目錄證據對齊。
- **錯誤處理：**新端點傳回嚴格的UAID解析錯誤
  記錄在 `docs/source/torii/portfolio_api.md` 中；表面那些代碼
  逐字記錄，以便支援團隊可以對問題進行分類，而無需重複步驟。
- **測試：** 連接上述固定裝置（加上您自己的 UAID 清單）
  進入 SDK 測試套件以證明 Norito 往返和清單評估
  匹配主機實作。

## 7. 參考文獻- `docs/space-directory.md` — 具有更深入生命週期詳細資訊的操作手冊。
- `docs/source/torii/portfolio_api.md` — UAID 組合的 REST 架構和
  明顯的端點。
- `crates/iroha_cli/src/space_directory.rs` — 中引用的 CLI 實現
  本指南。
- `fixtures/space_directory/capability/*.manifest.json` — 監管機構、零售和
  CBDC 清單範本可供克隆。