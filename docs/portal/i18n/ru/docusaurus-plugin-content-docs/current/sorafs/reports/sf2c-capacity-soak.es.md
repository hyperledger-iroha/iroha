---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Информация о впитывании емкости SF-2c

Феча: 21 марта 2026 г.

## Альканс

Это информация о регистрации определений впитывания емкости SoraFS на страницах
solicitadas bajo la hoja de ruta SF-2c.

- **Включить поддержку нескольких поставщиков в течение 30 дней:** Добавлено
  `capacity_fee_ledger_30_day_soak_deterministic` ru
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Поставщики услуг El Harvest Instancia cinco, Abarca 30 Ventanas de Settlement y
  действительность того, что общие суммы в бухгалтерской книге совпадают с ссылочным проецированием
  расчет независимой формы. La Prueba излучает дайджест Blake3
  (`capacity_soak_digest=...`), чтобы CI мог захватить и сравнить снимок
  канонико.
- **Наказания за нарушение:** Наказания за нарушение
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (архив мисмо). Подтверждение того, что удары, кулдауны, тени,
  сокращение залога и постоянное детерминирование реестра.

## Выброс

Выведите локальные подтверждения замачивания:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Las pruebas Completen en menos de un segundo en un portátil estándar y no requieren
внешние светильники.

## Наблюдательность

Torii теперь отображает снимки кредитов поставщиков вместе с регистрами комиссий для информационных панелей
Пуэдан ворота собре сальдос бахос и пенальти:

- ОСТАЛЬНОЕ: `GET /v2/sorafs/capacity/state` развить входы `credit_ledger[*]` que
  Отобразить проверенные кампос-дель-леджер в проверке замачивания. Вер
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Импорт Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` графический
  контрадоры экспортированных забастовок, общие суммы штрафов и залога для того, чтобы
  Оборудование по вызову позволяет сравнить базовые показатели замачивания в условиях окружающей среды.

## Сегимиенто

- Программируйте выбросы семантических ворот в CI для повторного запуска замачивания (уровень дыма).
- Расширитель таблицы Grafana с объектами очистки Torii при экспорте
  телеметрия производства доступна.