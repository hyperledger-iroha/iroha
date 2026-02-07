---
id: repair-plan
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs_repair_plan.md`. Sphinx to'plami tugagunga qadar ikkala versiyani ham sinxronlashtiring.
:::

## Boshqaruv qarorlarining hayot aylanishi
1. Ko'tarilgan ta'mirlash slash taklif loyihasini yaratadi va bahs oynasini ochadi.
2. Boshqaruv saylovchilari nizolar oynasida ma'qullash/rad etish ovozlarini beradilar.
3. `escalated_at_unix + dispute_window_secs` da qaror deterministik tarzda hisoblanadi: minimal saylovchilar, ma'qullashlar rad etishlardan oshib ketadi va ma'qullash nisbati kvorum chegarasiga javob beradi.
4. Tasdiqlangan qarorlar apellyatsiya oynasini ochadi; `approved_at_unix + appeal_window_secs` dan oldin qayd etilgan shikoyatlar qarorni shikoyat qilingan deb belgilaydi.
5. Barcha takliflar uchun jarima miqdori qo'llaniladi; chegaradan yuqori bo'lgan arizalar rad etiladi.

## Boshqaruvni kuchaytirish siyosati
Eskalatsiya siyosati `iroha_config` da `governance.sorafs_repair_escalation` dan olingan va har bir tuzatish taklifi uchun qo'llaniladi.

| Sozlama | Standart | Ma'nosi |
|---------|---------|---------|
| `quorum_bps` | 6667 | Hisoblangan ovozlar orasida minimal ma'qullash nisbati (asosiy ball). |
| `minimum_voters` | 3 | Qarorni hal qilish uchun zarur bo'lgan alohida saylovchilarning minimal soni. |
| `dispute_window_secs` | 86400 | Ovoz berish yakunlangunga qadar kuchaygan vaqt (sekundlar). |
| `appeal_window_secs` | 604800 | Tasdiqlangandan keyin murojaatlar qabul qilinadigan vaqt (sekundlar). |
| `max_penalty_nano` | 1 000 000 000 | Ta'mirlash eskalatsiyasi uchun ruxsat etilgan maksimal slash jazosi (nano-XOR). |

- Rejalashtiruvchi tomonidan yaratilgan takliflar `max_penalty_nano` bilan cheklangan; chegaradan yuqori auditorlik takliflari rad etiladi.
- Ovoz berish yozuvlari `repair_state.to` da deterministik tartib bilan (`voter_id` saralash) saqlanadi, shuning uchun barcha tugunlar bir xil qaror vaqt tamg'asi va natijasini oladi.