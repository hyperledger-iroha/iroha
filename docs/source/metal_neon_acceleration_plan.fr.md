---
lang: fr
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2026-01-03T18:08:01.870300+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plan d'accélération Métal & NEON (Swift & Rust)

Ce document capture le plan partagé pour activer le matériel déterministe
accélération (Intégration Metal GPU + NEON/Accelerate SIMD + StrongBox) à travers
l'espace de travail Rust et le SDK Swift. Il aborde les éléments de la feuille de route suivis
sous **Hardware Acceleration Workstream (macOS/iOS)** et fournit un transfert
artefact pour l'équipe Rust IVM, les propriétaires de ponts Swift et les outils de télémétrie.

> Dernière mise à jour : 2026-01-12  
> Propriétaires : IVM Performance TL, responsable du SDK Swift

## Objectifs

1. Réutilisez les noyaux GPU Rust (Poséidon/BN254/CRC64) sur le matériel Apple via Metal
   calculez des shaders avec une parité déterministe par rapport aux chemins CPU.
2. Exposez les bascules d'accélération (`AccelerationConfig`) de bout en bout pour que les applications Swift
   peut opter pour Metal/NEON/StrongBox tout en préservant les garanties ABI/parité.
3. Instrumenter les tableaux de bord CI + pour faire apparaître les données de parité/de référence et les signaler
   régressions sur les chemins CPU vs GPU/SIMD.
4. Partagez des leçons StrongBox/secure-enclave entre Android (AND2) et Swift
   (IOS4) pour maintenir les flux de signature alignés de manière déterministe.

**Mise à jour (actualisation CRC64 + Stage‑1) :** Les assistants GPU CRC64 sont désormais connectés à `norito::core::hardware_crc64` avec une limite par défaut de 192 Ko (remplacement via `NORITO_GPU_CRC64_MIN_BYTES` ou chemin d'assistance explicite `NORITO_CRC64_GPU_LIB`) tout en conservant les solutions de secours SIMD et scalaires. Les basculements JSON Stage‑1 ont été ré-étalonnés (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), en maintenant le basculement scalaire à 4 Ko et en alignant la valeur par défaut du GPU Stage‑1 sur 192 Ko (`NORITO_STAGE1_GPU_MIN_BYTES`), afin que les petits documents restent sur le CPU et que les charges utiles importantes amortissent les coûts de lancement du GPU.

## Livrables et propriétaires

| Jalon | Livrable | Propriétaire(s) | Cible |
|---------------|-------------|--------------|--------|
| Rouille WP2-A/B | Interfaces de shader métalliques reflétant les noyaux CUDA | IVM Perf TL | février 2026 |
| Rouille WP2-C | Tests de parité métal BN254 et voie CI | IVM Perf TL | T2 2026 |
| Rapide IOS6 | Pont bascule câblé (`connect_norito_set_acceleration_config`) + API SDK + échantillons | Propriétaires de ponts Swift | Terminé (janvier 2026) |
| Rapide IOS5 | Exemples d'applications/documents illustrant l'utilisation de la configuration | Swift DX TL | T2 2026 |
| Télémétrie | Flux du tableau de bord avec parité d'accélération + métriques de référence | Programme Swift PM / Télémétrie | Données pilotes T2 2026 |
| CI | Harnais de fumée XCFramework exerçant CPU vs Metal/NEON sur le pool d'appareils | Responsable de l'assurance qualité Swift | T2 2026 |
| Coffre Fort | Tests de parité de signature basés sur le matériel (vecteurs partagés) | Android Crypto TL / Sécurité Swift | T3 2026 |

## Interfaces et contrats API### Rouille (`ivm::AccelerationConfig`)
- Conserver les champs existants (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, seuils).
- Ajout d'un échauffement explicite du Metal pour éviter la latence à la première utilisation (Rust #15875).
- Fournir des API de parité renvoyant l'état/les diagnostics pour les tableaux de bord :
  - par ex. `ivm::vector::metal_status()` -> {activé, parité, last_error}.
- Mesures d'analyse comparative de sortie (timings de l'arbre Merkle, débit CRC) via
  crochets de télémétrie pour `ci/xcode-swift-parity`.
