---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
title : Plan de planification et de SLO pour SoraFS
sidebar_label : Démarrer et SLO
description : Le schéma télémétrique, national et politique est prévu pour les passerelles SoraFS, les utilisateurs et les opérateurs multiples.
---

:::note Канонический источник
Cette page présente un plan différent, qui se trouve dans le `docs/source/sorafs_observability_plan.md`. Vous pouvez obtenir des copies de synchronisation si le Sphinx ne fonctionne pas correctement.
:::

## Celi
- Prévoir des mesures et des structures pour les passerelles, les entreprises et les opérateurs multi-établissements.
- Préparer les passeports Grafana, envoyant des alertes et des crochets de validation.
- Le bureau du SLO est au pouvoir avec les dirigeants politiques et le chaos.

## Métrique du catalogue

### Passerelle de sécurité| Métrique | Astuce | Metki | Première |
|---------|-----|-------|------------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis par `SorafsGatewayOtel` ; Vous pouvez utiliser les opérations HTTP en combinant point de terminaison/méthode. |
| `sorafs_gateway_responses_total` | Compteur | | Каждая завершенная gateway-запросом операция увеличивает счетчик один раз; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | Temps de latence jusqu'au premier octet pour la passerelle d'accès ; exportation vers Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | Les résultats des tests documentaires sont effectués à l'instant même (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Распределение латентности проверки для PoR-replik. || `telemetry::sorafs.gateway.request` | Structures d'entreprise | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Journal structuré, émis pour la planification de la correspondance avec Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Compteur | Métaux individuels | Mesures Prometheus, liées à l'histoire des frontières; Il s'agit de la nouvelle série OTLP. |

La société `telemetry::sorafs.gateway.request` propose des systèmes OTEL avec des charges utiles structurées, en utilisant `endpoint`, `method`, `variant`, `status`, `error_code` et `duration_ms` pour la correction de Loki/Tempo, ainsi que les frontières utilisant la série OTLP pour la surveillance SLO.

### Télémétrie pour les documents| Métrique | Astuce | Metki | Première |
|---------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | L'augmentation du temps passe par `RecordCapacityTelemetry` et `SorafsProofHealthAlert`. `trigger` active PDP/PoTR/Both, et `penalty` permet de créer un temps de recharge vraiment efficace ou rapide. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | Après la mise en place du PDP/PoTR dans le cadre d'un problème de télémétrie, les commandes peuvent être utilisées par les fournisseurs de la politique précédente. |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | Par exemple, Nano-XOR vous envoie une alerte ultérieure (mais si le temps de recharge est prévu). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | Jauge bleue (`1` = alerte pendant le temps de recharge), vous pouvez le faire, puis les alertes se déclenchent régulièrement. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | À l'heure actuelle, les télémètres sont en contact avec l'alerte, les opérateurs peuvent corréler les articles d'art Norito. |

