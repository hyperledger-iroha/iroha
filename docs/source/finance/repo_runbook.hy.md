---
lang: hy
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo Settlement Runbook

Այս ուղեցույցը ներկայացնում է Iroha-ում ռեպո և հակադարձ ռեպո համաձայնագրերի որոշիչ հոսքը:
Այն ընդգրկում է CLI նվագախումբը, SDK օգնականները և սպասվող կառավարման կոճակները, որպեսզի օպերատորները կարողանան
նախաձեռնել, սահմանել և լուծարել համաձայնությունները՝ առանց Norito հումքային բեռներ գրելու: Կառավարման համար
ստուգաթերթերը, ապացույցների հավաքագրումը և խարդախության/վերադարձի ընթացակարգերը տես
[`repo_ops.md`](./repo_ops.md), որը բավարարում է ճանապարհային քարտեզի F1 կետին:

## CLI հրամաններ

`iroha app repo` հրամանը խմբավորում է ռեպո-հատուկ օգնականներ.

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset usd#wonderland \
  --cash-quantity 1000 \
  --collateral-asset bond#wonderland \
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
  --cash-asset usd#wonderland \
  --cash-quantity 1005 \
  --collateral-asset bond#wonderland \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` և `repo unwind` հարգում են `--input/--output`-ը, այնպես որ ստեղծված `InstructionBox`
  օգտակար բեռները կարող են խողովակաշարով մտցնել այլ CLI հոսքեր կամ անմիջապես ներկայացվել:
* Անցեք `--custodian <account>`՝ գրավը եռակողմ պահառուին փոխանցելու համար: Բաց թողնելու դեպքում,
  Կողմնակիցը գրավը ստանում է ուղղակիորեն (երկկողմանի ռեպո):
* `repo margin`-ը հարցում է անում մատյանում `FindRepoAgreements`-ի միջոցով և հայտնում հաջորդ սպասվող մարժան
  ժամանակի դրոշմակնիք (միլիվայրկյաններով) կողքին, թե արդյոք մարժայի հետ կանչը ներկայումս ենթակա է:
* `repo margin-call`-ը ավելացնում է `RepoMarginCallIsi` հրահանգը՝ գրանցելով սահմանային անցակետը և
  իրադարձությունների թողարկում բոլոր մասնակիցների համար: Զանգերը մերժվում են, եթե կադենսը չի անցել կամ եթե
  հրահանգը ներկայացվում է չմասնակցի կողմից:

## Python SDK օգնականներ

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

cash = RepoCashLeg(asset_definition_id="usd#wonderland", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="bond#wonderland",
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

* Երկու օգնականներն էլ նորմալացնում են թվային քանակությունները և մետատվյալների դաշտերը՝ նախքան PyO3 կապակցումները կանչելը:
* `RepoAgreementRecord`-ը արտացոլում է գործարկման ժամանակացույցի հաշվարկը, այնպես որ մատյանից դուրս ավտոմատացումը կարող է
  որոշել, թե երբ են սպասվում հետզանգերը՝ առանց ձեռքով վերահաշվարկի:

## DvP / PvP բնակավայրեր

`iroha app settlement` հրամանը փուլ առնում է առաքում ընդդեմ վճարման և վճարում ընդդեմ վճարման հրահանգները.

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset bond#wonderland \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset usd#wonderland \
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
  --primary-asset usd#wonderland \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset eur#wonderland \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* Ոտքի քանակները ընդունում են ամբողջական կամ տասնորդական արժեքներ և վավերացվում են ակտիվի ճշգրտության համեմատ:
* `--atomicity` ընդունում է `all-or-nothing`, `commit-first-leg` կամ `commit-second-leg`: Օգտագործեք այս ռեժիմները
  `--order`-ով արտահայտելու համար, թե որ ոտքը կմնա հավատարիմ, եթե հետագա մշակումը ձախողվի (`commit-first-leg`
  պահում է առաջին ոտքը կիրառված; `commit-second-leg`-ը պահպանում է երկրորդը):
* CLI կանչերն այսօր թողարկում են դատարկ հրահանգների մետատվյալներ. օգտագործեք Python-ի օգնականները կարգավորման մակարդակում
  մետատվյալները պետք է կցվեն:
* Տես [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ISO 20022 դաշտի քարտեզագրման համար, որը
  աջակցում է այս հրահանգներին (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`):
* Անցեք `--iso-xml-out <path>`, որպեսզի CLI-ն թողարկի կանոնական XML նախադիտում Norito-ի կողքին:
  հրահանգ; ֆայլը հետևում է վերը նշված քարտեզագրմանը (`sese.023` DvP-ի համար, `sese.025`՝ PvP-ի համար): Զուգավորել
  դրոշակավորել `--iso-reference-crosswalk <path>`-ով, որպեսզի CLI-ն ստուգի `--delivery-instrument-id`-ը
  նույն պատկերը Torii-ն օգտագործում է աշխատանքի ընդունման ժամանակ:

Python-ի օգնականները արտացոլում են CLI մակերեսը.

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
    asset_definition_id="bond#wonderland",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="usd#wonderland",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="usd#wonderland",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="eur#wonderland",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## Դետերմինիզմ և կառավարման ակնկալիքներ

Repo-ի հրահանգները հիմնված են բացառապես Norito կոդավորված թվային տեսակների և ընդհանուր տվյալների վրա
`RepoGovernance::with_defaults` տրամաբանություն. Նկատի ունեցեք հետևյալ անփոփոխությունները.* Քանակները սերիականացված են `NumericSpec` որոշիչ արժեքներով. կանխիկի ոտքերի օգտագործում
  `fractional(2)` (երկու տասնորդական տեղ), գրավի ոտքերը օգտագործում են `integer()`: Չեն հանձնել
  արժեքները ավելի մեծ ճշգրտությամբ. գործարկման ժամանակի պահակները կմերժեն դրանք, իսկ հասակակիցները կտարվեն:
* Եռակողմ ռեպոները պահպանում են պահառուի հաշվի ID-ն `RepoAgreement`-ում: Կյանքի ցիկլի և մարժայի իրադարձություններ
  թողարկեք `RepoAccountRole::Custodian` օգտակար բեռ, որպեսզի պահառուները կարողանան բաժանորդագրվել և համապատասխանեցնել գույքագրումը:
* Սանրվածքները սեղմված են մինչև 10000 bps (100%), իսկ լուսանցքի հաճախականությունը ամբողջ վայրկյան է: Տրամադրել
  Կառավարման պարամետրերն այդ կանոնական միավորներում, որպեսզի համահունչ մնան գործարկման ժամանակի ակնկալիքներին:
* Ժամային դրոշմանիշները միշտ ունիքս միլիվայրկյան են: Բոլոր օգնականները դրանք փոխանցում են Norito-ին անփոփոխ
  ծանրաբեռնվածություն, որպեսզի հասակակիցները ստանան նույնական գրաֆիկները:
* Սկսելու և արձակելու հրահանգները նորից օգտագործում են նույն պայմանագրի նույնացուցիչը: Գործարկման ժամանակը մերժվում է
  կրկնօրինակել ID-ները և անջատվել անհայտ համաձայնագրերի համար. CLI/SDK-ի օգնականները վաղ բացահայտում են այդ սխալները:
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` վերադարձնում է կանոնական կադենսը: Միշտ
  Նախքան հետադարձ զանգի գործարկումը, ծանոթացեք այս նկարին՝ հնացած ժամանակացույցերի կրկնությունից խուսափելու համար: