---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: deal-engine
title: Движок сделок SoraFS
sidebar_label: Движок сделок
description: Обзор движка сделок SF-8, интеграции Torii и телеметрических поверхностей.
---

:::note Канонический источник
:::

# Движок сделок SoraFS

Роадмап SF-8 вводит движок сделок SoraFS, обеспечивающий
детерминированный учет соглашений на хранение и извлечение между
клиентами и провайдерами. Соглашения описываются Norito payloads,
определенными в `crates/sorafs_manifest/src/deal.rs`, покрывая условия сделки,
блокировку bonds, вероятностные микроплатежи и записи расчетов.

Встроенный SoraFS worker (`sorafs_node::NodeHandle`) теперь создает
экземпляр `DealEngine` для каждого процесса узла. Движок:

- валидирует и регистрирует сделки через `DealTermsV1`;
- начисляет charges в XOR при отчетах об использовании репликации;
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  sampling на основе BLAKE3; и
- формирует snapshots ledger и payloads расчетов, пригодные для публикации в governance.

Юнит-тесты покрывают валидацию, выбор микроплатежей и расчетные потоки, чтобы
операторы могли уверенно проверять API. Расчеты теперь выпускают governance payloads
`DealSettlementV1`, напрямую подключаясь к pipeline публикации SF-12, и обновляют серию
OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) для дашбордов Torii и
SLO enforcement. Следующие шаги фокусируются на автоматизации slashing, инициируемой
аудиторами, и согласовании семантики отмены с политикой governance.

Телеметрия использования также питает набор метрик `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, а также счетчики билетов
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эти totals раскрывают вероятностный
лотерейный поток, чтобы операторы могли коррелировать выигрыши микроплатежей и
перенос кредитов с результатами расчетов.

## Интеграция Torii

Torii предоставляет выделенные endpoints, чтобы провайдеры могли отправлять usage и вести
жизненный цикл сделок без специального wiring:

- `POST /v1/sorafs/deal/usage` принимает телеметрию `DealUsageReport` и возвращает
  детерминированные результаты учета (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` завершает текущий window, стримя
  итоговый `DealSettlementRecord` вместе с base64-кодированным `DealSettlementV1`,
  готовым к публикации в governance DAG.
- Лента Torii `/v1/events/sse` теперь транслирует записи `SorafsGatewayEvent::DealUsage`,
  суммирующие каждую отправку usage (epoch, измеренные GiB-hours, счетчики билетов,
  детерминированные charges), записи `SorafsGatewayEvent::DealSettlement`,
  включающие канонический snapshot ledger расчетов плюс BLAKE3 digest/size/base64
  governance артефакта на диске, и алерты `SorafsGatewayEvent::ProofHealth` при превышении
  порогов PDP/PoTR (провайдер, окно, состояние strike/cooldown, сумма штрафа). Потребители могут
  фильтровать по провайдеру, чтобы реагировать на новую телеметрию, расчеты или proof-health алерты без polling.

Оба endpoints участвуют в SoraFS quota framework через новое окно
`torii.sorafs.quota.deal_telemetry`, позволяя операторам настраивать допустимую
частоту отправки для каждого деплоя.
