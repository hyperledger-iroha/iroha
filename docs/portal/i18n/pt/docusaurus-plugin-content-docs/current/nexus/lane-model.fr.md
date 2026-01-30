---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-lane-model
title: Modele de lanes Nexus
description: Taxonomie logique des lanes, geometrie de configuration et regles de merge du world-state pour Sora Nexus.
---

# Modele de lanes Nexus et partitionnement WSV

> **Statut:** livrable NX-1 - taxonomie de lanes, geometrie de configuration et layout de stockage prets pour implementation.  
> **Owners:** Nexus Core WG, Governance WG  
> **Reference roadmap:** NX-1 dans `roadmap.md`

Cette page du portail reflete le brief canonique `docs/source/nexus_lanes.md` afin que les operateurs Sora Nexus, owners SDK et relecteurs puissent lire la guidance lanes sans explorer l arbre mono-repo. L architecture cible garde le world state deterministe tout en permettant aux data spaces (lanes) d executer des ensembles de validateurs publics ou prives avec des workloads isoles.

## Concepts

- **Lane:** shard logique du ledger Nexus avec son propre ensemble de validateurs et backlog d execution. Identifie par un `LaneId` stable.
- **Data Space:** bucket de gouvernance regroupant une ou plusieurs lanes qui partagent les politiques de compliance, routing et settlement.
- **Lane Manifest:** metadata controlee par la gouvernance decrivant validateurs, politique DA, token de gas, regles de settlement et permissions de routing.
- **Global Commitment:** preuve emise par une lane qui resume les nouveaux roots d etat, les donnees de settlement et les transferts cross-lane optionnels. L anneau NPoS global ordonne les commitments.

## Taxonomie des lanes

Les types de lane decrivent canoniquement leur visibilite, surface de gouvernance et hooks de settlement. La geometrie de configuration (`LaneConfig`) capture ces attributs afin que les noeuds, SDKs et tooling puissent raisonner sur le layout sans logique sur mesure.

| Type de lane | Visibilite | Membreship des validateurs | Exposition WSV | Gouvernance par defaut | Politique de settlement | Usage typique |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Replica d etat complete | SORA Parliament | `xor_global` | Ledger public de base |
| `public_custom` | public | Permissionless ou stake-gated | Replica d etat complete | Module pondere par stake | `xor_lane_weighted` | Applications publiques a haut throughput |
| `private_permissioned` | restricted | Ensemble fixe de validateurs (approuve par gouvernance) | Commitments et proofs | Federated council | `xor_hosted_custody` | CBDC, workloads de consortium |
| `hybrid_confidential` | restricted | Membreship mixte; enrobe des ZK proofs | Commitments + disclosure selective | Module de monnaie programmable | `xor_dual_fund` | Monnaie programmable preservant la privacy |

Tous les types de lane doivent declarer:

- Alias de dataspace - regroupement lisible par humains liant les politiques de compliance.
- Handle de gouvernance - identifiant resolu via `Nexus.governance.modules`.
- Handle de settlement - identifiant consomme par le settlement router pour debiter les buffers XOR.
- Metadata de telemetrie optionnelle (description, contact, domaine business) exposee via `/status` et dashboards.

## Geometrie de configuration des lanes (`LaneConfig`)

`LaneConfig` est la geometrie runtime derivee du catalogue de lanes valide. Il ne remplace pas les manifests de gouvernance; il fournit plutot des identifiants de stockage deterministes et des hints de telemetrie pour chaque lane configuree.

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

- `LaneConfig::from_catalog` recalcule la geometrie quand la configuration est chargee (`State::set_nexus`).
- Les aliases sont sanitises en slugs en minuscules; les caracteres non alphanumeriques consecutifs se compressent en `_`. Si l alias produit un slug vide, on revient a `lane{id}`.
- `shard_id` est derive de la metadata key `da_shard_id` (par defaut `lane_id`) et pilote le journal de curseur de shard persiste afin de garder la relecture DA deterministe entre redemarrages/resharding.
- Les prefixes de cle garantissent que le WSV maintient des plages de cles par lane disjointes meme si le backend est partage.
- Les noms de segments Kura sont deterministes entre hosts; les auditeurs peuvent verifier les repertoires de segment et les manifests sans tooling sur mesure.
- Les segments merge (`lane_{id:03}_merge`) stockent les derniers roots de merge-hint et commitments d etat global pour cette lane.

## Partitionnement du world-state

