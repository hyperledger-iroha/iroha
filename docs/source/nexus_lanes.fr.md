---
lang: fr
direction: ltr
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de lanes Nexus et partition WSV

> **Statut:** livrable NX-1 - la taxonomie des lanes, la geometrie de configuration et le layout de storage sont prets pour implementation.  
> **Responsables:** Nexus Core WG, Governance WG  
> **Element de roadmap associe:** NX-1

Ce document capture l architecture cible pour la couche de consensus multi-lane de Nexus. L objectif est de produire un etat mondial deterministe unique tout en permettant a des data spaces (lanes) individuels d executer des ensembles de validateurs publics ou prives avec des workloads isoles.

> **Preuves cross-lane:** Cette note se concentre sur la geometrie et le storage. Les commitments de settlement par lane, le pipeline de relay et les preuves de merge-ledger requises pour la roadmap **NX-4** sont detaillees dans [nexus_cross_lane.md](nexus_cross_lane.md).

## Concepts

- **Lane:** shard logique du ledger Nexus avec son propre ensemble de validateurs et backlog d execution. Identifie par un `LaneId` stable.
- **Data Space:** bucket de gouvernance regroupant une ou plusieurs lanes partageant des politiques de compliance, routage et settlement. Chaque dataspace declare aussi `fault_tolerance (f)` utilise pour dimensionner les comites de relay de lane (`3f+1`).
- **Lane Manifest:** metadonnees controlees par la gouvernance decrivant validateurs, politique DA, token de gas, regles de settlement et permissions de routage.
- **Global Commitment:** preuve emise par une lane resumant les nouvelles racines d etat, les donnees de settlement et d eventuels transferts cross-lane. L anneau NPoS global ordonne les commitments.

## Taxonomie des lanes

Les types de lane decrivent canoniquement leur visibilite, surface de gouvernance et hooks de settlement. La geometrie de configuration (`LaneConfig`) capture ces attributs pour que les noeuds, SDKs et tooling puissent raisonner sur le layout sans logique bespoke.

| Type de lane | Visibilite | Membresie validateurs | Exposition WSV | Gouvernance par defaut | Politique de settlement | Usage typique |
|-------------|------------|-----------------------|----------------|------------------------|-------------------------|-------------|
| `default_public` | public | Permissionless (stake global) | Replique complete d etat | Parlement SORA | `xor_global` | Ledger public de base |
| `public_custom` | public | Permissionless ou stake-gated | Replique complete d etat | Module pondere par stake | `xor_lane_weighted` | Applications publiques a haut debit |
| `private_permissioned` | restreint | Ensemble fixe de validateurs (approuve par gouvernance) | Commitments et proofs | Conseil federatif | `xor_hosted_custody` | CBDC, workloads de consortium |
| `hybrid_confidential` | restreint | Membresie mixte; enrobe des preuves ZK | Commitments + divulgation selective | Module monnaie programmable | `xor_dual_fund` | Monnaie programmable preservant la confidentialite |

Tous les types de lane doivent declarer:

- Alias de dataspace - regroupement lisible liant les politiques de compliance.
- Handle de gouvernance - identifiant resolu via `Nexus.governance.modules`.
- Handle de settlement - identifiant consomme par le settlement router pour debiter les buffers XOR.
- Metadonnees telemetrie optionnelles (description, contact, domaine metier) exposees via `/status` et dashboards.

## Geometrie de configuration des lanes (`LaneConfig`)

