---
lang: zh-hans
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ 传输小工具设计

# 概述

当前的 FASTPQ 规划器记录 `TransferAsset` 指令中涉及的每个原始操作，这意味着每次传输分别支付余额算术、哈希轮次和 SMT 更新。为了减少每次传输的跟踪行数，我们引入了一个专用小工具，该小工具仅在主机继续执行规范状态转换时验证最少的算术/提交检查。

- **范围**：通过现有 Kotodama/IVM `TransferAsset` 系统调用表面发出单次传输和小批量。
- **目标**：通过共享查找表并将每次传输算术压缩为紧凑的约束块，减少大容量传输的 FFT/LDE 列占用空间。

# 架构

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## 成绩单格式

主机每次系统调用都会发出 `TransferTranscript`：

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` 将转录本与事务入口点哈希相关联以实现重播保护。
- `authority_digest` 是主机在排序签名者/仲裁数据上的哈希值；该小工具会检查相等性，但不会重做签名验证。具体来说，主机 Norito 对 `AccountId`（已嵌入规范的多重签名控制器）进行编码，并使用 Blake2b-256 对 `b"iroha:fastpq:v1:authority|" || encoded_account` 进行哈希处理，存储结果 `Hash`。
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || asset || amount || batch_hash);确保小工具重新计算与主机相同的摘要。原像字节使用裸 Norito 编码构造为 `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash`，然后将其传递给共享 Poseidon2 助手。此摘要对于单增量转录本存在，而对于多增量批次则省略。

所有字段都通过 Norito 进行序列化，因此现有的确定性保证有效。
`from_path` 和 `to_path` 均使用以下命令作为 Norito blob 发出
`TransferMerkleProofV1` 架构：`{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`。
未来版本可以扩展模式，同时证明者强制执行版本标签
解码之前。 `TransitionBatch` 元数据嵌入 Norito 编码的转录本
`transfer_transcripts` 密钥下的向量，以便证明者可以解码见证人
无需执行带外查询。公共输入（`dsid`、`slot`、根、
`perm_root`、`tx_set_hash`) 被携带在 `FastpqTransitionBatch.public_inputs` 中，
留下元数据用于条目哈希/转录计数簿记。直到主机管道
落地后，证明者从密钥/余额对中综合得出证明，因此行
即使转录本省略了可选字段，也始终包含确定性 SMT 路径。

## 小工具布局

1. **平衡算术块**
   - 输入：`from_balance_before`、`amount`、`to_balance_before`。
   - 检查：
     - `from_balance_before >= amount`（具有共享 RNS 分解的范围小工具）。
     - `from_balance_after = from_balance_before - amount`。
     - `to_balance_after = to_balance_before + amount`。
   - 打包到一个自定义门中，因此所有三个方程都消耗一个行组。2. **波塞冬承诺块**
   - 使用其他小工具中已使用的共享 Poseidon 查找表重新计算 `poseidon_preimage_digest`。跟踪中没有每次传输的波塞冬弹。

3. **默克尔路径块**
   - 通过“配对更新”模式扩展现有的 Kaigi SMT 小工具。两个叶子（发送者、接收者）共享同级哈希的同一列，从而减少重复的行。

4. **权威摘要检查**
   - 主机提供的摘要和见证值之间的简单相等约束。签名保留在他们的专用小工具中。

5. **批量循环**
   - 程序在 `transfer_asset` 构建器循环之前调用 `transfer_v1_batch_begin()`，之后调用 `transfer_v1_batch_end()`。当示波器处于活动状态时，主机会缓冲每次传输并将其作为单个 `TransferAssetBatch` 重播，每批重用一次 Poseidon/SMT 上下文。每个附加增量仅添加算术和两个叶检查。转录解码器现在接受多增量批次并将其显示为 `TransferGadgetInput::deltas`，因此计划者可以折叠见证人而无需重新读取 Norito。已经方便使用 Norito 有效负载的合约（例如 CLI/SDK）可以通过调用 `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` 来完全跳过范围，这会在一个系统调用中向主机提供完全编码的批次。

# 主机和证明者变更|层|变化|
|--------|---------|
| `ivm::syscalls` |添加 `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`)，以便程序可以将多个 `transfer_v1` 系统调用括起来，而不会发出中间 ISI，再加上 `transfer_v1_batch_apply` (`0x2B`)预编码批次。 |
| `ivm::host` 和测试 |当范围处于活动状态时，核心/默认主机将 `transfer_v1` 视为批量追加，表面 `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` 和模拟 WSV 主机在提交之前缓冲条目，因此回归测试可以断言确定性平衡更新。【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【板条箱/ivm/tests/wsv_host_pointer_tlv.rs:219】【板条箱/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` |状态转换后发出 `TransferTranscript`，在 `StateBlock::capture_exec_witness` 期间使用显式 `public_inputs` 构建 `FastpqTransitionBatch` 记录，并运行 FASTPQ 验证器通道，以便 Torii/CLI 工具和 Stage6 后端接收规范`TransitionBatch` 输入。 `TransferAssetBatch` 将顺序传输分组为单个转录本，省略多增量批次的海神摘要，以便小工具可以确定性地跨条目迭代。 |
| `fastpq_prover` | `gadgets::transfer` 现在为规划器 (`crates/fastpq_prover/src/gadgets/transfer.rs`) 验证多增量转录本（平衡算术 + Poseidon 摘要）并显示结构化见证（包括占位符配对的 SMT blob）。 `trace::build_trace` 从批次元数据中解码这些转录本，拒绝缺少 `transfer_transcripts` 有效负载的传输批次，将经过验证的见证附加到 `Trace::transfer_witnesses`，并且 `TracePolynomialData::transfer_plan()` 使聚合计划保持活动状态，直到规划器使用小工具 (`crates/fastpq_prover/src/trace.rs`)。行计数回归线束现在通过 `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) 提供，覆盖多达 65536 行填充的场景，而成对的 SMT 布线仍然落后于 TF-3 批处理辅助里程碑（占位符使走线布局保持稳定，直到交换落地）。 |
| Kotodama |将 `transfer_batch((from,to,asset,amount), …)` 帮助程序降低为 `transfer_v1_batch_begin`、顺序 `transfer_asset` 调用和 `transfer_v1_batch_end`。每个元组参数必须遵循 `(AccountId, AccountId, AssetDefinitionId, int)` 形状；单一转让保留现有的建设者。 |

Kotodama 用法示例：

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` 执行与单个 `Transfer::asset_numeric` 调用相同的权限和算术检查，但将所有增量记录在单个 `TransferTranscript` 内。多增量转录本会忽略海神摘要，直到每个增量承诺进入后续阶段。 Kotodama 构建器现在自动发出开始/结束系统调用，因此合约可以部署批量传输，而无需手动编码 Norito 有效负载。

## 行数回归工具

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) 使用可配置的选择器计数合成 FASTPQ 转换批次，并报告生成的 `row_usage` 摘要（`total_rows`、每个选择器计数、比率）以及填充的长度/log2。通过以下方式捕获 65536 行天花板的基准：

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```发出的 JSON 镜像了 `iroha_cli audit witness` 现在默认发出的 FASTPQ 批处理工件（通过 `--no-fastpq-batches` 来抑制它们），因此 `scripts/fastpq/check_row_usage.py` 和 CI 门可以在验证计划器更改时将合成运行与之前的快照进行比较。

# 推出计划

1. **TF-1（转录管道）**： ✅ `StateTransaction::record_transfer_transcripts` 现在为每个 `TransferAsset`/批次发出 Norito 转录本，`sumeragi::witness::record_fastpq_transcript` 将它们存储在全局见证中，`StateBlock::capture_exec_witness` 构建 `fastpq_batches`为操作员和验证通道提供显式 `public_inputs`（如果您需要更精简的，请使用 `--no-fastpq-batches`输出).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2（小工具实现）**： ✅ `gadgets::transfer` 现在验证多增量转录本（平衡算术 + Poseidon 摘要），在主机省略时合成配对的 SMT 证明，通过 `TransferGadgetPlan` 公开结构化见证，并且 `trace::build_trace` 将这些见证线程到 `Trace::transfer_witnesses`，同时从校样填充 SMT 列。 `fastpq_row_bench` 捕获 65536 行回归工具，因此规划人员无需重放即可跟踪行使用情况 Norito有效负载。【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3（批处理助手）**：启用批处理系统调用 + Kotodama 构建器，包括主机级顺序应用程序和小工具循环。
4. **TF-4（遥测和文档）**：更新 `fastpq_plan.md`、`fastpq_migration_guide.md` 和仪表板架构，以显示传输行与其他小工具的分配。

# 开放式问题

- **域限制**：当前的 FFT 规划器对于超过 21⁴ 行的跟踪会出现恐慌。 TF-2 应该提高域大小或记录减少的基准目标。
- **多资产批次**：初始小工具假定每个增量具有相同的资产 ID。如果我们需要异构批次，我们必须确保 Poseidon 见证人每次都包含该资产，以防止跨资产重放。
- **权限摘要重用**：从长远来看，我们可以将相同的摘要重用于其他授权操作，以避免每个系统调用重新计算签名者列表。


该文档跟踪设计决策；当里程碑落地时，使其与路线图条目保持同步。