---
slug: /nexus/confidential-assets
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Assets & ZK Transfers
description: Phase C blueprint for shielded circulation, registries, and operator controls.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# 机密资产和ZK传输设计

## 动机
- 提供选择加入的受保护资产流，以便域名可以在不改变透明流通的情况下保护交易隐私。
- 为审计员和操作员提供电路和加密参数的生命周期控制（激活、轮换、撤销）。

## 威胁模型
- 验证者诚实但好奇：他们忠实地执行共识，但尝试检查账本/状态。
- 网络观察者可以看到区块数据和八卦交易；没有私人八卦渠道的假设。
- 超出范围：账本外流量分析、量子对手（根据 PQ 路线图单独跟踪）、账本可用性攻击。

## 设计概述
- 除了现有的透明余额之外，资产还可以声明一个“屏蔽池”；屏蔽流通通过加密承诺来表示。
- 注释将 `(asset_id, amount, recipient_view_key, blinding, rho)` 封装为：
  - 承诺：`Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 无效符：`Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`，与音符顺序无关。
  - 加密有效负载：`enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- 交易传输 Norito 编码的 `ConfidentialTransfer` 有效负载，其中包含：
  - 公共输入：Merkle 锚、无效器、新承诺、资产 ID、电路版本。
  - 接收者和可选审核员的加密有效负载。
  - 零知识证明证明价值保存、所有权和授权。
- 验证密钥和参数集通过带有激活窗口的账本注册表进行控制；节点拒绝验证引用未知或已撤销条目的证明。
- 共识标头提交到活动的机密功能摘要，因此仅当注册表和参数状态匹配时才接受块。
- 证明构造使用 Halo2（Plonkish）堆栈，无需可信设置； v1 中故意不支持 Groth16 或其他 SNARK 变体。

### 确定性赛程

机密备忘录信封现在附带 `fixtures/confidential/encrypted_payload_v1.json` 的规范固定装置。该数据集捕获正的 v1 包络和负的畸形样本，以便 SDK 可以断言解析奇偶校验。 Rust 数据模型测试 (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) 和 Swift 套件 (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) 都直接加载夹具，保证 Norito 编码、错误表面和回归覆盖随着编解码器的发展保持一致。

Swift SDK 现在可以发出屏蔽指令，无需定制 JSON 胶水：构造一个
`ShieldRequest` 具有 32 字节票据承诺、加密有效负载和借方元数据，
然后调用 `IrohaSDK.submit(shield:keypair:)`（或 `submitAndWait`）来签名并转发
交易超过 `/v1/pipeline/transactions`。助手验证承诺长度，
将 `ConfidentialEncryptedPayload` 线程到 Norito 编码器中，并镜像 `zk::Shield`
下面描述的布局使钱包与 Rust 保持同步。

## 共识承诺和能力门控
- 块头公开 `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`；摘要参与共识哈希，并且必须等于本地注册表视图才能接受块。
- 治理可以通过将 `next_conf_features` 编程为未来的 `activation_height` 来进行阶段升级；直到这个高度，区块生产者必须继续发出之前的摘要。
- 验证节点必须使用 `confidential.enabled = true` 和 `assume_valid = false` 运行。如果任一条件失败或本地 `conf_features` 出现分歧，启动检查将拒绝加入验证器集。
- P2P 握手元数据现在包括 `{ enabled, assume_valid, conf_features }`。宣传不受支持的功能的同行会被 `HandshakeConfidentialMismatch` 拒绝，并且永远不会进入共识轮换。
- 非验证者观察者可以设置 `assume_valid = true`；他们盲目应用机密增量，但不影响共识安全。

## 资产保单
- 每个资产定义都带有由创建者或通过治理设置的 `AssetConfidentialPolicy`：
  - `TransparentOnly`：默认模式；仅允许透明指令（`MintAsset`、`TransferAsset` 等），并且拒绝屏蔽操作。
  - `ShieldedOnly`：所有发行和转让必须使用保密指令； `RevealConfidential` 被禁止，因此余额永远不会公开出现。
  - `Convertible`：持有者可以使用下面的入口/出口指令在透明和屏蔽表示之间移动价值。
