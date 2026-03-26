---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
título: Auditoria de parte da API do aplicativo Torii
descrição: Espelho da revista TORII-APP-1 para que as equipes SDK e a plataforma confirmem a cobertura pública.
---

Estatuto: Término em 21/03/2026  
Responsáveis: Plataforma Torii, líder do programa SDK  
Referência do roteiro: TORII-APP-1 - auditoria de paridade `app_api`

Esta página reflete a auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que os leitores fora do mono-repo possam ver essas superfícies `/v1/*` serem cabos, testados e documentados. A auditoria atende às rotas reexportadas via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Porta e método

A auditoria inspeciona as reexportações públicas em `crates/iroha_torii/src/lib.rs:256-522` e os construtores de rotas que estão no recurso gating. Para cada superfície `/v1/*` do roteiro, vamos verificar:

- Implementação do manipulador e definições de DTO em `crates/iroha_torii/src/routing.rs`.
- Registre o roteador sob os grupos de recursos `app_api` ou `connect`.
- Testes de integração/unidades existentes e equipe responsável pela cobertura a longo prazo.

As listas de ativos/transações de conta e as listas de detentores de ativos aceitam os parâmetros de solicitação `asset_id` facultativos para a pré-filtragem, além dos limites de paginação/contrapressão existentes.

## Autenticação e assinatura canônica

- Os endpoints GET/POST expõem aplicativos que aceitam cabeçalhos opcionais de recepção canônica (`X-Iroha-Account`, `X-Iroha-Signature`) construídos a partir de `METHOD\n/path\nsorted_query\nsha256(body)`; Torii o envelope em `QueryRequestWithAuthority` antes da validação do executor para refletir `/query`.
- Os auxiliares SDK são fornecidos em todos os clientes principais:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` depois de `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
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

## Inventário de endpoints

### Permissões de conta (`/v1/accounts/{id}/permissions`) - Couvert
- Manipulador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Proprietário: Plataforma Torii.
- Notas: A resposta é um corpo JSON Norito com `items`/`total`, conforme ajudantes de paginação do SDK.

### Avaliação OPRF d'alias (`POST /v1/aliases/voprf/evaluate`) - Couvert
- Manipulador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Ligação do roteador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testes: testes inline du handler (`crates/iroha_torii/src/lib.rs:9945-9986`) mais SDK de cobertura
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Proprietário: Plataforma Torii.
- Notas: A superfície de resposta impõe um hexadecimal determinante e identificadores de backend; o SDK usa o DTO.### Eventos de prova SSE (`GET /v1/events/sse`) - Couvert
- Manipulador: `handle_v1_events_sse` com suporte para filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mais a fiação à prova de filtro.
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: suítes de prova específica SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste de fumaça SSE do pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Proprietário: Plataforma Torii (runtime), GT de Testes de Integração (fixtures).
- Notas: Les chemins de filtre proof são válidos de bout en bout; a documentação é encontrada em `docs/source/zk_app_api.md`.

### Cycle de vie des contrats (`/v1/contracts/*`) - Couvert
- Manipuladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: suítes roteador/integração `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Proprietário: Smart Contract WG com plataforma Torii.
- Notas: Os endpoints armazenam em um arquivo as transações assinadas e reutilizam as métricas de compartilhamentos de telemetria (`handle_transaction_with_metrics`).

### Ciclo de vida das regras de verificação (`/v1/zk/vk/*`) - Couvert
- Manipuladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Proprietário: ZK Working Group com suporte à plataforma Torii.
- Notas: Os DTOs estão alinhados aos esquemas Norito referenciados pelo SDK; A limitação de taxa é imposta via `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - Couvert (recurso `connect`)
- Manipuladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Ligação do roteador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, ciclo de vida de sessão, handshake WS) et
  cobertura da matriz de recursos do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Proprietário: Nexus Connect WG.
- Notas: Les cles de rate limit são sucessivas via `limits::rate_limit_key`; os computadores de telemetria alimentam as métricas `connect.*`.### Telemetria de relé Kaigi - Couvert
- Manipuladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
Ligação do roteador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testes: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: O fluxo SSE reutiliza o canal global de transmissão em todos os aplicativos de controle do perfil de telemetria; Os esquemas de resposta estão documentados em `docs/source/torii/kaigi_telemetry_api.md`.

## Currículo da cobertura de testes

- Os testes de fumaça do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantem que as combinações de recursos sejam registradas em cada rota e que a geração OpenAPI fique sincronizada.
- As suítes de endpoints cobrem as solicitações de contas, o ciclo de vida dos contratos, as chaves de verificação ZK, os filtros à prova de SSE e os comportamentos Nexus Connect.
- Os recursos de parite SDK (JavaScript, Swift, Python) como o Alias ​​VOPRF e os endpoints SSE; nenhum trabalho suplementar necessário.

## Garder ce miroir a jour

Abra esta página e a fonte de auditoria (`docs/source/torii/app_api_parity_audit.md`) quando o comportamento da API do aplicativo Torii for alterado para que os proprietários SDK e os leitores externos permaneçam alinhados.