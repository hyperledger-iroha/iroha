---
lang: fr
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-18T05:31:56.951617+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Répartition du travail du prouveur FASTPQ

Ce document capture le plan par étapes pour fournir un prouveur FASTPQ-ISI prêt pour la production et le connecter au pipeline de planification de l'espace de données. Chaque définition ci-dessous est normative à moins qu'elle ne soit marquée comme TODO. La solidité estimée utilise les limites DEEP-FRI de style Caire ; les tests automatisés d'échantillonnage par rejet dans CI échouent si la limite mesurée tombe en dessous de 128 bits.

## Étape 0 — Espace réservé de hachage (débarqué)
- Codage déterministe Norito avec engagement BLAKE2b.
- Backend d'espace réservé renvoyant `BackendUnavailable`.
- Tableau des paramètres canoniques fourni par `fastpq_isi`.

## Étape 1 — Prototype du générateur de traces

> **Statut (09/11/2025) :** `fastpq_prover` expose désormais l'emballage canonique
> les assistants (`pack_bytes`, `PackedBytes`) et le Poséidon2 déterministe
> engagement de commande sur Boucle d'Or. Les constantes sont épinglées à
> `ark-poseidon2` commit `3f2b7fe`, clôturant le suivi sur l'échange du BLAKE2 intérimaire
> L'espace réservé est fermé. Luminaires dorés (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) ancrent désormais la suite de régression.

