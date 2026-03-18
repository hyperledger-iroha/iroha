---
lang: am
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto ሳጥኖችን በመጠቀም SM2 Annex D ቬክተሮችን ስለማረጋገጥ ማስታወሻዎች።

# SM2 አባሪ D የቬክተር ማረጋገጫ (RustCrypto)

ይህ የእግር ጉዞ የጂኤም/ቲ 0003 አባሪ ዲ ምሳሌን ከRustCrypto's `sm2` crate ጋር ለማረጋገጥ (እና ለማረም) የተጠቀምንባቸውን ደረጃዎች ይይዛል። ቀኖናዊው አባሪ ምሳሌ 1 ውሂብ (ማንነት `ALICE123@YAHOO.COM`፣ መልዕክት `"message digest"` እና የታተመው `(r, s)`) አሁን በ`crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` ውስጥ ተመዝግቧል። OpenSSL/Tongsuo/gmssl ፊርማውን በደስታ ያረጋግጡ (`sm_vectors.md` ይመልከቱ) ግን የ RustCrypto's `sm2 v0.13.3` አሁንም በ `signature::Error` ነጥቡን ውድቅ ያደርጋል፣ ስለዚህ CLI እኩልነት የተረጋገጠ ሲሆን የዝገቱ ታጥቆ ዥረት በመጠባበቅ ላይ ነው።

#ጊዜያዊ ሣጥን

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`፡

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

# ግኝቶች

- ከቀኖናዊው አባሪ ምሳሌ 1 `(r, s)` በአሁኑ ጊዜ አልተሳካም ምክንያቱም `sm2::VerifyingKey::from_sec1_bytes` `signature::Error` ይመልሳል። ወደላይ/የስር መንስኤን ይከታተሉ (በአሁኑ የሣጥኑ መለቀቅ ላይ ከርቭ-መለኪያ አለመመጣጠን የተነሳ ሊሆን ይችላል።
- መታጠቂያው በ `sm2 v0.13.3` በንጽህና ያጠናቅራል እና RustCrypto (ወይም የተለጠፈ ሹካ) የአባሪ ምሳሌ 1 ነጥብ/ፊርማ ጥንድ ከተቀበለ በኋላ አውቶሜትድ የማገገም ሙከራ ይሆናል።
- OpenSSL/Tongsuo/gmssl ማረጋገጫ በ `sm_vectors.md` ውስጥ ባሉ ትዕዛዞች ተሳክቷል; LibreSSL (የማክኦኤስ ነባሪ) አሁንም የSM2/SM3 ድጋፍ የለውም፣ ስለዚህም የአካባቢ ክፍተት።

## ቀጣይ እርምጃዎች

1. አንዴ እንደገና ይሞክሩ `sm2` አባሪ ምሳሌ 1 ነጥቡን የሚቀበል ኤፒአይ ሲያጋልጥ (ወይም ከላይ ከተሰቀለ በኋላ የጥምዝ መለኪያዎችን ያረጋግጣል) ስለዚህ ማሰሪያው በአካባቢው ማለፍ ይችላል።
2. RustCrypto እስክሪፕቶ እስኪያስተካክል ድረስ ቀኖናዊውን አባሪ ምሳሌ ለመጠበቅ CLI Sanity check (OpenSSL/Tongsuo/gmssl) በCI ቧንቧዎች ውስጥ ያስቀምጡ።
3. ሁለቱም የ RustCrypto እና OpenSSL ተመሳሳይነት ማረጋገጫዎች ከተሳኩ በኋላ መታጠቂያውን ወደ Iroha's regression suite ያስተዋውቁ።