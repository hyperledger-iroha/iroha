---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Livro de calibracao de gas confidentiel
description : Médico de qualité de libération qui supporte le cronogramme de gaz confidentiel.
limace : /nexus/confidential-gas-calibration
---

# Baselines de calibrage de gaz confidentiel

Cet enregistrement accompagne les résultats validés des benchmarks de calibrage de gaz confidentiel. Chaque ligne documente un ensemble de médecins de qualité de libération capturée avec la procédure décrite dans [Actifs confidentiels et transferts ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| Données (UTC) | S'engager | Profil | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | Exécution neutre x86_64 programmée sans hôte CI `bench-x86-neon0` ; voir le ticket GAS-214. Les résultats seront des adicionados lorsque la jeune fille du banc terminera (une liste de contrôle avant la fusion avant la version 2.1). |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | Calibrage AVX2 en accompagnement en utilisant le même commit/build pour l'exécution neutre ; demander l'hôte `bench-x86-avx2a`. GAS-214 est utilisé comme comparaison delta avec `baseline-neon`. |`ns/op` associe l'horloge murale au milieu selon les instructions du critère ; `gas/op` et l'arithmétique médiatique des responsables du calendrier des correspondants de `iroha_core::gas::meter_instruction` ; `ns/gas` divise les nanosegundos somados pelo gas somado no conjunto de nove instrucoes.

*Remarque.* L'hôte arm64 nao émet automatiquement des CV `raw.csv` selon le critère de sélection ; Rode novamente com `CRITERION_OUTPUT_TO=csv` ou uma corrigé en amont avant de l'étiquette une publication pour que les artefatos exigidos pela checklist de aceitacao sejam anexados. Si `target/criterion/` est également disponible via `--save-baseline`, vous pouvez l'exécuter sur un hôte Linux ou sérialiser la console dans le bundle de version comme provisoire. Pour référence, le journal de la console arm64 de la dernière exécution est fica sur `docs/source/confidential_assets_calibration_neon_20251018.log`.

Médianes pour les instructions de la personne exécutée (`cargo bench -p iroha_core --bench isi_gas_calibration`) :

| Instruction | médiane `ns/op` | horaire `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 3.46e5 | 200 | 1.73e3 |
| S'inscrireCompte | 3.15e5 | 200 | 1.58e3 |
| S'inscrireAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRôle | 3.33e5 | 96 | 3.47e3 |
| Révoquer le rôle de compte | 3.12e5 | 96 | 3.25e3 |
| ExécuterTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |Un calendrier de colonne et imposta por `gas::tests::calibration_bench_gas_snapshot` (total de 1 413 gaz dans le cadre de nouvelles instructions) et vous pourrez mettre en place de futurs correctifs pour modifier le comptage en actualisant les luminaires de calibrage.