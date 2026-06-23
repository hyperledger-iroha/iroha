<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi 正式模型 (TLA+ / Apalache)

该目录包含 Sumeragi 安全性和活性的有界正式模型。

## 范围

`Sumeragi.tla` 捕获提交路径：
- 阶段进展（`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`），
- 投票和法定人数阈值（`CommitQuorum`、`ViewQuorum`），
- NPoS 式提交保护的加权权益法定人数 (`StakeQuorum`)，
- RBC 因果关系 (`Init -> Chunk -> Ready -> Deliver`) 以及标题/摘要证据，
- 商品及服务税和对诚实进步行动的弱公平假设。

`SumeragiFrontierRecovery.tla` 捕捉了围绕一期的专注 Taira 悬挂课程
待处理的连续边界块：
- 低于法定人数或达到法定人数的提交投票证据，
- 投票队列积压和本地流失，
- 丢失与本地有效负载状态，
- 新的与陈旧的前沿恢复所有权，
- 仲裁重新安排标记/窗口节奏，
- 可以巩固当地前沿的未来前沿/新观点证据，
- 确定性 GST 后提交、重传、有界视图旋转以及
  零证据下降结果。

两种模型都有意抽象出有线格式、ECDSA/签名
验证和完整的网络详细信息。

## 文件- `Sumeragi.tla`：协议模型和属性。
- `Sumeragi_fast.cfg`：较小的 CI 友好参数集。
- `Sumeragi_deep.cfg`：更大的应力参数集。
- `SumeragiFrontierRecovery.tla`：重点边境恢复模型。
- `SumeragiFrontierRecovery_fast.cfg`：较小的 CI 友好前沿参数集。
- `SumeragiFrontierRecovery_deep.cfg`：更大的前沿积压/窗口/视图绑定集。
- `SumeragiFrontierRecovery_wide.cfg`：手动更宽边界设置。
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`：预期失败陈旧所有者突变。
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`：预期失败投票队列突变。

## 属性

不变量：
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

时间属性：
- `EventuallyCommit` (`[] (gst => <> committed)`)，采用 GST 后公平性编码
  在 `Next` 中运行（启用超时/故障抢占防护）
  进展行动）。这使得模型可以通过 Apalache 0.52.x 进行检查，
  不支持检查时间属性内的 `WF_` 公平运算符。

边界恢复不变量：
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`，排除终端
  GST 后状态，其中 `pending /\ voteBacked /\ ~committed` 没有恢复，
  提交、重传、旋转或有界丢弃转换。边界恢复时间属性：
- `PostGstVoteBackedFrontierEventuallyResolves`：GST之后，每个未解决的问题
  投票支持的待定边界状态最终达到提交、有效负载
  恢复、仲裁重传、未来前沿锚定或有界视图
  旋转。
- `RecoveredPayloadEventuallyAdvances`：一个投票支持的边境国家，
  恢复后的有效负载不能在没有提交的情况下永远保持挂起状态，
  重传、重新锚定或轮换。
- `QuorumRetransmitEventuallyLeavesPending`：一旦仲裁重新传输已触发
  对于投票支持的边境州，待处理的包装器最终必须清除。
- `FutureFrontierEvidenceEventuallyReanchors`：后来的前沿/新观点证据
  必须清除挂起的包装器或作为前沿锚点使用。

## 假设图

边界模型有意是有限的。这些是实现
它抽象的表面：|模型概念|实施面|
| --- | --- |
| `pending`、`contiguous`、`payloadState` | `PendingBlock` 处理和 `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` 中的本地有效负载检查，以及 `proposal_handlers.rs` 中的 BlockCreated/前沿所有权实现。 |
| `commitVotes`，`queuedVotes` |由 `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` 和 `reschedule_ignores_quorum_timeout_vote_queue_backlog` 在 `crates/iroha_core/src/sumeragi/main_loop/tests.rs` 中执行提交计票和投票入口门控。 |
| `recoveryOwner` |活动/过时前沿所有者状态在 `frontier_slot_has_active_owner_state_for_view(...)` 中，过时所有者产量在 `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` 中，并取代清理在 `drop_superseded_contiguous_frontier_owner_state(...)` 中。 |
| `quorumRescheduleArmed`，`quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` 中投票支持的法定人数重新安排节奏；回归覆盖范围包括 `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`。 |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`、`handle_frontier_body_gap_with_topology(...)` 和 `stale_frontier_rbc_repair_is_actionable(...)` 中的精确前沿身体修复和陈旧红细胞修复入院。 |
| `quorumRetransmitted`、`rotated` |仲裁重传目标选择 `rebroadcast_pending_block_updates(...)` 以及 `reschedule_stale_pending_blocks_with_now(...)` 中的确定性视图更改调用。 |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` 中的未来新视图/更高前沿法定人数证据，由 `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` 涵盖。 |

## 运行

从存储库根目录：

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

运行程序为每种模式设置显式 Apalache `--length`：|模式|长度 |预期用途 |
| --- | ---: | --- |
| `fast` | 10 | 10 CI 提交路径检查 |
| `deep` | 10 | 10更大的提交路径检查 |
| `frontier-fast` | 10 | 10 CI边境检查|
| `frontier-deep` | 12 | 12更大的边境检查|
| `frontier-wide` | 14 | 14手动/每晚边境压力检查 |

`APALACHE_LENGTH=<n>` 在本地探索时会覆盖每个模式的默认值
反例或扩大有界证明。

### 可重现的本地设置（不需要 Docker）

安装此存储库使用的固定本地 Apalache 工具链：

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

运行程序会自动检测此安装：
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
安装后，`ci/check_sumeragi_formal.sh` 应该可以在没有额外环境变量的情况下工作：

```bash
bash ci/check_sumeragi_formal.sh
```

预期失败突变有意位于正常 CI 之外。他们应该
在 Apalache 下失败，但在更改模型时很有用：

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

如果 Apalache 不在 `PATH` 中，您可以：

- 将 `APALACHE_BIN` 设置为可执行路径，或者
- 使用 Docker 后备（当 `docker` 可用时默认启用）：
  - 图像：`APALACHE_DOCKER_IMAGE`（默认 `ghcr.io/apalache-mc/apalache:0.52.2`）
  - 需要正在运行的 Docker 守护进程
  - 使用 `APALACHE_ALLOW_DOCKER=0` 禁用回退。

示例：

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## 注释- 该模型补充（而不是取代）可执行 Rust 模型测试
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  和
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- 检查受 `.cfg` 文件中的常量值限制。
- PR CI 通过 `.github/workflows/pr.yml` 运行这些检查
  `ci/check_sumeragi_formal.sh`。
