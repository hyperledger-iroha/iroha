---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
titre : SoraFS Observabilité et SLO پلان
sidebar_label : Observabilité et SLO
description : Passerelles SoraFS, nœuds et orchestrateur multi-sources, schéma de télémétrie, tableaux de bord et politique de budget d'erreur.
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ Il s'agit d'un Sphinx qui s'est rendu compte qu'il s'agissait d'une personne âgée.
:::

## Objectifs
- passerelles, nœuds, orchestrateur multi-source, métriques, événements structurés, applications
- Tableaux de bord Grafana, seuils d'alerte et crochets de validation
- erreurs de budget et politiques de chaos et d'exercices et cibles SLO pour tous

## Catalogue de métriques

### Surfaces de passerelle

| Métrique | Tapez | Étiquettes | Remarques |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` émet un message d'erreur Un point de terminaison/une méthode pour les opérations HTTP en cours de vol |
| `sorafs_gateway_responses_total` | Compteur | | ہر مکمل gateway request ایک بار incrément ہوتی ہے؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | réponses de la passerelle : temps de latence jusqu'au premier octet Prometheus `_bucket/_sum/_count` pour l'exportation |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | l'heure de la demande et les résultats de la vérification de la preuve capturent کیے جاتے ہیں (`result` ∈ {`success`, `failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Reçus PoR et distribution de latence de vérification |
| `telemetry::sorafs.gateway.request` | Événement structuré | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | ہر achèvement de la demande پر émission de journal structuré ہوتا ہے تاکہ Corrélation Loki/Tempo ہو سکے۔ |

`telemetry::sorafs.gateway.request` compteurs OTEL d'événements et charges utiles structurées et miroir et corrélation Loki/Tempo pour `endpoint`, `method`, `variant`, `status`, `error_code` et `duration_ms` Tableau de bord de tableau de bord Suivi SLO pour la série OTLP

### Télémétrie de preuve de santé| Métrique | Tapez | Étiquettes | Remarques |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | جب بھی `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` émettent کرے تو incrément ہوتا ہے۔ `trigger` PDP/PoTR/Both Failures فرق کرتا ہے، جبکہ `penalty` دکھاتا ہے کہ collatéral واقعی slash ہوا یا cooldown نے supprimer کیا۔ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | fenêtre de télémétrie incriminée et les comptes PDP/PoTR pour les fournisseurs de services et la politique des fournisseurs |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | L'alerte est activée par slash et par Nano-XOR (temps de recharge et suppression de l'application de la loi). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | Jauge booléenne (`1` = temps de recharge des alertes et suppression des alertes) Permet de suivre les alertes et de les mettre en sourdine et de les désactiver. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | alerte fenêtre de télémétrie époque époque opérateurs Norito artefacts corrélation corrélation |

یہ flux sur le tableau de bord de la visionneuse Taikai کی ligne de preuve de santé کو چلاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) pour les opérateurs CDN et les volumes d'alertes, le mélange de déclencheurs PDP/PoTR, les pénalités et l'état de recharge du fournisseur et la visibilité en direct

