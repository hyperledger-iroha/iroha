---
id: developer-sdk-index
lang: my
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

SoraFS toolchain ဖြင့် ပို့ဆောင်ပေးသော ဘာသာစကား တစ်ခုချင်း အကူအညီများကို ခြေရာခံရန် ဤအချက်အချာကို အသုံးပြုပါ။
Rust သီးသန့်အတိုအထွာများအတွက် [Rust SDK အတိုအထွာများ](./developer-sdk-rust.md) သို့ ခုန်တက်သည်။

## ဘာသာစကားအကူအညီပေးသူများ

- **Python** — `sorafs_multi_fetch_local` (ဒေသခံ သံစုံတီးဝိုင်း မီးခိုးစမ်းသပ်မှုများ) နှင့်
  ယခု `sorafs_gateway_fetch` (တံခါးပေါက် E2E လေ့ကျင့်ခန်းများ) စိတ်ကြိုက်ရွေးချယ်နိုင်ပါပြီ။
  `telemetry_region` နှင့် `transport_policy` ထပ်ရေးခြင်း
  (`"soranet-first"`, `"soranet-strict"`, or `"direct-only"`) CLI ကို ရောင်ပြန်ဟပ်ခြင်း၊
  rollout ခလုတ်များ။ ဒေသတွင်း QUIC ပရောက်စီတစ်ခု တက်လာသောအခါ၊
  `sorafs_gateway_fetch` သည် အောက်ရှိ browser manifest ကို ပြန်ပေးသည်။
  `local_proxy_manifest` ထို့ကြောင့် စမ်းသပ်မှုများသည် ယုံကြည်ရသောအစုအဝေးကို ဘရောက်ဆာ အဒက်တာများထံ လွှဲပြောင်းပေးနိုင်ပါသည်။
- **JavaScript** — `sorafsMultiFetchLocal` သည် Python helper ကို ရောင်ပြန်ဟပ်ကာ ပြန်တက်လာသည်
  `sorafsGatewayFetch` လေ့ကျင့်ခန်းလုပ်နေစဉ် payload bytes နှင့် ပြေစာအနှစ်ချုပ်
  Torii ဂိတ်ဝေးများ၊ စက်တွင်းပရောက်စီ စာတွဲများကို ထင်ရှားစေပြီး တူညီသည်
  တယ်လီမီတာ/သယ်ယူပို့ဆောင်ရေးသည် CLI အဖြစ် အစားထိုးသည်။
- **Rust** — ဝန်ဆောင်မှုများသည် အချိန်ဇယားကို တိုက်ရိုက်ထည့်သွင်းနိုင်သည်။
  `sorafs_car::multi_fetch`; [Rrust SDK အတိုအထွာများ](./developer-sdk-rust.md) ကိုကြည့်ပါ
  အထောက်အထားစီးကြောင်းအကူအညီပေးသူများနှင့် တီးမှုတ်သူပေါင်းစပ်မှုအတွက် ကိုးကားချက်။
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` သည် Torii HTTP ကို ပြန်လည်အသုံးပြုသည်
  executor နှင့် `GatewayFetchOptions` ကို ဂုဏ်ပြုပါသည်။ ၎င်းကို ပေါင်းစပ်ပါ။
  `ClientConfig.Builder#setSorafsGatewayUri` နှင့် PQ အပ်လုဒ် အရိပ်အမြွက်
  အပ်လုဒ်လုပ်သည့်အခါတွင် (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) ကို ကပ်ထားရပါမည်။
  PQ တစ်ခုတည်းသောလမ်းကြောင်းများ။

## အမှတ်စာရင်းဘုတ်နှင့် မူဝါဒခလုတ်များ

Python (`sorafs_multi_fetch_local`) နှင့် JavaScript နှစ်မျိုးလုံး
အကူအညီပေးသူများ (`sorafsMultiFetchLocal`) သည် telemetry-aware-aware scheduler scoreboard ကို ဖော်ထုတ်ပြသသည်
CLI မှအသုံးပြုသည်-

