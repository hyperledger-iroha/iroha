---
lang: my
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Deterministic Settlement Router (NX-3)

**အခြေအနေ-** ပြီးပါပြီ (NX-3)  
** ပိုင်ရှင်များ-** စီးပွားရေး WG / Core Ledger WG / Treasury / SRE  
** နယ်ပယ်-** လမ်းကြောများ/ဒေတာနေရာအားလုံးမှ အသုံးပြုသည့် Canonical XOR အခြေချမှုလမ်းကြောင်း။ တင်ပို့သည့် ရောက်တာသေတ္တာ၊ လမ်းသွားအဆင့် ပြေစာများ၊ ကြားခံစောင့်ရထားများ၊ တယ်လီမီတာနှင့် အော်ပရေတာ အထောက်အထား မျက်နှာပြင်များ။

## ပန်းတိုင်
- တစ်လမ်းသွားနှင့် Nexus တည်ဆောက်မှုများတစ်လျှောက် XOR ပြောင်းလဲခြင်းနှင့် လက်ခံဖြတ်ပိုင်းထုတ်လုပ်ခြင်းတို့ကို ပေါင်းစည်းပါ။
- အဆုံးအဖြတ်ပေးသောဆံပင်ညှပ်များ + အစောင့်အကြပ်များပါရှိသော မငြိမ်မသက်သောအနားသတ်များကို အသုံးပြုပါ အော်ပရေတာများသည် တည်ငြိမ်အေးချမ်းစွာဖြေရှင်းနိုင်စေရန်။
- စာရင်းစစ်များသည် စိတ်ကြိုက်ကိရိယာမပါဘဲ ပြန်ဖွင့်နိုင်သည့် ပြေစာများ၊ တယ်လီမီတာနှင့် ဒက်ရှ်ဘုတ်များကို ဖော်ထုတ်ပါ။

## ဗိသုကာ
| အစိတ်အပိုင်း | တည်နေရာ | တာဝန် |
|----------|----------------|----------------|
| Router နောက်ဆက်တွဲများ | `crates/settlement_router/` | အရိပ်-စျေးနှုန်းဂဏန်းတွက်စက်၊ ဆံပင်ညှပ်အဆင့်များ၊ ကြားခံမူဝါဒအထောက်အပံများ၊ ငွေပေးချေမှုပြေစာအမျိုးအစား။ 【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1
| Runtime façade | `crates/iroha_core/src/settlement/mod.rs:1` | Router config ကို `SettlementEngine` တွင် ထည့်သွင်းပြီး block execute အတွင်းအသုံးပြုသော `quote` + accumulator ကို ဖော်ထုတ်သည်။ |
| ပေါင်းစည်းခြင်း | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` မှတ်တမ်းများကို ထုတ်ယူပြီး၊ လမ်းသွား/ဒေတာနေရာတစ်ခုလျှင် `LaneSettlementCommitment` ကို စုစည်းကာ၊ လမ်းသွားကြားခံ မက်တာဒေတာကို ခွဲခြမ်းစိပ်ဖြာပြီး တယ်လီမီတာကို ထုတ်လွှတ်ပါသည်။ |
| ကြေးနန်းနှင့် ဒက်ရှ်ဘုတ်များ | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | ကြားခံများ၊ ကွဲလွဲမှု၊ ဆံပင်ညှပ်မှုများ၊ ပြောင်းလဲခြင်းအရေအတွက်များအတွက် Prometheus/OTLP မက်ထရစ်များ။ SRE အတွက် Grafana ဘုတ်။ |
| အကိုးအကား schema | `docs/source/nexus_fee_model.md:1` | စာရွက်စာတမ်းများ ပြေစာပြေစာ အကွက်များကို `LaneBlockCommitment` တွင် ဆက်လက်ရှိနေပါသည်။ |

## ဖွဲ့စည်းမှု
Router knob များသည် `[settlement.router]` (`iroha_config` မှ အတည်ပြုထားသည်)

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

ဒေတာအာကာသ ကြားခံအကောင့်ရှိ လမ်းသွား မက်တာဒေတာဝိုင်ယာကြိုးများ-
- `settlement.buffer_account` — စရန်ငွေ ကိုင်ဆောင်ထားသော အကောင့် (ဥပမာ၊ `buffer::cbdc_treasury`)။
- `settlement.buffer_asset` — ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် (ပုံမှန်အားဖြင့် `xor#sora`)။
- `settlement.buffer_capacity_micro` — မိုက်ခရို-XOR (ဒဿမလိုင်း) တွင် စီစဉ်သတ်မှတ်ထားသော စွမ်းရည်။

