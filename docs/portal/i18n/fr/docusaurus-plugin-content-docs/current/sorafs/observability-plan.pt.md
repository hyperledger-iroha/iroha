---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
titre : Plan d'observabilité et SLO da SoraFS
sidebar_label : Observabilité et SLO
description : Schéma de télémétrie, tableaux de bord et politique de budget d'erreur pour les passerelles SoraFS, nous et l'explorateur multi-source.
---

:::note Fonte canonica
Cette page s'étend sur le plan suivant dans `docs/source/sorafs_observability_plan.md`. Mantenha ambas comme copies synchronisées.
:::

## Objets
- Définir des mesures et des événements structurés pour les passerelles, ainsi que pour les explorateurs multi-sources.
- Fornecer tableaux de bord Grafana, limiares d'alerte et crochets de validation.
- Établir les objectifs du SLO en même temps que la politique budgétaire des erreurs et des exercices de chaos.

## Catalogue de métriques

### Les superficielles font passerelle| Métrique | Type | Étiquettes | Notes |
|---------|------|--------|-------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis via `SorafsGatewayOtel` ; Rastreia fonctionne HTTP en voo par combinaison de point de terminaison/méthode. |
| `sorafs_gateway_responses_total` | Compteur | | Chaque demande est complétée par la passerelle incrémentée à un moment donné ; `result` dans {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | Latence du délai jusqu'au premier octet pour les réponses à la passerelle ; exporté comme Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | Résultats de vérification des preuves capturées au moment de la sollicitation (`result` dans {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Distribuicao de latencia de verificacao para recibos PoR. || `telemetry::sorafs.gateway.request` | Événement structuré | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Le journal a été émis pour conclure chaque demande de correspondance avec Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Compteur | Ensembles d'étiquettes alternatives | Metricas Prometheus mantes pour tableaux de bord historiques ; émis conjointement avec une nouvelle série OTLP. |

Les événements `telemetry::sorafs.gateway.request` indiquent les contadores OTEL avec les charges utiles structurées, les extensions `endpoint`, `method`, `variant`, `status`, `error_code` et `duration_ms` pour la corrélation avec Loki/Tempo, indique les tableaux de bord utilisés dans la série OTLP pour l'accompagnement de SLO.

### Télémétrie saine des preuves| Métrique | Type | Étiquettes | Notes |
|---------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | Incrementa toda vez que `RecordCapacityTelemetry` émet un `SorafsProofHealthAlert`. `trigger` distingue les faux PDP/PoTR/Both, tandis que `penalty` capture la garantie lorsque celle-ci est réellement coupée ou supprimée pendant le temps de recharge. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | Contagènes les plus récents du PDP/PoTR liés à l'intérieur de la chaîne d'infrastructure de télémétrie pour que les équipes quantifient ou quanto les fournisseurs ultrapassaram à la politique. |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | Valor Nano-XOR est coupé dans la dernière alerte (zéro lorsque le temps de recharge est supprimé à l'application). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | Jauge booléenne (`1` = alerte supprimée pendant le temps de recharge) pour afficher lorsque les alertes d'accompagnement sont temporairement silencieuses. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | Epoca registrada para a janela de telemetria ligada ao alerta para que os operadores correlacionem com artefatos Norito. |

Esses alimente maintenant la ligne de saude des preuves du tableau de bord Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), ainsi que les opérateurs CDN visibles au rythme réel
sur les volumes d'alertes, le mélange des déclencheurs PDP/PoTR, les pénalités et l'état de refroidissement par
fournisseur.Comme mesmas metricas agora sustentam doubles regras de alerta do Taikai viewer :
`SorafsProofHealthPenalty` disparaît quand
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` augmente
nos dernières 15 minutes, pendant ce temps `SorafsProofHealthCooldown` émet un avis sur un
assurer un temps de recharge permanent pendant 5 minutes. Ambos os alertas vivem em
`dashboards/alerts/taikai_viewer_rules.yml` pour que les SRE soient reçus immédiatement dans le contexte
lorsque l'application PoR/PoTR s'intensifie.

### Superficies do orquestrador| Métrique / Événement | Type | Étiquettes | Producteur | Notes |
|------------------|------|--------|--------------|-------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | Sesssoes réellement en voo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | Histogramme de durée de vie en millisecondes ; seaux de 1 ms à 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Raisons : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguer les causes de nouvelle tentative (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Capture des capacités déstabilisantes et contagieuses au niveau de la session. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribue la latence de récupération par chunk (ms) pour analyser le débit/SLO. |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Les octets entrent dans le manifeste/fournisseur ; dériver le débit via `rate()` dans PromQL. |
| `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Conta chunks qui dépassent `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | Événement structuré | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Espelha le cycle de vie fait le travail (démarrer/terminer) avec la charge utile JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis par série de tentatives par le fournisseur ; `attempts` contient des tentatives incrémentielles (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publié quand un fournisseur cruza ou limite de faux. |
| `telemetry::sorafs.fetch.error` | Événement structuré | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registre du faux terminal, ami pour ingérer Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Émis lorsque la latence d'un morceau dépasse la limite configurée (écoutez les contadores de décrochage). |

### Superficies de no / réplicacao| Métrique | Type | Étiquettes | Notes |
|---------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | Histogramme OTEL du pourcentage d'utilisation du stockage (exporté comme `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | Contador monotono de amostras PoR bem-sucedidas, dérivé des instantanés du planificateur. |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | Contador monotono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | Jauges Prometheus existantes pour octets utilisés, profondeur de fil, contagènes PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | Les données de capacité/disponibilité sont réussies par le fournisseur ou exposées dans le tableau de bord de capacité. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | L'arriéré est profond, mais les contadores accumulés de faux exportés semper que `/v1/sorafs/por/ingestion/{manifest}` sont consultés, alimentant le tableau/alerte "PoR Stalls". |

### Preuve de récupération en temps opportun (PoTR) et SLA des morceaux| Métrique | Type | Étiquettes | Producteur | Notes |
|---------|------|--------|--------------|-------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Coordonnateur PoTR | Folga do date limite em milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Coordonnateur PoTR | Raisons : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | Moniteur SLA | Disparado quando a entrega de chunks falha no SLO (latencia, taxa de successo). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | Moniteur SLA | Jauge booléenne (0/1) alternée pendant une semaine de violation active. |

## Objetivos SLO

- Disponibilité d'une passerelle de confiance sans confiance : **99,9 %** (responsabilité HTTP 2xx/304).
- TTFB P95 sans confiance : niveau chaud = 99,5 % par dia.
- Sucesso do orquestrador (conclusion de morceaux) : >= 99%.

## Tableaux de bord et alertes1. **Observabilité de la passerelle** (`dashboards/grafana/sorafs_gateway_observability.json`) - accompagnement disponible sans confiance, TTFB P95, détails des recusas et faux PoR/PoTR via metricas OTEL.
2. **Saude do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) - chargement de câbles multi-sources, tentatives, échecs de fournisseurs et rajadas de décrochages.
3. **Metriques de confidentialité SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) - graphiques des seaux de relais anonymisés, des lignes de suppression et du collecteur via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` et `soranet_privacy_poll_errors_total{provider}`.

Paquets d'alertes :

- `dashboards/alerts/sorafs_gateway_rules.yml` - disponibilité de la passerelle, TTFB, picos de falha de provas.
- `dashboards/alerts/sorafs_fetch_rules.yml` - falhas/retries/décrochages de l'orquestrador ; validé via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` et `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - picos de dégradation de privacidade, alarmes de suppression, détection de collecteur éteint et alertes de collecteur désactivé (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - alarmes de baisse de tension anonymisées liées à `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - alarmes de dérive/absorption/décalage CEK du visualiseur Taikai mais les nouvelles alertes de pénalité/refroidissement de saude das provas SoraFS alimentées par `torii_sorafs_proof_health_*`.

