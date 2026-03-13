---
id: torii-app-api-parity
lang: ba
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Статус: 2026-03-21 йылдарҙы тамамланы.  
Хужалары: I18NT0000000008X платформаһы, SDK программаһы етәксеһе  
Юл картаһы һылтанмаһы: TORIII-APP-1 — I18NI0000000027X паритет аудиты

Был бит эске I18NI000000028X аудитын көҙгөләй (`docs/source/torii/app_api_parity_audit.md`)
тимәк, моно-репо тыш уҡыусылар күрә ала, ниндәй I18NI000000000030X өҫтө проводной, һынау,
һәм документлаштырылған. Аудит I18NI000000031X аша яңынан экспортланған маршруттарҙы күҙәтә,
I18NI000000032X, һәм I18NI000000033X.

## Scope & ысулы

Аудит йәмәғәт ҡабаттан экспортын тикшерә I18NI000000034X һәм
функция-ҡапҡа маршрут төҙөүселәр. Юл картаһында беҙ раҫланған һәр I18NI00000000035X өсөн беҙ раҫланыҡ:

- I18NI000000036X-та ручканы тормошҡа ашырыу һәм ДТО аныҡлауҙары.
- I18NI000000037X йәки I18NI0000000038X функцияһы төркөмдәре буйынса маршрутизатор теркәү.
- Ғәмәлдәге интеграция/блок һынауҙары һәм оҙайлы ваҡытҡа ҡаплау өсөн яуаплы милек командаһы.

Иҫәп активтары/транзакциялар һәм актив-холдер исемлеге ҡабул итеү опциональ I18NI000000039X эҙләү параметрҙары .
өсөн алдан фильтрлау, өҫтәүенә, ғәмәлдәге pagination/кире баҫым сиктәре.

## Аут & канон ҡул ҡуйыу

- App-йөҙ GET/POST ос нөктәләре ҡабул итеү опциональ канон запрос башлыҡтары (I18NI000000040X, `X-Iroha-Signature`) төҙөлгән I18NI0000000042Х; Torii уларҙы уратып I18NI000000043X башҡарыусы раҫлау алдынан, шулай итеп, улар көҙгө `/query`.
- SDK ярҙамсылары бөтә төп клиенттарҙа ла ташый:
  - JS/TS: I18NI0000045X I18NI000000046X-тан.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Андроид (Котлин/Ява): I18NI000000048X.
- Миҫал өҙөктәре:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v2/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v2/accounts/i105.../assets?limit=5`, { headers });
```
I18NF000000025X
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v2/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Аҙаҡҡы нөктә инвентаризацияһы

### Иҫәп рөхсәттәре (I18NI0000000049X) — Ҡапланған
- Ҡулға алынған: `handle_v1_account_permissions` (I18NI000000051X).
- ДТО: `filter::Pagination` + `AccountPermissionListItem` (I18NI000000054X).
- Маршрут бәйләүе: `Torii::add_app_api_routes` (I18NI000000056X).
- Һынауҙар: `crates/iroha_torii/tests/accounts_endpoints.rs:126` һәм I18NI000000058X.
- Хужа: I18NT000000012X платформаһы.
- Иҫкәрмәләр: Яуап I18NT0000000001X JSON органы менән I18NI000000059X X/I18NI00000000060X, SDK-ның ярҙамсыларына тап килгән.

