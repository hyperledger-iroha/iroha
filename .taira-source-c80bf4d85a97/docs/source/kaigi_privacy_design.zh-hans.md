---
lang: zh-hans
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kaigi 隐私和中继设计

本文档捕捉了引入零知识的以隐私为中心的演变
参与证明和洋葱式中继，而不牺牲确定性或
账本可审计性。

# 概述

该设计跨越三层：

- **名册隐私** – 在链上隐藏参与者身份，同时保持主持人权限和计费一致。
- **使用不透明** – 允许主机记录计量使用情况，而无需公开披露每个段的详细信息。
- **覆盖中继** – 通过多跳对等点路由传输数据包，因此网络观察者无法了解哪些参与者进行通信。

所有添加内容均保持 Norito 优先，在 ABI 版本 1 下运行，并且必须跨异构硬件确定性执行。

# 目标

1. 使用零知识证明接纳/驱逐参与者，这样账本就不会暴露原始账户 ID。
2. 保持强大的会计保证：每个加入、离开和使用事件仍然必须确定性地协调。
3. 提供可选的中继清单，描述控制/数据通道的洋葱路由，并可以在链上进行审核。
4. 对于不需要隐私的部署，保持后备（完全透明的名册）可操作。

# 威胁模型总结

- **对手：** 网络观察者 (ISP)、好奇的验证者、恶意中继运营商和半诚实的主机。
- **受保护的资产：** 参与者身份、参与时间、每个分段的使用/计费详细信息以及网络路由元数据。
- **假设：** 主机仍然了解链外的真实参与者集；账本节点确定性地验证证明；覆盖中继不受信任，但速率有限； HPKE 和 SNARK 原语已存在于代码库中。

# 数据模型更改

所有类型都位于 `iroha_data_model::kaigi` 中。

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` 获得以下字段：

- `roster_commitments: Vec<KaigiParticipantCommitment>` – 启用隐私模式后，替换公开的 `participants` 列表。经典部署可以在迁移过程中保持两者均已填充。
- `nullifier_log: Vec<KaigiParticipantNullifier>` – 严格仅附加，由滚动窗口限制以保持元数据有限。
- `room_policy: KaigiRoomPolicy` – 选择会话的查看者身份验证立场（`Public` 房间镜像只读中继；`Authenticated` 房间在出口转发数据包之前需要查看者票证）。
- `relay_manifest: Option<KaigiRelayManifest>` – 使用 Norito 编码的结构化清单，因此跃点、HPKE 密钥和权重无需 JSON 填充程序即可保持规范。
- `privacy_mode: KaigiPrivacyMode` 枚举（见下文）。

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` 接收匹配的可选字段，以便主机可以在创建时选择隐私。


- 字段使用 `#[norito(with = "...")]` 帮助程序强制执行规范编码（整数采用小端序，按位置排序跃点）。
- `KaigiRecord::from_new` 将新向量播种为空并复制任何提供的中继清单。

# 指令表面变化

## 演示快速入门助手

对于临时演示和互操作性测试，CLI 现在公开
`iroha kaigi quickstart`。它：- 重用 CLI 配置（域 `wonderland` + 帐户），除非通过 `--domain`/`--host` 覆盖。
- 当省略 `--call-name` 时生成基于时间戳的调用名称，并针对活动 Torii 端点提交 `CreateKaigi`。
- 可以选择自动加入主机 (`--auto-join-host`)，以便观看者可以立即连接。
- 发出一个 JSON 摘要，其中包含 Torii URL、呼叫标识符、隐私/房间策略、准备复制的加入命令以及假脱机路径测试人员应监视的内容（例如，`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`）。使用 `--summary-out path/to/file.json` 持久保存 blob。

此帮助器**不会**取代对正在运行的 `irohad --sora` 节点的需求：隐私路由、假脱机文件和中继清单仍然由账本支持。当为外部团体提供临时房间时，它只是简单地修剪了样板文件。

