---
id: preview-invite-flow
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Maqsad

“Yo‘l xaritasi” bandi **DOCS-SORA** sharhlovchini ishga tushirish va ommaviy oldindan ko‘rishga chaqiradi
portal beta versiyasidan chiqishidan oldin dasturni oxirgi blokerlar sifatida taklif qiling. Bu sahifa
Har bir taklif to'lqinini qanday ochishni tasvirlaydi, qaysi artefaktlar oldin jo'natilishi kerak
takliflar tashqariga chiqadi va oqimning tekshirilishi mumkinligini qanday isbotlash mumkin. Undan foydalaning:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) uchun
  har bir sharhlovchiga ishlov berish.
- nazorat summasi uchun [`devportal/preview-integrity-plan`](./preview-integrity-plan.md)
  kafolatlar.
- [`devportal/observability`](./observability.md) telemetriya eksporti va
  ogohlantiruvchi ilgaklar.

## To'lqin rejasi

| To'lqin | Tomoshabinlar | Kirish mezonlari | Chiqish mezonlari | Eslatmalar |
| --- | --- | --- | --- | --- |
| **W0 – Asosiy saqlovchilar** | Docs/SDK serverlari birinchi kunlik kontentni tasdiqlaydi. | `docs-portal-preview` GitHub jamoasi toʻplangan, `npm run serve` nazorat summasi eshigi yashil, Alertmanager 7 kun davomida jim. | Barcha P0 hujjatlari ko'rib chiqildi, orqada qoldirilgan, blokirovka qilingan hodisalar yo'q. | Oqimni tekshirish uchun ishlatiladi; taklif e-pochtasi yo'q, faqat oldindan ko'rish artefaktlarini baham ko'ring. |
| **W1 – Hamkorlar** | SoraFS operatorlari, Torii integratorlari, NDA bo'yicha boshqaruv sharhlovchilari. | W0 chiqdi, qonuniy shartlar ma'qullandi, Proksi-serverdan sinab ko'ring. | Hamkorning to‘plangan ro‘yxati (muammo yoki imzolangan shakl), telemetriya ≤10 bir vaqtda tekshiruvchini ko‘rsatadi, 14 kun davomida hech qanday xavfsizlik regressiyasi yo‘q. | Taklif shablonini qo'llash + chiptalarni so'rash. |
| **W2 – Hamjamiyat** | Hamjamiyat kutish roʻyxatidan tanlangan ishtirokchilar. | W1 dan chiqdi, hodisa mashqlari takrorlandi, ommaviy savollar yangilandi. | Fikr-mulohazalar hazm qilingan, ≥2 hujjat relizlari oldindan koʻrish quvuri orqali qaytarib yuborilmasdan joʻnatilgan. | Bir vaqtning o'zida takliflarni cheklash (≤25) va har hafta to'plam. |

`status.md` ichida va oldindan ko'rish so'rovida qaysi to'lqin faol ekanligini hujjat
treker, shuning uchun boshqaruv bir qarashda dastur qayerda joylashganini ko'rishi mumkin.

## Parvozdan oldin nazorat ro'yxati

To'lqin uchun takliflarni rejalashtirishdan **oldin** ushbu amallarni bajaring:

1. **CI artefaktlari mavjud**
   - Oxirgi `docs-portal-preview` + deskriptor tomonidan yuklangan
     `.github/workflows/docs-portal-preview.yml`.
   - SoraFS pin `docs/portal/docs/devportal/deploy-guide.md` da qayd etilgan
     (kesish tavsifi mavjud).
2. **Tekshiruv summasining bajarilishi**
   - `docs/portal/scripts/serve-verified-preview.mjs` orqali chaqirildi
     `npm run serve`.
   - `scripts/preview_verify.sh` ko'rsatmalari macOS + Linux da sinovdan o'tkazildi.
3. **Telemetriya bazasi**
   - `dashboards/grafana/docs_portal.json` sog'lom ko'rsatadi Trafik va
     `docs.preview.integrity` ogohlantirish yashil rangda.
   - Oxirgi `docs/portal/docs/devportal/observability.md` ilovasi bilan yangilangan
     Grafana havolalari.
