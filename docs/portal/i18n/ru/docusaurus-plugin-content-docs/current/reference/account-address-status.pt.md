---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
Название: Conformidade de enderecos de conta
описание: Резюме для Fluxo для устройства ADDR-2 и в качестве синхронизируемого оборудования SDK.
---

O pacote canonico ADDR-2 (`fixtures/account/address_vectors.json`) с фиксаторами I105 (предпочтительно), сжатый (`sora`, второй лучший; половина/полная ширина), мультиподпись и негатив. На поверхностном уровне SDK + Torii используйте или запишите JSON, чтобы обнаружить смещение кодека перед началом производства. На этой странице указан краткий внутренний статус (`docs/source/account_address_status.md` без репозитория), чтобы пользователи могли консультироваться с порталом или потоком семенных сосудов или монорепозиторием.

## Регенерация или проверка пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` — выводить или JSON в стандартный вывод для специальной проверки.
- `--out <path>` - Grava Em um Caminho Diferente (например: ao Comparar Mudancas Localmente).
- `--verify` - сравните копию выполненной работы с полученным сообщением (невозможно использовать комбинацию с `--stdout`).

Рабочий процесс CI **Дрейф вектора адреса** типа `cargo xtask address-vectors --verify`
Всегда, когда это необходимо, или документы, или документы должны быть немедленно оповещены рецензентами.

## Что использовать с приспособлением?

| Поверхность | Проверка |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada использует двусторонний обмен байтами canonicos + I105 + кодировки, полученные и проверяемые, если коды ошибок не указаны в стиле Norito, включая или приспособление для отрицательных случаев.

## Точная автоматизация?

Автоматический механизм освобождения подъемника обновляет приспособление с помощью помощника
`scripts/account_fixture_helper.py`, que busca ou verifica o pacote canonico sem copy/paste:

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

Помощник переопределяет `--source` или вариант окружения `IROHA_ACCOUNT_FIXTURE_URL`, чтобы задания CI SDK были выбраны для вашего предпочтительного зеркала. Когда `--metrics-out` работает, помогает записывать `account_address_fixture_check_status{target=\"...\"}` вместе с дайджестом SHA-256 canonico (`account_address_fixture_remote_info`) для сборщиков текстовых файлов Prometheus и панели управления Grafana `account_address_fixture_status` доказывает, что это поверхностное постоянство в синхронизации. Предупреждайте, когда будет указан целевой отчет `0`. Для автоматического многоуровневого использования оболочки `ci/account_fixture_metrics.sh` (повторения `--target label=path[::source]`) для обеспечения публичного вызова по вызову единого архива `.prom`, консолидированного для сборщика текстовых файлов и экспортера узлов.

## Precisa do Brief Complete?

O статус полного соответствия ADDR-2 (владельцы, план мониторинга, itens de acao em aberto)
fica em `docs/source/account_address_status.md` в репозитории, объединенном с RFC или структурой адреса (`docs/account_structure.md`). Используйте «Эста страница» как «Lembrete Operationacional Rapido»; Для более подробной информации обратитесь к репозиторию документации.