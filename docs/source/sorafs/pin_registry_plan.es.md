---
lang: es
direction: ltr
source: docs/source/sorafs/pin_registry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bfb0e80600ac368e40af132f1b37c4c28f3bc0f70c668b65ae0ae7893c677341
source_last_modified: "2025-11-22T12:55:26.785916+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plan de implementacion del Pin Registry SoraFS (SF-4)

SF-4 entrega el contrato Pin Registry y servicios de soporte que almacenan
compromisos de manifiesto, aplican politicas de pin y exponen APIs a Torii,
gateways y orquestadores. Este documento expande el plan de validacion con
 tareas concretas de implementacion, cubriendo logica on-chain, servicios del
host, fixtures y requisitos operativos.

## Alcance

1. **Maquina de estados del registry**: registros definidos en Norito para
   manifiestos, aliases, cadenas de sucesores, epochs de retencion y metadata de
   governance.
2. **Implementacion del contrato**: operaciones CRUD deterministas para el
   ciclo de vida del pin (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Service facade**: endpoints gRPC/REST respaldados por el registry que Torii
   y SDKs consumen, incluyendo paginacion y atestacion.
4. **Tooling & fixtures**: helpers CLI, vectores de test y documentacion para
   mantener manifiestos, aliases y sobres de governance en sync.
5. **Telemetria & ops**: metricas, alertas y runbooks para salud del registry.

## Modelo de datos

### Registros core (Norito)

| Struct | Descripcion | Campos |
|--------|-------------|--------|
| `PinRecordV1` | Entrada canonica de manifiesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Mapea alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruccion para providers de pinnear manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acknowledgement del provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politica de governance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementacion: ver `crates/sorafs_manifest/src/pin_registry.rs`
para los schemas Norito en Rust y helpers de validacion que respaldan estos
registros. La validacion refleja el tooling de manifiestos (lookup del registry
de chunker, gating de politica de pin) para que el contrato, los facades Torii
y el CLI compartan invariantes identicas.

Tareas:
- Finalizar schemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Generar codigo (Rust + otros SDKs) usando macros Norito.
- Actualizar docs (`sorafs_architecture_rfc.md`) una vez aterricen los schemas.

## Implementacion del contrato

| Tarea | Owner(s) | Notas |
|------|----------|-------|
| Implementar storage del registry (sled/sqlite/off-chain) o modulo de smart contract. | Core Infra / Smart Contract Team | Proveer hashing determinista, evitar floating point. |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | Usar `ManifestValidator` del plan de validacion. Alias binding ahora fluye via `RegisterPinManifest` (DTO Torii expuesto) mientras `bind_alias` dedicado sigue planificado para updates sucesivos. |
| Transiciones de estado: enforcement de sucesion (manifiesto A -> B), epochs de retencion, unicidad de alias. | Governance Council / Core Infra | Unicidad de alias, limites de retencion y checks de predecesor aprobados/retirados viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; deteccion multi-hop de sucesion y bookkeeping de replicacion siguen abiertos. |
| Parametros gobernados: cargar `ManifestPolicyV1` desde config/estado de governance; permitir updates via eventos de governance. | Governance Council | Proveer CLI para updates de politica. |
| Emision de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Definir schema de eventos + logging. |

Testing:
- Unit tests para cada entry point (positivo + rechazo).
- Property tests para la cadena de sucesion (sin ciclos, epochs monotonicos).
- Fuzz validation generando manifiestos aleatorios (acotados).

## Service facade (integracion Torii/SDK)

| Componente | Tarea | Owner(s) |
|-----------|-------|----------|
| Servicio Torii | Exponer `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Proveer paginacion + filtering. | Networking TL / Core Infra |
| Attestation | Incluir altura/hash del registry en responses; agregar struct de atestacion Norito consumida por SDKs. | Core Infra |
| CLI | Extender `sorafs_manifest_stub` o nuevo CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Generar bindings cliente (Rust/Go/TS) desde schema Norito; agregar integration tests. | SDK Teams |

Operaciones:
- Agregar layer de caching/ETag para endpoints GET.
- Proveer rate limiting / auth consistente con politicas Torii.

## Fixtures y CI

- Directorio de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda snapshots firmados de manifiesto/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso CI: `ci/check_sorafs_fixtures.sh` regenera el snapshot y falla si hay diffs, manteniendo fixtures alineados.
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el happy path mas rechazo de alias duplicado, guards de aprobacion/retencion de alias, mismatched chunker handles, validacion de replica-count y fallas de successor-guard (unknown/pre-approved/retired/self pointers); ver casos `register_manifest_rejects_*` para cobertura.
- Unit tests ahora cubren validacion de alias, guards de retencion y checks de sucesor en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; deteccion multi-hop de sucesion una vez aterrice la maquina de estados.
- Golden JSON para eventos usados por pipelines de observabilidad.

## Telemetria y observabilidad

Metricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetria existente de providers (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) sigue en scope para dashboards end-to-end.

Logs:
- Stream de eventos Norito estructurados para auditorias de governance (firmado?).

Alertas:
- Ordenes de replicacion pendientes excediendo SLA.
- Expiracion de alias < threshold.
- Violaciones de retencion (manifiesto no renovado antes de expiry).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` rastrea totales de ciclo de vida de manifiestos, cobertura de alias, saturacion de backlog, ratio SLA, overlays de latencia vs slack, y tasas de ordenes fallidas para revision on-call.

## Runbooks y documentacion

- Actualizar `docs/source/sorafs/migration_ledger.md` para incluir updates de estado del registry.
- Guia de operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ya publicada) cubre metricas, alertas, despliegue, backup y flows de recovery.
- Guia de governance: describir parametros de politica, workflow de aprobacion, manejo de disputas.
- Paginas de referencia API para cada endpoint (docs Docusaurus).

## Dependencias y secuenciacion

1. Completar tareas del plan de validacion (integracion ManifestValidator).
2. Finalizar schema Norito + defaults de politica.
3. Implementar contrato + servicio, cablear telemetria.
4. Regenerar fixtures, correr suites de integracion.
5. Actualizar docs/runbooks y marcar items del roadmap completos.

Cada item del checklist bajo SF-4 debe referenciar este plan cuando haya progreso.
Las REST facade ahora entregan endpoints de listado con atestacion:

- `GET /v1/sorafs/pin` y `GET /v1/sorafs/pin/{digest}` retornan manifiestos con
  alias bindings, ordenes de replicacion y un objeto de atestacion derivado del
  ultimo hash de bloque.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` exponen el catalogo de
  aliases activos y el backlog de ordenes de replicacion con paginacion y
  filtros de estado consistentes.

El CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que operadores puedan scriptar auditorias del registry
sin tocar APIs de bajo nivel.
