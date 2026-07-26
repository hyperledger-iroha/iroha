<!-- Hebrew translation of docs/source/crypto/dependency_audits.md -->

---
lang: he
direction: rtl
source: docs/source/crypto/dependency_audits.md
status: complete
translator: manual
---

<div dir="rtl">

# ביקורות תלות קריפטוגרפיות

## Streebog (קרייט `streebog`)

- **גרסה בעץ:** `0.11.0-rc.2` (ממוען תחת `vendor/streebog`, בשימוש כשהפיצ׳ר `gost` פעיל).
- **צרכן:** `crates/iroha_crypto::signature::gost` – DRBG מבוסס HMAC-Streebog + hashing.
- **סטטוס:** גרסת RC בלבד. אין כרגע crate יציב עם ה-API הנדרש, ולכן הקרייט ממוזער בריפו לצורך ביקורת, תוך מעקב אחרי יציאה רשמית.
- **נקודות בדיקה:**
  - `cargo test -p iroha_crypto --features gost` (קובץ `crates/iroha_crypto/tests/gost_wycheproof.rs`) מול Wycheproof ו-TC26.
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost` מודד Ed25519/Secp256k1 וכל עקומות TC26.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost` להשוואת ביצועים מול הבסיס (`--summary-only` ל-CI, `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` לעדכון).
  - `scripts/gost_bench.sh` עוטף את התהליך; `--write-baseline` לעדכון JSON. הרחבה: `docs/source/crypto/gost_performance.md`.
- **הקלות:** הקרייט נקרא רק דרך עטיפה דטרמיניסטית שמאפסת מפתחות; החתימה מגדרת nonces עם אנטרופיית מערכת.
- **צעדים הבאים:** לעקוב אחר שחרור streebog `0.11.x` הרשמי של RustCrypto, ולאחריו לבצע עדכון תלות רגיל: לוודא checksum, לעבור על ההבדלים, לתעד פרובננס ולהסיר את המראה המקומית בריפו.

</div>
