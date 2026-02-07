---
lang: fr
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-18T05:31:56.955438+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fusionner la conception du grand livre - Finalité des voies et réduction globale

Cette note finalise la conception du grand livre de fusion pour le jalon 5. Elle explique les
politique de blocage non vide, sémantique de fusion QC entre voies et flux de travail de finalité
qui lie l’exécution au niveau de la voie à l’engagement mondial de l’État.

La conception étend l'architecture Nexus décrite dans `nexus.md`. Des termes tels que
"bloc de voie", "QC de voie", "indice de fusion" et "registre de fusion" héritent de leurs
définition tirée de ce document ; cette note se concentre sur les règles comportementales et
des conseils de mise en œuvre qui doivent être appliqués par le runtime, le stockage et WSV
couches.

## 1. Politique de blocage non vide

**Règle (DOIT) :** Un proposant de voie émet un blocage uniquement lorsque le blocage contient au
au moins un fragment de transaction exécuté, un déclencheur temporel ou déterministe
mise à jour des artefacts (par exemple, cumul des artefacts DA). Les blocs vides sont interdits.

**Implication :**

- Slot keep-alive : lorsqu'aucune transaction ne respecte sa fenêtre de validation déterministe,
la voie n'émet aucun bloc et avance simplement vers l'emplacement suivant. Le grand livre de fusion
reste sur le conseil précédent pour cette voie.
- Lot de déclencheurs : déclencheurs en arrière-plan qui ne produisent aucune transition d'état (par exemple,
cron qui réaffirme les invariants) sont considérés comme vides et DOIVENT être ignorés ou
regroupé avec d’autres travaux avant de produire un bloc.
- Télémétrie : `pipeline_detached_merged` et traitement des métriques de suivi ignorés
créneaux explicitement : les opérateurs peuvent distinguer « aucun travail » de « pipeline bloqué ».
- Replay : le stockage en bloc n'insère pas d'espaces réservés synthétiques vides. Le Koura
la boucle de relecture observe simplement le même hachage parent pour les emplacements consécutifs si aucun
Le bloc a été émis.

**Vérification canonique :** Lors de la proposition et de la validation du bloc, `ValidBlock::commit`
affirme que le `StateBlock` associé comporte au moins une superposition validée
(delta, artefact, déclencheur). Cela correspond à la garde `StateBlock::is_empty`
cela garantit déjà que les écritures sans opération sont supprimées. L'application a lieu avant
des signatures sont demandées afin que les comités ne votent jamais sur des charges utiles vides.

## 2. Sémantique de fusion Cross-Lane QC

Chaque bloc de voie `B_i` finalisé par son comité produit :

- `lane_state_root_i` : l'engagement de Poséidon2-SMT sur les racines de l'état par DS a été touché
dans le bloc.
- `merge_hint_root_i` : candidat glissant pour le grand livre de fusion (`tag =
"iroha:fusion:candidat:v1\0"`).
- `lane_qc_i` : signatures agrégées du comité de piste sur la
  préimage de vote d'exécution (hachage de bloc, `parent_state_root`,
  `post_state_root`, hauteur/vue/époque, chain_id et balise de mode).

Fusionner les nœuds collecte les dernières astuces `{(B_i, lane_qc_i, merge_hint_root_i)}` pour
toutes les voies `i ∈ [0, K)`.

**Fusionner l'entrée (DOIT) :**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` est le hachage de la voie, bloque les sceaux d'entrée de fusion pour la voie
  `i`. Si une voie n'a émis aucun bloc depuis l'entrée de fusion précédente, cette valeur est
  répété.
- `merge_hint_root[i]` est le `merge_hint_root` de la voie correspondante
  bloquer. Il est répété lorsque `lane_tips[i]` se répète.
- `global_state_root` est égal à `ReduceMergeHints(merge_hint_root[0..K-1])`, un
  Poséidon2 se plie avec une balise de séparation de domaine
  `"iroha:merge:reduce:v1\0"`. La réduction est déterministe et DOIT
  reconstruire la même valeur entre pairs.
- `merge_qc` est un certificat de quorum BFT du comité de fusion sur la
  entrée sérialisée.

**Fusionner la charge utile QC (DOIT) :**

Les membres du comité de fusion signent un résumé déterministe :

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` est la vue du comité de fusion dérivée des conseils de voie (max
  `view_change_index` à travers les collecteurs de voies scellés par l'entrée).
- `chain_id` est la chaîne d'identifiant de chaîne configurée (UTF-8 octets).
- La charge utile utilise le codage Norito avec l'ordre des champs indiqué ci-dessus.

Le résumé résultant est stocké dans `merge_qc.message_digest` et constitue le message
vérifié par les signatures BLS.

**Fusionner QC Construction (DOIT) :**

- La liste du comité de fusion est l'ensemble actuel des validateurs de topologie de validation.
- Quorum requis = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` code les indices de validation participants (LSB-first)
  dans l'ordre de la topologie de validation.
- `merge_qc.aggregate_signature` est l'agrégat BLS-normal pour le résumé
  ci-dessus.

**Validation (DOIT) :**

1. Vérifiez chaque `lane_qc_i` par rapport à `lane_tips[i]` et confirmez les en-têtes de bloc.
   inclure le `merge_hint_root_i` correspondant.
2. Assurez-vous qu'aucun `lane_qc_i` ne pointe vers un `Invalid` ou un bloc non exécuté. Le
   La politique non vide ci-dessus garantit que l'en-tête inclut des superpositions d'état.
3. Recalculez `ReduceMergeHints` et comparez avec `global_state_root`.
4. Recalculez le résumé QC de fusion et vérifiez le bitmap du signataire, le seuil de quorum,
   et signature globale par rapport à la liste de topologie de validation.

