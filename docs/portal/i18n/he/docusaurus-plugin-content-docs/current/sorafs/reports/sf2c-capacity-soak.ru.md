---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о soak начисления емкости SF-2c

תאריך: 2026-03-21

## Область

Этот отчет фиксирует детерминированные тесты soak начисления емкости SoraFS и выплат,
запрошенные в дорожной карте SF-2c.

- טבילה מרובה ספקים של **30 דונם:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic` в
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  רתום создает пять ספקים, охватывает 30 окон התנחלות и
  проверяет, что итоги ספר חשבונות совпадают с независимо вычисленной эталонной
  проекцией. Тест выводит Blake3 digest (`capacity_soak_digest=...`), чтобы CI
  могла захватить и сравнить канонический תמונת מצב.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл). Тест подтверждает, что пороги שביתות, התקררות, חתכים
  בטחונות и счетчики ספר חשבונות остаются детерминированными.

## Выполнение

Запустите проверки להשרות локально:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты завершаются меньше чем за секунду на стандартном ноутбуке и не требуют
גופי внешних.

## Наблюдаемость

Torii теперь показывает צילומי מצב кредитов ספקי вместе с פנקסי עמלות, чтобы
לוחות מחוונים могли שער по низким балансам и פגעי עונשין:

- REST: `GET /v2/sorafs/capacity/state` возвращает записи `credit_ledger[*]`,
  которые отражают поля ספר חשבונות, проверенные в soak тесте. См.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- דגם Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` строит
  שביתות экспортированные счетчики, суммы штрафов и залог בטחונות, чтобы
  дежурная команда могла сравнивать קו הבסיס לספוג с живыми окружениями.

## Дальнейшие шаги

- Запланировать еженедельные gate-прогоны в CI для воспроизведения להשרות теста (מדרגת עשן).
- Расширить панель Grafana целями לגרד Torii после запуска экспортов telemetry в прод.