---
lang: zh-hans
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14dbd50875bebd8d9f8c9f03f85cb458c909c9da956a7a7048c9dae8c885b969
source_last_modified: "2026-01-22T16:26:46.496232+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

状态：伴随治理实施任务的草案/草图。在实施过程中形状可能会发生变化。决定论和RBAC政策是规范约束；当提供 `authority` 和 `private_key` 时，Torii 可以签署/提交交易，否则客户端构建并提交到 `/transaction`。

概述
- 所有端点都返回 JSON。对于交易生成流程，响应包括 `tx_instructions` — 一个或多个指令骨架的数组：
  - `wire_id`：指令类型的注册表标识符
  - `payload_hex`：Norito 有效负载字节（十六进制）
- 如果提供了 `authority` 和 `private_key`（或选票 DTO 上的 `private_key`），则 Torii 签署并提交交易，但仍返回 `tx_instructions`。
- 否则，客户端使用其权限和 chain_id 组装 SignedTransaction，然后签名并 POST 到 `/transaction`。
- SDK覆盖范围：
- Python（`iroha_python`）：`ToriiClient.get_governance_proposal_typed`返回`GovernanceProposalResult`（标准化状态/种类字段），`ToriiClient.get_governance_referendum_typed`返回`GovernanceReferendumResult`，`ToriiClient.get_governance_tally_typed`返回`GovernanceTally`， `ToriiClient.get_governance_locks_typed` 返回 `GovernanceLocksResult`，`ToriiClient.get_governance_unlock_stats_typed` 返回 `GovernanceUnlockStats`，`ToriiClient.list_governance_instances_typed` 返回 `GovernanceInstancesPage`，通过 README 使用示例在治理表面上强制执行类型化访问。
- Python 轻量级客户端 (`iroha_torii_client`)：`ToriiClient.finalize_referendum` 和 `ToriiClient.enact_proposal` 返回类型化的 `GovernanceInstructionDraft` 捆绑包（包装 Torii 骨架 `tx_instructions`），避免在脚本组成 Finalize/Enact 流时进行手动 JSON 解析。
- JavaScript (`@iroha/iroha-js`)：`ToriiClient` 表面键入了提案、公投、统计、锁定、解锁统计数据的帮助程序，现在 `listGovernanceInstances(namespace, options)` 加上理事会端点（`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、`governancePersistCouncil`、 `getGovernanceCouncilAudit`），因此 Node.js 客户端可以对 `/v1/gov/instances/{ns}` 进行分页，并在现有合约实例列表旁边驱动 VRF 支持的工作流程。

端点

- 后 `/v1/gov/proposals/deploy-contract`
  - 请求（JSON）：
    {
      “命名空间”：“应用程序”，
      "contract_id": "my.contract.v1",
      “code_hash”：“blake2b32：…”| “…64十六进制”，
      "abi_hash": "blake2b32:…" | “…64十六进制”，
      “abi_版本”：“1”，
      “窗口”：{“下”：12345，“上”：12400}，
      "authority": "i105…?",
      “私钥”：“……？”
    }
  - 响应（JSON）：
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 验证：节点将 `abi_hash` 规范化为提供的 `abi_version` 并拒绝不匹配。对于 `abi_version = "v1"`，预期值为 `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`。

合约 API（部署）
- 后 `/v1/contracts/deploy`
  - 请求：{ "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - 行为：从 IVM 程序体计算 `code_hash`，从标头 `abi_version` 计算 `abi_hash`，然后提交 `RegisterSmartContractCode`（清单）和 `RegisterSmartContractBytes`（完整的 `.to`）字节）代表`authority`。
  - 响应：{“ok”：true，“code_hash_hex”：“…”，“abi_hash_hex”：“…”}
  - 相关：
    - GET `/v1/contracts/code/{code_hash}` → 返回存储的清单
    - 获取 `/v1/contracts/code-bytes/{code_hash}` → 返回 `{ code_b64 }`
- 后 `/v1/contracts/instance`
  - 请求：{ "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 行为：部署提供的字节码并立即通过 `ActivateContractInstance` 激活 `(namespace, contract_id)` 映射。
  - 响应：{“ok”：true，“namespace”：“apps”，“contract_id”：“calc.v1”，“code_hash_hex”：“…”，“abi_hash_hex”：“…”}

别名服务
- 后 `/v1/aliases/voprf/evaluate`
  - 请求：{“blinded_element_hex”：“…”}
  - 响应：{“evaluated_element_hex”：“…128hex”，“后端”：“blake2b512-mock”}
    - `backend` 反映了评估器的实现。当前值：`blake2b512-mock`。
  - 注释：确定性模拟评估器应用带有域分离 `iroha.alias.voprf.mock.v1` 的 Blake2b512。用于测试工具，直到生产 VOPRF 管道通过 Iroha 连接。
  - 错误：十六进制输入格式错误时出现 HTTP `400`。 Torii 返回包含解码器错误消息的 Norito `ValidationFail::QueryFailed::Conversion` 信封。
- 后 `/v1/aliases/resolve`
  - 请求: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - 响应: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - 注意：需要 ISO 桥运行时分段（`[iso_bridge.account_aliases]` 中的 `iroha_config`）。 Torii 通过在查找之前去除空格和大写字母来标准化别名。当别名不存在时返回 404，当 ISO 桥接运行时被禁用时返回 503。
- 后 `/v1/aliases/resolve_index`
  - 请求：{“索引”：0}
  - 响应: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - 注意：别名索引是根据配置顺序（从 0 开始）确定性分配的。客户端可以离线缓存响应，以构建别名证明事件的审计跟踪。

代码 尺寸上限
- 自定义参数：`max_contract_code_bytes` (JSON u64)
  - 控制链上合约代码存储的最大允许大小（以字节为单位）。
  - 默认：16 MiB。当 `.to` 图像长度超过上限并出现不变违规错误时，节点会拒绝 `RegisterSmartContractBytes`。
  - 运营商可以通过提交 `SetParameter(Custom)` 和 `id = "max_contract_code_bytes"` 以及数字有效负载来进行调整。

- 后 `/v1/gov/ballots/zk`
  - 请求：{ "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 响应：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}
  - 注意事项：
    - 当电路的公共输入包括 `owner`、`amount` 和 `duration_blocks`，并且证明根据配置的 VK 进行验证时，节点使用 `owner` 创建或扩展 `election_id` 的治理锁。方向保持隐藏（`unknown`）；仅更新金额/到期日。重新投票是单调的：金额和到期日仅增加（节点应用 max(amount, prev.amount) 和 max(expiry, prev.expiry)）。
    - 尝试缩减金额或到期的 ZK 重新投票会被服务器端拒绝，并带有 `BallotRejected` 诊断。
    - 合约执行必须在排队 `SubmitBallot` 之前调用 `ZK_VOTE_VERIFY_BALLOT`；主机强制执行一次性锁存。

- 后 `/v1/gov/ballots/plain`
  - 请求：{ "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 响应：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}
  - 注意：重新投票只能延长——新的投票不能减少现有锁定的数量或到期时间。 `owner`必须等于交易权限。最短持续时间为 `conviction_step_blocks`。

- 后 `/v1/gov/finalize`
  - 请求：{“referendum_id”：“r1”，“proposal_id”：“…64hex”，“authority”：“i105…？”，“private_key”：“…？” }
  - 响应：{ "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - 链上效应（当前脚手架）：制定已批准的部署提案会插入由 `code_hash` 键入的最小 `ContractManifest` 和预期的 `abi_hash`，并将提案标记为已实施。如果 `code_hash` 的清单已存在且具有不同的 `abi_hash`，则颁布将被拒绝。
  - 注意事项：
    - 对于ZK选举，合约路径必须在执行`FinalizeElection`之前调用`ZK_VOTE_VERIFY_TALLY`；主机强制执行一次性锁存。 `FinalizeReferendum` 在选举计票最终确定之前拒绝 ZK 公投。
    - 在 `h_end` 处自动关闭仅针对普通公投发出批准/拒绝； ZK 公投保持关闭状态，直到提交最终计票并执行 `FinalizeReferendum`。
    - 投票率检查仅使用批准+拒绝；弃权不计入投票率。

- 后 `/v1/gov/enact`
  - 请求：{“proposal_id”：“…64hex”，“preimage_hash”：“…64hex？”，“window”：{“lower”：0，“upper”：0}？，“authority”：“i105…？”，“private_key”：“…？” }
  - 响应：{ "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注：Torii在提供`authority`/`private_key`时提交签名交易；否则，它会返回一个框架供客户签名和提交。原像是可选的并且当前是信息性的。

- 获取 `/v1/gov/proposals/{id}`
  - 路径 `{id}`：提案 ID 十六进制（64 个字符）
  - 响应：{“找到”：布尔，“建议”：{…}？ }

- 获取 `/v1/gov/locks/{rid}`
  - 路径 `{rid}`：公投 ID 字符串
  - 响应：{“found”：bool，“referendum_id”：“rid”，“locks”：{…}？ }

- 获取 `/v1/gov/council/current`
  - 响应：{ "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注释：返回存在的持久理事会；否则使用配置的权益资产和阈值得出确定性后备（镜像 VRF 规范，直到实时 VRF 证明保留在链上）。- POST `/v1/gov/council/derive-vrf`（功能：gov_vrf）
  - 请求：{“committee_size”：21，“epoch”：123？ , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 行为：根据源自 `chain_id`、`epoch` 和最新区块哈希信标的规范输入验证每个候选者的 VRF 证明；按输出字节 desc 进行排序；返回顶部 `committee_size` 成员。不坚持。
  - 响应：{“epoch”：N，“members”：[{“account_id”：“…”} …]，“total_candidates”：M，“verified”：K }
  - 注：Normal = G1 中的 pk，G2 中的证明（96 字节）。小 = G2 中的 pk，G1 中的证明（48 字节）。输入是域分隔的，包括 `chain_id`。

### 治理默认值 (iroha_config `gov.*`)

当不存在持久名册时，Torii 使用的理事会后备通过 `iroha_config` 进行参数化：

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

等效环境覆盖：

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` 限制没有持续存在理事会时返回的后备成员的数量，`parliament_term_blocks` 定义用于种子派生的纪元长度 (`epoch = floor(height / term_blocks)`)，`parliament_min_stake` 强制执行资格资产的最小权益（以最小单位），`parliament_eligibility_asset_id`选择构建候选集时扫描的资产余额。

