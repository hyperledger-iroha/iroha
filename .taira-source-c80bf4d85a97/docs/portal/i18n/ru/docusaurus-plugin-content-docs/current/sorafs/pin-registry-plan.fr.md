---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план-регистрации контактов
заголовок: План реализации реестра контактов SoraFS
Sidebar_label: Реестр Plan du Pin
описание: План реализации SF-4, связанный с машиной регистрации, фасадом Torii, инструментами и наблюдаемостью.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/pin_registry_plan.md`. Убедитесь, что две копии синхронизированы с оставшейся активной документацией.
:::

# План внедрения реестра контактов SoraFS (SF-4)

SF-4 прочитайте реестр контактов и сервисы приложений, которые есть в наличии
Обязательства манифестные, прикладные политические действия закрепления и разоблачения
API Torii, дополнительные шлюзы и дополнительные оркестраторы. Этот документ представляет собой план де
валидация с конкретными этапами реализации, соответствие логике
ончейн, услуги на дому, приспособления и операционные нужды.

## Порте

1. **Машина реестра**: регистрации Norito для манифестов, псевдонимов,
   цепи преемственности, эпохи удержания и метадонники управления.
2. **Реализация контракта**: операции CRUD детерминированы для жизненного цикла
   булавки (`ReplicationOrder`, `Precommit`, `Completion`, выселение).
3. **Фасад службы**: конечные точки gRPC/REST привязаны к реестру, соответствующему Torii.
   и SDK, с нумерацией страниц и аттестацией.
4. **Инструменты и приспособления**: вспомогательный интерфейс командной строки, векторы тестирования и документация для
   храните манифесты, псевдонимы и конверты синхронного управления.
5. **Télémétrie et ops**: метрики, оповещения и Runbook для обеспечения безопасности реестра.

## Модель Донне

### Принципы регистрации (Norito)

| Структура | Описание | Чемпионы |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Псевдоним Mappe -> CID манифеста. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Инструкция по заполнению манифеста поставщиками. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Обвиняемый на стойке регистрации поставщика услуг. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
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

## Светильники и CI- Досье приборов: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` запас снимков, подписанных манифестом/псевдонимом/порядком, измененным через `cargo run -p iroha_core --example gen_pin_snapshot`.
- Этап CI: `ci/check_sorafs_fixtures.sh` настраивает моментальный снимок и отображает его в случае разницы, следя за выравниванием приборов CI.
- Тесты интеграции (`crates/iroha_core/tests/pin_registry.rs`) включают счастливый путь плюс отказ от дублирования псевдонимов, защиту от одобрения/сохранения, несогласованные дескрипторы фрагментов, проверку достоверности копий и проверки преемственности (указатели) inconnus/pre-approuvés/retires/auto-reférencés); voir les cas `register_manifest_rejects_*` для деталей кувертюры.
- Единые тесты поддерживают проверку псевдонима, защиту удержания и проверку успеха в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; обнаружение последовательности многопрыжковых операций на машине d'états.
- Золотой JSON для используемых в конвейерах наблюдения событий.

## Телеметрия и наблюдаемость

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующий поставщик телеметрии (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) остается в рамках сквозных панелей мониторинга.

Журналы:
- Flux d'événements Norito структур для аудита управления (signés ?).

Оповещения:
- Порядок репликации при удалении SLA.
- Срок действия псевдонима <seuil.
- Нарушения удержания (выраженное отсутствие возобновления до истечения срока действия).

Панели мониторинга:
- JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` подходит для всего цикла соперничества манифестов, псевдонимов, насыщенности невыполненной работы, соотношения SLA, наложений задержки и резерва и других заказов для ревю по вызову.

## Runbook и документация

- Mettre à jour `docs/source/sorafs/migration_ledger.md` для включения несчастных случаев в дневной закон реестра.
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (опубликовано) по метрикам, оповещениям, развертыванию, сохранению и потоку повторов.
- Руководство по управлению: определение политических параметров, рабочий процесс одобрения, судебные решения.
— API справочных страниц для конечной точки проверки (документы Docusaurus).

## Зависимости и последовательность

1. Завершите действия плана проверки (интеграция ManifestValidator).
2. Финализатор схемы Norito + политические настройки по умолчанию.
3. Реализация контракта + услуга, ответвление на телеметрию.
4. Подготовьте светильники и выполните интеграцию.
5. Просматривайте документы и рабочие книги и отмечайте элементы дорожной карты как завершенные.

Каждый пункт контрольного списка SF-4 должен быть ссылочным по плану прогресса, зарегистрированного.
Фасад REST ухудшенной информации о конечных точках листинга:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` и `GET /v1/sorafs/replication` открытый каталог файлов
  псевдоним действия и резервная копия заказов репликации с последовательной нумерацией страниц
  и законные фильтры.CLI инкапсулит эти приложения (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы позволить операторам автоматизации аудита
реестр без дополнительных API-интерфейсов на нижнем уровне.