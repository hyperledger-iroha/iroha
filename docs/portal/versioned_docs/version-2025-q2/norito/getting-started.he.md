---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5cae8fa9d9a69d506d0fc49903e801382041d29f2e9a052321224bd3cb7a72d1
source_last_modified: "2025-11-04T12:24:28.217788+00:00"
translation_last_reviewed: 2026-01-30
---

# תחילת עבודה עם Norito

המדריך הקצר הזה מציג את הזרימה המינימלית לקומפילציה של חוזה Kotodama, בדיקת הבייטקוד Norito שנוצר, הרצה מקומית ופריסה לצומת Iroha.

## דרישות מקדימות

1. התקינו את שרשרת הכלים של Rust (1.76 ומעלה) ובצעו checkout לריפו הזה.
2. בנו או הורידו את הבינארים התומכים:
   - `koto_compile` - קומפיילר Kotodama שמפיק בייטקוד IVM/Norito
   - `ivm_run` ו-`ivm_tool` - כלי הרצה מקומית ובדיקה
   - `iroha_cli` - משמש לפריסת חוזים דרך Torii

   ה-Makefile של הריפו מצפה שהבינארים יהיו על `PATH`. אפשר להוריד ארטיפקטים מוכנים או לבנות מהמקור. אם אתם מקמפלים את ה-toolchain מקומית, הפנו את עזרי ה-Makefile לבינארים:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. ודאו שצומת Iroha פועל כשאתם מגיעים לשלב הפריסה. הדוגמאות למטה מניחות ש-Torii נגיש ב-URL שמוגדר בפרופיל `iroha_cli` שלכם (`~/.config/iroha/cli.toml`).

## 1. קומפילציה של חוזה Kotodama

הריפו כולל חוזה מינימלי "hello world" ב-`examples/hello/hello.ko`. קומפלו אותו לבייטקוד Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

דגלים חשובים:

- `--abi 1` נועל את החוזה לגרסת ABI 1 (הגרסה היחידה הנתמכת בזמן הכתיבה).
- `--max-cycles 0` מבקש הרצה ללא הגבלה; הגדירו מספר חיובי כדי להגביל padding של מחזורים עבור הוכחות zero-knowledge.

## 2. בדיקת ארטיפקט Norito (רשות)

השתמשו ב-`ivm_tool` כדי לבדוק את הכותרת והמטא-דאטה המוטמעים:

```sh
ivm_tool inspect target/examples/hello.to
```

אתם אמורים לראות את גרסת ה-ABI, הדגלים הפעילים ונקודות הכניסה שיוצאו. זו בדיקת sanity מהירה לפני פריסה.

## 3. הרצת החוזה מקומית

הריצו את הבייטקוד עם `ivm_run` כדי לאשר את ההתנהגות בלי לגעת בצומת:

```sh
ivm_run target/examples/hello.to --args '{}'
```

דוגמת `hello` רושמת ברכה ומבצעת syscall בשם `SET_ACCOUNT_DETAIL`. הרצה מקומית שימושית בזמן שאתם משפרים את לוגיקת החוזה לפני פרסום on-chain.

## 4. פריסה דרך `iroha_cli`

כשאתם מרוצים מהחוזה, פרסו אותו לצומת באמצעות CLI. ספקו חשבון סמכות, מפתח החתימה שלו, וקובץ `.to` או payload ב-Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

הפקודה שולחת bundle של manifest Norito + בייטקוד דרך Torii ומדפיסה את סטטוס העסקה. לאחר שהעסקה נחתמת בבלוק, ניתן להשתמש ב-hash הקוד שמופיע בתגובה כדי לשלוף manifests או להציג רשימת instances:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. הרצה מול Torii

לאחר רישום הבייטקוד, אפשר לקרוא לו על ידי שליחת instruction שמפנה לקוד המאוחסן (למשל דרך `iroha_cli ledger transaction submit` או הלקוח האפליקטיבי שלכם). ודאו שהרשאות החשבון מאפשרות את ה-syscalls הרצויים (`set_account_detail`, `transfer_asset`, וכו').

## טיפים ופתרון תקלות

- השתמשו ב-`make examples-run` כדי לקמפל ולהריץ את הדוגמאות במכה אחת. דרסו את משתני הסביבה `KOTO`/`IVM` אם הבינארים אינם ב-`PATH`.
- אם `koto_compile` דוחה את גרסת ה-ABI, בדקו שהקומפיילר והצומת מכוונים ל-ABI v1 (הריצו `koto_compile --abi` ללא ארגומנטים כדי לראות תמיכה).
- ה-CLI מקבל מפתחות חתימה ב-hex או Base64. לצורך בדיקות אפשר להשתמש במפתחות שמופקים ב-`iroha_cli tools crypto keypair`.
- בעת ניפוי תקלות ב-payloads של Norito, תת-פקודת `ivm_tool disassemble` מסייעת לקשור את ההוראות לקוד המקור של Kotodama.

הזרימה הזו משקפת את השלבים שמשמשים ב-CI ובבדיקות אינטגרציה. להעמקה בדקדוק Kotodama, מיפוי syscalls ופרטי Norito, ראו:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
