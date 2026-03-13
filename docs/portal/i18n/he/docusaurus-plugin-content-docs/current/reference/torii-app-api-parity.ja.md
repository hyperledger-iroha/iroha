---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0732dd94166eb79e0203a7cb406407a45c45038e389378bc57368516e32406a0
source_last_modified: "2026-01-30T09:29:51+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: torii-app-api-parity
title: בדיקת תאימות API של אפליקציית Torii
description: שיקוף של סקירת TORII-APP-1 כדי שצוותי SDK והפלטפורמה יוכלו לאשר את הכיסוי הציבורי.
---

סטטוס: הושלם 2026-03-21  
בעלים: Torii Platform, SDK Program Lead  
הפניה במפת הדרכים: TORII-APP-1 — בדיקת תאימות `app_api`

עמוד זה משקף את ביקורת `TORII-APP-1` הפנימית (`docs/source/torii/app_api_parity_audit.md`) כדי שקוראים מחוץ למונו-ריפו יוכלו לראות אילו משטחי `/v2/*` מחווטים, נבדקו ותועדו. הביקורת עוקבת אחר הנתיבים המיוצאים מחדש דרך `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` ו-`add_connect_routes`.

## היקף ושיטה

הביקורת בוחנת את הייצוא מחדש הציבורי ב-`crates/iroha_torii/src/lib.rs:256-522` ואת בוני הנתיבים שמוגנים לפי פיצ'רים. עבור כל משטח `/v2/*` ברודמאפ אימתנו:

- יישום ה-handler והגדרות DTO ב-`crates/iroha_torii/src/routing.rs`.
- רישום הנתב תחת קבוצות הפיצ'רים `app_api` או `connect`.
- בדיקות אינטגרציה/יחידה קיימות והצוות שאחראי לכיסוי לטווח ארוך.

רשימות נכסים/עסקאות של חשבון ורשימות בעלי נכסים מקבלות פרמטרי שאילתה `asset_id` אופציונליים לסינון מקדים, בנוסף למגבלות העימוד/ה־backpressure הקיימות.

## אימות וחתימה קנונית

- נקודות קצה GET/POST הפונות לאפליקציות מקבלות כותרות בקשה קנוניות אופציונליות (`X-Iroha-Account`, `X-Iroha-Signature`) שנבנות מ-`METHOD\n/path\nsorted_query\nsha256(body)`; Torii עוטפת אותן ב-`QueryRequestWithAuthority` לפני אימות ה-executor כך שהן משקפות את `/query`.
- עזרי SDK זמינים בכל הלקוחות העיקריים:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` מתוך `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- דוגמאות:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v2/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v2/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v2/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v2/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## מלאי נקודות הקצה

### הרשאות חשבון (`/v2/accounts/{id}/permissions`) — מכוסה
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` ו-`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: התגובה היא גוף Norito JSON עם `items`/`total`, בהתאם לעזרי הפגינציה של ה-SDK.

### הערכת OPRF של Alias (`POST /v2/aliases/voprf/evaluate`) — מכוסה
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: בדיקות inline ל-handler (`crates/iroha_torii/src/lib.rs:9945-9986`) וכן כיסוי SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notes: משטח התגובה כופה hex דטרמיניסטי ומזהי backend; ה-SDK צורכים את ה-DTO.

### אירועי SSE של proof (`GET /v2/events/sse`) — מכוסה
- Handler: `handle_v1_events_sse` עם תמיכת סינון (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) יחד עם חיווט מסנן proof.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: חבילות SSE ייעודיות ל-proof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) ובדיקת smoke של SSE בצינור
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: מסלולי מסנן ה-proof מאומתים מקצה לקצה; התיעוד נמצא תחת `docs/source/zk_app_api.md`.

### מחזור חיי חוזים (`/v2/contracts/*`) — מכוסה
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: חבילות router/integration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG בשיתוף Torii Platform.
- Notes: נקודות הקצה מתזמנות עסקאות חתומות וממחזרות מדדי טלמטריה משותפים (`handle_transaction_with_metrics`).

### מחזור חיי מפתחות אימות (`/v2/zk/vk/*`) — מכוסה
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) ו-`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group בתמיכת Torii Platform.
- Notes: ה-DTOs מיושרים לסכמות Norito אליהן ה-SDK מתייחסים; rate limiting נאכף דרך `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — מכוסה (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, מחזור חיי סשן, handshake WS) וכן
  כיסוי מטריצת פיצ'רים של ה-router (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notes: מפתחות rate limit נעקבים דרך `limits::rate_limit_key`; מוני הטלמטריה מזינים את מדדי `connect.*`.

### טלמטריית Relay של Kaigi — מכוסה
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes: זרם ה-SSE ממחזר את ערוץ ה-broadcast הגלובלי תוך אכיפת gating של פרופיל הטלמטריה; סכמות התגובה מתועדות ב-`docs/source/torii/kaigi_telemetry_api.md`.

## סיכום כיסוי בדיקות

- בדיקות smoke של ה-router (`crates/iroha_torii/tests/router_feature_matrix.rs`) מבטיחות ששילובי פיצ'רים רושמים כל נתיב ושהפקת OpenAPI נשארת מסונכרנת.
- חבילות ייעודיות לנקודות קצה מכסות שאילתות חשבון, מחזור חיי חוזים, מפתחות אימות ZK, מסנני proof SSE והתנהגויות Nexus Connect.
- תשתיות parity של ה-SDK (JavaScript, Swift, Python) כבר צורכות Alias VOPRF ונקודות SSE; אין צורך בעבודה נוספת.

## שמירה על עדכון המראה

עדכנו עמוד זה ואת מקור הביקורת (`docs/source/torii/app_api_parity_audit.md`) כאשר התנהגות Torii app API משתנה כדי שמובילי ה-SDK וקוראים חיצוניים יישארו מיושרים.
