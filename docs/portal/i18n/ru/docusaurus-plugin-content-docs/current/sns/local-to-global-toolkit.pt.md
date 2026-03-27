---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos Локальный -> Глобальный

Эта страница написана `docs/source/sns/local_to_global_toolkit.md` в монорепозитории. Группа помощников CLI и книг Runbook содержит один элемент дорожной карты **ADDR-5c**.

## Визао гераль

- `scripts/address_local_toolkit.sh` инкапсулирует CLI `iroha` для производства:
  - `audit.json` -- указано, что `iroha tools address audit --format json`.
  - `normalized.txt` -- буквенный i105 (предпочтительно) / сжатый (`sora`) (второй вариант) преобразование для выбора локального управления.
- Объедините скрипт с панелью управления приемом данных (`dashboards/grafana/address_ingest.json`)
  как указано в Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) для проверки переключения Local-8 /
  Local-12 и безопасно. Следите за болью в коллизиях Local-8 и Local-12 и оповещениями
  `AddressLocal8Resurgence`, `AddressLocal12Collision` и `AddressInvalidRatioSlo` раньше
  промоутер Муданкас де Манифест.
- См. [Правила отображения адреса] (address-display-guidelines.md) e o
  [Ранбук адресного манифеста] (../../../source/runbooks/address_manifest_ops.md) для контекста UX и ответа на инциденты.

## Усо

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Операции:

- `--format i105` для указанного `sora...` среди i105.
- `domainless output (default)` для написания букв в собственном доме.
- `--audit-only` для быстрого начала разговора.
- `--allow-errors`, чтобы продолжить работу с неправильным отображением строк (например, в режиме совместимости с CLI).

Сценарий создаст камино-артефатос для окончательного выполнения. Приложение os dois arquivos ao
Seu Ticket de gestao de mudancas junto com или скриншот do Grafana, который соответствует нулю
deteccoes Local-8 и нулевые колизои Local-12 пор >=30 диам.

## Интеграция CI

1. Ездил по сценарию, посвященному работе, и завидовал, как сказано.
2. Блокировка объединяет те сообщения `audit.json`, которые выбирают локальные отчеты (`domain.kind = local12`).
   нет доблести `true` (так что измените пункт `false` для разработки/тестирования кластеров и диагностики)
   регрессивные) и пристрастие
   `iroha tools address normalize` или CI для регресса
   falhem перед тем, как начать производство.

Используйте шрифт документа для более подробной информации, контрольные списки доказательств и фрагменты
Примечания к выпуску, которые можно повторно использовать, чтобы объявить о переключении для клиентов.