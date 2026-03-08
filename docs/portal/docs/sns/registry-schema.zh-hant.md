---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5307c80eba9ab93d4522c3e88485fa4d24f7f04903b7aea30b05e880e2c096b0
source_last_modified: "2026-01-28T17:11:30.697638+00:00"
translation_last_reviewed: 2026-02-07
id: registry-schema
title: Sora Name Service Registry Schema
sidebar_label: Registry schema
description: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/sns/registry_schema.md`，現在用作
規範門戶副本。保留源文件以供翻譯更新。
:::

# Sora 名稱服務註冊表架構 (SN-2a)

**狀態：** 起草於 2026 年 3 月 24 日 -- 已提交 SNS 計劃審核  
**路線圖鏈接：** SN-2a“註冊表架構和存儲佈局”  
**範圍：** 定義 Sora 名稱服務 (SNS) 的規範 Norito 結構、生命週期狀態和發出的事件，以便註冊中心和註冊商實現在合約、SDK 和網關之間保持確定性。

本文檔通過指定以下內容完成了 SN-2a 的架構可交付成果：

1. 標識符和哈希規則（`SuffixId`、`NameHash`、選擇器推導）。
2. Norito 名稱記錄、後綴策略、定價層、收入分成和註冊表事件的結構/枚舉。
3. 用於確定性重放的存儲佈局和索引前綴。
4. 狀態機，涵蓋註冊、續訂、寬限/贖回、凍結和墓碑。
5. DNS/網關自動化消耗的規範事件。

## 1. 標識符和哈希

|標識符 |描述 |推導|
|------------|-------------|------------|
| `SuffixId` (`u16`) |頂級後綴的註冊表範圍標識符（`.sora`、`.nexus`、`.dao`）。與 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) 中的後綴目錄對齊。 |由治理投票分配；存儲在 `SuffixPolicyV1` 中。 |
| `SuffixSelector` |後綴的規範字符串形式（ASCII，小寫）。 |示例：`.sora` → `sora`。 |
| `NameSelectorV1` |已註冊標籤的二進制選擇器。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。標籤為 NFC + 小寫字母（符合 Normv1）。 |
| `NameHash` (`[u8;32]`) |合約、事件和緩存使用的主查找鍵。 | `blake3(NameSelectorV1_bytes)`。 |

確定性要求：

- 標籤通過Normv1（嚴格UTS-46、STD3 ASCII、NFC）進行標準化。傳入的用戶字符串必須在散列之前進行標準化。
- 保留標籤（來自 `SuffixPolicyV1.reserved_labels`）永遠不會進入註冊表；僅治理覆蓋會發出 `ReservedNameAssigned` 事件。

## 2. Norito 結構

### 2.1 名稱記錄V1

|領域|類型 |筆記|
|--------|------|--------|
| `suffix_id` | `u16` |參考文獻 `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |用於審核/調試的原始選擇器字節。 |
| `name_hash` | `[u8; 32]` |地圖/事件的關鍵。 |
| `normalized_label` | `AsciiString` |人類可讀的標籤（Normv1 後）。 |
| `display_label` | `AsciiString` |管家提供的外殼；可選化妝品。 |
| `owner` | `AccountId` |控制續訂/轉讓。 |
| `controllers` | `Vec<NameControllerV1>` |引用目標帳戶地址、解析器或應用程序元數據。 |
| `status` | `NameStatus` |生命週期標誌（參見第 4 節）。 |
| `pricing_class` | `u8` |後綴定價等級的索引（標準、高級、保留）。 |
| `registered_at` | `Timestamp` |初始激活的區塊時間戳。 |
| `expires_at` | `Timestamp` |付費期限結束。 |
| `grace_expires_at` | `Timestamp` |自動續訂寬限期結束（默認+30 天）。 |
| `redemption_expires_at` | `Timestamp` |兌換窗口結束（默認+60 天）。 |
| `auction` | `Option<NameAuctionStateV1>` |當荷蘭重新開放或高級拍賣活動時出現。 |
| `last_tx_hash` | `Hash` |指向生成此版本的事務的確定性指針。 |
| `metadata` | `Metadata` |任意註冊商元數據（文本記錄、證明）。 |

支持結構：

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x…` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 後綴策略V1

|領域|類型 |筆記|
|--------|------|--------|
| `suffix_id` | `u16` |主鍵；跨策略版本穩定。 |
| `suffix` | `AsciiString` |例如，`sora`。 |
| `steward` | `AccountId` |管家在治理章程中定義。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |默認結算資產標識符（例如`xor#sora`）。 |
| `pricing` | `Vec<PriceTierV1>` |分級定價係數和期限規則。 |
| `min_term_years` | `u8` |購買期限的下限，無論層級覆蓋如何。 |
| `grace_period_days` | `u16` |默認 30。
| `redemption_period_days` | `u16` |默認 60。
| `max_term_years` | `u8` |最大預續訂期限。 |
| `referral_cap_bps` | `u16` |每包機 <=1000 (10%)。 |
| `reserved_labels` | `Vec<ReservedNameV1>` |治理提供帶有分配說明的列表。 |
| `fee_split` | `SuffixFeeSplitV1` |財務/管理/推荐股票（基點）。 |
| `fund_splitter_account` | `AccountId` |持有託管+分配資金的賬戶。 |
| `policy_version` | `u16` |每次更改都會增加。 |
| `metadata` | `Metadata` |擴展註釋（KPI 契約、合規文檔哈希）。 |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 收入及結算記錄

