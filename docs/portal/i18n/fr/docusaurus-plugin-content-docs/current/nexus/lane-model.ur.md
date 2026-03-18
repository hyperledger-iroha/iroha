---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : Modèle de voie Nexus
description : Sora Nexus pour les voies et la taxonomie, la géométrie de configuration et la fusion d'états mondiaux.
---

# Nexus modèle de voie et partitionnement WSV

> **Statut :** Livrable NX-1 - taxonomie des voies, géométrie de configuration, et disposition du stockage ici.  
> **Propriétaires :** Nexus GT principal, GT sur la gouvernance  
> **Référence de la feuille de route :** `roadmap.md` pour NX-1

Le dossier canonique `docs/source/nexus_lanes.md` est le dossier de présentation du SDK et les réviseurs de l'arbre mono-repo de Sora Nexus. میں جائے بغیر guidage sur voie پڑھ سکیں۔ Architecture de l'état mondial, déterminisme, systèmes de validation, espaces de données (voies), publics et privés, ensembles de validateurs, charges de travail isolées, systèmes de validation

##Concept- **Lane :** Grand livre Nexus avec fragment de partition, jeu de validateurs et retard d'exécution en cours اسے ایک مستحکم `LaneId` شناخت کیا جاتا ہے۔
- **Espace de données :** compartiment de gouvernance, y compris les voies, les règles de sécurité et la conformité, le routage et les politiques de règlement et les politiques de règlement.
- **Lane Manifest :** métadonnées contrôlées par la gouvernance, validateurs, politique DA, jeton de gaz, règles de règlement, et autorisations de routage pour les paiements en ligne
- **Engagement mondial :** preuve de preuve et transferts de voie, racines d'État, données de règlement, transferts transversaux facultatifs et transferts inter-voies engagements mondiaux du réseau NPoS

## Taxonomie des voies

Types de voies, visibilité, surface de gouvernance, crochets de règlement, canoniques, lignes directrices géométrie de configuration (`LaneConfig`) pour les attributs et la capture de plusieurs nœuds, SDK et outils pour une logique sur mesure et une mise en page| Type de voie | Visibilité | Adhésion du validateur | Exposition au WSV | Gouvernance par défaut | Politique de règlement | Utilisation typique |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | publique | Sans autorisation (enjeu mondial) | Réplique d'état complet | SORA Parlement | `xor_global` | Grand livre public de référence |
| `public_custom` | publique | Sans autorisation ou avec enjeu | Réplique d'état complet | Module pondéré par enjeux | `xor_lane_weighted` | Applications publiques à haut débit |
| `private_permissioned` | restreint | Ensemble de validateurs fixes (gouvernance approuvée) | Engagements & preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, charges de travail du consortium |
| `hybrid_confidential` | restreint | Adhésion mixte; enveloppe les épreuves ZK | Engagements + information sélective | Module d'argent programmable | `xor_dual_fund` | Argent programmable préservant la confidentialité |

Les types de voies que vous déclarez sont :

