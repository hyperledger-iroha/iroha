---
lang: zh-hans
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 数据模型和 ISI — 实现衍生规范

该规范是根据 `iroha_data_model` 和 `iroha_core` 的当前实现进行逆向工程的，以帮助设计审查。反引号中的路径指向权威代码。

## 范围
- 定义规范实体（域、帐户、资产、NFT、角色、权限、对等点、触发器）及其标识符。
- 描述状态更改指令 (ISI)：类型、参数、前提条件、状态转换、发出的事件和错误条件。
- 总结参数管理、事务和指令序列化。

确定性：所有指令语义都是纯粹的状态转换，没有依赖于硬件的行为。序列化使用Norito； VM 字节码使用 IVM，并在链上执行之前经过主机端验证。

---

## 实体和标识符
ID 具有稳定的字符串形式，可进行 `Display`/`FromStr` 往返。名称规则禁止空格和保留的 `@ # $` 字符。- `Name` — 经过验证的文本标识符。规则：`crates/iroha_data_model/src/name.rs`。
- `DomainId` — `name`。域名：`{ id, logo, metadata, owned_by }`。建造者：`NewDomain`。代码：`crates/iroha_data_model/src/domain.rs`。
- `AccountId` — 规范地址通过 `AccountAddress`（IH58 / `sora…` 压缩/十六进制）生成，Torii 通过 `AccountAddress::parse_encoded` 标准化输入。 IH58是首选账户格式； `sora…` 形式对于仅 Sora 的 UX 来说是第二好的。熟悉的 `alias@domain` 字符串仅保留作为路由别名。帐号：`{ id, metadata }`。代码：`crates/iroha_data_model/src/account.rs`。
- 帐户准入策略 — 域通过在元数据密钥 `iroha:account_admission_policy` 下存储 Norito-JSON `AccountAdmissionPolicy` 来控制隐式帐户创建。当密钥不存在时，链级自定义参数`iroha:default_account_admission_policy`提供默认值；当它也不存在时，硬默认值是 `ImplicitReceive`（第一个版本）。策略标签 `mode`（`ExplicitOnly` 或 `ImplicitReceive`）加上可选的每笔交易（默认 `16`）和每块创建上限、可选的 `implicit_creation_fee`（销毁或接收帐户）、每个资产定义的 `min_initial_amounts` 和可选的`default_role_on_create`（在 `AccountCreated` 之后授予，如果缺失则以 `DefaultRoleError` 拒绝）。 Genesis 无法选择加入；禁用/无效策略拒绝针对 `InstructionExecutionError::AccountAdmission` 的未知账户的收据式指令。隐式帐户在 `AccountCreated` 之前标记元数据 `iroha:created_via="implicit"`；默认角色发出后续 `AccountRoleGranted`，执行者所有者基线规则让新账户无需额外角色即可使用自己的资产/NFT。代码：`crates/iroha_data_model/src/account/admission.rs`、`crates/iroha_core/src/smartcontracts/isi/account_admission.rs`。
- `AssetDefinitionId` — `asset#domain`。定义：`{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`。代码：`crates/iroha_data_model/src/asset/definition.rs`。
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`。 NFT：`{ id, content: Metadata, owned_by }`。代码：`crates/iroha_data_model/src/nft.rs`。
- `RoleId` — `name`。角色：`{ id, permissions: BTreeSet<Permission> }` 和构建器 `NewRole { inner: Role, grant_to }`。代码：`crates/iroha_data_model/src/role.rs`。
- `Permission` — `{ name: Ident, payload: Json }`。代码：`crates/iroha_data_model/src/permission.rs`。
- `PeerId`/`Peer` — 对等身份（公钥）和地址。代码：`crates/iroha_data_model/src/peer.rs`。
- `TriggerId` — `name`。触发器：`{ id, action }`。行动：`{ executable, repeats, authority, filter, metadata }`。代码：`crates/iroha_data_model/src/trigger/`。
- `Metadata` — `BTreeMap<Name, Json>` 已检查插入/删除。代码：`crates/iroha_data_model/src/metadata.rs`。
- 订阅模式（应用层）：计划是带有 `subscription_plan` 元数据的 `AssetDefinition` 条目；订阅是带有 `subscription` 元数据的 `Nft` 记录；计费由引用订阅 NFT 的时间触发器执行。请参阅 `docs/source/subscriptions_api.md` 和 `crates/iroha_data_model/src/subscription.rs`。
- **加密原语**（功能 `sm`）：- `Sm2PublicKey` / `Sm2Signature` 镜像 SM2 的规范 SEC1 点 + 固定宽度 `r∥s` 编码。构造函数强制执行曲线成员资格和区分 ID 语义 (`DEFAULT_DISTID`)，而验证则拒绝格式错误或高范围标量。代码：`crates/iroha_crypto/src/sm.rs` 和 `crates/iroha_data_model/src/crypto/mod.rs`。
  - `Sm3Hash` 将 GM/T 0004 摘要公开为 Norito 可序列化的 `[u8; 32]` 新类型，无论哈希出现在清单或遥测中，都使用该新类型。代码：`crates/iroha_data_model/src/crypto/hash.rs`。
  - `Sm4Key` 表示 128 位 SM4 密钥，并在主机系统调用和数据模型装置之间共享。代码：`crates/iroha_data_model/src/crypto/symmetric.rs`。
  这些类型与现有的 Ed25519/BLS/ML-DSA 原语并存，一旦启用 `sm` 功能，数据模型使用者（Torii、SDK、创世工具）就可以使用这些类型。

重要特征：`Identifiable`、`Registered`/`Registrable`（构建器模式）、`HasMetadata`、`IntoKeyValue`。代码：`crates/iroha_data_model/src/lib.rs`。

事件：每个实体都有在突变时发出的事件（创建/删除/所有者更改/元数据更改等）。代码：`crates/iroha_data_model/src/events/`。

---

## 参数（链配置）
- 系列：`SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`、`BlockParameters { max_transactions }`、`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`、`SmartContractParameters { fuel, memory, execution_depth }` 以及 `custom: BTreeMap`。
- 差异的单个枚举：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter`。聚合器：`Parameters`。代码：`crates/iroha_data_model/src/parameter/system.rs`。

