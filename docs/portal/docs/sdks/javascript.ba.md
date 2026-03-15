---
lang: ba
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c677012c88133bce76df1704fb9c23a98f4843d891cf13f48b5cebbe2d898ce6
source_last_modified: "2026-01-30T18:06:01.644371+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
slug: /sdks/javascript
translator: machine-google-reviewed
---

`@iroha/iroha-js` — Torii менән үҙ-ара эш итеү өсөн канонлы Node.js пакеты. Был
25519-сы ярҙамсылары, механиктар һәм ныҡлылыҡ
HTTP/WebSocket клиент, шулай итеп, һеҙ көҙгө CLI ағымдарын TypeScript.

## Ҡуйыу

I18NF000000018X

Төҙөү аҙымы I18NI000000038X XX. Ҡорал сылбырын тәьмин итеүҙән 2000 йылдан алып .
I18NI000000039X урындағы кимәлдә I18NI000000040X эшләй.

## Төп идара итеү

I18NF000000019X

## транзакциялар төҙөү

I18NT0000000003Х инструкция төҙөүселәр идентификаторҙарҙы, метамағлүмәттәрҙе һәм күләмдәрҙе нормаға килтерә.
кодланған транзакциялар тура килә Rust/CLI файҙалы йөкләмәләр.

I18NF000000020X

## I18NT000000009X клиент конфигурацияһы

`ToriiClient` ҡабул итә ретия/тайм-аут ручкалар, улар көҙгө `iroha_config`. Файҙаланыу
I18NI000000043X кадельКейс конфиг объектын берләштереү өсөн (нормалләштереү
`iroha_config` беренсе), env өҫтөнлөк итә, һәм рәт варианттары.

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

Тирә-яҡ мөхит үҙгәртеүселәре өсөн локаль dev:

| Үҙгәртеүсән | Маҡсат |
|---------|----------|
| `IROHA_TORII_TIMEOUT_MS` | Һорау тайм-аут (миллисекундтар). |
| `IROHA_TORII_MAX_RETRIES` | Максималь ҡабаттан тырышлыҡтар. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Башланғыс яңынан backfy. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Экспоненциаль backoff множитель. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Максималь ҡабаттан әҙерәк тотҡарлыҡ. |
| `IROHA_TORII_RETRY_STATUSES` | Комма-айырылған HTTP статусы кодтары ҡабаттан ҡарау өсөн. |
| `IROHA_TORII_RETRY_METHODS` | Комма-айырылған HTTP ысулдарын ҡабаттан ҡарау өсөн. |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` өҫтәй. |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` башын өҫтәй. |

Ҡайтанан профилдәр көҙгө Android ғәҙәттәгесә һәм паритет тикшерергә экспортлана:
I18NI000000056X, I18NI000000057X,
`DEFAULT_RETRY_PROFILE_STREAMING`. Ҡара: I18NI000000059X
өсөн, ос нөктәһе-профиль картаһы һәм параметрҙары менән идара итеү аудиттары ваҡытында 2012 йыл.
JS4/JS7.

## Iterable исемлектәре & pagination

Пагинация ярҙамсылары I18NI000000060X өсөн Python SDK эргономикаһын көҙгөләй.
I18NI000000061X, I18NI000000062X, НФТ, баланс, активтар эйәләре, һәм был
иҫәп операцияһы тарихы.

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
const balances = await torii.listAccountAssets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## офлай

