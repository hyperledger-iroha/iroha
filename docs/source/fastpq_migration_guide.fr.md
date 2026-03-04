---
lang: fr
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Guide de migration de production FASTPQ

Ce runbook décrit comment valider le prouveur FASTPQ de production Stage6.
Le backend d'espace réservé déterministe a été supprimé dans le cadre de ce plan de migration.
Il complète le plan par étapes dans `docs/source/fastpq_plan.md` et suppose que vous suivez déjà
statut de l'espace de travail dans `status.md`.

## Audience et portée
- Opérateurs de validation déployant le prouveur de production dans des environnements de test ou de réseau principal.
- Ingénieurs de publication créant des binaires ou des conteneurs qui seront livrés avec le backend de production.
- Les équipes SRE/observabilité câblant les nouveaux signaux de télémétrie et d'alerte.

Hors de portée : création de contrat Kotodama et modifications ABI IVM (voir `docs/source/nexus.md` pour le
modèle d’exécution).

## Matrice des fonctionnalités
| Chemin | Fonctionnalités Cargo à activer | Résultat | Quand utiliser |
| ---- | ----------------------- | ------ | ----------- |
| Prouveur de production (par défaut) | _aucun_ | Backend Stage6 FASTPQ avec planificateur FFT/LDE et pipeline DEEP-FRI.【crates/fastpq_prover/src/backend.rs:1144】 | Valeur par défaut pour tous les binaires de production. |
| Accélération GPU en option | `fastpq_prover/fastpq-gpu` | Active les noyaux CUDA/Metal avec un repli automatique du processeur.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Hôtes avec accélérateurs pris en charge. |

## Procédure de construction
1. **Construction CPU uniquement**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Le backend de production est compilé par défaut ; aucune fonctionnalité supplémentaire n’est requise.

2. **Construction compatible GPU (facultatif)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   La prise en charge du GPU nécessite une boîte à outils SM80+ CUDA avec `nvcc` disponible lors de la construction.【crates/fastpq_prover/Cargo.toml:11】

3. **Auto-tests**
   ```bash
   cargo test -p fastpq_prover
   ```
   Exécutez-le une fois par version pour confirmer le chemin Stage6 avant l'empaquetage.

### Préparation de la chaîne d'outils métalliques (macOS)
1. Installez les outils de ligne de commande Metal avant la construction : `xcode-select --install` (si les outils CLI sont manquants) et `xcodebuild -downloadComponent MetalToolchain` pour récupérer la chaîne d'outils GPU. Le script de build appelle directement `xcrun metal`/`xcrun metallib` et échouera rapidement si les binaires sont absents.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Pour valider le pipeline avant CI, vous pouvez mettre en miroir le script de build localement :
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Lorsque cela réussit, la build émet `FASTPQ_METAL_LIB=<path>` ; le runtime lit cette valeur pour charger le metallib de manière déterministe.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Définissez `FASTPQ_SKIP_GPU_BUILD=1` lors de la compilation croisée sans la chaîne d'outils Metal ; la build imprime un avertissement et le planificateur reste sur le chemin du CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Les nœuds reviennent automatiquement au CPU si Metal n'est pas disponible (framework manquant, GPU non pris en charge ou `FASTPQ_METAL_LIB` vide) ; le script de construction efface la variable d'environnement et le planificateur enregistre la rétrogradation.### Liste de contrôle de publication (étape 6)
Gardez le ticket de version FASTPQ bloqué jusqu'à ce que chaque élément ci-dessous soit complet et joint.

1. **Mesures de preuve en moins d'une seconde** — Inspectez le `fastpq_metal_bench_*.json` fraîchement capturé et
   confirmez l'entrée `benchmarks.operations` où `operation = "lde"` (et le miroir
   `report.operations`) signale `gpu_mean_ms ≤ 950` pour la charge de travail de 20 000 lignes (32 768 complétées
   lignes). Les captures en dehors du plafond nécessitent de nouvelles exécutions avant que la liste de contrôle puisse être signée.
