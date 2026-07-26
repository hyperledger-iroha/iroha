---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план-регистрации контактов
заголовок: План реализации Pin Registry SoraFS
Sidebar_label: Реестр контактов плана
Описание: План реализации SF-4, охватывающая машину состояния реестра, фасад Torii, инструменты и наблюдение.
---

:::note Канонический источник
На этой странице отражено `docs/source/sorafs/pin_registry_plan.md`. Держите копии синхронизированными, пока наследственная документация остается активной.
:::

# План реализации Pin Registry SoraFS (SF-4)

SF-4 предлагает реестр контактов и важные сервисы, которые они хранят.
честно манифест, пин-код политики и предоставление API для Torii,
шлюзов и оркестраторов. Настоящий документ корректирует план валидации случаев
задачами реализации, охватывая сетевую логику, сервисы на стороне хоста,
приспособления и операционные требования.

## Область

1. **Реестр Машины обработки**: записи Norito для манифестов, псевдонимов,
   цепочек преемственности, эпох хранения и управления метаданными.
2. **Реализация контракта**: определенные CRUD-операции для жизнеобеспечения.
   вывод цикла (`ReplicationOrder`, `Precommit`, `Completion`, выселение).
3. **Сервисный фасад**: конечные точки gRPC/REST, опирающиеся на реестр и
   используются Torii и SDK, включая пагинацию и аттестацию.
4. **Инструменты и приспособления**: помощники CLI, тестовые компоненты и документация для
   манифесты синхронизации, псевдонимы и конверты управления.
5. **Телеметрия и операции**: метрики, оповещения и Runbook для реестра здоровья.

##Модель данных

### Основные записи (Norito)

| Структура | Описание | Поля |
|----------|----------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Сопоставляет псевдоним -> Манифест CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Инструкция для провайдеров по установке манифеста. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политического управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Светильники и CI- Каталог светильников: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные снапшоты манифеста/алиаса/заказа, пересоздаваемые через `cargo run -p iroha_core --example gen_pin_snapshot`.
- Шаг CI: `ci/check_sorafs_fixtures.sh` пересоздает снимок и падает при дифференциале, удерживая светильники CI синхронными.
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) раскрывают счастливый путь плюс отказ при дублировании псевдонима, охраняют одобрения/хранения псевдонима, несовпадающие дескрипторы chunker, проверки числа реплик и отказы охраняют преемственности (неизвестные/предодобренные/выведенные/самоссылки); см. кейсы `register_manifest_rejects_*` для деталей покрытия.
- Юнит-тесты теперь раскрывают псевдоним валидации, охраны хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; Обнаружение многошаговой преемственности возникает, когда заработает механизм рассмотрения.
- Золотой JSON для событий, влияние на пайплайны наблюдения.

## Телеметрия и наблюдение

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая провайдер-телеметрия (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) остается в области для сквозных дашбордов.

Логи:
- Структурированный поток событий Norito для аудиторов управления (подписанные?).

Оповещения:
- Заказы репликации в ожидании, превышающие SLA.
- Истечение срока псевдонима ниже порога.
- Нарушения хранения (проявление непродления до истечения).

Дашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` отслеживает итоговые значения циклических манифестов, псевдоним покрытия, насыщенность невыполненной работы, коэффициент SLA, наложение задержки против резерва и долю пропущенных заказов для проверки по вызову.

## Runbook и документация

- Обновить `docs/source/sorafs/migration_ledger.md`, чтобы включить обновление реестра.
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (уже опубликовано) с метриками, алертингом, развертыванием, резервным копированием и восстановлением.
- Руководство по управлению: описание параметров политики, одобрение рабочего процесса, обработка споров.
- Страницы справочника API для каждой конечной точки (документы Docusaurus).

## Зависимости и последовательности

1. Выполнить задачу плана валидации (интеграция ManifestValidator).
2. Финализировать схему Norito + политика по умолчанию.
3. Реализовать договор + сервис, подключить телеметрию.
4. Перегенерировать светильники, запустить интеграционный пакет.
5. Обновите документы/runbooks и отметьте пункты дорожной карты как завершенные.

Каждый пункт чеклиста SF-4 должен ссылаться на этот план при закреплении прогресса.
REST фасад теперь отображается с аттестованными конечными точками в списке:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` и `GET /v1/sorafs/replication` публикуют активный
  псевдоним каталога и невыполненные заказы репликации с консистентной пагинацией и
  фильтрами воздействия.

CLI оборачивает эти вызовы (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы операторы могли автоматизировать реестр аудита
без обращения к низкоуровневому API.