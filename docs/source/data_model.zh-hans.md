---
lang: zh-hans
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8337416254dfc062c40d691f6b35f7ee5818a1071279142bff75a74b75c0a802
source_last_modified: "2026-03-27T19:05:03.382221+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

# Iroha v2 数据模型 – 深入探讨

本文档解释了构成 Iroha v2 数据模型的结构、标识符、特征和协议，这些模型在 `iroha_data_model` 包中实现并在工作区中使用。它旨在成为您可以查看并提出更新建议的精确参考。

## 范围和基础

- 目的：为域对象（域、账户、资产、NFT、角色、权限、对等点）、状态更改指令（ISI）、查询、触发器、交易、区块和参数提供规范类型。
- 序列化：所有公共类型派生 Norito 编解码器 (`norito::codec::{Encode, Decode}`) 和架构 (`iroha_schema::IntoSchema`)。 JSON 在功能标志后面有选择地使用（例如，用于 HTTP 和 `Json` 有效负载）。
- IVM 注意：当以 Iroha 虚拟机 (IVM) 为目标时，某些反序列化时验证被禁用，因为主机在调用合约之前执行验证（请参阅 `src/lib.rs` 中的 crate 文档）。
- FFI 门：某些类型通过 `ffi_export`/`ffi_import` 后面的 `iroha_ffi` 有条件地注释 FFI，以避免不需要 FFI 时的开销。

## 核心特征和助手- `Identifiable`：实体具有稳定的 `Id` 和 `fn id(&self) -> &Self::Id`。应使用 `IdEqOrdHash` 导出，以实现地图/设置的友好性。
- `Registrable`/`Registered`：许多实体（例如，`Domain`、`AssetDefinition`、`Role`）使用构建器模式。 `Registered` 将运行时类型与适合注册事务的轻量级构建器类型 (`With`) 联系起来。
- `HasMetadata`：统一访问键/值 `Metadata` 映射。
- `IntoKeyValue`：存储分割助手，分别存储 `Key`（ID）和 `Value`（数据）以减少重复。
- `Owned<T>`/`Ref<'world, K, V>`：在存储和查询过滤器中使用轻量级包装器以避免不必要的复制。

## 名称和标识符- `Name`：有效的文本标识符。不允许空格和保留字符 `@`、`#`、`$`（在复合 ID 中使用）。可通过 `FromStr` 进行构造并进行验证。名称在解析时标准化为 Unicode NFC（规范等效的拼写被视为相同并存储组合）。特殊名称 `genesis` 被保留（检查不区分大小写）。
- `IdBox`：任何支持的 ID 的求和型信封（`DomainId`、`AccountId`、`AssetDefinitionId`、`AssetId`、`NftId`、`PeerId`、 `TriggerId`、`RoleId`、`Permission`、`CustomParameterId`）。对于通用流和 Norito 编码作为单一类型很有用。
- `ChainId`：用于交易中重放保护的不透明链标识符。ID 的字符串形式（可与 `Display`/`FromStr` 进行往返）：
- `DomainId`：`name`（例如，`wonderland`）。
- `AccountId`：仅通过 `AccountAddress` 编码为 I105 的规范无域帐户标识符。严格的解析器输入必须是规范的 I105；域后缀 (`@domain`)、帐户别名文字、规范十六进制解析器输入、旧版 `norito:` 有效负载和 `uaid:`/`opaque:` 帐户解析器形式将被拒绝。链上账户别名使用 `name@domain.dataspace` 或 `name@dataspace` 并解析为规范的 `AccountId` 值。
- `AssetDefinitionId`：规范资产定义字节上的规范无前缀 Base58 地址。这是公共资产 ID。链上资产别名使用 `name#domain.dataspace` 或 `name#dataspace` 并仅解析为此规范的 Base58 资产 ID。
- `AssetId`：规范裸 Base58 形式的公共资产标识符。 `name#dataspace` 或 `name#domain.dataspace` 等资产别名解析为 `AssetId`。内部账本持有量可能会在需要时另外公开拆分的 `asset + account + optional dataspace` 字段，但该复合形状不是公共 `AssetId`。
- `NftId`：`nft$domain`（例如，`rose$garden`）。
- `PeerId`：`public_key`（对等平等由公钥决定）。

