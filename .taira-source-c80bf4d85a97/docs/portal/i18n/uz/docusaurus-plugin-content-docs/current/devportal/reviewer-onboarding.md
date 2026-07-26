---
id: reviewer-onboarding
lang: uz
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Umumiy ko'rinish

DOCS-SORA dasturchi portalining bosqichma-bosqich ishga tushirilishini kuzatib boradi. Tekshirish summasi bilan himoyalangan tuzilmalar
(`npm run serve`) va qattiqlashtirilgan. Oqimlarini sinab ko'ring, keyingi bosqichni blokdan chiqaring:
ommaviy oldindan koʻrish keng ochilishidan oldin tekshirilgan sharhlovchilarni ishga tushirish. Ushbu qo'llanma
so'rovlarni qanday to'plash, muvofiqligini tekshirish, kirishni ta'minlash va
ishtirokchilarni xavfsiz tarzda bortdan chiqarib yuborish. ga murojaat qiling
[takliflar oqimini koʻrib chiqish](./preview-invite-flow.md) kohortni rejalashtirish, taklif qilish uchun
kadans va telemetriya eksporti; quyidagi qadamlar amalga oshiriladigan harakatlarga qaratilgan
sharhlovchi tanlanganidan keyin.

- **Qoʻl:** hujjatlarni oldindan koʻrishga ruxsati kerak boʻlgan sharhlovchilar (`docs-preview.sora`,
  GitHub Pages tuzilmalari yoki SoraFS to'plamlari) GA dan oldin.
- **Qo'llash doirasi tashqarisida:** Torii yoki SoraFS operatorlari (o'z bortiga qo'shiladi)
  to'plamlar) va ishlab chiqarish portalini joylashtirish (qarang
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Rollar va shartlar

| Rol | Odatdagi maqsadlar | Kerakli artefaktlar | Eslatmalar |
| --- | --- | --- | --- |
| Asosiy saqlovchi | Yangi qo'llanmalarni tekshiring, tutun sinovlarini o'tkazing. | GitHub tutqichi, Matritsa kontakti, faylda imzolangan CLA. | Odatda allaqachon `docs-preview` GitHub jamoasida; hali ham so'rov yuboring, shuning uchun kirish tekshirilishi mumkin. |
| Hamkor sharhlovchi | Ommaga chiqarishdan oldin SDK snippetlari yoki boshqaruv tarkibini tasdiqlang. | Korporativ elektron pochta, qonuniy POC, imzolangan oldindan ko'rish shartlari. | Telemetriya + ma'lumotlarni qayta ishlash talablarini tan olishi kerak. |
| Jamoa ko'ngillisi | Qo'llanmalar haqida fikr-mulohaza bildiring. | GitHub tutqichi, afzal qilingan kontakt, vaqt mintaqasi, CoC qabul qilinishi. | Kogortalarni kichik tuting; hissa qo'shish shartnomasini imzolagan sharhlovchilarga ustunlik bering. |

Barcha sharhlovchi turlari quyidagilarga ega bo'lishi kerak:

1. Artefaktlarni oldindan koʻrish uchun maqbul foydalanish siyosatini tan oling.
2. Xavfsizlik/kuzatish mumkin bo'lgan ilovalarni o'qing
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Har qanday xizmat ko'rsatishdan oldin `docs/portal/scripts/preview_verify.sh` ni ishga tushirishga rozilik bildiring
   mahalliy surat.

## Ish jarayonini qabul qilish

1. So'rovchidan to'ldirishni so'rang
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   shakl (yoki uni biror masalaga nusxalash/joylashtirish). Hech bo'lmaganda suratga oling: shaxs, aloqa
   usuli, GitHub tutqichi, ko'rib chiqish uchun mo'ljallangan sanalar va tasdiqlash
   xavfsizlik hujjatlari o'qildi.
2. So‘rovni `docs-preview` trekeriga yozib oling (GitHub muammosi yoki boshqaruvi
   chipta) va tasdiqlovchini tayinlang.
3. Old shartlarni tasdiqlang:
   - Fayl bo'yicha CLA / hissa qo'shuvchi shartnomasi (yoki hamkor shartnomasi ma'lumotnomasi).
   - Qabul qilinadigan foydalanishni tasdiqlovchi so'rovda saqlangan.
   - Xavflarni baholash tugallandi (masalan, yuridik tomonidan tasdiqlangan hamkor sharhlovchilar).
4. Tasdiqlovchi so'rovda imzo chekadi va kuzatuv muammosini istalganiga bog'laydi
   o'zgartirishni boshqarish yozuvi (misol: `DOCS-SORA-Preview-####`).

## Ta'minlash va asboblar

1. **Artefaktlarni ulashish** — Eng soʻnggi oldindan koʻrish deskriptorini + arxivini taqdim eting
   CI ish oqimi yoki SoraFS pin (`docs-portal-preview` artefakt). Eslatma
   Ishlaydigan sharhlovchilar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Cheklash summasi ijrosi bilan xizmat ko'rsatish** — Tekshiruv so'rovi bo'yicha tekshiruvchilar
   buyruq:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Bu `scripts/serve-verified-preview.mjs` dan qayta foydalanadi, shuning uchun hech qanday tasdiqlanmagan tuzilish bo'lishi mumkin emas
   tasodifan ishga tushirilgan.

3. **GitHub-ga ruxsat bering (ixtiyoriy)** — Agar sharhlovchilarga nashr etilmagan filiallar kerak bo'lsa,
   ko'rib chiqish muddati davomida ularni `docs-preview` GitHub jamoasiga qo'shing va
   a'zolik o'zgarishini so'rovga yozib qo'ying.

4. **Ko‘mak kanallari bilan muloqot qilish** — Qo‘ng‘iroq bo‘yicha kontaktni baham ko‘ring (Matrix/Slack)
   va [`incident-runbooks`](./incident-runbooks.md) dan hodisa tartibi.

5. **Telemetriya + fikr-mulohaza** — Sharhlovchilarga anonim tahlillar
  to'plangan (qarang: [`observability`](./observability.md)). Fikr bildiring
  taklifnomada ko'rsatilgan shakl yoki nashr shablonini kiriting va tadbirni ro'yxatdan o'tkazing
  [`preview-feedback-log`](./preview-feedback-log) yordamchi, shuning uchun to'lqin xulosasi
  dolzarbligicha qoladi.

## Tekshiruvchi nazorat ro'yxati

Ko'rib chiqishga kirishdan oldin sharhlovchilar quyidagilarni bajarishlari kerak:

1. Yuklab olingan artefaktlarni tekshiring (`preview_verify.sh`).
2. Portalni `npm run serve` (yoki `serve:verified`) orqali ishga tushiring.
   nazorat summasi himoyasi faol.
3. Yuqorida havola qilingan xavfsizlik va kuzatuvchanlik eslatmalarini o'qing.
4. OAuth-ni sinab ko'ring/konsolni qurilma kodiga kirish (agar mavjud bo'lsa) yordamida sinab ko'ring va
   ishlab chiqarish tokenlarini qayta ishlatishdan saqlaning.
5. Kelishilgan kuzatuvchi (muammo, umumiy hujjat yoki shakl) va tegdagi topilmalarni fayl
   ularni oldindan ko'rish reliz yorlig'i bilan.

## Xizmat ko'rsatuvchi mas'uliyat va offboarding

| Bosqich | Amallar |
| --- | --- |
| Boshlanish | Qabul qilish nazorat roʻyxati soʻrovga ilova qilinganligini tasdiqlang, artefaktlar + koʻrsatmalar bilan baham koʻring, [`preview-feedback-log`](./preview-feedback-log) orqali `invite-sent` yozuvini qoʻshing va agar koʻrib chiqish bir haftadan koʻproq davom etsa, oʻrta nuqta sinxronlashni rejalashtiring. |
| Monitoring | Oldindan koʻrish telemetriyasini kuzatib boring (noodatiy boʻlgan trafikni qidiring, nosozliklarni koʻring) va agar biror shubhali narsa yuz bersa, voqeani tekshirish kitobini kuzatib boring. `feedback-submitted`/`issue-opened` hodisalari topilmalar yetib borishi bilan jurnalga kiring, shunda toʻlqin koʻrsatkichlari toʻgʻri qoladi. |
| Bortdan ketish | Vaqtinchalik GitHub yoki SoraFS ruxsatini bekor qiling, `access-revoked`ni yozib oling, soʻrovni arxivlang (fikrlar xulosasi + bajarilmagan amallarni oʻz ichiga oladi) va koʻrib chiquvchilar registrini yangilang. Sharhlovchidan mahalliy tuzilmalarni tozalashni va [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) dan yaratilgan dayjestni biriktirishini so'rang. |

Tekshiruvchilarni to'lqinlar orasida aylantirganda ham xuddi shunday jarayondan foydalaning. ni saqlash
Repodagi qog'oz izi (muammo + andozalar) DOCS-SORA tekshirilishi mumkin bo'lib qolishiga yordam beradi va
boshqaruvga oldindan koʻrish ruxsati hujjatlashtirilgan boshqaruv elementlariga amal qilganligini tasdiqlash imkonini beradi.

## Taklif shablonlari va kuzatish

- Har bir targ'ibotni quyidagidan boshlang
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  fayl. U minimal yuridik tilni oladi, nazorat summasi ko'rsatmalarini ko'rib chiqadi,
  va sharhlovchilar maqbul foydalanish siyosatini tan olishlarini kutish.
- Shablonni tahrirlashda `<preview_tag>` uchun to'ldirgichlarni almashtiring,
  `<request_ticket>` va aloqa kanallari. Yakuniy xabarning nusxasini saqlang
  qabul qilish chiptasi, shuning uchun sharhlovchilar, tasdiqlovchilar va auditorlar murojaat qilishlari mumkin
  yuborilgan aniq so'z.
- Taklif yuborilgandan so'ng, kuzatuv jadvalini yoki muammoni yangilang
  `invite_sent_at` vaqt tamg'asi va kutilgan tugash sanasi
  [taklif oqimini oldindan ko'rish](./preview-invite-flow.md) hisoboti kogortni olishi mumkin
  avtomatik ravishda.