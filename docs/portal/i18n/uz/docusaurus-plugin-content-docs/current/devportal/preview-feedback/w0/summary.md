---
id: preview-feedback-w0-summary
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W0 midpoint feedback digest
sidebar_label: W0 feedback (midpoint)
description: Midpoint checkpoints, findings, and action items for the core-maintainer preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Element | Tafsilotlar |
| --- | --- |
| To'lqin | W0 - Asosiy saqlovchilar |
| Dijest sanasi | 2025-03-27 |
| Ko'rib chiqish oynasi | 2025‑03‑25 → 2025‑04‑08 |
| Ishtirokchilar | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Artefakt tegi | `preview-2025-03-24` |

## E'tiborga molik

1. **Tekshiruv ish jarayoni** — Barcha sharhlovchilar `scripts/preview_verify.sh` ni tasdiqladilar
   umumiy deskriptor/arxiv juftligiga qarshi muvaffaqiyatli boʻldi. Qo'lda bekor qilish yo'q
   talab qilinadi.
2. **Navigatsiya bo‘yicha fikr-mulohazalar** — Yon panelda buyurtma berish bo‘yicha ikkita kichik muammo yuzaga keldi
   (`docs-preview/w0 #1–#2`). Ikkalasi ham Docs/DevRel-ga yo'naltiriladi va ularni bloklamaydi
   to'lqin.
3. **SoraFS runbook parity** — sorafs-ops-01 aniqroq oʻzaro bogʻlanishlarni soʻradi
   `sorafs/orchestrator-ops` va `sorafs/multi-source-rollout` oralig'ida. Kuzatuv
   topshirilgan masala; W1 dan oldin murojaat qilish kerak.
4. **Telemetriya tekshiruvi** — kuzatuvchanlik-01 tasdiqlandi `docs.preview.integrity`,
   `TryItProxyErrors` va "Try it" proksi jurnallari yashil bo'lib qoldi; hech qanday ogohlantirish berilmagan.

## Harakat elementlari

| ID | Tavsif | Egasi | Holati |
| --- | --- | --- | --- |
| W0-A1 | Devportal yon paneldagi yozuvlarni qayta koʻrib chiquvchiga yoʻnaltirilgan hujjatlarga oʻzgartiring (`preview-invite-*` guruhi birgalikda). | Docs-core-01 | ✅ Bajarildi — yon panelda endi ko'rib chiquvchi hujjatlari ketma-ket ro'yxatga olinadi (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` va `sorafs/multi-source-rollout` o'rtasida aniq o'zaro bog'lanish qo'shing. | Sorafs-ops-01 | ✅ Bajarildi — endi har bir runbook boshqasiga ulanadi, shuning uchun operatorlar ishga tushirish vaqtida ikkala qoʻllanmani ham koʻradi. |
| W0-A3 | Telemetriya suratlarini + soʻrovlar toʻplamini boshqaruv kuzatuvchisi bilan baham koʻring. | Kuzatish qobiliyati-01 | ✅ Bajarildi — toʻplam `DOCS-SORA-Preview-W0` ga biriktirilgan. |

## Chiqish xulosasi (2025-04-08)

- Beshta ko'rib chiquvchining hammasi yakunlanganligini tasdiqladilar, mahalliy qurilishlarni tozalashdi va undan chiqishdi
  oldindan ko'rish oynasi; `DOCS-SORA-Preview-W0` da qayd etilgan kirishni bekor qilish.
- To'lqin paytida hech qanday hodisa va ogohlantirishlar yo'q; telemetriya asboblar paneli qoldi
  butun davr uchun yashil.
- Navigatsiya + o'zaro bog'liqlik harakatlari (W0-A1/A2) amalga oshiriladi va aks ettiriladi
  yuqoridagi hujjatlar; telemetriya dalillari (W0-A3) trekerga biriktirilgan.
- Arxivlangan dalillar to'plami: telemetriya skrinshotlari, taklifning minnatdorchiligi va
  bu dayjest treker muammosi bilan bog'langan.

## Keyingi qadamlar

- W1-ni ochishdan oldin W0 amallarini bajaring.
- Yuridik ma'qullash va proksi-serverni joylashtirish uyasiga ega bo'ling, keyin sherik-to'lqinga amal qiling
  [taklif oqimini oldindan koʻrish](../../preview-invite-flow.md) da koʻrsatilgan parvozdan oldingi bosqichlar.

_Ushbu dayjest [oldindan ko'rish taklifnomasi](../../preview-invite-tracker.md) orqali bog'langan
DOCS-SORA yoʻl xaritasini kuzatish mumkin boʻlsin._