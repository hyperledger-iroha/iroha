---
lang: uz
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Provenance surati
% Yaratilgan: 2026-01-30

# Atrof-muhit haqida qisqacha ma'lumot

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: hisobotlar `LibreSSL 3.3.6` (macOS’da tizim tomonidan taqdim etilgan TLS asboblar to‘plami).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: Rustga bog'liqlik to'plamini aniq ko'rish uchun `sm_iroha_crypto_tree.txt` ga qarang (`openssl` sandiq v0.10.74, `openssl-sys` v0.9.x, sotuvchi OpenSSL 3.1000NI orqali mavjud; `vendored` xususiyati `crates/iroha_crypto/Cargo.toml` da deterministik oldindan ko'rish tuzilmalari uchun yoqilgan).

# Eslatma

- LibreSSL sarlavhalari/kutubxonalariga qarshi mahalliy rivojlanish muhiti havolalari; ishlab chiqarishni oldindan ko'rish uchun tuzilmalar OpenSSL >= 3.0.0 yoki Tongsuo 8.x dan foydalanishi kerak. Yakuniy artefakt to'plamini yaratishda tizim asboblar to'plamini almashtiring yoki `OPENSSL_DIR`/`PKG_CONFIG_PATH` ni o'rnating.
- Aynan OpenSSL/Tongsuo tarball xeshini (`openssl version -v`, `openssl version -b`, `openssl version -f`) olish uchun ushbu suratni relizlar yaratish muhitida qayta yarating va qayta tiklanadigan qurilish skripti/cheksumini biriktiring. Sotuvchi tuzilmalar uchun Cargo tomonidan ishlatiladigan `openssl-src` sandiq versiyasini yozib oling (`target/debug/build/openssl-sys-*/output` da ko'rinadi).
- Apple Silicon xostlari OpenSSL tutun simini ishga tushirishda `RUSTFLAGS=-Aunsafe-code` ni talab qiladi, shuning uchun AArch64 SM3/SM4 tezlashtirish stublari kompilyatsiya qilinadi (ichki ma'lumotlar macOS'da mavjud emas). `scripts/sm_openssl_smoke.sh` skripti `cargo` ni chaqirishdan oldin CI va mahalliy ishga tushirishni izchil saqlash uchun ushbu bayroqni eksport qiladi.
- Qadoqlash quvuri mahkamlangandan so'ng, yuqori oqim manbasini (masalan, `openssl-src-<ver>.tar.gz` SHA256) biriktiring; CI artefaktlarida bir xil xeshdan foydalaning.