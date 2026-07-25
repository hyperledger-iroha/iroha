---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plan de implementación del Registro Pin de SoraFS
sidebar_label: Registro Plan del Pin
descripción: Plan de implementación SF-4 que cubre la maquina de estados del registro, la fachada Torii, utillaje y observabilidad.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/pin_registry_plan.md`. Mantenga ambas copias sincronizadas mientras la documentación heredada siga activa.
:::

# Plan de implementación del Registro Pin de SoraFS (SF-4)

SF-4 entrega el contrato del Pin Registry y los servicios de soporte que almacenan
compromisos de manifiesto, hacen cumplir politicas de pin y exponen APIs a Torii, gateways
y orquestadores. Este documento amplía el plan de validación con tareas de
implementacion concretas, cubriendo la logica on-chain, los servicios del host, los
accesorios y los requisitos operativos.

## Alcance1. **Maquina de estados del registro**: registros definidos por Norito para manifiestos,
   alias, cadenas sucesoras, epocas de retencion y metadatos de gobernanza.
2. **Implementación del contrato**: operaciones CRUD deterministas para el ciclo de vida
   de pines (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Fachada de servicio**: endpoints gRPC/REST respaldados por el registro que consumen
   Torii y los SDK, incluyendo paginación y atestación.
4. **Herramientas y accesorios**: ayudantes de CLI, vectores de prueba y documentación para mantener
   manifiestos, alias y sobres de gobernanza sincronizados.
5. **Telemetria y ops**: métricas, alertas y runbooks para la salud del registro.

## Modelo de datos

### Registros centrales (Norito)| Estructura | Descripción | Campos |
|------------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​de Mapea -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucción para que los proveedores pinneen el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acuse de recibo del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de política de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendario y CI- Directorio de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda instantáneas firmadas de manifiesto/alias/orden regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI: `ci/check_sorafs_fixtures.sh` regenera el snapshot y falla si hay diferencias, manteniendo los accesorios de CI alineados.
- Tests de integracion (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el flujo feliz mas el rechazo de alias duplicado, guards de aprobacion/retencion de alias, handles de chunker desalineados, validacion de conteo de replicas y fallos de guardas de sucesion (punteros desconocidos/preaprobados/retirados/autorreferencias); ver casos `register_manifest_rejects_*` para detalles de cobertura.
- Tests unitarios ahora cubren validación de alias, guards de retencion y checks de sucesor en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la deteccion de sucesion multi-hop cuando aterrice la maquina de estados.
- JSON golden para eventos usados ​​por tuberías de observabilidad.

## Telemetria y observabilidad

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetría existente de proveedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) sigue al alcance para paneles de control de extremo a extremo.

Registros:
- Stream de eventos Norito estructurados para auditorias de gobernanza (firmados?).Alertas:
- Órdenes de replicación pendientes excediendo el SLA.
- Expiración de alias por debajo del umbral.
- Violaciones de retención (manifiestas no renovadas antes de expirar).

Paneles de control:
- El JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea totales de ciclo de vida de manifiestos, cobertura de alias, saturación de backlog, ratio de SLA, overlays de latencia vs slack y tasas de órdenes perdidas para revisión de guardia.

## Runbooks y documentación

- Actualizar `docs/source/sorafs/migration_ledger.md` para incluir actualizaciones de estado del registro.
- Guía de operadores: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ya publicado) cubriendo métricas, alertas, despliegue, backup y flujos de recuperación.
- Guía de gobernanza: describir parámetros de política, flujo de trabajo de aprobación, manejo de disputas.
- Páginas de referencia de API para cada endpoint (documentos Docusaurus).

## Dependencias y secuenciación

1. Completar tareas del plan de validación (integración de ManifestValidator).
2. Finalizar esquema Norito + defaults de politica.
3. Implementar contrato + servicio, conectar telemetria.
4. Regenerar accesorios, correr suites de integración.
5. Actualizar docs/runbooks y marcar elementos del roadmap como completos.

Cada lista de verificación de SF-4 debe hacer referencia a este plan cuando se marque progreso.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` exponen el catálogo de
  alias activo y el backlog de órdenes de replicacion con paginacion consistente y
  filtros de estado.

La CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias del
registro sin tocar APIs de bajo nivel.