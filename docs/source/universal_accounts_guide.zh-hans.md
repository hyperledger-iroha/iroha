---
lang: zh-hans
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f972f8f82b7f4e89c1d48b0dbbc6eb5b73303e2fab0f580ab21e63990ba03af8
source_last_modified: "2026-03-27T19:05:17.617064+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 通用账户指南

本指南从以下内容中提取了 UAID（通用帐户 ID）部署要求：
Nexus 路线图并将其打包成以操作员 + SDK 为重点的演练。
它涵盖 UAID 推导、投资组合/清单检查、监管模板、
以及每个“iroha 应用程序空间目录清单”必须随附的证据
发布` run (roadmap reference: `roadmap.md:2209`)。

## 1.UAID快速参考- UAID 是 `uaid:<hex>` 文字，其中 `<hex>` 是 Blake2b-256 摘要，其
  LSB 设置为 `1`。规范类型位于
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`。
- 帐户记录（`Account` 和 `AccountDetails`）现在带有可选的 `uaid`
  字段，以便应用程序无需定制哈希即可了解标识符。
- 隐藏函数标识符策略可以绑定任意规范化输入
  （电话号码、电子邮件、帐号、合作伙伴字符串）到 `opaque:` ID
  在 UAID 命名空间下。链上的碎片是`IdentifierPolicy`，
  `IdentifierClaimRecord` 和 `opaque_id -> uaid` 索引。
- 空间目录维护一个 `World::uaid_dataspaces` 映射来绑定每个 UAID
  活动清单引用的数据空间帐户。 Torii 重用该
  `/portfolio` 和 `/uaids/*` API 的映射。
- `POST /v1/accounts/onboard` 发布默认空间目录清单
  当不存在时全局数据空间，因此 UAID 立即绑定。
  入职机构必须持有 `CanPublishSpaceDirectoryManifest{dataspace=0}`。
- 所有 SDK 都公开了用于规范 UAID 文字的帮助程序（例如，
  Android SDK 中的 `UaidLiteral`）。助手接受原始 64 十六进制摘要
  (LSB=1) 或 `uaid:<hex>` 文字并重新使用相同的 Norito 编解码器，以便
  摘要不能跨语言漂移。

## 1.1 隐藏标识符策略

UAID 现在是第二个身份层的锚点：- 全局 `IdentifierPolicyId` (`<kind>#<business_rule>`) 定义
  命名空间、公共承诺元数据、解析器验证密钥以及
  规范输入标准化模式（`Exact`、`LowercaseTrimmed`、
  `PhoneE164`、`EmailAddress` 或 `AccountNumber`）。
- 一项声明将一个派生的 `opaque:` 标识符恰好绑定到一个 UAID 和一个
  该政策下的规范 `AccountId`，但链只接受
  索赔时附有签名的 `IdentifierResolutionReceipt`。
- 分辨率仍然是 `resolve -> transfer` 流。 Torii 解决了不透明问题
  处理并返回规范的 `AccountId`；转移目标仍然是
  规范帐户，而不是直接 `uaid:` 或 `opaque:` 文字。
- 策略现在可以通过以下方式发布 BFV 输入加密参数
  `PolicyCommitment.public_parameters`。当存在时，Torii 在
  `GET /v1/identifier-policies`，客户端可以提交 BFV 包装的输入
  而不是明文。编程策略将 BFV 参数包装在
  规范的 `BfvProgrammedPublicParameters` 捆绑包还发布了
  公共 `ram_fhe_profile`；传统的原始 BFV 有效负载已升级到
  重建承诺时的规范包。
- 标识符路由经过相同的 Torii 访问令牌和速率限制
  作为其他面向应用程序的端点进行检查。它们不是绕过正常的
  API 政策。

## 1.2 术语

命名拆分是有意的：- `ram_lfe` 是外部隐藏函数抽象。涵盖政策
  注册、承诺、公共元数据、执行收据，以及
  验证模式。
