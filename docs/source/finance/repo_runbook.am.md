---
lang: am
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

ይህ መመሪያ በIroha ውስጥ የሪፖ እና የተገላቢጦሽ ስምምነቶችን የመወሰን ፍሰትን ይመዘግባል።
ኦፕሬተሮች እንዲችሉ የCLI ኦርኬስትራን፣ የኤስዲኬ ረዳቶችን እና የሚጠበቁትን የአስተዳደር ቁልፎችን ይሸፍናል።
ጥሬ የNorito ክፍያ ጭነቶችን ሳይጽፉ ስምምነቶችን ይጀምሩ፣ ህዳግ እና መፍታት። ለአስተዳደር
የማረጋገጫ ዝርዝሮች፣ የማስረጃ ቀረጻ እና ማጭበርበር/መመለሻ ሂደቶችን ያያሉ።
[`repo_ops.md`](./repo_ops.md)፣ ይህም የመንገድ ካርታ ንጥል F1 ያሟላል።

## CLI ያዛል

የ `iroha app repo` ትዕዛዝ ቡድኖች ልዩ ረዳቶች፡-

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
  --custodian <i105-account-id> \
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
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
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

* `repo initiate` እና `repo unwind` አክብሮት `--input/--output` ስለዚህ የመነጨው `InstructionBox`
  ጭነት ወደ ሌሎች የ CLI ፍሰቶች ሊገባ ወይም ወዲያውኑ ማስገባት ይችላል።
* ለባለሶስት ወገን ጠባቂ ለማስያዝ `--custodian <account>` ይለፉ። ሲቀር፣ የ
  counterparty ቃል ኪዳኑን በቀጥታ ይቀበላል (bilateral repo)።
* `repo margin` የሂሳብ መዝገብን በ`FindRepoAgreements` በኩል ጠይቆ ቀጣዩን የሚጠበቀውን ህዳግ ሪፖርት ያደርጋል
  የጊዜ ማህተም (በሚሊሰከንዶች) በአሁኑ ጊዜ የኅዳግ መልሶ መደወል ካለመሆኑ ጋር።
* `repo margin-call` የ `RepoMarginCallIsi` መመሪያን በማያያዝ የኅዳግ ፍተሻ ነጥቡን በመመዝገብ እና
  ለሁሉም ተሳታፊዎች ክስተቶችን ማሰራጨት. ጥሪው ካላለፈ ወይም የ
  መመሪያው ተሳታፊ ባልሆነ ሰው ነው የሚቀርበው።

## Python SDK ረዳቶች

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

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="<i105-account-id>",
    counterparty="<i105-account-id>",
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

* ሁለቱም ረዳቶች የ PyO3 ማሰሪያዎችን ከመጥራታቸው በፊት የቁጥር መጠኖችን እና የሜታዳታ መስኮችን መደበኛ ያደርጋሉ።
* `RepoAgreementRecord` የሩጫ ጊዜ መርሐግብር ስሌትን ያንፀባርቃል ስለዚህ ከደብዳቤ ውጭ አውቶማቲክ ማድረግ ይችላል
  የድጋሚ ጥሪውን በእጅ ሳያስሉት መቼ እንደሆነ ይወስኑ።

## DvP/PvP ሰፈራዎች

የ`iroha app settlement` ትእዛዝ መላኪያ-ከክፍያ እና ከክፍያ ጋር-የክፍያ መመሪያዎችን ደረጃ ይሰጣል፡-

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from <i105-account-id> \
  --delivery-to <i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from <i105-account-id> \
  --payment-to <i105-account-id> \
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
  --primary-from <i105-account-id> \
  --primary-to <i105-account-id> \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from <i105-account-id> \
  --counter-to <i105-account-id> \
  --iso-xml-out trade_pvp.xml