## Stratégie de traçage- Adote OpenTelemetry de pont à pont :
  - Les passerelles émettent des étendues OTLP (HTTP) annotées avec les identifiants de sollicitation, les résumés du manifeste et les hachages du jeton.
  - L'explorateur utilise `tracing` + `opentelemetry` pour exporter les étendues de tentatives de récupération.
  - Nos SoraFS embutidos exportam spans para desafios PoR e operacoes de stockage. Tous les composants partagent un ID de trace propagé via `x-sorafs-trace`.
- `SorafsFetchOtel` ligne de mesures de l'explorateur d'histogrammes OTLP concernant les événements `telemetry::sorafs.fetch.*` pour les charges utiles JSON qui sont destinées aux backends centraux dans les journaux.
- Collectionneurs : exécutez les collectionneurs OTEL à la place de Prometheus/Loki/Tempo (Tempo préféré). Exportadores compatveis com Jaeger permanecem opcionais.
- Les opéras de haute cardinalité doivent être amostradas (10 % pour les chemins de réussite, 100 % pour les faux).

## Coordination de la télémétrie TLS (SF-5b)-Alinhamento de metricas:
  - Un TLS automatique via `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` et `sorafs_gateway_tls_ech_enabled`.
  - Inclut les jauges sans tableau de bord Aperçu de la passerelle sur le tableau TLS/certificats.