|結構|領域 |目的|
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |每個結算週期（每週）的路由付款的確定性記錄。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 |每次付款（註冊、續訂、拍賣）時發出。 |

所有 `TokenValue` 字段都使用 Norito 的規範定點編碼以及關聯的 `SuffixPolicyV1` 中聲明的貨幣代碼。

### 2.4 註冊表事件

規範事件為 DNS/網關自動化和分析提供重播日誌。

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

事件必須附加到可重播日誌（例如，`RegistryEvents` 域）並鏡像到網關源，以便 DNS 緩存在 SLA 內失效。

## 3. 存儲佈局和索引

|關鍵|描述 |
|-----|-------------|
| `Names::<name_hash>` |主映射從 `name_hash` 到 `NameRecordV1`。 |
| `NamesByOwner::<AccountId, suffix_id>` |錢包 UI 的二級索引（分頁友好）。 |
| `NamesByLabel::<suffix_id, normalized_label>` |檢測衝突，增強確定性搜索能力。 |
| `SuffixPolicies::<suffix_id>` |最新的 `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` 歷史記錄。 |
| `RegistryEvents::<u64>` |僅附加日誌由單調遞增序列鍵入。 |

所有密鑰均使用 Norito 元組進行序列化，以保持跨主機的哈希確定性。索引更新與主記錄一起自動發生。

## 4. 生命週期狀態機

|狀態|入學條件 |允許的轉換 |筆記|
|--------|-----------------|---------------------|--------|
|可用 |當 `NameRecord` 不存在時派生。 | `PendingAuction`（高級）、`Active`（標準寄存器）。 |可用性搜索僅讀取索引。 |
|待拍賣 |當 `PriceTierV1.auction_kind` ≠ 無時創建。 | `Active`（拍賣結算），`Tombstoned`（無人出價）。 |拍賣發出 `AuctionOpened` 和 `AuctionSettled`。 |
|活躍|註冊或續訂成功。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` 驅動轉換。 |
|寬限期 |當 `now > expires_at` 時自動。 | `Active`（按時更新）、`Redemption`、`Tombstoned`。 |默認+30天；仍然解決但被標記。 |
|贖回 | `now > grace_expires_at` 但 `< redemption_expires_at`。 | `Active`（延遲更新）、`Tombstoned`。 |命令需要罰款。 |
|冷凍|治理或守護凍結。 | `Active`（修復後）、`Tombstoned`。 |無法傳輸或更新控制器。 |
|墓碑|自願退保、永久爭議結果或過期贖回。 | `PendingAuction`（荷蘭重新開放）或仍處於墓碑狀態。 |事件 `NameTombstoned` 必須包含原因。 |

狀態轉換必鬚髮出相應的 `RegistryEventKind`，以便下游緩存保持一致。進入荷蘭重新拍賣的墓碑名稱會附加 `AuctionKind::DutchReopen` 有效負載。

## 5. 規範事件和網關同步

網關通過以下方式訂閱 `RegistryEventV1` 並同步到 DNS/SoraFS：

1. 獲取事件序列引用的最新 `NameRecordV1`。
2.重新生成解析器模板（首選IH58 +次優壓縮（`sora`）地址、文本記錄）。
3. 通過 [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) 中描述的 SoraDNS 工作流程固定更新的區域數據。

活動交付保證：

- 影響 `NameRecordV1` 的每筆交易*必須*附加一個嚴格遞增的 `version` 事件。
- `RevenueSharePosted` 事件參考 `RevenueShareRecordV1` 發出的和解。
- 凍結/解凍/邏輯刪除事件包括 `metadata` 內的治理工件哈希，用於審核重放。

## 6. Norito 有效負載示例

### 6.1 名稱記錄示例

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 後綴策略示例

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. 後續步驟- **SN-2b（註冊器 API 和治理掛鉤）：** 通過 Torii（Norito 和 JSON 綁定）公開這些結構，並將准入檢查連接到治理工件。
- **SN-3（拍賣和註冊引擎）：** 重用 `NameAuctionStateV1` 來實現提交/顯示和荷蘭語重新打開邏輯。
- **SN-5（支付和結算）：** 利用 `RevenueShareRecordV1` 實現財務對賬和報告自動化。

問題或更改請求應與 `roadmap.md` 中的 SNS 路線圖更新一起提交，並在合併時反映在 `status.md` 中。