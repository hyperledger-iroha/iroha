---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK жылдам іске қосу

`@iroha2/torii-client` шолғышты және Torii айналасындағы Node.js ыңғайлы орауышты қамтамасыз етеді.
Бұл жылдам іске қосу SDK рецептеріндегі негізгі ағындарды көрсетеді, осылайша сіз келесі ақпаратты ала аласыз
клиент бірнеше минут ішінде іске қосылады. Толық мысалдар үшін қараңыз
Репозиторийде `javascript/iroha_js/recipes/`.

## 1. Орнату

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Жергілікті транзакцияларға қол қоюды жоспарласаңыз, криптографиялық көмекшілерді де орнатыңыз:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii клиентін жасаңыз

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Конфигурация рецепттерде пайдаланылған конструкторды көрсетеді. Егер сіздің түйініңіз
негізгі аутентификацияны пайдаланады, `{username, password}` параметрін `basicAuth` опциясы арқылы өткізіңіз.

## 3. Түйін күйін алу

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Барлық оқу әрекеттері Norito қолдайтын JSON нысандарын қайтарады. Жасалған түрлерін қараңыз
Өріс мәліметтері үшін `index.d.ts`.

## 4. Транзакцияны жіберіңіз

Қол қоюшылар көмекші API көмегімен транзакцияларды құра алады:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

Көмекші транзакцияны күтілетін Norito конвертіне автоматты түрде орады.
Torii бойынша. Неғұрлым бай мысал (соның ішінде түпкілікті күту) үшін қараңыз
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Жоғары деңгейдегі көмекшілерді қолданыңыз

SDK CLI-ны көрсететін арнайы ағындарды жинақтайды:

- **Басқару көмекшілері** – `recipes/governance.mjs` сахналауды көрсетеді
  `governance` нұсқаулығын құрастырушылармен ұсыныстар мен бюллетеньдер.
- **ISO көпірі** – `recipes/iso_bridge.mjs` `pacs.008` және жіберу жолын көрсетеді.
  `/v1/iso20022` соңғы нүктелерін пайдаланып сауалнаманы тасымалдау күйі.
- **SoraFS және триггерлер** – `src/toriiClient.js` астында беттеу көмекшілері
  келісім-шарттар, активтер, триггерлер және SoraFS провайдерлері үшін терілген итераторлар.

Сол ағындарды қайта пайдалану үшін сәйкес құрастырушы функцияларын `@iroha2/torii-client` ішінен импорттаңыз.

## 6. Қатені өңдеу

Барлық SDK қоңыраулары тасымалдау метадеректері бар бай `ToriiClientError` даналарын шығарады
және Norito қатесінің пайдалы жүктемесі. Қоңырауларды `try/catch` ішіне ораңыз немесе `.catch()` пайдаланыңыз.
пайдаланушыларға беткі контекст:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Келесі қадамдар

- `javascript/iroha_js/recipes/` ішіндегі рецепттерді ақырғы ағындар үшін зерттеңіз.
- Толық ақпарат алу үшін `javascript/iroha_js/index.d.ts` ішіндегі жасалған түрлерді оқыңыз
  әдіс қолтаңбалары.
- Пайдалы жүктемелерді тексеру және жөндеу үшін осы SDK-ны Norito жылдам іске қосу құралымен жұптаңыз
  Torii нөміріне жібересіз.