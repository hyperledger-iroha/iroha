---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
заголовок: Соответствие адресов аккаунтов
описание: Сводка рабочего процесса ADDR-2 и синхронизация команды SDK.
---

Канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) включает фикстуры I105 (предпочтительный), сжатый (`sora`, второй лучший; половинная/полная ширина), мультисигнатурный и негативный. Вся поверхность SDK + Torii основана на одном и том же JSON, чтобы найти выходной кодек дрейфа в продукте. На этой странице указан внутренний статусный бриф (`docs/source/account_address_status.md` в корневом репозитории), чтобы читатели портала могли обратиться к рабочему процессу без необходимости копироваться в монорепозиторий.

## Пегенерация или проверка пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` — выводит JSON в стандартный вывод для специальных проверок.
- `--out <path>` — записывает в другой путь (например, при локальном изменении изменений).
- `--verify` — содержит адаптер со свежесгенерированным содержимым (нельзя смешать с `--stdout`).

Рабочий процесс CI **Дрейф вектора адреса** запускается `cargo xtask address-vectors --verify`
Каждый раз, когда меняется прибор, генератор или документация, чтобы немедленно связаться с ревьюерами.

## Кто использует приспособление?

| Поверхность | Проверка |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Каждый жгут выполняет обратный ход канонических байтов + I105 + сжатых кодов, и, наконец, коды ошибок в стиле Norito совпадают с приспособлением для отрицательных кейсов.

## Нужна автоматизация?

Инструменты выпуска могут автоматизировать обновление приспособлений через помощника
`scripts/account_fixture_helper.py`, который получает или впоследствии канонический пакет без копирования/вставки:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Helper выполняет переопределения через `--source` или переменное окружение `IROHA_ACCOUNT_FIXTURE_URL`, чтобы CI-джобы SDK помогали отслеживать зеркало. При передаче помощник `--metrics-out` записывает `account_address_fixture_check_status{target="…"}` вместе с каноническим дайджестом SHA-256 (`account_address_fixture_remote_info`), чтобы сборщики текстовых файлов Prometheus и дашборд Grafana `account_address_fixture_status` могли организовать синхронизацию каждой поверхности. Будьте внимательны, когда цель сообщает `0`. Для автоматизации нескольких поверхностей воспользуйтесь оболочкой `ci/account_fixture_metrics.sh` (принимает повторяющиеся `--target label=path[::source]`), чтобы по вызову команды опубликовали единый файл `.prom` для узла-экспортера сборщика текстовых файлов.

## Нужен полный бриф?

Полный статус соответствия ADDR-2 (владельцы, план Диптихи, открытые задачи)
хранится в `docs/source/account_address_status.md` внутри репозитория вместе со структурой адреса RFC (`docs/account_structure.md`). Используйте эту страницу как оперативное напоминание; для более глубокой помощи в репозитории документов.