- ထုတ်လုပ်မှု binaries များသည် ပုံမှန်အားဖြင့် အမှတ်စာရင်းဘုတ်ကို ဖွင့်ပေးသည်။ `use_scoreboard=True` လို့ သတ်မှတ်ပါတယ်။
  ပွဲစဉ်များကို ပြန်လည်ကစားသည့်အခါ (သို့မဟုတ် `telemetry` ထည့်သွင်းမှုများ ပံ့ပိုးပေးသည်) သို့မှသာ ကူညီသူရရှိလာမည်ဖြစ်သည်။
  ကြော်ငြာမက်တာဒေတာနှင့် မကြာသေးမီက တယ်လီမီတာ လျှပ်တစ်ပြက်ရိုက်ချက်များမှ မှာယူသော အလေးချိန်ပေးသူ။
- အတုံးများနှင့်အတူ တွက်ချက်ထားသော အလေးချိန်များကို လက်ခံရရှိရန် `return_scoreboard=True` ကို သတ်မှတ်ပါ။
  CI မှတ်တမ်းများသည် ရောဂါရှာဖွေမှုများကို ဖမ်းယူနိုင်သောကြောင့် ဖြတ်ပိုင်းများ။
- ရွယ်တူများကိုငြင်းပယ်ရန် သို့မဟုတ် ထည့်ရန် `deny_providers` သို့မဟုတ် `boost_providers` ကိုသုံးပါ။
  အစီအစဉ်ဆွဲသူက ပံ့ပိုးပေးသူများကို ရွေးချယ်သောအခါ `priority_delta`။
- အဆင့်နှိမ့်ချခြင်းမပြုပါက မူလ `"soranet-first"` ကိုယ်ဟန်အနေအထားကို ထားရှိပါ။ ထောက်ပံ့ရေး
  `"direct-only"` သည် လိုက်လျောညီထွေရှိသော ဒေသတစ်ခုရှိမှသာလျှင် relays များကို ရှောင်ရှားရမည် သို့မဟုတ် မည်သည့်အချိန်တွင်မဆို
  SNNet-5a လှည့်ကွက်ကို ပြန်လည်လေ့ကျင့်နေပြီး PQ-only အတွက် `"soranet-strict"` ကို ကြိုတင်မှာယူပါ
  အုပ်ချုပ်မှုခွင့်ပြုချက်ဖြင့် လေယာဉ်မှူးများ။
- Gateway helpers များသည် `scoreboardOutPath` နှင့် `scoreboardNowUnixSecs` ကိုလည်း ဖော်ထုတ်ပါသည်။
  တွက်ချက်ထားသော အမှတ်စာရင်းကို ဆက်ထားရန် `scoreboardOutPath` ကို သတ်မှတ်ပါ (CLI ကို မှန်ကြည့်သည်
  `--scoreboard-out` အလံ) ထို့ကြောင့် `cargo xtask sorafs-adoption-check` ကို အတည်ပြုနိုင်သည်
  ပစ္စည်းများတည်ငြိမ်ရန်လိုအပ်သောအခါ SDK ပစ္စည်းများ၊ နှင့် `scoreboardNowUnixSecs` ကိုသုံးပါ
  ပြန်လည်ထုတ်လုပ်နိုင်သော မက်တာဒေတာအတွက် `assume_now` တန်ဖိုး။ JavaScript helper ထဲမှာ ပါပါတယ်။
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ကို ထပ်မံသတ်မှတ်နိုင်သည်။
  တံဆိပ်ကို ချန်လှပ်ထားသည့်အခါ ၎င်းသည် `region:<telemetryRegion>` (နောက်ပြန်ကျသွားသည်
  `sdk:js` သို့)။ Python helper သည် `telemetry_source="sdk:python"` ကို အလိုအလျောက် ထုတ်လွှတ်သည်။
  ရမှတ်ဘုတ်တွင် ဆက်လက်တည်ရှိနေသည့်အခါတိုင်း၊ သွယ်ဝိုက်သောမက်တာဒေတာကို ပိတ်ထားသည်။

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```