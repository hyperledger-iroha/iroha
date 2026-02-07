---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : Modèle de voies Nexus
description : Taxonomie logique des voies, géométrie de configuration et règles de fusion du monde-état pour Sora Nexus.
---

# Modele de voies Nexus et partitionnement WSV

> **Statut :** livrable NX-1 - taxonomie de voies, géométrie de configuration et disposition de stockage prête pour l'implémentation.  
> **Propriétaires :** Nexus GT principal, GT sur la gouvernance  
> **Feuille de route de référence :** NX-1 dans `roadmap.md`

Cette page du portail reflète le bref canonique `docs/source/nexus_lanes.md` afin que les opérateurs Sora Nexus, propriétaires SDK et rélecteurs puissent lire la guidance lanes sans explorer l'arbre mono-repo. L'architecture cible garde le déterministe de l'état mondial tout en permettant aux espaces de données (voies) d'exécuter des ensembles de validateurs publics ou privés avec des isoles de charges de travail.

##Concept- **Lane:** shard logique du ledger Nexus avec son propre ensemble de validateurs et backlog d'exécution. Identifier par un `LaneId` stable.
- **Data Space :** bucket de gouvernance regroupant une ou plusieurs voies qui partagent les politiques de conformité, de routage et de règlement.
- **Lane Manifest :** métadonnées contrôlées par la gouvernance décrivant les validateurs, la politique DA, le token de gas, les règles de règlement et les autorisations de routage.
- **Engagement Global :** preuve emise par une voie qui reprend les nouvelles racines d'état, les données de règlement et les transferts optionnels cross-lane. L'anneau NPoS global ordonne les engagements.

## Taxonomie des voies

Les types de voie décrivent canoniquement leur visibilité, surface de gouvernance et crochets de règlement. La géométrie de configuration (`LaneConfig`) capture ces attributs afin que les noeuds, SDK et outillage puissent raisonner sur la mise en page sans logique sur mesure.| Type de voie | Visibilité | Adhésion des validateurs | Exposition WSV | Gouvernance par défaut | Politique de règlement | Utilisation typique |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | publique | Sans autorisation (enjeu mondial) | Réplique d'état complète | SORA Parlement | `xor_global` | Grand livre public de base |
| `public_custom` | publique | Sans autorisation ou avec enjeux | Réplique d'état complète | Module pondérer par enjeu | `xor_lane_weighted` | Applications publiques à haut débit |
| `private_permissioned` | restreint | Ensemble fixe de validateurs (approuvé par gouvernance) | Engagements et preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, charges de travail du consortium |
| `hybrid_confidential` | restreint | Adhésion mixte; enrober des épreuves ZK | Engagements + divulgation sélective | Module de monnaie programmable | `xor_dual_fund` | Monnaie programmable préservant la vie privée |

Tous les types de voie doivent déclarer :

- Alias de dataspace - regroupement lisible par humains liant les politiques de conformité.
- Handle de gouvernance - identifiant résolu via `Nexus.governance.modules`.
- Handle de règlement - identifiant utilisé par le routeur de règlement pour débiter les tampons XOR.
- Métadonnées de télémétrie optionnelle (description, contact, domaine métier) exposées via `/status` et tableaux de bord.## Géométrie de configuration des voies (`LaneConfig`)

`LaneConfig` est la géométrie runtime dérivée du catalogue de voies valide. Il ne remplace pas les manifestes de gouvernance; il fournit plutot des identifiants de stockage déterministes et des conseils de télémétrie pour chaque voie configurée.

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

- `LaneConfig::from_catalog` recalcule la géométrie lorsque la configuration est chargée (`State::set_nexus`).
- Les alias sont sanitises en slugs en minuscules; les caractères non alphanumériques consécutifs se compressent en `_`. Si l'alias produit un slug vide, on revient à `lane{id}`.
- `shard_id` est dérivé de la clé de métadonnées `da_shard_id` (par défaut `lane_id`) et pilote le journal de curseur de shard persiste afin de garder la relecture DA déterministe entre redemarrages/resharding.
- Les préfixes de clé garantissent que le WSV maintient des plages de clés par voie disjointes même si le backend est partagé.
- Les noms de segments Kura sont déterministes entre hôtes ; les auditeurs peuvent vérifier les répertoires de segments et les manifestes sans outillage sur mesure.
- Les segments merge (`lane_{id:03}_merge`) stockent les dernières racines de merge-hint et engagements d'état global pour cette voie.

