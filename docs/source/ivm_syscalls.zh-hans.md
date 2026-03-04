---
lang: zh-hans
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 系统调用 ABI

本文档定义了 IVM 系统调用编号、指针 ABI 调用约定、保留编号范围以及 Kotodama 降低所使用的面向合约的系统调用规范表。它补充了 `ivm.md`（体系结构）和 `kotodama_grammar.md`（语言）。

版本控制
- 可识别的系统调用集取决于字节码标头 `abi_version` 字段。第一个版本仅接受 `abi_version = 1`；其他值在入学时被拒绝。活动 `abi_version` 的未知编号确定性地捕获 `E_SCALL_UNKNOWN`。
- 运行时升级保留 `abi_version = 1` 并且不扩展系统调用或指针 ABI 表面。
- 系统调用 Gas 成本是绑定到字节码标头版本的版本化 Gas Schedule 的一部分。请参阅 `ivm.md`（天然气政策）。

编号范围
- `0x00..=0x1F`：VM 核心/实用程序（调试/退出帮助程序在 `CoreHost` 下可用；其余开发帮助程序仅是模拟主机）。
- `0x20..=0x5F`：Iroha 核心 ISI 桥（在 ABI v1 中稳定）。
- `0x60..=0x7F`：由协议功能门控的扩展 ISI（启用后仍是 ABI v1 的一部分）。
- `0x80..=0xFF`：主机/加密助手和保留插槽；仅接受 ABI v1 允许列表中存在的号码。

耐用帮手 (ABI v1)
- 持久状态帮助程序系统调用（0x50–0x5A：STATE_{GET,SET,DEL}、ENCODE/DECODE_INT、BUILD_PATH_*、JSON/SCHEMA 编码/解码）是 V1 ABI 的一部分，并包含在 `abi_hash` 计算中。
- CoreHost 将 STATE_{GET,SET,DEL} 连接到 WSV 支持的持久智能合约状态；开发/测试主机可以在本地保留，但必须保留相同的系统调用语义。

指针 ABI 调用约定（智能合约系统调用）
- 参数作为原始 `u64` 值或作为指向不可变 Norito TLV 包络的 INPUT 区域的指针放置在寄存器 `r10+` 中（例如，`AccountId`、`AssetDefinitionId`、`Name`、 `Json`、`NftId`）。
- 标量返回值是从主机返回的 `u64`。主机将指针结果写入`r10`。

规范系统调用表（子集）|十六进制 |名称 |参数（在 `r10+` 中）|返回|气体（基础+变量）|笔记|
|------|----------------------------------------|----------------------------------------------------------------------------|------------------------|--------------------------------------------------------|------|
| 0x1A | 0x1A设置帐户详细信息 | `&AccountId`、`&Name`、`&Json` | `u64=0` | `G_set_detail + bytes(val)` |写入帐户详细信息 |
| 0x22 | 0x22 MINT_资产 | `&AccountId`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_mint` |向账户铸造 `amount` 资产 |
| 0x23 | 0x23 BURN_ASSET | 烧毁资产`&AccountId`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_burn` |从帐户中烧毁 `amount` |
| 0x24 | 0x24转让_资产| `&AccountId(from)`、`&AccountId(to)`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` |在账户之间转账 `amount` |
| 0x29 | 0x29 TRANSFER_V1_BATCH_BEGIN | 传输– | `u64=0` | `G_transfer` |开始 FASTPQ 传输批次范围 |
| 0x2A | 0x2A TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` |刷新累积的 FASTPQ 传输批次 |
| 0x2B | 0x2B TRANSFER_V1_BATCH_APPLY | 转移_V1_BATCH_APPLY `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` |在单个系统调用中应用 Norito 编码批处理 |
| 0x25 | 0x25 NFT_MINT_资产 | `&NftId`，`&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` |注册新的 NFT |
| 0x26 | 0x26 NFT_TRANSFER_ASSET | NFT_TRANSFER_ASSET | `&AccountId(from)`、`&NftId`、`&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` |转让 NFT 所有权 |
| 0x27 | 0x27 NFT_SET_METADATA | NFT_SET_METADATA | `&NftId`、`&Json` | `u64=0` | `G_nft_set_metadata` |更新 NFT 元数据 |
| 0x28 | 0x28 NFT_BURN_ASSET | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` |烧毁（销毁）NFT |
| 0xA1 | 0xA1 SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` |可迭代查询运行时间短暂； `QueryRequest::Continue` 被拒绝 |
| 0xA2 | 0xA2创建_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` |帮手;功能门控 || 0xA3 | 0xA3 SET_SMARTCONTRACT_EXECUTION_DEPTH | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` |行政;功能门控 |
| 0xA4 | 0xA4获取权限 | –（主机写入结果）| `&AccountId`| `G_get_auth` |主机将指向当前权限的指针写入 `r10` |
| 0xF7 | 0xF7获取_MERKLE_路径 | `addr:u64`、`out_ptr:u64`、可选 `root_out:u64` | `u64=len` | `G_mpath + len` |写入路径（叶→根）和可选的根字节 |
| 0xFA |获取_MERKLE_COMPACT | `addr:u64`、`out_ptr:u64`、可选 `depth_cap:u64`、可选 `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | 0xFF GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`、`out_ptr:u64`、可选 `depth_cap:u64`、可选 `root_out:u64` | `u64=depth` | `G_mpath + depth` |寄存器承诺的相同紧凑布局|

