---
id: nexus-transition-notes
lang: my
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

#Nexus အကူးအပြောင်းမှတ်စုများ

ဤမှတ်တမ်းသည် **PhaseB — Nexus အသွင်ကူးပြောင်းရေးဖောင်ဒေးရှင်း** ၏လုပ်ဆောင်မှုကို ခြေရာခံသည်
လမ်းသွားပေါင်းများစွာ ပစ်လွှတ်မှု စစ်ဆေးရေးစာရင်း မပြီးမချင်း။ ၎င်းသည် မှတ်တိုင်ကို အားဖြည့်ပေးသည်။
`roadmap.md` တွင် ထည့်သွင်းမှုများနှင့် B1–B4 မှကိုးကားထားသော အထောက်အထားများကို တစ်နေရာတည်းတွင် သိမ်းဆည်းသည်
ထို့ကြောင့် အုပ်ချုပ်ရေး၊ SRE နှင့် SDK ခေါင်းဆောင်များသည် တူညီသောအမှန်တရားအရင်းအမြစ်ကို မျှဝေနိုင်ပါသည်။

## Scope & Cadence

- လမ်းကြောင်းမှခြေရာခံစစ်ဆေးမှုများနှင့် telemetry guardrails (B1/B2) ကို ဖုံးအုပ်ပေးခြင်း၊
  အုပ်ချုပ်မှု-အတည်ပြုထားသော ဖွဲ့စည်းမှုမြစ်ဝကျွန်းပေါ်အစုံ (B3) နှင့် လမ်းသွားများစွာကို လွှင့်တင်ခြင်း။
  အစမ်းလေ့ကျင့်မှု နောက်ဆက်တွဲ (B4)။
- ယခင်က ဤနေရာတွင်နေထိုင်ခဲ့သော ယာယီ ကုဒ်မှတ်စုကို အစားထိုးပါ။ 2026 ခုနှစ်အထိ
  Q1 တွင် စာရင်းစစ်သည် အသေးစိတ် အစီရင်ခံစာတွင် တည်ရှိသည်။
  `docs/source/nexus_routed_trace_audit_report_2026q1.md` ဤစာမျက်နှာကို ပိုင်ဆိုင်နေစဉ်
  အပြေးအချိန်ဇယားနှင့် လျော့ပါးရေးစာရင်း။
- ခြေရာခံပြတင်းပေါက်တစ်ခုစီတိုင်း၊ အုပ်ချုပ်မှုမဲပေးခြင်း သို့မဟုတ် စတင်ပြီးနောက် ဇယားများကို အပ်ဒိတ်လုပ်ပါ။
  အစမ်းလေ့ကျင့်မှု။ အနုပညာပစ္စည်းများ ရွှေ့သည့်အခါတိုင်း၊ ဤစာမျက်နှာအတွင်းရှိ တည်နေရာအသစ်ကို ရောင်ပြန်ဟပ်ပါ။
  ထို့ကြောင့် downstream docs (အခြေအနေ၊ ဒက်ရှ်ဘုတ်များ၊ SDK ပေါ်တယ်များ) သည် တည်ငြိမ်သောတစ်ခုသို့ ချိတ်ဆက်နိုင်သည်။
  ကျောက်ဆူး။

## အထောက်အထား လျှပ်တစ်ပြက် (2026 Q1–Q2)

