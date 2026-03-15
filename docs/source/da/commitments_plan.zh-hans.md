---
lang: zh-hans
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T14:45:02.095688+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus 数据可用性承诺计划 (DA-3)

_起草时间：2026-03-25 — 所有者：核心协议工作组/智能合约团队/存储团队_

DA-3 扩展了 Nexus 块格式，因此每个通道都嵌入确定性记录
描述 DA-2 接受的 blob。本注释捕获规范数据
结构、块管道钩子、轻客户端证明和 Torii/RPC 表面
必须在验证者在准入期间依赖 DA 承诺之前落地
治理检查。所有有效负载均采用 Norito 编码；没有 SCALE 或临时 JSON。

## 目标

- 携带每个 blob 的承诺（块根 + 清单哈希 + 可选的 KZG
  每个 Nexus 块内的承诺），以便同行可以重建可用性
  状态，无需咨询账外存储。
- 提供确定性的成员资格证明，以便轻客户端可以验证
  清单哈希在给定块中最终确定。
- 公开 Torii 查询 (`/v1/da/commitments/*`) 和证据，让中继，
  SDK 和治理自动化审计可用性，无需重放每个
  块。
- 通过线程新的来保持现有的 `SignedBlockWire` 信封规范
  通过 Norito 元数据头和块哈希派生的结构。

## 范围概述

1. **`iroha_data_model::da::commitment` plus 块中的数据模型添加**
   `iroha_data_model::block` 中的标头发生变化。
2. **执行器挂钩**，因此 `iroha_core` 摄取 Torii 发出的 DA 收据
   （`crates/iroha_core/src/queue.rs` 和 `crates/iroha_core/src/block.rs`）。
3. **持久化/索引**，以便 WSV 可以快速回答承诺查询
   （`iroha_core/src/wsv/mod.rs`）。
4. **Torii RPC 添加**，用于列出/查询/证明端点
   `/v1/da/commitments`。
5. **集成测试 + 夹具** 验证线路布局和验证流程
   `integration_tests/tests/da/commitments.rs`。

