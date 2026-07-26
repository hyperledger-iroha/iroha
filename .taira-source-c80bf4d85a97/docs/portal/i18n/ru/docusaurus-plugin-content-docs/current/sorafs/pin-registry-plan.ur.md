---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план-регистрации контактов
title: SoraFS Реестр контактов
Sidebar_label: Реестр контактов
описание: SF-4 используется для реестра, конечного автомата, фасада Torii, инструментов и наблюдаемости, а также для обеспечения видимости.
---

:::примечание
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Реестр контактов منصوبہ (SF-4)

Реестр контактов SF-4.
политики выводов Поддержка Torii, шлюзы и оркестраторы Поддержка API-интерфейсов
یہ دستاویز کو ٹھوس задачи по реализации سے بڑھاتی ہے، جس میں on-chain логика,
услуги на стороне хоста, светильники, а также дополнительные услуги

## دائرہ کار

1. **Конечный автомат реестра**: Norito-определенные записи, манифесты, псевдонимы, цепочки преемников,
   периоды хранения, а также метаданные управления.
2. **کنٹریکٹ نفاذ**: жизненный цикл вывода или детерминированные операции CRUD (`ReplicationOrder`,
   `Precommit`, `Completion`, выселение).
3. **Фасад обработки**: конечные точки gRPC/REST в реестре, поддержка Torii и SDK, которые можно использовать.
   جن میں нумерация страниц и аттестация شامل ہے۔
4. **Инструменты и приспособления**: помощники CLI, тестовые векторы, документация, манифесты, псевдонимы и т. д.
   конверты управления ہم آہنگ رہیں۔
5. **Операции телеметрии**: реестр, метрики, оповещения и модули Runbook.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Структура | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | псевдоним → сопоставление CID манифеста. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Провайдеры имеют PIN-код манифеста. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Светильники اور CI- Каталог Fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` — подписанный манифест/псевдоним/снимки заказа.
- Шаг CI: `ci/check_sorafs_fixtures.sh` регенерировать моментальный снимок, чтобы получить разницу и сбой, если выровнены приспособления CI.
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`): счастливый путь: отклонение дубликатов псевдонимов, защита утверждения/сохранения псевдонимов, несовпадающие дескрипторы фрагментов, проверка количества реплик, сбои защиты преемственности (неизвестные/предварительно утвержденные/выведенные из эксплуатации/собственные указатели). ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` случаи دیکھیں۔
- Модульные тесты `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, проверка псевдонимов, защита хранения, проверки преемников и т. д. обнаружение многошаговой последовательности تب آئے گا جب конечный автомат دستیاب ہوگی۔
- Конвейеры наблюдения и золотые события JSON.

## Телеметрия и наблюдаемость

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Телеметрия поставщика услуг (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) сквозные информационные панели широкий диапазон возможностей

Журналы:
- аудит управления структурированный поток событий Norito (подписан?).

Оповещения:
- SLA سے زیادہ ожидающие заказы на репликацию.
- порог истечения срока действия псевдонима سے کم.
- нарушения хранения (явное обновление وقت سے پہلے نہ ہو).

Панели мониторинга:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json`. Общие сведения о жизненном цикле манифеста, покрытие псевдонимов, насыщенность журнала невыполненной работы, соотношение SLA, задержка и нехватка наложений, процент пропущенных заказов и проверка по вызову.

## Runbooks и документация

- `docs/source/sorafs/migration_ledger.md` Обновления статуса реестра.
- Руководство оператора: метрики `docs/source/sorafs/runbooks/pin_registry_ops.md` (по умолчанию), оповещения, развертывание, резервное копирование, потоки восстановления и многое другое.
- Руководство по управлению: параметры политики, рабочий процесс утверждения, разрешение споров.
- Конечная точка содержит справочные страницы API (документы Docusaurus).

## Зависимости и секвенирование

1. Задачи плана проверки (интеграция ManifestValidator).
2. Схема Norito + настройки политики по умолчанию.
3. контракт + услуга Наличие проводной телеметрии کریں۔
4. Фикстуры регенерируют и используют пакеты интеграции.
5. Документы и инструкции по планированию элементов дорожной карты.

Контрольный список для SF-4
Фасад REST и подтвержденный список конечных точек, например:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` или `GET /v1/sorafs/replication` в каталоге псевдонимов.
  журнал заказов репликации, согласованная нумерация страниц, фильтры состояния

CLI вызывает обертку (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) Операторы и API-интерфейсы и аудит реестра.