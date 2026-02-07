---
lang: zh-hans
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 字节码头


魔法
- 4 个字节：偏移量 0 处的 ASCII `IVM\0`。

布局（当前）
- 偏移量和大小（总共 17 个字节）：
  - 0..4：魔法 `IVM\0`
  - 4：`version_major: u8`
  - 5：`version_minor: u8`
  - 6：`mode: u8`（功能位；见下文）
  - 7：`vector_length: u8`
  - 8..16：`max_cycles: u64`（小端）
  - 16：`abi_version: u8`

模式位
- `ZK = 0x01`、`VECTOR = 0x02`、`HTM = 0x04`（保留/功能门控）。

字段（含义）
- `abi_version`：系统调用表和指针 ABI 架构版本。
- `mode`：ZK 跟踪/VECTOR/HTM 的功能位。
- `vector_length`：向量操作的逻辑向量长度（0 → 未设置）。
- `max_cycles`：ZK 模式和准入中使用的执行填充边界。

注释
- 字节序和布局由实现定义并绑定到 `version`。上面的在线布局反映了 `crates/ivm_abi/src/metadata.rs` 中的当前实现。
- 最小读者可以依赖此布局来获取当前工件，并应通过 `version` 门控处理未来的更改。
- 每个主机可选择硬件加速（SIMD/Metal/CUDA）。运行时从 `iroha_config` 读取 `AccelerationConfig` 值：`enable_simd` 在为 false 时强制标量回退，而 `enable_metal` 和 `enable_cuda` 即使在编译时也会对其各自的后端进行门控。这些切换在创建 VM 之前通过 `ivm::set_acceleration_config` 应用。
- 移动 SDK (Android/Swift) 具有相同的旋钮； `IrohaSwift.AccelerationSettings`
  调用 `connect_norito_set_acceleration_config`，因此 macOS/iOS 版本可以选择使用 Metal /
  NEON 同时保持确定性回退。
- 操作员还可以通过导出 `IVM_DISABLE_METAL=1` 或 `IVM_DISABLE_CUDA=1` 来强制禁用特定后端进行诊断。这些环境覆盖优先于配置，并使虚拟机保持在确定性 CPU 路径上。

持久状态助手和 ABI 表面
- 持久状态帮助程序系统调用（0x50–0x5A：STATE_{GET,SET,DEL}、ENCODE/DECODE_INT、BUILD_PATH_* 和 JSON/SCHEMA 编码/解码）是 V1 ABI 的一部分，并包含在 `abi_hash` 计算中。
- CoreHost 将 STATE_{GET,SET,DEL} 连接到 WSV 支持的持久智能合约状态；开发/测试主机可以使用覆盖或本地持久性，但必须保留相同的可观察行为。

验证
- 节点准入仅接受 `version_major = 1` 和 `version_minor = 0` 标头。
- `mode` 必须仅包含已知位：`ZK`、`VECTOR`、`HTM`（拒绝未知位）。
- `vector_length` 是建议性的，即使未设置 `VECTOR` 位，也可能为非零；准入仅强制执行上限。
- 支持的 `abi_version` 值：第一个版本仅接受 `1` (V1)；其他值在入学时被拒绝。

### 政策（生成）
以下政策摘要是在实施过程中生成的，不应手动编辑。<!-- BEGIN GENERATED HEADER POLICY -->
|领域 |政策 |
|---|---|
|主要版本 | 1 |
|次要版本 | 0 |
|模式（已知位）| 0x07（ZK=0x01，矢量=0x02，HTM=0x04）|
| abi_版本 | 1 |
|向量长度| 0 或 1..=64（建议；与 VECTOR 位无关）|
<!-- END GENERATED HEADER POLICY -->

### ABI 哈希（生成）
下表是根据实现生成的，并列出了受支持策略的规范 `abi_hash` 值。

<!-- BEGIN GENERATED ABI HASHES -->
|政策 | abi_hash（十六进制）|
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- 小更新可能会在 `feature_bits` 后面添加指令并保留操作码空间；主要更新可能会更改编码或仅与协议升级一起删除/重新调整用途。
- 系统调用范围稳定；对于活动的 `abi_version` 未知，产生 `E_SCALL_UNKNOWN`。
- Gas Schedule 与 `version` 绑定，并且需要更改时的黄金向量。

检查工件
- 使用 `ivm_tool inspect <file.to>` 获得稳定的标头字段视图。
- 对于开发，示例/包括一个小型 Makefile 目标 `examples-inspect`，该目标对构建的工件运行检查。

示例（Rust）：最小魔法+尺寸检查

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

注意：超出魔法范围的确切标题布局是版本化的并且是实现定义的；更喜欢 `ivm_tool inspect` 以获得稳定的字段名称和值。