- L'hôte Metal charge désormais le `fastpq.metallib` compilé, envoie FFT/IFFT/LDE
  et Poséidon, et revient à l'implémentation du processeur chaque fois que le
  metallib ou la file d'attente des périphériques n'est pas disponible.

### C FFI (`connect_norito_bridge`)
- Nouvelle structure `connect_norito_acceleration_config` (terminée).
- La couverture Getter inclut désormais `connect_norito_get_acceleration_config` (configuration uniquement) et `connect_norito_get_acceleration_state` (config + parité) pour refléter le setter.
- Disposition de la structure du document dans les commentaires d'en-tête pour les consommateurs SPM/CocoaPods.

### Rapide (`AccelerationSettings`)
- Valeurs par défaut : Métal activé, CUDA désactivé, seuils nuls (héritage).
- Valeurs négatives ignorées ; `apply()` invoqué automatiquement par `IrohaSDK`.
- `AccelerationSettings.runtimeState()` fait désormais surface au `connect_norito_get_acceleration_state`
  charge utile (config + statut de parité Metal/CUDA) pour que les tableaux de bord Swift émettent la même télémétrie
  comme Rouille (`supported/configured/available/parity`). L'assistant renvoie `nil` lorsque le
  le pont est absent pour garder les tests portables.
- `AccelerationBackendStatus.lastError` copie la raison de désactivation/erreur de
  `connect_norito_get_acceleration_state` et libère le tampon natif une fois la chaîne
  matérialisé afin que les tableaux de bord de parité mobile puissent annoter pourquoi Metal/CUDA ont été désactivés sur
  chaque hôte.
-`AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  tests sous `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) maintenant
  résout les manifestes d'opérateur dans le même ordre de priorité que la démo Norito : honorer
  `NORITO_ACCEL_CONFIG_PATH`, recherche groupée `acceleration.{json,toml}` / `client.{json,toml}`,
  enregistrez la source choisie et revenez aux valeurs par défaut. Les applications n'ont plus besoin de chargeurs sur mesure pour
  reflète la surface Rust `iroha_config`.
- Mettez à jour les exemples d'applications et le fichier README pour afficher les bascules et l'intégration de la télémétrie.

### Télémétrie (Tableaux de bord + Exportateurs)
- Flux de parité (mobile_parity.json) :
  - `acceleration.metal/neon/strongbox` -> {activé, parité, perf_delta_pct}.
  - Acceptez la comparaison de base CPU/GPU `perf_delta_pct`.
  - `acceleration.metal.disable_reason` miroirs `AccelerationBackendStatus.lastError`
    afin que l'automatisation Swift puisse signaler les GPU désactivés avec la même fidélité que Rust
    tableaux de bord.
- Flux CI (mobile_ci.json) :
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {processeur, métal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> Double.
- Les exportateurs doivent obtenir des métriques à partir de tests de référence Rust ou d'exécutions CI (par exemple, exécuter
  Microbench métal/CPU faisant partie de `ci/xcode-swift-parity`).### Boutons de configuration et valeurs par défaut (WP6-C)
- `AccelerationConfig` par défaut : `enable_metal = true` sur les builds macOS, `enable_cuda = true` lorsque la fonctionnalité CUDA est compilée, `max_gpus = None` (pas de plafond). Le wrapper Swift `AccelerationSettings` hérite des mêmes valeurs par défaut via `connect_norito_set_acceleration_config`.
- Heuristique Merkle Norito (GPU vs CPU) : `merkle_min_leaves_gpu = 8192` permet le hachage GPU pour les arbres avec ≥8 192 feuilles ; Les remplacements backend (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) sont par défaut définis sur le même seuil, sauf si cela est explicitement défini.
- Heuristique de préférence CPU (SHA2 ISA présent) : sur AArch64 (ARMv8 SHA2) et x86/x86_64 (SHA-NI), le chemin CPU reste préféré jusqu'aux feuilles `prefer_cpu_sha2_max_leaves_* = 32_768` ; au-dessus, le seuil GPU s’applique. Ces valeurs sont configurables via `AccelerationConfig` et doivent être ajustées uniquement avec des preuves de référence.

