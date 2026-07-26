---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 404ab4f7fe0986d386c6ed163cae03bf9d1d317703994b1e6ae0b078a976b1d3
source_last_modified: "2025-11-09T05:21:23.178760+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: pin-registry-plan
title: Plan de implementacion del Pin Registry de SoraFS
sidebar_label: Plan del Pin Registry
description: Plan de implementacion SF-4 que cubre la maquina de estados del registry, la fachada Torii, tooling y observabilidad.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sorafs/pin_registry_plan.md`. Manten ambas copias sincronizadas mientras la documentacion heredada siga activa.
:::

# Plan de implementacion del Pin Registry de SoraFS (SF-4)

SF-4 entrega el contrato del Pin Registry y los servicios de soporte que almacenan
compromisos de manifest, hacen cumplir politicas de pin y exponen APIs a Torii, gateways
y orquestadores. Este documento amplia el plan de validacion con tareas de
implementacion concretas, cubriendo la logica on-chain, los servicios del host, los
fixtures y los requisitos operativos.

## Alcance

1. **Maquina de estados del registry**: registros definidos por Norito para manifests,
   aliases, cadenas sucesoras, epocas de retencion y metadatos de gobernanza.
2. **Implementacion del contrato**: operaciones CRUD deterministas para el ciclo de vida
   de pins (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Fachada de servicio**: endpoints gRPC/REST respaldados por el registry que consumen
   Torii y los SDKs, incluyendo paginacion y atestacion.
4. **Tooling y fixtures**: helpers de CLI, vectores de prueba y documentacion para mantener
   manifests, aliases y envelopes de gobernanza sincronizados.
5. **Telemetria y ops**: metricas, alertas y runbooks para la salud del registry.

## Modelo de datos

### Registros centrales (Norito)

| Estructura | Descripcion | Campos |
|------------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mapea alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruccion para que los providers pinneen el manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acuse de recibo del provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politica de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Fixtures y CI

- Directorio de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda snapshots firmados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI: `ci/check_sorafs_fixtures.sh` regenera el snapshot y falla si hay diffs, manteniendo los fixtures de CI alineados.
- Tests de integracion (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el flujo feliz mas el rechazo de alias duplicado, guards de aprobacion/retencion de alias, handles de chunker desalineados, validacion de conteo de replicas y fallos de guardas de sucesion (punteros desconocidos/preaprobados/retirados/autorreferencias); ver casos `register_manifest_rejects_*` para detalles de cobertura.
- Tests unitarios ahora cubren validacion de alias, guards de retencion y checks de sucesor en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la deteccion de sucesion multi-hop cuando aterrice la maquina de estados.
- JSON golden para eventos usados por pipelines de observabilidad.

## Telemetria y observabilidad

Metricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetria existente de providers (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) sigue en alcance para dashboards end-to-end.

Logs:
- Stream de eventos Norito estructurados para auditorias de gobernanza (firmados?).

Alertas:
- Ordenes de replicacion pendientes excediendo el SLA.
- Expiracion de alias por debajo del umbral.
- Violaciones de retencion (manifest no renovado antes de expirar).

Dashboards:
- El JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea totales de ciclo de vida de manifests, cobertura de alias, saturacion de backlog, ratio de SLA, overlays de latencia vs slack y tasas de ordenes perdidas para revision on-call.

## Runbooks y documentacion

- Actualizar `docs/source/sorafs/migration_ledger.md` para incluir actualizaciones de estado del registry.
- Guia de operadores: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ya publicada) cubriendo metricas, alertas, despliegue, backup y flujos de recuperacion.
- Guia de gobernanza: describir parametros de politica, workflow de aprobacion, manejo de disputas.
- Paginas de referencia de API para cada endpoint (Docusaurus docs).

## Dependencias y secuenciacion

1. Completar tareas del plan de validacion (integracion de ManifestValidator).
2. Finalizar esquema Norito + defaults de politica.
3. Implementar contrato + servicio, conectar telemetria.
4. Regenerar fixtures, correr suites de integracion.
5. Actualizar docs/runbooks y marcar items del roadmap como completos.

Cada checklist de SF-4 debe referenciar este plan cuando se marque progreso.
La fachada REST ahora entrega endpoints de listado con atestacion:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` exponen el catalogo de
  alias activo y el backlog de ordenes de replicacion con paginacion consistente y
  filtros de estado.

La CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias del
registry sin tocar APIs de bajo nivel.
