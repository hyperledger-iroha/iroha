---
lang: uz
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ca51ec624ebbb4b3760d5f2265d31047cd3b6492e21bdb10a3aa61655ccca69
source_last_modified: "2025-12-29T18:16:35.069181+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Partner SLA Discovery Notes — Andoza

Har bir AND8 SLA kashfiyot seansi uchun ushbu shablondan foydalaning. To'ldirilgan nusxani saqlang
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` ostida
va qo'llab-quvvatlovchi artefaktlarni ilova qiling (so'rovga javoblar, minnatdorchilik,
qo'shimchalar) xuddi shu katalogda.

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. Kun tartibi va kontekst

- Sessiyaning maqsadi (uchuvchi doirasi, relizlar oynasi, telemetriyani kutish).
- Qo'ng'iroqdan oldin ulashilgan ma'lumotnoma hujjatlari (qo'llab-quvvatlash kitobi, nashr taqvimi,
  telemetriya asboblar paneli).

## 2. Ish yukining umumiy ko'rinishi

| Mavzu | Eslatmalar |
|-------|-------|
| Maqsadli ish yuklari / zanjirlar | |
| Kutilayotgan tranzaksiya hajmi | |
| Muhim biznes oynalari / o'chirish davrlari | |
| Tartibga solish rejimlari (GDPR, MAS, FISC va boshqalar) | |
| Kerakli tillar / mahalliylashtirish | |

## 3. SLA muhokamasi

| SLA klassi | Hamkor kutish | Dastlabki delta? | Harakat talab qilinadi |
|----------|--------------------|----------------------|-----------------|
| Kritik tuzatish (48 soat) | | Ha/Yo'q | |
| Yuqori jiddiylik (5 ish kuni) | | Ha/Yo'q | |
| Xizmat (30 kun) | | Ha/Yo'q | |
| Kesish xabari (60 kun) | | Ha/Yo'q | |
| Voqea sodir bo'lgan aloqa tezligi | | Ha/Yo'q | |

Hamkor soʻragan har qanday qoʻshimcha SLA bandlarini hujjatlashtiring (masalan, bagʻishlangan
telefon ko'prigi, qo'shimcha telemetriya eksporti).

## 4. Telemetriya va kirish talablari

- Grafana / Prometheus kirish talablari:
- Jurnal/track eksport talablari:
- Oflayn dalillar yoki ma'lumotlarning taxminlari:

## 5. Muvofiqlik va huquqiy eslatmalar

- Yurisdiksiya bildirishnomalariga qo'yiladigan talablar (nizom + vaqt).
- Voqealarni yangilash uchun zarur bo'lgan yuridik kontaktlar.
- Ma'lumotlar rezidentligi cheklovlari / saqlash talablari.

## 6. Qarorlar va harakatlar bandlari

| Element | Egasi | Muddati | Eslatmalar |
|------|-------|-----|-------|
| | | | |

## 7. Tasdiqlash

- Hamkor asosiy SLAni tan oldimi? (Y/N)
- Kuzatuvni tasdiqlash usuli (elektron pochta / chipta / imzo):
- Yopishdan oldin ushbu katalogga tasdiqlovchi elektron pochta yoki yig'ilish bayonnomalarini ilova qiling.