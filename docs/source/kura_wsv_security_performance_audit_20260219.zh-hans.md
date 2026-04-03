<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / WSV 安全与性能审计 (2026-02-19)

## 范围

此次审计涵盖：

- Kura 持久性和预算路径：`crates/iroha_core/src/kura.rs`
- 生产 WSV/状态提交/查询路径：`crates/iroha_core/src/state.rs`
- IVM WSV 模拟主机表面（测试/开发范围）：`crates/ivm/src/mock_wsv.rs`

超出范围：不相关的板条箱和全系统基准测试重新运行。

## 风险总结

- 严重：0
- 高：4
- 中：6
- 低：2

## 调查结果（按严重程度排序）

### 高

1. **Kura writer 对 I/O 故障感到恐慌（节点可用性风险）**
- 成分：库拉
- 类型：安全性 (DoS)、可靠性
- 详细信息：写入器循环在追加/索引/fsync错误上发生恐慌，而不是返回可恢复的错误，因此瞬时磁盘故障可以终止节点进程。
- 证据：
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- 影响：远程负载+本地磁盘压力可能会导致崩溃/重启循环。2. **Kura 逐出在 `block_store` 互斥体下进行完整数据/索引重写**
- 成分：库拉
- 类型：性能、可用性
- 详细信息：`evict_block_bodies` 在持有 `block_store` 锁定的同时通过临时文件重写 `blocks.data` 和 `blocks.index`。
- 证据：
  - 锁获取：`crates/iroha_core/src/kura.rs:834`
  - 完全重写循环：`crates/iroha_core/src/kura.rs:921`、`crates/iroha_core/src/kura.rs:942`
  - 原子替换/同步：`crates/iroha_core/src/kura.rs:956`、`crates/iroha_core/src/kura.rs:960`
- 影响：逐出事件可能会长时间拖延大型历史记录的写入/读取。

3. **状态提交在繁重的提交工作中保留粗略的 `view_lock`**
- 组件：生产 WSV
- 类型：性能、可用性
- 详细信息：块提交在提交事务、块哈希和世界状态时保留独占的 `view_lock`，从而在大块下造成读者饥饿。
- 证据：
  - 锁定开始：`crates/iroha_core/src/state.rs:17456`
  - 锁内工作：`crates/iroha_core/src/state.rs:17466`、`crates/iroha_core/src/state.rs:17476`、`crates/iroha_core/src/state.rs:17483`
- 影响：持续的大量提交会降低查询/共识响应能力。4. **IVM JSON 管理别名允许特权突变，无需调用者检查（测试/开发主机）**
- 组件：IVM WSV 模拟主机
- 类型：安全（测试/开发环境中的权限升级）
- 详细信息：JSON 别名处理程序直接路由到不需要调用者范围的权限令牌的角色/权限/对等突变方法。
- 证据：
  - 管理员别名：`crates/ivm/src/mock_wsv.rs:4274`、`crates/ivm/src/mock_wsv.rs:4371`、`crates/ivm/src/mock_wsv.rs:4448`
  - 非门控突变器：`crates/ivm/src/mock_wsv.rs:1035`、`crates/ivm/src/mock_wsv.rs:1055`、`crates/ivm/src/mock_wsv.rs:855`
  - 文件文档中的范围注释（测试/开发意图）：`crates/ivm/src/mock_wsv.rs:295`
- 影响：测试合约/工具可以自我提升并使集成工具中的安全假设失效。

### 中等

5. **Kura 预算检查重新编码每个排队上的待处理块（每次写入 O(n)）**
- 成分：库拉
类型：性能
- 详细信息：每个队列通过迭代挂起块并通过规范的线大小路径序列化每个块来重新计算挂起队列字节。
- 证据：
  - 队列扫描：`crates/iroha_core/src/kura.rs:2509`
  - 每块编码路径：`crates/iroha_core/src/kura.rs:2194`、`crates/iroha_core/src/kura.rs:2525`
  - 在队列预算检查中调用：`crates/iroha_core/src/kura.rs:2580`、`crates/iroha_core/src/kura.rs:2050`
