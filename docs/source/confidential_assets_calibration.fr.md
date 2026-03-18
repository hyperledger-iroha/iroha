---
lang: fr
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Lignes de base confidentielles pour l'étalonnage des gaz

Ce grand livre suit les résultats validés de l'étalonnage confidentiel des gaz.
des repères. Chaque ligne documente un ensemble de mesures de la qualité des versions capturées avec
la procédure décrite dans `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| Date (UTC) | S'engager | Profil | `ns/op` | `gas/op` | `ns/gas` | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | baseline-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`). Commande : `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428` ; connectez-vous à `docs/source/confidential_assets_calibration_neon_20260428.log`. Des exécutions de parité x86_64 (SIMD-neutre + AVX2) sont prévues pour le créneau du laboratoire de Zurich du 19/03/2026 ; les artefacts atterriront sous `artifacts/confidential_assets_calibration/2026-03-x86/` avec les commandes correspondantes et seront fusionnés dans la table de référence une fois capturés. |
| 2026-04-28 | — | baseline-simd-neutre | — | — | — | **Renoncé** sur Apple Silicon : `ring` applique NEON pour l'ABI de la plate-forme, de sorte que `RUSTFLAGS="-C target-feature=-neon"` échoue avant que le banc puisse s'exécuter (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Les données neutres restent bloquées sur l'hôte CI `bench-x86-neon0`. |
| 2026-04-28 | — | ligne de base-avx2 | — | — | — | **Différé** jusqu'à ce qu'un runner x86_64 soit disponible. `arch -x86_64` ne peut pas générer de fichiers binaires sur cette machine (« Mauvais type de processeur dans l'exécutable » ; voir `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). L'hôte CI `bench-x86-avx2a` reste la source d'enregistrement. |

`ns/op` regroupe l'horloge murale médiane par instruction mesurée par Criterion ;
`gas/op` est la moyenne arithmétique des coûts d'échéancier correspondants de
`iroha_core::gas::meter_instruction` ; `ns/gas` divise les nanosecondes totalisées par
le gaz totalisé dans l’ensemble d’échantillons de neuf instructions.

*Remarque.* L'hôte arm64 actuel n'émet pas de résumés du critère `raw.csv` hors de
la boîte ; réexécutez avec `CRITERION_OUTPUT_TO=csv` ou un correctif en amont avant de marquer un
libération afin que les artefacts requis par la liste de contrôle d'acceptation soient joints.
Si `target/criterion/` est toujours manquant après `--save-baseline`, récupérez l'analyse
sur un hôte Linux ou sérialisez la sortie de la console dans le bundle de versions en tant que
un palliatif temporaire. Pour référence, le journal de la console arm64 de la dernière exécution
vit à `docs/source/confidential_assets_calibration_neon_20251018.log`.

Médianes par instruction de la même exécution (`cargo bench -p iroha_core --bench isi_gas_calibration`) :

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
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon, compatible NEON)

Latences médianes pour l'actualisation du 28/04/2026 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`) :| Instructions | médiane `ns/op` | horaire `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 8.58e6 | 200 | 4.29e4 |
| S'inscrireCompte | 4.40e6 | 200 | 2.20e4 |
| S'inscrireAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRôle | 3.60e6 | 96 | 3.75e4 |
| Révoquer le rôle de compte | 3.76e6 | 96 | 3.92e4 |
| ExécuterTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| Transfert d'actifs | 3.59e6 | 180 | 1,99e4 |

Les agrégats `ns/op` et `ns/gas` dans le tableau ci-dessus sont dérivés de la somme de
ces médianes (total `3.85717e7`ns sur le jeu de neuf instructions et 1 413
unités à gaz).

La colonne planning est appliquée par `gas::tests::calibration_bench_gas_snapshot`
(total 1 413 gaz sur le jeu de neuf instructions) et se déclenchera si les futurs correctifs
modifier le dosage sans mettre à jour les appareils d'étalonnage.

## Preuves de télémétrie de l'arbre d'engagement (M2.2)

Selon la tâche de la feuille de route **M2.2**, chaque exécution d'étalonnage doit capturer les nouveaux
des jauges d'arbre d'engagement et des compteurs d'expulsions pour prouver que la frontière de Merkle reste
dans les limites configurées :

-`iroha_confidential_tree_commitments{asset_id}`
-`iroha_confidential_tree_depth{asset_id}`
-`iroha_confidential_root_history_entries{asset_id}`
-`iroha_confidential_frontier_checkpoints{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_height{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
-`iroha_confidential_root_evictions_total{asset_id}`
-`iroha_confidential_frontier_evictions_total{asset_id}`
-`iroha_zk_verifier_cache_events_total{cache,event}`

Enregistrez les valeurs immédiatement avant et après la charge de travail d’étalonnage. Un
une seule commande par actif suffit ; exemple pour `xor#wonderland` :

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Joignez la sortie brute (ou l'instantané Prometheus) au ticket d'étalonnage afin que le
l'examinateur de la gouvernance peut confirmer que les plafonds de l'historique racine et les intervalles de points de contrôle sont
honoré. Le guide de télémétrie dans `docs/source/telemetry.md#confidential-tree-telemetry-m22`
développe les attentes d'alerte et les panneaux Grafana associés.

Incluez les compteurs de cache du vérificateur dans le même scratch afin que les réviseurs puissent confirmer
le taux d'échec est resté inférieur au seuil d'alerte de 40 % :

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Documentez le rapport dérivé (`miss / (hit + miss)`) dans la note d'étalonnage
pour montrer que les exercices de modélisation des coûts neutres SIMD ont réutilisé les caches chauds au lieu de
détruire le registre du vérificateur Halo2.

## Neutre et renonciation AVX2

Le Conseil du SDK a accordé une dérogation temporaire pour la porte PhaseC exigeant
Mesures `baseline-simd-neutral` et `baseline-avx2` :

- **SIMD neutre :** Sur Apple Silicon, le backend cryptographique `ring` applique NEON pour
  Exactitude de l'ABI. Désactivation de la fonctionnalité (`RUSTFLAGS="-C target-feature=-neon"`)
  abandonne la construction avant que le binaire de banc ne soit produit (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2 :** La chaîne d'outils locale ne peut pas générer de binaires x86_64 (`arch -x86_64 rustc -V`
  → « Mauvais type de CPU dans l'exécutable » ; voir
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

Jusqu'à ce que les hôtes CI `bench-x86-neon0` et `bench-x86-avx2a` soient en ligne, NEON s'exécute
ci-dessus ainsi que les preuves télémétriques satisfont aux critères d’acceptation PhaseC.
La renonciation est enregistrée dans `status.md` et sera réexaminée une fois le matériel x86 installé.
disponible.