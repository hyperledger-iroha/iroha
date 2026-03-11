---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
название: Conformite des adresses de comte
описание: Возобновление рабочего процесса устройства ADDR-2 и синхронизации оборудования SDK.
---

Канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) захватывает устройства I105 (предпочтительно), сжатый (`sora`, второй лучший; половинная/полная ширина), мультиподпись и отрицательные значения. Chaque Surface SDK + Torii позволяет получить JSON-память для получения всех кодеков перед производством. На этой странице отражена краткая информация о внутреннем законодательстве (`docs/source/account_address_status.md` в депо racine) для того, чтобы лекторы du portail могли обратиться к рабочему процессу без фуллерена в монорепо.

## Регенератор или верификатор пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` — отображает JSON и стандартный вывод для специальной проверки.
- `--out <path>` - ecrit vers un autre chemin (например, для локального сравнения).
- `--verify` - сравните копию труда с общим содержанием (невозможно объединить с `--stdout`).

Рабочий процесс CI **Дрейф вектора адреса** выполнить `cargo xtask address-vectors --verify`
Каждый раз, когда прибор, генератор или документы меняются, немедленно оповещают рецензентов.

## Что можно сделать с прибором?

| Поверхность | Проверка |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Используйте функцию восстановления всех канонических октетов + I105 + кодируйте сжатие и проверяйте, какие коды ошибок стиля Norito соответствуют установочному приспособлению для отрицательных значений.

## Что такое автоматизация?

Автоматизированные устройства для разблокировки светильников с помощником
`scripts/account_fixture_helper.py`, который восстановит или проверит канонический пакет без копировального аппарата/коллера:

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

Помощник принимает переопределения `--source` или переменную среды `IROHA_ACCOUNT_FIXTURE_URL` для заданий CI из SDK, указывающих на предпочитаемый вами мир. Lorsque `--metrics-out` - это четыре, помощник `account_address_fixture_check_status{target="..."}`, который содержит канонический дайджест SHA-256 (`account_address_fixture_remote_info`) для сборщиков текстовых файлов Prometheus и приборной панели Grafana `account_address_fixture_status` может подтвердить, что поверхность остается синхронизированной. Alertez des qu'une cible rapporte `0`. Для автоматизации нескольких поверхностей используйте оболочку `ci/account_fixture_metrics.sh` (примите повторы `--target label=path[::source]`), чтобы опубликовать оборудование `.prom` для сборщика текстовых файлов из узла-экспортера.

## Besoin du Brief Complete ?

Le statut Complete de Conformite ADDR-2 (владельцы, план мониторинга, последующие действия)
Если вы нашли `docs/source/account_address_status.md` на складе, то же самое, что структура адреса RFC (`docs/account_structure.md`). Используйте эту страницу для быстрого спуска по канату; Ссылка на документы репозитория для получения подходящего руководства.