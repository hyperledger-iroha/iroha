---
lang: ur
direction: rtl
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Rustcrypto کریٹس کا استعمال کرتے ہوئے SM2 انیکس ڈی ویکٹر کی تصدیق کرنے پر نوٹس۔

# SM2 ضمیمہ ڈی ویکٹر کی توثیق (Rustcrypto)

یہ واک تھرو ان اقدامات کو اپنی گرفت میں لے رہا ہے جو ہم GM/T 0003 ضمیمہ D مثال کے ساتھ Rustcrypto کے `sm2` کریٹ کی توثیق (اور ڈیبگ) کے لئے استعمال کرتے ہیں۔ کیننیکل ضمیمہ مثال 1 ڈیٹا (شناخت `ALICE123@YAHOO.COM` ، پیغام `"message digest"` ، اور شائع شدہ `(r, s)`) اب `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` میں ریکارڈ کیا گیا ہے۔ اوپن ایس ایل/ٹونگسو/جی ایم ایس ایس ایل خوشی سے دستخط کی تصدیق کریں (`sm_vectors.md` دیکھیں) ، لیکن روسٹ کریپٹو کا `sm2 v0.13.3` اب بھی `signature::Error` کے ساتھ اس نقطہ کو مسترد کرتا ہے ، لہذا CLI کی برابری کی تصدیق کی گئی ہے جبکہ رمجین کی طاقت ایک اپ اسٹریم ٹھیک ہے۔

## عارضی کریٹ

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

## نتائج

- کیننیکل ضمیمہ کی تصدیق 1 `(r, s)` فی الحال ناکام ہوجاتا ہے کیونکہ `sm2::VerifyingKey::from_sec1_bytes` `signature::Error` کو لوٹاتا ہے۔ اپ اسٹریم/جڑ کی وجہ کو ٹریک کریں (ممکنہ طور پر کریٹ کی موجودہ ریلیز میں وکر پیرامیٹر سے مماثلت کی وجہ سے)۔
- کنٹرول `sm2 v0.13.3` کے ساتھ صاف ستھرا مرتب کرتا ہے اور ایک بار جب روسٹ کریپٹو (یا ایک پیچ والا کانٹا) ضمیمہ کی مثال 1 پوائنٹ/دستخطی جوڑی کو قبول کرتا ہے تو ایک خودکار ریگریشن ٹیسٹ بن جائے گا۔
- اوپن ایس ایل/ٹونگسو/جی ایم ایس ایس ایل کی توثیق `sm_vectors.md` میں کمانڈز کے ساتھ کامیاب ہوتی ہے۔ لائبریسل (میک او ایس ڈیفالٹ) میں اب بھی ایس ایم 2/ایس ایم 3 سپورٹ کا فقدان ہے ، لہذا مقامی فرق ہے۔

## اگلے اقدامات

1. دوبارہ ٹیسٹ ایک بار `sm2` ایک API کو بے نقاب کرتا ہے جو ضمیمہ مثال کے طور پر 1 پوائنٹ (یا اپ اسٹریم کے بعد وکر پیرامیٹرز کی تصدیق کرتا ہے) کو قبول کرتا ہے تاکہ مقامی طور پر استعمال ہوسکے۔
2. سی آئی پائپ لائنوں میں سی آئی پی سی کو ٹھیک کریں (اوپن ایس ایس ایل/ٹونگسو/جی ایم ایس ایس ایل) سی آئی پائپ لائنوں میں کیننیکل ضمیمہ کی مثال کے طور پر جب تک رسٹ کریپٹو کو ٹھیک نہیں کیا جاتا ہے۔
3. روسٹ کریپٹو اور اوپن ایس ایل پیریٹی چیک دونوں کے کامیاب ہونے کے بعد Iroha کے ریگریشن سویٹ میں استعمال کو فروغ دیں۔