### псевдоним OPRF баһалау (I18NI0000000061X) — Ҡапланған
- Ҡулға алынған: I18NI000000062X (`crates/iroha_torii/src/lib.rs:5645-5660`).
- ДТО: I18NI000000064X, I18NI000000065X, I18NI000000066X XX
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Маршрут бәйләүе: I18NI000000068X X (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Һынауҙар: руль артында йөрөүсе һынауҙар (I18NI000000070X) плюс SDK ҡаплау
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Хужа: I18NT000000014X платформаһы.
- Иҫкәрмәләр: Яуап өҫтө детерминистик гекс һәм бэкэнд идентификаторҙарын үтәй; СДК-лар ДТО ҡуллана.

### иҫбатлау саралары SSE (I18NI000000072X) — ҡапланған
- Ҡулға алынған: `handle_v1_events_sse` фильтр ярҙамы менән (`crates/iroha_torii/src/routing.rs:14008-14133`).
- ДТО: `EventsSseParams` (I18NI000000076X) плюс иҫбатлау фильтр проводкаһы.
- Маршрут бәйләүе: `Torii::add_app_api_routes` (I18NI000000078X).
- Һынауҙар: иҫбатлау өсөн махсус SSE люкстары (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  I18NI000000080X, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) һәм торба SSE төтөн һынау
  (`integration_tests/tests/events/sse_smoke.rs`).
- Хужа: I18NT000000016X платформаһы (йүгереү ваҡыты), Интеграция һынауҙары WG (фикстуралар).
- Иҫкәрмәләр: Дәлил фильтр юлдары раҫланған ос-осҡа; документация I18NI000000084X буйынса йәшәй.

### Контракт тормош циклы (I18NI000000085X) — Ҡапланған
- Ҡулға алыусылар: I18NI000000086X X (`crates/iroha_torii/src/routing.rs:5511-5566`),
  I18NI000000088X X (I18NI000000089X, 1990 й.
  I18NI000000090X (I18NI000000091X), 1990 й.
  I18NI000000092X (I18NI000000093X, 1990 й.
  `handle_get_contract_code_bytes` (I18NI0000000955Х).
- ДТО: I18NI000000096X, `DeployAndActivateInstanceDto`, I18NI000000098X, I18NI0000009XX
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Маршрут бәйләүе: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Һынауҙар: маршрутизатор/интеграция люкстары I18NI000000103X, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, I18NI000000106X,
  `contracts_instances_list_router.rs`.
- Хужа: аҡыллы контракт WG менән I18NT00000000018X платформаһы.
- Иҫкәрмәләр: Andpoints сиратҡа ҡул ҡуйылған операциялар һәм дөйөм телеметрия метрикаларын ҡабаттан ҡулланырға (`handle_transaction_with_metrics`).

### Төп тормош циклы (I18NI000000109X) — ҡапланған
- Ҡулға алыусылар: `handle_post_vk_register`, I18NI000000111X, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) һәм `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- ДТО: I18NI000000116X, I18NI000000117X, I18NI0000000118X, I18NI000000119X, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Маршруттарҙы бәйләү: I18NI000000122X (I18NI000000123X).
— Һынауҙар: I18NI000000124X,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Хужаһы: ZK эшсе төркөмө менән I18NT00000000020X Платформа ярҙамы.
- Иҫкәрмәләр: ДТО-лар I18NT000000002X схемалары менән тура килә, улар СДК-лар тарафынан һылтанма яһаған; ставка сикләү аша үтәлгән I18NI000000127X.

### I18NT0000000005X тоташтырыу (I18NI000000128X) — ҡапланған (функцияһы `connect`)
- Ҡулға алыусылар: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (I18NI000000134X).
- ДТО: I18NI000000135X, I18NI000000136X X (`crates/iroha_torii/src/routing.rs:1534-1559`),
  I18NI000000138X (I18NI000000139X X).
- Маршрут бәйләүе: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Һынауҙар: `crates/iroha_torii/tests/connect_gating.rs` (функция ҡапҡаһы, сеанс йәшәү циклы, WS ҡул ҡыҫыу) һәм
  маршрутизатор функцияһы матрица ҡаплау (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Хужаһы: Nexus X Connect WG.
- Иҫкәрмәләр: `limits::rate_limit_key` аша күҙәтелгән ставка сик асҡыстары; телеметрия иҫәпләүселәре `connect.*` метрикаһы каналы.

### Кайги реле телеметрия — ҡапланған
- Ҡулға алынғандар: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  I18NI000000148X, `handle_v1_kaigi_relays_sse` X.
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- ДТО-лар: `KaigiRelaySummaryDto`, I18NI000000152X, 1990 й.
  I18NI000000153X, I18NI000000154X,
  I18NI000000155X (I18NI000000156X).
- Маршрут бәйләүе: I18NI000000157X
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Һынауҙар: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Иҫкәрмәләр: SSE ағымы глобаль эфир каналын ҡабаттан файҙалана, шул уҡ ваҡытта үтәү
  телеметрия профиле ҡапҡаһы; яуап схемалары 2012 йылда документлаштырылған.
  `docs/source/torii/kaigi_telemetry_api.md`.

## Һынау ҡаплау резюме

- Маршрут төтөн анализдары (I18NI000000161X) тәьмин итеү функциялары комбинациялары һәр теркәү
  маршруты һәм был I18NT0000000000X быуын синхронизацияла ҡала.
- Andpoint-конкрет люкстар иҫәп яҙмаһы эҙләү, контракт йәшәү циклы, ZK тикшерергә асҡыс, SSE иҫбатлау фильтрҙары, һәм I18NT000000007X .
  Тоташтырыу тәртибе.
- SDK паритеты йүгәндәре (JavaScript, Swift, Python) инде псевдоним VOPRF һәм SSE ос нөктәләрен ҡуллана; өҫтәмә эш юҡ
  биргеҙәм.

## Был көҙгө заманса тотоу

Яңыртыу һәм был битте һәм сығанаҡ аудит (I18NI000000162X)
Ҡасан да булһа I18NT000000023X ҡушымта API тәртибе үҙгәрә, шулай SDK хужалары һәм тышҡы уҡыусылар тура килә ҡала.