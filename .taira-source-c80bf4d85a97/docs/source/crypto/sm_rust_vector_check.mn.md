---
lang: mn
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto хайрцаг ашиглан SM2 Хавсралт D векторуудыг баталгаажуулах тухай тэмдэглэл.

# SM2 Хавсралт D Вектор баталгаажуулалт (RustCrypto)

Энэхүү танилцуулга нь GM/T 0003 Хавсралт D жишээг RustCrypto-ийн `sm2` хайрцгаар баталгаажуулах (болон дибаг хийх) алхамуудыг агуулна. The canonical Annex Example 1 data (identity `ALICE123@YAHOO.COM`, message `"message digest"`, and the published `(r, s)`) is now recorded in `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl гарын үсгээ баяртайгаар баталгаажуулна (`sm_vectors.md`-г үзнэ үү), гэхдээ RustCrypto-н `sm2 v0.13.3` нь `signature::Error`-тэй цэгийг үгүйсгэсэн хэвээр байгаа тул CLI паритет нь батлагдаж, зэвэрсэн бэхэлгээний үзэгдлүүд хэвээр үлдэнэ.

## Түр зуурын хайрцаг

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

## Судалгаа

- `sm2::VerifyingKey::from_sec1_bytes` `signature::Error`-г буцаадаг тул каноник хавсралтын жишээ 1-тэй харьцуулан шалгах нь одоогоор амжилтгүй болсон; дээд урсгал/үндсэн шалтгааныг хянах (кратын одоогийн хувилбар дахь муруй-параметрийн таарамжгүй байдлаас үүдэлтэй байж магадгүй).
- Уяа нь `sm2 v0.13.3`-ээр цэвэрхэн эмхэтгэгдэх ба RustCrypto (эсвэл нөхөөстэй сэрээ) Хавсралтын жишээ 1 оноо/гарын тэмдгийн хосыг хүлээн авмагц автомат регрессийн тест болно.
- OpenSSL/Tongsuo/gmssl баталгаажуулалт `sm_vectors.md` дээрх тушаалуудыг ашиглан амжилттай болсон; LibreSSL (macOS-ийн өгөгдмөл) нь SM2/SM3 дэмжлэггүй хэвээр байгаа тул орон нутгийн зөрүү байна.

## Дараагийн алхамууд

1. `sm2` нь хавсралтын жишээ 1-ийг хүлээн зөвшөөрч буй API-г (эсвэл дээд тал нь муруй параметрүүдийг баталгаажуулсны дараа) илэрмэгц дахин тест хийж, оосорыг дотооддоо нэвтрүүлэх боломжтой.
2. RustCrypto-г засах хүртэл каноник Хавсралтын жишээг хамгаалахын тулд CI дамжуулах хоолойд CLI эрүүл мэндийн шалгалтыг (OpenSSL/Tongsuo/gmssl) байлга.
3. RustCrypto болон OpenSSL-ийн тэнцвэрийн шалгалт амжилттай болсны дараа Iroha-ийн регрессийн багцад бэхэлгээг дэмжээрэй.