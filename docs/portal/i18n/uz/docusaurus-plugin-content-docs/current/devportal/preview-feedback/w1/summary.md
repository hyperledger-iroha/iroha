---
id: preview-feedback-w1-summary
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W1 — Hamkorlar va Torii integratorlari |
| Taklif oynasi | 2025‑04‑12 → 2025‑04‑26 |
| Artefakt tegi | `preview-2025-04-12` |
| Kuzatuvchi muammosi | `DOCS-SORA-Preview-W1` |
| Ishtirokchilar | sorafs-op-01…03, torii-int-01…02, sdk-partner-01…02, shlyuz-ops-01 |

## E'tiborga molik

1. **Tekshiruv ish jarayoni** — Barcha sharhlovchilar `scripts/preview_verify.sh` orqali tavsiflovchi/arxivni tasdiqladilar; taklifnomalar bilan birga saqlanadigan jurnallar.
2. **Telemetriya** — `docs.preview.integrity`, `TryItProxyErrors` va `DocsPortal/GatewayRefusals` asboblar paneli butun toʻlqin davomida yashil rangda qoldi; hech qanday hodisalar yoki ogohlantirish sahifalari ishdan chiqmadi.
3. **Hujjatning fikr-mulohazasi (`docs-preview/w1`)** — Ikkita kichik nit topshirildi:
   - `docs-preview/w1 #1`: Sinab ko'ring (hal qilingan) bo'limida nav so'zlarini aniqlang.
   - `docs-preview/w1 #2`: yangilash Skrinshotni sinab ko'ring (hal qilingan).
4. **Runbook pariteti** — SoraFS operatorlari `orchestrator-ops` va `multi-source-rollout` o'rtasidagi yangi o'zaro bog'lanishlarni tasdiqladilar, ularning W0 muammolarini hal qilishdi.

## Harakat elementlari

| ID | Tavsif | Egasi | Holati |
| --- | --- | --- | --- |
| W1-A1 | Yangilash `docs-preview/w1 #1` bo'yicha navbatma-navbat so'zlab ko'ring. | Docs-core-02 | ✅ Tugallangan (2025-04-18). |
| W1-A2 | Yangilash `docs-preview/w1 #2` uchun skrinshotni sinab ko'ring. | Docs-core-03 | ✅ Tugallangan (2025-04-19). |
| W1-A3 | Yo'l xaritasi/statusidagi hamkorlar natijalarini + telemetriya dalillarini umumlashtiring. | Docs/DevRel yetakchisi | ✅ Tugallandi (qarang: treker + status.md). |

## Chiqish xulosasi (26-04-2025)

- Barcha sakkizta sharhlovchi oxirgi ish soatlarida yakunlanganligini tasdiqladilar, mahalliy artefaktlarni tozalashdi va ularga kirish huquqini bekor qilishdi.
- Telemetriya chiqish orqali yashil rangda qoldi; yakuniy suratlar `DOCS-SORA-Preview-W1` ga biriktirilgan.
- Chiqish tasdiqnomalari bilan yangilangan taklif jurnali; treker W1 ni 🈴 ga aylantirdi va nazorat nuqtasi yozuvlarini qo'shdi.
- `artifacts/docs_preview/W1/` ostida arxivlangan dalillar to'plami (deskriptor, nazorat summasi jurnali, tekshiruv chiqishi, sinab ko'ring proksi transkripti, telemetriya skrinshotlari, fikr-mulohazalar dayjesti).

## Keyingi qadamlar

- W2 hamjamiyatini qabul qilish rejasini tayyorlang (boshqaruvni tasdiqlash + so'rov shablonini sozlash).
- W2 to'lqini uchun oldindan ko'rish artefakt tegini yangilang va sanalar tugallangandan so'ng oldindan parvoz skriptini qayta ishga tushiring.
- Qo'llaniladigan W1 topilmalarini yo'l xaritasi/statusiga kiriting, shunda hamjamiyat to'lqini eng so'nggi ko'rsatmalarga ega bo'ladi.