---
lang: pt
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Modelo de gás IVM

Este documento define o cronograma de gás canônico para a máquina virtual Iroha
(IVM) e explica como o agendamento é criptografado e aplicado. A fonte da verdade
para custos é `crates/ivm/src/gas.rs`; a tabela de cronograma abaixo é uma renderização
visão desse mapeamento canônico.

## Escopo

- Aplica-se à execução de bytecode IVM (Executable::Ivm).
- A medição de gás ISI nativa é definida separadamente em `crates/iroha_core/src/gas.rs`.
- Os opcodes ISO 20022 são reservados na ABI v1 e ainda não carregam entradas de gás.

## Determinismo e hash de cronograma

A programação do gás é determinística e derivada de pares opcode → custo. O
o resumo canônico é calculado na lista ordenada de opcode com cada entrada
serializado como:

- byte de código de operação (u8)
- custo (u64, little endian)

O hash é exposto por:

- `ivm::gas::schedule_hash()` (hash de programação canônica)
- `ivm::limits::schedule_hash()` (alias voltado para host)

Use este resumo para verificar se todos os peers compartilham a mesma programação durante a conexão
verificações de configuração ou telemetria.

## Dimensionamento vetorial e novas tentativas HTM

- Escala de operações vetoriais (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) com a lógica
  comprimento do vetor definido por `SETVL`. Os custos básicos na tabela são escalonados por
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (linha de base = 2 pistas).
- Novas tentativas HTM multiplicam o custo por `(retries + 1)`; a maioria dos caminhos de consenso não
  incorrer em novas tentativas.

## Tabela de gás de código de operação canônico

A tabela abaixo lista os custos básicos usados pelo `ivm::gas::cost_of`. Escala vetorial
e as novas tentativas HTM são aplicadas sobre esses valores base, conforme observado acima.| Categoria | Código de operação | Mnemônico | Gás de base |
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
| memória | 0x30 | `LOAD64` | 3 |
| memória | 0x31 | `STORE64` | 3 |
| memória | 0x32 | `LOAD128` | 5 |
| memória | 0x33 | `STORE128` | 5 |
| controle | 0x40 | `BEQ` | 1 |
| controle | 0x41 | `BNE` | 1 |
| controle | 0x42 | `BLT` | 1 |
| controle | 0x43 | `BGE` | 1 |
| controle | 0x44 | `BLTU` | 1 |
| controle | 0x45 | `BGEU` | 1 |
| controle | 0x46 | `JAL` | 2 |
| controle | 0x48 | `JALR` | 2 |
| controle | 0x47 | `JR` | 2 |
| controle | 0x4A | `JMP` | 2 |
| controle | 0x4B | `JALS` | 2 |
| controle | 0x49 | `HALT` | 0 |
| sistema | 0x60 | `SCALL` | 5 |
| sistema | 0x61 | `GETGAS` | 0 |
| criptografia | 0x70 | `VADD32` | 2 |
| criptografia | 0x71 | `VADD64` | 2 |
| criptografia | 0x72 | `VAND` | 1 |
| criptografia | 0x73 | `VXOR` | 1 |
| criptografia | 0x74 | `VOR` | 1 |
| criptografia | 0x75 | `VROT32` | 1 |
| criptografia | 0x76 | `SETVL` | 1 |
| criptografia | 0x77 | `PARBEGIN` | 0 |
| criptografia | 0x78 | `PAREND` | 0 |
| criptografia | 0x80 | `SHA256BLOCK` | 50 || criptografia | 0x81 | `SHA3BLOCK` | 50 |
| criptografia | 0x82 | `POSEIDON2` | 10 |
| criptografia | 0x83 | `POSEIDON6` | 10 |
| criptografia | 0x84 | `PUBKGEN` | 50 |
| criptografia | 0x85 | `VALCOM` | 50 |
| criptografia | 0x86 | `ECADD` | 20 |
| criptografia | 0x87 | `ECMUL_VAR` | 100 |
| criptografia | 0x8E | `PAIRING` | 500 |
| criptografia | 0x88 | `AESENC` | 30 |
| criptografia | 0x89 | `AESDEC` | 30 |
| criptografia | 0x8A | `BLAKE2S` | 40 |
| criptografia | 0x8B | `ED25519VERIFY` | 1000 |
| criptografia | 0x8F | `ED25519BATCHVERIFY` | 500 |
| criptografia | 0x8C | `ECDSAVERIFY` | 1500 |
| criptografia | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |