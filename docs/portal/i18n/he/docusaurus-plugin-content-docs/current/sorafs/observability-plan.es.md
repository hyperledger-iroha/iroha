---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d6a52cb695b4b887a8f63dcca01ef11687a9f55e2e6fae744c96428bb0b9250
source_last_modified: "2025-11-07T10:33:21.922569+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את התוכנית המתוחזקת תחת `docs/source/sorafs_observability_plan.md`. שמרו על סנכרון שתי הגרסאות עד שמערך Sphinx הישן יושלם במלואו.
:::

## מטרות
- להגדיר מדדים ואירועים מובנים עבור gateways, נודים והאורקסטרטור מרובה מקורות.
- לספק דשבורדים של Grafana, ספי התראות ו-hooks של ולידציה.
- לקבוע יעדי SLO לצד מדיניות תקציב שגיאות ו-drills של כאוס.

## קטלוג מדדים

### משטחי gateway

| מדד | סוג | Labels | הערות |
|-----|-----|--------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | נפלט דרך `SorafsGatewayOtel`; עוקב אחרי פעולות HTTP בטיסה לפי צירוף endpoint/method. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | כל בקשת gateway שהושלמה מגדילה פעם אחת; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | לטנטיות time-to-first-byte עבור תגובות gateway; מיוצאת כ-Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | תוצאות אימות הוכחות נאספות בזמן הבקשה (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | התפלגות לטנטיות אימות עבור קבלות PoR. |
| `telemetry::sorafs.gateway.request` | אירוע מובנה | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | לוג מובנה שנפלט עם סיום כל בקשה לצורך קורלציה ב-Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Counter | סטים יורשים של Labels | מדדי Prometheus שנשמרו עבור דשבורדים היסטוריים; נפלטים לצד סדרת OTLP החדשה. |

אירועי `telemetry::sorafs.gateway.request` משקפים את מוני OTEL עם payloads מובנים, חושפים `endpoint`, `method`, `variant`, `status`, `error_code` ו-`duration_ms` לקורלציה ב-Loki/Tempo, בעוד הדשבורדים צורכים את סדרת OTLP למעקב SLO.

### טלמטריה של בריאות הוכחות

| מדד | סוג | Labels | הערות |
|-----|-----|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | גדל בכל פעם ש-`RecordCapacityTelemetry` פולט `SorafsProofHealthAlert`. `trigger` מבדיל בין כשלי PDP/PoTR/Both, בעוד `penalty` מציין אם קולטרל נחתך בפועל או דוכא ב-cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | הספירות האחרונות של PDP/PoTR בחלון הטלמטריה הפוגעני כדי שהצוותים ימדדו עד כמה ספקים חרגו מהמדיניות. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | סכום Nano-XOR שנחתך בהתראה האחרונה (אפס כאשר cooldown דיכא אכיפה). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Gauge בוליאני (`1` = התראה דוכאה ב-cooldown) כדי להראות מתי התראות המשך מושתקות זמנית. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | epoch של חלון הטלמטריה המקושר להתראה כדי לאפשר לאופרטורים לקשר מול ארטיפקטים של Norito. |

הפידים הללו מניעים כעת את שורת proof-health בדשבורד Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), ומעניקים לאופרטורים ב-CDN ראות בזמן אמת
לנפחי התראות, תמהיל טריגרים PDP/PoTR, קנסות ומצב cooldown לפי ספק.