设置参数（ISI）：`SetParameter(Parameter)` 更新相应字段并发出 `ConfigurationEvent::Changed`。代码：`crates/iroha_data_model/src/isi/transparent.rs`，执行者在`crates/iroha_core/src/smartcontracts/isi/world.rs`。

---

## 指令序列化和注册
- 核心特征：`Instruction: Send + Sync + 'static` 与 `dyn_encode()`、`as_any()`、稳定 `id()`（默认为具体类型名称）。
- `InstructionBox`：`Box<dyn Instruction>` 包装器。 Clone/Eq/Ord 在 `(type_id, encoded_bytes)` 上运行，因此相等是按值计算的。
- `InstructionBox` 的 Norito Serde 序列化为 `(String wire_id, Vec<u8> payload)`（如果没有线 ID，则回退到 `type_name`）。反序列化使用全局 `InstructionRegistry` 将标识符映射到构造函数。默认注册表包括所有内置 ISI。代码：`crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`。

---

## ISI：类型、语义、错误
执行是通过 `iroha_core::smartcontracts::isi` 中的 `Execute for <Instruction>` 实现的。下面列出了公共效果、先决条件、发出的事件和错误。

### 注册/取消注册
类型：`Register<T: Registered>` 和 `Unregister<T: Identifiable>`，总和类型 `RegisterBox`/`UnregisterBox` 覆盖具体目标。