- 政策遵循受限的 FSM，以防止资金搁浅：
  - `TransparentOnly → Convertible`（立即启用屏蔽池）。
  - `TransparentOnly → ShieldedOnly`（需要待处理的转换和转换窗口）。
  - `Convertible → ShieldedOnly`（强制最小延迟）。
  - `ShieldedOnly → Convertible`（需要迁移计划，以便受保护的票据仍然可以使用）。
  - `ShieldedOnly → TransparentOnly` 是不允许的，除非屏蔽池为空或者治理编码了取消屏蔽未完成票据的迁移。
- 治理指令通过 `ScheduleConfidentialPolicyTransition` ISI 设置 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }`，并可能使用 `CancelConfidentialPolicyTransition` 中止计划的更改。内存池验证确保没有交易跨越转换高度，并且如果策略检查会在块中发生更改，则包含会确定性失败。
- 打开新块时，会自动应用挂起的转换：一旦块高度进入转换窗口（对于 `ShieldedOnly` 升级）或达到编程的 `effective_height`，运行时将更新 `AssetConfidentialPolicy`，刷新 `zk.policy` 元数据，并清除挂起的条目。如果在 `ShieldedOnly` 转换成熟时仍然存在透明供应，则运行时将中止更改并记录警告，使先前的模式保持不变。
- 配置旋钮 `policy_transition_delay_blocks` 和 `policy_transition_window_blocks` 强制执行最短通知和宽限期，让钱包在开关周围转换票据。
- `pending_transition.transition_id` 兼作审核句柄；治理在完成或取消转换时必须引用它，以便操作员可以关联入口/出口报告。
- `policy_transition_window_blocks` 默认为 720（约 12 小时，60 秒区块时间）。节点会限制尝试更短通知的治理请求。
- Genesis 清单和 CLI 流程显示当前和待定的政策。准入逻辑在执行时读取策略以确认每条机密指令均已获得授权。
- 迁移清单 — 请参阅下面的“迁移顺序”，了解 Milestone M0 跟踪的分阶段升级计划。

#### 通过 Torii 监控转换

钱包和审计员轮询 `GET /v1/confidential/assets/{definition_id}/transitions` 进行检查
活动 `AssetConfidentialPolicy`。 JSON 有效负载始终包含规范的
资产id，最新观察到的区块高度，策略的`current_mode`，模式为
在该高度有效（转换窗口暂时报告 `Convertible`），并且
预期为 `vk_set_hash`/Poseidon/Pedersen 参数标识符。当治理
转换正在等待响应还嵌入：

- `transition_id` — `ScheduleConfidentialPolicyTransition` 返回的审核句柄。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` 和派生的 `window_open_height` （钱包必须
  开始转换为 ShieldedOnly 切换）。