Офлайн пособие яуаптары фашлау байытылған баш кейеме метамағлүмәттәр өҫкә-алғы —
I18NI000000063X, I18NI000000064X, I18NI000000065X, `verdict_id_hex`,
I18NI000000067X, һәм I18NI00000000068X сеймал менән бергә ҡайтарыла
рекорд шулай приборҙар таҡталары don’t тейеш, тип расшифровка встроенный I18NT0000000004X файҙалы йөк. Яңы
18NI0000069X, I18NI000000070X, `deadline_ms`,
I18NI000000072Х) сираттағы срокты айырып күрһәтә (яңыртыу → сәйәсәте
→ сертификат) шулай итеп, UI значоктары операторҙарҙы иҫкәртергә мөмкин, ҡасан пособие бар
<24 сәғәт ҡалған. СДК
I18NI000000073X тарафынан фашланған REST фильтрҙарын көҙгөләй:
`certificateExpiresBeforeMs/AfterMs`, I18NI000000075X,
I18NI000000076X, `attestationNonceHex`, `refreshBeforeMs/AfterMs` һәм
`requireVerdict` / I18NI000000080X бул. Дөрөҫ булмаған комбинациялар ( өсөн
миҫал `onlyMissingVerdict` + `verdictIdHex`) урындағы кимәлдә Torii тиклем кире ҡағыла.
тип атала.

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

## офлайн топ-аптар (эш + регистр)

