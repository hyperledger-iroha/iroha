---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ཇ་བ་ཨིསི་ཀིརིཔཊི་ ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་པ།

`@iroha2/torii-client` གིས་ བརྡ་འཚོལ་དང་ Node.js མཐུན་འབྲེལ་ཅན་གྱི་ བཀབ་ཆ་ Torii གི་མཐའ་འཁོར་ལུ་བྱིནམ་ཨིན།
འདི་མགྱོགས་དྲགས་འགོ་བཙུགས།
སྐར་མ་དག་པ་ཅིག་ནང་ མཁོ་མངགས་འབདཝ་ཨིན། དཔེ་ཆ་ཚང་གི་དོན་ལུ་ བལྟ།
`javascript/iroha_js/recipes/` མཛོད་ཁང་ནང་།

## 1. གཞི་བཙུགས་འབད་ནི།

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

ཁྱོད་ཀྱིས་ ཉེ་གནས་ལུ་ ཚོང་འབྲེལ་ཚུ་ མཚན་རྟགས་བཀོད་ནི་གི་འཆར་གཞི་ཡོད་པ་ཅིན་ ཀིརིཔ་ཊོ་གྲོགས་རམ་པ་ཚུ་ཡང་ གཞི་བཙུགས་འབད།

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii མཁོ་མངགས་ཅིག་གསར་བསྐྲུན་འབད།

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

རིམ་སྒྲིག་འདི་གིས་ བཞེས་སྒོའི་རིགས་ཚུ་ནང་ལག་ལེན་འཐབ་མི་ བཟོ་བསྐྲུན་པ་འདི་ མེ་ལོང་བཟོཝ་ཨིན། ཁྱོད་ཀྱི་མཛུབ་དཀྱི་གལ་ཏེ།
གཞི་རྟེན་བདེན་བཤད་ལག་ལེན་འཐབ་སྟེ་ `{username, password}` འདི་ `basicAuth` གདམ་ཁ་བརྒྱུད་དེ་ བརྒྱུད་དེ་འགྱོཝ་ཨིན།

## 3. ཕེཊི་ནའུཌི་གནས་ཚད།

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

ལྷག་པའི་བཀོལ་སྤྱོད་སླར་ལོག་ Norito-backed JSON དངོས་པོ་ཚུ་ཨིན། བཟོ་བཏོན་འབད་ཡོད་པའི་དབྱེ་བ་ཚུ་ ནང་བལྟ།
ས་སྒོའི་ཁ་གསལ་གྱི་དོན་ལུ་ `index.d.ts`.

## 4. ཚོང་བསྒྱུར་ཅིག་ཕུལ་བ།

མཚན་རྟགས་བཀོད་མི་ཚུ་གིས་ གྲོགས་རམ་ཨེ་པི་ཨའི་དང་གཅིག་ཁར་ ཚོང་འབྲེལ་ཚུ་བཟོ་ཚུགས།

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'i105...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

གྲོགས་རམ་པ་འདི་གིས་ རང་བཞིན་གྱིས་ I1NT0000001X ཡིག་ཤུབས་རེ་བ་ནང་ བཀབ་བཞགཔ་ཨིན།
by Torii. དཔེ་བཟང་པོ་ཅིག་གི་དོན་ལུ་ (མཐའ་མཐའ་ལུ་སྒུག་སྡོདཔ་ཨིན།) བལྟ།
Norito.

## 5. མཐོ་རིམ་རོགས་སྐྱོར་ལག་བསྟར་བྱེད་པ།

ཨེསི་ཌི་ཀེ་ བཱན་ཌལ་ཚུ་གིས་ སི་ཨེལ་ཨའི་ མེ་ལོང་ནང་ དམིགས་བསལ་སྦེ་ བཞུར་རྒྱུན་འབདཝ་ཨིན།

- **གཞུང་སྐྱོང་རོགས་སྐྱོར་པ་** – `recipes/governance.mjs` གིས་ གནས་རིམ་སྟོན་ཡོདཔ།
  `governance` དང་མཉམ་པའི་གྲོས་འཆར་དང་ཚོགས་རྒྱན་ཚུ།
- **ISO ཟམ་** – `recipes/iso_bridge.mjs` གིས་ `pacs.008` དང་ `pacs.008` དང་ བཙུགས་ཐངས་ཚུ་སྟོནམ་ཨིན།
  `/v1/iso20022` མཐའ་མཚམས་ཚུ་ལག་ལེན་འཐབ་སྟེ་ འོས་འདེམས་སྤོ་བཤུད་ཀྱི་གནས་ཚད།
- **SoraFS & trights*** – འགྲུལ་བཞུད་ཀྱི་གྲོགས་རམ་པ་ `src/toriiClient.js` འོག་ལུ་གསལ་སྟོན་འབདཝ་ཨིན།
  གན་རྒྱ་དང་ རྒྱུ་དངོས་ ཊི་རི་ཊི་ཚུ་ དེ་ལས་ SoraFS བྱིན་མི་ཚུ་གི་དོན་ལུ་ ཡིག་དཔར་རྐྱབས།

འཕྲོ་མཐུད་དེ་ཚུ་ ལོག་ལག་ལེན་འཐབ་ནི་ལུ་ `@iroha2/torii-client` ལས་ འབྲེལ་ཡོད་བཟོ་བསྐྲུན་པ་ལས་འགན་ཚུ་ ནང་འདྲེན་འབད།

## 6. འཛོལ་བ།

ཨེསི་ཌི་ཀེ་གིས་ སྐྱེལ་འདྲེན་མེ་ཊ་ཌེ་ཊ་དང་གཅིག་ཁར་ `ToriiClientError` གནས་སྟངས་ཚུ་ ཕྱུགཔོ་སྦེ་བཀོདཔ་ཨིན།
དང་ Norito འཛོལ་བའི་འཛོལ་བ་སྤྲོད་ལེན་འདི་ཨིན། `try/catch` ནང་ འབོད་བརྡ་གཏང་ ཡང་ན་ `.catch()` ལུ་ལག་ལེན་འཐབ།
ལག་ལེན་པ་ཚུ་ལུ་ ཁ་ཐོག་གི་སྐབས་དོན་:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## ཤུལ་མམ་གྱི་གོམ་པ།

- མཐའ་མཇུག་ལས་མཇུག་ཚུན་ཚོད་ཀྱི་ རྒྱུན་འགྲུལ་གྱི་དོན་ལུ་ `javascript/iroha_js/recipes/` ནང་ བཟའ་ཐབས་ཚུ་ འཚོལ་ཞིབ་འབད།
- ཁ་གསལ་གྱི་དོན་ལུ་ `javascript/iroha_js/index.d.ts` ནང་བཟོ་བཏོན་འབད་ཡོད་པའི་དབྱེ་བ་ཚུ་ལྷག།
  ཐབས་ལམ་མིང་རྟགས་ཚུ།
- ཨེསི་ཌི་ཀེ་འདི་ Norito མགྱོགས་འགོ་བཙུགས་དང་གཅིག་ཁར་ ཆ་སྒྲིག་འབད།
  ཁྱོད་ཀྱིས་ Torii ལུ་གཏང་ཡི།