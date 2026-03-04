---
lang: dz
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo གཞིས་ཆགས་རུན་དེབ།

ལམ་སྟོན་འདི་གིས་ Iroha ནང་ repo དང་ ཕྱི་འགྱུར་ Repo གན་ཡིག་ཚུ་གི་དོན་ལུ་ གཏན་འབེབས་ཀྱི་རྒྱུན་རིམ་འདི་ ཡིག་ཐོག་ལུ་བཀོད་དེ་ཡོདཔ་ཨིན།
འདི་གིས་ CLI གི་སྡེ་ཚན་དང་ SDK གྲོགས་རམ་པ་ དེ་ལས་ རེ་བ་བསྐྱེད་མི་ གཞུང་སྐྱོང་གི་ མཛུབ་མོ་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན།
Norito གི་དངུལ་ཕོགས་ཚུ་ མ་བྲིས་པར་ མཐའ་མཚམས་ དེ་ལས་ གན་ཡིག་ཚུ་ འགོ་བཙུགསཔ་ཨིན། གཞུང་སྐྱོང་གི་དོན་ལུ་
བརྟག་ཞིབ་ཐོ་ཡིག་དང་ སྒྲུབ་བྱེད་འཛིན་བཟུང་ དེ་ལས་ གཡོ་སྒྱུ་/རྒྱབ་ལོག་བྱ་རིམ་ཚུ་ མཐོངམ་ཨིན།
[`repo_ops.md`](./repo_ops.md) གིས་ ལམ་གྱི་ས་ཁྲ་དངོས་པོ་ F1 བསྒྲུབ་ཐུབ།

## CLI བཀའ་བཀོད།

`iroha app repo` བརྡ་བཀོད་སྡེ་ཚན་ repoདམིགས་བསལ་གྱི་གྲོགས་རམ་པ་ཚུ།

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator ih58... \
  --counterparty ih58... \
  --custodian ih58... \
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
  --initiator ih58... \
  --counterparty ih58... \
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

* `repo initiate` དང་ `repo unwind` གིས་ `--input/--output` དེ་ལུ་བརྟེན་ གསར་བཏོན་འབད་མི་ `InstructionBox`
  པེ་ལོཌི་ཚུ་ གཞན་མི་སི་ཨེལ་ཨའི་ རྒྱུན་འགྲུལ་ནང་ ཡང་ན་ དེ་འཕྲོ་ལས་ བཙུགས་ཚུགས།
* `--custodian <account>` འདི་ ཚོགས་པའི་བདག་འཛིན་པ་ལུ་ བརྟན་བཞུགས་འབད་ནི་གི་དོན་ལུ་ བརྡ་སྟོན། བཤུབ་པའི་སྐབས་ལུ།
  མཉམ་འབྲེལ་གྱི་ཁས་བླངས་འདི་ ཐད་ཀར་དུ་ཐོབ་ཨིན།
* `repo margin` queries the ledger via `FindRepoAgreements` and reports the next expected margin
  times tamp (མི་ལི་སྐར་ཆ་ནང་) ད་ལྟོ་མཐའ་མཚམས་འབོད་བརྡ་ཅིག་དགོཔ་ཨིན་ན་མེན་ན་ དེ་དང་གཅིག་ཁར་ཨིན།
* `repo margin-call` གིས་ `RepoMarginCallIsi` གི་བཀོད་རྒྱ་ཅིག་ ཟུར་ཐོ་ཞིབ་དཔྱད་ས་ཚིགས་ཐོ་བཀོད་འབད་ཡོདཔ།
  བཅའ་མར་གཏོགས་མི་ཆ་མཉམ་གྱི་དོན་ལུ་ ལས་རིམ་ཚུ་བཏོན་ནི། གལ་སྲིད་ གྱངས་ཁ་འདི་ མ་འགྱོ་བ་ཅིན་ ཡང་ན་ འདི་ མ་འགྱོ་བ་ཅིན་ ཁ་པར་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན།
  བཀོད་རྒྱ་འདི་ བཅའ་མར་གཏོགས་མི་མེན་མི་ཅིག་གིས་ བཙུགསཔ་ཨིན།

## པའི་ཐོན་ཨེས་ཌི་ཀེ་རོགས་སྐྱོར་པ།

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

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="ih58..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="ih58...",
    counterparty="ih58...",
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

* གྲོགས་རམ་པ་གཉིས་ཆ་ར་གིས་ PyO3 བཱའིན་ཌིང་ཚུ་ འབོད་བརྡ་མ་འབད་བའི་ཧེ་མ་ ཨང་གྲངས་ཀྱི་འབོར་ཚད་དང་ མེ་ཊ་ཌེ་ཊ་ས་སྒོ་ཚུ་ སྤྱིར་བཏང་བཟོཝ་ཨིན།
* `RepoAgreementRecord` རན་ཊའིམ་ལས་རིམ་རྩིས་སྟོན་འདི་ མེར་རི་བཟོཝ་ཨིན་ དེ་འབདཝ་ལས་ མཉམ་གྲངས་ཀྱི་ རང་བཞིན་གྱིས་ རང་བཞིན་གྱིས་ འབད་ཚུགས།
  ལག་ཐོག་ལས་ གདམ་ཁའི་ གདངས་འདི་ ལོག་རྩིས་མ་རྐྱབ་པར་ ལོག་འབོད་བརྡ་ཚུ་ ག་དུས་ཨིན་ན་ གཏན་འབེབས་བཟོ།