### Objectifs
- Implémenter le générateur de trace FASTPQ pour la mise à jour KV AIR. Chaque ligne doit coder :
  - `key_limbs[i]` : membres en base 256 (7 octets, petit-boutiste) du chemin de clé canonique.
  - `value_old_limbs[i]`, `value_new_limbs[i]` : même emballage pour les valeurs pré/post.
  - Colonnes de sélection : `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
  - Colonnes auxiliaires : `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Colonnes d'actifs : `asset_id_limbs[i]` utilisant des membres de 7 octets.
  - Colonnes SMT par niveau `ℓ` : `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, plus `neighbour_leaf` pour non-adhésion.
  - Colonnes de métadonnées : `dsid`, `slot`.
- **Ordre déterministe.** Triez les lignes lexicographiquement par `(key_bytes, op_rank, original_index)` en utilisant un tri stable. Cartographie `op_rank` : `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, `meta_set=5`. `original_index` est l'index basé sur 0 avant le tri. Conservez le hachage de commande Poséidon2 résultant (balise de domaine `fastpq:v1:ordering`). Codez la pré-image de hachage sous la forme `[domain_len, domain_limbs…, payload_len, payload_limbs…]`, où les longueurs sont des éléments de champ u64 afin que les zéros de fin restent distinguables.
- Témoin de recherche : produit `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` lorsque la colonne stockée `s_perm` (OU logique de `s_role_grant` et `s_role_revoke`) est 1. Les ID de rôle/autorisation sont des chaînes LE de 32 octets à largeur fixe ; l'époque est LE de 8 octets.
- Appliquer des invariants à la fois avant et à l'intérieur de l'AIR : sélecteurs mutuellement exclusifs, conservation par actif, constantes dsid/slot.
- `N_trace = 2^k` (`pow2_ceiling` du nombre de lignes) ; `N_eval = N_trace * 2^b` où `b` est l'exposant d'explosion.
- Assurer les agencements et tests immobiliers :
  - Emballage aller-retour (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Commande de hachage de stabilité (`tests/fixtures/ordering_hash.json`).
  - Luminaires par lots (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### Schéma de colonne AIR
| Groupe de colonnes | Noms | Descriptif |
| ----------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Activité | `s_active` | 1 pour les lignes réelles, 0 pour le remplissage.                                                                                       |
| Principal | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Éléments Goldilocks emballés (membres little-endian, 7 octets).                                                             |
| Actif | `asset_id_limbs[i]` | Identificateur d'actif canonique compressé (little-endian, membres de 7 octets).                                                      |
| Sélecteurs | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Contrainte : Σ sélecteurs (dont `s_perm`) = `s_active` ; `s_perm` reflète les lignes d'attribution/révocation de rôle.              |
| Auxiliaire | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | État utilisé pour les contraintes, la conservation et les pistes d'audit.                                                           |
| CMS | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Entrées/sorties Poséidon2 par niveau plus témoin voisin en cas de non-adhésion.                                         |
| Recherche | `perm_hash` | Hachage Poséidon2 pour la recherche d'autorisations (limité uniquement lorsque `s_perm = 1`).                                            |
| Métadonnées | `dsid`, `slot` | Constante sur toutes les lignes.                                                                                                 |### Mathématiques et contraintes
- **Field packaging :** les octets sont regroupés en membres de 7 octets (petit-boutiste). Chaque membre `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k` ; rejeter les membres ≥ module Boucle d’or.
- **Équilibre/conservation :** soit `δ = value_new - value_old`. Regroupez les lignes par `asset_id`. Définissez `r_asset_start = 1` sur la première ligne de chaque groupe d'actifs (0 sinon) et contraignez
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  Sur la dernière ligne de chaque groupe d'actifs, affirmez
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  Les transferts satisfont automatiquement à la contrainte car leurs valeurs δ totalisent zéro dans le groupe. Exemple : si `value_old = 100` et `value_new = 120` sur une ligne de menthe, δ = 20, donc la somme de menthe contribue à +20 et le contrôle final est résolu à zéro lorsqu'aucune brûlure ne se produit.
- **Padding :** présente `s_active`. Multipliez toutes les contraintes de ligne par `s_active` et appliquez un préfixe contigu : `s_active[i] ≥ s_active[i+1]`. Les lignes de remplissage (`s_active=0`) doivent conserver des valeurs constantes mais ne sont par ailleurs pas contraintes.
- **Hachage de commande :** Hachage Poséidon2 (domaine `fastpq:v1:ordering`) sur les encodages de lignes ; stocké dans Public IO à des fins d’audit.

## Étape 2 — STARK Prover Core

### Objectifs
- Construire les engagements Poseidon2 Merkle sur des vecteurs d'évaluation de trace et de recherche. Paramètres : taux = 2, capacité = 1, tours complets = 8, tours partiels = 57, constantes épinglées sur le commit `ark-poseidon2` `3f2b7fe` (v0.3.0).
- Extension de bas degré : évaluez chaque colonne sur le domaine `D = { g^i | i = 0 .. N_eval-1 }`, où `N_eval = 2^{k+b}` divise la capacité 2-adique de Boucle d'or. Soit `g = ω^{(p-1)/N_eval}` avec `ω` la racine primitive fixe de Boucle d'or et `p` son module ; utilisez le sous-groupe de base (pas de coset). Enregistrez `g` dans la transcription (balise `fastpq:v1:lde`).
- Polynômes de composition : pour chaque contrainte `C_j`, former `F_j(X) = C_j(X) / Z_N(X)` avec les marges de degrés listées ci-dessous.
- Argument de recherche (autorisations) : exemple `γ` à partir de la transcription. Produit trace `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Produit de table `T = ∏_j (table_perm_j - γ)`. Contrainte de limite : `Z_final / T = 1`.
- DEEP-FRI avec arité `r ∈ {8, 16}` : pour chaque couche, absorber la racine avec le tag `fastpq:v1:fri_layer_ℓ`, échantillonner `β_ℓ` (tag `fastpq:v1:beta_ℓ`), et plier via `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k`.
- Objet de preuve (codé en Norito) :
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- Vérificateur de miroirs; exécutez une suite de régression sur des traces de 1 000/5 000/20 000 lignes avec des transcriptions dorées.

### Diplôme Comptabilité
| Contrainte | Diplôme avant division | Diplôme après sélecteurs | Marge par rapport à `deg(Z_N)` |
|------------|--------------|--------------|----------------------|
| Conservation transfert/menthe/brûlure | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Recherche d'attribution/révocation de rôle | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Ensemble de métadonnées | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Hachage SMT (par niveau) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Recherche de grand produit | rapport produit | N/A | Contrainte de limite |
| Racines limites / totaux d'approvisionnement | 0 | 0 | exact |

Les lignes de remplissage sont gérées via `s_active` ; les lignes factices étendent la trace jusqu'à `N_trace` sans violer les contraintes.## Encodage et transcription (mondial)
- **Emballage d'octets :** base-256 (membres de 7 octets, petit-boutiste). Tests dans `fastpq_prover/tests/packing.rs`.
- **Encodage de champ :** Boucle d'or canonique (membre petit-boutiste 64 bits, rejet ≥ p) ; Sorties Poséidon2/racines SMT sérialisées sous forme de tableaux petit-boutiste de 32 octets.
- **Transcription (Fiat-Shamir) :**
  1. BLAKE2b absorbe les balises de validation `protocol_version`, `params_version`, `parameter_set`, `public_io` et Poseidon2 (`fastpq:v1:init`).
  2. Absorber `trace_root`, `lookup_root` (`fastpq:v1:roots`).
  3. Dérivez le défi de recherche `γ` (`fastpq:v1:gamma`).
  4. Dérivez les défis de composition `α_j` (`fastpq:v1:alpha_j`).
  5. Pour chaque racine de couche FRI, absorbez avec `fastpq:v1:fri_layer_ℓ`, dérivez `β_ℓ` (`fastpq:v1:beta_ℓ`).
  6. Dérivez les index de requête (`fastpq:v1:query_index`).

  Les balises sont en minuscules ASCII ; les vérificateurs rejettent les discordances avant les contestations d’échantillonnage. Luminaire de transcription doré : `tests/fixtures/transcript_v1.json`.
- **Versioning :** `protocol_version = 1`, `params_version` correspond au jeu de paramètres `fastpq_isi`.

## Argument de recherche (autorisations)
- Table validée triée lexicographiquement par `(role_id_bytes, permission_id_bytes, epoch_le)` et validée via l'arbre Poseidon2 Merkle (`perm_root` dans `PublicIO`).
- Le témoin de trace utilise `perm_hash` et le sélecteur `s_perm` (OU d'attribution/révocation de rôle). Le tuple est codé sous la forme `role_id_bytes || permission_id_bytes || epoch_u64_le` avec des largeurs fixes (32, 32, 8 octets).
- Relation produit :
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Affirmation de limite : `Z_final / T = 1`. Voir `examples/lookup_grand_product.md` pour une présentation pas à pas de l'accumulateur en béton.

## Contraintes d'arbre Merkle clairsemées
- Définir `SMT_HEIGHT` (nombre de niveaux). Les colonnes `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` apparaissent pour tous les `ℓ ∈ [0, SMT_HEIGHT)`.
- Paramètres Poséidon2 épinglés au commit `ark-poseidon2` `3f2b7fe` (v0.3.0) ; balise de domaine `fastpq:v1:poseidon_node`. Tous les nœuds utilisent un codage de champ petit-boutiste.
- Mettre à jour les règles par niveau :
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- Jeu d'inserts `(node_in_0 = 0, node_out_0 = value_new)` ; supprime l'ensemble `(node_in_0 = value_old, node_out_0 = 0)`.
- Les preuves de non-adhésion fournissent `neighbour_leaf` pour indiquer que l'intervalle interrogé est vide. Voir `examples/smt_update.md` pour un exemple concret et une mise en page JSON.
- Contrainte de limite : le hachage final est égal à `old_root` pour les pré-lignes et `new_root` pour les post-lignes.

## Paramètres de solidité et SLO
| N_trace | explosion | Arité VEN | couches | requêtes | les bits les plus | Format d'épreuve (≤) | RAM (≤) | Latence P95 (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 Ko | 1,5 Go | 0,40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 Ko | 2,5 Go | 0,75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 Ko | 3,5 Go | 1,20 s (A100) |

Les dérivations suivent l'annexe A. Le harnais CI produit des preuves mal formées et échoue si les bits estimés sont < 128.## Schéma d'E/S public
| Champ | Octets | Encodage | Remarques |
|-----------------|-------|----------------------------------------|------------------------------------|
| `dsid` | 16 | UUID petit-boutiste | ID d'espace de données pour le couloir d'entrée (global pour le couloir par défaut), haché avec la balise `fastpq:v1:dsid`. |
| `slot` | 8 | petit-endien u64 | Nanosecondes depuis l'époque.            |
| `old_root` | 32 | octets du champ Little-endian Poséidon2 | Racine SMT avant le lot.              |
| `new_root` | 32 | octets du champ Little-endian Poséidon2 | Racine SMT après lot.               |
| `perm_root` | 32 | octets du champ Little-endian Poséidon2 | Racine de la table d'autorisations pour l'emplacement. |
| `tx_set_hash` | 32 | BLAKE2b | Identificateurs d'instructions triés.     |
| `parameter` | var | UTF-8 (par exemple, `fastpq-lane-balanced`) | Nom du jeu de paramètres.                 |
| `protocol_version`, `params_version` | 2 chacun | petit-boutiste u16 | Valeurs de version.                      |
| `ordering_hash` | 32 | Poséidon2 (petit-boutiste) | Hachage stable des lignes triées.         |

La suppression est codée par des membres de valeur nulle ; les clés absentes utilisent zéro feuille + témoin voisin.

`FastpqTransitionBatch.public_inputs` est le support canonique pour `dsid`, `slot` et les engagements racine ;
les métadonnées par lots sont réservées à la comptabilité du nombre de hachages/transcriptions d'entrée.

## Encodage des hachages
- Hachage de commande : Poséidon2 (tag `fastpq:v1:ordering`).
- Hachage d'artefact par lots : BLAKE2b sur `PublicIO || proof.commitments` (tag `fastpq:v1:artifact`).

## Définitions d'étape de Terminé (DoD)
- **Étape 1 du DoD**
  - Fusion des tests aller-retour et des montages d'emballage.
  - La spécification AIR (`docs/source/fastpq_air.md`) inclut `s_active`, les colonnes d'actifs/SMT, les définitions de sélecteur (y compris `s_perm`) et les contraintes symboliques.
  - Commande de hachage enregistrée dans PublicIO et vérifiée via les appareils.
  - Génération de témoins SMT/recherche implémentée avec des vecteurs d'adhésion et de non-adhésion.
  - Les tests de conservation couvrent les lots de transfert, de menthe, de brûlure et mixtes.
- **Étape 2 du DoD**
  - Spécification de transcription mise en œuvre ; transcription dorée (`tests/fixtures/transcript_v1.json`) et balises de domaine vérifiées.
  - Validation du paramètre Poséidon2 `3f2b7fe` épinglé dans le prouveur et le vérificateur avec des tests d'endianité sur toutes les architectures.
  - Garde CI de solidité active ; taille de preuve/RAM/latence SLO enregistrés.
- **Étape 3 du DoD**
  - API Scheduler (`SubmitProofRequest`, `ProofResult`) documentée avec des clés d'idempotence.
  - Preuve des artefacts stockés de manière adressable avec nouvelle tentative/interruption.
  - Télémétrie exportée pour la profondeur de la file d'attente, le temps d'attente de la file d'attente, la latence d'exécution du prouveur, le nombre de tentatives, le nombre d'échecs du backend et l'utilisation du GPU/CPU, avec des tableaux de bord et des seuils d'alerte pour chaque métrique.## Étape 5 — Accélération et optimisation du GPU
- Noyaux cibles : LDE (NTT), hachage Poséidon2, construction d'arbre Merkle, pliage FRI.
- Déterminisme : désactivez les mathématiques rapides, garantissez des sorties identiques en bits sur CPU, CUDA et Metal. CI doit comparer les racines de preuve sur tous les appareils.
- Suite de référence comparant CPU et GPU sur du matériel de référence (par exemple, Nvidia A100, AMD MI210).
- Backend métallique (Apple Silicon) :
  - Le script de construction compile la suite du noyau (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) en `fastpq.metallib` via `xcrun metal`/`xcrun metallib` ; assurez-vous que les outils de développement macOS incluent la chaîne d'outils Metal (`xcode-select --install`, puis `xcodebuild -downloadComponent MetalToolchain` si nécessaire).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - Reconstruction manuelle (miroirs `build.rs`) pour échauffements CI ou packaging déterministe :
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Les builds réussis émettent `FASTPQ_METAL_LIB=<path>` afin que le runtime puisse charger le metallib de manière déterministe.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - Le noyau LDE suppose désormais que le tampon d'évaluation est initialisé à zéro sur l'hôte. Conservez le chemin d'allocation `vec![0; ..]` existant ou mettez explicitement à zéro les tampons lorsque vous les réutilisez.
  - La multiplication des cosets est fusionnée dans l'étape FFT finale pour éviter une passe supplémentaire ; toute modification apportée au staging LDE doit préserver cet invariant.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - Le noyau FFT/LDE à mémoire partagée s'arrête désormais à la profondeur de la tuile et transmet les papillons restants ainsi que toute mise à l'échelle inverse à une passe `fastpq_fft_post_tiling` dédiée. L'hôte Rust transmet les mêmes lots de colonnes à travers les deux noyaux et ne lance la distribution post-tuile que lorsque `log_len` dépasse la limite de tuiles, de sorte que la télémétrie de la profondeur de file d'attente, les statistiques du noyau et le comportement de secours restent déterministes pendant que le GPU gère entièrement le travail à grande échelle. sur l'appareil.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - Pour expérimenter les formes de lancement, définissez `FASTPQ_METAL_THREADGROUP=<width>` ; le chemin de répartition limite la valeur à la limite du périphérique et enregistre le remplacement afin que les exécutions de profilage puissent balayer les tailles des groupes de threads sans recompilation. 【crates/fastpq_prover/src/metal.rs:321】- Ajustez directement la tuile FFT : l'hôte dérive désormais les voies du groupe de threads (16 pour les traces courtes, 32 une fois `log_len ≥ 6`, 64 une fois `log_len ≥ 10`, 128 une fois `log_len ≥ 14` et 256 pour `log_len ≥ 18`) et la profondeur des tuiles (5 étapes pour les petites traces, 4 lorsque `log_len ≥ 12`, et une fois que le domaine atteint `log_len ≥ 18/20/22`, le passage en mémoire partagée exécute désormais les étapes 14/12/16 avant de confier le contrôle au noyau post-tile) à partir du domaine demandé plus la largeur d'exécution/threads maximum du périphérique. Remplacez par `FASTPQ_METAL_FFT_LANES` (puissance de deux entre 8 et 256) et `FASTPQ_METAL_FFT_TILE_STAGES` (1 à 16) pour épingler des formes de lancement spécifiques ; les deux valeurs transitent par `FftArgs`, sont fixées à la fenêtre prise en charge et sont enregistrées pour les balayages de profilage.
- Le traitement par lots des colonnes FFT/IFFT et LDE dérive désormais de la largeur du groupe de threads résolu : l'hôte cible environ 4096 threads logiques par tampon de commande, fusionne jusqu'à 64 colonnes à la fois avec le transfert de tuiles de tampon circulaire, et ne descend que sur 64 → 32 → 16 → 8 → 4 → 2 → 1 colonnes lorsque le domaine d'évaluation traverse le Seuils 2¹⁶/2¹⁸/2²⁰/2²². Cela maintient la capture de 20 000 lignes à ≥ 64 colonnes par expédition tout en garantissant que les cosets longs se terminent toujours de manière déterministe. Le planificateur adaptatif double toujours la largeur des colonnes jusqu'à ce que les répartitions approchent l'objectif de ≈2 ms et divise désormais automatiquement le lot par deux chaque fois qu'une répartition échantillonnée atteint ≥30 % au-dessus de cet objectif, de sorte que les transitions voie/carreau qui gonflent le coût par colonne retombent sans remplacement manuel. Les permutations Poséidon partagent le même planificateur adaptatif et le bloc `metal_heuristics.batch_columns.poseidon` dans `fastpq_metal_bench` enregistre désormais le nombre d'états résolus, le plafond, la dernière durée et l'indicateur de remplacement afin que la télémétrie de la profondeur de la file d'attente puisse être directement liée au réglage de Poséidon. Remplacez par `FASTPQ_METAL_FFT_COLUMNS` (1 à 64) pour épingler une taille de lot FFT déterministe et utilisez `FASTPQ_METAL_LDE_COLUMNS` (1 à 64) lorsque vous avez besoin que le répartiteur LDE respecte un nombre de colonnes fixe ; le banc métallique fait apparaître les entrées `kernel_profiles.*.columns` résolues dans chaque capture afin que les expériences de réglage restent reproductibles.- La répartition multi-files d'attente est désormais automatique sur les Mac discrets : l'hôte inspecte `is_low_power`, `is_headless` et l'emplacement du périphérique pour décider s'il faut lancer deux files d'attente de commandes Metal, ne se déploie que lorsque la charge de travail comporte au moins 16 colonnes (mise à l'échelle en fonction de la sortance résolue), et effectue un tourniquet entre les lots de colonnes afin que les longues traces occupent les deux voies GPU sans sacrifier le déterminisme. Le sémaphore du tampon de commande impose désormais un seuil de « deux vols par file d'attente », et la télémétrie de file d'attente enregistre la fenêtre de mesure globale (`window_ms`) ainsi que les taux d'occupation normalisés (`busy_ratio`) pour le sémaphore global et chaque entrée de file d'attente afin que les artefacts de libération puissent prouver que les deux files d'attente sont restées occupées à ≥ 50 % pendant la même période. Remplacez les valeurs par défaut par `FASTPQ_METAL_QUEUE_FANOUT` (1 à 4 voies) et `FASTPQ_METAL_COLUMN_THRESHOLD` (nombre total minimum de colonnes avant répartition) ; les tests de parité Metal forcent les remplacements afin que les Mac multi-GPU restent couverts, et la politique résolue est enregistrée avec la télémétrie de profondeur de file d'attente et le nouveau `metal_dispatch_queue.queues[*]` bloc.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- La détection de métaux sonde désormais directement `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` (réchauffement de CoreGraphics sur des coques sans tête) avant de revenir à `system_profiler`, et `FASTPQ_DEBUG_METAL_ENUM` imprime les périphériques énumérés lorsqu'ils sont définis afin que les exécutions de CI sans tête puissent expliquer pourquoi `FASTPQ_GPU=gpu` est toujours rétrogradé vers le CPU. chemin. Lorsque le remplacement est défini sur `gpu` mais qu'aucun accélérateur n'est détecté, `fastpq_metal_bench` génère désormais une erreur immédiatement avec un pointeur vers le bouton de débogage au lieu de continuer silencieusement sur le processeur. Cela réduit la classe de « secours silencieux du processeur » évoquée dans WP2‑E et donne aux opérateurs un bouton pour capturer les journaux d'énumération dans des tests de référence encapsulés.
  - Les timings du GPU Poséidon refusent désormais de traiter les replis du CPU comme des données « GPU ». `hash_columns_gpu` indique si l'accélérateur a réellement fonctionné, `measure_poseidon_gpu` supprime des échantillons (et enregistre un avertissement) chaque fois que le pipeline recule, et l'enfant du microbench Poséidon se termine avec une erreur si le hachage GPU n'est pas disponible. Par conséquent, `gpu_recorded=false`, chaque fois que l'exécution de Metal recule, le résumé de la file d'attente enregistre toujours la fenêtre de répartition ayant échoué et les résumés du tableau de bord signalent immédiatement la régression. Le wrapper (`scripts/fastpq/wrap_benchmark.py`) échoue désormais lorsque `metal_dispatch_queue.poseidon.dispatch_count == 0`, de sorte que les bundles Stage7 ne peuvent pas être signés sans une véritable expédition GPU Poseidon. preuves.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- Le hachage Poséidon reflète désormais ce contrat de mise en scène. `PoseidonColumnBatch` produit des tampons de charge utile aplatis ainsi que des descripteurs de décalage/longueur, l'hôte rebase ces descripteurs par lot et exécute un double tampon `COLUMN_STAGING_PIPE_DEPTH` afin que les téléchargements de charge utile + descripteur se chevauchent avec le travail GPU, et les deux noyaux Metal/CUDA consomment directement les descripteurs afin que chaque répartition absorbe tous les blocs de taux rembourrés sur l'appareil avant d'émettre les résumés de colonne. `hash_columns_from_coefficients` diffuse désormais ces lots via un thread de travail GPU, gardant plus de 64 colonnes en vol par défaut sur des GPU discrets (réglables via `FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH`). Le banc Metal enregistre les paramètres de pipeline résolus + le nombre de lots sous `metal_dispatch_queue.poseidon_pipeline`, et `kernel_profiles.poseidon.bytes` inclut le trafic du descripteur afin que les captures Stage7 prouvent le nouvel ABI. de bout en bout.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_cuda.cu:351】
- Le hachage Poséidon fusionné Stage7-P2 arrive désormais dans les deux backends GPU. Le flux de travail de streaming alimente les tranches `PoseidonColumnBatch::column_window()` contiguës dans `hash_columns_gpu_fused`, qui les redirige vers `poseidon_hash_columns_fused` afin que chaque répartition écrive `leaf_digests || parent_digests` avec le mappage parent canonique `(⌈columns / 2⌉)`. `ColumnDigests` conserve les deux tranches et `merkle_root_with_first_level` consomme la couche parent immédiatement, de sorte que le processeur ne recalcule jamais les nœuds de profondeur 1 et que la télémétrie Stage7 peut affirmer que les captures GPU ne signalent aucun parent « de repli » à chaque fois que le noyau fusionné réussit.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` émet désormais un bloc `device_profile` avec le nom du périphérique Metal, l'ID de registre, les indicateurs `low_power`/`headless`, l'emplacement (intégré, emplacement, externe), l'indicateur discret, `hw.model` et l'étiquette Apple SoC dérivée (par exemple, « M3 Max »). Les tableaux de bord Stage7 utilisent ce champ pour regrouper les captures par M4/M3 par rapport aux GPU discrets sans analyser les noms d'hôtes, et le JSON est livré à côté de la file d'attente/preuve heuristique afin que chaque artefact de version prouve quelle classe de flotte a produit l'exécution.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】- Le chevauchement hôte/périphérique FFT utilise désormais une fenêtre de transfert à double tampon : tandis que le lot *n* se termine à l'intérieur de `fastpq_fft_post_tiling`, l'hôte aplatit le lot *n+1* dans le deuxième tampon de transfert et ne fait une pause que lorsqu'un tampon doit être recyclé. Le backend enregistre le nombre de lots aplatis ainsi que le temps passé à l'aplatissement par rapport à l'attente de la fin du GPU, et `fastpq_metal_bench` fait apparaître le bloc `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` agrégé afin que les artefacts de version puissent prouver le chevauchement au lieu de blocages silencieux de l'hôte. Le rapport JSON répartit désormais également les totaux par phase sous `column_staging.phases.{fft,lde,poseidon}`, permettant aux captures Stage7 de prouver si le transfert FFT/LDE/Poseidon est lié à l'hôte ou attend la fin du GPU. Les permutations Poséidon réutilisent les mêmes tampons de transfert regroupés, de sorte que les captures `--operation poseidon_hash_columns` émettent désormais les deltas `column_staging` spécifiques à Poséidon aux côtés des preuves de profondeur de file d'attente sans instrumentation sur mesure. Les nouvelles matrices `column_staging.samples.{fft,lde,poseidon}` enregistrent les tuples `batch/flatten_ms/wait_ms/wait_ratio` par lot, ce qui rend trivial la preuve que le chevauchement `COLUMN_STAGING_PIPE_DEPTH` est maintenu (ou pour repérer le moment où l'hôte commence à attendre le GPU). finitions).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fas tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- L'accélération Poséidon2 fonctionne désormais comme un noyau Metal à haute occupation : chaque groupe de threads copie les constantes de tour et les lignes MDS dans la mémoire du groupe de threads, déroule les tours complets/partiels et parcourt plusieurs états par voie afin que chaque envoi lance au moins 4096 threads logiques. Remplacez la forme de lancement via `FASTPQ_METAL_POSEIDON_LANES` (puissances de deux entre 32 et 256, fixées à la limite de l'appareil) et `FASTPQ_METAL_POSEIDON_BATCH` (1 à 32 états par voie) pour reproduire les expériences de profilage sans reconstruire `fastpq.metallib` ; l'hôte Rust transmet le réglage résolu via `PoseidonArgs` avant la distribution. L'hôte prend désormais un instantané de `MTLDevice::{is_low_power,is_headless,location}` une fois par démarrage et oriente automatiquement les GPU discrets vers les lancements à plusieurs niveaux de VRAM (`256×24` sur les pièces ≥ 48 Go, `256×20` à 32 Go, `256×16` sinon), tandis que les SoC à faible consommation s'en tiennent à `256×8` (les solutions de repli pour le matériel à 128/64 voies continuent d'utiliser 8/6 états par voie), afin que les opérateurs obtiennent une profondeur de pipeline > 16 états sans toucher aux variables d'environnement. `fastpq_metal_bench` se réexécute sous `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` pour capturer un bloc `poseidon_microbench` dédié comparant la voie scalaire au noyau multi-états afin que les artefacts de version puissent fournir une accélération concrète. La même chose capture la télémétrie de surface `poseidon_pipeline` (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`), de sorte que les preuves Stage7 prouvent la fenêtre de chevauchement sur chaque GPU. trace.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【cra tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - La mise en scène des tuiles LDE reflète désormais l'heuristique FFT : les traces lourdes n'exécutent que 12 étapes dans le passage en mémoire partagée une fois `log₂(len) ≥ 18`, descendent à 10 étapes à log₂20 et se limitent à huit étapes à log₂22 afin que les larges papillons se déplacent dans le noyau post-carrelage. Remplacez par `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) chaque fois que vous avez besoin d'une profondeur déterministe ; l'hôte ne lance la répartition post-panneautage que lorsque l'heuristique s'arrête plus tôt afin que la profondeur de la file d'attente et la télémétrie du noyau restent déterministes.【crates/fastpq_prover/src/metal.rs:827】
  - Micro-optimisation du noyau : les tuiles FFT/LDE à mémoire partagée réutilisent désormais les foulées de twiddle et de coset par voie au lieu de réévaluer `pow_mod*` pour chaque papillon. Chaque voie précalcule `w_seed`, `w_stride` et (si nécessaire) la foulée du coset une fois par bloc, puis diffuse les décalages, réduisant les multiplications scalaires à l'intérieur de `apply_stage_tile`/`apply_stage_global` et ramenant la moyenne LDE de 20 000 lignes à ~ 1,55 s avec la dernière heuristique (toujours au-dessus de l'objectif de 950 ms, mais une amélioration supplémentaire d'environ 50 ms par rapport au réglage par lots uniquement).- La suite du noyau dispose désormais d'une référence dédiée (`docs/source/fastpq_metal_kernels.md`) qui documente chaque point d'entrée, les limites de groupes de threads/tuiles appliquées dans `fastpq.metallib` et les étapes de reproduction pour compiler manuellement la metallib. 【docs/source/fastpq_metal_kernels.md:1】
  - Le rapport de référence émet désormais un objet `post_tile_dispatches` qui enregistre le nombre de lots FFT/IFFT/LDE exécutés dans le noyau post-tiling dédié (nombre de répartitions par type plus les limites étape/log₂). `scripts/fastpq/wrap_benchmark.py` copie le bloc dans `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`, et la porte manifeste refuse les captures GPU qui omettent les preuves, de sorte que chaque artefact de 20 000 lignes prouve que le noyau multi-passes a été exécuté. sur l'appareil.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Configurez `FASTPQ_METAL_TRACE=1` pour qu'il émette des journaux de débogage par expédition (étiquette du pipeline, largeur du groupe de threads, groupes de lancement, temps écoulé) pour la corrélation Instruments/Trace Metal.【crates/fastpq_prover/src/metal.rs:346】
