---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Multi-Source Provider ကြော်ငြာများနှင့် အချိန်ဇယားဆွဲခြင်း။

ဤစာမျက်နှာသည် canonical spec ကို ခွဲထုတ်ထားသည်။
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)။
Norito အစီအစဉ်များနှင့် ပြောင်းလဲမှုမှတ်တမ်းများအတွက် ထိုစာရွက်စာတမ်းကို အသုံးပြုပါ။ portal ကော်ပီ
အော်ပရေတာလမ်းညွှန်ချက်၊ SDK မှတ်စုများနှင့် တယ်လီမီတာ အကိုးအကားများကို ကျန်အရာများနှင့် နီးကပ်စွာထားပါ။
SoraFS ၏ ရှေ့ပြေးစာအုပ်များ။

## Norito schema ထပ်လောင်းမှုများ

### အတိုင်းအတာ စွမ်းရည် (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - တောင်းဆိုမှုတစ်ခုအတွက် အကြီးဆုံးသော ဆက်စပ်နေသော အတိုင်းအတာ (ဘိုက်များ)၊ `≥ 1`။
- `min_granularity` - ကြည်လင်ပြတ်သားမှုကို ရှာပါ၊ `1 ≤ value ≤ max_chunk_span`။
- `supports_sparse_offsets` - တောင်းဆိုချက်တစ်ခုတွင် ဆက်တိုက်မဟုတ်သော အော့ဖ်ဆက်များကို ခွင့်ပြုသည်။
- `requires_alignment` – မှန်သောအခါ၊ အော့ဖ်ဆက်များသည် `min_granularity` နှင့် ချိန်ညှိရပါမည်။
- `supports_merkle_proof` - PoR သက်သေ ပံ့ပိုးမှုကို ညွှန်ပြသည်။

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` သည် canonical encoding ကို အသုံးပြုရန်
ထို့ကြောင့် အတင်းအဖျင်း ဝန်ထုပ်ဝန်ပိုးများသည် အဆုံးအဖြတ်အတိုင်း ရှိနေသည်။

### `StreamBudgetV1`
- အကွက်များ- `max_in_flight`၊ `max_bytes_per_sec`၊ ရွေးချယ်နိုင်သော `burst_bytes`။
- အတည်ပြုခြင်းစည်းမျဉ်းများ (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`။
  - `burst_bytes`၊ လက်ရှိအချိန်တွင် `> 0` နှင့် `≤ max_bytes_per_sec` ဖြစ်ရမည်။

### `TransportHintV1`
- အကွက်များ- `protocol: TransportProtocol`၊ `priority: u8` (0-15 ဝင်းဒိုးဖြင့် ပြဌာန်းထားသည်
  `TransportHintV1::validate`)။
- လူသိများသော ပရိုတိုကောများ- `torii_http_range`၊ `quic_stream`၊ `soranet_relay`၊
  `vendor_reserved`။
- ဝန်ဆောင်မှုပေးသူတစ်ဦးစီမှ မိတ္တူပရိုတိုကော ထည့်သွင်းမှုများကို ပယ်ချပါသည်။

### `ProviderAdvertBodyV1` အပိုများ
- ရွေးချယ်နိုင်သော `stream_budget: Option<StreamBudgetV1>`။
- ရွေးချယ်နိုင်သော `transport_hints: Option<Vec<TransportHintV1>>`။
- ယခုအခါ နယ်ပယ်နှစ်ခုစလုံးသည် `ProviderAdmissionProposalV1`၊ အုပ်ချုပ်ရေးကို ဖြတ်သန်းစီးဆင်းနေသည်။
  စာအိတ်များ၊ CLI ကိရိယာများနှင့် တယ်လီမက်ထရစ် JSON။

## အတည်ပြုခြင်းနှင့် အုပ်ချုပ်မှု စည်းနှောင်ခြင်း။

`ProviderAdvertBodyV1::validate` နှင့် `ProviderAdmissionProposalV1::validate`
ပုံစံမမှန်သော မက်တာဒေတာကို ငြင်းပယ်ပါ-

