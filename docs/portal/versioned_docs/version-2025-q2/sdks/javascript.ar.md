---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# جافا سكريبت SDK البدء السريع

يوفر `@iroha2/torii-client` متصفحًا ومغلفًا سهل الاستخدام لـ Node.js حول Torii.
تعكس هذه البداية السريعة التدفقات الأساسية من وصفات SDK حتى تتمكن من الحصول على
العميل يعمل في بضع دقائق. للحصول على أمثلة أكمل، انظر
`javascript/iroha_js/recipes/` في المستودع.

## 1. التثبيت

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

إذا كنت تخطط لتوقيع المعاملات محليًا، فقم أيضًا بتثبيت مساعدي التشفير:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. إنشاء عميل Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

يعكس التكوين المنشئ المستخدم في الوصفات. إذا كانت العقدة الخاصة بك
يستخدم المصادقة الأساسية، وقم بتمرير `{username, password}` عبر الخيار `basicAuth`.

## 3. جلب حالة العقدة

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

تقوم جميع عمليات القراءة بإرجاع كائنات JSON المدعومة بـ Norito. انظر الأنواع التي تم إنشاؤها في
`index.d.ts` للحصول على تفاصيل الحقل.

## 4. أرسل معاملة

يمكن للموقّعين إنشاء معاملات باستخدام واجهة برمجة التطبيقات المساعدة:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: '<katakana-i105-account-id>',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

يقوم المساعد تلقائيًا بتغليف المعاملة في المغلف Norito المتوقع
بواسطة Torii. للحصول على مثال أكثر ثراء (بما في ذلك انتظار النهاية)، انظر
`javascript/iroha_js/recipes/registration.mjs`.

## 5. استخدم مساعدين رفيعي المستوى

تجمع حزمة SDK التدفقات المتخصصة التي تعكس واجهة سطر الأوامر:

- **مساعدي الحوكمة** – `recipes/governance.mjs` يوضح التدريج
  المقترحات وبطاقات الاقتراع مع منشئي التعليمات `governance`.
- **جسر ISO** - يوضح `recipes/iso_bridge.mjs` كيفية إرسال `pacs.008` و
  حالة نقل الاستقصاء باستخدام نقاط النهاية `/v1/iso20022`.
- **SoraFS والمشغلات** - كشف مساعدي ترقيم الصفحات ضمن `src/toriiClient.js`
  التكرارات المكتوبة للعقود والأصول والمشغلات وموفري SoraFS.

قم باستيراد وظائف الإنشاء ذات الصلة من `@iroha2/torii-client` لإعادة استخدام تلك التدفقات.

## 6. معالجة الأخطاء

توفر كافة استدعاءات SDK مثيلات `ToriiClientError` غنية ببيانات تعريف النقل
وحمولة الخطأ Norito. قم بإنهاء المكالمات في `try/catch` أو استخدم `.catch()` لـ
السياق السطحي للمستخدمين:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## الخطوات التالية

- استكشف الوصفات الموجودة في `javascript/iroha_js/recipes/` للتدفقات الشاملة.
- اقرأ الأنواع التي تم إنشاؤها في `javascript/iroha_js/index.d.ts` للحصول على التفاصيل
  توقيعات الطريقة.
- قم بإقران SDK هذا مع التشغيل السريع Norito لفحص الحمولات وتصحيح أخطائها
  قمت بإرساله إلى Torii.