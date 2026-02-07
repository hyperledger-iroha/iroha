---
lang: mn
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd9f85b2c7845414c27016f699da179e13c41c9b9e0ce5b178ab88a950744500
source_last_modified: "2025-12-29T18:16:35.142540+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/nexus_routed_trace_audit_report_2026q1.md`-г тусгадаг. Үлдсэн орчуулгууд буух хүртэл хоёр хуулбарыг зэрэгцүүлэн хадгална уу.
:::

# 2026 оны 1-р улирлын Чиглүүлэлт-Мөшгих аудитын тайлан (B1)

Замын зургийн зүйл **B1 — Routed Trace Audits & Telemetry Baseline** нь
Nexus чиглүүлэгдсэн мөрийн хөтөлбөрийн улирлын тойм. Энэхүү тайлан нь
Q12026 аудитын цонх (1-3-р сар) тул засаглалын зөвлөл гарын үсэг зурж болно.
2-р улирлын хөөргөх сургуулилтаас өмнөх телеметрийн байрлал.

## Хамрах хүрээ ба цагийн хуваарь

| Trace ID | Цонх (UTC) | Зорилго |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Олон эгнээг идэвхжүүлэхийн өмнө эгнээний элсэлтийн гистограмм, хов живийн дараалал, дохиоллын урсгалыг шалгана уу. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | AND4/AND7 үе шатуудын өмнө OTLP дахин тоглуулах, ботын зөрүү болон SDK телеметрийн дамжуулалтыг баталгаажуулна уу. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 тайрахаас өмнө засаглалаар батлагдсан `iroha_config` дельта болон буцаах бэлэн байдлыг баталгаажуулна уу. |

Сургуулилт бүр чиглүүлэлтийн мөр бүхий үйлдвэрлэлийн төстэй топологи дээр явагдсан
багаж хэрэгслийг идэвхжүүлсэн (`nexus.audit.outcome` телеметр + Prometheus тоолуур),
Alertmanager дүрмийг ачаалж, нотлох баримтыг `docs/examples/` руу экспортлосон.

## Арга зүй

1. **Телеметрийн цуглуулга.** Бүх зангилаа нь бүтэцлэгдсэн
   `nexus.audit.outcome` үйл явдал болон дагалдах хэмжүүрүүд
   (`nexus_audit_outcome_total*`). Туслагч
   `scripts/telemetry/check_nexus_audit_outcome.py` JSON логыг дараалсан,
   үйл явдлын статусыг баталгаажуулж, доорх ачааллыг архивласан
   `docs/examples/nexus_audit_outcomes/`.【скриптүүд/телеметри/check_nexus_audit_outcome.py:1】
2. **Сэрүүлгийн баталгаажуулалт.** `dashboards/alerts/nexus_audit_rules.yml` ба түүний туршилт
   морины хэрэгсэл нь дохиоллын дуу чимээний босго болон ачааны загварт тэсвэртэй байдлыг хангасан
   тууштай. CI нь `dashboards/alerts/tests/nexus_audit_rules.test.yml` дээр ажилладаг
   өөрчлөлт бүр; цонх бүрийн үеэр ижил дүрмийг гараар хэрэгжүүлсэн.
3. **Хяналтын самбарын зураг авалт.** Операторууд чиглүүлэгдсэн трасс самбаруудыг дараахаас экспортолсон
   `dashboards/grafana/soranet_sn16_handshake.json` (гар барих эрүүл мэнд) болон
   Дарааллын эрүүл мэндийг аудитын үр дүнтэй уялдуулах телеметрийн тойм хяналтын самбар.
4. **Шүүмжлэгчийн тэмдэглэл.** Засаглалын нарийн бичгийн дарга шүүмжлэгчийн нэрийн эхний үсгийг бүртгэсэн,
   шийдвэр болон [Nexus шилжилтийн тэмдэглэл](./nexus-transition-notes) дахь аливаа бууруулах тасалбар
   болон тохиргооны дельта трекер (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Судалгаа

| Trace ID | Үр дүн | Нотлох баримт | Тэмдэглэл |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Сэрэмжлүүлгийн гал/сэргээх дэлгэцийн агшин (дотоод холбоос) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` дахин тоглуулах; [Nexus шилжилтийн тэмдэглэл](./nexus-transition-notes#quarterly-routed-trace-audit-schedule)-д бүртгэгдсэн телеметрийн ялгаа. | Дараалалд орох P95 нь 612 мс (зорилтот ≤750 мс) хэвээр байна. Дагаж мөрдөх шаардлагагүй. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Архивлагдсан үр дүнгийн ачаалал `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` дээр нэмэх нь `status.md` дээр бүртгэгдсэн OTLP дахин тоглуулах хэш. | SDK редакцийн давс нь Rust-ийн суурь үзүүлэлттэй таарч байсан; diff bot тэг дельта гэж мэдээлсэн. |
| `TRACE-CONFIG-DELTA` | Pass (зөвшөөрөл хаагдсан) | Засаглалын трекерийн оруулга (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS профайл манифест (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + телеметрийн багц манифест (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | 2-р улиралд дахин ажиллуулах нь батлагдсан TLS профайлыг хэш болгож, тэг stragglers-ыг баталгаажуулсан; телеметрийн манифест 912–936 хоорондын зай болон ажлын ачааллын үрийг `NEXUS-REH-2026Q2` тэмдэглэдэг. |

Бүх ул мөр нь дор хаяж нэг `nexus.audit.outcome` үйл явдлыг үүсгэсэн
Alertmanager хамгаалалтын хашлага (`NexusAuditOutcomeFailure`) хангасан цонхнууд
улирлын турш ногоон хэвээр байна).

## Хяналт

- Routed-trace хавсралтыг TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`-ээр шинэчилсэн;
  бууруулах `NEXUS-421` шилжилтийн тэмдэглэлд хаагдсан.
- Түүхий OTLP дахин тоглуулах болон Torii ялгаатай олдворуудыг архивт үргэлжлүүлэн хавсаргана уу.
  Android AND4/AND7 тоймуудын паритын нотолгоог бэхжүүлэх.
- Удахгүй болох `TRACE-MULTILANE-CANARY` сургуулилтууд ижил зүйлийг дахин ашиглахыг баталгаажуулна уу.
  телеметрийн туслагч тул 2-р улиралд гарын үсэг зурах нь батлагдсан ажлын урсгалаас ашиг тустай.

## Олдворын индекс

| Хөрөнгө | Байршил |
|-------|----------|
| Телеметрийн баталгаажуулагч | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Анхааруулах дүрэм ба тестүүд | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Үр дүнгийн ачааллын жишээ | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Дельта мөрдөгчийг тохируулах | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Чиглүүлсэн-мөшгих хуваарь & тэмдэглэл | [Nexus шилжилтийн тэмдэглэл](./nexus-transition-notes) |

Энэхүү тайлан, дээрх олдворууд болон дохиолол/телеметрийн экспортууд байх ёстой
улирлын B1-ийг хаахын тулд засаглалын шийдвэрийн бүртгэлд хавсаргав.