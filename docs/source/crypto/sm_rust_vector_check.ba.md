---
lang: ba
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2-ны раҫлау тураһында иҫкәрмәләр D RustCrypto йәшниктәре ярҙамында D векторҙары ҡушымтаһы.

# SM2 Ҡушымта D Вектор тикшерелеүе (RustCrypto)

Был йөрөү беҙ ҡулланған аҙымдарҙы тота (һәм отладка) GM/T 0003 Ҡушымта D миҫалы менән RustCrypto’s `sm2` йәшник. 1-се миҫал 1 мәғлүмәттәре (`ALICE123@YAHOO.COM`, `"message digest"` хәбәр, һәм баҫылған `(r, s)`) хәҙер `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`-та теркәлгән. Ҡултамғаны раҫлаусы OpenSSL/Tongsuo/gmssl шатлыҡлы рәүештә раҫлай (ҡара: `sm_vectors.md`), әммә RustCrypto’s `sm2 v0.13.3` һаман да `signature::Error` менән нөктәне кире ҡаға, шуға күрә CLI паритеты раҫлана, ә Рась йыяһы өҫкө ағымды төҙәтеүҙе көтә.

## Ваҡытлыса йәшник

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

## Табыштар

- 18NI0000014X 1-се миҫалына ҡаршы тикшерелгән, әлеге ваҡытта уңышһыҙлыҡҡа осрай, сөнки `sm2::VerifyingKey::from_sec1_bytes` `signature::Error` ҡайтарыуҙары; трек өҫкә/тамыр сәбәбе (моғайын, ҡойроҡ-параметр тап килмәү арҡаһында йәшник’s ток сығарыу).
- Йүгән `sm2 v0.13.3` менән таҙа компиляциялай һәм автоматлаштырылған регрессия һынауына әйләнәсәк, бер тапҡыр RustCrypto (йәки патчлы вилка) ҡабул итә Ennex миҫал 1 мәрәй/ҡултамға пары.
- OpenSSL/Tongsuo/gmssl тикшерелеүе `sm_vectors.md`-тағы командалар менән уңышҡа өлгәшә; LibreSSL (macOS ғәҙәттәгесә) әлегә SM2/SM3 ярҙамы етешмәй, шуға күрә урындағы айырма.

## Киләһе аҙымдар

1. Ҡабаттан һынау бер тапҡыр `sm2` API фашлай, тип ҡабул итә ҡушымта 1 нөктәһе (йәки өҫкө ағымдан һуң раҫлай ҡойроҡ параметрҙары) шулай йүгән локаль үтә ала.
2. CLI аҡыл тикшерергә (OpenSSL/Тонссуо/гмссл) CI торбаларында канонлы ҡушымта миҫалын һаҡлау өсөн RustCrypto ерҙәрҙе төҙәтергә.
3. Iroha’s регрессия люкс йүгән пропагандалау һуң һәм RustCrypto һәм OpenSSL паритет тикшерелгән уңыш.