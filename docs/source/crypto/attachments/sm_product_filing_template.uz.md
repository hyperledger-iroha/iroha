---
lang: uz
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2025-12-29T18:16:35.938246+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Mahsulotni topshirish shabloni
% Hyperledger Iroha Muvofiqlik ishchi guruhi
% 2026-05-06

#Ko'rsatmalar

Viloyatga *mahsulot ishlab chiqish uchun ariza* yuborishda ushbu shablondan foydalaning
yoki tarqatishdan oldin shahar Davlat kriptografiya boshqarmasi (SCA) idorasi
SM yoqilgan binarlar yoki Xitoyning materik qismidagi manba artefaktlari. ni almashtiring
loyihaga oid tafsilotlarga ega boʻlgan toʻldiruvchilar, toʻldirilgan shaklni PDF formatida eksport qiling, agar
talab qilinadi va nazorat ro'yxatiga havola qilingan artefaktlarni ilova qiling.

№ 1. Ariza beruvchi va mahsulot haqida xulosa

| Maydon | Qiymat |
|-------|-------|
| Tashkilot nomi | {{ TASHKILOT }} |
| Ro'yxatdan o'tgan manzil | {{ MANZIL }} |
| Qonuniy vakil | {{ HUQUQIY_REP }} |
| Asosiy kontakt (ism / unvon / elektron pochta / telefon) | {{ ALOQA }} |
| Mahsulot nomi | Hyperledger Iroha {{ RELEASE_NAME }} |
| Mahsulot versiyasi / qurish identifikatori | {{ VERSIYA }} |
| Hujjat turi | Mahsulotni ishlab chiqish (kàngāngì) |
| Ariza topshirish sanasi | {{ YYYY-AA-KK }} |

# 2. Kriptografiyadan foydalanishga umumiy nuqtai

- Qo'llab-quvvatlanadigan algoritmlar: `SM2`, `SM3`, `SM4` (quyida foydalanish matritsasini taqdim eting).
- Foydalanish konteksti:
  | Algoritm | Komponent | Maqsad | Deterministik himoya vositalari |
  |----------|-----------|---------|--------------------------|
  | SM2 | {{ KOMPONENT }} | {{ MAQSAD }} | RFC6979 + kanonik r∥s ijrosi |
  | SM3 | {{ KOMPONENT }} | {{ MAQSAD }} | `Sm3Digest` | orqali deterministik xeshlash
  | SM4 | {{ KOMPONENT }} | {{ MAQSAD }} | AEAD (GCM/CCM) majburiy bo'lmagan siyosat bilan |
- Qurilishdagi SM bo'lmagan algoritmlar: {{ OTHER_ALGORITHMS }} (to'liqlik uchun).

# 3. Rivojlanish va ta'minot zanjiri nazorati

- Manba kodlari ombori: {{ REPOSITORY_URL }}
- Deterministik qurish bo'yicha ko'rsatmalar:
  1. `git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (kerak bo'lganda sozlang).
  3. `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`) orqali yaratilgan SBOM.
- Uzluksiz integratsiya muhiti haqida xulosa:
  | Element | Qiymat |
  |------|-------|
  | OS / versiyasini yaratish | {{ BUILD_OS }} |
  | Kompilyator asboblar zanjiri | {{ TOLCHINA }} |
  | OpenSSL / Tongsuo manbasi | {{ OPENSSL_SOURCE }} |
  | Qayta ishlab chiqarishni tekshirish summasi | {{ CHEKK SO‘MASI }} |

# 4. Kalitlarni boshqarish va xavfsizlik

- Standart yoqilgan SM funksiyalari: {{ DEFAULTS }} (masalan, faqat tekshirish uchun).
- Imzolash uchun talab qilinadigan konfiguratsiya bayroqlari: {{ CONFIG_FLAGS }}.
- Asosiy saqlash yondashuvi:
  | Element | Tafsilotlar |
  |------|---------|
  | Kalit yaratish vositasi | {{ KEY_TOOL }} |
  | Saqlash muhiti | {{ STORAGE_MEDIUM }} |
  | Zaxiralash siyosati | {{ BACKUP_POLICY }} |
  | Kirish boshqaruvlari | {{ ACCESS_CONTROLS }} |
- Voqealarga javob berish aloqalari (24/7):
  | Rol | Ism | Telefon | Elektron pochta |
  |------|------|-------|-------|
  | Kripto etakchi | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} |
  | Platforma operatsiyalari | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} |
  | Huquqiy aloqa | {{ NAME }} | {{ TELEFON }} | {{ EMAIL }} |

№ 5. Qo'shimchalar nazorat ro'yxati- [ ] Manba kodi surati (`{{ SOURCE_ARCHIVE }}`) va xesh.
- [ ] Deterministik qurilish skripti / takrorlanuvchanlik eslatmalari.
- [ ] SBOM (`{{ SBOM_PATH }}`) va qaramlik manifesti (`Cargo.lock` barmoq izi).
- [ ] Deterministik test transkriptlari (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [ ] SM kuzatilishini ko'rsatuvchi telemetriya asboblar paneli eksporti.
- [ ] Eksport-nazorat bayonoti (alohida shablonga qarang).
- [ ] Audit hisobotlari yoki uchinchi tomon baholashlari (agar allaqachon to'ldirilgan bo'lsa).

№ 6. Ariza beruvchining deklaratsiyasi

> Men yuqoridagi ma'lumotlarning to'g'riligini, oshkor qilinganligini tasdiqlayman
> kriptografik funksionallik XXRning amaldagi qonunlari va qoidalariga mos keladi,
> va tashkilot taqdim etilgan artefaktlarni hech bo'lmaganda saqlab turishi
> uch yil.

- Imzo (qonuniy vakil): ________________________
- Sana: ______________________