- `BFV` 是 Brakerski/Fan-Vercauteren 使用的同态加密方案
  一些 `ram_lfe` 后端来评估加密输入。
- `ram_fhe_profile` 是 BFV 特定的元数据，而不是整个元数据的第二个名称
  功能。它描述了钱包和
  验证者必须针对策略使用编程后端的情况。

具体来说：

- `RamLfeProgramPolicy` 和 `RamLfeExecutionReceipt` 是 LFE 层类型。
- `BfvParameters`、`BfvCiphertext`、`BfvProgrammedPublicParameters` 和
  `BfvRamProgramProfile` 是 FHE 层类型。
- `HiddenRamFheProgram` 和 `HiddenRamFheInstruction` 是内部名称
  由编程后端执行的隐藏 BFV 程序。他们留在
  FHE 方面，因为它们描述的是加密执行机制而不是
  外部保单或收据抽象。

## 1.3 账户身份与别名

通用帐户的推出不会改变规范的帐户身份模型：- `AccountId` 仍然是规范的无域帐户主题。
- `ScopedAccountId { account, domain }` 是视图或的显式域上下文
  实现域名链接的注册。这不是第二个规范
  身份。
- SNS/帐户别名是该主题之上的单独绑定。一个
  域限定别名，例如 `merchant@hbl.sbp` 和数据空间根别名
  例如 `merchant@sbp` 都可以解析为相同的规范 `AccountId`。
- 存储帐户记录上的 `linked_domains` 是从
  帐户域索引。它描述了当前具体化的链接
  主题；它不是规范标识符的一部分。

算子、SDK、测试的实现规则：从规范开始
`AccountId`，然后添加别名租约、数据空间/域权限和显式
单独的域链接。不要合成虚假的领域范围规范
帐户只是因为别名或路由携带域段。

当前 Torii 路由：|路线 |目的|
|--------|---------|
| `GET /v1/ram-lfe/program-policies` |列出活动和非活动 RAM-LFE 程序策略及其公共执行元数据，包括可选 BFV `input_encryption` 参数和编程后端 `ram_fhe_profile`。 |
| `POST /v1/ram-lfe/programs/{program_id}/execute` |只接受 `{ input_hex }` 或 `{ encrypted_input }` 之一，并返回所选程序的无状态 `RamLfeExecutionReceipt` 和 `{ output_hex, output_hash, receipt_hash }`。当前的 Torii 运行时为已编程的 BFV 后端发出收据。 |
| `POST /v1/ram-lfe/receipts/verify` |根据已发布的链上程序策略无状态地验证 `RamLfeExecutionReceipt`，并可选择检查调用者提供的 `output_hex` 是否与收据 `output_hash` 匹配。 |
| `GET /v1/identifier-policies` |列出活动和非活动隐藏功能策略命名空间及其公共元数据，包括可选的 BFV `input_encryption` 参数、加密客户端输入所需的 `normalization` 模式以及编程的 BFV 策略的 `ram_fhe_profile`。 |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` |恰好接受 `{ input }` 或 `{ encrypted_input }` 之一。明文 `input` 是服务器端规范化的； BFV `encrypted_input` 必须已根据已发布的策略模式进行标准化。然后，端点派生 `opaque:` 句柄并返回 `ClaimIdentifier` 可以在链上提交的签名收据，包括原始 `signature_payload_hex` 和解析后的 `signature_payload`。 || `POST /v1/identifiers/resolve` |恰好接受 `{ input }` 或 `{ encrypted_input }` 之一。明文 `input` 是服务器端规范化的； BFV `encrypted_input` 必须已根据已发布的策略模式进行标准化。当存在有效声明时，端点将标识符解析为 `{ opaque_id, receipt_hash, uaid, account_id, signature }`，并且还将规范签名的有效负载返回为 `{ signature_payload_hex, signature_payload }`。 |
| `GET /v1/identifiers/receipts/{receipt_hash}` |查找与确定性收据哈希绑定的持久 `IdentifierClaimRecord`，以便操作员和 SDK 可以审核声明所有权或诊断重播/不匹配失败，而无需扫描完整标识符索引。 |

Torii的进程内执行运行时配置在
`torii.ram_lfe.programs[*]`，由 `program_id` 键入。标识符现在路由
重用相同的 RAM-LFE 运行时而不是单独的 `identifier_resolver`
配置表面。

目前的SDK支持：- `normalizeIdentifierInput(value, normalization)` 与 Rust 匹配
  `exact`、`lowercase_trimmed`、`phone_e164` 的规范化器、
  `email_address` 和 `account_number`。
- `ToriiClient.listIdentifierPolicies()` 列出策略元数据，包括 BFV
  策略发布时的输入加密元数据，加上解码的
  通过 `input_encryption_public_parameters_decoded` 的 BFV 参数对象。
  编程策略还公开解码后的 `ram_fhe_profile`。该字段是
  有意 BFV 范围：它让钱包验证预期的寄存器
  计数、通道计数、规范化模式和最小密文模数
  在加密客户端输入之前编程的 FHE 后端。
- `getIdentifierBfvPublicParameters(policy)` 和
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` 帮助
  JS 调用者使用已发布的 BFV 元数据并构建策略感知请求
  机构无需重新实施策略 ID 和规范化规则。
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` 和
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` 现在让
  JS 钱包在本地构建完整的 BFV Norito 密文信封
  发布策略参数而不是发送预构建的密文十六进制。
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  解析隐藏标识符并返回签名的收据有效负载，
  包括 `receipt_hash`、`signature_payload_hex` 和
  `signature_payload`。