### 单命令演示脚本

为了获得更快的路径，有一个配套脚本：`scripts/kaigi_demo.sh`。
它为您执行以下操作：

1. 将捆绑的 `defaults/nexus/genesis.json` 签名为 `target/kaigi-demo/genesis.nrt`。
2. 启动带有签名块的 `irohad --sora`（日志位于 `target/kaigi-demo/irohad.log` 下）并等待 Torii 公开 `http://127.0.0.1:8080/status`。
3. 运行 `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`。
4. 打印 JSON 摘要的路径以及假脱机目录 (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`)，以便您可以与外部测试人员共享。

环境变量：

- `TORII_URL` — 覆盖 Torii 端点进行轮询（默认 `http://127.0.0.1:8080`）。
- `RUN_DIR` — 覆盖工作目录（默认 `target/kaigi-demo`）。

按 `Ctrl+C` 停止演示；脚本中的陷阱会自动终止 `irohad`。假脱机文件和摘要保留在磁盘上，以便您可以在进程退出后移交工件。

## `CreateKaigi`

- 根据主机权限验证 `privacy_mode`。
- 如果提供了 `relay_manifest`，则强制执行 ≥3 跳、非零权重、HPKE 密钥存在和唯一性，以便链上清单保持可审核性。
- 验证来自 SDK/CLI 的 `room_policy` 输入（`public` 与 `authenticated`）并将其传播到 SoraNet 配置，以便中继缓存公开正确的 GAR 类别（`stream.kaigi.public` 与 `stream.kaigi.authenticated`）。主机通过 `iroha kaigi create --room-policy …`、JS SDK 的 `roomPolicy` 字段进行连接，或者在 Swift 客户端在提交之前组装 Norito 有效负载时设置 `room_policy` 来连接。
- 存储空的承诺/无效日志。

## `JoinKaigi`

参数：

- `proof: ZkProof`（Norito 字节包装器） - Groth16 证明证明调用者知道 `(account_id, domain_salt)`，其 Poseidon 哈希等于提供的 `commitment`。
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – 下一跳的可选每参与者覆盖。

执行步骤：

1. 如果 `record.privacy_mode == Transparent`，则回退到当前行为。
2. 根据电路注册表项 `KAIGI_ROSTER_V1` 验证 Groth16 证明。
3. 确保 `nullifier` 未出现在 `record.nullifier_log` 中。
4. 追加承诺/无效条目；如果提供了 `relay_hint`，则修补该参与者的中继清单视图（仅存储在内存中会话状态中，而不存储在链上）。## `LeaveKaigi`

透明模式符合当前逻辑。

私有模式需要：

1. 证明调用者知道 `record.roster_commitments` 中的承诺。
2. 无效器更新证明一次性休假。
3. 删除承诺/无效条目。审核保留固定保留窗口的墓碑，以避免结构泄漏。

## `RecordKaigiUsage`

通过以下方式扩展有效负载：

- `usage_commitment: FixedBinary<32>` – 对原始使用元组的承诺（持续时间、gas、段 ID）。
- 可选的 ZK 证明，验证增量是否与账本外提供的加密日志相匹配。

主办方仍然可以提交透明的总计；隐私模式仅使承诺字段成为强制性的。

# 验证和电路

- `iroha_core::smartcontracts::isi::kaigi::privacy` 现在执行完整名单
  默认验证。它解析 `zk.kaigi_roster_join_vk` （连接）和
  `zk.kaigi_roster_leave_vk`（离开）来自配置，
  在WSV中查找对应的`VerifyingKeyRef`（确保记录是
  `Active`，后端/电路标识符匹配，并且承诺一致），费用
  字节计费，并调度到配置的ZK后端。
- `kaigi_privacy_mocks` 功能保留了确定性存根验证器，因此
  单元/集成测试和受限 CI 作业可以在没有 Halo2 后端的情况下运行。
  生产版本必须禁用该功能以强制执行真实的证明。
- 如果在某个设备上启用了 `kaigi_privacy_mocks`，则该包会发出编译时错误
  非测试、非 `debug_assertions` 构建，防止意外发布二进制文件
  随存根一起运输。
- 运营商需要（1）注册通过治理设置的名册验证者，以及
  (2) 设置 `zk.kaigi_roster_join_vk`、`zk.kaigi_roster_leave_vk`，以及
  `zk.kaigi_usage_vk` 位于 `iroha_config` 中，以便主机可以在运行时解析它们。
  在密钥出现之前，隐私加入、离开和使用调用都会失败
  确定性地。
- `crates/kaigi_zk` 现在提供用于名册加入/离开和使用的 Halo2 电路
  与可重复使用压缩机一起的承诺（`commitment`、`nullifier`、
  `usage`）。花名册电路暴露了 Merkle 根（四个小端字节序）
  64 位肢体）作为额外的公共输入，以便主机可以交叉检查证明
  在验证之前针对存储的名册根。使用承诺是
  由 `KaigiUsageCommitmentCircuit` 强制执行，它与 `(duration、gas、
  段）`到账本哈希。
- `Join` 电路输入：`(commitment, nullifier, domain_salt)` 和专用
  `(account_id)`。公共输入包括 `commitment`、`nullifier` 和
  名册承诺树的 Merkle 根的四个分支（名册
  仍然处于链外，但根被绑定到转录本中）。
- 确定性：我们修复了 Poseidon 参数、电路版本和索引
  注册表。任何更改都会使 `KaigiPrivacyMode` 变为 `ZkRosterV2` 且匹配
  测试/黄金文件。

# 洋葱路由覆盖

## 中继注册- 中继自注册为域元数据条目 `kaigi_relay::<relay_id>`，包括 HPKE 密钥材料和带宽类别。
- `RegisterKaigiRelay` 指令将描述符保留在域元数据中，发出 `KaigiRelayRegistered` 摘要（具有 HPKE 指纹和带宽类别），并且可以重新调用以确定性地轮换密钥。
- 治理通过域元数据 (`kaigi_relay_allowlist`) 管理许可名单，并在接受新路径之前中继注册/清单更新强制执行成员资格。

## 清单创建

- 主机从可用中继构建多跳路径（最小长度为 3）。清单对 AccountId 序列和加密分层信封所需的 HPKE 公钥进行编码。
- 链上存储的 `relay_manifest` 包含跳描述符和过期时间（Norito 编码的 `KaigiRelayManifest`）；实际的临时密钥和每个会话的偏移量使用 HPKE 在账本外进行交换。

## 信令与媒体

- SDP/ICE 交换通过 Kaigi 元数据继续进行，但逐跳加密。验证器只能看到 HPKE 密文和标头索引。
- 媒体数据包使用带有密封有效负载的 QUIC 通过中继传输。每跳解密一层，获知下一跳地址；最终接收者在剥离所有层后获得媒体流。

## 故障转移

- 客户端通过 `ReportKaigiRelayHealth` 指令监控中继运行状况，该指令在域元数据 (`kaigi_relay_feedback::<relay_id>`) 中保留签名反馈，广播 `KaigiRelayHealthUpdated`，并允许治理/主机推断当前可用性。当中继失败时，主机会发出更新的清单并记录 `KaigiRelayManifestUpdated` 事件（见下文）。
- 主机通过 `SetKaigiRelayManifest` 指令在账本上应用清单更改，该指令会替换存储的路径或完全清除它。 Clearing 使用 `hop_count = 0` 发出摘要，以便操作员可以观察返回到直接路由的转换。
- Prometheus 指标（`kaigi_relay_registered_total`、`kaigi_relay_registration_bandwidth_class`、`kaigi_relay_manifest_updates_total`、`kaigi_relay_manifest_hop_count`、`kaigi_relay_health_reports_total`、`kaigi_relay_health_state`、`kaigi_relay_failover_total`、 `kaigi_relay_failover_hop_count`）现在可以在操作员仪表板上显示继电器流失、运行状况和故障转移节奏。

# 活动

扩展 `DomainEvent` 变体：

- `KaigiRosterSummary` – 发布匿名计数和当前名单
  每当花名册发生变化时，root（透明模式下 root 为 `None`）。
- `KaigiRelayRegistered` – 每当创建或更新中继注册时发出。
- `KaigiRelayManifestUpdated` – 当中继清单更改时发出。
- `KaigiRelayHealthUpdated` – 当主机通过 `ReportKaigiRelayHealth` 提交中继运行状况报告时发出。
- `KaigiUsageSummary` – 在每个使用段后发出，仅公开总计。

事件以 Norito 序列化，仅公开承诺哈希值和计数。CLI 工具 (`iroha kaigi …`) 包装每个 ISI，以便操作员可以注册会话，
提交名册更新、报告中继运行状况并记录使用情况，无需手工制作交易。
中继清单和隐私证明从传递的 JSON/hex 文件中加载
CLI 的正常提交路径，使编写合约脚本变得简单
进入暂存环境。

# 天然气核算

- `crates/iroha_core/src/gas.rs` 中的新常量：
  - `BASE_KAIGI_JOIN_ZK`、`BASE_KAIGI_LEAVE_ZK` 和 `BASE_KAIGI_USAGE_ZK`
    根据 Halo2 验证时间进行校准（名册的 ≈1.6 毫秒）
    加入/离开，在 Apple M2 Ultra 上使用时约为 1.2 毫秒）。附加费继续
    通过 `PER_KAIGI_PROOF_BYTE` 缩放证明字节大小。
- `RecordKaigiUsage` 承诺根据承诺规模和证明验证支付额外费用。
- 校准工具将重用具有固定种子的机密资产基础设施。

# 测试策略

- 单元测试验证 Norito 编码/解码 `KaigiParticipantCommitment`、`KaigiRelayManifest`。
- JSON 视图的黄金测试确保规范排序。
- 集成测试旋转迷你网络（参见
  当前覆盖范围为 `crates/iroha_core/tests/kaigi_privacy.rs`）：
  - 使用模拟证明的私有加入/离开周期（功能标志 `kaigi_privacy_mocks`）。
  - 通过元数据事件传播的中继清单更新。
- Trybuild UI 测试涵盖主机配置错误（例如，隐私模式下缺少中继清单）。
- 在受限环境中运行单元/集成测试时（例如 Codex
  沙箱），导出 `NORITO_SKIP_BINDINGS_SYNC=1` 以绕过 Norito 绑定
  由 `crates/norito/build.rs` 强制执行同步检查。

# 迁移计划

1. ✅ 在 `KaigiPrivacyMode::Transparent` 默认值后面添加数据模型。
2. ✅ 线控双路验证：量产禁用`kaigi_privacy_mocks`，
   解析`zk.kaigi_roster_vk`，并运行真实的信封验证；测试可以
   仍然启用确定性存根的功能。
3. ✅ 推出专用 `kaigi_zk` Halo2 板条箱、校准气体和有线
   集成覆盖率以端到端运行真实的证明（模拟现在仅供测试）。
4. ⬜ 一旦所有消费者都理解承诺，就弃用透明的 `participants` 向量。

# 开放式问题

- 定义 Merkle 树持久化策略：链上 vs 链下（当前倾向：具有链上根承诺的链下树）。 *（在 KPG-201 中跟踪。）*
- 确定中继清单是否应支持多路径（同时冗余路径）。 *（在 KPG-202 中跟踪。）*
- 澄清中继声誉的治理——我们需要削减还是只是软禁令？ *（在 KPG-203 中跟踪。）*

在生产中启用 `KaigiPrivacyMode::ZkRosterV1` 之前应解决这些问题。