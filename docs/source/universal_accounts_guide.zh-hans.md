<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
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
- `AccountAlias` 值是该主题之上的单独 SNS 绑定。一个
  域限定别名，例如 `merchant@banka.paynet` 和数据空间根别名
  例如 `merchant@paynet` 都可以解析为相同的规范 `AccountId`。
- 规范账户注册始终为 `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`；没有领域限定或领域具体化
  注册路径。
- 域所有权、别名权限和其他域范围内的行为实时
  他们自己的状态和 API，而不是帐户身份本身。
- 公共帐户查找遵循该分割：别名查询保持公开，而
  规范帐户身份仍然是纯粹的 `AccountId`。

算子、SDK、测试的实现规则：从规范开始
`AccountId`，然后添加别名租约、数据空间/域权限以及任何
域拥有状态单独。不要合成假的别名衍生帐户
或者仅仅因为别名或期望帐户记录上有任何链接域字段
路由携带域段。

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
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
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
每当出现时，空间目录观察器就会重建 `uaid_dataspaces` 映射
激活、过期或撤销，因此这些快照是证明的最快方法
在给定的时期哪些绑定是活跃的。## 4. 出版能力有据可依

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
