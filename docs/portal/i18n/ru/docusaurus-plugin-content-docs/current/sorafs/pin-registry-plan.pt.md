---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план-регистрации контактов
заголовок: План внедрения реестра контактов для SoraFS
Sidebar_label: Реестр Plano do Pin
Описание: План внедрения SF-4, объединенный с машиной регистрации состояния, фасадом Torii, инструментами и возможностью наблюдения.
---

:::примечание Fonte canonica
Эта страница отражает `docs/source/sorafs/pin_registry_plan.md`. Мантенья представил как копии синхронизированных документов, которые являются постоянными.
:::

# План внедрения реестра контактов для SoraFS (SF-4)

O SF-4 вводит договоры с реестром контактов и службами помощи, которые вооружены
компромиссы манифеста, включение политик в пин-код и демонстрация API-интерфейсов для Torii,
шлюзы и оркестраноры. Этот расширенный документ или план проверки подлинности с помощью
конкретные тарифы реализации, объединение логики в цепочке и сервисные услуги
хозяин, оборудование и все необходимые условия.

## Эскопо

1. **Maquina de estados do Registry**: реестры, определенные для Norito для манифестов,
   псевдонимы, наследники, эпохи удержания и метададо управления.
2. **Реализация контракта**: детерминированные операции CRUD для циклической жизни
   дос-пины (`ReplicationOrder`, `Precommit`, `Completion`, выселение).
3. **Доступ к обслуживанию**: конечные точки gRPC/REST поддерживаются в реестре Torii.
   Воспользуйтесь всеми SDK, включая разбивку по страницам и подтверждение.
4. **Инструменты и приспособления**: помощники по CLI, инструменты для тестирования и документирования для других задач.
   манифесты, псевдонимы и конверты синхронизированного управления.
5. **Телеметрия и операции**: метрики, оповещения и Runbook для каждого реестра.

## Дадо-модель

### Центральный реестр (Norito)

| Структура | Описание | Кампос |
|--------|-----------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Псевдоним Mapeia -> CID манифеста. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Инструкции по исправлению или манифесту поставщиков. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтвердите действия провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики правительства. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Светильники и CI- Директория приспособлений: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` — снапшоты файлов манифеста/псевдонима/заказа, восстановленные для `cargo run -p iroha_core --example gen_pin_snapshot`.
- Этап CI: `ci/check_sorafs_fixtures.sh` восстанавливает моментальный снимок и удаляет различия, сохраняя все приборы CI.
- Интегрированные тесты (`crates/iroha_core/tests/pin_registry.rs`) для выполнения упражнений или ошибок, связанных с проверкой дубликатов псевдонимов, защитой от подтверждения/сохранения псевдонимов, несовместимостью обрабатывающих блоков, проверкой заражения реплик и проверкой успешности (понтейросов) desconhecidos/preaprovados/retirados/autorreferencias); есть случай `register_manifest_rejects_*` для подробностей о кобертуре.
- Унитарные тесты, которые позволяют проверить псевдоним, защитить сохранение и проверить преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение успешного многопрыжкового режима, когда машина проходит этапы.
- Золотой JSON для событий, используемых в конвейерах наблюдения.

## Телеметрия и наблюдение

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Телеметрия существующих поставщиков (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) постоянно доступна для сквозных информационных панелей.

Журналы:
- Поток событий Norito создан для аудиторий управления (убит?).

Оповещения:
- Заказы на репликацию, превышающие SLA.
- Срок действия псевдонима истекает.
- Виолакоэс-де-ретенсао (проявляется до истечения срока обновления).

Панели мониторинга:
- O JSON для Grafana `docs/source/grafana_sorafs_pin_registry.json` обеспечивает полное распределение циклов просмотра манифестов, выбор псевдонимов, насыщение невыполненной работы, определение SLA, наложение задержки на резерв и сборы за отложенные заказы для проверки по вызову.

## Runbook и документация

- Установите `docs/source/sorafs/migration_ledger.md` для автоматического включения статуса реестра.
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (опубликовано) для объединения метрик, предупреждений, развертывания, резервного копирования и восстановления данных.
- Руководство по управлению: определение политических параметров, рабочий процесс утверждения, обработка споров.
- Страницы ссылок API для конечной точки (документы Docusaurus).

## Зависимости и последовательность

1. Завершите выполнение плана проверки (интеграция ManifestValidator).
2. Завершить настройку Norito + настройки по умолчанию.
3. Внедрить контракт + обслуживание, подключить телеметрию.
4. Регенерируйте светильники, интегрированные радары.
5. Настройте документацию/бухгалтерию и отметьте дорожную карту как полную.

Контрольный список для SF-4 должен быть ссылаться на этот план, когда он будет продвигаться вперед.
Перед входом в REST конечные точки списка с подтверждением:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` и `GET /v1/sorafs/replication` экспозиция или каталог
  псевдоним ativo и отставание в заказах репликации с последовательностью страниц и
  фильтры статуса.Инкапсула CLI essas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) для того, чтобы операторы могли автоматизировать аудитории
реестра, API-интерфейсы sem tocar de baixo nivel.