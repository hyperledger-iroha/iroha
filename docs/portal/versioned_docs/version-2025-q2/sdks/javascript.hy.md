---
lang: hy
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK Quickstart

`@iroha2/torii-client`-ն ապահովում է զննարկիչ և Node.js-ի բարեկամական փաթաթան Torii-ի շուրջ:
Այս արագ մեկնարկը արտացոլում է SDK-ի բաղադրատոմսերի հիմնական հոսքերը, որպեսզի կարողանաք ստանալ ա
հաճախորդը աշխատում է մի քանի րոպեում: Ավելի ամբողջական օրինակների համար տե՛ս
`javascript/iroha_js/recipes/` պահոցում:

## 1. Տեղադրեք

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Եթե նախատեսում եք գործարքներ կնքել տեղում, տեղադրեք նաև կրիպտո օգնականները.

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Ստեղծեք Torii հաճախորդ

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Կազմաձևը արտացոլում է բաղադրատոմսերում օգտագործվող կոնստրուկտորը: Եթե ձեր հանգույցը
օգտագործում է հիմնական հեղինակությունը, անցեք `{username, password}` `basicAuth` տարբերակով:

## 3. Ստացեք հանգույցի կարգավիճակը

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Բոլոր ընթերցման գործողությունները վերադարձնում են Norito-ով ապահովված JSON օբյեկտներ: Տեսեք ստեղծված տեսակները
`index.d.ts` դաշտի մանրամասների համար:

## 4. Ներկայացրե՛ք գործարք

Ստորագրողները կարող են գործարքներ կառուցել օգնական API-ով.

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

Օգնականը ավտոմատ կերպով փաթեթավորում է գործարքը սպասվող Norito ծրարով
Torii-ի կողմից: Ավելի հարուստ օրինակի համար (ներառյալ վերջնականության սպասումները), տե՛ս
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Օգտագործեք բարձր մակարդակի օգնականներ

SDK-ն միավորում է մասնագիտացված հոսքերը, որոնք արտացոլում են CLI-ը.

- **Կառավարման օգնականներ** – `recipes/governance.mjs`-ը ցուցադրում է բեմադրությունը
  առաջարկներ և քվեաթերթիկներ `governance` հրահանգների կառուցողների հետ:
- **ISO կամուրջ** – `recipes/iso_bridge.mjs` ցույց է տալիս, թե ինչպես ներկայացնել `pacs.008` և
  հարցումների փոխանցման կարգավիճակը՝ օգտագործելով `/v1/iso20022` վերջնակետերը:
- **SoraFS և գործարկիչներ** – Էջավորման օգնականներ `src/toriiClient.js`-ի ներքո
  տպագրված կրկնիչներ պայմանագրերի, ակտիվների, գործարկիչների և SoraFS մատակարարների համար:

Ներմուծեք համապատասխան շինարարական գործառույթները `@iroha2/torii-client`-ից՝ այդ հոսքերը նորից օգտագործելու համար:

## 6. Սխալների մշակում

Բոլոր SDK զանգերը նետում են `ToriiClientError` հարուստ օրինակներ՝ տրանսպորտային մետատվյալներով
և Norito սխալի ծանրաբեռնվածությունը: Փաթեթավորեք զանգերը `try/catch`-ով կամ օգտագործեք `.catch()`՝
մակերեսային համատեքստ օգտվողներին.

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Հաջորդ քայլերը

- Բացահայտեք `javascript/iroha_js/recipes/`-ի բաղադրատոմսերը ծայրից ծայր հոսքերի համար:
- Մանրամասն կարդացեք ստեղծված տեսակները `javascript/iroha_js/index.d.ts`-ում
  մեթոդի ստորագրություններ.
- Զուգավորեք այս SDK-ն Norito արագ մեկնարկի հետ՝ բեռները ստուգելու և վրիպազերծելու համար
  դուք ուղարկում եք Torii: