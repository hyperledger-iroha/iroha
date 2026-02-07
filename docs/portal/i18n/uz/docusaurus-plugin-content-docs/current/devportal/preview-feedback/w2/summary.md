---
id: preview-feedback-w2-summary
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community feedback & status
sidebar_label: W2 summary
description: Live digest for the community preview wave (W2).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W2 — Hamjamiyat sharhlovchilari |
| Taklif oynasi | 2025‑06‑15 → 2025‑06‑29 |
| Artefakt tegi | `preview-2025-06-15` |
| Kuzatuvchi muammosi | `DOCS-SORA-Preview-W2` |
| Ishtirokchilar | comm-vol-01 … comm-vol-08 |

## E'tiborga molik

1. **Boshqaruv va vositalar** — Hamjamiyat tomonidan qabul qilish siyosati 2025-05-20 da bir ovozdan tasdiqlangan; motivatsiya/vaqt mintaqasi maydonlari bilan yangilangan so'rov shablonlari `docs/examples/docs_preview_request_template.md` ostida ishlaydi.
2. **Parvozdan oldingi dalillar** — Proksi-serverni oʻzgartirishni sinab koʻring `OPS-TRYIT-188` 2025‑06‑09 ishga tushirildi, Grafana asboblar paneli yozib olindi va `preview-2025-06-15` deskriptor/tekshirish summasi/zond chiqishlari arxivi arxivlandi.
3. **Invite to'lqini** — 2025-06-15 da taklif qilingan sakkiz nafar hamjamiyat sharhlovchisi taklifnomalar jadvalida qayd etilgan; ko'rishdan oldin barcha tugallangan nazorat summasini tekshirish.
4. **Fikr-mulohaza** — `docs-preview/w2 #1` (maslahat matni) va `#2` (mahalliylashtirish yon paneli tartibi) 2025-06-18 da topshirilgan va 2025-06-21 (Docs-504) tomonidan hal qilingan; to'lqin paytida hech qanday hodisalar sodir bo'lmadi.

## Harakat elementlari

| ID | Tavsif | Egasi | Holati |
| --- | --- | --- | --- |
| W2-A1 | Manzil `docs-preview/w2 #1` (ko'rsatma matni). | Docs-core-04 | ✅ Tugallangan 2025‑06‑21 |
| W2-A2 | Manzil `docs-preview/w2 #2` (mahalliylashtirish yon paneli). | Docs-core-05 | ✅ Tugallangan 2025‑06‑21 |
| W2-A3 | Arxivdan chiqish dalillari + yoʻl xaritasi/holatini yangilash. | Docs/DevRel yetakchisi | ✅ Tugallangan 2025‑06‑29 |

## Chiqish xulosasi (29-06-2025)

- Barcha sakkizta hamjamiyat ko'rib chiquvchilari yakunlanganligini tasdiqladilar va oldindan ko'rish huquqi bekor qilindi; kuzatuvchi takliflar jurnalida qayd etilgan tasdiqlar.
- Yakuniy telemetriya suratlari (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) yashil rangda qoldi; logs plus `DOCS-SORA-Preview-W2` ga biriktirilgan proksi transkriptlarini sinab ko'ring.
- `artifacts/docs_preview/W2/preview-2025-06-15/` ostida arxivlangan dalillar to'plami (deskriptor, nazorat summasi jurnali, tekshirish chiqishi, havola hisoboti, Grafana skrinshotlari, taklifni tasdiqlash).
- Tracker W2 nazorat punkti jurnali chiqish orqali yangilanadi, bu W3 rejalashtirish boshlanishidan oldin yo'l xaritasi tekshiriladigan yozuvni saqlab turishini ta'minlaydi.