---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о выдержке расчета емкости SF-2c

Дата: 2026-03-21

## Область

В этом отчете фиксируются определенные тесты с учетом начисления емкостей SoraFS и выплат,
запрошенные в дорожной машине SF-2c.

- **30-дневная задержка с участием нескольких поставщиков:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic` в
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Harness представляет пять поставщиков, обвиняя 30 окон урегулирования и
  в результате бухгалтерская книга итогов происходит независимо от вычисленной эталонной
  проекцией. Тест выводит дайджест Blake3 (`capacity_soak_digest=...`), чтобы CI
  могла захватить и сравнить стандартный снимок.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл). Тест подтверждает, что пороги наносят удары, кулдауны, рубящие удары
  залог и счетчики книги остаются детерминированными.

## Выполнение

Запустите проверки локально:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты выполняются менее чем за секунду на стандартном ноутбуке и не требуют
разные светильники.

## Наблюдаемость

Torii теперь показывает снимки поставщиков кредитов вместе с книгами комиссий, чтобы
информационные панели могли бы побаловать баланс и наложить штрафы:

- REST: `GET /v2/sorafs/capacity/state` требует записи `credit_ledger[*]`,
  которые отражают поля бухгалтерской книги, проверенные в тесте. См.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Импорт Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` строит
  экспортированные счетчики страйков, суммы штрафов и залоговое обеспечение, чтобы
  дежурная команда могла провести базовое замачивание с живыми окружениями.

## дальнейшее шаги

- Запланировать еженедельные прогоны ворот в CI для проведения теста на замачивание (дымовой уровень).
- Расширить панель Grafana участников Scrape Torii после запуска экспортов телеметрии в прод.