אותם מדדים תומכים כעת בשתי כללי התראה של Taikai viewer:
`SorafsProofHealthPenalty` מופעל כאשר
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` גדל
ב-15 הדקות האחרונות, בעוד `SorafsProofHealthCooldown` מעלה אזהרה אם ספק נשאר
ב-cooldown חמש דקות. שתי ההתראות נמצאות ב-
`dashboards/alerts/taikai_viewer_rules.yml` כדי ש-SREs יקבלו הקשר מיידי כאשר
אכיפת PoR/PoTR מתעצמת.

### משטחי אורקסטרטור

| מדד / אירוע | סוג | Labels | מפיק | הערות |
|-------------|-----|--------|------|-------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | סשנים שנמצאים כעת בתהליך. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | היסטוגרמת משך במילישניות; buckets מ-1 ms עד 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | סיבות: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | מבדיל סיבות retry (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | קולט ספירות השבתה/כשל ברמת סשן. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | התפלגות לטנטיות fetch לכל chunk (ms) עבור ניתוח throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | בתים שנמסרו לכל manifest/provider; גוזרים throughput דרך `rate()` ב-PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | סופר chunks שחורגים מ-`ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | אירוע מובנה | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | משקף מחזור חיים של job (start/complete) עם payload JSON של Norito. |
| `telemetry::sorafs.fetch.retry` | אירוע מובנה | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | נפלט לכל streak של retries לספק; `attempts` סופר retries מצטברים (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | אירוע מובנה | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | מוצג כאשר ספק חוצה סף כשל. |
| `telemetry::sorafs.fetch.error` | אירוע מובנה | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | רישום כשל סופי, ידידותי לקליטה ב-Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | אירוע מובנה | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | נפלט כאשר לטנטיות chunk חוצה את הגבול שהוגדר (מראה את מוני stall). |

### משטחי נוד / רפליקציה

| מדד | סוג | Labels | הערות |
|-----|-----|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | היסטוגרמת OTEL של אחוז ניצולת אחסון (מיוצאת כ-`_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | מונה מונוטוני לדגימות PoR מוצלחות, נגזר מסנאפשוטים של ה-scheduler. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | מונה מונוטוני לדגימות PoR כושלות. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Gauges קיימים של Prometheus עבור בתים בשימוש, עומק תור וספירות PoR בתהליך. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | נתוני הצלחת capacity/uptime של ספק המוצגים בדשבורד capacity. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | עומק backlog בתוספת מוני כשל מצטברים שמיוצאים בכל polling של `/v1/sorafs/por/ingestion/{manifest}`, מזינים את פאנל/התראת "PoR Stalls". |

### תיקון ו‑SLA

| מדד | סוג | Labels | הערות |
|-----|-----|--------|-------|
| `sorafs_repair_tasks_total` | Counter | `status` | מונה OTEL למעברי משימות תיקון. |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | היסטוגרמת OTEL לזמן השהייה במחזור חיי התיקון. |
| `sorafs_repair_queue_depth` | Histogram | `provider` | היסטוגרמת OTEL של משימות בתור לכל ספק (סגנון snapshot). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | היסטוגרמת OTEL לגיל המשימה הוותיקה ביותר בתור (שניות). |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | מונה OTEL לפקיעות חכירה (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | מונה OTEL למעברי הצעות slash. |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | מונה Prometheus למעברי משימות. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | היסטוגרמת Prometheus לזמן השהייה במחזור חיי התיקון. |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | מדד Prometheus למשימות בתור לכל ספק. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | מדד Prometheus לגיל המשימה הוותיקה ביותר בתור (שניות). |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | מונה Prometheus לפקיעות חכירה. |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | מונה Prometheus למעברי הצעות slash. |

מטא־נתוני JSON של ביקורת הממשל משקפים את תוויות הטלמטריה של התיקון (`status`, `ticket_id`, `manifest`, `provider` באירועי תיקון; `outcome` בהצעות slash) כדי לאפשר קורלציה דטרמיניסטית בין מדדים לארטיפקטי ביקורת.

### Proof of Timely Retrieval (PoTR) ו-SLA של chunks

| מדד | סוג | Labels | מפיק | הערות |
|-----|-----|--------|------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR coordinator | מרווח דדליין במילישניות (חיובי = עמד). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | PoTR coordinator | סיבות: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | SLA monitor | מופעל כאשר מסירת chunks מחמיצה את ה-SLO (לטנטיות, שיעור הצלחה). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | SLA monitor | Gauge בוליאני (0/1) שמתחלף בחלון הפרה פעיל. |

## יעדי SLO

- זמינות trustless של gateway: **99.9%** (תגובות HTTP 2xx/304).
- Trustless TTFB P95: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- שיעור הצלחת הוכחות: ≥ 99.5% ליום.
- הצלחת אורקסטרטור (השלמת chunks): ≥ 99%.

## דשבורדים והתראות

1. **Observability של gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — עוקב אחרי זמינות trustless, TTFB P95, פירוק סירובים וכשלי PoR/PoTR דרך מדדי OTEL.
2. **בריאות אורקסטרטור** (`dashboards/grafana/sorafs_fetch_observability.json`) — מכסה עומס multi-source, retries, כשלי ספקים ופרצי stalls.
3. **מדדי פרטיות SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — מציג buckets של relay אנונימי, חלונות suppression ובריאות collector דרך `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` ו-`soranet_privacy_poll_errors_total{provider}`.

חבילות התראות:

- `dashboards/alerts/sorafs_gateway_rules.yml` — זמינות gateway, TTFB, קפיצות כשל הוכחות.
- `dashboards/alerts/sorafs_fetch_rules.yml` — כשלי/ retries/ stalls של האורקסטרטור; מאומת דרך `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` ו-`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — קפיצות ירידת פרטיות, אזעקות suppression, זיהוי collector סרק והתראות על collector מושבת (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — אזעקות brownout של אנונימיות המחוברות ל-`sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — אזעקות drift/ingest/CEK lag ב-Taikai viewer לצד התראות penalty/cooldown חדשות לבריאות הוכחות SoraFS המופעלות ב-`torii_sorafs_proof_health_*`.

## אסטרטגיית tracing

- לאמץ OpenTelemetry מקצה לקצה:
  - Gateways פולטות OTLP spans (HTTP) עם מזהי בקשה, digests של manifest ו-token hashes.
  - האורקסטרטור משתמש ב-`tracing` + `opentelemetry` לייצוא spans עבור נסיונות fetch.
  - נודי SoraFS משובצים פולטות spans עבור אתגרי PoR ופעולות אחסון. כל הרכיבים חולקים trace ID משותף שמופץ דרך `x-sorafs-trace`.
- `SorafsFetchOtel` מגשר בין מדדי האורקסטרטור להיסטוגרמות OTLP בעוד אירועי `telemetry::sorafs.fetch.*` מספקים payloads JSON קלילים ל-backends ממוקדי לוגים.
- Collectors: הריצו OTEL collectors לצד Prometheus/Loki/Tempo (Tempo מועדף). אקספורטרים תואמי Jaeger נשארים אופציונליים.
- פעולות בעלות קרדינליות גבוהה יש לדגום (10% למסלולי הצלחה, 100% לכשלים).

## תיאום טלמטריה TLS (SF-5b)

- יישור מדדים:
  - אוטומציית TLS משגרת `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` ו-`sorafs_gateway_tls_ech_enabled`.
  - הכלילו את המדדים הללו בדשבורד Gateway Overview תחת הפאנל TLS/Certificates.
- קישור התראות:
  - כאשר התראות תפוגת TLS מופעלות (≤ 14 ימים שנותרו) קשרו מול SLO זמינות trustless.
  - השבתת ECH מפיקה התראה משנית שמפנה גם ללוחות TLS וגם לזמינות.
- Pipeline: עבודת אוטומציית TLS מייצאת לאותו סטאק Prometheus של מדדי gateway; התיאום עם SF-5b מבטיח instrumentation ללא כפילויות.

## קונבנציות שמות ו-Labels

- שמות המדדים עוקבים אחר הפרפיקסים הקיימים `torii_sorafs_*` או `sorafs_*` שמשמשים את Torii וה-gateway.
- סטים של Labels סטנדרטיים:
  - `result` → תוצאת HTTP (`success`, `refused`, `failed`).
  - `reason` → קוד סירוב/שגיאה (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → מזהה ספק מקודד hex.
  - `manifest` → digest קנוני של manifest (נחתך כשקרדינליות גבוהה).
  - `tier` → Labels של tier ברמת הצהרה (`hot`, `warm`, `archive`).
- נקודות פליטת טלמטריה:
  - מדדי gateway חיים תחת `torii_sorafs_*` ומשתמשים בקונבנציות של `crates/iroha_core/src/telemetry.rs`.
  - האורקסטרטור פולט מדדי `sorafs_orchestrator_*` ואירועי `telemetry::sorafs.fetch.*` (lifecycle, retry, provider failure, error, stall) שמסומנים עם digest של manifest, job ID, region ומזהי ספק.
  - נודים מציגים `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` ו-`torii_sorafs_por_*`.
- תיאמו עם Observability לרישום קטלוג המדדים במסמך השמות של Prometheus המשותף, כולל ציפיות קרדינליות של labels (גבולות עליונים לספקים/manifests).

## Data Pipeline

- Collectors נפרסים לצד כל רכיב, ומייצאים OTLP ל-Prometheus (מדדים) ול-Loki/Tempo (לוגים/טרייסים).
- eBPF אופציונלי (Tetragon) מעשיר tracing נמוך-רמה עבור gateways/nודים.
- השתמשו ב-`iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` עבור Torii ונודים משובצים; האורקסטרטור ממשיך לקרוא ל-`install_sorafs_fetch_otlp_exporter`.

## Hooks של ולידציה

- הריצו `scripts/telemetry/test_sorafs_fetch_alerts.sh` ב-CI כדי לוודא שכללי ההתראה של Prometheus נשארים מסונכרנים עם מדדי stalls ובדיקות suppression לפרטיות.
- שמרו דשבורדים של Grafana תחת בקרת גרסאות (`dashboards/grafana/`) ועדכנו צילומי מסך/קישורים כאשר הפאנלים משתנים.
- Drills של כאוס מתעדים תוצאות דרך `scripts/telemetry/log_sorafs_drill.sh`; הוולידציה משתמשת ב-`scripts/telemetry/validate_drill_log.sh` (ראו [Playbook תפעול](operations-playbook.md)).
