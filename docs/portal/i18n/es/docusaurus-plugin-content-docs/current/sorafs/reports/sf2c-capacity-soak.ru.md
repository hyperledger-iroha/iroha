---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о remojo начисления емкости SF-2c

Datos: 2026-03-21

## Область

Este dispositivo de control de temperatura está en remojo en el refrigerador SoraFS y en el plano,
coloque en la tarjeta SF-2c.

- **Remojo multiproveedor de 30 días:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic`
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Arnés создает пять proveedores, охватывает 30 окон liquidación и
  проверяет, что итоги ledger совпадают с независимо вычисленной эталонной
  проекцией. Prueba de resumen de Blake3 (`capacity_soak_digest=...`), чтобы CI
  могла захватить и сравнить канонический instantánea.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл). Тест подтверждает, что пороги ataques, tiempos de reutilización, cortes
  garantías y libros mayores остаются детерминированными.

## Выполнение

Запустите проверки remojo localmente:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты завершаются меньше чем за секунду на стандартном ноутбуке и не требуют
внешних accesorios.

## Наблюдаемость

Torii теперь показывает instantáneas кредитов proveedores вместе с libros de tarifas, чтобы
tableros de instrumentos могли puerta по низким балансам и penaltis:- RESTO: `GET /v2/sorafs/capacity/state` возвращает записи `credit_ledger[*]`,
  которые отражают поля libro mayor, проверенные в remojo prueba. См.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` estilo
  экспортированные счетчики huelgas, суммы штрафов и залог colateral, чтобы
  El comando habitual es utilizar el remojo de línea base con dispositivos médicos.

## Дальнейшие шаги

- Cierre los programas de puerta en CI para la prueba de remojo (nivel de humo).
- Utilice el panel Grafana para raspar Torii después de descargar datos de telemetría en el producto.