治理 VK 验证无法绕过：选票验证始终需要具有内联字节的 `Active` 验证密钥，并且环境不得依赖仅测试切换来跳过验证。

RBAC
- 链上执行需要权限：
  - 提案：`CanProposeContractDeployment{ contract_id }`
  - 选票：`CanSubmitGovernanceBallot{ referendum_id }`
  - 颁布：`CanEnactGovernance`
  - 理事会管理（未来）：`CanManageParliament`

受保护的命名空间
- 自定义参数 `gov_protected_namespaces`（JSON 字符串数组）启用部署到列出的命名空间的准入门控。
- 客户端必须包含事务元数据密钥才能针对受保护的命名空间进行部署：
  - `gov_namespace`：目标命名空间（例如，`"apps"`）
  - `gov_contract_id`：命名空间内的逻辑合约ID
- `gov_manifest_approvers`：验证者帐户 ID 的可选 JSON 数组。当通道清单声明法定人数大于 1 时，准入需要交易权限加上列出的帐户来满足清单法定人数。
- 遥测通过 `governance_manifest_admission_total{result}` 公开整体准入计数器，以便操作员可以区分成功准入与 `missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected` 和 `runtime_hook_rejected` 路径。
- 遥测通过 `governance_manifest_quorum_total{outcome}`（值 `satisfied` / `rejected`）显示执行路径，以便操作员可以审核缺失的批准。
- 通道强制执行在其清单中发布的命名空间允许列表。任何设置 `gov_namespace` 的事务都必须提供 `gov_contract_id`，并且命名空间必须出现在清单的 `protected_namespaces` 集中。启用保护后，没有此元数据的 `RegisterSmartContractCode` 提交将被拒绝。
- 准入强制执行元组 `(namespace, contract_id, code_hash, abi_hash)` 存在已颁布的治理提案；否则验证失败并出现 NotPermissed 错误。

