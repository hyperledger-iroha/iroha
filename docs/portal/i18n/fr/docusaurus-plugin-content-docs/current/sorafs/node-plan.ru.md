---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : Plan de réalisation de l'entreprise SoraFS
sidebar_label : Plan de réalisation de votre entreprise
description : Précédez la carte du SF-3 dans le cadre de votre travail, vérifiez et testez.
---

:::note Канонический источник
:::

SF-3 est utilisé pour la première fois dans la caisse `sorafs-node`, lors du processus de préparation Iroha/Torii dans la configuration du fournisseur SoraFS. Utilisez ce plan avec [le propriétaire de votre entreprise](node-storage.md), [les fournisseurs de documents politiques](provider-admission-policy.md) et [le fournisseur La carte du marché contient des informations sur le produit](storage-capacity-marketplace.md) pour l'utilisation ultérieure du robot.

## Целевой объем (веха M1)1. **Иntegрация chunk store.** Téléchargez `sorafs_car::ChunkStore` dans le backend persistant, qui contient des fichiers, des manifestes et des PoR dans le répertoire d'origine. данных.
2. **Activer la passerelle.** Ouvrir les entrées HTTP Norito pour les broches, récupérer les fonctionnalités, l'échantillonnage PoR et les tâches de télémétrie dans le processus de travail Torii.
3. **Configuration de la structure.** Créer la configuration de la structure `SoraFsStorage` (flg включения, емкость, директории, лимиты конкурентности), prouvé par `iroha_config`, `iroha_core` et `iroha_torii`.
4. **Квоты/планирование.** Применять лемить диска/параллеLISма opérateur et ставить запросы в очередь с contre-pression.
5. **Телеметрия.** Émettez des mesures/logiques à l'aide d'une broche, sélectionnez les canaux de récupération, enregistrez les composants et résolvez l'échantillonnage PoR.

## Démarrage du robot

### A. Structure de la création et du module| Задача | Ответственный(е) | Première |
|------|--------|------------|
| Utiliser `crates/sorafs_node` avec les modules : `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Équipe de stockage | Utilisez les types d'utilisation pour l'intégration avec Torii. |
| Réalisez `StorageConfig`, en utilisant `SoraFsStorage` (utilisateur → réel → valeurs par défaut). | Équipe Stockage / Config WG | Garantissez les éléments de dosage Norito/`iroha_config`. |
| Avant de passer à la page `NodeHandle`, le Torii est utilisé pour les broches/récupérations. | Équipe de stockage | Incapsulirovatь внутренности хранения и async-проводку. |

### B. Magasin de morceaux persistants

| Задача | Ответственный(е) | Première |
|------|--------|------------|
| Connectez le backend du disque `sorafs_car::ChunkStore` avec les fichiers d'index sur disque (`sled`/`sqlite`). | Équipe de stockage | Disposition déterminée : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Prend en charge la métadonnée PoR (jusqu'à 64 KiB/4 KiB) à partir de `ChunkStore::sample_leaves`. | Équipe de stockage | Vous pouvez rejouer après le redémarrage ; échouer rapidement при коррупции. |
| Réalisez la relecture d'intégrité au démarrage (répétez les manifestes, чистка незавершенных broches). | Équipe de stockage | Le démarrage Torii est bloqué pour la relecture. |

### C. Passerelle Эндпоинты| Réponse | Поведение | Задачи |
|---------|-----------|--------|
| `POST /sorafs/pin` | En utilisant `PinProposalV1`, vous validez les manifestes, avant l'ingestion, en ouvrant le manifeste CID. | Validez le profil chunker, créez des billets, créez votre propre magasin de chunks. |
| `GET /sorafs/chunks/{cid}` + requête de plage | Отдает байты чанка с заголовками `Content-Chunker` ; Capacité de portée spécifique. | Utiliser le planificateur + la plage horaire (en fonction de la capacité de portée SF-2d). |
| `POST /sorafs/por/sample` | Utilisez l'échantillonnage PoR pour le manifeste et l'ensemble de preuves. | Exploitez le magasin de blocs d'échantillonnage et récupérez les charges utiles JSON Norito. |
| `GET /sorafs/telemetry` | Сводки: емкость, успех PoR, счетчики ошибок chercher. | Prévoyez les dates pour les passagers/opérateurs. |

Le produit de plage de changement de PoR est disponible pour `sorafs_node::por` : le module de transfert de données `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1`, les mesures `CapacityMeter` indiquent la gouvernance des paramètres par rapport à la logique externe Torii.【crates/sorafs_node/src/scheduler.rs#L147】

Exemples de réalisation :

- Utiliser Axum pour les charges utiles Torii avec `norito::json`.
- Ajouter les résultats des schémas Norito (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` permet de gérer le backlog, la même époque/date limite et de suivre les horodatages de réussite/échec pour chaque fournisseur, pour le moment. `sorafs_node::NodeHandle::por_ingestion_status`, et Torii mesures de précision `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour дашбордов.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Planificateur et planification du temps

| Задача | Détails |
|------|--------|
| Chat de disque | Ouvrir le disque sur le disque ; Retirez les nouvelles broches du modèle `max_capacity_bytes`. Подготовить хуки эвикции для будущих политик. |
| Conclure chercher | Sémaphore global (`max_parallel_fetches`) et offres par fournisseur dans les limites de la gamme SF-2d. |
| Ajouter des épingles | Лимитировать незавершенные travaux d'ingestion ; Prévoyez les extrémités du statut Norito pour les lubrifiants. |
| Cadenas PoR | Фоновый воркер, управляемый `por_sample_interval_secs`. |

### E. Télémétrie et logistique

Mesures (Prometheus) :

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogramme avec les métaux `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Логи / события:

