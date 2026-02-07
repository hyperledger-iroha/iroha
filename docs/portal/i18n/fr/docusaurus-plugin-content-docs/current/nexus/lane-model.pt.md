---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : Modèle de voies du Nexus
description : Taxonomie logique des voies, géométrie de configuration et règles de fusion de l'état mondial pour Sora Nexus.
---

# Modèle de voies du Nexus et participation de WSV

> **Statut :** entregavel NX-1 - taxonomie des voies, géométrie de configuration et disposition du stockage rapidement pour la mise en œuvre.  
> **Propriétaires :** Nexus GT principal, GT sur la gouvernance  
> **Référence de feuille de route :** NX-1 dans `roadmap.md`

Cette page du portail contient le bref canonique `docs/source/nexus_lanes.md` pour les opérateurs de Sora Nexus, les propriétaires du SDK et les réviseurs peuvent trouver le guide des voies semestriel pour créer un mono-repo. Une architecture permettant de déterminer l'état mondial en ce qui concerne les espaces de données (voies) individuels exécute des ensembles de validateurs publics ou privés avec des charges de travail isolées.

## Conceitos- **Lane :** fragment logique du grand livre Nexus avec votre propre ensemble de validateurs et de retard d'exécution. Identifié par un état `LaneId`.
- **Espace de données :** compartiment de gouvernance qui regroupe un groupe ou plus de voies qui partagent les politiques de conformité, de routage et de règlement.
- **Lane Manifest :** métadonnées contrôlées par les gouvernements qui décrivent les validateurs, la politique de DA, les jetons de gaz, les règles de règlement et les autorisations de routage.
- **Engagement mondial :** preuve émise pour une voie qui reprend les nouvelles racines de l'État, les données de règlement et les transferts d'options à travers les voies. O anel NPoS engagements mondiaux pour les ordonnances.

## Taxonomie des voies

Les types de voie décrivent la forme canonique de la visibilité, de la surface de gouvernance et des crochets de règlement. La géométrie de configuration (`LaneConfig`) capture ces attributs pour que nos SDK et outils puissent personnaliser la mise en page selon une logique sur mesure.| Type de voie | Visibilité | Membres des validateurs | Exposition WSV | Gouvernance par défaut | Politique de règlement | Utilisation typique |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | publique | Sans autorisation (enjeu mondial) | Réplique de l'état complet | SORA Parlement | `xor_global` | Base de données du grand livre |
| `public_custom` | publique | Sans autorisation ou avec enjeux | Réplique de l'état complet | Module réfléchi par enjeu | `xor_lane_weighted` | Applications publiques à haut débit |
| `private_permissioned` | restreint | Set fixo de validadores (approuvé par la gouvernance) | Engagements et preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, charges de travail du consortium |
| `hybrid_confidential` | restreint | Membresia mista; impliquent des preuves ZK | Engagements + divulgation sélective | Module de dîner programmé | `xor_dual_fund` | Dinheiro programavel com preservacao de privacidade |

Tous les types de voies doivent déclarer :