| အလုပ်လမ်းကြောင်း | အထောက်အထား | ပိုင်ရှင်(များ) | အဆင့်အတန်း | မှတ်စုများ |
|--------------------|----------|----------------|--------|--------|
| **B1 — Routed-trace စစ်ဆေးမှု** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops၊ @governance | ✅ အပြီးသတ် (Q1 2026) | စာရင်းစစ်ပြတင်းပေါက်သုံးခုကို မှတ်တမ်းတင်ထားသည်။ Q2 ပြန်လည်လည်ပတ်မှုအတွင်း `TRACE-CONFIG-DELTA` မှ TLS နောက်ကျကျန်ခဲ့သည်။ |
| **B2 — တယ်လီမီတာ ပြုပြင်ခြင်း နှင့် အကာအရံများ** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core၊ @telemetry-ops | ✅ အပြီးအစီး | သတိပေးချက်အထုပ်၊ ကွဲပြားသော ဘော့တ်မူဝါဒနှင့် OTLP အသုတ်အရွယ်အစား (`nexus.scheduler.headroom` မှတ်တမ်း + Grafana headroom panel) ပို့လိုက်သည်; ပွင့်လင်းစွာ စွန့်လွှတ်ခြင်း မရှိပါ။ |
| **B3 — Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | ✅ အပြီးအစီး | GOV-2026-03-19 မဲများကို ဖမ်းယူထားသည်။ လက်မှတ်ထိုးထားသောအတွဲသည် အောက်တွင်ဖော်ပြထားသော telemetry pack ကိုကျွေးသည်။ |
| **B4 — လမ်းသွားလမ်းသွား အစမ်းလေ့ကျင့်မှု** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | ✅ အပြီးသတ် (Q2 2026) | Q2 Canary rerun သည် TLS နောက်ကျခြင်းကို လျော့ပါးစေပါသည်။ validator manifest + `.sha256` ဖမ်းယူမှု အထိုင်အကွာအဝေး 912–936၊ workload seed `NEXUS-REH-2026Q2` နှင့် rerun မှ မှတ်တမ်းတင်ထားသော TLS ပရိုဖိုင် hash။ |

## သုံးလပတ်လမ်းကြောင်း-ခြေရာခံစာရင်းစစ်ဇယား

| ခြေရာခံ ID | Window (UTC) | ရလဒ် | မှတ်စုများ |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ Pass | တန်းစီ-ဝင်ခွင့် P95 သည် ≤ 750ms ပစ်မှတ်အောက်၌ ကောင်းမွန်စွာနေခဲ့သည်။ လုပ်ဆောင်ရန်မလိုအပ်ပါ။ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ Pass | `status.md` နှင့် တွဲထားသည့် OTLP ပြန်လည်ဖွင့်သည့် ဟက်ရှ်များ SDK diff bot parity သည် zero drift ကို အတည်ပြုသည်။ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ ခါးခါး | Q2 ပြန်လည်လည်ပတ်နေစဉ် TLS ပရိုဖိုင် နောက်ကျခြင်းကို ပိတ်ထားသည်။ `NEXUS-REH-2026Q2` မှတ်တမ်းများ TLS ပရိုဖိုင် hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/` ကိုကြည့်ပါ) နှင့် သုည stragglers အတွက် telemetry pack ။ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ Pass | အလုပ်ဝန်မျိုးစေ့ `NEXUS-REH-2026Q2`; `artifacts/nexus/rehearsals/2026q2/` တွင် အစီအစဉ်နှင့်အတူ `artifacts/nexus/rehearsals/2026q1/` (အထိုင်အကွာအဝေး 912–936) အောက်တွင် တယ်လီမီတာ ပက်ကေ့ + မန်နီးဖက်စ်/ချေဖျက်မှု။ |

အနာဂတ်ရပ်ကွက်များသည် အတန်းအသစ်များထည့်ကာ ရွှေ့သင့်သည်။
ဇယားသည် လက်ရှိထက် ကျော်လွန်လာသောအခါတွင် ဖြည့်စွက်ချက်တစ်ခုသို့ ဖြည့်သွင်းမှုများ
လေးပုံတစ်ပုံ လမ်းကြောင်းမှ ခြေရာခံ အစီရင်ခံစာများ သို့မဟုတ် အုပ်ချုပ်မှု မိနစ်များမှ ဤအပိုင်းကို ကိုးကားပါ။
`#quarterly-routed-trace-audit-schedule` ကျောက်ဆူးကို အသုံးပြု.

## လျော့ပါးရေးနှင့် Backlog ပစ္စည်းများ

