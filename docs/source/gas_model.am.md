---
lang: am
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ጋዝ ሞዴል

ይህ ሰነድ የ Iroha ምናባዊ ማሽን ቀኖናዊ የጋዝ መርሃ ግብርን ይገልጻል
(IVM) እና መርሐ ግብሩ እንዴት እንደተሰረዘ እና እንደሚተገበር ያብራራል። የእውነት ምንጭ
ለወጪዎች `crates/ivm/src/gas.rs`; ከታች ያለው የጊዜ ሰሌዳ ሠንጠረዥ ቀርቧል
የዚያ ቀኖናዊ ካርታ እይታ።

## ወሰን

- ለ IVM ባይትኮድ አፈፃፀም (ተፈፃሚ :: Ivm) ይተገበራል።
- ቤተኛ የ ISI ጋዝ መለኪያ በ `crates/iroha_core/src/gas.rs` ውስጥ በተናጠል ይገለጻል.
- ISO 20022 opcodes በ ABI v1 ውስጥ የተጠበቁ ናቸው እና እስካሁን የጋዝ ግቤቶችን አልያዙም።

## ቁርጠኝነት እና የጊዜ ሰሌዳ ሃሽ

የጋዝ መርሃ ግብሩ የሚወስነው እና ከ opcode → የወጪ ጥንዶች የተገኘ ነው። የ
ቀኖናዊ ዳይጀስት በእያንዳንዱ ግቤት በታዘዘው የኦፕኮድ ዝርዝር ላይ ይሰላል
ተከታታይ እንደሚከተለው

- ኦፕኮድ ባይት (u8)
- ወጪ (u64, ትንሽ-ኤንዲያን)

ሃሽ የሚጋለጠው በ፡

- `ivm::gas::schedule_hash()` (ቀኖናዊ የጊዜ ሰሌዳ ሃሽ)
- `ivm::limits::schedule_hash()` (አስተናጋጅ ፊት ለፊት ተለዋጭ ስም)

ሽቦ በሚሰሩበት ጊዜ ሁሉም እኩዮች አንድ አይነት መርሐግብር እንደሚጋሩ ለማረጋገጥ ይህንን ማሟያ ይጠቀሙ
config ወይም telemetry ቼኮች.

## የቬክተር ስኬል እና ኤችቲኤም ሙከራዎች

- የቬክተር ኦፕስ (`VADD*`፣ `VAND`፣ `VXOR`፣ `VOR`፣ `VROT32`) ልኬት ከሎጂክ ጋር።
  የቬክተር ርዝመት በ `SETVL` ተቀናብሯል። በሠንጠረዡ ውስጥ ያሉት የመሠረት ወጪዎች በ
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (መሰረታዊ = 2 መስመሮች).
- የኤችቲኤም ሙከራዎች ወጪውን በ `(retries + 1)` ያባዛሉ; አብዛኞቹ የጋራ መግባባት መንገዶች አያደርጉም።
  እንደገና መሞከር

## ቀኖናዊ ኦፕኮድ ጋዝ ጠረጴዛ