## 实体

### 域名
- `DomainId { name: Name }` – 唯一名称。
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`。
- 生成器：`NewDomain` 与 `with_logo`、`with_metadata`，然后 `Registrable::build(authority)` 设置 `owned_by`。

### 账户
- `AccountId` 是由控制器键入并编码为规范 I105 的规范无域帐户身份。
- `ScopedAccountId { account: AccountId, domain: DomainId }` 仅在需要范围视图时才携带显式域上下文。
- `Account { id, metadata, label?, uaid?, linked_domains? }` — `label` 是密钥更新记录使用的可选稳定别名，`uaid` 携带可选的 Nexus 范围 [通用帐户 ID](./universal_accounts_guide.md)，`linked_domains` 是派生索引状态而不是索引状态的一部分规范身份。
- 建设者：
  - `NewAccount` 通过 `Account::new(scoped_id)` 实现显式域链接注册，因此需要 `ScopedAccountId`。
  - `NewAccount` 通过 `Account::new_domainless(id)` 仅注册没有链接域的通用帐户主题。
- 别名模型：
  - 规范帐户身份从不包含域或数据空间段。
  - 帐户别名是位于 `AccountId` 之上的单独 SNS/帐户标签绑定。
  - 域限定别名（例如 `merchant@hbl.sbp`）在别名绑定中同时携带域和数据空间。
  - 数据空间根别名（例如 `merchant@sbp`）仅包含数据空间，因此与 `Account::new_domainless(...)` 自然配对。
  - 测试和固定装置应首先播种通用 `AccountId`，然后分别添加域链接、别名租用和别名权限，而不是将域假设编码到帐户身份本身中。

### 资产定义和资产
- `AssetDefinitionId { aid_bytes: [u8; 16] }` 以文本形式公开为带有版本控制和校验和的无前缀 Base58 地址。
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`。
  - `name` 是必需的人性化显示文本，并且不得包含 `#`/`@`。
  - `alias` 是可选的，并且必须是以下之一：
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    左侧段与 `AssetDefinition.name` 完全匹配。
  - 别名租用状态权威地存储在持久化别名绑定记录中；当通过 core/Torii API 读回定义时，会派生内联 `alias` 字段。
  - Torii 资产定义响应可以包括 `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`，其中 `status` 是 `permanent`、`leased_active`、`leased_grace` 或 `expired_pending_cleanup` 之一。
  - 别名解析使用最新提交的块时间戳而不是节点挂钟。一旦 `grace_until_ms` 过去，别名选择器将立即停止解析，即使清除清理尚未删除过时的绑定；直接定义读取仍可能将延迟绑定报告为 `expired_pending_cleanup`。
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`。
  - 建造者：`AssetDefinition::new(id, spec)` 或便利 `numeric(id)`； `name` 是必需的，并且必须通过 `.with_name(...)` 设置。
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`。
- `Asset { id, value: Numeric }` 与存储友好型 `AssetEntry`/`AssetValue`。- `AssetBalanceScope`：`Global` 用于无限制余额，`Dataspace(DataSpaceId)` 用于数据空间受限余额。
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` 公开用于摘要 API。

#

## NFT
- `NftId { domain: DomainId, name: Name }`。
- `Nft { id, content: Metadata, owned_by: AccountId }`（内容是任意键/值元数据）。
- 生成器：`NewNft` 通过 `Nft::new(id, content)`。

#

## 角色和权限
- `RoleId { name: Name }`。
- `Role { id, permissions: BTreeSet<Permission> }` 与构建器 `NewRole { inner: Role, grant_to: AccountId }`。
- `Permission { name: Ident, payload: Json }` – `name` 和有效负载模式必须与活动的 `ExecutorDataModel` 一致（见下文）。

#

## 同行
- `PeerId { public_key: PublicKey }`。
- `Peer { address: SocketAddr, id: PeerId }` 和可解析的 `public_key@address` 字符串形式。

#

## 加密原语（功能 `sm`）
- `Sm2PublicKey` 和 `Sm2Signature`：SM2 的 SEC1 兼容点和固定宽度 `r∥s` 签名。构造函数验证曲线成员资格和区分 ID； Norito 编码镜像 `iroha_crypto` 使用的规范表示。
- `Sm3Hash`：`[u8; 32]` 代表 GM/T 0004 摘要的新类型，用于清单、遥测和系统调用响应。
- `Sm4Key`：主机系统调用和数据模型装置之间共享的 128 位对称密钥包装器。
这些类型与现有的 Ed25519/BLS/ML-DSA 原语并存，一旦使用 `--features sm` 构建工作区，它们就会成为公共模式的一部分。

### 触发器和事件
- `TriggerId { name: Name }` 和 `Trigger { id, action: action::Action }`。
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`。
  - `Repeats`：`Indefinitely` 或 `Exactly(u32)`；包括排序和消耗实用程序。
  - 安全：`TriggerCompleted` 不能用作操作的过滤器（在（反）序列化期间验证）。
