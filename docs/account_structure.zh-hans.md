---
lang: zh-hans
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 账户结构 RFC

**状态：** 已接受 (ADDR-1)  
**受众：** 数据模型、Torii、Nexus、钱包、治理团队  
**相关问题：**待定

## 总结

本文档描述了在
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) 和
配套工具。它提供：

- 一个校验和、面向人的 **I105 地址 (I105)** 由
  `AccountAddress::to_i105` 将链判别式绑定到帐户
  控制器并提供确定性的互操作友好的文本形式。
- 隐式默认域和本地摘要的域选择器，带有
  为未来 Nexus 支持的路由保留全局注册表选择器标记（
  注册表查找**尚未发货**）。

## 动机

如今，钱包和链下工具依赖于原始 `alias@domain` (rejected legacy form) 路由别名。这个
有两个主要缺点：

1. **无网络绑定。** 该字符串没有校验和或链前缀，因此用户
   可以粘贴来自错误网络的地址而不立即反馈。的
   交易最终将被拒绝（链不匹配），或者更糟糕的是，成功
   如果目的地存在于本地，则针对非预期帐户。
2. **域冲突。** 域仅限命名空间，并且可以在每个域上重复使用
   链。服务联盟（托管人、桥梁、跨链工作流程）
   变得脆弱，因为链 A 上的 `finance` 与链 A 上的 `finance` 无关
   链B。

我们需要一种人性化的地址格式来防止复制/粘贴错误
以及从域名到权威链的确定性映射。

## 目标

- 描述数据模型中实现的 I105 信封以及
  `AccountId` 和 `AccountAddress` 遵循规范解析/别名规则。
- 将配置的链判别式直接编码到每个地址中并
  定义其治理/注册流程。
- 描述如何在不中断当前的情况下引入全局域注册机构
  部署并指定规范化/反欺骗规则。

## 非目标

- 实现跨链资产转移。路由层只返回
  目标链。
- 最终确定全球域名发行的治理。此 RFC 重点关注数据
  模型和传输原语。

## 背景

### 当前路由别名

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` 位于 `AccountId` 之外。节点检查交易的`ChainId`
针对准入期间的配置 (`AcceptTransactionFail::ChainIdMismatch`)
并拒绝国外交易，但账户字符串本身不携带
网络提示。

### 域标识符

`DomainId` 包装 `Name`（规范化字符串），并且作用域为本地链。
每个链可以独立注册`wonderland`、`finance`等。

### Nexus 上下文

Nexus 负责跨组件协调（通道/数据空间）。它
目前还没有跨链域路由的概念。

## 设计方案

### 1.确定性链判别式

`iroha_config::parameters::actual::Common` 现在公开：

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **限制：**
  - 每个活动网络都是唯一的；通过签署的公共注册表进行管理
    显式保留范围（例如，`0x0000–0x0FFF` test/dev、`0x1000–0x7FFF`
    社区分配，`0x8000–0xFFEF` 治理批准，`0xFFF0–0xFFFF`
    保留）。
  - 对于正在运行的链来说是不可变的。改变它需要硬分叉和
    注册表更新。
- **治理和注册（计划）：** 多重签名治理集将
  维护一个签名的 JSON 注册表，将判别式映射到人类别名，并且
  CAIP-2 标识符。该注册表还不是附带运行时的一部分。
- **用法：** 通过状态准入、Torii、SDK 和钱包 API 进行线程化，以便
  每个组件都可以嵌入或验证它。 CAIP-2 曝光仍是未来
  互操作任务。

### 2. 规范地址编解码器

Rust 数据模型公开了单个规范的有效负载表示
(`AccountAddress`) 可以以多种面向人的格式发出。 I105 是
用于共享和规范输出的首选帐户格式；压缩的
`sora` 形式是第二好的、仅限 Sora 的 UX 选项，其中假名字母
增加价值。规范十六进制仍然是一种调试辅助工具。

- **I105** – 嵌入链条的 I105 信封
  判别性的。解码器在将有效负载提升到之前验证前缀
  规范形式。
- **Sora 压缩视图** – 由 **105 个符号**构建的仅限 Sora 的字母表
  58字后追加半角イロハ诗（包括ヰ、ヱ）
  I105 套装。字符串以哨兵 `sora` 开头，嵌入 Bech32m 派生的
  校验和，并省略网络前缀（Sora Nexus 由哨兵暗示）。

  ```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **规范十六进制** – 规范字节的易于调试的 `0x…` 编码
  信封。

`AccountAddress::parse_encoded` 自动检测 I105（首选）、压缩（`sora`，第二好）或规范十六进制
（仅限 `0x...`；裸十六进制被拒绝）输入并返回解码的有效负载和检测到的负载
`AccountAddress`。 Torii 现在调用 `parse_encoded` 作为 ISO 20022 补充
寻址并存储规范的十六进制形式，因此元数据保持确定性
无论原始表示如何。

#### 2.1 标头字节布局（ADDR-1a）

每个规范有效负载都排列为 `header · controller`。的
`header` 是一个单字节，用于传达哪些解析器规则适用于以下字节：
遵循：

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

因此，第一个字节打包下游解码器的模式元数据：

|比特|领域|允许值 |违规错误 |
|------|------|----------------|--------------------|
| 7-5 | 7-5 `addr_version` | `0`（v1）。值 `1-7` 保留供将来修订。 | `0-7` 之外的值触发 `AccountAddressError::InvalidHeaderVersion`；目前，实现必须将非零版本视为不受支持。 |
| 4-3 | `addr_class` | `0` = 单密钥，`1` = 多重签名。 |其他值提高 `AccountAddressError::UnknownAddressClass`。 |
| 2-1 | 2-1 `norm_version` | `1`（规范 v1）。值 `0`、`2`、`3` 被保留。 | `0-3` 之外的值会提高 `AccountAddressError::InvalidNormVersion`。 |
| 0 | `ext_flag` |必须是 `0`。 |设置位升高 `AccountAddressError::UnexpectedExtensionFlag`。 |

