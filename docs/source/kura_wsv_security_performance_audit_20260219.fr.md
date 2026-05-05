<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / Audit de sécurité et de performance WSV (2026-02-19)

## Portée

Cet audit a porté sur :

- Persistance Kura et chemins budgétaires : `crates/iroha_core/src/kura.rs`
- Chemins de production WSV/validation d'état/requête : `crates/iroha_core/src/state.rs`
- Surfaces hôtes simulées WSV IVM (portée test/développement) : `crates/ivm/src/mock_wsv.rs`

Hors de portée : caisses sans rapport et réexécutions de référence du système complet.

## Résumé des risques

- Critique : 0
- Haut : 4
- Moyen : 6
- Faible : 2

## Résultats (classés par gravité)

### Élevé

1. **L'écrivain Kura panique en cas de pannes d'E/S (risque de disponibilité des nœuds)**
- Composant : Kura
- Type : Sécurité (DoS), Fiabilité
- Détail : la boucle d'écriture panique sur les erreurs append/index/fsync au lieu de renvoyer des erreurs récupérables, de sorte que des pannes de disque transitoires peuvent mettre fin au processus du nœud.
- Preuve :
  -`crates/iroha_core/src/kura.rs:1697`
  -`crates/iroha_core/src/kura.rs:1724`
  -`crates/iroha_core/src/kura.rs:1845`
  -`crates/iroha_core/src/kura.rs:1854`
  -`crates/iroha_core/src/kura.rs:1860`
- Impact : le chargement à distance + la pression du disque local peuvent provoquer des boucles de crash/redémarrage.2. **L'expulsion de Kura effectue des réécritures complètes des données/index sous le mutex `block_store`**
- Composant : Kura
- Type : Performance, Disponibilité
- Détail : `evict_block_bodies` réécrit `blocks.data` et `blocks.index` via des fichiers temporaires tout en maintenant le verrou `block_store`.
- Preuve :
  - Acquisition de serrure : `crates/iroha_core/src/kura.rs:834`
  - Boucles de réécriture complète : `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Remplacement/synchronisation atomique : `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Impact : les événements d'expulsion peuvent bloquer les écritures/lectures pendant des périodes prolongées sur des historiques volumineux.

3. **La validation d'état contient un `view_lock` grossier lors d'un travail de validation intensif**
- Composant : Production WSV
- Type : Performance, Disponibilité
- Détail : la validation de bloc contient `view_lock` exclusif lors de la validation des transactions, des hachages de bloc et de l'état du monde, créant ainsi une famine du lecteur sous des blocs lourds.
- Preuve :
  - Le maintien du verrouillage commence : `crates/iroha_core/src/state.rs:17456`
  - Travail à l'intérieur de la serrure : `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Impact : des commits lourds et soutenus peuvent dégrader la réactivité des requêtes/consensus.4. **Les alias d'administrateur JSON IVM autorisent des mutations privilégiées sans vérification des appelants (hôte de test/dev)**
- Composant : hôte fictif WSV IVM
- Type : Sécurité (élévation de privilèges dans les environnements de test/dév)
- Détail : les gestionnaires d'alias JSON sont acheminés directement vers les méthodes de mutation de rôle/autorisation/homologue qui ne nécessitent pas de jetons d'autorisation au niveau de l'appelant.
- Preuve :
  - Alias d'administrateur : `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Mutateurs non activés : `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Note de portée dans la documentation du fichier (intention de test/dév) : `crates/ivm/src/mock_wsv.rs:295`
- Impact : les contrats/outils de test peuvent s'auto-élever et invalider les hypothèses de sécurité dans les harnais d'intégration.

### Moyen