`LaneConfig` est la geometrie runtime derivee du catalogue de lanes valide. Il ne remplace pas les manifests de gouvernance; il fournit des identifiants de storage deterministes et des hints de telemetrie pour chaque lane configuree.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` recalcule la geometrie a chaque chargement de configuration (`State::set_nexus`).
- Les aliases sont sanitises en slugs minuscules; les caracteres non alphanumeriques consecutifs se compressent en `_`. Si l alias produit un slug vide, on retombe sur `lane{id}`.
- Les prefixes de cle garantissent que le WSV garde des plages par lane disjointes meme si le backend est partage.
- `shard_id` est derive de la cle metadata du catalogue `da_shard_id` (par defaut `lane_id`) et pilote le journal persistant du curseur de shard pour garder un replay DA deterministe apres redemarrage/resharding.
- Les noms de segments Kura sont deterministes entre hosts; les auditeurs peuvent verifier les repertoires et manifests sans tooling bespoke.
- Les segments merge (`lane_{id:03}_merge`) contiennent les dernieres racines merge-hint et les commitments d etat global pour cette lane.
- Quand la gouvernance renomme un alias de lane, les noeuds re-etiquettent automatiquement les repertoires `blocks/lane_{id:03}_{slug}` (et les snapshots en couches) afin que les auditeurs voient toujours le slug canonique sans nettoyage manuel.

## Partition du world-state

- Le world state logique de Nexus est l union des espaces d etat par lane. Les lanes publiques persistent l etat complet; les lanes privees/confidentielles exportent des racines Merkle/commitment vers le merge ledger.
- Le stockage MV prefixe chaque cle avec le prefixe 4 octets de la lane provenant de `LaneConfigEntry::key_prefix`, produisant des cles comme `[00 00 00 01] ++ PackedKey`.
- Les tables partagees (comptes, assets, triggers, enregistrements de gouvernance) stockent donc des entrees groupees par prefixe de lane, gardant des scans de plage deterministes.
- Les metadonnees du merge-ledger suivent le meme layout: chaque lane ecrit les racines merge-hint et les racines d etat global reduites dans `lane_{id:03}_merge`, permettant une retention ou eviction ciblee quand une lane se retire.
- Les index cross-lane (aliases de comptes, registres d assets, manifests de gouvernance) stockent des paires explicites `(LaneId, DataSpaceId)`. Ces index vivent dans des column families partagees mais utilisent le prefixe de lane et les ids de dataspace explicites pour garder des lookups deterministes.
- Le workflow de merge combine donnees publiques et commitments prives via des tuples `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` derives des entrees du merge-ledger.

## Partition Kura & WSV

- **Segments Kura**
  - `lane_{id:03}_{slug}` - segment principal de blocs pour la lane (blocs, index, receipts).
  - `lane_{id:03}_merge` - segment merge-ledger enregistrant les racines d etat reduites et les artefacts de settlement.
  - Les segments globaux (evidence de consensus, caches telemetrie) restent partages car ils sont neutres lane; leurs cles n incluent pas de prefixe de lane.
- Le runtime surveille les mises a jour du catalogue de lanes: les nouvelles lanes voient leurs repertoires de blocs et merge-ledger provisionnes automatiquement sous `kura/blocks/` et `kura/merge_ledger/`, tandis que les lanes retirees sont archivees sous `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- Les snapshots d etat en couches suivent le meme cycle; chaque lane ecrit dans `<cold_root>/lanes/lane_{id:03}_{slug}`, ou `<cold_root>` est `cold_store_root` (ou `da_store_root` lorsque `cold_store_root` est absent), et les retraits migrent l arbre de repertoires vers `<cold_root>/retired/lanes/`.
- **Prefixes de cle** - le prefixe 4 octets calcule depuis `LaneId` est toujours prepend a des cles MV encodees. Aucun hashing specifique a l host n est utilise, donc l ordre est identique entre noeuds.
- **Layout de block log** - donnees de block, index et hashes sont imbriques sous `kura/blocks/lane_{id:03}_{slug}/`. Les journaux merge-ledger reutilisent le meme slug (`kura/merge/lane_{id:03}_{slug}.log`), gardant des flux de recovery par lane isoles.
- **Politique de retention** - les lanes publiques retiennent les corps de blocs complets; les lanes a commitments seulement peuvent compacter les corps anciens apres checkpoints car les commitments font foi. Les lanes confidentielles gardent des journals chiffres dans des segments dedies pour ne pas bloquer les autres workloads.
- **Tooling** - `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` inspecte `<store>/blocks` et `<store>/merge_ledger` en utilisant le `LaneConfig` derive, reporte les segments actifs vs retires, et archive les repertoires/logs retires sous `<store>/retired/...` pour garder des preuves deterministes. Les utilitaires de maintenance (`kagami`, commandes admin CLI) doivent reutiliser le namespace slugge pour exposer des metriques, labels Prometheus ou archiver des segments Kura.

## Budgets de stockage

