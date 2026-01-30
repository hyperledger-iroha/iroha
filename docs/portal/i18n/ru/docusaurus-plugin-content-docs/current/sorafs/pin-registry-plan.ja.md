---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d35393ee4e86c80446bf0067247b677236c8dc549ab864dfc140ed051e075ddd
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: pin-registry-plan
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/pin_registry_plan.md`. Держите обе копии синхронизированными, пока наследственная документация остается активной.
:::

# План реализации Pin Registry SoraFS (SF-4)

SF-4 поставляет контракт Pin Registry и поддерживающие сервисы, которые хранят
обязательства manifest, применяют политики pin и предоставляют API для Torii,
шлюзов и оркестраторов. Этот документ расширяет план валидации конкретными
задачами реализации, охватывая on-chain логику, сервисы на стороне хоста,
fixtures и операционные требования.

## Область

1. **Машина состояний registry**: записи Norito для manifests, aliases,
   цепочек преемственности, эпох хранения и метаданных управления.
2. **Реализация контракта**: детерминированные CRUD-операции для жизненного
   цикла pin (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Сервисный фасад**: gRPC/REST endpoints, опирающиеся на registry и
   используемые Torii и SDK, включая пагинацию и аттестацию.
4. **Tooling и fixtures**: CLI helpers, тестовые векторы и документация для
   синхронизации manifests, aliases и governance envelopes.
5. **Телеметрия и ops**: метрики, алерты и runbooks для здоровья registry.

## Модель данных

### Основные записи (Norito)

| Структура | Описание | Поля |
|----------|----------|------|
| `PinRecordV1` | Каноническая запись manifest. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Сопоставляет alias -> CID manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Инструкция для providers закрепить manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` для схем Norito
на Rust и helpers проверки, которые лежат в основе этих записей. Валидация
повторяет manifest tooling (lookup chunker registry, pin policy gating), чтобы
контракт, фасады Torii и CLI разделяли идентичные инварианты.

Задачи:
- Завершить схемы Norito в `crates/sorafs_manifest/src/pin_registry.rs`.
- Сгенерировать код (Rust + другие SDK) с помощью макросов Norito.
- Обновить документацию (`sorafs_architecture_rfc.md`) после внедрения схем.

## Реализация контракта

| Задача | Ответственные | Примечания |
|--------|---------------|-----------|
| Реализовать хранение registry (sled/sqlite/off-chain) или модуль смарт-контракта. | Core Infra / Smart Contract Team | Обеспечить детерминированный hashing, избегать floating point. |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | Использовать `ManifestValidator` из плана валидации. Биндинг alias теперь проходит через `RegisterPinManifest` (DTO Torii), тогда как выделенный `bind_alias` остается в плане для последующих обновлений. |
| Переходы состояния: обеспечивать преемственность (manifest A -> B), эпохи хранения, уникальность alias. | Governance Council / Core Infra | Уникальность alias, лимиты хранения и проверки одобрения/вывода предшественников живут в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности и учет репликации остаются открытыми. |
| Управляемые параметры: загружать `ManifestPolicyV1` из config/состояния управления; разрешить обновления через события управления. | Governance Council | Предоставить CLI для обновления политик. |
| Эмиссия событий: выпускать события Norito для телеметрии (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Определить схему событий + logging. |

Тестирование:
- Юнит-тесты для каждой entry point (позитивные + отказные сценарии).
- Property-тесты для цепочки преемственности (без циклов, монотонные эпохи).
- Fuzz валидации с генерацией случайных manifests (с ограничениями).

## Сервисный фасад (Интеграция Torii/SDK)

| Компонент | Задача | Ответственные |
|-----------|--------|---------------|
| Сервис Torii | Экспонировать `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Обеспечить пагинацию + фильтрацию. | Networking TL / Core Infra |
| Аттестация | Включать высоту/хэш registry в ответы; добавить структуру аттестации Norito, потребляемую SDK. | Core Infra |
| CLI | Расширить `sorafs_manifest_stub` или новый CLI `sorafs_pin` с `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Сгенерировать клиентские bindings (Rust/Go/TS) из схемы Norito; добавить интеграционные тесты. | SDK Teams |

Операции:
- Добавить слой cache/ETag для GET endpoints.
- Предоставить rate limiting / auth в соответствии с политиками Torii.

## Fixtures и CI

- Каталог fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные snapshots manifest/alias/order, пересоздаваемые через `cargo run -p iroha_core --example gen_pin_snapshot`.
- Шаг CI: `ci/check_sorafs_fixtures.sh` пересоздает snapshot и падает при diff, удерживая fixtures CI синхронными.
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) покрывают happy path плюс отказ при дублировании alias, guards одобрения/хранения alias, несовпадающие handles chunker, проверку числа реплик и отказы guards преемственности (неизвестные/предодобренные/выведенные/самоссылки); см. кейсы `register_manifest_rejects_*` для деталей покрытия.
- Юнит-тесты теперь покрывают валидацию alias, guards хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности появится, когда заработает машина состояний.
- Golden JSON для событий, используемых в пайплайнах наблюдаемости.

## Телеметрия и наблюдаемость

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая provider-телеметрия (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) остается в области для end-to-end дашбордов.

Логи:
- Структурированный поток событий Norito для аудиторов управления (подписанные?).

Алерты:
- Заказы репликации в ожидании, превышающие SLA.
- Истечение срока alias ниже порога.
- Нарушения хранения (manifest не продлен до истечения).

Дашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` отслеживает totals жизненного цикла manifests, покрытие alias, насыщение backlog, SLA ratio, overlays latency vs slack и долю пропущенных заказов для on-call ревью.

## Runbooks и документация

- Обновить `docs/source/sorafs/migration_ledger.md`, чтобы включить обновления статуса registry.
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (уже опубликовано) с метриками, алертингом, развертыванием, backup и восстановлением.
- Руководство по управлению: описать параметры политики, workflow одобрения, обработку споров.
- Страницы справочника API для каждого endpoint (Docusaurus docs).

## Зависимости и последовательность

1. Завершить задачи плана валидации (интеграция ManifestValidator).
2. Финализировать схему Norito + defaults политики.
3. Реализовать контракт + сервис, подключить телеметрию.
4. Перегенерировать fixtures, запустить интеграционные suite.
5. Обновить docs/runbooks и отметить пункты roadmap как завершенные.

Каждый пункт чеклиста SF-4 должен ссылаться на этот план при фиксации прогресса.
REST фасад теперь поставляется с аттестованными endpoints списка:

- `GET /v1/sorafs/pin` и `GET /v1/sorafs/pin/{digest}` возвращают manifests с
  alias bindings, orders репликации и объектом аттестации, производным от
  хэша последнего блока.
- `GET /v1/sorafs/aliases` и `GET /v1/sorafs/replication` публикуют активный
  каталог alias и backlog заказов репликации с консистентной пагинацией и
  фильтрами статуса.

CLI оборачивает эти вызовы (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы операторы могли автоматизировать аудиты registry
без обращения к низкоуровневым API.
