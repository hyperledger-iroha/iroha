---
id: torii-app-api-parity
lang: dz
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

གནས་སྟངས་: ༢༠༢༦-༠༣-༢༡ མཇུག་བསྡུ་ཡོདཔ།  
ཇོ་བདག་: Torii གཞི་རྟེན་, ཨེསི་ཌི་ཀེ་ལས་རིམ་འགོ་ཁྲིད།  
ལམ་སྟོན་གཞི་བསྟུན་: TORII-APP-1 — `app_api` ཆ་སྙོམས་རྩིས་ཞིབ་འབད་ནི།

ཤོག་ལེབ་འདི་གིས་ ནང་འཁོད་ I18NI0000028X རྩིས་ཞིབ་ (`docs/source/torii/app_api_parity_audit.md`)
དེ་འབདཝ་ལས་ མོ་ནོ་རེ་པོ་གི་ཕྱི་ཁར་ ལྷག་མི་ཚུ་གིས་ I18NI000000030X གི་ཁ་ཐོག་འདི་ གློག་ཐག་སྦེ་ཡོདཔ་ཨིན་ན་ བལྟ་ཚུགས།
དང་ཡིག་ཐོག་ལུ་བཀོད་ཡོད། རྩིས་ཞིབ་འདི་གིས་ I18NI000000031X བརྒྱུད་དེ་ ལོག་ཕྱིར་ཚོང་འབད་མི་ ལམ་ཚུ་ འཚོལ་ཞིབ་འབདཝ་ཨིན།
`add_contracts_and_vk_routes`, དང་ `add_connect_routes`, .

## ཁྱབ་ཁོངས་དང་ཐབས་ལམ།

རྩིས་ཞིབ་འདི་གིས་ མི་མང་ལོག་སྟེ་ཕྱིར་ཚོང་འབད་མི་ I18NI000000034X དང་ དེ་ལས་
fatche-gated ལམ་བཟོ་མི། ང་བཅས་ཀྱིས་ བདེན་དཔྱད་འབད་མི་ ལམ་སྟོན་ནང་ `/v1/*` གི་ཁ་ཐོག་རེ་ལུ་ཨིན།

- ལག་བཟོཔ་ལག་ལེན་དང་ DTO ངེས་ཚིག་ `crates/iroha_torii/src/routing.rs` ནང་།
- `app_api` ཡང་ན་ `connect` ཁྱད་རྣམ་སྡེ་ཚན་འོག་ལུ་ རའུ་ཊར་ཐོ་བཀོད་འབད་ནི།
- ཡུན་རིང་གི་ཁྱབ་ཁོངས་ཀྱི་འགན་ཁུར་འབག་མི་ མཉམ་བསྡོམས་/ཡུ་ནིཊ་བརྟག་དཔྱད་དང་ བདག་དབང་སྡེ་ཚན་ཚུ།

རྩིས་ཐོ་རྒྱུ་དངོས་/ཚོང་འབྲེལ་དང་ རྒྱུ་དངོས་-འཛིན་བཟུང་ཚུ་གིས་ གདམ་ཁ་ཅན་གྱི་ `asset_id` འདྲི་དཔྱད་ཚད་གཞི་ཚུ་ དང་ལེན་འབདཝ་ཨིན།
སྔོན་སྒྲིག་ཚགས་མ་འབད་ནིའི་དོན་ལུ་ ད་ལྟོ་ཡོད་པའི་ ཤོག་ལེབ་/རྒྱབ་བསྐྱོད་ཚད་གཞི་ཚུ་གི་ཁ་སྐོང་ལུ་།

## བདེན་བཤད་དང་ ཁྲིམས་ལུགས་མཚན་རྟགས་བཀོད་པ།

