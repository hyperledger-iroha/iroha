---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: SoraFS Registro de PIN نفاذی منصوبہ
sidebar_label: Registro de PIN منصوبہ
descripción: SF-4 نفاذی منصوبہ جو registro کی máquina de estado, Torii fachada, herramientas اور observabilidad کو کور کرتا ہے۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Registro de PIN نفاذی منصوبہ (SF-4)

Registro de pines SF-4
políticas de pin نافذ کرتی ہیں، اور Torii, gateways اور Orchestrators کے لیے API ظاہر کرتی ہیں۔
یہ دستاویز plan de validación کو ٹھوس tareas de implementación سے بڑھاتی ہے، جس میں lógica en cadena،
servicios del lado del host, accesorios, اور عملیاتی تقاضے شامل ہیں۔

## دائرہ کار1. **máquina de estado del registro**: registros definidos por Norito, manifiestos, alias, cadenas sucesoras,
   épocas de retención, y metadatos de gobernanza.
2. **کنٹریکٹ نفاذ**: ciclo de vida del pin کے لیے operaciones CRUD deterministas (`ReplicationOrder`,
   `Precommit`, `Completion`, desalojo).
3. **Fachada**: puntos finales gRPC/REST, registro, software de instalación de Torii y SDKs de software.
   جن میں paginación اور atestación شامل ہے۔
4. **herramientas y accesorios**: ayudantes de CLI, vectores de prueba, documentación y manifiestos, alias y
   sobres de gobernanza ہم آہنگ رہیں۔
5. **operaciones y operaciones de telemetría**: registros, métricas, alertas y runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)| Estructura | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | alias -> mapeo CID manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | proveedores کو pin de manifiesto کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | reconocimiento del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Panorama de la política de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendario de CI- Directorio de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں manifiesto firmado/alias/instantáneas de orden محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerar ہوتے ہیں۔
- Paso CI: regeneración de instantánea `ci/check_sorafs_fixtures.sh` کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ Accesorios CI alineados رہیں۔
- Pruebas de integración (`crates/iroha_core/tests/pin_registry.rs`) camino feliz کے ساتھ rechazo de alias duplicados, protección de aprobación/retención de alias, identificadores de fragmentación no coincidentes, validación de recuento de réplicas, fallas de protección de sucesión (desconocido/preaprobado/retirado/autopunteros) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` casos دیکھیں۔
- Pruebas unitarias اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں validación de alias, guardias de retención, اور comprobaciones de sucesor کور کرتے ہیں؛ detección de sucesión de saltos múltiples تب آئے گا جب máquina de estado دستیاب ہوگی۔
- Canalizaciones de observabilidad کے لیے eventos JSON dorados ۔

## Telemetría y observabilidad

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Telemetría del proveedor de موجودہ (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) paneles de control de extremo a extremo کے لیے alcance میں رہے گی۔

Registros:
- auditorías de gobernanza کے لیے flujo de eventos Norito estructurado (¿firmado?).

Alertas:
- SLA سے زیادہ órdenes de replicación pendientes.
- umbral de caducidad del alias سے کم.
- violaciones de retención (renovación manifiesta وقت سے پہلے نہ ہو).Paneles de control:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` Totales del ciclo de vida del manifiesto, cobertura de alias, saturación de trabajos pendientes, relación SLA, latencia frente a superposiciones de holgura, tasas de pedidos perdidos y revisión de guardia.

## Runbooks y documentación

- `docs/source/sorafs/migration_ledger.md` کو actualizaciones de estado del registro شامل کرنے کے لیے اپڈیٹ کریں۔
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (اب شائع شدہ) métricas, alertas, implementación, copia de seguridad, flujos de recuperación کور کرتا ہے۔
- Guía de gobernanza: parámetros de políticas, flujo de trabajo de aprobación, manejo de disputas.
- ہر endpoint کے لیے Páginas de referencia de API (documentos Docusaurus).

## Dependencias y Secuenciación

1. Tareas del plan de validación مکمل کریں (integración de ManifestValidator).
2. Esquema Norito + valores predeterminados de política کو حتمی بنائیں۔
3. contrato + servicio نافذ کریں اور cable de telemetría کریں۔
4. Los dispositivos se regeneran کریں اور suites de integración چلائیں۔
5. documentos/runbooks اپڈیٹ کریں اور elementos de la hoja de ruta کو مکمل مارک کریں۔

SF-4 کے ہر lista de verificación آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
Fachada REST اب listado de puntos finales certificado کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` فعال alias catálogo اور
  acumulación de pedidos de replicación کو paginación consistente اور filtros de estado کے ساتھ ظاہر کرتے ہیں۔CLI y llamadas کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operadores کم سطحی API کو چھوئے بغیر auditorías de registro خودکار بنا سکیں۔