---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
Название: Cumplimiento de direcciones de cuenta
описание: Возобновление работы устройства ADDR-2 и как оборудование SDK синхронизируются.
---

Канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) с захватами IH58 (предпочтительно), сжатый (`sora`, второй лучший; половинная/полная ширина), мультиподпись и негатив. На поверхностном уровне SDK + Torii используется JSON, чтобы обнаружить любое получение кодека перед началом производства. На этой странице отражено краткое описание внутреннего состояния (`docs/source/account_address_status.md` в репозитории Raiz), чтобы лекторы портала консультировались с потоком без автобуса в монорепо.

## Регенерация или проверка пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` - выдает JSON в стандартный вывод для специальной проверки.
- `--out <path>` - опишите другой маршрут (стр. ej., al comparar cambios localmente).
- `--verify` - сравнить копии работы с полученным контентом (невозможно объединить с `--stdout`).

Рабочий процесс CI **Дрейф вектора адреса** выведен `cargo xtask address-vectors --verify`
каждый раз, когда вы устанавливаете приспособление, генератор документов или документы для немедленного оповещения рецензентов.

## Кто потребляет прибор?

| Поверхность | Проверка |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada использует двусторонний обмен каноническими байтами + IH58 + кодировки, полученные и проверяющие, что коды ошибок в стиле Norito совпадают с приспособлением для отрицательных случаев.

## Нужна автоматизация?

Выпускные устройства могут автоматически освещать светильники с помощью помощника.
`scripts/account_fixture_helper.py`, чтобы получить или проверить канонический пакет без копирования/копирования:

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

Помощник принимает переопределение `--source` или переменную `IROHA_ACCOUNT_FIXTURE_URL`, чтобы задания CI SDK были добавлены в ваше зеркало. Когда вы используете `--metrics-out`, помощник по описанию `account_address_fixture_check_status{target="..."}` вместе с дайджестом SHA-256 canonico (`account_address_fixture_remote_info`) для сборщиков текстовых файлов Prometheus и приборной панели Grafana `account_address_fixture_status` может проверить, что это очень похоже на синхронизацию. Оповещение о целевом отчете `0`. Для автоматизации с несколькими поверхностями используйте обертку `ci/account_fixture_metrics.sh` (принимаются повторы `--target label=path[::source]`), чтобы оборудование по вызову было опубликовано в едином архиве `.prom`, объединенном для сборщика текстовых файлов из узла-экспортера.

## Нужна ли краткая информация?

Полное состояние ADDR-2 (владельцы, план мониторинга, дополнительные действия) живет в `docs/source/account_address_status.md` в объединенном репозитории со структурой адресов RFC (`docs/account_structure.md`). США — это страница в качестве быстрой оперативной записи; Для более глубокого понимания проконсультируйтесь с документами репозитория.