- Range စွမ်းရည်များသည် span/granularity ကန့်သတ်ချက်များကို ကုဒ်နှင့် ကျေနပ်စေရပါမည်။
- တိုက်ရိုက်ထုတ်လွှင့်မှုဘတ်ဂျက်များ / သယ်ယူပို့ဆောင်ရေးအရိပ်အမြွက်များနှင့်ကိုက်ညီမှုလိုအပ်သည်။
  `CapabilityType::ChunkRangeFetch` TLV နှင့် ဗလာမဟုတ်သော အရိပ်အမြွက်စာရင်း။
- သယ်ယူပို့ဆောင်ရေးပရိုတိုကောများ ပွားခြင်းနှင့် မမှန်ကန်သော ဦးစားပေးမှုများသည် တရားဝင်မှုကို မြှင့်တင်ပါ။
  ကြော်ငြာများအတင်းပြောခြင်းမပြုမီ အမှားများ။
- ဝင်ခွင့်စာအိတ်များမှ တစ်ဆင့် အပိုင်းအခြား metadata အတွက် အဆိုပြုချက်/ကြော်ငြာများကို နှိုင်းယှဉ်ပါ။
  `compare_core_fields` ထို့ကြောင့် မကိုက်ညီသော အတင်းအဖျင်းပေးချေမှုများကို စောစောစီးစီး ပယ်ချပါသည်။