- La file d'attente de répartition est désormais instrumentée : `FASTPQ_METAL_MAX_IN_FLIGHT` limite les tampons de commandes Metal simultanés (par défaut automatique dérivé du nombre de cœurs GPU détecté via `system_profiler`, limité au moins au niveau de déploiement de la file d'attente avec un repli du parallélisme de l'hôte lorsque macOS refuse de signaler le périphérique). Le banc permet un échantillonnage en profondeur de file d'attente afin que le JSON exporté transporte un objet `metal_dispatch_queue` avec les champs `limit`, `dispatch_count`, `max_in_flight`, `busy_ms` et `overlap_ms` pour les preuves de version, ajoute un élément imbriqué. Bloc `metal_dispatch_queue.poseidon` chaque fois qu'une capture Poséidon uniquement (`--operation poseidon_hash_columns`) s'exécute et émet un bloc `metal_heuristics` décrivant la limite résolue du tampon de commande ainsi que les colonnes de lots FFT/LDE (y compris si les remplacements ont forcé les valeurs) afin que les réviseurs puissent auditer les décisions de planification parallèlement à la télémétrie. Les noyaux Poséidon alimentent également un bloc `poseidon_profiles` dédié distillé à partir des échantillons du noyau afin que les octets/thread, l'occupation et la géométrie de répartition soient suivis à travers les artefacts. Si l'exécution principale ne peut pas collecter la profondeur de la file d'attente ou les statistiques de remplissage nul du LDE (par exemple, lorsqu'une répartition du GPU revient silencieusement au CPU), le harnais déclenche automatiquement une répartition de sonde unique pour recueillir la télémétrie manquante et synthétise désormais les timings de remplissage zéro de l'hôte lorsque le GPU refuse de les signaler, de sorte que les preuves publiées incluent toujours le `zero_fill`. block.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Définissez `FASTPQ_SKIP_GPU_BUILD=1` lors de la compilation croisée sans la chaîne d'outils Metal ; l'avertissement enregistre le saut et le planificateur continue sur le chemin du processeur.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- La détection d'exécution utilise `system_profiler` pour confirmer la prise en charge de Metal ; si le framework, le périphérique ou metallib est manquant, le script de construction efface `FASTPQ_METAL_LIB` et le planificateur reste sur le processeur déterministe chemin.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - Liste de contrôle de l'opérateur (hôtes Metal) :
    1. Confirmez que la chaîne d'outils est présente et que `FASTPQ_METAL_LIB` pointe vers un `.metallib` compilé (`echo $FASTPQ_METAL_LIB` ne doit pas être vide après `cargo build --features fastpq-gpu`).【crates/fastpq_prover/build.rs:188】
    2. Exécutez des tests de parité avec les voies GPU activées : `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. Cela exerce les noyaux Metal et revient automatiquement si la détection échoue.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Capturez un échantillon de référence pour les tableaux de bord : localisez la bibliothèque Metal compilée
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), exportez-le via
       `FASTPQ_METAL_LIB` et exécutez
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       L'ensemble canonique `fastpq-lane-balanced` complète désormais chaque capture à 32 768 lignes, de sorte que le
       JSON reflète à la fois les 20 000 lignes demandées et le domaine complété qui pilote le GPU
       noyaux. Téléchargez le JSON/log dans votre magasin de preuves ; les miroirs de flux de travail macOS nocturnes
      cette course et archive les artefacts pour référence. Le rapport enregistre
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` aux côtés du `speedup` de chaque opération, le
     La section LDE ajoute `zero_fill.{bytes,ms,queue_delta}` afin que les artefacts de version prouvent le déterminisme,
     la surcharge de remplissage zéro de l'hôte et l'utilisation incrémentielle de la file d'attente GPU (limite, nombre de répartitions,
     pic en vol, temps d'occupation/chevauchement), et le nouveau bloc `kernel_profiles` capture par noyau
     taux d'occupation, bande passante estimée et plages de durée afin que les tableaux de bord puissent signaler le GPU
       régressions sans retraitement des échantillons bruts.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Attendez-vous à ce que le chemin Metal LDE reste inférieur à 950 ms (cible `<1 s` sur le matériel Apple série M) ;
