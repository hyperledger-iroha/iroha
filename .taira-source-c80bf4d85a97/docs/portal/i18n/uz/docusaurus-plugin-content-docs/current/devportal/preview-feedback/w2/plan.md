---
id: preview-feedback-w2-plan
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community intake plan
sidebar_label: W2 plan
description: Intake, approvals, and evidence checklist for the community preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W2 — Hamjamiyat sharhlovchilari |
| Maqsadli oyna | 2025-yilning 3-chorak 1-hafta (taxminiy) |
| Artefakt yorlig'i (rejalashtirilgan) | `preview-2025-06-15` |
| Kuzatuvchi muammosi | `DOCS-SORA-Preview-W2` |

## Maqsadlar

1. Jamiyatni qabul qilish mezonlarini va tekshirish ish jarayonini aniqlang.
2. Taklif etilayotgan ro'yxat va maqbul qo'shimcha uchun boshqaruv ma'qullashini olish.
3. Yangi oyna uchun nazorat summasi bilan tasdiqlangan oldindan ko'rish artefaktini va telemetriya to'plamini yangilang.
4. Taklifni yuborishdan oldin proksi-server + boshqaruv panelini sinab ko'ring.

## Vazifalarni taqsimlash

| ID | Vazifa | Egasi | Muddati | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Hamjamiyatni qabul qilish mezonlari loyihasi (muvofiqlik, maksimal slotlar, CoC talablari) va boshqaruvga tarqatish | Docs/DevRel yetakchisi | 2025-05-15 | ✅ Tugallandi | Qabul qilish siyosati `DOCS-SORA-Preview-W2` bilan birlashtirildi va 2025-05-20 kengash yig'ilishida ma'qullandi. |
| W2-P2 | Jamiyatga oid savollar (motivatsiya, mavjudlik, mahalliylashtirish ehtiyojlari) bilan so'rov shablonini yangilang | Docs-core-01 | 2025-05-18 | ✅ Tugallandi | `docs/examples/docs_preview_request_template.md` endi qabul qilish shaklida havola qilingan Hamjamiyat bo'limini o'z ichiga oladi. |
| W2-P3 | Qabul qilish rejasi uchun xavfsiz boshqaruvni tasdiqlash (yig'ilish ovozi + qayd etilgan bayonnomalar) | Boshqaruv bilan aloqa | 2025-05-22 | ✅ Tugallandi | 2025-05-20 da ovoz berish bir ovozdan qabul qilindi; daqiqalar + `DOCS-SORA-Preview-W2` da bog'langan qo'ng'iroq. |
| W2-P4 | Jadval Sinab ko'ring proksi staging + W2 oynasi uchun telemetriya tasviri (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ Tugallandi | Chiptani o'zgartirish `OPS-TRYIT-188` tasdiqlangan va bajarilgan 2025-06-09 02:00–04:00UTC; Grafana skrinshotlari chipta bilan arxivlangan. |
| W2-P5 | Yangi oldindan ko'rish artefakt yorlig'ini (`preview-2025-06-15`) yaratish/tasdiqlash va identifikator/cheksum/tekshirish jurnallarini arxivlash | Portal TL | 2025-06-07 | ✅ Tugallandi | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025‑06‑10 da ishlagan; `artifacts/docs_preview/W2/preview-2025-06-15/` ostida saqlanadigan chiqishlar. |
| W2-P6 | Boshqaruv tomonidan tasdiqlangan aloqa ma'lumotlari bilan hamjamiyat takliflari ro'yxatini (≤25 sharhlovchi, bosqichli partiyalar) yig'ing | Hamjamiyat menejeri | 2025-06-10 | ✅ Tugallandi | 8 ta jamoatchilik sharhlovchilarining birinchi kogortasi tasdiqlandi; trekerga kirgan `DOCS-SORA-Preview-REQ-C01…C08` so'rov identifikatorlari. |

## Dalillarni tekshirish ro'yxati

- [x] `DOCS-SORA-Preview-W2` ga ilova qilingan boshqaruvni tasdiqlash yozuvi (yig'ilish eslatmalari + ovoz berish havolasi).
- [x] `docs/examples/` ostida amalga oshirilgan yangilangan so'rov shabloni.
- [x] `preview-2025-06-15` identifikatori, nazorat summasi jurnali, tekshirish chiqishi, havola hisoboti va `artifacts/docs_preview/W2/` ostida saqlangan proksi transkriptini sinab ko'ring.
- [x] W2 oldindan parvoz oynasi uchun olingan Grafana skrinshotlari (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`).
- [x] Ko'rib chiquvchi identifikatorlari, so'rov chiptalari va jo'natilishdan oldin to'ldirilgan tasdiqlash vaqt belgilari bilan takliflar ro'yxati jadvali (W2 treker bo'limiga qarang).

Ushbu rejani yangilab turing; treker unga havola qiladi, shuning uchun DOCS-SORA yo'l xaritasi W2 taklifnomalari chiqmasdan oldin nima qolganligini aniq ko'rishi mumkin.