မရှိသော မက်တာဒေတာသည် ထိုလမ်းကြောအတွက် ကြားခံလျှပ်တစ်ပြက်ရိုက်ချက်များကို ပိတ်လိုက်သည် (တယ်လီမီတာသည် သုညစွမ်းရည်/အခြေအနေသို့ ပြန်ကျသွားသည်)။## ပိုက်လိုင်းပြောင်းခြင်း။
1. **ကိုးကားချက်-** `SettlementEngine::quote` သည် ပြင်ဆင်ထားသော epsilon + မငြိမ်မသက်သောအနားသတ်နှင့် ဆံပင်ညှပ်အဆင့်ကို TWAP ကိုးကားချက်များတွင် အသုံးပြုပြီး `SettlementReceipt` နှင့် `xor_due` နှင့် `xor_after_haircut` နှင့် `xor_after_haircut` နှင့် `xor_after_haircut` နှင့် `xor_after_haircut` တို့ကို ပြန်ပေးသည်။ `source_id`။ 【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Accumulate:** ပိတ်ဆို့လုပ်ဆောင်မှုအတွင်း executor သည် `PendingSettlement` သွင်းမှုများကို (ဒေသခံပမာဏ၊ TWAP၊ epsilon၊ မတည်ငြိမ်မှုပုံး၊ ငွေဖြစ်လွယ်မှုပရိုဖိုင်၊ oracle အချိန်တံဆိပ်တုံး) တို့ကို မှတ်တမ်းတင်ထားသည်။ `LaneSettlementBuilder` သည် စုစုပေါင်းများကို စုစည်းပြီး `(lane, dataspace)` နှုန်းဖြင့် မက်တာဒေတာကို ဖလှယ်ပါသည်။ 【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **buffer လျှပ်တစ်ပြက်-** လမ်းသွား မက်တာဒေတာသည် ကြားခံတစ်ခုအား ကြေညာပါက၊ တည်ဆောက်သူသည် `SettlementBufferSnapshot` (ကျန်ရှိသော headroom၊ စွမ်းရည်၊ အခြေအနေ) ကို config မှ `BufferPolicy` သတ်မှတ်ချက်များမှ ဖမ်းယူပါသည်။【crates/iroha_core/rs:20block】
4. **Commit + telemetry-** ပြေစာများနှင့် အထောက်အထား ဖလှယ်မှုများကို `LaneBlockCommitment` အတွင်းရှိ မြေယာနှင့် အခြေအနေကို လျှပ်တစ်ပြက်ပုံများအဖြစ် ထင်ဟပ်စေသည်။ Telemetry မှတ်တမ်းများသည် ကြားခံတိုင်းတာမှုများ၊ ကွဲလွဲမှု (`iroha_settlement_pnl_xor`)၊ အသုံးချအနားသတ် (`iroha_settlement_haircut_bp`)၊ ရွေးချယ်နိုင်သော လဲလှယ်အသုံးပြုမှု၊ နှင့် ပစ္စည်းတစ်ခုချင်းသို့ ပြောင်းလဲခြင်း/ဆံပင်ညှပ်ကောင်တာများသည် ဒိုင်ခွက်များနှင့် သတိပေးချက်များသည် ဘလောက်နှင့်ထပ်တူရှိနေစေရန် အကြောင်းအရာများ။ 【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **အထောက်အထား မျက်နှာပြင်များ-** `status::set_lane_settlement_commitments` သည် relays/DA သုံးစွဲသူများအတွက် ကတိကဝတ်များကို ထုတ်ပြန်သည်၊ Grafana ဒက်ရှ်ဘုတ်များသည် Prometheus မက်ထရစ်များကို ဖတ်ပြပြီး အော်ပရေတာများသည် `ops/runbooks/settlement-buffers.md`400ill/018X ကို တွဲလျက် `ops/runbooks/settlement-buffers.md`400ill0018X ကိုအသုံးပြုသည် အဖြစ်အပျက်များ။

