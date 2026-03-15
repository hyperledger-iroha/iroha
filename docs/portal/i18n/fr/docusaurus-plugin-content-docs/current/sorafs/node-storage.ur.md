---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : stockage de nœud
titre : Conception de stockage de nœuds SoraFS
sidebar_label : conception du stockage des nœuds
description : Hôte de données SoraFS et nœuds Torii pour l'architecture de stockage, quotas et hooks de cycle de vie
---

:::note مستند ماخذ
:::

## SoraFS Conception du stockage des nœuds (projet)

Vous avez besoin d'un nœud Iroha (Torii) pour activer la couche de disponibilité des données SoraFS avec opt-in Un disque local est constitué de morceaux de morceaux différents/سرو کرنے کے لیے مختص کر سکتا ہے۔ Spécifications de découverte `sorafs_node_client_protocol.md` pour le luminaire SF-1b, pour la configuration, pour l'architecture côté stockage, pour les contrôles de ressources, pour la configuration de la plomberie, pour les nœuds et pour les chemins de code de la passerelle. شامل ہونا ضروری ہے۔ عملی آپریشنل exercices
[Node Operations Runbook](./node-operations) میں موجود ہیں۔

### Objectifs

- Le validateur et le processus auxiliaire Iroha et le disque de rechange du fournisseur SoraFS exposent le grand livre principal et le grand livre principal.
- module de stockage et module déterministe piloté par Norito : manifestes, plans de fragments, racines de preuve de récupérabilité (PoR), et annonces de fournisseurs, source de vérité
- Les quotas définis par l'opérateur appliquent les nœuds de connexion pour les demandes de broche/extraction de ressources et de ressources.
- santé/télémétrie (échantillonnage PoR, latence de récupération de morceaux, pression du disque) et gouvernance et clients et clients

### Architecture de haut niveau```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Modules clés :

- **Passerelle** : les points de terminaison HTTP Norito exposent des propositions de broches, des demandes de récupération de fragments, un échantillonnage PoR et une télémétrie. Les charges utiles Norito valident les demandes et les requêtes du magasin de blocs et le maréchal Un démon est utilisé pour la réutilisation de la pile HTTP Torii.
- **Pin Registry** : état de la broche manifeste entre `iroha_data_model::sorafs` et `iroha_core` et piste de suivi. Manifeste accepté pour le résumé du manifeste du registre, le résumé du plan de fragments, la racine PoR et l'enregistrement des indicateurs de capacité du fournisseur.
- **Chunk Storage** : implémentation `ChunkStore` sauvegardée sur disque et acquisition de manifestes signés et `ChunkProfile::DEFAULT` et plans de fragments se matérialisent, ainsi que des fragments et une disposition déterministe et persistance ہے۔ Empreinte digitale de contenu de bloc et métadonnées PoR associées et échantillonnage pour revalidation et revalidation
- **Quota/Scheduler** : limites configurées par l'opérateur (nombre maximal d'octets de disque, nombre maximal de broches en attente, nombre maximal d'extractions parallèles, TTL de morceaux) pour appliquer les droits d'entrée et de sortie et les coordonnées des nœuds et les tâches du grand livre affamé Les preuves PoR du planificateur et les demandes d'échantillonnage et le processeur limité servent à servir les utilisateurs.

###Configuration

`iroha_config` Section complète de la section :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled` : bascule de participation۔ false et les points de terminaison de stockage de la passerelle 503 pour la découverte de nœuds et la publicité pour les utilisateurs
- `data_dir` : blocs de données, arbres PoR et récupération de télémétrie dans le répertoire racine Par défaut `<iroha.data_dir>/sorafs` ہے۔
- `max_capacity_bytes` : données de bloc épinglées ou limite stricte limite de tâche en arrière-plan پر پہنچنے پر نئے broches rejetées
- `max_parallel_fetches` : planificateur, plafond de concurrence, bande passante/E/S disque, charge de travail du validateur, équilibre et équilibre
- `max_pins` : les broches manifestes ont une longueur maximale et un nœud est fixé et l'expulsion/contre-pression s'applique.
- `por_sample_interval_secs` : tâches d'échantillonnage PoR automatiques et cadence Le travail `N` laisse un échantillon (par manifeste configurable) et les événements de télémétrie émettent un échantillon Gouvernance Clé de métadonnées `profile.sample_multiplier` (entier `1-4`) et `N` à échelle déterministe Valeur, nombre/chaîne unique, remplacements par profil et objet, valeur `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : structure et générateur d'annonces du fournisseur Champs `ProviderAdvertV1` (pointeur de mise, conseils QoS, sujets) Omettre les valeurs par défaut du registre de gouvernance des nœuds

