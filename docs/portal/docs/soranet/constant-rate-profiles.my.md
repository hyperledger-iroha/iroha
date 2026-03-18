---
lang: my
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7334c5f2ccfa93c15a0827390e78b6026bb65e80ac9d624321da84f2287ce581
source_last_modified: "2026-01-05T09:28:11.911258+00:00"
translation_last_reviewed: 2026-02-07
id: constant-rate-profiles
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

SNNet-17B သည် ပုံသေနှုန်းထား သယ်ယူပို့ဆောင်ရေးလမ်းကြောင်းများကို မိတ်ဆက်ထားသောကြောင့် relay များသည် 1,024 B ဆဲလ်များတွင် အသွားအလာများကို ရွေ့လျားစေသည်
payload အရွယ်အစား။ အော်ပရေတာများသည် ကြိုတင်သတ်မှတ်မှုသုံးခုမှ ရွေးသည်-

- **core** - data-centre သို့မဟုတ် professionally host relay များကို ကာမိစေရန် >=30 Mbps ကို အပ်နှံနိုင်ပါသည်။
  အသွားအလာ။
- **အိမ်** - အမည်မသိ ထုတ်ယူမှုများ လိုအပ်နေသေးသော လူနေရပ်ကွက် သို့မဟုတ် အောက်လင့်ခ် အော်ပရေတာများ
  လျှို့ဝှက်ရေး-အရေးပါသော ဆားကစ်များ။
- **null** - SNNet-17A2 စမ်းသပ်မှု ကြိုတင်သတ်မှတ်မှု။ ၎င်းသည် တူညီသော TLVs/စာအိတ်များကို ထိန်းသိမ်းထားသော်လည်း ၎င်းကို ဆန့်သည်။
  low-bandwidth အဆင့်အတွက် tick နှင့် မျက်နှာကျက်။

## ကြိုတင်သတ်မှတ် အကျဉ်းချုပ်

| ကိုယ်ရေးအကျဉ်း | အမှန်ခြစ် (ms) | Cell (B) | လမ်းသွားဦးထုပ် | ထုံကျဉ်ခင်း | တစ်လမ်းသွား ဝန်အား (Mb/s) | Ceiling payload (Mb/s) | uplink | ၏မျက်နှာကျက် % အကြံပြုထားသော လင့်ခ် (Mb/s) | အိမ်နီးနားချင်း ဦးထုပ် | အလိုအလျောက်ပိတ်ရန် အစပျိုး (%) |
|--------|-----------|----------------|----------------|----------------|------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------|
| အူတိုင် | 5.0 | 1024 | 12 | 4 | 1.64 | 19.50 | ၆၅ | 30.0 | 8 | 85 |
| အိမ် | 10.0 | 1024 | 4 | 2 | 0.82 | 4.00 | ၄၀ | 10.0 | 2 | 70 |
| null | 20.0 | 1024 | 2 | ၁ | 0.41 | 0.75 | 15 | 5.0 | ၁ | 55 |

- **လမ်းသွားအဖုံး** - အများဆုံး တစ်ပြိုင်တည်း အဆက်မပြတ်နှုန်း အိမ်နီးချင်းများ။ relay သည် အပိုဆားကစ်များကို တစ်ကြိမ်ငြင်းပယ်သည်။
  ဦးထုပ်ကို ထိပြီး `soranet_handshake_capacity_reject_total` ကို တိုးပေးပါ။
- **Dummy floor** - လက်တွေ့တွင်တောင်မှ dummy traffic ဖြင့် အသက်ရှင်နေနိုင်သော အနည်းဆုံးလမ်းသွားအရေအတွက်
  ဝယ်လိုအားက နည်းတယ်။
- **Ceiling payload** - မျက်နှာကျက်ကိုအသုံးပြုပြီးနောက် အဆက်မပြတ်နှုန်းလမ်းကြောင်းများအတွက် ရည်ရွယ်ထားသော uplink ဘတ်ဂျက်
  အပိုင်း အော်ပရေတာများသည် အပို bandwidth ရှိလျှင်ပင် ဤဘတ်ဂျက်ကို ဘယ်တော့မှ မကျော်လွန်သင့်ပါ။
- **အော်တို-ပိတ်စထရစ်** - ရွှဲစိုမှုရာခိုင်နှုန်း (ကြိုတင်သတ်မှတ်မှုတစ်ခုလျှင် ပျမ်းမျှအားဖြင့်)၊
  dummy ကြမ်းပြင်သို့ကျဆင်းရန် runtime ။ ပြန်လည်ရယူခြင်းအဆင့်ပြီးနောက် စွမ်းဆောင်ရည်ကို ပြန်လည်ရယူသည်။
  (`core` အတွက် 75%၊ `home` အတွက် 60%၊ `null` အတွက် 45%)။

**အရေးကြီးသည်-** `null` ကြိုတင်သတ်မှတ်မှုသည် အဆင့်မြှင့်တင်ခြင်းနှင့် စွမ်းရည်မြှင့်ခြင်းအတွက်သာဖြစ်သည်။ မကိုက်ညီပါ။
ထုတ်လုပ်မှုဆားကစ်များအတွက် လိုအပ်သော ကိုယ်ရေးကိုယ်တာအာမခံချက်။

## အမှတ်ခြစ် -> bandwidth table