Cela signifie que vous avez un coup de pouce proof-health à bord du Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), l'opérateur CDN vous montre la vidéo
Il s'agit d'alertes, de déclenchements multiples de PDP/PoTR, de pénalités et de temps de recharge pour
провайдерам.Ces mesures vous permettent de diffuser les alertes du visualiseur Taikai :
`SorafsProofHealthPenalty` sрабатывает, когда
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`
Après 15 minutes, un avertissement `SorafsProofHealthCooldown` a été émis, sinon
Le fournisseur s'installe pendant le temps de recharge pendant une minute. Оба алерта живут в
`dashboards/alerts/taikai_viewer_rules.yml`, pour les SRE dans le contexte
nécessaire pour l'augmentation de la valeur PoR/PoTR.

### Organisateur public| Métrique / Событие | Astuce | Metki | Projet | Première |
|-------------------|-----|-------|---------------|------------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | Сессии, находящиеся в полете. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма длительности в милLISекундах; seaux de 1 ms à 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Prix : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Sélectionnez les retours (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Fixez l'ouverture et les sorties pendant votre séance. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Réduire la latence de récupération d'un morceau (ms) pour analyser le débit/SLO. |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, доставленные на manifeste/fournisseur ; le débit est obtenu avec `rate()` dans PromQL. |
| `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Coupez les morceaux, en passant par `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | Structures d'entreprise | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Démarrez votre tâche (démarrage/terminée) avec la charge utile JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Structures d'entreprise | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis à chaque série de retours du fournisseur ; `attempts` считает инкрементальные ретраи (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Structures d'entreprise | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Публикуется, когда провайдер пересекает порог отказов. |
| `telemetry::sorafs.fetch.error` | Structures d'entreprise | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | La dernière fois que vous l'avez téléchargé, vous pouvez l'ingérer dans Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Structures d'entreprise | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Vous remarquerez que votre morceau de latence dépasse la limite maximale (pour les décrochages). |

### Поверхности узлов / репликации| Métrique | Astuce | Metki | Première |
|---------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | L'hôtel OTEL utilise une chaîne de transport (exportée par `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | Un planificateur monotone est un PoR-выборок, un planificateur d'instantanés. |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | Монотонный счетчик неуспешных PoR-выборок. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | Jauge Prometheus adaptée pour les bateaux, les balles et les avions en vol PoR. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | Vous pouvez donc utiliser un fournisseur de capacité/disponibilité, qui est disponible dans les locaux du pays. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | Le carnet de commandes et les dossiers d'exportation du groupe sont indiqués lors de l'ouverture du `/v1/sorafs/por/ingestion/{manifest}`, sur le panneau/alerte "PoR Stalls". |

### Preuve de récupération en temps opportun (PoTR) et morceaux de SLA| Métrique | Astuce | Metki | Projet | Première |
|---------|-----|-------|---------------|------------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Coordinateur PoTR | Запас по дедлайну в миллисекундах (положительный = выполнен). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Coordinateur PoTR | Prix : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | Moniteur SLA | Attention, la livraison des morceaux n'est pas effectuée dans SLO (taux de réussite). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | Moniteur SLA | La jauge supérieure (0/1) est indiquée lors de la période d'activité lors de la lecture. |

## Celi SLO

- Passerelle sans confiance : **99,9 %** (HTTP 2xx/304 réponses).
- TTFB P95 sans confiance : niveau chaud ≤ 120 ms, niveau chaud ≤ 300 ms.
- Taux de référence : ≥ 99,5 % par jour.
- Успех оркестратора (завершение morceaux): ≥ 99%.

## Daschbords et alertes1. **Démarrer la passerelle** (`dashboards/grafana/sorafs_gateway_observability.json`) — Démarrez la connexion sans confiance, TTFB P95, effectuez des achats et utilisez PoR/PoTR selon les mesures OTEL.
2. **Здоровье оркестратора** (`dashboards/grafana/sorafs_fetch_observability.json`) — покрывает нагрузку multi-sources, ретраи, отказы провайдеров и всплески stands.
3. **Метрики приватности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — graphiques anonymisés des seaux de relais, destinés à la mise à jour et au collecteur de commande pour `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` et `soranet_privacy_poll_errors_total{provider}`.

Paquets d'alertes :

- `dashboards/alerts/sorafs_gateway_rules.yml` — passerelle de téléchargement, TTFB, téléchargements de documents.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/ретраи/stands оркестратора ; Validez pour `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` et `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — Dégradations privées, suivi des alarmes, détection du collecteur inactif et alertes du collecteur d'ouverture (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de baisse de tension anonyme, transmises par `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes de retard/ingest/CEK lag Taikai viewer ainsi que de nouvelles alertes pénalité/rechargement fournies par SoraFS, appliquées à `torii_sorafs_proof_health_*`.

