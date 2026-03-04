<!-- Hebrew translation of docs/source/examples/lookup_grand_product.md -->

---
lang: he
direction: rtl
source: docs/source/examples/lookup_grand_product.md
status: complete
translator: manual
---

<div dir="rtl">

# דוגמת מוצר גראנד ל-lookup

הדוגמה מרחיבה את ארגומנט ה-lookup של FASTPQ כפי שממומש בשלב 2. הפלובר מעריך את עמודות `s_perm` ו-`perm_hash` על דומיין ה-LDE, מעדכן מוצר גראנד מצטבר `Z_i`, ואז מבצע Poseidon על הסדרה `[Z_1, Z_2, …]` ומוסיף אותה לטרנסקריפט תחת הדומיין `fastpq:v1:lookup:product`. במקביל הוא ממשיך להשוות את `Z_i` הסופי למוצר הטבלה הקנוני `T`.

בbatch קטן עם הערכים הבאים:

| שורה | `s_perm` | `perm_hash` |
| --- | --- | --- |
| 0 | 1 | `0x019a...` (הענקת תפקיד auditor, הרשאה transfer_asset) |
| 1 | 0 | `0xabcd...` (אין שינוי בהרשאות) |
| 2 | 1 | `0x42ff...` (שלילת auditor, הרשאה burn_asset) |

נתון את האתגר `gamma = 0xdead...`. הפלובר מאתחל `Z_0 = 1` ומחשב:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1)  // selector=0 ⇒ אין שינוי
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

שורות עם `s_perm = 0` אינן משנות את המצטבר. בתום העיבוד הפלובר מפיק hash של `[Z_1, Z_2, …]` לטרנסקריפט וגם שומר `Z_final = Z_3` לקיום תנאי הגבול מול הטבלה.

בצד הטבלה, עץ המרקל המתחייב מחזיק את קבוצת ההרשאות. מחושב:

```
T = Π (entry.hash + gamma)
```

התנאי `Z_final / T = 1` מוכתב. אם ה-TRACE הכניס הרשאה לא קנונית או החסיר אחת קיימת, היחס יתסטה והווריפייר ידחה. מאחר ששני הצדדים מכפילים ב-`(value + gamma)` במודולוס Goldilocks, התוצאה דטרמיניסטית גם על CPU וגם על GPU.

לייצוא JSON ב-Norito:

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

ניתן להחליף את ההקסות בערכי Goldilocks ממשיים בבדיקות. פיקסצ'רי שלב 2 מוסיפים גם את ה-hash של סדרת המצטבר, אך מבנה ה-JSON נשאר זהה כך שהדוגמה משמשת כתבנית.

</div>
