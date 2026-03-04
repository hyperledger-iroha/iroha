---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/javascript.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Краткое руководство по JavaScript SDK
описание: Создавайте транзакции, транслируйте события и управляйте предварительным просмотром Connect с помощью `@iroha/iroha-js`.
пуля: /sdks/javascript
---

`@iroha/iroha-js` — это канонический пакет Node.js для взаимодействия с Torii. Это
включает в себя сборщики Norito, помощники Ed25519, утилиты разбиения на страницы и устойчивый
Клиент HTTP/WebSocket, позволяющий зеркально отображать потоки CLI из TypeScript.

## Установка

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Шаг сборки завершает `cargo build -p iroha_js_host`. Убедитесь, что цепочка инструментов из
`rust-toolchain.toml` доступен локально перед запуском `npm run build:native`.

## Управление ключами

```ts
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(verifyEd25519(message, signature, publicKey));

const derived = publicKeyFromPrivate(privateKey);
console.assert(Buffer.compare(derived, publicKey) === 0);
```

## Создание транзакций

Разработчики инструкций Norito нормализуют идентификаторы, метаданные и количества, поэтому
закодированные транзакции соответствуют полезным нагрузкам Rust/CLI.

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "rose#wonderland#alice",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "rose#wonderland#alice",
  destinationAccountId: "ih58...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "ih58...",
  mint: { assetId: "rose#wonderland#alice", quantity: "10" },
  transfers: [{ destinationAccountId: "ih58...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii конфигурация клиента

`ToriiClient` принимает ручки повтора/тайм-аута, которые отражают `iroha_config`. Использование
`resolveToriiClientConfig` для объединения объекта конфигурации CamelCase (нормализовать
сначала `iroha_config`), переопределения env и встроенные параметры.

```ts
import { ToriiClient, resolveToriiClientConfig } from "@iroha/iroha-js";
import fs from "node:fs";

const rawConfig = JSON.parse(fs.readFileSync("./iroha_config.json", "utf8"));
const config = rawConfig?.torii
  ? {
      ...rawConfig,
      torii: {
        ...rawConfig.torii,
        apiTokens: rawConfig.torii.api_tokens ?? rawConfig.torii.apiTokens,
      },
    }
  : rawConfig;
const clientConfig = resolveToriiClientConfig({
  config,
  overrides: { timeoutMs: 2_000, maxRetries: 5 },
});

const torii = new ToriiClient(
  config?.torii?.address ?? "http://localhost:8080",
  {
    config,
    timeoutMs: clientConfig.timeoutMs,
    maxRetries: clientConfig.maxRetries,
  },
);
```

Переменные среды для локальной разработки:

| Переменная | Цель |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Тайм-аут запроса (миллисекунды). |
| `IROHA_TORII_MAX_RETRIES` | Максимальное количество повторных попыток. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Отсрочка первоначальной повторной попытки. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Экспоненциальный множитель отсрочки. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Максимальная задержка повтора. |
| `IROHA_TORII_RETRY_STATUSES` | Коды состояния HTTP, разделенные запятыми, для повторной попытки. |
| `IROHA_TORII_RETRY_METHODS` | HTTP-методы, разделенные запятыми, для повторной попытки. |
| `IROHA_TORII_API_TOKEN` | Добавляет `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | Добавляет заголовок `Authorization: Bearer …`. |

Профили повторных попыток отражают настройки Android по умолчанию и экспортируются для проверки четности:
И18НИ00000057Х, И18НИ00000058Х,
`DEFAULT_RETRY_PROFILE_STREAMING`. См. `docs/source/sdk/js/torii_retry_policy.md`.
для сопоставления конечной точки с профилем и аудита управления параметрами во время
JS4/JS7.

## Итерируемые списки и нумерация страниц

Помощники по нумерации страниц отражают эргономику Python SDK для `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFT, балансы, держатели активов и
история транзакций по счету.

```ts
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({
  pageSize: 50,
  maxItems: 200,
})) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const assetId = "rose#wonderland#alice@test";
const balances = await torii.listAccountAssets("alice@test", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("alice@test", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Офлайн-допуски и метаданные вердиктов

Ответы на офлайн-результаты заранее раскрывают метаданные расширенного реестра —
И18НИ00000064Х, И18НИ00000065Х, И18НИ00000066Х, И18НИ00000067Х,
`attestation_nonce_hex` и `remaining_amount` возвращаются вместе с необработанными данными.
запись, чтобы панелям мониторинга не приходилось декодировать встроенные полезные данные Norito. Новый
помощники обратного отсчета (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) выделить следующий истекающий срок (обновить → политика
→ сертификат), чтобы значки пользовательского интерфейса могли предупреждать операторов о превышении допуска.
Осталось <24 ч. SDK
отражает фильтры REST, представленные `/v1/offline/allowances`:
И18НИ00000075Х, И18НИ00000076Х,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` и
`requireVerdict` / `onlyMissingVerdict` логические значения. Недопустимые комбинации (для
пример `onlyMissingVerdict` + `verdictIdHex`) отклоняются локально до Torii
называется.

```ts
const { items: allowances } = await torii.listOfflineAllowances({
  limit: 25,
  policyExpiresBeforeMs: Date.now() + 86_400_000,
  requireVerdict: true,
});

for (const entry of allowances) {
  console.log(
    entry.controller_display,
    entry.remaining_amount,
    entry.verdict_id_hex,
    entry.refresh_at_ms,
  );
}
```

## Оффлайн пополнения (оформить + зарегистрироваться)Используйте помощники пополнения счета, когда хотите оформить сертификат и сразу
зарегистрировать его в реестре. SDK проверяет выданный и зарегистрированный сертификат.
Идентификаторы совпадают перед возвратом, и ответ включает обе полезные нагрузки. Есть
нет выделенной конечной точки пополнения счета; помощник связывает проблему + регистрирует вызовы. Если
у вас уже есть подписанный сертификат, позвоните по `registerOfflineAllowance` (или
`renewOfflineAllowance`) напрямую.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "alice@wonderland",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "alice@wonderland",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii запросы и потоковая передача (WebSockets)

Помощники запросов предоставляют статус, метрики Prometheus, снимки телеметрии и события.
потоки с использованием грамматики фильтра Norito. Потоковая передача автоматически обновляется до
WebSockets и возобновляет работу, когда позволяет бюджет повторных попыток.

```ts
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

const abort = new AbortController();
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
  signal: abort.signal,
})) {
  console.log(event.id, event.data);
  break;
}
abort.abort(); // closes the underlying WebSocket cleanly
```

Используйте `streamBlocks`, `streamTransactions` или `streamTelemetry` для другого
Конечные точки WebSocket. Все помощники потоковой передачи выявляют повторные попытки, поэтому перехватите
Обратный вызов `onReconnect` для подачи информационных панелей и оповещений.

## Снимки Explorer и полезные данные QR

Телеметрия Explorer предоставляет типизированные помощники для `/v1/explorer/metrics` и
Конечные точки `/v1/explorer/accounts/{account_id}/qr`, чтобы информационные панели могли воспроизводить
те же снимки, которые питают портал. `getExplorerMetrics()` нормализует
полезная нагрузка и возвращает `null`, когда маршрут отключен. Соедините его с
`getExplorerAccountQr()` всякий раз, когда вам нужны литералы IH58 (предпочтительный)/sora (второй лучший) плюс встроенные
SVG для кнопок «Поделиться».

```ts
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("last block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("ih58...", {
  addressFormat: "compressed",
});
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

Передача `addressFormat: "compressed"` отражает сжатый файл Explorer по умолчанию.
селекторы; опустите переопределение для предпочтительного выхода IH58 или запросите `ih58_qr`
когда вам нужен QR-безопасный вариант. Сжатый литерал является вторым лучшим
Вариант только для Sora для UX. Помощник всегда возвращает канонический идентификатор,
выбранный литерал и метаданные (префикс сети, версия/модули QR, ошибка
уровень коррекции и встроенный SVG), поэтому CI/CD может публиковать те же полезные данные, которые
Explorer появляется без вызова специальных преобразователей.

## Подключение сеансов и очередей

Помощники Connect отражают `docs/source/connect_architecture_strawman.md`.
Самый быстрый путь к сеансу, готовому к предварительной версии, — `bootstrapConnectPreviewSession`,
который объединяет детерминированную генерацию SID/URI и Torii
регистрационный звонок.

```ts
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(
  torii,
  {
    chainId: "sora-mainnet",
    node: "https://torii.nexus.example",
    sessionOptions: { node: "https://torii.backup.example" },
  },
);

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- Передайте `register: false`, если вам нужны только детерминированные URI для QR/Deeplink.
  превью.
- `generateConnectSid` остается доступным, когда вам нужно получить идентификаторы сеансов.
  без создания URI.
- Ключи направления и конверты зашифрованного текста поступают с родного моста; когда
  недоступен, SDK возвращается к кодеку JSON и выдает
  `ConnectQueueError.bridgeUnavailable`.
— Автономные буферы хранятся как большие двоичные объекты Norito `.to` в IndexedDB. Мониторинг очереди
  состояние через излучаемый `ConnectQueueError.overflow(limit)` /
  Ошибки `.expired(ttlMs)` и подайте телеметрию `connect.queue_depth`, как описано.
  в дорожной карте.

### Подключение снимков реестра и политикОператоры платформы могут просматривать и обновлять реестр Connect без
покидаю Node.js. `iterateConnectApps()` просматривает реестр, в то время как
`getConnectStatus()` и `getConnectAppPolicy()` предоставляют счетчики времени выполнения и
текущий политический пакет. `updateConnectAppPolicy()` принимает поля CamelCase,
поэтому вы можете разместить ту же полезную нагрузку JSON, которую ожидает Torii.

```ts
const status = await torii.getConnectStatus();
console.log("connect enabled:", status?.enabled ?? false);
console.log("active sessions:", status?.sessionsActive ?? 0);
console.log("buffered bytes:", status?.totalBufferBytes ?? 0);

for await (const app of torii.iterateConnectApps({ limit: 100 })) {
  console.log(app.appId, app.namespaces, app.policy?.relayEnabled ? "relay" : "wallet-only");
}

const policy = await torii.getConnectAppPolicy();
if ((policy.wsPerIpMaxSessions ?? 0) < 5) {
  await torii.updateConnectAppPolicy({
    wsPerIpMaxSessions: 5,
    pingIntervalMs: policy.pingIntervalMs ?? 30_000,
    pingMissTolerance: policy.pingMissTolerance ?? 3,
  });
}
```

Перед применением всегда делайте последний снимок `getConnectStatus()`.
мутации — контрольный список управления требует доказательств того, что обновления политики начинаются
от текущих пределов флота.

### Подключение набора номера через WebSocket

`ToriiClient.openConnectWebSocket()` собирает канонический
URL-адрес `/v1/connect/ws` (включая `sid`, `role` и параметры токена), обновления
`http→ws` / `https→wss` и передает конечный URL-адрес любому веб-сокету.
реализация, которую вы предоставляете. Браузеры автоматически повторно используют глобальные
`WebSocket`. Вызывающие Node.js должны передать конструктор, например `ws`:

```ts
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL ?? "https://torii.nexus.example");
const preview = await torii.createConnectSessionPreview({ chainId: "sora-mainnet" });
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});

socket.addEventListener("message", (event) => {
  console.log("Connect payload", event.data);
});
socket.addEventListener("close", () => {
  console.log("Connect socket closed");
});

socket.binaryType = "arraybuffer";
socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind);
    return;
  }
  pendingFrames.enqueue(new Uint8Array(event.data));
});
```

Если вам нужен только URL-адрес, позвоните по `torii.buildConnectWebSocketUrl(params)` или
помощник верхнего уровня `buildConnectWebSocketUrl(baseUrl, params)` и повторно используйте
результирующая строка в настраиваемом транспорте/очереди.

Ищете полный образец, ориентированный на CLI?
[Рецепт подключения предварительного просмотра] (./recipes/javascript-connect-preview.md) включает в себя
исполняемый сценарий плюс руководство по телеметрии, которое отражает план действий, который может быть достигнут
документирование очереди Connect + потока WebSocket.

### Телеметрия и оповещения очередей

Передавайте метрики очереди непосредственно во вспомогательные поверхности, чтобы панели мониторинга могли зеркально отражаться.
KPI дорожной карты.

```ts
import { bootstrapConnectPreviewSession, ConnectQueueError } from "@iroha/iroha-js";

async function dialWithTelemetry(client: ToriiClient) {
  try {
    const { session } = await bootstrapConnectPreviewSession(client, { chainId: "sora-mainnet" });
    queueDepthGauge.record(session.queue_depth ?? 0);
    // …open the WebSocket here…
  } catch (error) {
    if (error instanceof ConnectQueueError) {
      if (error.kind === ConnectQueueError.KIND.OVERFLOW) {
        queueOverflowCounter.add(1, { limit: error.limit ?? 0 });
      } else if (error.kind === ConnectQueueError.KIND.EXPIRED) {
        queueExpiryCounter.add(1, { ttlMs: error.ttlMs ?? 0 });
      }
      return;
    }
    throw error;
  }
}
```

`ConnectQueueError#toConnectError()` преобразует ошибки очереди в общие
`ConnectError`, чтобы общие перехватчики HTTP/WebSocket могли выдавать
стандартные `connect.queue_depth`, `connect.queue_overflow_total` и
Метрики `connect.queue_expired_total` упоминаются в дорожной карте.

## Наблюдатели потоковой передачи и курсоры событий

`ToriiClient.streamEvents()` предоставляет `/v1/events/sse` как асинхронный итератор с автоматическим
повторных попыток, поэтому интерфейсы командной строки Node/Bun могут отслеживать активность конвейера так же, как это делает интерфейс командной строки Rust.
Сохраните курсор `Last-Event-ID` рядом с артефактами Runbook, чтобы операторы могли
возобновить поток, не пропуская события при перезапуске процесса.

```ts
import fs from "node:fs/promises";
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const cursorFile = process.env.STREAM_CURSOR_FILE ?? ".cache/torii.cursor";
const resumeId = await fs
  .readFile(cursorFile, "utf8")
  .then((value) => value.trim())
  .catch(() => null);
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  lastEventId: resumeId || undefined,
  signal: controller.signal,
})) {
  if (event.id) {
    await fs.writeFile(cursorFile, `${event.id}\n`, "utf8");
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] id=${event.id ?? "∅"} status=${status ?? "n/a"}`);
}
```

- Переключите `PIPELINE_STATUS` (например, `Pending`, `Applied` или `Approved`) или установите
  `STREAM_FILTER_JSON` для воспроизведения тех же фильтров, которые принимает CLI.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` сохраняет итератор активным до тех пор, пока не будет
  сигнал принят; передать `STREAM_MAX_EVENTS=25`, когда вам нужны только первые несколько событий
  для дымового теста.