**Observabilité :** Les nœuds de fusion émettent des compteurs Prometheus pour
`merge_entry_lane_repeats_total{i}` pour mettre en évidence les voies qui ont sauté des emplacements pour
visibilité opérationnelle.

## 3. Flux de travail final

### 3.1 Finalité au niveau de la voie

1. Les transactions sont planifiées par voie dans des créneaux déterministes.
2. L'exécuteur applique des superpositions dans `StateBlock`, produisant des deltas et
artefacts.
3. Après validation, le comité de piste signe le pré-image d'exécution-vote qui
   lie le hachage du bloc, les racines de l'état et la hauteur/vue/époque. Le tuple
   `(block_hash, lane_qc_i, merge_hint_root_i)` est considéré comme la finale de la voie.
4. Les clients légers PEUVENT traiter le bout de la voie comme final pour les épreuves limitées DS, mais
doit enregistrer le `merge_hint_root` associé pour effectuer un rapprochement avec le grand livre de fusion
plus tard.Les comités de voie sont par espace de données et ne remplacent pas l'engagement global
topologie. La taille du comité est fixée à `3f+1`, où `f` vient du
catalogue d'espace de données (`fault_tolerance`). Le pool de validateurs est le pool de données de l'espace de données
validateurs (manifestes de gouvernance de voie pour les voies gérées par l'administrateur ou les voies publiques
jalonnement des voies élues). La composition du comité est
échantillonné de manière déterministe une fois par époque en utilisant la graine d'époque VRF liée à
`dataspace_id` et `lane_id`. Si le bassin est plus petit que `3f+1`, finalité de la voie
fait une pause jusqu'à ce que le quorum soit rétabli (la récupération d'urgence est gérée séparément).

### 3.2 Finalité de la fusion-grand livre

1. Le comité de fusion collecte les derniers conseils de voie, vérifie chaque `lane_qc_i` et
construit le `MergeLedgerEntry` comme défini ci-dessus.
2. Après avoir vérifié la réduction déterministe, le comité de fusion signe le
entrée (`merge_qc`).
3. Les nœuds ajoutent l'entrée au journal du grand livre de fusion et la conservent avec le
références aux blocs de voies.
4. `global_state_root` devient l’engagement faisant autorité de l’État mondial pour le
époque/créneau. Les nœuds complets mettent à jour leurs métadonnées de point de contrôle WSV pour refléter cela
valeur ; la relecture déterministe doit reproduire la même réduction.

### 3.3 WSV et intégration du stockage

- `State::commit_merge_entry` enregistre les racines de l'état par voie et le
  final `global_state_root`, reliant l'exécution de la voie à la somme de contrôle globale.
- Kura persiste `MergeLedgerEntry` à côté des artefacts de bloc de voie, donc un
  la relecture peut reconstruire à la fois les séquences de finalité au niveau de la voie et celles de finalité globale.
- Lorsqu'une voie saute un emplacement, le stockage conserve simplement le pourboire précédent ; non
  Les entrées de fusion d'espace réservé sont créées jusqu'à ce qu'au moins une voie produise une nouvelle
  bloquer.
- Les surfaces API (Torii, télémétrie) exposent les deux bouts de voie et la dernière fusion
  entrée afin que les opérateurs et les clients puissent concilier les vues par voie et globales.

## 4. Notes de mise en œuvre- `crates/iroha_core/src/state.rs` : `State::commit_merge_entry` valide le
  réduction et connecte les métadonnées de voie/globales à l'état mondial afin d'interroger
  et les observateurs peuvent accéder aux conseils de fusion et au hachage global faisant autorité.
- `crates/iroha_core/src/kura.rs` : `Kura::store_block_with_merge_entry` met en file d'attente
  le bloc et conserve l'entrée de fusion associée en une seule étape, en revenant en arrière
  le bloc en mémoire lorsque l'ajout échoue afin que le stockage n'enregistre jamais de bloc
  sans ses métadonnées de scellement. Le journal du grand livre de fusion est élagué étape par étape
  avec la hauteur de bloc validée lors de la récupération au démarrage et mise en cache en mémoire
  avec une fenêtre délimitée (`kura.merge_ledger_cache_capacity`, 256 par défaut) pour
  évitez une croissance illimitée sur les nœuds de longue durée. La récupération tronque partiellement ou
  écritures de fusion surdimensionnées et ajouter les écritures de rejet au-dessus du
  garde de taille maximale de charge utile pour plafonner les allocations.
- `crates/iroha_core/src/block.rs` : la validation des blocs rejette les blocs sans
  points d'entrée (transactions externes ou déclencheurs temporels) et sans déterminisme
  des artefacts tels que des bundles DA (`BlockValidationError::EmptyBlock`), garantissant
  la politique non vide est appliquée avant que les signatures ne soient demandées et portées
  dans le grand livre de fusion.
- L'assistant de réduction déterministe réside dans le service de fusion : `reduce_merge_hint_roots`.
  (`crates/iroha_core/src/merge.rs`) implémente le pli Poséidon2 décrit ci-dessus.
  Les hooks d'accélération matérielle restent des travaux futurs, mais le chemin scalaire s'applique désormais
  la réduction canonique de manière déterministe.
- Intégration de télémétrie : exposition des répétitions de fusion par voie et du
  La jauge `global_state_root` reste suivie dans le backlog d'observabilité afin que le
  le travail sur le tableau de bord peut être expédié parallèlement au déploiement du service de fusion.
- Tests multi-composants : la couverture de relecture dorée pour la réduction de fusion est
  suivi avec le backlog de tests d'intégration pour garantir les modifications futures de
  `reduce_merge_hint_roots` maintient les racines enregistrées stables.