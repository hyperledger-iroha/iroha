---
lang: uz
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2025-12-29T18:16:35.963528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Boshqaruv quvuri (Iroha 2 va SORA Parlamenti)

# Joriy holat (v1)
- Boshqaruv takliflari quyidagicha amalga oshiriladi: taklifchi → referendum → hisob → kuchga kirish. Referendum oynalari va ishtirok etish/tasdiqlash chegaralari `gov.md` da tavsiflanganidek amalga oshiriladi; qulflar faqat uzaytiriladi va muddati tugashi bilan ochiladi.
- Parlament tanlovida deterministik tartib va ​​muddat chegaralari bilan VRF asosidagi chizmalardan foydalaniladi; doimiy ro'yxat mavjud bo'lmaganda, Torii `gov.parliament_*` konfiguratsiyasidan foydalangan holda zaxirani oladi. Kengashga kirish va kvorum tekshiruvlari `gov_parliament_bodies` / `gov_pipeline_sla` testlarida amalga oshiriladi.
- Ovoz berish rejimlari: ZK (standart, ichki baytlar bilan `Active` VK talab qilinadi) va Oddiy (kvadrat og'irlik). Rejimning mos kelmasligi rad etiladi; qulfni yaratish/kengaytirish ZK va oddiy qayta ovoz berish uchun regressiya testlari bilan ikkala rejimda ham monotondir.
- Tasdiqlovchining noto'g'ri xatti-harakati `NextMode` + `ModeActivationHeight` tomonidan qo'llaniladigan qo'shma konsensus topshirish bilan dalillar quvuri (`/v2/sumeragi/evidence*`, CLI yordamchilari) orqali amalga oshiriladi.
- Himoyalangan nom maydonlari, ish vaqtini yangilash ilgaklari va boshqaruv manifestini qabul qilish `governance_api.md` da hujjatlashtirilgan va telemetriya bilan qoplangan (`governance_manifest_*`, `governance_protected_namespace_total`).

# Parvoz / orqada qolish
- VRF chizma artefaktlarini nashr qilish (urug', isbot, buyurtma qilingan ro'yxat, muqobillar) va ko'rsatilmaganlar uchun almashtirish qoidalarini kodlash; durang va almashtirish uchun oltin armatura qo'shing.
- Parlament organlari uchun SLA-ning bosqichma-bosqich bajarilishi (qoidalar → kun tartibi → o'rganish → ko'rib chiqish → hakamlar hay'ati → kuchga kirish) aniq taymerlar, eskalatsiya yo'llari va telemetriya hisoblagichlarini talab qiladi.
- Siyosat – hakamlar hay’atining maxfiy/kommit – oshkora ovoz berish va bog‘liq poraxo‘rlikka qarshilik tekshiruvlari hali ham amalga oshirilishi kerak.
- Rol-bog'lanish ko'paytmalari, yuqori xavfli jismlar uchun noto'g'ri harakatlar va xizmat ko'rsatish joylari o'rtasidagi sovutish konfiguratsiyasi sanitariya-tesisat va sinovlarni talab qiladi.
- `gov.md`/`status.md` da boshqaruv yoʻlaklarining muhrlanishi va referendum oynasi/saylov ishtirokchilari eshigi kuzatilgan; Qolgan qabul sinovlari tushganda yo'l xaritasi yozuvlarini yangilab turing.