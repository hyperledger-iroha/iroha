---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
translator: machine-google-reviewed
---

Bu jurnal takliflar ro'yxatini, telemetriya nazorat nuqtalarini va sharhlovchining fikr-mulohazalarini saqlaydi
**W1 hamkorni oldindan ko'rish**, bu qabul qilish vazifalari bilan birga keladi
[`preview-feedback/w1/plan.md`](./plan.md) va toʻlqin kuzatuvchisi
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Taklif kelganda uni yangilang
yuboriladi, telemetriya snapshoti yoziladi yoki fikr-mulohaza elementi triajlanadi, shuning uchun boshqaruvni tekshiruvchilar qayta koʻrishlari mumkin
tashqi chiptalarni ta'qib qilmasdan dalillar.

## Kohortlar ro'yxati

| Hamkor ID | Chipta so'rash | NDA qabul qilindi | Taklif yuborildi (UTC) | Qabul qilish/birinchi kirish (UTC) | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| hamkor-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Tugallangan sana: 2025-04-26 | sorafs-op-01; orchestrator doc parite dalillariga e'tibor qaratildi. |
| hamkor-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Tugallangan sana: 2025-04-26 | sorafs-op-02; tasdiqlangan Norito/temetrik o'zaro bog'lanishlar. |
| hamkor-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Tugallangan sana: 2025-04-26 | sorafs-op-03; ko'p manbali o'chirish mashqlarini bajardi. |
| hamkor-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Tugallangan sana: 2025-04-26 | torii-int-01; Torii `/v1/pipeline` + Ovqatlanish kitobini ko'rib chiqing. |
| hamkor-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Tugallangan sana: 2025-04-26 | torii-int-02; ulangan. Skrinshotni yangilash (docs-preview/w1 #2). |
| hamkor-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Tugallangan sana: 2025-04-26 | sdk-partner-01; JS/Swift oshpazlik kitobi bo'yicha fikr-mulohazalar + ISO ko'prigi aql-idrok tekshiruvi. |
| hamkor-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Tugallangan sana: 2025-04-26 | sdk-partner-02; muvofiqlik 2025-04-11 tozalandi, Ulanish/telemetriya qaydlariga qaratilgan. |
| hamkor-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Tugallangan sana: 2025-04-26 | shlyuz-ops-01; tekshirilgan shlyuz operatsiyalari uchun qo'llanma + anonim Proksi oqimini sinab ko'ring. |

Chiqish xati yuborilishi bilanoq **Invite sent** va **Ack** vaqt belgilarini to‘ldiring.
Vaqtlarni W1 rejasida belgilangan UTC jadvaliga bog'lang.

## Telemetriya nazorat punktlari

| Vaqt tamg'asi (UTC) | Boshqaruv paneli / zondlar | Egasi | Natija | Artefakt |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ Hammasi yashil | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` transkripti | Ops | ✅ Sahnalashtirilgan | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`, `probe:portal` | Docs/DevRel + Ops | ✅ Oldindan taklif qilingan surat, regressiyalarsiz | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Yuqoridagi asboblar paneli + Buni sinab ko'ring proksi kechikish farqi | Docs/DevRel yetakchisi | ✅ Oʻrta nuqta tekshiruvidan oʻtdi (0 ta ogohlantirish; Uni sinab koʻring kechikish p95=410ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Yuqoridagi asboblar paneli + probdan chiqish | Docs/DevRel + Boshqaruv aloqasi | ✅ Snapshotdan chiqish, nol ajoyib ogohlantirishlar | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Kundalik ish vaqti namunalari (2025-04-13 → 2025-04-25) NDJSON + PNG eksporti sifatida birlashtirilgan.
Fayl nomlari bilan `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
`docs-preview-integrity-<date>.json` va tegishli skrinshotlar.

## Fikr-mulohaza va muammolar jurnali

Sharhlovchi tomonidan taqdim etilgan xulosalarni umumlashtirish uchun ushbu jadvaldan foydalaning. Har bir kirishni GitHub/muhokamasiga bog'lang
chipta plus orqali olingan tuzilgan shakl
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Malumot | Jiddiylik | Egasi | Holati | Eslatmalar |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Past | Docs-core-02 | ✅ Qaror qabul qilingan 2025-04-18 | Aniqlangan Sinab ko'ring. Nav so'zlari + yon panel langari (`docs/source/sorafs/tryit.md` yangi yorliq bilan yangilangan). |
| `docs-preview/w1 #2` | Past | Docs-core-03 | ✅ Qaror qabul qilingan 2025-04-19 | Yangilandi Sinab ko'ring skrinshot + har bir sharhlovchi so'roviga sarlavha; artefakt `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Ma'lumot | Docs/DevRel yetakchisi | 🢠 Yopiq | Qolgan sharhlar faqat savol-javob uchun edi; `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` ostida har bir sherikning fikr-mulohaza shaklida yozib olingan. |

## Bilimlarni tekshirish va so'rovni kuzatish

1. Har bir sharhlovchi uchun viktorina ballarini yozib oling (maqsad ≥90%); bilan birga eksport qilingan CSV ni biriktiring
   artefaktlarni taklif qilish.
2. Fikr-mulohaza shakli shablonida olingan sifatli so'rov javoblarini to'plang va ularni aks ettiring
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` ostida.
3. Eshikdan past ball olganlar uchun tuzatish chaqiruvlarini rejalashtiring va ularni ushbu faylga kiriting.

Barcha sakkizta sharhlovchi bilimlarni tekshirishda ≥94% ball oldi (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Hech qanday tuzatish qo'ng'iroqlari yo'q
talab qilingan; ostida yashash har bir hamkor uchun so'rov eksport
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Artefakt inventarizatsiyasi

- Ko'rib chiqish deskriptori/nazorat summasi to'plami: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Prob + havolani tekshirish xulosasi: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Proksi-serverni o'zgartirish jurnalini sinab ko'ring: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetriya eksporti: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Kundalik ish soati telemetriya to'plami: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Fikr-mulohaza + so'rovnoma eksporti: ostidagi sharhlovchiga tegishli papkalarni joylashtiring
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Bilimlarni tekshirish CSV va xulosa: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Inventarizatsiyani kuzatuvchi muammosi bilan sinxronlashtiring. Artefaktlarni nusxalashda xeshlarni biriktiring
boshqaruv chiptasi, shuning uchun auditorlar fayllarni qobiqdan foydalanmasdan tekshirishlari mumkin.