## Stratégie de test

1. **Tests de parité unitaire (Rust)** : assurez-vous que les noyaux métalliques correspondent aux sorties du processeur pour
   vecteurs déterministes ; exécuté sous `cargo test -p ivm --features metal`.
   `crates/fastpq_prover/src/metal.rs` propose désormais des tests de parité uniquement pour macOS qui
   exercice FFT/IFFT/LDE et Poséidon par rapport à la référence scalaire.
2. **Faisceau de fumée Swift** : étendez le programme d'exécution de test IOS6 pour exécuter CPU vs Metal
   encodage (Merkle/CRC64) sur les émulateurs et les appareils StrongBox ; comparer
   résultats et journalisation du statut de parité.
3. **CI** : mettre à jour `norito_bridge_ios.yml` (appelle déjà `make swift-ci`) pour pousser
   mesures d'accélération des artefacts ; assurez-vous que l'exécution confirme Buildkite
   Métadonnées `ci/xcframework-smoke:<lane>:device_tag` avant de publier les modifications du faisceau,
   et échouer dans la voie en cas de dérive parité/référence.
4. **Tableaux de bord** : les nouveaux champs s'affichent désormais dans la sortie CLI. Veiller à ce que les exportateurs produisent
   données avant de retourner les tableaux de bord en direct.

## Plan de shader métallique WP2-A (Pipelines Poséidon)

La première étape du WP2 couvre le travail de planification des noyaux Poséidon Metal
qui reflètent la mise en œuvre de CUDA. Le plan divise l'effort en noyaux,
planification de l'hôte et mise en scène constante partagée afin que le travail ultérieur puisse se concentrer uniquement sur
mise en œuvre et tests.

### Portée du noyau

1. `poseidon_permute` : permute les états indépendants `state_count`. Chaque fil
   possède un `STATE_CHUNK` (4 états) et exécute toutes les itérations `TOTAL_ROUNDS` en utilisant
   Constantes rondes partagées par le groupe de threads organisées au moment de l'expédition.
2. `poseidon_hash_columns` : lit le catalogue clairsemé `PoseidonColumnSlice` et
   effectue un hachage convivial pour Merkle de chaque colonne (correspondant à celui du processeur)
   `PoseidonColumnBatch`). Il utilise le même tampon constant de groupe de threads
   comme noyau permuté mais boucle sur `(states_per_lane * block_count)`
   sorties afin que le noyau puisse amortir les soumissions en file d'attente.
3. `poseidon_trace_fused` : calcule les résumés parent/feuille pour la table de trace
   en un seul passage. Le noyau fusionné consomme `PoseidonFusedArgs` donc l'hôte
   peut décrire des régions non contiguës et un `leaf_offset`/`parent_offset`, et
   il partage toutes les tables rondes/MDS avec les autres noyaux.

### Planification des commandes et contrats d'hôte- Chaque distribution du noyau passe par `MetalPipelines::command_queue`, qui
  applique le planificateur adaptatif (cible ~ 2 ms) et les contrôles de diffusion de la file d'attente
  exposé via `FASTPQ_METAL_QUEUE_FANOUT` et
  `FASTPQ_METAL_COLUMN_THRESHOLD`. Le chemin d'échauffement dans `with_metal_state`
  compile les trois noyaux Poséidon à l'avance afin que la première expédition ne se fasse pas
  payer une pénalité de création de pipeline.
- Le dimensionnement du groupe de threads reflète les valeurs par défaut Metal FFT/LDE existantes : la cible est
  8 192 fils de discussion par soumission avec un plafond strict de 256 fils de discussion par groupe. Le
  l'hôte peut rétrograder le multiplicateur `states_per_lane` pour les appareils à faible consommation en
  numérotation des remplacements d'environnement (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  à ajouter dans WP2-B) sans modifier la logique du shader.
- La mise en scène des colonnes suit le même pool à double tampon déjà utilisé par la FFT
  canalisations. Les noyaux Poséidon acceptent les pointeurs bruts dans ce tampon intermédiaire
  et ne touchez jamais aux allocations globales de tas, ce qui maintient le déterminisme de la mémoire
  aligné avec l’hôte CUDA.

