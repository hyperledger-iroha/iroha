---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Реестр калибровки конфиденциального газа
description: Измерения уровня реLISа, подтверждающие график конфиденциального газа.
limace : /nexus/confidential-gas-calibration
---

# Les lignes de base calibrent le gaz confidentiel

Cela permet de vérifier les résultats des tests de calibrage du gaz confidentiel. Il s'agit d'un document relatif à la vérification de votre décision relative à la procédure relative aux [Actifs confidentiels et transferts ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| Données (UTC) | S'engager | Profil | `ns/op` | `gas/op` | `ns/gas` | Première |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | Planification indépendante du projet x86_64 sur le serveur CI `bench-x86-neon0` ; см. billet GAS-214. Les résultats seront ajoutés après la mise à jour du banc (le contrôle de pré-fusion s'oriente vers la version 2.1). |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | Après le calibrage d'AVX2, il s'agit d'un commit/build, qui est un projet indépendant ; требуется хост `bench-x86-avx2a`. GAS-214 s'applique au programme avec le service de dépannage automatique `baseline-neon`. |`ns/op` associe l'horloge murale médiane à l'instruction pour déterminer Criterion ; `gas/op` - c'est une solution arithmétique correspondant à la situation de `iroha_core::gas::meter_instruction` ; `ns/gas` ne permet pas de résumer les informations relatives au gaz synthétique pour le personnel conformément aux instructions détaillées.

*Примечание.* Текущий arm64 хост по умолчанию не выводит сводки Critère `raw.csv` ; s'il vous plaît utiliser `CRITERION_OUTPUT_TO=csv` ou s'assurer de la mise en œuvre en amont avant la mise en œuvre des travaux d'art, des travaux de verrouillage des pièces, etc. приложены. Si vous utilisez le `target/criterion/` après le `--save-baseline`, installez-vous sur un hôte Linux ou installez votre console en version série. бандл как временный palliatif. Pour les opérations, le journal de la console arm64 se trouve ensuite dans `docs/source/confidential_assets_calibration_neon_20251018.log`.

Médias d'instructions pour ce projet (`cargo bench -p iroha_core --bench isi_gas_calibration`) :

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
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |Le calendrier de colonisation correspond à `gas::tests::calibration_bench_gas_snapshot` (pour 1 413 gaz par jour selon les instructions de développement) et vous avez un budget limité pour les travaux de planification поменяют метеринг без обновления калибровочных фикстур.