ከዚህ በታች ያለው ሰንጠረዥ በ `ivm::gas::cost_of` ጥቅም ላይ የዋሉትን ወጪዎች ይዘረዝራል. የቬክተር ልኬት
እና የኤችቲኤም ሙከራዎች ከላይ እንደተገለፀው በእነዚህ መሰረታዊ እሴቶች ላይ ይተገበራሉ።| ምድብ | ኦፕኮድ | ማኒሞኒክ | ቤዝ ጋዝ |
|---|---:|---|---:|
| የሂሳብ | 0x01 | `ADD` | 1 |
| የሂሳብ | 0x02 | `SUB` | 1 |
| የሂሳብ | 0x03 | `AND` | 1 |
| የሂሳብ | 0x04 | `OR` | 1 |
| የሂሳብ | 0x05 | `XOR` | 1 |
| የሂሳብ | 0x06 | `SLL` | 1 |
| የሂሳብ | 0x07 | `SRL` | 1 |
| የሂሳብ | 0x08 | `SRA` | 1 |
| የሂሳብ | 0x0D | `NEG` | 1 |
| የሂሳብ | 0x0C | `NOT` | 1 |
| የሂሳብ | 0x20 | `ADDI` | 1 |
| የሂሳብ | 0x21 | `ANDI` | 1 |
| የሂሳብ | 0x22 | `ORI` | 1 |
| የሂሳብ | 0x23 | `XORI` | 1 |
| የሂሳብ | 0x10 | `MUL` | 3 |
| የሂሳብ | 0x11 | `MULH` | 3 |
| የሂሳብ | 0x12 | `MULHU` | 3 |
| የሂሳብ | 0x13 | `MULHSU` | 3 |
| የሂሳብ | 0x14 | `DIV` | 10 |
| የሂሳብ | 0x15 | `DIVU` | 10 |
| የሂሳብ | 0x16 | `REM` | 10 |
| የሂሳብ | 0x17 | `REMU` | 10 |
| የሂሳብ | 0x18 | `ROTL` | 2 |
| የሂሳብ | 0x19 | `ROTR` | 2 |
| የሂሳብ | 0x25 | `ROTL_IMM` | 2 |
| የሂሳብ | 0x26 | `ROTR_IMM` | 2 |
| የሂሳብ | 0x1A | `POPCNT` | 6 |
| የሂሳብ | 0x1B | `CLZ` | 6 |
| የሂሳብ | 0x1C | `CTZ` | 6 |
| የሂሳብ | 0x1D | `ISQRT` | 6 |
| የሂሳብ | 0x1E | `MIN` | 1 |
| የሂሳብ | 0x1F | `MAX` | 1 |
| የሂሳብ | 0x27 | `ABS` | 1 |
| የሂሳብ | 0x28 | `DIV_CEIL` | 12 |
| የሂሳብ | 0x29 | `GCD` | 12 |
| የሂሳብ | 0x2A | `MEAN` | 2 |
| የሂሳብ | 0x09 | `SLT` | 2 |
| የሂሳብ | 0x0A | `SLTU` | 2 |
| የሂሳብ | 0x0E | `SEQ` | 2 |
| የሂሳብ | 0x0F | `SNE` | 2 |
| የሂሳብ | 0x0B | `CMOV` | 3 |
| የሂሳብ | 0x24 | `CMOVI` | 3 |
| ትውስታ | 0x30 | `LOAD64` | 3 |
| ትውስታ | 0x31 | `STORE64` | 3 |
| ትውስታ | 0x32 | `LOAD128` | 5 |
| ትውስታ | 0x33 | `STORE128` | 5 |
| ቁጥጥር | 0x40 | `BEQ` | 1 |
| ቁጥጥር | 0x41 | `BNE` | 1 |
| ቁጥጥር | 0x42 | `BLT` | 1 |
| ቁጥጥር | 0x43 | `BGE` | 1 |
| ቁጥጥር | 0x44 | `BLTU` | 1 |
| ቁጥጥር | 0x45 | `BGEU` | 1 |
| ቁጥጥር | 0x46 | `JAL` | 2 |
| ቁጥጥር | 0x48 | `JALR` | 2 |
| ቁጥጥር | 0x47 | `JR` | 2 |
| ቁጥጥር | 0x4A | `JMP` | 2 |
| ቁጥጥር | 0x4B | `JALS` | 2 |
| ቁጥጥር | 0x49 | `HALT` | 0 |
| ስርዓት | 0x60 | `SCALL` | 5 |
| ስርዓት | 0x61 | `GETGAS` | 0 |
| crypto | 0x70 | `VADD32` | 2 |
| crypto | 0x71 | `VADD64` | 2 |
| crypto | 0x72 | `VAND` | 1 |
| crypto | 0x73 | `VXOR` | 1 |
| crypto | 0x74 | `VOR` | 1 |
| crypto | 0x75 | `VROT32` | 1 |
| crypto | 0x76 | `SETVL` | 1 |
| crypto | 0x77 | `PARBEGIN` | 0 |
| crypto | 0x78 | `PAREND` | 0 |
| crypto | 0x80 | `SHA256BLOCK` | 50 || crypto | 0x81 | `SHA3BLOCK` | 50 |
| crypto | 0x82 | `POSEIDON2` | 10 |
| crypto | 0x83 | `POSEIDON6` | 10 |
| crypto | 0x84 | `PUBKGEN` | 50 |
| crypto | 0x85 | `VALCOM` | 50 |
| crypto | 0x86 | `ECADD` | 20 |
| crypto | 0x87 | `ECMUL_VAR` | 100 |
| crypto | 0x8E | `PAIRING` | 500 |
| crypto | 0x88 | `AESENC` | 30 |
| crypto | 0x89 | `AESDEC` | 30 |
| crypto | 0x8A | `BLAKE2S` | 40 |
| crypto | 0x8B | `ED25519VERIFY` | 1000 |
| crypto | 0x8F | `ED25519BATCHVERIFY` | 500 |
| crypto | 0x8C | `ECDSAVERIFY` | 1500 |
| crypto | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |