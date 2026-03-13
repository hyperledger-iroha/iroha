---
lang: my
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/quickstart/default_lane.md` ဖြစ်သည်။ မိတ္တူ နှစ်ခုလုံးကို သိမ်းထားပါ။
ပေါ်တယ်ရှိမြေများကို ဒေသအလိုက်ပြောင်းလဲသတ်မှတ်သည်အထိ ညှိပါ။
:::

# မူရင်းလမ်းသွယ် Quickstart (NX-5)

> **လမ်းပြမြေပုံအကြောင်းအရာ-** NX-5 — မူရင်းအများပြည်သူလမ်းကြား ပေါင်းစည်းမှု။ ယခု runtime
> `nexus.routing_policy.default_lane` သည် လှည့်ကွက်ကို ထုတ်ပြသည် ထို့ကြောင့် Torii REST/gRPC
> endpoints နှင့် SDK တစ်ခုစီတိုင်းသည် traffic ကိုပိုင်ဆိုင်သောအခါ `lane_id` ကို ဘေးကင်းစွာ ချန်လှပ်ထားနိုင်သည်
> အများသူငါလမ်းကြား။ ဤလမ်းညွှန်ချက်သည် စီစဉ်သတ်မှတ်ခြင်းမှတစ်ဆင့် အော်ပရေတာများကို လမ်းလျှောက်ပေးသည်။
> ကတ်တလောက်၊ `/status` တွင် ဆုတ်ယုတ်မှုကို စစ်ဆေးခြင်းနှင့် client ကို လေ့ကျင့်ခန်းလုပ်ခြင်း
> အမူအရာ အဆုံးထိ။

## လိုအပ်ချက်များ

- `irohad` (`irohad --sora --config ...`) ၏ Sora/Nexus တည်ဆောက်ခြင်း။
- `nexus.*` ကဏ္ဍများကို တည်းဖြတ်နိုင်စေရန် ဖွဲ့စည်းမှုဆိုင်ရာ သိုလှောင်ခန်းသို့ ဝင်ရောက်ပါ။
- `iroha_cli` ကို ပစ်မှတ်အစုအဝေးသို့ စကားပြောရန် စီစဉ်သတ်မှတ်ထားသည်။
- `curl`/`jq` (သို့မဟုတ် ညီမျှသော) Torii `/status` ပါဝန်အား စစ်ဆေးရန်။

## 1. လမ်းကြောနှင့် ဒေတာအာကာသ ကတ်တလောက်ကို ဖော်ပြပါ။

ကွန်ရက်ပေါ်တွင် ရှိသင့်သော လမ်းကြောများနှင့် ဒေတာနေရာလွတ်များကို ကြေညာပါ။ အတိုအထွာ
အောက်တွင် (`defaults/nexus/config.toml` မှ ကွပ်ထားသော) အများသုံးလမ်းသွား သုံးခုကို မှတ်ပုံတင်သည်။
ထို့အပြင် ကိုက်ညီသော dataspace နာမည်တူများ-

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

`index` တစ်ခုစီသည် ထူးခြားပြီး တစ်ဆက်တည်းဖြစ်ရပါမည်။ Dataspace ids များသည် 64-bit တန်ဖိုးများဖြစ်သည်။
အထက်ဖော်ပြပါ ဥပမာများသည် ရှင်းလင်းပြတ်သားမှုအတွက် လမ်းကြောညွှန်းကိန်းများကဲ့သို့ တူညီသော ဂဏန်းတန်ဖိုးများကို အသုံးပြုသည်။

## 2. လမ်းကြောင်းသတ်မှတ်ပုံသေများနှင့် စိတ်ကြိုက် overrides သတ်မှတ်ပါ။

`nexus.routing_policy` အပိုင်းသည် နောက်ပြန်လမ်းကြောကို ထိန်းချုပ်ပြီး သင့်ကိုခွင့်ပြုသည်။
သတ်မှတ်ထားသော ညွှန်ကြားချက်များ သို့မဟုတ် အကောင့်ရှေ့ဆက်များအတွက် လမ်းကြောင်းကို အစားထိုးပါ။ စည်းကမ်းမရှိရင်
ကိုက်ညီမှုများ၊ အချိန်ဇယားဆွဲသူက ငွေပေးငွေယူကို စီစဉ်သတ်မှတ်ထားသော `default_lane` သို့ လမ်းကြောင်းပေးသည်
နှင့် `default_dataspace`။ Router logic သည် နေထိုင်သည်။
`crates/iroha_core/src/queue/router.rs` နှင့် မူဝါဒကို ပွင့်လင်းမြင်သာစွာ ကျင့်သုံးပါ။
Torii REST/gRPC မျက်နှာပြင်များ။

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

နောက်ပိုင်းတွင် လမ်းကြောအသစ်များ ထည့်သောအခါ၊ ကတ်တလောက်ကို ဦးစွာ အပ်ဒိတ်လုပ်ပါ၊ ထို့နောက် လမ်းကြောင်းကို တိုးချဲ့ပါ။
စည်းကမ်း။ နောက်ပြန်လမ်းကြောသည် ထိန်းထားနိုင်သော အများသူငှာလမ်းကို ဆက်လက်ညွှန်ပြနေသင့်သည်။

## 3. အသုံးပြုထားသော မူဝါဒဖြင့် node တစ်ခုကို စတင်ပါ။

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

စတင်ချိန်တွင် node သည် ဆင်းသက်လာသောလမ်းကြောင်းပေါ်လစီကို မှတ်တမ်းတင်ပါသည်။ အတည်ပြုချက်အမှားများ
(ပျောက်ဆုံးနေသော အညွှန်းများ၊ ပွားထားသော နာမည်တူများ၊ မမှန်ကန်သော dataspace ids) များကို ယခင်က ပေါ်လွင်စေပါသည်။
အတင်းအဖျင်းပြောသည် ။

## 4. လမ်းသွားအုပ်ချုပ်မှုအခြေအနေကို အတည်ပြုပါ။

node သည် အွန်လိုင်းတွင်ရှိပြီး၊ ပုံသေလမ်းကြောင်းဖြစ်ကြောင်း အတည်ပြုရန် CLI helper ကို အသုံးပြုပါ။
အလုံပိတ် (မန်နီးဖက်စ်တင်ထားသည်) နှင့် ယာဉ်အသွားအလာအတွက် အဆင်သင့်။ အနှစ်ချုပ်မြင်ကွင်းသည် အတန်းတစ်တန်းကို ပရင့်ထုတ်သည်။
လမ်းသွားတိုင်း-

```bash
iroha_cli app nexus lane-report --summary
```

နမူနာ အထွက်-

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

ပုံသေလမ်းကြောသည် `sealed` ကိုပြသပါက၊ ရှေ့လမ်းကြောအုပ်ချုပ်မှုစာအုပ်ကို လိုက်နာပါ။
ပြင်ပအသွားအလာခွင့်ပြုသည်။ `--fail-on-sealed` အလံသည် CI အတွက် အဆင်ပြေသည်။

## 5. Torii အခြေအနေ ပေးဆောင်မှုများကို စစ်ဆေးပါ။

`/status` တုံ့ပြန်မှုသည် လမ်းကြောင်းပေါ်လစီနှင့် တစ်လမ်းသွား အစီအစဉ်ဆွဲသူကို ဖော်ထုတ်ပေးသည်
လျှပ်တစ်ပြက်။ ပြင်ဆင်ထားသော ပုံသေများကို အတည်ပြုရန်နှင့် စစ်ဆေးရန် `curl`/`jq` ကို အသုံးပြုပါ။
နောက်ပြန်လမ်းကြောသည် တယ်လီမီတာကို ထုတ်လုပ်သည်-

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

နမူနာ အထွက်-

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` အတွက် တိုက်ရိုက် အချိန်ဇယား ကောင်တာများကို စစ်ဆေးရန်-

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