- 注册对等点：插入到世界对等点集中。
  - 前提条件：必须不存在。
  - 事件：`PeerEvent::Added`。
  - 错误：`Repetition(Register, PeerId)`（如果重复）；查找时为 `FindError`。代码：`core/.../isi/world.rs`。

- 注册域：从 `NewDomain` 和 `owned_by = authority` 构建。不允许：`genesis` 域。
  - 前提条件：域名不存在；不是 `genesis`。
  - 事件：`DomainEvent::Created`。
  - 错误：`Repetition(Register, DomainId)`、`InvariantViolation("Not allowed to register genesis domain")`。代码：`core/.../isi/world.rs`。- 注册帐户：从 `NewAccount` 构建，在 `genesis` 域中不允许； `genesis` 账户无法注册。
  - 前提条件：域名必须存在；账户不存在；不在创世域中。
  - 事件：`DomainEvent::Account(AccountEvent::Created)`。
  - 错误：`Repetition(Register, AccountId)`、`InvariantViolation("Not allowed to register account in genesis domain")`。代码：`core/.../isi/domain.rs`。

- 注册AssetDefinition：从构建器构建；设置 `owned_by = authority`。
  - 前提条件：定义不存在；域存在。
  - 事件：`DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`。
  - 错误：`Repetition(Register, AssetDefinitionId)`。代码：`core/.../isi/domain.rs`。

- 注册NFT：由构建者构建；设置 `owned_by = authority`。
  - 前提条件：NFT 不存在；域存在。
  - 事件：`DomainEvent::Nft(NftEvent::Created)`。
  - 错误：`Repetition(Register, NftId)`。代码：`core/.../isi/nft.rs`。

- 注册角色：从 `NewRole { inner, grant_to }`（通过帐户角色映射记录的第一个所有者）构建，存储 `inner: Role`。
  - 前提条件：角色不存在。
  - 事件：`RoleEvent::Created`。
  - 错误：`Repetition(Register, RoleId)`。代码：`core/.../isi/world.rs`。

- 注册触发器：将触发器存储在按过滤器类型设置的适当触发器中。
  - 前提条件：如果过滤器不可铸造，则 `action.repeats` 必须是 `Exactly(1)`（否则为 `MathError::Overflow`）。禁止重复 ID。
  - 事件：`TriggerEvent::Created(TriggerId)`。
  - 错误：转换/验证失败时出现 `Repetition(Register, TriggerId)`、`InvalidParameterError::SmartContract(..)`。代码：`core/.../isi/triggers/mod.rs`。

- 注销Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger：删除目标；发出删除事件。额外的级联删除：
  - 取消注册域：删除域中的所有帐户及其角色、权限、发送序列计数器、帐户标签和 UAID 绑定；删除其资产（以及每个资产的元数据）；删除域中的所有资产定义；删除域中的 NFT 以及已删除帐户拥有的任何 NFT；删除权限域匹配的触发器。事件：`DomainEvent::Deleted`，加上每个项目的删除事件。错误：`FindError::Domain`（如果缺失）。代码：`core/.../isi/world.rs`。
  - 注销账户：删除账户的权限、角色、交易序列计数器、账户标签映射和 UAID 绑定；删除帐户拥有的资产（以及每个资产的元数据）；删除该账户拥有的 NFT；删除权限为该帐户的触发器。事件：`AccountEvent::Deleted`，加上每个删除的 NFT 的 `NftEvent::Deleted`。错误：`FindError::Account`（如果缺失）。代码：`core/.../isi/domain.rs`。
  - 取消注册资产定义：删除该定义的所有资产及其每个资产的元数据。事件：每个资产的 `AssetDefinitionEvent::Deleted` 和 `AssetEvent::Deleted`。错误：`FindError::AssetDefinition`。代码：`core/.../isi/domain.rs`。
  - 注销NFT：删除NFT。事件：`NftEvent::Deleted`。错误：`FindError::Nft`。代码：`core/.../isi/nft.rs`。
  - 注销角色：先从所有账户中注销该角色；然后删除该角色。事件：`RoleEvent::Deleted`。错误：`FindError::Role`。代码：`core/.../isi/world.rs`。
  - 取消注册触发器：删除触发器（如果存在）；重复注销会产生 `Repetition(Unregister, TriggerId)`。事件：`TriggerEvent::Deleted`。代码：`core/.../isi/triggers/mod.rs`。