5. **Les contrôles budgétaires Kura réencodent les blocs en attente sur chaque mise en file d'attente (O(n) par écriture)**
- Composant : Kura
- Type : Performances
- Détail : chaque mise en file d'attente recalcule les octets de file d'attente en attente en itérant les blocs en attente et en sérialisant chacun via un chemin de taille de fil canonique.
- Preuve :
  - Analyse de la file d'attente : `crates/iroha_core/src/kura.rs:2509`
  - Chemin d'encodage par bloc : `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Appelé en contrôle budgétaire en file d'attente : `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Impact : dégradation du débit d'écriture en raison du backlog.6. **Les contrôles budgétaires Kura effectuent des lectures répétées de métadonnées en mode bloc par mise en file d'attente**
- Composant : Kura
- Type : Performances
- Détail : chaque contrôle lit le nombre d'index durables et la longueur des fichiers tout en verrouillant `block_store`.
- Preuve :
  -`crates/iroha_core/src/kura.rs:2538`
  -`crates/iroha_core/src/kura.rs:2548`
  -`crates/iroha_core/src/kura.rs:2575`
- Impact : surcharge d'E/S/verrouillage évitable sur le chemin de mise en file d'attente à chaud.

7. **L'expulsion de Kura est déclenchée en ligne à partir du chemin budgétaire de mise en file d'attente**
- Composant : Kura
- Type : Performance, Disponibilité
- Détail : le chemin de mise en file d'attente peut appeler l'expulsion de manière synchrone avant d'accepter de nouveaux blocs.
- Preuve :
  - Chaîne d'appels en file d'attente : `crates/iroha_core/src/kura.rs:2050`
  - Appel d'expulsion en ligne : `crates/iroha_core/src/kura.rs:2603`
- Impact : pics de latence extrême sur l'ingestion de transactions/blocs lorsque le budget est proche.

8. **`State::view` peut revenir sans acquérir un verrouillage grossier en conflit**
- Composant : Production WSV
- Type : Compromis Cohérence/Performance
- Détail : en cas de conflit de verrouillage en écriture, la vue de repli `try_read` renvoie une vue sans protection grossière de par sa conception.
- Preuve :
  -`crates/iroha_core/src/state.rs:14543`
  -`crates/iroha_core/src/state.rs:14545`
  -`crates/iroha_core/src/state.rs:18301`
- Impact : amélioration de la vivacité, mais les appelants doivent tolérer une atomicité inter-composants plus faible en cas de conflit.9. **`apply_without_execution` utilise `expect` dur lors de l'avancement du curseur DA**
- Composant : Production WSV
- Type : Sécurité (DoS via panic-on-invariant-break), Fiabilité
- Détail : le bloc validé applique des paniques de chemin si les invariants d'avancement du curseur DA échouent.
- Preuve :
  -`crates/iroha_core/src/state.rs:17621`
  -`crates/iroha_core/src/state.rs:17625`
- Impact : les bogues latents de validation/indexation peuvent devenir des échecs de destruction de nœuds.

10. **L'appel système de publication IVM TLV n'a pas de limite explicite de taille d'enveloppe avant l'allocation (hôte de test/dév)**
- Composant : hôte fictif WSV IVM
- Type : Sécurité (mémoire DoS), Performance
- Détail : lit la longueur de l'en-tête, puis alloue/copie la charge utile TLV complète sans plafond au niveau de l'hôte dans ce chemin.
- Preuve :
  -`crates/ivm/src/mock_wsv.rs:3750`
  -`crates/ivm/src/mock_wsv.rs:3755`
  -`crates/ivm/src/mock_wsv.rs:3759`
- Impact : des charges utiles de tests malveillantes peuvent forcer des allocations importantes.

### Faible

11. **Le canal de notification Kura est illimité (`std::sync::mpsc::channel`)**
- Composant : Kura
- Type : Performances/Hygiène de la mémoire
- Détail : le canal de notification peut accumuler des événements de réveil redondants lors d'une pression soutenue du producteur.
- Preuve :
  -`crates/iroha_core/src/kura.rs:552`
- Impact : le risque de croissance de la mémoire est faible par taille d'événement mais évitable.12. **La file d'attente side-car du pipeline est illimitée en mémoire jusqu'à ce que l'enregistreur se vide**
- Composant : Kura
- Type : Performances/Hygiène de la mémoire
- Détail : la file d'attente side-car `push_back` n'a pas de plafond/contre-pression explicite.
- Preuve :
  -`crates/iroha_core/src/kura.rs:104`
  -`crates/iroha_core/src/kura.rs:3427`
- Impact : croissance potentielle de la mémoire lors de délais d'écriture prolongés.

## Couverture des tests existants et lacunes

### Kura

- Couverture existante :
  - comportement du budget de stockage : `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - régularité de l'expulsion et réhydratation : `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Lacunes :
  - pas de couverture d'injection de fautes pour la gestion des échecs append/index/fsync sans panique
  - pas de test de régression des performances pour les grandes files d'attente en attente et le coût de vérification du budget de mise en file d'attente
  - pas de test de latence d'expulsion de longue durée en cas de conflit de verrouillage

### Production WSV