၎င်းသည် TEU လျှပ်တစ်ပြက်ရိုက်ချက်၊ alias မက်တာဒေတာနှင့် မန်နီးဖက်စ်အလံများ ကိုက်ညီကြောင်း အတည်ပြုသည်။
configuration နှင့်အတူ။ တူညီသော payload သည် Grafana panels အသုံးပြုသည့်အရာဖြစ်သည်။
lane-ingest ဒက်ရှ်ဘုတ်။

## 6. အသုံးပြုသူ၏ ပုံသေများကို လေ့ကျင့်ပါ။

- **Rust/CLI.** `iroha_cli` နှင့် Rust client crate သည် `lane_id` အကွက်ကို ချန်လှပ်ထားသည်။
  `--lane-id` / `LaneSelector` မအောင်သောအခါ။ တန်းစီနေသော router သည် ထို့ကြောင့်
  `default_lane` သို့ ပြန်ကျသွားသည် ။ ပြတ်သားသော `--lane-id`/`--dataspace-id` အလံများကို သုံးပါ
  ပုံသေမဟုတ်သော လမ်းကြောတစ်ခုကို ပစ်မှတ်ထားသည့်အခါမှသာ။
- **JS/Swift/Android။** နောက်ဆုံးထွက် SDK များသည် `laneId`/`lane_id` ကို စိတ်ကြိုက်ရွေးချယ်နိုင်သည်
  `/status` မှ ကြော်ငြာထားသော တန်ဖိုးသို့ ပြန်ကျသွားသည်။ လမ်းကြောင်းပေါ်လစီကို ထားရှိပါ။
  အဆင့်မြှင့်တင်ခြင်းနှင့် ထုတ်လုပ်ခြင်းတစ်လျှောက် ထပ်တူကျစေသောကြောင့် မိုဘိုင်းအက်ပ်များ အရေးပေါ်မလိုအပ်ပါ။
  ပြင်ဆင်မှုများ။