## DvP / PvP གཞིས་ཆགས་ཚུ།

`iroha app settlement` བརྡ་བཀོད་གནས་རིམ་གྱི་ སྐྱེལ་འདྲེན་གྱི་ མཐུན་རྐྱེན་ཚུ་ སྤྲོད་ནི་དང་ དངུལ་སྤྲོད་ཀྱི་ མཐུན་རྐྱེན་སྤྲོད་ནིའི་ བཀོད་རྒྱ་ཚུ།

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset bond#wonderland \
  --delivery-quantity 10 \
  --delivery-from ih58... \
  --delivery-to ih58... \
  --delivery-instrument-id US0378331005 \
  --payment-asset usd#wonderland \
  --payment-quantity 1000 \
  --payment-from ih58... \
  --payment-to ih58... \
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
  --primary-from ih58... \
  --primary-to ih58... \
  --counter-asset eur#wonderland \
  --counter-quantity 460 \
  --counter-from ih58... \
  --counter-to ih58... \
  --iso-xml-out trade_pvp.xml
```

* ལེག་འབོར་ཚད་ཚུ་གིས་ ཆ་ཤས་གཙོ་བོ་ཡང་ན་ བཅུ་ཚག་གནས་གོང་ཚུ་ངོས་ལེན་འབདཝ་ཨིནམ་དང་ འདི་ཚུ་ རྒྱུ་དངོས་གཏན་གཏན་ལུ་ བདེན་དཔྱད་འབདཝ་ཨིན།
* `--atomicity` གིས་ `all-or-nothing`, `commit-first-leg`, ཡང་ན་ `commit-second-leg`, ཡང་ན་ `commit-second-leg`. ཐབས་ལམ་འདི་ཚུ་ལག་ལེན་འཐབ།
  18NI000000029X དང་མཉམ་དུ། དེ་རྗེས་ཀྱི་ལས་སྦྱོར་འཐུས་ཤོར་བྱུང་ན། (`commit-first-leg` (`commit-first-leg`)
  རྐངམ་དང་པམ་འདི་འཇུག་སྤྱོད་འབད་བཞགཔ་ཨིན། `commit-second-leg` གིས་ གཉིས་པ་འདི་བཞག་ཡོདཔ་ཨིན།)
* CLI འབོད་བརྡ་ཚུ་གིས་ ད་རིས་ བཀོད་རྒྱ་སྟོང་པའི་མེ་ཊ་ཌེ་ཊ་ བཏོནམ་ཨིན། གཞིས་ཆགས་གནས་རིམ་གྱི་སྐབས་ལུ་ པའི་ཐོན་གྲོགས་རམ་པ་ཚུ་ལག་ལེན་འཐབ།
  མེ་ཊ་ཌེ་ཊ་ མཉམ་སྦྲགས་འབད་དགོ།
* [`settlement_iso_mapping.md`](ISO 20022 ས་ཁྲ་བཟོ་བའི་དོན་ལུ་ [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ལུ་བལྟ།
  ཕྱིར་ལོག་འདི་ (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`,