2. **Manifeste signé** — Exécuter
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   le ticket de sortie porte donc à la fois le manifeste et sa signature détachée
   (`artifacts/fastpq_bench_manifest.sig`). Les évaluateurs vérifient la paire résumé/signature avant
   promotion d'une version.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Le manifeste matriciel (construit
   via `scripts/fastpq/capture_matrix.sh`) code déjà le plancher de 20 000 lignes et
   déboguer une régression.
3. **Pièces jointes de preuves** — Téléchargez le benchmark Metal JSON, le journal stdout (ou la trace Instruments),
   Sorties du manifeste CUDA/Metal et signature détachée du ticket de version. L'entrée de la liste de contrôle
   doit être lié à tous les artefacts ainsi qu'à l'empreinte digitale de la clé publique utilisée pour la signature afin d'effectuer des audits en aval
   peut rejouer l'étape de vérification.【artifacts/fastpq_benchmarks/README.md:65】### Workflow de validation des métaux
1. Après une build compatible GPU, confirmez que `FASTPQ_METAL_LIB` pointe sur un `.metallib` (`echo $FASTPQ_METAL_LIB`) afin que le runtime puisse le charger de manière déterministe. 【crates/fastpq_prover/build.rs:188】
2. Exécutez la suite de parité avec les voies GPU forcées :\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Le backend exercera les noyaux Metal et enregistrera un repli déterministe du processeur si la détection échoue.
3. Capturez un échantillon de référence pour les tableaux de bord :\
   localiser la bibliothèque Metal compilée (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   exportez-le via `FASTPQ_METAL_LIB` et exécutez\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Le profil canonique `fastpq-lane-balanced` complète désormais chaque capture à 32 768 lignes (2¹⁵), de sorte que le JSON transporte à la fois `rows` et `padded_rows` ainsi que la latence Metal LDE ; réexécutez la capture si `zero_fill` ou les paramètres de file d'attente poussent le GPU LDE au-delà de l'objectif de 950 ms (<1 s) sur les hôtes de la série AppleM. Archivez le JSON/log résultant avec d'autres preuves de version ; le flux de travail macOS nocturne effectue la même exécution et télécharge ses artefacts à des fins de comparaison.
  Lorsque vous avez besoin d'une télémétrie Poséidon uniquement (par exemple, pour enregistrer une trace d'instruments), ajoutez `--operation poseidon_hash_columns` à la commande ci-dessus ; le banc respectera toujours `FASTPQ_GPU=gpu`, émettra `metal_dispatch_queue.poseidon` et inclura le nouveau bloc `poseidon_profiles` afin que le lot de versions documente explicitement le goulot d'étranglement de Poséidon.
  Les preuves incluent désormais `zero_fill.{bytes,ms,queue_delta}` plus `kernel_profiles` (par noyau
  occupation, Go/s estimés et statistiques de durée) afin que l'efficacité du GPU puisse être représentée graphiquement sans
  retraitement des traces brutes, et un bloc `twiddle_cache` (hits/misses + `before_ms`/`after_ms`) qui
  prouve que les téléchargements Twiddle mis en cache sont en vigueur. `--trace-dir` relance le harnais sous
  `xcrun xctrace record` et
  stocke un fichier `.trace` horodaté à côté du JSON ; vous pouvez toujours fournir un sur mesure
  `--trace-output` (avec `--trace-template` / `--trace-seconds` en option) lors de la capture vers un
  emplacement/modèle personnalisé. Le JSON enregistre `metal_trace_{template,seconds,output}` pour l'audit.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Après chaque capture, exécutez `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` afin que la publication contienne les métadonnées de l'hôte (incluant désormais `metadata.metal_trace`) pour le package de carte/alerte Grafana (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). Le rapport contient désormais un objet `speedup` par opération (`speedup.ratio`, `speedup.delta_ms`), le wrapper hisse `zero_fill_hotspots` (octets, latence, Go/s dérivés et compteurs delta de file d'attente Metal), aplatit `kernel_profiles` en `benchmarks.kernel_summary`, conserve le bloc `twiddle_cache` intact, copie le nouveau bloc/résumé `post_tile_dispatches` afin que les réviseurs puissent prouver que le noyau multi-passes a été exécuté pendant la capture, et résume désormais les preuves du microbench Poséidon dans `benchmarks.poseidon_microbench` afin que les tableaux de bord puissent citer la latence scalaire par rapport à la valeur par défaut sans réanalyse. le rapport brut. La porte manifeste lit le même bloc et rejette les ensembles de preuves GPU qui l'omettent, obligeant les opérateurs à actualiser les captures chaque fois que le chemin de post-pavage est ignoré ou mal configuré.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Le noyau Poseidon2 Metal partage les mêmes boutons : `FASTPQ_METAL_POSEIDON_LANES` (32 à 256, puissances de deux) et `FASTPQ_METAL_POSEIDON_BATCH` (1 à 32 états par voie) vous permettent d'épingler la largeur de lancement et de travailler par voie sans reconstruction ; l'hôte transmet ces valeurs via `PoseidonArgs` avant chaque envoi. Par défaut, le moteur d'exécution inspecte `MTLDevice::{is_low_power,is_headless,location}` pour orienter les GPU discrets vers les lancements à plusieurs niveaux VRAM (`256×24` lorsque ≥48 Go est signalé, `256×20` à 32 Go, `256×16` dans le cas contraire) tandis que les SoC basse consommation restent allumés. `256×8` (et les anciennes pièces à 128/64 voies s'en tiennent à 8/6 états par voie), de sorte que la plupart des opérateurs n'ont jamais besoin de définir les variables d'environnement manuellement. sous `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` et émet un bloc `poseidon_microbench` qui enregistre les deux profils de lancement ainsi que l'accélération mesurée par rapport à la voie scalaire afin que les bundles de versions puissent prouver que le nouveau noyau réduit réellement `poseidon_hash_columns`, et il inclut le bloc `poseidon_pipeline` afin que les preuves Stage7 capturent les boutons de profondeur/chevauchement des morceaux aux côtés des nouveaux niveaux d'occupation. Laissez l'environnement non défini pour les exécutions normales ; le harnais gère automatiquement la réexécution, enregistre les échecs si la capture enfant ne peut pas s'exécuter et se ferme immédiatement lorsque `FASTPQ_GPU=gpu` est défini mais qu'aucun backend GPU n'est disponible, de sorte que les solutions de secours silencieuses du processeur ne se faufilent jamais dans les performances. artefacts.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Le wrapper rejette les captures Poséidon pour lesquelles il manque le delta `metal_dispatch_queue.poseidon`, les compteurs `column_staging` partagés ou les blocs de preuve `poseidon_profiles`/`poseidon_microbench`. Les opérateurs doivent donc actualiser toute capture qui ne parvient pas à prouver le chevauchement des étapes ou la comparaison scalaire par rapport à la valeur par défaut. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Lorsque vous avez besoin d'un JSON autonome pour les tableaux de bord ou les deltas CI, exécutez `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ; l'assistant accepte à la fois les artefacts encapsulés et les captures `fastpq_metal_bench*.json` brutes, émettant `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` avec les timings par défaut/scalaires, ajustant les métadonnées et l'accélération enregistrée. 【scripts/fastpq/export_poseidon_microbench.py:1】
  Terminez l'exécution en exécutant `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` afin que la liste de contrôle de version Stage6 applique le plafond LDE `<1 s` et émette un ensemble manifeste/digest signé qui est livré avec le ticket de version.
4. Vérifiez la télémétrie avant le déploiement : bouclez le point de terminaison Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) et inspectez les journaux `telemetry::fastpq.execution_mode` à la recherche d'un `resolved="cpu"` inattendu. entrées.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Documentez le chemin de secours du processeur en le forçant intentionnellement (`FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"`) afin que les playbooks SRE restent alignés sur le comportement déterministe.6. Réglage facultatif : par défaut, l'hôte sélectionne 16 voies pour les traces courtes, 32 pour les traces moyennes et 64/128 une fois `log_len ≥ 10/14`, atterrissant à 256 lorsque `log_len ≥ 18`, et il conserve désormais la tuile de mémoire partagée à cinq étapes pour les petites traces, quatre une fois `log_len ≥ 12` et 12/14/16 étapes pour `log_len ≥ 18/20/22` avant de lancer le travail sur le noyau post-carrelage. Exportez `FASTPQ_METAL_FFT_LANES` (puissance de deux entre 8 et 256) et/ou `FASTPQ_METAL_FFT_TILE_STAGES` (1 à 16) avant d'exécuter les étapes ci-dessus pour remplacer ces heuristiques. Les tailles de lot de colonnes FFT/IFFT et LDE dérivent de la largeur du groupe de threads résolue (≈2048 threads logiques par répartition, plafonnés à 32 colonnes, et diminuant désormais jusqu'à 32→16→8→4→2→1 à mesure que le domaine grandit) tandis que le chemin LDE applique toujours ses limites de domaine ; définissez `FASTPQ_METAL_FFT_COLUMNS` (1 à 32) pour épingler une taille de lot FFT déterministe et `FASTPQ_METAL_LDE_COLUMNS` (1 à 32) pour appliquer le même remplacement au répartiteur LDE lorsque vous avez besoin de comparaisons bit par bit entre les hôtes. La profondeur des tuiles LDE reflète également l'heuristique FFT : les traces avec `log₂ ≥ 18/20/22` n'exécutent que 12/10/8 étapes de mémoire partagée avant de transmettre les larges papillons au noyau de post-pavage - et vous pouvez outrepasser cette limite via `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). Le runtime transmet toutes les valeurs via les arguments du noyau Metal, bloque les remplacements non pris en charge et enregistre les valeurs résolues afin que les expériences restent reproductibles sans reconstruire la metallib ; le JSON de référence fait apparaître à la fois le réglage résolu et le budget de remplissage nul de l'hôte (`zero_fill.{bytes,ms,queue_delta}`) capturés via les statistiques LDE afin que les deltas de file d'attente soient directement liés à chaque capture, et ajoute désormais un bloc `column_staging` (lots aplatis, flatten_ms, wait_ms, wait_ratio) afin que les réviseurs puissent vérifier le chevauchement hôte/périphérique introduit par le pipeline à double tampon. Lorsque le GPU refuse de signaler une télémétrie de remplissage nul, le harnais synthétise désormais un timing déterministe à partir des effacements de tampon côté hôte et l'injecte dans le bloc `zero_fill` afin que les preuves de publication ne soient jamais expédiées sans le champ.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. La répartition multi-files d'attente est automatique sur les Mac discrets : lorsque `Device::is_low_power()` renvoie false ou que le périphérique Metal signale un emplacement/emplacement externe, l'hôte instancie deux `MTLCommandQueue`, ne se déploie que lorsque la charge de travail transporte ≥ 16 colonnes (mise à l'échelle par la sortance), et effectue un tourniquet des lots de colonnes dans les files d'attente afin que de longues traces maintiennent les deux voies GPU occupées sans compromettre le déterminisme. Remplacez la stratégie par `FASTPQ_METAL_QUEUE_FANOUT` (1 à 4 files d'attente) et `FASTPQ_METAL_COLUMN_THRESHOLD` (colonnes totales minimales avant diffusion) chaque fois que vous avez besoin de captures reproductibles sur plusieurs machines ; les tests de parité forcent ces remplacements afin que les Mac multi-GPU restent couverts et que la distribution/le seuil résolus soient enregistrés à côté de la télémétrie de profondeur de file d'attente.### Preuve à archiver
| Artefact | Capturer | Remarques |
|--------------|---------|-------|
| Ensemble `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` et `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` suivis de `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` et `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Prouve que la CLI/la chaîne d'outils Metal a été installée et a produit une bibliothèque déterministe pour ce commit.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Aperçu de l'environnement | `echo $FASTPQ_METAL_LIB` après la construction ; conservez le chemin absolu avec votre ticket de sortie. | Une sortie vide signifie que Metal a été désactivé ; en enregistrant la valeur documentant que les voies GPU restent disponibles sur l'artefact d'expédition.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| Journal de parité GPU | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` et archivez l'extrait contenant `backend="metal"` ou l'avertissement de rétrogradation. | Démontre que les noyaux s'exécutent (ou reculent de manière déterministe) avant de promouvoir la build.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Sortie de référence | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces` ; enveloppez et signez via `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | Les enregistrements JSON encapsulés `speedup.ratio`, `speedup.delta_ms`, réglage FFT, lignes rembourrées (32 768), enrichis `zero_fill`/`kernel_profiles`, l'aplati `kernel_summary`, le vérifié Les blocs `metal_dispatch_queue.poseidon`/`poseidon_profiles` (lorsque `--operation poseidon_hash_columns` est utilisé) et les métadonnées de trace afin que la moyenne GPU LDE reste ≤950 ms et Poséidon reste <1 s ; conservez à la fois le bundle et la signature `.json.asc` générée avec le ticket de version afin que les tableaux de bord et les auditeurs puissent vérifier l'artefact sans réexécuter les charges de travail. |
| Manifeste de banc | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Valide les deux artefacts GPU, échoue si la moyenne LDE dépasse le plafond `<1 s`, enregistre les résumés BLAKE3/SHA-256 et émet un manifeste signé afin que la liste de contrôle de publication ne puisse pas avancer sans métriques vérifiables.
| Offre groupée CUDA | Exécutez `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` sur l'hôte du laboratoire SM80, enveloppez/signez le JSON dans `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (utilisez `--label device_class=xeon-rtx-sm80` pour que les tableaux de bord sélectionnent la classe correcte), ajoutez le chemin d'accès à `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` et conservez la paire `.json`/`.asc` avec l'artefact métallique avant de régénérer le manifeste. Le `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` enregistré illustre le format de bundle exact attendu par les auditeurs.
| Preuve de télémétrie | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` plus le journal `telemetry::fastpq.execution_mode` émis au démarrage. | Confirme que Prometheus/OTEL expose `device_class="<matrix>", backend="metal"` (ou un journal de rétrogradation) avant d'activer le trafic. 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 || Forage CPU forcé | Exécutez un court lot avec `FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"` et capturez le journal de rétrogradation. | Maintient les runbooks SRE alignés sur le chemin de secours déterministe au cas où une restauration serait nécessaire à mi-version.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Capture de trace (facultatif) | Répétez un test de parité avec `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` et enregistrez la trace de répartition émise. | Préserve les preuves d'occupation/groupe de discussion pour les examens de profilage ultérieurs sans réexécuter les tests de référence.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Les fichiers multilingues `fastpq_plan.*` font référence à cette liste de contrôle afin que les opérateurs de préparation et de production suivent la même piste de preuves. 【docs/source/fastpq_plan.md:1】

