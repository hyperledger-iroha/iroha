---
lang: uz
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN attestatsiyalari va aylanish

Ushbu eslatma Maxfiy ma'lumotlar tugunlari (SDN) sertifikatlari uchun majburiy modelni qamrab oladi
Yurisdiktsiya Data Guardian (JDG) oqimi tomonidan foydalaniladi.

## Majburiyat formati
- `JdgSdnCommitment` qamrovni bog'laydi (`JdgAttestationScope`), shifrlangan
  foydali yuk xeshi va SDN umumiy kaliti. Muhrlar terilgan imzolardir
  (`SignatureOf<JdgSdnCommitmentSignable>`) domen yorlig'i bo'yicha foydali yuk
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- Strukturaviy tekshirish (`validate_basic`) quyidagilarni amalga oshiradi:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - joriy blok diapazonlari
  - bo'sh bo'lmagan muhrlar
  - orqali amalga oshirilganda attestatsiyaga nisbatan tenglik
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- Deduplication attestatsiya validatori tomonidan amalga oshiriladi (signer+payload hash
  o'ziga xoslik) ushlab qolingan/takroriy majburiyatlarning oldini olish.

## Ro'yxatga olish va aylanish siyosati
- SDN kalitlari `JdgSdnRegistry` da ishlaydi, `(Algorithm, public_key_bytes)` tomonidan kalitlanadi.
- `JdgSdnKeyRecord` faollashtirish balandligini, ixtiyoriy pensiya balandligini qayd etadi,
  va ixtiyoriy asosiy kalit.
- Aylanish `JdgSdnRotationPolicy` tomonidan boshqariladi (hozirda: `dual_publish_blocks`
  qoplama oynasi). Bola kalitini ro'yxatdan o'tkazish ota-ona pensiyasini yangilaydi
  `child.activation + dual_publish_blocks`, to'siqlar bilan:
  - bedarak yo'qolgan ota-onalar rad etiladi
  - faollashtirishlar qat'iy ortib borishi kerak
  - imtiyozli oynadan oshib ketadigan qoplamalar rad etiladi
- Ro'yxatga olish kitobi yordamchilari holat uchun o'rnatilgan yozuvlarni (`record`, `keys`) yuzaga chiqaradi.
  va API ta'siri.

## Tasdiqlash oqimi
- `JdgAttestation::validate_with_sdn_registry` strukturani o'rab oladi
  attestatsiya tekshiruvlari va SDN ijrosi. `JdgSdnPolicy` mavzulari:
  - `require_commitments`: PII/maxfiy foydali yuklar mavjudligini ta'minlash
  - `rotation`: ota-ona pensiyasini yangilashda foydalaniladigan imtiyozli oyna
- Har bir majburiyat quyidagilar uchun tekshiriladi:
  - tuzilmaviy asoslilik + attestatsiya doirasi mosligi
  - ro'yxatdan o'tgan kalit mavjudligi
  - tasdiqlangan blok diapazonini qamrab oluvchi faol oyna (nafaqaga chiqish chegaralari allaqachon mavjud
    ikki tomonlama nashr qilish imtiyozini o'z ichiga oladi)
  - domen yorlig'i bo'lgan majburiyat organi ustidagi amal muhri
- Barqaror xatolar operator dalillari uchun indeksni yuzaga keltiradi:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  yoki tizimli `Commitment`/`ScopeMismatch` nosozliklari.

## Operator runbook
- **Ta'minlash:** birinchi SDN kalitini `activated_at` da yoki undan oldin ro'yxatdan o'tkazing
  birinchi maxfiy blok balandligi. Asosiy barmoq izini JDG operatorlariga e'lon qiling.
- **Rotate:** vorisi kalitni yarating, uni `rotation_parent` da ro'yxatdan o'tkazing
  joriy kalitga ishora va ota-ona pensiya tengligini tasdiqlang
  `child_activation + dual_publish_blocks`. bilan foydali yuk majburiyatlarini qayta muhrlang
  bir-biriga yopishgan oynada faol kalit.
- **Audit:** Torii/status orqali roʻyxatga olish kitobi suratlarini (`record`, `keys`) ochish
  auditorlar faol kalit va pensiya oynalarini tasdiqlashlari uchun yuzalar. Ogohlantirish
  agar tasdiqlangan diapazon faol oynadan tashqariga tushsa.
- **Qayta tiklash:** `UnknownSdnKey` → registrda muhrlash kaliti mavjudligiga ishonch hosil qiling;
  `InactiveSdnKey` → faollashtirish balandligini aylantirish yoki sozlash; `InvalidSeal` →
  foydali yuklarni qayta muhrlash va sertifikatlarni yangilash.## Ish vaqti yordamchisi
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) siyosatni paketlaydi +
  ro'yxatga olish kitobi va `validate_with_sdn_registry` orqali attestatsiyalarni tasdiqlaydi.
- Registrlarni Norito kodli `JdgSdnKeyRecord` to'plamlaridan yuklash mumkin (qarang.
  `JdgSdnEnforcer::from_reader`/`from_path`) yoki yig'ilgan
  `from_records`, bu ro'yxatga olish paytida aylanish to'siqlarini qo'llaydi.
- Operatorlar Norito to'plamini Torii/statusga dalil sifatida saqlashlari mumkin
  surfacing esa bir xil foydali yuk qabul tomonidan ishlatiladigan enforcer oziqlantiradi va
  konsensus himoyachilari. Yagona global enforcer orqali ishga tushirishda ishga tushirish mumkin
  `init_enforcer_from_path` va `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  jonli siyosatni + status/Torii sirtlari uchun asosiy yozuvlarni oching.

## Sinovlar
- `crates/iroha_data_model/src/jurisdiction.rs` da regressiya qamrovi:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, mavjud bilan bir qatorda
  tuzilmaviy attestatsiya/SDN tekshirish testlari.