| အမျိုးအမည် | ဖော်ပြချက် | ပိုင်ရှင် | ပစ်မှတ် | အဆင့်အတန်း / မှတ်စုများ |
|--------|----------------|------|--------------------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` ကာလအတွင်း နောက်ကျကျန်ခဲ့သော TLS ပရိုဖိုင်ကို ဖြန့်ကျက်ပြီး သက်သေအထောက်အထားများ ဖမ်းယူကာ လျှော့ချရေးမှတ်တမ်းကို ပိတ်လိုက်ပါ။ | @release-eng, @sre-core | Q2 2026 လမ်းကြောင်းမှ ခြေရာကောက် ဝင်းဒိုး | ✅ ပိတ်ထားသည် — TLS ပရိုဖိုင် hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ကို `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` တွင် ရိုက်ကူးထားသည်။ rerun သည် stragglers မရှိကြောင်းအတည်ပြုခဲ့သည်။ |
| `TRACE-MULTILANE-CANARY` ကြိုတင်ပြင်ဆင် | Q2 အစမ်းလေ့ကျင့်မှုကို အချိန်ဇယားဆွဲပါ၊ တယ်လီမီတာအထုပ်တွင် ကိရိယာများကို ပူးတွဲပါနှင့် SDK ကြိုးများသည် တရားဝင်သော အကူအညီကို ပြန်လည်အသုံးပြုကြောင်း သေချာပါစေ။ | @telemetry-ops၊ SDK ပရိုဂရမ် | 2026-04-30 | ခေါ်ဆိုရန် စီစဉ်ခြင်း။ ✅ ပြီးပါပြီ — `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` တွင် slot/workload metadata ဖြင့် သိမ်းဆည်းထားသော အစီအစဉ်။ Tracker တွင်ဖော်ပြထားသောကြိုးကိုပြန်လည်အသုံးပြုပါ။ |
| Telemetry pack ၏ အချေအတင်လည်ပတ်မှု | အစမ်းလေ့ကျင့်မှု/ထုတ်လွှတ်မှုတစ်ခုစီတိုင်းကို config delta tracker ဘေးတွင် မှတ်တမ်းမတင်မီ `scripts/telemetry/validate_nexus_telemetry_pack.py` ကို run ပါ။ | @telemetry-ops | လွှတ် တော် ကိုယ်စားလှယ်လောင်း တစ်ဦးလျှင် | ✅ ပြီးပါပြီ — `telemetry_manifest.json` + `.sha256` တွင် ထုတ်လွှတ်သော `artifacts/nexus/rehearsals/2026q1/` (အပေါက်အကွာအဝေး `912-936`၊ မျိုးစေ့ `NEXUS-REH-2026Q2`); ခြေရာခံကိရိယာနှင့် အထောက်အထားအညွှန်းသို့ ကူးယူထားသော မှတ်တမ်းများ။ |

## Config Delta Bundle ပေါင်းစပ်မှု

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ကျန်ရှိနေပါသည်။
  canonical diff အနှစ်ချုပ်။ `defaults/nexus/*.toml` သို့မဟုတ် ဥပါဒ်အသစ်ပြောင်းသောအခါ
  မြေသား၊ ထိုခြေရာခံကိရိယာကို ဦးစွာ အပ်ဒိတ်လုပ်ပါ၊ ထို့နောက် ပေါ်လွင်ချက်များကို ဤနေရာတွင် အလင်းပြန်ကြည့်ပါ။
- လက်မှတ်ရေးထိုးထားသော config အစုအဝေးများသည် အစမ်းလေ့ကျင့်မှု telemetry pack ကို ကျွေးမွေးပါသည်။ အထုပ်၊ အတည်ပြုပြီး
  `scripts/telemetry/validate_nexus_telemetry_pack.py` ဖြင့် ထုတ်ဝေရပါမည်။
  config delta အထောက်အထားများနှင့်အတူ အော်ပရေတာများသည် အတိအကျပြန်ဖွင့်နိုင်သည်။
  B4 ကာလအတွင်းအသုံးပြုသောပစ္စည်းများ။
- Iroha အတွဲ 2 ခုသည် လမ်းသွားလမ်းသွား-အခမဲ့ ရှိနေသည်- ယခု `nexus.enabled = false` ဖြင့် ပြင်ဆင်မှုများ
  Nexus ပရိုဖိုင်ကို ဖွင့်မထားပါက လမ်းကြော/ဒေတာနေရာ/လမ်းကြောင်းလမ်းကြောင်းကို ပယ်ချသည်
  (`--sora`)၊ ထို့ကြောင့် `nexus.*` အပိုင်းများကို လမ်းသွားပုံစံများထဲမှ ဖယ်ရှားလိုက်ပါ။
- အုပ်ချုပ်မှုမဲစာရင်း (GOV-2026-03-19) ကို ခြေရာခံကိရိယာနှင့် လင့်ခ်ချိတ်ထားပါ။
  ဤမှတ်စုသည် နောင်မဲများကို ပြန်လည်ရှာဖွေခြင်းမပြုဘဲ ဖော်မတ်ကို ကူးယူနိုင်ပါသည်။
  ခွင့်ပြုချက်ထုံးတမ်း။

## Launch Rehearsal Follow-Ups

- `docs/source/runbooks/nexus_multilane_rehearsal.md` ကိန္နရီအစီအစဉ်ကို ဖမ်းယူသည်၊
  ပါဝင်သူစာရင်းနှင့် နောက်ပြန်လှည့်ခြင်းအဆင့်များ လမ်းသွားသည့်အခါတိုင်း runbook ကို refresh လုပ်ပါ။
  topology သို့မဟုတ် telemetry တင်ပို့သူများ ပြောင်းလဲခြင်း။
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` သည် ပစ္စည်းတိုင်းကို စာရင်းပြုစုသည်။
  ဧပြီလ 9 ရက် အစမ်းလေ့ကျင့်မှုအတွင်း စစ်ဆေးခဲ့ပြီး ယခု Q2 ကြိုတင်ပြင်ဆင်မှတ်စုများ/အစီအစဉ်ကို သယ်ဆောင်လာပါသည်။
  တစ်ကြိမ်တည်းဖွင့်မည့်အစား အနာဂတ်အစမ်းလေ့ကျင့်မှုများကို တူညီသောခြေရာခံသို့ ပေါင်းထည့်ပါ။
  monotonic အထောက်အထားကိုစောင့်ရှောက်ရန် trackers ။
- OTLP စုဆောင်းသူ အတိုအထွာများနှင့် Grafana တင်ပို့မှုများကို ထုတ်ဝေပါ (`docs/source/telemetry.md` ကိုကြည့်ပါ)
  တင်ပို့သူသည် အစုလိုက်အပြုံလိုက် လမ်းညွှန်မှု ပြောင်းလဲသည့်အခါတိုင်း၊ Q1 အပ်ဒိတ်သည် တုန်လှုပ်သွားခဲ့သည်။
  headroom သတိပေးချက်များကိုတားဆီးရန် batch size 256 နမူနာများ။
- Multi-lane CI/test အထောက်အထားများသည် ယခုတွင် နေထိုင်ပါသည်။
  `integration_tests/tests/nexus/multilane_pipeline.rs` နှင့် အောက်တွင် လည်ပတ်နေသည်။
  `Nexus Multilane Pipeline` အလုပ်အသွားအလာ
  (`.github/workflows/integration_tests_multilane.yml`) အငြိမ်းစားကို အစားထိုးခြင်း။
  `pytests/nexus/test_multilane_pipeline.py` ရည်ညွှန်းချက် hash ကိုထားပါ။
  `defaults/nexus/config.toml` (`nexus.enabled = true`၊ blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) စင့်ခ်လုပ်ထားသည်။
  အစမ်းလေ့ကျင့်မှုအစုအဝေးများကို ပြန်လည်ဆန်းသစ်သည့်အခါ ခြေရာခံကိရိယာဖြင့်။

## Runtime Lane Lifecycle

- Runtime lane lifecycle အစီအစဉ်များသည် ယခု dataspace bindings များကို တရားဝင်စေပြီး ဘယ်သောအခါတွင်မှ ဖျက်ပစ်ပါ။
  Kura/tiered storage reconciliation မအောင်မြင်သဖြင့် catalog ကို မပြောင်းလဲဘဲထားခဲ့သည်။ ဟိ
  အငြိမ်းစား လမ်းကြောများအတွက် ကက်ချထားသည့် လမ်းသွားပြန်တမ်းများကို ဖြတ်တောက်လိုက်သောကြောင့် ပေါင်းစပ်-လယ်ဂျာပေါင်းစပ်မှု
  ဟောင်းနွမ်းနေသောအထောက်အထားများကို ပြန်လည်အသုံးမပြုပါ။
- Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`၊
  `Queue::apply_lane_lifecycle`) ပြန်လည်စတင်ခြင်းမပြုဘဲ လမ်းကြောင်းများပေါင်းထည့်ရန်/အနားယူရန်။ လမ်းကြောင်း၊
  TEU လျှပ်တစ်ပြက်ရိုက်ချက်များ၊ နှင့် manifest မှတ်ပုံတင်ခြင်းများသည် အောင်မြင်သောအစီအစဉ်တစ်ခုပြီးနောက် အလိုအလျောက်ပြန်လည်စတင်ပါသည်။
- အော်ပရေတာလမ်းညွှန်- အစီအစဉ်တစ်ခုပျက်သွားသောအခါ၊ ပျောက်ဆုံးနေသောဒေတာနေရာများ သို့မဟုတ် သိုလှောင်မှုကိုစစ်ဆေးပါ။
  ဖန်တီး၍မရသော အမြစ်များ (အအေးခံထားသော အမြစ်/Kura လမ်းသွားလမ်းညွှန်များ)။ ပြင်ပါ။
  ကျောထောက်နောက်ခံ လမ်းကြောင်းများနှင့် ထပ်စမ်းကြည့်ပါ။ အောင်မြင်သော အစီအစဉ်များသည် လမ်းကြော/ဒေတာအာကာသ တယ်လီမီတာကို ပြန်လည်ထုတ်လွှတ်သည်။
  ကွဲပြားသောကြောင့် ဒက်ရှ်ဘုတ်များသည် topology အသစ်ကို ထင်ဟပ်စေသည်။

## NPoS Telemetry & Backpressure အထောက်အထား

PhaseB ၏ပစ်လွှတ်ခြင်း-အစမ်းလေ့ကျင့်မှု retro သည် ၎င်းအား အဆုံးအဖြတ်ပေးသော တယ်လီမီတာဖမ်းယူမှုများအတွက် တောင်းဆိုခဲ့သည်။
NPoS အရှိန်မြှင့်စက်နှင့် အတင်းအဖျင်းအလွှာများသည် ၎င်းတို့၏နောက်ကျောဖိအားအတွင်းတွင် ရှိနေကြောင်း သက်သေပြပါ။
ကန့်သတ်ချက်များ။ ပေါင်းစည်းရေးမှာ စုစုစည်းစည်း ရှိတယ်။
`integration_tests/tests/sumeragi_npos_performance.rs` အဲဒါတွေကို လေ့ကျင့်တယ်။
အဖြစ်အပျက်များနှင့် JSON အနှစ်ချုပ်များကို ထုတ်လွှတ်သည် (`sumeragi_baseline_summary::<scenario>::…`)
မက်ထရစ်အသစ်များ မြေနေရာတိုင်း။ ၎င်းကို စက်တွင်းတွင် လုပ်ဆောင်ပါ-

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`၊ `SUMERAGI_NPOS_STRESS_COLLECTORS_K` သတ်မှတ်မည် သို့မဟုတ်
ပိုမိုမြင့်မားသောစိတ်ဖိစီးမှု topologies ကိုရှာဖွေရန် `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` အဆိုပါ
မူရင်းများသည် B4 တွင်အသုံးပြုသော 1s/`k=3` စုဆောင်းသူပရိုဖိုင်ကို ထင်ဟပ်စေပါသည်။

| ဇာတ်လမ်း/စမ်းသပ်မှု | လွှမ်းခြုံ | ကီးတယ်လီမီတာ |
| ---| ---| ---|
| `npos_baseline_1s_k3_captures_metrics` | အထောက်အထားအစုအဝေးကို နံပါတ်မဆက်မီ EMA မှနေချိန်အတွင်း စာအိတ်များ၊ တန်းစီခြင်းအတိမ်အနက်များနှင့် ထပ်နေသောပို့လွှတ်မှုဆိုင်ရာ တိုင်းတာချက်များကို မှတ်တမ်းတင်ရန် အစမ်းလေ့ကျင့်မှုပိတ်ဆို့ချိန်နှင့်အတူ 12 ပတ်ကို ပိတ်ဆို့ထားသည်။ | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`။ |
| `npos_queue_backpressure_triggers_metrics` | ဝင်ခွင့်ဆိုင်ရာ ရွှေ့ဆိုင်းမှုများကို တိကျပြတ်သားစွာ လုပ်ဆောင်ကြောင်း သေချာစေရန်နှင့် တန်းစီသည် တင်ပို့သည့် စွမ်းဆောင်ရည်/ ရွှဲကောင်တာများကို ထုတ်ပေးကြောင်း သေချာစေရန် ငွေပေးငွေယူတန်းစီခြင်းကို လွှမ်းမိုးပါသည်။ | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`။ |
| `npos_pacemaker_jitter_within_band` | နမူနာများ ±125‰ တီးဝိုင်းကို ပြဌာန်းထားသည်ကို သက်သေမပြမချင်း တုန်လှုပ်သွားပြီး အချိန်ကုန်သွားသည်ကို ကြည့်ရှုပါ။ | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`။ |
| `npos_rbc_store_backpressure_records_metrics` | စတိုးဆိုင်များကိုပြသရန်နှင့် ဘိုက်ကောင်တာများတက်၊ နောက်ပြန်ပိတ်ကာ စတိုးဆိုင်ကို ကျော်မသွားဘဲ အခြေချရန် ပျော့ပျောင်း/မာကျောသော စတိုးဆိုင်ကန့်သတ်ချက်များသို့ RBC ကြီးမားသောပေးဆောင်မှုပမာဏကို တွန်းပို့ပေးသည်။ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`။ |
| `npos_redundant_send_retries_update_metrics` | အတင်းပြန်ပို့သည် ထို့ကြောင့် မလိုအပ်သော ပေးပို့မှုအချိုး တိုင်းတာမှုများနှင့် ပစ်မှတ်ပေါ်ရှိ စုဆောင်းသူများ-ပစ်မှတ်ကောင်တာများ တိုးလာကာ၊ တောင်းဆိုထားသည့် တယ်လီမီတာကို နောက်ခံမှ အဆုံးသို့ ကြိုးဖြင့် ချိတ်ဆက်ထားကြောင်း သက်သေပြသည်။ | `sumeragi_collectors_targeted_current`၊ `sumeragi_redundant_sends_total`။ |
| `npos_rbc_chunk_loss_fault_reports_backlog` | ဝန်ထုပ်ဝန်ပိုးများကို တိတ်တဆိတ် ဖြုန်းတီးမည့်အစား backlog monitor များသည် အမှားအယွင်းများ တိုးလာသည်ကို အတည်ပြုရန်အတွက် ပိုင်းခြားထားသောအပိုင်းများကို ချပေးသည်။ | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`။ |JSON လိုင်းများကို Prometheus ခြစ်ခြင်းဖြင့် ကြိုးပရင့်ထုတ်ခြင်းအား ပူးတွဲပါ
ပြေးနေစဉ်အတွင်း ဖမ်းမိသော အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထား တောင်းသည့်အခါတိုင်း backpressure ရှိသည်။
နှိုးစက်များသည် အစမ်းလေ့ကျင့်မှုဆိုင်ရာ topology နှင့် ကိုက်ညီသည်။

## အပ်ဒိတ်စာရင်း

1. လမ်းကြောင်းပြောင်းသွားသည့် ပြတင်းပေါက်အသစ်များကို ပေါင်းထည့်ကာ လေးပုံတစ်ပုံကို လှည့်ပတ်နေချိန်တွင် အဟောင်းများကို အနားပေးပါ။
2. Alertmanager နောက်ဆက်တွဲလုပ်ဆောင်မှုတိုင်းပြီးနောက် လျှော့ချရေးဇယားကို အပ်ဒိတ်လုပ်ပါ။
   လုပ်ဆောင်ချက်က လက်မှတ်ပိတ်ဖို့ပါ။
3. config deltas ပြောင်းလဲသောအခါ၊ ခြေရာခံကိရိယာ၊ ဤမှတ်စုနှင့် တယ်လီမီတာကို အပ်ဒိတ်လုပ်ပါ။
   တူညီသောဆွဲထုတ်တောင်းဆိုမှုတွင် pack digest စာရင်း။
4. အစမ်းလေ့ကျင့်မှု/ ကြေးနန်းဆိုင်ရာ ပစ္စည်းအသစ်များကို ဤနေရာတွင် လင့်ခ်ချိတ်ပါ ထို့ကြောင့် အနာဂတ်လမ်းပြမြေပုံ အခြေအနေကို
   မွမ်းမံမှုများသည် ပြန့်ကျဲနေသော ad-hoc မှတ်စုများအစား စာရွက်စာတမ်းတစ်ခုတည်းကို ကိုးကားနိုင်သည်။

## သက်သေအညွှန်း

| ပိုင်ဆိုင်မှု | တည်နေရာ | မှတ်စုများ |
|---------|----------|-------|
| ခြေရာကောက်-ခြေရာခံစာရင်းစစ်အစီရင်ခံစာ (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | PhaseB1 အထောက်အထားအတွက် Canonical အရင်းအမြစ်၊ `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` အောက်ရှိ portal အတွက် mirrored လုပ်ထားသည်။ |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA ကွဲပြားသော အနှစ်ချုပ်များ၊ ပြန်လည်သုံးသပ်သူ အတိုကောက်နှင့် GOV-2026-03-19 မဲစာရင်း ပါရှိသည်။ |
| Telemetry ပြန်လည်ကုစားခြင်းအစီအစဉ် | `docs/source/nexus_telemetry_remediation_plan.md` | သတိပေးချက်အထုပ်၊ OTLP အတွဲလိုက်အရွယ်အစားနှင့် B2 နှင့်ချိတ်ဆက်ထားသော ဘတ်ဂျက်အစောင့်အကြပ်များကို မှတ်တမ်းတင်ပါ။ |
| ဘက်စုံလမ်းသွား အစမ်းလေ့ကျင့်မှု ခြေရာခံ | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | ဧပြီလ 9 တွင် အစမ်းလေ့ကျင့်မှုဆိုင်ရာ အထောက်အထားများ၊ တရားဝင်သက်သေပြချက်/အကျဉ်းချုပ်၊ Q2 ကြိုတင်ပြင်ဆင်မှတ်စုများ/အစီအစဉ် နှင့် ပြန်လည်သုံးသပ်ခြင်းအထောက်အထားများကို စာရင်းပြုစုထားသည်။ |
| Telemetry pack manifest/digest (နောက်ဆုံးပေါ်) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | မှတ်တမ်းအထိုင် အပိုင်းအခြား 912–936၊ မျိုးစေ့ `NEXUS-REH-2026Q2`၊ နှင့် အုပ်ချုပ်မှုအစုအဝေးများအတွက် artefact hashes။ |
| TLS ပရိုဖိုင် မန်နီးဖက်စ် | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Q2 ပြန်လည်လုပ်ဆောင်နေစဉ်အတွင်း ရိုက်ကူးထားသော အတည်ပြုထားသော TLS ပရိုဖိုင်၏ Hash၊ routed-trace appendices တွင်ကိုးကားပါ။ |
| TRACE-MULTILANE-CANARY အစီအစဉ် | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 အစမ်းလေ့ကျင့်မှု (ပြတင်းပေါက်၊ အထိုင်အကွာအဝေး၊ အလုပ်ဝန်မျိုးစေ့၊ လုပ်ဆောင်ချက်ပိုင်ရှင်များ) အတွက် စီစဉ်ခြင်းမှတ်စု။ |
| အစမ်းလေ့ကျင့်မှု runbook | စတင်ပါ။ `docs/source/runbooks/nexus_multilane_rehearsal.md` | အဆင့်သတ်မှတ်ခြင်း → ကွပ်မျက်ခြင်း → ပြန်လှည့်ခြင်းအတွက် လည်ပတ်မှုစစ်ဆေးခြင်းစာရင်း လမ်းကြောဆိုင်ရာ topology သို့မဟုတ် တင်ပို့သူ လမ်းညွှန်ချက် ပြောင်းလဲသည့်အခါ အပ်ဒိတ်လုပ်ပါ။ |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro မှရည်ညွှန်းသော CLI pack ကိုပြောင်းလဲသည့်အခါတိုင်း archive သည် tracker နှင့်တွဲနေပါသည်။ |
| Multilane ဆုတ်ယုတ်မှု | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | လမ်းသွားပေါင်းများစွာ စီစဉ်သတ်မှတ်မှုအတွက် `nexus.enabled = true` ကို သက်သေပြပြီး Sora ကက်တလောက် hash များကို ထိန်းသိမ်းကာ လမ်းသွား-ဒေသခံ Kura/merge-log လမ်းကြောင်းများ (`blocks/lane_{id:03}_{slug}`) ကို `ConfigLaneRouter` မှတစ်ဆင့် `ConfigLaneRouter` မှတစ်ဆင့် ပံ့ပိုးပေးပါသည်။ |