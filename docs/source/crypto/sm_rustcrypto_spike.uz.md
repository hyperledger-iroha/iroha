---
lang: uz
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto SM integratsiyalashuvi uchun eslatmalar.

# RustCrypto SM Spike eslatmalari

## Maqsad
RustCrypto’ning `sm2`, `sm3` va `sm4` kassalarini (qo‘shimcha ravishda `rfc6979`, `ccm`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` opsiyalari bilan bog‘liq bo‘lmagan kompensatsiyalar) joriy etishni tasdiqlang. `iroha_crypto` qutisi va xususiyat bayrog'ini kengroq ish maydoniga ulashdan oldin maqbul qurish vaqtlarini beradi.

## Taklif etilayotgan qaramlik xaritasi

| Sandiq | Tavsiya etilgan versiya | Xususiyatlar | Eslatmalar |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/imzolar) | `std` | `elliptic-curve` ga bog'liq; MSRV ish maydoniga mos kelishini tekshiring. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/xeshlar) | standart | API `sha2` parallel, mavjud `digest` belgilari bilan birlashadi. |
| `sm4` | `0.5.1` (RustCrypto/blok-shifrlar) | standart | Shifrlash belgilari bilan ishlaydi; AEAD o'ramlari keyinchalik ko'tarilish uchun qoldirildi. |
| `rfc6979` | `0.4` | standart | Deterministik bo'lmagan hosila uchun qayta foydalanish. |

*Versiyalar 2024-12 yillardagi joriy nashrlarni aks ettiradi; qo‘nishdan oldin `cargo search` bilan tasdiqlang.*

## Manifest o'zgarishlar (qoralama)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Kuzatuv: `iroha_crypto` (hozirda `0.13.8`)dagi versiyalarga mos kelish uchun `elliptic-curve` pinini.

## Spike nazorat ro'yxati
- [x] `crates/iroha_crypto/Cargo.toml` ga ixtiyoriy bog'liqliklar va xususiyat qo'shing.
- [x] Simlarni tasdiqlash uchun `signature::sm` modulini `cfg(feature = "sm")` orqasida to'ldiruvchi tuzilmalar bilan yarating.
- [x] kompilyatsiyani tasdiqlash uchun `cargo check -p iroha_crypto --features sm` ni ishga tushiring; rekord qurish vaqti va yangi qaramlik soni (`cargo tree --features sm`).
- [x] `cargo check -p iroha_crypto --features sm --locked` bilan faqat std holatini tasdiqlang; `no_std` tuzilmalari endi qo'llab-quvvatlanmaydi.
- [x] `docs/source/crypto/sm_program.md` da fayl natijalari (vaqtlar, qaramlik daraxti deltasi).

## Qo'lga olish uchun kuzatishlar
- Qo'shimcha kompilyatsiya vaqti va asosiy.
- `cargo builtinsize` bilan ikkilik o'lchamli ta'sir (agar o'lchash mumkin bo'lsa).
- Har qanday MSRV yoki xususiyat ziddiyatlari (masalan, `elliptic-curve` kichik versiyalari bilan).
- Ogohlantirishlar chiqarildi (xavfsiz kod, const-fn gating) yuqori oqim yamoqlarini talab qilishi mumkin.

## Kutilayotgan elementlar
- Ish maydoniga bog'liqlik grafigini oshirishdan oldin Crypto WG tasdiqlashini kuting.
- Ko'rib chiqish uchun qutilarni sotish yoki crates.io ga ishonishni tasdiqlang (oynalar talab qilinishi mumkin).
- Tekshirish ro'yxati tugallanganligini belgilashdan oldin `Cargo.lock` yangilanishini `sm_lock_refresh_plan.md` uchun muvofiqlashtiring.
- Qulflangan fayl va qaramlik daraxtini qayta tiklash uchun ruxsat berilgandan so'ng `scripts/sm_lock_refresh.sh` dan foydalaning.

## 19-01-2025 Spike jurnali
- `iroha_crypto` da ixtiyoriy bog'liqliklar (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) va `sm` xususiyat bayrog'i qo'shildi.
- Kompilyatsiya paytida xeshlash/blok shifrlash API-larini ishlatish uchun o'rnatilgan `signature::sm` moduli.
- `cargo check -p iroha_crypto --features sm --locked` endi qaramlik grafigini hal qiladi, lekin `Cargo.lock` yangilash talabi bilan bekor qiladi; repository siyosati qulflangan faylni tahrirlashni taqiqlaydi, shuning uchun biz ruxsat berilgan blokirovkani yangilashni muvofiqlashtirmagunimizcha kompilyatsiya jarayoni kutilmoqda.## 2026-02-12 Spike jurnali
- Oldingi blokirovka fayl blokeri hal qilindi - bog'liqliklar allaqachon qo'lga olindi - shuning uchun `cargo check -p iroha_crypto --features sm --locked` muvaffaqiyatli bo'ldi (dev Mac-da sovuq qurish 7,9 soniya; bosqichma-bosqich qayta ishga tushirish 0,23 soniya).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` faqat `std` konfiguratsiyalarida (`no_std` yoʻli qolmaydi) ixtiyoriy xususiyat kompilyatsiyasini tasdiqlovchi 1.0 soniyada oʻtadi.
- `sm` xususiyati yoqilgan qaramlik deltasi 11 ta qutini taqdim etadi: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pem-rfc7468`, `sm2` `primeorder`, `sm2`, `sm3`, `sm4` va `sm4-gcm`. (`rfc6979` allaqachon asosiy grafikning bir qismi edi.)
- Foydalanilmayotgan NEON siyosati yordamchilari uchun ogohlantirishlar saqlanib qoladi; o'lchashni tekislash ish vaqti ushbu kod yo'llarini qayta yoqmaguncha shunday qoldiring.