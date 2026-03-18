---
lang: kk
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b8769f4d2a6dd365b851c583d936fd78594ca60087fa3bbdcc6dc8177ee12be6
source_last_modified: "2026-01-30T12:29:10.183882+00:00"
translation_last_reviewed: 2026-02-07
id: torii-app-api-parity
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
---

Күйі: Аяқталды 21.03.2026  
Иелері: Torii платформасы, SDK бағдарламасының жетекшісі  
Жол картасының анықтамасы: TORII-APP-1 — `app_api` паритет аудиті

Бұл бет ішкі `TORII-APP-1` аудитін көрсетеді (`docs/source/torii/app_api_parity_audit.md`)
сондықтан моно-реподан тыс оқырмандар `/v1/*` беттерінің сымды, сыналған,
және құжатталған. Аудит `Torii::add_app_api_routes` арқылы қайта экспортталған маршруттарды бақылайды,
`add_contracts_and_vk_routes`, және `add_connect_routes`.

## Қолдану аймағы және әдісі

Аудит `crates/iroha_torii/src/lib.rs:256-522` және мемлекеттік реэкспорттарды тексереді.
ерекшелігі бар маршрут құрастырушылар. Жол картасындағы әрбір `/v1/*` беті үшін біз мыналарды тексердік:

- `crates/iroha_torii/src/routing.rs` ішіндегі өңдегіштің орындалуы және DTO анықтамалары.
- `app_api` немесе `connect` мүмкіндіктер топтары бойынша маршрутизаторды тіркеу.
- Қолданыстағы интеграция/бірлік сынақтары және ұзақ мерзімді қамтуға жауапты иеленуші топ.

Есептік жазба активтері/транзакциялары және активтерді ұстаушылардың тізімдері қосымша `asset_id` сұрау параметрлерін қабылдайды
бар беттеу/кері қысым шектеулеріне қосымша, алдын ала сүзгілеу үшін.

## Авторлық және канондық қол қою

- Қолданбаға бағытталған GET/POST соңғы нүктелері `METHOD\n/path\nsorted_query\nsha256(body)` бастап жасалған қосымша канондық сұрау тақырыптарын (`X-Iroha-Account`, `X-Iroha-Signature`) қабылдайды; Torii оларды `/query` шағылыстыруы үшін орындаушы тексеруден бұрын `QueryRequestWithAuthority` ішіне орады.
- SDK көмекшілері барлық негізгі клиенттерге жеткізіледі:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` `canonicalRequest.js` бастап.
  - Жылдам: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Мысал үзінділер:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Соңғы нүкте түгендеу

### Тіркелгі рұқсаттары (`/v1/accounts/{id}/permissions`) — Қамтылған
- Өңдеуші: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Маршрутизаторды байланыстыру: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесттер: `crates/iroha_torii/tests/accounts_endpoints.rs:126` және `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Иесі: Torii платформасы.
- Ескертпелер: Жауап - `items`/`total`, сәйкес SDK беттеу көмекшілері бар Norito JSON денесі.

### Бүркеншік ат OPRF бағалауы (`POST /v1/aliases/voprf/evaluate`) — Қамтылған
- Өңдеуші: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Маршрутизаторды байланыстыру: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесттер: кірістірілген өңдеуші сынақтары (`crates/iroha_torii/src/lib.rs:9945-9986`) плюс SDK қамту
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Иесі: Torii платформасы.
- Ескертулер: Жауап беті детерминирленген он алтылық және серверлік идентификаторларды мәжбүрлейді; SDKs DTO пайдаланады.

### Дәлелдеу оқиғалары SSE (`GET /v1/events/sse`) — Қамтылған
- Өңдеуші: сүзгі қолдауы бар `handle_v1_events_sse` (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) плюс өтпейтін сүзгі сымдары.
- Маршрутизаторды байланыстыру: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесттер: дәлелдеуге арналған SSE жиынтықтары (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) және құбыр желісінің SSE түтін сынағы
  (`integration_tests/tests/events/sse_smoke.rs`).
- Иесі: Torii платформасы (орындалу уақыты), WG интеграциялық сынақтары (құралдар).
- Ескертпелер: дәлелдеу сүзгі жолдарының басынан аяғына дейін расталған; құжаттама `docs/source/zk_app_api.md` астында өмір сүреді.

### Шарттың өмірлік циклі (`/v1/contracts/*`) — Қамтылған
- Өңдеуіштер: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Маршрутизаторды байланыстыру: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесттер: `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`, маршрутизатор/интеграциялық пакеттер,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Иесі: Torii платформасы бар Smart Contract WG.
- Ескертпелер: Соңғы нүктелер қол қойылған транзакцияларды кезекке қояды және ортақ телеметрия көрсеткіштерін қайта пайдаланады (`handle_transaction_with_metrics`).

### Кілттің өмірлік циклін тексеру (`/v1/zk/vk/*`) — Қамтылған
- Өңдеуіштер: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) және `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Маршрутизаторды байланыстыру: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесттер: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Иесі: Torii платформасының қолдауы бар ZK жұмыс тобы.
- Ескертпелер: DTO SDK арқылы сілтеме жасалған Norito схемаларына сәйкестендіріледі; мөлшерлемені шектеу `limits.rs` арқылы орындалады.

### Nexus Қосылу (`/v1/connect/*`) — Жабық (`connect` мүмкіндігі)
- Өңдеуіштер: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Маршрутизаторды байланыстыру: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесттер: `crates/iroha_torii/tests/connect_gating.rs` (функция қақпасы, сеанстың өмірлік циклі, WS қол алысу) және
  маршрутизатордың матрицалық мүмкіндіктерін қамту (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Иесі: Nexus Connect WG.
- Ескертпелер: `limits::rate_limit_key` арқылы бақыланатын тарифтік шектеу кілттері; телеметриялық есептегіштер `connect.*` метрикасын береді.

### Кайги релелік телеметрия — Жабық
- Өңдеушілер: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Маршрутизаторды байланыстыру: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесттер: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Ескертпелер: SSE ағыны орындау кезінде жаһандық тарату арнасын қайта пайдаланады
  телеметриялық профильді штрихтау; жауап схемалары құжатталады
  `docs/source/torii/kaigi_telemetry_api.md`.

## Сынақ туралы қорытынды

- Маршрутизатордың түтіндік сынақтары (`crates/iroha_torii/tests/router_feature_matrix.rs`) мүмкіндіктер комбинацияларының әрбір тіркелуін қамтамасыз етеді
  маршрут және сол OpenAPI буыны синхрондалады.
- Соңғы нүктеге арналған жинақтар тіркелгі сұрауларын, келісім-шарттың өмірлік циклін, ZK тексеру кілттерін, SSE тексеру сүзгілерін және Nexus қамтиды.
  Мінез-құлықты байланыстыру.
- SDK паритеттік белдеулері (JavaScript, Swift, Python) VOPRF Alias ​​және SSE соңғы нүктелерін пайдаланады; қосымша жұмыс жоқ
  қажет.

## Осы айнаны жаңартып отыру

Осы бетті де, бастапқы аудитті де жаңартыңыз (`docs/source/torii/app_api_parity_audit.md`)
Torii қолданбасының API әрекеті өзгерген сайын SDK иелері мен сыртқы оқырмандар біркелкі болады.