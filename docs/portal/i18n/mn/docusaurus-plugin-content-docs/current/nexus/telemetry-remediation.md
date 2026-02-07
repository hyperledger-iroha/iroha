---
id: nexus-telemetry-remediation
lang: mn
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Тойм

Замын зургийн зүйл **B2 — телеметрийн цоорхойг эзэмших** нь нийтэлсэн төлөвлөгөөний уялдааг шаарддаг
дохио, дохиоллын хашлага, эзэмшигч, Nexus телеметрийн ялгаа бүр.
эцсийн хугацаа болон 2026 оны 1-р улирлын аудитын цонх эхлэхээс өмнө баталгаажуулах олдвор.
Энэ хуудас нь `docs/source/nexus_telemetry_remediation_plan.md`-ийг тусгаж байгаа тул суллана уу
инженерчлэл, телеметрийн үйл ажиллагаа болон SDK эзэмшигчид хамрах хүрээг баталгаажуулах боломжтой
routed-trace болон `TRACE-TELEMETRY-BRIDGE` давтлага.

# Цоорхой матриц

| Gap ID | Дохио ба дохиоллын хашлага | Эзэмшигч / өргөлт | Хугацаа (UTC) | Нотлох баримт ба баталгаажуулалт |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 минутын турш асах үед **`SoranetLaneAdmissionLatencyDegraded`** дохиололтой `torii_lane_admission_latency_seconds{lane_id,endpoint}` гистограм (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (дохио) + `@telemetry-ops` (анхаарал); дуудлага дээр Nexus routed-trace-ээр дамжуулан хурдасгах. | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml`-ийн дагуу дохиоллын туршилтууд дээр нэмсэн `TRACE-LANE-ROUTING` давталтын зураг дээр шатсан/сэргээгдсэн дохиолол, Torii `/metrics` хусах нь [I18NT00000005-д архивлагдсан. тэмдэглэл](./nexus-transition-notes). |
| `GAP-TELEM-002` | `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` хашлагатай `nexus_config_diff_total{knob,profile}` тоолуур (`docs/source/telemetry.md`). | `@nexus-core` (хэрэгсэл) → `@telemetry-ops` (анхаарал); Гэнэтийн тоо нэмэгдэхэд засаглалын жижүүр хуудаснаа. | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`-ийн хажууд хадгалагдсан засаглалын хуурай гүйлтийн гаралт; хувилбарын хяналтын жагсаалтад Prometheus асуулгын дэлгэцийн агшин болон `StateTelemetry::record_nexus_config_diff` ялгаа ялгарсныг нотлох бүртгэлийн ишлэл багтсан болно. |
| `GAP-TELEM-003` | Алдаа эсвэл дутуу үр дүн нь 30 минутаас дээш хугацаагаар үргэлжилбэл **`NexusAuditOutcomeFailure`** дохиотой `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome` хэмжигдэхүүн) үйл явдал (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (дамжуулах хоолой) `@sec-observability` хүртэл нэмэгдэж байна. | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` нь NDJSON-ийн ачааллыг архивлаж, TRACE цонхонд амжилттай үйл явдал байхгүй үед амжилтгүй болдог; чиглүүлэлтийн тайланд хавсаргасан дохионы дэлгэцийн агшин. |
| `GAP-TELEM-004` | `nexus_lane_configured_total` хэмжигч `nexus_lane_configured_total != EXPECTED_LANE_COUNT` хашлага бүхий SRE дуудлагын хяналтын хуудсыг тэжээдэг. | Зангилаанууд нь каталогийн хэмжээ нийцэхгүй байгааг мэдээлэх үед `@telemetry-ops` (хэмжигч/экспорт) `@nexus-core` хүртэл нэмэгддэг. | 2026-02-28 | Төлөвлөгчийн телеметрийн туршилт `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ялгаруулалтыг баталж байна; операторууд Prometheus diff + `StateTelemetry::set_nexus_catalogs` бүртгэлийн хэсгийг TRACE давталтын багцад хавсаргана. |

# Үйл ажиллагааны ажлын урсгал

1. **Долоо хоног тутмын гурвалж.** Эзэмшигчид Nexus бэлэн байдлын дуудлагад явцын талаар мэдээлдэг;
   хориглогч болон дохиоллын туршилтын олдворуудыг `status.md`-д бүртгэсэн.
2. **Сэрэмжлүүлэг хуурай гүйлт.** Анхааруулах дүрэм бүр нь a
   `dashboards/alerts/tests/*.test.yml` оролт нь CI нь `promtool тестийг гүйцэтгэдэг.
   дүрэм` хашлага солигдох бүрт.
3. **Аудитын нотлох баримт.** `TRACE-LANE-ROUTING` болон
   `TRACE-TELEMETRY-BRIDGE` сургуулилт нь дуудлагаар Prometheus асуулга авдаг.
   үр дүн, дохиоллын түүх, холбогдох скрипт гаралт
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   Хамааралтай дохионы хувьд `scripts/telemetry/check_redaction_status.py`) ба
   тэднийг чиглүүлсэн ул мөрийн олдворуудтай хамт хадгалдаг.
4. **Даамжрах.** Хэрвээ бэлтгэл хийсэн цонхны гадна ямар нэгэн хашлага гал гарсан тохиолдолд эзэмшигч
   баг Nexus ослын тасалбарыг энэ төлөвлөгөөний лавлагаа, түүний дотор
   аудитыг үргэлжлүүлэхээс өмнө хэмжүүрийн агшин зуурын зураг болон багасгах алхмууд.

Энэхүү матрицыг нийтэлсэн бөгөөд `roadmap.md` болон аль алинаас нь иш татсан
`status.md` — замын зураглалын зүйл **B2** одоо "хариуцлага, эцсийн хугацаа,
дохиолол, баталгаажуулалт” хүлээн авах шалгуур.