Rust 编码器为单键控制器写入 `0x02`（版本 0、类别 0、
标准 v1，扩展标志已清除）和多重签名控制器的 `0x0A`（版本 0，
1 类，规范 v1，扩展标志已清除）。

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 控制器有效负载编码 (ADDR-1a)

控制器有效负载是附加在域选择器之后的另一个标记联合：|标签 |控制器|布局|笔记|
|-----|------------|--------|--------|
| `0x00` |单键| `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` 今天映射到 Ed25519。 `key_len` 绑定到 `u8`；较大的值会引发 `AccountAddressError::KeyPayloadTooLong`（因此大于 255 字节的单密钥 ML-DSA 公钥无法进行编码，必须使用多重签名）。 |
| `0x01` |多重签名 | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* |最多支持 255 个成员 (`CONTROLLER_MULTISIG_MEMBER_MAX`)。未知曲线引发 `AccountAddressError::UnknownCurve`；畸形的政策以 `AccountAddressError::InvalidMultisigPolicy` 的形式出现。 |

多重签名策略还公开了 CTAP2 风格的 CBOR 地图和规范摘要，以便
主机和 SDK 可以确定性地验证控制器。参见
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) 用于架构，
验证规则、散列程序和黄金装置。

所有密钥字节均按照 `PublicKey::to_bytes` 返回的方式进行编码；如果字节与声明的曲线不匹配，解码器会重建 `PublicKey` 实例并引发 `AccountAddressError::InvalidPublicKey`。

> **Ed25519 规范执行 (ADDR-3a)：** 曲线 `0x01` 密钥必须解码为签名者发出的确切字节字符串，并且不得位于小阶子组中。节点现在拒绝非规范编码（例如，以 `2^255-19` 为模减少的值）和身份元素等弱点，因此 SDK 应在提交地址之前显示匹配验证错误。

##### 2.3.1 曲线标识符注册表（ADDR-1d）

| ID (`curve_id`) |算法|特色门|笔记|
|----------------|----------|--------------|-----|
| `0x00` |保留 | — |不得发射；解码器表面 `ERR_UNKNOWN_CURVE`。 |
| `0x01` |埃德25519 | — | Canonical v1 算法 (`Algorithm::Ed25519`)；在默认配置中启用。 |
| `0x02` | ML-DSA (Dilithium3) | — |使用 Dilithium3 公钥字节（1952 字节）。单密钥地址无法编码 ML-DSA，因为 `key_len` 是 `u8`；多重签名使用 `u16` 长度。 |
| `0x03` | BLS12-381（正常）| `bls` | G1 中的公钥（48 字节），G2 中的签名（96 字节）。 |
| `0x04` | secp256k1 | — |基于 SHA-256 的确定性 ECDSA；公钥使用 33 字节 SEC1 压缩形式，签名使用规范的 64 字节 `r∥s` 布局。 |
| `0x05` | BLS12-381（小）| `bls` | G2 中的公钥（96 字节），G1 中的签名（48 字节）。 |
| `0x0A` | GOST R 34.10-2012（256，A 组）| `gost` |仅当启用 `gost` 功能时可用。 |
| `0x0B` | GOST R 34.10-2012（256，B 组）| `gost` |仅当启用 `gost` 功能时可用。 |
| `0x0C` | GOST R 34.10-2012（256，C 组）| `gost` |仅当启用 `gost` 功能时可用。 |
| `0x0D` | GOST R 34.10-2012（512，A 组）| `gost` |仅当启用 `gost` 功能时可用。 |
| `0x0E` | GOST R 34.10-2012（512，B 组）| `gost` |仅当启用 `gost` 功能时可用。 |
| `0x0F` | SM2 | `sm` | DistID 长度 (u16 BE) + DistID 字节 + 65 字节 SEC1 未压缩 SM2 密钥；仅当 `sm` 启用时才可用。 |

插槽 `0x06–0x09` 仍未分配其他曲线；介绍一个新的
算法需要路线图更新和匹配的 SDK/主机覆盖范围。编码器
必须使用 `ERR_UNSUPPORTED_ALGORITHM` 拒绝任何不支持的算法，并且
解码器必须在未知 ID 上快速失败，并保留 `ERR_UNKNOWN_CURVE`
失败关闭行为。

规范注册表（包括机器可读的 JSON 导出）位于
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)。
工具应该直接使用该数据集，以便保留曲线标识符
跨 SDK 和操作员工作流程保持一致。

- **SD​​K 门控：** SDK 默认仅进行 Ed25519 验证/编码。斯威夫特暴露
  编译时标志（`IROHASWIFT_ENABLE_MLDSA`、`IROHASWIFT_ENABLE_GOST`、
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK 需要
  `AccountAddress.configureCurveSupport(...)`； JavaScript SDK 使用
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`。
  secp256k1 支持可用，但在 JS/Android 中默认未启用
  软件开发工具包；调用者在发出非 Ed25519 控制器时必须明确选择加入。
- **主机门控：** `Register<Account>` 拒绝签名者使用算法的控制器
  节点的 `crypto.allowed_signing` 列表中缺少 **或** 曲线标识符不存在
  `crypto.curves.allowed_curve_ids`，因此集群必须通告支持（配置+
  genesis），然后才能注册 ML‑DSA/GOST/SM 控制器。 BLS控制器
  编译时始终允许算法（共识密钥依赖于它们），
  默认配置启用 Ed25519 + secp256k1。【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 多重签名控制器指南

`AccountController::Multisig` 通过序列化策略
`crates/iroha_data_model/src/account/controller.rs` 并强制实施架构
记录在 [`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) 中。
关键实施细节：