- `ToriiClient.streamSumeragiStatus()` отражает тот же интерфейс для
  `/v1/sumeragi/status/sse`, чтобы консенсусную телеметрию можно было отслеживать отдельно, а
  итератор учитывает `Last-Event-ID` таким же образом.
- См. `javascript/iroha_js/recipes/streaming.mjs` для готового CLI (постоянство курсора,
  переопределения фильтров env-var и ведение журнала `extractPipelineStatusKind`), используемые в JS4.
  Дорожная карта потоковой передачи/WebSocket.

## Портфолио UAID и каталог пространств

API-интерфейсы Space Directory отражают жизненный цикл универсального идентификатора учетной записи (UAID).
помощники принимают литералы `uaid:<hex>` или необработанные 64-шестнадцатеричные дайджесты (LSB=1) и
канонизируйте их перед отправкой запросов:- `getUaidPortfolio(uaid, { assetId })` агрегирует балансы по пространству данных,
  группировка активов по каноническим идентификаторам учетных записей; передайте `assetId` для фильтрации
  портфель до одного экземпляра актива.
- `getUaidBindings(uaid, { addressFormat })` перечисляет каждое пространство данных ↔ учетную запись.
  привязка (`addressFormat: "compressed"` возвращает литералы `sora…`).
- `getUaidManifests(uaid, { dataspaceId })` возвращает каждый манифест возможностей,
  статус жизненного цикла и привязанные счета для аудита.

