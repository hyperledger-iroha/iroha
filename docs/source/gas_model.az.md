---
lang: az
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Qaz Modeli

Bu sənəd Iroha Virtual Maşın üçün kanonik qaz cədvəlini müəyyən edir
(IVM) və cədvəlin necə hash edildiyini və tətbiq olunduğunu izah edir. Həqiqətin mənbəyi
xərclər üçün `crates/ivm/src/gas.rs`; aşağıdakı cədvəl cədvəli verilmişdir
həmin kanonik xəritənin görünüşü.

## Əhatə dairəsi

- IVM bayt kodunun icrasına aiddir (İcra edilə bilən::Ivm).
- Yerli ISI qazının ölçülməsi `crates/iroha_core/src/gas.rs`-də ayrıca müəyyən edilmişdir.
- ISO 20022 əməliyyat kodları ABI v1-də qorunur və hələ qaz girişlərini daşımır.

## Determinizm və qrafik hash

Qaz cədvəli deterministikdir və əməliyyat kodu → xərc cütlərindən əldə edilir. The
kanonik həzm hər bir giriş ilə sifariş edilmiş əməliyyat kodu siyahısı üzərində hesablanır
kimi seriallaşdırılır:

- əməliyyat kodu baytı (u8)
- xərc (u64, az-endian)

Hash ifşa olunur:

- `ivm::gas::schedule_hash()` (kanonik cədvəl hash)
- `ivm::limits::schedule_hash()` (host ləqəb)

Naqil çəkərkən bütün həmyaşıdların eyni cədvəli paylaşdığını yoxlamaq üçün bu həzmdən istifadə edin
konfiqurasiya və ya telemetriya yoxlamaları.

## Vektor miqyası və HTM sınaqları

- Vektor əməliyyatları (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) məntiqi ilə
  vektor uzunluğu `SETVL` tərəfindən təyin edilmişdir. Cədvəldəki əsas məsrəflər miqyası ilə ölçülür
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (əsas xətt = 2 zolaq).
- HTM təkrar cəhdləri dəyəri `(retries + 1)`-ə vurur; əksər konsensus yolları yoxdur
  təkrar cəhdlərə məruz qalır.

## Kanonik əməliyyat kodu qaz cədvəli

