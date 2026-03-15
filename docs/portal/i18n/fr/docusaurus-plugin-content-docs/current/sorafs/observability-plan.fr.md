---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
titre : Plan d'observabilité et de SLO de SoraFS
sidebar_label : Observabilité et SLO
description : Schéma de télémétrie, tableaux de bord et politique de budget d'erreur pour les passerelles SoraFS, les nœuds et l'orchestrateur multi-source.
---

:::note Source canonique
Cette page reflète le plan maintenu dans `docs/source/sorafs_observability_plan.md`. Gardez les deux copies synchronisées jusqu'à la migration complète de l'ancien ensemble Sphinx.
:::

## Objectifs
- définir des métriques et des événements structurés pour les passerelles, les nœuds et l'orchestrateur multi-source.
- Fournir des tableaux de bord Grafana, des seuils d'alerte et des crochets de validation.
- Établir des objectifs SLO avec des politiques de budget d'erreur et de exercices de chaos.

## Catalogue des métriques

### Surfaces du portail| Métrique | Tapez | Étiquettes | Remarques |
|---------|------|------------|-------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis via `SorafsGatewayOtel` ; suit les opérations HTTP en vol par combinaison endpoint/méthode. |
| `sorafs_gateway_responses_total` | Compteur | | Chaque requête gateway terminée incrémentiellement une fois ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | Temps de latence au premier octet pour les réponses gateway ; exportée en Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | Résultats de vérification des preuves capturées au moment de la requête (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Distribution de latence de vérification pour les reçus PoR. || `telemetry::sorafs.gateway.request` | Événement structuré | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log structuré émis à chaque fin de requête pour la corrélation Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Compteur | Jeux d'étiquettes héritées | Métriques Prometheus conservées pour les tableaux de bord historiques ; émises en parallèle de la nouvelle série OTLP. |

Les événements `telemetry::sorafs.gateway.request` renvoient les compteurs OTEL avec des charges utiles structurées, exposant `endpoint`, `method`, `variant`, `status`, `error_code` et `duration_ms` pour la corrélation Loki/Tempo, tandis que les tableaux de bord consomment la série OTLP pour le suivi des SLO.

### Télémétrie de santé des preuves| Métrique | Tapez | Étiquettes | Remarques |
|---------|------|------------|-------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | Incrémente à chaque fois que `RecordCapacityTelemetry` émet un `SorafsProofHealthAlert`. `trigger` distingue les échecs PDP/PoTR/Both, tandis que `penalty` capture si le collatéral a réellement été amputé ou supprimé par cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | Derniers comptes PDP/PoTR reportés dans la fenêtre de télémétrie fautive pour que les équipes quantifient le dépassement de politique par les fournisseurs. |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | Montant Nano-XOR amputé sur la dernière alerte (zéro lorsque le cooldown a supprimé l'application). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | Jauge booléen (`1` = alerte supprimée par cooldown) pour signaler lorsque les alertes suivantes sont temporairement muettes. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | Époque enregistrée pour la fenêtre de télémétrie liée à l'alerte afin que les opérateurs puissent corréler avec les artefacts Norito. |

Ces flux alimentent désormais la ligne proof-health du tableau de bord Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), offrant aux opérateurs CDN une visibilité en direct
sur les volumes d'alertes, le mix de triggers PDP/PoTR, les pénalités et l'état de cooldown par
fournisseur.Les mêmes métriques soutiennent maintenant deux règles d'alerte Taikai viewer :
`SorafsProofHealthPenalty` se déclenche lorsque
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` augmenté
dans les 15 dernières minutes, tandis que `SorafsProofHealthCooldown` émet un avertissement si un
fournisseur reste en cooldown pendant cinq minutes. Les deux alertes vivent dans
`dashboards/alerts/taikai_viewer_rules.yml` afin que les SRE disposent d'un contexte immédiat
lorsque l'application PoR/PoTR s'intensifie.

### Surfaces de l'orchestrateur| Métrique / Événement | Tapez | Étiquettes | Producteur | Remarques |
|----------------------|------|------------|------------|-------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | Séances actuellement en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | Histogramme de durée en millisecondes ; seaux 1 ms à 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Motifs : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguez les causes de nouvelle tentative (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Capturez les désactivations et comptes d'échecs au niveau session. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribution de latence fetch par chunk (ms) pour analyser le débit/SLO. |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Octets livrés par manifest/fournisseur ; déduisez le débit via `rate()` en PromQL. || `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Compte les chunks qui dépassent `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Événement structuré | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Reflète le cycle de vie du job (start/complete) avec payload JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis par streak de retry par fournisseur ; `attempts` compte les tentatives incrémentales (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publié lorsqu'un fournisseur franchit le seuil d'échecs. |
| `telemetry::sorafs.fetch.error` | Événement structuré | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Enregistrement de dysfonctionnement du terminal, adapté à l'ingestion Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Événement structuré | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Émis lorsque la latence chunk dépasse la limite configurée (reflète les compteurs de décrochage). |### Surfaces noeud / réplication

| Métrique | Tapez | Étiquettes | Remarques |
|---------|------|------------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | Histogramme OTEL du pourcentage d'utilisation du stockage (exporté en `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | Compteur monotone des échantillons PoR réussis, dérivé des instantanés du planificateur. |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | Compteur monotone des échantillons PoR échecs. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | Jauges Prometheus existantes pour octets utilisés, profondeur de file, comptes PoR en vol. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | Les données de capacité/uptime réussies du fournisseur sont exposées dans le tableau de bord de capacité. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | Profondeur du backlog plus compteurs cumulés d'échecs exportés à chaque interrogation de `/v1/sorafs/por/ingestion/{manifest}`, alimentant le panneau/alerte "PoR Stalls". |

### Preuve de récupération en temps utile (PoTR) et SLA des chunks| Métrique | Tapez | Étiquettes | Producteur | Remarques |
|-------------|------|------------|------------|-------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Coordinateur PoTR | Marge de délai en millisecondes (positif = respecté). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Coordinateur PoTR | Motifs : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | Moniteur SLA | Déclenché quand la livraison de chunks rate le SLO (latence, taux de succès). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | Moniteur SLA | Jauge booléen (0/1) activée pendant la fenêtre de violation active. |

## Objectifs SLO

- Disponibilité trustless du gateway : **99.9%** (réponses HTTP 2xx/304).
- TTFB P95 trustless : niveau chaud ≤ 120 ms, niveau chaud ≤ 300 ms.
- Taux de succès des preuves : ≥ 99,5% par jour.
- Succès de l'orchestrateur (finalisation des morceaux) : ≥ 99%.

## Tableaux de bord et alertes1. **Observabilité gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — suit la disponibilité trustless, TTFB P95, la répartition des refus et les échecs PoR/PoTR via les métriques OTEL.
2. **Santé de l'orchestrateur** (`dashboards/grafana/sorafs_fetch_observability.json`) — couvre la charge multi-source, les tentatives, les échecs fournisseurs et les rafales de stalls.
3. **Métriques de confidentialité SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — trace les buckets de relais anonymisés, les fenêtres de suppression et la santé des collecteurs via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` et `soranet_privacy_poll_errors_total{provider}`.

Paquets d'alertes :

- `dashboards/alerts/sorafs_gateway_rules.yml` — passerelle de disponibilité, TTFB, photos d'échecs de preuves.
- `dashboards/alerts/sorafs_fetch_rules.yml` — échecs/retries/décrochages de l'orchestrateur ; validé via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` et `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — photos de dégradation de confidentialité, alarmes de suppression, détection de collecteur inactif et alertes de collecteur désactivées (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de baisse de tension d'anonymat câblées sur `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes de drift/ingest/CEK lag Taikai viewer plus les nouvelles alertes de pénalité/cooldown de santé des preuves SoraFS alimentées par `torii_sorafs_proof_health_*`.

## Stratégie de traçage- Adopter OpenTelemetry de bout en bout :
  - Les passerelles émettent des spans OTLP (HTTP) annotés avec des ID de requête, des digests de manifest et des hashes de token.
  - L'orchestrateur utilise `tracing` + `opentelemetry` pour exporter des spans de tentatives de fetch.
  - Les nœuds SoraFS embarqués exportent des travées pour les défis PoR et les opérations de stockage. Tous les composants partagent un ID de trace commun propagé via `x-sorafs-trace`.
- `SorafsFetchOtel` repose sur les métriques orchestrateur à des histogrammes OTLP tandis que les événements `telemetry::sorafs.fetch.*` fournissent des payloads JSON légers pour des backends centres logs.
- Collectors : exécutez des collectors OTEL à côté de Prometheus/Loki/Tempo (Tempo préféré). Les exportateurs API Jaeger restent optionnels.
- Les opérations à haute cardinalité doivent être échantillonnées (10% pour les chemins de succès, 100% pour les échecs).

## Coordination de la télémétrie TLS (SF-5b)- Alignement des métriques :
  - L'automatisation TLS publie `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` et `sorafs_gateway_tls_ech_enabled`.
  - Incluez ces jauges dans le tableau de bord Gateway Overview sous le panneau TLS/Certificates.
- Liaison des alertes :
  - Lorsque les alertes d'expiration TLS se déclenchent (≤ 14 jours restants), corrélez avec le SLO de disponibilité trustless.
  - La désactivation ECH émet une alerte secondaire référant à la fois aux panneaux TLS et disponibilité.
- Pipeline : le job d'automatisation TLS exporte vers la même stack Prometheus que les métriques gateway ; la coordination avec SF-5b assure une instrumentation dédupliquée.

## Conventions de nommage et d'étiquetage des métriques- Les noms de métriques suivent les préfixes existants `torii_sorafs_*` ou `sorafs_*` utilisés par Torii et la passerelle.
- Les ensembles d'étiquettes sont standardisés :
  - `result` → résultat HTTP (`success`, `refused`, `failed`).
  - `reason` → code de refus/erreur (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identifiant fournisseur encodé en hexadécimal.
  - `manifest` → digest canonique de manifest (tronqué quand la cardinalité est élevée).
  - `tier` → étiquettes de niveau déclaratif (`hot`, `warm`, `archive`).
- Points d'émission de télémétrie :
  - Les métriques gateway vivent sous `torii_sorafs_*` et réutilisent les conventions de `crates/iroha_core/src/telemetry.rs`.
  - L'orchestrateur émet des métriques `sorafs_orchestrator_*` et des événements `telemetry::sorafs.fetch.*` (lifecycle, retry, supplier failed, error, stall) étiquetés avec digest de manifest, job ID, région et identifiants fournisseur.
  - Les nœuds exposant `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` et `torii_sorafs_por_*`.
- Coordonnez avec Observability pour enregistrer le catalogue des métriques dans le document de nommage Prometheus partagé, y compris les attentes de cardinalité des labels (bornes supérieures fournisseur/manifestes).

## Pipeline de données- Les collecteurs se déploient à côté de chaque composant, exportant OTLP vers Prometheus (métriques) et Loki/Tempo (logs/traces).
- eBPF optionnel (Tetragon) enrichit le traçage bas niveau pour passerelles/nœuds.
- Utilisez `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` pour Torii et les nœuds embarqués ; l'orchestrateur continue d'appeler `install_sorafs_fetch_otlp_exporter`.

## Hooks de validation

- Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh` en CI pour garantir que les règles d'alerte Prometheus restent alignées sur les métriques de décrochage et les vérifications de suppression de confidentialité.
- Gardez les tableaux de bord Grafana sous contrôle de version (`dashboards/grafana/`) et mettez à jour les captures/liens lorsque les panneaux changent.
- Les exercices de chaos journalisent les résultats via `scripts/telemetry/log_sorafs_drill.sh` ; la validation s'appuie sur `scripts/telemetry/validate_drill_log.sh` (voir le [Playbook d'exploitation](operations-playbook.md)).