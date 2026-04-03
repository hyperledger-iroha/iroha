---
lang: he
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: סיור בלדג'ר
description: שחזרו זרימה דטרמיניסטית register -> mint -> transfer עם CLI `iroha` ואמתו את מצב הלדג'ר המתקבל.
slug: /norito/ledger-walkthrough
---

הסיור הזה משלים את [Norito quickstart](./quickstart.md) בכך שהוא מראה כיצד לשנות ולבדוק את מצב הלדג'ר עם CLI `iroha`. תרשמו הגדרת נכס חדשה, תבצעו mint של יחידות לחשבון המפעיל ברירת המחדל, תעבירו חלק מהיתרה לחשבון אחר ותאמתו את העסקאות וההחזקות שנוצרו. כל שלב משקף את הזרימות שמכוסות ב-quickstarts של Rust/Python/JavaScript SDK כדי שתוכלו לאשר התאמה בין CLI להתנהגות ה-SDK.

## דרישות מקדימות

- עקבו אחרי [quickstart](./quickstart.md) כדי להרים רשת עם צומת יחיד באמצעות
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- ודאו ש-`iroha` (ה-CLI) נבנה או הורד ושאתם יכולים להגיע ל-peer דרך `defaults/client.toml`.
- עוזרים אופציונליים: `jq` (עיצוב תגובות JSON) ו-shell POSIX עבור קטעי משתני הסביבה בהמשך.

לאורך המדריך החליפו את `$ADMIN_ACCOUNT` ו-`$RECEIVER_ACCOUNT` במזהי החשבונות שבהם תשתמשו. ה-bundle ברירת המחדל כבר כולל שני חשבונות שנגזרו מהמפתחות של הדמו:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

אמתו את הערכים על ידי הצגת החשבונות הראשונים:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. בדיקת מצב ה-genesis

התחילו בחקירת הלדג'ר שאליו ה-CLI מכוון:

```sh
# Domains שנרשמו ב-genesis
iroha --config defaults/client.toml domain list all --table

# Accounts בתוך wonderland (החליפו את --limit במספר גבוה יותר אם צריך)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions שכבר קיימים
iroha --config defaults/client.toml asset definition list all --table
```

הפקודות האלו נשענות על תשובות שמבוססות על Norito, כך שסינון ופגינציה הם דטרמיניסטיים ותואמים למה שה-SDK מקבלים.

## 2. רישום הגדרת נכס

צרו נכס חדש שניתן למינט ללא הגבלה בשם `coffee` בתוך הדומיין `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

ה-CLI מדפיס את ה-hash של הטרנזקציה שנשלחה (לדוגמה, `0x5f…`). שמרו אותו כדי לבדוק את הסטטוס בהמשך.

## 3. מינט יחידות לחשבון המפעיל

כמויות נכס חיות תחת הצמד `(asset definition, account)`. מינטו 250 יחידות של `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` אל `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

שוב, שמרו את ה-hash (`$MINT_HASH`) מפלט ה-CLI. כדי לאמת את היתרה הריצו:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

או כדי למקד רק בנכס החדש:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. העברת חלק מהיתרה לחשבון אחר

העבירו 50 יחידות מחשבון המפעיל ל-`$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

שמרו את ה-hash כ-`$TRANSFER_HASH`. בדקו את ההחזקות בשני החשבונות כדי לאמת את היתרות החדשות:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. אימות ראיות מהלדג'ר

השתמשו ב-hashes ששמרתם כדי לאשר ששתי העסקאות קומיטו:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

אפשר גם להזרים בלוקים אחרונים כדי לראות איזה בלוק כלל את ההעברה:

```sh
# Stream מהבלוק האחרון ועצרו אחרי ~5 שניות
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

כל הפקודות למעלה משתמשות באותם Norito payloads כמו ה-SDK. אם תשחזרו את הזרימה הזו בקוד (ראו quickstarts של SDK למטה), ה-hashes והיתרות יתאימו כל עוד אתם מכוונים לאותה רשת ולאותם defaults.

## קישורי Parity של SDK

- [Rust SDK quickstart](../sdks/rust) — מדגים רישום הוראות, שליחת טרנזקציות ופולינג סטטוס מ-Rust.
- [Python SDK quickstart](../sdks/python) — מציג את אותן פעולות register/mint עם Norito-backed JSON helpers.
- [JavaScript SDK quickstart](../sdks/javascript) — מכסה בקשות Torii, helpers של governance ו-wrappers של typed query.

הריצו קודם את הסיור ב-CLI, ואז חזרו על התרחיש עם ה-SDK המועדף כדי לוודא ששני המשטחים מסכימים על hashes של טרנזקציות, יתרות ותוצאות שאילתות.