Regression coverage တွင် နေထိုင်ပါသည်။
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`။

## ကိရိယာတန်ဆာပလာများ

- ဝန်ဆောင်မှုပေးသူ ကြော်ငြာပေးချေမှုတွင် `range_capability`၊ `stream_budget` နှင့်
  `transport_hints` မက်တာဒေတာ။ `/v2/sorafs/providers` တုံ့ပြန်မှုများနှင့် သက်သေပြပါ။
  ဝင်ခွင့်ပွဲများ; JSON အနှစ်ချုပ်များတွင် ခွဲခြမ်းစိတ်ဖြာနိုင်စွမ်း ပါဝင်သင့်သည်၊
  ထုတ်လွှင့်သောဘတ်ဂျက်နှင့် တယ်လီမက်ထရီထည့်သွင်းမှုအတွက် အရိပ်အမြွက်ပေးသည့် အခင်းအကျင်းများ။
- `cargo xtask sorafs-admission-fixtures` သည် ဘတ်ဂျက်များနှင့် သယ်ယူပို့ဆောင်ရေးလမ်းကြောင်းများကို ထုတ်ပေးသည်။
  ၎င်း၏ JSON လက်ရာများအတွင်းတွင် အရိပ်အမြွက်ပြသထားသောကြောင့် ဒက်ရှ်ဘုတ်များသည် အင်္ဂါရပ်ကို လက်ခံကျင့်သုံးမှုကို ခြေရာခံပါသည်။
- ယခု `fixtures/sorafs_manifest/provider_admission/` အောက်တွင် အကျုံးဝင်သည်-
  - canonical multi-source ကြော်ငြာများ၊
  - `multi_fetch_plan.json` သို့မှသာ SDK suites များသည် အဆုံးအဖြတ်ပေးသော multi-peer ကို ပြန်လည်ပြသနိုင်သည်
    အကျိူးအစီအစဉ်။

## Orchestrator & Torii ပေါင်းစပ်မှု

- Torii `/v2/sorafs/providers` သည် ခွဲခြမ်းစိတ်ဖြာနိုင်စွမ်းရှိသော မက်တာဒေတာကို တစ်ပါတည်း ပြန်ပေးသည်
  `stream_budget` နှင့် `transport_hints`။ သတိပေးချက်များ မီးလောင်နေသည်ကို အဆင့်နှိမ့်ချပါ။
  ဝန်ဆောင်မှုပေးသူများသည် မက်တာဒေတာအသစ်ကို ချန်လှပ်ထားကာ ဂိတ်ဝေးအကွာအဝေး အဆုံးအမှတ်များသည် အတူတူပင်ဖြစ်ပါသည်။
  တိုက်ရိုက်ဖောက်သည်များအတွက် ကန့်သတ်ချက်များ။
- ရင်းမြစ်ပေါင်းစုံ သံစုံတီးဝိုင်း (`sorafs_car::multi_fetch`) သည် ယခုအခါ အပိုင်းအခြားကို တွန်းအားပေးနေသည်
  အလုပ်တာဝန်ပေးသည့်အခါ ကန့်သတ်ချက်များ၊ စွမ်းဆောင်ရည် ချိန်ညှိမှုနှင့် တိုက်ရိုက်ဘတ်ဂျက်များ။ ယူနစ်
  စမ်းသပ်မှုများသည် အတုံးအခဲကြီးလွန်းသော၊ ကျဲကျဲရှာသော၊ နှင့် အဟန့်အတားဖြစ်စေသော အခြေအနေများကို အကျုံးဝင်ပါသည်။
- `sorafs_car::multi_fetch` သည် အဆင့်နှိမ့်ထားသော အချက်ပြမှုများကို ထုတ်လွှင့်သည် (ချိန်ညှိမှု မအောင်မြင်မှုများ၊
  throttled တောင်းဆိုမှုများ) ထို့ကြောင့် အော်ပရေတာများသည် တိကျသောပံ့ပိုးပေးသူများ အဘယ်ကြောင့်ဖြစ်သည်ကို ခြေရာခံနိုင်သည်။
  စီစဉ်နေစဉ်အတွင်း ကျော်သွားခဲ့သည်။

## Telemetry အကိုးအကား

Torii အကွာအဝေး ထုတ်ယူမှု ကိရိယာသည် **SoraFS ထုတ်ယူနိုင်မှု** ကို ကျွေးမွေးသည်
Grafana ဒိုင်ခွက် (`dashboards/grafana/sorafs_fetch_observability.json`) ကိုလည်းကောင်း၊
တွဲထားသည့် သတိပေးချက်စည်းမျဉ်းများ (`dashboards/alerts/sorafs_fetch_rules.yml`)။

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | ဖော်ပြချက် |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | တိုင်းထွာ | `feature` (`providers`၊ `supports_sparse_offsets`၊ `requires_alignment`၊ `supports_merkle_proof`၊ `stream_budget`၊ `transport_hints`) ဝန်ဆောင်မှုပေးသူများ ကြော်ငြာပေးနိုင်စွမ်း အင်္ဂါရပ်များ။ |
| `torii_sorafs_range_fetch_throttle_events_total` | ကောင်တာ | `reason` (`quota`, `concurrency`, `byte_rate`) | မူဝါဒဖြင့် အုပ်စုဖွဲ့ထားသော ကန့်သတ်ထားသော အပိုင်းအခြားကို ရယူရန် ကြိုးပမ်းမှုများ။ |
| `torii_sorafs_range_fetch_concurrency_current` | တိုင်းထွာ | — | မျှဝေထားသော ပေါင်းစပ်ငွေကြေးဘတ်ဂျက်ကို အသုံးပြုသည့် အကာအကွယ်ပေးထားသော လမ်းကြောင်းများ။ |

ဥပမာ PromQL အတိုအထွာများ-

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

မဖွင့်မီ ခွဲတမ်းအား အတည်ပြုရန် အခိုးအငွေ့ကောင်တာကို အသုံးပြုပါ။
ရင်းမြစ်ပေါင်းစုံ တီးမှုတ်သူသည် ပုံသေသတ်မှတ်ထားပြီး တူညီသောငွေကြေးသည် ချဉ်းကပ်လာသည့်အခါ သတိပေးချက်
သင့်ရေယာဉ်အတွက် ဘတ်ဂျက် အမြင့်ဆုံးကို တိုက်ရိုက်ထုတ်လွှင့်ပါ။