- 影响：积压情况下写入吞吐量下降。6. **Kura 预算检查对每个队列执行重复的块存储元数据读取**
- 成分：库拉
类型：性能
- 详细信息：每次检查都会读取持久索引计数和文件长度，同时锁定 `block_store`。
- 证据：
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- 影响：热排队路径上可避免的 I/O/锁开销。

7. **Kura 驱逐是从队列预算路径内联触发的**
- 成分：库拉
- 类型：性能、可用性
- 细节：入队路径可以在接受新块之前同步调用驱逐。
- 证据：
  - 排队调用链：`crates/iroha_core/src/kura.rs:2050`
  - 内联驱逐调用：`crates/iroha_core/src/kura.rs:2603`
- 影响：接近预算时，交易/区块摄取的尾部延迟会激增。

8. **`State::view` 可能会在争用情况下返回而不获取粗略锁**
- 组件：生产 WSV
- 类型：一致性/性能权衡
- 详细信息：在写锁争用时，`try_read` 后备返回没有设计粗略保护的视图。
- 证据：
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- 影响：提高了活跃度，但调用者必须容忍争用下较弱的跨组件原子性。9. **`apply_without_execution` 在 DA 光标前进中使用硬 `expect`**
- 组件：生产 WSV
- 类型：安全性（通过panic-on-invariant-break进行DoS）、可靠性
- 详细信息：如果 DA 游标前进不变量失败，则提交的块应用路径会发生混乱。
- 证据：
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- 影响：潜在的验证/索引错误可能会导致节点终止故障。

10. **IVM TLV 发布系统调用在分配之前缺少显式信封大小限制（测试/开发主机）**
- 组件：IVM WSV 模拟主机
- 类型：安全（内存 DoS）、性能
- 详细信息：读取标头长度，然后分配/复制完整的 TLV 负载，在此路径中没有主机级上限。
- 证据：
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- 影响：恶意测试有效负载可能会强制进行大量分配。

### 低

11. **Kura通知通道无限制（`std::sync::mpsc::channel`）**
- 成分：库拉
- 类型：性能/内存卫生
- 详细信息：通知通道可以在持续的生产者压力期间累积冗余唤醒事件。
- 证据：
  - `crates/iroha_core/src/kura.rs:552`
- 影响：每个事件大小的内存增长风险较低，但可以避免。12. **管道边车队列在内存中是无限的，直到写入器耗尽**
- 成分：库拉
- 类型：性能/内存卫生
- 详细信息：边车队列 `push_back` 没有明确的上限/背压。
- 证据：
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- 影响：长时间写入延迟期间潜在的内存增长。

## 现有测试覆盖率和差距

### 库拉

- 现有承保范围：
  - 存储预算行为：`store_block_rejects_when_budget_exceeded`、`store_block_rejects_when_pending_blocks_exceed_budget`、`store_block_evicts_when_block_exceeds_budget`（`crates/iroha_core/src/kura.rs:6820`、`crates/iroha_core/src/kura.rs:6949`、`crates/iroha_core/src/kura.rs:6984`）
  - 逐出正确性和补液：`evict_block_bodies_does_not_truncate_unpersisted`、`evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`、`crates/iroha_core/src/kura.rs:8126`)
- 差距：
  - 没有错误注入覆盖用于追加/索引/fsync失败处理而不会出现恐慌
  - 没有针对大型挂起队列和排队预算检查成本的性能回归测试
  - 锁争用下没有长期历史驱逐延迟测试

### 生产 WSV

- 现有承保范围：
  - 争用回退行为：`state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - 分层后端的锁定顺序安全：`state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- 差距：
  - 没有定量争用测试断言在繁重的世界提交下最大可接受的提交保持时间
  - 如果 DA 光标前进不变量意外中断，则不会进行无恐慌处理的回归测试