- Alias d'espace de données - انسان کے لئے پڑھنے کے قابل regroupement et politiques de conformité et lier کرتی ہے۔
- Poignée de gouvernance - Identifiant de l'identifiant et `Nexus.governance.modules` pour résoudre le problème
- Descripteur de règlement - Identifiant d'un routeur de règlement Tampons XOR et débit d'un routeur de règlement
- Métadonnées de télémétrie facultatives (description, contact, domaine d'activité) et tableaux de bord `/status` pour les clients## Géométrie de configuration des voies (`LaneConfig`)

Catalogue de voies validé `LaneConfig` et géométrie d'exécution dérivée یہ la gouvernance se manifeste et remplace نہیں کرتا؛ Il s'agit d'une voie configurée, d'identifiants de stockage déterministes et d'indices de télémétrie.

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

- La géométrie `LaneConfig::from_catalog` permet de calculer la charge de configuration et la charge de configuration (`State::set_nexus`).
- Alias ​​​​et limaces minuscules pour désinfecter les gens مسلسل caractères non alphanumériques `_` میں réduire ہوتے ہیں۔ Un alias de slug vide est utilisé pour le repli `lane{id}`.
- Clé de métadonnées du catalogue `shard_id` `da_shard_id` pour dériver le code (par défaut `lane_id`) et le journal du curseur de fragment persistant, le lecteur, le redémarrage/le repartitionnement et la relecture DA déterministe رہے۔
- Préfixes clés pour les plages de clés WSV par voie et pour le backend partagé
- Les noms de segments Kura hébergent des noms déterministes et déterministes. les auditeurs utilisent des outils sur mesure et des répertoires de segments et manifestent des vérifications croisées.
- Fusionner les segments (`lane_{id:03}_merge`) sur la voie, les dernières racines d'indice de fusion et les engagements mondiaux de l'État.

## Partitionnement de l'état du monde- Espaces d'état logiques par voie d'état mondial Nexus et union ہے۔ L'état complet des voies publiques persiste voies privées/confidentielles Merkle/racines d'engagement pour fusionner le grand livre et exporter les données
- Clé de stockage MV `LaneConfigEntry::key_prefix` et préfixe de 4 octets et préfixe de 4 octets avec clés `[00 00 00 01] ++ PackedKey`
- Entrées de tables partagées (comptes, actifs, déclencheurs, enregistrements de gouvernance) et préfixe de voie et de groupe et analyses de plage déterministes.
- Mise en page des métadonnées du grand livre de fusion et mise en page miroir : la voie `lane_{id:03}_merge` contient des racines d'indice de fusion et des racines d'état globales réduites, ainsi que le retrait de la voie et la rétention ciblée. expulsion ممکن ہوتی ہے۔
- Index multi-voies (alias de compte, registres d'actifs, manifestes de gouvernance) Les préfixes de voie explicites stockent les entrées des opérateurs et les rapprochent.
- **Politique de rétention** - voies publiques et corps de bloc رکھتی ہیں؛ voies réservées aux engagements points de contrôle کے بعد پرانے corps compacts کر سکتی ہیں کیونکہ engagements faisant autorité ہیں۔ Voies confidentielles journaux chiffrés et segments dédiés pour les blocs de charges de travail
- **Outils** - utilitaires de maintenance (`kagami`, commandes d'administration CLI) et les métriques exposent les étiquettes Prometheus dans l'archive des segments Kura et l'espace de noms slugged se réfèrent à l'archive des segments Kura.

## Routage et API- Points de terminaison REST/gRPC Torii en option `lane_id` pour les utilisateurs عدم موجودگی `lane_default` کو ظاہر کرتی ہے۔
- Sélecteurs de voies SDK et alias conviviaux et catalogue de voies et carte `LaneId`.
- Catalogue de règles de routage validées pour le fonctionnement des voies et de l'espace de données dans les zones de stockage `LaneConfig` tableaux de bord et journaux et alias compatibles avec la télémétrie

## Règlement et frais

- Ensemble de validateurs mondiaux de voie et frais XOR pour tous les utilisateurs Les jetons de gaz natifs de Lanes sont liés à des engagements et des équivalents XOR Escrow sont en vigueur.
- Preuves de règlement, montant, métadonnées de conversion, et preuve de séquestre, paiement (pour le coffre-fort global des frais et transfert)
- Le routeur de règlement unifié (NX-3) tamponne les préfixes de voie et les préfixes de débit pour la géométrie de stockage de la télémétrie de règlement et l'alignement des paramètres.

## Gouvernance

- Lanes module de gouvernance et catalogue et déclaration de projets `LaneConfigEntry` alias اور slug ساتھ رکھتا ہے télémétrie et pistes d'audit lisibles رہیں۔
- La voie signée par le registre Nexus manifeste des manifestes de connexion avec `LaneId`, liaison d'espace de données, descripteur de gouvernance, descripteur de règlement et de métadonnées en ligne.
- Politiques de gouvernance des hooks de mise à niveau d'exécution (`gov_upgrade_id` par défaut) et pont de télémétrie (événements `nexus.config.diff`) et journal des différences ہیں۔## Télémétrie et statut

- Alias de voie `/status`, liaisons d'espace de données, poignées de gouvernance et profils de règlement et exposition des informations sur le catalogue et `LaneConfig` pour dériver des informations
- Métriques du planificateur (`nexus_scheduler_lane_teu_*`) alias/slugs de voie pour le carnet de commandes des opérateurs et la pression TEU et la carte
- `nexus_lane_configured_total` entrées de voie dérivées pour le calcul et la configuration Géométrie des voies de télémétrie بدلنے پر les diffs signés émettent کرتی ہے۔
- Le backlog de l'espace de données mesure les métadonnées d'alias/description, ainsi que la pression de la file d'attente des opérateurs et les domaines d'activité.

## Configuration et types Norito

- `LaneCatalog`, `LaneConfig`, et `DataSpaceCatalog` `iroha_data_model::nexus` présentent des manifestes et des SDK ainsi que des structures de format Norito. فراہم کرتے ہیں۔
- `LaneConfig` `iroha_config::parameters::actual::Nexus` میں رہتا ہے اور catalogue سے خودکار طور پر dériver ہوتا ہے؛ L'encodage Norito est un outil d'aide à l'exécution interne.
- Configuration orientée utilisateur (`iroha_config::parameters::user::Nexus`) voie déclarative et descripteurs d'espace de données pour les utilisateurs. analyse de la géométrie dériver ou des alias invalides ou des identifiants de voie en double ou rejeter des erreurs

## Travail remarquable- Les mises à jour du routeur de règlement (NX-3) et la géométrie de la nouvelle version intègrent les débits tampon XOR et les reçus lane slug ainsi que les balises de règlement.
- Outils d'administration pour étendre la liste des familles de colonnes et les voies retirées compactes et l'espace de noms slugged et les journaux de bloc par voie inspecter.
- Algorithme de fusion (classement, élagage, détection des conflits) finalisation de la relecture cross-lane et des appareils de régression
- Listes blanches/listes noires pour les politiques monétaires programmables et les crochets de conformité pour les systèmes d'exploitation (NX-12 میں ٹریک)۔

---

*یہ NX-2 et NX-18 et suivis NX-1 et suivis NX-1 Le suivi des documents canoniques `roadmap.md` et le suivi de la gouvernance sont en ligne avec les documents canoniques alignés*