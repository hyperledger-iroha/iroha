---
lang: uz
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Bog'liqlik auditining xulosasi

Sana: 2025-09-01

Qo'llanish doirasi: Cargo.toml fayllarida e'lon qilingan va Cargo.lock'da hal qilingan barcha qutilarni ish maydoni bo'ylab ko'rib chiqish. RustSec konsalting ma'lumotlar bazasiga qarshi yuk-audit, shuningdek, sandiqning qonuniyligi va algoritmlar uchun "asosiy sandiq" tanlovini qo'lda tekshirish bilan amalga oshirildi.

Ishlaydigan asboblar/buyruqlar:
- `cargo tree -d --workspace --locked --offline` – tekshirilgan dublikat versiyalari
- `cargo audit` - ma'lum zaifliklar va tortilgan qutilar uchun Cargo.lock skanerdan o'tkazildi

Xavfsizlik boʻyicha maslahatlar topildi (hozir 0 ta zaiflik; 2 ta ogohlantirish):
- o'zaro faoliyat kanal - RUSTSEC-2025-0024
  - Ruxsat etilgan: `crates/ivm/Cargo.toml` da `0.5.15` ga o'zgartirildi.

  - Tuzatildi: `pprof` `crates/iroha_torii/Cargo.toml` da `prost-codec` ga aylantirildi.

- halqa — RUSTSEC-2025-0009
  - Ruxsat etilgan: QUIC/TLS stegi buzilgan (`quinn 0.11`, `rustls 0.23`, `tokio-rustls 0.26`) va WS stek `tungstenite/tokio-tungstenite 0.24` ga yangilandi. `cargo update -p ring --precise 0.17.12` orqali `ring 0.17.12` ga majburiy qulflash.

Qolgan maslahatlar: yo'q. Qolgan ogohlantirishlar: `backoff` (xizmat ko'rsatilmagan), `derivative` (xizmat ko'rsatilmagan).

Qonuniylik va "asosiy sandiq" baholash (diqqat markazida):
- Xeshlash: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak` (keng tarqalgan) - kanonik tanlovlar.
- AEAD/Simmetrik: `aes-gcm`, `chacha20poly1305`, `aead` belgilari (RustCrypto) — kanonik.
- Imzolar/ECC: `ed25519-dalek`, `x25519-dalek` (dalek loyihasi), `k256` (RustCrypto), `secp256k1` (libsecp ulanishlari) — barchasi qonuniy; sirt maydonini kamaytirish uchun bitta secp256k1 stekini (sof Rust uchun `k256` yoki libsecp uchun `secp256k1`) afzal qiling.
- BLS12-381/ZK: `blstrs`, `halo2_*` — ishlab chiqarishda keng qo'llaniladi ZK ekotizimlari; qonuniy.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` - qonuniy mos yozuvlar qutilari.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — kanonik zamonaviy Rust TLS to'plami.
- Shovqin: `snow` - kanonik amalga oshirish.
- Seriyalashtirish: `parity-scale-codec` SCALE uchun kanonik hisoblanadi. Serde ish maydoni bo'ylab ishlab chiqarishga bog'liqlikdan olib tashlandi; Norito hosil qiladi/yozuvchilar har bir ish vaqti yo'lini qamrab oladi. Har qanday qoldiq Serde havolalari tarixiy hujjatlarda, himoya skriptlarida yoki faqat sinov uchun ruxsat etilgan ro'yxatlarda mavjud.
- FFI/libs: `libsodium-sys-stable`, `openssl` — qonuniy; ishlab chiqarish yo'llarida OpenSSL orqali Rustls ni afzal ko'ring (joriy kod allaqachon mavjud).

Tavsiyalar:
- Manzil ogohlantirishlari:
  - `backoff` o'rniga `retry`/`futures-retry` yoki mahalliy eksponensial backoff yordamchisi bilan almashtirishni o'ylab ko'ring.
  - `derivative` hosilalarini qo'lda ko'rsatma yoki kerak bo'lganda `derive_more` bilan almashtiring.
- O'rta: takroriy ilovalarni kamaytirish uchun `k256` yoki `secp256k1` da birlashtiring (ikkalasini ham faqat chindan kerak bo'lganda qoldiring).
- O'rta: ZK foydalanish uchun `poseidon-primitives 0.2.0` kelib chiqishini ko'rib chiqing; Agar iloji bo'lsa, parallel ekotizimlarni minimallashtirish uchun Arkworks/Halo2-native Poseidon ilovasi bilan moslashishni ko'rib chiqing.

Eslatmalar:
- `cargo tree -d` kutilgan ikki nusxadagi asosiy versiyalarni (`bitflags` 1/2, bir nechta `ring`) ko'rsatadi, bu o'z-o'zidan xavfsizlik xavfi emas, balki qurilish sirtini oshiradi.
- typosquatga o'xshash qutilar kuzatilmadi; barcha nomlar va manbalar taniqli ekotizim qutilariga yoki ichki ish maydoni a'zolariga tegishli.
- Eksperimental: BLS-ni faqat blstrlar uchun backendga ko'chirishni boshlash uchun `iroha_crypto` xususiyati qo'shildi `bls-backend-blstrs` (yoqilganda arkworksga bog'liqlikni yo'q qiladi). Xulq-atvor/kodlash o'zgarishlarini oldini olish uchun standart `w3f-bls` bo'lib qoladi. Tenglash rejasi:
  - `crates/iroha_crypto/tests/bls_backend_compat.rs`-da kalitlarni bir marta chiqaradigan va `SecretKey`, `PublicKey` va imzolarni yig'ishni qamrab oluvchi ikkala orqa tomonda tenglikni ta'minlaydigan `crates/iroha_crypto/tests/bls_backend_compat.rs`-ga qaytish moslamalarini qo'shing.

Kuzatuvlar (taklif etilgan ish elementlari):
- Serde to'siqlarini CI (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) da saqlang, shuning uchun yangi ishlab chiqarish usullarini kiritib bo'lmaydi.

Ushbu audit uchun sinov o'tkazildi:
- `cargo audit` so'nggi maslahat ma'lumotlar bazasi bilan ishladi; to'rtta maslahat va ularning qaramlik daraxtlarini tasdiqladi.
- Tuzatish joylarini aniqlash uchun ta'sirlangan qutilarning to'g'ridan-to'g'ri bog'liqlik deklaratsiyasi qidirildi.