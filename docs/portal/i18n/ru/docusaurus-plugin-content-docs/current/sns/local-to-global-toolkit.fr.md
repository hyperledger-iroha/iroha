---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор адресов Локальный -> Глобальный

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` du mono-repo. Она перегруппирует вспомогательные интерфейсы командной строки и книги запусков, необходимые для дорожной карты элемента **ADDR-5c**.

## Аперку

- `scripts/address_local_toolkit.sh` инкапсулирует CLI `iroha` для создания продукта:
  - `audit.json` -- структура вылета `iroha tools address audit --format json`.
  - `normalized.txt` -- буквенный IH58 (предпочтительно) / сжатый (`sora`) (второй выбор) конвертировать для выбора локального домена.
- Ассоциация сценариев с информационной панелью для приема адресов (`dashboards/grafana/address_ingest.json`)
  и дополнительные правила Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), чтобы убедиться, что переключение Local-8 /
  Local-12 est sur. Наблюдение за панелями столкновений Local-8 и Local-12 и предупреждения
  `AddressLocal8Resurgence`, `AddressLocal12Collision` и `AddressInvalidRatioSlo` перед
  рекламируйте изменения манифеста.
- Ознакомьтесь с [Рекомендациями по отображению адресов] (address-display-guidelines.md) и другими файлами.
  [Ранбук адресного манифеста] (../../../source/runbooks/address_manifest_ops.md) для контекста пользовательского интерфейса и реагирования на дополнительные инциденты.

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Опции:

- `--format compressed (`sora`)` для вылазки `sora...` на месте IH58.
- `--no-append-domain` для чтения новых книг.
- `--audit-only` для игнорирования этапа преобразования.
- `--allow-errors` для продолжения сканирования четырех линий неправильного устройства (соответствует работе CLI).

Le script ecrit les chemins des artefacts a la fin de l'execution. Joignez les deux fichiers a
Ваш билет на изменение с захватом Grafana, который доказывает ноль
обнаружения Local-8 и отсутствие столкновений Local-12 подвеска >=30 дней.

## Интеграция CI

1. Создайте сценарий в задании и загрузите вылеты.
2. Заблокируйте объединения и сигналы `audit.json` для локального выбора (`domain.kind = local12`).
   значение по умолчанию `true` (не проходит мимо `false`, которое используется для разработки/тестирования кластеров).
   диагностическая дерегрессия) и др.
   `iroha tools address normalize --fail-on-warning --only-local` CI для регрессий
   эхо перед производством.

Просмотрите источник документа, а также подробную информацию, контрольные списки доказательств и фрагмент
Примечания к выпуску, которые вы можете повторно использовать для объявления о переключении дополнительных клиентов.