4. Capturez la télémétrie d'utilisation des lignes à partir d'un véritable ExecWitness afin que les tableaux de bord puissent tracer le gadget de transfert
   adoption. Récupérez un témoin auprès de Torii
  (`iroha_cli audit witness --binary --out exec.witness`) et décodez-le avec
  `iroha_cli audit witness --decode exec.witness` (ajouter éventuellement
  `--fastpq-parameter fastpq-lane-balanced` pour affirmer le jeu de paramètres attendu ; Lots FASTPQ
  émettre par défaut ; passez `--no-fastpq-batches` uniquement si vous devez découper la sortie).
   Chaque entrée de lot émet désormais un objet `row_usage` (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, nombres par sélecteur et `transfer_ratio`). Archiver cet extrait JSON
   retraitement des transcriptions brutes.【crates/iroha_cli/src/audit.rs:209】 Comparez la nouvelle capture avec
   la ligne de base précédente avec `scripts/fastpq/check_row_usage.py` donc CI échoue si les ratios de transfert ou
   régression totale des lignes :

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```Exemples de blobs JSON pour les tests de fumée en direct dans `scripts/fastpq/examples/`. Localement, vous pouvez exécuter `make check-fastpq-row-usage`
   (encapsule `ci/check_fastpq_row_usage.sh`), et CI exécute le même script via `.github/workflows/fastpq-row-usage.yml` pour comparer le commit
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` instantanés pour que l'ensemble de preuves échoue rapidement à chaque fois
   les rangées de transfert remontent. Transmettez `--summary-out <path>` si vous souhaitez une différence lisible par machine (le travail CI télécharge `fastpq_row_usage_summary.json`).
   Lorsqu'un ExecWitness n'est pas à portée de main, synthétisez un échantillon de régression avec `fastpq_row_bench`
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), qui émet exactement le même `row_usage`
   objet pour le nombre de sélecteurs configurables (par exemple, un test de contrainte de 65 536 lignes) :

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Les bundles de déploiement Stage7-3 doivent également satisfaire à la norme `scripts/fastpq/validate_row_usage_snapshot.py`, qui
   impose que chaque entrée `row_usage` contienne le nombre de sélecteurs et que
   `transfer_ratio = transfer_rows / total_rows` ; `ci/check_fastpq_rollout.sh` appelle l'assistant
   automatiquement, de sorte que les bundles manquant de ces invariants échouent avant que les voies GPU ne soient obligatoires.
       la porte du manifeste de banc applique cela via `--max-operation-ms lde=950`, alors actualisez le
       capturez chaque fois que vos preuves dépassent cette limite.
      Lorsque vous avez également besoin de preuves d'instruments, transmettez `--trace-dir <dir>` afin que le harnais
      se relance via `xcrun xctrace record` (modèle par défaut « Metal System Trace ») et
      stocke un fichier `.trace` horodaté à côté du JSON ; vous pouvez toujours remplacer l'emplacement /
      modèle manuellement avec `--trace-output <path>` plus `--trace-template` en option /
      `--trace-seconds`. Le JSON résultant annonce `metal_trace_{template,seconds,output}` donc
      les lots d'artefacts identifient toujours la trace capturée.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Enveloppez chaque capture avec
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (ajoutez `--gpg-key <fingerprint>` si vous devez épingler une identité de signature) pour que le bundle échoue
       rapide chaque fois que la moyenne GPU LDE dépasse l'objectif de 950 ms, que Poséidon dépasse 1 seconde ou que le
       Il manque des blocs de télémétrie Poséidon, embarque un `row_usage_snapshot`
      à côté du JSON, fait apparaître le résumé du microbench Poséidon sous `benchmarks.poseidon_microbench`,
      et contient toujours des métadonnées pour les runbooks et le tableau de bord Grafana
    (`dashboards/grafana/fastpq_acceleration.json`). Le JSON émet désormais `speedup.ratio` /
     `speedup.delta_ms` par opération afin que les preuves de publication puissent prouver que GPU vs
     Le CPU gagne sans retraiter les échantillons bruts, et le wrapper copie à la fois les
     statistiques de remplissage à zéro (plus `queue_delta`) dans `zero_fill_hotspots` (octets, latence, dérivées
     GB/s), enregistre les métadonnées des instruments sous `metadata.metal_trace`, enfile le paramètre facultatif
     `metadata.row_usage_snapshot` lorsque `--row-usage <decoded witness>` est fourni, et aplatit le
     compteurs par noyau dans `benchmarks.kernel_summary` afin de combler les goulots d'étranglement, file d'attente métallique
     l'utilisation, l'occupation du noyau et les régressions de bande passante sont visibles d'un coup d'œil sans
     spéléologie du rapport brut.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Étant donné que l'instantané d'utilisation des lignes voyage désormais avec l'artefact encapsulé, les tickets de déploiement se contentent simplement de
     référencer le bundle au lieu de joindre un deuxième extrait de code JSON, et CI peut différencier le package intégré
    compte directement lors de la validation des soumissions Stage7. Pour archiver les données du microbench seul,
    exécutez `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    et stockez le fichier résultant sous `benchmarks/poseidon/`. Gardez le manifeste agrégé à jour avec
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    afin que les tableaux de bord/CI puissent comparer l'historique complet sans parcourir chaque fichier manuellement.4. Validez la télémétrie en recourbant `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (point de terminaison Prometheus) ou en recherchant les journaux `telemetry::fastpq.execution_mode` ; Des entrées `resolved="cpu"` inattendues indiquent que l'hôte s'est replié malgré l'intention du GPU.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Utilisez `FASTPQ_GPU=cpu` (ou le bouton de configuration) pour forcer l'exécution du processeur pendant la maintenance et confirmer que les journaux de secours apparaissent toujours ; cela maintient les runbooks SRE alignés sur le chemin déterministe.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Télémétrie et repli :
  - Les journaux du mode d'exécution (`telemetry::fastpq.execution_mode`) et les compteurs (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) exposent le mode demandé ou résolu afin que les solutions de secours silencieuses soient visibles dans les tableaux de bord. 【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - La carte `FASTPQ Acceleration Overview` Grafana (`dashboards/grafana/fastpq_acceleration.json`) visualise le taux d'adoption de Metal et renvoie aux artefacts de référence, tandis que les règles d'alerte appariées (`dashboards/alerts/fastpq_acceleration_rules.yml`) sont déployées en cas de déclassements soutenus.
  - Les remplacements `FASTPQ_GPU={auto,cpu,gpu}` restent pris en charge ; les valeurs inconnues génèrent des avertissements mais se propagent toujours à la télémétrie pour l'audit.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - Les tests de parité GPU (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) doivent réussir pour CUDA et Metal ; CI saute gracieusement lorsque le metallib est absent ou que la détection échoue.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Preuve de préparation au métal (archivez les artefacts ci-dessous à chaque déploiement afin que l'audit de la feuille de route puisse prouver le déterminisme, la couverture télémétrique et le comportement de repli) :| Étape | Objectif | Commandement / Preuve |
    | ---- | ---- | ------------------ |
    | Construire Metallib | Assurez-vous que `xcrun metal`/`xcrun metallib` sont disponibles et émettez le déterministe `.metallib` pour ce commit | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` ; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` ; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` ; exporter `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | Vérifier la variable d'environnement | Confirmez que Metal reste activé en vérifiant la variable d'environnement enregistrée par le script de construction | `echo $FASTPQ_METAL_LIB` (doit renvoyer un chemin absolu ; vide signifie que le backend a été désactivé).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | Suite de parité GPU | Prouver que les noyaux s'exécutent (ou émettent des journaux de rétrogradation déterministes) avant l'expédition | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` et stockez l'extrait de journal résultant qui affiche soit `backend="metal"`, soit l'avertissement de repli. 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | Échantillon de référence | Capturez la paire JSON/log qui enregistre les réglages `speedup.*` et FFT afin que les tableaux de bord puissent ingérer les preuves de l'accélérateur | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces` ; archivez le JSON, le `.trace` horodaté et la sortie standard avec les notes de version afin que la carte Grafana récupère l'exécution Metal (le rapport enregistre les 20 000 lignes demandées plus le domaine complété de 32 768 lignes afin que les réviseurs puissent confirmer le LDE `<1 s`. cible).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Envelopper et signer le rapport | Échouez la version si la moyenne GPU LDE dépasse 950 ms, si Poséidon dépasse 1 seconde ou si des blocs de télémétrie Poséidon sont manquants et produisez un ensemble d'artefacts signé | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` ; expédier à la fois le JSON encapsulé et la signature `.json.asc` générée afin que les auditeurs puissent vérifier les métriques en moins d'une seconde sans réexécuter la charge de travail.
    | Manifeste de banc signé | Appliquez les preuves LDE `<1 s` dans les bundles Metal/CUDA et capturez les résumés signés pour l'approbation de la publication | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` ; attachez le manifeste + la signature au ticket de version afin que l'automatisation en aval puisse valider les métriques de preuve en moins d'une seconde. 【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| Offre groupée CUDA | Gardez la capture SM80 CUDA en phase avec les preuves Metal afin que les manifestes couvrent les deux classes de GPU. | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` sur l'hôte Xeon+RTX → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output` ; ajoutez le chemin encapsulé à `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, conservez la paire `.json`/`.asc` à côté du bundle Metal et citez le `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` prédéfini lorsque les auditeurs ont besoin d'une référence. mise en page.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Vérification de télémétrie | Validez les surfaces Prometheus/OTEL reflètent `device_class="<matrix>", backend="metal"` (ou enregistrez le déclassement) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` et copiez le journal `telemetry::fastpq.execution_mode` émis au démarrage.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| Exercice de repli forcé | Documenter le chemin déterministe du processeur pour les playbooks SRE | Exécutez une courte charge de travail avec `FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"` et capturez le journal de rétrogradation afin que les opérateurs puissent répéter la procédure de restauration.
    | Capture de trace (facultatif) | Lors du profilage, capturez les traces de répartition afin que les remplacements de voies/tuiles du noyau soient révisables ultérieurement | Réexécutez un test de parité avec `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` et joignez le journal de trace produit à vos artefacts de version.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Archivez les preuves avec le ticket de publication et reproduisez la même liste de contrôle dans `docs/source/fastpq_migration_guide.md` afin que les déploiements de préparation et de production suivent un playbook identique. 【docs/source/fastpq_migration_guide.md:1】

### Application de la liste de contrôle de publication

Ajoutez les portes suivantes à chaque ticket de version FASTPQ. Les versions sont bloquées jusqu'à ce que tous les éléments soient
complet et joint en tant qu'objets signés.

1. **Mesures de preuve inférieures à la seconde** — La capture canonique de référence Metal
   (`fastpq_metal_bench_*.json`) doit prouver que la charge de travail de 20 000 lignes (32 768 lignes remplies) se termine dans
   <1s. Concrètement, l'entrée `benchmarks.operations` où `operation = "lde"` et l'entrée correspondante
   L'échantillon `report.operations` doit afficher `gpu_mean_ms ≤ 950`. Les courses qui dépassent le plafond nécessitent
   enquête et une recapture avant que la liste de contrôle puisse être signée.
2. **Manifeste de référence signé** — Après avoir enregistré de nouveaux bundles Metal + CUDA, exécutez
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` à émettre
   `artifacts/fastpq_bench_manifest.json` et la signature détachée
   (`artifacts/fastpq_bench_manifest.sig`). Joignez les deux fichiers ainsi que l'empreinte digitale de la clé publique au
   publier le ticket afin que les réviseurs puissent vérifier le résumé et la signature indépendamment.【xtask/src/fastpq.rs:1】
3. **Pièces jointes de preuves** — Stockez le JSON de référence brut, le journal stdout (ou la trace Instruments, lorsque
   capturé) et la paire manifeste/signature avec le ticket de version. La liste de contrôle est uniquement
   considéré comme vert lorsque le ticket renvoie à ces artefacts et que l'examinateur de garde confirme le
   Le résumé enregistré dans `fastpq_bench_manifest.json` correspond aux fichiers téléchargés. 【artifacts/fastpq_benchmarks/README.md:1】

## Étape 6 — Durcissement et documentation
- Backend d'espace réservé retiré ; le pipeline de production est livré par défaut sans basculement de fonctionnalités.
- Constructions reproductibles (chaînes d'outils d'épingles, images de conteneurs).
- Fuzzers pour les structures de trace, SMT et de recherche.
- Les tests de fumée de niveau Prover couvrent les subventions de vote de gouvernance et les transferts de fonds pour maintenir la stabilité des appareils Stage6 avant le déploiement complet de l'IVM. 【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Runbooks avec seuils d'alerte, procédures de remédiation, directives de planification des capacités.
- Relecture de preuves multi-architectures (x86_64, ARM64) en CI.

### Manifeste de banc et porte de déverrouillage

Les preuves de libération incluent désormais un manifeste déterministe couvrant à la fois le métal et
Offres groupées de référence CUDA. Exécutez :

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```La commande valide les bundles encapsulés, applique les seuils de latence/accélération,
émet des résumés BLAKE3 + SHA-256 et (éventuellement) signe le manifeste avec un
Clé Ed25519 pour que les outils de publication puissent vérifier la provenance. Voir
`xtask/src/fastpq.rs`/`xtask/src/main.rs` pour la mise en œuvre et
`artifacts/fastpq_benchmarks/README.md` pour des conseils opérationnels.

> **Remarque :** Les faisceaux métalliques qui omettent `benchmarks.poseidon_microbench` provoquent désormais
> la génération manifeste à l'échec. Réexécutez `scripts/fastpq/wrap_benchmark.py`
> (et `scripts/fastpq/export_poseidon_microbench.py` si vous avez besoin d'un
> résumé) chaque fois que la preuve de Poséidon manque, la libération se manifeste
> capturez toujours la comparaison scalaire par rapport à la valeur par défaut.【xtask/src/fastpq.rs:409】

L'indicateur `--matrix` (par défaut `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
lorsqu'il est présent) charge les médianes multi-appareils capturées par
`scripts/fastpq/capture_matrix.sh`. Le manifeste code l'étage de 20 000 lignes et
limites de latence/accélération par opération pour chaque classe d'appareils, donc sur mesure
Les remplacements `--require-rows`/`--max-operation-ms`/`--min-operation-speedup` ne sont pas
n'est plus nécessaire, sauf si vous déboguez une régression spécifique.

Actualisez la matrice en ajoutant les chemins de référence encapsulés au
Listes `artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` et en cours d'exécution
`scripts/fastpq/capture_matrix.sh`. Le script capture les médianes par appareil,
émet le `matrix_manifest.json` consolidé et imprime le chemin relatif qui
`cargo xtask fastpq-bench-manifest` consommera. Les AppleM4, Xeon+RTX et
Listes de capture Neoverse+MI300 (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) plus leur emballage
offres groupées de référence
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) sont désormais vérifiés
dans, de sorte que chaque version applique les mêmes médianes multi-appareils avant le manifeste
est signé.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neovere-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artefacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Résumé des critiques et actions ouvertes

## Étape 7 — Preuves d'adoption et de déploiement de la flotte