## 1. 数据模型添加

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` 重用下使用的现有 48 字节点
  `iroha_crypto::kzg`。 Merkle 通道将其留空；现在为 `kzg_bls12_381` 通道
  接收从块根派生的确定性 BLAKE3-XOF 承诺，并且
  存储票证，使块哈希在没有外部证明的情况下保持稳定。
- `proof_scheme`源自车道目录； Merkle 通道拒绝杂散 KZG
  有效负载，而 `kzg_bls12_381` 通道需要非零 KZG 承诺。
- `proof_digest` 预计 DA-5 PDP/PoTR 集成，因此记录相同
  枚举用于保持 blob 存活的采样计划。

### 1.2 区块头扩展

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

捆绑哈希同时输入到块哈希和 `SignedBlockWire` 元数据中。
开销。

实现说明：`BlockPayload` 和透明 `BlockBuilder` 现在公开
`da_commitments` 设置器/获取器（请参阅 `BlockBuilder::set_da_commitments` 和
`SignedBlock::set_da_commitments`)，因此主机可以附加预构建的捆绑包
在密封块之前。所有辅助构造函数默认字段为 `None`
直到 Torii 将真正的捆绑包穿过。

### 1.3 有线编码- `SignedBlockWire::canonical_wire()` 附加 Norito 标头
  `DaCommitmentBundle` 紧接在现有事务列表之后。的
  版本字节为 `0x01`。
- `SignedBlockWire::decode_wire()` 拒绝 `version` 未知的捆绑包，
  与 `norito.md` 中描述的 Norito 策略匹配。
- 哈希推导更新仅存在于 `block::Hasher` 中；轻客户端解码
  现有的有线格式自动获得新字段，因为 Norito
  标头宣告其存在。

## 2. 区块生产流程

1. Torii DA 摄取将签名的收据和承诺记录保存到
   DA 线轴 (`da-receipt-*.norito` / `da-commitment-*.norito`)。经久耐用
   收据日志在重新启动时播种游标，因此重播的收据仍按顺序排列
   确定性地。
2. 块组件从线轴加载收据，丢弃陈旧/已密封的收据
   使用提交的游标快照的条目，并强制每个条目的连续性
   `(lane, epoch)`。如果可到达的收据缺少匹配的承诺或
   清单散列使提案中止而不是默默地忽略它。
3. 在密封之前，构建者将承诺包切片为
   收据驱动集，按 `(lane_id, epoch, sequence)` 排序，编码
   与 Norito 编解码器捆绑在一起，并更新 `da_commitments_hash`。
4. 完整的包存储在 WSV 中并与内部的块一起发出
   `SignedBlockWire`；承诺的捆绑包推进收据光标（水合
   重新启动时从 Kura 中删除）并修剪过时的假脱机条目以限制磁盘增长。

块组装和 `BlockCreated` 摄取重新验证每个承诺
通道目录：Merkle 通道拒绝杂散的 KZG 承诺，KZG 通道需要
非零 KZG 承诺和非零 `chunk_root`，并且未知车道是
掉了。 Torii 的 `/v1/da/commitments/verify` 端点镜像相同的防护，
并摄取现在将确定性 KZG 承诺融入到每个
`kzg_bls12_381` 记录，以便符合策略的捆绑包到达块组装。

DA-2 摄取计划中描述的清单固定装置兼作来源
承诺捆绑器的真相。 Torii 测试
`manifest_fixtures_cover_all_blob_classes` 为每个重新生成清单
`BlobClass` 变体并拒绝编译，直到新类获得固定装置，
确保每个 `DaCommitmentRecord` 内的编码清单哈希与
金色 Norito/JSON 对。【crates/iroha_torii/src/da/tests.rs:2902】

如果块创建失败，收据仍保留在队列中，因此下一个块
尝试可以捡起它们；构建器记录最后包含的 `sequence` 每
车道以避免重放攻击。

## 3. RPC 和查询界面

Torii 公开三个端点：|路线 |方法|有效负载|笔记|
|--------|--------|---------|--------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery`（按泳道/纪元/序列、分页进行范围过滤）|返回 `DaCommitmentPage` 以及总计数、承诺和块哈希。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest`（通道 + 清单哈希或 `(epoch, sequence)` 元组）。 |响应 `DaCommitmentProof`（记录 + Merkle 路径 + 区块哈希）。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |无状态助手，重播块哈希计算并验证包含；由无法直接链接到 `iroha_crypto` 的 SDK 使用。 |

所有有效负载均位于 `iroha_data_model::da::commitment` 下。 Torii 路由器安装座
现有 DA 摄取端点旁边的处理程序可重用令牌/mTLS
政策。

## 4. 包含证明和轻客户端

- 区块生产者在序列化的基础上构建二叉 Merkle 树
  `DaCommitmentRecord` 列表。根提供 `da_commitments_hash`。
- `DaCommitmentProof` 打包目标记录加上向量 `(sibling_hash,
  position)` 条目，以便验证者可以重建根。证明还包括
  块哈希和签名标头，以便轻客户端可以验证最终性。
- CLI 助手 (`iroha_cli app da prove-commitment`) 包装证明请求/验证
  为操作员提供循环和表面 Norito/十六进制输出。

## 5. 存储和索引

WSV 将承诺存储在由 `manifest_hash` 键入的专用列族中。
二级索引涵盖 `(lane_id, epoch)` 和 `(lane_id, sequence)` 所以查询
避免扫描完整的捆绑包。每条记录都跟踪密封它的区块高度，
允许追赶节点从块日志中快速重建索引。

## 6. 遥测和可观测性

- 每当一个块密封至少一个时，`torii_da_commitments_total` 就会递增
  记录。
- `torii_da_commitment_queue_depth` 跟踪等待捆绑的收据（每
  车道）。
- Grafana 仪表板 `dashboards/grafana/da_commitments.json` 可视化块
  包含、队列深度和证明吞吐量，以便 DA-3 发布门可以审核
  行为。

## 7. 测试策略

1. **`DaCommitmentBundle` 编码/解码和块哈希的单元测试**
   推导更新。
2. **`fixtures/da/commitments/` 捕获规范下的黄金装置**
   捆绑字节和 Merkle 证明。每个包引用清单字节
   来自 `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`，所以
   正在再生 `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   在 `ci/check_da_commitments.sh` 刷新承诺之前，保持 Norito 故事的一致性
   证明。【fixtures/da/ingest/README.md:1】
3. **集成测试** 启动两个验证器，摄取样本 blob，以及
   断言两个节点都同意捆绑内容和查询/证明
   回应。
4. **`integration_tests/tests/da/commitments.rs` 中的轻客户端测试**
   （Rust）调用 `/prove` 并验证证明，而不与 Torii 交谈。
5. **CLI Smoke** 脚本 `scripts/da/check_commitments.sh` 以保留操作员
   工具可重复。

## 8. 推出计划|相|描述 |退出标准 |
|--------|-------------|---------------|
| P0 — 数据模型合并 |登陆 `DaCommitmentRecord`、块头更新和 Norito 编解码器。 | `cargo test -p iroha_data_model` 绿色，带新灯具。 |
| P1 — 核心/WSV 接线 |线程队列 + 块构建器逻辑、持久索引并公开 RPC 处理程序。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` 通过捆绑证明断言。 |
| P2 — 操作员工具 |发布 CLI 帮助程序、Grafana 仪表板和证明验证文档更新。 | `iroha_cli app da prove-commitment` 适用于 devnet；仪表板显示实时数据。 |
| P3——治理门|启用需要在 `iroha_config::nexus` 中标记的通道上进行 DA 承诺的块验证器。 |状态条目+路线图更新将DA-3标记为🈴。 |

## 开放问题

1. **KZG 与 Merkle 默认值** — 小斑点是否应该始终跳过 KZG 的承诺
   减小块大小？建议：保留 `kzg_commitment` 可选并通过gate via
   `iroha_config::da.enable_kzg`。
2. **序列间隙** — 我们是否允许无序通道？目前的计划拒绝存在差距
   除非治理切换 `allow_sequence_skips` 进行紧急重播。
3. **轻客户端缓存** — SDK 团队请求轻量级 SQLite 缓存
   证明；有待 DA-8 下的后续行动。

在实施 PR 中回答这些问题会将 DA-3 从 🈸（本文档）移至 🈺
一旦代码工作开始。