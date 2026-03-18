---
lang: uz
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kripto qaramlik tekshiruvi

## Streebog (`streebog` sandiq)

- **Daraxtdagi versiya:** `0.11.0-rc.2` `vendor/streebog` ostida ishlab chiqariladi (`gost` funksiyasi yoqilganda ishlatiladi).
- **Iste'molchi:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + xabar xeshlash).
- **Holat:** Faqat nomzod uchun. Hozirda hech qanday RC bo'lmagan quti talab qilinadigan API sirtini taklif qilmaydi,
  Shunday qilib, biz oxirgi reliz uchun yuqori oqimni kuzatib, auditorlik uchun daraxt ichidagi qutini aks ettiramiz.
- ** Tekshirish punktlarini ko'rib chiqing:**
  - Wycheproof to'plami va TC26 qurilmalariga nisbatan tasdiqlangan hash chiqishi orqali
    `cargo test -p iroha_crypto --features gost` (qarang: `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    Joriy bog'liqlik bilan har bir TC26 egri chizig'i bilan birga Ed25519/Secp256k1 mashq qiladi.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    yangiroq o'lchovlarni tekshirilgan medianalar bilan taqqoslaydi (CIda `--summary-only` dan foydalaning, qo'shing
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` qayta asoslashda).
  - `scripts/gost_bench.sh` dastgohni o'rab oladi + oqimni tekshirish; JSON-ni yangilash uchun `--write-baseline`-dan o'ting.
    Oxir-oqibat ish jarayoni uchun `docs/source/crypto/gost_performance.md` ga qarang.
- **Mitigatsiyalar:** `streebog` faqat kalitlarni nolga tenglashtiradigan deterministik paketlar orqali chaqiriladi;
  imzolovchi halokatli RNG ishlamay qolishining oldini olish uchun OS entropiyasi bilan noncesni himoya qiladi.
- **Keyingi harakatlar:** RustCrypto streebog `0.11.x` versiyasini kuzatib boring; teg tushgach, davolang
  standart qaramlik zarbasi sifatida yangilang (tekshiruv summasini tekshiring, farqni ko'rib chiqing, kelib chiqishini yozing va
  sotuvchi oynani tashlang).