Stage7 fait passer le prouveur de « documenté et comparé » (Stage6) à
« prêt par défaut pour les flottes de production ». L'accent est mis sur l'ingestion de télémétrie,
parité de capture multi-appareils et ensembles de preuves de l'opérateur pour une accélération GPU
peut être imposé de manière déterministe.- **Stage7-1 — Ingestion de télémétrie de flotte et SLO.** Tableaux de bord de production
  (`dashboards/grafana/fastpq_acceleration.json`) doit être câblé pour fonctionner
  Flux Prometheus/OTel avec couverture Alertmanager pour les blocages en profondeur de file d'attente,
  régressions sans remplissage et replis silencieux du processeur. Le pack d'alerte reste sous
  `dashboards/alerts/fastpq_acceleration_rules.yml` et alimente les mêmes preuves
  bundle requis dans Stage6.【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  Le tableau de bord expose désormais les variables de modèle pour `device_class`, `chip_family`,
  et `gpu_kind`, permettant aux opérateurs de faire pivoter l'adoption du métal selon la matrice exacte
  étiquette (par exemple, `apple-m4-max`), par famille de puces Apple ou par discrète ou discrète.
  classes GPU intégrées sans modifier les requêtes.
  Les nœuds macOS construits avec `irohad --features fastpq-gpu` émettent désormais
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (taux d'occupation/chevauchement), et
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (limit, max_in_flight, dispatch_count, window_seconds) donc les tableaux de bord et
  Les règles d'Alertmanager peuvent lire le rapport cyclique/la marge de sécurité du sémaphore métallique directement à partir de
  Prometheus sans attendre un bundle de référence. Les hôtes exportent désormais
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` et
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` à chaque fois
  l'assistant LDE remet à zéro les tampons d'évaluation du GPU et Alertmanager a obtenu le
  `FastpqQueueHeadroomLow` (hauteur  0,40 ms sur 15 m), donc marge de file d'attente et
  remplissez immédiatement les opérateurs de page de régression à zéro au lieu d'attendre le
  prochain benchmark enveloppé. Une nouvelle alerte au niveau de la page `FastpqCpuFallbackBurst` suit
  Les requêtes GPU qui atterrissent sur le backend CPU pour plus de 5 % de la charge de travail,
  obliger les opérateurs à capturer des preuves et à identifier les causes profondes des pannes temporaires du GPU
  avant de réessayer le déploiement.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  L'ensemble SLO applique désormais également l'objectif de cycle de service ≥ 50 % pour le métal via le
  Règle `FastpqQueueDutyCycleDrop`, qui fait la moyenne
  `fastpq_metal_queue_ratio{metric="busy"}` sur une fenêtre glissante de 15 minutes et
  avertit chaque fois que le travail du GPU est toujours en cours de planification mais qu'une file d'attente ne parvient pas à conserver le
  occupation requise. Cela maintient le contrat de télémétrie en direct aligné sur le
  des preuves de référence avant que les voies GPU ne soient obligatoires.
- **Stage7-2 — Matrice de capture multi-appareils.** La nouvelle
  `scripts/fastpq/capture_matrix.sh` construit
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` à partir de chaque appareil
  capturer les listes sous `artifacts/fastpq_benchmarks/matrix/devices/`. AppleM4,
  Les médianes Xeon+RTX et Neoverse+MI300 vivent désormais en pension aux côtés de leurs
  bundles encapsulés, donc `cargo xtask fastpq-bench-manifest` charge le manifeste
  automatiquement, applique le plancher de 20 000 lignes et s'applique par appareil
  limites de latence/accélération sans indicateurs CLI sur mesure avant qu'un ensemble de versions ne soitapprouvé.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
Les raisons d'instabilité agrégées sont désormais accompagnées de la matrice : réussite
`--reason-summary-out` à `scripts/fastpq/geometry_matrix.py` pour émettre un
Histogramme JSON des causes d'échec/d'avertissement, saisi par étiquette d'hôte et source
résumé, afin que les réviseurs de Stage7-2 puissent voir les pannes de processeur ou la télémétrie manquante à
un coup d'œil sans parcourir le tableau Markdown complet. La même aide maintenant
accepte `--host-label chip_family:Chip` (répétez pour plusieurs clés) afin que le
Les sorties Markdown/JSON incluent des colonnes d'étiquettes d'hôte organisées au lieu d'être enterrées
ces métadonnées dans le résumé brut, ce qui rend trivial le filtrage des versions du système d'exploitation ou
Versions du pilote Metal lors de la compilation de l'ensemble de preuves Stage7-2.【scripts/fastpq/geometry_matrix.py:1】
Les balayages géométriques tamponnent également les champs ISO8601 `started_at`/`completed_at` dans le
les sorties résumé, CSV et Markdown afin que les ensembles de capture puissent prouver la fenêtre pour
chaque hôte lorsque les matrices Stage7-2 fusionnent plusieurs exécutions de laboratoire.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` assemble désormais la matrice géométrique avec
Instantanés `row_usage/*.json` dans un seul bundle Stage7 (`stage7_bundle.json`
+ `stage7_geometry.md`), validation des taux de transfert via
`validate_row_usage_snapshot.py` et résumés persistants d'hôte/env/raison/source
afin que les tickets de déploiement puissent attacher un artefact déterministe au lieu de jongler
tables par hôte.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Étape 7-3 — Preuves d'adoption par les opérateurs et exercices de restauration.** Le nouveau
  `docs/source/fastpq_rollout_playbook.md` décrit le lot d'artefacts
  (`fastpq_bench_manifest.json`, captures Metal/CUDA enveloppées, exportation Grafana,
  Instantané Alertmanager, journaux de restauration) qui doivent accompagner chaque ticket de déploiement
  plus la chronologie par étapes (pilote → rampe → par défaut) et les exercices de repli forcés.
  `ci/check_fastpq_rollout.sh` valide ces bundles afin que CI applique le Stage7
  porte avant que les versions n'avancent. Le pipeline de versions peut désormais extraire la même chose
  regroupe dans `artifacts/releases/<version>/fastpq_rollouts/…` via
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, assurant le
  les manifestes signés et les preuves de déploiement restent ensemble. Un bundle de référence vit
  sous `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` à conserver
  le workflow GitHub (`.github/workflows/fastpq-rollout.yml`) vert bien que réel
  les soumissions de déploiement sont examinées.

### Répartition de la file d'attente FFT Stage7`crates/fastpq_prover/src/metal.rs` instancie désormais un `QueuePolicy` qui
génère automatiquement plusieurs files d'attente de commandes Metal chaque fois que l'hôte signale un
GPU discret. Les GPU intégrés conservent le chemin de file d'attente unique
(`MIN_QUEUE_FANOUT = 1`), alors que les périphériques discrets utilisent par défaut deux files d'attente et uniquement
se déploie lorsqu'une charge de travail couvre au moins 16 colonnes. Les deux heuristiques peuvent être ajustées
via les nouveaux `FASTPQ_METAL_QUEUE_FANOUT` et `FASTPQ_METAL_COLUMN_THRESHOLD`
les variables d'environnement et les lots FFT/LDE du planificateur à tour de rôle à travers le
files d'attente actives avant d'émettre la répartition post-pavage appariée sur la même file d'attente
pour préserver les garanties de commande.
Les opérateurs de nœuds n'ont plus besoin d'exporter ces variables d'environnement manuellement : le
Le profil `iroha_config` expose `fastpq.metal_queue_fanout` et
`fastpq.metal_queue_column_threshold` et `irohad` les applique via
`fastpq_prover::set_metal_queue_policy` avant que le backend Metal ne s'initialise ainsi
les profils de flotte restent reproductibles sans emballages de lancement sur mesure.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Les lots FFT inverses restent désormais dans une seule file d'attente chaque fois que la charge de travail vient juste d'être atteinte.
atteint le seuil de diffusion (par exemple, la capture équilibrée en voie à 16 colonnes), ce qui
restaure ≥1,0× la parité pour WP2-D tout en laissant FFT/LDE/Poséidon à grande colonne
répartit sur le chemin multi-files d'attente.【crates/fastpq_prover/src/metal.rs:2018】

Les tests d'assistance exercent les contraintes de politique de file d'attente et la validation de l'analyseur afin que CI puisse
prouver l'heuristique Stage7 sans nécessiter de matériel GPU sur chaque constructeur,
et les tests spécifiques au GPU forcent le remplacement de la diffusion pour conserver la couverture de relecture dans
synchroniser avec les nouveaux paramètres par défaut.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Étiquettes d'appareil et contrat d'alerte Stage7-1

`scripts/fastpq/wrap_benchmark.py` sonde désormais `system_profiler` sur la capture macOS
héberge et enregistre les étiquettes matérielles dans chaque référence encapsulée afin que la télémétrie de la flotte
et la matrice de capture peut pivoter par appareil sans feuilles de calcul sur mesure. Un
La capture de métal sur 20 000 lignes comporte désormais des entrées telles que :

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

Ces étiquettes sont ingérées avec `benchmarks.zero_fill_hotspots` et
`benchmarks.metal_dispatch_queue` donc l'instantané Grafana, matrice de capture
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) et Alertmanager
les preuves sont toutes d’accord sur la classe de matériel qui a produit les métriques. Le
L'indicateur `--label` permet toujours les remplacements manuels lorsqu'un hôte de laboratoire manque
`system_profiler`, mais les identifiants détectés automatiquement couvrent désormais AppleM1-M4 et
GPU PCIe discrets prêts à l'emploi.【scripts/fastpq/wrap_benchmark.py:1】

Les captures Linux reçoivent le même traitement : `wrap_benchmark.py` inspecte désormais
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` et `lspci` pour que CUDA et OpenCL fonctionnent
dériver `cpu_model`, `gpu_model` et un `device_class` canonique (`xeon-rtx-sm80`
pour l'hôte Stage7 CUDA, `neoverse-mi300` pour le laboratoire MI300A). Les opérateurs peuvent
remplacent toujours les valeurs détectées automatiquement, mais les ensembles de preuves Stage7 ne sont plus
nécessiter des modifications manuelles pour marquer les captures Xeon/Neoverse avec le bon appareil
métadonnées.Au moment de l'exécution, chaque hôte définit `fastpq.device_class`, `fastpq.chip_family` et
`fastpq.gpu_kind` (ou les variables d'environnement `FASTPQ_*` correspondantes) au
mêmes étiquettes de matrice qui apparaissent dans le bundle de capture donc exportation Prometheus
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` et
le tableau de bord d'accélération FASTPQ peut filtrer selon l'un des trois axes. Le
Les règles d'Alertmanager se regroupent sur le même ensemble d'étiquettes, permettant aux opérateurs de tracer
adoption, rétrogradations et solutions de repli par profil matériel au lieu d'un seul
ratio à l'échelle de la flotte.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

Le contrat de télémétrie SLO/alerte relie désormais les métriques capturées au Stage7.
portes. Le tableau ci-dessous résume les signaux et les points d’application :

