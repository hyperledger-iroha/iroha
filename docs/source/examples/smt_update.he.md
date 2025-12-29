<!-- Hebrew translation of docs/source/examples/smt_update.md -->

---
lang: he
direction: rtl
source: docs/source/examples/smt_update.md
status: complete
translator: manual
---

<div dir="rtl">

# דוגמת עדכון Merkle דליל

הדוגמה מראה כיצד TRACE של FASTPQ בשלב 2 מקודד עדות אי-חברות באמצעות העמודה `neighbour_leaf`. העץ הוא Sparse Merkle בינארי מעל שדה Poseidon2; מפתחות מומרות למחרוזות LE באורך 32 בתים, מהושים לשדה והביט הגבוה בכל רמה קובע את הענף.

## תרחיש
- עלי מצב-קודם:
  - `asset::alice::rose` → hash `0x12b7...`, ערך `0x0000_...5`
  - `asset::bob::rose`   → hash `0x1321...`, ערך `0x0000_...3`
- בקשה: להכניס `asset::carol::rose` בערך 2.
- Hash של Carol מייצר prefix `0b01011`; שכנים קיימים: `0b01010` (Alice), `0b01101` (Bob).

לא קיימת עלה עם prefix `0b01011`, לכן יש צורך להוכיח שהטווח `(alice, bob)` ריק. בשלב 2 השורה מפוזרת לעמודות `path_bit_{level}`, ‏`sibling_{level}`, ‏`node_in_{level}`, ‏`node_out_{level}` (עבור `level = 0..31`). כל הערכים מוצגים כ-LE של Poseidon2:

| level | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | הערות |
| --- | --- | --- | --- | --- | --- |
| 0 | 1 | `0x241f...` (עלה Alice) | `0x0000...` | `0x4b12...` (ערך 2) | Insert: התחלה מ-0 והכנסת ערך |
| 1 | 1 | `0x7d45...` (ימין ריק) | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | ממשיכים עם ביט 1 |
| 2 | 0 | `0x03ae...` (ענף Bob) | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`) | הביט 0 מחליף סדר |
| 3 | 1 | `0x9bc4...` | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | העלאה לרמות גבוהות |
| 4 | 0 | `0xe112...` | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`) | רמת שורש (שורש חדש) |

`neighbour_leaf` מכיל את עלה Bob (`key = 0x1321...`, ‏`value = 3`, ‏`hash = Poseidon2(key, value) = 0x03ae...`). ה-AIR מאמת:
1. שה- neighbour תואם לסיבלי ברמה 2.
2. שהמפתח שלו גדול מן המפתח המוכנס ושל Alice קטן ממנו.
3. שהחלפת העלה המוכנס ב-neighbour משחזרת את השורש הישן.

כך מוכיחים שלא הייתה עלה בטווח `(0b01010, 0b01101)` לפני העדכון. ניתן להשתמש בלייאאוט כפי שהוא; המספרים להמחשה. להפקת עדות JSON מלאה, יש להוציא את העמודות בדיוק כפי שבטבלה (כולל הסיומות ברמת `level`) באמצעות סדרת ה-JSON של Norito.

</div>