Для пакетов доказательств оператора, потоков публикации/отзыва манифестов и миграции SDK
руководству, следуйте Универсальному руководству по учетным записям (`docs/source/universal_accounts_guide.md`).
вместе с этими помощниками клиента, чтобы портал и исходная документация оставались синхронизированными.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "cash#global::holder@global",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, { addressFormat: "compressed" });
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

Операторы также могут менять манифесты или выполнять экстренные потоки отказа в победе без
переход к CLI. Оба помощника принимают необязательный объект `{ signal }`, поэтому
длительные отправки можно отменить с помощью `AbortController`; необъект
опции или входы, отличные от `AbortSignal`, вызывают синхронный `TypeError` перед
запросы попадают Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "ih58...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "ih58...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` принимает необработанный манифест JSON (соответствующий
светильники под `fixtures/space_directory/`) или любой объект, который сериализуется в
та же структура. `privateKey`, `privateKeyHex` или `privateKeyMultihash` сопоставляются с
поле `ExposedPrivateKey` Torii ожидает и по умолчанию имеет значение `ed25519`
алгоритм, когда префикс не указан. Оба запроса возвращаются после постановки Torii в очередь.
инструкцию (`202 Accepted`), после чего реестр выдаст
соответствующий `SpaceDirectoryEvent`.

## Управление и мост ISO

`ToriiClient` предоставляет API управления для проверки контрактов,
предложения, подача бюллетеней (обычных или ZK), ротация совета и созыв
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` без рукописных DTO. Помощники ISO 20022
следуйте той же схеме через `buildPacs008Message`/`buildPacs009Message` и
`submitIso*`/`waitForIsoMessageStatus` трио.