## တယ်လီမီတာနှင့် အထောက်အထား
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — လမ်းသွား/ဒေတာနေရာအတွက် ကြားခံလျှပ်တစ်ပြက်ပုံများ (micro-XOR + ကုဒ်ဖြင့်ပြုလုပ်ထားသော အခြေအနေ)။【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — ဘလောက်အသုတ်အတွက် ဆံပင်ညှပ်ပြီးနောက် XOR နှင့် ဆံပင်ညှပ်ပြီးနောက် ကွဲလွဲမှုကို သဘောပေါက်ပါသည်။ 【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — အတွဲလိုက်အတွက် ထိရောက်သော epsilon/ဆံပင်ညှပ်အခြေခံအချက်များ။ 【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — ငွေဖြစ်လွယ်မှုပရိုဖိုင်ဖြင့် ပုံးထည့်ထားသော စိတ်ကြိုက်အသုံးပြုမှု။ 【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — အခြေချနေထိုင်မှုကူးပြောင်းမှုများနှင့် တိုးပွားလာသောဆံပင်ညှပ်များ (XOR ယူနစ်များ) အတွက် တစ်လမ်းသွား/ဒေတာအာကာသကောင်တာများ။
- Grafana ဘုတ်- `dashboards/grafana/settlement_router_overview.json` (buffer headroom၊ ကွဲပြားမှု၊ ဆံပင်ညှပ်မှုများ) နှင့် Nexus လမ်းကြောင်းသတိပေးချက်ထုပ်တွင် ထည့်သွင်းထားသော Alertmanager စည်းမျဉ်းများ။
- အော်ပရေတာ runbook- `ops/runbooks/settlement-buffers.md` (ပြန်လည်ဖြည့်သွင်းခြင်း/သတိပေးချက်လုပ်ဆောင်မှု) နှင့် `docs/source/nexus_settlement_faq.md` တွင် FAQ။## Developer & SRE စစ်ဆေးရန်စာရင်း
- `[settlement.router]` တန်ဖိုးများကို `config/config.json5` (သို့မဟုတ် TOML) တွင် သတ်မှတ်ပြီး `irohad --version` မှတ်တမ်းများမှတစ်ဆင့် အတည်ပြုပါ။ သတ်မှတ်ချက်များသည် `alert > throttle > xor_only > halt` အား ကျေနပ်မှုရှိစေရန်။
- ကြားခံအကောင့်/ပိုင်ဆိုင်မှု/စွမ်းရည်များဖြင့် လမ်းသွားမက်တာဒေတာကို ဖြည့်သွင်းခြင်းဖြင့် ကြားခံတိုင်းတာမှုများသည် တိုက်ရိုက်အရန်ငွေများကို ထင်ဟပ်စေပါသည်။ ကြားခံများကို ခြေရာခံခြင်းမပြုသင့်သော လမ်းများအတွက် အကွက်များကို ချန်လှပ်ထားပါ။
- `dashboards/grafana/settlement_router_overview.json` မှတစ်ဆင့် `settlement_router_*` နှင့် `iroha_settlement_*` တိုင်းတာချက်များကို စောင့်ကြည့်ပါ။ throttle/XOR-only/halt states တွင် သတိပေးချက်။
- စျေးနှုန်း/မူဝါဒအကျုံးဝင်မှုနှင့် `crates/iroha_core/src/block.rs` တွင် ရှိပြီးသား ပိတ်ဆို့အဆင့် ပေါင်းစပ်စစ်ဆေးမှုများအတွက် `cargo test -p settlement_router` ကို လုပ်ဆောင်ပါ။
- `docs/source/nexus_fee_model.md` တွင် config အပြောင်းအလဲများအတွက် အုပ်ချုပ်မှုခွင့်ပြုချက်များကို မှတ်တမ်းတင်ပြီး အဆင့်သတ်မှတ်ချက်များ သို့မဟုတ် တယ်လီမီတာမျက်နှာပြင်များ ပြောင်းလဲသည့်အခါ `status.md` ကို မွမ်းမံထားပါ။

## စတင်မိတ်ဆက်ခြင်း အစီအစဉ် လျှပ်တစ်ပြက်
- တည်ဆောက်မှုတိုင်းတွင် Router + telemetry သင်္ဘော။ အင်္ဂါရပ်ဂိတ်များမရှိပါ။ Lane မက်တာဒေတာသည် ကြားခံလျှပ်တစ်ပြက်ရိုက်ချက်များကို ဖြန့်ချိခြင်းရှိမရှိ ထိန်းချုပ်သည်။
- ပုံသေ config သည် လမ်းပြမြေပုံတန်ဖိုးများ (60s TWAP၊ 25bp အခြေခံ epsilon၊ 72h ကြားခံမိုးကုတ်စက်ဝိုင်း) နှင့် ကိုက်ညီပါသည်။ အသုံးပြုရန်အတွက် config မှတဆင့်ညှိပြီး `irohad` ကို ပြန်လည်စတင်ပါ။
- အထောက်အထားအစုအဝေး = လမ်းကြောဖြေရှင်းရေးကတိကဝတ်များ + Prometheus `settlement_router_*`/`iroha_settlement_*` စီးရီးအတွက် + Grafana ဖန်သားပြင်ဓာတ်ပုံ/JSON ထုတ်ယူမှုအတွက် ခြစ်ပါ။

## အထောက်အထားများနှင့် ကိုးကားချက်များ
- NX-3 အခြေချရောက်တာ လက်ခံမှတ်စုများ- `status.md` (NX-3 အပိုင်း)။
- အော်ပရေတာမျက်နှာပြင်များ- `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`။
- ပြေစာအစီအစဉ်နှင့် API မျက်နှာပြင်များ- `docs/source/nexus_fee_model.md`, `/v2/sumeragi/status` -> `lane_settlement_commitments`။