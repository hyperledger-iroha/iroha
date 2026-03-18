---
lang: zh-hans
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-17T06:10:29.077000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 合并账本设计 — 通道最终性和全局缩减

本说明最终确定了 Milestone 5 的合并账本设计。它解释了
非空块策略、跨通道 QC 合并语义和最终工作流程
这将车道级执行与全球世界状态承诺联系起来。

该设计扩展了 `nexus.md` 中描述的 Nexus 架构。术语如
“车道块”、“车道QC”、“合并提示”和“合并分类账”继承它们的
该文件中的定义；本说明重点关注行为规则和
必须由运行时、存储和 WSV 强制执行的实施指南
层。

## 1. 非空块策略

**规则（必须）：** 仅当块包含 at 时，车道提议者才发出块
至少一个已执行的事务片段、基于时间的触发器或确定性的
工件更新（例如，DA 工件汇总）。禁止空块。

**影响：**

- Slot keep-alive：当没有事务满足其确定性提交窗口时，
该车道不发出任何方块，只是前进到下一个槽位。合并账本
仍保留在该车道的前一个提示上。
- 触发器批处理：不产生状态转换的后台触发器（例如，
重申不变量的 cron）被认为是空的并且必须被跳过或者
在生成块之前与其他工作捆绑在一起。
- 遥测：`pipeline_detached_merged` 和后续指标处理被跳过
明确的插槽——操作员可以区分“无工作”和“管道停滞”。
- 重播：块存储不会插入合成的空占位符。库拉
如果没有，重播循环只是观察连续插槽的相同父哈希值
块被发射。

**规范检查：** 在区块提案和验证期间，`ValidBlock::commit`
断言关联的 `StateBlock` 至少携带一个已提交的覆盖
（增量、工件、触发器）。这与 `StateBlock::is_empty` 防护一致
这已经确保了无操作写入被省略。强制执行发生在之前
要求签名，以便委员会永远不会对空有效负载进行投票。

## 2. 跨通道 QC 合并语义

由其委员会最终确定的每个车道块 `B_i` 产生：

- `lane_state_root_i`：Poseidon2-SMT 对每个 DS 状态根源的承诺已触及
在街区里。
- `merge_hint_root_i`：合并分类账的滚动候选者（`tag =
"iroha:merge:candidate:v1\0"`).
- `lane_qc_i`：车道委员会在
  执行投票原像（块哈希，`parent_state_root`，
  `post_state_root`、高度/视图/纪元、chain_id 和模式标签）。

合并节点收集最新提示`{(B_i, lane_qc_i, merge_hint_root_i)}`
所有车道 `i ∈ [0, K)`。

**合并条目（必须）：**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` 是通道块的哈希值，通道的合并条目密封
  `i`。如果自上一个合并条目以来车道没有发出任何块，则该值为
  重复。
- `merge_hint_root[i]` 是对应通道的 `merge_hint_root`
  块。当 `lane_tips[i]` 重复时，它会重复。
- `global_state_root` 等于 `ReduceMergeHints(merge_hint_root[0..K-1])`，
  Poseidon2 折叠与域分离标签
  `"iroha:merge:reduce:v1\0"`。减少是确定性的并且必须
  在同行之间重建相同的价值。
- `merge_qc` 是合并委员会颁发的 BFT 法定人数证书
  序列化条目。

**合并 QC 有效负载（必须）：**

合并委员会成员签署确定性摘要：

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` 是源自车道提示的合并委员会视图（最大
  `view_change_index` 穿过由入口密封的车道标头）。
- `chain_id` 是配置的链标识符字符串（UTF-8 字节）。
- 有效负载使用 Norito 编码，字段顺序如上所示。

生成的摘要存储在 `merge_qc.message_digest` 中，并且是消息
由 BLS 签名验证。

**合并 QC 构建（必须）：**

- 合并委员会花名册是当前提交拓扑验证者集。
- 所需法定人数 = `commit_quorum_from_len(roster_len)`。
- `merge_qc.signers_bitmap` 对参与验证者索引进行编码（LSB优先）
  按照提交拓扑顺序。
- `merge_qc.aggregate_signature` 是摘要的 BLS 正常聚合
  上面。

**验证（必须）：**

1. 对照 `lane_tips[i]` 验证每个 `lane_qc_i` 并确认块头
   包括匹配的 `merge_hint_root_i`。
2. 确保没有 `lane_qc_i` 指向 `Invalid` 或未执行的块。的
   上面的非空策略确保标头包含状态覆盖。