- `ToriiClient.issueIdentifierClaimReceipt（accountId，{policyId，输入|
  加密输入})` issues the signed receipt needed by `ClaimIdentifier`。
- `verifyIdentifierResolutionReceipt(receipt, policy)` 验证返回的
  针对客户端策略解析器密钥的收据，以及`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` 获取
  为以后的审计/调试流程保留索赔记录。
- `IrohaSwift.ToriiClient` 现在公开 `listIdentifierPolicies()`，
  `resolveIdentifier(policyId:input:encryptedInputHex:)`，
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`，
  和 `getIdentifierClaimByReceiptHash(_)`，加上
  `ToriiIdentifierNormalization` 同一电话/电子邮件/帐号
  规范化模式。
- `ToriiIdentifierLookupRequest` 和
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` 帮助程序提供类型化的 Swift 请求表面
  解决和索赔接收调用，Swift 策略现在可以导出 BFV
  通过 `encryptInput(...)` / `encryptedRequest(input:...)` 本地密文。
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` 验证
  顶级收据字段与签名的有效负载匹配并验证
  提交前解析器签名客户端。
- Android SDK 中的 `HttpClientTransport` 现在公开
  `listIdentifierPolicies()`，`resolveIdentifier（policyId，输入，
  加密输入十六进制）`, `issueIdentifierClaimReceipt（accountId，policyId，
  输入，加密的InputHex）`, and `getIdentifierClaimByReceiptHash（...）`，
  加上 `IdentifierNormalization` 以获得相同的规范化规则。
- `IdentifierResolveRequest` 和
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` 帮助程序提供类型化的 Android 请求表面，
  而 `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` 推导出BFV密文信封
  从本地发布的策略参数。
  `IdentifierResolutionReceipt.verifySignature(policy)` 验证返回的
  解析器签名客户端。