- Alias de dataspace - agrupamento legivel por humanos que vincula politicas de conformité.
- Handle de gouvernance - identificador resolvido via `Nexus.governance.modules`.
- Handle de règlement - identifiant le consommateur du routeur de règlement pour débiter les tampons XOR.
- Métadonnées facultatives de télémétrie (description, contact, domaine d'activité) exposées via `/status` et les tableaux de bord.## Géométrie de configuration des voies (`LaneConfig`)

`LaneConfig` et un runtime géométrique dérivé du catalogue de voies validé. Ele nao substitui os manifests de gouvernance; em vez disso fornece identificadores de storage deterministas e dicas de telemetria para cada lane configurada.

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

- `LaneConfig::from_catalog` recalcule la géométrie lors de la configuration et du transport (`State::set_nexus`).
- Alias ​​sao sanitizados em limaces minusculos; les caractères nao alfanumericos consecutivos colapsam em `_`. Voir l'alias produire un slug vazio, nous utilisons `lane{id}`.
- `shard_id` et dérivé de la clé de métadonnées `da_shard_id` (par défaut `lane_id`) et dirige le journal du curseur de fragment persistant pour permettre la relecture du DA déterminé entre les redémarrages/resharding.
- Les préfixes de garantie garantissent que le WSV conserve des gammes de garanties pour les disjoints lorsque le backend et le partage sont partagés.
- Nomes de segments Kura sao déterministas entre hôtes ; Les auditeurs peuvent vérifier les diretorios de segmento et manifestent des outils sur mesure.
- Les segments de fusion (`lane_{id:03}_merge`) sont gardés comme racines ultimes de l'indice de fusion et des engagements de l'état mondial pour cette voie.

## Partitionnement de l'État-monde- La logique de l'état mondial Nexus et l'unité des espaces de l'état par voie. Lanes publicas persistem estado completo ; voies privées/confidentiel exportam racines Merkle/engagement pour la fusion du grand livre.
- L'armement du préfixe MV est chaque fois avec le préfixe de 4 octets de `LaneConfigEntry::key_prefix`, Gerando chaves comme `[00 00 00 01] ++ PackedKey`.
- Tableaux de comparaison (comptes, actifs, déclencheurs, registres de gouvernance) armazenam entradas agrupadas por prefixo de lane, mantendo os range scans deterministas.
- Des métadonnées qui reflètent la mise en page du grand livre de fusion : chaque voie enregistre les racines de l'indice de fusion et les racines de l'état global réduit dans `lane_{id:03}_merge`, permettant de retenir ou d'expulser la direction lorsqu'une voie est retirée.
- Indices cross-lane (alias de compte, registres d'actifs, manifestes de gouvernance) armazenam préfixos de lane explicitos pour que les opérateurs concilient les entrées rapidement.
- **Politica de retencao** - lanes publicas retencem corpos de bloco completos ; voies apenas de engagements podem compactar corpos antigos apos points de contrôle porque engagements sao autoritativos. Les journaux de garde confidentiels de Lanes sont cifrados dans des segments dédiés pour ne pas bloquer d'autres charges de travail.
- **Tooling** - les utilitaires de maintenance (`kagami`, commandes administrateur de CLI) développent un référencement pour l'espace de noms avec slug pour l'exportation de mesures, les étiquettes Prometheus ou pour archiver des segments Kura.

## Routage des API électroniques- Points de terminaison Torii REST/gRPC avec le `lane_id` facultatif ; ausencia implica `lane_default`.
- Les SDK exposent les sélections de voies et les alias mapeiam amis pour `LaneId` en utilisant le catalogue de voies.
- Informations sur l'opération de routage sur le catalogue validé et peut explorer la voie et l'espace de données. `LaneConfig` fournit des alias amis pour la télémétrie dans les tableaux de bord et les journaux.

## Frais de règlement et

- Chaque voie paye les frais XOR et définit les validateurs globaux. Lanes peut récupérer des jetons de gaz naturel, mais doit faire appel à un dépôt équivalent XOR avec des engagements.
- Les preuves de règlement incluent le montant, les métadonnées de conversation et la preuve de séquestre (par exemple, le transfert pour le coffre-fort global des frais).
- Le routeur de règlement unifié (NX-3) tamponne le débit en utilisant mes préfixes de voie, ainsi que la télémétrie de règlement et la géométrie de stockage.

## Gouvernance

- Lanes déclare son module de gouvernance via catalogo. `LaneConfigEntry` charge l'alias et le slug original pour gérer la télémétrie et les pistes d'audit légitimes.
- Le registre Nexus distribue les manifestes assassinés qui incluent `LaneId`, la liaison de l'espace de données, le descripteur de gouvernance, le descripteur de règlement et les métadonnées.
- Hooks de mise à niveau d'exécution continue appliquant la politique de gouvernance (`gov_upgrade_id` par défaut) et les différences d'enregistrement via le pont de télémétrie (événements `nexus.config.diff`).

## Télémétrie et statut- `/status` expose les alias de voie, les liaisons d'espace de données, les poignées de gouvernance et les paramètres de règlement, les dérivés du catalogue et le `LaneConfig`.
- Les mesures du planificateur (`nexus_scheduler_lane_teu_*`) génèrent des alias/slugs pour que les opérateurs mappent le backlog et pressent rapidement le TEU.
- `nexus_lane_configured_total` contient le nombre d'entrées de voies dérivées et est recalculé lors de la configuration. Une télémétrie émet des différences lorsque la géométrie des voies est modifiée.
- Les jauges de retard dans l'espace de données incluent des métadonnées d'alias/description pour aider les opérateurs associés à la presse de fil aux domaines commerciaux.

## Configuration et types Norito

- `LaneCatalog`, `LaneConfig` et `DataSpaceCatalog` viennent dans `iroha_data_model::nexus` et forment des structures compatibles avec Norito pour les manifestes et les SDK.
- `LaneConfig` est présent dans `iroha_config::parameters::actual::Nexus` et est automatiquement dérivé du catalogue ; nao requer l'encodage Norito pour un assistant d'exécution interne.
- La configuration sélectionnée par l'utilisateur (`iroha_config::parameters::user::Nexus`) continue à suivre les descriptions déclaratives de la voie et de l'espace de données ; L'analyse syntaxique vient de dériver la géométrie et de rejeter les alias invalides ou les identifiants de voie dupliqués.

## Trabalho pendente- Mises à jour intégrées du routeur de règlement (NX-3) avec une nouvelle géométrie pour que les débits et les réceptions du tampon XOR soient étiquetés par slug de lane.
- Estender Tooling Admin pour lister les familles de colonnes, compacter les voies posées et inspecter les journaux de blocs pour la voie utilisée ou l'espace de noms avec slug.
- Finaliser l'algorithme de fusion (ordonnancement, élagage, détection de conflit) et les appareils de régression annexes pour la relecture cross-lane.
- Ajout de crochets de conformité pour les listes blanches/listes noires et les programmes de politique monétaire (accompagnés du NX-12).

---

*Cette page continue de suivre les suivis du NX-1 conforme au NX-2 et au NX-18. Por favor traga perguntas em aberto para `roadmap.md` ou o tracker de gouvernance pour que le portail soit aligne avec les documents canoniques.*