---
lang: zh-hans
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 气体模型

本文档定义了 Iroha 虚拟机的规范 Gas Schedule
(IVM) 并解释了如何对计划进行散列和应用。真理的来源
成本为 `crates/ivm/src/gas.rs`；下面的时间表是渲染的
该规范映射的视图。

## 范围

- 适用于 IVM 字节码执行 (Executable::Ivm)。
- 本机 ISI 燃气计量在 `crates/iroha_core/src/gas.rs` 中单独定义。
- ISO 20022 操作码在 ABI v1 中保留，尚不携带气体条目。

## 确定性和调度哈希

Gas 调度是确定性的，源自操作码 → 成本对。的
规范摘要是通过每个条目的有序操作码列表计算的
序列化为：

- 操作码字节 (u8)
- 成本（u64，小端）

哈希值通过以下方式公开：

- `ivm::gas::schedule_hash()`（规范时间表哈希）
- `ivm::limits::schedule_hash()`（面向主机的别名）

使用此摘要来验证所有对等方在接线时是否共享相同的时间表
配置或遥测检查。

## 矢量缩放和 HTM 重试

- 向量操作（`VADD*`、`VAND`、`VXOR`、`VOR`、`VROT32`）与逻辑缩放
  向量长度由 `SETVL` 设置。表中的基本成本按比例缩放
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES`（基线 = 2 条车道）。
- HTM重试将成本乘以`(retries + 1)`；大多数共识路径没有
  进行重试。

## 规范操作码气体表

下表列出了 `ivm::gas::cost_of` 使用的基本成本。矢量缩放
如上所述，HTM 重试应用于这些基值之上。|类别 |操作码 |助记词|基础气体|
|---|---:|---|---:|
|算术| 0x01 | 0x01 `ADD` | 1 |
|算术| 0x02 | 0x02 `SUB` | 1 |
|算术| 0x03 | 0x03 `AND` | 1 |
|算术| 0x04 | 0x04 `OR` | 1 |
|算术| 0x05 | 0x05 `XOR` | 1 |
|算术| 0x06 | 0x06 `SLL` | 1 |
|算术| 0x07 | 0x07 `SRL` | 1 |
|算术| 0x08 | 0x08 `SRA` | 1 |
|算术| 0x0D | 0x0D `NEG` | 1 |
|算术| 0x0C | 0x0C `NOT` | 1 |
|算术| 0x20 | 0x20 `ADDI` | 1 |
|算术| 0x21 | 0x21 `ANDI` | 1 |
|算术| 0x22 | 0x22 `ORI` | 1 |
|算术| 0x23 | 0x23 `XORI` | 1 |
|算术| 0x10 | 0x10 `MUL` | 3 |
|算术| 0x11 | 0x11 `MULH` | 3 |
|算术| 0x12 | 0x12 `MULHU` | 3 |
|算术| 0x13 | 0x13 `MULHSU` | 3 |
|算术| 0x14 | 0x14 `DIV` | 10 | 10
|算术| 0x15 | 0x15 `DIVU` | 10 | 10
|算术| 0x16 | 0x16 `REM` | 10 | 10
|算术| 0x17 | 0x17 `REMU` | 10 | 10
|算术| 0x18 | 0x18 `ROTL` | 2 |
|算术| 0x19 | 0x19 `ROTR` | 2 |
|算术| 0x25 | 0x25 `ROTL_IMM` | 2 |
|算术| 0x26 | 0x26 `ROTR_IMM` | 2 |
|算术| 0x1A | 0x1A `POPCNT` | 6 |
|算术| 0x1B | 0x1B `CLZ` | 6 |
|算术| 0x1C | 0x1C `CTZ` | 6 |
|算术| 0x1D | 0x1D `ISQRT` | 6 |
|算术| 0x1E | 0x1E `MIN` | 1 |
|算术| 0x1F | 0x1F `MAX` | 1 |
|算术| 0x27 | 0x27 `ABS` | 1 |
|算术| 0x28 | 0x28 `DIV_CEIL` | 12 | 12
|算术| 0x29 | 0x29 `GCD` | 12 | 12
|算术| 0x2A | 0x2A `MEAN` | 2 |
|算术| 0x09 | 0x09 `SLT` | 2 |
|算术| 0x0A | 0x0A `SLTU` | 2 |
|算术| 0x0E| `SEQ` | 2 |
|算术| 0x0F| `SNE` | 2 |
|算术| 0x0B | 0x0B `CMOV` | 3 |
|算术| 0x24 | 0x24 `CMOVI` | 3 |
|记忆 | 0x30 | 0x30 `LOAD64` | 3 |
|记忆 | 0x31 | 0x31 `STORE64` | 3 |
|记忆 | 0x32 | 0x32 `LOAD128` | 5 |
|记忆 | 0x33 | 0x33 `STORE128` | 5 |
|控制| 0x40 | 0x40 `BEQ` | 1 |
|控制| 0x41 | 0x41 `BNE` | 1 |
|控制| 0x42 | 0x42 `BLT` | 1 |
|控制| 0x43 | 0x43 `BGE` | 1 |
|控制| 0x44 | 0x44 `BLTU` | 1 |
|控制| 0x45 | 0x45 `BGEU` | 1 |
|控制| 0x46 | 0x46 `JAL` | 2 |
|控制| 0x48 | 0x48 `JALR` | 2 |
|控制| 0x47 | 0x47 `JR` | 2 |
|控制| 0x4A | 0x4A `JMP` | 2 |
|控制| 0x4B | 0x4B `JALS` | 2 |
|控制| 0x49 | 0x49 `HALT` | 0 |
|系统| 0x60 | 0x60 `SCALL` | 5 |
|系统| 0x61 | 0x61 `GETGAS` | 0 |
|加密 | 0x70 | 0x70 `VADD32` | 2 |
|加密 | 0x71 | 0x71 `VADD64` | 2 |
|加密 | 0x72 | 0x72 `VAND` | 1 |
|加密 | 0x73 | 0x73 `VXOR` | 1 |
|加密 | 0x74 | 0x74 `VOR` | 1 |
|加密 | 0x75 | 0x75 `VROT32` | 1 |
|加密 | 0x76 | 0x76 `SETVL` | 1 |
|加密 | 0x77 | 0x77 `PARBEGIN` | 0 |
|加密 | 0x78 | 0x78 `PAREND` | 0 |
|加密 | 0x80 | 0x80 `SHA256BLOCK` | 50 | 50|加密 | 0x81 | 0x81 `SHA3BLOCK` | 50 | 50
|加密 | 0x82 | 0x82 `POSEIDON2` | 10 | 10
|加密 | 0x83 | 0x83 `POSEIDON6` | 10 | 10
|加密 | 0x84 | 0x84 `PUBKGEN` | 50 | 50
|加密 | 0x85 | 0x85 `VALCOM` | 50 | 50
|加密 | 0x86 | 0x86 `ECADD` | 20 |
|加密 | 0x87 | 0x87 `ECMUL_VAR` | 100 | 100
|加密 | 0x8E| `PAIRING` | 500 |
|加密 | 0x88 | 0x88 `AESENC` | 30|
|加密 | 0x89 | 0x89 `AESDEC` | 30|
|加密 | 0x8A| `BLAKE2S` | 40|
|加密 | 0x8B | 0x8B `ED25519VERIFY` | 1000 | 1000
|加密 | 0x8F| `ED25519BATCHVERIFY` | 500 |
|加密 | 0x8C | 0x8C `ECDSAVERIFY` | 1500 | 1500
|加密 | 0x8D | 0x8D `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | 0xA0 `ASSERT` | 1 |
| zk | 0xA1 | 0xA1 `ASSERT_EQ` | 1 |
| zk | 0xA2 | 0xA2 `FADD` | 1 |
| zk | 0xA3 | 0xA3 `FSUB` | 1 |
| zk | 0xA4 | 0xA4 `FMUL` | 3 |
| zk | 0xA5 | 0xA5 `FINV` | 5 |
| zk | 0xA6 | 0xA6 `ASSERT_RANGE` | 1 |