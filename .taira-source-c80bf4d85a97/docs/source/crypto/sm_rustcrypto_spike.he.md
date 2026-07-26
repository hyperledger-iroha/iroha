---
lang: he
direction: rtl
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2026-01-03T18:07:57.103009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! הערות עבור ספייק האינטגרציה של RustCrypto SM.

# RustCrypto SM Spike Notes

## מטרה
ודא שהכנסת ארגזי `sm2`, `sm3` ו-`sm4` של RustCrypto (בתוספת `rfc6979`, `ccm`, `sm4` מותאמת לתלות אופציונלית ב-I100NI060) `iroha_crypto` ארגז ומניב זמני בנייה מקובלים לפני חיווט דגל הפיצ'ר לסביבת העבודה הרחבה יותר.

## מפת תלות מוצעת

| ארגז | גרסה מוצעת | תכונות | הערות |
|-------|------------------------|--------|-------|
| `sm2` | `0.13` (RustCrypto/חתימות) | `std` | תלוי ב-`elliptic-curve`; ודא ש-MSRV מתאים למרחב העבודה. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hashes) | ברירת מחדל | API מקביל ל-`sha2`, משתלב עם תכונות `digest` קיימות. |
| `sm4` | `0.5.1` (RustCrypto/בלוק-ciphers) | ברירת מחדל | עובד עם תכונות צופן; עטיפות AEAD נדחו לספייק מאוחר יותר. |
| `rfc6979` | `0.4` | ברירת מחדל | שימוש חוזר לגזירת אי-נונס דטרמיניסטית. |

*גרסאות משקפות מהדורות נוכחיות נכון לשנים 2024-12; אשר עם `cargo search` לפני הנחיתה.*

## שינויים גלויים (טיוטה)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

מעקב: הצמד `elliptic-curve` כדי להתאים לגרסאות שכבר נמצאות ב-`iroha_crypto` (כיום `0.13.8`).

## רשימת ספייק
- [x] הוסף תלות ותכונה אופציונלית ל-`crates/iroha_crypto/Cargo.toml`.
- [x] צור מודול `signature::sm` מאחורי `cfg(feature = "sm")` עם מבני מיקום לאישור החיווט.
- [x] הפעל את `cargo check -p iroha_crypto --features sm` כדי לאשר קומפילציה; שיא זמן בנייה וספירת תלות חדשה (`cargo tree --features sm`).
- [x] אשר את היציבה הרגילה בלבד עם `cargo check -p iroha_crypto --features sm --locked`; `no_std` לא נתמכים עוד.
- [x] תוצאות קובץ (תזמונים, דלתא של עץ התלות) ב-`docs/source/crypto/sm_program.md`.

## תצפיות ללכוד
- זמן קומפילציה נוסף לעומת קו הבסיס.
- השפעה בגודל בינארי (אם ניתן למדידה) עם `cargo builtinsize`.
- כל התנגשות MSRV או תכונה (למשל, עם גרסאות מינוריות של `elliptic-curve`).
- נפלטו אזהרות (קוד לא בטוח, const-fn gating) שעשויות לדרוש תיקונים במעלה הזרם.

## פריטים ממתינים
- המתן לאישור Crypto WG לפני ניפוח גרף התלות של סביבת העבודה.
- אשר אם לספק ארגזים לבדיקה או להסתמך על crates.io (ייתכן שיידרשו מראות).
- תאמו את רענון `Cargo.lock` לכל `sm_lock_refresh_plan.md` לפני סימון רשימת התיוג הושלמה.
- השתמש ב-`scripts/sm_lock_refresh.sh` לאחר מתן אישור לחידוש קובץ הנעילה ועץ התלות.

## 2025-01-19 ספייק יומן
- נוספו תלות אופציונלית (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) ודגל תכונה `sm` ב-`iroha_crypto`.
- מודול `signature::sm` מעוצב להפעלת ממשקי API של גיבוש/חסימה של צופן במהלך ההידור.
- `cargo check -p iroha_crypto --features sm --locked` פותר כעת את גרף התלות אך מבטל עם דרישת העדכון של `Cargo.lock`; מדיניות המאגר אוסרת עריכות של קבצי נעילה, כך שהפעלת ההידור נשארת בהמתנה עד שנתאם רענון מותר לנעילה.## 2026-02-12 ספייק יומן
- פתר את חוסם קבצי הנעילה הקודם - התלות כבר נלכדה - כך ש-`cargo check -p iroha_crypto --features sm --locked` מצליח (בנייה קרה 7.9s ב-dev Mac; הפעלה מחדש מצטברת של 0.23s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` עובר ב-1.0 שניות, מה שמאשר את התכונה האופציונלית קומפילציה בתצורות `std` בלבד (לא נשאר נתיב `no_std`).
- דלתא תלות עם תכונת `sm` מופעלת מציגה 11 ארגזים: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` `primeorder`, `sm2`, `sm3`, `sm4`, ו-`sm4-gcm`. (`rfc6979` כבר היה חלק מגרף הבסיס.)
- אזהרות בנייה נמשכות עבור עוזרי מדיניות NEON שאינם בשימוש; השאר כפי שהוא עד שזמן הריצה של החלקת המדידה יאפשר מחדש את נתיבי הקוד הללו.