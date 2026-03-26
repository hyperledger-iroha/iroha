---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 300ae819b5315b1ae4edab6caffc936e506c0d550a1ba75be1ace4d42f8e0b11
source_last_modified: "2026-01-28T17:11:30.701656+00:00"
translation_last_reviewed: 2026-02-07
id: registrar-api
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/sns/registrar_api.md`，現在用作
規範門戶副本。源文件保留用於翻譯工作流程。
:::

# SNS 註冊器 API 和治理掛鉤 (SN-2b)

**狀態：** 起草於 2026 年 3 月 24 日——根據 Nexus 核心審查  
**路線圖鏈接：** SN-2b“註冊商 API 和治理掛鉤”  
**先決條件：** [`registry-schema.md`](./registry-schema.md) 中的架構定義

本說明指定了操作 Sora 名稱服務 (SNS) 註冊器所需的 Torii 端點、gRPC 服務、請求/響應 DTO 和治理工件。它是需要註冊、續訂或管理 SNS 名稱的 SDK、錢包和自動化的權威合約。

## 1. 傳輸和認證

|要求 |詳情 |
|-------------|--------|
|協議| `/v1/sns/*` 和 gRPC 服務 `sns.v1.Registrar` 下的 REST。兩者都接受 Norito-JSON (`application/json`) 和 Norito-RPC 二進製文件 (`application/x-norito`)。 |
|授權 |每個後綴管理員頒發的 `Authorization: Bearer` 令牌或 mTLS 證書。治理敏感端點（凍結/解凍、保留分配）需要 `scope=sns.admin`。 |
|速率限制 |註冊服務商與 JSON 調用者共享 `torii.preauth_scheme_limits` 存儲桶以及每個後綴的突發上限：`sns.register`、`sns.renew`、`sns.controller`、`sns.freeze`。 |
|遙測| Torii 向註冊商處理程序公開 `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`（在 `scheme="norito_rpc"` 上進行過濾）； API 還會遞增 `sns_registrar_status_total{result, suffix_id}`。 |

## 2.DTO 概述

字段引用 [`registry-schema.md`](./registry-schema.md) 中定義的規範結構。所有有效負載均嵌入 `NameSelectorV1` + `SuffixId` 以避免路由不明確。

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3.REST 端點

|端點 |方法|有效負載|描述 |
|----------|--------|---------|------------|
| `/v1/sns/names` |發布 | `RegisterNameRequestV1` |註冊或重新命名名稱。解決定價層、驗證支付/治理證明、發出註冊事件。 |
| `/v1/sns/names/{namespace}/{literal}/renew` |發布 | `RenewNameRequestV1` |延長期限。強制實施政策中的寬限/贖回窗口。 |
| `/v1/sns/names/{namespace}/{literal}/transfer` |發布 | `TransferNameRequestV1` |一旦獲得治理批准，就轉讓所有權。 |
| `/v1/sns/names/{namespace}/{literal}/controllers` |放置 | `UpdateControllersRequestV1` |更換控制器組；驗證簽名的帳戶地址。 |
| `/v1/sns/names/{namespace}/{literal}/freeze` |發布 | `FreezeNameRequestV1` |監護人/議會凍結。需要監護人票證和治理記錄參考。 |
| `/v1/sns/names/{namespace}/{literal}/freeze` |刪除 | `GovernanceHookV1` |修復後解凍；確保理事會推翻記錄。 |
| `/v1/sns/reserved/{selector}` |發布 | `ReservedAssignmentRequestV1` |管理員/理事會分配保留名稱。 |
| `/v1/sns/policies/{suffix_id}` |獲取 | — |獲取當前 `SuffixPolicyV1`（可緩存）。 |
| `/v1/sns/names/{namespace}/{literal}` |獲取 | — |返回當前 `NameRecordV1` + 有效狀態（Active、Grace 等）。 |

**選擇器編碼：** `{selector}` 路徑段接受 I105（首選）、壓縮（`sora`，第二好）或每個 ADDR-5 的規範十六進制； Torii 通過 `NameSelectorV1` 將其標準化。

**錯誤模型：**所有端點都返回 Norito JSON，其中包含 `code`、`message`、`details`。代碼包括 `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing`。

### 3.1 CLI 幫助程序（N0 手動註冊商要求）

封閉測試管理員現在可以通過 CLI 行使註冊商的職責，而無需手工製作 JSON：

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` 默認為 CLI 配置帳戶；重複 `--controller` 以附加其他控制器帳戶（默認 `[owner]`）。
- 內嵌支付標誌直接映射到 `PaymentProofV1`；當您已有結構化收據時，請通過 `--payment-json PATH`。元數據 (`--metadata-json`) 和治理掛鉤 (`--governance-json`) 遵循相同的模式。

只讀助手完成排練：

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

實現參見`crates/iroha_cli/src/commands/sns.rs`；這些命令重用本文檔中描述的 Norito DTO，因此 CLI 輸出與 Torii 響應逐字節匹配。

其他幫助者包括續訂、轉讓和監護人行動：

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner soraカタカナ... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` 必須包含有效的 `GovernanceHookV1` 記錄（提案 ID、投票哈希、管理員/監護人簽名）。每個命令都簡單地鏡像相應的 `/v1/sns/names/{namespace}/{literal}/…` 端點，因此 beta 操作員可以排練 SDK 將調用的確切 Torii 表面。

## 4.gRPC 服務

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

有線格式：編譯時 Norito 模式哈希記錄在
`fixtures/norito_rpc/schema_hashes.json`（行 `RegisterNameRequestV1`，
`RegisterNameResponseV1`、`NameRecordV1` 等）。

## 5. 治理要點和證據

每個變異調用都必須附加適合重放的證據：

|行動|所需治理數據|
|--------|-------------------------------------|
|標準註冊/續訂 |參考結算指令的付款證明；除非層級需要管理員批准，否則不需要理事會投票。 |
|高級註冊/保留分配 | `GovernanceHookV1` 參考提案 ID + 管理員確認。 |
|轉讓|理事會投票哈希+DAO信號哈希；爭議解決觸發轉移時監護人許可。 |
|凍結/解凍 |監護人票證簽名加上理事會覆蓋（解凍）。 |

Torii 通過檢查來驗證證明：

1. 治理賬本中存在提案 ID（`/v1/governance/proposals/{id}`），狀態為 `Approved`。
2. 哈希值與記錄的投票工件相匹配。
3. 管理員/監護人簽名引用 `SuffixPolicyV1` 中的預期公鑰。

失敗的檢查返回 `sns_err_governance_missing`。

## 6. 工作流程示例

### 6.1 標準註冊

1. 客戶端查詢 `/v1/sns/policies/{suffix_id}` 以獲取定價、寬限和可用等級。
2.客戶端構建`RegisterNameRequestV1`：
   - `selector` 源自首選 I105 或次佳壓縮 (`sora`) 標籤。
   - `term_years` 在政策範圍內。
   - `payment` 引用財務/管家分割轉移。
3. Torii 驗證：
   - 標籤標準化+保留列表。
   - 期限/總價與 `PriceTierV1` 的比較。
   - 付款證明金額 >= 計算價格 + 費用。
4.成功後Torii：
   - 持續存在 `NameRecordV1`。
   - 發出 `RegistryEventV1::NameRegistered`。
   - 發出 `RevenueAccrualEventV1`。
   - 返回新記錄+事件。

### 6.2 寬限期的更新

寬限續訂包括標準請求和懲罰檢測：

- Torii 檢查 `now` 與 `grace_expires_at` 並添加來自 `SuffixPolicyV1` 的附加費表。
- 付款證明必須包含附加費。失敗 => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` 記錄新的 `expires_at`。

### 6.3 監護人凍結和議會推翻

1. 監護人提交 `FreezeNameRequestV1` 以及引用事件 ID 的工單。
2. Torii 將記錄移動到 `NameStatus::Frozen`，發出 `NameFrozen`。
3.整改後，理事會問題優先；操作員發送 DELETE `/v1/sns/names/{namespace}/{literal}/freeze` 和 `GovernanceHookV1`。
4. Torii 驗證覆蓋，發出 `NameUnfrozen`。

## 7. 驗證和錯誤代碼

|代碼|描述 | HTTP |
|------|-------------|------|
| `sns_err_reserved` |標籤被保留或屏蔽。 | 409 | 409
| `sns_err_policy_violation` |術語、層級或控制器集違反了政策。 | 422 | 422
| `sns_err_payment_mismatch` |付款證明價值或資產不匹配。 | 402 | 402
| `sns_err_governance_missing` |所需的治理工件不存在/無效。 | 403 | 403
| `sns_err_state_conflict` |當前生命週期狀態不允許進行操作。 | 409 | 409

所有代碼均通過 `X-Iroha-Error-Code` 和結構化 Norito JSON/NRPC 信封顯示。

## 8. 實施注意事項

- Torii 將掛起的拍賣存儲在 `NameRecordV1.auction` 下，並在 `PendingAuction` 期間拒絕直接註冊嘗試。
- 付款證明重複使用Norito分類賬收據；財務服務提供幫助程序 API (`/v1/finance/sns/payments`)。
- SDK 應使用強類型幫助程序包裝這些端點，以便錢包可以提供明確的錯誤原因（`ERR_SNS_RESERVED` 等）。

## 9. 後續步驟

- 一旦 SN-3 拍賣落地，將 Torii 處理程序連接到實際的註冊合約。
- 發布引用此 API 的 SDK 特定指南 (Rust/JS/Swift)。
- 通過與治理掛鉤證據字段的交叉鏈接擴展 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)。