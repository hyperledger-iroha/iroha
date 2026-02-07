---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : Modèle de voies de Nexus
description : Taxonomie logique des voies, géométrie de configuration et règles de fusion de l'état mondial pour Sora Nexus.
---

# Modèle de voies de Nexus et cloisonné de WSV

> **État :** NX-1 intégré - taxonomie des voies, géométrie de configuration et disposition des listes de stockage pour la mise en œuvre.  
> **Propriétaires :** Nexus GT principal, GT sur la gouvernance  
> **Référence de feuille de route :** NX-1 en `roadmap.md`

Cette page du portail reflète le bref canonique `docs/source/nexus_lanes.md` pour les opérateurs de Sora Nexus, les propriétaires du SDK et les réviseurs peuvent lire le guide des voies sans entrer dans l'arbol mono-repo. L'objet architectural maintient l'état mondial dans des conditions déterministes, ce qui permet aux espaces de données (voies) individuels d'être exécutés conjointement par des validateurs publics ou privés avec des charges de travail réparties.

## Concepts- **Lane :** fragment logique du grand livre de Nexus avec votre propre ensemble de validateurs et le retard d'exécution. Identifié par un `LaneId` établi.
- **Espace de données :** compartiment de gouvernance qui regroupe un ou plusieurs axes qui partagent les politiques de conformité, de routage et de règlement.
- **Lane Manifest :** métadonnées contrôlées par la gouvernance qui décrivent les validateurs, la politique de DA, les jetons de gaz, les règles de règlement et les autorisations de routage.
- **Engagement mondial :** preuve émise pour une voie qui reprend les nouvelles racines de l'état, les données de règlement et les transferts optionnels entre voies. El anillo NPoS global ordena engagements.

## Taxonomie des voies

Les types de voies décrivent la forme canonique de la visibilité, de la surface de gouvernance et des crochets de règlement. La géométrie de configuration (`LaneConfig`) capture ces attributs pour que les nœuds, les SDK et les outils puissent définir la mise en page sans logique sur mesure.| Type de voie | Visibilité | Membres des validateurs | Exposition WSV | Gouvernement par défaut | Politique de règlement | Utilisation typique |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | publique | Sans autorisation (enjeu mondial) | Réplique de l'état complet | SORA Parlement | `xor_global` | Base de données du grand livre |
| `public_custom` | publique | Sans autorisation ou avec enjeu | Réplique de l'état complet | Module réfléchi par enjeu | `xor_lane_weighted` | Applications publiques à haut débit |
| `private_permissioned` | restreint | Set fijo de validadores (approuvé par la gouvernance) | Engagements et preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, charges de travail du consortium |
| `hybrid_confidential` | restreint | Membresia mixta; envelopper les épreuves ZK | Engagements + divulgation sélective | Module d'argent programmable | `xor_dual_fund` | Dinero programmable avec préservation de la confidentialité |

Tous les types de voies doivent déclarer :

