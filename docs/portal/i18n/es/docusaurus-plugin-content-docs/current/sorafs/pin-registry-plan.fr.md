---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plan de implementación del Registro de PIN de SoraFS
sidebar_label: Registro Plan du Pin
descripción: Plan de implementación SF-4 que cubre la máquina de estados del registro, la fachada Torii, las herramientas y la observabilidad.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/pin_registry_plan.md`. Guarde las dos copias sincronizadas mientras la documentación heredada permanece activa.
:::

# Plan de implementación del Registro de PIN de SoraFS (SF-4)

SF-4 libre de contrato Pin Registry y servicios de aplicación que almacenan les
compromisos de manifiesto, aplicar las políticas de fijación y exponer los
API a Torii, puertas de enlace auxiliares y orquestadores auxiliares. Este documento étend le plan de
validación con los pasos de implementación concreta, couvrant la lógica
on-chain, los servicios côté hôte, los accesorios y las exigencias operativas.

## Portée1. **Máquina de estados de registro**: registros Norito para manifiestos, alias,
   cadenas de sucesión, épocas de retención y metadonnées de gobierno.
2. **Implementación del contrato**: operaciones CRUD determinadas para el ciclo de vida
   des pines (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Fachada de servicio**: puntos finales gRPC/REST añadidos al registro consommés por Torii
   y los SDK, con paginación y atestación.
4. **Herramientas y accesorios**: CLI auxiliares, vectores de prueba y documentación para
   guardar manifiestos, alias y sobres de gobierno sincronizados.
5. **Télémétrie et ops**: métricas, alertas y runbooks para la salud del registro.

## Modelo de données

### Registros principales (Norito)| Estructura | Descripción | Campeones |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​​​del mapa -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucciones para que los proveedores pinent el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusado de recepción del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de la política de gobierno. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Accesorios y CI- Dossier de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stocke des snapshots signés de manifest/alias/order régénérés via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa CI: `ci/check_sorafs_fixtures.sh` regénère le snapshot et echoue en cas de diff, manteniendo los accesorios CI alineados.
- Pruebas de integración (`crates/iroha_core/tests/pin_registry.rs`) que cubren el camino feliz más el rechazo de alias dupliqué, los guardias de aprobación/rétención, las manijas de fragmentación no concordantes, la validación del recuento de réplicas y las pruebas de guardia de sucesión (pointeurs inconnus/pré-approuvés/retirés/auto-référencés); Consulte el caso `register_manifest_rejects_*` para los detalles de la cobertura.
- Las pruebas unitarias deben mantener la validación de alias, los guardias de retención y las comprobaciones de éxito en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la detección de sucesión de múltiples saltos asiste a la máquina de estados.
- JSON golden para los eventos utilizados por las tuberías de observación.

## Telemetría y observabilidad

Métricas (Prometheus) :
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- El proveedor de televisión existente (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece en el alcance de los paneles de control de un extremo a otro.

Registros:
- Flux d'événements Norito structures pour les audits de gouvernance (signés ?).Alertas:
- Órdenes de réplica en atención al paso del SLA.
- Expiration d'alias < seuil.
- Violaciones de retención (manifiesto non renouvelé avant expiración).

Paneles de control:
- El JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` se adapta a todos los ciclos de vida de los manifiestos, la cobertura de alias, la saturación del trabajo pendiente, la relación SLA, las superposiciones de latencia frente a slack y los numerosos pedidos pendientes para la revista de guardia.

## Runbooks y documentación

- Mettre à jour de `docs/source/sorafs/migration_ledger.md` para incluir las actualizaciones del estado del registro.
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) couvrant métriques, alerting, déploiement, sauvegarde et flux de reprise.
- Guía de gobierno: describa los parámetros políticos, el flujo de trabajo de aprobación y la gestión de litigios.
- Páginas de referencia API para cada punto final (docs Docusaurus).

## Dependencias y secuenciación

1. Finalizar las tareas del plan de validación (integración ManifestValidator).
2. Finalizar el esquema Norito + defaults de politique.
3. Implementar el contrato + servicio, ramificar la télémétrie.
4. Regénérer les accesorios, ejecuter les suites d'intégration.
5. Mettre à jour docs/runbooks y marque los elementos de la hoja de ruta como completos.

Cada elemento de la lista de verificación SF-4 debe consultar este plan cuando el progreso esté registrado.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` catálogo de archivos expuestos
  d'alias actif et le backlog des ordres de replication avec una paginación coherente
  et des filtres de statut.

La CLI encapsula estas aplicaciones (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para permitir a los operadores de automatización las auditorías del
registro sin contacto con API auxiliares bajo nivel.