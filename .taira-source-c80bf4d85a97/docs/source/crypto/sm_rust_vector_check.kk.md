---
lang: kk
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto жәшіктерін пайдаланып SM2 D қосымшасының векторларын тексеру туралы ескертпелер.

# SM2 Қосымша D векторлық тексеру (RustCrypto)

Бұл шолу GM/T 0003 D қосымшасының мысалын RustCrypto `sm2` жәшігімен тексеру (және жөндеу) үшін қолданылған қадамдарды қамтиды. The canonical Annex Example 1 data (identity `ALICE123@YAHOO.COM`, message `"message digest"`, and the published `(r, s)`) is now recorded in `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl қолтаңбаны қуана растайды (`sm_vectors.md` қараңыз), бірақ RustCrypto `sm2 v0.13.3` әлі күнге дейін `signature::Error` нүктесін қабылдамайды, сондықтан CLI паритеті расталады, ал Rust жоғары ағыны түзетіледі.

## Уақытша жәшік

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

## Нәтижелер

- `(r, s)` мысалы  канондық қосымшаға қарсы тексеру қазір орындалмайды, себебі `sm2::VerifyingKey::from_sec1_bytes` `signature::Error` қайтарады; жоғары/түбір себебін қадағалаңыз (мүмкін жәшіктің ағымдағы шығарылымындағы қисық-параметрлердің сәйкес келмеуіне байланысты).
- Жинақ `sm2 v0.13.3` көмегімен таза түрде құрастырылады және RustCrypto (немесе патчталған шанышқы) 1-қосымшаның мысалын/қолтаңба жұбын қабылдағаннан кейін автоматтандырылған регрессия сынағы болады.
- OpenSSL/Tongsuo/gmssl тексеруі `sm_vectors.md` пәрмендері арқылы сәтті орындалады; LibreSSL (macOS әдепкі) әлі де SM2/SM3 қолдауы жоқ, демек, жергілікті олқылық.

## Келесі қадамдар

1. `sm2` 1-ші Қосымшаны қабылдайтын API ашқаннан кейін (немесе жоғары ағын қисық параметрлерді растағаннан кейін) желілік желіден өтуі үшін қайта сынақтан өткізіңіз.
2. RustCrypto түзетілгенге дейін канондық қосымша үлгісін қорғау үшін CI құбырларында CLI сауаттылығын тексеруді (OpenSSL/Tongsuo/gmssl) сақтаңыз.
3. RustCrypto және OpenSSL тепе-теңдік тексерулері сәтті болғаннан кейін Iroha регрессиялық жиынтығына қосылымды жылжытыңыз.