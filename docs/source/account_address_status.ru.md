---
lang: ru
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

## Статус соответствия адресов аккаунтов (ADDR-2)

Статус: Принято 2026-03-30  
Владельцы: команда модели данных / гильдия QA  
Ссылка на дорожную карту: ADDR-2 — Dual-Format Compliance Suite

### 1. Обзор

- Фикстура: `fixtures/account/address_vectors.json` (I105 + multisig: позитивные/негативные кейсы).
- Область: детерминированные V1 payloads, покрывающие implicit-default, Local-12, Global registry и multisig controllers с полной таксономией ошибок.
- Распространение: используется в Rust data-model, Torii, JS/TS, Swift и Android SDK; CI падает, если любой потребитель отклоняется.
- Источник истины: генератор находится в `crates/iroha_data_model/src/account/address/compliance_vectors.rs` и доступен через `cargo xtask address-vectors`.

### 2. Перегенерация и проверка

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Флаги:

- `--out <path>` — опциональное переопределение для ad-hoc bundles (по умолчанию `fixtures/account/address_vectors.json`).
- `--stdout` — выводит JSON в stdout вместо записи на диск.
- `--verify` — сравнивает текущий файл со свежесгенерированным контентом (быстро падает при дрейфе; несовместимо с `--stdout`).

### 3. Матрица артефактов

| Поверхность | Контроль | Примечания |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Парсит JSON, восстанавливает канонические payloads и проверяет конверсии I105/canonical + структурированные ошибки. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Проверяет серверные codecs, чтобы Torii детерминированно отклонял некорректные I105 payloads. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Зеркалирует V1 fixtures (I105/fullwidth) и проверяет коды ошибок в стиле Norito для каждого негативного кейса. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Проверяет декодирование I105, multisig payloads и выдачу ошибок на платформах Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Гарантирует, что Kotlin/Java биндинги остаются согласованными с канонической фикстурой. |

### 4. Мониторинг и незавершенная работа

- Отчетность: этот документ связан со `status.md` и roadmap, чтобы еженедельные обзоры могли проверять здоровье фикстуры.
- Сводка для портала разработчиков: см. **Reference -> Account address compliance** в docs портале (`docs/portal/docs/reference/account-address-status.md`) для внешнего резюме.
- Prometheus и дашборды: при проверке копии SDK запускайте helper с `--metrics-out` (и при необходимости `--metrics-label`), чтобы textfile collector Prometheus мог собирать `account_address_fixture_check_status{target=...}`. Дашборд Grafana **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) показывает pass/fail по каждой поверхности и выводит канонический SHA-256 digest как доказательство аудита. Поднимайте алерт, если любой target сообщает `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` теперь эмитируется для каждого успешно разобранного account literal, отражая `torii_address_invalid_total`/`torii_address_local8_total`. Поднимайте алерт на любой трафик `domain_kind="local12"` в продакшене и зеркалируйте счетчики в SRE дашборд `address_ingest`, чтобы уход от Local-12 имел аудиторские доказательства.
- Fixture helper: `scripts/account_fixture_helper.py` скачивает или проверяет канонический JSON, чтобы автоматизация релизов SDK могла получать/проверять bundle без ручного копирования, при необходимости записывая метрики Prometheus. Пример:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  Helper пишет `account_address_fixture_check_status{target="android"} 1`, когда target совпадает, а также gauges `account_address_fixture_remote_info` / `account_address_fixture_local_info`, которые показывают SHA-256 digests. Отсутствующие файлы дают `account_address_fixture_local_missing`.
  Automation wrapper: вызывайте `ci/account_fixture_metrics.sh` из cron/CI, чтобы сформировать единый textfile (по умолчанию `artifacts/account_fixture/address_fixture.prom`). Передавайте повторяющиеся `--target label=path` (при необходимости добавляйте `::https://mirror/...` для каждого target, чтобы переопределить источник), чтобы Prometheus скрейпил один файл для всех копий SDK/CLI. GitHub workflow `address-vectors-verify.yml` уже запускает helper против канонической фикстуры и загружает artifact `account-address-fixture-metrics` для SRE ingestion.
