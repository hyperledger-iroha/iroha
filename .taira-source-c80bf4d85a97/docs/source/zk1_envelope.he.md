<!-- Hebrew translation of docs/source/zk1_envelope.md -->

---
lang: he
direction: rtl
source: docs/source/zk1_envelope.md
status: complete
translator: manual
---

<div dir="rtl">

# פורמט מעטפת ZK1 (מכולת הוכחה/מפתח אימות)

מסמך זה מגדיר את מעטפת ZK1 המשמשת בבדיקות ובכלי עזר כדי לשאת הוכחות, מפתחות אימות ומופעים פומביים בצורה שאינה תלויה ב-backend. ZK1 הוא מיכל TLV עם כותרת Magic בת 4 בתים ורצף רשומות מסוג. הפרסרים דטרמיניסטיים ומאכפים גבולות גודל לבטיחות.

## מבנה המכולה

- Magic: ASCII `ZK1\0` (ארבעה בתים).
- לאחר מכן אפס או יותר רשומות TLV:
  - `tag[4]` (ASCII)
  - `len[u32 LE]`
  - `payload[len]`

## TLV מוכרים

- `PROF`: Bytes גולמיים של הוכחה (opaque ל-ZK1). ה-backend מפרש את המטען.
- `IPAK`: פרמטרי Halo2 IPA ל-Pasta (שקופים). המטען הוא `u32 k`, מעריך גודל הדומיין `N = 2^k`. ה-verifier בונה `Params::<EqAffine>::new(k)`.
- `H2VK`: Bytes של מפתח אימות Halo2 עבור המעגל שנבחר.
- `I10P`: בלוק עמודות Instance ל-Pasta Fp. מבנה: `cols[u32] || rows[u32] || rows*cols*32`.

הערות:
- `I10P` משתמש בייצוג שדה קנוני של 32 בתים; ערכים לא קנוניים נדחים.
- מספר עמודות Instance יש לארוז ב-TLV יחיד; בדיקות פשוטות יכולות להשתמש בעמודה אחת.
- ZK1 אינו תלוי backend; יש לצרף תג backend נפרד (למשל `halo2/pasta/tiny-add-v1`, `halo2/pasta/tiny-add-public-v1`, `halo2/pasta/tiny-add-2rows-v1`).
- בדיקות מייצרות הוכחות/מפתחות אימות דטרמיניסטיים עבור `tiny-add-v1`, `tiny-add-public-v1`, ו-`tiny-add-2rows-v1`; עבור מזהי מעגל אחרים משתמשים במטעני דמה אלא אם מספקים בתים אמיתיים של VK/Proof.

## דוגמאות (Rust)

```rust
let mut vk_env = zk1::wrap_start();
zk1::wrap_append_ipa_k(&mut vk_env, 5); // k = 5

let mut prf_env = zk1::wrap_start();
zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
zk1::wrap_append_instances_pasta_fp(&[public_scalar], &mut prf_env);
```

## מקרים שליליים (טסטים)

מפענחי ZK1 והמאמתים דוחים מעטפות פגומות: אי-התאמת backend/tag, בלוקי Instance קטועים, או ערכי שדה לא קנוניים.

</div>