Les métriques du visualiseur Taikai et les règles d'alerte ainsi que les fonctionnalités suivantes :
`SorafsProofHealthPenalty` pour le feu et le feu
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`
Il y a 15 jours que le message d'avertissement `SorafsProofHealthCooldown` est disponible.
fournisseur پانچ منٹ تک cooldown میں رہے۔ Alertes دونوں
`dashboards/alerts/taikai_viewer_rules.yml` SRE et PoR/PoTR
mise en application بڑھنے پر فوری contexte ملے۔

### Surfaces de l'orchestrateur| Métrique/Événement | Tapez | Étiquettes | Producteur | Remarques |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | موجودہ sessions en vol۔ |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | histogramme de durée (millisecondes)؛ 1 ms × 30 s buckets۔ |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Motifs : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | la nouvelle tentative provoque une erreur (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | capture des décomptes de désactivation/d'échec au niveau de la session |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | distribution de latence de récupération par fragment (ms) débit/analyse SLO |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | manifeste/fournisseur کے حساب سے octets livrés؛ PromQL utilise `rate()` pour un débit élevé |
| `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` est un ensemble de morceaux et de morceaux |
| `telemetry::sorafs.fetch.lifecycle` | Événement structuré | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | cycle de vie du travail (démarrage/terminé) avec charge utile JSON Norito et miroir miroir |
| `telemetry::sorafs.fetch.retry` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | la séquence de nouvelles tentatives du fournisseur émet ہوتا ہے؛ `attempts` tentatives incrémentielles ont lieu ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Le seuil de défaillance du fournisseur franchit le seuil de défaillance du fournisseur. |
| `telemetry::sorafs.fetch.error` | Événement structuré | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | enregistrement de défaillance du terminal, ingestion de Loki/Splunk |
| `telemetry::sorafs.fetch.stall` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | le capuchon configuré pour la latence des morceaux est émis pour émettre 4 (compteurs de décrochage et miroir pour 4). |

### Surfaces de nœuds/réplications| Métrique | Tapez | Étiquettes | Remarques |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | pourcentage d'utilisation du stockage dans l'histogramme OTEL ( `_bucket/_sum/_count` dans l'exportation). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | instantanés du planificateur et échantillons PoR réussis dérivés et compteur monotone |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | Échantillons PoR ayant échoué et compteur monotone |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | octets utilisés, profondeur de file d'attente et nombre de vols PoR et jauges Prometheus |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | données de réussite sur la capacité/la disponibilité du fournisseur et tableau de bord de capacité |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | profondeur du backlog et compteurs de défaillances cumulées ainsi que `/v1/sorafs/por/ingestion/{manifest}` sondage et exportation et panneau/alerte "PoR Stalls" et flux d'informations |

### Preuve de récupération en temps opportun (PoTR) et chunk SLA

| Métrique | Tapez | Étiquettes | Producteur | Remarques |
|--------|------|--------|--------------|-------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Coordinateur PoTR | délai de relâche en millisecondes میں (positif = respecté). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Coordinateur PoTR | Motifs : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | Moniteur SLA | Le SLO de livraison de morceaux manque ou incendie (latence, taux de réussite). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | Moniteur SLA | Jauge booléenne (0/1) et fenêtre de brèche active et bascule |

## Objectifs SLO

- Disponibilité sans confiance de la passerelle : **99,9 %** (réponses HTTP 2xx/304).
- TTFB P95 sans confiance : niveau chaud ≤ 120 ms, niveau chaud ≤ 300 ms.
- Taux de réussite des épreuves : ≥ 99,5% par jour.
- Succès de l'orchestrateur (achèvement des fragments) : ≥ 99 %.

## Tableaux de bord et alertes

1. **Observabilité de la passerelle** (`dashboards/grafana/sorafs_gateway_observability.json`) — disponibilité sans confiance, panne de refus TTFB P95, échecs PoR/PoTR et métriques OTEL pour les problèmes de sécurité.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — chargement multi-source, tentatives, échecs du fournisseur et rafales de décrochage et de décrochage.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — compartiments de relais anonymisés, fenêtres de suppression et santé des collecteurs, `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` et `soranet_privacy_poll_errors_total{provider}` et `soranet_privacy_poll_errors_total{provider}`. کرتا ہے۔

Groupes d'alertes :- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilité de la passerelle, pics de défaillance à l'épreuve du TTFB,
- `dashboards/alerts/sorafs_fetch_rules.yml` — échecs/nouvelles tentatives/blocages de l'orchestrateur؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, et `dashboards/alerts/tests/soranet_policy_rules.test.yml` pour valider
- `dashboards/alerts/soranet_privacy_rules.yml` — pics de dégradation de la confidentialité, alarmes de suppression, détection d'inactivité du collecteur et alertes de collecteur désactivé (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de baisse de tension d'anonymat et `sorafs_orchestrator_brownouts_total` en version filaire
- `dashboards/alerts/taikai_viewer_rules.yml` — Alarmes de dérive/absorption/décalage CEK du visualiseur Taikai et alertes de pénalité/temps de recharge SoraFS et `torii_sorafs_proof_health_*` alimentées par SoraFS.

## Stratégie de traçage

- OpenTelemetry et solution de bout en bout :
  - Les passerelles OTLP spans (HTTP) émettent des identifiants de requête, des résumés de manifeste et des hachages de jetons.
  - Orchestrator `tracing` + `opentelemetry` pour les tentatives de récupération et l'exportation de spans.
  - Les nœuds SoraFS intégrés défient les opérations de stockage et les opérations de stockage et s'étendent sur l'exportation. Les composants `x-sorafs-trace` se propagent et partagent des identifiants de trace communs.
- Métriques de l'orchestrateur `SorafsFetchOtel` et histogrammes OTLP et pont et charges utiles JSON légères et backends centrés sur le journal des événements `telemetry::sorafs.fetch.*` et charges utiles JSON légères.
- Collecteurs : collecteurs OTEL کو Prometheus/Loki/Tempo کے ساتھ چلائیں (Tempo préféré). Jaeger API exportateurs اختیاری رہتے ہیں۔
- Opérations à haute cardinalité comme échantillon (chemins de réussite entre 10 % et échecs entre 100 %).

## Coordination de télémétrie TLS (SF-5b)

- Alignement métrique :
  - Automatisation TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, et `sorafs_gateway_tls_ech_enabled` en français
  - Les jauges, le tableau de bord de présentation de la passerelle et le panneau TLS/Certificats.
- Lien d'alerte :
  - Les alertes d'expiration TLS se déclenchent (≤ 14 jours restants) et la disponibilité sans confiance SLO est en corrélation avec les autres.
  - Désactivation ECH et émission d'alerte secondaire et TLS et disponibilité des panneaux et références.
- Pipeline : tâche d'automatisation TLS pour la pile Prometheus pour l'exportation et les métriques de passerelle SF-5b pour la coordination des instruments dédupliqués et des instruments de coordination

## Conventions de dénomination et d'étiquetage des métriques- Noms de métriques, préfixes `torii_sorafs_*` et `sorafs_*` et suivi des préfixes et Torii de la passerelle et de la passerelle.
- Jeux d'étiquettes standardisés :
  - `result` → résultat HTTP (`success`, `refused`, `failed`).
  - `reason` → code refus/erreur (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identifiant du fournisseur codé en hexadécimal۔
  - `manifest` → résumé canonique du manifeste (haute cardinalité میں trim کیا جاتا ہے). 
  - `tier` → étiquettes de niveau déclaratives (`hot`, `warm`, `archive`).
- Points d'émission de télémétrie :
  - Métriques de passerelle `torii_sorafs_*` pour la réutilisation des conventions et `crates/iroha_core/src/telemetry.rs` pour la réutilisation des conventions
  - Les métriques de l'orchestrateur `sorafs_orchestrator_*` et les événements `telemetry::sorafs.fetch.*` (cycle de vie, nouvelle tentative, échec du fournisseur, erreur, blocage) émettent un résumé du manifeste, une ID de travail, une région et des balises d'identification du fournisseur.
  - Nœuds `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, et `torii_sorafs_por_*` دکھاتے ہیں۔
