---
id: nexus-routed-trace-audit-2026q1
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: Eslatma Kanonik manba
Bu sahifa `docs/source/nexus_routed_trace_audit_report_2026q1.md`ni aks ettiradi. Qolgan tarjimalar tushmaguncha ikkala nusxani ham tekislang.
:::

№ 2026 1-chorak Marshrutlangan kuzatuv auditi hisoboti (B1)

Yoʻl xaritasi bandi **B1 — Routed-trace Audits & Baseline telemetry** talab qiladi
Nexus marshrutlangan kuzatuv dasturini choraklik ko'rib chiqish. Ushbu hisobot hujjatni tasdiqlaydi
Q12026 audit oynasi (yanvar-mart), shuning uchun boshqaruv kengashi imzo chekishi mumkin
Q2 ishga tushirish mashqlaridan oldin telemetriya holati.

## Qo'llanish doirasi va vaqt jadvali

| Trace ID | Oyna (UTC) | Maqsad |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | Ko'p qatorli yoqishdan oldin qatorga kirish gistogrammalarini, navbatdagi g'iybatlarni va ogohlantirish oqimini tekshiring. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | AND4/AND7 bosqichlari oldidan OTLP qayta ijrosi, farq bot pariteti va SDK telemetriya qabul qilinishini tasdiqlang. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | RC1 kesishidan oldin boshqaruv tomonidan tasdiqlangan `iroha_config` deltalari va orqaga qaytishga tayyorligini tasdiqlang. |

Har bir mashq marshrutlangan iz bilan ishlab chiqarishga o'xshash topologiyada o'tdi
asboblar yoqilgan (`nexus.audit.outcome` telemetriya + Prometheus hisoblagichlari),
Alertmanager qoidalari yuklandi va dalillar `docs/examples/` ga eksport qilindi.

## Metodologiya

1. **Telemetriya toʻplami.** Barcha tugunlar tuzilgan
   `nexus.audit.outcome` hodisasi va unga hamroh bo'lgan ko'rsatkichlar
   (`nexus_audit_outcome_total*`). Yordamchi
   `scripts/telemetry/check_nexus_audit_outcome.py` JSON jurnalini yozdi,
   hodisa holatini tasdiqladi va foydali yukni arxivga kiritdi
   `docs/examples/nexus_audit_outcomes/`.【skriptlar/telemetry/check_nexus_audit_outcome.py:1】
2. **Ogohlantirishni tekshirish.** `dashboards/alerts/nexus_audit_rules.yml` va uning testi
   Jabduqlar ogohlantirish shovqin chegaralarini va foydali yuk shablonini saqlab qolishni ta'minladi
   izchil. CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` da ishlaydi
   har bir o'zgarish; har bir oynada bir xil qoidalar qo'lda bajarilgan.
3. **Boshqarish panelini suratga olish.** Operatorlar marshrutlangan kuzatuv panellarini eksport qildilar
   `dashboards/grafana/soranet_sn16_handshake.json` (qo'l siqish salomatligi) va
   Navbat holatini audit natijalari bilan o'zaro bog'lash uchun telemetriya umumiy ko'rinishi asboblar paneli.
4. **Sharhchi qaydlari.** Boshqaruv kotibi sharhlovchining bosh harflarini qayd etdi,
   qaror va [Nexus o'tish eslatmalarida] (./nexus-transition-notes) har qanday yumshatish chiptalari
   va konfiguratsiya delta kuzatuvchisi (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Topilmalar

| Trace ID | Natija | Dalil | Eslatmalar |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Ogohlantirish yong'in/tiklash skrinshotlari (ichki havola) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` takrorlash; [Nexus o'tish qaydlari](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) da qayd etilgan telemetriya farqlari. | Navbatga kirish P95 612ms qoldi (maqsad ≤750ms). Kuzatuv talab qilinmaydi. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Arxivlangan natija yuki `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` va `status.md` da qayd etilgan OTLP takroriy xesh. | SDK redaktsiya tuzlari Rust bazasiga mos keldi; diff bot nol deltalar haqida xabar berdi. |
| `TRACE-CONFIG-DELTA` | Pass (yumshatish yopiq) | Boshqaruv kuzatuvchisi yozuvi (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profili manifesti (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetriya paketi manifesti (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | 2-chorak qayta ishga tushirilganda tasdiqlangan TLS profili xeshlangan va nol stragglerlar tasdiqlangan; telemetriya manifest 912–936 diapazonini va ish yuki urug'ini `NEXUS-REH-2026Q2` qayd qiladi. |

Barcha izlar ularning ichida kamida bitta `nexus.audit.outcome` hodisasini hosil qildi
derazalar, Alertmanager himoya panjaralariga javob beradi (`NexusAuditOutcomeFailure`
chorak davomida yashil bo'lib qoldi).

## Kuzatuvlar

- `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` TLS xesh bilan yangilangan marshrutlangan iz ilovasi;
  yumshatish `NEXUS-421` o'tish eslatmalarida yopildi.
- Arxivga xom OTLP replays va Torii diff artefaktlarini biriktirishda davom eting.
  Android AND4/AND7 sharhlari uchun paritet dalillarini kuchaytirish.
- Kelgusi `TRACE-MULTILANE-CANARY` mashqlari xuddi shu tarzda ishlatilishini tasdiqlang
  telemetriya yordamchisi, shuning uchun Q2 ro'yxatdan o'tish tasdiqlangan ish oqimidan foyda oladi.

## Artefakt indeksi

| Aktiv | Manzil |
|-------|----------|
| Telemetriya tekshiruvi | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Ogohlantirish qoidalari va testlari | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Natija yuki namunasi | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Delta kuzatuvchisini sozlash | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Marshrutlangan kuzatuv jadvali & eslatmalar | [Nexus o'tish qaydlari](./nexus-transition-notes) |

Ushbu hisobot, yuqoridagi artefaktlar va ogohlantirish/telemetriya eksporti bo'lishi kerak
chorak uchun B1ni yopish uchun boshqaruv qarorlari jurnaliga ilova qilinadi.