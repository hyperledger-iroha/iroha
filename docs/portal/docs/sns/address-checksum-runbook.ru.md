---
lang: ru
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d15f10764ee430825f4d7caa142c6664016d3934ce7c0da68314a4a4a6f700bd
source_last_modified: "2025-11-15T15:21:14.374476+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/address_checksum_failure_runbook.md`. Сначала обновите исходный файл, затем синхронизируйте эту копию.
:::

Ошибки контрольной суммы проявляются как `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) в Torii, SDK и клиентах wallet/explorer. Пункты дорожной карты ADDR-6/ADDR-7 требуют, чтобы операторы следовали этому runbook, когда срабатывают алерты контрольной суммы или тикеты поддержки.

## Когда запускать этот play

- **Оповещения:** `AddressInvalidRatioSlo` (определен в `dashboards/alerts/address_ingest_rules.yml`) срабатывает, и аннотации содержат `reason="ERR_CHECKSUM_MISMATCH"`.
- **Дрейф fixture:** текстовый файл Prometheus `account_address_fixture_status` или дашборд Grafana сообщает о checksum mismatch для любой копии SDK.
- **Эскалации поддержки:** команды wallet/explorer/SDK сообщают об ошибках checksum, порче IME или сканировании буфера обмена, которое больше не декодируется.
- **Ручное наблюдение:** логи Torii многократно показывают `address_parse_error=checksum_mismatch` для production endpoints.

Если инцидент связан именно с коллизиями Local-8/Local-12, следуйте playbook `AddressLocal8Resurgence` или `AddressLocal12Collision`.

## Чеклист доказательств

| Доказательство | Команда / Расположение | Примечания |
|---------------|------------------------|------------|
| Snapshot Grafana | `dashboards/grafana/address_ingest.json` | Зафиксируйте разбивку причин ошибок и затронутые endpoints. |
| Payload оповещения | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Включите контекстные метки и таймстемпы. |
| Состояние fixture | `artifacts/account_fixture/address_fixture.prom` + Grafana | Подтверждает, что копии SDK не ушли от `fixtures/account/address_vectors.json`. |
| Запрос PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Экспортируйте CSV для документа инцидента. |
| Логи | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (или агрегация логов) | Очистите PII перед обменом. |
| Проверка fixture | `cargo xtask address-vectors --verify` | Подтверждает, что канонический генератор и коммитнутый JSON совпадают. |
| Проверка паритета SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Запустите для каждого SDK из алертов/тикетов. |
| Буфер обмена/IME | `iroha address inspect <literal>` | Выявляет скрытые символы или переписывание IME; см. `address_display_guidelines.md`. |

## Немедленный ответ

1. Подтвердите алерт, добавьте snapshots Grafana + вывод PromQL в поток инцидента и отметьте затронутые контексты Torii.
2. Заморозьте промоции manifest / релизы SDK, которые затрагивают парсинг адресов.
3. Сохраните snapshots дашборда и артефакты Prometheus textfile в папке инцидента (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. Соберите примеры логов с payloads `checksum_mismatch`.
5. Уведомите владельцев SDK (`#sdk-parity`) примерными payloads для triage.

## Изоляция причины

### Дрейф fixture или генератора

- Повторно запустите `cargo xtask address-vectors --verify`; при ошибке пересоздайте.
- Запустите `ci/account_fixture_metrics.sh` (или `scripts/account_fixture_helper.py check` отдельно) для каждого SDK и подтвердите, что включенные fixtures совпадают с каноническим JSON.

### Регрессии клиентских энкодеров / IME

- Проверьте literals, предоставленные пользователями, через `iroha address inspect` и ищите нулевой join, преобразования kana или обрезанные payloads.
- Сверьте потоки wallet/explorer с `docs/source/sns/address_display_guidelines.md` (двойное копирование, предупреждения, QR helpers), чтобы убедиться в следовании утвержденному UX.

### Проблемы manifest или реестра

- Следуйте `address_manifest_ops.md`, чтобы повторно проверить последний manifest bundle и убедиться, что селекторы Local-8 не вернулись.

### Злонамеренный или поврежденный трафик

- Разберите offending IPs/app IDs через логи Torii и `torii_http_requests_total`.
- Сохраните не менее 24 часов логов для Security/Governance.

## Митигирование и восстановление

| Сценарий | Действия |
|----------|----------|
| Дрейф fixture | Пересоздайте `fixtures/account/address_vectors.json`, повторно запустите `cargo xtask address-vectors --verify`, обновите SDK bundles и приложите snapshots `address_fixture.prom` к тикету. |
| Регрессия SDK/клиента | Откройте issues, ссылаясь на канонический fixture + вывод `iroha address inspect`, и заблокируйте релизы через SDK parity CI (например, `ci/check_address_normalize.sh`). |
| Злонамеренные отправки | Примените rate-limit или блокировку offending principals, эскалируйте в Governance при необходимости tombstone селекторов. |

После применения мер снова выполните запрос PromQL и убедитесь, что `ERR_CHECKSUM_MISMATCH` остается на нуле (кроме `/tests/*`) минимум 30 минут перед понижением инцидента.

## Закрытие

1. Архивируйте snapshots Grafana, CSV PromQL, фрагменты логов и `address_fixture.prom`.
2. Обновите `status.md` (раздел ADDR) и строку roadmap при изменении инструментов/документации.
3. Добавьте пост-инцидентные заметки в `docs/source/sns/incidents/`, если появились новые выводы.
4. Убедитесь, что в релизных заметках SDK упомянуты исправления checksum при необходимости.
5. Подтвердите, что алерт остается зеленым 24h и проверки fixture остаются зелеными перед закрытием.
