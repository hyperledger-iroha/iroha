---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : Plan de mise en œuvre du nœud SoraFS
sidebar_label : Plan de mise en œuvre du nœud
description : Convertissez la feuille de route de stockage SF-3 en travail d'ingénierie actif avec les marques, les tarifications et la couverture des testicules.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx alternativa seja retirada.
:::

SF-3 entre le premier exécutable de caisse `sorafs-node` qui transforme un processus Iroha/Torii dans un fournisseur de stockage SoraFS. Utilisez ce plan conjointement avec la [guide de stockage du nœud](node-storage.md), la [politique d'admission des fournisseurs](provider-admission-policy.md) et la [feuille de route du marché de capacité de stockage](storage-capacity-marketplace.md) pour séquencer les échanges.

## Escopo alvo (Marco M1)1. **Intégration du chunk store.** Envolver `sorafs_car::ChunkStore` avec un backend persistant qui arme les octets du chunk, se manifeste et se présente dans le répertoire de données configuré.
2. **Points de terminaison de la passerelle.** Exporter les points de terminaison HTTP Norito pour la soumission du code PIN, récupérer les morceaux, stocker le PoR et la télémétrie de stockage dans le processus Torii.
3. **Plumbing de configuracao.** Ajouter une structure de configuration `SoraFsStorage` (drapeau autorisé, capacité, répertoires, limites de cohérence) connectée via `iroha_config`, `iroha_core` et `iroha_torii`.
4. **Quota/agenda.** Impor limites de disco/paralelismo definidos pelo operador e enfileirar requisicoes com back-pression.
5. **Télémétrie.** Émettre des mesures/journaux pour réussir la broche, latence de récupération des morceaux, utilisation de la capacité et résultats de la sauvegarde PoR.

## Quebra de travail

### A. Structure de la caisse et des modules| Taréfa | Don(s) | Notes |
|------|---------|------|
| Criar `crates/sorafs_node` avec modules : `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Équipe de stockage | Types de réexportation réutilisés pour l'intégration avec Torii. |
| Implémenter `StorageConfig` mapeado de `SoraFsStorage` (utilisateur -> réel -> valeurs par défaut). | Équipe Stockage / Config WG | Garante que as camadas Norito/`iroha_config` permanecam deterministicas. |
| Fornecer une façade `NodeHandle` et Torii utiliser pour les broches/récupérations du sous-mètre. | Équipe de stockage | Encapsule interne de stockage et de plomberie asynchrone. |

### B. Magasin de morceaux persistant

| Taréfa | Don(s) | Notes |
|------|---------|------|
| Construisez un backend en discothèque en utilisant `sorafs_car::ChunkStore` avec l'indice de manifeste en discothèque (`sled`/`sqlite`). | Équipe de stockage | Déterministe de la mise en page : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Manter metadodos PoR (archives 64 KiB/4 KiB) en utilisant `ChunkStore::sample_leaves`. | Équipe de stockage | Suporta replay après redémarrage ; falha rapido em corrupcao. |
| Implémenter la relecture de l'intégrité au démarrage (réédition des manifestes, broches incomplètes). | Équipe de stockage | Bloqueia o start de Torii ate or replay terminal. |

### C. Points de terminaison de la passerelle| Point de terminaison | Comportement | Tarefas |
|--------------|--------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`, valide les manifestes, enfileira ingestao, répond au CID du manifeste. | Valider le profil du chunker, importer des quotas, diffuser des données via le chunk store. |
| `GET /sorafs/chunks/{cid}` + requête de plage | Servir les octets du chunk avec les en-têtes `Content-Chunker` ; respectez les spécifications de capacité de portée. | Utiliser le planificateur + les orcamentos de stream (ligar a capacidade de range SF-2d). |
| `POST /sorafs/por/sample` | Rodar amostragem PoR para um manifest e retornar bundle de prova. | Réutiliser le magasin de morceaux, répondre avec les charges utiles Norito JSON. |
| `GET /sorafs/telemetry` | Résumé : capacité, succès PoR, contadores de erro de fetch. | Fornecer dados para tableaux de bord/opérateurs. |

La plomberie dans l'exécution passe en tant qu'interface PoR via `sorafs_node::por` : le tracker enregistre chaque `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1` pour que les mesures `CapacityMeter` reflitam soient des vérifications de gouvernance selon la logique Torii sur mesure. [crates/sorafs_node/src/scheduler.rs:147]

Notes de mise en œuvre :

- Utilisez la pile Axum de Torii avec les charges utiles `norito::json`.
- Ajout des schémas Norito pour les réponses (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).- `/v2/sorafs/por/ingestion/{manifest_digest_hex}` a récemment exposé la profondeur du backlog plus à l'époque/date limite plus ancienne et les horodatages plus récents de succès/échec par fournisseur, via `sorafs_node::NodeHandle::por_ingestion_status`, et Torii enregistrent les jauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour tableaux de bord. [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src/metrics.rs:5390]

### D. Planificateur et gestion des quotas

| Taréfa | Détails |
|------|--------------|
| Quota de discothèque | Rastrear octets en discothèque ; Rejeitar novos pins ao exceder `max_capacity_bytes`. Fornecer hooks de eviccao para politicas futurs. |
| Concordance de récupération | Semaforo global (`max_parallel_fetches`) plus d'orcamentos por provenor oriundos de caps de range SF-2d. |
| Fila de pins | Limiter les tâches d'ingestao pendentes; expor endpoints Norito de status para profundidade da fila. |
| Cadence PoR | Travailleur d'arrière-plan dirigé par `por_sample_interval_secs`. |

### E. Télémétrie et journalisation

Métriques (Prometheus) :

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec étiquettes `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Journaux/événements :- Télémétrie Norito structurée pour l'administration de la gouvernance (`StorageTelemetryV1`).
- Alertes lorsque l'utilisation est > 90 % ou que la séquence d'erreurs PoR dépasse le seuil.

### F. Stratégie testiculaire

1. **Tests unitaires.** Persistance du magasin de morceaux, calculs de quota, invariants du planificateur (version `crates/sorafs_node/src/scheduler.rs`).
2. **Testes d'intégration** (`crates/sorafs_node/tests`). Épingler -> récupérer l'aller-retour, récupération après redémarrage, rejet du quota, vérification des preuves d'amostragem PoR.
3. **Tests d'intégration Torii.** Rodar Torii avec stockage autorisé, exercer les points de terminaison HTTP via `assert_cmd`.
4. **Roadmap de caos.** Drills futurs simulam exaustao de disco, IO lento, remocao de provenores.

## Dépendances

- Politique d'admission SF-2b - garantir que les nœuds vérifient les enveloppes d'admission avant l'annonce.
- Marketplace de capacité SF-2c - relier les télémétries de tension aux déclarations de capacité.
- Extensions d'annonce SF-2d - consommation de capacité de gamme + orcamentos de stream lorsqu'ils sont disponibles.

## Critères de saya do marco- `cargo run -p sorafs_node --example pin_fetch` fonction contre les luminaires locaux.
- Torii compile avec `--features sorafs-storage` et passe les tests d'intégration.
- Documentation ([guia de storage do nodo](node-storage.md)) actualisée avec les paramètres de configuration par défaut + exemples de CLI ; runbook de l'opérateur disponible.
- Télémétrie visible dans les tableaux de bord de mise en scène ; alertes configurées pour la saturation de la capacité et les erreurs PoR.

## Entreprend des documents et des opérations

- Actualiser la [référence de stockage du nœud](node-storage.md) avec les paramètres de configuration par défaut, l'utilisation de la CLI et les étapes de dépannage.
- Manter o [runbook de operacoes de nodo](node-operations.md) a été conçu avec une implémentation conforme à SF-3 évolutive.
- Publier les références de l'API pour les points de terminaison `/sorafs/*` dans le portail du développeur et se connecter au manifeste OpenAPI avec les gestionnaires de Torii qui seront bientôt disponibles.