- App-facing GET/POST མཐའ་མཚམས་ཚུ་གིས་ གདམ་ཁ་ཅན་གྱི་ ཀེ་ནོ་ནིག་ཞུ་བ་མགོ་ཡིག་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན། I18NT000000010X གིས་ ལག་ལེན་འཐབ་མིའི་བདེན་བཤད་མ་འབད་བའི་ཧེ་མ་ `QueryRequestWithAuthority` ནང་ལུ་ བཀབ་སྟེ་ཡོདཔ་ལས་ ཁོང་གིས་ I18NI000000044X ལུ་ མེ་ལོང་བཏགས།
- ཨེསི་ཌི་ཀེ་གྲོགས་རམ་པ་ཚུ་གིས་ གཞི་རྟེན་མཁོ་མངགས་འབད་མི་ཆ་མཉམ་ནང་ སྐྱེལ་འདྲེན་འབདཝ་ཨིན།
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` ལས་ I18NI000000046X.
  - སུའིཕཊི་: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - ཨེན་ཌོརཌི་ (ཀོ་ཊི་ལིན་/ཇ་ཝ): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- དཔེ་ཚད།
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "ih58...", method: "get", path: "/v1/accounts/ih58.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/ih58.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "ih58...",
                                                  method: "get",
                                                  path: "/v1/accounts/ih58.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("ih58...", "get", "/v1/accounts/ih58.../assets", "limit=5", ByteArray(0), signer)
```

## ངོས་ འཛིན།

### རྩིས་ཁྲའི་གནང་བ་ (`/v1/accounts/{id}/permissions`) — ཁྱབ་བསྒྲགས།
- ལག་པ་: `handle_v1_account_permissions` (I18NI000000051X).
- ཌི་ཊི་ཨོ་: I༡༨NI00000052X + I18NI000000053X (I18NI0000000054X).
- རའུ་ཊར་བཱའིན་ཌིང་: I18NI000000055X (I18NI0000000056X).
- བརྟག་དཔྱད་: I18NI000000057X དང་ I18NI000000058X.
- ཇོ་བདག་: Torii སྟེགས་བུ།
- དྲན་འཛིན་ཚུ་: ལན་འདེབས་འདི་ I18NT0000001X JSON འདི་ I18NI000000059X/I18NI0000000600, མཐུན་སྒྲིག་ཅན་གྱི་ SDK pagination གྲོགས་རམ་པ་ཚུ་ཨིན།

### ཨ་ལི་ཡས་ཨོ་པི་ཨར་ཨེཕ་དབྱེ་ཞིབ་ (`POST /v1/aliases/voprf/evaluate`) — ཁྱབ་ཁོངས།
- ལག་པ་: I18NI000000062X (I18NI000000063X).
- ཌི་ཊི་ཨོ་: I༡༨NI00000064X, I18NI0000000065X, I18NI0000000066X,
  (`crates/iroha_torii/src/routing.rs:809-865`).
- རའུ་ཊར་བཱའིན་ཌིང་: I18NI000000068X (I18NI000000069X).
- བརྟག་དཔྱད་: ནང་ཐིག་འཛིན་སྐྱོང་པ་བརྟག་དཔྱད་ (I18NI0000070X) དང་ ཨེསི་ཌི་ཀེ་ཁྱབ་ཁོངས།
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- ཇོ་བདག་: Torii སྟེགས་བུ།
- དྲན་ཐོ། ལན་འདེབས་ཁ་ཐོག་བཀག་དམ་ཚུ་ གཏན་འབེབས་དང་རྒྱབ་རྟེན་ངོས་འཛིན་ཚུ་ གཏན་འབེབས་བཟོཝ་ཨིན། SDKs ཚུ་གིས་ DTO ཟ་སྤྱོད་འབདཝ་ཨིན།