当前指令集：- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier`（绑定收据；原始 `opaque_id` 索赔被拒绝）
- `RevokeIdentifier`

`iroha_crypto::ram_lfe` 中现在存在三个后端：

- 历史承诺约束的 `HKDF-SHA3-512` PRF，以及
- BFV 支持的秘密仿射评估器，使用 BFV 加密的标识符
  直接插槽。当使用默认构建 `iroha_crypto` 时
  `bfv-accel` 特征，BFV 环乘法使用精确的确定性
  内置CRT-NTT后端；禁用该功能会回到
  具有相同输出的标量教科书路径，以及
- BFV 支持的秘密编程评估器，可导出指令驱动的
  对加密寄存器和密文内存进行 RAM 式执行跟踪
  派生不透明标识符和收据哈希之前的通道。已编程的
  后端现在需要比仿射路径更强的 BFV 模数底限，并且
  它的公共参数发布在一个规范包中，其中包括
  钱包和验证者使用的 RAM-FHE 执行配置文件。

这里 BFV 表示 Brakerski/Fan-Vercauteren FHE 方案实施于
`crates/iroha_crypto/src/fhe_bfv.rs`。这是加密执行机制
由仿射和编程后端使用，而不是外部隐藏的名称
函数抽象。Torii使用策略承诺发布的后端。当 BFV 后端
处于活动状态，明文请求先标准化，然后在服务器端加密
评价。评估仿射后端的 BFV `encrypted_input` 请求
直接且必须已标准化客户端；已编程的后端
将加密输入规范化回解析器的确定性 BFV
执行秘密 RAM 程序之前的信封，以便保留收据哈希值
在语义等效的密文中保持稳定。

## 2. 导出并验证 UAID

支持三种获取 UAID 的方式：

1. **从世界状态或 SDK 模型中读取。** 任何 `Account`/`AccountDetails`
   通过 Torii 查询的有效负载现在填充了 `uaid` 字段
   参与者选择使用通用账户。
2. **查询 UAID 注册表。** Torii 公开
   `GET /v1/space-directory/uaids/{uaid}` 返回数据空间绑定
   以及空间目录主机保留的清单元数据（请参阅
   `docs/space-directory.md` §3 用于有效负载样本）。
3. **确定性地推导它。** 当离线引导新的 UAID 时，散列
   规范参与者种子为 Blake2b-256 并在结果前加上前缀
   `uaid:`。下面的代码片段反映了中记录的帮助程序
   `docs/space-directory.md` §3.3：

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```始终以小写形式存储文字，并在散列之前标准化空格。
CLI 帮助程序，例如 `iroha app space-directory manifest scaffold` 和 Android
`UaidLiteral` 解析器应用相同的修剪规则，因此治理审查可以
无需临时脚本即可交叉检查值。

## 3. 检查 UAID 持有量和清单

`iroha_core::nexus::portfolio` 中的确定性投资组合聚合器
显示引用 UAID 的每个资产/数据空间对。运营商和 SDK
可以通过以下表面消费数据：

