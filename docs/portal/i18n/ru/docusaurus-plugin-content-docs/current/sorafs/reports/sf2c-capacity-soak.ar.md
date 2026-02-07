---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير اختبار замочить لتراكم سعة SF-2c

Сообщение: 21 марта 2026 г.

## النطاق

Воспользуйтесь функцией замачивания, чтобы замочить защитную пленку SoraFS. Он был создан на базе SF-2c.

- **замачивание в течение 30 дней:**
  `capacity_fee_ledger_30_day_soak_deterministic` в
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Его упряжь была пройдена в Нью-Йорке, 30 дней назад. إجماليات
  бухгалтерская книга Дайджест новостей от Blake3
  (`capacity_soak_digest=...`) Зарегистрирован CI в режиме онлайн-обновления.
- ** عقوبات نقص التسليم:** تُفرض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). Нанесение ударов и ударов с перезарядкой и рубящих ударов
  бухгалтерская книга تبقى حتمية.

## تنفيذ

Вот пример замачивания в сказке:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Футбольный клуб "Спорт-Сити" в матче с "Сент-Луис" и "Кейлсон Уиллс" - расписание матчей خارجية.

## عرصد

يعرض Torii الآن لقطات رصيد المزوّدين جنبًا إلى جنب ععرض المزيد لوحات المتابعة
В случае с пенальти:

- REST: `GET /v1/sorafs/capacity/state` в режиме `credit_ledger[*]`
  Создал бухгалтерскую книгу, в которой он находится в замачивании. راجع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana импорт: `dashboards/grafana/sorafs_capacity_penalties.json` يرسم
  Скарлетт наносит удар
  Базовые линии, которые используются в работе, впитывают изменения.

## المتابعة

- Замачивание в CI-лабораторном шкафу для замачивания (дымовой уровень).
- Запустите Grafana, чтобы очистить Torii, и включите телеметрию.