### 薄荷/燃烧
类型：`Mint<O, D: Identifiable>` 和 `Burn<O, D: Identifiable>`，盒装为 `MintBox`/`BurnBox`。- 资产（数字）铸造/销毁：调整余额和定义的 `total_quantity`。
  - 前提条件：`Numeric`值必须满足`AssetDefinition.spec()`； `mintable` 允许的薄荷：
    - `Infinitely`：始终允许。
    - `Once`：仅允许一次；第一个铸币厂将 `mintable` 翻转为 `Not` 并发出 `AssetDefinitionEvent::MintabilityChanged`，以及用于可审计的详细 `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`。
    - `Limited(n)`：允许 `n` 额外的铸造操作。每个成功的铸币厂都会减少计数器；当它达到零时，定义翻转到 `Not` 并发出与上面相同的 `MintabilityChanged` 事件。
    - `Not`：错误 `MintabilityError::MintUnmintable`。
  - 状态变化：如果铸币厂丢失则创建资产；如果销毁时余额变为零，则删除资产条目。
  - 事件：`AssetEvent::Added`/`AssetEvent::Removed`、`AssetDefinitionEvent::MintabilityChanged`（当 `Once` 或 `Limited(n)` 耗尽其配额时）。
  - 错误：`TypeError::AssetNumericSpec(Mismatch)`、`MathError::Overflow`/`NotEnoughQuantity`。代码：`core/.../isi/asset.rs`。

- 触发重复薄荷/燃烧：更改触发的 `action.repeats` 计数。
  - 前提条件：在薄荷上，过滤器必须是可薄荷的；算术不能溢出/下溢。
  - 事件：`TriggerEvent::Extended`/`TriggerEvent::Shortened`。
  - 错误：`MathError::Overflow` 无效铸币； `FindError::Trigger`（如果丢失）。代码：`core/.../isi/triggers/mod.rs`。

### 转移
类型：`Transfer<S: Identifiable, O, D: Identifiable>`，盒装为 `TransferBox`。

- 资产（数字）：从源 `AssetId` 中减去，添加到目标 `AssetId`（相同定义，不同帐户）。删除归零的源资产。
  - 前提条件：源资产存在；值满足 `spec`。
  - 事件：`AssetEvent::Removed`（源）、`AssetEvent::Added`（目标）。
  - 错误：`FindError::Asset`、`TypeError::AssetNumericSpec`、`MathError::NotEnoughQuantity/Overflow`。代码：`core/.../isi/asset.rs`。

- 域所有权：将 `Domain.owned_by` 更改为目标帐户。
  - 前提条件：两个账户都存在；域存在。
  - 事件：`DomainEvent::OwnerChanged`。
  - 错误：`FindError::Account/Domain`。代码：`core/.../isi/domain.rs`。

- AssetDefinition 所有权：将 `AssetDefinition.owned_by` 更改为目标账户。
  - 前提条件：两个账户都存在；定义存在；源当前必须拥有它。
  - 事件：`AssetDefinitionEvent::OwnerChanged`。
  - 错误：`FindError::Account/AssetDefinition`。代码：`core/.../isi/account.rs`。

- NFT 所有权：将 `Nft.owned_by` 更改为目标账户。
  - 前提条件：两个账户都存在； NFT 存在；源当前必须拥有它。
  - 事件：`NftEvent::OwnerChanged`。
  - 错误：`FindError::Account/Nft`、`InvariantViolation`（如果来源不拥有 NFT）。代码：`core/.../isi/nft.rs`。

