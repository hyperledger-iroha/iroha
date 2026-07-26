---
lang: uz
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM konfiguratsiyasi migratsiyasi

# SM konfiguratsiyasi migratsiyasi

SM2/SM3/SM4 funksiyalar to'plamini ishlab chiqarish bilan kompilyatsiya qilishdan ko'ra ko'proq narsani talab qiladi
`sm` xususiyat bayrog'i. Tugunlar qatlamning orqasida funksionallikni o'rnatadi
`iroha_config` profillari va genezis manifestining mos kelishini kuting
sukut bo'yicha. Ushbu eslatmani targ'ib qilishda tavsiya etilgan ish jarayoni tasvirlangan
mavjud tarmoq "faqat Ed25519" dan "SM-yoqilgan" ga.

## 1. Qurilish profilini tekshiring

- `--features sm` bilan binarlarni kompilyatsiya qilish; faqat siz qachon `sm-ffi-openssl` qo'shing
  OpenSSL/Tongsuo oldindan ko'rish yo'lidan foydalanishni rejalashtiring. `sm`siz tuziladi
  Agar konfiguratsiya yoqilgan bo'lsa ham, kirish paytida xususiyat `sm2` imzolarini rad etadi
  ular.
- CI `sm` artefaktlarini nashr etishini va tasdiqlashning barcha bosqichlarini (`yuk) tasdiqlang
  test -p iroha_crypto --sm` xususiyatlari, integratsiya moslamalari, fuzz to'plamlari) o'tish
  siz joylashtirmoqchi bo'lgan aniq ikkilik fayllarda.

## 2. Qatlam konfiguratsiyasini bekor qilish

`iroha_config` uchta darajani qo'llaydi: `defaults` → `user` → `actual`. SM yuboring
operatorlar validatorlarga tarqatadigan `actual` profilidagi bekor qiladi va
`user`-ni Ed25519-da qoldiring - faqat ishlab chiquvchining standart sozlamalari o'zgarishsiz qoladi.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

Xuddi shu blokni `kagami genesis orqali `defaults/genesis` manifestiga nusxalash
yaratish …` (add ` - ruxsat berilgan-imzolash sm2 --standart-xesh sm3-256` agar kerak bo'lsa
bekor qiladi) shuning uchun `parameters` bloki va kiritilgan metama'lumotlar quyidagilarga mos keladi.
ish vaqti konfiguratsiyasi. Tengdoshlar manifest va konfiguratsiya qachon boshlashdan bosh tortadilar
suratlar farqlanadi.

## 3. Ibtido manifestlarini qayta tiklash

- Har biri uchun `kagami genesis generate --consensus-mode <mode>` ni ishga tushiring
  muhitni o'rnating va TOML bekor qilish bilan birga yangilangan JSONni bajaring.
- Manifestga imzo cheking (`kagami genesis sign …`) va `.nrt` foydali yukini taqsimlang.
  Imzolanmagan JSON manifestidan yuklanadigan tugunlar ish vaqti kriptosini oladi
  to'g'ridan-to'g'ri fayldan konfiguratsiya - hali ham bir xil mustahkamlikka bo'ysunadi
  tekshiruvlar.

## 4. Trafikdan oldin tasdiqlang

- Yangi ikkilik fayllar va konfiguratsiyalar bilan bosqichli klasterni ta'minlang, so'ngra tasdiqlang:
  - Tengdoshlar qayta ishga tushganda `/status` `crypto.sm_helpers_available = true` ni ko'rsatadi.
  - Torii qabuli hali ham SM2 imzolarini rad etadi, `sm2` esa yo'q.
    `allowed_signing` va aralash Ed25519/SM2 to'plamlarini ro'yxatda qabul qiladi
    ikkala algoritmni ham o'z ichiga oladi.
  - `iroha_cli tools crypto sm2 export …` asosiy material yangi orqali ekilgan
    sukut bo'yicha.
- SM2 deterministik imzolarini qamrab oluvchi integratsiya tutun skriptlarini ishga tushiring va
  Xost/VM muvofiqligini tasdiqlash uchun SM3 xeshlash.

## 5. Orqaga qaytarish rejasi- O'zgartirishni hujjatlashtiring: `sm2` ni `allowed_signing` dan olib tashlang va qayta tiklang
  `default_hash = "blake2b-256"`. O'zgartirishni bir xil `actual` orqali o'tkazing
  profil quvur liniyasi, shuning uchun har bir validator monoton ravishda aylanadi.
- SM manifestlarini diskda saqlash; mos kelmaydigan konfiguratsiya va genezisni ko'rgan tengdoshlar
  ma'lumotlar boshlashni rad etadi, bu qisman orqaga qaytishdan himoya qiladi.
- Agar OpenSSL/Tongsuo oldindan ko'rish ishtirok etsa, o'chirish uchun qadamlarni qo'shing
  `crypto.enable_sm_openssl_preview` va umumiy ob'ektlarni o'chirish
  ish vaqti muhiti.

## Malumot materiali

- [`docs/genesis.md`](../../genesis.md) - genezis manifestining tuzilishi va
  `crypto` bloki.
- [`docs/source/references/configuration.md`](../references/configuration.md) -
  `iroha_config` bo'limlari va sukut bo'yicha umumiy ko'rinish.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – oxirigacha
  SM kriptografiyasini jo'natish uchun oxirgi operator nazorat ro'yxati.