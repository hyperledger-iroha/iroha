---
lang: my
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

ဤလမ်းညွှန်ချက်သည် Iroha တွင် repo နှင့် reverse-repo သဘောတူညီချက်များအတွက် အဆုံးအဖြတ်စီးဆင်းမှုကို မှတ်တမ်းတင်ထားသည်။
၎င်းသည် CLI orchestration၊ SDK အကူအညီပေးသူများနှင့် အော်ပရေတာများ လုပ်ဆောင်နိုင်စေရန်အတွက် မျှော်လင့်ထားသော အုပ်ချုပ်မှုခလုတ်များ ပါဝင်သည်။
အကြမ်း Norito payload များကို မရေးဘဲ အစပြု၊ အနားသတ် နှင့် သဘောတူညီချက်များကို ဖြေလျှော့ပါ။ အုပ်ချုပ်ရေးအတွက်
စစ်ဆေးစာရင်းများ၊ အထောက်အထားများ ဖမ်းယူခြင်းနှင့် လိမ်လည်မှု/ပြန်လှည့်ခြင်းဆိုင်ရာ လုပ်ထုံးလုပ်နည်းများကို ကြည့်ပါ။
လမ်းပြမြေပုံပါ အကြောင်းအရာ F1 ကို ကျေနပ်စေသည့် [`repo_ops.md`](./repo_ops.md)။

## CLI အမိန့်များ

`iroha app repo` command သည် repo-specific helpers များကို အုပ်စုဖွဲ့သည်-

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

* `repo initiate` နှင့် `repo unwind` လေးစားမှု `--input/--output` ထို့ကြောင့် ထုတ်လုပ်လိုက်သော `InstructionBox`
  payload များကို အခြားသော CLI စီးဆင်းမှုများသို့ ပိုက်ထည့်နိုင်သည် သို့မဟုတ် ချက်ချင်းတင်သွင်းနိုင်သည်။
* သုံးပါတီအုပ်ထိန်းသူထံ အပေါင်ပစ္စည်းပေးဆောင်ရန် `--custodian <account>` ကို ဖြတ်ပါ။ ချန်လှပ်ထားသောအခါ၊
  မိတ်ဖက်သည် ကတိကဝတ်ကို တိုက်ရိုက်လက်ခံသည် (bilateral repo)။
* `repo margin` သည် `FindRepoAgreements` မှတစ်ဆင့် လယ်ဂျာကို မေးမြန်းပြီး လာမည့်မျှော်လင့်ထားသောအနားသတ်ကို အစီရင်ခံသည်။
  အနားသတ်ခေါ်ဆိုမှု လောလောဆယ် ကုန်ဆုံးနေသလား၊ အချိန်တံဆိပ် (မီလီစက္ကန့်အတွင်း)။
* `repo margin-call` သည် `RepoMarginCallIsi` ညွှန်ကြားချက်ကို ဖြည့်စွက်ပြီး အနားသတ်စစ်ဆေးရေးဂိတ်ကို မှတ်တမ်းတင်ခြင်းနှင့်
  ပါဝင်သူအားလုံးအတွက် ထုတ်လွှင့်သော ပွဲများ။ အတန်းမပြီးပါက သို့မဟုတ် ခေါ်ဆိုမှုများကို ပယ်ချပါသည်။
  ညွှန်ကြားချက်ကို ပါဝင်သူမဟုတ်သူတစ်ဦးမှ တင်ပြပါသည်။

## Python SDK အကူအညီပေးသူများ

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

* အကူအညီပေးသူနှစ်ဦးစလုံးသည် PyO3 ချိတ်ဆက်မှုများကို မခေါ်ဆိုမီ ကိန်းဂဏန်းပမာဏများနှင့် မက်တာဒေတာအကွက်များကို ပုံမှန်ဖြစ်စေသည်။
* `RepoAgreementRecord` သည် runtime အချိန်ဇယားတွက်ချက်မှုကို ထင်ဟပ်နေသောကြောင့် off-ledger automation လုပ်နိုင်သည်
  cadence ကို ကိုယ်တိုင်ပြန်မတွက်ဘဲ ပြန်ခေါ်မည့်အချိန်ကို ဆုံးဖြတ်ပါ။

## DvP/PvP အခြေချနေထိုင်မှုများ

`iroha app settlement` ညွှန်ကြားချက်သည် ပေးပို့ခြင်း-ဆန့်ကျင်-ငွေပေးချေမှုနှင့် ငွေပေးချေမှု-ငွေပေးချေမှု ညွှန်ကြားချက်များကို အဆင့်သတ်မှတ်သည်-

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

* ခြေထောက်ပမာဏများသည် တစ်ပေါင်းတစ်စည်း သို့မဟုတ် ဒဿမတန်ဖိုးများကို လက်ခံပြီး ပိုင်ဆိုင်မှုတိကျမှုနှင့် ကိုက်ညီကြောင်း အတည်ပြုထားသည်။
* `--atomicity` သည် `all-or-nothing`၊ `commit-first-leg` သို့မဟုတ် `commit-second-leg` ကို လက်ခံသည်။ ဤမုဒ်များကိုသုံးပါ။
  နောက်ဆက်တွဲလုပ်ဆောင်မှု မအောင်မြင်ပါက မည်သည့်ခြေထောက်ကို ဆက်လက်ကျူးလွန်ကြောင်းဖော်ပြရန် `--order` နှင့် (`commit-first-leg`
  ပထမခြေထောက်ကို အသုံးချပါ။ `commit-second-leg` သည် ဒုတိယမြောက်) ကို ထိန်းသိမ်းထားသည်။
