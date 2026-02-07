---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e797879d1f77c8cfd62fcc67874d584f6bdeee9395faafe52fc33f26ce2e6a21
source_last_modified: "2025-12-29T18:16:35.904811+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus Dasturchilar portaliga xush kelibsiz

SORA Nexus dasturchi portali interaktiv hujjatlar, SDK to'plamlarini taqdim etadi
Nexus operatorlari va Hyperledger Iroha uchun darsliklar va API havolalari
hissa qo'shuvchilar. U amaliy qo'llanmalar orqali asosiy hujjatlar saytini to'ldiradi
va spetsifikatsiyalarni bevosita ushbu ombordan yaratdi.

## Bu yerda nima qila olasiz

- **Norito-ni o'rganing** – umumiy ko'rinishdan boshlang va tushunish uchun tezkor boshlang
  serializatsiya modeli va bayt-kod vositalari.
- **Bootstrap SDKs** – bugungi kunda JavaScript va Rust uchun tezkor ishga tushirishni kuzatib boring; Python,
  Swift va Android yo'riqnomalari retseptlar ko'chirilayotganda ularga qo'shiladi.
- ** API ma'lumotnomalarini ko'rib chiqish** - Torii OpenAPI sahifasi so'nggi RESTni taqdim etadi
  spetsifikatsiya va konfiguratsiya jadvallari kanonik Markdown bilan bog'lanadi
  manbalar.
- **O'rnatishlarni tayyorlang** - operatsion ish kitoblari (telemetriya, hisob-kitob, Nexus
  qoplamalar) `docs/source/` dan ko'chirilmoqda va ushbu saytga qo'nadi.
  migratsiya davom etmoqda.

## Joriy holat

- ✅ Norito va SDK tezkor ishga tushirish uchun jonli sahifalar bilan Docusaurus v3 iskala.
- ✅ Torii OpenAPI plagini `npm run sync-openapi` ga ulangan.
- ⏳ Qolgan qoʻllanmalarni `docs/source/` dan koʻchirish.
- ⏳ CI hujjatlariga oldindan koʻrish tuzilmalari va lintinglarni qoʻshish.

## Ishtirok etish

- Mahalliy ishlab chiqish buyruqlari uchun `docs/portal/README.md` ga qarang (`npm install`,
  `npm run start`, `npm run build`).
- Kontentni ko'chirish vazifalari `DOCS-*` yo'l xaritasi elementlari bilan birga kuzatiladi.
  Hissalar qabul qilinadi - `docs/source/` port bo'limlari va sahifani qo'shing
  yon panelga.
- Agar siz yaratilgan artefaktni qo'shsangiz (xususiyatlar, konfiguratsiya jadvallari), qurilishni hujjatlashtiring
  bo'lajak hissa qo'shuvchilar uni osongina yangilashlari uchun buyruq.