Ҡулланығыҙ, ярҙамсыларҙы тулыландырыу, ҡасан һеҙ теләйһегеҙ, сертификат бирергә һәм шунда уҡ
уны теркәүҙә. SDK тикшерелгән һәм теркәлгән сертификат раҫлай
Идентификаторҙар ҡайтҡансы тап килә, ә яуап ике файҙалы йөктө лә үҙ эсенә ала. Бында бар
бағышланған өҫкө-өҫкә ос нөктәһе юҡ; ярҙамсы мәсьәләне сылбырлай + регистр шылтыратыуҙар. Әгәр
һеҙ инде ҡул ҡуйылған сертификат, шылтыратыу I18NI0000000083X (йәки
`renewOfflineAllowance` туранан-тура.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_i105>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_i105>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## I18NT000000011X эҙләүҙәр & потоковый (WebSockets)

Һорау ярҙамсылары статусын фашлай, I18NT000000000000 метрикаһы, телеметрия снимоктары һәм ваҡиға
I18NT000000005X фильтр грамматикаһы ярҙамында ағымдар. Ағым автоматик рәүештә яңыртыу өсөн .
WebSockets һәм резюме ҡасан ҡайтанан тырышып бюджет мөмкинлек бирә.

I18NF000000025X

Ҡулланыу I18NI0000000085X, I18NI000000086X, йәки I18NI000000087X өсөн икенсеһе
WebSocket остары. Бөтә стриминг ярҙамсылары өҫтөндә ҡабаттан тырышып тырышлыҡтар, шулай итеп, ҡармаҡ
I18NI000000088X шылтыратыу приборҙар таҡталары һәм иҫкәртергә.

## Эксплорер снимоктар & QR файҙалы йөкләмәләр

Эксплорер телеметрия I18NI000000089X һәм
I18NI000000090X ос нөктәләре, шуға күрә приборҙар таҡталары реплей мөмкин
шул уҡ снимоктар, тип ҡөҙрәт портал. I18NI000000091X нормализацияһы
файҙалы йөк һәм ҡайтарыу I18NI0000000092X ҡасан маршрут өҙөлгән. Уны пар менән
I18NI00000000933Х ҡасан кәрәк I105 (өҫтөнлөк)/сора (икенсе-иң яҡшы) литералдар плюс р.
СВГ өсөн акциялар төймәләре.

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

const qr = await torii.getExplorerAccountQr("i105...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

I18NI000000094X аша үткән Көҙгөләр Explorer’s стандарт ҡыҫылған
селекторҙар; өҫтөнлөклө I105 сығыш йәки үтенес өсөн өҫтөнлөктө үткәрмәү I18NI0000000955.
ҡасан һеҙгә кәрәк QR-хәүефһеҙ вариант. Ҡыҫылған туранан-тура икенсе иң яҡшы .
Сора-тик вариант өсөн UX. Ярҙам һәр ваҡыт канонлы идентификаторҙы кире ҡайтара,
һайланған туранан-тура, һәм метамағлүмәттәр (селтәр префикс, QR версия/модулдәр, хата
төҙәтеү ярус, һәм рәтле SVG), шуға күрә CI/CD шул уҡ файҙалы йөктәрҙе баҫтырып сығара ала, тип
эксплуатациялаусы ер өҫтө өндәшмәйенсә, заказ буйынса конвертерҙар.

## Тоташтырыу сессиялары & сират

Тоташтырыусы ярҙамсылары I18NI000000966X XX. 1990 й.
тиҙ юл алдан ҡарау-әҙер сессия I18NI0000000097X,
улар бергә һыҙылған детерминистик SID/URI быуын һәм I18NT000000012X
теркәү шылтыратыу.

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

- QR/deeplink өсөн детерминистик URI ғына кәрәк булғанда I18NI000000098X үткән
  алдан ҡарау.
- I18NI0000000099X X сессия ids алыу кәрәк булғанда ҡала
  УРИ-ны һуғыуһыҙ.
- Йүнәлешле асҡыстар һәм шифрлы конверттар тыуған күперҙән килә; ҡасан
  булмаған SDK кире JSON кодек һәм ташлай
  `ConnectQueueError.bridgeUnavailable`.
- офлайн буферҙар I18NT000000006X `.to` блобтары булараҡ һаҡлана IndexedDB. Сират мониторы
  дәүләт аша сығарылған I18NI000000102X /
  I18NI000000103X хаталары һәм туҡланыу I18NI0000000104X телеметрияһы тип билдәләнгән
  юл картаһында.

### Тоташтырыу реестры & сәйәсәт снимок

Платформа операторҙары үҙ-үҙенә эйә була ала һәм яңырта Connect реестры .
ҡалдыра Node.js. `iterateConnectApps()` биттәр аша реестр, шул уҡ ваҡытта
`getConnectStatus()` һәм `getConnectAppPolicy()` эшләй.
ғәмәлдәге сәйәсәт конверты. `updateConnectAppPolicy()` кадельКейс ятҡылыҡтарын ҡабул итә,
тимәк, һеҙ шул уҡ JSON файҙалы йөк һалырға мөмкин, тип I18NT0000000013X көтә.

I18NF000000028X

Һәр ваҡыт тотоп һуңғы I18NI00000000109X снимок ғариза биргәнсе
мутациялар — идара итеү тикшерелгән исемлеге сәйәсәтте яңыртыуҙар башлана, тип дәлилдәр талап итә
автопарктан’ағымдағы сиктәр.

### WebSocket йыя

I18NI000000110X канон йыя
`/v2/connect/ws` URL (шул иҫәптән I18NI000000112X, `role`, һәм жетон параметрҙары), яңыртыуҙар
I18NI000000114X / `https→wss`, һәм һуңғы URL-адресты WebSocket
тормошҡа ашырыу һеҙ тәьмин итеү. Браузерҙар автоматик рәүештә глобаль ҡабаттан файҙаланыу .
`WebSocket`. Node.js шылтыратыусылар I18NI000000117X кеүек конструктор үтергә тейеш:

I18NF000000029X.

Ҡасан һеҙгә кәрәк тик URL, шылтыратыу I18NI000000118X йәки
юғары кимәлдәге I18NI000000119X ярҙамсыһы һәм ҡабаттан ҡулланыу
һөҙөмтәлә епкә ҡулланыусы транспорт/сиратта.

Тулы CLI-ориентированный өлгө эҙләй? 1990 й.
[Тоташыу алдан ҡарау рецепты](I18NU000000035X) үҙ эсенә ала
йүгерергә яраҡлы сценарий плюс телеметрия етәкселеге, тип көҙгө юл картаһы тапшырыу өсөн
документлаштырыу Тоташтырыу сираты + WebSocket ағымы.

### Сират телеметрия & иҫкәртмә

Сым сират метрикаһы туранан-тура ярҙамсы өҫтөнә инә, шуға күрә приборҙар таҡталары көҙгө ала
юл картаһы КПИ-лар.

I18NF000000030X

I18NI000000120X сират етешһеҙлектәрен дөйөм үҙгәртә
I18NI000000121X таксономияһы шулай уртаҡ HTTP/WebSocket перехватчиктар сығара ала
`connect.queue_depth` стандарты, I18NI000000123X, һәм
`connect.queue_expired_total` метрикаһы бөтә юл картаһы буйлап һылтанма яһалған.

## Стриминг күҙәтеүселәр & ваҡиға курсорҙары

I18NI000000125X I18NI000000126X автоматик менән асинк итератор булараҡ фашлай
Ретиялар, шулай итеп, төйөн/Бунлы CLIs ҡойроҡ торба эшмәкәрлеге эшмәкәрлеге шул уҡ ысул менән Rust CLI эшләй.
Персист I18NI000000127X курсоры менән бергә һеҙҙең runbook артефакттар, шулай итеп, операторҙар ала
тергеҙергә ағым ағымы ваҡиғаларҙы үткәрмәйенсә, ҡасан процесс ҡабаттан эшләй башлай.

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

- I18NI000000128X Switch
  I18NI000000132X CLI ҡабул иткән шул уҡ фильтрҙарҙы ҡабатлау өсөн.
- I18NI000000133X был итераторҙы тере тота, тик
  сигнал ҡабул ителә; үткәреү I18NI000000134X, ҡасан һеҙгә кәрәк генә тәүге бер нисә ваҡиғалар .
  төтөн анализы өсөн.
- I18NI000000135X өсөн шул уҡ интерфейс көҙгөләй.
  I18NI000000136XX шулай консенсус телеметрияһы айырым ҡойроҡло була ала, һәм
  итератор `Last-Event-ID` XIX 1890 йылда ла шулай уҡ хөрмәт итә.
- Ҡарағыҙ I18NI000000138X өсөн ток асҡыс CLI (курсор ныҡышмалы,
  env-var фильтрҙары өҫтөнлөк итә, һәм I18NI000000139X логин) JS4-тә ҡулланыла.
  потоковый/WebSocket юл картаһы тапшырыу.

## UAID портфелдәре & Йыһан каталогы

Йыһан каталогы API-лар өҫтөндә дөйөм иҫәп идентификаторы (UAID) йәшәү циклы. 1990 й.
ярҙамсылары ҡабул итә I18NI000000140X литерал йәки сеймал 64-гекс disists (LSB=1) һәм
уларҙы үтенестәр тапшырыр алдынан канонлаштырығыҙ:

- `getUaidPortfolio(uaid, { assetId })` агрегаттары мәғлүмәттәр киңлегенә баланстары,
  төркөмләү активтар холдингы канон иҫәп идентификаторҙары буйынса; үткәреү I18NI000000142X фильтрлау өсөн
  портфолио бер актив экземплярына тиклем.
- I18NI000000143X һәр мәғлүмәт киңлеге ↔ иҫәп яҙмаһын иҫәпләп сыға
  бәйләү (`I105` I18NI000000145X литералдарын ҡайтара).
- `getUaidManifests(uaid, { dataspaceId })` һәр мөмкинлекте ҡайтара, манифест,
  тормош циклы статусы, һәм аудит өсөн бәйле иҫәп-хисап.Оператор өсөн дәлилдәр пакеттары, асыҡ баҫтырыу/ҡайтарала ағымдар, һәм SDK миграция .
етәкселек, Универсаль иҫәп яҙмаһы буйынса ҡулланма (I18NI000000147X)
был клиент ярҙамсылары менән бер рәттән, шулай итеп, портал һәм сығанаҡ документацияһы синхронизация ҡала.

I18NF000000032X

Операторҙар шулай уҡ әйләндерә ала, йәки үтәй ғәҙәттән тыш хәлдәр инҡар итеү-еңеп ағымдарһыҙ .
CLI-ға төшөү. Ике ярҙамсы ла ҡабул итә опциональ I18NI000000148X объект шулай
оҙаҡ эшләгән тапшырыуҙарҙы `AbortController` менән туҡтатырға мөмкин; булмаған
варианттары йәки I18NI000000150X индереүҙәр синхрон `TypeError` тиклем күтәрелә.
запрос Torii хиттары:

I18NF000000033X

I18NI000000152X ҡабул итә йәки сеймал манифест JSON (тапировка
`fixtures/space_directory/` буйынса ҡорамалдар йәки ниндәй ҙә булһа объект, улар сериализацияларға
шул уҡ структура. `privateKey`, I18NI0000001555Х, йәки `privateKeyMultihash` картаһы.
`ExposedPrivateKey` яланы I18NT000000015X көтә һәм ғәҙәттәгесә `ed25519`
алгоритмы ҡасан бер ниндәй ҙә префикс бирелмәй. Ике үтенес тә бер тапҡыр ҡайтара I18NT0000000016X эҙләнеүҙәре
инструкция (`202 Accepted`), был мәлдә баш китап сығарасаҡ
тап килгән I18NI000000160X.

## Идара итеү & ISO күпере

I18NI000000161X идара итеү API-ларҙы контракттарҙы тикшергән өсөн фашлай, сәхнәләштереү
тәҡдимдәр, бюллетендәрҙе тапшырыу (ябай йәки З.К.), советты әйләндереп, шылтыратыу
`governanceFinalizeReferendumTyped` /
Ҡулдан яҙылған ДТО-ларһыҙ `governanceEnactProposalTyped`. ISO 20022 ярҙамсылары
`buildPacs008Message`/I18NI0000000165X һәм был ҡалыпты үтәргә
I18NI0000166X/`waitForIsoMessageStatus` трио.

Ҡарағыҙ [идара итеү & ISO күпер рецепты](I18NU000000036X)
өсөн CLI-әҙер өлгөләр плюс күрһәткестәр кире тулы ялан етәкселек
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC үлсәү & тапшырыу дәлилдәре

JS юл картаһы шулай уҡ Roadrunner Блок йөкләмәһен талап итә (РБК) үлсәү, шулай итеп, операторҙар ала
иҫбатлауынса, улар I18NT000000001X аша алынған блок улар раҫлай торған өлөшләтә дәлилдәргә тап килә.
Ҡул менән файҙалы йөктәр төҙөү урынына төҙөлгән ярҙамсыларҙы ҡулланығыҙ:

1. `getSumeragiRbcSessions()` X көҙгө `/v2/sumeragi/rbc/sessions`, һәм
   I18NI000000171X авто-һайлай беренсе тапшырылған сеанс менән блок хеш
   (интеграция люксы ҡасан да булһа, уға кире төшә
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` необработанный).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` I18NI000000174X нормализацияһы
   Плю
   етергә I18NT0000000017X.
.
   һәм Меркл юлдары (I18NI000000178X, I18NI000000179X, `payloadHash`) һеҙ архив менән тейеш.
   ҡалған һеҙҙең тәрбиәгә алыу дәлилдәре.
4. I18NI000000181X когортаның тапшырыу метамағлүмәттәрен тота, шулай итеп аудиторҙар
   реплей оптаж осона тиклем.

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
```

Персистный ике яуап аҫтында артефакт тамыр һеҙ идара итеүгә тапшыра. Өҫтөнлөк
автоһайланған сессия аша I18NI0000000182X
Ҡасан ғына һеҙгә кәрәк, аныҡ блок тикшерергә, һәм дауалау етешһеҙлектәрен алыу өсөн эритроцит снимоктар а.
осош алдынан ҡапҡа хатаһы түгел, ә өнһөҙ генә туранан-тура режимға тиклем түбәнәйтергә.

## Һынау & CI

1. Кэш йөк һәм npm артефакттары.
2. Йүгереп I18NI000000183X.
3. `npm test` башҡарырға (йәки төтөн эш урындары өсөн I18NI000000185X).

Һылтанма GitHub Actions эш ағымы 1990 йылда йәшәй.
`docs/source/examples/iroha_js_ci.md`.

## Киләһе аҙымдар

- I18NI000000187X-тағы генерацияланған типтарҙы ҡабатлау.
- `javascript/iroha_js/recipes/` XX рецептарын тикшерергә.
- Пар I18NI0000000189X менән I18NT0000000007X frompart менән бер рәттән файҙалы йөктәрҙе тикшерергә
  СДК шылтыратыуҙары.