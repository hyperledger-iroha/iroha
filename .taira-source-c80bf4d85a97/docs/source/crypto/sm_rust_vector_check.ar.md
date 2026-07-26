---
lang: ar
direction: rtl
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ملاحظات حول التحقق من متجهات SM2 Annex D باستخدام صناديق RustCrypto.

# التحقق من المتجهات في الملحق D من SM2 (RustCrypto)

توضح هذه الإرشادات الخطوات التي استخدمناها للتحقق من صحة (وتصحيح الأخطاء) مثال GM/T 0003 Annex D مع صندوق RustCrypto `sm2`. يتم الآن تسجيل بيانات الملحق رقم 1 الأساسية (الهوية `ALICE123@YAHOO.COM` والرسالة `"message digest"` والرقم `(r, s)` المنشور) في `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. يقوم OpenSSL/Tongsuo/gmssl بالتحقق من التوقيع بكل سرور (راجع `sm_vectors.md`)، لكن `sm2 v0.13.3` الخاص بـ RustCrypto لا يزال يرفض النقطة ذات `signature::Error`، لذلك تم تأكيد تكافؤ CLI بينما يظل حزام Rust في انتظار الإصلاح المنبع.

##صندوق مؤقت

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

## النتائج

- فشل التحقق وفقًا لمثال الملحق الأساسي 1 `(r, s)` حاليًا لأن `sm2::VerifyingKey::from_sec1_bytes` يُرجع `signature::Error`؛ تتبع السبب الجذري/المنبع (على الأرجح بسبب عدم تطابق معلمة المنحنى في الإصدار الحالي للصندوق).
- يتم تجميع الأداة بشكل نظيف باستخدام `sm2 v0.13.3` وستصبح بمثابة اختبار انحدار تلقائي بمجرد قبول RustCrypto (أو الشوكة المصححة) زوج النقطة/التوقيع في المثال المرفق رقم 1.
- ينجح التحقق من OpenSSL/Tongsuo/gmssl باستخدام الأوامر الموجودة في `sm_vectors.md`؛ لا يزال LibreSSL (نظام التشغيل MacOS الافتراضي) يفتقر إلى دعم SM2/SM3، ومن هنا جاءت الفجوة المحلية.

## الخطوات التالية

1. أعد الاختبار بمجرد أن يكشف `sm2` عن واجهة برمجة التطبيقات (API) التي تقبل نقطة الملحق 1 للمثال (أو بعد تأكيد المنبع لمعلمات المنحنى) حتى يتمكن الحزام من المرور محليًا.
2. احتفظ بفحص سلامة واجهة سطر الأوامر (OpenSSL/Tongsuo/gmssl) في خطوط أنابيب CI لحماية مثال الملحق الأساسي حتى يتم إصلاح RustCrypto.
3. قم بترقية الأداة إلى مجموعة الانحدار الخاصة بـ Iroha بعد نجاح عمليات التحقق من التكافؤ RustCrypto وOpenSSL.