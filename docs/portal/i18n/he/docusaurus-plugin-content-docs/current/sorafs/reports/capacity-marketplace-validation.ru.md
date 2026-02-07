---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Валидация маркетплейса емкости SoraFS
תגיות: [SF-2c, קבלה, רשימת תיוג]
תקציר: Чеклист приемки, покрывающий онбординг, потоки споров и сверку казначейства, которые закрываю маркетплейса емкости SoraFS к GA.
---

# Чеклист валидации маркетплейса емкости SoraFS

**Окно проверки:** 2026-03-18 -> 2026-03-24  
**Владельцы программы:** צוות אחסון (`@storage-wg`), מועצת ממשל (`@council`), גילדת האוצר (`@treasury`)  
**מוסמך:** ספקי שירותים אופנתיים, ציוד שירותים ושירותי שירותים, необходиGA, необходиGA.

Чеклист ниже должен быть проверен до включения маркетплейса для внешних операторов. Каждая строка ссылается на детерминированные доказательства (בדיקות, מתקנים או מסמכים), קוטגוריות аудоститоры.

## Чеклист приемки

### ספקי אומנות

| Проверка | Валидация | Доказательство |
|-------|----------------|--------|
| רישום принимает канонические декларации емкости | Интеграционный тест выполняет `/v1/sorafs/capacity/declare` משתמש ב-API של אפליקציית האפליקציה, הורד שרת, הורד מטא נתונים ורישום רישום. | `crates/iroha_torii/src/routing.rs:7654` |
| חוזה חכם отклоняет несовпадающие מטענים | יחידה מתחייבת, בין ספקי תעודות זהות ופלגות מחויבים GiB совпадают с подписанной декларацией перед сохранением. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI выпускает канонические חפצי אמנות онбординга | רתמת ה-CLI пишет детерминированные Norito/JSON/Base64 מאפשרת נסיעות הלוך ושוב, מבצעים מערכות הפעלה במצב לא מקוון. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Руководство оператора описывает זרימת עבודה приема и מעקות בטיחות управления | Документация перечисляет схему декларации, מדיניות ברירת מחדל и шаги ревью למועצה. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Проверка | Валидация | Доказательство |
|-------|----------------|--------|
| Записи споров сохраняются с каноническим לעכל מטען | יחידה-טרמינציה של רכיב, מטען סוקרוני ופרטי אחסון ממתינים לניהול ספר חשבונות. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Генератор споров CLI соответствует канонической схеме | CLI тест покрывает Base64/Norito выходы ו-JSON-сводки ל-`CapacityDisputeV1`, гарантируя детерминированный חבילות הוכחות hash. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay-тест доказывает детерминизм споров/пенализаций | טלמטריה הוכחה-כשל, воспроизведенная дважды, дает идентичные פנקס צילומי מצב, אשראי ומחלוקת, чтобы slashes были детерминированны между. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook документирует поток эскалации и ревокации | Операционное руководство фиксирует מועצת זרימת העבודה, требования к доказательствам и процедуры החזרה לאחור. | `../dispute-revocation-runbook.md` |

### Сверка казначейства| Проверка | Валидация | Доказательство |
|-------|----------------|--------|
| ספר חשבונות צבירה совпадает с 30-дневной проекцией ספיגה | להשרות тест охватывает пять ספקים за 30 окон הסדר, сравнивая записи חשבונות с ожидаемой референсной выплатой. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| ספר חשבונות ספר ראשי | `capacity_reconcile.py` פנקס אגרות של מדינת ישראל עם יצוא XOR, ציוד מתקנים Prometheus ו-gate-изочень מנהל התראה. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| לוחות מחוונים לחיוב показывают пенализации וצבירת טלמטריה | Импорт Grafana שיטת צבירה של GiB-hour, תקלות ובטחונות מלוכדים לשירותי כוננות. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Опубликованный отчет архивирует методологию לספוג וקומאנדы שידור חוזר | ניתן לספוג את הציוד, להשרות ציוד ולראות ווים לעזרה. | `./sf2c-capacity-soak.md` |

## Примечания по выполнению

Перезапустите набор проверок перед חתימה:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторы должны заново сгенерировать loadloads запросов онбординга/споров через `sorafs_manifest_stub capacity {declaration,dispute}` ו архивировать полун0ч/споров bytes вместе с כרטיס ממשל.

## חתימה על אמנות

| Артефакт | נתיב | blake2b-256 |
|--------|------|--------|
| ספקי Пакет одобрения онбординга | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрешения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Пакет одобрения сверки казначейства | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

פתחו את חבילת הפרסום של חבילת השחרור או התקנות הרשאות.

## פרטים

- ראש צוות אחסון - @storage-tl (2026-03-24)  
- מזכיר מועצת הממשל - @council-sec (2026-03-24)  
- מוביל תפעול משרד האוצר - @treasury-ops (2026-03-24)