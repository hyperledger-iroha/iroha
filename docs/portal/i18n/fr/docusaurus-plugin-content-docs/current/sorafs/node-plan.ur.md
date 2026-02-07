---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : Plan de mise en œuvre du nœud SoraFS
sidebar_label : plan de mise en œuvre du nœud
description : Feuille de route du stockage SF-3, jalons, tâches et couverture des tests, travaux d'ingénierie exploitables.
---

:::note مستند ماخذ
:::

SF-3 est un système de caisse exécutable `sorafs-node` et un processus Iroha/Torii et un fournisseur de stockage SoraFS. Voir [Guide de stockage des nœuds] (node-storage.md) et [Politique d'admission du fournisseur] (provider-admission-policy.md) et [Feuille de route du marché de la capacité de stockage] (storage-capacity-marketplace.md) جب séquence des livrables کریں۔

## Portée cible (jalon M1)1. **Intégration du magasin de morceaux.** `sorafs_car::ChunkStore` est un backend persistant et un wrapper et un répertoire de données configuré avec des octets de morceaux, des manifestes et des arbres PoR.
2. **Points de terminaison de la passerelle.** Processus Torii pour la soumission de broches, la récupération de morceaux, l'échantillonnage PoR et la télémétrie de stockage et les points de terminaison HTTP Norito exposent les points de terminaison HTTP.
3. ** Plomberie de configuration. ** Structure de configuration `SoraFsStorage` (indicateur activé, capacité, répertoires, limites de concurrence) شامل کریں اور `iroha_config`, `iroha_core`, `iroha_torii` کے ذریعے fil کریں۔
4. **Quota/planification.** Les limites de disque/parallélisme définies par l'opérateur appliquent les requêtes de transfert, la contre-pression et la file d'attente.
5. **Télémétrie.** Réussite des broches, latence de récupération de fragments, utilisation de la capacité et résultats d'échantillonnage PoR et les métriques/journaux émettent des messages.

## Répartition du travail

### A. Structure de la caisse et du module| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| `crates/sorafs_node` Modules `config`, `store`, `gateway`, `scheduler`, `telemetry` modules noir | Équipe de stockage | Torii intégration pour réexportation de types réutilisables |
| `StorageConfig` implémente le système et `SoraFsStorage` mappé (utilisateur → réel → valeurs par défaut) | Équipe Stockage / Config WG | Couches Norito/`iroha_config` et couches déterministes |
| Torii broches/récupérations de façade `NodeHandle` façade de façade | Équipe de stockage | les composants internes de stockage et la plomberie asynchrone encapsulent les éléments internes |

### B. Magasin de morceaux persistants

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| `sorafs_car::ChunkStore` et index de manifeste sur disque (`sled`/`sqlite`) pour le backend du disque et le wrapper | Équipe de stockage | Disposition déterministe : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` Les métadonnées PoR (arborescences de 64 Ko/4 Ko) maintiennent les métadonnées PoR | Équipe de stockage | Redémarrez pour prendre en charge la relecture la corruption échoue rapidement |
| Démarrage et mise en œuvre de relecture d'intégrité (manifeste rehash, élagage incomplet des broches) | Équipe de stockage | replay مکمل ہونے تک Torii start block کریں۔ |

### C. Points de terminaison de la passerelle| Point de terminaison | Comportement | Tâches |
|----------|-----------|-------|
| `POST /sorafs/pin` | `PinProposalV1` Les manifestes valident la file d'attente d'ingestion et le CID du manifeste et le client | profil de bloc valider les quotas appliquer le magasin de blocs et le flux de données |
| `GET /sorafs/chunks/{cid}` + requête de plage | Les en-têtes `Content-Chunker` et les octets de fragments servent ici. gamme capacité spécification respect کریں۔ | planificateur + budgets de flux en cours (capacité de gamme SF-2d en cas d'égalité) |
| `POST /sorafs/por/sample` | manifeste et échantillonnage PoR et paquet de preuves et paquet de preuves | réutilisation de l'échantillonnage du magasin de fragments Les charges utiles JSON Norito répondent correctement |
| `GET /sorafs/telemetry` | Résumés : capacité, réussite du PoR, nombre d'erreurs de récupération | tableaux de bord/opérateurs et données |

Runtime Plumbing `sorafs_node::por` pour les interactions PoR et les threads : tracker pour `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1` et enregistrement Les verdicts de gouvernance des métriques `CapacityMeter` et la logique spécifique à Torii reflètent les éléments suivants : crates/sorafs_node/src/scheduler.rs#L147

Notes de mise en œuvre :

- Torii pour la pile Axum et les charges utiles `norito::json` pour les charges utiles
- réponses aux schémas Norito et aux schémas (`PinResultV1`, `FetchErrorV1`, structures de télémétrie)- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` pour la profondeur du backlog, l'époque/date limite la plus ancienne et le fournisseur, les horodatages de réussite/échec récents et `sorafs_node::NodeHandle::por_ingestion_status`, alimenté par `/v1/sorafs/por/ingestion/{manifest_digest_hex}`. Les tableaux de bord Torii et les jauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` enregistrent ہے۔【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Planification et application des quotas

| Tâche | Détails |
|------|--------------|
| Quota de disque | Piste des octets du disque `max_capacity_bytes` les broches rejetées sont rejetées مستقبل کی politiques کے لیے crochets d'expulsion فراہم کریں۔ |
| Récupérer la simultanéité | Sémaphore global (`max_parallel_fetches`) pour les budgets par fournisseur et les plafonds de plage SF-2d pour chaque fournisseur |
| File d'attente des broches | Travaux d'ingestion exceptionnels محدود کریں؛ profondeur de file d'attente et les points de terminaison d'état Norito exposent les points de terminaison |
| Cadence PoR | `por_sample_interval_secs` pour un travailleur en arrière-plan |

### E. Télémétrie et journalisation

Métriques (Prometheus) :

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec étiquettes `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Journaux/événements :

- ingestion de gouvernance par télémétrie structurée Norito (`StorageTelemetryV1`).
- utilisation > 90 % et seuil de séquence de défaillances PoR et alertes### F. Stratégie de test

1. **Tests unitaires.** persistance du magasin de morceaux, calculs de quotas, invariants du planificateur (voir `crates/sorafs_node/src/scheduler.rs`).
2. **Tests d'intégration** (`crates/sorafs_node/tests`). Épingler → récupérer l'aller-retour, redémarrer la récupération, rejet de quota, vérification de la preuve d'échantillonnage PoR.
3. **Tests d'intégration Torii.** Torii pour le stockage activé pour les points de terminaison HTTP et pour l'exercice `assert_cmd`.
4. **Feuille de route du chaos.** L'objectif est de percer l'épuisement du disque et de simuler la suppression lente du fournisseur d'E/S.

## Dépendances

- Politique d'admission SF-2b — les nœuds et les annonces publient les enveloppes d'admission et vérifient les conditions d'admission.
- Marché de capacité SF-2c — télémétrie et déclarations de capacité et cravates
- Extensions d'annonces SF-2d — capacité de portée + budgets de flux pour consommer plus

## Critères de sortie d'étape

- Appareils locaux `cargo run -p sorafs_node --example pin_fetch` pour les appareils locaux
- Torii `--features sorafs-storage` pour construire et tester l'intégration
- Documentation ([guide de stockage des nœuds] (node-storage.md)) paramètres par défaut + exemples CLI mis à jour maintenant opérateur runbook دستیاب ہو۔
- Tableaux de bord de mise en scène de télémétrie saturation de la capacité et pannes PoR et alertes configurer

## Livrables de documentation et d'opérations- [référence de stockage du nœud] (node-storage.md) pour les valeurs par défaut de configuration, l'utilisation de la CLI et les étapes de dépannage pour la mise à jour
- [runbook des opérations de nœud] (node-operations.md) pour la mise en œuvre et l'alignement des tâches pour que SF-3 évolue et
- Points de terminaison `/sorafs/*` et références API du portail des développeurs pour publier des gestionnaires Torii et un manifeste OpenAPI avec fil.