См. [рецепт моста управления и ISO] (./recipes/javascript-governance-iso.md).
для готовых к CLI образцов, а также ссылки на полное руководство по эксплуатации в
`docs/source/sdk/js/governance_iso_examples.md`.

## Отбор проб эритроцитов и доказательства доставки

Дорожная карта JS также требует выборки Roadrunner Block Commitment (RBC), чтобы операторы могли
доказать, что блок, который они получили через Sumeragi, соответствует подтверждению фрагмента, которое они проверяют.
Используйте встроенные помощники вместо создания полезных данных вручную:

1. `getSumeragiRbcSessions()` зеркально отражает `/v1/sumeragi/rbc/sessions`, и
   `findRbcSamplingCandidate()` автоматически выбирает первый доставленный сеанс с хешем блока.
   (пакет интеграции возвращается к нему всякий раз, когда
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` не установлен).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` нормализует `{blockHash,height,view}`.
   плюс необязательные переопределения `{count,seed,apiToken}`, поэтому искаженные шестнадцатеричные или отрицательные целые числа никогда не
   достичь Torii.
3. `sampleRbcChunks()` отправляет запрос POST на `/v1/sumeragi/rbc/sample`, возвращая доказательства фрагмента.
   и пути Меркла (`samples[].chunkHex`, `chunkRoot`, `payloadHash`), которые вы должны заархивировать с помощью
   остальные доказательства вашего усыновления.
4. `getSumeragiRbcDelivered(height, view)` фиксирует метаданные о доставке когорты, чтобы аудиторы
   может воспроизвести доказательство от начала до конца.

```js
import assert from "node:assert";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080", {
  apiToken: process.env.TORII_API_TOKEN,
});

const candidate =
  (await torii.findRbcSamplingCandidate().catch(() => null)) ??
  (await torii.getSumeragiRbcSessions()).items.find((session) => session.delivered);
if (!candidate) {
  throw new Error("no delivered RBC session available; set IROHA_TORII_INTEGRATION_RBC_SAMPLE");
}

const request = ToriiClient.buildRbcSampleRequest(candidate, {
  count: Number(process.env.RBC_SAMPLE_COUNT ?? 2),
  seed: Number(process.env.RBC_SAMPLE_SEED ?? 0),
  apiToken: process.env.RBC_SAMPLE_API_TOKEN ?? process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks(request);
sample.samples.forEach((chunk) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, "chunk must be hex");
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```Сохраните оба ответа в корне артефакта, который вы передаете управлению. Переопределить
автоматически выбранный сеанс через `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
всякий раз, когда вам нужно проверить определенный блок и рассматривать неудачи при получении снимков RBC как
ошибка предполетного стробирования, а не автоматический переход в прямой режим.

## Тестирование и CI

1. Кэшируйте груз и артефакты npm.
2. Запустите `npm run build:native`.
3. Выполните `npm test` (или `node --test` для заданий дыма).

Эталонный рабочий процесс GitHub Actions находится в
`docs/source/examples/iroha_js_ci.md`.

## Следующие шаги

- Просмотрите сгенерированные типы в `javascript/iroha_js/index.d.ts`.
- Изучите рецепты под `javascript/iroha_js/recipes/`.
- Соедините `ToriiClient` с кратким руководством Norito для параллельной проверки полезной нагрузки.
  SDK вызывает.