| Signalisation | Source | Cible / Déclencheur | Application |
| ------ | ------ | ---------------- | ----------- |
| Taux d'adoption des GPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95 % des résolutions par (device_class, chip_family, gpu_kind) doivent atterrir sur `resolved="gpu", backend="metal"` ; page lorsqu'un triplet tombe en dessous de 50 % sur 15 mois | Alerte `FastpqMetalDowngrade` (page)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| Écart back-end | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Doit rester à 0 pour chaque triplet ; avertir après toute rafale soutenue (> 10 m) | Alerte `FastpqBackendNoneBurst` (avertissement)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| Taux de repli du processeur | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5 % des épreuves demandées par le GPU peuvent atterrir sur le backend du processeur pour n'importe quel triplet ; page lorsqu'un triplet dépasse 5% pendant ≥10m | Alerte `FastpqCpuFallbackBurst` (page)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Cycle de service de la file d'attente métallique | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | La moyenne mobile sur 15 mois doit rester ≥50 % chaque fois que les tâches GPU sont mises en file d'attente ; avertir lorsque l'utilisation tombe en dessous de l'objectif alors que les requêtes GPU persistent | Alerte `FastpqQueueDutyCycleDrop` (avertissement)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| Profondeur de la file d'attente et budget de remplissage nul | Blocs de référence enveloppés `metal_dispatch_queue` et `zero_fill_hotspots` | `max_in_flight` doit rester au moins un emplacement en dessous de `limit` et la moyenne de remplissage à zéro LDE doit rester ≤0,4 ms (≈80 Go/s) pour la trace canonique de 20 000 lignes ; toute régression bloque le bundle de déploiement | Examiné via la sortie `scripts/fastpq/wrap_benchmark.py` et joint à l'ensemble de preuves Stage7 (`docs/source/fastpq_rollout_playbook.md`). |
| Marge de file d'attente d'exécution | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` pour chaque triplet ; avertir après 10 m sans hauteur libre | Alerte `FastpqQueueHeadroomLow` (avertissement)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Latence de remplissage nulle à l'exécution | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | Le dernier échantillon à remplissage nul doit rester ≤0,40 ms (limite Stage7) | Alerte `FastpqZeroFillRegression` (page)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |Le wrapper applique directement la ligne à remplissage nul. Passer
`--require-zero-fill-max-ms 0.40` à `scripts/fastpq/wrap_benchmark.py` et il
échouera lorsque le banc JSON ne dispose pas de télémétrie de remplissage nul ou lorsque le plus chaud
L'échantillon de remplissage nul dépasse le budget Stage7, empêchant les offres groupées de déploiement de
expédition sans les preuves obligatoires.【scripts/fastpq/wrap_benchmark.py:1008】

#### Liste de contrôle de gestion des alertes Stage7-1

Chaque alerte répertoriée ci-dessus alimente un exercice d'astreinte spécifique afin que les opérateurs rassemblent les
mêmes artefacts que ceux requis par le bundle de version :

1. **`FastpqQueueHeadroomLow` (avertissement).** Exécutez une requête Prometheus instantanée
   pour `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` et
   capturer le panneau Grafana « Queue headroom » à partir du `fastpq-acceleration`
   planche. Enregistrez le résultat de la requête dans
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   avec l'ID d'alerte afin que le bundle de versions prouve que l'avertissement a été
   reconnu avant que la file d'attente ne soit affamée. 【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (page).** Inspecter
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` et, si la métrique est
   bruyant, réexécutez `scripts/fastpq/wrap_benchmark.py` sur le banc JSON le plus récent
   pour actualiser le bloc `zero_fill_hotspots`. Attachez la sortie promQL,
   captures d'écran et fichier de banc actualisé dans le répertoire de déploiement ; cela crée
   la même preuve que `ci/check_fastpq_rollout.sh` attend lors de la sortie
   validation.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (page).** Confirmez que
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` dépasse les 5%
   étage, puis échantillonnez les journaux `irohad` pour les messages de rétrogradation correspondants
   (`telemetry::fastpq.execution_mode resolved="cpu"`). Stocker le dump promQL
   ainsi que des extraits de journaux dans `metrics_cpu_fallback.prom`/`rollback_drill.log` afin que le
   L'offre groupée démontre à la fois l'impact et la reconnaissance de l'opérateur.
4. **Emballage des preuves.** Une fois toute alerte supprimée, réexécutez les étapes Stage7-3 dans
   le playbook de déploiement (exportation Grafana, instantané d'alerte, exercice de restauration) et
   revalidez le bundle via `ci/check_fastpq_rollout.sh` avant de le rattacher
   au ticket de sortie.【docs/source/fastpq_rollout_playbook.md:114】

Les opérateurs qui préfèrent l'automatisation peuvent exécuter
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
pour interroger l'API Prometheus pour connaître la marge de file d'attente, le remplissage à zéro et le repli du processeur
les mesures énumérées ci-dessus ; l'assistant écrit le JSON capturé (préfixé par le
promQL d'origine) dans `metrics_headroom.prom`, `metrics_zero_fill.prom` et
`metrics_cpu_fallback.prom` sous le répertoire de déploiement choisi afin que ces fichiers
peut être attaché au bundle sans invocations curl manuelles.`ci/check_fastpq_rollout.sh` applique désormais la marge de file d'attente et le remplissage à zéro
budgétiser directement. Il analyse chaque banc `metal` référencé par
`fastpq_bench_manifest.json`, inspecte
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` et
`benchmarks.zero_fill_hotspots[]` et échoue le bundle lorsque la marge diminue
en dessous d'un emplacement ou lorsqu'un point d'accès LDE signale `mean_ms > 0.40`. Cela maintient le
Garde de télémétrie Stage7 dans CI, correspondant à l'examen manuel effectué sur le
Instantané Grafana et preuves de publication.【ci/check_fastpq_rollout.sh#L1】
Dans le cadre de la même passe de validation, le script insiste désormais sur le fait que chaque
benchmark porte les étiquettes matérielles détectées automatiquement (`metadata.labels.device_class`
et `metadata.labels.gpu_kind`). Les lots manquant ces étiquettes échouent immédiatement,
garantissant que les artefacts de version, les manifestes de la matrice Stage7-2 et le temps d'exécution
les tableaux de bord font tous référence exactement aux mêmes noms de classe d’appareils.

Le panneau Grafana « Latest Benchmark » et l'ensemble de déploiement associé citent désormais le
`device_class`, budget de remplissage nul et instantané de la profondeur de la file d'attente pour des ingénieurs de garde
peut corréler la télémétrie de production avec la classe de capture exacte utilisée lors de la signature
éteint. Les futures entrées de matrice héritent des mêmes étiquettes, c'est-à-dire que le périphérique Stage7-2
les listes et les tableaux de bord Prometheus partagent un seul schéma de dénomination pour AppleM4,
M3 Max et prochaines captures MI300/RTX.

### Stage7-1 Runbook de télémétrie de flotte

Suivez cette liste de contrôle avant d'activer les voies GPU par défaut afin de télémétrie de la flotte
et les règles d'Alertmanager reflètent les mêmes preuves capturées lors de la préparation de la version :

1. **Capture d'étiquettes et hôtes d'exécution.** `python3 scripts/fastpq/wrap_benchmark.py`
   émet déjà `metadata.labels.device_class`, `chip_family` et `gpu_kind`
   pour chaque JSON encapsulé. Gardez ces étiquettes synchronisées avec
   `fastpq.{device_class,chip_family,gpu_kind}` (ou le
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` variables d'environnement) à l'intérieur de `iroha_config`
   donc les métriques d'exécution sont publiées
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   et les jauges `fastpq_metal_queue_*` avec les mêmes identifiants qui apparaissent
   dans `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. Lors de la mise en scène d'un nouveau
   classe, régénérez le manifeste matriciel via
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   afin que CI et les tableaux de bord comprennent l'étiquette supplémentaire.
2. **Vérifiez les jauges de file d'attente et les mesures d'adoption.** Exécutez `irohad --features fastpq-gpu`
   sur les hôtes Metal et grattez le point de terminaison de télémétrie pour confirmer la file d'attente en direct
   les jauges exportent :

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```La première commande prouve que l'échantillonneur de sémaphore émet le `busy`,
   Séries `overlap`, `limit` et `max_in_flight` et la seconde indique si
   chaque classe d'appareil se résout en `backend="metal"` ou revient à
   `backend="cpu"`. Câblez la cible à gratter via Prometheus/OTel avant
   importer le tableau de bord afin que Grafana puisse tracer immédiatement la vue de la flotte.
3. **Installez le pack tableau de bord + alertes.** Importer
   `dashboards/grafana/fastpq_acceleration.json` dans Grafana (conserver le
   variables de modèle de classe de périphérique, de famille de puces et de type GPU intégrées) et chargez
   `dashboards/alerts/fastpq_acceleration_rules.yml` dans Alertmanager ensemble
   avec son dispositif de test unitaire. Le pack de règles est livré avec un harnais `promtool` ; courir
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   chaque fois que les règles changent pour prouver `FastpqMetalDowngrade` et
   `FastpqBackendNoneBurst` se déclenche toujours aux seuils documentés.
4. **Gate libère avec le lot de preuves.** Gardez
   `docs/source/fastpq_rollout_playbook.md` pratique lors de la génération d'un déploiement
   soumission afin que chaque bundle contient les benchmarks enveloppés, exportation Grafana,
   pack d'alertes, preuve de télémétrie de file d'attente et journaux de restauration. CI applique déjà le
   contrat : `make check-fastpq-rollout` (ou invoquant
   `ci/check_fastpq_rollout.sh --bundle <path>`) valide le bundle, réexécute
   l'alerte teste et refuse de se déconnecter en cas de marge de file d'attente ou de remplissage nul
   les budgets régressent.
5. **Liez les alertes à la correction.** Lorsque vous consultez Alertmanager, utilisez le Grafana.
   carte et les compteurs bruts Prometheus de l'étape 2 pour confirmer si
   les rétrogradations proviennent d'un manque de file d'attente, de replis du processeur ou de rafales backend=none.
Le runbook réside dans
ce document plus `docs/source/fastpq_rollout_playbook.md` ; mettre à jour le
ticket de sortie avec le `fastpq_execution_mode_total` correspondant,
Extraits `fastpq_metal_queue_ratio` et `fastpq_metal_queue_depth` ensemble
avec des liens vers le panneau Grafana et l'instantané d'alerte afin que les réviseurs puissent voir
exactement quel SLO a déclenché.

### WP2-E — Instantané du profilage des métaux étape par étape

`scripts/fastpq/src/bin/metal_profile.rs` résume les captures de métal enveloppées
afin que la cible inférieure à 900 ms puisse être suivie au fil du temps (exécuter
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
Le nouvel assistant Markdown
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
génère les tableaux d'étape ci-dessous (il imprime le Markdown avec un texte
résumé afin que les tickets WP2-E puissent intégrer les preuves textuellement). Deux captures sont suivies
en ce moment :

> **Nouvelle instrumentation WP2-E :** `fastpq_metal_bench --gpu-probe ...` émet désormais un
> instantané de détection (mode d'exécution demandé/résolu, `FASTPQ_GPU`
> remplacements, backend détecté et périphériques Metal/identifiants de registre énumérés)
> avant l'exécution des noyaux. Capturez ce journal chaque fois qu'un GPU forcé fonctionne toujours
> revient au chemin du processeur afin que le groupe de preuves enregistre les hôtes qui voient
> `MTLCopyAllDevices` renvoie zéro et quels remplacements étaient en vigueur pendant la
> benchmark.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Aide à la capture de scène :** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> pilote désormais `fastpq_metal_bench` pour FFT, LDE et Poséidon individuellement,
> stocke les sorties JSON brutes dans des répertoires par étape et émet un seul
> Bundle `stage_profile_summary.json` qui enregistre les timings CPU/GPU, la profondeur de la file d'attente
> télémétrie, statistiques de transfert de colonnes, profils de noyau et trace associée
> les artefacts. Passez `--stage fft --stage lde --stage poseidon` pour cibler un sous-ensemble,
> `--trace-template "Metal System Trace"` pour choisir un modèle xctrace spécifique,
> et `--trace-dir` pour acheminer les bundles `.trace` vers un emplacement partagé. Attachez le
> résumé JSON plus les fichiers de trace générés pour chaque problème WP2-E afin que les réviseurs
> peut différer l'occupation de la file d'attente (`metal_dispatch_queue.*`), les taux de chevauchement et le
> Géométrie de lancement capturée à travers les courses sans spéléologie manuelle multiple
> Invocations `fastpq_metal_bench`.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Aide à la mise en file d'attente/à la préparation des preuves (09/05/2026) :** `scripts/fastpq/profile_queue.py` maintenant
> ingère un ou plusieurs `fastpq_metal_bench` JSON capture et émet à la fois une table Markdown et
> un résumé lisible par machine (`--markdown-out/--json-out`) donc la profondeur de la file d'attente, les taux de chevauchement et
> La télémétrie de mise en scène côté hôte peut accompagner chaque artefact WP2-E. Courir
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` a produit le tableau ci-dessous et a signalé que les captures de métaux archivées rapportent toujours
> `dispatch_count = 0` et `column_staging.batches = 0`—WP2-E.1 reste ouvert jusqu'à ce que le Metal
> l'instrumentation est reconstruite avec la télémétrie activée. Les artefacts JSON/Markdown générés sont en direct
> sous `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` pour audit.
> L'assistant (2026-05-19) fait également apparaître la télémétrie du pipeline Poséidon (`pipe_depth`,
> `batches`, `chunk_columns` et `fallbacks`) à la fois dans le tableau Markdown et dans le résumé JSON,
> afin que les réviseurs du WP2-E.4/6 puissent prouver si le GPU est resté sur le chemin pipeline et s'il y en a
> des replis se sont produits sans ouvrir la capture brute.【scripts/fastpq/profile_queue.py:1】> **Résumateur du profil d'étape (30/05/2026) :** `scripts/fastpq/stage_profile_report.py` consomme
> le bundle `stage_profile_summary.json` émis par `cargo xtask fastpq-stage-profile` et
> restitue à la fois les résumés Markdown et JSON afin que les réviseurs WP2-E puissent copier les preuves dans les tickets
> sans retranscrire manuellement les timings. Invoquer
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> produire des tableaux déterministes répertoriant les moyennes GPU/CPU, les deltas d'accélération, la couverture des traces et
> écarts de télémétrie par étage. La sortie JSON reflète le tableau et enregistre les balises de problème par étape
> (`trace missing`, `queue telemetry missing`, etc.) afin que l'automatisation de la gouvernance puisse différencier l'hôte
> exécutions référencées dans WP2-E.1 à WP2-E.6.
> **Protection contre le chevauchement hôte/périphérique (04/06/2026) :** `scripts/fastpq/profile_queue.py` annote désormais
> Les ratios d'attente FFT/LDE/Poséidon ainsi que les totaux d'aplatissement/d'attente par étape en millisecondes et émettent un
> problème chaque fois que `--max-wait-ratio <threshold>` détecte un mauvais chevauchement. Utiliser
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> pour capturer à la fois la table Markdown et le bundle JSON avec des taux d'attente explicites donc les tickets WP2-E.5
> peut montrer si la fenêtre de double tampon a alimenté le GPU. La sortie de la console en texte brut également
> répertorie les ratios par phase pour faciliter les enquêtes d'astreinte.
> **Telemetry guard + run status (2026-06-09) :** `fastpq_metal_bench` émet désormais un bloc `run_status`
> (étiquette backend, nombre d'envois, raisons) et le nouvel indicateur `--require-telemetry` fait échouer l'exécution
> chaque fois que les timings GPU ou la télémétrie de file d'attente/de transfert sont manquants. `profile_queue.py` restitue l'exécution
> le statut en tant que colonne dédiée et fait apparaître les états non `ok` dans la liste des problèmes, et
> `launch_geometry_sweep.py` intègre le même état dans les avertissements/classifications afin que les matrices ne puissent pas
> n'admet plus les captures qui reviennent silencieusement au processeur ou à l'instrumentation de file d'attente ignorée.
> **Réglage automatique Poséidon/LDE (12/06/2026) :** `metal_config::poseidon_batch_multiplier()` évolue désormais
> avec les astuces du jeu de travail Metal et `lde_tile_stage_target()` augmente la profondeur des tuiles sur les GPU discrets.
> Le multiplicateur appliqué et la limite de tuiles sont inclus dans le bloc `metal_heuristics` de
> Sorties `fastpq_metal_bench` et rendu par `scripts/fastpq/metal_capture_summary.py`, donc WP2-E
> Les bundles enregistrent les boutons de pipeline exacts utilisés dans chaque capture sans fouiller dans le JSON brut.

| Étiquette | Expédition | Occupé | Chevauchement | Profondeur maximale | FFT aplatir | Attendre FFT | % d'attente FFT | LDE aplatir | LDE attendre | % d'attente LDE | Poséidon aplatir | Poséidon attend | Poséidon attends % | Profondeur du tuyau | Lots de tuyaux | Solutions de secours pour les tuyaux |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### Instantané de 20 000 (pré-remplacement)

`fastpq_metal_bench_20k_latest.json`| Scène | Colonnes | Objectif d'entrée | Moyenne GPU (ms) | Moyenne CPU (ms) | Partage GPU | Accélération | Δ CPU (ms) |
| --- | --- : | --- : | --- : | --- : | --- : | --- : | --- : |
| FFT | 16 | 32768 | 130,986 ms (115,761–167,755) | 112,616 ms (95,335–132,929) | 2,4% | 0,860× | −18.370 |
| IFFT | 16 | 32768 | 129,296 ms (111,127–142,955) | 158,144 ms (126,847–237,887) | 2,4% | 1,223× | +28.848 |
| LDE | 16 | 262144 | 1 570,656 ms (1 544,397–1 584,502) | 1752,523 ms (1548,807–2191,930) | 29,2% | 1,116 × | +181.867 |
| Poséidon | 16 | 524288 | 3548,329 ms (3519,881–3576,041) | 3642,706 ms (3539,055–3758,279) | 66,0% | 1,027× | +94.377 |

Observations clés :1. Le total du GPU est de 5,379 s, soit **4,48 s de plus** que l'objectif de 900 ms. Poséidon
   le hachage domine toujours le runtime (≈66%) avec le noyau LDE en seconde
   place (≈29%), donc WP2-E doit attaquer à la fois la profondeur du pipeline Poséidon et
   le plan de résidence/tuilage de la mémoire LDE avant que les solutions de secours du processeur ne disparaissent.
2. FFT reste une régression (0,86×) même si IFFT est >1,22× sur le scalaire
   chemin. Nous avons besoin d'un balayage de la géométrie de lancement
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) pour comprendre
   si l'occupation FFT peut être récupérée sans nuire à ce qui est déjà meilleur
   Horaires IFFT. L'assistant `scripts/fastpq/launch_geometry_sweep.py` pilote désormais
   ces expériences de bout en bout : passez les remplacements séparés par des virgules (par exemple,
   `--fft-columns 16,32 --queue-fanout 1,2` et
   `--poseidon-lanes auto,256`) et il invoquera
   `fastpq_metal_bench` pour chaque combinaison, stockez les charges utiles JSON sous
   `artifacts/fastpq_geometry/<timestamp>/` et conserver un bundle `summary.json`
   décrivant les ratios de file d'attente de chaque exécution, les choix de lancement FFT/LDE, les timings GPU vs CPU,
   et les métadonnées de l'hôte (nom d'hôte/étiquette, plate-forme triple, périphérique détecté
   classe, fournisseur/modèle de GPU), les comparaisons entre appareils sont donc déterministes
   provenance. L'assistant écrit désormais également `reason_summary.json` à côté du
   résumé par défaut, en utilisant le même classificateur que la matrice géométrique à rouler
   des pannes de processeur et des télémétries manquantes. Utilisez `--host-label staging-m3` pour marquer
   captures à partir de laboratoires partagés.
   L'outil compagnon `scripts/fastpq/geometry_matrix.py` ingère désormais un ou
   plus de bundles récapitulatifs (`--summary hostA/summary.json --summary hostB/summary.json`)
   et émet des tables Markdown/JSON qui étiquetent chaque forme de lancement comme *stable*
   (timings GPU FFT/LDE/Poséidon capturés) ou *instable* (timeout, repli du CPU,
   backend non métallique ou télémétrie manquante) à côté des colonnes hôtes. Le
   les tables incluent désormais le `execution_mode`/`gpu_backend` résolu plus un
   `Reason` afin que les replis du processeur et les timings GPU manquants soient évidents dans
   Matrices Stage7 même lorsque des blocs de synchronisation sont présents ; une ligne récapitulative compte
   les exécutions stables par rapport au total. Passer `--operation fft|lde|poseidon_hash_columns`
   lorsque le balayage doit isoler un seul étage (par exemple, pour profiler
   Poséidon séparément) et gardez `--extra-args` gratuit pour les drapeaux spécifiques au banc.
   L'assistant accepte tout
   préfixe de commande (par défaut `cargo run … fastpq_metal_bench`) plus facultatif
   `--halt-on-error` / `--timeout-seconds` protège pour que les ingénieurs de performance puissent
   reproduire le balayage sur différentes machines tout en collectant des comparables,
   Ensembles de preuves multi-appareils pour Stage7.