### 元数据：设置/删除键值
类型：`SetKeyValue<T>` 和 `RemoveKeyValue<T>` 与 `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`。提供盒装枚举。

- 设置：插入或替换 `Metadata[key] = Json(value)`。
- 移除：移除钥匙；如果丢失则出错。
- 事件：`<Target>Event::MetadataInserted` / `MetadataRemoved` 以及旧/新值。
- 错误：如果目标不存在，则为 `FindError::<Target>`； `FindError::MetadataKey` 缺少用于移除的钥匙。代码：`crates/iroha_data_model/src/isi/transparent.rs` 和每个目标的执行器实现。### 权限和角色：授予/撤销
类型：`Grant<O, D>` 和 `Revoke<O, D>`，带有 `Permission`/`Role` 与 `Account` 之间的盒装枚举，以及 `Permission` 与 `Role` 之间的盒装枚举。

- 授予帐户权限：添加 `Permission`，除非已经固有。事件：`AccountEvent::PermissionAdded`。错误：`Repetition(Grant, Permission)`（如果重复）。代码：`core/.../isi/account.rs`。
- 撤销帐户的权限：如果存在则删除。事件：`AccountEvent::PermissionRemoved`。错误：`FindError::Permission`（如果不存在）。代码：`core/.../isi/account.rs`。
- 将角色授予帐户：插入 `(account, role)` 映射（如果不存在）。事件：`AccountEvent::RoleGranted`。错误：`Repetition(Grant, RoleId)`。代码：`core/.../isi/account.rs`。
- 从帐户中撤销角色：删除映射（如果存在）。事件：`AccountEvent::RoleRevoked`。错误：`FindError::Role`（如果不存在）。代码：`core/.../isi/account.rs`。
- 授予角色权限：重建角色并添加权限。事件：`RoleEvent::PermissionAdded`。错误：`Repetition(Grant, Permission)`。代码：`core/.../isi/world.rs`。
- 撤销角色的权限：在没有该权限的情况下重建角色。事件：`RoleEvent::PermissionRemoved`。错误：`FindError::Permission`（如果不存在）。代码：`core/.../isi/world.rs`。

### 触发器：执行
类型：`ExecuteTrigger { trigger: TriggerId, args: Json }`。
- 行为：将触发子系统的 `ExecuteTriggerEvent { trigger_id, authority, args }` 排入队列。仅允许调用触发（`ExecuteTrigger` 过滤器）手动执行；过滤器必须匹配，并且调用者必须是触发操作权限或持有该权限的 `CanExecuteTrigger`。当用户提供的执行器处于活动状态时，触发器执行由运行时执行器验证，并消耗事务的执行器燃料预算（基本 `executor.fuel` 加上可选元数据 `additional_fuel`）。
- 错误：如果未注册，则为 `FindError::Trigger`； `InvariantViolation`（如果由非权威机构调用）。代码：`core/.../isi/triggers/mod.rs`（并在 `core/.../smartcontracts/isi/mod.rs` 中进行测试）。

### 升级并登录
- `Upgrade { executor }`：使用提供的 `Executor` 字节码迁移执行器，更新执行器及其数据模型，发出 `ExecutorEvent::Upgraded`。错误：迁移失败时包装为 `InvalidParameterError::SmartContract`。代码：`core/.../isi/world.rs`。
- `Log { level, msg }`：发出给定级别的节点日志；没有状态改变。代码：`core/.../isi/world.rs`。

### 错误模型
通用信封：`InstructionExecutionError`，具有评估错误、查询失败、转换、未找到实体、重复、可铸造性、数学、无效参数和不变违规等变体。枚举和帮助程序位于 `crates/iroha_data_model/src/isi/mod.rs` 下的 `pub mod error` 中。

