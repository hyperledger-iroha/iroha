---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plano de implementación del Registro de PIN del SoraFS
sidebar_label: Registro Plano do Pin
descripción: Plano de implementación SF-4 cobrindo una máquina de estados de registro, una fachada Torii, herramientas y observabilidade.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas mientras la documentacao herdada permanezca activa.
:::

# Plano de implementación del Registro de PIN de SoraFS (SF-4)

La entrega del SF-4 o el contrato del Registro Pin y los servicios de apoyo que armazenam
compromisos de manifiesto, impoem politicas de pin y expoem APIs para Torii,
pasarelas y orquestadores. Este documento amplio o plano de validación com
tarefas de implementacao concretas, cobrindo a logica on-chain, os servicos do
host, los accesorios y los requisitos operativos.

##escopo1. **Maquina de estados de registro**: registros definidos por Norito para manifiestos,
   alias, cadeias sucessoras, epocas de retencao e metadados degobernanza.
2. **Implementación del contrato**: operaciones CRUD determinísticas para el ciclo de vida
   dos pines (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Fachada de servicio**: endpoints gRPC/REST sustentados por el registro que Torii
   Además de los SDK, se incluyen páginas y pruebas.
4. **Herramientas y accesorios**: ayudantes de CLI, vectores de prueba y documentación para mantener
   manifiestos, alias y sobres de gobierno sincronizados.
5. **Telemetría y operaciones**: métricas, alertas y runbooks para el registro.

## Modelo de dados

### Registros centrales (Norito)| Estructura | Descripción | Campos |
|--------|-----------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​de Mapeia -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucciones para que los proveedores fijen el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmacao do proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de política de gobierno. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendario y CI- Directorio de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assinados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot y falha se houver diffs, manteniendo los accesorios de CI alinhados.
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeicao de alias duplicado, guards de aprovacao/retencao de alias, handles de chunker incompativeis, validacao de contagem de replicas e falhas de guardas de sucessao (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` para detalles de cobertura.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados ​​pelos pipelines de observabilidade.

## Telemetría y observabilidad

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetría existente de proveedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece en el alcance de los paneles de control de un extremo a otro.

Registros:
- Stream de eventos Norito estruturados para auditorias degobernanza (assinados?).

Alertas:
- Órdenes de replicación pendientes excedendo o SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (manifiesto nao renovado antes de expirar).Paneles de control:
- El JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea todo el ciclo de vida de dos manifiestos, cobertura de alias, saturación del trabajo pendiente, raza de SLA, superposiciones de latencia vs slack y taxas de órdenes perdidas para revisión de guardia.

## Runbooks y documentación

- Actualizar `docs/source/sorafs/migration_ledger.md` para incluir actualizaciones de estado del registro.
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) cobrindo métricas, alertas, despliegue, respaldo y flujos de recuperación.
- Guía de gobernanza: descripción de parámetros de política, flujo de trabajo de aprobación, tratamento de disputas.
- Páginas de referencia de API para cada endpoint (docs Docusaurus).

## Dependencias y secuenciación

1. Completar tarefas del plano de validación (integración del ManifestValidator).
2. Finalizar esquema Norito + defaults de politica.
3. Implementar contrato + servicio, conectar telemetria.
4. Regenerar accesorios, rodar suites de integracao.
5. Actualizar documentos/runbooks y marcar elementos de la hoja de ruta como completos.

Cada lista de verificación del SF-4 debe hacer referencia a este plano cuando usted progresa.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` exposición o catálogo de
  alias ativo e o backlog de órdenes de replicacao com paginacao consistente e
  filtros de estado.

Una CLI encapsula esas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias
registro sin tocar API de bajo nivel.