```

* የእግር መጠኖች የተዋሃዱ ወይም የአስርዮሽ እሴቶችን ይቀበላሉ እና በንብረቱ ትክክለኛነት የተረጋገጡ ናቸው።
* `--atomicity` `all-or-nothing`፣ `commit-first-leg`፣ ወይም `commit-second-leg` ይቀበላል። እነዚህን ሁነታዎች ተጠቀም
  ከ`--order` ጋር የሚቀጥለው ሂደት ካልተሳካ የትኛው እግር እንደሚቆይ ለመግለፅ (`commit-first-leg`)
  የመጀመሪያውን እግር እንዲተገበር ያቆያል; `commit-second-leg` ሁለተኛውን ይይዛል).
* የ CLI ጥሪዎች ዛሬ ባዶ የመመሪያ ሜታዳታ ያወጣሉ። በሰፈራ ደረጃ የ Python አጋዥዎችን ይጠቀሙ
  ሜታዳታ መያያዝ አለበት።
* ለ ISO 20022 የመስክ ካርታ ስራ [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ይመልከቱ
  እነዚህን መመሪያዎች ይደግፋል (`sese.023`፣ `sese.025`፣ `colr.007`፣ `pacs.009`፣ `camt.054`)።
* CLI ከ Norito ጎን ለጎን ቀኖናዊ የኤክስኤምኤል ቅድመ እይታ እንዲያወጣ `--iso-xml-out <path>` ይለፉ
  መመሪያ; ፋይሉ ከላይ ያለውን ካርታ (`sese.023` ለDvP፣ `sese.025` ለ PvP`) ይከተላል። ያጣምሩ
  ባንዲራ ከ `--iso-reference-crosswalk <path>` ጋር ስለዚህ CLI `--delivery-instrument-id`ን ከ
  ተመሳሳይ ቅጽበተ-ፎቶ Torii በሂደት መግቢያ ጊዜ ይጠቀማል።

የፓይዘን ረዳቶች የ CLI ገጽን ያንፀባርቃሉ፡

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
)
```

## ቆራጥነት እና የአስተዳደር ተስፋዎች

የሪፖ መመሪያዎች በNorito የተመሰጠሩ የቁጥር አይነቶች እና በተጋሩ ላይ ብቻ ይተማመናሉ።
`RepoGovernance::with_defaults` አመክንዮ. የሚከተሉትን ልዩነቶች ግምት ውስጥ ያስገቡ፦* መጠኖች በቆራጥነት `NumericSpec` እሴቶች ተከታታይ ናቸው፡ የገንዘብ እግሮች አጠቃቀም
  `fractional(2)` (ሁለት አስርዮሽ ቦታዎች)፣ የዋስትና እግሮች `integer()` ይጠቀማሉ። አታቅርቡ
  የበለጠ ትክክለኛነት ያላቸው እሴቶች - የአሂድ ጊዜ ጠባቂዎች ውድቅ ያደርጋቸዋል እና እኩዮችም ይለያያሉ።
* የሶስትዮሽ ወገኖች የመጠባበቂያ መለያ መታወቂያ በ`RepoAgreement` እንደቀጠለ ነው። የሕይወት ዑደት እና የኅዳግ ክስተቶች
  አሳዳጊዎች በደንበኝነት እንዲመዘገቡ እና ክምችትን እንዲያስታርቁ የ`RepoAccountRole::Custodian` ክፍያ ያቅርቡ።
* የፀጉር መቆራረጥ ወደ 10000bps (100%) እና የኅዳግ ድግግሞሾች ሙሉ ሴኮንዶች ናቸው። ያቅርቡ
  በእነዚያ ቀኖናዊ ክፍሎች ውስጥ የአስተዳደር መለኪያዎች ከአሂድ ጊዜ ከሚጠበቁት ጋር እንዲጣጣሙ።
* የጊዜ ማህተሞች ሁል ጊዜ ዩኒክስ ሚሊሰከንዶች ናቸው። ሁሉም ረዳቶች ሳይለወጡ ወደ Norito ያስተላልፋሉ
  ሸክም ስለዚህ እኩዮች ተመሳሳይ መርሃግብሮችን ያገኛሉ።
* የማስጀመር እና የመፍታት መመሪያዎች ተመሳሳዩን የስምምነት መለያ እንደገና ይጠቀሙ። የሩጫ ሰዓቱ ውድቅ ያደርጋል
  ለማይታወቁ ስምምነቶች የተባዙ መታወቂያዎች እና ዊንድስ; የCLI/SDK ረዳቶች እነዚያን ስህተቶች ቀደም ብለው ያሳያሉ።
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` ቀኖናዊውን ቃና ይመልሳል። ሁሌም
  የቆዩ መርሃ ግብሮችን ላለመድገም መልሶ ጥሪዎችን ከማስነሳትዎ በፊት ይህንን ቅጽበታዊ ገጽ እይታ ይመልከቱ።