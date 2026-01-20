---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Набор инструментов Local -> Global адресов

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` из mono-repo. Она включает CLI helpers и runbooks, требуемые пунктом дорожной карты **ADDR-5c**.

## Обзор

- `scripts/address_local_toolkit.sh` оборачивает CLI `iroha`, чтобы получить:
  - `audit.json` -- структурированный вывод `iroha address audit --format json`.
  - `normalized.txt` -- преобразованные IH58/compressed literals для каждого Local-domain selector.
- Используйте скрипт вместе с dashboard ingest адресов (`dashboards/grafana/address_ingest.json`)
  и правилами Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), чтобы доказать безопасность cutover Local-8 /
  Local-12. Следите за панелями коллизий Local-8 и Local-12 и алертами
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, и `AddressInvalidRatioSlo` перед
  продвижением изменений manifest.
- Сверяйтесь с [Address Display Guidelines](address-display-guidelines.md) и
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) для UX и incident-response контекста.

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Опции:

- `--format compressed` для вывода `snx1...` вместо IH58.
- `--no-append-domain` для вывода bare literals.
- `--audit-only` чтобы пропустить шаг конвертации.
- `--allow-errors` чтобы продолжать сканирование при ошибочных строках (поведение совпадает с CLI).

Скрипт выводит пути артефактов в конце выполнения. Приложите оба файла к
change-management ticket вместе с Grafana screenshot, подтверждающим ноль
Local-8 детекций и ноль Local-12 коллизий минимум за >=30 дней.

## Интеграция CI

1. Запустите скрипт в отдельном job и загрузите outputs.
2. Блокируйте merges, когда `audit.json` сообщает Local selectors (`domain.kind = local12`).
   со значением по умолчанию `true` (меняйте на `false` только в dev/test при диагностике регрессий) и
   добавьте `iroha address normalize --fail-on-warning --only-local` в CI, чтобы попытки регрессии
   падали до production.

См. исходный документ для деталей, evidence чеклистов и release-note snippet, который можно
использовать при анонсе cutover для клиентов.
