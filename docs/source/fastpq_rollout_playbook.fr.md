---
lang: fr
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Playbook de déploiement FASTPQ (étape 7-3)

Ce playbook met en œuvre les exigences de la feuille de route Stage7-3 : chaque mise à niveau de flotte
qui permet l'exécution du GPU FASTPQ doit joindre un manifeste de référence reproductible,
une preuve Grafana associée à un exercice de restauration documenté. Il complète
`docs/source/fastpq_plan.md` (cibles/architecture) et
`docs/source/fastpq_migration_guide.md` (étapes de mise à niveau au niveau du nœud) en se concentrant
sur la liste de contrôle de déploiement destinée à l'opérateur.

## Portée et rôles

- **Release Engineering / SRE :** propres captures de référence, signature de manifeste et
  exportations du tableau de bord avant l’approbation du déploiement.
- **Ops Guild :** exécute des déploiements par étapes, enregistre les répétitions de restauration et les magasins
  le lot d'artefacts sous `artifacts/fastpq_rollouts/<timestamp>/`.
- **Gouvernance / Conformité :** vérifie que des preuves accompagnent chaque changement
  demande avant que la valeur par défaut FASTPQ ne soit basculée pour une flotte.

## Exigences relatives au lot de preuves

Chaque soumission de déploiement doit contenir les artefacts suivants. Joindre tous les fichiers
au ticket de version/mise à niveau et conservez le bundle dans
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefact | Objectif | Comment produire |
|--------------|---------|----------------|
| `fastpq_bench_manifest.json` | Prouve que la charge de travail canonique de 20 000 lignes reste inférieure au plafond LDE `<1 s` et enregistre les hachages pour chaque référence encapsulée.| Capture Metal/CUDA s'exécute, encapsulez-les, puis exécutez :`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
| Benchmarks encapsulés (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Capturez les métadonnées de l'hôte, les preuves d'utilisation des lignes, les points chauds sans remplissage, les résumés du microbench Poséidon et les statistiques du noyau utilisées par les tableaux de bord/alertes.| Exécutez `fastpq_metal_bench`/`fastpq_cuda_bench`, puis encapsulez le JSON brut :`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`Répétez pour les captures CUDA (point `--row-usage` et `--poseidon-metrics` dans les dossiers témoins/grattages pertinents). L'assistant intègre les échantillons filtrés `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` afin que les preuves WP2-E.6 soient identiques dans Metal et CUDA. Utilisez `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` lorsque vous avez besoin d'un résumé autonome du microbench Poséidon (entrées encapsulées ou brutes prises en charge). |
|  |  | **Exigence d'étiquette Stage7 :** `wrap_benchmark.py` échoue désormais, sauf si la section `metadata.labels` résultante contient à la fois `device_class` et `gpu_kind`. Lorsque la détection automatique ne peut pas les déduire (par exemple, lors du wrapper sur un nœud CI détaché), transmettez des remplacements explicites tels que `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **Télémétrie d'accélération :** le wrapper capture également `cargo xtask acceleration-state --format json` par défaut, en écrivant `<bundle>.accel.json` et `<bundle>.accel.prom` à côté du test de référence encapsulé (remplacement avec les indicateurs `--accel-*` ou `--skip-acceleration-state`). La matrice de capture utilise ces fichiers pour créer `acceleration_matrix.{json,md}` pour les tableaux de bord de flotte. |
| Grafana exportation | Prouve la télémétrie d'adoption et les annotations d'alerte pour la fenêtre de déploiement.| Exportez le tableau de bord `fastpq-acceleration` :`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Annotez le tableau avec les heures de démarrage/arrêt du déploiement avant l'exportation. Le pipeline de versions peut le faire automatiquement via `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (jeton fourni via `GRAFANA_TOKEN`). |
| Instantané d'alerte | Capture les règles d'alerte qui ont surveillé le déploiement.| Copiez `dashboards/alerts/fastpq_acceleration_rules.yml` (et le luminaire `tests/`) dans le bundle afin que les réviseurs puissent réexécuter `promtool test rules …`. |
| Journal de forage de restauration | Démontre que les opérateurs ont répété le repli forcé du processeur et les accusés de réception de télémétrie.| Utilisez la procédure dans [Rollback Drills] (#rollback-drills) et stockez les journaux de la console (`rollback_drill.log`) ainsi que le scraping Prometheus résultant (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | Enregistre l'allocation de lignes ExecWitness FASTPQ que TF-5 suit dans CI et les tableaux de bord.| Téléchargez un nouveau témoin à partir de Torii, décodez-le via `iroha_cli audit witness --decode exec.witness` (ajoutez éventuellement `--fastpq-parameter fastpq-lane-balanced` pour affirmer le jeu de paramètres attendu ; les lots FASTPQ émettent par défaut) et copiez le JSON `row_usage` dans `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. Gardez les noms de fichiers horodatés afin que les réviseurs puissent les corréler avec le ticket de déploiement et exécutez `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (ou `make check-fastpq-rollout`) afin que la porte Stage7-3 vérifie que chaque lot annonce le nombre de sélecteurs et l'invariant `transfer_ratio = transfer_rows / total_rows` avant de joindre les preuves. |

> **Conseil :** `artifacts/fastpq_rollouts/README.md` documente la dénomination préférée
> schéma (`<stamp>/<fleet>/<lane>`) et les dossiers de preuves requis. Le
> Le dossier `<stamp>` doit coder `YYYYMMDDThhmmZ` pour que les artefacts restent triables
> sans consulter les tickets.

## Liste de contrôle pour la génération de preuves1. **Capturez des benchmarks GPU.**
   - Exécuter la charge de travail canonique (20 000 lignes logiques, 32 768 lignes complétées) via
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - Enveloppez le résultat avec `scripts/fastpq/wrap_benchmark.py` en utilisant `--row-usage <decoded witness>` afin que le bundle transporte les preuves du gadget aux côtés de la télémétrie GPU. Transmettez `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` pour que le wrapper échoue rapidement si l'un des accélérateurs dépasse la cible ou si la télémétrie de file d'attente/profil Poséidon est manquante, et pour générer la signature détachée.
   - Répétez sur l'hôte CUDA pour que le manifeste contienne les deux familles de GPU.
   - Ne **pas** dépouiller le `benchmarks.metal_dispatch_queue` ou
     `benchmarks.zero_fill_hotspots` bloque le JSON encapsulé. La porte CI
     (`ci/check_fastpq_rollout.sh`) lit désormais ces champs et échoue lorsque la file d'attente
     la marge chute en dessous d'un emplacement ou lorsqu'un point d'accès LDE signale `mean_ms >
     0,40 ms, appliquant automatiquement la protection de télémétrie Stage7.
2. **Générez le manifeste.** Utilisez `cargo xtask fastpq-bench-manifest …` comme
   indiqué dans le tableau. Stockez `fastpq_bench_manifest.json` dans le bundle de déploiement.
3. **Exporter Grafana.**
   - Annoter la carte `FASTPQ Acceleration Overview` avec la fenêtre de déploiement,
     liaison vers les ID de panneau Grafana pertinents.
   - Exporter le tableau de bord JSON via l'API Grafana (commande ci-dessus) et inclure
     la section `annotations` afin que les évaluateurs puissent faire correspondre les courbes d'adoption aux
     déploiement par étapes.
4. **Alertes instantanées.** Copiez les règles d'alerte exactes (`dashboards/alerts/…`) utilisées
   par le déploiement dans le bundle. Si les règles Prometheus ont été remplacées, incluez
   le diff de remplacement.
5. **Prometheus/OTEL scrape.** Capturez `fastpq_execution_mode_total{device_class="<matrix>"}` de chaque
   hôte (avant et après l'étape) plus le comptoir OTEL
   `fastpq.execution_mode_resolutions_total` et le jumelé
   Entrées de journal `telemetry::fastpq.execution_mode`. Ces artefacts prouvent que
   L'adoption du GPU est stable et les replis forcés du processeur émettent toujours de la télémétrie.
6. **Archiver la télémétrie d'utilisation des lignes.** Après avoir décodé l'exécution d'ExecWitness pour le
   déploiement, déposez le JSON résultant sous `row_usage/` dans le bundle. L'IC
   l'assistant (`ci/check_fastpq_row_usage.sh`) compare ces instantanés avec les
   lignes de base canoniques, et `ci/check_fastpq_rollout.sh` nécessite désormais chaque
   bundle pour expédier au moins un fichier `row_usage` pour conserver les preuves TF-5 jointes
   au ticket de sortie.

## Flux de déploiement par étapes

Utilisez trois phases déterministes pour chaque flotte. Avancez seulement après la sortie
les critères de chaque phase sont satisfaits et documentés dans l’ensemble des preuves.| Phases | Portée | Critères de sortie | Pièces jointes |
|-------|-------|---------------|-------------|
| Pilote (P1) | 1 plan de contrôle + 1 nœud de plan de données par région | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90 % pendant 48 h, aucun incident Alertmanager et un exercice de restauration réussi. | Regroupez les deux hôtes (JSON de banc, exportation Grafana avec annotation pilote, journaux de restauration). |
| Rampe (P2) | ≥50 % des validateurs plus au moins une voie d'archivage par cluster | L'exécution du GPU a duré 5 jours, pas plus d'un pic de rétrogradation > 10 minutes et les compteurs Prometheus prouvent une alerte de repli dans les 60 secondes. | Exportation Grafana mise à jour montrant l'annotation de la rampe, les différences de grattage Prometheus, capture d'écran/journal d'Alertmanager. |
| Par défaut (P3) | Nœuds restants ; FASTPQ marqué par défaut dans `iroha_config` | Manifeste de banc signé + exportation Grafana faisant référence à la courbe d'adoption finale et exercice de restauration documenté démontrant la bascule de configuration. | Manifeste final, Grafana JSON, journal de restauration, référence de ticket à l'examen des modifications de configuration. |

Documentez chaque étape de la promotion dans le ticket de déploiement et créez un lien direct vers le
Annotations `grafana_fastpq_acceleration.json` afin que les réviseurs puissent corréler les
chronologie avec les preuves.

## Exercices de restauration

Chaque étape de déploiement doit inclure une répétition de restauration :

1. Choisissez un nœud par cluster et enregistrez les métriques actuelles :
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Forcez le mode CPU pendant 10 minutes à l'aide du bouton de configuration
   (`zk.fastpq.execution_mode = "cpu"`) ou le remplacement de l'environnement :
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Confirmez le journal de rétrogradation
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) et grattez
   le point de terminaison Prometheus à nouveau pour afficher les incréments du compteur.
4. Restaurez le mode GPU, vérifiez que `telemetry::fastpq.execution_mode` signale désormais
   `resolved="metal"` (ou `resolved="cuda"/"opencl"` pour les voies non métalliques),
   confirmez que le scrap Prometheus contient à la fois les échantillons de CPU et de GPU dans
   `fastpq_execution_mode_total{backend=…}` et enregistrez le temps écoulé dans
   détection/nettoyage.
5. Stockez les transcriptions du shell, les métriques et les accusés de réception des opérateurs comme
   `rollback_drill.log` et `metrics_rollback.prom` dans le bundle de déploiement. Ces
   les fichiers doivent illustrer le cycle complet de rétrogradation + restauration car
   `ci/check_fastpq_rollout.sh` échoue désormais chaque fois que le journal ne dispose pas du GPU
   La ligne de récupération ou l'instantané des métriques omet les compteurs CPU ou GPU.

Ces journaux prouvent que chaque cluster peut se dégrader progressivement et que les équipes SRE
savoir comment reculer de manière déterministe si les pilotes ou les noyaux GPU régressent.

## Preuve de repli en mode mixte (WP2-E.6)

Chaque fois qu'un hôte a besoin d'un GPU FFT/LDE mais d'un hachage CPU Poséidon (selon Stage7 <900 ms
exigence), regroupez les artefacts suivants avec les journaux de restauration standard :1. **Config diff.** Enregistrez (ou attachez) le remplacement hôte-local qui définit
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) en partant
   `zk.fastpq.execution_mode` intact. Nommer le patch
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Comptoir Poséidon.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   La capture doit afficher `path="cpu_forced"` incrémenté de manière synchronisée avec le
   Compteur GPU FFT/LDE pour cette classe d'appareil. Faites un deuxième grattage après le retour
   revenir au mode GPU afin que les réviseurs puissent voir le résumé de la ligne `path="gpu"`.

   Transmettez le fichier résultant à `wrap_benchmark.py --poseidon-metrics …` afin que le test encapsulé enregistre les mêmes compteurs dans sa section `poseidon_metrics` ; cela maintient les déploiements de Metal et CUDA sur le même flux de travail et rend les preuves de secours vérifiables sans ouvrir de fichiers de récupération séparés.