## Builds reproductibles
Utilisez le workflow de conteneur épinglé pour produire des artefacts Stage6 reproductibles :

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Le script d'assistance crée l'image de la chaîne d'outils `rust:1.88.0-slim-bookworm` (et `nvidia/cuda:12.2.2-devel-ubuntu22.04` pour le GPU), exécute la construction à l'intérieur du conteneur et écrit `manifest.json`, `sha256s.txt` et les binaires compilés dans la sortie cible. répertoire.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Remplacements d'environnement :
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – épinglez une base/tag Rust explicite.
- `FASTPQ_CUDA_IMAGE` – échangez la base CUDA lors de la production d'artefacts GPU.
- `FASTPQ_CONTAINER_RUNTIME` – force un runtime spécifique ; par défaut, `auto` essaie `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – ordre de préférence séparé par des virgules pour la détection automatique à l'exécution (par défaut : `docker,podman,nerdctl`).

## Mises à jour de configuration
1. Définissez le mode d'exécution du runtime dans votre TOML :
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   La valeur est analysée via `FastpqExecutionMode` et est transmise au backend au démarrage.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Remplacez au lancement si nécessaire :
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   La CLI remplace la configuration résolue avant le démarrage du nœud.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Les développeurs peuvent temporairement forcer la détection sans toucher aux configurations en exportant
   `FASTPQ_GPU={auto,cpu,gpu}` avant de lancer le binaire ; le remplacement est enregistré et le pipeline
   fait toujours apparaître le mode résolu.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Liste de contrôle de vérification
1. **Journaux de démarrage**
   - Attendez-vous à `FASTPQ execution mode resolved` de la cible `telemetry::fastpq.execution_mode` avec
     Étiquettes `requested`, `resolved` et `backend`.【crates/fastpq_prover/src/backend.rs:208】
   - Lors de la détection automatique du GPU, un journal secondaire de `fastpq::planner` signale la voie finale.
   - Metal héberge la surface `backend="metal"` lorsque metallib se charge avec succès ; si la compilation ou le chargement échoue, le script de construction émet un avertissement, efface `FASTPQ_METAL_LIB` et le planificateur enregistre `GPU acceleration unavailable` avant de rester actif. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Mesures Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Le compteur est incrémenté via `record_fastpq_execution_mode` (désormais étiqueté par
   `{device_class,chip_family,gpu_kind}`) chaque fois qu'un nœud résout son exécution
   mode.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Pour la couverture Métal, confirmer
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     incréments parallèlement à vos tableaux de bord de déploiement.【crates/iroha_telemetry/src/metrics.rs:5397】
   - Les nœuds macOS compilés avec `irohad --features fastpq-gpu` exposent également
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     et
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` donc les tableaux de bord Stage7
     peut suivre le cycle de service et la marge de file d'attente à partir des scrapes Prometheus en direct. 【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Exportation de télémétrie**
   - Les builds OTEL émettent `fastpq.execution_mode_resolutions_total` avec les mêmes étiquettes ; assurer votre
     les tableaux de bord ou les alertes surveillent les `resolved="cpu"` inattendus lorsque les GPU doivent être actifs.

