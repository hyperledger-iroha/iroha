---
lang: hy
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

# Ընդհանուր ակնարկ

Ճանապարհային քարտեզի կետ **B2 — հեռաչափության բացը սեփականության իրավունքը** պահանջում է հրապարակված պլանի կապակցում
յուրաքանչյուր ակնառու Nexus հեռաչափական բացը դեպի ազդանշան, զգոն պահակ, սեփականատեր,
վերջնաժամկետը և ստուգման արտեֆակտը մինչև 2026 թվականի առաջին եռամսյակի աուդիտի պատուհանների սկիզբը:
Այս էջը արտացոլում է `docs/source/nexus_telemetry_remediation_plan.md`-ը, ուստի թողարկեք
ճարտարագիտություն, հեռաչափության օպերացիաներ և SDK-ի սեփականատերերը կարող են հաստատել նախօրոք ծածկույթը
routed-trace և `TRACE-TELEMETRY-BRIDGE` փորձեր:

# Բաց մատրիցա

| Բաց ID | Ազդանշանի և ազդանշանային պահակապակույտ | Սեփականատեր / էսկալացիա | Ժամկետանց (UTC) | Ապացույցներ և ստուգում |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` հիստոգրամը զգուշացումով **`SoranetLaneAdmissionLatencyDegraded`** կրակում է `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 րոպեի ընթացքում (`dashboards/alerts/soranet_lane_rules.yml`): | `@torii-sdk` (ազդանշան) + `@telemetry-ops` (զգուշացում); էսկալացիա Nexus երթուղված-հետք կանչի միջոցով: | 2026-02-23 | Զգուշացման փորձարկումներ `dashboards/alerts/tests/soranet_lane_rules.test.yml`-ի ներքո, գումարած `TRACE-LANE-ROUTING` փորձնական նկարահանումը, որը ցույց է տալիս կրակված/վերականգնված ահազանգը և Torii `/metrics` քերծվածքը՝ արխիվացված [I18NT00000005-ում: նշումներ](./nexus-transition-notes): |
| `GAP-TELEM-002` | `nexus_config_diff_total{knob,profile}` հաշվիչը պաշտպանիչ բազրիքով `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` տեղադրվում է դարպաս (`docs/source/telemetry.md`): | `@nexus-core` (գործիքավորում) → `@telemetry-ops` (զգուշացում); կառավարման հերթապահը էջ է անում, երբ հաշվիչն անսպասելիորեն ավելանում է: | 2026-02-26 | Կառավարման չոր գործարկման արդյունքները պահվում են `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`-ի կողքին; թողարկման ստուգաթերթը ներառում է Prometheus հարցման սքրինշոթը և գրանցամատյանի հատվածը, որն ապացուցում է, որ `StateTelemetry::record_nexus_config_diff`-ը թողարկել է տարբերությունը: |
| `GAP-TELEM-003` | Իրադարձություն `TelemetryEvent::AuditOutcome` (մետրային `nexus.audit.outcome`) զգուշացումով **`NexusAuditOutcomeFailure`**, երբ ձախողումները կամ բացակայող արդյունքները պահպանվում են ավելի քան 30 րոպե (`dashboards/alerts/nexus_audit_rules.yml`): | `@telemetry-ops` (խողովակաշար)՝ աճող մինչև `@sec-observability`: | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py`-ը արխիվացնում է NDJSON-ի ծանրաբեռնվածությունը և ձախողվում է, երբ TRACE պատուհանում բացակայում է հաջող իրադարձությունը. ազդանշանային սքրինշոթներ, որոնք կցված են երթուղված հետքի հաշվետվությանը: |
| `GAP-TELEM-004` | Չափիչ `nexus_lane_configured_total` պաշտպանական բազրիքով `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, որը սնուցում է SRE-ի հերթապահ ստուգաթերթը: | `@telemetry-ops` (չափիչ/արտահանում) աճում է մինչև `@nexus-core`, երբ հանգույցները հայտնում են կատալոգի անհամապատասխան չափերը: | 2026-02-28 | Ժամանակացույցի հեռաչափության թեստը `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ապացուցում է արտանետումը; օպերատորները TRACE փորձնական փաթեթին կցում են Prometheus diff + `StateTelemetry::set_nexus_catalogs` մատյանից քաղվածք: |

# Գործառնական աշխատանքային հոսք

1. **Շաբաթական տրաժ.** Սեփականատերերը հայտնում են առաջընթացի մասին Nexus պատրաստականության զանգի ժամանակ;
   արգելափակողները և ազդանշանային փորձարկման արտեֆակտները գրանցված են `status.md`-ում:
2. **Զգուշացնել չոր վազքները։** Զգուշացման յուրաքանչյուր կանոն առաքվում է ա
   `dashboards/alerts/tests/*.test.yml` մուտքագրում, այնպես որ CI-ն իրականացնում է «promtool test
   կանոններ՝ ամեն անգամ, երբ փոխվում է պահակապակույտը:
3. **Աուդիտորական ապացույցներ.** `TRACE-LANE-ROUTING` ընթացքում և
   `TRACE-TELEMETRY-BRIDGE`-ը կրկնում է հերթապահությունը և գրավում է Prometheus հարցումը
   արդյունքներ, ահազանգերի պատմություն և համապատասխան սցենարի արդյունքներ
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` փոխկապակցված ազդանշանների համար) և
   պահում է դրանք հետագծված արտեֆակտներով:
4. **Էսկալացիա։** Եթե որևէ պաշտպանական բազրիք կրակում է փորձված պատուհանից դուրս, սեփականատիրոջը.
   թիմը ներկայացնում է Nexus միջադեպի տոմս՝ հղում կատարելով այս պլանին, ներառյալ
   Աուդիտը վերսկսելուց առաջ մետրական պատկեր և մեղմացման քայլեր:

Այս մատրիցով հրապարակված — և հղում է արվել ինչպես `roadmap.md`-ից, այնպես էլ
`status.md` — ճանապարհային քարտեզի կետը **B2** այժմ համապատասխանում է «պատասխանատվությունը, վերջնաժամկետը,
ահազանգ, ստուգում» ընդունման չափանիշները: