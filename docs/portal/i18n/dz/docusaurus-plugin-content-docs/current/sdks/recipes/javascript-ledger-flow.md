---
slug: /sdks/recipes/javascript-ledger-flow
lang: dz
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

བཀོད་སྒྲིག་འདི་གིས་ Node.js `@iroha2/torii-client` དང་ དེ་ལས་ ལག་ལེན་འཐབ་ཨིན།
`@iroha2/crypto-target-node` ཐུམ་སྒྲིལ་ཚུ་ སི་ཨེལ་ཨའི་ ལེཌ་ཇར་གྱི་ འགྲུལ་བསྐྱོད་ཀྱི་ བསྐྱར་བཟོ་འབདཝ་ཨིན།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལན་/ཇ་བ་སི་ཀིརིཔཊི་/ཀེནཌི་ཇར་-ཕོལ.ཨེམ་ཇེ་སི།"
  filen name="ལེ་ཇར་-ཕོལོ་.ཨེམ་ཇི་སི།"
  secont="ལེད་ཇར་འགྲུལ་བསྐྱོད་འདི་ནང་ལག་ལེན་འཐབ་མི་ ཇ་བ་ཨིསི་ཀིརིཔཊི་ཡིག་ཚུགས་འདི་ཕབ་ལེན་འབད།"
།/>།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## དཔེར་ཡིག་།

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

I18NI000000004X དང་ཅིག་ཁར་གཡོག་བཀོལ།(ཡང་ན་མཐའ་འཁོར་ཕྱིར་འདྲེན་འབད།
འགྱུར་ཅན་ཚུ་ ལག་ཐོག་ལས་)། དྲན་ཐོ་འདི་གིས་ ཚོང་འབྲེལ་ཧེཤ་ (ཐོབ་ཐང་ལས་ སྟོན་དགོ།
pataad) དང་ དུས་མཐུན་བཟོ་ཡོད་པའི་ ལེན་མི་ལྷག་ལུས་ཚུ།

## ཆ་འཇོག་འབད་ནི།

- I18NI0000005X བརྒྱུད་དེ་ ཚོང་འབྲེལ་ཁ་གསལ་ཚུ་ ལེན་དགོ།
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` དང་མཉམ་པའི་ cross-ཞིབ་དཔྱད་འབད་ནི།
- ཨེསི་ཌི་ཀེ་ ཆ་སྙོམས་འདི་ ངེས་གཏན་བཟོ་ནིའི་དོན་ལུ་ རསཊ་དང་ པའི་ཐཱོན་བཟོ་ཐངས་ཚུ་དང་ ག་བསྡུར་རྐྱབ།