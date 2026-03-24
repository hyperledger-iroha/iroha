---
slug: /sdks/recipes/javascript-ledger-flow
lang: my
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤစာရွက်သည် Node.js `@iroha2/torii-client` နှင့် အသုံးပြုသည်။
CLI လယ်ဂျာလမ်းညွှန်ချက်အား ပြန်လည်ထုတ်လုပ်ရန် `@iroha2/crypto-target-node` ပက်ကေ့ဂျ်များ။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="ဤစာရင်းဇယားလမ်းညွှန်ချက်တွင် အသုံးပြုသည့် JavaScript အတိအကျကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

## လိုအပ်ချက်များ

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## ဥပမာ ဇာတ်ညွှန်း

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

`node --env-file=.env ledger-flow.mjs` ဖြင့် လုပ်ဆောင်ပါ (သို့မဟုတ် ပတ်ဝန်းကျင်ကို တင်ပို့ပါ။
variable များကို manually)။ မှတ်တမ်းသည် ငွေပေးငွေယူ hash ကို ပြသင့်သည် (ပြေစာမှ
payload) နှင့် အပ်ဒိတ်လက်ခံသူလက်ကျန်။

## တူညီမှုကို အတည်ပြုပါ။

- `iroha --config defaults/client.toml transaction get --hash <hash>` မှတစ်ဆင့် ငွေပေးငွေယူအသေးစိတ်အချက်အလက်များကို ရယူပါ။
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` ဖြင့် အပြန်အလှန်စစ်ဆေးသော လက်ကျန်ငွေ။
- SDK တူညီမှုရှိစေရန်အတွက် ထုတ်လွှတ်သော hash ကို Rust နှင့် Python ချက်ပြုတ်နည်းများနှင့် နှိုင်းယှဉ်ပါ။