---
lang: mn
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK хурдан эхлүүлэх

`@iroha2/torii-client` нь Torii-ийн эргэн тойронд хөтөч болон Node.js-д ээлтэй боодолтой.
Энэхүү хурдан эхлүүлэх нь SDK жоруудын үндсэн урсгалыг тусгаснаар та дараахийг авах боломжтой
үйлчлүүлэгч хэдхэн минутын дотор ажиллах болно. Илүү дэлгэрэнгүй жишээг үзнэ үү
`javascript/iroha_js/recipes/` хадгалах санд байна.

## 1. Суулгах

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Хэрэв та дотооддоо гүйлгээ хийхээр төлөвлөж байгаа бол крипто туслахуудыг суулгаарай:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii клиент үүсгэ

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Тохиргоо нь жоронд ашигласан бүтээгчийг тусгадаг. Хэрэв таны зангилаа
үндсэн баталгаажуулалтыг ашигладаг, `{username, password}`-г `basicAuth` сонголтоор дамжуулна.

## 3. Зангилааны төлөвийг дуудах

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Унших бүх үйлдлүүд нь Norito дэмждэг JSON объектуудыг буцаана. Үүсгэсэн төрлүүдийг үзнэ үү
Талбайн дэлгэрэнгүй мэдээллийг `index.d.ts`.

## 4. Гүйлгээ илгээх

Гарын үсэг зурсан хүмүүс туслах API ашиглан гүйлгээ хийх боломжтой:

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

Туслагч нь хүлээгдэж буй Norito дугтуйнд гүйлгээг автоматаар боож өгнө
Torii. Илүү баялаг жишээг (эцсийн төгсгөлийг хүлээхийг оруулаад) үзнэ үү
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Дээд түвшний туслахуудыг ашигла

SDK нь CLI-г тусгадаг тусгай урсгалуудыг нэгтгэдэг:

- **Засаглалын туслахууд** – `recipes/governance.mjs` шатлалыг харуулж байна
  санал болон саналын хуудас `governance` заавар барилгачид.
- **ISO bridge** – `recipes/iso_bridge.mjs` нь `pacs.008` болон хэрхэн илгээхийг харуулж байна.
  `/v1/iso20022` төгсгөлийн цэгүүдийг ашиглан санал асуулгын шилжүүлгийн статус.
- **SoraFS ба триггер** – `src/toriiClient.js`-ийн доор хуудас бичих туслахууд
  гэрээ, хөрөнгө, триггер болон SoraFS үйлчилгээ үзүүлэгчийн бичсэн давталт.

Тэдгээр урсгалуудыг дахин ашиглахын тулд `@iroha2/torii-client`-аас холбогдох бүтээгчийн функцуудыг импортлоорой.

## 6. Алдаа засах

Бүх SDK дуудлага нь тээврийн мета өгөгдөл бүхий баялаг `ToriiClientError` инстанцуудыг шиддэг.
болон Norito алдааны ачаалал. Дуудлага хийхдээ `try/catch` эсвэл `.catch()`-г ашиглана уу.
Хэрэглэгчдэд үзүүлэх гадаргуугийн контекст:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Дараагийн алхамууд

- `javascript/iroha_js/recipes/` дээрх төгсгөл хоорондын урсгалын жорыг судлаарай.
- Үүсгэсэн төрлүүдийг `javascript/iroha_js/index.d.ts` дээрээс дэлгэрэнгүй уншина уу
  аргын гарын үсэг.
- Энэ SDK-г Norito хурдан эхлүүлэгчтэй хослуулан ачааллыг шалгаж, дибаг хийнэ үү.
  Та Torii руу илгээнэ үү.