---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор инструментов Локальный -> Глобальный адресов

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` из монорепозитория. Она включает помощники CLI и Runbook, требуемые пунктом дорожной карты **ADDR-5c**.

## Обзор

- `scripts/address_local_toolkit.sh` оборачивает CLI `iroha`, чтобы получить:
  - `audit.json` -- структурированный вывод `iroha tools address audit --format json`.
  - `normalized.txt` -- преобразованные IH58 (предпочтительно) / сжатые (`sora`) (второй выбор) литералы для каждого Селектор локального домена.
- Используйте скрипт вместе с приемом адресов панели управления (`dashboards/grafana/address_ingest.json`)
  и режим Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), чтобы обеспечить переключение безопасности Local-8 /
  Местный-12. Следите за панелями коллизий Local-8 и Local-12 и алертами
  `AddressLocal8Resurgence`, `AddressLocal12Collision` и `AddressInvalidRatioSlo` перед
  Продвижение изменений манифест.
- Сверяйтесь с [Рекомендациями по отображению адресов] (address-display-guidelines.md) и
  [Ранбук адресного манифеста] (../../../source/runbooks/address_manifest_ops.md) для пользовательского интерфейса и контекста реагирования на инциденты.

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Опции:

- `--format compressed (`sora`)` для вывода `sora...` вместо IH58.
- `--no-append-domain` для вывода голых литералов.
- `--audit-only`, чтобы пропустить шаг конвертации.
- `--allow-errors` для продолжения подтверждения ошибочных строк (поведение соответствует CLI).

Скрипт выводит пути документов в конце выполнения. Приложите оба файла к
Заявка на управление изменениями вместе со скриншотом Grafana, подтверждающий ноль
Локальные-8 детекций и ноль Локальные-12 коллизий минимум за >=30 дней.

## Интеграция CI

1. Запустите скрипт в отдельном задании и загрузите выходные данные.
2. Блокируйте слияния, когда `audit.json` сообщает Локальные селекторы (`domain.kind = local12`).
   со значением по умолчанию `true` (меняйте на `false` только в dev/test при диагностике регрессий) и
   записи `iroha tools address normalize --fail-on-warning --only-local` в CI, чтобы попытаться регрессии
   падали до производства.

См. исходный документ для деталей, контрольные списки доказательств и фрагмент примечания к выпуску, которые можно
использовать при объявлении переключения для клиентов.