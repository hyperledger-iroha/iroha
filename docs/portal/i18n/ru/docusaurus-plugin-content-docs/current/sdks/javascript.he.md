---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/javascript.he.md
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
  assetId: "norito:4e52543000000001",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "<i105-account-id>",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "<i105-account-id>",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "<i105-account-id>", quantity: "5" }],
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

const assetId = "norito:4e52543000000001";
const balances = await torii.listAccountAssets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Offline readiness

JavaScript integrations should use `GET /v1/offline/readiness?asset_definition_id=xor%23wonderland` for offline feature discovery.
Kagemusha readiness fields advertise the active offline payment implementation.

```ts
const readiness = await torii.getOfflineReadiness("xor#wonderland");
console.log("offline ready", readiness.ready, readiness.blockers);
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
`getExplorerAccountQr()` всякий раз, когда вам нужны литералы i105 (предпочтительный)/sora (второй лучший) плюс встроенные
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

const qr = await torii.getExplorerAccountQr("<i105-account-id>");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

Передача `i105` отражает сжатый файл Explorer по умолчанию.
селекторы; опустите переопределение для предпочтительного выхода i105 или запросите `i105_qr`
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

## Live event streams

`ToriiClient.streamEvents()` exposes `/v1/events/sse` as a live-only async
iterator. Torii retains no replay log for this route, so the helper has no
`lastEventId` option and reconnecting can leave a gap. A terminal
`event: stream_error` is yielded before the iterator ends; handle it explicitly
instead of treating closure as a lossless continuation point.

```js
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  signal: controller.signal,
})) {
  if (event.event === "stream_error") {
    console.error("terminal stream error", event.data);
    break;
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] status=${status ?? "n/a"}`);
}
```

- Switch `PIPELINE_STATUS` (for example `Pending`, `Applied`, or `Approved`) or set
  `STREAM_FILTER_JSON` to use the same filters the CLI accepts.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` keeps the iterator alive until a
  signal is received; pass `STREAM_MAX_EVENTS=25` when you only need the first few events
  for a smoke test.
- `ToriiClient.streamSumeragiStatus()` exposes the separate
  `/v1/sumeragi/status/sse` consensus telemetry feed.
- See `javascript/iroha_js/recipes/streaming.mjs` for a live-only turnkey CLI with
  environment-driven filters and explicit terminal-error handling.

## Портфолио UAID и каталог пространств

API-интерфейсы Space Directory отражают жизненный цикл универсального идентификатора учетной записи (UAID).
помощники принимают литералы `uaid:<hex>` или необработанные 64-шестнадцатеричные дайджесты (LSB=1) и
канонизируйте их перед отправкой запросов:- `getUaidPortfolio(uaid, { assetId })` агрегирует балансы по пространству данных,
  группировка активов по каноническим идентификаторам учетных записей; передайте `assetId` для фильтрации
  портфель до одного экземпляра актива.
- `getUaidBindings(uaid)` перечисляет каждое пространство данных ↔ учетную запись.
  привязка (`i105` возвращает литералы `i105`).
- `getUaidManifests(uaid, { dataspaceId })` возвращает каждый манифест возможностей,
  статус жизненного цикла и привязанные счета для аудита.

Для пакетов доказательств оператора, потоков публикации/отзыва манифестов и миграции SDK
руководству, следуйте Универсальному руководству по учетным записям (`docs/source/universal_accounts_guide.md`).
вместе с этими помощниками клиента, чтобы портал и исходная документация оставались синхронизированными.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "norito:4e52543000000002",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, {} );
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
    authority: "<i105-account-id>",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "<i105-account-id>",
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

## Sumeragi availability telemetry

Reliable broadcast remains an internal Sumeragi v2 transport and recovery mechanism.
The public Torii catalog exposes aggregate diagnostics through
`GET /v1/sumeragi/telemetry`; it does not publish per-session RBC state, chunk
samples, delivery probes, or a deterministic collector plan.

```js
const telemetry = await torii.getSumeragiTelemetryTyped();
console.log(`collector votes=${telemetry.availability.total_votes_ingested}`);
console.log(`pending sessions=${telemetry.rbc_backlog.pending_sessions}`);
```

Archive `availability.collectors`, `rbc_backlog`, and `rbc_pending` from the raw
telemetry response together with Prometheus counters and consensus logs. These
fields are aggregate operational evidence and must not be treated as light-client
chunk proofs or transaction-finality evidence.

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
