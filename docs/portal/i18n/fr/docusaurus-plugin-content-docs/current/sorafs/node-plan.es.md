---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : Plan de mise en œuvre du nœud SoraFS
sidebar_label : Plan de mise en œuvre du nœud
description : Convierte la hoja de ruta de almacenamiento SF-3 en trabajo de ingeniería accionable con hitos, tareaes and cobertura de pruebas.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/sorafs_node_plan.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que la documentation héritée de Sphinx soit retirée.
:::

SF-3 entre la caisse d'amorce exécutable `sorafs-node` qui convertit un processus Iroha/Torii dans un fournisseur de stockage SoraFS. Utilisez ce plan conjointement avec le [guide de stockage du nœud](node-storage.md), la [politique d'admission des fournisseurs](provider-admission-policy.md) et la [heure de route du marché de capacité de stockage](storage-capacity-marketplace.md) pour sécuriser les échanges.

## Alcance objetivo (Hito M1)1. **Intégration de l'ensemble des chunks.** Envoyez `sorafs_car::ChunkStore` avec un backend persistant qui garde les octets du chunk, les manifestes et les arbres PoR dans le répertoire de données configuré.
2. **Points de terminaison de la passerelle.** Exposez les points de terminaison HTTP Norito pour l'envoi de broches, la récupération de morceaux, l'affichage de PoR et la télémétrie dans le processus Torii.
3. **Plomería de configuración.** Ajoutez une structure de configuration `SoraFsStorage` (drapeau autorisé, capacité, répertoires, limites de concurrence) câblée aux passages de `iroha_config`, `iroha_core` et `iroha_torii`.
4. **Cuotas/planificación.** Imone límites de disco/paralelismo définies par l'opérateur et encola sollicitudes con contre-pression.
5. **Télémétrie.** Émettre des données/logs pour l'exécution des broches, la latence de récupération des morceaux, l'utilisation de la capacité et les résultats du muestreo PoR.

## Desglose du travail

### A. Structure de la caisse et des modules| Tarée | Responsable(s) | Notes |
|------|---------------|------|
| Créez `crates/sorafs_node` avec les modules : `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Équipement de stockage | Réexporter des types réutilisables pour l'intégration avec Torii. |
| Implémenter `StorageConfig` mapeado à partir de `SoraFsStorage` (utilisateur → réel → valeurs par défaut). | Equipe de stockage / Config WG | Assurez-vous que les capacités Norito/`iroha_config` peuvent être déterminées de manière permanente. |
| Prouvez une méthode `NodeHandle` que Torii utilise pour envoyer des broches/récupérations. | Équipement de stockage | Encapsula interne de stockage et de plomberie asynchrone. |

### B. Almacén de chunks persistants

| Tarée | Responsable(s) | Notes |
|------|---------------|------|
| Construisez un backend en discothèque qui envoie `sorafs_car::ChunkStore` avec un indice de manifestes en discothèque (`sled`/`sqlite`). | Équipement de stockage | Déterministe de mise en page : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Garder les métadonnées PoR (arbres de 64 KiB/4 KiB) en utilisant `ChunkStore::sample_leaves`. | Équipement de stockage | Soporta replay tras reinicios ; falla rapido ante corrupción. |
| Implémenter la relecture de l'intégrité au début (rehash des manifestes, podar pins incompletos). | Équipement de stockage | Bloquea l'arranque de Torii jusqu'à ce que la relecture soit terminée. |

### C. Points de terminaison de la passerelle| Point de terminaison | Comportement | Zones |
|--------------|----------------|--------|
| `POST /sorafs/pin` | Accepter `PinProposalV1`, valider les manifestes, inclure l'ingestion, répondre au CID du manifeste. | Validez le profil du chunker, imponez des cuotas, diffusez des données via le chunk store. |
| `GET /sorafs/chunks/{cid}` + consulter par rango | Sirve octets du chunk avec en-têtes `Content-Chunker` ; respectez les spécifications de capacité de portée. | Planificateur américain + présupposés de flux (vinculaire à capacité de rang SF-2d). |
| `POST /sorafs/por/sample` | Ejecuta muestreo PoR para un manifest y devuelve bundle de pruebas. | Réutilisez le magasin de morceaux, répondez avec les charges utiles Norito JSON. |
| `GET /sorafs/telemetry` | Résumé : capacité, succès de PoR, conteos d'erreurs de récupération. | Fournir des données pour les tableaux de bord/opérateurs. |

La surveillance au moment de l'exécution active les interactions pour les passages de `sorafs_node::por` : le tracker enregistre chaque `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1` pour que les paramètres de `CapacityMeter` reflètent les décrets d'administration sans logique Torii personnalisés.【crates/sorafs_node/src/scheduler.rs#L147】

Notes de mise en œuvre :

- Utilisez la pile Axum de Torii avec les charges utiles `norito::json`.
- Ajoutez les codes Norito pour les réponses (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` expose désormais la profondeur du retard plus l'époque/limite la plus ancienne et les horodatages d'éxito/fallo plus récents par le fournisseur, poussé par `sorafs_node::NodeHandle::por_ingestion_status`, et Torii enregistrent les jauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour tableaux de bord.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Planificateur et cumul de commandes

