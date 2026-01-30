---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba19f8679246db446ee58ee457333e295390800c1eb951de3f9f617ff3fc1f57
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: node-storage
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Source canonique
Cette page reflète `docs/source/sorafs/sorafs_node_storage.md`. Gardez les deux copies synchronisées jusqu'au retrait de la documentation Sphinx historique.
:::

## Conception du stockage du nœud SoraFS (Brouillon)

Cette note précise comment un nœud Iroha (Torii) peut s'inscrire dans la couche
availability de données SoraFS et réserver une partie du disque local pour
stocker et servir des chunks. Elle complète la spécification de discovery
`sorafs_node_client_protocol.md` et le travail de fixtures SF-1b en détaillant
l'architecture côté stockage, les contrôles de ressources et la plomberie de
configuration qui doivent arriver dans les chemins de code du nœud et du gateway.
Les drills opérateurs se trouvent dans le
[Runbook d'opérations du nœud](./node-operations).

### Objectifs

- Permettre à tout validateur ou processus Iroha auxiliaire d'exposer du disque
  disponible en tant que provider SoraFS sans affecter les responsabilités du ledger.
- Garder le module de stockage déterministe et piloté par Norito : manifests,
  plans de chunk, racines Proof-of-Retrievability (PoR) et adverts de provider
  sont la source de vérité.
- Appliquer des quotas définis par l'opérateur afin qu'un nœud n'épuise pas ses
  propres ressources en acceptant trop de demandes de pin ou de fetch.
- Exposer la santé/la télémétrie (échantillonnage PoR, latence de fetch de chunk,
  pression disque) à la gouvernance et aux clients.

### Architecture de haut niveau

```
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

Modules clés :

- **Gateway** : expose des endpoints HTTP Norito pour les propositions de pin,
  les requêtes de fetch de chunks, l'échantillonnage PoR et la télémétrie. Il
  valide les payloads Norito et achemine les requêtes vers le chunk store. Réutilise
  la pile HTTP Torii existante pour éviter un nouveau daemon.
- **Pin Registry** : l'état des pins de manifest suivi dans `iroha_data_model::sorafs`
  et `iroha_core`. Lorsqu'un manifest est accepté, le registre enregistre le digest
  du manifest, le digest du plan de chunk, la racine PoR et les flags de capacité du
  provider.
- **Chunk Storage** : implémentation `ChunkStore` sur disque qui ingère des manifests
  signés, matérialise les plans de chunk via `ChunkProfile::DEFAULT`, et persiste
  les chunks selon un layout déterministe. Chaque chunk est associé à un fingerprint
  de contenu et à des métadonnées PoR afin que l'échantillonnage puisse revalider
  sans relire le fichier complet.
- **Quota/Scheduler** : impose des limites configurées par l'opérateur (bytes disque max,
  pins en attente max, fetches parallèles max, TTL de chunk) et coordonne l'IO pour que
  les tâches ledger ne soient pas affamées. Le scheduler est également responsable du
  service des preuves PoR et des requêtes d'échantillonnage avec CPU borné.

### Configuration

Ajoutez une nouvelle section à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag optionnel lisible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled` : toggle de participation. Quand false, le gateway renvoie 503 pour les
  endpoints de storage et le nœud ne s'annonce pas en discovery.
- `data_dir` : répertoire racine des données de chunk, arbres PoR et télémétrie de
  fetch. Défaut `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : limite dure pour les chunks pinés. Une tâche de fond
  rejette les nouveaux pins quand la limite est atteinte.
- `max_parallel_fetches` : plafond de concurrence imposé par le scheduler pour
  équilibrer bande passante/IO disque avec la charge du validateur.
- `max_pins` : nombre maximum de pins de manifest que le nœud accepte avant d'appliquer
  eviction/back pressure.
- `por_sample_interval_secs` : cadence des jobs d'échantillonnage PoR automatiques. Chaque job
  échantillonne `N` feuilles (configurable par manifest) et émet des événements de télémétrie.
  La gouvernance peut scaler `N` de manière déterministe via la clé de métadonnées
  `profile.sample_multiplier` (entier `1-4`). La valeur peut être un nombre/une chaîne unique
  ou un objet avec overrides par profil, par exemple `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : structure utilisée par le générateur d'adverts pour remplir les champs
  `ProviderAdvertV1` (stake pointer, hints QoS, topics). Si omis, le nœud utilise les
  defaults du registre de gouvernance.

Plomberie de configuration :

- `[sorafs.storage]` est défini dans `iroha_config` comme `SorafsStorage` et chargé
  depuis le fichier de config du nœud.
- `iroha_core` et `iroha_torii` transmettent la config de storage au builder gateway
  et au chunk store au démarrage.
- Des overrides dev/test existent (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mais
  les déploiements de production doivent s'appuyer sur le fichier de config.

### Utilitaires CLI

Alors que la surface HTTP Torii est encore en cours de câblage, le crate
`sorafs_node` embarque une CLI fine pour que les opérateurs puissent scripter des
exercices d'ingestion/export vers le backend persistant.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` attend un manifest `.to` encodé Norito et les bytes de payload associés.
  Il reconstruit le plan de chunk depuis le profil de chunking du manifest, impose
  la parité des digests, persiste les fichiers de chunk, et émet en option un blob
  JSON `chunk_fetch_specs` pour que les outils downstream puissent valider le layout.
- `export` accepte un ID de manifest et écrit le manifest/payload stocké sur disque
  (avec plan JSON optionnel) pour que les fixtures restent reproductibles.

Les deux commandes impriment un résumé Norito JSON sur stdout, ce qui facilite le
piping dans des scripts. La CLI est couverte par un test d'intégration pour garantir
que manifests et payloads se round-trip correctement avant l'arrivée des APIs Torii.【crates/sorafs_node/tests/cli.rs:1】

> Parité HTTP
>
> Le gateway Torii expose désormais des helpers en lecture seule basés sur le même
> `NodeHandle` :
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — renvoie le manifest
>   Norito stocké (base64) avec digest/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — renvoie le plan de chunk
>   déterministe JSON (`chunk_fetch_specs`) pour les outils downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Ces endpoints reflètent la sortie CLI afin que les pipelines puissent passer
> des scripts locaux aux probes HTTP sans changer de parseurs.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Cycle de vie du nœud

1. **Démarrage** :
   - Si le stockage est activé, le nœud initialise le chunk store avec le répertoire
     et la capacité configurés. Cela inclut la vérification ou la création de la base
     de données de manifest PoR et le replay des manifests pinés pour chauffer les caches.
   - Enregistrer les routes du gateway SoraFS (endpoints Norito JSON POST/GET pour pin,
     fetch, échantillonnage PoR, télémétrie).
   - Lancer le worker d'échantillonnage PoR et le moniteur de quotas.
2. **Discovery / Adverts** :
   - Générer des documents `ProviderAdvertV1` avec la capacité/santé courante, les signer
     avec la clé approuvée par le conseil, et publier via le canal de discovery.
     Utiliser la nouvelle liste `profile_aliases` pour garder les handles canoniques
3. **Workflow de pin** :
   - Le gateway reçoit un manifest signé (incluant plan de chunk, racine PoR, signatures
     du conseil). Valider la liste d'alias (`sorafs.sf1@1.0.0` requis) et s'assurer que
     le plan de chunk correspond aux métadonnées du manifest.
   - Vérifier les quotas. Si les limites de capacité/pins seraient dépassées, répondre
     par une erreur de politique (Norito structuré).
   - Streamer les données de chunk dans `ChunkStore` en vérifiant les digests à l'ingestion.
     Mettre à jour les arbres PoR et stocker les métadonnées du manifest dans le registre.
4. **Workflow de fetch** :
   - Servir les requêtes de range de chunk depuis le disque. Le scheduler impose
     `max_parallel_fetches` et renvoie `429` en cas de saturation.
   - Émettre une télémétrie structurée (Norito JSON) avec latence, bytes servis et
     compteurs d'erreurs pour le monitoring downstream.
5. **Échantillonnage PoR** :
   - Le worker sélectionne les manifests proportionnellement au poids (ex. bytes stockés)
     et exécute un échantillonnage déterministe via l'arbre PoR du chunk store.
   - Persister les résultats pour les audits de gouvernance et inclure des résumés dans
     les adverts provider / endpoints de télémétrie.
6. **Éviction / application des quotas** :
   - Quand la capacité est atteinte, le nœud rejette les nouveaux pins par défaut. En
     option, les opérateurs pourront configurer des politiques d'éviction (ex. TTL, LRU)
     une fois le modèle de gouvernance défini ; pour l'instant, le design suppose des
     quotas stricts et des opérations d'unpin initiées par l'opérateur.

### Déclaration de capacité et intégration du scheduling

- Torii relaie désormais les mises à jour `CapacityDeclarationRecord` depuis `/v1/sorafs/capacity/declare`
  vers le `CapacityManager` embarqué, de sorte que chaque nœud construit une vue en mémoire de ses
  allocations chunker/lane engagées. Le manager expose des snapshots read-only pour la télémétrie
  (`GET /v1/sorafs/capacity/state`) et applique des réservations par profil ou par lane avant que de
  nouvelles commandes ne soient acceptées.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- L'endpoint `/v1/sorafs/capacity/schedule` accepte des payloads `ReplicationOrderV1` émis par la gouvernance.
  Lorsque l'ordre cible le provider local, le manager vérifie la planification en doublon, valide la
  capacité chunker/lane, réserve la tranche, et renvoie un `ReplicationPlan` décrivant la capacité restante
  afin que les outils d'orchestration puissent poursuivre l'ingestion. Les ordres pour d'autres providers
  sont acquittés avec une réponse `ignored` pour faciliter les workflows multi-opérateurs.【crates/iroha_torii/src/routing.rs:4845】
- Des hooks de complétion (par ex. déclenchés après succès d'ingestion) appellent
  `POST /v1/sorafs/capacity/complete` pour libérer les réservations via `CapacityManager::complete_order`.
  La réponse inclut un snapshot `ReplicationRelease` (totaux restants, résiduels chunker/lane) afin que
  les outils d'orchestration puissent queue la commande suivante sans polling. Un travail futur reliera
  cela au pipeline de chunk store lorsque la logique d'ingestion sera prête.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Le `TelemetryAccumulator` embarqué peut être muté via `NodeHandle::update_telemetry`, permettant aux
  workers de fond d'enregistrer des échantillons PoR/uptime et de dériver ensuite des payloads canoniques
  `CapacityTelemetryV1` sans toucher aux internes du scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Intégrations et travail futur

- **Gouvernance** : étendre `sorafs_pin_registry_tracker.md` avec la télémétrie de stockage
  (taux de succès PoR, utilisation disque). Les politiques d'admission peuvent exiger une capacité
  minimale ou un taux de succès PoR minimal avant d'accepter des adverts.
- **SDKs clients** : exposer la nouvelle configuration de storage (limites disque, alias) pour que
  les outils de gestion puissent bootstrapper les nœuds par programme.
- **Télémétrie** : intégrer avec la stack de métriques existante (Prometheus /
  OpenTelemetry) afin que les métriques de storage apparaissent dans les dashboards d'observabilité.
- **Sécurité** : exécuter le module de storage dans un pool de tâches async dédié avec back-pressure
  et envisager le sandboxing des lectures de chunks via io_uring ou des pools tokio bornés pour empêcher
  des clients malveillants d'épuiser les ressources.

Ce design maintient le module de storage optionnel et déterministe tout en donnant aux opérateurs les
boutons nécessaires pour participer à la couche de disponibilité des données SoraFS. Son implémentation
impliquera des changements dans `iroha_config`, `iroha_core`, `iroha_torii` et le gateway Norito, ainsi
que les outils d'advert de provider.
