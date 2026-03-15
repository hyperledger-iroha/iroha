---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-29T18:16:35.177959+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-reconciliation
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
---

လမ်းပြမြေပုံပါ အကြောင်းအရာ **SF-2c** သည် စွမ်းရည်အခကြေးငွေ လယ်ဂျာကို သက်သေပြသော ဘဏ္ဍာတိုက်မှ လုပ်ပိုင်ခွင့်များ
ညတိုင်း လုပ်ဆောင်ခဲ့သော XOR လွှဲပြောင်းမှုများနှင့် ကိုက်ညီပါသည်။ ကိုသုံးပါ။
နှိုင်းယှဉ်ရန် `scripts/telemetry/capacity_reconcile.py` အကူအညီပေးသည်။
`/v2/sorafs/capacity/state` သည် ကွပ်မျက်ခံရသောလွှဲပြောင်းမှုအသုတ်နှင့်ဆန့်ကျင်ဘက်လျှပ်တစ်ပြက်ဓာတ်ပုံ
Alertmanager အတွက် Prometheus စာသားဖိုင်မက်ထရစ်များကို ထုတ်လွှတ်သည်။

## လိုအပ်ချက်များ
- Torii မှ ထုတ်လွှတ်သော စွမ်းရည်အခြေအနေ လျှပ်တစ်ပြက်ပုံ (`fee_ledger`)။
- တူညီသောဝင်းဒိုးအတွက် လယ်ဂျာထုတ်ယူခြင်း (`provider_id_hex` ဖြင့် JSON သို့မဟုတ် NDJSON၊
  `kind` = ဖြေရှင်းခြင်း/ပြစ်ဒဏ် နှင့် `amount_nano`)။
- သင်သတိပေးချက်များလိုပါက node_exporter textfile စုဆောင်းသူထံသို့လမ်းကြောင်း။

## ရှေ့ပြေးစာအုပ်
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- ထွက်ပေါက်ကုဒ်များ- သန့်ရှင်းသောပွဲစဉ်တွင် `0`၊ ပေးဆပ်မှုများ/ပြစ်ဒဏ်များ ပျောက်ဆုံးနေချိန်တွင် `1`
  မမှန်ကန်သော သွင်းအားစုများပေါ်တွင် `2` သို့မဟုတ် ငွေပိုပေးသည်။
- JSON အနှစ်ချုပ် + hash များကို treasury packet တွင် ပူးတွဲပါ
  `docs/examples/sorafs_capacity_marketplace_validation/`။
- `.prom` ဖိုင်သည် textfile စုဆောင်းသူထံ ရောက်သည့်အခါ သတိပေးချက်၊
  `SoraFSCapacityReconciliationMismatch` (ကြည့်ပါ။
  `dashboards/alerts/sorafs_capacity_rules.yml`) မီးလောင်ပျောက်ဆုံးသည့်အခါတိုင်း၊
  ငွေပိုပေးခြင်း၊ သို့မဟုတ် မမျှော်လင့်ထားသော ဝန်ဆောင်မှုပေးသူ လွှဲပြောင်းမှုများကို တွေ့ရှိပါသည်။

## ရလဒ်များ
- အပေးအယူနှင့် ပြစ်ဒဏ်များအတွက် ကွာခြားချက်များရှိသော ဝန်ဆောင်မှုပေးသူ တစ်ဦးချင်း အခြေအနေများ။
- တိုင်းတာမှုအဖြစ် တင်ပို့သည့် စုစုပေါင်း
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## မျှော်လင့်ထားသော အတိုင်းအတာများနှင့် သည်းခံနိုင်မှု
- ပြန်လည်သင့်မြတ်ရေးသည် အတိအကျဖြစ်သည်- မျှော်မှန်းထားသည်နှင့် အမှန်တကယ် ပြေလည်မှု/ပြစ်ဒဏ် နာနိုများသည် လုံးဝသည်းခံမှု နှင့် ကိုက်ညီသင့်ပါသည်။ သုညမဟုတ်သော ကွဲပြားမှုတိုင်းသည် စာမျက်နှာအော်ပရေတာများ ဖြစ်သင့်သည်။
- CI သည် စွမ်းရည်အခကြေးငွေစာရင်းဇယား (စမ်းသပ်မှု `capacity_fee_ledger_30_day_soak_deterministic`) အတွက် ရက် 30 ကြာစိမ်ထားသော အချေအတင်ကို `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` သို့ ကပ်ထားသည်။ စျေးနှုန်း သို့မဟုတ် cooldown semantics အပြောင်းအလဲရှိမှသာ အချေအတင်ကို ပြန်လည်စတင်ပါ။
- စိမ်ပရိုဖိုင်တွင် (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) ပြစ်ဒဏ်များသည် သုညတွင်ရှိနေပါသည်။ အသုံးချခြင်း/ဖွင့်ချိန်/PoR ကြမ်းပြင်များကို ချိုးဖောက်ပြီး မျဉ်းစောင်းများ ဆက်တိုက်မချမီ ပြင်ဆင်ထားသော cooldown ကို လေးစားသောအခါမှသာ ထုတ်လုပ်မှုသည် ပြစ်ဒဏ်များ ထုတ်လွှတ်သင့်ပါသည်။