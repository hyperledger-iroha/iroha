---
lang: es
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Modelo a gasolina

Este documento define el programa de gas canónico para la máquina virtual Iroha.
(IVM) y explica cómo se aplica el hash y la programación. La fuente de la verdad
para costos es `crates/ivm/src/gas.rs`; la tabla de programación a continuación es una representación
vista de ese mapeo canónico.

## Alcance

- Se aplica a la ejecución del código de bytes IVM (Ejecutable::Ivm).
- La medición de gas ISI nativo se define por separado en `crates/iroha_core/src/gas.rs`.
- Los códigos de operación ISO 20022 están reservados en ABI v1 y aún no incluyen entradas de gas.

## Determinismo y hash de programación

El programa de gas es determinista y se deriva de pares de código de operación → costos. el
El resumen canónico se calcula sobre la lista ordenada de códigos de operación con cada entrada.
serializado como:

- byte de código de operación (u8)
- costo (u64, little-endian)

El hash está expuesto por:

- `ivm::gas::schedule_hash()` (hash de programación canónica)
- `ivm::limits::schedule_hash()` (alias orientado al host)

Utilice este resumen para verificar que todos los pares compartan el mismo horario al realizar el cableado
comprobaciones de configuración o telemetría.

## Escalado vectorial y reintentos HTM

- Las operaciones vectoriales (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) se escalan con la lógica
  longitud del vector establecida por `SETVL`. Los costos base en la tabla están escalados por
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (línea de base = 2 carriles).
- Los reintentos de HTM multiplican el costo por `(retries + 1)`; la mayoría de los caminos de consenso no lo hacen
  incurrir en reintentos.

## Tabla de gas de código de operación canónico

La siguiente tabla enumera los costos base utilizados por `ivm::gas::cost_of`. Escalado vectorial
y los reintentos de HTM se aplican además de estos valores base como se indicó anteriormente.| Categoría | Código de operación | Mnemotécnico | Gas base |
|---|---:|---|---:|
| aritmética | 0x01 | `ADD` | 1 |
| aritmética | 0x02 | `SUB` | 1 |
| aritmética | 0x03 | `AND` | 1 |
| aritmética | 0x04 | `OR` | 1 |
| aritmética | 0x05 | `XOR` | 1 |
| aritmética | 0x06 | `SLL` | 1 |
| aritmética | 0x07 | `SRL` | 1 |
| aritmética | 0x08 | `SRA` | 1 |
| aritmética | 0x0D | `NEG` | 1 |
| aritmética | 0x0C | `NOT` | 1 |
| aritmética | 0x20 | `ADDI` | 1 |
| aritmética | 0x21 | `ANDI` | 1 |
| aritmética | 0x22 | `ORI` | 1 |
| aritmética | 0x23 | `XORI` | 1 |
| aritmética | 0x10 | `MUL` | 3 |
| aritmética | 0x11 | `MULH` | 3 |
| aritmética | 0x12 | `MULHU` | 3 |
| aritmética | 0x13 | `MULHSU` | 3 |
| aritmética | 0x14 | `DIV` | 10 |
| aritmética | 0x15 | `DIVU` | 10 |
| aritmética | 0x16 | `REM` | 10 |
| aritmética | 0x17 | `REMU` | 10 |
| aritmética | 0x18 | `ROTL` | 2 |
| aritmética | 0x19 | `ROTR` | 2 |
| aritmética | 0x25 | `ROTL_IMM` | 2 |
| aritmética | 0x26 | `ROTR_IMM` | 2 |
| aritmética | 0x1A | `POPCNT` | 6 |
| aritmética | 0x1B | `CLZ` | 6 |
| aritmética | 0x1C | `CTZ` | 6 |
| aritmética | 0x1D | `ISQRT` | 6 |
| aritmética | 0x1E | `MIN` | 1 |
| aritmética | 0x1F | `MAX` | 1 |
| aritmética | 0x27 | `ABS` | 1 |
| aritmética | 0x28 | `DIV_CEIL` | 12 |
| aritmética | 0x29 | `GCD` | 12 |
| aritmética | 0x2A | `MEAN` | 2 |
| aritmética | 0x09 | `SLT` | 2 |
| aritmética | 0x0A | `SLTU` | 2 |
| aritmética | 0x0E | `SEQ` | 2 |
| aritmética | 0x0F | `SNE` | 2 |
| aritmética | 0x0B | `CMOV` | 3 |
| aritmética | 0x24 | `CMOVI` | 3 |
| memoria | 0x30 | `LOAD64` | 3 |
| memoria | 0x31 | `STORE64` | 3 |
| memoria | 0x32 | `LOAD128` | 5 |
| memoria | 0x33 | `STORE128` | 5 |
| controlar | 0x40 | `BEQ` | 1 |
| controlar | 0x41 | `BNE` | 1 |
| controlar | 0x42 | `BLT` | 1 |
| controlar | 0x43 | `BGE` | 1 |
| controlar | 0x44 | `BLTU` | 1 |
| controlar | 0x45 | `BGEU` | 1 |
| controlar | 0x46 | `JAL` | 2 |
| controlar | 0x48 | `JALR` | 2 |
| controlar | 0x47 | `JR` | 2 |
| controlar | 0x4A | `JMP` | 2 |
| controlar | 0x4B | `JALS` | 2 |
| controlar | 0x49 | `HALT` | 0 |
| sistema | 0x60 | `SCALL` | 5 |
| sistema | 0x61 | `GETGAS` | 0 |
| cripto | 0x70 | `VADD32` | 2 |
| cripto | 0x71 | `VADD64` | 2 |
| cripto | 0x72 | `VAND` | 1 |
| cripto | 0x73 | `VXOR` | 1 |
| cripto | 0x74 | `VOR` | 1 |
| cripto | 0x75 | `VROT32` | 1 |
| cripto | 0x76 | `SETVL` | 1 |
| cripto | 0x77 | `PARBEGIN` | 0 |
| cripto | 0x78 | `PAREND` | 0 |
| cripto | 0x80 | `SHA256BLOCK` | 50 || cripto | 0x81 | `SHA3BLOCK` | 50 |
| cripto | 0x82 | `POSEIDON2` | 10 |
| cripto | 0x83 | `POSEIDON6` | 10 |
| cripto | 0x84 | `PUBKGEN` | 50 |
| cripto | 0x85 | `VALCOM` | 50 |
| cripto | 0x86 | `ECADD` | 20 |
| cripto | 0x87 | `ECMUL_VAR` | 100 |
| cripto | 0x8E | `PAIRING` | 500 |
| cripto | 0x88 | `AESENC` | 30 |
| cripto | 0x89 | `AESDEC` | 30 |
| cripto | 0x8A | `BLAKE2S` | 40 |
| cripto | 0x8B | `ED25519VERIFY` | 1000 |
| cripto | 0x8F | `ED25519BATCHVERIFY` | 500 |
| cripto | 0x8C | `ECDSAVERIFY` | 1500 |
| cripto | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |