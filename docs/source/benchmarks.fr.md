---
lang: fr
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2026-01-03T18:07:57.716816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rapport d'analyse comparative

Des instantanés détaillés par exécution et l'historique FASTPQ WP5-B sont disponibles
[`benchmarks/history.md`](benchmarks/history.md) ; utiliser cet index lors de la fixation
des artefacts aux revues de feuilles de route ou aux audits SRE. Régénérez-le avec
`python3 scripts/fastpq/update_benchmark_history.py` chaque fois qu'un nouveau GPU capture
ou Poséidon manifeste la terre.

## Ensemble de preuves d'accélération

Chaque benchmark GPU ou mode mixte doit inclure les paramètres d'accélération appliqués
ainsi WP6-B/WP6-C peut prouver la parité de configuration aux côtés des artefacts de synchronisation.

- Capturez l'instantané d'exécution avant/après chaque exécution :
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (utilisez `--format table` pour les journaux lisibles par l'homme). Ceci enregistre `enable_{metal,cuda}`,
  Seuils Merkle, limites de biais du processeur SHA-2, bits d'intégrité du backend détectés et tout
  erreurs de parité persistantes ou raisons de désactivation.
- Stockez le JSON à côté de la sortie du benchmark encapsulée
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, balayage Merkle
  captures, etc.) afin que les réviseurs puissent comparer les horaires et la configuration ensemble.
- Les définitions des boutons et les valeurs par défaut se trouvent dans `docs/source/config/acceleration.md` ; quand
  les remplacements sont appliqués (par exemple, `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  notez-les dans les métadonnées d’exécution pour que les réexécutions restent reproductibles sur tous les hôtes.

## Norito benchmark de niveau 1 (WP5-B/C)

- Commande : `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  émet JSON + Markdown sous `benchmarks/norito_stage1/` avec des timings par taille
  pour le constructeur d'index structurel scalaire ou accéléré.
- Dernières exécutions (macOS aarch64, profil dev) en direct sur
  `benchmarks/norito_stage1/latest.{json,md}` et le nouveau CSV de basculement de
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) affiche SIMD
  gagne à partir de ~ 6 à 8 Ko. GPU/parallèle Stage-1 est désormais par défaut à **192 Ko**
  coupure (`NORITO_STAGE1_GPU_MIN_BYTES=<n>` pour remplacer) pour éviter les problèmes de lancement
  sur de petits documents tout en permettant des accélérateurs pour des charges utiles plus importantes.

## Enum vs répartition d'objets de trait

- Temps de compilation (build debug) : 16,58s
- Runtime (Critère, plus bas c'est mieux) :
  - `enum` : 386 ch (moyenne)
  - `trait_object` : 1,56 ns (moyenne)

Ces mesures proviennent d'un microbenchmark comparant une répartition basée sur une énumération à une implémentation d'objet de trait en boîte.

## Traitement par lots Poséidon CUDA

Le benchmark Poséidon (`crates/ivm/benches/bench_poseidon.rs`) inclut désormais des charges de travail qui exercent à la fois des permutations de hachage unique et les nouveaux assistants par lots. Exécutez la suite avec :

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion enregistrera les résultats sous `target/criterion/poseidon*_many`. Lorsqu'un travailleur GPU est disponible, exportez les résumés JSON (par exemple, copiez `target/criterion/**/new/benchmark.json` dans `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) (par exemple, copiez `target/criterion/**/new/benchmark.json` dans `benchmarks/poseidon/`) afin que les équipes en aval puissent comparer le débit CPU par rapport au débit CUDA pour chaque taille de lot. Jusqu'à ce que la voie GPU dédiée soit mise en ligne, le benchmark revient à l'implémentation SIMD/CPU et fournit toujours des données de régression utiles pour les performances des lots.

Pour des captures reproductibles (et pour conserver la preuve de parité avec les données de synchronisation), exécutez

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```qui génère des lots déterministes Poséidon2/6, enregistre les raisons de santé/désactivation de CUDA, vérifie
parité avec le chemin scalaire, et émet des résumés ops/sec + accélération aux côtés du Metal
état d'exécution (indicateur de fonctionnalité, disponibilité, dernière erreur). Les hôtes CPU uniquement écrivent toujours le scalaire
faites référence et notez l'accélérateur manquant, afin que CI puisse publier des artefacts même sans GPU
coureur.

## Benchmark FASTPQ Metal (Apple Silicon)

La voie GPU a capturé une exécution de bout en bout mise à jour de `fastpq_metal_bench` sur macOS 14 (arm64) avec le jeu de paramètres équilibrés en voie, 20 000 lignes logiques (remplies à 32 768) et 16 groupes de colonnes. L'artefact enveloppé se trouve à `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`, avec la trace de métal stockée aux côtés des captures précédentes sous `traces/fastpq_metal_trace_*_rows20000_iter5.trace`. Les timings moyennés (à partir de `benchmarks.operations[*]`) sont désormais les suivants :

| Opération | Moyenne CPU (ms) | Moyenne des métaux (ms) | Accélération (x) |
|---------------|---------------|-----------------|-------------|
| FFT (32 768 entrées) | 83.29 | 79,95 | 1.04 |
| IFFT (32 768 entrées) | 93,90 | 78.61 | 1.20 |
| LDE (262 144 entrées) | 669,54 | 657,67 | 1.02 |
| Colonnes de hachage Poséidon (524 288 entrées) | 29 087,53 | 30 004,90 | 0,97 |

Observations :

- FFT/IFFT bénéficient tous deux des noyaux BN254 actualisés (IFFT efface la régression précédente d'environ 20 %).
- LDE reste proche de la parité ; le remplissage nul enregistre désormais 33 554 432 octets complétés avec une moyenne de 18,66 ms afin que le bundle JSON capture l'impact de la file d'attente.
- Le hachage Poséidon est toujours lié au processeur sur ce matériel ; continuez à comparer avec les manifestes du microbench Poséidon jusqu'à ce que le chemin Metal adopte les derniers contrôles de file d'attente.
- Chaque capture enregistre désormais `AccelerationSettings.runtimeState().metal.lastError`, permettant
  les ingénieurs annotent les replis du processeur avec la raison de désactivation spécifique (basculement de politique,
  échec de parité, pas de périphérique) directement dans l'artefact de référence.

Pour reproduire l'exécution, construisez les noyaux Metal et exécutez :

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

Validez le JSON résultant sous `artifacts/fastpq_benchmarks/` avec la trace Metal afin que les preuves de déterminisme restent reproductibles.

## Automatisation FASTPQ CUDA

Les hôtes CUDA peuvent exécuter et encapsuler le benchmark SM80 en une seule étape avec :

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

L'assistant invoque `fastpq_cuda_bench`, passe par les étiquettes/périphériques/notes, honore
`--require-gpu` et (par défaut) encapsule/signe via `scripts/fastpq/wrap_benchmark.py`.
Les sorties incluent le JSON brut, le bundle encapsulé sous `artifacts/fastpq_benchmarks/`,
et un `<name>_plan.json` à côté de la sortie qui enregistre les commandes/env exactes donc
Les captures de l'étape 7 restent reproductibles sur tous les exécuteurs GPU. Ajoutez `--sign-output` et
`--gpg-key <id>` lorsque les signatures sont requises ; utilisez `--dry-run` pour émettre uniquement le
plan/chemins sans exécuter le banc.

### Capture de version GA (macOS 14 arm64, équilibrage des voies)

Pour satisfaire WP2-D, nous avons également enregistré une version build sur le même hôte avec GA-ready
heuristique de file d'attente et l'a publié comme
`fastpq_metal_bench_20k_release_macos14_arm64.json`. L'artefact capture deux
lots de colonnes (équilibrés en voies, complétés à 32 768 lignes) et incluent Poséidon
échantillons de microbench pour la consommation du tableau de bord.| Opération | Moyenne CPU (ms) | Moyenne des métaux (ms) | Accélération | Remarques |
|-----------|---------------|-----------------|---------|-------|
| FFT (32 768 entrées) | 12.741 | 10.963 | 1,16× | Les noyaux GPU suivent les seuils de file d'attente actualisés. |
| IFFT (32 768 entrées) | 17.499 | 25.688 | 0,68× | Régression attribuée à une répartition conservatrice de la file d'attente ; continuez à affiner l’heuristique. |
| LDE (262 144 entrées) | 68.389 | 65.701 | 1,04× | Le remplissage à zéro enregistre 33 554 432 octets en 9,651 ms pour les deux lots. |
| Colonnes de hachage Poséidon (524 288 entrées) | 1 728,835 | 1 447,076 | 1,19× | Le GPU bat enfin le CPU après les ajustements de la file d'attente Poséidon. |

Les valeurs du microbench Poséidon intégrées dans le JSON affichent une accélération de 1,10 × (voie par défaut
596,229 ms contre 656,251 ms scalaire sur cinq itérations), les tableaux de bord peuvent désormais créer des graphiques
améliorations par voie à côté du banc principal. Reproduisez le run avec :

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Conservez les traces JSON et `FASTPQ_METAL_TRACE_CHILD=1` encapsulées sous
`artifacts/fastpq_benchmarks/` afin que les révisions ultérieures du WP2-D/WP2-E puissent différer le GA
capturez par rapport aux exécutions d’actualisation précédentes sans réexécuter la charge de travail.

Chaque nouvelle capture `fastpq_metal_bench` écrit désormais également un bloc `bn254_metrics`,
qui expose les entrées `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` pour le CPU
de base et quel que soit le backend GPU (Metal/CUDA) actif, **et** un
Bloc `bn254_dispatch` qui enregistre les largeurs de groupe de threads observées, thread logique
nombres et limites de pipeline pour les expéditions BN254 FFT/LDE à colonne unique. Le
le wrapper de référence copie les deux cartes dans `benchmarks.bn254_*`, donc les tableaux de bord et
Les exportateurs Prometheus peuvent supprimer les latences et la géométrie étiquetées sans réanalyser
le tableau des opérations brutes. La dérogation `FASTPQ_METAL_THREADGROUP` s'applique désormais à
Les noyaux BN254 également, ce qui rend les balayages de groupes de threads reproductibles à partir d'un seul bouton.

Pour simplifier les tableaux de bord en aval, exécutez `python3 scripts/benchmarks/export_csv.py`
après avoir capturé un paquet. L'assistant aplatit `poseidon_microbench_*.json` en
faire correspondre les fichiers `.csv` afin que les tâches d'automatisation puissent différer les voies par défaut et scalaires sans
analyseurs personnalisés.

## Microbanc Poséidon (Métal)

`fastpq_metal_bench` se réexécute désormais sous `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` et promeut les timings dans `benchmarks.poseidon_microbench`. Nous avons exporté les dernières captures Metal avec `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` et les avons regroupées via `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`. Les résumés ci-dessous se trouvent sous `benchmarks/poseidon/` :

| Résumé | Paquet emballé | Moyenne par défaut (ms) | Moyenne scalaire (ms) | Accélération vs scalaire | Colonnes x états | Itérations |
|---------|----------------|---------|------------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1 990,49 | 1 994,53 | 1.002 | 64 x 262 144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2 167,66 | 2 152,18 | 0,993 | 64 x 262 144 | 5 |Les deux captures ont haché 262 144 états par exécution (trace log2 = 12) avec une seule itération d’échauffement. La voie « par défaut » correspond au noyau multi-états réglé tandis que « scalaire » verrouille le noyau sur un état par voie à des fins de comparaison.

## Seuils de Merkle balayés

L'exemple `merkle_threshold` (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) met l'accent sur les chemins de hachage Merkle Metal-vs-CPU. La dernière capture AppleSilicon (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) se trouve dans `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` avec une exportation CSV correspondante. Les lignes de base macOS 14 pour processeur uniquement restent sous `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` pour les hôtes sans Metal.

| Feuilles | Meilleur processeur (ms) | Métal meilleur (ms) | Accélération |
|--------|---------------|-------|---------|
| 1 024 | 23.01 | 19h69 | 1,17× |
| 4 096 | 50,87 | 62.12 | 0,82× |
| 8 192 | 95,77 | 96,57 | 0,99 × |
| 16 384 | 64,48 | 58,98 | 1,09× |
| 32 768 | 109,49 | 87,68 | 1,25× |
| 65 536 | 177,72 | 137,93 | 1,29× |

Un plus grand nombre de feuilles bénéficie du métal (1,09-1,29×) ; les compartiments plus petits fonctionnent toujours plus rapidement sur le processeur, de sorte que le CSV conserve les deux colonnes pour l'analyse. L'assistant CSV conserve l'indicateur `metal_available` à côté de chaque profil pour maintenir l'alignement des tableaux de bord de régression GPU et CPU.

Étapes de reproduction :

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Définissez `FASTPQ_METAL_LIB`/`FASTPQ_GPU` si l'hôte nécessite une activation explicite de Metal, et conservez les captures CPU + GPU enregistrées afin que WP1-F puisse tracer les seuils de politique.

Lors de l'exécution à partir d'un shell sans tête, définissez `IVM_DEBUG_METAL_ENUM=1` pour enregistrer l'énumération des périphériques et `IVM_FORCE_METAL_ENUM=1` pour contourner `MTLCreateSystemDefaultDevice()`. La CLI réchauffe la session CoreGraphics **avant** de demander le périphérique Metal par défaut et revient à `MTLCreateSystemDefaultDevice()` lorsque `MTLCopyAllDevices()` renvoie zéro ; si l'hôte ne signale toujours aucun périphérique, la capture conservera `metal_available=false` (les lignes de base utiles du processeur se trouvent sous `macos14_arm64_*`), tandis que les hôtes GPU doivent garder `FASTPQ_GPU=metal` activé afin que le bundle enregistre le backend choisi.

`fastpq_metal_bench` expose un bouton similaire via `FASTPQ_DEBUG_METAL_ENUM=1`, qui imprime les résultats `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` avant que le backend ne décide s'il doit rester sur le chemin du GPU. Activez-le chaque fois que `FASTPQ_GPU=gpu` signale toujours `backend="none"` dans le JSON encapsulé afin que le bundle de capture enregistre exactement comment l'hôte a énuméré le matériel Metal ; le faisceau s'arrête immédiatement lorsque `FASTPQ_GPU=gpu` est défini mais aucun accélérateur n'est détecté, pointant vers le bouton de débogage afin que le bundle de versions ne cache jamais un repli du processeur derrière une exécution forcée du GPU.

L'assistant CSV émet des tables par profil (par exemple `macos14_arm64_*.csv` et `takemiyacStudio.lan_25.0.0_arm64.csv`), en préservant l'indicateur `metal_available` afin que les tableaux de bord de régression puissent ingérer les mesures CPU et GPU sans analyseurs sur mesure.