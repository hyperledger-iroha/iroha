---
lang: ba
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Тоташтырыу сессияһын алдан ҡарау Рунбук (IOS7 / JS4)

Был runbook документтар өсөн стадиялау, раҫлау, һәм
өҙөп төшөп Connect алдан ҡарау сессиялары талап итә, юл картаһы осҡондары **IOS7**
һәм **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Был аҙымдарҙы үтәгеҙ, ҡасан
һеҙ демо-Коннект strawman (`docs/source/connect_architecture_strawman.md`),
сират/телеметрия ҡармаҡтарын ҡулланыу SDK юл карталарында вәғәҙә ителә, йәки йыйыу
`status.md` тураһында дәлилдәр.

## 1. Осоу алдынан тикшерелгән исемлек

| Элемент | Ентекле | Һылтанмалар |
|------|---------|------------ |
| Torii ос нөктәһе + Тоташыу сәйәсәте | Torii база URL, `chain_id`, һәм тоташтырыу сәйәсәте (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). JSON снимокты runbook билетында төшөрөп алырға. | Grafana, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Фикстура + күпер версиялары | Иҫкәрмә Norito ҡоролма хеш һәм күпер төҙөү һеҙ ҡулланасаҡ (Swift талап итә `NoritoBridge.xcframework`, JS талап итә `@iroha/iroha-js` ≥ версияһы, тип ебәргән `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` X |
| Телеметрия приборҙар таҡтаһы | `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, Grafana, һ.б. Prometheus снимоктары). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Дәлилдәр папкалар | `docs/source/status/swift_weekly_digest.md` (аҙна һайын һеңдерелгән) һәм `docs/source/sdk/swift/connect_risk_tracker.md` (хәүефле трекер) кеүек йүнәлеште һайлағыҙ. Магазин журналдары, метрика скриншоттар һәм `docs/source/sdk/swift/readiness/archive/<date>/connect/` буйынса таныуҙар. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Загрузка алдан ҡарау сессияһын

1. **Сәйәсәт + квоталар.** Шылтыратыу:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ``` X
   Үпкәү йүгерә, әгәр `queue_max` йәки TTL айырыла конфиг һеҙ планлаштырған
   тест.
2. **Детерминистик SID/URIs генерациялау.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` ярҙамсыһы SID/URI быуынын Torii менән бәйләй
   сеанс теркәү; уны ҡулланыу хатта ҡасан Swift водитель WebSocket ҡатламы.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false` комплекты ҡоро идара итеү өсөн QR/тәрән-бәйләнешле сценарийҙар.
   - Ҡайтып алынған `sidBase64Url`, deeplink URL-адрестар, һәм `tokens` тап 1990 йылда.
     дәлилдәр папкаһы; идара итеүгә рецензия был артефакттарҙы көтә.
3. **Серҙәрҙе таратыу.** Өслө һылтанма менән уртаҡлашыу URI менән янсыҡ операторы .
   (свифт dApp өлгөһө, Android янсыҡ, йәки QA йүгән). Бер ҡасан да сей токендарҙы йәбештермәгеҙ
   чатҡа инә; ҡулланыу шифрланған склеп документлаштырылған өҫтәмә пакет.

## 3. Сессияны йөрөтөү1. **WebSocket асырға.** Клиенттар свифт ғәҙәттә:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   Өҫтәмә ҡоролма өсөн Һылтанма `docs/connect_swift_integration.md`
   импорт, конкурентлыҡ адаптерҙары).
2. **Таны + билдә ағымы.** DApps шылтыратыу `ConnectSession.requestSignature(...)`,
   ә янсыҡтар `approveSession` аша яуап бирә / `reject`. Һәр раҫлау тейеш логин .
   хешэд псевдонимы + рөхсәттәр тура килтерергә Connect идара итеү уставы.
