---
lang: uz
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto qutilari yordamida SM2 D ilovasi vektorlarini tekshirish bo'yicha eslatmalar.

# SM2 Ilova D vektor tekshiruvi (RustCrypto)

Ushbu ko'rsatma GM/T 0003 D ilovasini RustCrypto-ning `sm2` kassasi bilan tekshirish (va disk raskadrovka) uchun qo'llagan qadamlarimizni qamrab oladi. The canonical Annex Example 1 data (identity `ALICE123@YAHOO.COM`, message `"message digest"`, and the published `(r, s)`) is now recorded in `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl imzoni mamnuniyat bilan tasdiqlaydi (qarang: `sm_vectors.md`), lekin RustCrypto-ning `sm2 v0.13.3` hali ham `signature::Error` bilan nuqtani rad etadi, shuning uchun CLI pariteti tasdiqlanadi, shu bilan birga Rust jabduqlari o'zgarmagan holda qoladi.

## Vaqtinchalik quti

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`:

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`:

```rust
use hex::FromHex;
use sm2::dsa::{signature::Verifier, Signature, VerifyingKey};

fn main() {
    let distid = "ALICE123@YAHOO.COM";
    let sig_bytes = <Vec<u8>>::from_hex(
        "40f1ec59f793d9f49e09dcef49130d4194f79fb1eed2caa55bacdb49c4e755d16fc6dac32c5d5cf10c77dfb20f7c2eb667a457872fb09ec56327a67ec7deebe7",
    )
    .expect("signature hex");
    let sig_array = <[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap();
    let signature = Signature::from_bytes(&sig_array).unwrap();

    let public_key = <Vec<u8>>::from_hex(
        "040ae4c7798aa0f119471bee11825be46202bb79e2a5844495e97c04ff4df2548a7c0240f88f1cd4e16352a73c17b7f16f07353e53a176d684a9fe0c6bb798e857",
    )
    .expect("public key hex");

    // This still returns Err with RustCrypto 0.13.3 – track upstream.
    let verifying_key = VerifyingKey::from_sec1_bytes(distid, &public_key).unwrap();

    verifying_key
        .verify(b"message digest", &signature)
        .expect("signature verified");
}
```

## Topilmalar

- `(r, s)` 1-misolining kanonik ilovasiga muvofiq tekshirish hozirda bajarilmaydi, chunki `sm2::VerifyingKey::from_sec1_bytes` `signature::Error`ni qaytaradi; yuqori oqim/asosiy sababni kuzatib boring (ehtimol, sandiqning joriy versiyasidagi egri parametr mos kelmasligi sababli).
- Jabduqlar `sm2 v0.13.3` bilan toza kompilyatsiya qilinadi va RustCrypto (yoki yamalgan vilka) Ilova 1-misol nuqta/imzo juftligini qabul qilgandan so‘ng avtomatlashtirilgan regressiya testiga aylanadi.
- OpenSSL/Tongsuo/gmssl tekshiruvi `sm_vectors.md` da buyruqlar bilan muvaffaqiyatli amalga oshiriladi; LibreSSL (macOS sukut bo'yicha) hali ham SM2/SM3-ni qo'llab-quvvatlamaydi, shuning uchun mahalliy bo'shliq.

## Keyingi qadamlar

1. `sm2` 1-ilova misolini qabul qiladigan API ochilganda (yoki yuqori oqim egri chiziq parametrlarini tasdiqlagandan so‘ng) jabduqlar mahalliy darajada o‘tishi uchun qayta sinovdan o‘tkazing.
2. RustCrypto tuzatmaguncha kanonik ilova misolini himoya qilish uchun CI quvurlarida CLI aql-idrok tekshiruvini (OpenSSL/Tongsuo/gmssl) saqlang.
3. RustCrypto va OpenSSL paritet tekshiruvlari muvaffaqiyatli o‘tgandan so‘ng, Iroha regressiya to‘plamiga jabduqni ko‘taring.