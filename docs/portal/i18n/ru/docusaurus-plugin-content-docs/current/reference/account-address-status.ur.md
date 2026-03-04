---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
Название: اکاؤنٹ ایڈریس تعمیل
описание: Прибор ADDR-2 может быть установлен в SDK или установлен на устройстве.
---

канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) IH58 (предпочтительный), сжатый (`sora`, второй лучший; половинная/полная ширина), мультисигнатура, отрицательные фиксаторы и возможность захвата ہر SDK + Torii поверхность JSON и кодек, дрейф кодека и др. پکڑا جا سکے۔ یہ صفحہ اندرونی краткое описание статуса (`docs/source/account_address_status.md` ریپوزٹری روٹ میں) کو зеркало کرتا ہے تاکہ читателей портала بغیر Использование монорепозитория для рабочего процесса

## Bundle کو восстановить یا проверить کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` — специальная проверка JSON со стандартным выводом и испускаемым потоком.
- `--out <path>` — مختلف path پر لکھتا ہے (مثلا لوکل diff کے وقت)۔
- `--verify` — рабочая копия, позволяющая генерировать контент и сравнивать данные (`--stdout` позволяет объединить все файлы). سکتا)۔

Рабочий процесс CI **Дрейф вектора адреса** `cargo xtask address-vectors --verify`
Используйте инструменты, генератор, документы и рецензенты, а также оповещения о них.

## Крепеж для установки

| Поверхность | Проверка |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Использование канонических байтов + IH58 + сжатых (`sora`, второй лучший) кодировок. Возможность двустороннего обхода. Возможность устранения отрицательных случаев. Коды ошибок в стиле Norito. کرتا ہے۔

## Автоматизация

Обновление инструмента выпуска для `scripts/account_fixture_helper.py` или скрипта для копирования и вставки, а также для извлечения канонического пакета и проверки:

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

Helper `--source` переопределяет переменную среды `IROHA_ACCOUNT_FIXTURE_URL`. Задание в SDK CI. Задание зеркала. کر سکیں۔ `--metrics-out` и помощник `account_address_fixture_check_status{target="…"}` могут быть использованы для канонического дайджеста SHA-256 (`account_address_fixture_remote_info`). Prometheus коллекторы текстовых файлов Grafana приборная панель `account_address_fixture_status` Поверхность для синхронизации и синхронизации. Выберите target `0` и установите оповещение. многоповерхностная автоматизация оболочка `ci/account_fixture_metrics.sh` استعمال کریں (повторяемый `--target label=path[::source]` قبول کرتا ہے) Сборщик текстовых файлов команд по вызову Консолидированный `.prom` для публикации.

## Краткое описание

Статус соответствия ADDR-2 (владельцы, план мониторинга, открытые действия)
Проверьте структуру адреса RFC (`docs/account_structure.md`) для `docs/source/account_address_status.md`. Быстрое напоминание о работе Доступ к репозиторию документов.