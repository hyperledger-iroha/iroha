---
lang: zh-hans
direction: ltr
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-04T08:19:26.508527+00:00"
translation_last_reviewed: 2026-02-07
title: Governance ZK Vote Tally
translator: machine-google-reviewed
---

## 概述

Iroha 的治理计数流程依赖于 Halo2/IPA 电路，该电路验证比特投票承诺及其在合格选民集中的成员资格。本注释捕获电路参数、公共输入和审核装置，以便审核人员可以重新生成测试中使用的验证密钥和证明。

## 电路总结

- **电路标识符**：`halo2/pasta/vote-bool-commit-merkle8-v1`
- **实现**：`VoteBoolCommitMerkle::<8>` 中的 `iroha_core::zk::depth`
- **域名大小**：`k = 6`
- **后端**：透明Halo2/IPA over Pasta（ZK1信封：`IPAK` + `H2VK`用于VK，`PROF` + `I10P`用于校样）
- **见证形状**：
  - 选票位 `v ∈ {0,1}`
  - 随机标量 `ρ`
  - Merkle 路径的八个同级标量
  - 方向位（参考见证中全为零）
- **Merkle 压缩器**：`H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)`，其中 `p` 是 Pasta 标量模数
- **公共投入**：
  - 第 0 列：`commit`
  - 第 1 列：Merkle 根
  - 通过 `I10P` TLV 暴露（`cols = 2`、`rows = 1`）

### 电路布局

- **建议栏**：
  - `v` – 选票位限制为布尔值。
  - `ρ` – 投票承诺中使用的致盲标量。
  - `sibling[i]` for `i ∈ [0, 7]` – 深度 `i` 处的 Merkle 路径元素。
  - `dir[i]` 用于 `i ∈ [0, 7]` – 方向位选择左 (`0`) 或右 (`1`) 分支。
  - `node[i]` for `i ∈ [0, 7]` – 深度 `i` 后的 Merkle 累加器。
- **实例列**：
  - `commit` – 选民发布的公开承诺。
  - `root` – 合格选民集的 Merkle 根。
- **选择器**：`s_vote` 启用单个填充行上的门。

所有建议单元格都分配在该区域的第一行（也是唯一一行）；该电路使用 `SimpleFloorPlanner`。

### 门系统

令 `H` 为上面定义的压缩器，`prev_0 = H(v, ρ)` 为。门强制执行：

1. `s_vote · v · (v - 1) = 0` – 布尔选票位。
2. `s_vote · (H(v, ρ) - commit) = 0` – 承诺一致性。
3. 对于每个深度 `i`：
   - `s_vote · dir[i] · (dir[i] - 1) = 0` – 布尔路径方向。
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` – 累加器等于公共 Merkle 根。

压缩器仅使用五次形状；不需要查找表。所有算术都在 Pasta 标量字段中执行，行计数 `k = 6` 分配 `2^k = 64` 行 — 仅填充第 0 行。

### 规范夹具

确定性工具 (`zk_testkit::vote_merkle8_bundle`) 向见证人填充：

- `v = 1`
- `ρ = 12345`
- `sibling[i] = 10 + i` 为 `i ∈ [0, 7]`
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])` 与 `node[-1] = H(v, ρ)`

这产生了公共价值观：

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

验证密钥注册表中记录的 `public_inputs_schema_hash` 是 `blake2b-256(commit_bytes || root_bytes)`，最低有效位强制为 `1`，产生：

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### 验证密钥记录

治理将验证者注册为：- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)`（32字节摘要）

规范捆绑包包括一个内联验证密钥 (`key = Some(VerifyingKeyBox { … })`) 以及证明信封。 `vk_len`、`max_proof_bytes` 和可选元数据 URI 是从生成的工件中填充的。

## 参考赛程

使用 `cargo xtask zk-vote-tally-bundle --print-hashes` 重新生成集成测试消耗的内联验证密钥和证明包（默认情况下输出位于 `fixtures/zk/vote_tally/` 中）。该命令打印一个简短的摘要（`backend`、`commit`、`root`、模式哈希、长度）以及可选的文件哈希，以便审核员可以捕获证明注释。传递 `--summary-json -` 以发出与 JSON 相同的数据（或提供将其写入磁盘的路径）。传递 `--attestation attestation.json`（或标准输出的 `-`）来编写 Norito JSON 清单，其中包含摘要以及每个捆绑包工件的 Blake2b-256 摘要和大小，以便可以使用固定装置存档证明数据包。使用 `--verify` 运行时，提供 `--attestation <path>` 检查清单的捆绑包元数据和工件长度是否与新生成的捆绑包相匹配（它不会比较每次运行的证明摘要，该摘要会随转录随机性而变化）。

重新生成规范的装置和清单：

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

验证签入的工件是否保持最新（要求固定装置目录包含基线包）：

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

清单示例：

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

将当前清单存储在规范工件旁边（例如 `fixtures/zk/vote_tally/bundle.attestation.json`）。上游存储库将此目录保持为空，以避免提交大型二进制包，因此在依赖 `--verify` 之前在本地播种。

`generated_unix_ms` 是从承诺/验证密钥指纹确定性派生的，因此它在再生过程中保持稳定。生成器使用固定的 ChaCha20 转录本，因此元数据、验证密钥和证明信封哈希是可重现的。现在，任何摘要不匹配都表明必须调查漂移。审核员应记录发出的值以及他们证明的伪影。

工作流程提醒：

1. 运行 `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json` 在本地为捆绑包做种。
2. 根据需要提交或归档生成的工件。
3. 在后续重新生成时使用 `--verify` 以确保证明与规范包匹配。

该任务在内部运行 `xtask/src/vote_tally.rs` 中的确定性生成器，其中：

1. 对证人进行采样（`v = 1`、`ρ = 12345`、兄弟姐妹 `10..17`）
2. 运行 `keygen_vk`/`keygen_pk`
3. 生成 Halo2 证明并将其包装在 ZK1 信封中（包括公共实例）
4. 使用适当的 `public_inputs_schema_hash` 发出验证密钥记录

## 篡改覆盖率

`crates/iroha_core/tests/zk_vote_tally_audit.rs` 加载捆绑包并检查：

- 真实证据与捆绑的内联 VK 进行验证。
- 翻转承诺列中的任何字节都会导致验证失败。
- 翻转根列中的任何字节都会导致验证失败。这些回归测试保证 Torii（和主机）拒绝其公共输入在证明生成后被篡改的信封。

使用以下命令在本地运行回归：

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## 审核清单

1. 检查 `VoteBoolCommitMerkle::<8>` 的约束完整性和常数选择。
2. 重新运行 `cargo xtask zk-vote-tally-bundle --verify --print-hashes` 以重现 VK/proof 并确认记录的哈希值。
3. 确认 Torii 的计数处理程序使用相同的后端标识符和信封布局。
4. 执行篡改回归以确保变异证明无法通过验证。
5. 对 `bundle.attestation.json` 输出 (Blake2b-256) 进行散列和八卦，以便审阅者可以在其证明旁边记录规范清单。