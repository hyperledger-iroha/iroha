---
lang: kk
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 35fe9abd10cb1454b72042b5b9dfbc35d45cc1cd91e2a4d0af4909032189df22
source_last_modified: "2025-12-29T18:16:35.147058+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-telemetry-remediation
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
---

# Шолу

Жол картасы тармағы **B2 — телеметриялық алшақтықты иелену** жарияланған жоспарды байланыстыруды талап етеді
сигналға дейінгі әрбір көрнекті Nexus телеметриялық алшақтық, ескерту қоршауы, иесі,
соңғы мерзім және 2026 жылдың 1 тоқсанының тексеру терезелері басталғанға дейін растау артефакті.
Бұл бет `docs/source/nexus_telemetry_remediation_plan.md` көрсетеді, сондықтан шығарыңыз
инженерлік, телеметриялық операциялар және SDK иелері қамтуды алдын ала растай алады
routed-trace және `TRACE-TELEMETRY-BRIDGE` жаттығулары.

# Алшақтық матрицасы

| Gap ID | Сигнал және ескерту қоршауы | Меншік иесі/эскалация | Төлеу мерзімі (UTC) | Дәлелдеу және тексеру |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 минут бойы (`dashboards/alerts/soranet_lane_rules.yml`) іске қосылғанда **`SoranetLaneAdmissionLatencyDegraded`** дабылы бар `torii_lane_admission_latency_seconds{lane_id,endpoint}` гистограммасы. | `@torii-sdk` (сигнал) + `@telemetry-ops` (дабыл); Nexus маршруттық бақылау арқылы қоңырау шалу. | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` астында ескерту сынақтары және `TRACE-LANE-ROUTING` репетиция түсірілімі іске қосылған/қалпына келтірілген ескертуді және Torii `/metrics` [I18NT00000005 мұрағатталған ескертпелер](./nexus-transition-notes). |
| `GAP-TELEM-002` | `nexus_config_diff_total{knob,profile}` есептегіш қоршауы бар `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` қақпасы бар (`docs/source/telemetry.md`). | `@nexus-core` (аспаптар) → `@telemetry-ops` (ескерту); санауыш күтпеген жерден ұлғайған кезде басқарушы кезекші бетке шықты. | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` жанында сақталған басқару құрғақ орындалатын шығыстары; шығарылымды тексеру тізімінде Prometheus сұрау скриншоты және `StateTelemetry::record_nexus_config_diff` айырмашылықты шығарғанын дәлелдейтін журнал үзіндісі бар. |
| `GAP-TELEM-003` | Сәтсіздіктер немесе жетіспейтін нәтижелер >30 минут бойы сақталса, ескертуі бар `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome` метрикалық) оқиғасы **`NexusAuditOutcomeFailure`** (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (құбыр) `@sec-observability` дейін көтеріледі. | 2026-02-27 | CI қақпасы `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON пайдалы жүктемелерін мұрағаттайды және TRACE терезесінде сәтті оқиға болмаған кезде сәтсіздікке ұшырайды; бағдарланған бақылау есебіне тіркелген ескерту скриншоттары. |
| `GAP-TELEM-004` | `nexus_lane_configured_total` көрсеткіші `nexus_lane_configured_total != EXPECTED_LANE_COUNT` қоршауы бар SRE шақыру бойынша бақылау парағын береді. | Түйіндер сәйкес келмейтін каталог өлшемдерін хабарлағанда, `@telemetry-ops` (өлшеуіш/экспорт) `@nexus-core` дейін өседі. | 2026-02-28 | Жоспарлағыш телеметрия сынағы `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` сәуле шығаруды дәлелдейді; операторлар Prometheus diff + `StateTelemetry::set_nexus_catalogs` журналының үзіндісін TRACE репетиция пакетіне қосады. |

# Операциялық жұмыс процесі

1. **Апта сайынғы триаж.** Иелер Nexus дайындық қоңырауында орындалу барысы туралы хабарлайды;
   блокаторлар мен ескерту сынағы артефактілері `status.md` жүйесінде тіркеледі.
2. **Құрғақ жұмыстарды ескерту.** Әрбір ескерту ережесі a-мен бірге жіберіледі
   `dashboards/alerts/tests/*.test.yml` жазбасы, сондықтан CI 'promtool сынағын орындайды.
   ережелер` қоршау өзгерген сайын.
3. **Аудиторлық дәлелдер.** `TRACE-LANE-ROUTING` кезінде және
   `TRACE-TELEMETRY-BRIDGE` репетициялары қоңырау бойынша Prometheus сұрауын түсіреді
   нәтижелер, ескертулер журналы және сәйкес сценарий шығыстары
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   Корреляциялық сигналдар үшін `scripts/telemetry/check_redaction_status.py`) және
   оларды бағдарланған із артефактілерімен бірге сақтайды.
4. **Эскалация.** Егер қандай да бір қоршау репетицияланған терезенің сыртында өртенсе, иесі
   команда осы жоспарға сілтеме жасайтын Nexus оқиға билетін, соның ішінде
   тексерулерді жалғастырмас бұрын метрикалық сурет және азайту қадамдары.

Осы матрица жарияланған — және сілтеме `roadmap.md` және
`status.md` — жол картасының **B2** тармағы енді «жауапкершілікке, мерзімге,
ескерту, тексеру» қабылдау критерийлері.