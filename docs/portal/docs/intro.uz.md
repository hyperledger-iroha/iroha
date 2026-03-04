---
lang: uz
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f775ae297c910da91c6ce97e97ee36fb87f60218fcfb97639ace6eba39f2252
source_last_modified: "2025-12-29T18:16:35.118933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus Dasturchilar portaliga xush kelibsiz

SORA Nexus dasturchi portali interaktiv hujjatlar, SDK to'plamlarini taqdim etadi.
Nexus operatorlari va Hyperledger Iroha uchun darsliklar va API havolalari
hissa qo'shuvchilar. U amaliy qo'llanmalar orqali asosiy hujjatlar saytini to'ldiradi
va spetsifikatsiyalarni bevosita ushbu ombordan yaratdi. Endi ochilish sahifasi ko'tariladi
mavzuli Norito/SoraFS kirish nuqtalari, imzolangan OpenAPI suratlari va maxsus
Norito Striming ma'lumotnomasi, shuning uchun hissa qo'shuvchilar oqimni boshqarish tekisligini topishlari mumkin
ildiz spetsifikatsiyasini qazmasdan shartnoma tuzing.

## Bu yerda nima qila olasiz

- **Norito-ni o'rganing** – umumiy ko'rinishdan boshlang va tushunish uchun tezkor boshlang
  serializatsiya modeli va bayt-kod vositalari.
- **Bootstrap SDKs** – bugungi kunda JavaScript va Rust uchun tezkor ishga tushirishni kuzatib boring; Python,
  Swift va Android yo'riqnomalari retseptlar ko'chirilayotganda ularga qo'shiladi.
- ** API havolalarini ko'rib chiqish** - Torii OpenAPI sahifasi eng so'nggi RESTni taqdim etadi
  spetsifikatsiya va konfiguratsiya jadvallari kanonik Markdown bilan bog'lanadi
  manbalar.
- **O'rnatishlarni tayyorlang** - operatsion ish kitoblari (telemetriya, hisob-kitob, Nexus
  qoplamalar) `docs/source/` dan ko'chirilmoqda va ushbu saytga quyidagi tarzda tushadi
  migratsiya davom etmoqda.

## Joriy holat

- ✅ Docusaurus v3 tematik qo'nish, yangilangan tipografiya, gradientga asoslangan
  qahramon/kartalar va Norito Streaming xulosasini o'z ichiga olgan resurs plitalari.
- ✅ Torii OpenAPI plagini `npm run sync-openapi` ga ulangan, imzolangan surat bilan
  cheklar va CSP qo'riqchilari `buildSecurityHeaders` tomonidan amalga oshiriladi.
- ✅ CI (`docs-portal-preview.yml` +) da qamrovni oldindan ko'rish va tekshirish
  `scripts/portal-probe.mjs`), endi oqimli hujjatga kirish, SoraFS tezkor ishga tushirish,
  va artefaktlar nashr etilishidan oldin ma'lumotnomalar ro'yxati.
- ✅ Norito, SoraFS va SDK tezkor ishga tushirish va mos yozuvlar bo'limlari ushbu sahifada mavjud.
  yon panel; `docs/source/` dan yangi importlar (striming, orkestratsiya, runbooks)
  ular mualliflik qilganidek bu yerga qo'ndi.

## Ishtirok etish

- Mahalliy ishlab chiqish buyruqlari uchun `docs/portal/README.md` ga qarang (`npm install`,
  `npm run start`, `npm run build`).
- Kontentni ko'chirish vazifalari `DOCS-*` yo'l xaritasi elementlari bilan birga kuzatiladi.
  Hissalar qabul qilinadi - `docs/source/` port bo'limlari va sahifani qo'shing
  yon panelga.
- Agar siz yaratilgan artefaktni qo'shsangiz (xususiyatlar, konfiguratsiya jadvallari), qurilishni hujjatlashtiring
  bo'lajak hissa qo'shuvchilar uni osongina yangilashlari uchun buyruq.