- 政策之前经过 `MultisigPolicy::validate()` 标准化和验证
  被嵌入。阈值必须≥1且≤Σ权重；重复的成员是
  按 `(algorithm || 0x00 || key_bytes)` 排序后确定性删除。
- 二进制控制器有效负载 (`ControllerPayload::Multisig`) 编码
  `version:u8`、`threshold:u16`、`member_count:u8`，然后是每个成员的
  `(curve_id, weight:u16, key_len:u16, key_bytes)`。这正是
  `AccountAddress::canonical_bytes()` 写入 I105（首选）/sora（第二佳）有效负载。
- 散列 (`MultisigPolicy::digest_blake2b256()`) 使用 Blake2b-256 和
  `iroha-ms-policy` 个性化字符串，以便治理清单可以绑定到
  与 I105 中嵌入的控制器字节匹配的确定性策略 ID。
- 灯具覆盖范围位于 `fixtures/account/address_vectors.json`（案例
  `addr-multisig-*`）。钱包和 SDK 应断言规范的 I105 字符串
  下面确认它们的编码器与 Rust 实现匹配。

|案例编号 |门槛/会员 | I105 文字（前缀 `0x02F1`）| Sora 压缩 (`sora`) 文字 |笔记|
|--------|---------------------------------|--------------------------------|----------------------------------------|--------|
| `addr-multisig-council-threshold3` | `≥3` 重量，成员 `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` |理事会域治理法定人数。 |
| `addr-multisig-wonderland-threshold2` | `≥2`，成员 `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` |双签名仙境示例（权重 1 + 2）。 |
| `addr-multisig-default-quorum3` | `≥3`，成员 `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` |用于基础治理的隐式默认域仲裁。

#### 2.4 失败规则（ADDR-1a）

- 短于所需标头 + 选择器或具有剩余字节的有效负载会发出 `AccountAddressError::InvalidLength` 或 `AccountAddressError::UnexpectedTrailingBytes`。
- 设置保留 `ext_flag` 或通告不受支持的版本/类的标头必须使用 `UnexpectedExtensionFlag`、`InvalidHeaderVersion` 或 `UnknownAddressClass` 拒绝。
- 未知的选择器/控制器标签引发 `UnknownDomainTag` 或 `UnknownControllerTag`。
- 过大或畸形的密钥材料会引发 `KeyPayloadTooLong` 或 `InvalidPublicKey`。
- 超过 255 个成员的多重签名控制器筹集了 `MultisigMemberOverflow`。
- IME/NFKC 转换：半角 Sora 假名可以标准化为其全角形式，而不会破坏解码，但 ASCII `sora` 哨兵和 I105 数字/字母必须保持 ASCII。全角或大小写折叠标记表面为 `ERR_MISSING_COMPRESSED_SENTINEL`，全角 ASCII 有效负载引发 `ERR_INVALID_COMPRESSED_CHAR`，校验和不匹配冒泡为 `ERR_CHECKSUM_MISMATCH`。 `crates/iroha_data_model/src/account/address.rs` 中的属性测试涵盖了这些路径，因此 SDK 和钱包可以依赖确定性故障。
- 当 I105（首选）/sora（第二佳）输入在别名回退之前失败（例如，校验和不匹配、域摘要不匹配）时，Torii 和 `address@domain` (rejected legacy form) 别名的 SDK 解析现在会发出相同的 `ERR_*` 代码，因此客户端可以中继结构化原因，而无需从散文字符串中猜测。
- 本地选择器有效负载短于 12 字节表面 `ERR_LOCAL8_DEPRECATED`，保留传统 Local-8 摘要的硬切换。
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 规范二元向量

- **隐式默认域（`default`，种子字节 `0x00`）**  
  规范十六进制：`0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`。  
  细分：`0x02` 标头、`0x00` 选择器（隐式默认值）、`0x00` 控制器标签、`0x01` 曲线 id (Ed25519)、`0x20` 密钥长度，后跟 32 字节密钥负载。
- **本地域摘要（`treasury`，种子字节 `0x01`）**  
  规范十六进制：`0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`。  
  细分：`0x02` 标头、选择器标签 `0x01` 加上摘要 `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`，后跟单密钥有效负载（`0x00` 标签、`0x01` 曲线 id、`0x20` 长度、32 字节 Ed25519键）。单元测试 (`account::address::tests::parse_encoded_accepts_all_formats`) 通过 `AccountAddress::parse_encoded` 断言下面的 V1 向量，保证工具可以依赖十六进制、I105（首选）和压缩（`sora`，第二佳）形式的规范有效负载。使用 `cargo run -p iroha_data_model --example address_vectors` 重新生成扩展夹具组。

|域名 |种子字节 |规范六角 |压缩 (`sora`) |
|------------------------|----------|--------------------------------------------------------------------------------------------------------|------------|
|默认 | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
|国库| `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|仙境| `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
|伊吕波 | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
|阿尔法| `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
|欧米茄| `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
|治理| `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
|验证者 | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
|探险家| `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
|索拉内特| `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
|狐狸 | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
|达| `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

审核者：数据模型工作组、密码学工作组 — ADDR-1a 范围已批准。

##### Sora Nexus 参考别名

Sora Nexus 网络默认为 `chain_discriminant = 0x02F1`
（`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`）。的
因此，`AccountAddress::to_i105` 和 `to_i105` 助手会发出
每个规范有效负载的一致文本形式。精选赛程来自
`fixtures/account/address_vectors.json`（通过生成
`cargo xtask address-vectors`）如下所示以供快速参考：