Aşağıdakı cədvəldə `ivm::gas::cost_of` tərəfindən istifadə edilən əsas xərclərin siyahısı verilmişdir. Vektor miqyası
və HTM təkrar cəhdləri yuxarıda qeyd edildiyi kimi bu əsas dəyərlərin üzərinə tətbiq edilir.| Kateqoriya | Əməliyyat kodu | Mnemonik | Əsas qaz |
|---|---:|---|---:|
| arifmetik | 0x01 | `ADD` | 1 |
| arifmetik | 0x02 | `SUB` | 1 |
| arifmetik | 0x03 | `AND` | 1 |
| arifmetik | 0x04 | `OR` | 1 |
| arifmetik | 0x05 | `XOR` | 1 |
| arifmetik | 0x06 | `SLL` | 1 |
| arifmetik | 0x07 | `SRL` | 1 |
| arifmetik | 0x08 | `SRA` | 1 |
| arifmetik | 0x0D | `NEG` | 1 |
| arifmetik | 0x0C | `NOT` | 1 |
| arifmetik | 0x20 | `ADDI` | 1 |
| arifmetik | 0x21 | `ANDI` | 1 |
| arifmetik | 0x22 | `ORI` | 1 |
| arifmetik | 0x23 | `XORI` | 1 |
| arifmetik | 0x10 | `MUL` | 3 |
| arifmetik | 0x11 | `MULH` | 3 |
| arifmetik | 0x12 | `MULHU` | 3 |
| arifmetik | 0x13 | `MULHSU` | 3 |
| arifmetik | 0x14 | `DIV` | 10 |
| arifmetik | 0x15 | `DIVU` | 10 |
| arifmetik | 0x16 | `REM` | 10 |
| arifmetik | 0x17 | `REMU` | 10 |
| arifmetik | 0x18 | `ROTL` | 2 |
| arifmetik | 0x19 | `ROTR` | 2 |
| arifmetik | 0x25 | `ROTL_IMM` | 2 |
| arifmetik | 0x26 | `ROTR_IMM` | 2 |
| arifmetik | 0x1A | `POPCNT` | 6 |
| arifmetik | 0x1B | `CLZ` | 6 |
| arifmetik | 0x1C | `CTZ` | 6 |
| arifmetik | 0x1D | `ISQRT` | 6 |
| arifmetik | 0x1E | `MIN` | 1 |
| arifmetik | 0x1F | `MAX` | 1 |
| arifmetik | 0x27 | `ABS` | 1 |
| arifmetik | 0x28 | `DIV_CEIL` | 12 |
| arifmetik | 0x29 | `GCD` | 12 |
| arifmetik | 0x2A | `MEAN` | 2 |
| arifmetik | 0x09 | `SLT` | 2 |
| arifmetik | 0x0A | `SLTU` | 2 |
| arifmetik | 0x0E | `SEQ` | 2 |
| arifmetik | 0x0F | `SNE` | 2 |
| arifmetik | 0x0B | `CMOV` | 3 |
| arifmetik | 0x24 | `CMOVI` | 3 |
| yaddaş | 0x30 | `LOAD64` | 3 |
| yaddaş | 0x31 | `STORE64` | 3 |
| yaddaş | 0x32 | `LOAD128` | 5 |
| yaddaş | 0x33 | `STORE128` | 5 |
| nəzarət | 0x40 | `BEQ` | 1 |
| nəzarət | 0x41 | `BNE` | 1 |
| nəzarət | 0x42 | `BLT` | 1 |
| nəzarət | 0x43 | `BGE` | 1 |
| nəzarət | 0x44 | `BLTU` | 1 |
| nəzarət | 0x45 | `BGEU` | 1 |
| nəzarət | 0x46 | `JAL` | 2 |
| nəzarət | 0x48 | `JALR` | 2 |
| nəzarət | 0x47 | `JR` | 2 |
| nəzarət | 0x4A | `JMP` | 2 |
| nəzarət | 0x4B | `JALS` | 2 |
| nəzarət | 0x49 | `HALT` | 0 |
| sistem | 0x60 | `SCALL` | 5 |
| sistem | 0x61 | `GETGAS` | 0 |
| kripto | 0x70 | `VADD32` | 2 |
| kripto | 0x71 | `VADD64` | 2 |
| kripto | 0x72 | `VAND` | 1 |
| kripto | 0x73 | `VXOR` | 1 |
| kripto | 0x74 | `VOR` | 1 |
| kripto | 0x75 | `VROT32` | 1 |
| kripto | 0x76 | `SETVL` | 1 |
| kripto | 0x77 | `PARBEGIN` | 0 |
| kripto | 0x78 | `PAREND` | 0 |
| kripto | 0x80 | `SHA256BLOCK` | 50 || kripto | 0x81 | `SHA3BLOCK` | 50 |
| kripto | 0x82 | `POSEIDON2` | 10 |
| kripto | 0x83 | `POSEIDON6` | 10 |
| kripto | 0x84 | `PUBKGEN` | 50 |
| kripto | 0x85 | `VALCOM` | 50 |
| kripto | 0x86 | `ECADD` | 20 |
| kripto | 0x87 | `ECMUL_VAR` | 100 |
| kripto | 0x8E | `PAIRING` | 500 |
| kripto | 0x88 | `AESENC` | 30 |
| kripto | 0x89 | `AESDEC` | 30 |
| kripto | 0x8A | `BLAKE2S` | 40 |
| kripto | 0x8B | `ED25519VERIFY` | 1000 |
| kripto | 0x8F | `ED25519BATCHVERIFY` | 500 |
| kripto | 0x8C | `ECDSAVERIFY` | 1500 |
| kripto | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |