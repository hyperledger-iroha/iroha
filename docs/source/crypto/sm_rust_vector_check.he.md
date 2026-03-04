---
lang: he
direction: rtl
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! הערות על אימות וקטורים של נספח D של SM2 באמצעות ארגזי RustCrypto.

# SM2 נספח D וקטור אימות (RustCrypto)

הדרכה זו לוכדת את השלבים בהם השתמשנו כדי לאמת (ולניפוי באגים) של הדוגמה של GM/T 0003 נספח D עם ארגז `sm2` של RustCrypto. נתוני הנספח הקנוני לדוגמה 1 (זהות `ALICE123@YAHOO.COM`, הודעה `"message digest"` וה-`(r, s)` שפורסם) מתועדים כעת ב-`crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl מאמתים בשמחה את החתימה (ראה `sm_vectors.md`), אך ה-`sm2 v0.13.3` של RustCrypto עדיין דוחה את הנקודה עם `signature::Error`, כך ששוויון ה-CLI מאושרת בזמן שרתום ה-Rust במעלה הזרם נותרה בהמתנה.

## ארגז זמני

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

## ממצאים

- אימות מול הנספח הקנוני לדוגמה 1 `(r, s)` נכשל כעת מכיוון ש-`sm2::VerifyingKey::from_sec1_bytes` מחזיר `signature::Error`; עקוב אחר גורם השורש/הזרם (כנראה בגלל חוסר התאמה של פרמטרים של עקומה בשחרור הנוכחי של הארגז).
- הרתמה מתהדרת בצורה נקייה עם `sm2 v0.13.3` ותהפוך למבחן רגרסיה אוטומטי ברגע ש-RustCrypto (או מזלג טלאי) יקבל את צמד נקודה/חתימה של נספח 1.
- אימות OpenSSL/Tongsuo/gmssl מצליח עם הפקודות ב-`sm_vectors.md`; LibreSSL (ברירת המחדל של macOS) עדיין חסרה תמיכה ב-SM2/SM3, ומכאן הפער המקומי.

## השלבים הבאים

1. בדוק שוב ברגע ש-`sm2` חושף API שמקבל את נקודת הנספח דוגמה 1 (או לאחר אישור במעלה הזרם את פרמטרי העקומה), כך שהרתמה תוכל לעבור באופן מקומי.
2. שמור על בדיקת שפיות CLI (OpenSSL/Tongsuo/gmssl) בצינורות CI כדי לשמור על דוגמה הנספח הקנונית עד שהתיקון RustCrypto נוחת.
3. קדם את הרתמה לחבילת הרגרסיה של Iroha לאחר שבדיקות השוויון של RustCrypto ו-OpenSSL יצליחו.