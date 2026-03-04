<!-- Hebrew translation of docs/source/zk_envelopes.md -->

---
lang: he
direction: rtl
source: docs/source/zk_envelopes.md
status: complete
translator: manual
---

<div dir="rtl">

# מעטפות ZK (Norito)

המסמך מגדיר מעטפות מקודדות Norito שהמאמתים הילידיים של Iroha 2 משתמשים בהן. מעטפות הן בגרסה מפורשת, דטרמיניסטיות, וניתנות להעברה בין רכיבים (לקוחות, IVM, צומת).

טווח (נוכחי)
- IPA (שקוף, ללא trusted setup): הוכחות פתיחת פולינום עבור זרימות בסגנון Halo2 באמצעות מאמת Native. סוג המעטפה: `OpenVerifyEnvelope`.
- STARK (FRI): הוכחות עקביות רב-קיפול על דומיין בגודל 2^k עם מחויבויות Merkle ב-SHA-256 וטרנסקריפט דטרמיניסטי.

תגי Backend
- IPA (Pallas, native): ‏`halo2/ipa-v1/poly-open`
- IPA (BN254): ‏`halo2/ipa/ipa-v1/poly-open`
- IPA (Goldilocks): ‏`halo2/goldilocks-ipa-v1/poly-open`
- STARK (native): ‏`stark/fri-v1/<profile>` (לדוגמה, ‏`stark/fri-v1/sha256-goldilocks-v1`)

הערות כלליות
- Norito משמש לקידוד המעטפות והמטענים הפנימיים. אלא אם צוין אחרת, סקלרים הם ליטל אנדיאן לפי טיפוס המבנה.
- דטרמיניזם: אתגרים נגזרים מתוויות טרנסקריפט קבועות; STARK משתמש ב-SHA-256, ‏IPA ב-SHA3.
- מגבלות גודל ואימות: יש להגדיר גבולות עבור וקטורים ולדחות מטענים פגומים מוקדם (ראו קוד להגבלות עדכניות).

## IPA — מעטפת פתיחת פולינום

טיפוסי Wire ב-`crates/iroha_zkp_halo2`

- `IpaParams`
  - `version: u16`
  - `curve_id: u16` (`1 = Pallas`, ‏`2 = Goldilocks`)
  - `n: u32`
  - `g`, ‏`h`: וקטורי נקודות דחוסות `[u8;32]`
  - `u`: נקודת גנרטור דחוסה `[u8;32]`

- `IpaProofData`
  - `version: u16`
  - `l`, ‏`r`: וקטורי Commitments לכל סבב
  - `a_final`, ‏`b_final`: סקלרים סופיים

- `PolyOpenPublic`
  - `version`, ‏`curve_id`, ‏`n`
  - `z`: נקודת הערכה
  - `t`: f(z) נטען
  - `p_g`: קומיטמנט ל-coeffs

- `OpenVerifyEnvelope`
  - פרמטרים, נתונים ציבוריים, הוכחה, ותווית טרנסקריפט משותפת.

מאמת IPA:
- משחזר את וקטור b = [1, z, …, z^{n-1}].
- מריץ מחדש סבבי טרנסקריפט כדי לקפל את הגנרטורים ולעדכן Q.
- בודק שהיחס הסופי מתקיים עם `(a_final, b_final)`.
- טרנסקריפט מעל SHA3-256 עם DST מוגדר.
- פונקציות batch מאפשרות אימות מקבילי או סידרתי.

דוגמה Rust:
```rust
use iroha_zkp_halo2::{Params, Polynomial, PrimeField64, Transcript, norito_helpers as nh};
let n = 8;
let params = Params::new(n).unwrap();
let coeffs = (0..n).map(|i| PrimeField64::from((i+1) as u64)).collect();
let poly = Polynomial::from_coeffs(coeffs);
let p_g = poly.commit(&params).unwrap();
let z = PrimeField64::from(3u64);
let mut tr = Transcript::new(\"IROHA-TEST-IPA\");
let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
let env = iroha_zkp_halo2::OpenVerifyEnvelope {
    params: nh::params_to_wire(&params),
    public: nh::poly_open_public::<iroha_zkp_halo2::backend::pallas::PallasBackend>(n, z, t, p_g),
    proof: nh::proof_to_wire(&proof),
    transcript_label: \"IROHA-TEST-IPA\".into(),
};
let bytes = norito::to_bytes(&env).unwrap();
```

## STARK — מעטפת מרובת קפולים בסגנון FRI

Hash וטרנסקריפט:
- Leaves: ‏`LEAF || u64_le(value)` עם SHA-256.
- פנימי: ‏`SHA256(left || right)`.
- אתגר בכל שכבה: ‏`r_k = H(label || version || n_log2 || root_k)`.
- השדה: Goldilocks `p = 2^64 - 2^32 + 1`.

טיפוסים ב-`iroha_core::zk_stark`:
- `StarkFriParamsV1` (גרסה, ‏`n_log2`)
- `MerklePath` (כיוונים וסיבילינגים)
- `StarkCommitmentsV1` (שורשים, ‏`comp_root`)
- `FoldDecommitV1` (אינדקס, ערכים, מסלולי Merkle)
- `StarkProofV1` (commitments, queries, comp_values)
- `StarkCompositionValueV1`, ‏`StarkCompositionTermV1`
- `StarkVerifyEnvelopeV1` (פרמטרים, הוכחה, תווית)

מאמת STARK:
- לכל שאילתה, משחזר קפולים, בודק מסלולי Merkle ויחס `z == y0 + r_k*y1`.
- אם יש comp_root, מאמת עלה הרכב ומוודא `constant + z_coeff*z_final + Σ coeff_i*value_i`.

דוגמת Rust:
```rust
use iroha_core::zk_stark::*;
let n_log2 = 3u8;
let env = StarkVerifyEnvelopeV1 { /* ... */ };
let bytes = norito::to_bytes(&env).unwrap();
```

</div>
