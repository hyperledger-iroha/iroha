---
lang: am
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

ሁኔታ፡ 2026-03-21 ተጠናቅቋል  
ባለቤቶች፡ I18NT0000008X መድረክ፣ የኤስዲኬ ፕሮግራም መሪ  
የመንገድ ካርታ ማጣቀሻ፡ TORII-APP-1 — `app_api` እኩልነት ኦዲት

ይህ ገጽ የውስጥ I18NI0000028X ኦዲት (`docs/source/torii/app_api_parity_audit.md`) ያንጸባርቃል
ስለዚህ ከሞኖ-ሪፖ ውጭ ያሉ አንባቢዎች የትኞቹ የ `/v1/*` ንጣፎች በሽቦ እንደተፈተኑ ማየት ይችላሉ ፣
እና በሰነድ. ኦዲቱ በ `Torii::add_app_api_routes` በኩል በድጋሚ ወደ ውጭ የተላኩ መንገዶችን ይከታተላል፣
`add_contracts_and_vk_routes`፣ እና `add_connect_routes`።

## ወሰን እና ዘዴ

ኦዲቱ በ`crates/iroha_torii/src/lib.rs:256-522` እና በ
ባህሪ-የተዘጋ የመንገድ ገንቢዎች. በመንገድ ካርታው ውስጥ ላለው እያንዳንዱ የ`/v1/*` ወለል አረጋግጠናል፡-

- ተቆጣጣሪ አተገባበር እና የ DTO ትርጓሜዎች በ `crates/iroha_torii/src/routing.rs`.
- የራውተር ምዝገባ በ I18NI0000037X ወይም `connect` ባህሪ ቡድኖች።
- ነባር የውህደት/የክፍል ፈተናዎች እና የረጅም ጊዜ ሽፋን ኃላፊነት ያለው የባለቤትነት ቡድን።

የመለያ ንብረቶች/ግብይቶች እና የንብረት ባለቤት ዝርዝሮች አማራጭ `asset_id` መጠይቅ መለኪያዎችን ይቀበላሉ
ለቅድመ-ማጣራት, ከነባሩ የገጽታ / የጀርባ ግፊት ገደቦች በተጨማሪ.

## ትክክለኛ እና ቀኖናዊ ፊርማ

- የመተግበሪያ ፊት ለፊት GET/POST የመጨረሻ ነጥቦች ከ`METHOD\n/path\nsorted_query\nsha256(body)` የተገነቡ የአማራጭ ቀኖናዊ ጥያቄ ራስጌዎችን (`X-Iroha-Account`፣ `X-Iroha-Signature`) ይቀበላሉ፤ Torii ወደ `QueryRequestWithAuthority` ይጠቀለላል ከፈጻሚው ማረጋገጫ በፊት ስለዚህ `/query` ያንፀባርቃሉ።
- የኤስዲኬ ረዳቶች በሁሉም ዋና ደንበኞች ይላካሉ፡
  - JS/TS: I18NI0000045X ከ `canonicalRequest.js`።
  - ስዊፍት: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - አንድሮይድ (ኮትሊን/ጃቫ)፡ `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`።
- ምሳሌ ቅንጥቦች:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<katakana-i105-account-id>", method: "get", path: "/v1/accounts/<katakana-i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<katakana-i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<katakana-i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<katakana-i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<katakana-i105-account-id>", "get", "/v1/accounts/<katakana-i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## የመጨረሻ ነጥብ ክምችት

