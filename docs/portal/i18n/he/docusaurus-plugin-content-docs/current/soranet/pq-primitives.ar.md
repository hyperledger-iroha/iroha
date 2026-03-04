---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitives
כותרת: פרימיטיבים פוסט-קוונטיים ב- SoraNet
sidebar_label: PQ primitives
תיאור: סקירה כללית של ארגז `soranet_pq` וכיצד לחיצת היד של SoraNet צורך עזרי ML-KEM/ML-DSA.
---

:::הערה מקור סטנדרטי
דף זה משקף את `docs/source/soranet/pq_primitives.md`. שמור את שני העותקים זהים עד להוצאת מערכת המסמכים הישנה.
:::

ארגז `soranet_pq` מכיל את אבני הבניין הפוסט-קוונטיות שכל ממסר, לקוח וכלי עבודה ב- SoraNet תלויים בהם. הוא עוטף את חפיסות ה-Kyber (ML-KEM) וה-Dilithium (ML-DSA) הנתמכות על ידי PQClean ומוסיף עוזרים עבור HKDF ו-RNG המגודרים לפרוטוקול כך שכל העורים חולקים את אותם יישומים.

## מה כלול ב-`soranet_pq`

- **ML-KEM-512/768/1024:** עוזרי יצירת מפתח דטרמיניסטים, אנקפסולציה ו-Decapsulation עם הפצת שגיאות בזמן קבוע.
- **ML-DSA-44/65/87:** חתימה/אימות נפרדים המקושרים לסקריפטים מופרדים בהיקף.
- **תג HKDF:** `derive_labeled_hkdf` מוסיף מרחב שמות לכל גזירה עם שלב לחיצת היד (`DH/es`, `KEM/1`, ...) כך שמיתרים היברידיים יישארו נקיים מהתנגשות.
- **אקראיות מגודרת:** `hedged_chacha20_rng` מערבבת זרעים דטרמיניסטיים עם האנטרופיה של המערכת ומאפס את מצב הביניים עם השחרור.

כל הסודות חיים בתוך מיכלי `Zeroizing` ו-CI בדיקות PQClean bindings בכל הפלטפורמות הנתמכות.

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

## כיצד להשתמש

1. **הוסף אישורים** לארגזים מחוץ לשורש סביבת העבודה:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **בחר את הקבוצה הנכונה** במוקדי הקריאה. לעבודה ראשונית על לחיצת היד ההיברידית, השתמש ב-`MlKemSuite::MlKem768` ו-`MlDsaSuite::MlDsa65`.

3. **הפקת מפתחות עם תגים.** השתמש ב-`HkdfDomain::soranet("KEM/1")` (ובמקביליו) כך שרצפי טקסט יישארו דטרמיניסטים על פני צמתים.

4. **השתמש ב-RNG hedged** בעת דגימת סודות חלופיים:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

לחיצת היד של הליבה של SoraNet והצפנת CID (`iroha_crypto::soranet`) מושכים אותם ישירות, כלומר ארגזים ילדים יורשים את אותם יישומים מבלי לחייב את PQClean עצמם.

## רשימת בדיקה לאימות

- `cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- ראה דוגמאות שימוש ב-README (`crates/soranet_pq/README.md`)
- עדכן את מסמך עיצוב לחיצת היד של SoraNet כאשר הגיעו היברידיות