---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : Plan d'exécution du nœud SoraFS
sidebar_label : Plan d'implémentation du nœud
description : Transformer la feuille de route de stockage SF-3 en travail d'ingénierie actionnable avec jalons, tâches et couverture de tests.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/sorafs_node_plan.md`. Gardez les deux copies synchronisées jusqu'au retrait de la documentation Sphinx historique.
:::

SF-3 livre le premier crate exécutable `sorafs-node` qui transforme un processus Iroha/Torii en fournisseur de stockage SoraFS. Utilisez ce plan avec le [guide de stockage du nœud](node-storage.md), la [politique d'admission des fournisseurs](provider-admission-policy.md) et la [feuille de route du marché de capacité de stockage](storage-capacity-marketplace.md) pour séquencer les livrables.

## Portée cible (Jalon M1)1. **Intégration du chunk store.** Envelopper `sorafs_car::ChunkStore` avec un backend persistant qui stocke les octets de chunk, les manifestes et les arbres PoR dans le répertoire de données configuré.
2. **Endpoints gateway.** Exposer des endpoints HTTP Norito pour la soumission de pin, le fetch de chunks, l'échantillonnage PoR et la télémétrie de stockage dans le processus Torii.
3. **Plomberie de configuration.** Ajouter une structure de config `SoraFsStorage` (flag d'activation, capacité, répertoires, limites de concurrence) reliée à `iroha_config`, `iroha_core` et `iroha_torii`.
4. **Quota/ordonnancement.** Appliquer des limites disque/parallélisme définies par l'opérateur et mettre en fichier les requêtes avec contre-pression.
5. **Télémétrie.** Émettre des métriques/logs pour les succès de pin, la latence de fetch de chunks, l'utilisation de capacité et les résultats d'échantillonnage PoR.

## Décomposition du travail

### A. Structure de caisse et modules| Tâche | Responsable(s) | Remarques |
|------|------|------|
| Créez `crates/sorafs_node` avec les modules : `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Équipe Rangement | Ré-exporter les types réutilisables pour l'intégration Torii. |
| Implémenter `StorageConfig` mappé depuis `SoraFsStorage` (utilisateur → actuel → valeurs par défaut). | Équipe Stockage / Config WG | Garantir que les canapés Norito/`iroha_config` restent déterministes. |
| Fournir une façade `NodeHandle` que Torii utiliser pour soumettre des broches/extractions. | Équipe Rangement | Encapsuler les internes de stockage et la plomberie async. |

### B. Magasin de morceaux persistant

| Tâche | Responsable(s) | Remarques |
|------|------|------|
| Construisez un disque backend enveloppant `sorafs_car::ChunkStore` avec un index de manifeste sur disque (`sled`/`sqlite`). | Équipe Rangement | Implantation déterministe : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintenir les métadonnées PoR (arbres 64 KiB/4 KiB) via `ChunkStore::sample_leaves`. | Équipe Rangement | Supporte le replay après rétablissement ; échouer rapidement en cas de corruption. |
| Implémenter le replay d'intégrité au démarrage (rehash des manifestes, purge des pins incomplets). | Équipe Rangement | Bloquer le démarrage de Torii jusqu'à la fin du replay. |

### C. Passerelle des points de terminaison| Point de terminaison | Comportement | Tâches |
|--------------|--------------|-------|
| `POST /sorafs/pin` | Acceptez `PinProposalV1`, validez les manifestes, avec l'ingestion en fichier, répondez avec le CID du manifeste. | Valider le profil de chunker, appliquer les quotas, streamer les données via le chunk store. |
| `GET /sorafs/chunks/{cid}` + requête de plage | Sert les octets du chunk avec les en-têtes `Content-Chunker` ; respectez les spécifications de capacité de gamme. | Utilisez le planificateur + budgets de flux (lié à la capacité de gamme SF-2d). |
| `POST /sorafs/por/sample` | Lancez un échantillonnage PoR pour un manifeste et renvoyez un bundle de preuves. | Réutilisez l'échantillonnage du chunk store, répondez avec des payloads Norito JSON. |
| `GET /sorafs/telemetry` | Résumés : capacité, succès PoR, compteurs d'erreurs de fetch. | Fournir les données pour tableaux de bord/opérateurs. |

La plomberie runtime relie les interactions PoR via `sorafs_node::por` : le tracker enregistre chaque `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1` pour que les métriques `CapacityMeter` entraînent les verdicts de gouvernance sans logique Torii spécifique.【crates/sorafs_node/src/scheduler.rs#L147】

Notes d'implémentation :

- Utiliser le stack Axum de Torii avec des payloads `norito::json`.
- Ajouter des schémas Norito pour les réponses (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` expose maintenant la profondeur du backlog ainsi que l'époque/échéance la plus ancienne et les timestamps de succès/échec les plus récents par fournisseur, via `sorafs_node::NodeHandle::por_ingestion_status`, et Torii enregistrer les jauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour les tableaux de bord.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Planificateur et application des quotas

| Tâche | Détails |
|------|--------------|
| Quota disque | Suivre les octets sur disque ; rejeter les nouvelles broches au-delà de `max_capacity_bytes`. Fournir des crochets d'éviction pour des politiques futures. |
| Concurrence de récupération | Sémaphore global (`max_parallel_fetches`) plus budgets par fournisseur issu des caps de gamme SF-2d. |
| Fichier de broches | Limiter les jobs d'ingestion en attente ; exposer les points finaux de statut Norito pour la profondeur de fichier. |
| Cadence PoR | Ouvrier de fond piloté par `por_sample_interval_secs`. |

### E. Télémétrie et logging

Métriques (Prometheus) :

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec étiquettes `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Journaux / événements :- Télémétrie Norito structurée pour l'ingestion gouvernance (`StorageTelemetryV1`).
- Alertes lorsque l'utilisation > 90 % ou que la série d'échecs PoR dépasse le seuil.

### F. Stratégie de tests

1. **Tests unitaires.** Persistance du chunk store, calculs de quota, invariants du planificateur (voir `crates/sorafs_node/src/scheduler.rs`).
2. **Tests d'intégration** (`crates/sorafs_node/tests`). Pin → fetch aller-retour, reprise après restauré, rejet de quota, vérification de preuve d'échantillonnage PoR.
3. **Tests d'intégration Torii.** Exécuter Torii avec le stockage activé, exécuter les points de terminaison HTTP via `assert_cmd`.
4. **Roadmap chaos.** Des exercices futurs simulent l'épuisement du disque, l'IO lente, la suppression des fournisseurs.

## Dépendances

- Politique d'admission SF-2b — s'assurer que les nœuds vérifient les enveloppes d'admission avant la publication d'annonces.
- Marketplace de capacité SF-2c — rattacher la télémétrie aux déclarations de capacité.
- Extensions d'annonce SF-2d — consommer la capacité de gamme + budgets de flux dès disponibilité.

## Critères de sortie du jalon- `cargo run -p sorafs_node --example pin_fetch` fonctionne sur les appareils locaux.
- Torii build avec `--features sorafs-storage` et réussir les tests d'intégration.
- Documentation ([guide de stockage du noeud](node-storage.md)) mise à jour avec défauts de configuration + exemples CLI ; opérateur runbook disponible.
- Télémétrie visible sur les tableaux de bord de mise en scène ; alertes configurées pour saturation de capacité et échecs PoR.

## Documentation et opérations des Livrables

- Mettre à jour la [référence de stockage du nœud](node-storage.md) avec les valeurs par défaut de configuration, l'utilisation CLI et les étapes de dépannage.
- Garder le [runbook d'opérations du nœud](node-operations.md) aligné avec l'implémentation au fur et à mesure de l'évolution SF-3.
- Publier les références API pour les endpoints `/sorafs/*` dans le portail développeur et les relier au manifeste OpenAPI une fois les handlers Torii en place.