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

Estado:
- Los schemas Norito en `crates/sorafs_manifest/src/pin_registry.rs` son la
  superficie de primer release usada por core, Torii, fixtures y validadores de
  referencia.
- La generacion Rust usa derives Norito; la paridad de SDKs sigue los guardrails
  normales cuando cambia el schema.
- La arquitectura, migracion, pipeline de manifiestos, CLI, OpenAPI, status y
  roadmap ya describen la ruta de validacion compartida y el comportamiento de
  endpoints.

## Implementacion del contrato

| Tarea | Owner(s) | Notas |
|------|----------|-------|
| Estado de smart-contract y storage del registry. | Core Infra / Smart Contract Team | Implementado en el estado world de Iroha (`pin_manifests`, `manifest_aliases`, `replication_orders`) con hashing Norito determinista y aritmetica entera de politicas. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`. | Core Infra | La ejecucion core valida aliases, envelopes de consejo, politica de pin, ownership de replicacion y completion de ordenes; no queda backlog local separado para `bind_alias`. |
| Transiciones de estado: sucesion (manifiesto A -> B), epochs de retencion, unicidad de alias y cambios de estado de replicacion. | Governance Council / Core Infra | `ensure_successor_chain` fuerza lineage multi-hop aprobado, no retirado y aciclico; unicidad/retencion de alias y bookkeeping issue/complete de replicacion estan cubiertos por unit tests. |
| Parametros gobernados: cargar `ManifestPolicyV1` desde config/estado de governance. | Governance Council | La config runtime mapea constraints de pin-policy al validador compartido. Las ceremonias live de cambio de politica son evidencia de rollout, no codigo local faltante. |
| Telemetria y auditoria del registry. | Observability | Torii exporta metricas del registry y snapshots REST atestados. Archivos de eventos firmados pueden superponerse a esos snapshots si governance lo requiere. |

Cobertura:
- Unit tests cubren registro, aprobacion, retiro, alias binding, issue/complete
  de ordenes de replicacion, permisos, rechazo de duplicados y fallos sin side effects.
- Tests de sucesor cubren referencias a si mismo, predecesores unknown/pending/retired,
  cierre de ciclos y ciclos existentes malformados en la cadena.
- `ci/check_sorafs_fixtures.sh` regenera fixtures de chunker, provider-admission
  y pin registry, y luego ejecuta checks de paridad que mantienen estable la
  superficie canonica de schemas.

## Service facade (integracion Torii/SDK)

| Componente | Tarea | Owner(s) |
|-----------|-------|----------|
| Servicio Torii | Publica `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest}`, `/v1/sorafs/aliases` y `/v1/sorafs/replication` para list/lookup con paginacion y filtros deterministas. | Networking TL / Core Infra |
| Attestation | Las responses de lista y detalle incluyen el objeto de atestacion derivado del ultimo block hash. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list` y `replication list` envuelven las superficies REST e ISI para auditorias de operador. | Tooling WG |
| SDK | Los builders Rust y los guardrails JavaScript, Python, Swift y C# reflejan el payload de manifiesto y la validacion pin-register. | SDK Teams |

Operaciones:
- Los endpoints GET usan snapshots atestados, paginacion determinista y el
  comportamiento de cache documentado en la politica de alias cuando participan
  alias proofs.
- Las mutaciones pasan por permisos ISI/governance; el manejo REST conserva el
  mismo modelo Torii de auth y resource guards que el resto de APIs SoraFS.

## Fixtures y CI

- Directorio de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda snapshots firmados de manifiesto/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso CI: `ci/check_sorafs_fixtures.sh` regenera el snapshot y falla si hay diffs, manteniendo fixtures alineados.
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el happy path mas rechazo de alias duplicado, guards de aprobacion/retencion de alias, mismatched chunker handles, validacion de replica-count y fallas de successor-guard (unknown/pre-approved/retired/self pointers); ver casos `register_manifest_rejects_*` para cobertura.
- Unit tests cubren validacion de alias, guards de retencion, issue/complete de ordenes de replicacion y rechazo de ciclos multi-hop en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
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
- Snapshots REST atestados y metricas del registry forman la superficie local de
  auditoria; archivos firmados de governance pueden consumir esos snapshots via
  el DAG de governance cuando un rollout operador requiere evidencia externa
  durable.

Alertas:
- Ordenes de replicacion pendientes excediendo SLA.
- Expiracion de alias < threshold.
- Violaciones de retencion (manifiesto no renovado antes de expiry).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` rastrea totales de ciclo de vida de manifiestos, cobertura de alias, saturacion de backlog, ratio SLA, overlays de latencia vs slack, y tasas de ordenes fallidas para revision on-call.

## Runbooks y documentacion

- `docs/source/sorafs/migration_ledger.md`, `docs/source/sorafs/migration_roadmap.md` y `roadmap.md` llevan updates de estado del registry.
- Guia de operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` cubre metricas, alertas, despliegue, backup y recovery.
- Flujos de governance y disputas estan documentados en admission policy, alias policy, capacity marketplace y runbooks de dispute/revocation.
- El comportamiento de endpoints esta cubierto por las superficies SoraFS CLI, node-client protocol y OpenAPI.

## Dependencias y secuenciacion

1. Endpoint/client submission polish, validacion compartida, governance config mapping,
   validacion Torii `manifest_b64` y checks de politica del DTO registry estan cableados.
2. Schema Norito, defaults de politica, estado de contrato, facade de servicio,
   telemetria, fixtures y cobertura local de integracion estan implementados.
3. El trabajo SF-4 continuo es evidencia de rollout: auditorias live del registry,
   handoff de archivos de governance y transcripts de cambios de politica de operador.

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