|表面|用途 |
|--------|--------|
| `GET /v1/accounts/{uaid}/portfolio` |返回数据空间→资产→余额摘要； `docs/source/torii/portfolio_api.md` 中描述。 |
| `GET /v1/space-directory/uaids/{uaid}` |列出与 UAID 关联的数据空间 ID + 帐户文字。 |
| `GET /v1/space-directory/uaids/{uaid}/manifests` |提供完整的 `AssetPermissionManifest` 历史记录以供审核。 |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI 快捷方式包装绑定端点并可选择将 JSON 写入磁盘 (`--json-out`)。 |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` |获取证据包的清单 JSON 包。 |

CLI 会话示例（通过 `iroha.json` 中的 `torii_api_url` 配置的 Torii URL）：

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

将 JSON 快照与审核期间使用的清单哈希一起存储；的
每当出现时，空间目录观察器都会重建 `uaid_dataspaces` 地图
激活、过期或撤销，因此这些快照是证明的最快方法
在给定的时期哪些绑定是活跃的。

## 4. 出版能力有据可依

每当推出新配额时，请使用下面的 CLI 流程。每一步都必须
记录在用于治理签署的证据包中的土地。

1. **对清单 JSON 进行编码**，以便审阅者可以在之前看到确定性哈希
   提交：

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **使用 Norito 有效负载 (`--manifest`) 或
   JSON 描述 (`--manifest-json`)。记录 Torii/CLI 收据以及
   `PublishSpaceDirectoryManifest` 指令哈希：

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **捕获SpaceDirectory事件证据。** 订阅
   `SpaceDirectoryEvent::ManifestActivated` 并将事件有效负载包含在
   以便审计人员可以确认变更何时生效。

4. **生成审核包** 将清单与其数据空间配置文件联系起来，并
   遥测挂钩：

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **通过 Torii**（`bindings fetch` 和 `manifests fetch`）验证绑定
   使用上面的哈希 + 捆绑包归档这些 JSON 文件。

证据清单：

- [ ] 由变更审批者签名的清单哈希 (`*.manifest.hash`)。
- [ ] CLI/Torii 发布调用的收据（stdout 或 `--json-out` 工件）。
- [ ] `SpaceDirectoryEvent` 有效负载证明激活。
- [ ] 审核包含数据空间配置文件、挂钩和清单副本的捆绑包目录。
- [ ] 绑定 + 从 Torii 激活后获取的清单快照。这反映了 `docs/space-directory.md` §3.2 中的要求，同时提供 SDK
拥有在发布审核期间可指向的单个页面。

## 5. 监管机构/区域清单模板

当制作能力显现时，使用回购中的固定装置作为起点
对于监管机构或地区监管机构。他们演示了如何确定允许/拒绝的范围
规则并解释审查者期望的政策说明。

|夹具|目的|亮点|
|--------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB 审计源。 | `compliance.audit::{stream_reports, request_snapshot}` 的只读津贴，并拒绝零售转账，以保持监管机构 UAID 的被动。 |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA 监管车道。 |添加有上限的 `cbdc.supervision.issue_stop_order` 限额（每日窗口 + `max_amount`）和对 `force_liquidation` 的明确拒绝，以实施双重控制。 |

克隆这些装置时，更新：

1. `uaid` 和 `dataspace` id 与您启用的参与者和通道相匹配。
2. `activation_epoch`/`expiry_epoch` 基于治理时间表的窗口。
3. `notes` 字段以及监管机构的政策参考（MiCA 文章，JFSA
   圆形等）。
4. 津贴窗口（`PerSlot`、`PerMinute`、`PerDay`）和可选
   `max_amount` 上限，因此 SDK 强制执行与主机相同的限制。

## 6. SDK 使用者的迁移说明引用每个域帐户 ID 的现有 SDK 集成必须迁移到
上面描述的以 UAID 为中心的表面。在升级期间使用此清单：

  帐户 ID。对于 Rust/JS/Swift/Android，这意味着升级到最新版本
  工作区板条箱或重新生成 Norito 绑定。
- **API 调用：** 将域范围的投资组合查询替换为
  `GET /v1/accounts/{uaid}/portfolio` 和清单/绑定端点。
  `GET /v1/accounts/{uaid}/portfolio` 接受可选的 `asset_id` 查询
  当钱包只需要单个资产实例时的参数。客户帮手如
  如 `ToriiClient.getUaidPortfolio` (JS) 和 Android
  `SpaceDirectoryClient` 已经包装了这些路由；比起定制更喜欢它们
  HTTP 代码。
- **缓存和遥测：** 通过 UAID + 数据空间而不是原始缓存条目
  帐户 ID，并发出显示 UAID 文字的遥测数据，以便操作可以
  将日志与空间目录证据对齐。
- **错误处理：**新端点返回严格的UAID解析错误
  记录在 `docs/source/torii/portfolio_api.md` 中；表面那些代码
  逐字记录，以便支持团队可以对问题进行分类，而无需重复步骤。
- **测试：** 连接上述固定装置（加上您自己的 UAID 清单）
  进入 SDK 测试套件以证明 Norito 往返和清单评估
  匹配主机实现。

## 7. 参考文献- `docs/space-directory.md` — 具有更深入生命周期详细信息的操作手册。
- `docs/source/torii/portfolio_api.md` — UAID 组合的 REST 架构和
  明显的端点。
- `crates/iroha_cli/src/space_directory.rs` — 中引用的 CLI 实现
  本指南。
- `fixtures/space_directory/capability/*.manifest.json` — 监管机构、零售和
  CBDC 清单模板可供克隆。