- Couverture existante :
  - comportement de repli de contention : `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - sécurité de l'ordre de verrouillage autour du backend à plusieurs niveaux : `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Lacunes :
  - pas de test de contention quantitatif affirmant un temps de maintien de commit maximum acceptable sous des engagements mondiaux lourds
  - pas de test de régression pour une manipulation sans panique si les invariants d'avancement du curseur DA se cassent de manière inattendue

### IVM Hôte simulé WSV- Couverture existante :
  - autorisation sémantique de l'analyseur JSON et analyse homologue (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - smoke tests syscall autour du décodage TLV et du décodage JSON (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Lacunes :
  - aucun test de rejet d'alias d'administrateur non autorisé
  - pas de tests de rejet d'enveloppe TLV surdimensionnée dans `INPUT_PUBLISH_TLV`
  - pas de tests de référence/garde-corps autour du coût du clone de point de contrôle/restauration

## Plan de remédiation prioritaire

### Phase 1 (durcissement à fort impact)

1. Remplacez les branches `panic!` de l'écrivain Kura par une propagation d'erreur récupérable + une signalisation de santé dégradée.
- Fichiers cibles : `crates/iroha_core/src/kura.rs`
- Acceptation :
  - les échecs d'append/index/fsync injectés ne paniquent pas
  - les erreurs sont détectées grâce à la télémétrie/journalisation et l'écrivain reste contrôlable

2. Ajoutez des vérifications d’enveloppe délimitée pour la publication TLV de l’hôte fictif IVM et les chemins d’enveloppe JSON.
- Fichiers cibles : `crates/ivm/src/mock_wsv.rs`
- Acceptation :
  - les charges utiles surdimensionnées sont rejetées avant un traitement nécessitant beaucoup d'allocation
  - les nouveaux tests couvrent à la fois les cas surdimensionnés TLV et JSON

3. Appliquez des vérifications explicites des autorisations de l'appelant pour les alias d'administrateur JSON (ou les alias de porte derrière des indicateurs de fonctionnalité stricts de test uniquement et documentez clairement).
- Fichiers cibles : `crates/ivm/src/mock_wsv.rs`
- Acceptation :
  - l'appelant non autorisé ne peut pas muter le rôle/l'autorisation/l'état du pair via des alias

### Phase 2 (performances Hot-path)4. Rendre la comptabilité budgétaire Kura incrémentielle.
- Remplacez le recalcul complet de la file d'attente en attente par mise en file d'attente par des compteurs maintenus mis à jour lors de la mise en file d'attente/persist/drop.
- Acceptation :
  - coût de mise en file d'attente proche de O(1) pour le calcul des octets en attente
  - le test de régression montre une latence stable à mesure que la profondeur en attente augmente

5. Réduisez le temps de maintien du verrouillage d’expulsion.
- Options : compactage segmenté, copie fragmentée avec limites de déverrouillage ou mode de maintenance en arrière-plan avec blocage de premier plan limité.
- Acceptation :
  - la latence d'expulsion d'un historique important diminue et les opérations de premier plan restent réactives

6. Raccourcissez la section critique grossière `view_lock` lorsque cela est possible.
- Évaluez le fractionnement des phases de validation ou la capture instantanée des deltas par étapes pour minimiser les fenêtres de conservation exclusives.
- Acceptation :
  - Les mesures de contention démontrent un temps de maintien réduit de 99p en cas de validations de blocs importantes

### Phase 3 (Garde-corps opérationnels)

7. Introduire une signalisation de sillage délimitée/fusionnée pour l'écrivain Kura et la contre-pression/capuchons de file d'attente side-car.
8. Développez les tableaux de bord de télémétrie pour :
- Distributions attente/attente `view_lock`
- durée d'expulsion et octets récupérés par exécution
- latence de mise en file d'attente de vérification du budget

## Ajouts de tests suggérés1. `kura_writer_io_failures_do_not_panic` (unité, injection défaut)
2. `kura_budget_check_scales_with_pending_depth` (régression des performances)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (intégration/perf)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (régression des conflits)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (résilience)
6. `mock_wsv_admin_alias_requires_permissions` (régression de sécurité)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (garde DoS)
8. `mock_wsv_checkpoint_restore_cost_regression` (référence de performances)

## Notes sur la portée et la confiance

- Les résultats pour `crates/iroha_core/src/kura.rs` et `crates/iroha_core/src/state.rs` sont des résultats sur le chemin de production.
- Les résultats pour `crates/ivm/src/mock_wsv.rs` sont explicitement limités à l'hôte de test/dév, selon la documentation au niveau du fichier.
- Aucune modification de version ABI n'est requise par cet audit lui-même.