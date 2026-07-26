---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
titre : Plan d'observabilité et SLO de SoraFS
sidebar_label : Observabilité et SLO
description : Esquema de télémétrie, tableaux de bord et politique de présupposé d'erreur pour les passerelles SoraFS, nœuds et explorateur multifonction.
---

:::note Source canonique
Cette page reflète le plan de gestion en `docs/source/sorafs_observability_plan.md`. Assurez-vous que les copies sont synchronisées jusqu'à ce que le ensemble de documents Sphinx soit migré complètement.
:::

## Objets
- Définir des paramètres et des événements structurés pour les passerelles, les nœuds et l'explorateur multifonction.
- Tableaux de bord Proveer de Grafana, parapluies d'alerte et crochets de validation.
- Établir les objectifs du SLO conjointement avec la politique de présupposé d'erreur et les exercices de chaos.

## Catalogue de mesures

### Superficies de la passerelle

| Métrique | Type | Étiquettes | Notes |
|--------|------|-----------|-------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis via `SorafsGatewayOtel` ; Rastrea les opérations HTTP en vuelo par combinaison de point de terminaison/méthode. |
| `sorafs_gateway_responses_total` | Compteur | | Chaque demande complétée par la passerelle incrémente una vez ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | Latence du délai jusqu'au premier octet pour les réponses de la passerelle ; exporté comme Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | Résultats de vérification des essais capturés au moment de la sollicitude (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Distribution de latence de vérification pour les recettes PoR. |
| `telemetry::sorafs.gateway.request` | Événement structuré | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Journal structuré émis pour compléter chaque demande de correspondance avec Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Compteur | Ensembles d'étiquettes héritées | Métricas Prometheus retenues pour tableaux de bord historiques ; émis conjointement avec la nouvelle série OTLP. |

Les événements `telemetry::sorafs.gateway.request` reflètent les contadores OTEL avec des charges utiles structurées, exposant `endpoint`, `method`, `variant`, `status`, `error_code` et `duration_ms` pour la corrélation entre Loki/Tempo, pendant que les tableaux de bord consomment la série OTLP pour la suite de SLO.

### Télémétrie de santé des essais| Métrique | Type | Étiquettes | Notes |
|--------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | Se incrémenta chaque fois que `RecordCapacityTelemetry` émet un `SorafsProofHealthAlert`. `trigger` distingue les erreurs PDP/PoTR/Ambos, tandis que `penalty` capture si la garantie est réellement enregistrée ou supprimée pendant le temps de recharge. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | Les rapports les plus récents du PDP/PoTR ont été rapportés dans la fenêtre de télémétrie frauduleuse pour que les équipes quantitatives soient dépassées par les fournisseurs de la politique. |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | Montez Nano-XOR enregistré dans la dernière alerte (sans que le temps de recharge soit supprimé lors de l'application). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | Jauge booléenne (`1` = alerte supprimée pendant le temps de recharge) pour afficher lorsque les alertes de suivi sont temporairement silencieuses. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | Époque enregistrée pour la fenêtre de télémétrie vinculée à l'alerte pour les opérateurs corrélés aux artefacts Norito. |

Estos nourrit maintenant le fil de santé des essais du tableau de bord Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), donc pour les opérateurs CDN visibilité en temps réel
des volumes d'alertes, un mélange de disparadores PDP/PoTR, des pénalités et un état de refroidissement pour
fournisseur.