### የመለያ ፈቃዶች (`/v1/accounts/{id}/permissions`) - የተሸፈነ
- ተቆጣጣሪ፡ `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)።
- DTOs፡ I18NI0000052X + I18NI0000053X (`crates/iroha_torii/src/routing.rs:16867`)።
- የራውተር ማሰሪያ፡ I18NI0000055X (`crates/iroha_torii/src/lib.rs:6678-6797`)።
- ሙከራዎች: `crates/iroha_torii/tests/accounts_endpoints.rs:126` እና I18NI0000058X.
- ባለቤት: I18NT0000012X መድረክ.
- ማስታወሻዎች፡ ምላሹ ከኤስዲኬ ፔጃኒሽን ረዳቶች ጋር የሚዛመድ የNorito JSON አካል ከ `items`/`total` ጋር ነው።

### ተለዋጭ ስም OPRF ይገምግሙ (`POST /v1/aliases/voprf/evaluate`) - የተሸፈነ
- ተቆጣጣሪ፡ `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)።
- DTOs፡ I18NI0000064X፣ `AliasVoprfEvaluateResponseDto`፣ `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)።
- ራውተር ማሰሪያ፡ I18NI0000068X (`crates/iroha_torii/src/lib.rs:6357-6380`)።
- ሙከራዎች፡ የመስመር ውስጥ ተቆጣጣሪ ሙከራዎች (`crates/iroha_torii/src/lib.rs:9945-9986`) እና የኤስዲኬ ሽፋን
  (`javascript/iroha_js/test/toriiClient.test.js:72`)።
- ባለቤት: I18NT0000014X መድረክ.
- ማስታወሻዎች፡ የምላሽ ወለል ወሳኙን ሄክስ እና የኋላን መለያዎችን ያስፈጽማል። ኤስዲኬዎች DTO ይበላሉ።

### የማረጋገጫ ክስተቶች SSE (`GET /v1/events/sse`) - የተሸፈነ
- ተቆጣጣሪ: I18NI0000073X በማጣሪያ ድጋፍ (`crates/iroha_torii/src/routing.rs:14008-14133`)።
- DTOs: I18NI0000075X (I18NI0000076X) እና የማረጋገጫ ማጣሪያ ሽቦ።
- ራውተር ማሰሪያ፡ I18NI0000077X (`crates/iroha_torii/src/lib.rs:6678-6797`)።
- ሙከራዎች፡-ማስረጃ-ተኮር የኤስኤስኢ ስብስቦች (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`፣
  `sse_proof_callhash.rs`፣ `sse_proof_verified_fields.rs`፣ `sse_proof_rejected_fields.rs`) እና የቧንቧ መስመር SSE የጭስ ሙከራ
  (`integration_tests/tests/events/sse_smoke.rs`)።
- ባለቤት፡ I18NT0000016X Platform (የአሂድ ጊዜ)፣ የውህደት ሙከራዎች WG (ቋሚዎች)።
- ማስታወሻዎች፡ ከጫፍ እስከ ጫፍ የተረጋገጡ የማጣሪያ መንገዶች ማረጋገጫ; ሰነዶች በ `docs/source/zk_app_api.md` ስር ይኖራሉ።

### የኮንትራት የህይወት ዑደት (`/v1/contracts/*`) - የተሸፈነ
- ተቆጣጣሪዎች፡- `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)፣
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)፣
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)፣
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)፣
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)።
- DTOs፡ I18NI0000096X፣ `DeployAndActivateInstanceDto`፣ `ActivateInstanceDto`፣ `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)።
- ራውተር ማሰሪያ፡ I18NI0000101X (`crates/iroha_torii/src/lib.rs:6456-6483`)።
- ሙከራዎች: ራውተር / ውህደት ስብስቦች `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`፣ `contracts_call_integration.rs`፣
  `contracts_instances_list_router.rs`.
- ባለቤት: ስማርት ኮንትራት WG ከ I18NT0000018X መድረክ ጋር።
- ማስታወሻዎች፡ የመጨረሻ ነጥቦች የተፈረሙ ግብይቶችን ወረፋ እና የጋራ የቴሌሜትሪ መለኪያዎችን (`handle_transaction_with_metrics`) እንደገና ይጠቀሙ።

### ቁልፍ የህይወት ኡደትን ማረጋገጥ (`/v1/zk/vk/*`) - የተሸፈነ
- ተቆጣጣሪዎች፡ I18NI0000110X፣ `handle_post_vk_update`፣ `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) እና `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)።
- DTOs፡ `ZkVkRegisterDto`፣ `ZkVkUpdateDto`፣ `ZkVkDeprecateDto`፣ `VkListQuery`፣ `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)።
- ራውተር ማሰር: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- ሙከራዎች: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`፣
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- ባለቤት፡ ZK Working Group with Torii Platform ድጋፍ።
- ማስታወሻዎች: DTOs በኤስዲኬዎች ከተጣቀሱ Norito እቅዶች ጋር ይጣጣማሉ; በ `limits.rs` በኩል የዋጋ ገደብ ተፈጻሚ ነው።

### Nexus አገናኝ (`/v1/connect/*`) - የተሸፈነ (ባህሪ `connect`)
- ተቆጣጣሪዎች፡ I18NI0000130X፣ `handler_connect_session_delete`፣ `handle_connect_ws`፣
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)።
- DTOs፡ I18NI0000135X፣ `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)፣
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)።
- ራውተር ማሰር: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
ሙከራዎች፡- `crates/iroha_torii/tests/connect_gating.rs` (የጌቲንግ ባህሪ፣ የክፍለ ጊዜ የህይወት ኡደት፣ WS እጅ መጨባበጥ) እና
  የራውተር ባህሪ ማትሪክስ ሽፋን (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)።
- ባለቤት: I18NT0000006X አገናኝ WG.
- ማስታወሻዎች: በ `limits::rate_limit_key` በኩል የሚከታተሉ የደረጃ ገደብ ቁልፎች; የቴሌሜትሪ ቆጣሪዎች `connect.*` መለኪያዎችን ይመገባሉ።

### የካይጊ ሪሌይ ቴሌሜትሪ - የተሸፈነ
- ተቆጣጣሪዎች፡ I18NI0000146X፣ `handle_v1_kaigi_relay_detail`፣
  `handle_v1_kaigi_relays_health`፣ `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)።
- DTOs፡ I18NI0000151X፣ `KaigiRelaySummaryListDto`፣
  `KaigiRelayDetailDto`፣ `KaigiRelayDomainMetricsDto`፣
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)።
- ራውተር ማሰር: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)።
- ሙከራዎች: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ማስታወሻዎች፡ የኤስኤስኢ ዥረት በማስፈጸም ላይ እያለ የአለምአቀፍ ስርጭት ቻናልን እንደገና ይጠቀማል
  ቴሌሜትሪ ፕሮፋይል ጋቲንግ; የምላሽ መርሃግብሮች በ ውስጥ ተመዝግበዋል
  `docs/source/torii/kaigi_telemetry_api.md`.

## የፈተና ሽፋን ማጠቃለያ

- የራውተር ጭስ ሙከራዎች (`crates/iroha_torii/tests/router_feature_matrix.rs`) የባህሪ ጥምረት እያንዳንዱን መመዝገቡን ያረጋግጣል
  መንገድ እና ያ I18NT0000000X ትውልድ እንደተመሳሰለ ይቆያል።
- የመጨረሻ ነጥብ-ተኮር ስብስቦች የመለያ ጥያቄዎችን ፣ የኮንትራት የሕይወት ዑደት ፣ ZK ማረጋገጫ ቁልፎችን ፣ የኤስኤስኢ ማረጋገጫ ማጣሪያዎችን እና Nexus ይሸፍናሉ።
  ባህሪያትን ያገናኙ.
- የኤስዲኬ እኩልነት ማሰሪያዎች (ጃቫ ስክሪፕት ፣ ስዊፍት ፣ ፓይዘን) ቀድሞውኑ ተለዋጭ VOPRF እና SSE የመጨረሻ ነጥቦችን ይበላሉ ። ምንም ተጨማሪ ሥራ የለም
  ያስፈልጋል።

## ይህንን መስታወት ወቅታዊ ማድረግ

ሁለቱንም ይህንን ገጽ እና የምንጭ ኦዲት (`docs/source/torii/app_api_parity_audit.md`) ያዘምኑ
በማንኛውም ጊዜ የኤስዲኬ ባለቤቶች እና ውጫዊ አንባቢዎች እንዲሰለፉ የTorii መተግበሪያ ኤፒአይ ባህሪ ሲቀየር።