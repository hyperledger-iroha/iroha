---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/pq-primitives.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cc4055613fa90b02d2fbc9b11dbd5a5a50d3b40af3ce349a5213fa80595240a4
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: pq-primitives
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
דף זה משקף את `docs/source/soranet/pq_primitives.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של התיעוד יופסק.
:::

ה-crate `soranet_pq` מכיל את אבני הבניין הפוסט-קוונטיות שעליהן נשענים כל relay, client ורכיבי tooling של SoraNet. הוא עוטף את הסוויטות Kyber (ML-KEM) ו-Dilithium (ML-DSA) הנתמכות ב-PQClean ומוסיף helpers של HKDF ו-RNG hedged ידידותיים לפרוטוקול, כך שכל המשטחים חולקים מימושים זהים.

## מה מגיע בתוך `soranet_pq`

- **ML-KEM-512/768/1024:** יצירת מפתחות דטרמיניסטית, helpers של encapsulation ו-decapsulation עם הפצת שגיאות בזמן קבוע.
- **ML-DSA-44/65/87:** חתימה/אימות מנותקים המחוברים לתמלילים מופרדי תחום.
- **HKDF מתויג:** `derive_labeled_hkdf` מוסיף namespace לכל גזירה לפי שלב ה-handshake (`DH/es`, `KEM/1`, ...) כדי שתמלילים היברידיים יישארו ללא התנגשות.
- **אקראיות hedged:** `hedged_chacha20_rng` משלב זרעים דטרמיניסטיים עם אנטרופיית מערכת ומאפס מצב ביניים בעת שחרור.

כל הסודות מאוחסנים במכולות `Zeroizing`, ו-CI מפעילה את bindings של PQClean בכל הפלטפורמות הנתמכות.

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## איך משתמשים בזה

1. **הוסיפו תלות** ל-crates שנמצאים מחוץ לשורש ה-workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **בחרו את הסוויטה הנכונה** בנקודות הקריאה. לעבודת ה-handshake ההיברידית הראשונית השתמשו ב-`MlKemSuite::MlKem768` וב-`MlDsaSuite::MlDsa65`.

3. **גזרו מפתחות עם תוויות.** השתמשו ב-`HkdfDomain::soranet("KEM/1")` (ועוד דומים) כדי ששרשור התמלילים יישאר דטרמיניסטי בין nodes.

4. **השתמשו ב-RNG hedged** בעת דגימת סודות חלופיים:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

ה-handshake הליבתי של SoraNet ועזרי ה-CID blinding (`iroha_crypto::soranet`) משתמשים בכלים האלה ישירות, כך ש-crates downstream יורשים את אותם מימושים בלי לקשר bindings של PQClean בעצמם.

## Checklist לאימות

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- בדקו את דוגמאות השימוש ב-README (`crates/soranet_pq/README.md`)
- עדכנו את מסמך עיצוב ה-handshake של SoraNet לאחר שה-hybrids נוחתים
