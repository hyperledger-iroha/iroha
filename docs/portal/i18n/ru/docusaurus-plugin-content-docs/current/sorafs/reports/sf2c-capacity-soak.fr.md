---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Соединение для впитывания накопительной емкости SF-2c

Дата: 21 марта 2026 г.

## Порте

В этом сообщении представлены тесты, определяющие задержку накопления и оплату емкости SoraFS, которые требуются.
на трассе SF-2c.

- **Включить работу нескольких поставщиков в течение 30 дней:** Выполнено по номиналу.
  `capacity_fee_ledger_30_day_soak_deterministic` в
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Используйте мгновенные поставщики услуг, включающие 30 условий урегулирования и т. д.
  действительно, что все корреспонденции бухгалтерской книги являются эталонной проекцией
  расчет независимый. Тестирование дайджеста Blake3 (`capacity_soak_digest=...`)
  afin que CI puisse capturer et Compare le snapshot canonique.
- **Наказания за су-ливрейсон:** Аппликации по
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (meme fichier). Тест подтверждает результаты ударов, время восстановления,
  сокращение залогового обеспечения и счетов бухгалтерской книги, остающихся детерминированными.

## Исполнение

Просмотрите проверки локалитета с учетом:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты завершаются в течение нескольких секунд в соответствии со стандартами ноутбуков и другими
необходимо дополнительное внешнее приспособление.

## Наблюдаемость

Torii предоставляет доступ к моментальным снимкам поставщиков кредитных услуг и реестрам комиссий на панелях мониторинга.
Мощные ворота для убийств и пенальти:

- ОСТАЛЬНОЕ: `GET /v2/sorafs/capacity/state` отправка входных билетов `credit_ledger[*]` qui
  отражены проверенные поля champs du Ledger в тесте на замачивание. Вуар
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Импортировать файлы трассировки Grafana: `dashboards/grafana/sorafs_capacity_penalties.json`.
  контролеры по экспорту забастовок, все штрафы и залоговое обеспечение, задействованное в ближайшее время
  Оборудование по вызову позволяет сравнить базовые линии погружения в живую среду.

## Суйви

- Планировщик казней ворот и CI для повторного испытания на замачивание (уровень дыма).
- Étendre le tableau Grafana с кабелями очистки Torii для экспорта телеметрии производства
  seront en ligne.