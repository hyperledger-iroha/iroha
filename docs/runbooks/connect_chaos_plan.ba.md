---
lang: ba
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Хаос тоташтырыу & Репетиция планы етешһеҙлектәре (IOS3 / IOS7)

Был пьеса китабы IOS3/IOS7-не ҡәнәғәтләндергән ҡабатланған хаос күнекмәләрен билдәләй.
юл картаһы ғәмәле _ “план берлектәге хаос репетицияһы”_ (`roadmap.md:1527`). Уны пар менән
Connect алдан ҡарау runbook (`docs/runbooks/connect_session_preview_runbook.md`)
ҡасан сәхнәләштереү кросс-SDK демо.

## Маҡсаттар & Уңыш критерийҙары
- Дөйөм ҡулланыу Connect ретия/артҡы сәйәсәт, офлайн сират сиктәре, һәм
  телеметрия экспортерҙары контролдә тотолған етешһеҙлектәр аҫтында мутацияһыҙ етештереү коды.
- Детерминистик артефакттарҙы тотоу (`iroha connect queue inspect` сығыш,
  `connect.*` метрикаһы снимоктар, Swift/Android/JS SDK журналдары) шулай идара итеү ала
  һәр күнекмәләрҙе аудитория.
- Ҡушымталар һәм dApps конфиг үҙгәрештәрен хөрмәт итегеҙ (төшөү дрейфтары, тоҙ
  әйләнеш, аттестация етешһеҙлектәре) канонлы `ConnectError`-ты өҫтөн өҫкә күтәреп
  категорияһы һәм редакция-хәүефһеҙ телеметрия ваҡиғалары.

## Алдан шарттар
1. **Тикменселек загрузка**
   - Башланғыс демо Torii стека: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Кәмендә бер SDK өлгөһөн эшләтеп ебәрегеҙ (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Инструментация**
   - SDK диагностикаһын рөхсәт итеү (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` Свифтта; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     эквиваленттары Android/JS).
   - CLI `iroha connect queue inspect --sid <sid> --metrics` хәл итеүен тәьмин итеү
     SDK тарафынан етештерелгән сират юлы (`~/.iroha/connect/<sid>/state.json` һәм
     `metrics.ndjson`).
   - Сым телеметрия экспортерҙары шулай киләһе ваҡыт рәттәре 2012 йылда күренә.
     Grafana һәм `scripts/swift_status_export.py telemetry` аша: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Дәлилдәр папкалары** – `artifacts/connect-chaos/<date>/` булдырыу һәм магазин:
   - сеймал журналдары (`*.log`), метрика снимоктар (`*.json`), приборҙар таҡтаһы экспорты
     (`*.png`), CLI сығыштары, һәм PagerDuty идентификаторҙары.

## Сценарий матрицаһы| ID | Һынғыһыҙ | Инъекция аҙымдары | Көтөлгән сигналдар | Дәлилдәр |
|----|------|------------------------------------|-----------||
| С1 | WebSocket өҙөлгән & ҡабаттан тоташтырыу | 18NI00000000046X прокси артта ҡала (мәҫәлән, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) йәки ваҡытлыса хеҙмәтте блоклау (`kubectl scale deploy/torii --replicas=0` ≤60s өсөн). Көскөм янсыҡ ебәрергә рамкалар шулай офлайн сират тултырырға. | `connect.reconnects_total` өҫтәүҙәр, `connect.resume_latency_ms` шпиктар, әммә ҡала - Приборҙар таҡтаһы аннотацияһы өсөн өҙөлгән тәҙрә.- Өлгө журнал өҙөк менән ҡабаттан тоташтырыу + дренаж хәбәрҙәре. |
| С2 | Офлайн сират ташыу / TTL срогы | Өлгө патч өсөн ҡыҫҡартыу сират сиктәре (Swift: экземпляр `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` эсендә `ConnectSessionDiagnostics`; Android/JS тейешле конструкторҙар ҡулланыу). ≥2× `retentionInterval` өсөн янсыҡты туҡтатыу, ә dApp дымлы запростар һаҡлай. | `connect.queue_dropped_total{reason="overflow"}` һәм `{reason="ttl"}` өҫтәү, `connect.queue_depth` яңы сиктә, SDKs өҫтөндә `ConnectError.QueueOverflow(limit: 4)` (йәки Torii). `iroha connect queue inspect` `state=Overflow` X күрһәтә `warn/drop` һыу билдәләре менән 100%. | - метрик счетчиктар скриншот.- CLI JSON сығышын ташып ташлау.- Swift/Android журнал фрагменты составында `ConnectError` һыҙығы. |
| С3 | Манифест дрейф / ҡабул итеү кире ҡағыу | 18NI000000070X менән `--connect-manifest-path` менән Torii йәки `permissions` менән айырылып торған Torii-се башланғыс менән `docs/connect_swift_ios.md` өлгө манифестына хеҙмәт итә. dApp раҫлауҙы үтенес һәм тәьмин итеү өсөн янсыҡ сәйәсәт аша кире ҡаға. | Torii өсөн `/v2/connect/session` өсөн `manifest_mismatch`, SDKs `ConnectError.Authorization.manifestMismatch(manifestVersion)` сығарыу, телеметрия йыйыу Torii, һәм сираттар буш ҡала. (`state=Idle`). | - Torii журнал өҙөк күрһәтеү тап килмәү асыҡлау.- SDK скриншот ер өҫтө хатаһы.- Метрика снимок иҫбатлау өсөн бер ниндәй ҙә сират кадрҙар һынау ваҡытында. |
| С4 | Төп әйләнеш / тоҙ-версия ҡабарып | Тоташтырыу тоҙо йәки AEAD төймәһен урта сезия әйләндерергә. 18NI000000079X менән Torii ҡасып йөрөү стекаларында (`docs/source/sdk/android/telemetry_schema_diff.md`-та Android редакция тоҙ һынауын көҙгөләр). Тоҙ әйләнеше тамамланғансы янсыҡты офлайн тотоғоҙ, һуңынан тергеҙегеҙ. | Беренсе резюме тырышлығы менән уңышһыҙлыҡҡа осрай `ConnectError.Authorization.invalidSalt`, сират ҡыҙарыу (dApp рамкаларҙы ташлай рамкалар менән сәбәп `salt_version_mismatch`), телеметрия `android.telemetry.redaction.salt_version` (Android) һәм `swift.connect.session_event{event="salt_rotation"}` сығара. Икенсе сессиянан һуң SID яңыртыу уңышҡа өлгәшә. | - Приборҙар таҡтаһы аннотацияһы менән тоҙ эпохаһына тиклем/һуң.- Журналдар составында дөрөҫ булмаған-тоҙ хатаһы һәм артабанғы уңыш.- `iroha connect queue inspect` сығыш күрһәтеү `state=Stalled`, унан һуң яңы `state=Active`. || С5 | Аттестация / Көслө Box етешһеҙлеге | Android янсыҡтарында `ConnectApproval` XX конфигурациялау өсөн `attachments[]` + Стронг-Бокс аттестацияһын индерә. Аттестация жгут ҡулланыу (`scripts/android_keystore_attestation.sh` менән `--inject-failure strongbox-simulated`) йәки аттестация менән үҙгәртә JSON тапшырыу алдынан dApp. | DApp `ConnectError.Authorization.invalidAttestation` менән раҫлауҙы кире ҡаға, Torii логин етешһеҙлек сәбәбе, экспортерҙар `connect.attestation_failed_total` ҡабарта, ә сират таҙартыу рәнйеткес инеү. Swift/JS dApps хаталарҙы теркәү, шул уҡ ваҡытта сессияны йәшәтергә. | - Журнал менән инъекция етешһеҙлектәре ID.- SDK хата журналы + телеметрия ҡаршы тотоу.- дәлилдәре, тип сират насар кадрҙы алып ташланы (`recordsRemoved > 0`). |

## Сценарий деталдәре

### C1 — WebSocket өҙөлгән & ҡабаттан тоташтырыу
.
   һеҙ бөтә төйөн үлтермәйенсә, доступность үҙгәртә ала.
2. 45-се өҙөклөктө Триггер:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Телеметрия таҡталарын һәм `сценарий/свифт_статус_экспорты.py телеметрияны күҙәтеү
   --json-out артефакттар/тоташтырыу-хаос//c1_метры.json`.
4. Сүп-сар сират хәле өҙөклөктән һуң шунда уҡ:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Уңыш = бер тапҡыр ҡабаттан тоташтырыу тырышлығы, сикләнгән сират үҫеше һәм автоматик
   дренаждан һуң прокси тергеҙелә.

### C2 — офлайн сират ташыу / TTL срогы
1. Урындағы төҙөгәндә сират сиктәрен ҡыҫҡартыу:
   - Swift: яңыртыу Torii инициализатор эсендә һеҙҙең өлгө .
     (мәҫәлән, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` үтергә.
   - Android/JS: төҙөгәндә эквивалентлы конфиг объектын үтергә
     `ConnectQueueJournal`.
2. ≥60s өсөн янсыҡты (симулятор фоны йәки ҡоролма режимы) туҡтатығыҙ.
   ә dApp `ConnectClient.requestSignature(...)` шылтыратыуҙары мәсьәләләре.
3. Ҡулланыу `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) йәки JS
   диагностика ярҙамсыһы дәлилдәр өйөмөн экспортлау өсөн (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Уңыш = ташыу иҫәпләүселәре өҫтәү, SDK өҫтө `ConnectError.QueueOverflow`
   бер тапҡыр, ә сират янсыҡ тергеҙгәндән һуң тергеҙелә.

### С3 — Манифест дрейф / ҡабул итеү кире ҡағыу
1. Ҡабул итеү манифест күсермәһен эшләгеҙ, мәҫәлән:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. `--connect-manifest-path /tmp/manifest_drift.json` менән Torii.
   яңыртыу docker cotpose/k8s конфигурация өсөн бурау).
3. Кошелектан сеанс башларға тырышыу; 409-сы HTTP-ны көтөгөҙ.
.
   телеметрия приборҙар панелендә.
5. Уңыш = сират үҫеше булмаған кире ҡағыу, өҫтәүенә янсыҡ күрһәтелгән дөйөм
   таксономия хатаһы (`ConnectError.Authorization.manifestMismatch` X).### С4 — Төп әйләнеш / тоҙ ҡабарынҡыһы
1. Телеметриянан хәҙерге тоҙ версияһын яҙып алыу:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Яңы тоҙ менән Torii ҡабаттан башлау (`CONNECT_SALT_VERSION=$((OLD+1))` йәки яңыртыу
   конфигурация картаһы). Офлайн офлайн тотоп, тиклем перезапуск тамамланған.
3. Ҡунсыҡты тергеҙергә; беренсе резюме дөрөҫ булмаған-тоҙ хатаһы менән уңышһыҙлыҡҡа осрарға тейеш
   һәм `connect.queue_dropped_total{reason="salt_version_mismatch"}` өҫтәүҙәр.
4. Ҡушымтаны көсләп, кэшланған кадрҙарҙы ташларға, сессия каталогын юйып
   (`rm -rf ~/.iroha/connect/<sid>` йәки платформаға хас кэш асыҡ), һуңынан
   сессияны яңы жетондар менән яңынан эшләтергә.
5. Уңыш = телеметрия тоҙ ҡабарынҡы күрһәтә, дөрөҫ булмаған резюме ваҡиғаһы логин
   бер тапҡыр, һәм киләһе сессия ҡул ҡыҫылыуһыҙ уңышҡа өлгәшә.

### С5 — Аттестация / Көслө Box етешһеҙлектәре
1. `scripts/android_keystore_attestation.sh` ҡулланып аттестация өйөмө генерациялау
   (кормать `--inject-failure strongbox-simulated` ҡултамға бит әйләндерергә).
2. Ҡунсыҡ был өйөм аша уның `ConnectApproval` API аша беркетергә; dApp
   тейеш раҫлау һәм кире ҡағыу файҙалы йөк.
3. Телеметрияны тикшерергә (`connect.attestation_failed_total`, Свифт/Андроид ваҡиғаһы
   метрика) һәм сират ағыуланған яҙманы төшөрөп тәьмин итеү.
4. Уңыш = кире ҡағыу насар раҫлау өсөн айырыла, сират һау-сәләмәт ҡала,
   һәм аттестация журналы быраулау дәлилдәре менән һаҡлана.

## Дәлилдәр тикшерелгән исемлек
— `artifacts/connect-chaos/<date>/c*_metrics.json` экспорты 2012 йылдан алып.
  `scripts/swift_status_export.py telemetry`.
- CLI сығыштары (`c*_queue.txt`) `iroha connect queue inspect`-тан.
- SDK + Torii ваҡыт маркалары һәм СИД хештары менән журналдар.
- Һәр сценарий өсөн аннотациялар менән приборҙар таҡтаһы скриншоттары.
- PagerDuty / инцидент идентификаторҙары, әгәр Sev1/2 иҫкәртмәләр атыу.

Тулы матрицаны кварталға бер тапҡыр тамамлап, юл картаһы ҡапҡаһын ҡәнәғәтләндерә һәм
күрһәтә, тип Swift/Android/JS тоташтырыуҙы тормошҡа ашырыу детерминистик яуап бирә
иң юғары хәүефле етешһеҙлектәр режимдары буйынса.