* CLI တောင်းခံမှုများသည် ယနေ့တွင် အချည်းနှီးသော ညွှန်ကြားချက် မက်တာဒေတာကို ထုတ်လွှတ်သည်။ ဖြေရှင်းမှုအဆင့်တွင် Python helpers ကိုသုံးပါ။
  မက်တာဒေတာကို ပူးတွဲထားရန် လိုအပ်သည်။
* ISO 20022 အကွက်ပုံဖော်ခြင်းအတွက် [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ကိုကြည့်ပါ
  ဤညွှန်ကြားချက်များကို ထောက်ခံသည် (`sese.023`၊ `sese.025`၊ `colr.007`၊ `pacs.009`၊ `camt.054`)။
* CLI သည် Norito နှင့်အတူ Canonical XML အစမ်းကြည့်ရှုမှုအား ထုတ်လွှတ်ရန် `--iso-xml-out <path>` ကို ဖြတ်ပါ။
  ညွှန်ကြားချက်; ဖိုင်သည် အထက်ဖော်ပြပါ မြေပုံကို လိုက်နာသည် (DvP အတွက် `sese.023`၊ PvP` အတွက် `sese.025`)။ တွဲပါ။
  `--iso-reference-crosswalk <path>` ဖြင့် အလံပြထားသောကြောင့် CLI သည် `--delivery-instrument-id` နှင့် ဆန့်ကျင်ဘက်ဖြစ်သည်
  တူညီသောလျှပ်တစ်ပြက်ရိုက်ချက် Torii ကို runtime ဝင်ခွင့်အတွင်းအသုံးပြုသည်။

Python အကူအညီပေးသူများသည် CLI မျက်နှာပြင်ကို ထင်ဟပ်စေသည်-

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

## အဆုံးအဖြတ်နှင့် အုပ်ချုပ်မှုမျှော်လင့်ချက်များ

Repo ညွှန်ကြားချက်များသည် Norito-ကုဒ်လုပ်ထားသော ဂဏန်းအမျိုးအစားများနှင့် မျှဝေထားသည့်အပေါ် မှီခိုနေရသည်
`RepoGovernance::with_defaults` လော့ဂျစ်။ အောက်ဖော်ပြပါ ပုံစံကွဲများကို သတိပြုပါ။* ပမာဏများကို အဆုံးအဖြတ်ပေးသော `NumericSpec` တန်ဖိုးများဖြင့် အမှတ်စဉ်ထားသည်- ငွေသားခြေထောက်အသုံးပြုမှု
  `fractional(2)` (ဒဿမနှစ်နေရာ)၊ စရံခြေထောက် `integer()` ကို အသုံးပြုသည်။ မတင်ပြပါနှင့်
  ပိုမိုတိကျသောတန်ဖိုးများ—runtime guards များသည် ၎င်းတို့ကို ငြင်းပယ်ပြီး ရွယ်တူများ ကွဲပြားသွားမည်ဖြစ်သည်။
* Tri-party repos သည် `RepoAgreement` တွင် ထိန်းသိမ်းသူအကောင့် ID ကို ဆက်လက်တည်ရှိနေပါသည်။ ဘဝသံသရာနှင့် အနားသတ်ဖြစ်ရပ်များ
  `RepoAccountRole::Custodian` payload ကို ထုတ်လွှတ်သောကြောင့် ထိန်းသိမ်းသူများသည် စာရင်းသွင်းပြီး စာရင်းကို ညှိနှိုင်းနိုင်ပါသည်။
* ဆံပင်ညှပ်ခြင်းကို 10000bps (100%) နှင့် အနားသတ်ကြိမ်နှုန်းများသည် စက္ကန့်တိုင်းဖြစ်သည်။ ပေးသည်။
  runtime မျှော်မှန်းချက်များနှင့် လိုက်လျောညီထွေနေရန် အဆိုပါ canonical ယူနစ်များရှိ အုပ်ချုပ်မှုဘောင်များ။
* အချိန်တံဆိပ်တုံးများသည် အမြဲတမ်း Unix မီလီစက္ကန့်များဖြစ်သည်။ အကူအညီပေးသူများအားလုံး ၎င်းတို့အား Norito သို့ မပြောင်းလဲဘဲ ပေးပို့သည်။
  payload ကြောင့် ရွယ်တူများသည် ထပ်တူကျသော အချိန်ဇယားများကို ရယူသည်။
* အစပြုခြင်း နှင့် ညွှန်ကြားချက်များကို ဖြေလျှော့ပြီး တူညီသော သဘောတူညီချက် သတ်မှတ်စနစ်ကို ပြန်သုံးပါ။ runtime က ငြင်းပယ်ပါတယ်။
  ID များကိုပွားပြီး အမည်မသိသဘောတူညီချက်များအတွက် ဖြေလျှော့ပါ။ CLI/SDK အကူအညီပေးသူများသည် အဆိုပါအမှားများကို စောစီးစွာဖော်ပြသည်။
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` သည် canonical cadence ကို ပြန်ပေးသည်။ အမြဲတမ်း
  ပျက်နေသောအချိန်ဇယားများကို ပြန်လည်ပြသခြင်းမှရှောင်ရှားရန် ဖုန်းခေါ်ဆိုမှုများကို မစတင်မီ ဤလျှပ်တစ်ပြက်ရိုက်ချက်နှင့် တိုင်ပင်ပါ။