---
id: preview-feedback-w3-summary
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W3 — Beta kohortlar (moliya + operatsiyalar + SDK hamkori + ekotizim himoyachisi) |
| Taklif oynasi | 2026‑02‑18 → 2026‑02‑28 |
| Artefakt tegi | `preview-20260218` |
| Kuzatuvchi muammosi | `DOCS-SORA-Preview-W3` |
| Ishtirokchilar | moliya-beta-01, kuzatuv-ops-02, hamkor-sdk-03, ekotizim-advokat-04 |

## E'tiborga molik

1. **`npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` har bir to‘lqin xulosasini (`artifacts/docs_portal_preview/preview-20260218-summary.json`), dayjestni (`preview-20260218-digest.md`) yaratadi va `docs/portal/src/data/previewFeedbackSummary.json`ni yangilaydi, shuning uchun boshqaruvni bitta ko‘rib chiquvchiga ishonib topshiradi.
2. **Telemetriya + boshqaruv qamrovi.** To'rtta ko'rib chiquvchining hammasi nazorat summasi bilan himoyalangan kirishni tan oldi, fikr-mulohazalarini yubordi va o'z vaqtida bekor qilindi; Dayjestda toʻlqin davomida toʻplangan Grafana yugurishlari bilan bir qatorda fikr-mulohaza masalalari (`docs-preview/20260218` toʻplami + `DOCS-SORA-Preview-20260218`) haqida soʻz boradi.
3. **Portal sirtini ko'rsatish.** Yangilangan portal jadvali endi kechikish va javob tezligi ko'rsatkichlari bilan yopiq W3 to'lqinini ko'rsatadi va quyida joylashgan yangi jurnal sahifasi xom JSON jurnalini tortmaydigan auditorlar uchun voqealar vaqt jadvalini aks ettiradi.

## Harakat elementlari

| ID | Tavsif | Egasi | Holati |
| --- | --- | --- | --- |
| W3-A1 | Oldindan ko'rish dayjestini yozib oling va trekerga biriktiring. | Docs/DevRel yetakchisi | ✅ Tugallangan 2026‑02‑28 |
| W3-A2 | Dalillarni portalga + yoʻl xaritasiga/statusga aks ettirish/taklif qilish. | Docs/DevRel yetakchisi | ✅ Tugallangan 2026‑02‑28 |

## Chiqish xulosasi (28-02-2026)

- 2026-02-18 da yuborilgan taklifnomalar bir necha daqiqadan so'ng qayd etilgan; Yakuniy telemetriya tekshiruvidan o‘tgandan so‘ng 2026-02-28-da oldindan ko‘rishga kirish bekor qilindi.
- Digest + xulosasi `artifacts/docs_portal_preview/` ostida olingan, xom jurnal qayta o'qilishi uchun `artifacts/docs_portal_preview/feedback_log.json` tomonidan biriktirilgan.
- `DOCS-SORA-Preview-20260218` boshqaruv kuzatuvchisi bilan `docs-preview/20260218` bo'yicha topshirilgan muammoni kuzatish; CSP/Try it qaydlari kuzatuv/moliya egalariga yo'naltirilgan va dayjestdan bog'langan.
- Kuzatuvchi qatori 🈴 ga yangilandi va portalning fikr-mulohazalari jadvali DOCS-SORA beta-tayyorligining qolgan vazifasini bajarib, yopiq to‘lqinni aks ettiradi.