4. **Boshqaruv artefaktlari**
   - Taklif trekerining soni tayyor (har bir to'lqin uchun bitta masala).
   - Ko'rib chiquvchining reestri shablonidan ko'chirildi (qarang
     [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Huquqiy va SRE tomonidan talab qilinadigan tasdiqlar masalaga ilova qilinadi.

Har qanday xat yuborishdan oldin taklif kuzatuvchisida parvozdan oldin tugallanganligini yozib oling.

## Oqim qadamlari

1. **Nomzodlarni tanlang**
   - Kutish roʻyxati elektron jadvalidan yoki hamkorlar qatoridan torting.
   - Har bir nomzodda to'ldirilgan so'rov shabloniga ega ekanligiga ishonch hosil qiling.
2. **Kirishni tasdiqlash**
   - Taklif kuzatuvchisi muammosiga tasdiqlovchini tayinlang.
   - Old shartlarni tekshiring (CLA/shartnoma, maqbul foydalanish, xavfsizlik bo'yicha qisqacha).
3. **Takliflarni yuborish**
   - To'ldiring
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     to'ldiruvchilar (`<preview_tag>`, `<request_ticket>`, kontaktlar).
   - Deskriptor + arxiv xeshini biriktiring, URL manzilini ko'ring va qo'llab-quvvatlang
     kanallar.
   - Yakuniy elektron pochtani (yoki Matrix/Slack transkriptini) nashrda saqlang.
4. **Ishga kirishni kuzatish**
   - Taklif kuzatuvchisini `invite_sent_at`, `expected_exit_at` bilan yangilang va
     holati (`pending`, `active`, `complete`, `revoked`).
   - Tekshiruvchining auditorlik uchun qabul qilish so'roviga havola.
5. **Monitor telemetriya**
   - `docs.preview.session_active` va `TryItProxyErrors` ogohlantirishlarini tomosha qiling.
   - Agar telemetriya asosiy chiziqdan chetga chiqsa, voqeani yozing va yozib oling
     taklif yozuvi yonidagi natija.
6. **Fikr-mulohazalarni yig‘ish va chiqish**
   - Fikr bildirilganda yoki `expected_exit_at` o'tgandan so'ng takliflarni yoping.
   - To'lqin muammosini qisqacha xulosa bilan yangilang (topilmalar, hodisalar, keyingi
     harakatlar) keyingi kogortaga o'tishdan oldin.

## Dalil va hisobot

| Artefakt | Qayerda saqlash kerak | Kadansni yangilash |
| --- | --- | --- |
| Taklif kuzatuvchisi muammosi | `docs-portal-preview` GitHub loyihasi | Har bir taklifdan keyin yangilang. |
| Sharhlovchilar ro'yxatini eksport qilish | `docs/portal/docs/devportal/reviewer-onboarding.md` bog'langan registr | Haftalik. |
| Telemetriya suratlari | `docs/source/sdk/android/readiness/dashboards/<date>/` (telemetriya to'plamini qayta ishlatish) | Har bir to'lqin + hodisalardan keyin. |
| Fikrlar dayjesti | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (har bir to'lqin uchun papka yaratish) | Toʻlqin chiqqandan keyin 5 kun ichida. |
| Boshqaruv majlisi bayonnomasi | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Har bir DOCS-SORA boshqaruv sinxronlashidan oldin toʻldiring. |

`cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json` ni ishga tushiring
Har bir partiyadan keyin mashinada o'qiladigan voqea dayjestini ishlab chiqarish. Tasvirlanganni biriktiring
JSON to'lqin muammosiga, shuning uchun boshqaruvni ko'rib chiquvchilar takliflar sonini tasdiqlamasdan turib tasdiqlashlari mumkin
butun jurnalni takrorlash.

Dalillar ro'yxatini `status.md` ga to'lqin tugashi uchun ilova qiling, shunda yo'l xaritasi
kirish tezda yangilanishi mumkin.

## Orqaga qaytarish va pauza mezonlari

Quyidagilardan biri sodir bo'lganda taklif oqimini to'xtatib turing (va boshqaruvga xabar bering):

- Orqaga qaytarishni talab qiluvchi proksi-serverni sinab ko'ring (`npm run manage:tryit-proxy`).
- Ogohlantirish charchoq: 7 kun ichida faqat oldindan koʻrish mumkin boʻlgan soʻnggi nuqtalar uchun >3 ta ogohlantirish sahifasi.
- Muvofiqlikdagi bo'shliq: imzolangan shartlarsiz yoki ro'yxatdan o'tmasdan yuborilgan taklif
  so'rov shabloni.
- Butunlik xavfi: nazorat summasining mos kelmasligi `scripts/preview_verify.sh` tomonidan aniqlandi.

Taklif trekerida tuzatishni hujjatlashtirgandan keyingina davom eting va
telemetriya asboblar panelining kamida 48 soat barqarorligini tasdiqlash.