3. **Extrait du journal.** Copiez les entrées `telemetry::fastpq.poseidon` qui prouvent le
   résolveur basculé vers CPU (`cpu_forced`) dans
   `poseidon_fallback.log`, conservation des horodatages afin que les délais d'Alertmanager puissent être
   corrélé au changement de configuration.

CI applique aujourd'hui les contrôles de file d'attente/remplissage à zéro ; une fois la porte en mode mixte atterrie,
`ci/check_fastpq_rollout.sh` insistera également pour que tout bundle contenant
`poseidon_fallback.patch` fournit l'instantané `metrics_poseidon.prom` correspondant.
Le suivi de ce flux de travail permet de maintenir la politique de secours WP2-E.6 vérifiable et liée à
les mêmes collecteurs de preuves utilisés lors du déploiement par défaut.

## Rapports et automatisation

- Attachez l'intégralité du répertoire `artifacts/fastpq_rollouts/<stamp>/` au
  Libérez le ticket et référencez-le à partir de `status.md` une fois le déploiement terminé.
- Exécutez `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (via
  `promtool`) au sein de CI pour garantir que les ensembles d'alertes fournis avec le déploiement soient toujours
  compiler.
- Valider le bundle avec `ci/check_fastpq_rollout.sh` (ou
  `make check-fastpq-rollout`) et réussissez `FASTPQ_ROLLOUT_BUNDLE=<path>` lorsque vous
  souhaitez cibler un seul déploiement. CI appelle le même script via
  `.github/workflows/fastpq-rollout.yml`, donc les artefacts manquants échouent rapidement avant qu'un
  le ticket de sortie peut se fermer. Le pipeline de versions peut archiver les bundles validés
  aux côtés des manifestes signés en passant
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` à
  `scripts/run_release_pipeline.py` ; l'assistant réexécute
  `ci/check_fastpq_rollout.sh` (sauf si `--skip-fastpq-rollout-check` est défini) et
  copie l'arborescence des répertoires dans `artifacts/releases/<version>/fastpq_rollouts/…`.
  Dans le cadre de cette porte, le script applique la profondeur de file d'attente Stage7 et le remplissage nul.
  budgets en lisant `benchmarks.metal_dispatch_queue` et
  `benchmarks.zero_fill_hotspots` sur chaque banc JSON `metal`.

En suivant ce manuel, nous pouvons démontrer l'adoption déterministe, fournir un
un seul ensemble de preuves par déploiement et des exercices de restauration audités parallèlement
les manifestes de référence signés.