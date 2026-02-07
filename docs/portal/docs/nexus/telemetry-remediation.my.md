---
lang: my
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 35fe9abd10cb1454b72042b5b9dfbc35d45cc1cd91e2a4d0af4909032189df22
source_last_modified: "2025-12-29T18:16:35.147058+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-telemetry-remediation
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
---

# ခြုံငုံသုံးသပ်ချက်

လမ်းပြမြေပုံပါ အကြောင်းအရာ **B2 — တယ်လီမီတာ ကွာဟချက် ပိုင်ဆိုင်မှု** သည် ထုတ်ပြန်ထားသော အစီအစဉ်ကို ချိတ်ဆက်ရန် လိုအပ်သည်။
ထူးထူးခြားခြား Nexus တယ်လီမီတာ ကွာဟချက်တိုင်းသည် အချက်ပြမှု၊ သတိပေးချက် အကာအကွယ်လမ်း၊ ပိုင်ရှင်၊
နောက်ဆုံးရက်၊ နှင့် Q1 2026 စာရင်းစစ်ဝင်းဒိုးများ မစတင်မီ အတည်ပြုရေးပစ္စည်း။
ဤစာမျက်နှာသည် `docs/source/nexus_telemetry_remediation_plan.md` ဖြစ်သည့်အတွက် ထုတ်ဝေရန်
အင်ဂျင်နီယာ၊ telemetry ops နှင့် SDK ပိုင်ရှင်များသည် လွှမ်းခြုံမှုမတိုင်မီ အတည်ပြုနိုင်သည်။
routed-trace နှင့် `TRACE-TELEMETRY-BRIDGE` လေ့ကျင့်မှုများ။

# ကွာဟချက်

