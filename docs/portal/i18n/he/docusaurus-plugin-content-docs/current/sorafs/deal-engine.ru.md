---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: מנוע עסקה
כותרת: Движок сделок SoraFS
sidebar_label: Движок сделок
תיאור: Обзор движка сделок SF-8, интеграции Torii ו телеметрических поверхностей.
---

:::note Канонический источник
:::

# Движок сделок SoraFS

Роадмап SF-8 вводит движок сделок SoraFS, обеспечивающий
детерминированный учет соглашений на хранение и извлечение между
клиентами и провайдерами. Соглашения описываются Norito מטענים,
определенными в `crates/sorafs_manifest/src/deal.rs`, покрывая условия сделки,
אגרות חוב блокировку, вероятностные микроплатежи и записи расчетов.

Встроенный SoraFS עובד (`sorafs_node::NodeHandle`) теперь создает
экземпляр `DealEngine` для каждого процесса узла. Движок:

- валидирует и регистрирует сделки через `DealTermsV1`;
- חיובים начисляет в XOR при отчетах об использовании репликации;
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  דגימה на основе BLAKE3; и
- פורמט פנקס צילומי מצב ומטענים расчетов, пригодные для публикации в governance.

Юнит-тесты покрывают валидацию, выбор микроплатежей и расчетные потоки, чтобы
операторы могли уверенно проверять API. Расчеты теперь выпускают משאבי ממשל
`DealSettlementV1`, напрямую подключаясь к pipeline публикации SF-12, и обновляют серию
OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) עבור дашбордов Torii ו
אכיפת SLO. Следующие шаги фокусируются на автоматизации חיתוך, инициируемой
аудиторами, и согласовании семантики отмены с политикой ממשל.

Телеметрия использования также питает набор метрик `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, а также счетчики билетов
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эти סך הכל раскрывают вероятностный
лотерейный поток, чтобы операторы могли коррелировать выигрыши микроплатежей и
перенос кредитов с результатами расчетов.

## Интеграция Torii

Torii предоставляет выделенные נקודות קצה, чтобы провайдеры могли отправлять שימוש и вести
жизненный цикл сделок без специального חיווט:- `POST /v2/sorafs/deal/usage` принимает телеметрию `DealUsageReport` и возвращает
  детерминированные результаты учета (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` завершает текущий חלון, стримя
  итоговый `DealSettlementRecord` вместе с base64-кодированным `DealSettlementV1`,
  готовым к публикации в governance DAG.
- Лента Torii `/v2/events/sse` צור קשר עם `SorafsGatewayEvent::DealUsage`,
  суммирующие каждую отправку (תקופה, измеренные GiB-hours, счетчики билетов,
  חיובים детерминированные), записи `SorafsGatewayEvent::DealSettlement`,
  включающие канонический פנקס צילום מצב расчетов плюс BLAKE3 digest/size/base64
  ממשל артефакта на диске, и алерты `SorafsGatewayEvent::ProofHealth` при превышении
  порогов PDP/PoTR (провайдер, окно, состояние strike/cooldown, сумма штрафа). Потребители могут
  סרטון על הוכחה, הוכחה על טלפונים חדשים, או אלטרנטים הוכחה בריאות ללא סקרים.

Оба נקודות קצה участвуют в SoraFS מסגרת מכסה через новое окно
`torii.sorafs.quota.deal_telemetry`, позволяя операторам настраивать допустимую
частоту отправки для каждого деплоя.