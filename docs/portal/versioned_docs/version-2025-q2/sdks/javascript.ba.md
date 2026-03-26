---
lang: ba
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

`@iroha2/torii-client` браузер һәм Nod
Был тиҙ старт көҙгө ядро ​​ағымдары SDK рецепттары, шулай итеп, һеҙ аласыз а
клиент бер нисә минут эсендә эшләй. Тулы миҫалдар өсөн ҡарағыҙ
Репозиторийҙа `javascript/iroha_js/recipes/`.

## 1. Ҡуй.

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Әгәр һеҙ локаль транзакциялар ҡул ҡуйырға планлаштыра, шулай уҡ крипто ярҙамсылар ҡуйырға:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii клиент булдырыу

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Конфигурация рецепттарҙа ҡулланылған конструкторҙы көҙгөләй. Әгәр һеҙҙең төйөн
төп аут ҡуллана, үткән `{username, password}` аша `basicAuth` варианты.

## 3. Төйөн төйөн статусы

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Бөтә уҡыу операциялары ҡайтара Norito-яғында JSON объекттары. Ҡарағыҙ генерацияланған типтарҙы 2019 йылда.
`index.d.ts` өсөн ялан реквизиттары.

## 4. Транзакция тапшырыу

Ҡул ҡуйыусылар ярҙамсы API менән транзакциялар төҙөй ала:

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

Ярҙам автоматик рәүештә транзакцияны Norito конвертында көтөлгән .
Torii . Байыраҡ миҫал өсөн (шул иҫәптән финал көтә), ҡарағыҙ
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Юғары кимәлдәге ярҙамсыларҙы ҡулланығыҙ

SDK өйөмдәре махсуслаштырылған ағымдар, улар КЛИ көҙгөләй:

- **Идара итеү ярҙамсылары** – `recipes/governance.mjs` стажировка күрһәтә
  тәҡдимдәр һәм бюллетендәр менән `governance` инструкция төҙөүселәр.
- **ИСО күпер** – `recipes/iso_bridge.mjs` `pacs.008` һәм 1990 йылдарҙа нисек тапшырырға икәнен күрһәтә һәм
  һорау алыу тапшырыу статусы ҡулланып `/v1/iso20022` ос нөктәләре.
- **SoraFS & триггерҙар** – `src/toriiClient.js` буйынса фантазия ярҙамсылары фашлау
  тип аталған итераторҙар өсөн контракттар, активтар, триггерҙар, һәм SoraFS провайдерҙары.

Импорт тейешле төҙөүсе функциялары `@iroha2/torii-client` был ағымдарҙы ҡабаттан ҡулланыу өсөн.

## 6. Хата менән эш итеү

Бөтә SDK шылтыратыуҙар ташлай бай Norito осраҡтар менән транспорт метамағлүмәттәре .
һәм Norito хата файҙалы йөк. Wrap шылтыратыуҙар `try/catch` йәки ҡулланыу `.catch()` .
өҫкө контекст ҡулланыусылар өсөн:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Киләһе аҙымдар

- `javascript/iroha_js/recipes/` рецептарын тикшерергә ос-ос ағымы өсөн.
- `javascript/iroha_js/index.d.ts` ентекле өсөн генерацияланған төрҙәрен уҡығыҙ
  метод ҡултамғалары.
- Был SDK менән парлы Norito caverstart тикшерергә һәм отладка файҙалы йөкләмәләр
  һеҙ Torii ебәрергә.