## Partitionnement du monde-état- Le world state logique de Nexus est l'union des espaces d'état par voie. Les voies publiques persistantes l'état complet; les voies privées/confidentiel exportent des racines Merkle/engagement vers le merge ledger.
- Le stockage MV préfixe chaque clé avec le préfixe 4 octets de `LaneConfigEntry::key_prefix`, produisant des clés comme `[00 00 00 01] ++ PackedKey`.
- Les tables partagées (comptes, actifs, déclencheurs, enregistrements de gouvernance) stockent des entrées groupées par préfixe de voie, gardant les range scans déterministes.
- Les métadonnées du merge-ledger reflètent le meme layout : chaque voie écrite des racines de merge-hint et des racines d'état global réduit dans `lane_{id:03}_merge`, permettant une rétention ou une expulsion ciblée lorsqu'une voie se retire.
- Les index cross-lane (alias de compte, registres d'actifs, manifestes de gouvernance) stockent des préfixes de voie explicites pour que les opérateurs concilient rapidement les entrées.
- **Politique de rétention** - les voies publiques conservent les corps de bloc complets; les voies d'engagements uniquement peuvent compacter les corps anciens après les points de contrôle car les engagements sont autoritatifs. Les voies confidentielles gardent des journaux chiffres dans des segments dédiés pour ne pas bloquer d’autres charges de travail.- **Tooling** - les utilitaires de maintenance (`kagami`, commandes admin CLI) doivent référencer l'espace de noms avec slug lors de l'exposition des métriques, labels Prometheus ou archivage des segments Kura.

## Routage et API

- Les points de terminaison Torii REST/gRPC acceptent un `lane_id` optionnel ; l absence implique `lane_default`.
- Les SDK exposent des sélecteurs de voie et mappent des alias conviviaux vers `LaneId` en utilisant le catalogue de voies.
- Les règles de routage en vigueur sur le catalogue valides et peuvent choisir lane et dataspace. `LaneConfig` fournit des alias conviviaux pour la télémétrie dans les tableaux de bord et les journaux.

## Règlement et frais

- Chaque voie paie des frais XOR au set global de validateurs. Les voies peuvent collecter des jetons de gaz natifs mais doivent escrow des équivalents XOR avec les engagements.
- Les preuves de règlement incluent le montant, la métadonnée de conversion et la preuve d'entiercement (par exemple, transfert vers le vault global de frais).
- Le routeur de règlement unifié (NX-3) débite les buffers en utilisant les mèmes préfixes de voie, donc la télémétrie de règlement s'aligne avec la géométrie de stockage.

## Gouvernance- Les voies déclarent leur module de gouvernance via le catalogue. `LaneConfigEntry` porte l'alias et le slug d'origine pour garder la télémétrie et les pistes d'audit lisibles.
- Le registre Nexus distribue des signes manifestes de voie qui incluent `LaneId`, liaison de dataspace, handle de gouvernance, handle de règlement et métadonnées.
- Les hooks de runtime-upgrade continuent d'appliquer les politiques de gouvernance (`gov_upgrade_id` par défaut) et journalisent les diffs via le pont de télémétrie (events `nexus.config.diff`).

## Télémétrie et statut

- `/status` expose les alias de lane, les liaisons de dataspace, les handles de gouvernance et les profils de règlement, dérive du catalogue et de `LaneConfig`.
- Les métriques du planificateur (`nexus_scheduler_lane_teu_*`) affichent des alias/slugs afin que les opérateurs puissent mapper rapidement le backlog et pression TEU.
- `nexus_lane_configured_total` compte le nombre d'entrées de voie dérivées et est recalculé quand la configuration change. La télémétrie émet des signes différents lorsque la géométrie des voies change.
- Les jauges de backlog dataspace incluent la métadonnée alias/description pour aider les opérateurs à associer la pression de fichier à des domaines métier.

## Configuration et types Norito- `LaneCatalog`, `LaneConfig`, et `DataSpaceCatalog` vivent dans `iroha_data_model::nexus` et fourniture des structures au format Norito pour les manifestes et SDK.
- `LaneConfig` vit dans `iroha_config::parameters::actual::Nexus` et est dérive automatiquement du catalogue ; il ne nécessite pas d'encodage Norito car c'est un helper runtime interne.
- La configuration côté utilisateur (`iroha_config::parameters::user::Nexus`) continue d'accepter des descripteurs déclaratifs de lane et dataspace; le parsing dérive maintenant la géométrie et rejette les alias invalides ou les ID de voie dupliqués.

## Travail restant

- Intégrer les mises à jour du routeur de règlement (NX-3) avec la nouvelle géométrie afin que les débits et recus de buffer XOR soient étiquettes par slug de lane.
- Etendre l'administrateur outillage pour lister les familles de colonnes, compacter les voies retirées et inspecter les logs de blocs par voie via le namespace slugge.
- Finaliser l'algorithme de fusion (ordonnancement, élagage, détection de conflits) et attacher des luminaires de régression pour le replay cross-lane.
- Ajout des hooks de conformité pour whitelists/blacklists et politiques de monnaie programmables (suivi sous NX-12).

---*Cette page continue de suivre les suivis NX-1 a mesure que NX-2 jusqu'à a NX-18 atterrissent. Merci de remonter les questions ouvertes dans `roadmap.md` ou le tracker de gouvernance afin que le portail reste aligné avec les docs canoniques.*