### བདེན་དཔང་བྱུང་རིམ་ SSE (`GET /v1/events/sse`) — ཁྱབ་བསྒྲགས།
- ལག་པ་: ཚགས་མ་རྒྱབ་སྐྱོར་ (I18NI000000074X) དང་གཅིག་ཁར་ `handle_v1_events_sse` ཨིན།
- ཌི་ཊི་ཨོ་: I18NI000000075X (I18NI0000000076X) དང་ བདེན་དཔང་ཚགས་མ་གློག་ཐག་བཏོན།
- རའུ་ཊར་བཱའིན་ཌིང་: I18NI000000077X (I18NI0000000078X).
- བརྟག་དཔྱད་: བདེན་དཔང་དམིགས་བསལ་གྱི་ཨེསི་ཨེསི་ཨི་ཁང་མིག་ (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  I18NI000000080X, `sse_proof_verified_fields.rs`, I18NI000000082X, དང་ པའིཔ་ལིན་ཨེས་ཨེསི་ཨི་ དུ་ཁ་བརྟག་དཔྱད།
  (I 18NI00000083X).
- ཇོ་བདག་: Torii སྟེགས་བུ་ (runtime), མཉམ་བསྡོམས་བརྟག་དཔྱད་ WG (fixtures)།
- དྲན་འཛིན་ཚུ་: བདེན་ཁུངས་ཚགས་མ་འགྲུལ་ལམ་ཚུ་ བདེན་དཔྱད་འབད་ཡོད་པའི་མཐའ་མ་ལས་མཇུག་བསྡུ། ཡིག་ཆ་འདི་ `docs/source/zk_app_api.md` གི་འོག་ལུ་ཡོདཔ་ཨིན།

### གན་ཡིག་གན་ཡིག་གན་ཡིག་ (I18NI0000085X) — ཁྱབ་བསྒྲགས།
- ལག་པ་: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`).
  I18NI0000008X (I18NI0000089X).
  `handle_post_contract_instance_activate` (I18NI0000091X).
  `handle_post_contract_call` (I18NI0000093X).
  I18NI0000094X (I18NI0000095X).
- ཌི་ཊི་ཨོ་: ཨི༡༨ཨེན་ཨའི་ཡུ་༠༠༠༠༩༦ཨེགསི་, ཨའི་༡༨ཨེན་ཨའི་ཡུ་༠༠༠༠༠༠༠༩༧ཨེགསི་, ངའི་ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༩༨ཨེགསི་, ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༩X
  (I 18NI00000100X).
- རའུ་ཊར་བཱའིན་ཌིང་: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- བརྟག་དཔྱད་: རའུ་ཊར་/མཉམ་བསྡོམས་ཆ་ཚང་ `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, I18NI00000106,
  `contracts_instances_list_router.rs`.
- ཇོ་བདག་: I18NT0000018X གཞི་རྟེན་དང་གཅིག་ཁར་ གན་ཡིག་གི་གན་ཡིག་ WG དང་མཉམ་དུ།
- དྲན་འཛིན་ཚུ་: མཐའ་ཐིག་ཚུ་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ གྱལ་རིམ་དང་ བསྐྱར་ལོག་འབད་ཡོད་པའི་ བརྒྱུད་འཕྲིན་མེ་ཊིག་ (`handle_transaction_with_metrics`)།

### ལྡེ་མིག་མི་ཚེ་འཁོར་རིམ་ བདེན་དཔྱད་འབད་ནི། (`/v1/zk/vk/*`) — ཁ་བསྡམས།
- ལག་པ་: `handle_post_vk_register`, I18NI0000001111X, I18NI000000112X
  (`crates/iroha_torii/src/routing.rs:4282-4382`) དང་ `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- ཌི་ཊི་ཨོ་: ཨི༡༨ཨེན་ཨའི་༠༠༠༠༠༡༡༦ཨེགསི་, ཨི༡༨ཨེན་ཨའི་༠༠༠༠༠༡༡༧ཨེགསི་, ཨི༡༨ཨེན་ཨའི་༠༠༠༠༠༡༡༨ཨེགསི་, ཨི༡༨ཨེན་ཨའི་༠༠༠༠༡༡༩X, I༡༨NI00000120X,
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- རའུ་ཊར་བཱའིན་ཌིང་: `Torii::add_contracts_and_vk_routes` (I18NI000000123X).
- བརྟག་དཔྱད་: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- ཇོ་བདག་: ZK ལས་དོན་སྡེ་ཚན་ I18NT0000020X སྟེགས་བུ་རྒྱབ་སྐྱོར།
- དྲན་འཛིན་ཚུ་: ཨེསི་ཌི་ཀེ་ཨེསི་གིས་ རྒྱབ་རྟེན་འབད་མི་ I18NT0000002X གི་ལས་རིམ་ཚུ་དང་གཅིག་ཁར་ ཕྲང་སྒྲིག་འབདཝ་ཨིན། ཚད་གཞི་ཚད་འཛིན་འབད་མི་ `limits.rs` བརྒྱུད་དེ་ བསྟར་སྤྱོད་འབད་ཡོདཔ།

### I18NT0000005X མཐུད་ལམ་ (`/v1/connect/*`) — ཁྱབ་ཁོངས་ (`connect`)
- ལག་པ་: `handle_connect_session`, I18NI0000000131X, I18NI000000132X,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- ཌི་ཊི་ཨོ་: I༡༨NI00000135X, I18NI000000136X (I18NI000000137X),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- རའུ་ཊར་བཱའིན་ཌིང་: `Torii::add_connect_routes` (I18NI000000141X).
- བརྟག་དཔྱད་: `crates/iroha_torii/tests/connect_gating.rs` (པར་རིས་ཀྱི་སྒོ་སྒྲིག
  རའུ་ཊར་ཁྱད་རྣམ་མེ་ཊིགསི་ཁྱབ་ཚད་ (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- ཇོ་བདག་: Nexus མཐུད་སྦྲེལ་མཐུད་འབྲེལ་འབད་ནི།
- དྲན་འཛིན་: `limits::rate_limit_key` བརྒྱུད་དེ་ བརྟག་ཞིབ་འབད་མི་ ཚད་གཞི་ཚད་ལྡེ་མིག་ཚུ།; telemetry གྱངས་ཁ་ `connect.*` མེ་ཊིགས།

### ཀ་གི་རི་ལེ་ཊེ་ལི་མི་ཊི་རི། — ཁྱབ་ཁོངས།
- ལག་པ་: `handle_v1_kaigi_relays`, I18NI000000147X,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`,
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- ཌི་ཊི་ཨོ་: I༡༨NI00000151X, I18NI000000152X,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- རའུ་ཊར་བཱའིན་ཌིང་: `Torii::add_app_api_routes`
  (I 18NI00000158X).
- བརྟག་དཔྱད་: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- དྲན་འཛིན་ཚུ་: ཨེསི་ཨེསི་ཨི་ རྒྱུན་ལམ་འདི་གིས་ བསྟར་སྤྱོད་འབད་བའི་སྐབས་ ཡོངས་ཁྱབ་རྒྱང་བསྒྲགས་རྒྱུ་ལམ་འདི་ ལོག་ལག་ལེན་འཐབ་ཨིན།
  telemetry གསལ་སྡུད་སྒོ་སྒྲ།; 2 ནང་ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་ལས་རིམ།
  `docs/source/torii/kaigi_telemetry_api.md`.

## བརྟག་དཔྱད་ཁྱབ་ཁོངས་མདོར་བསྡུས།

- རའུ་ཊར་དུ་བ་བརྟག་དཔྱད་ (`crates/iroha_torii/tests/router_feature_matrix.rs`) ཁྱད་རྣམ་མཉམ་སྡེབ་ཚུ་ རེ་རེ་བཞིན་ཐོ་བཀོད་འབད་ཡོདཔ་ངེས་གཏན་བཟོ།
  ལམ་དང་དེ་ I18NT000000000X མི་རབས་འདི་ མཉམ་དུ་སྡོད།
- མཐའ་མཚམས་དམིགས་བསལ་གྱི་ཆ་ཤས་ཚུ་གིས་ རྩིས་ཁྲའི་འདྲི་དཔྱད་དང་ གན་རྒྱ་བཟོ་བའི་མི་ཚེ་འཁོར་རིམ་ ZK བདེན་དཔྱད་ལྡེ་མིག་ ཨེསི་ཨེསི་ཨི་བདེན་དཔང་ཚགས་མ་ཚུ་ དེ་ལས་ I18NT0000007X ཚུ་ཁྱབ་སྟེ་ཡོདཔ་ཨིན།
  འབྲེལ་འཐབ་ཚུ་ མཐུད་འབྲེལ་འབད་ནི།
- SDK parity harness (JavaScript, Swift, Python) གིས་ ཧེ་མ་ལས་ར་ ཨ་ལི་ཡས་ VOPRF དང་ SSE མཐའ་མཚམས་ཚུ་ བཀོལ་སྤྱོད་འབདཝ་ཨིན། ཁ་སྐོང་ལཱ་མེདཔ།
  དགོས་མཁོ།

## མེ་ལོང་འདི་ད་རེས་ནངས་པར་བཞག་དོ།

ཤོག་ལེབ་འདི་དང་ འབྱུང་ཁུངས་རྩིས་ཞིབ་ (`docs/source/torii/app_api_parity_audit.md`) གཉིས་ཆ་རང་དུས་མཐུན་བཟོ་ནི།
ག་དེམ་ཅིག་སྦེ་ I18NT0000000023X གློག་རིག་ཨེཔ་ཨེ་པི་ཨའི་སྤྱོད་ལམ་བསྒྱུར་བཅོས་འགྱོཝ་ལས་ ཨེསི་ཌི་ཀེ་གི་ཇོ་བདག་དང་ཕྱི་ཁའི་ལྷག་མི་ཚུ་ ཕྲང་སྒྲིག་སྦེ་སྡོདཔ་ཨིན།