3. **Күнекмә сират + резюме юлдары.** Селтәр тоташыуын кәметергә йәки туҡтатыу .
   янсыҡты тәьмин итеү өсөн сикләнгән сират һәм реплей ҡармаҡтар журнал яҙмалары. JS/Android
   SDKs `ConnectQueueError.overflow(limit)` / сығара.
   `.expired(ttlMs)` улар рамдарҙы төшөргәндә; Свифт бер тапҡыр шул уҡ күҙәтергә тейеш
   IOS7 сират ҡоролмалары ерҙәре (`docs/source/connect_architecture_strawman.md`).
   Һеҙ яҙғандан һуң, кәмендә бер ҡабаттан тоташтырыу, йүгерергә
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (йәки экспорт каталогын `ConnectSessionDiagnostics` тарафынан ҡайтарған) һәм
   беркетелгән таблица/JSON runbook билетына. CLI шул уҡ уҡый .
   `state.json` / `metrics.ndjson` пары, тип `ConnectQueueStateTracker` X,
   шулай итеп, идара итеү рецензенттары эҙләй ала быраулау дәлилдәре, инструменталь инструменталь.

## 4. Телеметрия & Күҙәтеүсәнлек

- **Метриканы тотоу өсөн:**
  - `connect.queue_depth{direction}` датчигы (сәйәси капиталдан түбәнерәк ҡалырға тейеш).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` счетчигы (нуль булмаған
    етешһеҙлек-инъекция ваҡытында).
  - `connect.resume_latency_ms` гистограмма (яҙма p95 мәжбүрҙән һуң a
    яңынан тоташтырыу).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Свифт-специфик `swift.connect.session_event` һәм
    `swift.connect.frame_latency` экспорты (Torii).
- ** Приборҙар таҡталары:** Аннотация маркерҙары менән тоташтырылған таҡта закладкаларын яңыртыу.
  Беркетергә скриншоттар (йәки JSON экспорты) сеймал менән бер рәттән дәлилдәр папкаһына
  OTLP/Prometheus снимоктары телеметрия экспортеры аша тартылған CLI.
- **Иҫкәртмә:** Әгәр ниндәй ҙә булһа Sev1/2 сиктәре триггер (пер `docs/source/android_support_playbook.md` §5),
  битендә SDK программаһы етәксеһе һәм документ PagerDuty инцидент идентификаторы runbook
  билет дауам иткәнсе.

## 5. Таҙа & Ролбек

1. **Эшкә сәхнәләштерелгән сеанстарҙы юй.
   сигналдар мәғәнәле булып ҡала:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Swift-тик һынау өсөн йүгерә, шылтыратыу шул уҡ ос нөктәһе аша Rust/CLI ярҙамсыһы.
2. **Пурж журналдары.** Теләһә ниндәй һаҡланған сират журналдарын юйырға
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB магазиндарында һ.б.) шулай
   сираттағы йүгерә таҙа башлана. Яҙылған файл хеш юйыу алдынан, әгәр һеҙгә кәрәк
   отладка реплей мәсьәләһе.
3. **Файл ваҡиға иҫкәрмәләр.** Йүгереүҙе йомғаҡлау:
   - `docs/source/status/swift_weekly_digest.md`X (дельтас блогы), 1990 й.
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-200-се асыҡ йәки түбәнәйтелә.
     бер тапҡыр телеметрия урынында),
   - JS SDK demperglog йәки рецепт, әгәр яңы тәртип раҫланды.
4. **Уңышһыҙлыҡтарҙы көсәйтергә:**
   - Сират ташыу инъекцияһыҙ инъекцияһыҙ ⇒ ҡаршы хаталар ҡаршы SDK кем
     сәйәсәте Torii-тан айырылған.
   - Резюме хаталары ⇒ `connect.queue_depth` + `connect.resume_latency_ms` беркетегеҙ
     снимоктар инцидент отчеты.
   - Идара итеү тап килмәүе (токендар ҡабаттан ҡулланылған, TTL артып китте) ⇒ күтәреү менән SDK .
     Программа етәксеһе һәм аннотация `roadmap.md` сираттағы ҡабатлау ваҡытында.

## 6. Дәлилдәр тикшерелгән исемлек| Артефакт | Урыны |
|---------|-----------|
| SID/deeplink/токендар JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Приборҙар таҡтаһы экспорты (`connect.queue_depth` һ.б.) | `.../metrics/` подпапкаһы |
| PagerDuty / ваҡиға идентификаторҙары | `.../notes.md` |
| Таҙартыу раҫлау (Torii юйыу, журнал салфетка) | `.../cleanup.log` |

Был тикшерелгән исемлекте тамамлау ҡәнәғәтләндерә “доктар/йүгереп яңыртылған” сығыу критерийы .
IOS7/JS4 өсөн һәм идара итеү рецензенттарына һәр өсөн детерминистик эҙ бирә
Ҡушымта алдан ҡарау сессияһы.