- Alias de dataspace - agrupacion lisible por humanos que vincula politicas de conformité.
- Handle de gouvernance - identifiant du résultat via `Nexus.governance.modules`.
- Handle de règlement - identifiant du consommateur par le routeur de règlement pour débiter les tampons XOR.
- Métadonnées facultatives de télémétrie (description, contact, domaine d'activité) affichées via `/status` et tableaux de bord.## Géométrie de configuration des voies (`LaneConfig`)

`LaneConfig` est le runtime géométrique dérivé du catalogue de voies validé. Pas de réinsertion des manifestes de gouvernance ; et sur votre lieu, vous trouverez des identifiants de déterministes de stockage et des pistes de télémétrie pour chaque voie configurée.

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
- Les alias sont désinfectés pour les limaces et les minusculas ; les caractères alphanumériques consécutifs ne se collapsan en `_`. Si l'alias produit un slug vacio, nous utilisons `lane{id}`.
- `shard_id` est dérivé de la clé de métadonnées `da_shard_id` (par défaut `lane_id`) et conduit le journal du curseur de fragment persistant pour maintenir la relecture de DA déterministe entre redémarrages/resharding.
- Les préfixes de clé garantissent que le WSV conserve une gamme de touches pour les disjonctions lorsqu'ils partagent le même backend.
- Les nombres de segments Kura sont déterministes entre les hôtes ; Les auditeurs peuvent vérifier les directeurs de segment et les manifestes sans outillage sur mesure.
- Les segments fusionnés (`lane_{id:03}_merge`) gardent les racines ultimes de l'indice de fusion et les engagements de l'état mondial pour cette voie.

## Partitionné de l'État-monde- La logique d'état mondial de Nexus est l'union des espaces de l'état par voie. Les voies publiques persistent dans leur état complet ; las lanes privadas/confidentiel exportan racines Merkle/engagement al merge ledger.
- L'emplacement MV prend en compte chaque clé avec le préfixe de 4 octets de `LaneConfigEntry::key_prefix`, génère des clés comme `[00 00 00 01] ++ PackedKey`.
- Les tableaux compartimentés (comptes, actifs, déclencheurs, registres de gouvernance) contiennent des entrées agrégées par choix de voie, en maintenant les analyses de gamme déterministes.
- Les métadonnées du grand livre de fusion reflètent la même disposition : chaque voie écrit les racines de l'indice de fusion et les racines de l'état global réduit en `lane_{id:03}_merge`, permettant la rétention ou l'expulsion lorsqu'une voie est retirée.
- Les indices cross-lane (alias de compte, registres d'actifs, manifestes de gouvernance) contiennent des préfixes de voie explicites pour que les opérateurs concilient les entrées rapidement.
- **Politica de retencion** - las lanes publicas retienen cuerpos de bloque completos ; las lanes solo de engagements pueden compactar cuerpos antiguos après les points de contrôle parce que los engagements son autoritativos. Les journaux de garde confidentiels des voies sont cifrados en segments dédiés pour ne pas bloquer d'autres charges de travail.- **Tooling** - utilitaires de maintenance (`kagami`, commandes administrateur de CLI) doivent référencer l'espace de noms avec slug aux métriques d'exposant, étiquettes Prometheus ou aux segments d'archivage Kura.

## Routage et API

- Les points de terminaison Torii REST/gRPC acceptent un `lane_id` facultatif ; l'ausencia implique `lane_default`.
- Les SDK exposent les sélecteurs de voies et les alias mapéens compatibles avec `LaneId` en utilisant le catalogue de voies.
- Les règles de routage fonctionnent sur le catalogue validé et peuvent sélectionner la voie et l'espace de données. `LaneConfig` prouve des alias amigables pour la télémétrie et les tableaux de bord et les journaux.

## Frais de règlement et frais

- Chaque voie paye les frais XOR pour l'ensemble des validateurs globaux. Les voies peuvent cobrar des jetons de gaz naturel mais doivent avoir un dépôt d'équivalents XOR avec des engagements.
- Les preuves de règlement incluent le montant, les métadonnées de conversion et la vérification du dépôt (par exemple, le transfert au coffre-fort global des frais).
- Le routeur de règlement unifié (NX-3) tamponne le débit en utilisant les mêmes préférences de voie, ainsi que la télémétrie de règlement est alignée avec la géométrie de stockage.

## Gouvernance- Les voies déclarent leur module de gouvernance via le catalogue. `LaneConfigEntry` apporte les alias et les slug originaux pour maintenir lisibles la télémétrie et les pistes d'audit.
- Le registre de Nexus distribue les manifestes des sociétés qui incluent le `LaneId`, la liaison de l'espace de données, le descripteur de gouvernance, le descripteur de règlement et les métadonnées.
- Les hooks de mise à niveau du runtime s'appliquent aux politiques de gouvernance (`gov_upgrade_id` par défaut) et enregistrent les différences via le pont de télémétrie (événements `nexus.config.diff`).

## Statut et télémétrie

- `/status` expose les alias de voie, les liaisons d'espace de données, les poignées de gouvernance et les profils de règlement, les dérivés du catalogue et `LaneConfig`.
- Les mesures du planificateur (`nexus_scheduler_lane_teu_*`) utilisent des alias/slugs pour que les opérateurs mappent rapidement le retard et la pression du TEU.
- `nexus_lane_configured_total` indique le nombre d'entrées de voies dérivées et est recalculé lorsque vous modifiez la configuration. La télémétrie émet des différences entre les entreprises lorsque la géométrie des voies est modifiée.
- Les jauges de backlog de l'espace de données incluent des métadonnées d'alias/description pour aider les opérateurs à la pression associée de cola avec les domaines d'affaires.

## Configuration et types Norito- `LaneCatalog`, `LaneConfig` et `DataSpaceCatalog` vivent en `iroha_data_model::nexus` et ont prouvé des structures au format Norito pour les manifestes et les SDK.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` et dérive automatiquement du catalogue ; aucun codage requis Norito car il s'agit d'un runtime interne d'assistance.
- La configuration de la personne à l'utilisateur (`iroha_config::parameters::user::Nexus`) doit accepter les descripteurs déclaratifs de la voie et de l'espace de données ; L'analyseur dérive maintenant la géométrie et recherche les alias invalides ou les identifiants des voies dupliquées.

## Travail pendant

- Intégrer les mises à jour du routeur de règlement (NX-3) avec la nouvelle géométrie pour que les débits et les recettes du tampon XOR soient étiquetés par slug de lane.
- Outils d'extension d'administration pour lister les familles de colonnes, compacter les voies retirées et inspecter les journaux de blocage par voie en utilisant l'espace de noms avec slug.
- Finaliser l'algorithme de fusion (ordonnancement, élagage, détection de conflit) et ajouter des appareils de régression pour la relecture cross-lane.
- Ajouter des crochets de conformité pour les listes blanches/listes noires et les politiques d'argent programmables (en suivant le NX-12).

---

*Cette page suit les suivis du NX-1 à mesure que le NX-2 est connecté au NX-18. Merci d'exposer des questions ouvertes sur `roadmap.md` ou sur le tracker de gouvernance pour que le portail soit aligné avec les documents canoniques.*