- `EventBox`：管道、管道批、数据、时间、执行触发和触发完成事件的总和类型； `EventFilterBox` 镜像订阅和触发过滤器。

## 参数及配置

- 系统参数系列（所有 `Default`ed，携带吸气剂，并转换为单独的枚举）：
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`。
  - `BlockParameters { max_transactions: NonZeroU64 }`。
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`。
  - `SmartContractParameters { fuel, memory, execution_depth }`。
- `Parameters` 将所有系列和 `custom: BTreeMap<CustomParameterId, CustomParameter>` 分组。
- 单参数枚举：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter` 用于类似 diff 的更新和迭代。
- 自定义参数：执行器定义，携带为`Json`，由`CustomParameterId`（a `Name`）标识。

## ISI（Iroha 特别说明）- 核心特征：`Instruction` 与 `dyn_encode`、`as_any` 和稳定的每类型标识符 `id()`（默认为具体类型名称）。所有指令均为 `Send + Sync + 'static`。
- `InstructionBox`：拥有 `Box<dyn Instruction>` 包装器，通过类型 ID + 编码字节实现克隆/eq/ord。
- 内置指令系列的组织方式如下：
  - `mint_burn`、`transfer`、`register` 和 `transparent` 帮助程序包。
  - 元流的类型枚举：`InstructionType`，盒装总和，如 `SetKeyValueBox`（域/帐户/asset_def/nft/trigger）。
- 错误：`isi::error` 下的丰富错误模型（评估类型错误、查找错误、可铸造性、数学、无效参数、重复、不变量）。
- 指令注册表：`instruction_registry!{ ... }` 宏构建一个按类型名称键入的运行时解码注册表。由 `InstructionBox` 克隆和 Norito serde 使用来实现动态（反）序列化。如果没有通过 `set_instruction_registry(...)` 显式设置注册表，则在首次使用时会延迟安装包含所有核心 ISI 的内置默认注册表，以保持二进制文件的稳健性。

## 交易- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`。 `IvmBytecode` 序列化为 base64（`Vec<u8>` 上的透明新类型）。
- `TransactionBuilder`：使用 `chain`、`authority`、`creation_time_ms`、可选 `time_to_live_ms` 和 `nonce`、`metadata` 和`Executable`。
  - 帮助程序：`with_instructions`、`with_bytecode`、`with_executable`、`with_metadata`、`set_nonce`、`set_ttl`、`set_creation_time`、`sign`。
- `SignedTransaction`（版本为`iroha_version`）：携带`TransactionSignature`和有效负载；提供哈希和签名验证。
- 切入点和结果：
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`。
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` 带有哈希助手。
  - `ExecutionStep(ConstVec<InstructionBox>)`：事务中的单个有序批次指令。