4. ** Sanité mentale prouver/vérifier **
   - Exécutez un petit lot via `iroha_cli` ou un harnais d'intégration et confirmez la vérification des épreuves sur un
     homologue compilé avec les mêmes paramètres.

## Dépannage
- **Le mode résolu reste CPU sur les hôtes GPU** — vérifiez que le binaire a été construit avec
  `fastpq_prover/fastpq-gpu`, les bibliothèques CUDA sont sur le chemin du chargeur et `FASTPQ_GPU` ne force pas
  `cpu`.
- **Metal indisponible sur Apple Silicon** — vérifiez que les outils CLI sont installés (`xcode-select --install`), réexécutez `xcodebuild -downloadComponent MetalToolchain` et assurez-vous que la build a produit un chemin `FASTPQ_METAL_LIB` non vide ; une valeur vide ou manquante désactive le backend par conception.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **Erreurs `Unknown parameter`** — assurez-vous que le prouveur et le vérificateur utilisent le même catalogue canonique
  émis par `fastpq_isi` ; la surface ne correspond pas à `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Repli inattendu du processeur** — inspectez `cargo tree -p fastpq_prover --features` et
  confirmez que `fastpq_prover/fastpq-gpu` est présent dans les versions GPU ; vérifiez que les bibliothèques `nvcc`/CUDA se trouvent sur le chemin de recherche.
- **Compteur de télémétrie manquant** — vérifiez que le nœud a été démarré avec `--features telemetry` (par défaut)
  et que l'exportation OTEL (si activée) inclut le pipeline de métriques.【crates/iroha_telemetry/src/metrics.rs:8887】

## Procédure de secours
Le backend d’espace réservé déterministe a été supprimé. Si une régression nécessite un retour en arrière,
redéployer les artefacts de version précédemment connus et enquêter avant de rééditer Stage6
binaires. Documentez la décision de gestion du changement et assurez-vous que le déploiement ultérieur se termine uniquement après la
la régression est comprise.

3. Surveillez la télémétrie pour vous assurer que `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` reflète les valeurs attendues.
   exécution d’espace réservé.

## Référence matérielle
| Profil | Processeur | GPU | Remarques |
| ------- | --- | --- | ----- |
| Référence (étape 6) | AMD EPYC7B12 (32 cœurs), 256 Go de RAM | NVIDIA A10040GB (CUDA12.2) | Les lots synthétiques de 20 000 lignes doivent être exécutés ≤ 1 000 ms.【docs/source/fastpq_plan.md:131】 |
| CPU uniquement | ≥32 cœurs physiques, AVX2 | – | Attendez-vous à environ 0,9 à 1,2 s pour 20 000 lignes ; gardez `execution_mode = "cpu"` pour le déterminisme. |## Tests de régression
-`cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (sur les hôtes GPU)
- Vérification facultative des luminaires dorés :
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Documentez tout écart par rapport à cette liste de contrôle dans votre runbook d'opérations et mettez à jour `status.md` après la
la fenêtre de migration est terminée.