---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : Modèle voies Nexus
description : Voies de taxinomie logiques, configurations géométriques et configuration de l'état mondial pour Sora Nexus.
---

# Modèle de voies Nexus et partitionnement WSV

> **État :** poste NX-1 - voies de circulation, configurations géométriques et configuration des accès à l'extérieur.  
> **Propriétaires :** Nexus GT principal, GT sur la gouvernance  
> **Ссылка в roadmap:** NX-1 в `roadmap.md`

Cette page du portail utilise le dossier canonique `docs/source/nexus_lanes.md`, pour les opérateurs Sora Nexus, le SDK des propriétaires et les commentaires qui peuvent être effectués sur les voies. sans avoir recours à un mono-repo. Un archéologue de renom s'occupe de déterminer l'état mondial, afin de permettre l'accès à d'autres espaces de données (voies) publiés ou verrouillés par des validateurs. изолированными charges de travail.

##Concept- **Lane :** Grand livre de fragments logique Nexus pour les validateurs et le backlog. Identifier la stabilité `LaneId`.
- **Espace de données :** compartiment de gouvernance, groupes ou voies réservées à la conformité politique, au routage et au règlement.
- ** Manifeste de voie : ** métadonnées de contrôle de gouvernance, validation des validations, politique DA, jeton de gaz, règlement autorisé et autorisations de routage.
- **Engagement mondial :** preuve, voie privée et résumé des nouvelles racines de l'État, données de règlement et transferts inter-voies opérationnels. Le groupe mondial NPoS respecte les engagements.

## Voies de taxinomie

Les types de voies canoniques présentent la surface de gouvernance et les crochets de règlement. La configuration géométrique (`LaneConfig`) définit ces attributs, tels que les utilisateurs, les SDK et les outils, qui peuvent personnaliser la mise en page sans logique sur mesure.| Voie de type | Vidéo | Validateurs d'adhésion | Utilisation WSV | Gouvernance по умолчанию | Règlement politique | Présentation typique |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | publique | Sans autorisation (enjeu mondial) | Réplique d'état complet | SORA Parlement | `xor_global` | Grand livre public bas |
| `public_custom` | publique | Sans autorisation ou avec enjeu | Réplique d'état complet | Module pondéré par enjeux | `xor_lane_weighted` | Applications publiques pour votre débit |
| `private_permissioned` | restreint | Фиксированный набор валидаторов (одобрен gouvernance) | Engagements & preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, charges de travail du consortium |
| `hybrid_confidential` | restreint | Adhésion à Смешанная; оборачивает ZK preuves | Engagements + information sélective | Module d'argent programmable | `xor_dual_fund` | Argent programmable préservant la confidentialité |

Voici les types de voies utilisées :

