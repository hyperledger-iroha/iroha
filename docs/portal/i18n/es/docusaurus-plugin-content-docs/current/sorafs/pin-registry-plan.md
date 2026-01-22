<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: pin-registry-plan
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
| `PinRecordV1` | Entrada canonica de manifest. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Mapea alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruccion para que los providers pinneen el manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acuse de recibo del provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politica de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementacion: ver `crates/sorafs_manifest/src/pin_registry.rs` para los
esquemas Norito en Rust y los helpers de validacion que respaldan estos registros. La
validacion refleja el tooling de manifest (lookup del chunker registry, pin policy gating)
para que el contrato, las fachadas Torii y la CLI compartan invariantes identicas.

Tareas:
- Finalizar los esquemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Generar codigo (Rust + otros SDKs) usando macros Norito.
- Actualizar la documentacion (`sorafs_architecture_rfc.md`) una vez que los esquemas esten listos.

## Implementacion del contrato

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Implementar almacenamiento del registry (sled/sqlite/off-chain) o modulo de smart contract. | Core Infra / Smart Contract Team | Proveer hashing determinista, evitar punto flotante. |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | Aprovechar `ManifestValidator` del plan de validacion. El binding de alias ahora fluye via `RegisterPinManifest` (DTO de Torii) mientras `bind_alias` dedicado sigue planeado para actualizaciones sucesivas. |
| Transiciones de estado: imponer sucesion (manifest A -> B), epocas de retencion, unicidad de alias. | Governance Council / Core Infra | Unicidad de alias, limites de retencion y verificaciones de aprobacion/retiro de predecesores viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la deteccion de sucesion multi-hop y la contabilidad de replicacion siguen abiertas. |
| Parametros gobernados: cargar `ManifestPolicyV1` desde config/estado de gobernanza; permitir actualizaciones via eventos de gobernanza. | Governance Council | Proveer CLI para actualizaciones de politica. |
| Emision de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Definir esquema de eventos + logging. |

Pruebas:
- Tests unitarios para cada entry point (positivo + rechazo).
- Tests de propiedades para la cadena de sucesion (sin ciclos, epocas monotonicamente crecientes).
- Fuzz de validacion generando manifests aleatorios (acotados).

## Fachada de servicio (Integracion Torii/SDK)

| Componente | Tarea | Responsable(s) |
|-----------|-------|----------------|
| Servicio Torii | Exponer `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Proveer paginacion + filtrado. | Networking TL / Core Infra |
| Atestacion | Incluir altura/hash del registry en respuestas; agregar estructura de atestacion Norito consumida por los SDKs. | Core Infra |
| CLI | Extender `sorafs_manifest_stub` o un nuevo CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Generar bindings de cliente (Rust/Go/TS) desde el esquema Norito; agregar tests de integracion. | SDK Teams |

Operaciones:
- Agregar capa de cache/ETag para endpoints GET.
- Proveer rate limiting / auth consistentes con las politicas de Torii.

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

- `GET /v1/sorafs/pin` y `GET /v1/sorafs/pin/{digest}` devuelven manifests con
  bindings de alias, ordenes de replicacion y un objeto de atestacion derivado del
  hash del ultimo bloque.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` exponen el catalogo de
  alias activo y el backlog de ordenes de replicacion con paginacion consistente y
  filtros de estado.

La CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias del
registry sin tocar APIs de bajo nivel.