* `--iso-xml-out <path>` འདི་ Norito གི་མཉམ་དུ་ ཀེ་ནོ་ནིག་ཨེགསི་ཨེམ་ཨེལ་སྔོན་ལྟ་ཅིག་ བཏོན་གཏང་ནི་ལུ་ བརྒལ་དགོ།
  བཀོད་རྒྱ་; ཡིག་སྣོད་འདི་གིས་ གོང་ལུ་སབ་ཁྲ་བཟོ་ནི་ (`sese.023`, DvP, `sese.025` གི་དོན་ལུ་ PvP` གི་དོན་ལུ་ཨིན།) ཆ་སྒྲིག་འབད།
  `--iso-reference-crosswalk <path>` དང་ཅིག་ཁར་ སི་ཨེལ་ཨའི་གིས་ `--delivery-instrument-id` བདེན་དཔྱད་འབདཝ་ཨིན།
  འདྲ་པར་ Torii གིས་ རན་དུས་ཚོད་ནང་འཛུལ་བའི་སྐབས་ ལག་ལེན་འཐབ་ཨིན།

པའི་ཐོན་རོགས་རམ་འབད་མི་ཚུ་གིས་ སི་ཨེལ་ཨའི་ ཁ་ཐོག་ལུ་ མེ་ལོང་:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="ih58..."))
delivery = SettlementLeg(
    asset_definition_id="bond#wonderland",
    quantity="10",
    from_account="ih58...",
    to_account="ih58...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="usd#wonderland",
    quantity="1000",
    from_account="ih58...",
    to_account="ih58...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="usd#wonderland",
        quantity="500",
        from_account="ih58...",
        to_account="ih58...",
    ),
    SettlementLeg(
        asset_definition_id="eur#wonderland",
        quantity="460",
        from_account="ih58...",
        to_account="ih58...",
    ),
)
```

## གཏན་འཁེལ་དང་གཞུང་སྐྱོང་གི་རེ་བ།

རེ་པོ་བཀོད་རྒྱ་ཚུ་ Norito-encoded ཨང་གྲངས་ཀྱི་དབྱེ་བ་དང་ བརྗེ་སོར་འབད་མི་ལུ་རྐྱངམ་ཅིག་ བརྟེན་དོ་ཡོདཔ་ཨིན།
`RepoGovernance::with_defaults` ཚད་མ་ནི། འོག་གི་འགྱུར་ལྡོག་མེད་པའི་སེམས་ལུ་བཞག་ནི།* འབོར་ཚད་ཚུ་ གཏན་འབེབས་ `NumericSpec` གནས་གོང་ཚུ་: དངུལ་གྱི་རྐངམ་ལག་ལེན་འཐབ་ནི།
  `fractional(2)` (ཚག་གཉིས་) གིས་ བརྟན་བཞུགས་རྐངམ་ཚུ་གིས་ `integer()` ལག་ལེན་འཐབ་ཨིན། ཕུལ་མ་བཏུབ།
  གནས་གོང་ཚུ་ གཏན་གཏན་སྦེ་ གཏན་གཏན་སྦེ་—རན་ཊའིམ་ ཉེན་སྲུངཔ་ཚུ་གིས་ ཁོང་ཚུ་ ངོས་ལེན་མ་འབད་བར་ མཉམ་རོགས་ཀྱིས་ ཁ་སྟོར་འགྱོ་འོང་།
* Tri-party repos གིས་ `RepoAgreement` ནང་ བདག་འཛིན་རྩིས་ཐོ་ id འདི་ གནས་ཏེ་ཡོདཔ་ཨིན། མི་ཚེ་འཁོར་བ་དང་མཐའ་མཚམས་ཀྱི་བྱུང་རིམ།
  `RepoAccountRole::Custodian` གིས་ བདག་དབང་འབད་མི་ཚུ་གིས་ ཐོ་བཀོད་ཚུ་ མཐུད་དེ་ ཐོ་བཀོད་འབད་ཚུགས།
* སྐྲ་གཅོད་ཚུ་ ༡༠༠༠༠བི་པི་ཨེསི་ (༡༠༠%) དང་ མཐའ་མཚམས་བསྐྱར་འབྱུང་ཚུ་ སྐར་ཆ་ཆ་མཉམ་ཨིན། བྱིན་ནི
  རན་དུས་ཚོད་ཀྱི་རེ་བ་དང་འཁྲིལ་ཏེ་སྡོད་ནིའི་དོན་ལུ་ ཁྲིམས་ལུགས་ཆ་ཚན་ཚུ་ནང་ གཞུང་སྐྱོང་ཚད་གཞི་ཚུ།
* དུས་ཚོད་མཚོན་རྟགས་ཚུ་ དུས་རྒྱུན་དུ་ ཡུ་ནིགསི་ མི་ལི་སྐར་ཆ་ཨིན། གྲོགས་རམ་པ་ཆ་མཉམ་གྱིས་ཁོང་ཚུ་ Norito ལུ་བསྒྱུར་བཅོས་མ་འབད།
  པེ་ལོཌ་འབདཝ་ལས་ མཉམ་རོགས་ཀྱིས་ གོ་རིམ་འདྲ་མཚུངས་ཚུ་ ཐོབ་ཨིན།
* འགོ་བཙུགས་དང་ བསླབ་སྟོན་ཚུ་གིས་ ཆིངས་ཡིག་ངོས་འཛིན་འབད་མི་ ཅོག་འཐདཔ་འདི་ ལོག་ལག་ལེན་འཐབ། རན་དུས་ཚོད་བཀག་ཆ་འབདཝ་ཨིན།
  མ་ཤེས་པའི་གན་ཡིག་ཚུ་གི་དོན་ལུ་ ངོ་རྟགས་དང་ གོང་འཁོད་ཚུ། CLI/SDK གྲོགས་རམ་འབད་མི་ཚུ་གིས་ འཛོལ་བ་དེ་ཚུ་ ཧེ་མ་ལས་ ཁ་ཐོག་ལུ་ཐོན་དོ་ཡོདཔ་ཨིན།
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` གིས་ ཀེ་ནོ་ནིག་གི་ཚད་འདི་སླར་ལོག་འབད། ཨ་རྟག་ར
  ལས་རིམ་ཚུ་ ལོག་རྩེད་ནི་ལས་ བཀག་ཐབས་ལུ་ ལོག་འབོད་བརྡ་ཚུ་ མ་འབྱུང་པའི་ཧེ་མ་ པར་འདི་ བལྟ།