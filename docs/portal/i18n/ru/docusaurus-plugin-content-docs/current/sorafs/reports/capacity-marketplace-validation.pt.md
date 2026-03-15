---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Validacao do mercado de capacidade SoraFS
тэги: [SF-2c, приемка, контрольный список]
Краткое описание: Контрольный список согласования действий с поставщиками, потоками споров и согласованием тех вопросов, которые позволяют освободить общую торговую способность SoraFS.
---

# Контрольный список проверки емкости магазина SoraFS

**Жанела де ревизао:** 18 марта 2026 г. -> 24 марта 2026 г.  
**Ответы на программу:** Группа хранения (`@storage-wg`), Совет управления (`@council`), Гильдия казначейства (`@treasury`)  
**Эскопо:** Каналы регистрации поставщиков, потоки судебных решений по спорам и процессы согласования всех необходимых материалов для GA SF-2c.

Контрольный список должен быть пересмотрен перед использованием или торговлей для внешних операций. Cada linha conecta evidencias deterministicas (тесты, приспособления или документация), которые аудиторы могут воспроизвести.

## Контрольный список асейтакао

### Регистрация поставщиков

| Чекагем | Валидакао | Эвиденсия |
|-------|------------|----------|
| Канонические объявления о регистрации | При проверке интеграции `/v1/sorafs/capacity/declare` через API приложения выполните проверку операций удаления, захват метаданных и передачу данных для узла реестра. | `crates/iroha_torii/src/routing.rs:7654` |
| Смарт-контракт изменяет полезные нагрузки | Единая гарантия, что идентификаторы провайдера и кампуса GiB будут компрометированы в соответствии с объявленным подтверждением до сохранения. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI выдает канонические артефакты адаптации | Используйте CLI, чтобы получить детерминированные данные Norito/JSON/Base64 и валидацию туда и обратно, чтобы операторы могли подготовить объявления в автономном режиме. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Помощь оператору в рабочем процессе приема и ограждениях управления | В документе перечислены схемы деклараций, политики по умолчанию и разрешения на пересмотр для совета. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Чекагем | Валидакао | Эвиденсия |
|-------|------------|----------|
| Реестр постоянных споров с каноническим дайджестом полезной нагрузки | Унитарная регистрация в споре, расшифровка или зашифрованная полезная нагрузка и подтверждение статуса в ожидании для гарантии детерминированности реестра. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| В ходе споров CLI соответствует канонической схеме | Если вы тестируете CLI в формате Base64/Norito и резюме JSON для `CapacityDisputeV1`, вы можете гарантировать, что пакеты доказательств детерминированы по хэшу Tenham. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Тест повтора доказывает детерминизм спора/наказания | Телеметрия доказательства-неудачи воспроизводится в двух случаях, когда создаются снимки идентификаторов бухгалтерской книги, кредитов и споров, которые разрезают все детерминированные узлы между узлами. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Документация по Runbook или потоку повышения и отзыва | Руководство по захвату или рабочему процессу совета, реквизиты доказательств и процедуры отката. | `../dispute-revocation-runbook.md` |

### Reconciliacao do tesouro| Чекагем | Валидакао | Эвиденсия |
|-------|------------|----------|
| Начисления в бухгалтерской книге соответствуют прогнозу на 30 дней | Если вы хотите выбрать пять поставщиков из 30 расчетных счетов, сравните входы в бухгалтерскую книгу со ссылкой на выплаты. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Выверка экспортной книги и регистрация каждый день | `capacity_reconcile.py` сравнивает ожидаемые комиссии с реестром комиссий с экспортом выполненных операций XOR, выдает метрики Prometheus и выполняет этап подтверждения запроса через Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Информационные панели выставления счетов и телеметрии начислений | Импортируйте график Grafana или начисление ГиБ-часов, контрагентов по забастовкам и залоговое обеспечение для наблюдения по вызову. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Публичный архив с методологией замачивания и командами воспроизведения | В отношении подробного описания или поиска, команды выполнения и крючки для наблюдения для аудиторов. | `./sf2c-capacity-soak.md` |

## Уведомления об исполнении

Повторно выполните набор действий перед подписанием:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторы должны регенерировать полезные данные запроса на регистрацию/диспута с помощью `sorafs_manifest_stub capacity {declaration,dispute}` и архивировать байты JSON/Norito, полученные в результате объединения с билетом управления.

## Артефатос де апровакао

| Артефато | Каминьо | blake2b-256 |
|----------|------|-------------|
| Пакет подтверждения регистрации поставщиков | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрешения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Соглашение о подтверждении примирения людей | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Охраняйте копии убитых артефактов как связку освобождения и винкуле - как отсутствие регистрации муниципальных властей.

## Апровакоэс

- Руководитель группы хранения данных - @storage-tl (24 марта 2026 г.)  
- Секретарь Совета управления - @council-sec (24 марта 2026 г.)  
- Руководитель казначейских операций - @treasury-ops (24 марта 2026 г.)