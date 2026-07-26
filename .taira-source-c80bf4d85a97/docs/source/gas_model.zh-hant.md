---
lang: zh-hant
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 氣體模型

本文檔定義了 Iroha 虛擬機的規範 Gas Schedule
(IVM) 並解釋瞭如何對計劃進行散列和應用。真理的來源
成本為 `crates/ivm/src/gas.rs`；下面的時間表是渲染的
該規範映射的視圖。

## 範圍

- 適用於 IVM 字節碼執行 (Executable::Ivm)。
- 本機 ISI 燃氣計量在 `crates/iroha_core/src/gas.rs` 中單獨定義。
- ISO 20022 操作碼在 ABI v1 中保留，尚不攜帶氣體條目。

## 確定性和調度哈希

Gas 調度是確定性的，源自操作碼 → 成本對。的
規範摘要是通過每個條目的有序操作碼列表計算的
序列化為：

- 操作碼字節 (u8)
- 成本（u64，小端）

哈希值通過以下方式公開：

- `ivm::gas::schedule_hash()`（規範時間表哈希）
- `ivm::limits::schedule_hash()`（面向主機的別名）

使用此摘要來驗證所有對等方在接線時是否共享相同的時間表
配置或遙測檢查。

## 矢量縮放和 HTM 重試

- 向量操作（`VADD*`、`VAND`、`VXOR`、`VOR`、`VROT32`）與邏輯縮放
  向量長度由 `SETVL` 設置。表中的基本成本按比例縮放
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES`（基線 = 2 條車道）。
- HTM重試將成本乘以`(retries + 1)`；大多數共識路徑沒有
  進行重試。

## 規範操作碼氣體表

下表列出了 `ivm::gas::cost_of` 使用的基本成本。矢量縮放
如上所述，HTM 重試應用於這些基值之上。|類別 |操作碼|助記詞 |基礎氣體|
|---|---:|---|---:|
|算術| 0x01 | 0x01 `ADD` | 1 |
|算術| 0x02 | 0x02 `SUB` | 1 |
|算術| 0x03 | 0x03 `AND` | 1 |
|算術| 0x04 | 0x04 `OR` | 1 |
|算術| 0x05 | 0x05 `XOR` | 1 |
|算術| 0x06 | 0x06 `SLL` | 1 |
|算術| 0x07 | 0x07 `SRL` | 1 |
|算術| 0x08 | 0x08 `SRA` | 1 |
|算術| 0x0D | 0x0D `NEG` | 1 |
|算術| 0x0C | 0x0C `NOT` | 1 |
|算術| 0x20 | 0x20 `ADDI` | 1 |
|算術| 0x21 | 0x21 `ANDI` | 1 |
|算術| 0x22 | 0x22 `ORI` | 1 |
|算術| 0x23 | 0x23 `XORI` | 1 |
|算術| 0x10 | 0x10 `MUL` | 3 |
|算術| 0x11 | 0x11 `MULH` | 3 |
|算術| 0x12 | 0x12 `MULHU` | 3 |
|算術| 0x13 | 0x13 `MULHSU` | 3 |
|算術| 0x14 | 0x14 `DIV` | 10 | 10
|算術| 0x15 | 0x15 `DIVU` | 10 | 10
|算術| 0x16 | 0x16 `REM` | 10 | 10
|算術| 0x17 | 0x17 `REMU` | 10 | 10
|算術| 0x18 | 0x18 `ROTL` | 2 |
|算術| 0x19 | 0x19 `ROTR` | 2 |
|算術| 0x25 | 0x25 `ROTL_IMM` | 2 |
|算術| 0x26 | 0x26 `ROTR_IMM` | 2 |
|算術| 0x1A | 0x1A `POPCNT` | 6 |
|算術| 0x1B | 0x1B `CLZ` | 6 |
|算術| 0x1C | 0x1C `CTZ` | 6 |
|算術| 0x1D | 0x1D `ISQRT` | 6 |
|算術| 0x1E | 0x1E `MIN` | 1 |
|算術| 0x1F | 0x1F `MAX` | 1 |
|算術| 0x27 | 0x27 `ABS` | 1 |
|算術| 0x28 | 0x28 `DIV_CEIL` | 12 | 12
|算術| 0x29 | 0x29 `GCD` | 12 | 12
|算術| 0x2A | 0x2A `MEAN` | 2 |
|算術| 0x09 | 0x09 `SLT` | 2 |
|算術| 0x0A | 0x0A `SLTU` | 2 |
|算術| 0x0E| `SEQ` | 2 |
|算術| 0x0F| `SNE` | 2 |
|算術| 0x0B | 0x0B `CMOV` | 3 |
|算術| 0x24 | 0x24 `CMOVI` | 3 |
|記憶 | 0x30 | 0x30 `LOAD64` | 3 |
|記憶 | 0x31 | 0x31 `STORE64` | 3 |
|記憶 | 0x32 | 0x32 `LOAD128` | 5 |
|記憶 | 0x33 | 0x33 `STORE128` | 5 |
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
|系統| 0x60 | 0x60 `SCALL` | 5 |
|系統| 0x61 | 0x61 `GETGAS` | 0 |
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
|加密 | 0x8E| `PAIRING` | 500 | 500
|加密 | 0x88 | 0x88 `AESENC` | 30|
|加密 | 0x89 | 0x89 `AESDEC` | 30|
|加密 | 0x8A| `BLAKE2S` | 40 | 40
|加密 | 0x8B | 0x8B `ED25519VERIFY` | 1000 | 1000
|加密 | 0x8F| `ED25519BATCHVERIFY` | 500 | 500
|加密 | 0x8C | 0x8C `ECDSAVERIFY` | 1500 | 1500
|加密 | 0x8D | 0x8D `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | 0xA0 `ASSERT` | 1 |
| zk | 0xA1 | 0xA1 `ASSERT_EQ` | 1 |
| zk | 0xA2 | 0xA2 `FADD` | 1 |
| zk | 0xA3 | 0xA3 `FSUB` | 1 |
| zk | 0xA4 | 0xA4 `FMUL` | 3 |
| zk | 0xA5 | 0xA5 `FINV` | 5 |
| zk | 0xA6 | 0xA6 `ASSERT_RANGE` | 1 |