响应示例：

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` 响应表示不存在匹配的资产定义。当没有过渡时
预定的`pending_transition`字段是`null`。

### 策略状态机|当前模式 |下一个模式 |先决条件 |有效高度搬运|笔记|
|--------------------------------|--------------------------------|------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
|只透明|敞篷车|治理已激活验证程序/参数注册表项。提交 `ScheduleConfidentialPolicyTransition` 和 `effective_height ≥ current_height + policy_transition_delay_blocks`。 |转换恰好在 `effective_height` 处执行；屏蔽池立即可用。                   |用于在保持透明流的同时启用机密性的默认路径。               |
|只透明|仅屏蔽 |同上，加上 `policy_transition_window_blocks ≥ 1`。                                                         |运行时在 `effective_height - policy_transition_window_blocks` 处自动进入 `Convertible`；在 `effective_height` 处翻转至 `ShieldedOnly`。 |在禁用透明指令之前提供确定性转换窗口。   |
|敞篷车|仅屏蔽 |计划转换为 `effective_height ≥ current_height + policy_transition_delay_blocks`。治理应通过审计元数据进行认证（`transparent_supply == 0`）；运行时在切换时强制执行此操作。 |与上面相同的窗口语义。如果透明电源在 `effective_height` 处不为零，则转换将在 `PolicyTransitionPrerequisiteFailed` 处中止。 |将资产锁定在完全保密的流通状态。                                     |
|仅屏蔽 |敞篷车|预定的过渡；没有主动紧急提款（`withdraw_height` 未设置）。                                    |状态翻转于 `effective_height`；显示坡道重新打开，同时屏蔽音符仍然有效。                           |用于维护窗口或审核员审查。                                          |
|仅屏蔽 |只透明|治理必须证明 `shielded_supply == 0` 或制定已签署的 `EmergencyUnshield` 计划（需要审核员签名）。 |运行时在 `effective_height` 之前打开 `Convertible` 窗口；在最高峰时，机密指令会发生故障，并且资产会返回到仅透明模式。 |最后的出口。如果在窗口期间有任何机密票据花费，过渡将自动取消。 |
|任何|与当前相同| `CancelConfidentialPolicyTransition` 清除挂起的更改。                                                        | `pending_transition` 立即删除。                                                                          |维持现状；显示完整性。                                             |

上面未列出的过渡在治理提交期间将被拒绝。运行时在应用计划的转换之前检查先决条件；失败的先决条件会将资产推回到之前的模式，并通过遥测和块事件发出 `PolicyTransitionPrerequisiteFailed`。

### 迁移排序

2. **分阶段过渡：** 提交 `ScheduleConfidentialPolicyTransition` 以及尊重 `policy_transition_delay_blocks` 的 `effective_height`。当转向 `ShieldedOnly` 时，指定转换窗口 (`window ≥ policy_transition_window_blocks`)。
3. **发布操作员指南：** 记录返回的 `transition_id` 并分发进/出匝道运行手册。钱包和审计员订阅 `/v1/confidential/assets/{id}/transitions` 以了解窗口打开高度。
4. **窗口强制：** 当窗口打开时，运行时将策略切换到 `Convertible`，发出 `PolicyTransitionWindowOpened { transition_id }`，并开始拒绝冲突的治理请求。
5. **完成或中止：** 在 `effective_height` 处，运行时验证转换先决条件（零透明供应、无紧急提款等）。成功将策略翻转到请求的模式；失败会发出 `PolicyTransitionPrerequisiteFailed`，清除挂起的转换，并使策略保持不变。
6. **架构升级：** 成功过渡后，治理会提高资产架构版本（例如，`asset_definition.v2`），并且 CLI 工具在序列化清单时需要 `confidential_policy`。 Genesis 升级文档指示操作员在重新启动验证器之前添加策略设置和注册表指纹。

从启用保密性开始的新网络直接在创世中编码所需的策略。在发布后更改模式时，他们仍然遵循上面的清单，以便转换窗口保持确定性，并且钱包有时间进行调整。

### Norito 清单版本控制和激活

- Genesis 清单必须包含自定义 `confidential_registry_root` 密钥的 `SetParameter`。有效负载是与 `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` 匹配的 Norito JSON：当没有活动的验证器条目时省略该字段 (`null`)，否则提供一个 32 字节的十六进制字符串 (`0x…`)，该字符串等于 `compute_vk_set_hash` 通过清单中提供的验证器指令生成的哈希值。如果参数丢失或哈希值与编码的注册表写入不一致，节点将拒绝启动。
- 在线 `ConfidentialFeatureDigest::conf_rules_version` 嵌入清单布局版本。对于 v1 网络，它必须保持 `Some(1)` 并等于 `iroha_config::parameters::defaults::confidential::RULES_VERSION`。当规则集演变时，改变常量，重新生成清单，并同步推出二进制文件；混合版本会导致验证器拒绝带有 `ConfidentialFeatureDigestMismatch` 的块。
- 激活清单应该捆绑注册表更新、参数生命周期更改和策略转换，以便摘要保持一致：
  1. 在离线状态视图中应用计划的注册表突变（`Publish*`、`Set*Lifecycle`），并使用 `compute_confidential_feature_digest` 计算激活后摘要。
  2. 使用计算出的散列发出 `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`，以便落后的对等方即使错过中间注册表指令也可以恢复正确的摘要。
  3. 附加 `ScheduleConfidentialPolicyTransition` 指令。每条指令必须引用治理发布的`transition_id`；忘记它的清单将被运行时拒绝。
  4. 保留激活计划中使用的清单字节、SHA-256 指纹和摘要。操作员在投票使清单生效之前验证所有三个工件，以避免分区。
- 当部署需要延迟切换时，在配套自定义参数中记录目标高度（例如 `custom.confidential_upgrade_activation_height`）。这为审计员提供了 Norito 编码的证据，证明验证者在摘要更改生效之前遵守了通知窗口。

## 验证器和参数生命周期
### ZK注册表
- Ledger 存储 `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`，其中 `proving_system` 目前固定为 `Halo2`。
- `(circuit_id, version)`对是全球唯一的；注册表维护一个二级索引，用于通过电路元数据进行查找。在入院期间尝试注册重复的配对会被拒绝。
- `circuit_id` 必须非空，并且必须提供 `public_inputs_schema_hash`（通常是验证者规范公共输入编码的 Blake2b-32 哈希值）。准入会拒绝省略这些字段的记录。
- 治理指令包括：
  - `PUBLISH` 添加仅包含元数据的 `Proposed` 条目。
  - `ACTIVATE { vk_id, activation_height }` 在纪元边界安排条目激活。
  - `DEPRECATE { vk_id, deprecation_height }` 标记最终高度，校样可以参考该条目。
  - `WITHDRAW { vk_id, withdraw_height }` 用于紧急关闭；受影响的资产在提款高峰后冻结机密支出，直到新条目激活。
- Genesis 清单自动发出 `confidential_registry_root` 自定义参数，其 `vk_set_hash` 与活动条目匹配；在节点加入共识之前，验证会根据本地注册表状态交叉检查此摘要。
- 注册或更新验证器需要`gas_schedule_id`；验证强制要求注册表项为 `Active`，存在于 `(circuit_id, version)` 索引中，并且 Halo2 证明提供 `OpenVerifyEnvelope`，其 `circuit_id`、`vk_hash` 和 `public_inputs_schema_hash` 与注册表记录匹配。

### 证明密钥
- 证明密钥保留在账本外，但由与验证者元数据一起发布的内容寻址标识符（`pk_cid`、`pk_hash`、`pk_len`）引用。
- 钱包 SDK 获取 PK 数据、验证哈希值并在本地缓存。

### Pedersen 和 Poseidon 参数
- 单独的注册表（`PedersenParams`、`PoseidonParams`）镜像验证器生命周期控制，每个都有 `params_id`、生成器/常量的哈希值、激活、弃用和撤回高度。## 确定性排序和取消器
- 每项资产均维护 `CommitmentTree` 和 `next_leaf_index`；块按确定性顺序追加承诺：按块顺序迭代交易；在每个事务中，通过升序序列化 `output_idx` 迭代屏蔽输出。
- `note_position` 源自树偏移量，但 **不是** 无效符的一部分；它只提供证据见证人中的成员路径。
- PRF设计保证了重组下的无效器稳定性； PRF 输入绑定 `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`，锚点引用受 `max_anchor_age_blocks` 限制的历史 Merkle 根。

## 账本流程
1. **MintConfidential { asset_id, amount,recipient_hint }**
   - 需要资产保单 `Convertible` 或 `ShieldedOnly`；准入检查资产权限，检索当前 `params_id`，采样 `rho`，发出承诺，更新 Merkle 树。
   - 发出 `ConfidentialEvent::Shielded` 以及新的承诺、Merkle 根增量和审计跟踪的交易调用哈希。
2. **TransferConfidential { asset_id、proof、circle_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - VM 系统调用使用注册表项验证证据；主机确保无效符未使用，确定性附加承诺，锚点是最近的。
   - Ledger 记录 `NullifierSet` 条目，为接收者/审计者存储加密的有效负载，并发出 `ConfidentialEvent::Transferred` 总结无效符、有序输出、证明哈希和 Merkle 根。
3. **RevealConfidential { asset_id、proof、circle_id、version、nullifier、amount、recipient_account、anchor_root }**
   - 仅适用于 `Convertible` 资产；证明验证票据价值等于显示的金额，分类账记入透明余额，并通过标记已用的废纸来销毁受保护的票据。
   - 发出 `ConfidentialEvent::Unshielded` 以及公开金额、消耗的无效符、证明标识符和交易调用哈希。

## 数据模型添加
- `ConfidentialConfig`（新配置部分），带有启用标志、`assume_valid`、气体/限制旋钮、锚点窗口、验证器后端。
- 具有显式版本字节 (`CONFIDENTIAL_ASSET_V1 = 0x01`) 的 `ConfidentialNote`、`ConfidentialTransfer` 和 `ConfidentialMint` Norito 模式。
- `ConfidentialEncryptedPayload` 使用 `{ version, ephemeral_pubkey, nonce, ciphertext }` 包装 AEAD 备忘录字节，对于 XChaCha20-Poly1305 布局，默认为 `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1`。
- 规范密钥派生向量位于 `docs/source/confidential_key_vectors.json` 中； CLI 和 Torii 端点均针对这些装置进行回归。
- `asset::AssetDefinition` 获得 `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` 保留 `(backend, name, commitment)` 绑定以用于传输/取消屏蔽验证器；执行会拒绝引用或内联验证密钥与注册承诺不匹配的证明。
- `CommitmentTree`（每个具有边境检查点的资产）、`NullifierSet`，由存储在世界状态中的 `(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` 键入。
- Mempool 维护瞬态 `NullifierIndex` 和 `AnchorIndex` 结构，用于早期重复检测和锚点年龄检查。
- Norito 模式更新包括公共输入的规范排序；往返测试确保编码确定性。
- 加密有效负载往返通过单元测试锁定 (`crates/iroha_data_model/src/confidential.rs`)。后续钱包向量将为审计员附上规范的 AEAD 成绩单。 `norito.md` 记录信封的在线标头。

## IVM 集成和系统调用
- 引入 `VERIFY_CONFIDENTIAL_PROOF` 系统调用接受：
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof` 以及生成的 `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - 系统调用从注册表加载验证者元数据，强制执行大小/时间限制，收取确定性气体，并且仅在证明成功时应用增量。
- 主机公开只读 `ConfidentialLedger` 特征，用于检索 Merkle 根快照和无效器状态； Kotodama 库提供见证程序集帮助程序和架构验证。
- 更新了 Pointer-ABI 文档以阐明证明缓冲区布局和注册表句柄。

## 节点能力协商
- Handshake 将 `feature_bits.confidential` 与 `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` 一起进行广告。验证者参与需要 `confidential.enabled=true`、`assume_valid=false`、相同的验证者后端标识符和匹配的摘要；不匹配导致与 `HandshakeConfidentialMismatch` 的握手失败。
- 配置仅支持观察者节点的 `assume_valid`：禁用时，遇到机密指令会产生确定性 `UnsupportedInstruction`，而不会出现恐慌；启用后，观察者应用声明的状态增量而不验证证明。
- 如果禁用本地功能，Mempool 将拒绝机密交易。八卦过滤器避免向没有匹配能力的对等方发送屏蔽交易，同时在大小限制内盲目转发未知验证者 ID。

### 揭示修剪和无效保留政策

机密账本必须保留足够的历史记录以证明票据的新鲜度并
重播治理驱动的审计。默认策略，由
`ConfidentialLedger`，是：

- **无效化保留：**保留用过的无效化*最少* `730` 天（24
  几个月）后度过高度，或监管机构规定的窗口（如果更长）。
  运营商可以通过 `confidential.retention.nullifier_days` 扩展窗口。
  比保留窗口年轻的无效符必须保持可通过 Torii 进行查询，因此
  审计员可以证明双花缺席。
- **显示修剪：**透明显示（`RevealConfidential`）修剪
  区块最终确定后立即进行相关票据承诺，但
  消耗的无效符仍受上述保留规则的约束。揭示相关
  事件（`ConfidentialEvent::Unshielded`）记录公开金额、接收者、
  和证明哈希，因此重建历史揭示不需要修剪
  密文。
- **边境检查点：**承诺边境维持滚动检查点
  覆盖 `max_anchor_age_blocks` 和保留窗口中较大的一个。节点
  仅在间隔内的所有无效符到期后才压缩较旧的检查点。
- **过时摘要修复：** 如果 `HandshakeConfidentialMismatch` 到期
  为了消化漂移，操作员应该 (1) 验证无效器保留窗口
  跨集群对齐，(2) 运行 `iroha_cli app confidential verify-ledger` 以
  针对保留的无效集重新生成摘要，并且 (3) 重新部署
  刷新清单。任何过早修剪的无效符都必须从
  在重新加入网络之前进行冷存储。

在操作手册中记录本地覆盖；治理政策延伸
保留窗口必须更新节点配置和归档存储计划
步调一致。

### 驱逐和恢复流程

1. 在拨号过程中，`IrohaNetwork` 会比较公布的功能。任何不匹配都会引发 `HandshakeConfidentialMismatch`；连接关闭，对等点保留在发现队列中，而不会提升为 `Ready`。
2. 故障通过网络服务日志（包括远程摘要和后端）显示，并且 Sumeragi 从未安排对等点进行提案或投票。
3. 操作员通过调整验证者注册表和参数集（`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`）或通过将 `next_conf_features` 与商定的 `activation_height` 暂存来进行修复。一旦摘要匹配，下一次握手就会自动成功。
4. 如果过时的对等点设法广播一个块（例如，通过存档重放），验证器将使用 `BlockRejectionReason::ConfidentialFeatureDigestMismatch` 确定性地拒绝它，从而保持整个网络的账本状态一致。

### 重放安全握手流程

1. 每次出站尝试都会分配新的 Noise/X25519 密钥材料。签名的握手有效负载 (`handshake_signature_payload`) 连接本地和远程临时公钥、Norito 编码的通告套接字地址，以及使用 `handshake_chain_id` 编译时的链标识符。消息在离开节点之前经过 AEAD 加密。
2. 响应方以相反的对等/本地密钥顺序重新计算有效负载，并验证嵌入在 `HandshakeHelloV1` 中的 Ed25519 签名。由于临时密钥和通告的地址都是签名域的一部分，因此针对另一个对等点重放捕获的消息或恢复过时的连接会导致验证失败。
3. 机密功能标志和 `ConfidentialFeatureDigest` 位于 `HandshakeConfidentialMeta` 内部。接收器将元组 `{ enabled, assume_valid, verifier_backend, digest }` 与其本地配置的 `ConfidentialHandshakeCaps` 进行比较；在传输转换为 `Ready` 之前，任何不匹配都会提前退出 `HandshakeConfidentialMismatch`。
4. 操作员必须重新计算摘要（通过 `compute_confidential_feature_digest`）并在重新连接之前使用更新的注册表/策略重新启动节点。宣传旧摘要的节点继续使握手失败，从而防止过时状态重新进入验证器集。
5. 握手成功和失败会更新标准 `iroha_p2p::peer` 计数器（`handshake_failure_count`，错误分类助手），并发出标有远程对等 ID 和摘要指纹的结构化日志条目。监视这些指示器以捕获部署期间的重放尝试或错误配置。## 密钥管理和有效负载
- 每个帐户的密钥派生层次结构：
  - `sk_spend` → `nk`（无效键）、`ivk`（传入查看键）、`ovk`（传出查看键）、`fvk`。
- 加密的票据有效负载使用 AEAD 和 ECDH 派生的共享密钥；可选的审计员查看键可以附加到每个资产策略的输出。
- CLI 添加：`confidential create-keys`、`confidential send`、`confidential export-view-key`、用于解密备忘录的审核工具，以及用于离线生成/检查 Norito 备忘录信封的 `iroha app zk envelope` 帮助程序。 Torii 通过 `POST /v1/confidential/derive-keyset` 公开相同的派生流程，返回十六进制和 base64 形式，以便钱包可以以编程方式获取密钥层次结构。

## Gas、限制和 DoS 控制
- 确定性气体调度：
  - Halo2 (Plonkish)：每个公共输入的基础 `250_000` 气体 + `2_000` 气体。
  - `5` 每个证明字节的 Gas 费用，加上每个无效器 (`300`) 和每个承诺 (`500`) 费用。
  - 操作员可以通过节点配置覆盖这些常量（`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`）；更改在启动时或配置层热重载时传播，并确定性地应用于整个集群。
- 硬限制（可配置的默认值）：
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。超过 `verify_timeout_ms` 的证明会确定性地中止指令（治理选票发出 `proof verification exceeded timeout`，`VerifyProof` 返回错误）。
- 额外配额确保活性：`max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block` 和 `max_public_inputs` 绑​​定块构建器； `reorg_depth_bound` (≥ `max_anchor_age_blocks`) 管理边境检查点保留。
- 运行时执行现在会拒绝超出这些每笔交易或每块限制的交易，发出确定性 `InvalidParameter` 错误并使账本状态保持不变。
- Mempool 在调用验证器之前通过 `vk_id`、证明长度和锚年龄预过滤机密交易以限制资源使用。
- 验证在超时或违反约束时确定性停止；事务因显式错误而失败。 SIMD 后端是可选的，但不会改变 Gas 核算。

### 校准基线和验收门
- **参考平台。** 校准运行必须涵盖以下三个硬件配置文件。未能捕获所有配置文件的运行在审核期间将被拒绝。

  |简介 |建筑| CPU/实例|编译器标志 |目的|
  | ---| ---| ---| ---| ---|
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) 或 Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` |无需向量内在函数即可建立下限值；用于调整后备成本表。 |
  | `baseline-avx2` | `x86_64` |英特尔至强金牌 6430 (24c) |默认发布 |验证 AVX2 路径；检查 SIMD 加速是否保持在中性气体的耐受范围内。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | AWS Graviton3 (c7g.4xlarge) | AWS Graviton3 (c7g.4xlarge)默认发布 |确保 NEON 后端保持确定性并与 x86 计划保持一致。 |

- **基准线束。** 所有气体校准报告必须包含以下内容：
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` 确认确定性夹具。
  - 每当 VM 操作码成本发生变化时，`CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`。

- **修复了随机性。** 在运行工作台之前导出 `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1`，以便 `iroha_test_samples::gen_account_in` 切换到确定性 `KeyPair::from_seed` 路径。线束打印一次`IROHA_CONF_GAS_SEED_ACTIVE=…`；如果变量丢失，审核必须失败。任何新的校准实用程序在引入辅助随机性时都必须继续遵守此环境变量。

- **结果捕获。**
  - 将每个配置文件的标准摘要 (`target/criterion/**/raw.csv`) 上传到发布工件中。
  - 将派生指标（`ns/op`、`gas/op`、`ns/gas`）与使用的 git 提交和编译器版本一起存储在 [机密气体校准分类帐](./confidential-gas-calibration) 中。
  - 维护每个配置文件的最后两条基线；一旦最新的报告得到验证，就删除旧的快照。

- **验收公差。**
  - `baseline-simd-neutral` 和 `baseline-avx2` 之间的气体增量必须保持 ≤ ±1.5%。
  - `baseline-simd-neutral` 和 `baseline-neon` 之间的气体增量必须保持 ≤ ±2.0%。
  - 超过这些阈值的校准建议需要调整时间表或使用 RFC 来解释差异和缓解措施。

- **审核清单。** 提交者负责：
  - 校准日志中包括 `uname -a`、`/proc/cpuinfo` 摘录（模型、步进）和 `rustc -Vv`。
  - 验证工作台输出中回显的 `IROHA_CONF_GAS_SEED`（工作台打印活动种子）。
  - 确保起搏器和机密验证器功能标志镜像生产（使用遥测运行工作台时为 `--features confidential,telemetry`）。

## 配置和操作
- `iroha_config` 获得 `[confidential]` 部分：
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- 遥测发出聚合指标：`confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}` 和 `confidential_policy_transitions_total`，从不暴露明文数据。
- RPC 表面：
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## 测试策略
- 确定性：区块内的随机交易洗牌会产生相同的默克尔根和无效集。
- 重组弹性：用锚点模拟多块重组；无效器保持稳定，陈旧的锚被拒绝。
- Gas 不变量：验证有或没有 SIMD 加速的节点之间相同的 Gas 使用情况。
- 边界测试：大小/gas 上限的证明、最大输入/输出计数、超时执行。
- 生命周期：验证者和参数激活/弃用的治理操作、轮换支出测试。
- 政策 FSM：允许/不允许的转换、待处理的转换延迟以及有效高度附近的内存池拒绝。
- 注册紧急情况：紧急提款将冻结受影响的资产 `withdraw_height`，并随后拒绝证明。
- 能力门控：具有不匹配的 `conf_features` 拒绝块的验证器； `assume_valid=true` 的观察者可以跟上而不影响共识。
- 状态等效：验证者/完整/观察者节点在规范链上产生相同的状态根。
- 负模糊测试：格式错误的证明、过大的有效负载和无效冲突确定性地拒绝。

## 杰出作品
- 对 Halo2 参数集（电路大小、查找策略）进行基准测试，并将结果记录在校准手册中，以便气体/超时默认值可以在下一次 `confidential_assets_calibration.md` 刷新时进行更新。
- 最终确定审计师披露政策和相关的选择性查看 API，一旦治理草案签署，将批准的工作流程连接到 Torii 中。
- 扩展见证加密方案以涵盖多接收者输出和批量备忘录，为 SDK 实施者记录信封格式。
- 委托对电路、注册表和参数轮换程序进行外部安全审查，并将结果存档在内部审计报告旁边。
- 指定审计员支出调节 API 并发布视图密钥范围指南，以便钱包供应商可以实现相同的证明语义。## 实施阶段
1. **阶段 M0 — 停船强化**
   - ✅ 无效器推导现在遵循 Poseidon PRF 设计（`nk`、`rho`、`asset_id`、`chain_id`），并在账本更新中强制执行确定性承诺排序。
   - ✅ 执行强制执行证明大小上限和每笔交易/每块机密配额，拒绝具有确定性错误的超出预算的交易。
   - ✅ P2P 握手通告 `ConfidentialFeatureDigest`（后端摘要 + 注册表指纹），并通过 `HandshakeConfidentialMismatch` 确定性地失败不匹配。
   - ✅ 消除机密执行路径中的恐慌，并为没有匹配能力的节点添加角色门控。
   - ⚪ 强制执行验证者超时预算和边境检查点的重组深度限制。
     - ✅ 执行验证超时预算；超过 `verify_timeout_ms` 的证明现在确定性地失败。
     - ✅ 前沿检查点现在遵循 `reorg_depth_bound`，修剪早于配置窗口的检查点，同时保持确定性快照。
   - 引入 `AssetConfidentialPolicy`、策略 FSM 和铸造/转移/显示指令的执行门。
   - 在块头中提交 `conf_features` 并在注册表/参数摘要出现分歧时拒绝验证者参与。
2. **阶段 M1 — 注册表和参数**
   - 通过治理操作、创世锚定和缓存管理登陆 `ZkVerifierEntry`、`PedersenParams` 和 `PoseidonParams` 注册表。
   - 连接系统调用以要求注册表查找、gas Schedule ID、模式散列和大小检查。
   - 提供加密有效负载格式 v1、钱包密钥派生向量以及用于机密密钥管理的 CLI 支持。
3. **M2 阶段 — 气体与性能**
   - 通过遥测技术实施确定性的 Gas Schedule、每块计数器和基准测试工具（验证延迟、证明大小、内存池拒绝）。
   - 强化多资产工作负载的 CommitmentTree 检查点、LRU 加载和无效索引。
4. **M3 阶段 — 轮换和钱包工具**
   - 实现多参数、多版本证明验收；通过过渡运行手册支持治理驱动的激活/弃用。
   - 提供钱包 SDK/CLI 迁移流程、审核员扫描工作流程和支出核对工具。
5. **M4 阶段 — 审计和运营**
   - 提供审核员关键工作流程、选择性披露 API 和操作手册。
   - 安排外部加密/安全审查并在 `status.md` 中发布调查结果。

每个阶段都会更新路线图里程碑和相关测试，以维持区块链网络的确定性执行保证。