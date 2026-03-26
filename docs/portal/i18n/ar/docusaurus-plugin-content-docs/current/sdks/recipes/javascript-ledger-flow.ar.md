---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fce6fbde7987c30354be74cb32984189b4bdb91341c052012eec37cf78730b32
source_last_modified: "2025-11-11T10:22:54.086603+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: وصفة تدفق دفتر الأستاذ بجافاسكربت
description: سجّل أصلًا، وسكّ، وحوّل، واستعلم عن الأرصدة باستخدام `@iroha2/torii-client`.
slug: /sdks/recipes/javascript-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

تستخدم هذه الوصفة حزمتَي Node.js `@iroha2/torii-client` و`@iroha2/crypto-target-node` لإعادة تنفيذ جولة دفتر الأستاذ في CLI.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="نزّل سكربت JavaScript نفسه المستخدم في هذه الجولة عبر دفتر الأستاذ."
/>

## المتطلبات المسبقة

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## مثال على النص البرمجي

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

شغّل `node --env-file=.env ledger-flow.mjs` (أو صدّر متغيرات البيئة يدويًا). يجب أن يُظهر السجل تجزئة المعاملة (من حمولة الإيصال) ورصيد المستلم المحدّث.

## تحقّق من التكافؤ

- اجلب تفاصيل المعاملة عبر `iroha --config defaults/client.toml transaction get --hash <hash>`.
- طابق الأرصدة باستخدام `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- قارن التجزئة الصادرة مع وصفات Rust وPython لضمان تكافؤ حزم SDK.
