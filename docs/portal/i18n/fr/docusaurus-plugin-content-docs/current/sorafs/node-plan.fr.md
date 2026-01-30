---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-plan
title: Plan d'implémentation du nœud SoraFS
sidebar_label: Plan d'implémentation du nœud
description: Transformer la feuille de route de stockage SF-3 en travail d'ingénierie actionnable avec jalons, tâches et couverture de tests.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/sorafs_node_plan.md`. Gardez les deux copies synchronisées jusqu'au retrait de la documentation Sphinx historique.
:::

SF-3 livre le premier crate exécutable `sorafs-node` qui transforme un processus Iroha/Torii en fournisseur de stockage SoraFS. Utilisez ce plan avec le [guide de stockage du nœud](node-storage.md), la [politique d'admission des providers](provider-admission-policy.md) et la [feuille de route du marketplace de capacité de stockage](storage-capacity-marketplace.md) pour séquencer les livrables.

## Portée cible (Jalon M1)

1. **Intégration du chunk store.** Envelopper `sorafs_car::ChunkStore` avec un backend persistant qui stocke les bytes de chunk, les manifests et les arbres PoR dans le répertoire de données configuré.
2. **Endpoints gateway.** Exposer des endpoints HTTP Norito pour la soumission de pin, le fetch de chunks, l'échantillonnage PoR et la télémétrie de stockage dans le processus Torii.
3. **Plomberie de configuration.** Ajouter une structure de config `SoraFsStorage` (flag d'activation, capacité, répertoires, limites de concurrence) reliée à `iroha_config`, `iroha_core` et `iroha_torii`.
4. **Quota/ordonnancement.** Appliquer des limites disque/parallélisme définies par l'opérateur et mettre en file les requêtes avec back-pressure.
5. **Télémétrie.** Émettre des métriques/logs pour les succès de pin, la latence de fetch de chunks, l'utilisation de capacité et les résultats d'échantillonnage PoR.

## Décomposition du travail

### A. Structure de crate et modules

| Tâche | Responsable(s) | Notes |
|------|----------------|------|
| Créer `crates/sorafs_node` avec les modules : `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Équipe Storage | Ré-exporter les types réutilisables pour l'intégration Torii. |
| Implémenter `StorageConfig` mappé depuis `SoraFsStorage` (user → actual → defaults). | Équipe Storage / Config WG | Garantir que les couches Norito/`iroha_config` restent déterministes. |
| Fournir une façade `NodeHandle` que Torii utilise pour soumettre pins/fetches. | Équipe Storage | Encapsuler les internes de stockage et la plomberie async. |

### B. Chunk store persistant

| Tâche | Responsable(s) | Notes |
|------|----------------|------|
| Construire un backend disque enveloppant `sorafs_car::ChunkStore` avec un index de manifest sur disque (`sled`/`sqlite`). | Équipe Storage | Layout déterministe : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintenir les métadonnées PoR (arbres 64 KiB/4 KiB) via `ChunkStore::sample_leaves`. | Équipe Storage | Supporte le replay après redémarrage ; fail fast en cas de corruption. |
| Implémenter le replay d'intégrité au démarrage (rehash des manifests, purge des pins incomplets). | Équipe Storage | Bloque le démarrage de Torii jusqu'à la fin du replay. |

### C. Endpoints gateway

| Endpoint | Comportement | Tâches |
|----------|--------------|-------|
| `POST /sorafs/pin` | Accepte `PinProposalV1`, valide les manifests, met l'ingestion en file, répond avec le CID du manifest. | Valider le profil de chunker, appliquer les quotas, streamer les données via le chunk store. |
| `GET /sorafs/chunks/{cid}` + requête de range | Sert les bytes de chunk avec les headers `Content-Chunker` ; respecte la spécification de capacité de range. | Utiliser le scheduler + budgets de stream (lié à la capacité de range SF-2d). |
| `POST /sorafs/por/sample` | Lance un échantillonnage PoR pour un manifest et renvoie un bundle de preuve. | Réutiliser l'échantillonnage du chunk store, répondre avec des payloads Norito JSON. |
| `GET /sorafs/telemetry` | Résumés : capacité, succès PoR, compteurs d'erreurs de fetch. | Fournir les données pour dashboards/opérateurs. |

La plomberie runtime relie les interactions PoR via `sorafs_node::por` : le tracker enregistre chaque `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1` pour que les métriques `CapacityMeter` reflètent les verdicts de gouvernance sans logique Torii spécifique.【crates/sorafs_node/src/scheduler.rs#L147】

Notes d'implémentation :

- Utiliser le stack Axum de Torii avec des payloads `norito::json`.
- Ajouter des schémas Norito pour les réponses (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` expose maintenant la profondeur du backlog ainsi que l'époque/échéance la plus ancienne et les timestamps de succès/échec les plus récents par provider, via `sorafs_node::NodeHandle::por_ingestion_status`, et Torii enregistre les gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour les dashboards.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Scheduler et application des quotas

| Tâche | Détails |
|------|---------|
| Quota disque | Suivre les bytes sur disque ; rejeter les nouveaux pins au-delà de `max_capacity_bytes`. Fournir des hooks d'éviction pour des politiques futures. |
| Concurrence de fetch | Sémaphore global (`max_parallel_fetches`) plus budgets par provider issus des caps de range SF-2d. |
| File de pins | Limiter les jobs d'ingestion en attente ; exposer des endpoints de statut Norito pour la profondeur de file. |
| Cadence PoR | Worker de fond piloté par `por_sample_interval_secs`. |

### E. Télémétrie et logging

Métriques (Prometheus) :

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec labels `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logs / événements :

- Télémétrie Norito structurée pour l'ingestion gouvernance (`StorageTelemetryV1`).
- Alertes quand l'utilisation > 90 % ou que la série d'échecs PoR dépasse le seuil.

### F. Stratégie de tests

1. **Tests unitaires.** Persistance du chunk store, calculs de quota, invariants du scheduler (voir `crates/sorafs_node/src/scheduler.rs`).
2. **Tests d'intégration** (`crates/sorafs_node/tests`). Pin → fetch aller-retour, reprise après redémarrage, rejet de quota, vérification de preuve d'échantillonnage PoR.
3. **Tests d'intégration Torii.** Exécuter Torii avec le stockage activé, exercer les endpoints HTTP via `assert_cmd`.
4. **Roadmap chaos.** Des drills futurs simulent l'épuisement du disque, l'IO lente, la suppression de providers.

## Dépendances

- Politique d'admission SF-2b — s'assurer que les nœuds vérifient les envelopes d'admission avant publication d'adverts.
- Marketplace de capacité SF-2c — rattacher la télémétrie aux déclarations de capacité.
- Extensions d'advert SF-2d — consommer la capacité de range + budgets de stream dès disponibilité.

## Critères de sortie du jalon

- `cargo run -p sorafs_node --example pin_fetch` fonctionne sur des fixtures locales.
- Torii build avec `--features sorafs-storage` et passe les tests d'intégration.
- Documentation ([guide de stockage du nœud](node-storage.md)) mise à jour avec defaults de configuration + exemples CLI ; runbook opérateur disponible.
- Télémétrie visible sur les dashboards de staging ; alertes configurées pour saturation de capacité et échecs PoR.

## Livrables documentation et ops

- Mettre à jour la [référence de stockage du nœud](node-storage.md) avec les defaults de configuration, l'usage CLI et les étapes de troubleshooting.
- Garder le [runbook d'opérations du nœud](node-operations.md) aligné avec l'implémentation au fur et à mesure de l'évolution SF-3.
- Publier les références API pour les endpoints `/sorafs/*` dans le portail développeur et les relier au manifeste OpenAPI une fois les handlers Torii en place.