### IVM WSV 模拟主机- 现有承保范围：
  - 权限 JSON 解析器语义和对等解析（`crates/ivm/src/mock_wsv.rs:5234`、`crates/ivm/src/mock_wsv.rs:5332`）
  - 围绕 TLV 解码和 JSON 解码的系统调用冒烟测试（`crates/ivm/src/mock_wsv.rs:5962`、`crates/ivm/src/mock_wsv.rs:6078`）
- 差距：
  - 没有未经授权的管理员别名拒绝测试
  - `INPUT_PUBLISH_TLV` 中没有超大 TLV 包络抑制测试
  - 没有关于检查点/恢复克隆成本的基准/护栏测试

## 优先修复计划

### 第 1 阶段（高强度强化）

1. 将 Kura writer `panic!` 分支替换为可恢复错误传播 + 健康状况恶化信号。
- 目标文件：`crates/iroha_core/src/kura.rs`
- 验收：
  - 注入的追加/索引/fsync失败不会惊慌
  - 通过遥测/日志记录显示错误，并且编写器仍然可控

2. 为 IVM 模拟主机 TLV 发布和 JSON 信封路径添加有界信封检查。
- 目标文件：`crates/ivm/src/mock_wsv.rs`
- 验收：
  - 在大量分配处理之前拒绝过大的有效负载
  - 新测试涵盖 TLV 和 JSON 超大情况

3. 对 JSON 管理员别名（或严格的仅测试功能标志和清晰文档后面的门别名）强制执行显式调用者权限检查。
- 目标文件：`crates/ivm/src/mock_wsv.rs`
- 验收：
  - 未经授权的调用者无法通过别名改变角色/权限/对等状态

### 第 2 阶段（热路径性能）4.使Kura预算会计增量。
- 将每个入队的完整挂起队列重新计算替换为在入队/保留/删除时更新的维护计数器。
- 验收：
  - 用于待处理字节计算的排队成本接近 O(1)
  - 回归基准显示随着待定深度的增长稳定的延迟

5. 减少驱逐锁保持时间。
- 选项：分段压缩、具有锁释放边界的分块复制或具有有界前景阻塞的后台维护模式。
- 验收：
  - 大历史驱逐延迟减少，前台操作保持响应

6. 在可行的情况下缩短粗 `view_lock` 关键部分。
- 评估分割提交阶段或对分阶段增量进行快照，以最大限度地减少独占保留窗口。
- 验收：
  - 争用指标显示在大量块提交下减少了 99p 的保持时间

### 第三阶段（运行护栏）

7. 为 Kura writer 和 sidecar 队列背压/上限引入有界/合并唤醒信号。
8. 扩展遥测仪表板：
- `view_lock` 等待/保持分布
- 逐出持续时间和每次运行回收的字节数
- 预算检查排队延迟

## 建议的测试添加1. `kura_writer_io_failures_do_not_panic`（单元，故障注入）
2. `kura_budget_check_scales_with_pending_depth`（性能回归）
3. `kura_eviction_does_not_block_reads_beyond_threshold`（集成/性能）
4. `state_commit_view_lock_hold_under_heavy_world_commit`（争用回归）
5. `state_apply_without_execution_handles_da_cursor_error_without_panic`（弹性）
6. `mock_wsv_admin_alias_requires_permissions`（安全回归）
7. `mock_wsv_input_publish_tlv_rejects_oversize`（DoS防护）
8. `mock_wsv_checkpoint_restore_cost_regression`（性能基准）

## 关于范围和置信度的注释

- `crates/iroha_core/src/kura.rs` 和 `crates/iroha_core/src/state.rs` 的结果是生产路径结果。
- 根据文件级文档，`crates/ivm/src/mock_wsv.rs` 的结果明确为测试/开发主机范围。
- 此审核本身不需要 ABI 版本控制更改。