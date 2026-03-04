<!-- Hebrew translation of docs/dependency_audit.md -->

---
lang: he
direction: rtl
source: docs/dependency_audit.md
status: complete
translator: manual
---

<div dir="rtl">

//! סיכום ביקורת תלותים

תאריך: 2025-09-01

היקף: סקירה של כל הקרייטים המוגדרים בקבצי Cargo.toml ונפתרים בתוך Cargo.lock ברחבי המרחב. הבדיקה התבססה על cargo-audit מול מסד העדכונים של RustSec ובוצעה לצד בחינה ידנית של לגיטימציה ובחירת "קרייט עיקרי" לכל אלגוריתם.

כלים/פקודות שהורצו:
- `cargo tree -d --workspace --locked --offline` — בחינת גרסאות כפולות
- `cargo audit` — סריקת Cargo.lock אחר פגיעויות ידועות וקרייטים משוכים

אזהרות אבטחה שנמצאו (כעת: 0 פגיעויות; 2 אזהרות):
- crossbeam-channel — RUSTSEC-2025-0024
  - תוקן: שודרג ל־`0.5.15` בתוך `crates/ivm/Cargo.toml`.

  - תוקן: הוחלף ל־`prost-codec` תחת `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC-2025-0009
  - תוקן: עודכנו מחסנית ה-QUIC/TLS (`quinn 0.11`, ‏`rustls 0.23`, ‏`tokio-rustls 0.26`) והמחסנית של WS (`tungstenite/tokio-tungstenite 0.24`). נועל ל־`ring 0.17.12` באמצעות `cargo update -p ring --precise 0.17.12`.

אזהרות שנותרו: אין. התראות שנשארו: `backoff` (ללא תחזוקה), ‏`derivative` (ללא תחזוקה).

הערכת לגיטימציה ו"קרייט עיקרי" (עיקרי):
- גיבוב: ‏`sha2` (RustCrypto), ‏`blake2` (RustCrypto), ‏`tiny-keccak` (שימוש רחב) — בחירות תקניות.
- AEAD/סימטרי: ‏`aes-gcm`, ‏`chacha20poly1305`, ‏`aead` traits (RustCrypto) — בחירות קנוניות.
- חתימות/ECC: ‏`ed25519-dalek`, ‏`x25519-dalek` (פרויקט dalek), ‏`k256` (RustCrypto), ‏`secp256k1` (קישוריות libsecp) — כולם לגיטימיים; מומלץ לצמצם לשכבה אחת (`k256` או `secp256k1`) כשאפשר כדי להפחית שטח קוד כפול.
- BLS12-381/ZK: ‏`blstrs`, ‏`halo2_*` — מקובלים בשימוש בייצור בעולם ה-ZK.
- PQ: ‏`pqcrypto-dilithium`, ‏`pqcrypto-traits` — קרייטים אמינים כייחוס.
- TLS: ‏`rustls`, ‏`tokio-rustls`, ‏`hyper-rustls` — מחסנית TLS מודרנית ונפוצה ב-Rust.
- Noise: ‏`snow` — מימוש תקני.
- סריאליזציה: ‏`parity-scale-codec` הוא הסטנדרט ל-SCALE. Serde הוסרה מתלותי הייצור ברחבי המרחב; מחוללי/כותבי Norito מכסים את כל מסלולי הריצה. שאר ההפניות ל-Serde נשארות בתיעוד היסטורי, תסריטי הגנה או רשימות-הלבן של בדיקות בלבד.
- FFI/ספריות: ‏`libsodium-sys-stable`, ‏`openssl` — קרייטים לגיטימיים; בהפקה מעדיפים Rustls על פני OpenSSL (והקוד הנוכחי כבר עומד בכך).
- ‏`pprof` 0.13.0 (crates.io) — משתמשים בגרסה הרשמית עם `prost-codec` ו-frame-pointer לאחר שהתקלה תוקנה.

המלצות:
- טיפול באזהרות:
  - לשקול החלפת `backoff` ב-`retry`/`futures-retry` או במימוש backoff מעריכי מקומי.
  - להחליף נגזרות `derivative` במימושים ידניים או ב-`derive_more` היכן שניתן.
- בעדיפות בינונית: לאחד סביב `k256` או `secp256k1` על מנת לצמצם כפילויות (להשאיר את שתיהן רק כשיש צורך ממשי).
- בעדיפות בינונית: לבדוק מחדש את מקור `poseidon-primitives 0.2.0` בשימוש ה-ZK; אם ניתן, ליישר לקו אחד עם מימוש Poseidon של Arkworks/Halo2 כדי לצמצם אקוסיסטמות מקבילות.

הערות:
- `cargo tree -d` מציג כפילויות גרסה צפויות (`bitflags` גרסה 1/2, מספר מופעי `ring`), שאינן סיכון אבטחתי בפני עצמן אך מרחיבות את שטח הבנייה.
- לא נמצאו קרייטים החשודים כטיפוסקוואט; כל השמות והמקורות מפנים לקרייטים מוכרים בקהילה או לחברים פנים-ארגוניים במרחב.
- ניסיוני: נוספה אפשרות `bls-backend-blstrs` ל-`iroha_crypto` כדי לפתוח מעבר ל-backend יחיד של blstrs עבור BLS (מסירה את תלות Arkworks כאשר היא פעילה). ברירת המחדל נשארת `w3f-bls` כדי להימנע משינויי התנהגות/קידוד. תכנית היישור: לנרמל את קידוד המפתחות הפרטיים ל-32 בתים בליטל־אנדיאן, לעטוף את כיווץ המפתחות הציבוריים סביב `blstrs::G1Affine::to_compressed` עם בדיקת תאימות מול הקידוד של w3f, ולהוסיף בדיקות round-trip ב-`crates/iroha_crypto/tests/bls_backend_compat.rs` שמבטיחות זהות בין SecretKey/PublicKey/חתימות בשני ה-backends לפני שמעבירים את blstrs לברירת המחדל.

מעקבים (משימות מוצעות):
- להשאיר את מנגנוני ההגנה על Serde ב-CI (`scripts/check_no_direct_serde.sh`, ‏`scripts/deny_serde_json.sh`) כדי למנוע הוספת שימושי ייצור חדשים.

בדיקות שבוצעו לביקורת זו:
- הורץ `cargo audit` עם מסד העדכונים העדכני ואומת שכל ארבע האזהרות והעצים התלויים עודכנו.
- בוצע חיפוש אחר הצהרות תלות ישירות של הקרייטים הפגועים כדי למקד את נקודות התיקון.
</div>
