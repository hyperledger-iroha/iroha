---
lang: he
direction: rtl
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T15:55:55+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito תחילת העבודה

מדריך מהיר זה מציג את זרימת העבודה המינימלית עבור קומפילציה של חוזה Kotodama,
בודקים את קוד הביטים Norito שנוצר, מפעיל אותו באופן מקומי ופריסה שלו
לצומת Iroha.

## דרישות מוקדמות

1. התקן את שרשרת הכלים Rust (1.76 ומעלה) ובדוק את המאגר הזה.
2. בנה או הורד את הקבצים הבינאריים התומכים:
   - `koto_compile` - מהדר Kotodama שפולט קוד בתים IVM/Norito
   - `ivm_run` ו-`ivm_tool` - כלי עזר מקומיים לביצוע ובדיקה
   - `iroha_cli` - משמש לפריסת חוזה דרך Torii

   המאגר Makefile מצפה לקבצים בינאריים אלה ב-`PATH`. אתה יכול גם
   הורד פריטים שנבנו מראש או בנה אותם ממקור. אם תרכיב את ה
   צרור הכלים באופן מקומי, כוון את עוזרי Makefile אל הקבצים הבינאריים:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. ודא שצומת Iroha פועל כאשר אתה מגיע לשלב הפריסה. ה
   הדוגמאות שלהלן מניחות ש-Torii נגיש בכתובת האתר שהוגדרה ב-
   פרופיל `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. הרכיב חוזה Kotodama

המאגר שולח חוזה מינימלי של "שלום עולם".
`examples/hello/hello.ko`. הידור אותו ל-Norito/IVM bytecode (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

דגלי מפתח:

- `--abi 1` נועל את החוזה לגרסה 1 של ABI (הגרסה הנתמכת היחידה בכתובת
  זמן הכתיבה).
- `--max-cycles 0` מבקש ביצוע בלתי מוגבל; הגדר מספר חיובי לתחום
  ריפוד מחזור להוכחות אפס ידע.

## 2. בדוק את חפץ Norito (אופציונלי)

השתמש ב-`ivm_tool` כדי לאמת את הכותרת והמטא נתונים המוטבעים:

```sh
ivm_tool inspect target/examples/hello.to
```

אתה אמור לראות את גרסת ה-ABI, דגלי תכונה מופעלים ואת הערך המיוצא
נקודות. זוהי בדיקת שפיות מהירה לפני הפריסה.

## 3. הפעל את החוזה באופן מקומי

בצע את קוד הבתים עם `ivm_run` כדי לאשר התנהגות מבלי לגעת ב-
צומת:

```sh
ivm_run target/examples/hello.to --args '{}'
```

הדוגמה של `hello` מתעדת ברכה ומנפיקה `SET_ACCOUNT_DETAIL` מערכת הפעלה.
הפעלה מקומית שימושית תוך איטרציה על לוגיקה של חוזה לפני הפרסום
זה בשרשרת.

## 4. פריס דרך `iroha_cli`

כאשר אתה מרוצה מהחוזה, פרוס אותו לצומת באמצעות ה-CLI.
ספק חשבון רשות, מפתח החתימה שלו וקובץ `.to` או
מטען Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

הפקודה שולחת מניפסט Norito + חבילת קוד בתים מעל Torii ומדפיסה
מצב העסקה שנוצר. לאחר ביצוע העסקה, הקוד
ניתן להשתמש ב-hash המוצג בתגובה כדי לאחזר מניפסטים או רשימה של מופעים:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. הפעל נגד Torii

עם קוד הבתים רשום, אתה יכול להפעיל אותו על ידי שליחת הוראה
שמפנה לקוד המאוחסן (לדוגמה, דרך `iroha_cli ledger transaction submit`
או לקוח האפליקציה שלך). ודא שהרשאות החשבון מאפשרות את הרצוי
syscalls (`set_account_detail`, `transfer_asset` וכו').

## טיפים ופתרון בעיות- השתמש ב-`make examples-run` כדי לקמפל ולהפעיל את הדוגמאות שסופקו באחת
  ירייה. עקוף משתני סביבה `KOTO`/`IVM` אם הקבצים הבינאריים אינם פועלים
  `PATH`.
- אם `koto_compile` דוחה את גרסת ה-ABI, ודא שהמהדר והצומת
  שניהם יעד ABI v1 (הפעל `koto_compile --abi` ללא ארגומנטים לרשימה
  תמיכה).
- ה-CLI מקבל מפתחות חתימה hex או Base64. לבדיקה, אתה יכול להשתמש
  מפתחות הנפלטים על ידי `iroha_cli tools crypto keypair`.
- בעת איתור באגים של Norito, פקודת המשנה `ivm_tool disassemble` עוזרת
  מתאם הוראות עם מקור Kotodama.

זרימה זו משקפת את השלבים המשמשים ב-CI ובמבחני האינטגרציה. לעומק יותר
לצלול לתוך דקדוק Kotodama, מיפויי מערכות הפעלה ומידע פנימי Norito, ראה:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`