Les mêmes mesures sont désormais prises en compte dans les règles d'alerte du téléspectateur Taikai :
`SorafsProofHealthPenalty` disparaît lorsque
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` augmente fr
les 15 dernières minutes, pendant que `SorafsProofHealthCooldown` lance une publicité si un
le fournisseur est permanent pendant le temps de recharge pendant 5 minutes. Ambas alertas viven fr
`dashboards/alerts/taikai_viewer_rules.yml` pour que les SRE reçoivent le contexte immédiatement
lorsque l’application PoR/PoTR s’intensifie.

### Superficies de l'orchestre| Métrique / Événement | Type | Étiquettes | Producteur | Notes |
|-----------------|------|-----------|---------------|-------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | Sessions actuellement en vuelo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | Histogramme de durée en millisecondes ; seaux de 1 ms à 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razones : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguer les causes de réintégration (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Capturez la déshabilitation et les conteos de fallos au niveau de la session. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Répartition de la latence de récupération par bloc (ms) pour l'analyse du débit/SLO. |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Octets entreposés par le manifeste/fournisseur ; dériver le débit via `rate()` et PromQL. |
| `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Cuenta chunks qui dépassent `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Événement structuré | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Réfléchissez au cycle de vie du travail (initial/terminé) avec la charge utile JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis par Racha de reintentos par le fournisseur ; `attempts` compte réintenté incrémental (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Il est public lorsqu'un fournisseur croise l'ombre des chutes. |
| `telemetry::sorafs.fetch.error` | Événement structuré | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registre du terminal, compatible avec l'ingestion de Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Il émet lorsque la latence du morceau dépasse la limite configurée (refléte les contadores de décrochage). |

### Superficies de nœud / réplication| Métrique | Type | Étiquettes | Notes |
|--------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | Histogramme OTEL du pourcentage d'utilisation de l'exploitation (exporté comme `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | Contador monotonico de muestras PoR exitosas, dérivé des instantanés du planificateur. |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | Contador monotonico de muestras PoR fallidas. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | Jauges Prometheus existent pour les octets utilisés, profondeur de cola, conteos PoR en vuelo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | Données de capacité/disponibilité fournies par le fournisseur dans le tableau de bord de capacité. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | L'arriéré est plus important que les contadores accumulés de chutes exportées chaque fois que vous consultez `/v1/sorafs/por/ingestion/{manifest}`, en alimentant le panneau/alerte "PoR Stalls". |

### Test de récupération d'opportunité (PoTR) et SLA de morceaux

| Métrique | Type | Étiquettes | Producteur | Notes |
|--------|------|-----------|-----------|-------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Coordinateur PoTR | Holgura del date limite en milisegundos (positivo = cumplido). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Coordinateur PoTR | Razones : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | Moniteur SLA | Il disparaît lorsque l'entrega de chunks contient le SLO (latence, tasa de exito). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | Moniteur SLA | Jauge booléenne (0/1) alternée pendant la fenêtre d'allumage activée. |

## Objetivos SLO

- Disponibilité de la passerelle trustless : **99,9 %** (réponse HTTP 2xx/304).
- TTFB P95 sans confiance : niveau chaud ≤ 120 ms, niveau chaud ≤ 300 ms.
- Taux de réussite des essais : ≥ 99,5 % par jour.
- Éxito del orquestador (finalisation des morceaux) : ≥ 99 %.

## Tableaux de bord et alertes

1. **Observabilité de la passerelle** (`dashboards/grafana/sorafs_gateway_observability.json`) — disponibilité sans confiance, TTFB P95, desglose de rechazos and fallos PoR/PoTR via les métriques OTEL.
2. **Salud del orquestador** (`dashboards/grafana/sorafs_fetch_observability.json`) — cubre carga multifuente, reintentos, fallos de provenedores et ráfagas de stalls.
3. **Métriques de confidentialité de SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — seaux graphiques de relais anonymisés, fenêtres de suppression et sécurité des collecteurs via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` et `soranet_privacy_poll_errors_total{provider}`.

Paquets d'alertes :- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilité de la passerelle, TTFB, picos de fallos de pruebas.
- `dashboards/alerts/sorafs_fetch_rules.yml` — fallos/reintentos/stalls del orquestador ; validé via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` et `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — pics de dégradation de la vie privée, alarmes de suppression, détection de collecteur inactif et alertes de collecteur déshabilité (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de baisse de tension anonymisées connectées à `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes de dérivée/ingest/CEK lag del Taikai viewer plus les nouvelles alertes de pénalité/refroidissement de la santé des essais SoraFS déclenchées par `torii_sorafs_proof_health_*`.

## Stratégie de travail