## 块- `SignedBlock`（版本化）封装：
  - `signatures: BTreeSet<BlockSignature>`（来自验证器），
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`，
  - `result: BlockResult`（辅助执行状态），包含 `time_triggers`、条目/结果 Merkle 树、`transaction_results` 和 `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`。
- 实用程序：`presigned`、`set_transaction_results(...)`、`set_transaction_results_with_transcripts(...)`、`header()`、`signatures()`、`hash()`、`add_signature`、`replace_signatures`。
- Merkle 根：交易入口点和结果通过 Merkle 树提交；结果 Merkle root 被放入区块头中。
- 区块包含证明 (`BlockProofs`) 公开条目/结果 Merkle 证明和 `fastpq_transcripts` 映射，以便链外证明者可以获取与交易哈希相关的传输增量。
- `ExecWitness` 消息（通过 Torii 流式传输并搭载共识八卦）现在包括 `fastpq_transcripts` 和带有嵌入式 `public_inputs`（dsid、slot、roots、perm_root、 tx_set_hash），因此外部证明者可以摄取规范的 FASTPQ 行，而无需重新编码转录本。

## 查询- 两种口味：
  - 单数：实施 `SingularQuery<Output>`（例如，`FindParameters`、`FindExecutorDataModel`）。
  - 可迭代：实现 `Query<Item>`（例如，`FindAccounts`、`FindAssets`、`FindDomains` 等）。
- 类型擦除表格：
  - `QueryBox<T>` 是一个盒装的、已擦除的 `Query<Item = T>`，带有由全局注册表支持的 Norito serde。
  - `QueryWithFilter<T> { query, predicate, selector }` 将查询与 DSL 谓词/选择器配对；通过 `From` 转换为已擦除的可迭代查询。
- 注册表和编解码器：
  - `query_registry!{ ... }` 构建一个全局注册表，按类型名称将具体查询类型映射到构造函数以进行动态解码。
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` 和 `QueryResponse = Singular(..) | Iterable(QueryOutput)`。
  - `QueryOutputBatchBox` 是同质向量的求和类型（例如，`Vec<Account>`、`Vec<Name>`、`Vec<AssetDefinition>`、`Vec<BlockHeader>`），以及用于高效分页的元组和扩展助手。
- DSL：在 `query::dsl` 中实现，具有用于编译时检查谓词和选择器的投影特征 (`HasProjection<PredicateMarker>` / `SelectorMarker`)。如果需要，`fast_dsl` 功能会公开更轻的变体。

