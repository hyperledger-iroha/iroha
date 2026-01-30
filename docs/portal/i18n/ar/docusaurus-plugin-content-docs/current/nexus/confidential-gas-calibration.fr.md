---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Registre de calibration du gas confidentiel
description: Mesures de qualite release qui etayent le calendrier de gas confidentiel.
slug: /nexus/confidential-gas-calibration
---

# Lignes de base de calibration du gas confidentiel

Ce registre suit les sorties validees des benchmarks de calibration du gas confidentiel. Chaque ligne documente un jeu de mesures de qualite release capture avec la procedure decrite dans [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Date (UTC) | Commit | Profil | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | Execution neutre x86_64 planifiee sur l'hote CI `bench-x86-neon0`; voir ticket GAS-214. Les resultats seront ajoutes une fois la fenetre de bench terminee (la checklist pre-merge vise le release 2.1). |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | Calibration AVX2 de suivi utilisant le meme commit/build que l'execution neutre; requiert l'hote `bench-x86-avx2a`. GAS-214 couvre les deux runs avec comparaison delta contre `baseline-neon`. |

`ns/op` agrege la mediane wall-clock par instruction mesuree par Criterion; `gas/op` est la moyenne arithmetique des couts de schedule correspondants de `iroha_core::gas::meter_instruction`; `ns/gas` divise les nanosecondes sommees par le gas somme sur l'ensemble de neuf instructions.

*Note.* L'hote arm64 actuel ne produit pas les resumes `raw.csv` de Criterion par defaut; relancez avec `CRITERION_OUTPUT_TO=csv` ou une correction upstream avant d'etiqueter un release afin que les artefacts requis par la checklist d'acceptation soient attaches. Si `target/criterion/` manque encore apres `--save-baseline`, capturez l'execution sur un hote Linux ou serializez la sortie console dans le bundle de release comme stopgap temporaire. A titre de reference, le log console arm64 de la derniere execution se trouve dans `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianes par instruction de la meme execution (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruction | mediane `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

La colonne schedule est imposee par `gas::tests::calibration_bench_gas_snapshot` (total 1,413 gas sur l'ensemble de neuf instructions) et echouera si des correctifs futurs modifient le metering sans mettre a jour les fixtures de calibration.
