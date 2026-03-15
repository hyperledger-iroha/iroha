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
| `PinRecordV1` | Entrada canónica de manifiesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​de Mapea -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucción para que los proveedores pinneen el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acuse de recibo del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de política de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementación: ver `crates/sorafs_manifest/src/pin_registry.rs` para los
esquemas Norito en Rust y los ayudantes de validación que respaldan estos registros. la
validación refleja las herramientas de manifiesto (búsqueda del registro fragmentador, activación de políticas de pin)
Para que el contrato, las fachadas Torii y la CLI compartan invariantes idénticas.

Tareas:
- Finalizar los esquemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Generar código (Rust + otros SDK) usando macros Norito.
- Actualizar la documentación (`sorafs_architecture_rfc.md`) una vez que los esquemas están listos.## Implementación del contrato

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Implementar almacenamiento del registro (sled/sqlite/off-chain) o módulo de contrato inteligente. | Equipo central de infraestructura/contrato inteligente | Proveer hash determinista, evitar punto flotante. |
| Puntos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | Aprovechar `ManifestValidator` del plan de validación. El enlace de alias ahora fluye a través de `RegisterPinManifest` (DTO de Torii) mientras `bind_alias` dedicado sigue planeado para actualizaciones sucesivas. |
| Transiciones de estado: imponer sucesión (manifiesto A -> B), épocas de retención, unicidad de alias. | Consejo de Gobernanza / Core Infraestructura | Unicidad de alias, límites de retención y verificaciones de aprobación/retiro de predecesores viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la detección de sucesión multi-hop y la contabilidad de replicación siguen abiertas. |
| Parámetros gobernados: cargar `ManifestPolicyV1` desde config/estado de gobernanza; Permitir actualizaciones a través de eventos de gobernanza. | Consejo de Gobierno | Proveer CLI para actualizaciones de política. |
| Emisión de eventos: emite eventos Norito para telemetría (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidad | Definir esquema de eventos + logging. |Pruebas:
- Pruebas unitarias para cada punto de entrada (positivo + rechazo).
- Tests de propiedades para la cadena de sucesión (sin ciclos, epocas monotonicamente crecientes).
- Fuzz de validacion generando manifiestos aleatorios (acotados).

## Fachada de servicio (Integración Torii/SDK)

| Componente | Tarea | Responsable(s) |
|-----------|---------------|----------------|
| Servicio Torii | Exponer `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (búsqueda), `/v1/sorafs/aliases` (listar/vincular), `/v1/sorafs/replication` (pedidos/recibos). Proveer paginacion + filtrado. | Redes TL / Core Infraestructura |
| Atestación | Incluir altura/hash del registro en respuestas; Agregar estructura de atestación Norito consumida por los SDK. | Infraestructura básica |
| CLI | Extensor `sorafs_manifest_stub` o un nuevo CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Grupo de Trabajo sobre Herramientas |
| SDK | Generar fijaciones de cliente (Rust/Go/TS) desde el esquema Norito; Agregar pruebas de integración. | Equipos SDK |

Operaciones:
- Agregar capa de caché/ETag para puntos finales GET.
- Proveer rate limiting / auth consistentes con las políticas de Torii.

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
La fachada REST ahora entrega endpoints de listado con atestación:- `GET /v1/sorafs/pin` y `GET /v1/sorafs/pin/{digest}` devuelven manifests con
  vinculaciones de alias, órdenes de replicación y un objeto de atestación derivado del
  hash del último bloque.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` exponen el catálogo de
  alias activo y el backlog de órdenes de replicacion con paginacion consistente y
  filtros de estado.

La CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias del
registro sin tocar APIs de bajo nivel.