| Tarée | Détails |
|------|----------|
| Table de discothèque | Suite de bytes en discothèque ; rechaza nouvelles broches al superar `max_capacity_bytes`. Prouvez des crochets d’expulsion pour des politiques futures. |
| Concurrence de récupération | Semáforo global (`max_parallel_fetches`) plus présupposé par le fournisseur obtenu des limites de portée SF-2d. |
| Cola de pins | Limiter les tâches d'ingestion pendantes ; expose les points finaux Norito de l'état pour la profondeur du cola. |
| Cadence PoR | Travailleur en deuxième plan impulsé par `por_sample_interval_secs`. |

### E. Télémétrie et journalisation

Métriques (Prometheus) :

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec étiquettes `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Journaux/événements :- Télémétrie Norito structurée pour l'ingestion de gouvernance (`StorageTelemetryV1`).
- Alertes lorsque l'utilisation est > 90 % ou la fin des chutes est supérieure à l'ombre.

### F. Stratégie de test

1. **Évaluations unitaires.** Persistance du magasin de morceaux, calculs de chaque, variables du planificateur (version `crates/sorafs_node/src/scheduler.rs`).
2. **Projets d'intégration** (`crates/sorafs_node/tests`). Pin → fetch round trip, recuperación tras reinicio, rechazo por cuota, verificación de pruebas de muestreo PoR.
3. **Études d'intégration de Torii.** Exécutez Torii avec un stockage autorisé, en éjectant les points de terminaison HTTP via `assert_cmd`.
4. **Hoja de ruta de caos.** Futuros s'entraîne simultanément à l'agotamiento de disco, IO lento, retiro de provenedores.

## Dépendances

- Politique d'admission SF-2b — s'assurer que les nœuds vérifient les enveloppes d'admission avant l'annonce.
- Marketplace de capacité SF-2c — faire la télémétrie de vuelta aux déclarations de capacité.
- Extensions de publicité SF-2d — capacité de consommation de portée + présupposés de flux lorsqu'ils sont disponibles.

## Critères de sortie du hito- `cargo run -p sorafs_node --example pin_fetch` fonctionne contre les luminaires locaux.
- Torii compilé avec `--features sorafs-storage` et passa tests d'intégration.
- Documentation ([guide d'installation du nœud](node-storage.md)) actualisée avec les paramètres de configuration par défaut + exemples de CLI ; runbook de l'opérateur disponible.
- Télémétrie visible sur les tableaux de bord de mise en scène ; alertes configurées pour la saturation de la capacité et les chutes PoR.

## Entregables de documentation et d'opérations

- Actualiser la [référence de stockage du nœud](node-storage.md) avec les paramètres de configuration par défaut, l'utilisation de la CLI et les étapes de dépannage.
- Garder le [runbook des opérations de nœud](node-operations.md) aligné avec la mise en œuvre conforme à l'évolution de SF-3.
- Publier les références de l'API pour les points de terminaison `/sorafs/*` dans le portail des développeurs et se connecter au manifeste OpenAPI une fois que les gestionnaires de Torii sont répertoriés.