|帐户/选择器 | I105 文字（前缀 `0x02F1`）| Sora 压缩 (`sora`) 文字 |
|--------------------------------|--------------------------------|-------------------------|
| `default` 域（隐式选择器，种子 `0x00`）| `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE`（提供显式路由提示时可选 `@default` 后缀）|
| `treasury`（本地摘要选择器，种子 `0x01`）| `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|全局注册表指针（`registry_id = 0x0000_002A`，相当于 `treasury`）| `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

这些字符串与 CLI (`iroha tools address convert`)、Torii 发出的字符串匹配
响应 (`canonical I105 literal rendering`) 和 SDK 帮助程序，因此 UX 复制/粘贴
流量可以逐字依赖它们。仅当您需要显式路由提示时才附加 `<address>@<domain>` (rejected legacy form)；后缀不是规范输出的一部分。

#### 2.6 用于互操作性的文本别名（计划）

- **链别名样式：** `ih:<chain-alias>:<alias@domain>` 用于日志和人类
  条目。钱包必须解析前缀，验证嵌入的链，并阻止
  不匹配。
- **CAIP-10 形式：** `iroha:<caip-2-id>:<i105-addr>` 与链无关
  集成。此映射在发货中**尚未实现**
  工具链。
- **机器助手：** 发布 Rust、TypeScript/JavaScript、Python 的编解码器
  和 Kotlin 涵盖 I105 和压缩格式（`AccountAddress::to_i105`，
  `AccountAddress::parse_encoded` 及其 SDK 等效项）。 CAIP-10 助手是
  未来的工作。

#### 2.7 确定性 I105 别名

- **前缀映射：** 重新使用 `chain_discriminant` 作为 I105 网络前缀。
  `encode_i105_prefix()`（参见 `crates/iroha_data_model/src/account/address.rs`）
  为值 `<64` 发出 6 位前缀（单字节）和 14 位两字节
  更大网络的形式。权威作业在
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK 必须保持匹配的 JSON 注册表同步以避免冲突。
- **帐户材料：** I105 编码由以下构建的规范有效负载
  `AccountAddress::canonical_bytes()`—标头字节、域选择器和
  控制器有效负载。没有额外的哈希步骤； I105 嵌入
  Rust 生成的二进制控制器有效负载（单密钥或多签名）
  编码器，而不是用于多重签名策略摘要的 CTAP2 映射。
- **编码：** `encode_i105()` 将前缀字节与规范连接起来
  有效负载并附加从 Blake2b-512 派生的 16 位校验和，其中固定
  Prefix: `I105PRE` (`b"I105PRE"` || prefix || payload). The result is encoded via `bs58` using the I105 alphabet.
  CLI/SDK 帮助程序公开相同的过程，并且 `AccountAddress::parse_encoded`
  通过 `decode_i105` 反转它。

#### 2.8 规范文本测试向量

`fixtures/account/address_vectors.json` 包含完整的 I105（首选）和压缩的（`sora`，第二好的）
每个规范有效负载的文字。亮点：

- **`addr-single-default-ed25519`（Sora Nexus，前缀 `0x02F1`）。**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`，压缩 (`sora`)
  `sora2QG…U4N5E5`。 Torii 从 `AccountId` 发出这些确切的字符串
  `Display` 实现（规范 I105）和 `AccountAddress::to_i105`。
- **`addr-global-registry-002a`（注册表选择器→财务）。**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`，压缩 (`sora`)
  `sorakX…CM6AEP`。证明注册表选择器仍然解码为
  与相应的本地摘要相同的规范有效负载。
- **失败案例（`i105-prefix-mismatch`）。**  
  在节点上解析使用前缀 `NETWORK_PREFIX + 1` 编码的 I105 文字
  期望默认前缀产生
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  在尝试域路由之前。 `i105-checksum-mismatch` 夹具
  对 Blake2b 校验和进行篡改检测。

#### 2.9 合规装置

ADDR-2 提供了一个可重玩的夹具包，涵盖正面和负面
跨规范十六进制、I105（首选）、压缩（`sora`、半角/全角）、隐式的场景
默认选择器、全局注册表别名和多重签名控制器。的
规范的 JSON 存在于 `fixtures/account/address_vectors.json` 中，并且可以
重新生成：

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

对于临时实验（不同的路径/格式），示例二进制文件仍然是
可用：

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` 中的 Rust 单元测试
和 `crates/iroha_torii/tests/account_address_vectors.rs` 以及 JS，
Swift 和 Android 线束 (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`，
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
使用相同的固定装置来保证 SDK 和 Torii 准入之间的编解码器奇偶性。

### 3. 全球唯一的域名和标准化