- **Pipeline/SSE စမ်းသပ်မှုများ။** ငွေပေးငွေယူ ကိစ္စရပ်များကို စစ်ထုတ်ခြင်း လက်ခံပါသည်။
  `tx_lane_id == <u32>` ကြိုတင်ခန့်မှန်းချက်များ (`docs/source/pipeline.md` ကိုကြည့်ပါ)။ စာရင်းသွင်းပါ။
  `/v2/pipeline/events/transactions` ဖြင့် ရေးထားသည်ကို သက်သေပြရန် ထို filter ကို ပေးပို့ပါ။
  ပြတ်သားသောလမ်းကြောမရှိဘဲ လှည့်ပြန်လမ်းကြော ID အောက်တွင် ရောက်ရှိသည်။

## 7. Observability နှင့် governance ချိတ်

- `/status` ကိုလည်း `nexus_lane_governance_sealed_total` နှင့် ထုတ်ဝေသည်။
  `nexus_lane_governance_sealed_aliases` ထို့ကြောင့် Alertmanager သည် အချိန်တိုင်းသတိပေးနိုင်သည်။
  လမ်းကြောက ထင်ရှားသည်။ devnets အတွက်ပင် ထိုသတိပေးချက်များကို ဖွင့်ထားပါ။
- အစီအစဉ်ဆွဲသူ တယ်လီမီတာမြေပုံနှင့် လမ်းသွားအုပ်ချုပ်ရေး ဒက်ရှ်ဘုတ်
  (`dashboards/grafana/nexus_lanes.json`) မှ alias/slug အကွက်များကို မျှော်လင့်သည်။
  ကက်တလောက် နံမည်ပြောင်းပါက သက်ဆိုင်ရာ Kura လမ်းညွှန်များကို ပြန်တပ်ပါ။
  စာရင်းစစ်များသည် အဆုံးအဖြတ်ပေးသည့် လမ်းကြောင်းများကို ထားရှိသည် (NX-1 အောက်တွင် ခြေရာခံသည်)။
- ပုံသေလမ်းကြောများအတွက် လွှတ်တော်၏အတည်ပြုချက်များတွင် နောက်ပြန်ဆုတ်ခြင်းအစီအစဉ်တစ်ခု ပါဝင်သင့်သည်။ မှတ်တမ်း
  သင့်တွင် ဤအမြန်စတင်ခြင်းနှင့်အတူ ထင်ရှားသော hash နှင့် အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားများ
  အော်ပရေတာ runbook ထို့ကြောင့် အနာဂတ်လည်ပတ်မှုများသည် လိုအပ်သည့်အခြေအနေကို ခန့်မှန်း၍မရပါ။

ဤစစ်ဆေးမှုများပြီးသည်နှင့်သင် `nexus.routing_policy.default_lane` ကို ကုသနိုင်ပါသည်။
ကွန်ရက်ပေါ်ရှိ ကုဒ်လမ်းကြောင်းများ။