- Adopter OpenTelemetry de l'extrême à l'extrême :
  - Les passerelles émettent des annotations OTLP (HTTP) avec les identifiants de sollicitation, les résumés du manifeste et les hachages du jeton.
  - L'orquestador utilise `tracing` + `opentelemetry` pour exporter les étendues d'intentions de récupération.
  - Les nœuds SoraFS intègrent des travées exportées pour desafíos Por et operaciones de almacenamiento. Tous les composants partagent un ID de trace communément propagé via `x-sorafs-trace`.
- `SorafsFetchOtel` connecte les paramètres de l'explorateur aux histogrammes OTLP pendant que les événements `telemetry::sorafs.fetch.*` fournissent des charges utiles JSON légères pour les backends centraux et les journaux.
- Collecteurs : ejecuta collectors OTEL junto con Prometheus/Loki/Tempo (Tempo preferido). Les exportateurs API Jaeger sont toujours disponibles en option.
- Les opérations de haute cardinalité doivent être effectuées (10 % pour les routes de sortie, 100 % pour les chutes).

## Coordination de la télémétrie TLS (SF-5b)

- Alineación de métriques :
  - L'automatisation TLS envoie `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` et `sorafs_gateway_tls_ech_enabled`.
  - Inclut ces jauges dans le tableau de bord Gateway Overview sous le panneau TLS/Certificates.
- Vinculación de alertas:
  - Lorsqu'il y a des alertes d'expiration TLS (≤ 14 jours restants) en corrélation avec le SLO de disponibilité trustless.
  - La déshabilitation de ECH émet une alerte secondaire qui se réfère aux panneaux TLS comme à la disponibilité.
- Pipeline : le travail d'automatisation d'exportation TLS vers la même pile Prometheus que les paramètres de la passerelle ; la coordination avec le SF-5b assure la sécurité des instruments dédupliqués.

## Conventions de nombres et étiquettes de métriques- Les nombres de paramètres suivent les préfixes existants `torii_sorafs_*` ou `sorafs_*` utilisés par Torii et la passerelle.
- Les ensembles d'étiquettes sont standardisés :
  - `result` → résultat HTTP (`success`, `refused`, `failed`).
  - `reason` → code de erreur/erreur (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identifiant du fournisseur codifié en hexadécimal.
  - `manifest` → digest canónico de manifest (recortado cuando hay alta cardinalidad).
  - `tier` → étiquettes déclaratives de niveau (`hot`, `warm`, `archive`).
- Points d'émission de télémétrie :
  - Les paramètres de la passerelle vivent sous `torii_sorafs_*` et réutilisent les conventions de `crates/iroha_core/src/telemetry.rs`.
  - L'explorateur émet des données `sorafs_orchestrator_*` et des événements `telemetry::sorafs.fetch.*` (cycle de vie, nouvelle tentative, échec du fournisseur, erreur, blocage) avec des étiquettes contenant le résumé du manifeste, l'ID de travail, la région et les identifiants du fournisseur.
  - Les nœuds exposent `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` et `torii_sorafs_por_*`.
- Coordonner l'observabilité pour enregistrer le catalogue de données dans le document composé de nombres Prometheus, y compris les attentes de cardinalité des étiquettes (limites supérieures du fournisseur/manifestes).

## Pipeline de données

- Les collectionneurs sont livrés avec chaque composant, exportant OTLP vers Prometheus (métricas) et Loki/Tempo (logs/trazas).
- eBPF optionnel (Tetragon) active le travail de bas niveau pour les passerelles/nœuds.
- Usa `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` pour Torii et nœuds intégrés ; l’orquestador continue d’appeler le `install_sorafs_fetch_otlp_exporter`.

## Hooks de validation

- Exécution `scripts/telemetry/test_sorafs_fetch_alerts.sh` pendant la CI pour garantir que les règles d'alerte de Prometheus peuvent être effectuées en continu avec des mesures de décrochage et des contrôles de suppression de confidentialité.
- Gardez les tableaux de bord de Grafana sous le contrôle des versions (`dashboards/grafana/`) et actualisez les captures/liens lorsque vous modifiez les panneaux.
- Les exercices de caos enregistrés résultats via `scripts/telemetry/log_sorafs_drill.sh` ; la validation utilise `scripts/telemetry/validate_drill_log.sh` (consultez le [Playbook de operaciones](operations-playbook.md)).