- Observabilité des coordonnées du catalogue métrique partagé Prometheus document de dénomination du registre des attentes de cardinalité de l'étiquette (fournisseur/manifeste limites supérieures) ہوں۔

## Pipeline de données

- Les collecteurs et les composants doivent déployer et déployer OTLP pour Prometheus (métriques) et Loki/Tempo (journaux/traces) et exporter les composants.
- Passerelles/nœuds eBPF (Tetragon) en option pour le traçage de bas niveau et l'enrichissement des données
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` et Torii pour les nœuds intégrés et les nœuds intégrés Orchestrator `install_sorafs_fetch_otlp_exporter` est un instrument de travail en ligne

## Crochets de validation

- CI pour `scripts/telemetry/test_sorafs_fetch_alerts.sh` et Prometheus les règles d'alerte bloquent les métriques et les contrôles de suppression de confidentialité et les règles de verrouillage
- Tableaux de bord Grafana et contrôle de version (`dashboards/grafana/`) avec les panneaux et les captures d'écran/liens ci-dessous.
- Exercices de chaos avec `scripts/telemetry/log_sorafs_drill.sh` et journal de bord validation `scripts/telemetry/validate_drill_log.sh` استعمال کرتی ہے (دیکھیے [Operations Playbook](operations-playbook.md)).