---## 事务和可执行文件
- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`；字节码序列化为 base64。代码：`crates/iroha_data_model/src/transaction/executable.rs`。
- `TransactionBuilder`/`SignedTransaction`：使用元数据、`chain_id`、`authority`、`creation_time_ms`、可选的 `ttl_ms` 和 `nonce` 构造、签署和打包可执行文件。代码：`crates/iroha_data_model/src/transaction/`。
- 在运行时，`iroha_core` 通过 `Execute for InstructionBox` 执行 `InstructionBox` 批次，向下转换为适当的 `*Box` 或具体指令。代码：`crates/iroha_core/src/smartcontracts/isi/mod.rs`。
- 运行时执行器验证预算（用户提供的执行器）：来自参数的基础 `executor.fuel` 加上可选的事务元数据 `additional_fuel` (`u64`)，在事务内的指令/触发器验证之间共享。

---

## 不变量和注释（来自测试和防护）
- 创世保护：无法注册 `genesis` 域或 `genesis` 域中的帐户； `genesis` 账户无法注册。代码/测试：`core/.../isi/world.rs`、`core/.../smartcontracts/isi/mod.rs`。
- 数字资产在铸造/转移/销毁时必须满足 `NumericSpec` 要求；规格不匹配产生 `TypeError::AssetNumericSpec`。
- 可铸造性：`Once` 允许单次铸造，然后翻转至 `Not`；在翻转到 `Not` 之前，`Limited(n)` 正好允许 `n` 铸币。尝试禁止在 `Infinitely` 上铸造会导致 `MintabilityError::ForbidMintOnMintable`，配置 `Limited(0)` 会产生 `MintabilityError::InvalidMintabilityTokens`。
- 元数据操作是关键精确的；删除不存在的密钥是一个错误。
- 触发过滤器可以是不可铸造的；那么 `Register<Trigger>` 只允许 `Exactly(1)` 重复。
- 触发元数据键 `__enabled` (bool) 门执行；缺少默认启用的触发器，并且禁用的触发器会在数据/时间/按调用路径上跳过。
- 确定性：所有算术都使用检查操作；下溢/溢出返回键入的数学错误；零余额会删除资产条目（无隐藏状态）。

---## 实际例子
- 铸造和转让：
  - `Mint::asset_numeric(10, asset_id)` → 如果规格/可铸造性允许，则增加 10；事件：`AssetEvent::Added`。
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 移动 5；删除/添加事件。
- 元数据更新：
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → 更新插入；通过 `RemoveKeyValue::account(...)` 删除。
- 角色/权限管理：
  - `Grant::account_role(role_id, account)`、`Grant::role_permission(perm, role)` 及其 `Revoke` 对应项。
- 触发器生命周期：
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`，带有过滤器暗示的可铸造性检查； `ExecuteTrigger::new(id).with_args(&args)` 必须与配置的权限匹配。
  - 可以通过将元数据键 `__enabled` 设置为 `false` 来禁用触发器（缺少默认启用）；通过 `SetKeyValue::trigger` 或 IVM `set_trigger_enabled` 系统调用进行切换。
  - 加载时修复触发器存储：删除重复的 id、不匹配的 id 以及引用丢失字节码的触发器；重新计算字节码引用计数。
  - 如果触发器的 IVM 字节码在执行时丢失，则触发器将被删除，并且执行将被视为具有失败结果的无操作。
  - 耗尽的触发器立即被移除；如果在执行过程中遇到耗尽的条目，则会将其修剪并视为丢失。
- 参数更新：
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` 更新并发出 `ConfigurationEvent::Changed`。

---

## 可追溯性（选定来源）
 - 数据模型核心：`crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`。
 - ISI 定义和注册表：`crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`。
 - ISI 执行：`crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`。
 - 事件：`crates/iroha_data_model/src/events/**`。
 - 交易：`crates/iroha_data_model/src/transaction/**`。

如果您希望将此规范扩展为呈现的 API/行为表或交叉链接到每个具体事件/错误，请说出这个词，我将扩展它。