- Vinculo des alertes :
  - Lorsque les alertes d'expiration TLS dispararem (<= 14 jours restants), correlación avec le SLO de disponibilidade trustless.
  - Le desativação de ECH émet une alerte secondaire référencée tant que le paineis TLS quanto à la disponibilité.
- Pipeline : le travail d'exportation automatique TLS pour une pile de données Prometheus des mesures de la passerelle ; a coordenacao com SF-5b garante instrumentacao déduplicada.

## Conférences de noms et étiquettes de métriques- Les noms de mesures suivent les préfixes existants `torii_sorafs_*` ou `sorafs_*` utilisés par Torii et la passerelle.
- Conjuntos de labels sao padronizados:
  - `result` -> résultat HTTP (`success`, `refused`, `failed`).
  - `reason` -> code de recusa/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` -> identifiant du fournisseur codifié en hexadécimal.
  - `manifest` -> digest canonico do manifest (cortado quando a cardinalidade e alta).
  - `tier` -> étiquettes déclaratives de tier (`hot`, `warm`, `archive`).
- Ponts d'émission de télémétrie :
  - Les mesures de la passerelle fonctionnent sur `torii_sorafs_*` et réutilisent les conventions de `crates/iroha_core/src/telemetry.rs`.
  - L'explorateur émet des mesures `sorafs_orchestrator_*` et des événements `telemetry::sorafs.fetch.*` (cycle de vie, nouvelle tentative, échec du fournisseur, erreur, blocage) étiquettes avec le résumé du manifeste, l'ID de travail, la région et les identifiants du fournisseur.
  - Nos expositions `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` et `torii_sorafs_por_*`.
- Coordene com Observability para registrar o catalogo de metricas no documento compartimenté de noms Prometheus, y compris les attentes de cardinalité des étiquettes (limites supérieures de fournisseur/manifestes).

## Pipeline de données- Les collecteurs sont implantés avec chaque composant, exportant OTLP pour Prometheus (métriques) et Loki/Tempo (journaux/traces).
- eBPF optionnel (Tetragon) permet le traçage du bas niveau pour les passerelles/nos.
- Utilisez `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` pour Torii et nos embutidos ; o orquestrador continua chamando `install_sorafs_fetch_otlp_exporter`.

## Hooks de validation

- Exécuter `scripts/telemetry/test_sorafs_fetch_alerts.sh` pendant le CI pour garantir que le contrôle de l'alerte Prometheus est permanent en synchronisation avec les mesures de décrochage et les contrôles de suppression de confidentialité.
- Les tableaux de bord Mantenha Grafana permettent de contrôler le verso (`dashboards/grafana/`) et d'actualiser des captures d'écran/liens lorsque le pain est modifié.
- Exercices de caos registram resultados via `scripts/telemetry/log_sorafs_drill.sh` ; a validacao usa `scripts/telemetry/validate_drill_log.sh` (voir le [Playbook de operacoes](operations-playbook.md)).