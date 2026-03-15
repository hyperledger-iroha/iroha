---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c Выдержка для увеличения мощности

Дата: 21 марта 2026 г.

## اسکوپ

SoraFS начисление мощности и выплата детерминированных тестов на выдержку ریکارڈ کرتی ہے۔

- **30 дней для нескольких поставщиков:**
  `capacity_fee_ledger_30_day_soak_deterministic` کے ذریعے
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  поставщики жгутов بناتا ہے، 30 поселение ونڈوز کا احاطہ کرتا ہے، اور
  تصدیق کرتا ہے کہ итоги бухгалтерской книги ایک آزادانہ طور پر حساب شدہ سے ملتے ہیں۔
  Получение дайджеста Blake3 (`capacity_soak_digest=...`) и создание канонического моментального снимка CI.
  захват اور diff کر سکے۔
– **Штрафы за недопоставку:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔ ٹیسٹ تصدیق کرتا ہے کہ поражает пороги, кулдауны, побочные удары
  Детерминированные счетчики бухгалтерской книги رہتے ہیں۔

## Выполнение

Впитывать валидации. Дополнительные сведения:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

یہ ٹیسٹ ایک عام لیپ ٹاپ پر ایک سیکنڈ سے کم وقت میں مکمل ہو جاتے ہیں اور بیرونی светильники и видео

## Наблюдаемость

Torii — снимки кредитов поставщика, книги комиссий, а также информационные панели, балансы и штрафные удары, а также ворота:

- REST: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` записи для проверки или проверки на выдержку.
  Поля бухгалтерской книги دیکھیں
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana импорт: `dashboards/grafana/sorafs_capacity_penalties.json` экспортировали счетчики ударов, суммы штрафов,
  залоговое обеспечение, сюжет, дежурство по вызову, замачивание базовых линий, живая среда и многое другое.

## Последующие действия

- CI обеспечивает тест на выдержку и тест на замачивание (дымовой уровень).
- Экспорт телеметрии производства в реальном времени. Grafana Board Torii. Целевые объекты очистки.