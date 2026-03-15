---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: torii-app-api-parity
כותרת: Auditoria de paridade da API de app do Torii
תיאור: Espelho da revisao TORII-APP-1 para que as equipes de SDK e plataforma confirmem a cobertura publica.
---

סטטוס: Concluido 2026-03-21  
תשובות: Torii Platform, מוביל תוכנית SDK  
מפת דרכים: TORII-APP-1 - auditoria de paridade `app_api`

Esta pagina espelha a auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que leitores fora do mono-repo vejam quais superficies `/v2/*` estao conectadas, testadas e documentadas. A Auditoria acompanha as rotas reexportadas דרך `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` ו `add_connect_routes`.

## אסקופו ושיטות

A Auditoria inspeciona as reexportacoes publicas em `crates/iroha_torii/src/lib.rs:256-522` e os construtores de rotas com feature gating. עבור השטחים `/v2/*` לעשות אימות מפת הדרכים:

- Implementacao do handler e definicoes DTO em `crates/iroha_torii/src/routing.rs`.
- רישום הנתב לא כולל תכונות `app_api` או `connect`.
- Testes de integracao/unitarios existentes e a equipe responsavel pela cobertura de longo prazo.

כמו רשימה של ativos/transações da conta e de titulares de ativos aceitam parametros de consulta `asset_id` אופציונליים לפרה-פילטרים, יש מגבלות קיימות של עמודים/לחץ גב.

## Autenticacao e assinatura canonica

- נקודות קצה GET/POST voltados ואפליקציות aceitam headers opcionais de requisicao canonica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos de `METHOD\n/path\nsorted_query\nsha256(body)`; o Torii os envolve em `QueryRequestWithAuthority` antes da validacao do executor para espelhar `/query`.
- Helpers de SDK existem em dodos של לקוחות עיקרי:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
  - סוויפט: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - אנדרואיד (קוטלין/ג'אווה): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
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

## ממצאי נקודות קצה

### Permissoes de conta (`/v2/accounts/{id}/permissions`) - קוברטו
- מטפל: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- בעלים: Torii Platform.
- הערות: תשובה לגוף JSON Norito com `items`/`total`, alinhado aos helpers de pagecao dos SDKs.

### Avaliacao OPRF de alias (`POST /v2/aliases/voprf/evaluate`) - קוברטו
- מטפל: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- כריכת נתב: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- בדיקות: אשכים מוטבעים לעשות מטפל (`crates/iroha_torii/src/lib.rs:9945-9986`) mais cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- בעלים: Torii Platform.
- Notas: A superficie de resposta reforca hex deterministico e identificadores de backend; ערכות SDK של מערכת הפעלה משתמשות ב-DTO.### Eventos de proof SSE (`GET /v2/events/sse`) - קוברטו
- מטפל: `handle_v1_events_sse` עם תמיכה בפילטרים (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) עם הוכחת חיווט.
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: סוויטות SSE especificas de proof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste smoke SSE do pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- בעלים: פלטפורמת Torii (זמן ריצה), בדיקות אינטגרציה WG (מתקנים).
- הערות: Os caminhos de filtro de proof foram validados מקצה לקצה; a documentacao fica em `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v2/contracts/*`) - קוברטו
- מטפלים: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: סוויטות ראוטר/integracao `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- בעלים: Smart Contract WG com Torii Platform.
- הערות: נקודות הקצה של מערכת ההפעלה enfileiram transacoes assinadas e reutilizam metricas de telemetria compartilhadas (`handle_transaction_with_metrics`).

### Ciclo de vida de chaves de verificacao (`/v2/zk/vk/*`) - קוברטו
- מטפלים: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) ו-`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- בעלים: ZK Working Group com suporte da Torii Platform.
- הערות: Os DTOs se alinham aos schemas Norito Referenciados pelos SDKs; יישום מגביל תעריף באמצעות `limits.rs`.

### Nexus Connect (`/v2/connect/*`) - Coberto (תכונה `connect`)
- מטפלים: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- כריכת נתב: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- בדיקות: `crates/iroha_torii/tests/connect_gating.rs` (שער תכונה, ciclo de vida de sessao, לחיצת יד WS) e
  cobertura da matriz de features do נתב (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- בעלים: Nexus Connect WG.
- הערות: מגבלת הקצב של Chaves de rate sao rastreadas דרך `limits::rate_limit_key`; contadores de telemetria alimentam as metricas `connect.*`.

### Telemetria de relay Kaigi - Coberto
- מטפלים: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- כריכת נתב: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- בדיקות: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- הערות: O stream SSE reutiliza o canal global de broadcast inquanto aplica o gating do perfil de telemetria; OS schemas de resposta estao documentados em `docs/source/torii/kaigi_telemetry_api.md`.## Resumo de cobertura de testes

- Testes עשן לעשות נתב (`crates/iroha_torii/tests/router_feature_matrix.rs`) מבטיח שילוב של תכונות רישום כמו רוטאס e que a geracao de OpenAPI Fique Sincronizada.
- סוויטות מיוחדות לנקודות קצה cobrem queries de contas, ciclo de vida de contratos, chaves de verificacao ZK, filtros de proof SSE e comportamentos do Nexus Connect.
- רתמות של SDK (JavaScript, Swift, Python) ו-Consomem כינוי VOPRF ו-Endpoints SSE; nao ha trabalho adicional.

## מנוטר este espelho atualizado

אטואלי את העמוד והפונט של אודיטוריה (`docs/source/torii/app_api_parity_audit.md`) עבור האפליקציה API de Torii עבור הבעלים של SDK ומידע חיצוני אחר.