- Structure télémétrique Norito pour l'ingestion de gouvernance (`StorageTelemetryV1`).
- Alertes en cas d'utilisation > 90 % ou en cas de pré-alerte de la série PoR-ошибок.### F. Test de la stratégie

1. **Юнит-testы.** Персистентность chunk store, вычисления квот, инварианты planificateur (см. `crates/sorafs_node/src/scheduler.rs`).
2. **Tests de test** (`crates/sorafs_node/tests`). Épingler → récupérer l'aller-retour, восстановление после рестарта, отказ по квоте, проверка PoR proof sampling.
3. **Tests d'intégration Torii.** Téléchargez Torii dans l'interface utilisateur pour programmer l'extrémité HTTP ici. `assert_cmd`.
4. ** Feuille de route chaos.

## Avis

- Le dossier politique SF-2b — il est prévu que vous fournissiez des enveloppes d'admission pour la publication d'une annonce.
- Marché des marchandises SF-2c — Connectez-vous au téléphone avec la déclaration des marchandises.
- Расширения advert SF-2d — использовать la capacité de portée + бюджеты стримов по мере появления.

## Critères de vérification pour chacun

- `cargo run -p sorafs_node --example pin_fetch` s'applique aux luminaires locaux.
- Torii est compatible avec `--features sorafs-storage` et permet des tests d'intégration.
- La documentation ([гайд по хранилищу узла](node-storage.md)) est disponible avec la configuration par défaut + les paramètres CLI ; runbook pour les opérateurs installés.
- Телеметрия видна в tableau de bord de mise en scène ; alertes sur les excédents de marchandises et les fuites de PoR.

## Documentation et exploitation des livrables- Ouvrir [le lien vers l'utilisateur] (node-storage.md) avec les configurations définies, l'utilisation de la CLI et le dépannage.
- Lancez [runbook операций узла](node-operations.md) en lien avec la réalisation de l'évolution de SF-3.
- Déployez les fichiers de l'API `/sorafs/*` sur le portail des robots et connectez-vous au manifeste OpenAPI, ainsi que les gestionnaires Torii.