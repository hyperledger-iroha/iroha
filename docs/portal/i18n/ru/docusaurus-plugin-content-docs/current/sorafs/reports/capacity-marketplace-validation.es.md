---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Validacion del mercado de capacidad SoraFS
тэги: [SF-2c, приемка, контрольный список]
Краткое описание: Контрольный список принятия решений о привлечении поставщиков, разрешении споров и согласовании вопросов, связанных с общей диспонибилизацией торгового потенциала SoraFS.
---

# Список проверки подлинности товара на складе SoraFS

**Вентана ревизии:** 18 марта 2026 г. -> 24 марта 2026 г.  
**Ответственные за программу:** Группа хранения (`@storage-wg`), Совет управления (`@council`), Гильдия казначейства (`@treasury`)  
**Alcance:** Каналы регистрации поставщиков, процессы разрешения споров и процедуры согласования документов, необходимые для GA SF-2c.

Контрольный список необходимо пересмотреть перед освоением рынка для внешних операторов. Cada fila enlaza evidencia Deterministica (тесты, приспособления или документация), которые могут быть воспроизведены аудиторами.

## Контрольный список принятия

### Регистрация поставщиков

| Чекео | Проверка | Эвиденсия |
|-------|------------|----------|
| Реестр принимает канонические объявления о возможностях | Тест интеграции с кодом `/v2/sorafs/capacity/declare` через API приложения, проверка управления фирмой, захват метаданных и передача данных в реестр узла. | `crates/iroha_torii/src/routing.rs:7654` |
| Смарт-контракт загружает полезные нагрузки опресняно | Унитарный тест гарантирует, что идентификаторы провайдера и компрометированные кампусы GiB совпадают с заявлением фирмы до сохранения. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI создает канонические артефакты адаптации | В жгуте CLI описываются детерминированные Norito/JSON/Base64 и валидация туда и обратно, чтобы операторы могли подготовить объявления в автономном режиме. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Руководство оператора захватывает входной проход и ограждения губернатора | В документации перечислены вопросы декларации, политики по умолчанию и правила пересмотра для совета. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Чекео | Проверка | Эвиденсия |
|-------|------------|----------|
| Сохраняются реестры споров с каноническим дайджестом полезной нагрузки | Унитарная тестовая регистрация для спора, расшифровка полезной нагрузки и подтверждение статуса в ожидании, чтобы гарантировать детерминированность реестра. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Генератор споров CLI совпадает с канонической книгой | Тестирование куба CLI в формате Base64/Norito и возобновление JSON для `CapacityDisputeV1` гарантирует, что пакеты доказательств имеют хэш-форму детерминированной формы. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Тест повтора проверяет детерминизм спора/наказания | Телеметрия подтверждения-отказа воспроизводится каждый раз, создавая снимки идентификаторов бухгалтерской книги, кредитов и споров, чтобы косые черты были определены между узлами. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Документация по маршруту запуска и отзыва | Руководство по выполнению операций по захвату маршрута совета, реквизитам доказательств и процедурам отката. | `../dispute-revocation-runbook.md` |### Согласие по тесорерии

| Чекео | Проверка | Эвиденсия |
|-------|------------|----------|
| Накопление бухгалтерской книги совпадает с выдержкой за 30 дней | Воспользуйтесь тестом для пяти поставщиков в 30 расчетных отчетах, сравните входы в бухгалтерскую книгу со ссылками на выплаты. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Согласование по экспорту бухгалтерской книги при регистрации каждый день | `capacity_reconcile.py` сравнивает ожидаемые данные из реестра комиссий с экспортированными выбросами XOR, выдает метрики Prometheus и получает подтверждение проверки через Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Панели выставления счетов, штрафы и телеметрия накопления | Импорт Grafana графического накопления ГиБ в час, контрадоры забастовок и залогового обеспечения для видимости по вызову. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Публикация отчета в архиве методологии замачивания и команд воспроизведения | В отчете подробно описаны места для замачивания, команды выброса и крючки для наблюдения для аудиторов. | `./sf2c-capacity-soak.md` |

## Уведомления об изгнании

Повторите набор действий перед подписанием:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторы должны регенерировать полезные данные запроса на подключение/спора с `sorafs_manifest_stub capacity {declaration,dispute}` и архивировать байты JSON/Norito, полученные вместе с билетом губернатора.

## Артефакты апробации

| Артефакт | Рута | blake2b-256 |
|----------|------|-------------|
| Пакет апробации регистрации поставщиков | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрешения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Пакет одобрения примирения по делу | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Берегите копии фирм этих артефактов с пакетом выпуска и помещайте их в реестр кадров правительства.

## Апробасьоны

- Руководство по хранению оборудования — @storage-tl (24 марта 2026 г.)  
- Секретарь Совета управления — @council-sec (24 марта 2026 г.)  
- Руководитель операций Tesoreria — @treasury-ops (24 марта 2026 г.)