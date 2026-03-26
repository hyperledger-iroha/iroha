---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fce6fbde7987c30354be74cb32984189b4bdb91341c052012eec37cf78730b32
source_last_modified: "2025-11-11T10:22:54.086603+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: جاوا اسکرپٹ لیجر فلو ترکیب
description: `@iroha2/torii-client` کے ساتھ اثاثہ رجسٹر کریں، منٹ کریں، منتقل کریں اور بیلنس معلوم کریں۔
slug: /sdks/recipes/javascript-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

یہ ترکیب Node.js کے `@iroha2/torii-client` اور `@iroha2/crypto-target-node` پیکجز استعمال کرتی ہے تاکہ CLI کے لیجر واک تھرو کو دوبارہ دہرایا جا سکے۔

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="اس لیجر واک تھرو میں استعمال ہونے والا عین JavaScript اسکرپٹ ڈاؤن لوڈ کریں۔"
/>

## پیشگی تقاضے

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## مثالی اسکرپٹ

```ts title="ledger-flow.mjs"
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';

const adminAccount = process.env.ADMIN_ACCOUNT!;
const receiverAccount = process.env.RECEIVER_ACCOUNT!;
const adminPrivateKey = process.env.ADMIN_PRIVATE_KEY!;

const client = ToriiClient.create({
  apiUrl: 'http://127.0.0.1:8080',
});

const {publicKey, privateKey} = createKeyPairFromHex(adminPrivateKey);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000',
  authority: adminAccount,
  instructions: [
    {Register: {assetDefinition: {numeric: {id: '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ'}}}},
    {Mint: {asset: {id: `norito:4e52543000000002`}, value: {quantity: '250'}}},
    {Transfer: {
      asset: {id: `norito:4e52543000000002`},
      destination: receiverAccount,
      value: {quantity: '50'},
    }},
  ],
});

tx.sign({publicKey, privateKey});
const receipt = await client.submitTransaction(tx);
const txHash = receipt?.payload?.tx_hash ?? "<pending>";
console.log('Submitted tx', txHash);

const balances = await client.listAccountAssets(receiverAccount, {limit: 10});
for (const asset of balances.items) {
  if (asset.id.definition === '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

`node --env-file=.env ledger-flow.mjs` چلائیں (یا ماحولیاتی متغیرات دستی طور پر ایکسپورٹ کریں)۔ لاگ میں ٹرانزیکشن ہیش (رسید پے لوڈ سے) اور وصول کنندہ کا اپ ڈیٹ شدہ بیلنس دکھائی دینا چاہیے۔

## برابری کی تصدیق

- `iroha --config defaults/client.toml transaction get --hash <hash>` کے ذریعے ٹرانزیکشن کی تفصیلات حاصل کریں۔
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` سے بیلنس کا موازنہ کریں۔
- SDK کی برابری یقینی بنانے کے لیے Rust اور Python کی ترکیبوں کے ساتھ اخراج شدہ ہیش کا موازنہ کریں۔