### Constantes partagées

- Le manifeste `PoseidonSnapshot` décrit dans
  `docs/source/fastpq/poseidon_metal_shared_constants.md` est désormais le canonique
  source pour les constantes rondes et la matrice MDS. Les deux métaux (`poseidon2.metal`)
  et les noyaux CUDA (`fastpq_cuda.cu`) doivent être régénérés chaque fois que le manifeste
  changements.
- WP2-B ajoutera un petit chargeur d'hôte qui lit le manifeste au moment de l'exécution et
  émet le SHA-256 en télémétrie (`acceleration.poseidon_constants_sha`) donc
  les tableaux de bord de parité peuvent affirmer que les constantes du shader correspondent aux valeurs publiées.
  instantané.
- Pendant l'échauffement, nous copierons les constantes `TOTAL_ROUNDS x STATE_WIDTH` dans un
  `MTLBuffer` et téléchargez-le une fois par appareil. Chaque noyau copie ensuite les données
  dans la mémoire du groupe de threads avant de traiter son morceau, garantissant ainsi le déterminisme
  commande même lorsque plusieurs tampons de commandes s'exécutent en vol.

### Crochets de validation

- Les tests unitaires (`cargo test -p fastpq_prover --features fastpq-gpu`) feront croître un
  assertion qui hache les constantes du shader intégrées et les compare avec
  le SHA du manifeste avant d’exécuter la suite de luminaires GPU.
