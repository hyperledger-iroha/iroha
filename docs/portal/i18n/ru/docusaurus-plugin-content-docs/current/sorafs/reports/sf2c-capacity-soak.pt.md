---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Релатор для замачивания накопительной емкости SF-2c

Данные: 21 марта 2026 г.

## Эскопо

Это отношение регистрации яичек, определяющих впитывание накопления и увеличение емкости SoraFS
запросили три SF-2c для составления дорожной карты.

- **Включить поддержку нескольких поставщиков в течение 30 дней:** Выполнить для
  `capacity_fee_ledger_30_day_soak_deterministic` ем
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Используйте провайдеров Instancia cinco, начиная с 30 дней поселения и
  действительность того, что все бухгалтерские книги соответствуют проекту ссылки
  расчет независимой формы. O тесте излучает эм дайджест Blake3
  (`capacity_soak_digest=...`), чтобы CI мог сделать снимок и сравнить его
  канонико.
- **Наказания за нарушение:** Применение за
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (месмо архив). Я проверяю, какие ограничения на удары, кулдауны и рубящие удары.
  залог и контрагенты делают реестр постоянным и детерминированным.

## Экзекукао

Выполните как валидацию localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Наши тесты завершены во второй половине дня на ноутбуке и нао exigem
внешние светильники.

## Наблюдательность

Torii назад были показаны снимки кредитов поставщиков и реестры комиссий для панелей мониторинга.
Поссам Фацер Гейт в Сальдос Байшос и пенальти:

- ОСТАЛЬНОЕ: `GET /v2/sorafs/capacity/state` возвращает `credit_ledger[*]` que
  refletem os Campos do Ledger Verificados без проверки. Вежа
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Импортируемая операционная система Grafana: `dashboards/grafana/sorafs_capacity_penalties.json`
  контрадоры экспортированных забастовок, общее количество штрафов и залога для того, чтобы
  Время дежурства можно сравнить с базовыми показателями пребывания в окружающей среде на производстве.

## Последующие действия

- Повестка дня выполняется несколько раз в воротах CI для повторного выполнения или проверки замачивания (дымовой уровень).
- Используйте краску Grafana со всеми инструментами очистки Torii, которые используются для экспорта телеметрических данных для производства и эксплуатации.