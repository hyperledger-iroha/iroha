---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-11-10T16:20:18.769519+00:00"
translation_last_reviewed: 2026-01-30
---

# Отчет о soak начисления емкости SF-2c

Дата: 2026-03-21

## Область

Этот отчет фиксирует детерминированные тесты soak начисления емкости SoraFS и выплат,
запрошенные в дорожной карте SF-2c.

- **30-дневный multi-provider soak:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic` в
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Harness создает пять providers, охватывает 30 окон settlement и
  проверяет, что итоги ledger совпадают с независимо вычисленной эталонной
  проекцией. Тест выводит Blake3 digest (`capacity_soak_digest=...`), чтобы CI
  могла захватить и сравнить канонический snapshot.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл). Тест подтверждает, что пороги strikes, cooldowns, slashes
  collateral и счетчики ledger остаются детерминированными.

## Выполнение

Запустите проверки soak локально:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты завершаются меньше чем за секунду на стандартном ноутбуке и не требуют
внешних fixtures.

## Наблюдаемость

Torii теперь показывает snapshots кредитов providers вместе с fee ledgers, чтобы
dashboards могли gate по низким балансам и penalty strikes:

- REST: `GET /v1/sorafs/capacity/state` возвращает записи `credit_ledger[*]`,
  которые отражают поля ledger, проверенные в soak тесте. См.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Импорт Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` строит
  экспортированные счетчики strikes, суммы штрафов и залог collateral, чтобы
  дежурная команда могла сравнивать baseline soak с живыми окружениями.

## Дальнейшие шаги

- Запланировать еженедельные gate-прогоны в CI для воспроизведения soak теста (smoke-tier).
- Расширить панель Grafana целями scrape Torii после запуска экспортов telemetry в прод.
