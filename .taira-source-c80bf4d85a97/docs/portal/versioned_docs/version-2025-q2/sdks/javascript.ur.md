---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# جاوا اسکرپٹ SDK کوئیک اسٹارٹ

`@iroha2/torii-client` Torii کے ارد گرد ایک براؤزر اور نوڈ. جے ایس دوستانہ ریپر فراہم کرتا ہے۔
یہ کوئک اسٹارٹ ایس ڈی کے ترکیبوں سے بنیادی بہاؤ کو آئینہ دیتا ہے تاکہ آپ کو ایک مل سکے
کلائنٹ چند منٹ میں چل رہا ہے۔ مکمل مثالوں کے لئے ، دیکھیں
`javascript/iroha_js/recipes/` ذخیرہ میں۔

## 1۔ انسٹال کریں

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

اگر آپ مقامی طور پر لین دین پر دستخط کرنے کا ارادہ رکھتے ہیں تو ، کرپٹو مددگار بھی انسٹال کریں:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii کلائنٹ بنائیں

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

ترتیب ترکیبوں میں استعمال ہونے والے کنسٹرکٹر کی آئینہ دار ہے۔ اگر آپ کا نوڈ ہے
`basicAuth` آپشن کے ذریعہ بنیادی تصنیف ، پاس `{username, password}` کو استعمال کرتا ہے۔

## 3. بازیافت نوڈ کی حیثیت

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

تمام پڑھنے والی کاروائیاں Norito کی حمایت یافتہ JSON آبجیکٹ کو لوٹاتی ہیں۔ میں پیدا شدہ اقسام دیکھیں
`index.d.ts` فیلڈ کی تفصیلات کے لئے۔

## 4. ٹرانزیکشن جمع کروائیں

دستخط کرنے والے مددگار API کے ساتھ لین دین تیار کرسکتے ہیں:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: '<i105-account-id>',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

ہیلپر متوقع Norito لفافے میں خود بخود لین دین کو لپیٹ دیتا ہے
Torii کے ذریعہ۔ ایک بھرپور مثال کے ل ((بشمول فائنلٹی کے منتظر) ، دیکھیں
`javascript/iroha_js/recipes/registration.mjs`۔

## 5. اعلی سطحی مددگار استعمال کریں

ایس ڈی کے نے خصوصی بہاؤ کو بنڈل کیا جو سی ایل آئی کو آئینہ دار کرتے ہیں:

- ** گورننس مددگار ** - `recipes/governance.mjs` اسٹیجنگ کا مظاہرہ کرتا ہے
  `governance` انسٹرکشن بلڈروں کے ساتھ تجاویز اور بیلٹ۔
- ** آئی ایس او برج ** - `recipes/iso_bridge.mjs` ظاہر کرتا ہے کہ `pacs.008` کو کس طرح پیش کیا جائے اور
  `/v1/iso20022` اختتامی مقامات کا استعمال کرتے ہوئے پول کی منتقلی کی حیثیت۔
- ** SoraFS & trigers ** - `src/toriiClient.js` کے تحت صفحہ بندی مددگار
  معاہدوں ، اثاثوں ، محرکات ، اور SoraFS فراہم کنندگان کے لئے ٹائپ شدہ ایٹریٹرز۔

ان بہاؤوں کو دوبارہ استعمال کرنے کے لئے `@iroha2/torii-client` سے متعلقہ بلڈر کے افعال درآمد کریں۔

## 6. غلطی سے نمٹنے کے

تمام SDK کالز ٹرانسپورٹ میٹا ڈیٹا کے ساتھ رچ `ToriiClientError` مثال کے طور پر پھینک دیتے ہیں
اور Norito غلطی پے لوڈ۔ `try/catch` میں کالوں کو لپیٹیں یا `.catch()` کو استعمال کریں
صارفین کو سطح کا سیاق و سباق:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## اگلے اقدامات

-اختتامی بہاؤ کے لئے `javascript/iroha_js/recipes/` میں ترکیبیں دریافت کریں۔
- تفصیلی کے لئے `javascript/iroha_js/index.d.ts` میں تیار کردہ اقسام کو پڑھیں
  طریقہ کے دستخط۔
- پے لوڈ کا معائنہ اور ڈیبگ کرنے کے لئے اس SDK کو Norito کوئیک اسٹارٹ کے ساتھ جوڑیں
  آپ Torii پر بھیجیں۔