3. `metal_dispatch_queue` a signalé `dispatch_count = 0`, donc occupation de la file d'attente
   la télémétrie manquait même si les noyaux GPU fonctionnaient. Le runtime Metal utilise désormais
   acquérir/libérer les clôtures pour les bascules de mise en file d'attente/colonne afin que les threads de travail
   observez les drapeaux d'instrumentation et le rapport de la matrice géométrique appelle
   formes de lancement instables chaque fois que les timings GPU FFT/LDE/Poseidon sont absents. Garder
   joindre la matrice Markdown/JSON aux tickets WP2-E afin que les évaluateurs puissent voir
   quelles combinaisons échouent toujours une fois que la télémétrie de file d’attente devient disponible.La garde `run_status` et l'indicateur `--require-telemetry` échouent désormais à la capture.
   chaque fois que les timings GPU sont manquants ou que la télémétrie de file d'attente/de transfert est absente, donc
   dispatch_count=0 les exécutions ne peuvent plus se glisser inaperçues dans les bundles WP2-E.
   `fastpq_metal_bench` expose désormais `--require-gpu`, et
   `launch_geometry_sweep.py` l'active par défaut (désinscription avec
   `--allow-cpu-fallback`) donc les replis du processeur et les échecs de détection de métaux sont abandonnés
   immédiatement au lieu de polluer les matrices Stage7 avec une télémétrie non GPU.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Les métriques de remplissage à zéro ont disparu auparavant pour la même raison ; la réparation de la clôture
   maintient l'instrumentation hôte en direct, la prochaine capture doit donc inclure le
   Bloc `zero_fill` sans timings synthétiques.

#### Instantané 20 000 avec `FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

| Scène | Colonnes | Objectif d'entrée | Moyenne GPU (ms) | Moyenne CPU (ms) | Partage GPU | Accélération | Δ CPU (ms) |
| --- | --- : | --- : | --- : | --- : | --- : | --- : | --- : |
| FFT | 16 | 32768 | 79,951 ms (65,645–93,193) | 83,289 ms (59,956-107,585) | 0,3% | 1,042× | +3.338 |
| IFFT | 16 | 32768 | 78,605 ms (69,986–83,726) | 93,898 ms (80,656-119,625) | 0,3% | 1,195× | +15.293 |
| LDE | 16 | 262144 | 657,673 ms (619,219–712,367) | 669,537 ms (619,716–723,285) | 2,1% | 1,018× | +11.864 |
| Poséidon | 16 | 524288 | 30004,898 ms (27284,117–32945,253) | 29087,532 ms (24969,810–33020,517) | 97,4% | 0,969 × | −917.366 |

Observations :

1. Même avec `FASTPQ_GPU=gpu`, cette capture reflète toujours le repli du processeur :
   ~ 30 secondes par itération avec `metal_dispatch_queue` bloqué à zéro. Quand le
   le remplacement est défini mais l'hôte ne peut pas découvrir un périphérique Metal, la CLI se ferme maintenant
   avant d'exécuter un noyau et imprime le mode demandé/résolu ainsi que le
   étiquette backend afin que les ingénieurs puissent savoir si la détection, les droits ou le
   La recherche de metallib a provoqué le déclassement. Exécutez `fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` pour capturer le journal d'énumération et
   corrigez le problème de détection sous-jacent avant de réexécuter le profileur.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. La télémétrie à remplissage zéro enregistre désormais un échantillon réel (18,66 ms sur 32 Mo), ce qui prouve
   le correctif de clôture fonctionne, mais les deltas de file d'attente restent absents jusqu'à l'envoi du GPU
   réussir.
3. Étant donné que le backend continue de se dégrader, la porte de télémétrie Stage7 est toujours
   bloqué : les preuves de marge de file d'attente et le chevauchement de Poséidon nécessitent un véritable GPU
   courir.

Ces captures ancrent désormais le backlog du WP2-E. Actions suivantes : rassembler le profileur
flamecharts et journaux de file d'attente (une fois le backend exécuté sur le GPU), ciblez le
Goulots d'étranglement Poséidon/LDE avant de revoir FFT et débloquer le backend du backend
la télémétrie Stage7 contient donc de vraies données GPU.

### Points forts
- Mise en scène incrémentale, conception trace-first, pile STARK transparente.### Éléments d'action hautement prioritaires
1. Mettre en œuvre les modalités d'emballage/de commande et mettre à jour les spécifications AIR.
2. Finalisez le commit Poseidon2 `3f2b7fe` et publiez des exemples de vecteurs SMT/lookup.
3. Conservez les exemples travaillés (`lookup_grand_product.md`, `smt_update.md`) à côté des luminaires.
4. Ajouter l'annexe A documentant la méthodologie de calcul de la solidité et de rejet des CI.

### Décisions de conception résolues
- ZK désactivé (exactitude uniquement) dans P1 ; revisiter dans une étape ultérieure.
- Racine de la table d'autorisation dérivée de l'état de gouvernance ; les lots traitent la table en lecture seule et prouvent son appartenance via une recherche.
- Les preuves à clé absente utilisent une feuille zéro plus un témoin voisin avec un codage canonique.
- Supprimer la sémantique = valeur de la feuille définie sur zéro dans l'espace de clés canonique.

Utilisez ce document comme référence canonique ; mettez-le à jour avec le code source, les appareils et les annexes pour éviter toute dérive.

## Annexe A — Dérivation de la solidité

Cette annexe explique comment le tableau « Solidité et SLO » est produit et comment CI applique le plancher ≥ 128 bits mentionné précédemment.

### Notation
- `N_trace = 2^k` — longueur de trace après tri et remplissage à une puissance de deux.
- `b` — facteur d'explosion (`N_eval = N_trace × b`).
- `r` — Arité FRI (8 ou 16 pour les ensembles canoniques).
- `ℓ` — nombre de réductions FRI (colonne `layers`).
- `q` — requêtes du vérificateur par preuve (colonne `queries`).
- `ρ` — taux de code effectif signalé par le planificateur de colonnes : `ρ = max_i(degree_i / domain_i)` sur les contraintes qui survivent au premier tour FRI.

Le champ de base Boucle d'or a `|F| = 2^64 - 2^32 + 1`, donc les collisions Fiat-Shamir sont délimitées par `q / 2^64`. Le broyage ajoute un facteur orthogonal `2^{-g}`, avec `g = 23` pour `fastpq-lane-balanced` et `g = 21` pour le profil de latence.【crates/fastpq_isi/src/params.rs:65】

### Lié à l'analyse

Avec DEEP-FRI à taux constant, la probabilité de défaillance statistique satisfait

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

car chaque couche réduit le degré polynomial et la largeur du domaine du même facteur `r`, en gardant `ρ` constant. La colonne `est bits` du tableau indique `⌊-log₂ p_fri⌋` ; Fiat-Shamir et le meulage servent de marge de sécurité supplémentaire.

### Sortie du planificateur et calcul travaillé

L'exécution du planificateur de colonnes Stage1 sur des lots représentatifs donne les résultats suivants :

