---
lang: uz
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-12-29T18:16:35.076964+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
# SNS arbitrajning shaffofligi hisoboti — <YYYYY oy>

- **Suffiks:** `<.sora / .nexus / .dao>`
- **Hisobot oynasi:** `<ISO start>` → `<ISO end>`
- **Tayyorlovchi:** `<Council liaison>`
- **Manba artefaktlari:** `cases.ndjson` SHA256 `<hash>`, asboblar paneli eksporti `<filename>.json`

## 1. Xulosa

- Jami yangi holatlar: `<count>`
- Ushbu davrni yopib qo'ygan holatlar: `<count>`
- SLA muvofiqligi: `<ack %>` tan olish / `<resolution %>` qarori
- Himoyachi bekor qilindi: `<count>`
- O'tkazmalar/to'lovlar amalga oshirildi: `<count>`

## 2. Case Mix

| Bahs turi | Yangi holatlar | Yopiq holatlar | O'rtacha ruxsat (kun) |
|-------------|-----------|--------------|--------------------------|
| Egalik | 0 | 0 | 0 |
| Siyosat buzilishi | 0 | 0 | 0 |
| Suiiste'mol | 0 | 0 | 0 |
| Billing | 0 | 0 | 0 |
| Boshqa | 0 | 0 | 0 |

## 3. SLA ishlashi

| Ustuvorlik | SLAni tan olish | erishildi | Rezolyutsiya SLA | erishildi | Buzilishlar |
|----------|-----------------|----------|----------------|----------|----------|
| Shoshilinch | ≤ 2 soat | 0% | ≤ 72 soat | 0% | 0 |
| Yuqori | ≤ 8 soat | 0% | ≤ 10d | 0% | 0 |
| Standart | ≤ 24 soat | 0% | ≤ 21d | 0% | 0 |
| Ma'lumot | ≤ 3D | 0% | ≤ 30d | 0% | 0 |

Har qanday buzilishlarning asosiy sabablarini tavsiflang va tuzatish chiptalariga havola qiling.

## 4. Ishlar reestri

| Ish ID | Selektor | Ustuvorlik | Holati | Natija | Eslatmalar |
|---------|----------|----------|--------|---------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standart | Yopiq | Qo'llab-quvvatlandi | `<summary>` |

Anonim faktlar yoki ommaviy ovoz berish havolalari uchun bir qatorli eslatmalarni taqdim eting. Muhr
zarur bo'lganda va qo'llaniladigan tahrirlarni eslatib o'ting.

## 5. Harakatlar va muolajalar

- **Muzlatadi / chiqaradi:** `<counts + case ids>`
- **O'tkazmalar:** `<counts + assets moved>`
- **Hisob-kitob tuzatishlari:** `<credits/debits>`
- **Siyosatni kuzatish:** `<tickets or RFCs opened>`

## 6. Murojaatlar va vasiyning bekor qilishlari

Vasiylik kengashiga etkazilgan har qanday murojaatlarni, shu jumladan vaqt belgilarini va
qarorlar (tasdiqlash / rad etish). `sns governance appeal` yozuvlari yoki kengashiga havola
ovozlar.

## 7. Ajoyib buyumlar

- `<Action item>` — Egasi `<name>`, ETA `<date>`
- `<Action item>` — Egasi `<name>`, ETA `<date>`

Ushbu hisobotda keltirilgan NDJSON, Grafana eksportlari va CLI jurnallarini biriktiring.