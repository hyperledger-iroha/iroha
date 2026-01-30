---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Валидация маркетплейса емкости SoraFS
tags: [SF-2c, acceptance, checklist]
summary: Чеклист приемки, покрывающий онбординг providers, потоки споров и сверку казначейства, которые закрывают готовность маркетплейса емкости SoraFS к GA.
---

# Чеклист валидации маркетплейса емкости SoraFS

**Окно проверки:** 2026-03-18 -> 2026-03-24  
**Владельцы программы:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Область:** Пайплайны онбординга providers, потоки рассмотрения споров и процессы сверки казначейства, необходимые для GA SF-2c.

Чеклист ниже должен быть проверен до включения маркетплейса для внешних операторов. Каждая строка ссылается на детерминированные доказательства (tests, fixtures или документацию), которые аудиторы могут воспроизвести.

## Чеклист приемки

### Онбординг providers

| Проверка | Валидация | Доказательство |
|-------|------------|----------|
| Registry принимает канонические декларации емкости | Интеграционный тест выполняет `/v1/sorafs/capacity/declare` через app API, проверяя обработку подписей, захват metadata и передачу в registry ноды. | `crates/iroha_torii/src/routing.rs:7654` |
| Smart contract отклоняет несовпадающие payloads | Unit-тест гарантирует, что IDs providers и поля committed GiB совпадают с подписанной декларацией перед сохранением. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI выпускает канонические artefacts онбординга | CLI harness пишет детерминированные Norito/JSON/Base64 выходы и валидирует round-trips, чтобы операторы могли готовить декларации offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Руководство оператора описывает workflow приема и guardrails управления | Документация перечисляет схему декларации, defaults policy и шаги ревью для council. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Проверка | Валидация | Доказательство |
|-------|------------|----------|
| Записи споров сохраняются с каноническим digest payload | Unit-тест регистрирует спор, декодирует сохраненный payload и подтверждает статус pending для гарантии детерминизма ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Генератор споров CLI соответствует канонической схеме | CLI тест покрывает Base64/Norito выходы и JSON-сводки для `CapacityDisputeV1`, гарантируя детерминированный hash evidence bundles. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay-тест доказывает детерминизм споров/пенализаций | Telemetry proof-failure, воспроизведенная дважды, дает идентичные snapshots ledger, credit и dispute, чтобы slashes были детерминированны между peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook документирует поток эскалации и ревокации | Операционное руководство фиксирует workflow council, требования к доказательствам и процедуры rollback. | `../dispute-revocation-runbook.md` |

### Сверка казначейства

| Проверка | Валидация | Доказательство |
|-------|------------|----------|
| Accrual ledger совпадает с 30-дневной проекцией soak | Soak тест охватывает пять providers за 30 окон settlement, сравнивая записи ledger с ожидаемой референсной выплатой. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Сверка экспорта ledger записывается каждую ночь | `capacity_reconcile.py` сравнивает ожидания fee ledger с выполненными XOR export, публикует метрики Prometheus и gate-ит одобрение казначейства через Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Billing dashboards показывают пенализации и telemetry accrual | Импорт Grafana строит accrual GiB-hour, счетчики strikes и bonded collateral для on-call видимости. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Опубликованный отчет архивирует методологию soak и команды replay | Отчет описывает объем soak, команды выполнения и observability hooks для аудиторов. | `./sf2c-capacity-soak.md` |

## Примечания по выполнению

Перезапустите набор проверок перед sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторы должны заново сгенерировать payloads запросов онбординга/споров через `sorafs_manifest_stub capacity {declaration,dispute}` и архивировать полученные JSON/Norito bytes вместе с governance ticket.

## Артефакты sign-off

| Артефакт | Path | blake2b-256 |
|----------|------|-------------|
| Пакет одобрения онбординга providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрешения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Пакет одобрения сверки казначейства | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Храните подписанные копии этих артефактов вместе с release bundle и укажите ссылки в реестре изменений управления.

## Подписи

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