Configuration plomberie :- `[sorafs.storage]` `iroha_config` et `SorafsStorage` pour définir le fichier de configuration du nœud et charger le fichier de configuration
- `iroha_core` et `iroha_torii` configuration de stockage et démarrage et générateur de passerelle et magasin de blocs et fil de discussion
- L'environnement de développement/test remplace les versions (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`) pour les déploiements de production et le fichier de configuration pour s'appuyer sur les versions ultérieures.

### Utilitaires CLI

Il s'agit d'un fil de surface HTTP Torii et d'un `sorafs_node` crate pour une interface CLI mince pour les opérateurs de backend persistant et d'exercices d'ingestion/exportation. script de démarrage 【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- Manifeste codé `ingest` Norito `.to` et les octets de charge utile correspondants s'attendent à ce que Un manifeste, un profil de segmentation, une reconstruction du plan de fragmentation, un condensé de parité imposé, et des fichiers de fragments persistants, et éventuellement `chunk_fetch_specs`, un blob JSON émet un message et une disposition des outils en aval. contrôle de santé mentale کر سکے۔
- `export` ID de manifeste pour le manifeste/charge utile stocké et le disque dur (plan facultatif JSON pour les environnements d'appareils) et les environnements d'appareils reproductibles

Voici les commandes stdout du résumé JSON Norito pour les scripts et les pipelines. Le test d'intégration CLI couvre l'aller-retour des manifestes/charges utiles avec les API Torii mises à jour par 【crates/sorafs_node/tests/cli.rs:1】> Parité HTTP
>
> La passerelle Torii et les assistants en lecture seule exposent les versions basées sur `NodeHandle` :
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — Résumé/métadonnées du manifeste Norito (base64) stocké pour le stockage en ligne (crates/iroha_torii/src/sorafs/api.rs:1207)
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — plan de fragments déterministe JSON (`chunk_fetch_specs`) outillage en aval pour créer un plan de fragments déterministe (crates/iroha_torii/src/sorafs/api.rs:1259)
>
> Sortie CLI des points de terminaison et miroir, ainsi que des pipelines, des scripts locaux et des sondes HTTP ainsi qu'un analyseur syntaxique. سکیں۔【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Cycle de vie des nœuds1. **Démarrage** :
   - stockage activé et répertoire configuré par nœud et capacité et initialisation du magasin de blocs. Je veux que la base de données des manifestes PoR vérifie/crée et que les manifestes épinglés rejouent et que les caches soient chauds.
   - Registre des routes de passerelle SoraFS (points de terminaison Norito JSON POST/GET pour la broche, la récupération, l'échantillon PoR, la télémétrie)
   - Travailleur d'échantillonnage PoR et moniteur de quota spawn ici
2. **Découverte / Annonces** :
3. **Flux de travail d'épinglage** :
   - Manifeste signé par la passerelle et signatures du conseil (plan de fragments, racine du PoR, signatures du conseil) liste d'alias valide (`sorafs.sf1@1.0.0` requis) et plan de fragments et métadonnées du manifeste et correspondance avec les éléments
   - Vérification des quotas Les limites de capacité/broches dépassent une erreur de stratégie (structurée Norito)
   - Chunk de données comme `ChunkStore` pour le flux et l'ingestion des résumés de vérification Mise à jour des arbres PoR et registre des métadonnées du manifeste et magasin
4. **Récupérer le flux de travail** :
   - Les requêtes de plage de fragments de disque servent à Scheduler `max_parallel_fetches` appliquer les paramètres saturés et `429` pour les paramètres saturés
   - La télémétrie structurée (Norito JSON) émet une grande latence, des octets servis et un nombre d'erreurs ainsi qu'une surveillance en aval et une surveillance en aval.
5. **Échantillonnage PoR** :
   - Le travailleur manifeste son poids (en octets stockés) et sélectionne le magasin de morceaux, l'arbre PoR et l'échantillonnage déterministe.- Les résultats des audits de gouvernance et les résultats persistants des annonces des fournisseurs/points de terminaison de télémétrie et des résumés sont également disponibles.
6. **Expulsion/application des quotas** :
   - Capacité par défaut du nœud et rejet des broches par défaut Les politiques facultatives d'expulsion des opérateurs (TTL et LRU) configurent le modèle de gouvernance du modèle de gouvernance. Pour concevoir des quotas stricts et des opérations de désépinglage initiées par l'opérateur.

### Déclaration de capacité et intégration de la planification- Torii et `/v2/sorafs/capacity/declare` et `CapacityDeclarationRecord` mises à jour intégrées `CapacityManager` pour le relais et le nœud pour les allocations de chunker/voie validées et en mémoire voir بنائے۔ La télémétrie du gestionnaire et les instantanés en lecture seule (`GET /v2/sorafs/capacity/state`) exposent les commandes et les commandes par profil et les réservations par voie sont appliquées. ہے۔【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Charges utiles `ReplicationOrderV1` émises par la gouvernance des points de terminaison `/v2/sorafs/capacity/schedule` Pour commander un fournisseur local et cibler un gestionnaire de planification en double pour vérifier la capacité du chunker/voie et réserver une tranche pour `ReplicationPlan` Capacité restante pour l'ingestion des outils d'orchestration Les fournisseurs fournissent des commandes et une réponse `ignored` et accusent réception des flux de travail multi-opérateurs ainsi que 【crates/iroha_torii/src/routing.rs:4845】
- Crochets d'achèvement (ingestion de fin de processus) `POST /v2/sorafs/capacity/complete` et hit pour la libération des réservations `CapacityManager::complete_order` ہوں۔ Réponse avec instantané `ReplicationRelease` (totaux restants, résidus de chunker/voie) et avec l'interrogation des outils d'orchestration et avec la file d'attente de commandes. Il s'agit d'un pipeline de magasin de morceaux et d'un fil et d'un terrain logique d'ingestion 【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】- `TelemetryAccumulator` et `NodeHandle::update_telemetry` intégrés pour muter les tâches de mutation et les travailleurs en arrière-plan PoR/enregistrement des échantillons de disponibilité Les charges utiles canoniques `CapacityTelemetryV1` dérivent des éléments internes du planificateur چھیڑے۔【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Intégrations et travaux futurs

- **Gouvernance** : `sorafs_pin_registry_tracker.md` et télémétrie de stockage (taux de réussite PoR, utilisation du disque) et extension du système Les annonces relatives aux politiques d'admission acceptent une capacité minimale et un taux de réussite PoR minimum requis.
- **SDK client** : la configuration du stockage (limites de disque et alias) expose les outils de gestion des nœuds par programme et amorce les nœuds.
- **Télémétrie** : pile de métriques de stockage (Prometheus / OpenTelemetry) pour intégrer de nouveaux tableaux de bord d'observabilité des métriques de stockage.
- **Sécurité** : module de stockage et pool de tâches asynchrone dédié, contre-pression et lectures de morceaux, io_uring et pools Tokio délimités, bac à sable et bac à sable. Les ressources des clients malveillants sont épuisées.un module de stockage de conception en option et un module déterministe pour les opérateurs et une couche de disponibilité des données SoraFS pour les boutons de commande ہے۔ Pour la passerelle `iroha_config`, `iroha_core`, `iroha_torii` et la passerelle Norito, vous avez besoin d'outils publicitaires pour les fournisseurs میں تبدیلیاں مانگتا ہے۔