---
lang: zh-hans
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

:::注意规范来源
此页面镜像 `docs/source/sns/registry_schema.md`，现在用作
规范门户副本。保留源文件以供翻译更新。
:::

# Sora 名称服务注册表架构 (SN-2a)

**状态：** 起草于 2026 年 3 月 24 日 -- 已提交 SNS 计划审核  
**路线图链接：** SN-2a“注册表架构和存储布局”  
**范围：** 定义 Sora 名称服务 (SNS) 的规范 Norito 结构、生命周期状态和发出的事件，以便注册中心和注册商实现在合约、SDK 和网关之间保持确定性。

本文档通过指定以下内容完成了 SN-2a 的架构可交付成果：

1. 标识符和哈希规则（`SuffixId`、`NameHash`、选择器推导）。
2. Norito 名称记录、后缀策略、定价层、收入分成和注册表事件的结构/枚举。
3. 用于确定性重放的存储布局和索引前缀。
4. 状态机，涵盖注册、续订、宽限/赎回、冻结和墓碑。
5. DNS/网关自动化消耗的规范事件。

## 1. 标识符和哈希

|标识符 |描述 |推导|
|------------|-------------|------------|
| `SuffixId` (`u16`) |顶级后缀的注册表范围标识符（`.sora`、`.nexus`、`.dao`）。与 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) 中的后缀目录对齐。 |由治理投票分配；存储在 `SuffixPolicyV1` 中。 |
| `SuffixSelector` |后缀的规范字符串形式（ASCII，小写）。 |示例：`.sora` → `sora`。 |
| `NameSelectorV1` |已注册标签的二进制选择器。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。标签为 NFC + 小写字母（符合 Normv1）。 |
| `NameHash` (`[u8;32]`) |合约、事件和缓存使用的主查找键。 | `blake3(NameSelectorV1_bytes)`。 |

确定性要求：

- 标签通过Normv1（严格UTS-46、STD3 ASCII、NFC）进行标准化。传入的用户字符串必须在散列之前进行标准化。
- 保留标签（来自 `SuffixPolicyV1.reserved_labels`）永远不会进入注册表；仅治理覆盖会发出 `ReservedNameAssigned` 事件。

## 2. Norito 结构

### 2.1 名称记录V1

|领域|类型 |笔记|
|--------|------|--------|
| `suffix_id` | `u16` |参考文献 `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |用于审核/调试的原始选择器字节。 |
| `name_hash` | `[u8; 32]` |地图/事件的关键。 |
| `normalized_label` | `AsciiString` |人类可读的标签（Normv1 后）。 |
| `display_label` | `AsciiString` |管家提供的外壳；可选化妆品。 |
| `owner` | `AccountId` |控制续订/转让。 |
| `controllers` | `Vec<NameControllerV1>` |引用目标帐户地址、解析器或应用程序元数据。 |
| `status` | `NameStatus` |生命周期标志（参见第 4 节）。 |
| `pricing_class` | `u8` |后缀定价等级的索引（标准、高级、保留）。 |
| `registered_at` | `Timestamp` |初始激活的区块时间戳。 |
| `expires_at` | `Timestamp` |付费期限结束。 |
| `grace_expires_at` | `Timestamp` |自动续订宽限期结束（默认+30 天）。 |
| `redemption_expires_at` | `Timestamp` |兑换窗口结束（默认+60 天）。 |
| `auction` | `Option<NameAuctionStateV1>` |当荷兰重新开放或高级拍卖活动时出现。 |
| `last_tx_hash` | `Hash` |指向生成此版本的事务的确定性指针。 |
| `metadata` | `Metadata` |任意注册商元数据（文本记录、证明）。 |

支持结构：

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

### 2.2 后缀策略V1

|领域|类型 |笔记|
|--------|------|--------|
| `suffix_id` | `u16` |主键；跨策略版本稳定。 |
| `suffix` | `AsciiString` |例如，`sora`。 |
| `steward` | `AccountId` |管家在治理章程中定义。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |默认结算资产标识符（例如`xor#sora`）。 |
| `pricing` | `Vec<PriceTierV1>` |分级定价系数和期限规则。 |
| `min_term_years` | `u8` |购买期限的下限，无论层级覆盖如何。 |
| `grace_period_days` | `u16` |默认 30。
| `redemption_period_days` | `u16` |默认 60。
| `max_term_years` | `u8` |最大预续订期限。 |
| `referral_cap_bps` | `u16` |每包机 <=1000 (10%)。 |
| `reserved_labels` | `Vec<ReservedNameV1>` |治理提供带有分配说明的列表。 |
| `fee_split` | `SuffixFeeSplitV1` |财务/管理/推荐股票（基点）。 |
| `fund_splitter_account` | `AccountId` |持有托管+分配资金的账户。 |
| `policy_version` | `u16` |每次更改都会增加。 |
| `metadata` | `Metadata` |扩展注释（KPI 契约、合规文档哈希）。 |

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

