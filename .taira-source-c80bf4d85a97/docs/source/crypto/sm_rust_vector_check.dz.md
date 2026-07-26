---
lang: dz
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

///! རུསི་ཀིརིཔ་ཊོ་ཀེརེཊིསི་ལག་ལེན་འཐབ་སྟེ་ ཨེསི་ཨེམ་༢ ཟུར་ཐོ་ ཌི་ཝེག་ཊར་ཚུ་ བདེན་དཔྱད་འབད་ནིའི་སྐོར་ལས་ དྲན་ཐོ།

# SM2 མཉམ་སྦྲེལ་ཌི་ཝེག་ཊར་བདེན་དཔྱད་ (RustCrypto)

འདི་གིས་ ང་བཅས་ཀྱིས་ GM/T 0003 Annex D དཔེ་འདི་ RustCrypto’s `sm2` crate དང་ཅིག་ཁར་ བདེན་དཔྱད་འབད་ནི་ལུ་ལག་ལེན་འཐབ་མི་ གོམ་པ་ཚུ་ བཟུང་ཚུགསཔ་ཨིན། ཀེན་ནོ་ནེགསི་ ཟུར་ཐོ་དཔེ་ ༡ གནད་སྡུད་ (ངོ་རྟགས་ `ALICE123@YAHOO.COM`, འཕྲིན་དོན་ `"message digest"`, དང་ དཔར་བསྐྲུན་འབད་ཡོད་པའི་ `(r, s)`) ད་ལྟོ་ `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` ནང་ཐོ་བཀོད་འབད་ཡོདཔ་ཨིན། OpenSSL/Tongsoo/gmssl གིས་ མཚན་རྟགས་འདི་ དགའ་སྤྲོ་ཐོག་ལས་ བདེན་དཔྱད་འབདཝ་ཨིན། (`sm_vectors.md`) དེ་འབདཝ་ད་ RustCrypto གི་ `sm2 v0.13.3` གིས་ ད་ལྟོ་ཡང་ `signature::Error` གི་ས་ཚིགས་འདི་ ངོས་ལེན་མི་འབདཝ་ལས་ CLI parity འདི་ ངོས་ལེན་འབདཝ་ཨིནམ་ལས་ Rust harness འདི་ ཡར་འཕེལ་གྱི་བཅོ་ཁ་རྐྱབ་མ་ཚུགས་པར་ལུས་ཡོདཔ་ཨིན།

## གནས་སྐབས་ཀྱི་ཀེར་རེ།

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

## འཚོལ་ཐོབ།

- ཀེན་ནོ་ནིག་ཟུར་འཛིན་དཔེ་གཞི་ལུ་ བདེན་དཔྱད་འབད་ནི་འདི་ `sm2::VerifyingKey::from_sec1_bytes` གིས་ `signature::Error` སླར་ལོག་འབདཝ་ལས་ ད་ལྟོ་འཐུས་ཤོར་བྱུང་ཡོདཔ་ཨིན། ཊེག་ཊར་ ཡར་རྒྱུན་/རྩ་བའི་རྒྱུ་རྐྱེན་ (ད་ལྟའི་གསར་བཏོན་ནང་ གུག་ཀྱོག་ཚད་གཞི་མ་མཐུནམ་འོང་ནི་མས།)
- བརྡ་རྟགས་འདི་ `sm2 v0.13.3` དང་ཅིག་ཁར་ གཙང་ཏོག་ཏོ་སྦེ་ བསྡུ་སྒྲིག་འབད་དེ་ རཱསི་ཊི་ཀིརིཔ་ཊོ་ (ཡང་ན་ ཐིག་ཡོད་པའི་ཕོརཀ་) གིས་ ཨེན་ནེགསི་དཔེ་གཟུགས་ ༡ ས་ཚིགས་/མཚན་རྟགས་ཆ་ཅན་འདི་ངོས་ལེན་འབད་ཚར་བའི་ཤུལ་ལས་ རང་བཞིན་གྱིས་ བསྐྱར་ལོག་བརྟག་དཔྱད་ཅིག་ལུ་འགྱུར་འོང་།
- OpenSSL/Tongsoo/gmssl བདེན་དཔྱད་འདི་གིས་ `sm_vectors.md` ནང་ལུ་བརྡ་བཀོད་ཚུ་དང་གཅིག་ཁར་མཐར་འཁྱོལ་འབདཝ་ཨིན། LibreSSL (macOS free) ད་དུང་ SM2/SM3 རྒྱབ་སྐྱོར་མེདཔ་ལས་ ཉེ་གནས་ཀྱི་བར་སྟོང་འདི་ཨིན།

## ཤུལ་མམ་གྱི་གོམ་པ།

1. བསྐྱར་དུ་བརྟག་དཔྱད་ཐེངས་གཅིག་ `sm2` གིས་ ཟུར་རྟགས་དཔེ་ ༡ པ་འདི་ངོས་ལེན་འབད་མི་ API ཅིག་ ཕྱིར་བཏོན་འབདཝ་ཨིན་ ༼ཡང་ན་ ཡར་འཕེལ་གྱི་ཤུལ་ལས་ གུག་གུགཔ་གི་ཚད་གཞི་ཚུ་ ངེས་གཏན་བཟོཝ་ཨིན་༽ དེ་འབདཝ་ལས་ མཐུད་མཚམས་འདི་ ནང་འཁོད་ལས་ བརྒལ་ཚུགས།
2. CLI གཙང་སྦྲ་བརྟག་དཔྱད་ (OpenSSL/Tongsuo/gmssl) འདི་ CI གི་མདོང་ལམ་ནང་ལུ་བཞག་དགོ།
3. RustCrypto དང་ OpenSSL parity chections གཉིས་ཆ་རའི་ཤུལ་ལས་ Iroha གི་ འགྱུར་ལྡོག་སྒྲིག་ཆས་ནང་ལུ་ ཡར་འཕེལ་གཏང་།