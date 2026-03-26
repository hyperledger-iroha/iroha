---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4e63baf02a1981cdf3ad5ff90765d760fa447e3955cfb1ad281e6aae44440bb
source_last_modified: "2026-01-30T09:29:51+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: torii-app-api-parity
title: Auditoria de paridade da API de app do Torii
description: Espelho da revisao TORII-APP-1 para que as equipes de SDK e plataforma confirmem a cobertura publica.
---

Status: Concluido 2026-03-21  
Responsaveis: Torii Platform, SDK Program Lead  
Referencia do roadmap: TORII-APP-1 - auditoria de paridade `app_api`

Esta pagina espelha a auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que leitores fora do mono-repo vejam quais superficies `/v1/*` estao conectadas, testadas e documentadas. A auditoria acompanha as rotas reexportadas via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Escopo e metodo

A auditoria inspeciona as reexportacoes publicas em `crates/iroha_torii/src/lib.rs:256-522` e os construtores de rotas com feature gating. Para cada superficie `/v1/*` do roadmap verificamos:

- Implementacao do handler e definicoes DTO em `crates/iroha_torii/src/routing.rs`.
- Registro do router nos grupos de features `app_api` ou `connect`.
- Testes de integracao/unitarios existentes e a equipe responsavel pela cobertura de longo prazo.

As listagens de ativos/transações da conta e de titulares de ativos aceitam parâmetros de consulta `asset_id` opcionais para pré-filtragem, além dos limites existentes de paginação/backpressure.

## Autenticacao e assinatura canonica

- Endpoints GET/POST voltados a apps aceitam headers opcionais de requisicao canonica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos de `METHOD\n/path\nsorted_query\nsha256(body)`; o Torii os envolve em `QueryRequestWithAuthority` antes da validacao do executor para espelhar `/query`.
- Helpers de SDK existem em todos os clientes principais:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemplos:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/soraカタカナ.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/soraカタカナ.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/soraカタカナ.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/soraカタカナ.../assets", "limit=5", ByteArray(0), signer)
```

## Inventario de endpoints

### Permissoes de conta (`/v1/accounts/{id}/permissions`) - Coberto
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notas: A resposta e um body JSON Norito com `items`/`total`, alinhado aos helpers de paginacao dos SDKs.

### Avaliacao OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Coberto
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: testes inline do handler (`crates/iroha_torii/src/lib.rs:9945-9986`) mais cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notas: A superficie de resposta reforca hex deterministico e identificadores de backend; os SDKs consomem o DTO.

### Eventos de proof SSE (`GET /v1/events/sse`) - Coberto
- Handler: `handle_v1_events_sse` com suporte a filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mais o wiring de filtro de proof.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: suites SSE especificas de proof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste smoke SSE do pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notas: Os caminhos de filtro de proof foram validados end-to-end; a documentacao fica em `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Coberto
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: suites router/integracao `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG com Torii Platform.
- Notas: Os endpoints enfileiram transacoes assinadas e reutilizam metricas de telemetria compartilhadas (`handle_transaction_with_metrics`).

### Ciclo de vida de chaves de verificacao (`/v1/zk/vk/*`) - Coberto
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group com suporte da Torii Platform.
- Notas: Os DTOs se alinham aos schemas Norito referenciados pelos SDKs; rate limiting e aplicado via `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - Coberto (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, ciclo de vida de sessao, handshake WS) e
  cobertura da matriz de features do router (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notas: Chaves de rate limit sao rastreadas via `limits::rate_limit_key`; contadores de telemetria alimentam as metricas `connect.*`.

### Telemetria de relay Kaigi - Coberto
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: O stream SSE reutiliza o canal global de broadcast enquanto aplica o gating do perfil de telemetria; os schemas de resposta estao documentados em `docs/source/torii/kaigi_telemetry_api.md`.

## Resumo de cobertura de testes

- Testes smoke do router (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantem que combinacoes de features registrem todas as rotas e que a geracao de OpenAPI fique sincronizada.
- Suites especificas de endpoints cobrem queries de contas, ciclo de vida de contratos, chaves de verificacao ZK, filtros de proof SSE e comportamentos do Nexus Connect.
- Harnesses de paridade de SDK (JavaScript, Swift, Python) ja consomem Alias VOPRF e endpoints SSE; nao ha trabalho adicional.

## Manter este espelho atualizado

Atualize esta pagina e a auditoria fonte (`docs/source/torii/app_api_parity_audit.md`) quando o comportamento da app API de Torii mudar para que os owners de SDK e leitores externos fiquem alinhados.