## 执行器和可扩展性- `Executor { bytecode: IvmBytecode }`：验证器执行的代码包。
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` 声明执行者定义的域：
  - 自定义配置参数，
  - 自定义指令标识符，
  - 权限令牌标识符，
  - 描述客户端工具自定义类型的 JSON 模式。
- `data_model/samples/executor_custom_data_model` 下存在定制示例，演示：
  - 通过 `iroha_executor_data_model::permission::Permission` 派生自定义权限令牌，
  - 自定义参数定义为可转换为 `CustomParameter` 的类型，
  - 自定义指令序列化到 `CustomInstruction` 中以供执行。

#

## CustomInstruction（执行者定义的ISI）- 类型：`isi::CustomInstruction { payload: Json }`，具有稳定的线 ID `"iroha.custom"`。
- 目的：在私有/联盟网络中执行程序特定指令或用于原型设计的信封，无需分叉公共数据模型。
- 默认执行器行为：`iroha_core` 中的内置执行器不执行 `CustomInstruction`，如果遇到会出现恐慌。自定义执行器必须将 `InstructionBox` 向下转换为 `CustomInstruction` 并确定性地解释所有验证器上的有效负载。
- Norito：通过 `norito::codec::{Encode, Decode}` 进行编码/解码，包含模式； `Json` 有效负载是确定性序列化的。只要指令注册表包含 `CustomInstruction`（它是默认注册表的一部分），往返就是稳定的。
- IVM：Kotodama 编译为 IVM 字节码 (`.to`)，是应用程序逻辑的推荐路径。仅将 `CustomInstruction` 用于尚无法用 Kotodama 表示的执行程序级扩展。确保同行之间的确定性和相同的执行器二进制文件。
- 不适用于公共网络：不要用于异构执行者存在共识分叉风险的公共链。当您需要平台功能时，更愿意建议新的内置 ISI 上游。

## 元数据- `Metadata(BTreeMap<Name, Json>)`：附加到多个实体的键/值存储（`Domain`、`Account`、`AssetDefinition`、`Nft`、触发器和事务）。
- API：`contains`、`iter`、`get`、`insert` 和（使用 `transparent_api`）`remove`。

## 特征和确定性

- 功能控制可选 API（`std`、`json`、`transparent_api`、`ffi_export`、`ffi_import`、`fast_dsl`、`http`、 `fault_injection`）。
- 确定性：所有序列化都使用 Norito 编码，以便跨硬件移植。 IVM 字节码是不透明的字节 blob；执行不得引入非确定性缩减。主机验证事务并向 IVM 确定性地提供输入。

#

## 透明 API (`transparent_api`)- 目的：公开对 `#[model]` 结构/枚举的完整、可变访问，以用于内部组件（例如 Torii、执行器和集成测试）。如果没有它，这些项目就会故意不透明，因此外部 SDK 只能看到安全的构造函数和编码的有效负载。
- 机制：`iroha_data_model_derive::model` 宏使用 `#[cfg(feature = "transparent_api")] pub` 重写每个公共字段，并为默认构建保留一个私有副本。启用该功能会翻转这些 cfgs，因此解构 `Account`、`Domain`、`Asset` 等在其定义模块之外变得合法。
- 表面检测：包导出 `TRANSPARENT_API: bool` 常量（生成为 `transparent_api.rs` 或 `non_transparent_api.rs`）。下游代码可以在需要回退到不透​​明助手时检查此标志和分支。
- 启用：将 `features = ["transparent_api"]` 添加到 `Cargo.toml` 的依赖项中。需要 JSON 投影的工作区 crate（例如 `iroha_torii`）会自动转发该标志，但第三方使用者应将其关闭，除非他们控制部署并接受更广泛的 API 表面。

## 简单示例

创建域和帐户、定义资产并按照说明构建交易：

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id = AssetDefinitionId::new(
    "wonderland".parse().unwrap(),
    "usd".parse().unwrap(),
);
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

通过DSL查询账户和资产：

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

使用 IVM 智能合约字节码：

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

资产定义 ID/别名快速参考 (CLI + Torii)：

```bash
# Register an asset definition with a canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to the canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```迁移注意事项：
- v1 中不接受旧的 `name#domain` 资产定义 ID。
- 公共资产选择器仅使用一种资产定义格式：规范的 Base58 id。别名仍然是可选选择器，但解析为相同的规范 ID。
- 公共资产查找通过 `asset + account + optional scope` 解决自有余额；原始编码的 `AssetId` 文字是内部表示，不是 Torii/CLI 选择器表面的一部分。
- 除了 `id` 之外，`POST /v1/assets/definitions/query` 和 `GET /v1/assets/definitions` 还接受 `alias_binding.status`、`alias_binding.lease_expiry_ms`、`alias_binding.grace_until_ms` 和 `alias_binding.bound_at_ms` 上的资产定义过滤器/排序， `name`、`alias` 和 `metadata.*`。

## 版本控制

- `SignedTransaction`、`SignedBlock` 和 `SignedQuery` 是规范的 Norito 编码结构。当通过 `EncodeVersioned` 编码时，每个都实现 `iroha_version::Version`，以当前 ABI 版本（当前为 `1`）作为其有效负载的前缀。

## 评论注释/潜在更新

- 查询 DSL：考虑记录一个稳定的面向用户的子集以及常见过滤器/选择器的示例。
- 指令系列：展开公共文档，列出 `mint_burn`、`register`、`transfer` 公开的内置 ISI 变体。

---
如果任何部分需要更多深度（例如，完整的 ISI 目录、完整的查询注册表列表或块头字段），请告诉我，我将相应地扩展这些部分。