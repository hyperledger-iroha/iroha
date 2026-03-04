---
id: public-preview-invite
lang: uz
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Dastur maqsadlari

Ushbu o'yin kitobida bir marta ommaviy oldindan ko'rishni qanday e'lon qilish va ishga tushirish kerakligi tushuntiriladi
sharhlovchini ishga tushirish ish jarayoni jonli. U DOCS-SORA yo'l xaritasini halol saqlaydi
har bir taklif kemalarini tekshirilishi mumkin bo'lgan artefaktlar, xavfsizlik bo'yicha ko'rsatmalar va a
aniq fikr-mulohaza yo'li.

- **Tomoshabinlar:** hamjamiyat a'zolari, hamkorlar va ta'minotchilarning tanlangan ro'yxati
  oldindan koʻrish mumkin boʻlgan foydalanish siyosatini imzoladi.
- ** Shiftlar:** standart to'lqin hajmi ≤ 25 sharhlovchi, 14 kunlik kirish oynasi, hodisa
  24 soat ichida javob.

## Gating nazorat ro'yxatini ishga tushiring

Har qanday taklifnoma yuborishdan oldin ushbu vazifalarni bajaring:

1. CI (`docs-portal-preview`,
   nazorat summasi manifest, deskriptor, SoraFS to'plami).
2. `npm run --prefix docs/portal serve` (checksum-gated) bir xil tegda sinovdan o'tkazildi.
3. Ko'rib chiquvchi bortga chiqish chiptalari tasdiqlangan va taklif to'lqiniga bog'langan.
4. Xavfsizlik, kuzatuvchanlik va hodisa hujjatlari tasdiqlangan
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Fikr-mulohaza shakli yoki nashr shablonini tayyorlang (jiddiylik uchun maydonlarni kiriting,
   ko'paytirish bosqichlari, skrinshotlar va atrof-muhit haqida ma'lumot).
6. Docs/DevRel + Governance tomonidan ko'rib chiqilgan e'lon nusxasi.

## Taklif paketi

Har bir taklif quyidagilarni o'z ichiga olishi kerak:

1. **Tasdiqlangan artefaktlar** — SoraFS manifest/reja yoki GitHub artefaktini taqdim eting
   havolalar va nazorat summasi manifest va deskriptor. Tekshirishga havola
   Ko'rib chiquvchilar saytni ishga tushirishdan oldin uni ishga tushirishlari uchun aniq buyruq bering.
2. **Xizmat ko‘rsatmalari** — nazorat summasi bo‘lgan oldindan ko‘rish buyrug‘ini qo‘shing:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Xavfsizlik eslatmalari** — Tokenlar avtomatik ravishda tugashini chaqiring, havolalar
   baham ko'rilmasligi kerak va hodisalar haqida darhol xabar qilinishi kerak.
4. **Teskari aloqa kanali** — Muammo shabloniga/shakliga havola va javobni aniqlang
   vaqt kutishlari.
5. **Dastur sanalari** — Boshlanish/tugash sanalari, ish soatlari yoki uchrashuvlarni sinxronlash,
   va keyingi yangilash oynasi.

Elektron pochta namunasi
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
ushbu talablarni qamrab oladi. To'ldiruvchilarni yangilang (sanalar, URL manzillar, kontaktlar)
yuborishdan oldin.

## Ko'rib chiqish xostini oching

Oldindan ko‘rish xostini faqat ishga tushirish tugallangandan va chiptani o‘zgartirgandan so‘ng reklama qiling
tasdiqlangan. [Host taʼsir qilish qoʻllanmasini oldindan koʻrish](./preview-host-exposure.md)
ushbu bo'limda qo'llaniladigan yakuniy qurish/nashr qilish/tekshirish bosqichlari uchun.

1. **Yaratish va qadoqlash:** Chiqarish yorlig'ini muhrlang va deterministik ishlab chiqing
   artefaktlar.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   Pin skripti `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   va `portal.dns-cutover.json` `artifacts/sorafs/` ostida. Ushbu fayllarni ilovaga biriktiring
   har bir sharhlovchi bir xil bitlarni tekshirishi uchun taklif to'lqini.

2. **Oldindan ko‘rish taxallusni nashr qilish:** `--skip-submit`siz buyruqni qayta ishga tushiring
   (ta'minot `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` va
   boshqaruv tomonidan berilgan taxallusni tasdiqlovchi hujjat). Skript manifestni bog'laydi
   `docs-preview.sora` va `portal.manifest.submit.summary.json` plyus chiqaradi
   Dalillar to'plami uchun `portal.pin.report.json`.

3. **O'rnatishni tekshirib ko'ring:** Taxallus hal qilinganligini va nazorat summasi mos kelishini tasdiqlang
   taklif yuborishdan oldin teg.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) ni qulay joyda saqlang
   Agar oldindan ko'rish chekkasi o'chib qolsa, sharhlovchilar mahalliy nusxani aylantirishi mumkin.

## Aloqa xronologiyasi

| Kun | Harakat | Egasi |
| --- | --- | --- |
| D-3 | Taklif nusxasini yakunlash, artefaktlarni yangilash, quruq ishga tushirish | Docs/DevRel |
| D-2 | Boshqaruvni imzolash + chiptani o'zgartirish | Docs/DevRel + Governance |
| D-1 | Shablon yordamida takliflarni yuboring, qabul qiluvchilar ro'yxati bilan trekerni yangilang | Docs/DevRel |
| D | Boshlanish qo'ng'irog'i / ish vaqti, telemetriya asboblar panelini kuzatib boring | Docs/DevRel + On-Call |
| D+7 | O'rta nuqtadagi fikr-mulohazalar dayjesti, triajni blokirovka qilish muammolari | Docs/DevRel |
| D+14 | To'lqinni yoping, vaqtinchalik ruxsatni bekor qiling, xulosani `status.md` da nashr qiling | Docs/DevRel |

## Kirish kuzatuvi va telemetriya

1. Har bir qabul qiluvchini, taklif vaqt tamg'asini va bekor qilingan sanani yozib oling
   fikr-mulohazalarni ro'yxatga oluvchini oldindan ko'rish (qarang
   [`preview-feedback-log`](./preview-feedback-log)) shuning uchun har bir to'lqin
   bir xil dalillar izi:

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Qo'llab-quvvatlanadigan tadbirlar: `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` va `access-revoked`. Jurnal yashaydi
   Sukut bo'yicha `artifacts/docs_portal_preview/feedback_log.json`; unga biriktiring
   taklif to'lqin chiptasi rozilik shakllari bilan birga. Xulosa yordamchisidan foydalaning
   yopilish eslatmasidan oldin tekshirilishi mumkin bo'lgan to'plamni yaratish uchun:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON xulosasi har bir to'lqin uchun takliflarni, ochiq qabul qiluvchilarni, fikr-mulohazalarni sanab o'tadi
   hisoblar va oxirgi voqeaning vaqt tamg'asi. Yordamchi tomonidan qo'llab-quvvatlanadi
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   shuning uchun bir xil ish jarayoni mahalliy yoki CIda ishlashi mumkin. Dijest shablonidan foydalaning
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   to'lqin recap nashr qachon.
2. Telemetriya asboblar panelini to'lqin uchun ishlatiladigan `DOCS_RELEASE_TAG` bilan belgilang, shuning uchun
   tikanlar taklif kogortalari bilan bog'lanishi mumkin.
3. Joylashtirishdan keyin `npm run probe:portal -- --expect-release=<tag>` ni ishga tushiring
   oldindan ko'rish muhiti to'g'ri reliz metama'lumotlarini reklama qilishini tasdiqlang.
4. Runbook shablonidagi barcha hodisalarni yozib oling va ularni kogortaga bog'lang.

## Fikr-mulohaza va yopish

1. Umumiy hujjat yoki nashrlar panelida fikr-mulohazalarni jamlang. Elementlarni belgilang
   `docs-preview/<wave>`, shuning uchun yo'l xaritasi egalari ularni osongina so'rashlari mumkin.
2. To'lqin hisobotini to'ldirish uchun oldindan ko'rish jurnali xulosasidan foydalaning, keyin
   `status.md` da kohortni umumlashtiring (ishtirokchilar, asosiy topilmalar, rejalashtirilgan
   tuzatishlar) va DOCS-SORA bosqichi o'zgargan bo'lsa, `roadmap.md`-ni yangilang.
3. dan tashqariga chiqish bosqichlarini bajaring
   [`reviewer-onboarding`](./reviewer-onboarding.md): kirishni bekor qilish, arxivlash
   so'rovlar va ishtirokchilarga rahmat.
4. Artefaktlarni yangilash, nazorat summasini qayta ishga tushirish orqali keyingi to'lqinni tayyorlang,
   va taklif shablonini yangi sanalar bilan yangilash.

Ushbu o'yin kitobini doimiy ravishda qo'llash oldindan ko'rish dasturini tekshirilishi va tekshirilishini ta'minlaydi
Docs/DevRel-ga portal GA ga yaqinlashganda takliflarni ko'paytirishning takrorlanadigan usulini beradi.