---
lang: ja
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ガスモデル

このドキュメントでは、Iroha 仮想マシンの正規のガス スケジュールを定義します。
(IVM) スケジュールがどのようにハッシュ化されて適用されるかについて説明します。真実の情報源
コストは `crates/ivm/src/gas.rs` です。以下のスケジュール表はレンダリングされたものです
その正規マッピングのビュー。

## 範囲

- IVM バイトコード実行 (Executable::Ivm) に適用されます。
- ネイティブ ISI ガス計測は、`crates/iroha_core/src/gas.rs` で別途定義されます。
- ISO 20022 オペコードは ABI v1 で予約されており、ガス エントリはまだ含まれていません。

## 決定論とスケジュールハッシュ

ガススケジュールは決定的であり、オペコード→コストのペアから導出されます。の
正規ダイジェストは、各エントリの順序付けされたオペコード リストに基づいて計算されます。
としてシリアル化されます:

- オペコードバイト (u8)
- コスト (u64、リトルエンディアン)

ハッシュは次の方法で公開されます。

- `ivm::gas::schedule_hash()` (正規スケジュール ハッシュ)
- `ivm::limits::schedule_hash()` (ホスト側のエイリアス)

このダイジェストを使用して、配線時にすべてのピアが同じスケジュールを共有していることを確認します。
構成またはテレメトリのチェック。

## ベクトルのスケーリングと HTM の再試行

- ベクトル演算 (`VADD*`、`VAND`、`VXOR`、`VOR`、`VROT32`) は論理
  ベクトルの長さは `SETVL` によって設定されます。表の基本コストは次のようにスケールされます。
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (ベースライン = 2 レーン)。
- HTM の再試行では、コストに `(retries + 1)` が乗算されます。ほとんどのコンセンサスパスはそうではありません
  再試行が発生します。

## 正規のオペコードガステーブル

以下の表に、`ivm::gas::cost_of` で使用される基本コストを示します。ベクトルのスケーリング
HTM 再試行は、上で説明したように、これらの基本値に基づいて適用されます。|カテゴリー |オペコード |ニーモニック |ベースガス |
|---|---:|---|---:|
|算術 | 0x01 | `ADD` | 1 |
|算術 | 0x02 | `SUB` | 1 |
|算術 | 0x03 | `AND` | 1 |
|算術 | 0x04 | `OR` | 1 |
|算術 | 0x05 | `XOR` | 1 |
|算術 | 0x06 | `SLL` | 1 |
|算術 | 0x07 | `SRL` | 1 |
|算術 | 0x08 | `SRA` | 1 |
|算術 | 0x0D | `NEG` | 1 |
|算術 | 0x0C | `NOT` | 1 |
|算術 | 0x20 | `ADDI` | 1 |
|算術 | 0x21 | `ANDI` | 1 |
|算術 | 0x22 | `ORI` | 1 |
|算術 | 0x23 | `XORI` | 1 |
|算術 | 0x10 | `MUL` | 3 |
|算術 | 0x11 | `MULH` | 3 |
|算術 | 0x12 | `MULHU` | 3 |
|算術 | 0x13 | `MULHSU` | 3 |
|算術 | 0x14 | `DIV` | 10 |
|算術 | 0x15 | `DIVU` | 10 |
|算術 | 0x16 | `REM` | 10 |
|算術 | 0x17 | `REMU` | 10 |
|算術 | 0x18 | `ROTL` | 2 |
|算術 | 0x19 | `ROTR` | 2 |
|算術 | 0x25 | `ROTL_IMM` | 2 |
|算術 | 0x26 | `ROTR_IMM` | 2 |
|算術 | 0x1A | `POPCNT` | 6 |
|算術 | 0x1B | `CLZ` | 6 |
|算術 | 0x1C | `CTZ` | 6 |
|算術 | 0x1D | `ISQRT` | 6 |
|算術 | 0x1E | `MIN` | 1 |
|算術 | 0x1F | `MAX` | 1 |
|算術 | 0x27 | `ABS` | 1 |
|算術 | 0x28 | `DIV_CEIL` | 12 |
|算術 | 0x29 | `GCD` | 12 |
|算術 | 0x2A | `MEAN` | 2 |
|算術 | 0x09 | `SLT` | 2 |
|算術 | 0x0A | `SLTU` | 2 |
|算術 | 0x0E | `SEQ` | 2 |
|算術 | 0x0F | `SNE` | 2 |
|算術 | 0x0B | `CMOV` | 3 |
|算術 | 0x24 | `CMOVI` | 3 |
|記憶 | 0x30 | `LOAD64` | 3 |
|記憶 | 0x31 | `STORE64` | 3 |
|記憶 | 0x32 | `LOAD128` | 5 |
|記憶 | 0x33 | `STORE128` | 5 |
|コントロール | 0x40 | `BEQ` | 1 |
|コントロール | 0x41 | `BNE` | 1 |
|コントロール | 0x42 | `BLT` | 1 |
|コントロール | 0x43 | `BGE` | 1 |
|コントロール | 0x44 | `BLTU` | 1 |
|コントロール | 0x45 | `BGEU` | 1 |
|コントロール | 0x46 | `JAL` | 2 |
|コントロール | 0x48 | `JALR` | 2 |
|コントロール | 0x47 | `JR` | 2 |
|コントロール | 0x4A | `JMP` | 2 |
|コントロール | 0x4B | `JALS` | 2 |
|コントロール | 0x49 | `HALT` | 0 |
|システム | 0x60 | `SCALL` | 5 |
|システム | 0x61 | `GETGAS` | 0 |
|暗号 | 0x70 | `VADD32` | 2 |
|暗号 | 0x71 | `VADD64` | 2 |
|暗号 | 0x72 | `VAND` | 1 |
|暗号 | 0x73 | `VXOR` | 1 |
|暗号 | 0x74 | `VOR` | 1 |
|暗号 | 0x75 | `VROT32` | 1 |
|暗号 | 0x76 | `SETVL` | 1 |
|暗号 | 0x77 | `PARBEGIN` | 0 |
|暗号 | 0x78 | `PAREND` | 0 |
|暗号 | 0x80 | `SHA256BLOCK` | 50 ||暗号 | 0x81 | `SHA3BLOCK` | 50 |
|暗号 | 0x82 | `POSEIDON2` | 10 |
|暗号 | 0x83 | `POSEIDON6` | 10 |
|暗号 | 0x84 | `PUBKGEN` | 50 |
|暗号 | 0x85 | `VALCOM` | 50 |
|暗号 | 0x86 | `ECADD` | 20 |
|暗号 | 0x87 | `ECMUL_VAR` | 100 |
|暗号 | 0x8E | `PAIRING` | 500 |
|暗号 | 0x88 | `AESENC` | 30 |
|暗号 | 0x89 | `AESDEC` | 30 |
|暗号 | 0x8A | `BLAKE2S` | 40 |
|暗号 | 0x8B | `ED25519VERIFY` | 1000 |
|暗号 | 0x8F | `ED25519BATCHVERIFY` | 500 |
|暗号 | 0x8C | `ECDSAVERIFY` | 1500 |
|暗号 | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |