---
id: nexus-routed-trace-audit-2026q1
lang: my
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/nexus_routed_trace_audit_report_2026q1.md` ဖြစ်သည်။ မိတ္တူနှစ်စောင်လုံးကို လက်ကျန်ဘာသာပြန်သည်အထိ ချိန်ညှိထားပါ။
:::

# 2026 Q1 Routed-Trace Audit Report (B1)

လမ်းပြမြေပုံ အကြောင်းအရာ **B1 — Routed-Trace Audits & Telemetry Baseline** လိုအပ်သည်။
Nexus လမ်းကြောင်းဖြင့် ခြေရာကောက်ခြင်း အစီအစဉ်ကို သုံးလတစ်ကြိမ် ပြန်လည်သုံးသပ်ခြင်း။ ဤအစီရင်ခံစာကို မှတ်တမ်းတင်ထားသည်။
Q12026 စာရင်းစစ်ဝင်းဒိုး (ဇန်နဝါရီလမှမတ်လ) သည် အုပ်ချုပ်ရေးကောင်စီမှ လက်မှတ်ရေးထိုးနိုင်စေရန်၊
Q2 စတင်စမ်းသပ်မှုမတိုင်မီ telemetry ကိုယ်ဟန်အနေအထား။

## နယ်ပယ်နှင့် အချိန်ဇယား

| ခြေရာခံ ID | Window (UTC) | ရည်ရွယ်ချက် |
|----------|-----------------|----------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | လမ်းသွားဝင်ခွင့် ဟီစတိုဂရမ်များ၊ တန်းစီနေသော အတင်းအဖျင်းများနှင့် လမ်းကြောင်းပေါင်းစုံကို ဖွင့်ခြင်းမပြုမီ သတိပေးချက် စီးဆင်းမှုကို စစ်ဆေးပါ။ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | AND4/AND7 မှတ်တိုင်များရှေ့တွင် OTLP ပြန်ဖွင့်ခြင်း၊ ကွဲပြားသော ဘော့တ်ညီမျှခြင်း နှင့် SDK တယ်လီမီတာထည့်သွင်းခြင်းတို့ကို အတည်ပြုပါ။ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | RC1 ဖြတ်တောက်ခြင်းမပြုမီ အုပ်ချုပ်မှု-အတည်ပြုထားသော `iroha_config` မြစ်ဝကျွန်းပေါ်ဒေသနှင့် ပြန်လှည့်ရန် အဆင်သင့်ဖြစ်ကြောင်း အတည်ပြုပါ။ |

အစမ်းလေ့ကျင့်မှုတစ်ခုစီသည် လမ်းကြောင်းမှ ခြေရာကောက်ဖြင့် ထုတ်လုပ်မှုကဲ့သို့ ထိပ်တန်းနည်းပညာကို လည်ပတ်ခဲ့သည်။
ကိရိယာတန်ဆာပလာကို ဖွင့်ထားသည် (`nexus.audit.outcome` telemetry + Prometheus ကောင်တာများ)၊
Alertmanager စည်းမျဉ်းများကို တင်ပြီး အထောက်အထားများကို `docs/examples/` သို့ တင်ပို့ထားသည်။

## နည်းစနစ်

1. **Telemetry စုဆောင်းခြင်း။** ဆုံမှတ်များအားလုံးသည် ဖွဲ့စည်းတည်ဆောက်ထားသော ထုတ်လွှတ်မှု
   `nexus.audit.outcome` ဖြစ်ရပ်နှင့် ပါ၀င်သည့် တိုင်းတာမှုများ
   (`nexus_audit_outcome_total*`)။ အထောက်အမ
   `scripts/telemetry/check_nexus_audit_outcome.py` သည် JSON မှတ်တမ်းကို အမြီးဆွဲထားသည်။
   ဖြစ်ရပ်အခြေအနေကို အတည်ပြုပြီး payload အောက်တွင် သိမ်းဆည်းထားသည်။
   `docs/examples/nexus_audit_outcomes/`။【scripts/telemetry/check_nexus_audit_outcome.py:1】
2. **သတိပေးချက် အတည်ပြုခြင်း** `dashboards/alerts/nexus_audit_rules.yml` နှင့် ၎င်း၏ စမ်းသပ်မှု
   ကြိုးသည် သတိပေးချက် ဆူညံသံ ကန့်သတ်ချက်များ နှင့် ပါဝါပုံစံ ပုံစံအတိုင်း ရှိနေကြောင်း သေချာစေသည်။
   တသမတ်တည်း။ CI သည် `dashboards/alerts/tests/nexus_audit_rules.test.yml` ကိုဖွင့်ထားသည်။
   အပြောင်းအလဲတိုင်း၊ ပြတင်းပေါက်တစ်ခုစီတွင် တူညီသောစည်းမျဉ်းများကို ကိုယ်တိုင်ကျင့်သုံးခဲ့သည်။
3. **Dashboard capture.** အော်ပရေတာများသည် လမ်းကြောင်းပေါ်မှ ခြေရာခံအကန့်များကို တင်ပို့သည်။
   `dashboards/grafana/soranet_sn16_handshake.json` (လက်ဆွဲနှုတ်ဆက်ခြင်း ကျန်းမာရေး) နှင့်
   စာရင်းစစ်ရလဒ်များနှင့် တန်းစီကျန်းမာရေးကို ဆက်စပ်ရန် telemetry ခြုံငုံသုံးသပ်ချက် ဒိုင်ခွက်များ။
4. **ပြန်လည်သုံးသပ်သူ မှတ်စုများ။** အုပ်ချုပ်မှုအတွင်းဝန်သည် ဝင်ရောက်သုံးသပ်သူ၏ အတိုကောက်၊
   ဆုံးဖြတ်ချက်နှင့် [Nexus အကူးအပြောင်းမှတ်စုများ](./nexus-transition-notes) ရှိ လျော့ပါးရေးလက်မှတ်များ
   နှင့် config delta tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)။

## တွေ့ရှိချက်

| ခြေရာခံ ID | ရလဒ် | အထောက်အထား | မှတ်စုများ |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | သည်း | မီးသတိပေးချက်/ ဖန်သားပြင်ဓာတ်ပုံများ ပြန်လည်ရယူခြင်း (အတွင်းပိုင်းလင့်ခ်) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` ပြန်လည်ပြသခြင်း။ [Nexus အကူးအပြောင်းမှတ်စု](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) တွင် မှတ်တမ်းတင်ထားသော တယ်လီမီတာ ကွာခြားချက်များ။ | တန်းစီ-ဝင်ခွင့် P95 သည် 612ms (ပစ်မှတ် ≤750ms) ကျန်ခဲ့သည်။ နောက်ဆက်တွဲလုပ်ရန်မလိုအပ်ပါ။ |
| `TRACE-TELEMETRY-BRIDGE` | သည်း | သိမ်းဆည်းထားသော ရလဒ်ဖြစ်သည့် payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` နှင့် `status.md` တွင် မှတ်တမ်းတင်ထားသော OTLP ပြန်လည်ဖွင့်ခြင်း hash။ | SDK redaction ဆားများသည် Rust အခြေခံလိုင်းနှင့် ကိုက်ညီသည်၊ diff bot သည် zero deltas ကိုဖော်ပြခဲ့သည်။ |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | အုပ်ချုပ်မှု ခြေရာခံစနစ် ထည့်သွင်းခြင်း (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS ပရိုဖိုင် မန်နီးဖက်စ် (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + တယ်လီမီတာ ထုပ်ပိုး မန်နီးဖက်စ် (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)။ | Q2 သည် အတည်ပြုထားသော TLS ပရိုဖိုင်ကို ဖျက်လိုက်ပြီး သုည stragglers များကို အတည်ပြုထားသည်။ telemetry manifest မှတ်တမ်းများ အထိုင်အကွာအဝေး 912–936 နှင့် workload seed `NEXUS-REH-2026Q2`။ |

သဲလွန်စများအားလုံးသည် ၎င်းတို့၏အတွင်းတွင် အနည်းဆုံး `nexus.audit.outcome` ဖြစ်ရပ်တစ်ခု ထုတ်လုပ်ခဲ့သည်။
ပြတင်းပေါက်များ၊ Alertmanager guardrails (`NexusAuditOutcomeFailure` ကို ကျေနပ်စေသည်
လေးပုံတစ်ပုံအတွက် စိမ်းနေခဲ့သည်။)

## နောက်ဆက်တွဲ

- Routed-trace နောက်ဆက်တွဲကို TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ဖြင့် အပ်ဒိတ်လုပ်ထားသည်။
  အကူးအပြောင်းမှတ်စုများတွင် `NEXUS-421` လျော့ပါးသက်သာစေခြင်း။
- အကြမ်းထည် OTLP ပြန်လည်ပြသမှုများနှင့် Torii ကွဲပြားသည့် ရှေးဟောင်းပစ္စည်းများကို မော်ကွန်းတွင် ဆက်လက် ပူးတွဲပါရှိသည်။
  Android AND4/AND7 သုံးသပ်ချက်များအတွက် တန်းတူညီမျှမှု အထောက်အထားများကို အားကောင်းစေပါသည်။
- လာမည့် `TRACE-MULTILANE-CANARY` လေ့ကျင့်မှုများကို အလားတူ ပြန်လည်အသုံးပြုကြောင်း အတည်ပြုပါ။
  တယ်လီမီတာအထောက်အကူဖြစ်တာကြောင့် Q2 မှာ တရားဝင်အတည်ပြုထားတဲ့ အလုပ်အသွားအလာမှ အကျိုးကျေးဇူးများ။

## Artefact အညွှန်း

| ပိုင်ဆိုင်မှု | တည်နေရာ |
|--------|----------|
| Telemetry validator | `scripts/telemetry/check_nexus_audit_outcome.py` |
| သတိပေးချက် စည်းမျဉ်းများနှင့် စမ်းသပ်မှုများ | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| နမူနာရလဒ် payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Routed-ခြေရာခံအချိန်ဇယား & မှတ်စုများ | [Nexus အကူးအပြောင်းမှတ်စုများ](./nexus-transition-notes) |

ဤအစီရင်ခံစာ၊ အထက်ဖော်ပြပါအရာများနှင့် သတိပေးချက်/တယ်လီမီတာတင်ပို့မှုတို့သည် ဖြစ်သင့်သည်။
သုံးလပတ်အတွက် B1 ကိုပိတ်ရန် အုပ်ချုပ်မှုဆုံးဖြတ်ချက်မှတ်တမ်းတွင် ပူးတွဲပါရှိသည်။