- Les statistiques du noyau existant basculent (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, télémétrie de la profondeur de la file d'attente) deviennent des preuves obligatoires
  pour la sortie WP2 : chaque test doit prouver que le planificateur ne viole jamais les
  configuration de la sortance et que le noyau de trace fusionné maintient la file d'attente en dessous du
  fenêtre adaptative.
- Le harnais anti-fumée Swift XCFramework et les coureurs de référence Rust démarreront
  exporter `acceleration.poseidon.permute_p90_ms{cpu,metal}` pour que WP2-D puisse tracer
  Deltas métal/CPU sans réinventer de nouveaux flux de télémétrie.

## WP2-B Poseidon Manifest Loader et parité d'auto-test- `fastpq_prover::poseidon_manifest()` intègre et analyse désormais
  `artifacts/offline_poseidon/constants.ron`, calcule son SHA-256
  (`poseidon_manifest_sha256()`) et valide l'instantané par rapport au processeur
  tables Poséidon avant l'exécution de tout travail GPU. `build_metal_context()` enregistre le
  digérer pendant l'échauffement afin que les exportateurs de télémétrie puissent publier
  `acceleration.poseidon_constants_sha`.
- L'analyseur de manifeste rejette les tuples de largeur/taux/nombre de tours incompatibles et
  garantit que la matrice MDS manifeste est égale à l'implémentation scalaire, empêchant
  dérive silencieuse lorsque les tables canoniques sont régénérées.
- Ajout de `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`, qui
  analyse les tables Poséidon intégrées dans `poseidon2.metal` et
  `fastpq_cuda.cu` et affirme que les deux noyaux sérialisent exactement de la même manière
  constantes comme manifeste. CI échoue désormais si quelqu'un modifie le shader/CUDA
  fichiers sans régénérer le manifeste canonique.
- Les futurs hooks de parité (WP2-C/D) pourront réutiliser `poseidon_manifest()` pour mettre en scène le
  arrondir les constantes dans les tampons GPU et exposer le résumé via Norito
  flux de télémétrie.

## WP2-C BN254 Pipelines métalliques et tests de parité- **Portée et écart :** Les répartiteurs d'hôtes, les harnais de parité et `bn254_status()` sont actifs, et `crates/fastpq_prover/metal/kernels/bn254.metal` implémente désormais les primitives Montgomery ainsi que les boucles FFT/LDE synchronisées avec les groupes de threads. Chaque répartition exécute une colonne entière à l'intérieur d'un seul groupe de threads avec des barrières par étape, de sorte que les noyaux exercent les manifestes par étapes en parallèle. La télémétrie est désormais câblée et les remplacements du planificateur sont honorés afin que nous puissions lancer le déploiement par défaut avec les mêmes preuves que celles que nous utilisons pour les noyaux Boucle d'or.
- **Exigences du noyau :** ✅ réutilisez les manifestes twiddle/coset mis en scène, convertissez les entrées/sorties une fois et exécutez toutes les étapes radix-2 à l'intérieur du groupe de threads par colonne afin que nous n'ayons pas besoin de synchronisation multi-envoi. Les assistants Montgomery restent partagés entre FFT/LDE, donc seule la géométrie de la boucle a changé.
- **Câblage hôte :** ✅ `crates/fastpq_prover/src/metal.rs` met en scène les membres canoniques, remplit à zéro le tampon LDE, sélectionne un seul groupe de threads par colonne et expose `bn254_status()` pour le déclenchement. Aucun changement d’hôte supplémentaire n’est requis pour la télémétrie.
- **Build guards :** le `fastpq.metallib` est livré avec les noyaux en mosaïque, donc CI échoue toujours rapidement si le shader dérive. Toutes les optimisations futures restent derrière les portes de télémétrie/fonctionnalités plutôt que les commutateurs au moment de la compilation.
- **Appareils de parité :** ✅ Les tests `bn254_parity` continuent de comparer les sorties GPU FFT/LDE aux appareils CPU et s'exécutent désormais en direct sur du matériel Metal ; gardez à l'esprit les tests de manifeste falsifié si de nouveaux chemins de code du noyau apparaissent.
- **Télémétrie et benchmarks :** `fastpq_metal_bench` émet désormais :
  - un bloc `bn254_dispatch` résumant les largeurs de groupe de threads par répartition, le nombre de threads logiques et les limites de pipeline pour les lots FFT/LDE à colonne unique ; et
  - un bloc `bn254_metrics` qui enregistre `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` pour la référence du processeur ainsi que le backend GPU exécuté.
  Le wrapper de référence copie les deux cartes dans chaque artefact encapsulé afin que les tableaux de bord WP2-D ingèrent les latences/géométries étiquetées sans procéder à une ingénierie inverse du tableau des opérations brutes. `FASTPQ_METAL_THREADGROUP` s'applique désormais également aux expéditions BN254 FFT/LDE, ce qui rend le bouton utilisable pour les performances. balayages.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## Questions ouvertes (résolues en mai 2027)1. **Nettoyage des ressources métalliques :** `warm_up_metal()` réutilise le thread-local
   `OnceCell` et dispose désormais de tests d'idempotence/régression
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`), donc transitions du cycle de vie des applications
   peut appeler en toute sécurité le chemin d'échauffement sans fuite ni double initialisation.
2. **Bases de référence :** Les voies métalliques doivent rester à moins de 20 % du processeur.
   référence pour FFT/IFFT/LDE et dans les 15 % pour les assistants Poséidon CRC/Merkle ;
   l'alerte doit se déclencher lorsque `acceleration.*_perf_delta_pct > 0.20` (ou manquant)
   dans le flux de parité mobile. Régressions IFFT observées dans le bundle de traces 20k
   sont désormais bloqués par le correctif de remplacement de file d'attente indiqué dans WP2-D.
3. **StrongBox de secours :** Swift suit le playbook de secours d'Android en
   enregistrement des échecs d'attestation dans le runbook de support
   (`docs/source/sdk/swift/support_playbook.md`) et passage automatique à
   le chemin logiciel documenté soutenu par HKDF avec journalisation d'audit ; vecteurs de parité
   rester partagé via les appareils OA existants.
4. **Stockage de télémétrie :** Les captures d'accélération et les preuves de pool de périphériques sont
   archivé sous `configs/swift/` (par ex.,
   `configs/swift/xcframework_device_pool_snapshot.json`), et exportateurs
   doit refléter la même disposition (`artifacts/swift/telemetry/acceleration/*.json`
   ou `.prom`) afin que les annotations Buildkite et les tableaux de bord du portail puissent ingérer le
   se nourrit sans grattage ad hoc.

## Prochaines étapes (février 2026)

- [x] Rust : intégration hôte Land Metal (`crates/fastpq_prover/src/metal.rs`) et
      exposer l'interface du noyau pour Swift ; transfert de documents suivi aux côtés du
      Notes de pont rapides.
- [x] Swift : exposer les paramètres d'accélération au niveau du SDK (effectué en janvier 2026).
- [x] Télémétrie : `scripts/acceleration/export_prometheus.py` convertit désormais
      Sortie `cargo xtask acceleration-state --format json` dans un Prometheus
      fichier texte (avec l'étiquette `--instance` en option) afin que les exécutions CI puissent attacher un GPU/CPU
      activation, seuils et raisons de parité/désactivation directement dans un fichier texte
      collecteurs sans grattage sur mesure.
- [x] Swift QA : `scripts/acceleration/acceleration_matrix.py` regroupe plusieurs
      captures de l'état d'accélération dans des tables JSON ou Markdown saisies par appareil
      étiquette, donnant au faisceau de fumée une matrice déterministe « CPU vs Metal/CUDA »
      à télécharger avec l'exemple d'application fume. La sortie Markdown reflète le
      Créez un format de preuve Buildkite afin que les tableaux de bord puissent ingérer le même artefact.
- [x] Mettez à jour status.md maintenant que `irohad` expédie les exportateurs de file d'attente/remplissage zéro et
      les tests de validation env/config couvrent les remplacements de file d'attente Metal, donc WP2-D
      la télémétrie + les liaisons sont accompagnées de preuves en direct.

Commandes d'aide à la télémétrie/exportation :

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## Référence de version WP2-D et notes de liaison- **Capture de version de 20 000 lignes :** Enregistrement d'un nouveau benchmark Metal vs CPU sur macOS14
  (arm64, paramètres équilibrés en voie, trace remplie de 32 768 lignes, lots de deux colonnes) et
  vérifié le bundle JSON dans `fastpq_metal_bench_20k_release_macos14_arm64.json`.
  L'indice de référence exporte les délais par opération ainsi que les preuves du microbanc Poséidon.
  WP2-D possède un artefact de qualité GA lié à la nouvelle heuristique de file d'attente Metal. Titre
  deltas (le tableau complet se trouve dans `docs/source/benchmarks.md`) :

  | Opération | Moyenne CPU (ms) | Moyenne des métaux (ms) | Accélération |
  |-----------|---------------|-----------------|---------|
  | FFT (32 768 entrées) | 12.741 | 10.963 | 1,16× |
  | IFFT (32 768 entrées) | 17.499 | 25.688 | 0,68 × *(régression : la distribution de la file d'attente est limitée pour conserver le déterminisme ; nécessite un réglage de suivi)* |
  | LDE (262 144 entrées) | 68.389 | 65.701 | 1,04× |
  | Colonnes de hachage Poséidon (524 288 entrées) | 1 728,835 | 1 447,076 | 1,19× |

  Chaque capture enregistre les timings `zero_fill` (9,651 ms pour 33 554 432 octets) et
  Entrées `poseidon_microbench` (voie par défaut 596,229 ms vs scalaire 656,251 ms,
  1,10 × accélération) afin que les utilisateurs du tableau de bord puissent différer la pression de la file d'attente en même temps que le
  principales opérations.
- **Lien croisé liaisons/docs :** `docs/source/benchmarks.md` fait désormais référence au
  release JSON et la commande reproducteur, les remplacements de file d'attente Metal sont validés
  via les tests env/manifest `iroha_config`, et `irohad` publie en direct
  `fastpq_metal_queue_*` mesure les tableaux de bord pour signaler les régressions IFFT sans
  grattage de journaux ad hoc. Le `AccelerationSettings.runtimeState` de Swift expose le
  même charge utile de télémétrie livrée dans le bundle JSON, fermant le WP2-D
  écart de liaison/doc avec une ligne de base d'acceptation reproductible.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **Correction de la file d'attente IFFT :** Les lots FFT inverses ignorent désormais l'envoi multi-files d'attente à chaque fois
  la charge de travail atteint à peine le seuil de diffusion (16 colonnes sur la voie équilibrée
  profile), supprimant la régression Metal-vs-CPU évoquée ci-dessus tout en conservant
  charges de travail à grandes colonnes sur le chemin multi-files d'attente pour FFT/LDE/Poseidon.