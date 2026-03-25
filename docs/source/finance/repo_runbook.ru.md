---
lang: ru
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Книга расчетов репо

В этом руководстве документирован детерминированный поток для соглашений репо и обратного репо в Iroha.
Он охватывает оркестровку CLI, помощники SDK и ожидаемые регуляторы управления, чтобы операторы могли
инициировать, обеспечивать маржу и расторгать соглашения без записи необработанных полезных данных Norito. Для управления
контрольные списки, сбор доказательств и процедуры мошенничества/отката см.
[`repo_ops.md`](./repo_ops.md), который соответствует пункту F1 дорожной карты.

## команды CLI

Команда `iroha app repo` группирует помощники, специфичные для репозитория:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` и `repo unwind` учитывают `--input/--output`, поэтому сгенерированный `InstructionBox`
  полезные данные могут быть переданы в другие потоки CLI или отправлены немедленно.
* Передайте `--custodian <account>`, чтобы направить обеспечение трехстороннему хранителю. Если опущено,
  контрагент получает залог напрямую (двустороннее репо).
* `repo margin` запрашивает реестр через `FindRepoAgreements` и сообщает о следующей ожидаемой марже.
  временная метка (в миллисекундах) вместе с указанием того, требуется ли в данный момент обратный вызов маржи.
* `repo margin-call` добавляет инструкцию `RepoMarginCallIsi`, записывающую контрольную точку маржи и
  создание событий для всех участников. Вызовы отклоняются, если каденция не истекла или если
  поручение подано неучастником.

## Помощники Python SDK

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="i105...",
    counterparty="i105...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* Оба помощника нормализуют числовые величины и поля метаданных перед вызовом привязок PyO3.
* `RepoAgreementRecord` отражает расчет графика выполнения, поэтому автоматизация вне реестра может
  определить, когда должны быть выполнены обратные вызовы, без пересчета частоты кадров вручную.

## DvP/PvP поселения

Команда `iroha app settlement` выполняет инструкции «доставка против платежа» и «платеж против платежа»:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from i105... \
  --payment-to i105... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* Количества ветвей принимают целые или десятичные значения и проверяются на соответствие точности актива.
* `--atomicity` принимает `all-or-nothing`, `commit-first-leg` или `commit-second-leg`. Используйте эти режимы
  с `--order`, чтобы указать, какая часть остается зафиксированной, если последующая обработка не удалась (`commit-first-leg`
  держит первую ногу прижатой; `commit-second-leg` сохраняет второе).
* Сегодня вызовы CLI выдают пустые метаданные инструкций; используйте помощники Python на уровне поселений
  метаданные необходимо прикрепить.
* См. [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) для сопоставления полей ISO 20022, которое
  поддерживает эти инструкции (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Передайте `--iso-xml-out <path>`, чтобы CLI выдавал канонический предварительный просмотр XML вместе с Norito.
  инструкция; файл соответствует приведенному выше сопоставлению (`sese.023` для DvP, `sese.025` для PvP`). Соедините
  флаг с `--iso-reference-crosswalk <path>`, чтобы CLI сверил `--delivery-instrument-id` с
  тот же моментальный снимок, который Torii использует при входе во время выполнения.

Помощники Python отражают поверхность CLI:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## Детерминизм и ожидания в области управления

Инструкции репо полагаются исключительно на числовые типы в кодировке Norito и общие
Логика `RepoGovernance::with_defaults`. Помните о следующих инвариантах:* Количества сериализуются с использованием детерминированных значений `NumericSpec`: использование денежных потоков.
  `fractional(2)` (два знака после запятой), в боковых частях используется `integer()`. Не отправлять
  значения с большей точностью — средства защиты во время выполнения отклонят их, и одноранговые узлы будут расходиться.
* Трехсторонние репозитории сохраняют идентификатор учетной записи хранителя в `RepoAgreement`. Жизненный цикл и маржинальные события
  выдать полезную нагрузку `RepoAccountRole::Custodian`, чтобы хранители могли подписаться и согласовать инвентарь.
* Стрижки ограничены до 10 000 бит/с (100%), а допустимые частоты составляют целые секунды. Предоставить
  параметры управления в этих канонических единицах, чтобы они соответствовали ожиданиям времени выполнения.
* Временные метки всегда указываются в миллисекундах Unix. Все помощники пересылают их без изменений на Norito.
  полезная нагрузка, чтобы одноранговые узлы получали идентичные расписания.
* Инструкции по инициированию и отмене повторно используют один и тот же идентификатор соглашения. Среда выполнения отклоняет
  дублировать удостоверения личности и раскручивать неизвестные соглашения; Помощники CLI/SDK выявляют эти ошибки на ранней стадии.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` возвращают канонический ритм. Всегда
  просмотрите этот снимок перед запуском обратных вызовов, чтобы избежать повторного воспроизведения устаревших расписаний.