运行时升级挂钩
- 通道清单可以声明 `hooks.runtime_upgrade` 来控制运行时升级指令（`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`）。
- 钩子字段：
  - `allow`（布尔值，默认为 `true`）：当 `false` 时，所有运行时升级指令都会被拒绝。
  - `require_metadata`（布尔值，默认 `false`）：需要 `metadata_key` 指定的事务元数据条目。
  - `metadata_key`（字符串）：钩子强制执行的元数据名称。当需要元数据或存在允许列表时，默认为 `gov_upgrade_id`。
  - `allowed_ids`（字符串数组）：可选的元数据值白名单（修剪后）。当提供的值未列出时拒绝。
- 当挂钩存在时，队列准入会在事务进入队列之前强制执行元数据策略。缺少元数据、空白值或白名单之外的值会产生确定性 `NotPermitted` 错误。
- 遥测通过 `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` 跟踪执法结果。
- 满足挂钩的交易必须包含元数据 `gov_upgrade_id=<value>`（或清单定义的密钥）以及清单法定人数所需的任何验证器批准。

便利端点
- POST `/v1/gov/protected-namespaces` — 直接在节点上应用 `gov_protected_namespaces`。
  - 请求：{“命名空间”：[“应用程序”，“系统”]}
  - 响应：{“ok”：true，“applied”：1}
  - 注释：用于管理/测试；如果已配置，则需要 API 令牌。对于生产，更喜欢提交带有 `SetParameter(Custom)` 的签名交易。

