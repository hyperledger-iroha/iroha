---
lang: ka
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

`@iroha2/torii-client` უზრუნველყოფს ბრაუზერს და Node.js მეგობრულ შეფუთვას Torii-ის გარშემო.
ეს სწრაფი დაწყება ასახავს SDK-ის რეცეპტების ძირითად ნაკადებს, ასე რომ თქვენ შეგიძლიათ მიიღოთ ა
კლიენტი მუშაობს რამდენიმე წუთში. უფრო სრული მაგალითებისთვის იხ
`javascript/iroha_js/recipes/` საცავში.

## 1. დააინსტალირეთ

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

თუ გეგმავთ ტრანზაქციების ადგილობრივ ხელმოწერას, ასევე დააინსტალირეთ კრიპტო დამხმარეები:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. შექმენით Torii კლიენტი

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

კონფიგურაცია ასახავს რეცეპტებში გამოყენებულ კონსტრუქტორს. თუ თქვენი კვანძი
იყენებს ძირითად ავტორიზაციას, გაიარეთ `{username, password}` `basicAuth` ვარიანტის მეშვეობით.

## 3. კვანძის სტატუსის მიღება

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

წაკითხვის ყველა ოპერაცია აბრუნებს Norito მხარდაჭერილ JSON ობიექტებს. იხილეთ გენერირებული ტიპები
`index.d.ts` ველის დეტალებისთვის.

## 4. წარადგინეთ ტრანზაქცია

ხელმომწერებს შეუძლიათ შექმნან ტრანზაქციები დამხმარე API-ით:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'soraカタカナ...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

დამხმარე ავტომატურად ახვევს ტრანზაქციას Norito მოსალოდნელ კონვერტში
Torii-ის მიერ. უფრო მდიდარი მაგალითისთვის (მათ შორის ელოდება საბოლოო დასრულებას), იხ
`javascript/iroha_js/recipes/registration.mjs`.

## 5. გამოიყენეთ მაღალი დონის დამხმარეები

SDK აგროვებს სპეციალიზებულ ნაკადებს, რომლებიც ასახავს CLI-ს:

- **მმართველობის დამხმარეები** – `recipes/governance.mjs` აჩვენებს დადგმას
  წინადადებები და ბიულეტენი `governance` ინსტრუქციების შემქმნელებთან.
- **ISO ხიდი** – `recipes/iso_bridge.mjs` გვიჩვენებს, თუ როგორ უნდა გაგზავნოთ `pacs.008` და
  გამოკითხვის გადაცემის სტატუსი `/v1/iso20022` ბოლო წერტილების გამოყენებით.
- **SoraFS და ტრიგერები** – პაგინაციის დამხმარეები `src/toriiClient.js` გამოფენის ქვეშ
  აკრეფილი იტერატორები კონტრაქტების, აქტივების, ტრიგერებისა და SoraFS პროვაიდერებისთვის.

ამ ნაკადების ხელახლა გამოსაყენებლად, შემოიტანეთ შესაბამისი Builder ფუნქციები `@iroha2/torii-client`-დან.

## 6. შეცდომების დამუშავება

ყველა SDK ზარი აგდებს მდიდარ `ToriiClientError` მაგალითებს ტრანსპორტის მეტამონაცემებით
და Norito შეცდომის დატვირთვა. გადაიტანეთ ზარები `try/catch`-ში ან გამოიყენეთ `.catch()`
ზედაპირული კონტექსტი მომხმარებლებისთვის:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## შემდეგი ნაბიჯები

- გამოიკვლიეთ რეცეპტები `javascript/iroha_js/recipes/`-ში ბოლოდან ბოლომდე ნაკადებისთვის.
- დეტალურად წაიკითხეთ გენერირებული ტიპები `javascript/iroha_js/index.d.ts`-ში
  მეთოდის ხელმოწერები.
- დააწყვილეთ ეს SDK Norito სწრაფ დაწყებასთან, რათა შეამოწმოთ და გამართოთ დატვირთვები
  თქვენ აგზავნით Torii-ზე.