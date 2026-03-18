---
lang: uz
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d19690fde1b9e3c6f072cf1ac5ad89fb82578eb1b21e4d8dbc82d399518b4d42
source_last_modified: "2026-01-28T17:11:30.641899+00:00"
translation_last_reviewed: 2026-02-07
title: Address Safety & Accessibility
description: UX requirements for presenting and sharing Iroha addresses safely (ADDR-6c).
translator: machine-google-reviewed
---

Ushbu sahifada ADDR-6c hujjatlari taqdim etiladi. Bularni qo'llang
hamyonlar, tadqiqotchilar, SDK vositalari va har qanday portal yuzasiga cheklovlar
insonga qaragan manzillarni ko'rsatadi yoki qabul qiladi. Kanonik ma'lumotlar modeli yashaydi
`docs/account_structure.md`; Quyidagi nazorat ro'yxati ularni qanday ochishni tushuntiradi
xavfsizlik yoki foydalanish imkoniyatini buzmasdan formatlar.

## Xavfsiz almashish oqimlari

- I105 manziliga nusxa ko'chirish/ulashish uchun har bir amalni standart qilib qo'ying. Yechilganlarni ko'rsatish
  domenni qo'llab-quvvatlovchi kontekst sifatida ishlating, shuning uchun nazorat yig'indisi qatori old va markazda qoladi.
- Toʻliq matnli manzil va QRni birlashtirgan “Ulashish” imtiyozini taklif qiling
  bir xil foydali yukdan olingan kod. Foydalanuvchilarga amal qilishdan oldin ikkalasini ham tekshirishga ruxsat bering.
- Bo'sh joy kesishni talab qilganda (kichik kartalar, bildirishnomalar), yetakchilikni saqlang
  inson tomonidan o'qiladigan prefiks, ellipslarni ko'rsating va oxirgi 4-6 belgini shunday saqlang
  nazorat summasi langari saqlanib qoladi. Toʻliq nusxa olish uchun teginish/klaviatura yorligʻini taqdim eting
  kesilmasdan string.
- Oldindan ko'rib chiqadigan tasdiqlovchi tostni chiqarish orqali almashish buferining sinxronlanishini oldini oling
  ko'chirilgan aniq I105 qatori. Telemetriya mavjud bo'lgan joyda nusxasini hisoblang
  UX regressiyalari tezda yuzaga kelishi uchun harakatlarni almashishga urinishlar.

## IME va kiritish himoyasi

- Manzil maydonlarida ASCII bo'lmagan kiritishni rad etish. Qachon IME kompozitsion artefaktlar (to'liq
  kengligi, Kana, ohang belgilari) paydo bo'ladi, qanday qilib tushuntirilgan qatorli ogohlantirish paydo bo'ladi
  qayta urinishdan oldin klaviaturani lotin yozuviga oʻtkazish uchun.
- Birlashtiruvchi belgilarni ajratib turadigan va almashtiradigan oddiy matn joylashtirish zonasini taqdim eting
  tekshirishdan oldin ASCII bo'shliqlari bilan bo'sh joy. Bu foydalanuvchilarni yo'qotishdan saqlaydi
  ular IME o'rta oqimini o'chirib qo'yganlarida taraqqiyot.
- Nol kenglikdagi birlashtiruvchilarga, variatsiya selektorlariga va boshqalarga nisbatan tasdiqlashni qattiqlashtiring
  yashirin Unicode kod nuqtalari. Rad etilgan kod nuqtasi toifasini ro'yxatdan o'tkazing
  Suite telemetriyani import qilishi mumkin.

## Yordamchi texnologiya kutilmalari

- `aria-label` yoki `aria-describedby` bilan har bir manzil blokiga izoh bering
  inson oʻqishi mumkin boʻlgan prefiksni talaffuz qiladi va foydali yukni 4–8 belgidan iborat boʻladi
  guruhlar ("ih tire b uch ikki ..."). Bu ekranni o'qiydiganlarni ishlab chiqarishni to'xtatadi
  tushunarsiz belgilar oqimi.
- Hududni muloyim jonli yangilash orqali muvaffaqiyatli nusxa ko'chirish/ulashish voqealarini e'lon qiling. O'z ichiga oladi
  maqsad (bufer, almashish varag'i, QR), shuning uchun foydalanuvchi harakatni biladi
  diqqatni harakatlantirmasdan yakunlandi.
- QR oldindan koʻrish uchun tavsiflovchi `alt` matnini taqdim eting (masalan, “I105 manzili
  `<account>` zanjirida `0x1234`”). “Manzilni matn sifatida nusxalash”ni taqdim eting
  ko'rish qobiliyati past bo'lgan foydalanuvchilar uchun QR tuvaliga ulashgan zaxira.

## Sora-faqat siqilgan manzillar

- Gating: `i105` siqilgan qatorni aniq tasdiqlash orqasida yashiring.
  Tasdiqlash ariza faqat Sora Nexus zanjirlarida ishlashini takrorlashi kerak.
- Yorliqlash: har bir hodisada ko'rinadigan "Faqat Sora" nishoni va a bo'lishi kerak
  Boshqa tarmoqlar nima uchun I105 shaklini talab qilishini tavsiflovchi maslahat.
- Guardrails: agar faol zanjir diskriminanti Nexus taqsimoti bo'lmasa,
  siqilgan manzilni to'liq yaratishdan bosh torting va foydalanuvchini qaytib yo'naltiring
  I105.
- Telemetriya: siqilgan shakl qanchalik tez-tez so'ralishini va shuning uchun nusxa ko'chirilishini yozib oling
  voqea o'yin kitobi tasodifiy almashish keskinliklarini aniqlay oladi.

## Sifatli eshiklar

- Ushbu manzilni tasdiqlash uchun avtomatlashtirilgan UI testlarini (yoki hikoyalar kitobi a11y to'plamlarini) kengaytiring
  komponentlar zarur ARIA metama'lumotlarini va IME rad etish xabarlarini ochib beradi
  paydo bo'ladi.
- IME kiritish (kana, pinyin), ekranni o'qish uchun o'tish uchun qo'lda QA stsenariylarini qo'shing
  (VoiceOver/NVDA) va yuqori kontrastli mavzularda QR nusxasini chiqarishdan oldin.
- Ushbu tekshiruvlarni I105 paritet testlari bilan bir qatorda relizlar ro'yxatida ko'rsating
  shuning uchun regressiyalar tuzatilmaguncha bloklanadi.