CLI 助手
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 获取命名空间的合约实例并交叉检查：
    - Torii 存储每个 `code_hash` 的字节码，并且其 Blake2b-32 摘要与 `code_hash` 匹配。
    - 存储在 `/v1/contracts/code/{code_hash}` 下的清单报告匹配 `code_hash` 和 `abi_hash` 值。
    - `(namespace, contract_id, code_hash, abi_hash)` 存在已颁布的治理提案，该提案是通过节点使用的相同提案 ID 散列得出的。
  - 输出一份 JSON 报告，其中每个合同包含 `results[]`（问题、清单/代码/提案摘要）以及一行摘要（除非被抑制）（`--no-summary`）。
  - 对于审核受保护的命名空间或验证治理控制的部署工作流程很有用。
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - 发出将部署提交到受保护的命名空间时使用的 JSON 元数据框架，包括用于满足清单仲裁规则的可选 `gov_manifest_approvers`。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — 当 `min_bond_amount > 0` 时需要锁定提示，并且任何提供的提示集必须包括 `owner`、`amount` 和 `duration_blocks`。
  - 验证规范帐户 ID，规范化 32 字节无效提示，并将提示合并到 `public_inputs_json`（使用 `--public <path>` 进行额外覆盖）。
  - 无效符源自证明承诺（公共输入）加上 `domain_tag`、`chain_id` 和 `election_id`； `--nullifier` 已根据提供的证明进行验证。
  - 单行摘要现在显示从编码的 `CastZkBallot` 派生的确定性 `fingerprint=<hex>` 以及任何解码的提示（`owner`、`amount`、`duration_blocks`、`direction`（如果提供））。
  - CLI 响应使用 `payload_fingerprint_hex` 加上解码字段来注释 `tx_instructions[]`，以便下游工具可以验证骨架，而无需重新实现 Norito 解码。
  - 一旦电路公开相同的值，提供锁定提示允许节点为 ZK 选票发出 `LockCreated`/`LockExtended` 事件。
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` 接受规范的 I105 文字；可选的 `@<domain>` 后缀仅是路由提示。
  - 别名 `--lock-amount`/`--lock-duration-blocks` 镜像 ZK 标志名称以实现脚本奇偶校验。
  - 摘要输出通过包含编码的指令指纹和人类可读的选票字段（`owner`、`amount`、`duration_blocks`、`direction`）来镜像 `vote --mode zk`，在签署框架之前提供快速确认。

实例列表
- GET `/v1/gov/instances/{ns}` — 列出命名空间的活动合约实例。
  - 查询参数：
    - `contains`：按 `contract_id` 的子字符串过滤（区分大小写）
    - `hash_prefix`：按 `code_hash_hex`（小写）的十六进制前缀过滤
    - `offset`（默认 0）、`limit`（默认 100，最大 10_000）
    - `order`：`cid_asc`（默认）、`cid_desc`、`hash_asc`、`hash_desc` 之一
  - 响应：{ "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK 帮助程序：`ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) 或 `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

解锁扫码（操作员/审计）
- 获取 `/v1/gov/unlocks/stats`
  - 响应：{ "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - 注：`last_sweep_height` 反映了过期锁被清除和持久化的最新区块高度。 `expired_locks_now` 是通过扫描 `expiry_height <= height_current` 的锁定记录来计算的。
- 后 `/v1/gov/ballots/zk-v1`
  - 请求（v1 样式 DTO）：
    {
      "权威": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      “后端”：“halo2/ipa”，
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex？",
      "owner": "i105…?", // 规范 AccountId（I105 文字）
      "金额": "100？",
      “duration_blocks”：6000？，
      "direction": "赞成|反对|弃权？",
      "nullifier": "blake2b32:…64hex？"
    }
  - 响应：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}- POST `/v1/gov/ballots/zk-v1/ballot-proof`（特征：`zk-ballot`）
  - 直接接受 `BallotProof` JSON 并返回 `CastZkBallot` 骨架。
  - 要求：
    {
      "权威": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      “选票”：{
        “后端”：“halo2/ipa”，
        "envelope_bytes": "AAECAwQ=", // ZK1 或 H2* 容器的 base64
        "root_hint": null, // 可选的 32 字节十六进制字符串（资格根）
        "owner": null, // 可选的规范 AccountId（I105 文字）
        "nullifier": null, // 可选的 32 字节十六进制字符串（nullifier 提示）
        "amount": "100", // 可选的锁定金额提示（十进制字符串）
        "duration_blocks": 6000, // 可选的锁定持续时间提示
        "direction": "Aye" // 可选方向提示
      }
    }
  - 回应：
    {
      “好的”：正确的，
      “已接受”：正确，
      "reason": "构建交易骨架",
      “tx_指令”：[
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - 注意事项：
    - 服务器将选票中的可选 `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` 映射到 `public_inputs_json` `CastZkBallot`。
    - 信封字节被重新编码为指令有效负载的 base64。
    - 当 Torii 提交选票时，响应 `reason` 更改为 `submitted transaction`。
    - 仅当启用 `zk-ballot` 功能时，此端点才可用。

CastZkBallot验证路径
- `CastZkBallot` 解码提供的 Base64 证明并拒绝空或格式错误的有效负载（`BallotRejected` 和 `invalid or empty proof`）。
- 如果提供 `public_inputs_json`，则它必须是 JSON 对象；非对象有效负载被拒绝。
- 主机从公投 (`vk_ballot`) 或治理默认值解析选票验证密钥，并要求记录存在，为 `Active`，并携带内联字节。
- 存储的验证密钥字节使用 `hash_vk` 重新散列；任何承诺不匹配都会在验证之前中止执行，以防止注册表项被篡改（`BallotRejected` 与 `verifying key commitment mismatch`）。
- 证明字节通过 `zk::verify_backend` 分派到注册后端；无效的转录本显示为 `BallotRejected` 和 `invalid proof`，并且指令确定性失败。
- 证明必须将投票承诺和资格根源公开为公共投入；根必须与选举的 `eligible_root` 匹配，并且派生的无效符必须与任何提供的提示匹配。
- 成功的证明发出 `BallotAccepted`；重复的无效符、过时的资格根或锁定回归继续产生本文档前​​面描述的现有拒绝原因。

## 验证者的不当行为和联合共识

### 削减和监禁工作流程

每当验证者违反协议时，共识就会发出 Norito 编码的 `Evidence`。每个有效负载都会落在内存中的 `EvidenceStore` 中，如果看不见，则会具体化到 WSV 支持的 `consensus_evidence` 映射中。早于 `sumeragi.npos.reconfig.evidence_horizon_blocks`（默认 `7 200` 块）的记录将被拒​​绝，因此存档仍受限制，但会为操作员记录拒绝。范围内的证据遵循联合共识暂存规则（`mode_activation_height requires next_mode to be set in the same block`）、激活延迟（`sumeragi.npos.reconfig.activation_lag_blocks`，默认 `1`）和削减延迟（`sumeragi.npos.reconfig.slashing_delay_blocks`，默认 `259200`），因此治理可以在处罚之前取消处罚。

公认的犯罪行为一对一映射到 `EvidenceKind`；判别式是稳定的并且由数据模型强制执行：

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** — 验证器为同一 `(phase,height,view,epoch)` 元组签署了冲突的哈希值。
- **InvalidQc** — 聚合器传播了形状未通过确定性检查的提交证书（例如，空签名者位图）。
- **InvalidProposal** — 领导者提出了一个未通过结构验证的区块（例如，破坏了锁链规则）。
- **审查** - 签名的提交收据显示从未提议/提交的交易。

VRF 处罚在 `activation_lag_blocks` 后自动执行（违法者将被监禁）。除非治理取消惩罚，否则共识削减仅在 `slashing_delay_blocks` 窗口之后应用。

操作员和工具可以通过以下方式检查和重新广播有效负载：

- Torii：`GET /v1/sumeragi/evidence` 和 `GET /v1/sumeragi/evidence/count`。
- CLI：`iroha ops sumeragi evidence list`、`… count` 和 `… submit --evidence-hex <payload>`。

治理必须将证据字节视为规范证明：

1. **在有效负载过期之前收集**。将原始 Norito 字节与高度/视图元数据一起存档。
2. **如果需要取消**，在 `slashing_delay_blocks` 失效之前提交带有证据负载的 `CancelConsensusEvidencePenalty`；该记录标记为 `penalty_cancelled` 和 `penalty_cancelled_at_height`，并且不适用削减。
3. **通过将有效负载嵌入公投或 sudo 指令（例如 `Unregister::peer`）来实施惩罚**。执行重新验证有效负载；格式错误或过时的证据将被确定性地拒绝。
4. **安排后续拓扑**，以便有问题的验证者无法立即重新加入。具有更新名册的典型流队列 `SetParameter(Sumeragi::NextMode)` 和 `SetParameter(Sumeragi::ModeActivationHeight)`。
5. 通过 `/v1/sumeragi/evidence` 和 `/v1/sumeragi/status` **审核结果**，以确保证据反驳取得进展并由治理部门实施删除。

### 联合共识测序

联合共识保证即将发出的验证器集在新集开始提议之前最终确定边界块。运行时通过配对参数强制执行规则：

- `SumeragiParameter::NextMode` 和 `SumeragiParameter::ModeActivationHeight` 必须在**同一块**中提交。 `mode_activation_height` 必须严格大于进行更新的区块高度，提供至少一个区块的滞后。
- `sumeragi.npos.reconfig.activation_lag_blocks`（默认 `1`）是防止零延迟切换的配置保护：
- `sumeragi.npos.reconfig.slashing_delay_blocks`（默认 `259200`）延迟共识削减，以便治理可以在处罚实施之前取消处罚。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- 运行时和 CLI 通过 `/v1/sumeragi/params` 和 `iroha --output-format text ops sumeragi params` 公开分阶段参数，以便操作员可以确认激活高度和验证器名册。
- 治理自动化应始终：
  1. 最终确定有证据支持的移除（或恢复）决定。
  2. 使用 `mode_activation_height = h_current + activation_lag_blocks` 对后续重新配置进行排队。
  3. 监视 `/v1/sumeragi/status`，直到 `effective_consensus_mode` 翻转到预期高度。

任何轮换验证器或应用削减的脚本**不得**尝试零延迟激活或省略交接参数；此类交易将被拒绝，并使网络保持先前的模式。

## 遥测表面

- Prometheus 指标导出治理活动：
  - `governance_proposals_status{status}`（仪表）按状态跟踪提案计数。
  - 当受保护的命名空间准入允许或拒绝部署时，`governance_protected_namespace_total{outcome}`（计数器）递增。
  - `governance_manifest_activations_total{event}`（计数器）记录清单插入（`event="manifest_inserted"`）和命名空间绑定（`event="instance_bound"`）。
- `/status` 包括一个 `governance` 对象，该对象镜像提案计数、报告受保护命名空间总数，并列出最近的清单激活（命名空间、合约 ID、代码/ABI 哈希、块高度、激活时间戳）。操作员可以轮询此字段以确认已更新清单的制定以及已强制执行受保护的命名空间门。
- Grafana 模板 (`docs/source/grafana_governance_constraints.json`) 和
  `telemetry.md` 中的遥测操作手册展示了如何连接卡住警报
  提案、缺少清单激活或意外的受保护命名空间
  运行时升级期间的拒绝。