| Gap ID | အချက်ပြ & အချက်ပြ guardrail | Owner/escalation| ပေးရမည့် (UTC) | အထောက်အထားနှင့် အတည်ပြုချက် |
|--------|--------------------------------|--------------------------------|-----------------|--------------------------------|
| `GAP-TELEM-001` | သတိပေးချက်ပါရှိသော Histogram `torii_lane_admission_latency_seconds{lane_id,endpoint}` **`SoranetLaneAdmissionLatencyDegraded`** တွင် `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` ကို 5 မိနစ်ကြာ (`dashboards/alerts/soranet_lane_rules.yml`) ဖြင့် ပစ်ခတ်သည်။ | `@torii-sdk` (အချက်ပြမှု) + `@telemetry-ops` (သတိပေးချက်); Nexus မှတဆင့် ခေါ်ဆိုမှုတွင် လမ်းကြောင်းပြောင်း-ခြေရာကောက်။ | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` အောက်ရှိ သတိပေးချက်များနှင့် `TRACE-LANE-ROUTING` လေ့ကျင့်မှု ဖမ်းယူမှု နှင့် Torii `/metrics` ခြစ်ထုတ်ခြင်းကို [`TRACE-LANE-ROUTING` တွင် သိမ်းဆည်းထားပြီး ပြန်လည်ကောင်းမွန်လာသည့် သတိပေးချက်နှင့် Torii `/metrics` ဖိုင်ကို [`/metrics`] တွင် သိမ်းဆည်းထားသည်။ 0Nexus။ |
| `GAP-TELEM-002` | Counter `nexus_config_diff_total{knob,profile}` နှင့် guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` ဂိတ်ချထားသည် (`docs/source/telemetry.md`)။ | `@nexus-core` (ကိရိယာတန်ဆာပလာ) → `@telemetry-ops` (သတိပေးချက်); မမျှော်လင့်ဘဲ တန်ပြန်တိုးလာသောအခါ အုပ်ချုပ်မှုတာဝန် အရာရှိက ခေါ်သည်။ | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ဘေးတွင် သိမ်းဆည်းထားသော အုပ်ချုပ်ရေးစနစ် ခြောက်သွေ့သော ရလဒ်များ ထုတ်ပြန်ချက်စာရင်းတွင် Prometheus စုံစမ်းမေးမြန်းချက်စခရင်ပုံနှင့် `StateTelemetry::record_nexus_config_diff` ကွာခြားချက်ကို သက်သေပြသည့် မှတ်တမ်းကောက်နုတ်ချက်တို့ ပါဝင်သည်။ |
| `GAP-TELEM-003` | ပျက်ကွက်မှုများ သို့မဟုတ် ပျောက်ဆုံးနေသော ရလဒ်များသည် မိနစ် 30 ကျော်ကြာ ဆက်လက်တည်ရှိနေချိန်တွင် ပျက်ကွက်မှုများ သို့မဟုတ် ပျောက်ဆုံးနေသောရလဒ်များ (`dashboards/alerts/nexus_audit_rules.yml`) သတိပေးချက်နှင့်အတူ `TelemetryEvent::AuditOutcome` (မက်ထရစ် `nexus.audit.outcome`)။ | `@telemetry-ops` (ပိုက်လိုင်း) `@sec-observability` သို့ မြင့်တက်နေသည်။ | 2026-02-27 | CI ဂိတ် `scripts/telemetry/check_nexus_audit_outcome.py` သည် NDJSON payloads များကို မော်ကွန်းတင်ပြီး TRACE ဝင်းဒိုးတွင် အောင်မြင်သည့်ဖြစ်ရပ်မရှိသည့်အခါ မအောင်မြင်ပါ။ လမ်းကြောင်းဖြင့် ခြေရာခံခြင်း အစီရင်ခံစာတွင် ပူးတွဲပါရှိသော သတိပေးချက် ဖန်သားပြင်ဓာတ်ပုံများ။ |
| `GAP-TELEM-004` | SRE on-call checklist ကို ကျွေးမွေးသော guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` ဖြင့် တိုင်းထွာ `nexus_lane_configured_total`။ | node များသည် ကိုက်ညီမှုမရှိသော ကတ်တလောက်အရွယ်အစားများကို သတင်းပို့သည့်အခါ `@telemetry-ops` (gauge/export) သည် `@nexus-core` သို့ တိုးနေသည်။ | 2026-02-28 | Scheduler telemetry test `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` သည် ထုတ်လွှတ်မှုကို သက်သေပြသည်၊ အော်ပရေတာများသည် Prometheus diff + `StateTelemetry::set_nexus_catalogs` မှတ်တမ်းကောက်နုတ်ချက်ကို TRACE အစမ်းလေ့ကျင့်မှု ပက်ကေ့ချ်တွင် ပူးတွဲထားသည်။ |

# လုပ်ငန်းလည်ပတ်မှုလုပ်ငန်းစဉ်

1. **အပတ်စဉ် စမ်းသပ်မှု။** ပိုင်ရှင်များသည် Nexus အဆင်သင့်ခေါ်ဆိုမှုတွင် တိုးတက်မှု အစီရင်ခံပါသည်။
   blockers များနှင့် alert-test artefacts များကို `status.md` တွင် အကောင့်ဝင်ထားသည်။
2. **သတိပေးချက် ခြောက်သွေ့ခြင်း** သတိပေးချက်တစ်ခုစီသည် a နှင့်တွဲလျက် ရှိသည်။
   `dashboards/alerts/tests/*.test.yml` ဝင်ခွင့်ကြောင့် CI သည် `promtool စမ်းသပ်မှုကို လုပ်ဆောင်သည်
   အကာအရံများ ပြောင်းလဲသည့်အခါတိုင်း စည်းမျဉ်းများ။
3. **စာရင်းစစ်အထောက်အထား** `TRACE-LANE-ROUTING` နှင့်
   `TRACE-TELEMETRY-BRIDGE` က အစမ်းလေ့ကျင့်မှုတွင် ဖုန်းခေါ်ဆိုမှုတွင် Prometheus မေးခွန်းကို ဖမ်းယူသည်
   ရလဒ်များ၊ သတိပေးချက်မှတ်တမ်းနှင့် သက်ဆိုင်ရာ script outputs များ
   (`scripts/telemetry/check_nexus_audit_outcome.py`၊
   ဆက်စပ်အချက်ပြမှုများအတွက် `scripts/telemetry/check_redaction_status.py`) နှင့်
   ၎င်းတို့ကို routed-trace artefacts များဖြင့် သိမ်းဆည်းပါ။
4. **အရှိန်မြှင့်ခြင်း။** အစမ်းလေ့ကျင့်ထားသော ပြတင်းပေါက်အပြင်ဘက်တွင် အကာအရံတစ်ခုခု မီးလောင်ပါက ပိုင်ဆိုင်မှု၊
   အဖွဲ့သည် ဤအစီအစဉ်အပါအဝင် Nexus အဖြစ်အပျက်လက်မှတ်ကို ပေးပို့ပါသည်။
   စာရင်းစစ်များမစတင်မီ မက်ထရစ်လျှပ်တစ်ပြက်ရိုက်ချက်နှင့် လျော့ပါးရေးအဆင့်များ။

ဤ matrix ဖြင့်ထုတ်ဝေသည် — နှင့် `roadmap.md` နှင့် နှစ်ခုစလုံးမှကိုးကားထားသည်။
`status.md` — လမ်းပြမြေပုံပါအရာ **B2** ယခု “တာဝန်ယူမှု၊ နောက်ဆုံးရက်၊
သတိပေးချက်၊ အတည်ပြုခြင်း" လက်ခံမှုစံနှုန်း။