- Le world state logique de Nexus est l union des espaces d etat par lane. Les lanes publiques persistent l etat complet; les lanes privees/confidential exportent des roots Merkle/commitment vers le merge ledger.
- Le stockage MV prefixe chaque cle avec le prefixe 4 octets de `LaneConfigEntry::key_prefix`, produisant des cles comme `[00 00 00 01] ++ PackedKey`.
- Les tables partagees (accounts, assets, triggers, enregistrements de gouvernance) stockent des entrees groupees par prefixe de lane, gardant les range scans deterministes.
- La metadata du merge-ledger reflete le meme layout: chaque lane ecrit des roots de merge-hint et des roots d etat global reduit dans `lane_{id:03}_merge`, permettant une retention ou eviction ciblee quand une lane se retire.
- Les indexes cross-lane (account aliases, asset registries, governance manifests) stockent des prefixes de lane explicites pour que les operateurs reconcillent rapidement les entrees.
- **Politique de retention** - les lanes publiques conservent les block bodies complets; les lanes commitments only peuvent compacter les bodies anciens apres checkpoints car les commitments sont autoritatifs. Les lanes confidential gardent des journals chiffres dans des segments dedies pour ne pas bloquer d autres workloads.
- **Tooling** - les utilitaires de maintenance (`kagami`, commandes admin CLI) doivent referencer le namespace avec slug lors de l exposition des metriques, labels Prometheus ou archivage des segments Kura.

## Routing et APIs

- Les endpoints Torii REST/gRPC acceptent un `lane_id` optionnel; l absence implique `lane_default`.
- Les SDKs exposent des selecteurs de lane et mappent des aliases conviviaux vers `LaneId` en utilisant le catalogue de lanes.
- Les regles de routing operent sur le catalogue valide et peuvent choisir lane et dataspace. `LaneConfig` fournit des aliases friendly pour la telemetrie dans dashboards et logs.

## Settlement et frais

- Chaque lane paie des frais XOR au set global de validateurs. Les lanes peuvent collecter des tokens de gas natifs mais doivent escrow des equivalents XOR avec les commitments.
- Les proofs de settlement incluent le montant, la metadata de conversion et la preuve d escrow (par exemple, transfert vers le vault global de frais).
- Le settlement router unifie (NX-3) debite les buffers en utilisant les memes prefixes de lane, donc la telemetrie de settlement s aligne avec la geometrie de stockage.

## Gouvernance

- Les lanes declarent leur module de gouvernance via le catalogue. `LaneConfigEntry` porte l alias et le slug d origine pour garder la telemetrie et les audit trails lisibles.
- Le registry Nexus distribue des lane manifests signes qui incluent `LaneId`, binding de dataspace, handle de gouvernance, handle de settlement et metadata.
- Les hooks de runtime-upgrade continuent d appliquer les politiques de gouvernance (`gov_upgrade_id` par defaut) et journalisent les diffs via le telemetry bridge (events `nexus.config.diff`).

## Telemetrie et status

- `/status` expose les aliases de lane, bindings de dataspace, handles de gouvernance et profils de settlement, derives du catalogue et de `LaneConfig`.
- Les metriques du scheduler (`nexus_scheduler_lane_teu_*`) affichent aliases/slugs afin que les operateurs puissent mapper rapidement backlog et pression TEU.
- `nexus_lane_configured_total` compte le nombre d entrees de lane derivees et est recalcule quand la configuration change. La telemetrie emet des diffs signes quand la geometrie des lanes change.
- Les gauges de backlog dataspace incluent la metadata alias/description pour aider les operateurs a associer la pression de file a des domaines business.

## Configuration et types Norito

- `LaneCatalog`, `LaneConfig`, et `DataSpaceCatalog` vivent dans `iroha_data_model::nexus` et fournissent des structures au format Norito pour les manifests et SDKs.
- `LaneConfig` vit dans `iroha_config::parameters::actual::Nexus` et est derive automatiquement du catalogue; il ne requiert pas d encodage Norito car c est un helper runtime interne.
- La configuration cote utilisateur (`iroha_config::parameters::user::Nexus`) continue d accepter des descripteurs declaratifs de lane et dataspace; le parsing derive maintenant la geometrie et rejette les aliases invalides ou les IDs de lane dupliques.

## Travail restant

- Integrer les updates du settlement router (NX-3) avec la nouvelle geometrie afin que les debits et recus de buffer XOR soient etiquetes par slug de lane.
- Etendre le tooling admin pour lister les column families, compacter les lanes retirees et inspecter les logs de blocs par lane via le namespace slugge.
- Finaliser l algorithme de merge (ordering, pruning, conflict detection) et attacher des fixtures de regression pour le replay cross-lane.
- Ajouter des hooks de compliance pour whitelists/blacklists et politiques de monnaie programmable (suivi sous NX-12).

---

*Cette page continuera de suivre les follow-ups NX-1 a mesure que NX-2 jusqu a NX-18 atterrissent. Merci de remonter les questions ouvertes dans `roadmap.md` ou le tracker de gouvernance afin que le portail reste aligne avec les docs canoniques.*