| Jeu de paramètres | `N_trace` | `b` | `N_eval` | `ρ` (planificateur) | Diplôme effectif (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | -------------------------------- | --- | --- | ------------------ |
| Lot équilibré de 20 000 | `2^15` | 8 | 262144 | 0,077026 | 20192 | 5 | 52 | 190 bits |
| Débit 65 000 lots | `2^16` | 8 | 524288 | 0,200208 | 104967 | 6 | 58 | 132 bits |
| Latence 131k lot | `2^17` | 16 | 2097152 | 0,209492 | 439337 | 5 | 64 | 142 bits |Exemple (lot équilibré de 20 000) :
1. `N_trace = 2^15`, donc `N_eval = 2^15 × 8 = 2^18`.
2. L'instrumentation du planificateur rapporte `ρ = 0.077026`, donc `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, correspondant à l'entrée du tableau.
4. Les collisions Fiat-Shamir ajoutent au maximum `2^{-58.3}`, et le broyage (`g = 23`) soustrait un autre `2^{-23}`, maintenant la solidité totale confortablement au-dessus de 160 bits.

### Faisceau d'échantillonnage par rejet CI

Chaque exécution de CI exécute un exploit de Monte Carlo pour garantir que les mesures empiriques restent à ± 0,6 bit de la limite analytique :
1. Dessinez un jeu de paramètres canoniques et synthétisez un `TransitionBatch` avec le nombre de lignes correspondant.
2. Construisez la trace, inversez une contrainte choisie au hasard (par exemple, perturber le grand produit de recherche ou un frère SMT) et essayez de produire une preuve.
3. Réexécutez le vérificateur, en rééchantillonnant les défis Fiat-Shamir (meulage inclus) et enregistrez si la preuve falsifiée est rejetée.
4. Répétez l'opération pour 16 384 graines par jeu de paramètres et convertissez la limite inférieure de 99 % de Clopper-Pearson du taux de rejet observé en bits.

Le travail échoue immédiatement si la limite inférieure mesurée passe en dessous de 128 bits, de sorte que les régressions dans le planificateur, la boucle de pliage ou le câblage de transcription sont détectées avant la fusion.

## Annexe B — Dérivation domaine-racine

Stage0 épingle les générateurs de trace et d'évaluation aux constantes dérivées de Poséidon afin que toutes les implémentations partagent les mêmes sous-groupes.

### Procédure
1. **Sélection des graines.** Absorbez la balise UTF‑8 `fastpq:v1:domain_roots` dans l'éponge Poséidon2 utilisée ailleurs dans FASTPQ (largeur d'état = 3, taux = 2, quatre tours complets + 57 tours partiels). Les entrées réutilisent le codage `[len, limbs…]` de `pack_bytes`, produisant le générateur de base `g_base = 7`.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **Générateur de traces.** Calculez `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` et vérifiez `trace_root^{2^{trace_log_size}} = 1` alors que la demi-puissance n'est pas 1.
3. **Générateur LDE.** Répétez la même exponentiation avec `lde_log_size` pour dériver `lde_root`.
4. **Sélection Coset.** Stage0 utilise le sous-groupe de base (`omega_coset = 1`). Les futurs cosets pourront absorber une balise supplémentaire telle que `fastpq:v1:domain_roots:coset`.
5. **Taille de permutation.** Conservez explicitement `permutation_size` afin que les planificateurs ne déduisent jamais de règles de remplissage à partir de puissances implicites de deux.

###Reproduction et validation
- Outillage : `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` émet soit des extraits de code Rust, soit une table Markdown (voir `--format table`, `--seed`, `--filter`).【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- Tests : `canonical_sets_meet_security_target` maintient les jeux de paramètres canoniques alignés avec les constantes publiées (racines non nulles, couplage explosion/arité, dimensionnement de permutation), donc `cargo test -p fastpq_isi` détecte immédiatement la dérive. 【crates/fastpq_isi/src/params.rs:138】
- Source de vérité : mettez à jour la table Stage0 et `fastpq_isi/src/params.rs` ensemble chaque fois que de nouveaux packs de paramètres sont introduits.

## Annexe C — Détails du pipeline d'engagement### Streaming du flux d'engagement de Poséidon
L'étape 2 définit l'engagement de trace déterministe partagé par le prouveur et le vérificateur :
1. **Normaliser les transitions.** `trace::build_trace` trie chaque lot, le complète avec `N_trace = 2^{⌈log₂ rows⌉}` et émet des vecteurs de colonnes dans l'ordre ci-dessous.【crates/fastpq_prover/src/trace.rs:123】
2. **Colonnes de hachage.** `trace::column_hashes` diffuse les colonnes via des éponges Poséidon2 dédiées étiquetées `fastpq:v1:trace:column:<name>`. Lorsque la fonctionnalité `fastpq-prover-preview` est active, le même parcours recycle les coefficients IFFT/LDE requis par le backend, donc aucune copie de matrice supplémentaire n'est allouée.【crates/fastpq_prover/src/trace.rs:474】
3. **Soulevez dans un arbre Merkle.** `trace::merkle_root` plie les résumés de colonnes avec les nœuds Poséidon étiquetés `fastpq:v1:trace:node`, dupliquant la dernière feuille chaque fois qu'un niveau présente une répartition étrange pour éviter des cas particuliers.【crates/fastpq_prover/src/trace.rs:656】
4. **Finalisez le résumé.** `digest::trace_commitment` préfixe la balise de domaine (`fastpq:v1:trace_commitment`), le nom du paramètre, les dimensions complétées, les résumés de colonne et la racine Merkle en utilisant le même codage `[len, limbs…]`, puis hache la charge utile avec SHA3-256 avant de l'intégrer dans `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

Le vérificateur recalcule le même résumé avant d'échantillonner les défis Fiat-Shamir, donc les preuves d'abandon ne correspondent pas avant toute ouverture.

### Commandes de secours de Poséidon- Le prouveur expose désormais un remplacement de pipeline Poséidon dédié (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) afin que les opérateurs puissent mélanger GPU FFT/LDE avec le hachage CPU Poséidon sur les appareils qui ne parviennent pas à atteindre l'objectif Stage7 <900 ms. Les valeurs prises en charge reflètent le bouton de mode d'exécution (`auto`, `cpu`, `gpu`), par défaut sur le mode global lorsqu'il n'est pas spécifié. Le moteur d'exécution transmet cette valeur via la configuration de voie (`FastpqPoseidonMode`) et la propage dans le prouveur (`Prover::canonical_with_modes`), de sorte que les remplacements sont déterministes et vérifiables dans la configuration. dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- La télémétrie exporte le mode pipeline résolu via le nouveau compteur `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` (et le jumeau OTLP `fastpq.poseidon_pipeline_resolutions_total`). Les tableaux de bord `sorafs`/opérateur peuvent donc confirmer quand un déploiement exécute un hachage GPU fusionné/pipeline par rapport au repli forcé du processeur (`path="cpu_forced"`) ou aux rétrogradations du runtime (`path="cpu_fallback"`). La sonde CLI s'installe automatiquement dans `irohad`, de sorte que les bundles de versions et la télémétrie en direct partagent le même flux de preuves.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- Les preuves en mode mixte sont également inscrites dans chaque tableau de bord via la porte d'adoption existante : le prouveur émet le mode résolu + l'étiquette de chemin pour chaque lot, et le compteur `fastpq_poseidon_pipeline_total` s'incrémente aux côtés du compteur du mode d'exécution chaque fois qu'une preuve arrive. Cela satisfait au WP2-E.6 en rendant les baisses de tension visibles et en fournissant un commutateur propre pour les rétrogradations déterministes pendant que l'optimisation se poursuit.
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` analyse désormais les scrapes Prometheus (métal ou CUDA) et intègre un résumé `poseidon_metrics` dans chaque paquet enveloppé. L'assistant filtre les lignes du compteur par `metadata.labels.device_class`, capture les échantillons `fastpq_execution_mode_total` correspondants et échoue à l'enroulement lorsque les entrées `fastpq_poseidon_pipeline_total` sont manquantes afin que les bundles WP2-E.6 expédient toujours des preuves CUDA/Metal reproductibles au lieu d'éléments ad hoc. notes.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Politique déterministe en mode mixte (WP2-E.6)1. **Détecter le déficit du GPU.** Signalez toute classe d'appareil dont la capture Stage7 ou l'instantané Grafana en direct montre la latence de Poséidon en gardant le temps de preuve total > 900 ms tandis que FFT/LDE reste en dessous de l'objectif. Les opérateurs annotent la matrice de capture (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) et bipent l'astreinte lorsque `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` stagne tandis que `fastpq_execution_mode_total{backend="metal"}` enregistre toujours les expéditions GPU FFT/LDE.
2. ** Basculez vers CPU Poseidon uniquement pour les hôtes concernés. ** Définissez `zk.fastpq.poseidon_mode = "cpu"` (ou `FASTPQ_POSEIDON_MODE=cpu`) dans la configuration locale de l'hôte à côté des étiquettes de flotte, en conservant `zk.fastpq.execution_mode = "gpu"` afin que FFT/LDE continue d'utiliser l'accélérateur. Enregistrez la différence de configuration dans le ticket de déploiement et ajoutez le remplacement par hôte au bundle sous le nom `poseidon_fallback.patch` afin que les réviseurs puissent rejouer la modification de manière déterministe.
3. **Prouvez le déclassement.** Grattez le compteur Poséidon immédiatement après le redémarrage du nœud :
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   Le vidage doit afficher `path="cpu_forced"` croissant au même rythme que le compteur d'exécution du GPU. Stockez le scrape sous le nom `metrics_poseidon.prom` à côté de l'instantané `metrics_cpu_fallback.prom` existant et capturez les lignes de journal `telemetry::fastpq.poseidon` correspondantes dans `poseidon_fallback.log`.
4. **Surveiller et quitter.** Continuez à alerter sur `fastpq_poseidon_pipeline_total{path="cpu_forced"}` pendant que le travail d'optimisation se poursuit. Une fois qu'un correctif ramène le temps d'exécution par preuve en dessous de 900 ms sur l'hôte de test, restaurez la configuration sur `auto`, réexécutez le scrape (affichant à nouveau `path="gpu"`) et attachez les métriques avant/après au bundle pour fermer l'exercice en mode mixte.

**Contrat de télémétrie.**

| Signalisation | PromQL / Source | Objectif |
|--------|-----------------|---------|
| Compteur mode Poséidon | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Confirme que le hachage du processeur est intentionnel et limité à la classe de périphérique signalée. |
| Compteur de mode d'exécution | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Prouve que FFT/LDE fonctionne toujours sur GPU même pendant la rétrogradation de Poséidon. |
| Enregistrer les preuves | Entrées `telemetry::fastpq.poseidon` capturées dans `poseidon_fallback.log` | Fournit une preuve probante que l’hôte a résolu le hachage du processeur avec la raison `cpu_forced`. |

Le bundle de déploiement doit désormais inclure `metrics_poseidon.prom`, la différence de configuration et l'extrait de journal chaque fois que le mode mixte est actif afin que la gouvernance puisse auditer la politique de secours déterministe parallèlement à la télémétrie FFT/LDE. `ci/check_fastpq_rollout.sh` applique déjà les limites de file d'attente/remplissage à zéro ; la porte de suivi vérifiera le compteur Poséidon une fois que le mode mixte aura atterri dans l'automatisation de la version.

L'outil de capture Stage7 gère déjà CUDA : enveloppez chaque bundle `fastpq_cuda_bench` avec `--poseidon-metrics` (pointant vers le `metrics_poseidon.prom` gratté) et la sortie contient désormais les mêmes compteurs de pipeline/résumé de résolution utilisés sur Metal afin que la gouvernance puisse vérifier les replis CUDA sans outils sur mesure. 【scripts/fastpq/wrap_benchmark.py:1】### Ordre des colonnes
Le pipeline de hachage consomme les colonnes dans cet ordre déterministe :
1. Indicateurs de sélection : `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
2. Colonnes de membres compactées (chacune étant complétée par des zéros à la longueur de la trace) : `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Scalaires auxiliaires : `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`.
4. Témoins Merkle clairsemés pour chaque niveau `ℓ ∈ [0, SMT_HEIGHT)` : `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` parcourt les colonnes exactement dans cet ordre, de sorte que le backend d'espace réservé et l'implémentation de Stage2 STARK restent stables dans toutes les versions.【crates/fastpq_prover/src/trace.rs:474】

### Balises de domaine de transcription
Stage2 corrige le catalogue Fiat-Shamir ci-dessous pour que la génération de défis reste déterministe :

| Étiquette | Objectif |
| --- | ------- |
| `fastpq:v1:init` | Absorbez la version du protocole, l’ensemble de paramètres et `PublicIO`. |
| `fastpq:v1:roots` | Validez la trace et recherchez les racines de Merkle. |
| `fastpq:v1:gamma` | Essayez le défi de recherche de grand produit. |
| `fastpq:v1:alpha:<i>` | Exemples de défis de composition-polynôme (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Absorbez le grand produit de recherche évalué. |
| `fastpq:v1:beta:<round>` | Goûtez au défi de pliage pour chaque tour du VENDREDI. |
| `fastpq:v1:fri_layer:<round>` | Validez la racine Merkle pour chaque couche FRI. |
| `fastpq:v1:fri:final` | Enregistrez la couche FRI finale avant d’ouvrir les requêtes. |
| `fastpq:v1:query_index:0` | Dérivez de manière déterministe les indices de requête du vérificateur. |