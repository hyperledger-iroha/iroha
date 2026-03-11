---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор инструментов מקומי -> כתובות גלובליות

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` עם מונו-ריפו. Она включает CLI helpers ו-runbooks, требуемые пунктом дорожной карты **ADDR-5c**.

## תיאור

- `scripts/address_local_toolkit.sh` оборачивает CLI `iroha`, чтобы получить:
  - `audit.json` -- структурированный вывод `iroha tools address audit --format json`.
  - `normalized.txt` -- преобразованные I105 (предпочтительно) / דחוס (`sora`) (второй выбор) מילוליות ל-каждого Local-domain selector.
- Используйте скрипт вместе с לוח המחוונים ingest адресов (`dashboards/grafana/address_ingest.json`)
  и правилами Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), чтобы доказать безопасность cutover Local-8 /
  מקומי-12. הצג את האוסף המקומי Local-8 ו-Local-12 ו-Alertem
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, ו-`AddressInvalidRatioSlo`
  продвижением изменений מניפסט.
- Сверяйтесь с [הנחיות להצגת כתובת](address-display-guidelines.md) и
  [פנקס ריצה של מניפסט כתובות](../../../source/runbooks/address_manifest_ops.md) ל-UX и אירוע-תגובה контекста.

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

תיאור:

- `--format I105` עבור вывода `sora...` вместо I105.
- `domainless output (default)` עבור вывода מילוליות חשופות.
- `--audit-only` чтобы пропустить шаг конвертации.
- `--allow-errors` чтобы продолжать сканирование при ошибочных строках (поведение совпадает с CLI).

Скрипт выводит пути артефактов в конце выполнения. Приложите оба файла к
כרטיס לניהול שינויים вместе с Grafana צילום מסך, подтверждающим ноль
Local-8 детекций и ноль Local-12 коллизий минимум за >=30 дней.

## Интеграция CI

1. Запустите скрипт в отдельном job и загрузите פלטים.
2. Блокируйте מתמזג, когда `audit.json` сообщает בוררים מקומיים (`domain.kind = local12`).
   со значением по умолчанию `true` (מומלץ ל-`false` только в dev/test при диагностике регрессий)
   добавьте `iroha tools address normalize` в CI, чтобы попытки регрессии
   падали до ייצור.

См. исходный документ для деталей, ראיות чеклистов וקטע הערת שחרור, который можно
использовать при анонсе cutover для клиентов.