---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plan de realización Registro de PIN SoraFS
sidebar_label: Registro de PIN del plan
descripción: Plan de realización SF-4, registro de máquina de escribir, modelo Torii, herramientas y datos.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/pin_registry_plan.md`. Deje copias sincronizadas de todos los documentos que se encuentren activos.
:::

# План реализации Registro de PIN SoraFS (SF-4)

SF-4 contrato de registro de pines y servicios de soporte, códigos de registro
manifiesto de configuración, configuración de pin y API previa para Torii,
шлюзов и оркестраторов. Este documento requiere un plan de validación de contratos
задачами реализации, охватывая on-chain логику, сервисы на стороне хоста,
accesorios y operaciones.

## Область1. **Registro de registro**: записи Norito para manifiestos, alias,
   цепочек преемственности, эпох хранения и метаданных управления.
2. **Realisis del contrato**: operaciones CRUD determinadas para el bienestar
   pin de ciclo (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Faso principal**: puntos finales gRPC/REST, operaciones de registro y
   Utilice Torii y SDK, páginas actualizadas y certificados.
4. **Herramientas y accesorios**: ayudantes de CLI, vectores de prueba y documentación para el usuario
   manifiestos de sincronización, alias y sobres de gobernanza.
5. **Metricidad y operaciones**: métricas, alertas y runbooks para el registro de usuarios.

## Модель данных

### Основные записи (Norito)| Estructura | Descripción | Polonia |
|----------|----------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​de Сопоставляет -> Manifiesto CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Las instrucciones para los proveedores eliminan el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendario y CI- Accesorios del catálogo: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные manifiesto/alias/orden de instantáneas, por ejemplo `cargo run -p iroha_core --example gen_pin_snapshot`.
- Muestra CI: `ci/check_sorafs_fixtures.sh` Después de la instantánea y de la diferencia, se sincronizan los accesorios CI.
- Pruebas integrales (`crates/iroha_core/tests/pin_registry.rs`) que permiten acceder a Happy Path además de incluir alias de duplicación, alias de guardias o alias de хранения, identificadores no disponibles chunker, проверку числа реплик и отказы guards преемственности (неизвестные/предодобренные/выведенные/самоссылки); см. кейсы `register_manifest_rejects_*` для деталей покрытия.
- Юнит-тесты теперь покрывают валидацию alias, guards хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности появится, когда заработает машина состояний.
- Golden JSON para dispositivos instalados en archivos de pago.

## Telemetría y наблюдаемость

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая proveedor-telemetría (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) instalado en la conexión de un extremo a otro.

Logotipos:
- Структурированный поток событий Norito для аудиторов управления (¿подписанные?).

Alertas:
- Заказы репликации в ожидании, превышающие SLA.
- Истечение срока alias ниже порога.
- Нарушения хранения (manifiesto не продлен до истечения).Дашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` elimina los totales de los manifiestos del ciclo de vida, alias de descarga, acumulación de pedidos, relación de SLA, latencia de superposiciones frente a holgura y mucho más. пропущенных заказов для de guardia ревью.

## Runbooks y documentación

- Обновить `docs/source/sorafs/migration_ledger.md`, чтобы включить обновления статуса registro.
- Usuario del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (útil) con métricas, alertas, actualizaciones, copias de seguridad y mantenimiento.
- Руководство по управлению: описать параметры политики, flujo de trabajo одобрения, обработку споров.
- API de aplicación para el punto final (documentos Docusaurus).

## Зависимости и последовательность

1. Завершить задачи плана валидации (integración ManifestValidator).
2. Finalice el conjunto Norito + políticas predeterminadas.
3. Realice el contrato + servicio, coloque el televisor.
4. Перегенерировать los accesorios, запустить интеграционные suite.
5. Actualizar documentos/runbooks y eliminar puntos de la hoja de ruta que se pueden guardar.

El punto de control SF-4 debe estar conectado a este plan de progreso de la tecnología.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` activos públicos
  alias de catálogo y trabajo pendiente заказов репликации с консистентной пагинацией и
  estado del filtro.

CLI muestra estos datos (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), los operadores pueden automatizar el registro de auditoría
без обращения к низкоуровневым API.