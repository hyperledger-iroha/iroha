---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор направлений Локальный -> Глобальный

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` монорепозитория. Воспользуйтесь помощниками CLI и Runbook, необходимыми для элемента дорожной карты **ADDR-5c**.

## Резюме

- `scripts/address_local_toolkit.sh` запустит CLI `iroha` для создания:
  - `audit.json` -- структура `iroha tools address audit --format json`.
  - `normalized.txt` — литералы I105 (предпочтительно) / сжатые (`sora`) (второй лучший вариант), преобразованные для каждого выбора локального управления.
- Комбинация сценария с панелью управления приемом направлений (`dashboards/grafana/address_ingest.json`)
  и правила Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) для проверки переключения Local-8 /
  Local-12 безопасен. Наблюдайте за панелями столкновений Local-8 и Local-12 и оповещениями
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, y `AddressInvalidRatioSlo` раньше
  промоутер камбиос де манифест.
- Ссылка на [Правила отображения адреса] (address-display-guidelines.md) и эл.
  [Ранбук адресного манифеста] (../../../source/runbooks/address_manifest_ops.md) для контекста UX и ответа на инциденты.

## Усо

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Опции:

- `--format I105` для продажи `sora...` в приложении I105.
- `domainless output (default)` для эмитирования литералов без владения.
- `--audit-only`, чтобы пропустить этап преобразования.
- `--allow-errors`, чтобы выполнить сканирование при повреждении неправильных файлов (совпадает с совместимостью CLI).

Сценарий описывает маршруты артефактов до финального выброса. Adjunta ambos archives a
Ваш билет на участие в сборе со скриншотом Grafana, который вы получите
обнаружения Local-8 и cero colisiones Local-12 при >=30 диам.

## Интеграция CI

1. Выполните сценарий на посвященной работе и выполните все задания.
2. Bloquea объединяет локальные селекторы отчетов `audit.json` (`domain.kind = local12`).
   в вашей доблести из-за дефекта `true` (одиночное переопределение `false` в кластерах dev/test al
   диагностические регрессии) и агрегаты
   `iroha tools address normalize` — это CI для того, чтобы
   регресс упал до начала производства.

Проконсультируйтесь с документом, содержащим дополнительные сведения, контрольными списками доказательств и фрагментами
Примечания к выпуску, которые можно повторно использовать, чтобы объявить о переключении клиентам.