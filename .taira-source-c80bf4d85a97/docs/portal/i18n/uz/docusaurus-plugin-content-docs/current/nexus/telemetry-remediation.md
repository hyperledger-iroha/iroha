---
id: nexus-telemetry-remediation
lang: uz
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Umumiy ko'rinish

Yoʻl xaritasi bandi **B2 — telemetriya boʻshligʻiga egalik** uchun eʼlon qilingan rejani bogʻlash kerak
signalga har bir ajoyib Nexus telemetriya bo'shlig'i, ogohlantirish panjarasi, egasi,
oxirgi muddat va 2026-yilning 1-choragi audit oynalari boshlanishidan oldin tekshirish artefakti.
Ushbu sahifa `docs/source/nexus_telemetry_remediation_plan.md`ni aks ettiradi, shuning uchun qo'yib yuboring
muhandislik, telemetriya operatsiyalari va SDK egalari qamrovni oldindan tasdiqlashlari mumkin
routed-trace va `TRACE-TELEMETRY-BRIDGE` mashqlari.

# Bo'shliq matritsasi

| Gap ID | Signal va ogohlantirish panjarasi | Egasi / eskalatsiya | Muddati (UTC) | Dalil va tekshirish |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 daqiqa davomida yonayotganda **`SoranetLaneAdmissionLatencyDegraded`** ogohlantirishli `torii_lane_admission_latency_seconds{lane_id,endpoint}` gistogrammasi (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (ogohlantirish); Nexus routed-trace on-chaqiriq orqali oshirish. | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` ostida ogohlantirish testlari va `TRACE-LANE-ROUTING` repetitsiya surati, [I18NT00000005 faylida arxivlangan. eslatmalar](./nexus-transition-notes). |
| `GAP-TELEM-002` | `nexus_config_diff_total{knob,profile}` hisoblagichi himoya panjarali `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` o'rnatildi (`docs/source/telemetry.md`). | `@nexus-core` (asboblar) → `@telemetry-ops` (ogohlantirish); Hisoblagich kutilmaganda oshib ketganda boshqaruv navbatchisi sahifaga kirdi. | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` yonida saqlangan boshqaruv quruq ish natijalari; chiqarish nazorat roʻyxati Prometheus soʻrovi skrinshotini va `StateTelemetry::record_nexus_config_diff` farqni chiqarganligini tasdiqlovchi jurnaldan koʻchirmani oʻz ichiga oladi. |
| `GAP-TELEM-003` | `TelemetryEvent::AuditOutcome` hodisasi (metrik `nexus.audit.outcome`) **`NexusAuditOutcomeFailure`** ogohlantirishi bilan nosozliklar yoki etishmayotgan natijalar >30 daqiqa davom etganda (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (quvur liniyasi) `@sec-observability` gacha ko'tarilmoqda. | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON foydali yuklarini arxivlaydi va TRACE oynasida muvaffaqiyatli hodisa boʻlmaganda ishlamay qoladi; marshrutlangan kuzatuv hisobotiga biriktirilgan ogohlantirish skrinshotlari. |
| `GAP-TELEM-004` | Qo'riqchi panjarali `nexus_lane_configured_total` o'lchagichi `nexus_lane_configured_total != EXPECTED_LANE_COUNT` SRE chaqiruv bo'yicha nazorat ro'yxatini oziqlantiradi. | Tugunlar nomuvofiq katalog o‘lchamlari haqida xabar berganda, `@telemetry-ops` (o‘lchov/eksport) `@nexus-core` ga ko‘tariladi. | 2026-02-28 | Rejalashtiruvchi telemetriya testi `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` emissiyani tasdiqlaydi; operatorlar TRACE repetisiya paketiga Prometheus diff + `StateTelemetry::set_nexus_catalogs` jurnalidan parchani biriktiradilar. |

# Operatsion ish jarayoni

1. **Haftalik triaj.** Egalari Nexus tayyorgarligi chaqiruvida taraqqiyot haqida hisobot beradi;
   blokerlar va ogohlantirish-test artefaktlari `status.md` da qayd etilgan.
2. **Ogohlantirish quruq yugurish.** Har bir ogohlantirish qoidasi a bilan birga keladi
   `dashboards/alerts/tests/*.test.yml` yozuvi, shuning uchun CI promtool testini amalga oshiradi
   to'siq o'zgarganda qoidalar`.
3. **Audit dalillari.** `TRACE-LANE-ROUTING` va
   `TRACE-TELEMETRY-BRIDGE` mashqlari qo'ng'iroq bo'yicha Prometheus so'rovini oladi
   natijalar, ogohlantirishlar tarixi va tegishli skript natijalari
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   Korrelyatsiya qilingan signallar uchun `scripts/telemetry/check_redaction_status.py`) va
   ularni marshrutlangan iz artefaktlari bilan saqlaydi.
4. **Eskalatsiya.** Mashq qilingan oynadan tashqarida biron bir panjara o't ochsa, egasi
   jamoa Nexus voqea chiptasini ushbu rejaga havola qiladi, shu jumladan
   tekshirishni davom ettirishdan oldin metrik surat va yumshatish bosqichlari.

Ushbu matritsa nashr etilgan va ikkala `roadmap.md` va havola qilingan
`status.md` — yoʻl xaritasi bandi **B2** endi “masʼuliyat, muddat,
ogohlantirish, tekshirish” qabul qilish mezonlari.