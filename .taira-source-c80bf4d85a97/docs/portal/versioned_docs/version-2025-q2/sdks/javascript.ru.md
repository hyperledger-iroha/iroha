---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Краткое руководство по JavaScript SDK

`@iroha2/torii-client` предоставляет браузер и дружественную к Node.js оболочку для Torii.
В этом кратком руководстве отражены основные процессы из рецептов SDK, поэтому вы можете получить
клиент запускается через несколько минут. Более полные примеры см.
`javascript/iroha_js/recipes/` в репозитории.

## 1. Установить

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Если вы планируете подписывать транзакции локально, также установите криптопомощники:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Создайте клиент Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Конфигурация отражает конструктор, используемый в рецептах. Если ваш узел
использует базовую аутентификацию, передайте `{username, password}` через опцию `basicAuth`.

## 3. Получить статус узла

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Все операции чтения возвращают объекты JSON, поддерживаемые Norito. См. сгенерированные типы в
`index.d.ts` для получения подробной информации о поле.

## 4. Отправьте транзакцию

Подписанты могут создавать транзакции с помощью вспомогательного API:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: '<i105-account-id>',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

Помощник автоматически упаковывает транзакцию в ожидаемый конверт Norito.
автор Torii. Более богатый пример (включая ожидание окончательности) см.
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Используйте помощников высокого уровня

SDK объединяет специализированные потоки, отражающие CLI:

- **Помощники в управлении** – `recipes/governance.mjs` демонстрирует постановку
  предложения и голосования с разработчиками инструкций `governance`.
- **Мост ISO** – `recipes/iso_bridge.mjs` показывает, как отправить `pacs.008` и
  опрос статуса передачи с использованием конечных точек `/v1/iso20022`.
- **SoraFS и триггеры** – помощники по разбиению на страницы под `src/toriiClient.js`.
  типизированные итераторы для контрактов, активов, триггеров и поставщиков SoraFS.

Импортируйте соответствующие функции компоновщика из `@iroha2/torii-client`, чтобы повторно использовать эти потоки.

## 6. Обработка ошибок

Все вызовы SDK создают богатые экземпляры `ToriiClientError` с транспортными метаданными.
и полезные данные ошибки Norito. Оберните вызовы в `try/catch` или используйте `.catch()` для
поверхностный контекст для пользователей:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Следующие шаги

- Изучите рецепты в `javascript/iroha_js/recipes/` для сквозных потоков.
- Прочтите сгенерированные типы в `javascript/iroha_js/index.d.ts` для получения подробной информации.
  сигнатуры методов.
- Соедините этот SDK с кратким руководством Norito для проверки и отладки полезных данных.
  вы отправляете на Torii.