- `nexus.storage.max_disk_usage_bytes` définit le budget total sur disque que les nœuds Nexus doivent consommer entre Kura, snapshots WSV froids, stockage SoraFS et spools de streaming (SoraNet/SoraVPN).
- `nexus.storage.max_wsv_memory_bytes` plafonne la couche chaude WSV en propageant le dimensionnement déterministe du payload Norito dans `tiered_state.hot_retained_bytes` ; la rétention de grâce peut temporairement dépasser le budget, mais le dépassement est observable via télémétrie (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` répartit le budget disque entre composants en points de base (doit sommer à 10 000). Les plafonds dérivés s’appliquent à `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes` et `streaming.soravpn.provision_spool_max_bytes`.
- L’application du budget de Kura additionne les octets du block-store sur les segments de lanes actives et retirées et inclut les blocs en file d’attente non encore persistés afin d’éviter les dépassements pendant la latence d’écriture.
- Les spools de provisionnement SoraVPN utilisent les réglages `streaming.soravpn` et sont plafonnés indépendamment du spool de provisionnement SoraNet.
- Les limites par composant restent actives : lorsqu’un composant a un plafond explicite non nul, le plus petit entre ce plafond et le budget Nexus dérivé est appliqué.
- La télémétrie de budget utilise `storage_budget_bytes_used{component=...}` et `storage_budget_bytes_limit{component=...}` pour rapporter l’usage/les plafonds de `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool` et `soravpn_spool` ; `storage_budget_exceeded_total{component=...}` s’incrémente lorsque l’enforcement rejette de nouvelles données et les logs émettent un avertissement opérateur.
- Kura reporte la même comptabilité utilisée à l’admission (octets sur disque plus blocs en file d’attente, y compris les payloads merge-ledger lorsqu’ils sont présents), de sorte que les jauges reflètent la pression effective plutôt que les seuls octets persistés.

## Routage et APIs

- Les endpoints REST/gRPC Torii acceptent un `lane_id` optionnel; l absence implique `lane_default`.
- Les SDKs exposent des selecteurs de lane et mappent des aliases conviviaux vers `LaneId` via le catalogue de lanes.
- Les regles de routage operent sur le catalogue valide et peuvent choisir lane et dataspace. `LaneConfig` fournit des aliases telemetrie-friendly pour dashboards et logs.

## Settlement et fees

- Chaque lane paie des fees XOR au set global de validateurs. Les lanes peuvent collecter des tokens de gas natifs mais doivent escrow des equivalents XOR avec les commitments.
- Les preuves de settlement incluent montant, metadata de conversion et preuve d escrow (ex: transfert vers la vault globale de fees).
- Le settlement router unifie (NX-3) debite les buffers en utilisant les memes prefixes de lane, afin que la telemetrie de settlement s aligne avec la geometrie de storage.

## Gouvernance

- Les lanes declarent leur module de gouvernance via le catalogue. `LaneConfigEntry` transporte l alias et le slug originaux pour garder telemetrie et audits lisibles.
- Le registre Nexus distribue des manifests de lane signes qui incluent `LaneId`, binding dataspace, handle de gouvernance, handle de settlement et metadata.
- Les hooks de runtime-upgrade continuent d appliquer les politiques de gouvernance (`gov_upgrade_id` par defaut) et consignent les diffs via le pont telemetrie (events `nexus.config.diff`).
- Les manifests de lane definissent le pool de validateurs dataspace pour les lanes administrees; les lanes elues par stake derivent leur pool de validateurs depuis les enregistrements de staking des lanes publiques.

## Telemetrie et status

- `/status` expose les aliases de lane, bindings dataspace, handles de gouvernance et profils de settlement, derives du catalogue et de `LaneConfig`.
- Les metriques du scheduler (`nexus_scheduler_lane_teu_*`) rendent les aliases/slugs de lane afin que les operateurs puissent mapper backlog et pression TEU rapidement.
- `nexus_lane_configured_total` compte le nombre d entrees de lane derivees et est recalcule lorsque la configuration change. La telemetrie emet des diffs signes quand la geometrie des lanes change.
- Les gauges de backlog dataspace incluent les metadonnees d alias/description pour aider les operateurs a associer la pression de queue aux domaines metier.

## Configuration et types Norito

- `LaneCatalog`, `LaneConfig` et `DataSpaceCatalog` vivent dans `iroha_data_model::nexus` et fournissent des structures au format Norito pour manifests et SDKs.
- `LaneConfig` vit dans `iroha_config::parameters::actual::Nexus` et est derive automatiquement du catalogue; il ne requiert pas d encodage Norito car c est un helper runtime interne.
- La configuration cote utilisateur (`iroha_config::parameters::user::Nexus`) continue d accepter des descripteurs declaratifs de lanes et dataspaces; le parsing derive maintenant la geometrie et rejette les aliases invalides ou les ids de lane dupliques.
- `DataSpaceMetadata.fault_tolerance` controle le dimensionnement des comites de relay de lane; la membresie est echantillonnee de maniere deterministe par epoque depuis le pool de validateurs du dataspace en utilisant la graine VRF liee a `(dataspace_id, lane_id)`.

## Travaux restants

- Integrer les mises a jour du settlement router (NX-3) avec la nouvelle geometrie afin que les debits de buffers XOR et les receipts soient etiquetes par slug de lane.
- Finaliser l algorithme de merge (ordering, pruning, detection de conflits) et attacher des fixtures de regression pour le replay cross-lane.
- Ajouter des hooks de compliance pour whitelists/blacklists et politiques de monnaie programmable (suivi sous NX-12).

---

*Ce document evoluera a mesure que les taches NX-2 a NX-18 progressent. Merci de capturer les questions ouvertes dans le roadmap ou le tracker de gouvernance.*
