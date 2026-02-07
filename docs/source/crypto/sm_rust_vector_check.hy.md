---
lang: hy
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Նշումներ RustCrypto արկղերի միջոցով SM2 հավելված D վեկտորների ստուգման վերաբերյալ:

# SM2 Հավելված D վեկտորի ստուգում (RustCrypto)

Այս ուղեցույցը նկարագրում է այն քայլերը, որոնք մենք օգտագործել ենք վավերացնելու (և կարգաբերելու) GM/T 0003 Հավելված D օրինակը RustCrypto-ի `sm2` տուփով: Հավելվածի օրինակ 1-ի կանոնական տվյալները (ինքնությունը `ALICE123@YAHOO.COM`, հաղորդագրություն `"message digest"` և հրապարակված `(r, s)`) այժմ գրանցված են `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`-ում: OpenSSL/Tongsuo/gmssl-ը ուրախությամբ ստուգում է ստորագրությունը (տես `sm_vectors.md`), սակայն RustCrypto-ի `sm2 v0.13.3`-ը դեռևս մերժում է կետը `signature::Error`-ով, այնպես որ CLI հավասարությունը հաստատվում է, մինչդեռ Rust-ի զրահը շտկվում է:

## Ժամանակավոր վանդակ

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

## Գտածոներ

- Հաստատումը կանոնական հավելվածի օրինակով 1 `(r, s)`-ը ներկայումս ձախողվում է, քանի որ `sm2::VerifyingKey::from_sec1_bytes`-ը վերադարձնում է `signature::Error`; հետևել հոսանքին հակառակ/արմատային պատճառին (հավանաբար, կորի պարամետրերի անհամապատասխանության պատճառով արկղի ընթացիկ թողարկումում):
- Զուգահեռը մաքուր կերպով հավաքվում է `sm2 v0.13.3`-ով և կդառնա ավտոմատացված ռեգրեսիայի թեստ, երբ RustCrypto-ն (կամ կարկատված պատառաքաղը) ընդունի Հավելվածի Օրինակ 1 միավոր/ստորագրություն զույգը:
- OpenSSL/Tongsuo/gmssl ստուգումը հաջողվում է `sm_vectors.md`-ի հրամաններով; LibreSSL-ը (macOS լռելյայն) դեռևս չունի SM2/SM3 աջակցություն, հետևաբար տեղական բացը:

## Հաջորդ քայլերը

1. Կրկին փորձարկեք, երբ `sm2`-ը բացահայտի API-ն, որն ընդունում է Հավելվածի օրինակի 1 կետը (կամ այն բանից հետո, երբ վերին հոսքը կհաստատի կորի պարամետրերը), որպեսզի ամրագոտին կարողանա տեղային անցնել:
2. Պահպանեք CLI առողջական վիճակի ստուգում (OpenSSL/Tongsuo/gmssl) CI խողովակաշարերում՝ կանոնական Հավելվածի օրինակը պահպանելու համար, մինչև RustCrypto ամրագրումը վայրէջք կատարի:
3. Խրախուսեք զրահը Iroha-ի ռեգրեսիոն փաթեթում՝ RustCrypto և OpenSSL հավասարության ստուգումից հետո: