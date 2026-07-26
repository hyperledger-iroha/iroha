---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de implementação do Pin Registry de SoraFS
sidebar_label: Registro do Plano del Pin
descrição: Plano de implementação SF-4 que cobre a maquina de estados do registro, a fachada Torii, ferramental e observabilidade.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as cópias sincronizadas enquanto a documentação herdada segue ativa.
:::

# Plano de implementação do Pin Registry de SoraFS (SF-4)

SF-4 entrega o contrato do Pin Registry e os serviços de suporte que armazena
compromissos de manifesto, cumplir políticas de pin e expor APIs a Torii, gateways
e orquestradores. Este documento amplia o plano de validação com tarefas de
implementação concreta, cobrindo a lógica on-chain, os serviços do host, os
fixtures e os requisitos operacionais.

## Alcance

1. **Máquina de estados do registro**: registros definidos por Norito para manifestos,
   aliases, cadenas sucessoras, épocas de retenção e metadados de governança.
2. **Implementação do contrato**: operações CRUD deterministas para o ciclo de vida
   de pinos (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Fachada de serviço**: endpoints gRPC/REST respaldados pelo registro que consome
   Torii e os SDKs, incluindo paginação e atestado.
4. **Ferramentas e acessórios**: auxiliares de CLI, vetores de teste e documentação para manutenção
   manifestos, aliases e envelopes de governança sincronizados.
5. **Telemetria e operações**: métricas, alertas e runbooks para a saúde do registro.

## Modelo de dados

### Registros centrais (Norito)

| Estrutura | Descrição | Campos |
|------------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​do Mapea -> CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruções para que os provedores fixem o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusação de recibo do provedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantâneo da política de governo. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Luminárias e CI- Diretório de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda snapshots firmados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se houver diferenças, mantendo os fixtures de CI alineados.
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) ejercitam o fluxo feliz, mas o rechazo de alias duplicado, guardas de aprovação/retenção de alias, alças de chunker desalineados, validação de conteúdo de réplicas e falhas de guardas de sucessão (punteros desconhecidos/pré-aprovados/retirados/autorreferencias); ver casos `register_manifest_rejects_*` para detalhes de cobertura.
- Testes unitários agora cobrem validação de alias, guardas de retenção e verificações de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucessão multi-hop quando a máquina de estados é acionada.
- JSON golden para eventos usados ​​por pipelines de observabilidade.

## Telemetria e observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- A telemetria existente dos provedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) segue em alcance para painéis de controle ponta a ponta.

Registros:
- Stream de eventos Norito estruturados para auditórios de governo (firmados?).

Alertas:
- Ordens de replicação pendentes excedendo o SLA.
- Expiração de alias por baixo do umbral.
- Violações de retenção (manifestadas sem renovação antes de expirar).

Painéis:
- O JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia total de ciclo de vida de manifestos, cobertura de alias, saturação de backlog, proporção de SLA, sobreposições de latência vs folga e tarefas de ordens perdidas para revisão de plantão.

## Runbooks e documentação

- Atualizar `docs/source/sorafs/migration_ledger.md` para incluir atualizações de estado do registro.
- Guia de operadores: `docs/source/sorafs/runbooks/pin_registry_ops.md` (já publicado) cubriendo métricas, alertas, despliegue, backup e fluxos de recuperação.
- Guia de governança: descreve parâmetros políticos, fluxo de trabalho de aprovação, manejo de disputas.
- Páginas de referência de API para cada endpoint (documentos Docusaurus).

## Dependências e sequências

1. Completar tarefas do plano de validação (integração do ManifestValidator).
2. Finalizar esquema Norito + padrões de política.
3. Implementar contrato + serviço, conectar telemetria.
4. Regenerar fixtures, executar suítes de integração.
5. Atualize documentos/runbooks e marque itens do roadmap como completos.

Cada lista de verificação do SF-4 deve referenciar este plano à medida que progride.
La fachada REST agora entrega endpoints de listado com atestado:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` expõem o catálogo de
  alias ativo e o backlog de ordens de replicação com paginação consistente e
  filtros de estado.O ambiente CLI é chamado (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que os operadores possam automatizar auditorias do
registro sem tocar APIs de baixo nível.