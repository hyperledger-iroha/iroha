---
lang: fr
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Modèle à gaz

Ce document définit le programme de gaz canonique pour la machine virtuelle Iroha.
(IVM) et explique comment la planification est hachée et appliquée. La source de la vérité
pour les frais est `crates/ivm/src/gas.rs` ; le tableau de planification ci-dessous est un rendu
vue de cette cartographie canonique.

## Portée

- S'applique à l'exécution du bytecode IVM (Executable::Ivm).
- Le comptage de gaz ISI natif est défini séparément dans `crates/iroha_core/src/gas.rs`.
- Les opcodes ISO 20022 sont réservés dans ABI v1 et ne comportent pas encore d'entrées de gaz.

## Déterminisme et hachage de planning

Le programme de gaz est déterministe et dérivé des paires opcode → coût. Le
le résumé canonique est calculé sur la liste d'opcodes ordonnée avec chaque entrée
sérialisé comme :

- octet d'opcode (u8)
- coût (u64, petit-boutiste)

Le hachage est exposé par :

- `ivm::gas::schedule_hash()` (hachage de planification canonique)
- `ivm::limits::schedule_hash()` (alias côté hôte)

Utilisez ce résumé pour vérifier que tous les pairs partagent le même calendrier lors du câblage
vérifications de configuration ou de télémétrie.

## Mise à l'échelle des vecteurs et tentatives HTM

- Échelle des opérations vectorielles (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) avec la logique
  longueur du vecteur définie par `SETVL`. Les coûts de base du tableau sont échelonnés selon
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (référence = 2 voies).
- Les tentatives HTM multiplient le coût par `(retries + 1)` ; la plupart des voies de consensus ne le font pas
  encourir des tentatives.

## Table de gaz d'opcode canonique

Le tableau ci-dessous répertorie les coûts de base utilisés par `ivm::gas::cost_of`. Mise à l'échelle vectorielle
et les tentatives HTM sont appliquées en plus de ces valeurs de base, comme indiqué ci-dessus.| Catégorie | Opcode | Mnémonique | Gaz de base |
|---|---:|---|---:|
| arithmétique | 0x01 | `ADD` | 1 |
| arithmétique | 0x02 | `SUB` | 1 |
| arithmétique | 0x03 | `AND` | 1 |
| arithmétique | 0x04 | `OR` | 1 |
| arithmétique | 0x05 | `XOR` | 1 |
| arithmétique | 0x06 | `SLL` | 1 |
| arithmétique | 0x07 | `SRL` | 1 |
| arithmétique | 0x08 | `SRA` | 1 |
| arithmétique | 0x0D | `NEG` | 1 |
| arithmétique | 0x0C | `NOT` | 1 |
| arithmétique | 0x20 | `ADDI` | 1 |
| arithmétique | 0x21 | `ANDI` | 1 |
| arithmétique | 0x22 | `ORI` | 1 |
| arithmétique | 0x23 | `XORI` | 1 |
| arithmétique | 0x10 | `MUL` | 3 |
| arithmétique | 0x11 | `MULH` | 3 |
| arithmétique | 0x12 | `MULHU` | 3 |
| arithmétique | 0x13 | `MULHSU` | 3 |
| arithmétique | 0x14 | `DIV` | 10 |
| arithmétique | 0x15 | `DIVU` | 10 |
| arithmétique | 0x16 | `REM` | 10 |
| arithmétique | 0x17 | `REMU` | 10 |
| arithmétique | 0x18 | `ROTL` | 2 |
| arithmétique | 0x19 | `ROTR` | 2 |
| arithmétique | 0x25 | `ROTL_IMM` | 2 |
| arithmétique | 0x26 | `ROTR_IMM` | 2 |
| arithmétique | 0x1A | `POPCNT` | 6 |
| arithmétique | 0x1B | `CLZ` | 6 |
| arithmétique | 0x1C | `CTZ` | 6 |
| arithmétique | 0x1D | `ISQRT` | 6 |
| arithmétique | 0x1E | `MIN` | 1 |
| arithmétique | 0x1F | `MAX` | 1 |
| arithmétique | 0x27 | `ABS` | 1 |
| arithmétique | 0x28 | `DIV_CEIL` | 12 |
| arithmétique | 0x29 | `GCD` | 12 |
| arithmétique | 0x2A | `MEAN` | 2 |
| arithmétique | 0x09 | `SLT` | 2 |
| arithmétique | 0x0A | `SLTU` | 2 |
| arithmétique | 0x0E | `SEQ` | 2 |
| arithmétique | 0x0F | `SNE` | 2 |
| arithmétique | 0x0B | `CMOV` | 3 |
| arithmétique | 0x24 | `CMOVI` | 3 |
| mémoire | 0x30 | `LOAD64` | 3 |
| mémoire | 0x31 | `STORE64` | 3 |
| mémoire | 0x32 | `LOAD128` | 5 |
| mémoire | 0x33 | `STORE128` | 5 |
| contrôle | 0x40 | `BEQ` | 1 |
| contrôle | 0x41 | `BNE` | 1 |
| contrôle | 0x42 | `BLT` | 1 |
| contrôle | 0x43 | `BGE` | 1 |
| contrôle | 0x44 | `BLTU` | 1 |
| contrôle | 0x45 | `BGEU` | 1 |
| contrôle | 0x46 | `JAL` | 2 |
| contrôle | 0x48 | `JALR` | 2 |
| contrôle | 0x47 | `JR` | 2 |
| contrôle | 0x4A | `JMP` | 2 |
| contrôle | 0x4B | `JALS` | 2 |
| contrôle | 0x49 | `HALT` | 0 |
| système | 0x60 | `SCALL` | 5 |
| système | 0x61 | `GETGAS` | 0 |
| crypto-monnaie | 0x70 | `VADD32` | 2 |
| crypto-monnaie | 0x71 | `VADD64` | 2 |
| crypto-monnaie | 0x72 | `VAND` | 1 |
| crypto-monnaie | 0x73 | `VXOR` | 1 |
| crypto-monnaie | 0x74 | `VOR` | 1 |
| crypto-monnaie | 0x75 | `VROT32` | 1 |
| crypto-monnaie | 0x76 | `SETVL` | 1 |
| crypto-monnaie | 0x77 | `PARBEGIN` | 0 |
| crypto-monnaie | 0x78 | `PAREND` | 0 |
| crypto-monnaie | 0x80 | `SHA256BLOCK` | 50 || crypto-monnaie | 0x81 | `SHA3BLOCK` | 50 |
| crypto-monnaie | 0x82 | `POSEIDON2` | 10 |
| crypto-monnaie | 0x83 | `POSEIDON6` | 10 |
| crypto-monnaie | 0x84 | `PUBKGEN` | 50 |
| crypto-monnaie | 0x85 | `VALCOM` | 50 |
| crypto-monnaie | 0x86 | `ECADD` | 20 |
| crypto-monnaie | 0x87 | `ECMUL_VAR` | 100 |
| crypto-monnaie | 0x8E | `PAIRING` | 500 |
| crypto-monnaie | 0x88 | `AESENC` | 30 |
| crypto-monnaie | 0x89 | `AESDEC` | 30 |
| crypto-monnaie | 0x8A | `BLAKE2S` | 40 |
| crypto-monnaie | 0x8B | `ED25519VERIFY` | 1000 |
| crypto-monnaie | 0x8F | `ED25519BATCHVERIFY` | 500 |
| crypto-monnaie | 0x8C | `ECDSAVERIFY` | 1500 |
| crypto-monnaie | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |