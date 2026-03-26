---
id: registrar-api
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/sns/registrar_api.md`，现在用作
规范门户副本。源文件保留用于翻译工作流程。
:::

# SNS 注册器 API 和治理挂钩 (SN-2b)

**状态：** 起草于 2026 年 3 月 24 日——根据 Nexus 核心审查  
**路线图链接：** SN-2b“注册商 API 和治理挂钩”  
**先决条件：** [`registry-schema.md`](./registry-schema.md) 中的架构定义

本说明指定了操作 Sora 名称服务 (SNS) 注册器所需的 Torii 端点、gRPC 服务、请求/响应 DTO 和治理工件。它是需要注册、续订或管理 SNS 名称的 SDK、钱包和自动化的权威合约。

## 1. 传输和认证

|要求 |详情 |
|-------------|--------|
|协议| `/v1/sns/*` 和 gRPC 服务 `sns.v1.Registrar` 下的 REST。两者都接受 Norito-JSON (`application/json`) 和 Norito-RPC 二进制文件 (`application/x-norito`)。 |
|授权 |每个后缀管理员颁发的 `Authorization: Bearer` 令牌或 mTLS 证书。治理敏感端点（冻结/解冻、保留分配）需要 `scope=sns.admin`。 |
|速率限制 |注册服务商与 JSON 调用者共享 `torii.preauth_scheme_limits` 存储桶以及每个后缀的突发上限：`sns.register`、`sns.renew`、`sns.controller`、`sns.freeze`。 |
|遥测| Torii 向注册商处理程序公开 `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`（在 `scheme="norito_rpc"` 上进行过滤）； API 还会递增 `sns_registrar_status_total{result, suffix_id}`。 |

## 2.DTO 概述

字段引用 [`registry-schema.md`](./registry-schema.md) 中定义的规范结构。所有有效负载均嵌入 `NameSelectorV1` + `SuffixId` 以避免路由不明确。

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

## 3.REST 端点

|端点 |方法|有效负载|描述 |
|----------|--------|---------|------------|
| `/v1/sns/names` |发布 | `RegisterNameRequestV1` |注册或重新命名名称。解决定价层、验证支付/治理证明、发出注册事件。 |
| `/v1/sns/names/{namespace}/{literal}/renew` |发布 | `RenewNameRequestV1` |延长期限。强制实施政策中的宽限/赎回窗口。 |
| `/v1/sns/names/{namespace}/{literal}/transfer` |发布 | `TransferNameRequestV1` |一旦获得治理批准，就转让所有权。 |
| `/v1/sns/names/{namespace}/{literal}/controllers` |放置 | `UpdateControllersRequestV1` |更换控制器组；验证签名的帐户地址。 |
| `/v1/sns/names/{namespace}/{literal}/freeze` |发布 | `FreezeNameRequestV1` |监护人/议会冻结。需要监护人票证和治理记录参考。 |
| `/v1/sns/names/{namespace}/{literal}/freeze` |删除 | `GovernanceHookV1` |修复后解冻；确保理事会推翻记录。 |
| `/v1/sns/reserved/{selector}` |发布 | `ReservedAssignmentRequestV1` |管理员/理事会分配保留名称。 |
| `/v1/sns/policies/{suffix_id}` |获取 | — |获取当前 `SuffixPolicyV1`（可缓存）。 |
| `/v1/sns/names/{namespace}/{literal}` |获取 | — |返回当前 `NameRecordV1` + 有效状态（Active、Grace 等）。 |

**选择器编码：** `{selector}` 路径段接受 i105（首选）、压缩（`sora`，第二好）或每个 ADDR-5 的规范十六进制； Torii 通过 `NameSelectorV1` 将其标准化。

**错误模型：**所有端点都返回 Norito JSON，其中包含 `code`、`message`、`details`。代码包括 `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing`。

### 3.1 CLI 帮助程序（N0 手动注册商要求）

封闭测试管理员现在可以通过 CLI 行使注册商的职责，而无需手工制作 JSON：

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

- `--owner` 默认为 CLI 配置帐户；重复 `--controller` 以附加其他控制器帐户（默认 `[owner]`）。
- 内嵌支付标志直接映射到 `PaymentProofV1`；当您已有结构化收据时，请通过 `--payment-json PATH`。元数据 (`--metadata-json`) 和治理挂钩 (`--governance-json`) 遵循相同的模式。

只读助手完成排练：

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

实现参见`crates/iroha_cli/src/commands/sns.rs`；这些命令重用本文档中描述的 Norito DTO，因此 CLI 输出与 Torii 响应逐字节匹配。

其他帮助者包括续订、转让和监护人行动：

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

`--governance-json` 必须包含有效的 `GovernanceHookV1` 记录（提案 ID、投票哈希、管理员/监护人签名）。每个命令都简单地镜像相应的 `/v1/sns/names/{namespace}/{literal}/…` 端点，因此 beta 操作员可以排练 SDK 将调用的确切 Torii 表面。

## 4.gRPC 服务

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

有线格式：编译时 Norito 模式哈希记录在
`fixtures/norito_rpc/schema_hashes.json`（行 `RegisterNameRequestV1`，
`RegisterNameResponseV1`、`NameRecordV1` 等）。

## 5. 治理要点和证据

每个变异调用都必须附加适合重放的证据：

|行动|所需治理数据|
|--------|-------------------------------------|
|标准注册/续订 |参考结算指令的付款证明；除非层级需要管理员批准，否则不需要理事会投票。 |
|高级注册/保留分配| `GovernanceHookV1` 参考提案 ID + 管理员确认。 |
|转让|理事会投票哈希+DAO信号哈希；争议解决触发转移时监护人许可。 |
|冻结/解冻 |监护人票证签名加上理事会覆盖（解冻）。 |

Torii 通过检查来验证证明：

1. 治理账本中存在提案 ID（`/v1/governance/proposals/{id}`），状态为 `Approved`。
2. 哈希值与记录的投票工件相匹配。
3. 管理员/监护人签名引用 `SuffixPolicyV1` 中的预期公钥。

失败的检查返回 `sns_err_governance_missing`。

## 6. 工作流程示例

### 6.1 标准注册

1. 客户端查询 `/v1/sns/policies/{suffix_id}` 以获取定价、宽限和可用等级。
2.客户端构建`RegisterNameRequestV1`：
   - `selector` 源自首选 i105 或次佳压缩 (`sora`) 标签。
   - `term_years` 在政策范围内。
   - `payment` 引用财务/管家分割转移。
3. Torii 验证：
   - 标签标准化+保留列表。
   - 期限/总价与 `PriceTierV1` 的比较。
   - 付款证明金额 >= 计算价格 + 费用。
4.成功后Torii：
   - 持续存在 `NameRecordV1`。
   - 发出 `RegistryEventV1::NameRegistered`。
   - 发出 `RevenueAccrualEventV1`。
   - 返回新记录+事件。

### 6.2 宽限期的更新

宽限续订包括标准请求和惩罚检测：

- Torii 检查 `now` 与 `grace_expires_at` 并添加来自 `SuffixPolicyV1` 的附加费表。
- 付款证明必须包含附加费。失败 => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` 记录新的 `expires_at`。

### 6.3 监护人冻结和议会推翻

1. 监护人提交 `FreezeNameRequestV1` 以及引用事件 ID 的工单。
2. Torii 将记录移动到 `NameStatus::Frozen`，发出 `NameFrozen`。
3.整改后，理事会问题优先；操作员发送 DELETE `/v1/sns/names/{namespace}/{literal}/freeze` 和 `GovernanceHookV1`。
4. Torii 验证覆盖，发出 `NameUnfrozen`。

## 7. 验证和错误代码

|代码|描述 | HTTP |
|------|-------------|------|
| `sns_err_reserved` |标签被保留或屏蔽。 | 409 | 409
| `sns_err_policy_violation` |术语、层级或控制器集违反了政策。 | 422 | 422
| `sns_err_payment_mismatch` |付款证明价值或资产不匹配。 | 402 | 402
| `sns_err_governance_missing` |所需的治理工件不存在/无效。 | 403 | 403
| `sns_err_state_conflict` |当前生命周期状态不允许进行操作。 | 409 | 409

所有代码均通过 `X-Iroha-Error-Code` 和结构化 Norito JSON/NRPC 信封显示。

## 8. 实施注意事项

- Torii 将挂起的拍卖存储在 `NameRecordV1.auction` 下，并在 `PendingAuction` 期间拒绝直接注册尝试。
- 付款证明重复使用Norito分类账收据；财务服务提供帮助程序 API (`/v1/finance/sns/payments`)。
- SDK 应使用强类型帮助程序包装这些端点，以便钱包可以提供明确的错误原因（`ERR_SNS_RESERVED` 等）。

## 9. 后续步骤

- 一旦 SN-3 拍卖落地，将 Torii 处理程序连接到实际的注册合约。
- 发布引用此 API 的 SDK 特定指南 (Rust/JS/Swift)。
- 通过与治理挂钩证据字段的交叉链接扩展 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)。