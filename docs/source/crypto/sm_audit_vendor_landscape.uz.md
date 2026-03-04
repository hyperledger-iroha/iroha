---
lang: uz
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2025-12-29T18:16:35.941305+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM Audit Vendor Landscape
% Iroha Kripto ishchi guruhi
% 2026-02-12

# Umumiy ko'rinish

Kripto ishchi guruhiga mustaqil sharhlovchilarning doimiy skameykasi kerak
Rust kriptografiyasi va Xitoy GM/T (SM2/SM3/SM4) standartlarini tushuning.
Ushbu eslatma tegishli ma'lumotnomalar bilan firmalarni kataloglaydi va audit natijalarini umumlashtiradi
biz odatda so'raymiz, shuning uchun taklif so'rovi (RFP) sikllari tez va davom etadi
izchil.

# Nomzod firmalar

## Bitlar izi (CN kriptografiya amaliyoti)

- Hujjatlashtirilgan majburiyatlar: 2023 yilda Ant Group kompaniyasining Tongsuo xavfsizligini ko'rib chiqish
  (SM-yoqilgan OpenSSL tarqatish) va Rust-ga asoslangan takroriy auditlar
  Diem/Libra, Sui va Aptos kabi blokcheynlar.
- Kuchli tomonlari: maxsus Rust kriptografiya jamoasi, avtomatlashtirilgan doimiy vaqt
  tahlil vositalari, deterministik bajarilishini tekshirish tajribasi va apparat
  jo'natish siyosati.
- Iroha uchun mos: joriy SM audit SOWni uzaytirishi yoki mustaqil ravishda bajarishi mumkin
  qayta sinovlar; Norito moslamalari va IVM tizimi bilan qulay ishlash
  yuzalar.

## NCC guruhi (APAC kriptografiya xizmatlari)

- Hujjatlangan majburiyatlar: mintaqaviy to'lov uchun gm/T (SM) kodini tekshirish
  tarmoqlar va HSM sotuvchilari; Parity Substrate, Polkadot uchun Rustning oldingi sharhlari,
  va Libra komponentlari.
- Kuchli tomonlari: ikki tilli hisobotga ega katta APAC dastgohi, birlashtirish qobiliyati
  chuqur kodni ko'rib chiqish bilan muvofiqlik uslubidagi jarayonni tekshirish.
- Iroha uchun mos: ikkinchi fikr yoki boshqaruvga asoslangan baholash uchun ideal
  Trail of Bits topilmalari bilan bir qatorda tasdiqlash.

## SECBIT Labs (Pekin)

- Hujjatlangan majburiyatlar: ochiq manbali `libsm` Rust sandiqini ta'minlovchilar
  Nervos CKB va CITA tomonidan qo'llaniladi; Nervos, Muta va uchun Guomi faolligini tekshirdi
  Ikki tilda yetkazib beriladigan FISCO BCOS Rust komponentlari.
- Kuchli tomonlar: Rustda SM ibtidoiylarini faol ravishda jo'natadigan muhandislar, kuchli
  mulkni sinovdan o'tkazish qobiliyati, ichki muvofiqlik bilan chuqur tanishish
  talablar.
- Iroha uchun mos: qiyosiy tahlillarni taqdim eta oladigan sharhlovchilar kerak bo'lganda qimmatlidir
  sinov vektorlari va topilmalar bilan bir qatorda amalga oshirish bo'yicha ko'rsatmalar.

## SlowMist Security (Chengdu)

- Hujjatlashtirilgan majburiyatlar: Substrate/Polkadot Rust xavfsizlik tekshiruvlari, shu jumladan
  Xitoy operatorlari uchun Guomi vilkalar; SM2/SM3/SM4 hamyonini muntazam baholash
  va birjalar tomonidan ishlatiladigan ko'prik kodi.
- Kuchli tomonlari: blokcheynga yo'naltirilgan audit amaliyoti, hodisalarga integratsiyalashgan javob,
  asosiy protokol kodi va operator vositalarini qamrab oluvchi yo'riqnoma.
- Iroha uchun mos: SDK pariteti va operatsion aloqa nuqtalarini tekshirish uchun foydalidir
  asosiy qutilarga qo'shimcha ravishda.

## Chaitin Tech (QAX 404 xavfsizlik laboratoriyasi)- Hujjatlangan majburiyatlar: GmSSL/Tongsuo hardning va SM2/SM3/ga hissa qo'shuvchilar
  Mahalliy moliya institutlari uchun SM4ni amalga oshirish bo'yicha qo'llanma; tashkil etilgan
  TLS steklari va kriptografik kutubxonalarni qamrab oluvchi zang auditi amaliyoti.
- Kuchli tomonlari: chuqur kriptoanaliz foni, rasmiy tekshirishni juftlashtirish qobiliyati
  qo'lda ko'rib chiqishga ega artefaktlar, uzoq muddatli regulyator munosabatlari.
- Iroha uchun mos: me'yoriy imzolanganda yoki rasmiy isbotlangan artefaktlar uchun mos keladi
  standart kodni ko'rib chiqish hisobotiga hamroh bo'lishi kerak.

# Odatdagi audit doirasi va yetkazib berish

- ** Spetsifikatsiyaga muvofiqligi:** SM2 ZA hisobini tasdiqlash, imzo
  kanoniklashtirish, SM3 toʻldirish/siqish va SM4 kalit jadvali va IV bilan ishlash
  GM/T 0003-2012, GM/T 0004-2012 va GM/T 0002-2012 ga qarshi.
- **Determinizm va doimiy vaqtdagi xatti-harakatlar:** dallanishni, qidirishni ko'rib chiqing
  jadvallar va apparat xususiyatlari eshiklari (masalan, NEON, SM4 ko'rsatmalari) ta'minlash uchun
  Rust va FFI jo'natmalari qo'llab-quvvatlanadigan uskunada deterministik bo'lib qolmoqda.
- **FFI va provayder integratsiyasi:** OpenSSL/Tongsuo ulanishlarini ko'rib chiqing,
  PKCS#11/HSM adapterlari va konsensus xavfsizligi uchun xato tarqalish yo'llari.
- **Sinov va armatura qamrovi:** noaniq jabduqlarni baholash, Norito bo'ylab sayohatlar,
  deterministik tutun testlari va bo'shliqlar bo'lgan joyda differentsial testlarni tavsiya eting
  paydo bo'ladi.
- **Tobelik va ta'minot zanjiri tekshiruvi:** ishlab chiqarish kelib chiqishini, sotuvchini tasdiqlang
  yamoq siyosati, SBOM aniqligi va qayta tiklanadigan qurish ko'rsatmalari.
- **Hujjatlar va operatsiyalar:** operatorning ish kitoblarini tasdiqlash, muvofiqlik
  qisqacha ma'lumotlar, konfiguratsiya standartlari va orqaga qaytarish protseduralari.
- **Hisobot kutishlari:** xavf reytingi bilan ijrochi xulosa, batafsil
  topilmalar kodli havolalar va tuzatish bo'yicha ko'rsatmalar, qayta sinov rejasi va
  determinizm kafolatlarini o'z ichiga olgan attestatsiyalar.

# Keyingi qadamlar

- RFQ davrlarida ushbu sotuvchilar ro'yxatidan foydalaning; yuqoridagi nazorat ro'yxatini sozlang
  RFP berishdan oldin faol SM bosqichiga mos keling.
- Ishlash natijalarini `docs/source/crypto/sm_audit_brief.md` va
  shartnomalar bajarilgandan so'ng `status.md` da sirt holati yangilanishi.