天然气执法
- CoreHost 使用本机 ISI 计划对 ISI 系统调用收取额外的 Gas 费用； FASTPQ 批量传输按条目收费。
- ZK_VERIFY 系统调用重用机密验证气体计划（基础+证明大小）。
- SMARTCONTRACT_EXECUTE_QUERY 收费基本+每项+每字节；排序会使每个项目的成本成倍增加，而未排序的偏移量会增加每个项目的损失。

注释
- 所有指针参数都引用 INPUT 区域中的 Norito TLV 信封，并在第一次取消引用时进行验证（出错时为 `E_NORITO_INVALID`）。
- 所有突变均通过 Iroha 的标准执行器（通过 `CoreHost`）应用，而不是直接由 VM 应用。
- 精确的气体常数 (`G_*`) 由活动气体表定义；参见 `ivm.md`。

错误
- `E_SCALL_UNKNOWN`：活动 `abi_version` 无法识别系统调用号。
- 输入验证错误作为 VM 陷阱传播（例如，对于格式错误的 TLV 为 `E_NORITO_INVALID`）。

交叉引用
- 架构和VM语义：`ivm.md`
- 语言和内置映射：`docs/source/kotodama_grammar.md`

代记
- 可以通过以下方式从源生成系统调用常量的完整列表：
  - `make docs-syscalls` → 写入 `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → 验证生成的表是最新的（在 CI 中有用）
- 上面的子集仍然是面向合约的系统调用的精心策划的稳定表。

## 管理员/角色 TLV 示例（模拟主机）

本节记录了测试中使用的管理样式系统调用的模拟 WSV 主机接受的 TLV 形状和最小 JSON 有效负载。所有指针参数都遵循指针 ABI（放置在 INPUT 中的 Norito TLV 信封）。生产主机可以使用更丰富的模式；这些例子旨在阐明类型和基本形状。- REGISTER_PEER / UNREGISTER_PEER
  - 参数：`r10=&Json`
  - JSON 示例：`{ "peer": "peer-id-or-info" }`
  - CoreHost 注意：`REGISTER_PEER` 需要一个 `RegisterPeerWithPop` JSON 对象，其中包含 `peer` + `pop` 字节（可选 `activation_at`、`expiry_at`、`hsm`）； `UNREGISTER_PEER` 接受对等 ID 字符串或 `{ "peer": "..." }`。

- 创建_触发/删除_触发/设置_触发_启用
  - 创建_触发：
    - 参数：`r10=&Json`
    - 最小 JSON：`{ "name": "t1" }`（模拟忽略的其他字段）
  - 删除触发：
    - 参数：`r10=&Name`（触发器名称）
  - SET_TRIGGER_ENABLED：
    - 参数：`r10=&Name`、`r11=enabled:u64`（0 = 禁用，非零 = 启用）
  - CoreHost 注意：`CREATE_TRIGGER` 需要完整的触发器规范（base64 Norito `Trigger` 字符串或
    `{ "id": "<trigger_id>", "action": ... }` 以 `action` 作为基数64 Norito `Action` 字符串或
    JSON 对象），并且 `SET_TRIGGER_ENABLED` 切换触发器元数据键 `__enabled`（缺少
    默认启用）。

- 角色：CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - 创建角色：
    - 参数：`r10=&Name`（角色名称）、`r11=&Json`（权限集）
    - JSON 接受键 `"perms"` 或 `"permissions"`，每个键都是权限名称的字符串数组。
    - 示例：
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:ih58...", "transfer_asset:rose#wonder" ] }`
    - 模拟中支持的权限名称前缀：
      - `register_domain`、`register_account`、`register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - 删除角色：
    - 参数：`r10=&Name`
    - 如果任何帐户仍分配有此角色，则会失败。
  - 授予角色/撤销角色：
    - 参数：`r10=&AccountId`（主题）、`r11=&Name`（角色名称）
  - CoreHost注意：权限JSON可以是完整的`Permission`对象（`{ "name": "...", "payload": ... }`）或字符串（有效负载默认为`null`）； `GRANT_PERMISSION`/`REVOKE_PERMISSION` 接受 `&Name` 或 `&Json(Permission)`。

- 取消注册操作（域/帐户/资产）：不变量（模拟）
  - 如果域中存在帐户或资产定义，UNREGISTER_DOMAIN (`r10=&DomainId`) 将失败。
  - 如果账户余额非零或拥有 NFT，UNREGISTER_ACCOUNT (`r10=&AccountId`) 会失败。
  - 如果资产存在任何余额，UNREGISTER_ASSET (`r10=&AssetDefinitionId`) 将失败。

注释
- 这些示例反映了测试中使用的模拟 WSV 主机；真实的节点主机可能会暴露更丰富的管理模式或需要额外的验证。指针 ABI 规则仍然适用：TLV 必须位于 INPUT 中，版本 = 1，类型 ID 必须匹配，有效负载哈希必须验证。