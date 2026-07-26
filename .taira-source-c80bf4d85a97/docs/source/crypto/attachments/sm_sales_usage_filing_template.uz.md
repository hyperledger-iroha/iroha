---
lang: uz
direction: ltr
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2025-12-29T18:16:35.938696+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Sotish va foydalanish toʻgʻrisidagi hujjatlarni taqdim etish (língàng/yàngìnì) andozasi
% Hyperledger Iroha Muvofiqlik ishchi guruhi
% 2026-05-06

#Ko'rsatmalar

Quruqlik uchun SCA ofisiga joylashtirishdan foydalanishni topshirishda ushbu shablondan foydalaning
operatorlar. Har bir joylashtirish klasteri yoki maʼlumotlar maydoni uchun bitta taqdimotni taqdim eting. Yangilash
operatorga xos ma'lumotlarga ega bo'lgan joy egalari va sanab o'tilgan dalillarni ilova qiling
nazorat ro'yxatida.

# 1. Operator va tarqatish bo'yicha xulosa

| Maydon | Qiymat |
|-------|-------|
| Operator nomi | {{ OPERATOR_NAME }} |
| Biznesni ro'yxatdan o'tkazish ID | {{ REG_ID }} |
| Ro'yxatdan o'tgan manzil | {{ MANZIL }} |
| Asosiy kontakt (ism / unvon / elektron pochta / telefon) | {{ ALOQA }} |
| Joylashtirish identifikatori | {{ DEPLOYMENT_ID }} |
| Joylashtirish joyi(lar)i | {{ JOYLASHMALAR }} |
| Hujjat turi | Sotish/foydalanish (kàngàng/kàngàngí) |
| Ariza topshirish sanasi | {{ YYYY-AA-KK }} |

# 2. Joylashtirish tafsilotlari

- Dasturiy ta'minot identifikatori / xesh: `{{ BUILD_HASH }}`
- Qurilish manbasi: {{ BUILD_SOURCE }} (masalan, operator tomonidan manbadan qurilgan, sotuvchi tomonidan taqdim etilgan ikkilik).
- Faollashtirish sanasi: {{ ACTIVATION_DATE }}
- Rejalashtirilgan parvarishlash oynalari: {{ MAINTENANCE_CADENCE }}
- SM imzolashda ishtirok etuvchi tugun rollari:
  | Tugun | Rol | SM funksiyalari yoqilgan | Kalit omborining joylashuvi |
  |------|------|--------------------|--------------------|
  | {{ NODE_ID }} | {{ ROL }} | {{ XUSUSIYATLARI }} | {{ VAULT }} |

№ 3. Kriptografik boshqaruv elementlari

- Ruxsat etilgan algoritmlar: {{ ALGORITMLAR }} (SM toʻplami konfiguratsiyaga mos kelishiga ishonch hosil qiling).
- Hayotiy tsiklning asosiy xulosasi:
  | Bosqich | Tavsif |
  |-------|-------------|
  | Avlod | {{ KEY_GENERATION }} |
  | Saqlash | {{ KEY_STORAGE }} |
  | Aylanish | {{ KEY_ROTATION }} |
  | Bekor qilish | {{ KEY_REVOCATION }} |
- Alohida identifikatsiya (`distid`) siyosati: {{ DISTID_POLICY }}
- Konfiguratsiya ko'chirmasi (`crypto` bo'limi): xeshlar bilan Norito/JSON oniy tasvirini taqdim eting.

# 4. Telemetriya va audit izlari

- Monitoring oxirgi nuqtalari: {{ METRICS_ENDPOINTS }} (`/metrics`, asboblar paneli).
- Jurnalga kiritilgan ko'rsatkichlar: `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  kechikish histogramlari, xato hisoblagichlari.
- Jurnalni saqlash siyosati: {{ LOG_RETENTION }} (≥ uch yil tavsiya etiladi).
- Audit jurnali saqlash joyi: {{ AUDIT_STORAGE }}

# 5. Hodisaga javob berish va kontaktlar

| Rol | Ism | Telefon | Elektron pochta | SLA |
|------|------|-------|-------|-----|
| Xavfsizlik operatsiyalari etakchi | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Qo'ng'iroq bo'yicha kripto | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Qonuniy / muvofiqlik | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Sotuvchini qo'llab-quvvatlash (agar mavjud bo'lsa) | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |

№ 6. Qo'shimchalar nazorat ro'yxati- [ ] Xeshli konfiguratsiya surati (Norito + JSON).
- [ ] Deterministik tuzilishning isboti (xeshlar, SBOM, takrorlanuvchanlik eslatmalari).
- [ ] Telemetriya asboblar paneli eksporti va ogohlantirish taʼriflari.
- [ ] Hodisalarga javob berish rejasi va chaqiruv bo'yicha aylanish hujjati.
- [ ] Operator treningi tasdiqlanishi yoki runbook kvitansiyasi.
- [ ] Yetkazib berilgan artefaktlarni aks ettiruvchi eksport-nazorat bayonoti.
- [ ] Tegishli shartnoma kelishuvlari yoki siyosatdan voz kechishlar nusxalari.

№ 7. Operator deklaratsiyasi

> Yuqorida sanab o'tilgan joylashtirish XXR reklamasiga mos kelishini tasdiqlaymiz
> kriptografiya qoidalari, SM-ni yoqadigan xizmatlar hujjatlashtirilganga amal qiladi
> hodisalarga javob berish va telemetriya siyosati va audit artefaktlari bo'ladi
> kamida uch yil saqlanadi.

- Vakolatli imzolovchi: ________________________
- Sana: ______________________