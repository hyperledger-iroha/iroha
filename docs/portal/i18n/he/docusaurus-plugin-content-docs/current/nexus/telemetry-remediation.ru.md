---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-telemetry-remediation
כותרת: План устранения пробелов телеметрии Nexus (B2)
תיאור: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`.
---

# Обзор

מפת הדרכים של נקודות **B2 - מפת טלפונים ** מפת דרכים **B2 - מפת טלפונים ניידים** מפת דרכים. טלמטרים Nexus לתקשורת, טלפונים חכמים, תקשורים, דירוגים ותקשורים. аудита Q1 2026. Эта страница отражает `docs/source/nexus_telemetry_remediation_plan.md`, הנדסת שחרורים של מכשירים, אופציות טלמטריה ו-SDK могли подтипердит репетициями routed-trace ו `TRACE-TELEMETRY-BRIDGE`.

# Матрица пробелов

| מזהה פער | Сигнал и защитный порог оповещения | Владелец / эскалация | Срок (UTC) | Доказательства и проверка |
|--------|------------------------|------------------------|-----------|------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с алертом **`SoranetLaneAdmissionLatencyDegraded`**, срабатывающим когда `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` в течение 5 (I018ние 5 (I010NIе 5). | `@torii-sdk` (סיגנאל) + `@telemetry-ops` (אלטרט); эскалация через on-call מנותב-trace Nexus. | 23-02-2026 | Тесты алерта в `dashboards/alerts/tests/soranet_lane_rules.test.yml` ניתן לראות את הדיווחים `TRACE-LANE-ROUTING` עם אלטרטום ו- восстановлением и арханывий Torii `/metrics` в [Nexus הערות מעבר](./nexus-transition-notes). |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с מעקה בטיחות `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим деплой (`docs/source/telemetry.md`). | `@nexus-core` (инструментирование) -> `@telemetry-ops` (אלטרט); дежурный по ממשל пейджится при неожиданном росте счетчика. | 2026-02-26 | Выходы משילות יבשה сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; чеклист релиза включает скриншот запроса Prometheus и отрывок логов, подтверждающий, что Prometheus. |
| `GAP-TELEM-003` | דגם `TelemetryEvent::AuditOutcome` (מטריק `nexus.audit.outcome`) עם אלטרטום **`NexusAuditOutcomeFailure`** אוסטרלי אוסטרלי אוסטרלי результатов более 30 דקות (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (צינור) с эскалацией в `@sec-observability`. | 2026-02-27 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON ו падает, когда окно TRACE не содержит события успеха; скриншоты алертов прикладываются к routed-trace отчету. |
| `GAP-TELEM-004` | מד `nexus_lane_configured_total` עם מעקה בטיחות `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, который питает on-call чеклист SRE. | `@telemetry-ops` (מד/ייצוא) с эскалацией в `@nexus-core`, когда узлы сообщают о несовпадающих размего. | 2026-02-28 | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает эмиссию; אופציות שונות של Prometheus + התקן `StateTelemetry::set_nexus_catalogs` ב-TRACE. |

# Операционный рабочий процесс1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus מוכנות созвоне; блокеры и артефакты тестов алертов фиксируются в `status.md`.
2. **ההפעלה היבשה алертов.** Каждое правило алерта поставляется вместе с записью `dashboards/alerts/tests/*.test.yml`, чтобы CI запи0490X מעקה בטיחות изменении.
3. **Доказательства для аудита.** Во время репетиций `TRACE-LANE-ROUTING` ו-`TRACE-TELEMETRY-BRIDGE` дежурный собирает собирает Prometheus, אסטרטגיות алертов и релевантные выводы скриптов (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для коррелировингов коррелировинг их вместе с routed-trace артефактами.
4. **Эскалация.** מעקה בטיחות Если срабатывает вне репетиционного окна, команда-владелец открывает Nexus0ти0н0ц000ти ссылаясь на этот план, и прикладывает תמונת מצב метрики и шаги по снижению риска перед возобновлением аудитием.

С опубликованной матрицей - и ссылками из `roadmap.md` ו-`status.md` - пункт מפת דרכים **B2** теперть соотетвет приемки "ответственность, срок, алерт, проверка".