- Alias d'espace de données - человекочитаемая группировка, связывающая политики conformité.
- Poignée de gouvernance - идентификатор, разрешаемый через `Nexus.governance.modules`.
- Poignée de règlement - identifiant, routeur de règlement utilisé pour la description des tampons XOR.
- Métadonnées de télémétrie fonctionnelles (description, contact, domaine d'activité), disponibles à partir de `/status` et des tableaux de bord.## Configuration géométrique des voies (`LaneConfig`)

`LaneConfig` - Géométrie d'exécution, permettant de visualiser les voies du catalogue de validation. Она не заменяет les manifestes de gouvernance ; Cela permet de déterminer les identifiants de stockage et les conseils de télémétrie pour chaque voie de configuration.

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

- `LaneConfig::from_catalog` permet de définir la géométrie lors de la configuration (`State::set_nexus`).
- Alias ​​санитизируются в нижний регистр slug ; Les symptômes suivants ne sont pas conformes à la norme `_`. Si l'alias est un slug, j'utilise `lane{id}`.
- `shard_id` vous permet d'accéder à la clé de métadonnées `da_shard_id` (en utilisant `lane_id`) et de mettre à jour le curseur de partition, pour permettre la relecture. детерминированным между redémarre/repartitionnement.
- Les préfixes clés garantissent que WSV dispose de plusieurs plages de clés sur la voie, ainsi que sur le backend.
- Имена Kura segment детерминированы между hôtes; Les auditeurs peuvent prendre en charge la direction des segments et des manifestes sans outillage sur mesure.
- Fusionner les segments (`lane_{id:03}_merge`) donne suite aux racines des indices de fusion et aux engagements mondiaux de l'État pour cette voie.

## Партиционирование monde-état- Логический world state Nexus - c'est la définition des espaces d'état sur la voie. Les voies publiques sont entièrement couvertes; voies privées/confidentielles экспортируют Merkle/racines d'engagement dans le grand livre de fusion.
- Les paramètres de stockage MV sont fournis avec le préfixe à 4 emplacements `LaneConfigEntry::key_prefix`, pour afficher les clés `[00 00 00 01] ++ PackedKey`.
- Tables partagées (comptes, actifs, déclencheurs, enregistrements de gouvernance) хранят записи, сгруппированные по lane prefixe, сохраняя детерминизм range scans.
- Les métadonnées du grand livre de fusion sont affectées à la disposition : la voie contient les racines des indices de fusion et les racines d'état globales réduites dans `lane_{id:03}_merge`, ce qui permet de retarder la rétention/l'expulsion ciblée de votre voie.
- Les index croisés (alias de compte, registres d'actifs, manifestes de gouvernance) contiennent les préfixes de chaque voie, que les opérateurs peuvent utiliser pour le rapprochement.
- **Politique de rétention** - les voies publiques сохраняют полные les corps de bloc ; les voies d'engagement uniquement peuvent compacter les corps des étoiles après les points de contrôle, en tant qu'autorités d'engagement. Des voies confidentielles transmettent les journaux de texte chiffré dans de nombreux segments, ce qui ne bloque pas les charges de travail supplémentaires.
- **Outils** - Utilisation d'utilitaires (`kagami`, commandes d'administration CLI) pour l'utilisation de l'espace de noms slugged lors de l'utilisation de métriques, d'étiquettes Prometheus ou d'archivage Segments de Kura.

## Routage et API- Les points de terminaison REST/gRPC Torii sont disponibles en option `lane_id` ; отсутствие означает `lane_default`.
- Les SDK permettent de sélectionner des sélecteurs de voie et de mapper des alias conviviaux dans `LaneId` dans le catalogue de voies.
- Les règles de routage sont utilisées dans le catalogue de validation et peuvent être utilisées pour la voie et l'espace de données. `LaneConfig` fournit des alias compatibles avec la télémétrie pour les tableaux de bord et les journaux.

## Règlement et frais

- Les frais XOR du plateau Lane Lane sont globaux pour les validateurs. Lanes peut récupérer des jetons de gaz natifs, sans bloquer les équivalents XOR dans les engagements.
- Les preuves de règlement incluent le montant, les métadonnées de conversion et le dépôt de preuves (par exemple, avant le coffre-fort mondial des frais).
- Le routeur de règlement unique (NX-3) utilise des tampons pour les préfixes de voies, en complément de la télémétrie de règlement avec la géométrie de stockage.

## Gouvernance

- Lanes объявляют свой module de gouvernance через каталог. `LaneConfigEntry` ne nécessite pas d'alias et de slug d'origine, car la télémétrie et les pistes d'audit sont établies.
- Le registre Nexus prend en charge les manifestes de voie, notamment `LaneId`, la liaison d'espace de données, le descripteur de gouvernance, le descripteur de règlement et les métadonnées.
- Les hooks de mise à niveau d'exécution permettent de créer des politiques de gouvernance (`gov_upgrade_id` pour la gestion) et d'enregistrer les différences à partir du pont de télémétrie (événements `nexus.config.diff`).

## Télémétrie et statut- `/status` permet d'identifier les alias de voies, les liaisons d'espace de données, les poignées de gouvernance et les profils de règlement, ainsi que le catalogue et `LaneConfig`.
- Les métriques du planificateur (`nexus_scheduler_lane_teu_*`) détectent les alias/slugs, qui permettent aux opérateurs de gérer l'arriéré et la pression TEU.
- `nexus_lane_configured_total` permet de sélectionner les entrées de voies et de configurer les configurations. La télémétrie permet de prendre en compte les différences pour la géométrie des voies.
- Les jauges de backlog d'espace de données incluent les métadonnées d'alias/description, permettant à l'opérateur de mesurer la pression de la file d'attente dans les affaires.

## Configuration et types Norito

- `LaneCatalog`, `LaneConfig` et `DataSpaceCatalog` sont installés dans `iroha_data_model::nexus` et pré-installés sur Norito. pour les manifestes et les SDK.
- `LaneConfig` est disponible dans `iroha_config::parameters::actual::Nexus` et automatiquement à partir du catalogue ; L'encodage Norito n'est pas un problème, car c'est une aide à l'exécution disponible.
- La configuration globale (`iroha_config::parameters::user::Nexus`) permet de définir la voie des descripteurs et l'espace de données ; Vous pouvez analyser votre géométrie et ouvrir des alias non valides ou doubler des identifiants de voie.

## Оставшаяся работа- Intégrez les mises à jour du routeur de règlement (NX-3) avec la nouvelle géométrie, qui permet aux débits et aux reçus du tampon XOR de slug lane.
- Améliorez les outils d'administration pour les familles de colonnes spécifiques, les voies retirées et les journaux de bloc par voie pour l'espace de noms slugged.
- Finalisez l'algorithme de fusion (classement, élagage, détection de conflit) et créez des appareils de régression pour la relecture inter-voies.
- Добавить compliance hooks для whitelists/blacklists и programmable-money policies (трекится под NX-12).

---

*Эта страница продолжит отслеживать NX-1 follow-ups по мере посадки NX-2 до NX-18. Il est possible d'ouvrir les informations sur `roadmap.md` ou sur le suivi de la gouvernance, en utilisant le portail de synchronisation avec les documents canoniques.*