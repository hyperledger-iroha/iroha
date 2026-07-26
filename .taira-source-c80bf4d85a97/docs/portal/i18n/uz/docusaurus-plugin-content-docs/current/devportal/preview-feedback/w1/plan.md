---
id: preview-feedback-w1-plan
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W1 — Hamkorlar va Torii integratorlari |
| Maqsadli oyna | 2025-yilning 2-chorak 3-hafta |
| Artefakt yorlig'i (rejalashtirilgan) | `preview-2025-04-12` |
| Kuzatuvchi muammosi | `DOCS-SORA-Preview-W1` |

## Maqsadlar

1. Hamkorni oldindan ko'rish shartlari uchun xavfsiz huquqiy + boshqaruv tasdiqlari.
2. Takliflar to‘plamida ishlatiladigan proksi-server va telemetriya suratlarini sinab ko‘ring.
3. Tekshirish summasi bilan tasdiqlangan oldindan ko'rish artefaktini va tekshiruv natijalarini yangilang.
4. Takliflar yuborilishidan oldin hamkorlar ro'yxatini + so'rov shablonlarini yakunlang.

## Vazifalarni taqsimlash

| ID | Vazifa | Egasi | Muddati | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Oldindan ko'rish shartlariga qo'shimcha | uchun qonuniy ruxsat oling Hujjatlar/DevRel yetakchisi → Huquqiy | 2025-04-05 | ✅ Tugallandi | `DOCS-SORA-Preview-W1-Legal` yuridik chiptasi 2025-04-05 da imzolangan; Pdf kuzatuvchiga biriktirilgan. |
| W1-P2 | Suratga oling Uni sinab ko'ring proksi-serverni sozlash oynasi (2025-04-10) va proksi-server holatini tasdiqlang | Docs/DevRel + Ops | 2025-04-06 | ✅ Tugallandi | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 da bajarilgan; CLI transkripti + `.env.tryit-proxy.bak` arxivlangan. |
| W1-P3 | Oldindan koʻrish artefaktini (`preview-2025-04-12`) yarating, `scripts/preview_verify.sh` + `npm run probe:portal` ishga tushiring, arxiv identifikatori/nazorat summalari | Portal TL | 2025-04-08 | ✅ Tugallandi | Artefact + tekshirish jurnallari `artifacts/docs_preview/W1/preview-2025-04-12/` ostida saqlanadi; prob chiqishi trekerga biriktirilgan. |
| W1-P4 | Hamkorlarni qabul qilish shakllarini ko'rib chiqing (`DOCS-SORA-Preview-REQ-P01…P08`), kontaktlarni tasdiqlang + NDA | Boshqaruv bilan aloqa | 2025-04-07 | ✅ Tugallandi | Barcha sakkizta soʻrov maʼqullandi (oxirgi ikkitasi 2025-04-11 da tozalangan); tasdiqlashlar trekerga bog'langan. |
| W1-P5 | Taklif loyihasi nusxasi (`docs/examples/docs_preview_invite_template.md` asosida), har bir hamkor uchun `<preview_tag>` va `<request_ticket>` o'rnating | Docs/DevRel yetakchisi | 2025-04-08 | ✅ Tugallandi | Taklif qoralamasi artefakt havolalari bilan birga 2025-04-12 15:00 UTC kuni yuborilgan. |

## Parvozdan oldin nazorat ro'yxati

> Maslahat: 1-5-bosqichlarni avtomatik ravishda bajarish uchun `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` dasturini ishga tushiring (qurilish, nazorat summasini tekshirish, portal tekshiruvi, havola tekshiruvi va proksi-serverni yangilash). Skript JSON jurnalini yozib oladi, siz kuzatuvchi muammosiga biriktirishingiz mumkin.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` bilan) `build/checksums.sha256` va `build/release.json`ni qayta tiklash uchun.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` va arxiv `build/link-report.json` deskriptor yonida.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (yoki `--tryit-target` orqali tegishli maqsadni taqdim eting); yangilangan `.env.tryit-proxy` ni bajaring va `.bak` ni orqaga qaytarish uchun saqlang.
6. Jurnal yo'llari bilan W1 kuzatuvchisi muammosini yangilang (deskriptor nazorat summasi, prob chiqishi, Proksi-serverni o'zgartirishni sinab ko'ring, Grafana suratlari).

## Dalillarni tekshirish ro'yxati

- [x] `DOCS-SORA-Preview-W1` ga biriktirilgan imzolangan qonuniy tasdiq (PDF yoki chipta havolasi).
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` uchun Grafana skrinshotlari.
- [x] `preview-2025-04-12` deskriptor + `artifacts/docs_preview/W1/` ostida saqlangan nazorat summasi jurnali.
- [x] `invite_sent_at` vaqt belgilari to'ldirilgan takliflar ro'yxati jadvali (W1 treker jurnaliga qarang).
- [x] Fikr-mulohaza artefaktlari [`preview-feedback/w1/log.md`](./log.md) da har bir sherik uchun bitta qator bilan aks ettirilgan (ro‘yxat/temetriya/muammo ma’lumotlari bilan 2025-04-26 yangilangan).

Vazifalar bajarilganda ushbu rejani yangilang; treker yo'l xaritasini saqlash uchun unga havola qiladi
tekshirilishi mumkin.

## Fikr-mulohaza ish jarayoni

1. Har bir sharhlovchi uchun shablonni nusxa ko'chiring
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   metama'lumotlarni to'ldiring va tugallangan nusxani ostida saqlang
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Takliflar, telemetriya nazorat punktlari va jonli jurnal ichidagi ochiq muammolarni umumlashtiring
   [`preview-feedback/w1/log.md`](./log.md) shuning uchun boshqaruv sharhlovchilari butun toʻlqinni takrorlashlari mumkin
   ombordan chiqmasdan.
3. Ma'lumotni tekshirish yoki so'rov eksporti kelganda, ularni jurnalda ko'rsatilgan artefakt yo'liga biriktiring.
   va treker muammosini o'zaro bog'lash.