### 2.3 收入及结算记录

|结构|领域 |目的|
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |每个结算周期（每周）的路由付款的确定性记录。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 |每次付款（注册、续订、拍卖）时发出。 |

所有 `TokenValue` 字段都使用 Norito 的规范定点编码以及关联的 `SuffixPolicyV1` 中声明的货币代码。

### 2.4 注册表事件

规范事件为 DNS/网关自动化和分析提供重播日志。

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

事件必须附加到可重播日志（例如，`RegistryEvents` 域）并镜像到网关源，以便 DNS 缓存在 SLA 内失效。

## 3. 存储布局和索引

|关键|描述 |
|-----|-------------|
| `Names::<name_hash>` |主映射从 `name_hash` 到 `NameRecordV1`。 |
| `NamesByOwner::<AccountId, suffix_id>` |钱包 UI 的二级索引（分页友好）。 |
| `NamesByLabel::<suffix_id, normalized_label>` |检测冲突，增强确定性搜索能力。 |
| `SuffixPolicies::<suffix_id>` |最新的 `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` 历史记录。 |
| `RegistryEvents::<u64>` |仅附加日志由单调递增序列键入。 |

所有密钥均使用 Norito 元组进行序列化，以保持跨主机的哈希确定性。索引更新与主记录一起自动发生。

## 4. 生命周期状态机

|状态|入学条件 |允许的转换 |笔记|
|--------|-----------------|---------------------|--------|
|可用 |当 `NameRecord` 不存在时派生。 | `PendingAuction`（高级）、`Active`（标准寄存器）。 |可用性搜索仅读取索引。 |
|待拍卖 |当 `PriceTierV1.auction_kind` ≠ 无时创建。 | `Active`（拍卖结算），`Tombstoned`（无人出价）。 |拍卖发出 `AuctionOpened` 和 `AuctionSettled`。 |
|活跃|注册或续订成功。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` 驱动转换。 |
|宽限期 |当 `now > expires_at` 时自动。 | `Active`（按时更新）、`Redemption`、`Tombstoned`。 |默认+30天；仍然解决但被标记。 |
|赎回 | `now > grace_expires_at` 但 `< redemption_expires_at`。 | `Active`（延迟更新）、`Tombstoned`。 |命令需要罚款。 |
|冷冻|治理或守护冻结。 | `Active`（修复后）、`Tombstoned`。 |无法传输或更新控制器。 |
|墓碑|自愿退保、永久争议结果或过期赎回。 | `PendingAuction`（荷兰重新开放）或仍处于墓碑状态。 |事件 `NameTombstoned` 必须包含原因。 |

状态转换必须发出相应的 `RegistryEventKind`，以便下游缓存保持一致。进入荷兰重新拍卖的墓碑名称会附加 `AuctionKind::DutchReopen` 有效负载。

## 5. 规范事件和网关同步

网关通过以下方式订阅 `RegistryEventV1` 并同步到 DNS/SoraFS：

1. 获取事件序列引用的最新 `NameRecordV1`。
2.重新生成解析器模板（首选IH58 +次优压缩（`sora`）地址、文本记录）。
3. 通过 [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) 中描述的 SoraDNS 工作流程固定更新的区域数据。

活动交付保证：

- 影响 `NameRecordV1` 的每笔交易*必须*附加一个严格递增的 `version` 事件。
- `RevenueSharePosted` 事件参考 `RevenueShareRecordV1` 发出的和解。
- 冻结/解冻/逻辑删除事件包括 `metadata` 内的治理工件哈希，用于审核重放。

## 6. Norito 有效负载示例

### 6.1 名称记录示例

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
            account_address: Some(AccountAddress("0x02000001...")),
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

### 6.2 后缀策略示例

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

## 7. 后续步骤- **SN-2b（注册器 API 和治理挂钩）：** 通过 Torii（Norito 和 JSON 绑定）公开这些结构，并将准入检查连接到治理工件。
- **SN-3（拍卖和注册引擎）：** 重用 `NameAuctionStateV1` 来实现提交/显示和荷兰语重新打开逻辑。
- **SN-5（支付和结算）：** 利用 `RevenueShareRecordV1` 实现财务对账和报告自动化。

问题或更改请求应与 `roadmap.md` 中的 SNS 路线图更新一起提交，并在合并时反映在 `status.md` 中。