## Stratégie de transition- Utiliser OpenTelemetry de bout en bout :
  - Les passerelles émettent des étendues OTLP (HTTP) avec des identifiants de requête, des résumés de manifeste et des hachages de jetons.
  - L'opérateur utilise `tracing` + `opentelemetry` pour l'exportation.
  - Les étendues d'exportation SoraFS sont disponibles pour l'exploitation PoR et l'exploitation du stockage. Tous les composants permettent d'obtenir l'ID de trace, avant `x-sorafs-trace`.
- `SorafsFetchOtel` est la plupart des opérateurs de mesures dans les historiques OTLP, et `telemetry::sorafs.fetch.*` utilise les charges utiles JSON pour les backends de gestion des journaux.
- Collecteurs : запускайте OTEL collectors рядом с Prometheus/Loki/Tempo (Tempo предпочтителен). Les exportateurs Jaeger-совместимые остаются опциональными.
- Les opérations avec votre carte cardinale sont simplifiées (10 % pour les clients potentiels, 100 % pour les achats).

## Coordination télémétrique TLS (SF-5b)- Mesure de la valeur :
  - L'automatisation TLS publie `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` et `sorafs_gateway_tls_ech_enabled`.
  - Consultez ces jauges dans le tableau de bord Présentation de la passerelle dans les panneaux TLS/Certificats.
- Alertes suivantes :
  - Lorsque vous créez une alerte d'installation TLS (≤ 14 jours), corrélez le SLO de disponibilité sans confiance.
  - L'activation de l'alerte automatique de l'émetteur ECH, associée à TLS et aux panneaux de disponibilité.
- Pipeline : exportation de tâches d'automatisation TLS vers la pile Prometheus, qui est une passerelle de mesures ; La coordination avec le SF-5b garantit la dédoublement des instruments.

## Conventions d'aménagement et de restauration- La mesure correspond au préfixe `torii_sorafs_*` ou `sorafs_*`, en utilisant Torii et la passerelle.
- Наборы меток стандартизированы:
  - `result` → résultat HTTP (`success`, `refused`, `failed`).
  - `reason` → code d'entrée/sortie (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → fournisseur d'identifiant codé en hexadécimal.
  - `manifest` → manifeste de digestion canonique (обрезается при высокой кардинальности).
  - `tier` → niveau de déclaration des métaux (`hot`, `warm`, `archive`).
- Точки эмиссии телеметрии:
  - Les mesures de la passerelle proviennent du module `torii_sorafs_*` et des conventions `crates/iroha_core/src/telemetry.rs`.
  - L'opérateur émet des mesures `sorafs_orchestrator_*` et utilise `telemetry::sorafs.fetch.*` (cycle de vie, nouvelle tentative, échec du fournisseur, erreur, blocage), manifeste de résumé final, ID de travail, région et identification du fournisseur.
  - Vous pouvez publier `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` et `torii_sorafs_por_*`.
- Coordonner l'observabilité pour enregistrer le catalogue de mesures dans le document relatif à la dénomination Prometheus, ainsi que l'organisation des cartes cardinales. меток (верхние границы fournisseur/manifestes).

## Pipeline de données- Les collectionneurs s'approvisionnent en composants de base, exportant OTLP vers Prometheus (métriques) et Loki/Tempo (logiques/traces).
- L'eBPF (Tetragon) spécial prend en charge le trafic des passerelles/uslots.
- Utilisez `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` pour Torii et les utilisateurs actuels ; L'opérateur propose un `install_sorafs_fetch_otlp_exporter`.

## Crochets de validation

- Appuyez sur `scripts/telemetry/test_sorafs_fetch_alerts.sh` dans CI pour afficher les alertes Prometheus afin d'établir les synchronisations avec les mesures de décrochage et les suppressions privées.
- Ouvrez le tableau de bord Grafana selon la version du contrôleur (`dashboards/grafana/`) et ouvrez les écrans/accessoires pour le panneau de commande.
- Les exercices du chaos enregistrent les résultats selon `scripts/telemetry/log_sorafs_drill.sh` ; La validation utilise `scripts/telemetry/validate_drill_log.sh` (avec [Playbook операций](operations-playbook.md)).