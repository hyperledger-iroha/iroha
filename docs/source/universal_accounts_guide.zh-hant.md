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
  域限定別名，例如 `merchant@banka.paynet` 和資料空間根別名
  例如 `merchant@paynet` 都可以解析為相同的規範 `AccountId`。
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

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

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
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
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
