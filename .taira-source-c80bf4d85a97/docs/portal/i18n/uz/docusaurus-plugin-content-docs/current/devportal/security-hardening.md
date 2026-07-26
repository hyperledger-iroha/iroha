---
id: security-hardening
lang: uz
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Security hardening & pen-test checklist
sidebar_label: Security hardening
description: Harden the developer portal before exposing the Try it sandbox outside the lab.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Umumiy ko'rinish

“Yo‘l xaritasi” bandi **DOCS-1b** uchun OAuth qurilma kodiga kirish, kuchli kontent talab qilinadi
xavfsizlik siyosatlari va oldindan ko'rish portalidan oldin takrorlanadigan penetratsion testlar
laboratoriya bo'lmagan tarmoqlarda ishlashi mumkin. Ushbu ilova tahdid modelini tushuntiradi
repoda amalga oshirilgan boshqaruvlar va ko'rib chiqishga imkon beruvchi jonli nazorat ro'yxati
bajarishi kerak.

- **Qoʻl:** Sinab koʻring proksi-server, oʻrnatilgan Swagger/RapiDoc panellari va maxsus
  `docs/portal/src/components/TryItConsole.jsx` tomonidan taqdim etilgan konsolni sinab ko'ring.
- ** Qo'llash doirasi tashqarisida:** Toriining o'zi (Torii tayyorgarlik ko'rib chiqishlari bilan qamrab olingan) va SoraFS
  nashriyot (DOCS-3/7 tomonidan qamrab olingan).

## Tahdid modeli

| Aktiv | Xavf | Yumshatish |
| --- | --- | --- |
| Torii tashuvchi tokenlari | Docs sandbox tashqarisida o'g'irlik yoki qayta foydalanish | Qurilma kodiga kirish (`DOCS_OAUTH_*`) qisqa muddatli tokenlarni chiqaradi, proksi-server sarlavhalarni o'zgartiradi va konsol keshlangan hisob ma'lumotlarini avtomatik ravishda tugatadi. |
| Sinab ko'ring proksi | Ochiq reley sifatida suiiste'mol qilish yoki Torii tezligi chegaralarini chetlab o'tish | `scripts/tryit-proxy*.mjs` kelib chiqishiga ruxsat beruvchi roʻyxatlar, tariflarni cheklash, sogʻliqni tekshirishlar va aniq `X-TryIt-Auth` yoʻnaltirishni amalga oshirish; hech qanday hisobga olish ma'lumotlari saqlanib qolmaydi. |
| Portal ish vaqti | Saytlararo skript yoki zararli o'rnatish | `docusaurus.config.js` Kontent-Xavfsizlik-Siyosat, Ishonchli turlar va Ruxsatlar-Siyosat sarlavhalarini kiritadi; inline skriptlar Docusaurus ish vaqti bilan cheklangan. |
| Kuzatish ma'lumotlari | Telemetriya etishmayotgan yoki o'zgartirish | `docs/portal/docs/devportal/observability.md` problarni/boshqaruv panelini hujjatlashtiradi; `scripts/portal-probe.mjs` nashrdan oldin CI da ishlaydi. |

Dushmanlar orasida ommaga oldindan ko'rishni ko'rayotgan qiziquvchan foydalanuvchilar, zararli aktyorlar kiradi
o'g'irlangan havolalarni sinovdan o'tkazish va saqlangan qirqishga urinayotgan buzilgan brauzerlar
hisobga olish ma'lumotlari. Barcha boshqaruv elementlari ishonchli brauzerlarsiz ishlashi kerak
tarmoqlar.

## Kerakli boshqaruv elementlari

1. **OAuth qurilma kodiga kirish**
   - `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL` sozlang,
     `DOCS_OAUTH_CLIENT_ID` va qurilish muhitidagi tegishli tugmalar.
   - Sinab ko'ring kartasi tizimga kirish vidjetini (`OAuthDeviceLogin.jsx`) ko'rsatadi.
     qurilma kodini oladi, token so‘nggi nuqtasini so‘raydi va tokenlarni avtomatik tozalaydi
     ular muddati tugagandan keyin. Manual Bearer bekor qilish favqulodda holatlar uchun mavjud bo'lib qoladi
     qaytish.
   - OAuth konfiguratsiyasi yo'q bo'lganda yoki endi tuzilmalar muvaffaqiyatsiz tugadi
     zaxira TTLlar DOCS-1b tomonidan belgilangan 300-900-yillar oynasidan tashqariga siljiydi;
     `DOCS_OAUTH_ALLOW_INSECURE=1` ni faqat bir martalik mahalliy oldindan ko'rish uchun o'rnating.
2. **Proksi himoyasi**
   - `scripts/tryit-proxy.mjs` ruxsat etilgan manbalar, tarif cheklovlari, so'rovlarni amalga oshiradi
     o'lcham cheklari va trafikni belgilashda yuqori oqimdagi taymoutlar
     `X-TryIt-Client` va jurnallardan tokenlarni tahrirlash.
   - `scripts/tryit-proxy-probe.mjs` plus `docs/portal/docs/devportal/observability.md`
     jonli zond va asboblar paneli qoidalarini aniqlang; ularni har safar oldin ishga tushiring
     tarqatish.
3. **CSP, ishonchli turlar, ruxsatnomalar siyosati**
   - `docusaurus.config.js` endi deterministik xavfsizlik sarlavhalarini eksport qiladi:
     `Content-Security-Policy` (sukut bo'yicha o'z-o'zidan, qattiq ulanish/img/skript
     ro'yxatlar, Ishonchli turlar talablari), `Permissions-Policy` va
     `Referrer-Policy: no-referrer`.
   - CSP ulanish ro'yxati OAuth qurilma kodi va token so'nggi nuqtalarini oq ro'yxatga oladi
     (Faqat `DOCS_SECURITY_ALLOW_INSECURE=1` bo'lmasa HTTPS), shuning uchun qurilmaga kirish ishlaydi
     boshqa kelib chiqishi uchun qum qutisini bo'shashtirmasdan.
   - Sarlavhalar to'g'ridan-to'g'ri yaratilgan HTML-ga joylashtirilgan, shuning uchun statik xostlar
     qo'shimcha konfiguratsiyani talab qilmaydi. Inline skriptlari bilan cheklangan holda saqlang
     Docusaurus yuklash tizimi.
4. **Runbooks, kuzatuvchanlik va orqaga qaytarish**
   - `docs/portal/docs/devportal/observability.md` problarni tasvirlaydi va
     tizimga kirishda xatoliklarni, proksi-server javob kodlarini va so'rovni kuzatuvchi asboblar paneli
     byudjetlar.
   - `docs/portal/docs/devportal/incident-runbooks.md` eskalatsiyani qamrab oladi
     agar qum qutisi suiiste'mol qilingan bo'lsa, yo'l; bilan birlashtiring
     Oxirgi nuqtalarni xavfsiz aylantirish uchun `scripts/tryit-proxy-rollback.mjs`.

## Qalam sinovi va chiqarish nazorat ro'yxati

Har bir oldindan koʻrish reklamasi uchun ushbu roʻyxatni toʻldiring (natijalarni nashrga ilova qiling
chipta):

1. **OAuth simlarini tekshiring**
   - `DOCS_OAUTH_*` eksporti bilan `npm run start` ni mahalliy sifatida ishga tushiring.
   - Toza brauzer profilidan Sinab ko'ring konsolni oching va tasdiqlang
     qurilma-kod oqimi tokenni chiqaradi, ishlash muddatini hisoblaydi va tozalaydi
     muddati tugagandan yoki tizimdan chiqqandan keyin maydon.
2. **Proksi-serverni tekshirish**
   - `npm run tryit-proxy` Torii bosqichiga qarshi, keyin bajaring
     Sozlangan namuna yo'li bilan `npm run probe:tryit-proxy`.
   - `authSource=override` yozuvlari uchun jurnallarni tekshiring va tezlikni cheklashni tasdiqlang
     oynadan oshib ketganingizda hisoblagichlarni oshiradi.
3. **CSP/Ishonchli turlarni tasdiqlang**
   - `npm run build` va `build/index.html` ni oching. `<meta
     http-equiv="Content-Security-Policy">` tegi kutilgan ko'rsatmalarga mos keladi
     va DevTools oldindan ko'rishni yuklashda hech qanday CSP buzilishlarini ko'rsatmaydi.
   - O'rnatilgan HTMLni olish uchun `npm run probe:portal` (yoki curl) dan foydalaning; prob
     endi `Content-Security-Policy`, `Permissions-Policy` yoki
     `Referrer-Policy` meta teglari yoʻq yoki eʼlon qilingan qiymatlardan farq qiladi
     `docusaurus.config.js` da, shuning uchun boshqaruv sharhlovchilari chiqishga ishonishlari mumkin
     eyeballing curl chiqishi o'rniga kod.
4. **Kuzatilish imkoniyatini ko‘rib chiqish**
   - Sinab ko'ring proksi boshqaruv paneli yashil rangda ekanligini tekshiring (stavkalar chegaralari, xato nisbatlari,
     sog'liqni tekshirish ko'rsatkichlari).
   - `docs/portal/docs/devportal/incident-runbooks.md` da voqea matkapini bajaring
     agar xost o'zgargan bo'lsa (yangi Netlify/SoraFS joylashtirish).
5. **Natijalarni hujjatlash**
   - Chiptaga skrinshotlar/jurnallarni biriktiring.
   - Har bir topilmani tuzatish hisoboti shablonida yozib oling
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     shuning uchun egalar, SLA va qayta sinovdan o'tgan dalillarni keyinroq tekshirish oson.
   - DOCS-1b yoʻl xaritasi bandi tekshirilishi mumkin boʻlib qolishi uchun ushbu nazorat roʻyxatiga qayta havola qiling.

Agar biron bir qadam muvaffaqiyatsiz bo'lsa, reklamani to'xtating, blokirovka qilish muammosini yozing va e'tibor bering
`status.md` da tuzatish rejasi.