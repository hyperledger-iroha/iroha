---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Libro mayor de calibracion de gas confidentiel
description: Médiciones de qualité de libération qui respectent le calendrier de gaz confidentiel.
limace : /nexus/confidential-gas-calibration
---

# Lignes de base de calibrage de gaz confidentielles

Este registro rastrea los resultados validados de los benchmarko de calibracion de gas confidentiel. Chaque fois, vous documentez un ensemble de médicaments de qualité de libération capturés selon la procédure décrite dans [Actifs confidentiels et transferts ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| Fécha (UTC) | S'engager | Profil | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | Éjection neutre x86_64 programmée sur l'hôte CI `bench-x86-neon0` ; voir le ticket GAS-214. Les résultats sont ajoutés lorsque la fenêtre du banc est terminée (la liste de contrôle de pré-fusion pointe vers la version 2.1). |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | Calibrage AVX2 postérieur en utilisant le même commit/build que la course neutre ; nécessite l'hôte `bench-x86-avx2a`. GAS-214 cubre ambas corridas con comparacion delta contra `baseline-neon`. |`ns/op` ajoute la médiane de temps de pared selon les instructions données par Criterion ; `gas/op` est l'arithmétique média des coûts de calendrier correspondants de `iroha_core::gas::meter_instruction` ; `ns/gas` divisez les nanosegundos sumados entre la somme de gaz dans le ensemble de nouvelles instructions.

*Remarque.* L'hôte arm64 actuel n'émet pas de reprise `raw.csv` du critère par défaut ; Essayez d'exécuter avec `CRITERION_OUTPUT_TO=csv` ou une correction en amont avant la publication de l'étiquette pour les artefacts requis pour la liste de contrôle d'acceptation queden adjuntos. Si `target/criterion/` échoue après `--save-baseline`, j'ai récupéré la corrida sur un hôte Linux ou sérialisé la sortie de console dans le bundle de la version comme palliatif temporel. Comme référence, le journal de la console arm64 de la dernière course vive en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Médianes pour les instructions de la mission (`cargo bench -p iroha_core --bench isi_gas_calibration`) :| Instruction | médiane `ns/op` | horaire `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 3.46e5 | 200 | 1.73e3 |
| S'inscrireCompte | 3.15e5 | 200 | 1.58e3 |
| S'inscrireAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRôle | 3.33e5 | 96 | 3.47e3 |
| Révoquer le rôle de compte | 3.12e5 | 96 | 3.25e3 |
| ExécuterTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |

La colonne de calendrier est impuesta par `gas::tests::calibration_bench_gas_snapshot` (total 1,413 gaz dans le cadre de nouvelles instructions) et tombera si les futurs changements de mesure sans actualiser les appareils d'étalonnage.