3. 重新计算 `ReduceMergeHints` 并与 `global_state_root` 进行比较。
4. 重新计算合并QC摘要并验证签名者位图、仲裁阈值、
   并根据提交拓扑名册聚合签名。

**可观察性：** 合并节点发出 Prometheus 计数器
`merge_entry_lane_repeats_total{i}` 突出显示跳过插槽的通道
运营可见性。

## 3. 最终确定工作流程

### 3.1 车道级最终确定性

1. 事务在确定性时隙中按通道进行调度。
2. 执行器将覆盖应用到 `StateBlock` 中，生成增量和
文物。
3. 验证后，车道委员会签署执行投票原像，
   绑定区块哈希、状态根和高度/视图/纪元。元组
   `(block_hash, lane_qc_i, merge_hint_root_i)` 被认为是车道最终的。
4. 轻客户端可以将车道提示视为 DS 限制证明的最终结果，但是
必须记录关联的 `merge_hint_root` 以与合并分类账进行核对
稍后。通道委员会是针对每个数据空间的，不会取代全局提交
拓扑。委员会大小固定为 `3f+1`，其中 `f` 来自
数据空间目录 (`fault_tolerance`)。验证器池是数据空间的
验证器（管理员管理的通道或公共通道的通道治理清单
权益选出的车道的权益记录）。委员会成员是
使用绑定的 VRF 纪元种子每个纪元确定性采样一次
`dataspace_id` 和 `lane_id`。如果池小于 `3f+1`，则通道最终确定
暂停，直到恢复仲裁（紧急恢复单独处理）。

### 3.2 合并账本最终性

1. 合并委员会收集最新的车道提示，验证每个`lane_qc_i`，并
构造如上定义的 `MergeLedgerEntry`。
2. 验证确定性约简后，合并委员会签署
条目（`merge_qc`）。
3. 节点将条目追加到合并分类账日志中，并将其与
车道块参考。
4. `global_state_root`成为权威的世界国家承诺
纪元/时段。完整节点更新其 WSV 检查点元数据以反映此情况
价值；确定性重放必须重现相同的减少。

### 3.3 WSV 和存储集成

- `State::commit_merge_entry` 记录每通道状态根和
  最终 `global_state_root`，使用全局校验和桥接通道执行。
- Kura 坚持 `MergeLedgerEntry` 与车道块伪影相邻，因此
  重播可以重建车道级和全局最终性序列。
- 当通道跳过一个槽位时，存储仅保留前一个提示；不
  创建占位符合并条目，直到至少一个通道产生新的
  块。
- API表面（Torii，遥测）公开车道提示和最新合并
  入口，以便运营商和客户可以协调每车道和全局视图。

## 4. 实施注意事项- `crates/iroha_core/src/state.rs`：`State::commit_merge_entry` 验证
  减少并将车道/全局元数据连接到世界状态中，以便查询
  观察者可以访问合并提示和权威的全局哈希。
- `crates/iroha_core/src/kura.rs`：`Kura::store_block_with_merge_entry` 入队
  块并在一步中保留关联的合并条目，回滚
  追加失败时内存中的块，因此存储永远不会记录块
  没有其密封元数据。合并账本日志被同步修剪
  在启动恢复期间验证块高度，并缓存在内存中
  使用有界窗口（`kura.merge_ledger_cache_capacity`，默认 256）
  避免长时间运行的节点上无限制的增长。恢复截断部分或
  过大的合并账本尾部条目，并且附加拒绝上面的条目
  最大有效负载大小保护以限制分配。
- `crates/iroha_core/src/block.rs`：块验证拒绝没有块
  入口点（外部事务或时间触发器）并且没有确定性
  诸如 DA 捆绑包 (`BlockValidationError::EmptyBlock`) 等工件，确保
  在请求和携带签名之前强制执行非空策略
  进入合并分类账。
- 确定性缩减助手位于合并服务中：`reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) 实现了上述 Poseidon2 折叠。
  硬件加速钩子仍然是未来的工作，但标量路径现在强制执行
  确定性的规范约简。
- 遥测集成：公开每车道合并重复和
  `global_state_root` 仪表仍在可观测性积压中进行跟踪，因此
  仪表板工作可以与合并服务的推出一起进行。
- 跨组件测试：合并减少的黄金重播覆盖率是
  与集成测试待办事项进行跟踪，以确保未来的更改
  `reduce_merge_hint_roots` 保持记录的根稳定。