---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Registre de calibrage du gaz confidentiel
description : Mesures de qualite release qui etayent le calendrier de gas confidentiel.
limace : /nexus/confidential-gas-calibration
---

# Lignes de base de calibrage du gaz confidentiel

Ce registre suit les sorties validées des benchmarks de calibrage du gaz confidentiel. Chaque ligne documente un jeu de mesures de qualite release capture avec la procédure décrite dans [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Date (UTC) | S'engager | Profil | `ns/op` | `gas/op` | `ns/gas` | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | Exécution neutre x86_64 planifiée sur l'hôte CI `bench-x86-neon0`; voir billet GAS-214. Les résultats seront ajoutés une fois la fenêtre de bench terminée (la checklist pre-merge vise la release 2.1). |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | Calibration AVX2 de suivi utilisant le même commit/build que l'exécution neutre; nécessite l'hôte `bench-x86-avx2a`. GAS-214 couvre les deux runs avec comparaison delta contre `baseline-neon`. |`ns/op` agréger l'horloge murale médiane par instruction mesurée par critère ; `gas/op` est la moyenne arithmétique des couts de planning correspondants de `iroha_core::gas::meter_instruction`; `ns/gas` divise les nanosecondes somme par le gaz somme sur l'ensemble de neuf instructions.

*Note.* L'hôte arm64 actuel ne produit pas les CV `raw.csv` de Critère par défaut; relancez avec `CRITERION_OUTPUT_TO=csv` ou une correction en amont avant d'etiqueter un release afin que les artefacts requis par la checklist d'acceptation soient attachés. Si `target/criterion/` manque encore après `--save-baseline`, capturez l'exécution sur un hôte Linux ou sérialisez la sortie console dans le bundle de release comme palliatif temporaire. A titre de référence, le log console arm64 de la dernière exécution se trouve dans `docs/source/confidential_assets_calibration_neon_20251018.log`.

Médianes par instruction de la même exécution (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instructions | médiane `ns/op` | horaire `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 3.46e5 | 200 | 1.73e3 |
| S'inscrireCompte | 3.15e5 | 200 | 1.58e3 |
| S'inscrireAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRôle | 3.33e5 | 96 | 3.47e3 |
| Révoquer le rôle de compte | 3.12e5 | 96 | 3.25e3 |
| ExécuterTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |La colonne horaire est imposée par `gas::tests::calibration_bench_gas_snapshot` (total 1,413 gaz sur l'ensemble de neuf instructions) et échouera si des correctifs futurs modifier le comptage sans mettre à jour les appareils de calibrage.