payload cell တစ်ခုစီတွင် 1,024 B ပါသောကြောင့် KiB/sec ကော်လံသည် တစ်ခုလျှင် ထုတ်လွှတ်သောဆဲလ်အရေအတွက်နှင့် ညီမျှသည်
ဒုတိယ။ စိတ်ကြိုက်အမှန်ခြစ်များဖြင့် ဇယားကို တိုးချဲ့ရန် အထောက်အကူကို အသုံးပြုပါ။

| အမှန်ခြစ် (ms) | ဆဲလ်/စက္ကန့် | ပေးချေမှု KiB/sec | ပေးချေမှု Mb/s |
|----------|-----------------|-----------------|-----------------|
| 5.0 | 200.00 | 200.00 | 1.64 |
| ၇.၅ | 133.33 | 133.33 | 1.09 |
| 10.0 | 100.00 | 100.00 | 0.82 |
| 15.0 | 66.67 | 66.67 | 0.55 |
| 20.0 | 50.00 | 50.00 | 0.41 |

ဖော်မြူလာ-

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI အကူအညီပေးသူ-

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` သည် ကြိုတင်သတ်မှတ်ထားသော အကျဉ်းချုပ်နှင့် ရွေးချယ်နိုင်သော tick cheat နှစ်ခုလုံးအတွက် GitHub ပုံစံဇယားများကို ထုတ်လွှတ်သည်
ထို့ကြောင့် သင်သည် အဆုံးအဖြတ်ရလဒ်ကို ပေါ်တယ်သို့ ကူးထည့်နိုင်သည်။ ၎င်းကို သိမ်းဆည်းရန် `--json-out` နှင့် တွဲပါ။
အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားအတွက် ပြန်ဆိုထားသော အချက်အလက်။

## ဖွဲ့စည်းမှုပုံစံနှင့် အစားထိုးမှုများ

`tools/soranet-relay` သည် config ဖိုင်များနှင့် runtime overrides နှစ်ခုလုံးတွင် ကြိုတင်သတ်မှတ်မှုများကို ဖော်ထုတ်သည်-

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

config key သည် `core`၊ `home` သို့မဟုတ် `null` (မူလ `core`) ကို လက်ခံပါသည်။ CLI overrides များသည် အသုံးဝင်သည်။
ပြင်ဆင်မှုများကို ပြန်လည်ရေးသားခြင်းမပြုဘဲ တာဝန်စက်ဝန်းကို ယာယီလျှော့ချသည့် လေ့ကျင့်ခန်းများ သို့မဟုတ် SOC တောင်းဆိုမှုများ။

## MTU အကာအရံများ

- Payload cells သည် Norito+Noise framing ၏ 1,024 B နှင့် ~96 B ကို အသုံးပြုပြီး QUIC/UDP ခေါင်းစီးများကို အနည်းငယ်သာ အသုံးပြုပါသည်။
  datagram တစ်ခုစီကို IPv6 1,280 B အနိမ့်ဆုံး MTU အောက်တွင်ထားရှိခြင်း။
- ဥမင်လှိုဏ်ခေါင်းများ (WireGuard/IPsec) အပို encapsulation ထပ်ထည့်သောအခါ သင် ** `padding.cell_size` ကို လျှော့ချရပါမည် **
  ဒီတော့ `cell_size + framing <= 1,280 B`။ relay validator မှ ပြဋ္ဌာန်းထားသည်။
  `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 overhead - 96 B ဘောင်)။
- `core` ပရိုဖိုင်များသည် >=4 အိမ်နီးနားချင်းများကို ပျင်းရိနေချိန်၌ပင် ပင်ထိုးထားသင့်သည် ဖြစ်သောကြောင့် dummy လမ်းသွားများသည် အပိုင်းခွဲတစ်ခုကို အမြဲဖုံးလွှမ်းထားသည်။
  PQ အစောင့်များ။ `home` ပရိုဖိုင်များသည် ပိုက်ဆံအိတ်များ/စုစည်းမှုများအတွက် အဆက်မပြတ်နှုန်းဆားကစ်များကို ကန့်သတ်ထားနိုင်သော်လည်း အသုံးပြုရမည်
  telemetry windows သုံးခုအတွက် saturation 70% ကျော်လွန်သောအခါ back-pressure

## တယ်လီမီတာနှင့် သတိပေးချက်များ

Relay များသည် ကြိုတင်သတ်မှတ်မှုအလိုက် အောက်ပါ မက်ထရစ်များကို ထုတ်ယူသည်-

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

သတိပေးချက်-

1. Dummy အချိုးသည် ကြိုတင်သတ်မှတ်ကြမ်းပြင် (`core >= 4/8`၊ `home >= 2/2`၊ `null >= 1/1`) ထက် ပိုနေပါသည်
   ပြတင်းပေါက်နှစ်ခု။
2. `soranet_constant_rate_ceiling_hits_total` သည် ငါးမိနစ်လျှင် တစ်ကြိမ်နှုန်းထက် ပိုမြန်သည်။
3. `soranet_constant_rate_degraded` သည် စီစဉ်ထားသော လေ့ကျင့်ခန်းအပြင်ဘက်တွင် `1` သို့ ပြောင်းသည်။

စာရင်းစစ်များသည် အဆက်မပြတ်နှုန်းကို သက်သေပြနိုင်စေရန် ကြိုတင်သတ်မှတ်ထားသော အညွှန်းနှင့် အိမ်နီးနားချင်းစာရင်းကို အဖြစ်အပျက်အစီရင်ခံစာများတွင် မှတ်တမ်းတင်ပါ။
မူဝါဒများသည် လမ်းပြမြေပုံလိုအပ်ချက်များနှင့် ကိုက်ညီပါသည်။