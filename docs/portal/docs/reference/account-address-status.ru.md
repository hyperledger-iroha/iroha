---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f18b4a15e42363483c4f65945aba2cc208f9a2e59b6f31f143e5dc792d8d9071
source_last_modified: "2025-11-27T10:35:59.095888+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: account-address-status
title: Соответствие адресов аккаунтов
description: Сводка рабочего процесса ADDR-2 fixture и синхронизации команд SDK.
---

Канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) включает fixtures canonical Katakana i105, multisignature и negative. Каждая поверхность SDK + Torii опирается на один и тот же JSON, чтобы обнаруживать дрейф codec до выхода в прод. Эта страница отражает внутренний статусный бриф (`docs/source/account_address_status.md` в корне репозитория), чтобы читатели портала могли обратиться к workflow без необходимости копаться в mono-repo.

## Перегенерация или проверка пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — выводит JSON в stdout для ad-hoc проверки.
- `--out <path>` — записывает в другой путь (например, при локальном сравнении изменений).
- `--verify` — сравнивает рабочую копию со свежесгенерированным содержимым (нельзя совмещать с `--stdout`).

CI workflow **Address Vector Drift** запускает `cargo xtask address-vectors --verify`
каждый раз, когда меняется fixture, генератор или docs, чтобы немедленно предупредить ревьюеров.

## Кто использует fixture?

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Каждый harness выполняет round-trip канонических байт + I105 + сжатых кодировок и проверяет, что коды ошибок в стиле Norito совпадают с fixture для negative кейсов.

## Нужна автоматизация?

Release tooling может автоматизировать обновления fixture через helper
`scripts/account_fixture_helper.py`, который получает или проверяет канонический пакет без копирования/вставки:

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

Helper принимает overrides через `--source` или переменную окружения `IROHA_ACCOUNT_FIXTURE_URL`, чтобы CI джобы SDK могли указывать предпочтительное зеркало. При передаче `--metrics-out` helper записывает `account_address_fixture_check_status{target="…"}` вместе с каноническим SHA-256 digest (`account_address_fixture_remote_info`), чтобы Prometheus textfile collectors и дашборд Grafana `account_address_fixture_status` могли подтвердить синхронизацию каждой поверхности. Настройте алерт, когда target сообщает `0`. Для multi-surface автоматизации используйте wrapper `ci/account_fixture_metrics.sh` (принимает повторяющиеся `--target label=path[::source]`), чтобы on-call команды публиковали единый `.prom` файл для textfile collector node-exporter.

## Нужен полный бриф?

Полный статус соответствия ADDR-2 (owners, план мониторинга, открытые задачи)
хранится в `docs/source/account_address_status.md` внутри репозитория вместе с Address Structure RFC (`docs/account_structure.md`). Используйте эту страницу как оперативное напоминание; для глубокой справки обращайтесь к docs репозитория.
