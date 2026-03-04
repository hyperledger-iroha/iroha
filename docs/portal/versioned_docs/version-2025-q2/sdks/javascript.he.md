---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# התחלה מהירה של JavaScript SDK

`@iroha2/torii-client` מספק דפדפן ועטיפה ידידותית ל-Node.js סביב Torii.
התחלה מהירה זו משקפת את זרימות הליבה ממתכוני ה-SDK כך שתוכל לקבל א
הלקוח פועל תוך מספר דקות. לדוגמאות מלאות יותר, ראה
`javascript/iroha_js/recipes/` במאגר.

## 1. התקן

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

אם אתה מתכנן לחתום על עסקאות באופן מקומי, התקן גם את עוזרי ההצפנה:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. צור לקוח Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

התצורה משקפת את הבנאי המשמש במתכונים. אם הצומת שלך
משתמש באישור בסיסי, העבר את `{username, password}` דרך אפשרות `basicAuth`.

## 3. אחזר מצב צומת

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

כל פעולות הקריאה מחזירות אובייקטי JSON מגובי Norito. ראה את הסוגים שנוצרו ב
`index.d.ts` לפרטי שדה.

## 4. שלח עסקה

חותמים יכולים לבנות עסקאות עם ממשק ה-API של עוזר:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

העוזר עוטף אוטומטית את העסקה במעטפת Norito הצפויה
מאת Torii. לדוגמא עשירה יותר (כולל המתנה לסופיות), ראה
`javascript/iroha_js/recipes/registration.mjs`.

## 5. השתמש בעוזרים ברמה גבוהה

ה-SDK חבילות זרימות מיוחדות המשקפות את ה-CLI:

- **עוזרים ממשל** - `recipes/governance.mjs` מדגים בימוי
  הצעות ופתקי הצבעה עם בוני ההוראות `governance`.
- **גשר ISO** - `recipes/iso_bridge.mjs` מראה כיצד להגיש את `pacs.008` ו
  סטטוס העברת סקר באמצעות נקודות הקצה `/v1/iso20022`.
- **SoraFS וטריגרים** - עוזרי עימוד תחת `src/toriiClient.js` חשיפת
  איטרטורים מוקלדים עבור חוזים, נכסים, טריגרים וספקי SoraFS.

ייבא את פונקציות הבונה הרלוונטיות מ-`@iroha2/torii-client` כדי לעשות שימוש חוזר בזרימות אלה.

## 6. טיפול בשגיאות

כל שיחות SDK זורקות מופעי `ToriiClientError` עשירים עם מטא נתונים של תחבורה
ואת מטען השגיאה Norito. עטוף שיחות ב-`try/catch` או השתמש ב-`.catch()` כדי
הקשר פני השטח למשתמשים:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## השלבים הבאים

- חקור את המתכונים ב-`javascript/iroha_js/recipes/` לזרימות מקצה לקצה.
- קרא את הסוגים שנוצרו ב-`javascript/iroha_js/index.d.ts` לפרטים
  חתימות השיטה.
- התאמה של SDK זה עם ה-Norito להתחלה מהירה כדי לבדוק וניפוי באגים של המטענים
  אתה שולח ל-Torii.