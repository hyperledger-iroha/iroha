---
id: payment-settlement-plan
lang: ru
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# План платежей и расчетов SNS

> Канонический источник: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Задача roadmap **SN-5 -- Payment & Settlement Service** вводит детерминированный
платежный слой для Sora Name Service. Каждая регистрация, продление или возврат
должны выпускать структурированный Norito payload, чтобы казначейство, stewards
и управление могли воспроизводить финансовые потоки без таблиц. Эта страница
сжимает спецификацию для аудитории портала.

## Модель дохода

- Базовая комиссия (`gross_fee`) берется из матрицы цен регистратора.
- Казначейство получает `gross_fee x 0.70`, stewards получают остаток за вычетом
  referral бонусов (лимит 10 %).
- Опциональные holdbacks позволяют управлению приостанавливать выплаты steward
  во время споров.
- Settlement bundles раскрывают блок `ledger_projection` с конкретными ISI
  `Transfer`, чтобы автоматизация могла отправлять движения XOR прямо в Torii.

## Сервисы и автоматизация

| Компонент | Назначение | Доказательство |
|-----------|-----------|----------------|
| `sns_settlementd` | Применяет политику, подписывает bundles, публикует `/v2/sns/settlements`. | JSON bundle + hash. |
| Settlement queue & writer | Идемпотентная очередь + отправитель ledger, управляемый `iroha_cli app sns settlement ledger`. | bundle hash <-> tx hash manifest. |
| Reconciliation job | Ежедневный diff + ежемесячный отчет под `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | Возвраты, одобренные управлением через `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

CI helper'ы отражают эти потоки:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Наблюдаемость и отчетность

- Дашборды: `dashboards/grafana/sns_payment_settlement.json` для итогов
  казначейства vs stewards, referral выплат, глубины очереди и задержки возвратов.
- Алерты: `dashboards/alerts/sns_payment_settlement_rules.yml` отслеживает возраст
  pending, сбои reconciliation и дрейф ledger.
- Отчеты: ежедневные digests (`settlement_YYYYMMDD.{json,md}`) сворачиваются в
  ежемесячные отчеты (`settlement_YYYYMM.md`), которые загружаются в Git и в
  объектное хранилище управления (`s3://sora-governance/sns/settlements/<period>/`).
- Governance packets объединяют дашборды, логи CLI и approvals перед sign-off
  совета.

## Чеклист развертывания

1. Прототипировать quote + ledger helpers и захватить staging bundle.
2. Запустить `sns_settlementd` с queue + writer, подключить дашборды и выполнить
   alert тесты (`promtool test rules ...`).
3. Доставить refund helper и шаблон ежемесячного отчета; отразить артефакты в
   `docs/portal/docs/sns/reports/`.
4. Провести партнерскую репетицию (полный месяц settlements) и зафиксировать
   голосование управления, отмечающее завершение SN-5.

Вернитесь к исходному документу за точными определениями схемы, открытыми
вопросами и будущими изменениями.
