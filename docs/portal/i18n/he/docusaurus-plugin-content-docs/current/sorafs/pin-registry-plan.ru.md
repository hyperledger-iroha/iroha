---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
כותרת: План реализации Pin Registry SoraFS
sidebar_label: План Pin Registry
תיאור: План реализации SF-4, охватывающий машину состояний registry, фасад Torii, כלי עבודה ותקשורת.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/pin_registry_plan.md`. Держите обе копии синхронизированными, пока наследственная документация остается активной.
:::

# План реализации Pin Registry SoraFS (SF-4)

SF-4 поставляет контракт Pin Registry и поддерживающие сервисы, которые хранят
מניפסט обязательства, применяют политики pin и предоставляют API ל-Torii,
шлюзов и оркестраторов. Этот документ расширяет план валидации конкретными
задачами реализации, охватывая on-chain логику, сервисы на стороне хоста,
מתקנים и операционные требования.

## Область

1. **רישום Машина состояний**: записи Norito למניפסטים, כינויים,
   цепочек преемственности, эпох хранения и метаданных управления.
2. **Реализация контракта**: детерминированные CRUD-операции для жизненного
   סיכת цикла (`ReplicationOrder`, `Precommit`, `Completion`, פינוי).
3. **Serвисный фасад**: נקודות קצה gRPC/REST, אופציה ברישום и
   используемые Torii и SDK, включая пагинацию и аттестацию.
4. **כלים והתקנים**: CLI עוזרי, тестовые векторы и документация для
   מניפסטים של синхронизации, כינויים ומעטפות ממשל.
5. **Teлеметрия и ops**: метрики, алерты и runbooks для здоровья registry.

## Модель данных

### Основные записи (Norito)

| Структура | Описание | Поля |
|--------|--------|------|
| `PinRecordV1` | Каноническая запись מניפסט. | I18NIS `governance_envelope_hash`. |
| `AliasBindingV1` | כינוי Сопоставляет -> מניפסט CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Инструкция для ספקים закрепить מניפסט. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` לצומת Norito
на Rust и helpers проверки, которые лежат в основе этих записей. Валидация
повторяет כלי מניפסט (רישום חיפוש chunker, שער מדיניות סיכות), чтобы
קונטרקט, חיבורים Torii ו-CLI разделяли идентичные инварианты.

Задачи:
- Завершить схемы Norito в `crates/sorafs_manifest/src/pin_registry.rs`.
- Сгенерировать код (Rust + другие SDK) עם помощью макросов Norito.
- אישור מסמך (`sorafs_architecture_rfc.md`) после внедрения схем.

## Реализация контракта| Задача | Ответственные | Примечания |
|--------|--------------|--------|
| Реализовать хранение הרישום (מזחלת/sqlite/לא שרשרת) או модуль смарт-контракта. | Core Infra / צוות חוזה חכם | Обеспечить детерминированный hashing, избегать נקודה צפה. |
| נקודות כניסה: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | אינפרא ליבה | Использовать `ManifestValidator` из плана валидации. Биндинг alias теперь проходит через `RegisterPinManifest` (DTO Torii), тогда как выделенный для станеленнестанова071X последующих обновлений. |
| Переходы состояния: обеспечивать преемственность (מניפסט A -> B), эпохи хранения, уникальность כינוי. | מועצת ממשל / Infra Core | Уникальность כינוי, лимиты хранения и проверки одобрения/вывода предшественников живут в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности и учет репликации остаются открытыми. |
| Управляемые параметры: загружать `ManifestPolicyV1` из config/состояния управления; разрешить обновления через события управления. | מועצת ממשל | Предоставить CLI для обновления политик. |
| Эмиссия событий: выпускать события Norito לטלמטרים (`ManifestApproved`, `ReplicationOrderIssued`, I100NI700X). | צפייה | Определить схему событий + רישום. |

Тестирование:
- Юнит-тесты для каждой נקודת כניסה (позитивные + отказные сценарии).
- Property-тесты для цепочки преемственности (ללא циклов, монотонные эпохи).
- Fuzz валидации с генерацией случайных מניפסטים (с ограничениями).

## Сервисный фасад (Интеграция Torii/SDK)

