---
lang: ka
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! შენიშვნები SM2 დანართის D ვექტორების გადამოწმების შესახებ RustCrypto ყუთების გამოყენებით.

# SM2 დანართი D ვექტორული დადასტურება (RustCrypto)

ეს მიმოხილვა ასახავს ნაბიჯებს, რომლებიც ჩვენ გამოვიყენეთ GM/T 0003 დანართის D მაგალითის დასადასტურებლად (და გამართვისთვის) RustCrypto-ს `sm2` ყუთით. კანონიკური დანართის მაგალითი 1 მონაცემები (იდენტიფიკაცია `ALICE123@YAHOO.COM`, შეტყობინება `"message digest"` და გამოქვეყნებული `(r, s)`) ახლა ჩაწერილია `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`-ში. OpenSSL/Tongsuo/gmssl სიამოვნებით ამოწმებს ხელმოწერას (იხ. `sm_vectors.md`), მაგრამ RustCrypto-ს `sm2 v0.13.3` მაინც უარყოფს `signature::Error` წერტილს, ამიტომ CLI პარიტეტი დადასტურებულია, ხოლო Rust-ის აღკაზმულობა შესწორებულია.

## დროებითი ყუთი

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

## დასკვნები

- დადასტურება კანონიკური დანართი მაგალითი 1 `(r, s)` ამჟამად ვერ ხერხდება, რადგან `sm2::VerifyingKey::from_sec1_bytes` აბრუნებს `signature::Error`; თვალყური ადევნეთ ზემოთ/ძირის მიზეზს (სავარაუდოდ, მრუდის პარამეტრის შეუსაბამობის გამო კრატის მიმდინარე გამოშვებაში).
- აღკაზმულობა შედგენილია სუფთად `sm2 v0.13.3`-ით და გახდება ავტომატური რეგრესიის ტესტი მას შემდეგ, რაც RustCrypto (ან დაყენებული ჩანგალი) მიიღებს დანართის მაგალითს 1 ქულა/ხელმოწერის წყვილს.
- OpenSSL/Tongsuo/gmssl გადამოწმება წარმატებულია `sm_vectors.md`-ში ბრძანებებით; LibreSSL-ს (macOS ნაგულისხმევი) ჯერ კიდევ არ აქვს SM2/SM3 მხარდაჭერა, აქედან გამომდინარე, ადგილობრივი უფსკრული.

## შემდეგი ნაბიჯები

1. ხელახლა ტესტირება მას შემდეგ, რაც `sm2` გამოავლენს API-ს, რომელიც მიიღებს დანართის მაგალითს 1 პუნქტს (ან მას შემდეგ, რაც ზემო დინებაში დაადასტურებს მრუდის პარამეტრებს), რათა აღკაზმულობა ადგილობრივად გაიაროს.
2. განახორციელეთ CLI საღი აზრის შემოწმება (OpenSSL/Tongsuo/gmssl) CI მილსადენებში, რათა დაიცვათ კანონიკური დანართის მაგალითი, სანამ RustCrypto დაფიქსირება არ დადგება.
3. დააწინაურეთ აღკაზმულობა Iroha-ის რეგრესიის კომპლექტში RustCrypto და OpenSSL პარიტეტის შემოწმების წარმატებით დასრულების შემდეგ.