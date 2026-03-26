---
lang: ba
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Репо ҡасаба

Был ҡулланма документлаштырыу өсөн детерминистик ағым өсөн репо һәм кире-репо килешеп Iroha.
Ул CLI оркестрацияһын, SDK ярҙамсыларын һәм көтөлгән идара итеү ручкаларын ҡаплай, шуға күрә операторҙар
инициаторы, маржа, һәм сеймал яҙмайынса, маржа һәм сеймал Norito файҙалы йөктәр. Идара итеү өсөн
тикшерелгән исемлектәр, дәлилдәр тотоу, һәм мутлыҡ/кире кире ҡайтарыу процедуралары күрә
[`repo_ops.md`] (./repo_ops.md), был юл картаһы пунктын ҡәнәғәтләндерә F1.

## CLI командалары

`iroha app repo` команда төркөмдәре репо-конкрет ярҙамсылар:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
  --custodian soraカタカナ... \
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
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
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

* `repo initiate` һәм `repo unwind` хөрмәт `--input/--output` шулай итеп генерацияланған `InstructionBox`
  файҙалы йөктәрҙе башҡа CLI ағымдарына торбаларға йәки шунда уҡ тапшырырға мөмкин.
* Pass `--custodian <account>` өс яҡлы һаҡсыға залог маршрутлаштырыу өсөн. Ҡасан төшөрөп ҡалдырылған, был
  контрагент туранан-тура вәғәҙәне ала (ике яҡлы репо).
* `repo margin` эҙләүҙәр 18NI000000020X аша баш китабы һәм сираттағы көтөлгән маржа тураһында хәбәр итә
  ваҡыт маркаһы (миллисекундтарҙа) әлеге ваҡытта маржа шылтыратыуының тейешме-юҡмы икәнлеген бер рәттән.
* `repo margin-call` `RepoMarginCallIsi` инструкцияһын өҫтәй, маржа тикшерелеү пунктын теркәй һәм
  ҡатнашыусылар өсөн саралар сығарыу. Шылтыратыуҙар кире ҡағыла, әгәр каденция үткән йәки әгәр
  инструкцияны ҡатын-ҡыҙһыҙ кеше тапшыра.

## Python SDK ярҙамсылары

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

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="soraカタカナ...",
    counterparty="soraカタカナ...",
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

* Ике ярҙамсы ла һанлы күләмдәр һәм метамағлүмәт ҡырҙарын PyO3 бәйләүҙәрен саҡырғансы нормаға килтерә.
* `RepoAgreementRecord` көҙгө графигы иҫәпләүен иҫәпләү шулай офф-леджер автоматлаштырыу мөмкин
  ҡасан шылтыратыуҙар тейеш, тип билдәләне, каденцияны ҡул менән ҡабаттан иҫәпләмәйенсә.

## DvP / PvP ҡасабалары

`iroha app settlement` командаһы тапшырыу-ҡаршы-түләү һәм түләү-ҡаршы түләүле күрһәтмәләр этаптары:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from soraカタカナ... \
  --delivery-to soraカタカナ... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from soraカタカナ... \
  --payment-to soraカタカナ... \
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
  --primary-from soraカタカナ... \
  --primary-to soraカタカナ... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from soraカタカナ... \
  --counter-to soraカタカナ... \
  --iso-xml-out trade_pvp.xml
```

* Аяҡ күләме интеграль йәки унлыҡ ҡиммәттәрен ҡабул итә һәм актив теүәллегенә ҡаршы раҫлана.
* `--atomicity` ҡабул итә `all-or-nothing`, `commit-first-leg`, йәки `commit-second-leg`. Был режимдарҙы ҡулланығыҙ
  `--order` менән, әгәр ҙә артабан эшкәрткән етешһеҙлектәр булһа, ниндәй аяҡ ҡылғанын белдерергә (Norito
  беренсе аяҡты ҡуллана; `commit-second-leg` икенсеһен һаҡлай).
* CLI саҡырыуҙары бөгөн буш инструкция метамағлүмәттәрен сығара; ҡулланыу Python ярҙамсылары ҡасан ҡасаба кимәлендә
  метамағлүмәттәр беркетелергә тейеш.
* Ҡара: ISO 20022 ялан картаһы өсөн ISO
  backs these instructions (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Pass `--iso-xml-out <path>` CLI канонлы XML алдан ҡарау Norito менән бер рәттән сығарыу өсөн.
  өйрәтеү; файл өҫтәге картаға ярашлы (`sese.023` өсөн DvP, `sese.025` өсөн PvP`). Пар
  флаг менән `--iso-reference-crosswalk <path>` шулай CLI раҫлай `--delivery-instrument-id` ҡаршы .
  шул уҡ снимок Torii ҡулланыу ваҡытында ҡабул итеү ваҡытында.

Питон ярҙамсылары CLI өҫтөн көҙгөләй:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
)
``` X

## Детерминизм & идара итеү көтөүҙәре

Репо күрһәтмәләре тик Norito-кодланған һанлы типтағы һәм уртаҡлыҡҡа ғына таяна.
`RepoGovernance::with_defaults` логикаһы. Түбәндәге инварианттарҙы күҙ уңында тотоғоҙ:* Һандар менән сериализацияланған детерминистик `NumericSpec` ҡиммәттәре: аҡса аяҡтары ҡулланыу
  `fractional(2)` (ике унлыҡ урыны), залог аяҡтары `integer()`X ҡуллана. Тапшырыуҙы тапшырмағыҙ .
  ҡиммәттәре ҙурыраҡ теүәллек менән — йүгереүҙең һаҡсылары уларҙы кире ҡаға һәм тиҫтерҙәре айырылыр ине.
* Три-партия репоһы һаҡлана һаҡсы иҫәп id `RepoAgreement`. Йәшәү циклы һәм маржа ваҡиғалары
  `RepoAccountRole::Custodian` файҙалы йөктө сығарырға, шуға күрә һаҡсылар инвентаризация яҙыла һәм яраштыра ала.
* Стрижка 10000б/п (100%) тиклем ҡыҫыла, ә маржа йышлыҡтары тотош секунд. Тәьмин итергә
  идара итеү параметрҙары шул канонлы берәмектәрҙә эшләү өсөн тура килтереп, йөрөү ваҡыты өмөттәре.
* Ваҡыт тамғалары һәр ваҡыт бер тапҡыр миллисекунд. Бөтә ярҙамсылар ҙа уларҙы Norito-ҡа үҙгәртмәйенсә тапшыра.
  файҙалы йөк шулай тиҫтерҙәре бер үк графиктар ала.
* Инициация һәм ялһыҙ күрһәтмәләр бер үк килешәү идентификаторын ҡабаттан ҡулланырға. Эшләү ваҡыты кире ҡаға.
  дубликаты идентификаторҙар һәм билдәһеҙ килешелгән өсөн сисергә; CLI/SDK ярҙамсылары был хаталарҙы иртә сығара.
* `repo margin` X/`RepoAgreementRecord::next_margin_check_after` канон каденцияһын ҡайтара. Һәр ваҡыт
  был снимок менән кәңәшләшергә тиклем триггер шылтыратыуҙар ҡотолоу өсөн реплей иҫке графиктар.