| Компонент | Задача | Ответственные |
|--------|--------|----------------|
| Сервис Torii | Экспонировать `/v1/sorafs/pin` (שלח), `/v1/sorafs/pin/{cid}` (חיפוש), `/v1/sorafs/aliases` (רשימה/כריכה), `/v1/sorafs/replication` (הזמנות/קבלות). Обеспечить пагинацию + фильтрацию. | Networking TL / Core Infra |
| Аттестация | Включать высоту/хэш הרישום в ответы; добавить структуру аттестации Norito, потребляемую SDK. | אינפרא ליבה |
| CLI | Расширить `sorafs_manifest_stub` или новый CLI `sorafs_pin` с `pin submit`, `alias bind`, `order issue`, Prometheus, `crates/sorafs_manifest/src/pin_registry.rs`. | Tooling WG |
| SDK | Сгенерировать клиентские כריכות (Rust/Go/TS) из схемы Norito; добавить интеграционные тесты. | צוותי SDK |

אופציות:
- בדוק את המטמון/ETag הקל ל-GET נקודות קצה.
- הגדר הגבלת קצב / אישור בשירותים של Torii.

## מתקנים и CI- אביזרי קטלוג: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные מניפסט/כינוי/סדר, пересоздаваемые через `cargo run -p iroha_core --example gen_pin_snapshot`.
- Шаг CI: `ci/check_sorafs_fixtures.sh` пересоздает תמונת מצב ו падает при diff, удерживая גופי CI синхронными.
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) покрывают happy path плюс отказ при дублировании alias, guards одобрения/храния/храния несовпадающие handles chunker, проверку числа реплик и отказы guards преемственности (неизвестные/предодобреннесдые/выдодобреннсые/высовпадающие); см. кейсы `register_manifest_rejects_*` для деталей покрытия.
- Юнит-тесты теперь покрывают валидацию כינוי, שומרים хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности появится, когда заработает машина состояний.
- Golden JSON עבור событий, используемых в пайплайнах наблюдаемости.

## טלמטריה ותקשורת

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая provider-телеметрия (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) остается в области למכשירים מקצה לקצה.

Логи:
- Структурированный поток событий Norito для аудиторов управления (подписанные?).

אלטרטים:
- Заказы репликации в ожидании, превышающие SLA.
- Истечение срока כינוי ниже порога.
- Нарушения хранения (מניפסט не продлен до истечения).

Дашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` סכומים סכומים של מניפסטים жизненного цикла, כינוי של покрытие, פיגור צבר, יחס חביון של שכבות-על, שכבות-על пропущенных заказов для כוננות ревью.

## ספרי הפעלה ומסמכים

- אופיין ב-`docs/source/sorafs/migration_ledger.md`, צור את הרישום של הסטטוסים.
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (уже опубликовано) с метриками, алертингом, развертыванием, גיבוי ו-восстановлением.
- Руководство по управлению: описать параметры политики, זרימת עבודה одобрения, обработку споров.
- ממשק API עבור נקודת קצה (Docusaurus מסמכים).

## Зависимости и последовательность

1. Завершить задачи плана валидации (интеграция ManifestValidator).
2. Финализировать схему Norito + ברירות מחדל политики.
3. Реализовать контракт + сервис, подключить телеметрию.
4. Перегенерировать גופי, запустить интеграционные סוויטה.
5. אישור מסמכים/רונבוקס ואישור מפת הדרכים как завершенные.

Каждый пункт чеклиста SF-4 должен ссылаться на этот план при фиксации прогресса.
REST фасад теперь поставляется с аттестованными נקודות קצה списка:

- `GET /v1/sorafs/pin` ו-`GET /v1/sorafs/pin/{digest}` מציגים ביטויים с
  כריכות כינוי, הזמנות репликации объектом аттестации, производным от
  хэша последнего блока.
- `GET /v1/sorafs/aliases` ו-`GET /v1/sorafs/replication` публикуют активный
  каталог alias и backlog заказов репликации с консистентной пагинацией и
  фильтрами статуса.

CLI оборачивает эти вызовы (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы операторы могли автоматизировать аудиты registry
без обращения к низкоуровневым API.