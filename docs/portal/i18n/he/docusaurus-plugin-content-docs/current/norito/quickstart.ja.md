---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d3c69fd324ef227fb25a4304a5cf6675fa51c3e1fcc396cd147849e1e099c88
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: התחלה מהירה של Norito
description: בנו, אמתו ופרסו חוזה Kotodama עם כלי ה-release והרשת ברירת המחדל של צומת יחיד.
slug: /norito/quickstart
---

המדריך הזה משקף את הזרימה שאנו מצפים שמפתחים יבצעו כאשר הם לומדים Norito ו-Kotodama בפעם הראשונה: להרים רשת דטרמיניסטית עם צומת יחיד, לקמפל חוזה, להריץ אותו מקומית כ-dry-run ואז לשלוח אותו דרך Torii עם ה-CLI הייחוסי.

חוזה הדוגמה כותב זוג מפתח/ערך לחשבון של הקורא כדי שתוכלו לוודא את תופעת הלוואי מיד בעזרת `iroha_cli`.

## דרישות מקדימות

- [Docker](https://docs.docker.com/engine/install/) עם Compose V2 פעיל (משמש להפעלת ה-peer לדוגמה שמוגדר ב-`defaults/docker-compose.single.yml`).
- Rust toolchain (1.76+) לבניית הבינארים המסייעים אם אינכם מורידים את המפורסמים.
- בינארים `koto_compile`, `ivm_run` ו-`iroha_cli`. אפשר לבנות אותם מה-checkout של ה-workspace כפי שמוצג למטה או להוריד את release artifacts המתאימים:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> הבינארים למעלה בטוחים להתקנה לצד שאר ה-workspace.
> הם לעולם לא מקשרים ל-`serde`/`serde_json`; קודקי Norito נאכפים מקצה לקצה.

## 1. הפעלת רשת dev עם צומת יחיד

הריפו כולל Docker Compose bundle שנוצר על ידי `kagami swarm` (`defaults/docker-compose.single.yml`). הוא מחבר את ה-genesis ברירת המחדל, את תצורת הלקוח ואת health probes כדי ש-Torii יהיה נגיש ב-`http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

השאירו את הקונטיינר פועל (בקדמה או מנותק). כל קריאות ה-CLI הבאות מכוונות אל ה-peer הזה דרך `defaults/client.toml`.

## 2. כתיבת החוזה

צרו ספריית עבודה ושמרו את הדוגמה המינימלית של Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> מומלץ לשמור מקורות Kotodama תחת בקרת גרסאות. דוגמאות שמאוחסנות בפורטל זמינות גם ב-[גלריית דוגמאות Norito](./examples/) אם אתם רוצים נקודת התחלה עשירה יותר.

## 3. קומפילציה ו-dry-run עם IVM

קמפלו את החוזה לבייטקוד IVM/Norito (`.to`) והריצו אותו מקומית כדי לוודא ש-syscalls של ה-host מצליחים לפני שנוגעים ברשת:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ה-runner מדפיס את הלוג `info("Hello from Kotodama")` ומבצע את syscall `SET_ACCOUNT_DETAIL` מול host מדומה. אם הבינארי האופציונלי `ivm_tool` זמין, `ivm_tool inspect target/quickstart/hello.to` מציג את ABI header, את feature bits ואת ה-entrypoints המיוצאים.

## 4. שליחת הבייטקוד דרך Torii

כאשר הצומת עדיין רץ, שלחו את הבייטקוד המקומפל ל-Torii באמצעות ה-CLI. הזהות הדיפולטית לפיתוח נגזרת מהמפתח הציבורי ב-`defaults/client.toml`, כך שמזהה החשבון הוא
```
ih58...
```

השתמשו בקובץ ההגדרות כדי לספק את כתובת Torii, chain ID ומפתח החתימה:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

ה-CLI מקודד את הטרנזקציה ב-Norito, חותם עליה עם מפתח הפיתוח ושולח אותה ל-peer הפועל. עקבו אחר לוגים של Docker עבור syscall `set_account_detail` או עקבו אחרי פלט ה-CLI עבור ה-hash של הטרנזקציה ה-committed.

## 5. אימות שינוי מצב

השתמשו באותו פרופיל CLI כדי לקבל את ה-account detail שהחוזה כתב:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

אתם אמורים לראות את ה-JSON payload שמגובה ב-Norito:

```json
{
  "hello": "world"
}
```

אם הערך חסר, ודאו ששירות ה-Docker compose עדיין פועל ושה-hash שדווח על ידי `iroha` הגיע למצב `Committed`.

## צעדים הבאים

- חקרו את [גלריית הדוגמאות](./examples/) שנוצרה אוטומטית כדי לראות
  איך קטעי Kotodama מתקדמים יותר ממופים ל-syscalls של Norito.
- קראו את [מדריך Norito getting started](./getting-started) להסבר מעמיק יותר
  של כלי הקומפיילר/runner, פריסת manifests ומטא-דאטה של IVM.
- כאשר אתם עובדים על החוזים שלכם, השתמשו ב-`npm run sync-norito-snippets` בתוך ה-workspace כדי
  לחדש את ה-snippets להורדה כך שמסמכי הפורטל והארטיפקטים יישארו מסונכרנים עם המקורות תחת `crates/ivm/docs/examples/`.