另请参阅：[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
适用于 Torii、数据模型和 SDK 中使用的规范 Norm v1 管道。

将 `DomainId` 重新定义为标记元组：

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` 包装当前链管理的域的现有名称。
当通过全局注册表注册域时，我们会保留所有权
链的判别式。显示/解析目前保持不变，但是
扩展结构允许路由决策。

#### 3.1 标准化和欺骗防御

Norm v1 定义了每个组件在域之前必须使用的规范管道
名称被保留或嵌入到 `AccountAddress` 中。完整的演练
居住在 [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)；
下面的摘要记录了钱包、Torii、SDK 和治理的步骤
工具必须实施。

1. **输入验证。** 拒绝空字符串、空格和保留
   分隔符 `@`、`#`、`$`。这与强制执行的不变量相匹配
   `Name::validate_str`。
2. **Unicode NFC 组成。** 规范地应用 ICU 支持的 NFC 标准化
   等效序列确定性地崩溃（例如，`e\u{0301}` → `é`）。
3. **UTS-46 标准化。** 通过 UTS-46 运行 NFC 输出
   `use_std3_ascii_rules = true`、`transitional_processing = false` 和
   已启用 DNS 长度强制。结果是小写的 A 标签序列；
   违反 STD3 规则的输入在此失败。
4. **长度限制。** 强制执行 DNS 样式限制：每个标签必须为 1–63
   字节，并且在步骤 3 之后完整域不得超过 255 字节。
5. **可选的易混淆策略。** UTS-39 脚本检查被跟踪
   规范 v2；操作员可以提前启用它们，但如果检查失败则必须中止
   处理。

如果每个阶段都成功，则小写 A 标签字符串将被缓存并用于
地址编码、配置、清单和注册表查找。本地文摘
选择器将其 12 字节值派生为 `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` 使用步骤 3 的输出。所有其他尝试（混合
大小写、大写、原始 Unicode 输入）会被结构化拒绝
`ParseError`s 位于提供名称的边界处。

展示这些规则的规范装置 - 包括 punycode 往返
和无效的 STD3 序列 — 列于
`docs/source/references/address_norm_v1.md` 并镜像在 SDK CI 中
在 ADDR-2 下跟踪的矢量套件。

### 4. Nexus 域名注册和路由- **注册表架构：** Nexus 维护签名映射 `DomainName -> ChainRecord`
  其中 `ChainRecord` 包括链判别式、可选元数据（RPC
  端点）和权威证明（例如，治理多重签名）。
- **同步机制：**
  - 链向 Nexus 提交签名的域名声明（在创世期间或通过
    治理指令）。
  - Nexus 发布定期清单（签名的 JSON 加上可选的 Merkle 根）
    通过 HTTPS 和内容寻址存储（例如 IPFS）。客户固定
    最新清单并验证签名。
- **查找流程：**
  - Torii 接收引用 `DomainId` 的交易。
  - 如果本地未知域，则 Torii 查询缓存的 Nexus 清单。
  - 如果清单指示外部链，则交易将被拒绝
    确定性 `ForeignDomain` 错误和远程链信息。
  - 如果 Nexus 中缺少域，则 Torii 返回 `UnknownDomain`。
- **信任锚和轮换：** 治理关键标志清单；旋转或
  撤销作为新的清单条目发布。客户执行清单
  TTL（例如 24 小时）并拒绝查阅超出该窗口的陈旧数据。
- **失败模式：** 如果清单检索失败，Torii 会回退到缓存
  TTL内的数据；过了 TTL，它会发出 `RegistryUnavailable` 并拒绝
  跨域路由以避免状态不一致。

### 4.1 注册表不变性、别名和逻辑删除 (ADDR-7c)

Nexus 发布 **仅附加清单**，以便每个域或别名分配
可以审核和重播。运营商必须处理中描述的捆绑包
[地址清单 Runbook](source/runbooks/address_manifest_ops.md) 作为
唯一事实来源：如果清单丢失或验证失败，Torii 必须
拒绝解析受影响的域。

自动化支持：`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
重放校验和、模式和先前摘要检查
运行手册。将命令输出包含在变更单中以显示 `sequence`
在发布捆绑包之前验证了 `previous_digest` 链接。

#### 清单标头和签名合同

|领域|要求 |
|--------|-------------|
| `version` |目前为 `1`。仅通过匹配的规格更新进行碰撞。 |
| `sequence` |每个出版物**精确**增加一。 Torii 缓存拒绝有间隙或回归的修订。 |
| `generated_ms` + `ttl_hours` |建立缓存新鲜度（默认 24 小时）。如果 TTL 在下一次发布之前到期，则 Torii 会翻转为 `RegistryUnavailable`。 |
| `previous_digest` |先前清单正文的 BLAKE3 摘要（十六进制）。验证者使用 `b3sum` 重新计算它以证明不变性。 |
| `signatures` |清单通过 Sigstore (`cosign sign-blob`) 进行签名。运营人员必须运行 `cosign verify-blob --bundle manifest.sigstore manifest.json` 并在推出之前强制执行治理身份/发行者约束。 |

发布自动化发出 `manifest.sigstore` 和 `checksums.sha256`
与 JSON 主体一起。镜像到 SoraFS 或时将文件放在一起
HTTP 端点，以便审核员可以逐字重播验证步骤。

#### 条目类型

|类型 |目的|必填字段 |
|------|---------|-----------------|
| `global_domain` |声明域已全局注册，并且应映射到链判别式和 I105 前缀。 | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` |永久停用别名/选择器。擦除 Local-8 摘要或删除域时需要。 | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` 条目可以选择包含 `manifest_url` 或 `sorafs_cid`
将钱包指向签名的链元数据，但规范元组仍然存在
`{domain, chain, discriminant/i105_prefix}`。 `tombstone` 记录**必须**引用
已退休的选择器和授权的票证/治理工件
进行更改，以便可以离线重建审计跟踪。

#### 别名/逻辑删除工作流程和遥测

1. **检测漂移。** 使用 `torii_address_local8_total{endpoint}`，
   `torii_address_local8_domain_total{endpoint,domain}`，
   `torii_address_collision_total{endpoint,kind="local12_digest"}`，
   `torii_address_collision_domain_total{endpoint,domain}`，
   `torii_address_domain_total{endpoint,domain_kind}`，和
   `torii_address_invalid_total{endpoint,reason}`（呈现于
   `dashboards/grafana/address_ingest.json`）以确认本地提交和
   在提出墓碑之前，Local-12 碰撞保持为零。的
   每个域计数器让所有者证明只有开发/测试域发出 Local-8
   流量（并且 Local-12 冲突映射到已知的暂存域），同时
   包括 **Domain Kind Mix (5m)** 面板，以便 SRE 可以绘制出有多少
   `domain_kind="local12"` 流量保持不变，并且 `AddressLocal12Traffic`
   每当生产环境仍然看到 Local-12 选择器时，就会触发警报，尽管
   退休门。
2. **导出规范摘要。** 运行
   `iroha tools address convert <address> --format json --expect-prefix 753`
   （或通过消耗 `fixtures/account/address_vectors.json`
   `scripts/account_fixture_helper.py`) 捕获准确的 `digest_hex`。
   CLI 接受 I105、`i105` 和规范 `0x…` 文字；追加
   `@<domain>` 仅当您需要保留清单标签时。
   JSON 摘要通过 `input_domain` 字段显示该域，并且
   `legacy  suffix` 将转换后的编码重播为 `<address>@<domain>` (rejected legacy form)
   清单差异（此后缀是元数据，而不是规范帐户 ID）。
   对于面向换行的导出使用
   `iroha tools address normalize --input <file> legacy-selector input mode` 批量转换本地
   选择器转换成规范的 I105（首选）、压缩的（`sora`，第二好的）、十六进制或 JSON 形式，同时跳过
   非本地行。当审计员需要电子表格友好的证据时，运行
   `iroha tools address audit --input <file> --format csv` 发出 CSV 摘要
   (`input,status,format,domain_kind,…`) 突出显示本地选择器，
   规范编码和同一文件中的解析失败。
3. **附加清单条目。** 起草 `tombstone` 记录（以及后续
   `global_domain` 迁移到全局注册表时记录）并验证
   在请求签名之前使用 `cargo xtask address-vectors` 的清单。
4. **验证并发布。** 遵循运行手册清单（哈希值、Sigstore、
   序列单调性），然后将包镜像到 SoraFS。现在 Torii
   捆绑包落地后立即规范化 I105（首选）/sora（第二佳）文字。
5. **监控和回滚。** 将 Local-8 和 Local-12 碰撞面板保持在
   30天归零；如果出现回归，请重新发布之前的清单
   仅在受影响的非生产环境中，直到遥测稳定为止。

上述所有步骤都是 ADDR-7c 的强制性证据：清单中没有
`cosign` 签名包或没有匹配的 `previous_digest` 值必须
自动拒绝，操作员必须附上验证日志
他们的改签机票。

### 5.钱包和API人体工程学

- **显示默认值：** 钱包显示 I105 地址（短，校验和）
  加上从注册表中获取的解析域作为标签。域名是
  明确标记为可能更改的描述性元数据，而 I105 是
  稳定的地址。
- **输入规范化：** Torii 和 SDK 接受 I105（首选）/sora（次佳）/0x
  地址加上 `alias@domain` (rejected legacy form)、`uaid:…` 和
  `opaque:…` 形式，然后规范化为 I105 进行输出。没有
  严格模式切换；原始电话/电子邮件标识符必须保留在账本之外
  通过 UAID/不透明映射。
- **错误预防：** 钱包解析 I105 前缀并强制链判别
  期望。链不匹配会通过可操作的诊断触发硬故障。
- **编解码器库：** 官方 Rust、TypeScript/JavaScript、Python 和 Kotlin
  库提供 I105 编码/解码以及压缩 (`sora`) 支持
  避免碎片化的实施。 CAIP-10 转换尚未发货。

#### 辅助功能和安全共享指南- 实时跟踪产品表面的实施指南
  `docs/portal/docs/reference/address-safety.md`；参考该清单时
  使这些要求适应钱包或浏览器用户体验。
- **安全共享流程：** 复制或显示地址的表面默认为 I105 形式，并公开相邻的“共享”操作，该操作显示完整的字符串和从同一负载派生的 QR 码，以便用户可以通过视觉或扫描来验证校验和。当截断不可避免时（例如，小屏幕），请保留字符串的开头和结尾，添加清晰的省略号，并保持可通过复制到剪贴板访问完整地址，以防止意外剪辑。
- **IME 保护措施：** 地址输入必须拒绝来自 IME/IME 风格键盘的组合伪影。强制仅输入 ASCII，在检测到全角或假名字符时显示内联警告，并提供纯文本粘贴区域，在验证之前剥离组合标记，以便日语和中文用户可以禁用其 IME，而不会丢失进度。
- **屏幕阅读器支持：** 提供可视化隐藏标签 (`aria-label`/`aria-describedby`)，用于描述前导 I105 前缀数字并将 I105 有效负载分为 4 或 8 个字符组，因此辅助技术读取分组字符而不是连续字符串。通过礼貌的实时区域宣布复制/共享成功，并确保二维码预览包含描述性替代文本（“链 0x02F1 上 <alias> 的 I105 地址”）。
- **仅 Sora 压缩使用：** 始终将 `i105` 压缩视图标记为“仅 Sora”，并在复制之前将其置于显式确认后面。当链判别式不是 Sora Nexus 值时，SDK 和钱包必须拒绝显示压缩输出，并应引导用户返回 I105 进行网络间转账，以避免资金错误路由。

## 实施清单

- **I105 信封：** 前缀使用紧凑型编码 `chain_discriminant`
  来自 `encode_i105_prefix()` 的 6 位/14 位方案，主体是规范字节
  (`AccountAddress::canonical_bytes()`)，校验和是前两个字节
  Blake2b-512(`b"I105PRE"` || prefix || body). The full payload is encoded via `bs58` using the I105 alphabet.
- **注册合约：** 签名 JSON（和可选的 Merkle 根）发布
  `{discriminant, i105_prefix, chain_alias, endpoints}` 带 24 小时 TTL 和
  旋转键。
- **域名政策：** ASCII `Name` 今天；如果启用 i18n，请申请 UTS-46
  标准化和 UTS-39 用于易混淆的检查。强制执行最大标签 (63) 和
  总长度 (255)。
- **文本助手：** 在 Rust 中提供 I105 ↔ 压缩 (`i105`) 编解码器，
  具有共享测试向量的 TypeScript/JavaScript、Python 和 Kotlin (CAIP-10
  映射仍然是未来的工作）。
- **CLI 工具：** 通过 `iroha tools address convert` 提供确定性操作员工作流程
  （参见 `crates/iroha_cli/src/address.rs`），它接受 I105/`0x…` 文字和
  可选的 `<address>@<domain>` (rejected legacy form) 标签，默认为使用 Sora Nexus 前缀 (`753`) 的 I105 输出，
  并且仅在操作员明确请求时才发出仅 Sora 的压缩字母表
  `--format i105` 或 JSON 摘要模式。该命令强制执行前缀期望
  解析，记录提供的域（JSON 中的 `input_domain`）和 `legacy  suffix` 标志
  将转换后的编码重播为 `<address>@<domain>` (rejected legacy form)，因此明显的差异仍然符合人体工程学。
- **钱包/浏览器用户体验：**遵循[地址显示准则](source/sns/address_display_guidelines.md)
  附带 ADDR-6 — 提供双复制按钮，将 I105 保留为 QR 有效负载，并发出警告
  用户认为压缩的 `i105` 形式仅适用于 Sora，并且容易受到 IME 重写。
- **Torii 集成：** 缓存 Nexus 体现尊重 TTL，发出
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` 确定性地，并且
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### Torii 响应格式

- `GET /v1/accounts` 接受可选的 `canonical I105 rendering` 查询参数并且
  `POST /v1/accounts/query` 接受 JSON 信封内的相同字段。
  支持的值为：
  - `i105`（默认）——响应发出规范的 I105 有效负载（例如，
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`）。
  - `i105_default` — 响应发出仅 Sora 的 `i105` 压缩视图，同时
    保持过滤器/路径参数规范。
- 无效值返回 `400` (`QueryExecutionFail::Conversion`)。这允许
  钱包和浏览器请求压缩字符串以获得仅限 Sora 的 UX，同时
  将 I105 保留为可互操作的默认值。
- 资产持有者列表 (`GET /v1/assets/{definition_id}/holders`) 及其 JSON
  对应的信封 (`POST …/holders/query`) 也兑现 `canonical I105 rendering`。
  每当 `items[*].account_id` 字段发出压缩文字
  参数/信封字段设置为 `i105_default`，镜像帐户
  端点，以便浏览器可以跨目录呈现一致的输出。
- **测试：** 添加编码器/解码器往返、错误链的单元测试
  失败和明显的查找；在 Torii 和 SDK 中添加集成覆盖范围
  对于 I105 流端到端。

## 错误代码注册表

地址编码器和解码器通过以下方式暴露故障
`AccountAddressError::code_str()`。下表提供了稳定的代码
SDK、钱包和 Torii 表面应与人类可读的表面一起显示
消息以及建议的修复指南。

### 规范构造

|代码|失败|建议的补救措施|
|------|---------|------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` |编码器收到注册表或构建功能不支持的签名算法。 |将帐户构建限制为注册表和配置中启用的曲线。 |
| `ERR_KEY_PAYLOAD_TOO_LONG` |签名密钥有效负载长度超出支持的限制。 |单键控制器仅限于 `u8` 长度；对大型公钥使用多重签名（例如 ML-DSA）。 |
| `ERR_INVALID_HEADER_VERSION` |地址标头版本超出支持范围。 |为 V1 地址发出标头版本 `0`；在采用新版本之前升级编码器。 |
| `ERR_INVALID_NORM_VERSION` |无法识别标准化版本标志。 |使用规范化版本 `1` 并避免切换保留位。 |
| `ERR_INVALID_I105_PREFIX` |无法对请求的 I105 网络前缀进行编码。 |在链注册表中发布的包含 `0..=16383` 范围内选择一个前缀。 |
| `ERR_CANONICAL_HASH_FAILURE` |规范有效负载哈希失败。 |重试操作；如果错误仍然存​​在，请将其视为哈希堆栈中的内部错误。 |

### 格式解码和自动检测

|代码|失败|建议的补救措施|
|------|---------|------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 字符串包含字母表之外的字符。 |确保地址使用已发布的 I105 字母表，并且在复制/粘贴过程中未被截断。 |
| `ERR_INVALID_LENGTH` |有效负载长度与选择器/控制器的预期规范大小不匹配。 |为选定的域选择器和控制器布局提供完整的规范有效负载。 |
| `ERR_CHECKSUM_MISMATCH` | I105（首选）或压缩（`sora`，第二好的）校验和验证失败。 |从可信来源重新生成地址；这通常表示复制/粘贴错误。 |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 前缀字节格式错误。 |使用兼容的编码器重新编码地址；不要手动更改前导 I105 字节。 |
| `ERR_INVALID_HEX_ADDRESS` |规范的十六进制形式无法解码。 |提供由官方编码器生成的以 `0x` 为前缀、偶数长度的十六进制字符串。 |
| `ERR_MISSING_COMPRESSED_SENTINEL` |压缩形式不以 `sora` 开头。 |在将压缩的 Sora 地址交给解码器之前，使用所需的哨兵作为前缀。 |
| `ERR_COMPRESSED_TOO_SHORT` |压缩字符串缺少足够的有效负载和校验和数字。 |使用编码器发出的完整压缩字符串而不是截断的片段。 |
| `ERR_INVALID_COMPRESSED_CHAR` |遇到压缩字母表之外的字符。 |将字符替换为已发布的半角/全角表中的有效 Base-105 字形。 |
| `ERR_INVALID_COMPRESSED_BASE` |编码器尝试使用不受支持的基数。 |针对编码器提交错误； V1 中压缩字母表固定为基数 105。 |
| `ERR_INVALID_COMPRESSED_DIGIT` |数字值超过压缩字母大小。 |确保每个数字都在 `0..105)` 范围内，如有必要，请重新生成地址。 |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` |自动检测无法识别输入格式。 |调用解析器时提供 I105（首选）、压缩的 (`sora`) 或规范的 `0x` 十六进制字符串。 |

### 域和网络验证|代码|失败|建议的补救措施|
|------|---------|------------------------|
| `ERR_DOMAIN_MISMATCH` |域选择器与预期域不匹配。 |使用为目标域颁发的地址或更新期望。 |
| `ERR_INVALID_DOMAIN_LABEL` |域标签未通过规范化检查。 |编码前使用 UTS-46 非过渡处理规范化域。 |
| `ERR_UNEXPECTED_NETWORK_PREFIX` |解码后的 I105 网络前缀与配置的值不同。 |切换到目标链中的地址或调整预期的判别式/前缀。 |
| `ERR_UNKNOWN_ADDRESS_CLASS` |地址类别位无法识别。 |将解码器升级到可以理解新类的版本，或避免篡改标头位。 |
| `ERR_UNKNOWN_DOMAIN_TAG` |域选择器标签未知。 |更新到支持新选择器类型的版本，或避免在 V1 节点上使用实验性负载。 |
| `ERR_UNEXPECTED_EXTENSION_FLAG` |保留扩展位已设置。 |清除保留位；它们将保持封闭状态，直到未来的 ABI 引入它们。 |
| `ERR_UNKNOWN_CONTROLLER_TAG` |无法识别控制器有效负载标签。 |升级解码器以在解析新控制器类型之前识别它们。 |
| `ERR_UNEXPECTED_TRAILING_BYTES` |规范有效负载包含解码后的尾随字节。 |重新生成规范有效负载；仅应显示记录的长度。 |

### 控制器有效负载验证

|代码|失败|建议的补救措施|
|------|---------|------------------------|
| `ERR_INVALID_PUBLIC_KEY` |关键字节与声明的曲线不匹配。 |确保关键字节完全按照所选曲线的要求进行编码（例如，32 字节 Ed25519）。 |
| `ERR_UNKNOWN_CURVE` |曲线标识符未注册。 |使用曲线 ID `1` (Ed25519)，直到其他曲线获得批准并在注册表中发布。 |
| `ERR_MULTISIG_MEMBER_OVERFLOW` |多重签名控制器声明的成员数量超出了支持的数量。 |在编码之前将多重签名成员资格减少到记录的限制。 |
| `ERR_INVALID_MULTISIG_POLICY` |多重签名策略有效负载验证失败（阈值/权重/架构）。 |重建策略，使其满足 CTAP2 架构、权重边界和阈值约束。 |

## 考虑的替代方案

- **纯 checksum envelope（比特币风格）。** 校验和更简单，但错误检测更弱
  比 Blake2b 派生的 I105 校验和（`encode_i105` 截断 512 位哈希值）
  并且缺乏 16 位判别式的显式前缀语义。
- **在域字符串中嵌入链名称（例如，`finance@chain`）。** 中断
- **仅依靠 Nexus 路由而不更改地址。**用户仍然会
  复制/粘贴不明确的字符串；我们希望地址本身携带上下文。
- **Bech32m 信封。** QR 友好并提供人类可读的前缀，但是
  将偏离运输 I105 实施 (`AccountAddress::to_i105`)
  并需要重新创建所有装置/SDK。目前的路线图保持 I105+
  压缩（`sora`）支持，同时继续研究未来
  Bech32m/QR 层（CAIP-10 映射被推迟）。

## 开放问题

- 确认`u16`判别式加上预留范围覆盖长期需求；
  否则使用 varint 编码评估 `u32`。
- 完成注册表更新的多重签名治理流程以及如何实施
  处理撤销/过期分配。
- 定义确切的清单签名方案（例如，Ed25519 多重签名）以及
  Nexus 分发的传输安全（HTTPS 固定、IPFS 哈希格式）。
- 确定是否支持迁移的域别名/重定向以及如何支持
  在不打破决定论的情况下将它们浮现出来。
- 指定 Kotodama/IVM 合约如何访问 I105 助手（`to_address()`、
  `parse_address()`）以及链上存储是否应该公开 CAIP-10
  映射（现在 I105 是规范的）。
- 探索在外部注册表中注册 Iroha 链（例如 I105 注册表、
  CAIP 命名空间目录）以实现更广泛的生态系统协调。

## 后续步骤

1、I105编码登陆`iroha_data_model`（`AccountAddress::to_i105`，
   `parse_encoded`);继续将固定装置/测试移植到每个 SDK 并清除任何
   Bech32m 占位符。
2. 使用 `chain_discriminant` 扩展配置模式并导出合理的
  现有测试/开发设置的默认值。 **（完成：`common.chain_discriminant`
  现在以 `iroha_config` 形式提供，每个网络默认为 `0x02F1`
  覆盖。）**
3. 起草 Nexus 注册表架构和概念验证清单发布者。
4. 收集钱包提供商和托管人对人为因素的反馈
   （HRP 命名、显示格式）。
5. 更新文档（`docs/source/data_model.md`、Torii API 文档）
   实施路径已承诺。
6. 发布官方编解码库（Rust/TS/Python/Kotlin）并进行规范测试
   涵盖成功和失败案例的向量。
