<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi Modèle formel (TLA+ / Apalache)

Ce répertoire contient des modèles formels limités pour la sécurité et l'activité Sumeragi.

## Portée

`Sumeragi.tla` capture le chemin de validation :
- évolution des phases (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- les seuils de vote et de quorum (`CommitQuorum`, `ViewQuorum`),
- quorum de mise pondéré (`StakeQuorum`) pour les commit guards de type NPoS,
- Causalité GR (`Init -> Chunk -> Ready -> Deliver`) avec preuve d'en-tête/digest,
- TPS et faibles hypothèses d'équité par rapport aux actions de progrès honnêtes.

`SumeragiFrontierRecovery.tla` capture la classe de suspension Taira ciblée autour d'un
en attente d'un bloc frontière contigu :
- preuve de vote engagé en dessous ou au quorum,
- retard dans la file d'attente des votes et fuite locale,
- état de charge utile manquant ou local,
- propriété de récupération de frontière fraîche ou obsolète,
- marqueur de reprogrammation du quorum/rythme de la fenêtre,
- des preuves de frontières futures/nouvelles perspectives qui peuvent réancrer la frontière locale,
- validation déterministe post-TPS, retransmission, rotation de vue limitée et
  résultats de chute sans preuve.

Les deux modèles éliminent intentionnellement les formats de fil, ECDSA/signature
vérification et détails complets du réseau.

## Fichiers- `Sumeragi.tla` : modèle et propriétés du protocole.
- `Sumeragi_fast.cfg` : jeu de paramètres plus petit compatible CI.
- `Sumeragi_deep.cfg` : jeu de paramètres de contrainte plus large.
- `SumeragiFrontierRecovery.tla` : modèle ciblé de rétablissement des frontières.
- `SumeragiFrontierRecovery_fast.cfg` : ensemble de paramètres de frontière plus petit et compatible CI.
- `SumeragiFrontierRecovery_deep.cfg` : ensemble de limites de backlog/fenêtre/vue de frontière plus large.
- `SumeragiFrontierRecovery_wide.cfg` : ensemble manuel de limites de frontière plus larges.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg` : mutation de propriétaire obsolète à échec attendu.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg` : mutation de la file d'attente des votes à échec attendu.

## Propriétés

Invariants :
-`TypeInvariant`
-`CommitImpliesQuorum`
-`CommitImpliesStakeQuorum`
-`CommitImpliesDelivered`
-`DeliverImpliesEvidence`

Propriété temporelle :
- `EventuallyCommit` (`[] (gst => <> committed)`), avec équité post-TPS codée
  opérationnellement dans `Next` (protections de préemption de délai/défaut activées
  actions de progrès). Cela permet de conserver le modèle vérifiable avec Apalache 0.52.x, qui
  ne prend pas en charge les opérateurs d'équité `WF_` dans les propriétés temporelles vérifiées.

Invariants de récupération des frontières :
-`TypeInvariant`
-`CommitImpliesVoteQuorum`
-`CommitImpliesPayloadAvailability`
-`VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, qui exclut un terminal
  état post-TPS où `pending /\ voteBacked /\ ~committed` n'a pas de récupération,
  validation, retransmission, rotation ou transition de suppression limitée.Propriété temporelle de récupération de frontière :
- `PostGstVoteBackedFrontierEventuallyResolves` : après TPS, chaque non résolu
  L'État frontière en attente, soutenu par le vote, atteint finalement son engagement et sa charge utile
  récupération, retransmission du quorum, réancrage de la frontière future ou vue limitée
  rotation.
- `RecoveredPayloadEventuallyAdvances` : un État frontière soutenu par le vote et qui a
  récupéré, la charge utile ne peut pas rester en attente pour toujours sans validation,
  retransmission, réancrage ou rotation.
- `QuorumRetransmitEventuallyLeavesPending` : une fois la retransmission du quorum déclenchée
  pour un État frontière soutenu par le vote, l’emballage en attente doit finalement être effacé.
- `FutureFrontierEvidenceEventuallyReanchors` : preuve de frontière ultérieure/nouvelle vue
  doit soit effacer l'emballage en attente, soit être consommé comme réancre de frontière.

## Carte des hypothèses

Le modèle frontière est intentionnellement fini. Ce sont la mise en œuvre
les surfaces qu'il résume :| Notion de modèle | Surface de mise en œuvre |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | Gestion `PendingBlock` et contrôles de charge utile locale dans `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, ainsi que matérialisation de la propriété BlockCreated/frontier dans `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Comptage des votes par validation et contrôle de l'entrée des votes exercés par `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` et `reschedule_ignores_quorum_timeout_vote_queue_backlog` dans `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | État du propriétaire de frontière actif/périmé dans `frontier_slot_has_active_owner_state_for_view(...)`, rendement du propriétaire périmé dans `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` et remplacement du nettoyage dans `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Le quorum reprogrammé par le vote dans `reschedule_stale_pending_blocks_with_now(...)` ; la couverture de régression inclut `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Réparation exacte de la carrosserie Frontier et admission pour réparation RBC obsolète dans `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` et `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Sélection de cible de retransmission de quorum, `rebroadcast_pending_block_updates(...)` et appels déterministes de changement de vue dans `reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | Futures preuves de quorum nouvelle vue/frontière supérieure dans `on_pacemaker_propose_ready(...)`, couvertes par `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## En cours d'exécution

Depuis la racine du référentiel :

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Le coureur définit un Apalache `--length` explicite pour chaque mode :| Mode | Longueur | Utilisation prévue |
| --- | --- : | --- |
| `fast` | 10 | Vérification du chemin de validation CI |
| `deep` | 10 | Vérification plus importante du chemin de validation |
| `frontier-fast` | 10 | Contrôle aux frontières CI |
| `frontier-deep` | 12 | Contrôle aux frontières plus important |
| `frontier-wide` | 14 | Contrôle de stress aux frontières manuel/nuit |

`APALACHE_LENGTH=<n>` remplace la valeur par défaut par mode lors de l'exploration locale d'un
contre-exemple ou élargissement d’une preuve bornée.

### Configuration locale reproductible (aucun Docker requis)

Installez la chaîne d'outils Apalache locale épinglée utilisée par ce référentiel :

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Le programme d'exécution détecte automatiquement cette installation à :
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Après l'installation, `ci/check_sumeragi_formal.sh` devrait fonctionner sans variables d'environnement supplémentaires :

```bash
bash ci/check_sumeragi_formal.sh
```

Les mutations d’échec attendu sont intentionnellement en dehors de l’IC normal. Ils devraient
échouent sous Apalache et sont utiles lors du changement de modèle :

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Si Apalache n'est pas dans `PATH`, vous pouvez :

- définissez `APALACHE_BIN` sur le chemin de l'exécutable, ou
- utiliser le repli Docker (activé par défaut lorsque `docker` est disponible) :
  - image : `APALACHE_DOCKER_IMAGE` (par défaut `ghcr.io/apalache-mc/apalache:0.52.2`)
  - nécessite un démon Docker en cours d'exécution
  - désactiver le repli avec `APALACHE_ALLOW_DOCKER=0`.

Exemples :

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Remarques- Ce modèle complète (ne remplace pas) les tests de modèles Rust exécutables dans
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  et
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Les contrôles sont délimités par des valeurs constantes dans les fichiers `.cfg`.
- PR CI effectue ces contrôles dans `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.