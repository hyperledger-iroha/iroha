---
id: observability-plan
lang: my
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

## ရည်ရွယ်ချက်များ
- ဂိတ်ဝေးများ၊ ဆုံမှတ်များနှင့် ရင်းမြစ်အစုံအလင်အတွက် မက်ထရစ်များနှင့် ဖွဲ့စည်းတည်ဆောက်ထားသည့် ဖြစ်ရပ်များကို သတ်မှတ်ပါ။
- Grafana ဒက်ရှ်ဘုတ်များ၊ သတိပေးချက် သတ်မှတ်ချက်များနှင့် အတည်ပြုခြင်းချိတ်များကို ပေးပါ။
- အမှားအယွင်း-ဘတ်ဂျက်နှင့် ပရမ်းပတာလေ့ကျင့်မှုမူဝါဒများနှင့်အတူ SLO ပစ်မှတ်များကို ထူထောင်ပါ။

## မက်ထရစ်ကတ်တလောက်

### Gateway လည်း ရပါသေးတယ်။

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` မှတဆင့် ထုတ်လွှတ်သည်။ အဆုံးမှတ်/နည်းလမ်း ပေါင်းစပ်မှုတစ်ခုအတွက် လေယာဉ်တွင်း HTTP လုပ်ဆောင်ချက်များကို ခြေရာခံသည်။ |
| `sorafs_gateway_responses_total` | ကောင်တာ | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000108041X, I18NI000000108041X ပြီးစီးသွားသော တံခါးပေါက်တောင်းဆိုမှုတိုင်းသည် တစ်ကြိမ်တိုးများ; `result` ∈ {`success`,`error`,`dropped`}။ |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000001080540X ဂိတ်ဝ တုံ့ပြန်မှုများအတွက် အချိန်မှ ပထမ-ဘိုက် တုံ့ပြန်ချိန်၊ Prometheus `_bucket/_sum/_count` အဖြစ် တင်ပို့ခဲ့သည်။ |
| `sorafs_gateway_proof_verifications_total` | ကောင်တာ | `profile_version`, `result`, `error_code` | တောင်းဆိုသည့်အချိန်၌ ဖမ်းယူထားသော အထောက်အထားစိစစ်ခြင်းရလဒ်များ (`result` ∈ {`success`,`failure`})။ |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | PoR ပြေစာများအတွက် အတည်ပြု တုံ့ပြန်မှု ဖြန့်ဖြူးခြင်း။ |
| `telemetry::sorafs.gateway.request` | အစီအစဥ် | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` Loki/Tempo ဆက်စပ်မှုအတွက် တောင်းဆိုချက်တိုင်းတွင် ဖြည့်စွက်ထားသော ဖွဲ့စည်းပုံမှတ်တမ်းကို ထုတ်လွှတ်သည်။ |

`telemetry::sorafs.gateway.request` ဖြစ်ရပ်များသည် OTEL ကောင်တာများကို ဖွဲ့စည်းတည်ဆောက်ထားသော ပေးဆောင်မှုများဖြင့် ရောင်ပြန်ဟပ်ပြီး၊ `endpoint`၊ `method`၊ `variant`၊ `status`၊ I18NI00000018NI00000018NI00000 I18NI000000X ဒက်ရှ်ဘုတ်များသည် SLO ခြေရာခံခြင်းအတွက် OTLP စီးရီးကို အသုံးပြုနေစဉ် Loki/Tempo ဆက်စပ်မှု။

### ကျန်းမာရေးဆိုင်ရာ တယ်လီမီတာ အထောက်အထား

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | ကောင်တာ | `provider_id`, `trigger`, `penalty` | `RecordCapacityTelemetry` သည် `SorafsProofHealthAlert` ထုတ်သည့်အခါတိုင်း တိုးများသည်။ `trigger` သည် PDP/PoTR/မအောင်မြင်မှုနှစ်ခုလုံးကို ခွဲခြားသိမြင်စေပြီး `penalty` သည် အပေါင်ပစ္စည်းကို အမှန်တကယ် ဖြတ်တောက်ခြင်း သို့မဟုတ် cooldown ဖြင့် ဖြတ်ခြင်းရှိမရှိ ဖမ်းယူသည်။ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | တိုင်းထွာ | `provider_id` | နောက်ဆုံးပေါ် PDP/PoTR ရေတွက်မှုများကို မှားယွင်းနေသော တယ်လီမီတာပြတင်းပေါက်အတွင်းတွင် အစီရင်ခံတင်ပြသောကြောင့် အဖွဲ့များသည် ဝန်ဆောင်မှုပေးသူများ၏ မူဝါဒကို ဘယ်လောက်အထိ ကျော်လွန်သွားသည်ကို တွက်ချက်နိုင်ပါသည်။ |
| `torii_sorafs_proof_health_penalty_nano` | တိုင်းထွာ | `provider_id` | နောက်ဆုံးသတိပေးချက်တွင် နာနို-XOR ပမာဏကို ဖြတ်တောက်လိုက်သည် (အအေးလျှော့ခြင်းကို နှိမ်နင်းသည့်အခါ သုည)။ |
| `torii_sorafs_proof_health_cooldown` | တိုင်းထွာ | `provider_id` | နောက်ဆက်တွဲသတိပေးချက်များကို ခေတ္တပိတ်ထားသောအခါတွင် Boolean gauge (`1` = cooldown ဖြင့်သတိပေးချက်ကို ဖိနှိပ်ထားသည်)။ |
| `torii_sorafs_proof_health_window_end_epoch` | တိုင်းထွာ | `provider_id` | အော်ပရေတာများသည် Norito ပစ္စည်းများနှင့် ဆက်စပ်နိုင်စေရန် သတိပေးချက်နှင့် ချိတ်ထားသော တယ်လီမီတာပြတင်းပေါက်အတွက် မှတ်တမ်းတင်ထားသော အပိုင်းဖြစ်သည်။ |

ယခု ဤဖိဒ်များသည် Taikai ကြည့်ရှုသူ ဒက်ရှ်ဘုတ်၏ အထောက်အထား-ကျန်းမာရေးအတန်းကို အားကောင်းစေသည်။
(`dashboards/grafana/taikai_viewer.json`)၊ CDN အော်ပရေတာများအား တိုက်ရိုက်မြင်နိုင်စွမ်းကို ပေးခြင်း
သတိပေးချက်ပမာဏများ၊ PDP/PoTR အစပျိုးပေါင်းစပ်မှု၊ ပြစ်ဒဏ်များနှင့် cooldown အခြေအနေအလိုက်
ပံ့ပိုးပေးသူ။

တူညီသောမက်ထရစ်များသည် ယခု Taikai ကြည့်ရှုသူသတိပေးချက်စည်းမျဉ်းနှစ်ခုကို ပြန်ပေးသည်-
`SorafsProofHealthPenalty` မီးလောင်သည့်အခါတိုင်း
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` တိုးလာသည်။
နောက်ဆုံး 15 မိနစ်တွင် `SorafsProofHealthCooldown` က သတိပေးချက်တစ်ခု တက်လာချိန်တွင်
ဝန်ဆောင်မှုပေးသူသည် ငါးမိနစ်ကြာအောင် အအေးခံထားဆဲဖြစ်သည်။ သတိပေးချက် နှစ်ခုလုံး နေထိုင်ပါသည်။
`dashboards/alerts/taikai_viewer_rules.yml` ထို့ကြောင့် SRE များသည် ချက်ခြင်းအကြောင်းအရာကို ရရှိသည်။
PoR/PoTR ကျင့်သုံးမှု ကြီးထွားလာသည့်အခါတိုင်း။

### Orchestrator လို့ ရပါသေးတယ်။

| မက်ထရစ်/ဖြစ်ရပ် | ရိုက် | တံဆိပ်များ | ထုတ်လုပ်သူ | မှတ်စုများ |
|----------------|------|--------|----------------|------|
| `sorafs_orchestrator_active_fetches` | တိုင်းထွာ | `manifest_id`, `region` | `FetchMetricsCtx` | လက်ရှိ လေယာဉ်ပေါ်ရှိ ဆက်ရှင်များ။ |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | ကြာချိန် ဟစ်စတိုဂရမ် မီလီစက္ကန့်၊ 1ms → 30s ပုံးများ။ |
| `sorafs_orchestrator_fetch_failures_total` | ကောင်တာ | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | အကြောင်းရင်းများ- `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`။ |
| `sorafs_orchestrator_retries_total` | ကောင်တာ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | ပြန်ကြိုးစားခြင်း အကြောင်းရင်းများ (`retry`၊ `digest_mismatch`၊ `length_mismatch`၊ `provider_error`) ကို ခွဲခြားသည်။ |
| `sorafs_orchestrator_provider_failures_total` | ကောင်တာ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | စက်ရှင်အဆင့် မသန်စွမ်းမှု/မအောင်မြင်မှု မှတ်တမ်းများကို ဖမ်းယူသည်။ |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | ဖြတ်သန်းမှု/SLO ခွဲခြမ်းစိတ်ဖြာမှုအတွက် တစ်ပိုင်းတစ်စ ထုတ်ယူမှု latency ဖြန့်ဖြူးမှု (ms)။ |
| `sorafs_orchestrator_bytes_total` | ကောင်တာ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | မန်နီးဖက်စ်/ဝန်ဆောင်မှုပေးသူမှ ပေးပို့သော ဘိုက်များ။ PromQL တွင် `rate()` မှတဆင့် ဖြတ်တောက်မှုကို ရယူပါ။ |
| `sorafs_orchestrator_stalls_total` | ကောင်တာ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` ထက်ကျော်လွန်သော အပိုင်းများကို ရေတွက်သည်။ |
| `telemetry::sorafs.fetch.lifecycle` | အစီအစဥ် | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI0000010160X, I18NI001000180X `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Norito JSON payload ဖြင့် Mirrors အလုပ်သက်တမ်း (စတင်/ပြီးမြောက်သည်)။ |
| `telemetry::sorafs.fetch.retry` | အစီအစဥ် | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | ဝန်ဆောင်မှုပေးသူမှ ထပ်စမ်းကြည့်သည့်နှုန်းကို ထုတ်လွှတ်သည်။ `attempts` သည် တိုးမြင့်ကြိုးစားမှုများ (≥1) ကို ရေတွက်သည်။ |
| `telemetry::sorafs.fetch.provider_failure` | အစီအစဥ် | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | ဝန်ဆောင်မှုပေးသူသည် ကျရှုံးမှုအဆင့်ကို ကျော်သွားသောအခါတွင် ပေါ်လာသည်။ |
| `telemetry::sorafs.fetch.error` | အစီအစဥ် | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` `FetchTelemetryCtx` | Terminal ချို့ယွင်းမှုမှတ်တမ်း၊ Loki/Splunk ထည့်သွင်းခြင်းအတွက် အဆင်ပြေသည်။ |
| `telemetry::sorafs.fetch.stall` | အစီအစဥ် | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | အတုံးလိုက် latency သည် ပြင်ဆင်ထားသော ဦးထုပ်များ (ကြည့်မှန်များ အရောင်းကောင်တာများ) ကို ချိုးဖောက်သည့်အခါ တိုးလာပါသည်။ |

### Node/replication ရပါသေးတယ်။

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | OTEL သိုလှောင်မှုအသုံးပြုမှုရာခိုင်နှုန်း (`_bucket/_sum/_count` အဖြစ် တင်ပို့ထားသည်)။ |
| `sorafs_node_por_success_total` | ကောင်တာ | `provider_id` | အစီအစဉ်ဆွဲသူလျှပ်တစ်ပြက်မှ ဆင်းသက်လာသော အောင်မြင်သော PoR နမူနာများအတွက် Monotonic တန်ပြန်။ |
| `sorafs_node_por_failure_total` | ကောင်တာ | `provider_id` | မအောင်မြင်သော PoR နမူနာများအတွက် Monotonic တန်ပြန်။ |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | တိုင်းထွာ | `provider` | အသုံးပြုထားသော bytes အတွက် ရှိပြီးသား Prometheus တိုင်းတာချက်များ၊ တန်းစီခြင်းအတိမ်အနက်၊ PoR ပျံသန်းမှုအရေအတွက် |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | တိုင်းထွာ | `provider` | ဝန်ဆောင်မှုပေးသူ၏ စွမ်းဆောင်ရည်/အလုပ်ချိန်အောင်မြင်မှုဒေတာ စွမ်းရည်ဒိုင်ခွက်တွင် ပေါ်လာသည်။ |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | တိုင်းထွာ | `provider`, `manifest` | `/v1/sorafs/por/ingestion/{manifest}` ကို စစ်တမ်းကောက်ယူသည့်အခါတိုင်း၊ “PoR Stalls” အကန့်/သတိပေးချက်အား ဖြည့်သွင်းသည့်အချိန်တိုင်းတွင် Backlog အတိမ်အနက်နှင့် များပြားသော ချို့ယွင်းချက်ကောင်တာများကို တင်ပို့သည်။ |

### ပြုပြင်ခြင်း & SLA

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | ကောင်တာ | `status` | ပြုပြင်ခြင်းလုပ်ငန်းအကူးအပြောင်းအတွက် OTEL ကောင်တာ။ |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | ဘဝသံသရာ latency ကို ပြုပြင်ရန်အတွက် OTEL ဟစ်စတိုဂရမ်။ |
| `sorafs_repair_queue_depth` | Histogram | `provider` | ဝန်ဆောင်မှုပေးသူတစ်ဦးစီတွင် တန်းစီထားသော လုပ်ဆောင်စရာများ၏ OTEL ၏ ဟစ်စတိုဂရမ် (လျှပ်တစ်ပြက်ရိုက်ချက်ပုံစံ)။ |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | ရှေးအကျဆုံး တန်းစီထားသော အလုပ်သက်တမ်း (စက္ကန့်) ၏ OTEL ၏ ဟစ်စတိုဂရမ်။ |
| `sorafs_repair_lease_expired_total` | ကောင်တာ | `outcome` | အငှားသက်တမ်းကုန်ဆုံးမှုအတွက် OTEL ကောင်တာ (`requeued`/`escalated`)။ |
| `sorafs_repair_slash_proposals_total` | ကောင်တာ | `outcome` | အဆိုပြုချက်အကူးအပြောင်းများအတွက် OTEL တန်ပြန်။ |
| `torii_sorafs_repair_tasks_total` | ကောင်တာ | `status` | အလုပ်အကူးအပြောင်းများအတွက် Prometheus တန်ပြန်။ |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | ဘဝလည်ပတ်ချိန်ကို ပြုပြင်ရန်အတွက် Prometheus ဟီစတိုဂရမ်။ |
| `torii_sorafs_repair_queue_depth` | တိုင်းထွာ | `provider` | ဝန်ဆောင်မှုပေးသူ တစ်ဦးစီအတွက် တန်းစီထားသော အလုပ်များအတွက် Prometheus gauge။ |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | တိုင်းထွာ | — | Prometheus သည် အသက်အကြီးဆုံး တန်းစီနေသော အလုပ်အသက် (စက္ကန့်) အတွက် တိုင်းတာမှု။ |
| `torii_sorafs_repair_lease_expired_total` | ကောင်တာ | `outcome` | အငှားသက်တမ်းကုန်ဆုံးမှုအတွက် Prometheus ကောင်တာ။ |
| `torii_sorafs_slash_proposals_total` | ကောင်တာ | `outcome` | မျဥ်းစောင်း အဆိုပြုချက် အကူးအပြောင်းများအတွက် Prometheus တန်ပြန်။ |

အုပ်ချုပ်မှုစာရင်းစစ် JSON မက်တာဒေတာသည် ပြုပြင်မှုဆိုင်ရာ တယ်လီမီတာအညွှန်းများ (`status`၊ `ticket_id`၊ `manifest`၊ `provider`၊ ပြုပြင်ခြင်းဆိုင်ရာ ဖြစ်ရပ်များပေါ်ရှိ `outcome`) နှင့် ပြုပြင်ခြင်းဆိုင်ရာ စည်းမျဥ်းများပေါ်ရှိ '`outcome`) အဆုံးအဖြတ်အရ ဆက်စပ်နေသည်။

### Retention & GC| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | ကောင်တာ | `result` | မြှုပ်ထားသော node မှထုတ်လွှတ်သော GC ကောက်ယူမှုများအတွက် OTEL ကောင်တာ။ |
| `sorafs_gc_evictions_total` | ကောင်တာ | `reason` | အကြောင်းပြချက်ဖြင့် အုပ်စုဖွဲ့၍ နှင်ထုတ်ခံရသော ထင်ရှားမှုများအတွက် OTEL တန်ပြန်။ |
| `sorafs_gc_bytes_freed_total` | ကောင်တာ | `reason` | အကြောင်းပြချက်ဖြင့် အုပ်စုဖွဲ့ထားသော ဘိုက်များအတွက် OTEL ကောင်တာ။ |
| `sorafs_gc_blocked_total` | ကောင်တာ | `reason` | လက်ရှိပြင်ဆင်မှု သို့မဟုတ် မူဝါဒဖြင့် ပိတ်ဆို့ထားသော နှင်ထုတ်ခြင်းအတွက် OTEL တန်ပြန်။ |
| `torii_sorafs_gc_runs_total` | ကောင်တာ | `result` | Prometheus ကောင်တာ GC (အောင်မြင်မှု/အမှား)။ |
| `torii_sorafs_gc_evictions_total` | ကောင်တာ | `reason` | Prometheus သည် အကြောင်းပြချက်ဖြင့် အုပ်စုဖွဲ့ပြီး နှင်ထုတ်ခံရသော သရုပ်ပြများအတွက် တန်ပြန်။ |
| `torii_sorafs_gc_bytes_freed_total` | ကောင်တာ | `reason` | အကြောင်းပြချက်ဖြင့် အုပ်စုဖွဲ့ထားသော ဘိုက်များအတွက် Prometheus ကောင်တာ။ |
| `torii_sorafs_gc_blocked_total` | ကောင်တာ | `reason` | Prometheus အကြောင်းပြချက်ဖြင့် အုပ်စုဖွဲ့ပိတ်ဆို့ထားသော နှင်ထုတ်ခြင်းအတွက် တန်ပြန်။ |
| `torii_sorafs_gc_expired_manifests` | တိုင်းထွာ | — | GC ကောက်ယူမှုများဖြင့် စောင့်ကြည့်ထားသော သက်တမ်းကုန်ဆုံးနေသော သရုပ်ဖော်ပုံများ၏ လက်ရှိရေတွက်။ |
| `torii_sorafs_gc_oldest_expired_age_seconds` | တိုင်းထွာ | — | ရှေးအကျဆုံး ကုန်ဆုံးသွားသော ထင်ရှားသော စက္ကန့်ပိုင်းအတွင်း အသက် (ထိန်းသိမ်းခြင်း ကျေးဇူးတော်ပြီးနောက်)။ |

### ပြန်လည်သင့်မြတ်ရေး

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | မှတ်စုများ |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | ကောင်တာ | `result` | ပြန်လည်သင့်မြတ်ရေး လျှပ်တစ်ပြက်ရိုက်ချက်များအတွက် OTEL ကောင်တာ။ |
| `sorafs.reconciliation.divergence_total` | ကောင်တာ | — | OTEL သည် ပြေးနှုန်းတစ်ခုချင်း ကွဲလွဲမှုအရေအတွက်များ။ |
| `torii_sorafs_reconciliation_runs_total` | ကောင်တာ | `result` | ပြန်လည်သင့်မြတ်ခြင်းအတွက် Prometheus ကောင်တာ |
| `torii_sorafs_reconciliation_divergence_count` | တိုင်းထွာ | — | ပြန်လည်သင့်မြတ်ရေး အစီရင်ခံစာတွင် နောက်ဆုံးအကြိမ် ကွဲလွဲမှုအရေအတွက်ကို စောင့်ကြည့်လေ့လာခဲ့သည်။ |

### အချိန်မီ ပြန်လည်ထုတ်ယူခြင်းဆိုင်ရာ အထောက်အထား (PoTR) နှင့် SLA တို့ကို အပိုင်းလိုက်

| မက်ထရစ် | ရိုက် | တံဆိပ်များ | ထုတ်လုပ်သူ | မှတ်စုများ |
|--------|------|--------|----------|--------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR ညှိနှိုင်းရေးမှူး | မီလီစက္ကန့်များအတွင်း နောက်ဆုံးရက်ကို အနားပေးသည် (အပြုသဘော = တွေ့ဆုံသည်)။ |
| `sorafs_potr_failures_total` | ကောင်တာ | `tier`, `provider`, `reason` | PoTR ညှိနှိုင်းရေးမှူး | အကြောင်းရင်းများ- `expired`, `missing_proof`, `corrupt_proof`။ |
| `sorafs_chunk_sla_violation_total` | ကောင်တာ | `provider`, `manifest_id`, `reason` | SLA စောင့်ကြည့် | အတုံးလိုက်ပေးပို့ခြင်း SLO (latency၊ အောင်မြင်မှုနှုန်း) လွတ်သွားသောအခါ အလုပ်ထုတ်သည်။ |
| `sorafs_chunk_sla_violation_active` | တိုင်းထွာ | `provider`, `manifest_id` | SLA စောင့်ကြည့် | တက်ကြွသောချိုးဖောက်ဝင်းဒိုးအတွင်း Boolean gauge (0/1) ကို ဖွင့်ထားသည်။ |

## SLO ပစ်မှတ်များ

- Gateway စိတ်မချရနိုင်မှု- **99.9%** (HTTP 2xx/304 တုံ့ပြန်မှုများ)။
- ယုံကြည်မှုမရှိသော TTFB P95- ပူသောအဆင့် ≤120ms၊ နွေးထွေးသောအဆင့် ≤300ms။
- သက်သေအောင်မြင်မှုနှုန်း- တစ်ရက်လျှင် ≥99.5%။
- Orchestrator အောင်မြင်မှု (အတုံးအခဲများ ပြီးစီး): ≥99%

## ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — ယုံကြည်မှုမရှိသောရရှိနိုင်မှု၊ TTFB P95၊ ငြင်းဆန်မှု ပျက်ပြားမှုနှင့် OTEL တိုင်းတာမှုမှတစ်ဆင့် PoR/PoTR ကျရှုံးမှုများကို ခြေရာခံသည်။
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — ရင်းမြစ်ပေါင်းစုံ ဝန်ချခြင်း၊ ထပ်စမ်းခြင်း၊ ဝန်ဆောင်မှုပေးသူ ပျက်ကွက်ခြင်းနှင့် ဆိုင်ခွဲများ ပေါက်ကွဲခြင်းများ ပါဝင်သည်။
3. **SoraNet ကိုယ်ရေးကိုယ်တာ မက်ထရစ်** (`dashboards/grafana/soranet_privacy_metrics.json`) — `soranet_privacy_last_poll_unixtime`၊ `soranet_privacy_collector_enabled`၊ နှင့် `soranet_privacy_poll_errors_total{provider}` မှတစ်ဆင့် အမည်ဝှက်ထားသော relay ပုံးများ၊ ဖိနှိပ်မှုပြတင်းပေါက်များနှင့် စုဆောင်းသူကျန်းမာရေးကို ဇယားကွက်များ။
4. **စွမ်းဆောင်ရည်ကျန်းမာရေး** (`dashboards/grafana/sorafs_capacity_health.json`) — ဝန်ဆောင်မှုပေးသူ၏ ခေါင်းခန်းအပြင် SLA တိုးချဲ့မှုများကို ပြုပြင်ခြင်း၊ ဝန်ဆောင်မှုပေးသူမှ တန်းစီခြင်း၏အနက်ကို ပြုပြင်ခြင်း၊ နှင့် GC ရှင်းရှင်းလင်းလင်း/ထုတ်ပယ်ခြင်း/ဘိုက်များ လွတ်မြောက်ခြင်း/ပိတ်ဆို့ထားသော အကြောင်းရင်းများ/သက်တမ်းကုန်-ထင်ရှားသောအသက်အရွယ်နှင့် ပြန်လည်သင့်မြတ်ရေး ကွဲလွဲနေသည့် လျှပ်တစ်ပြက်ပုံများ။

သတိပေးချက်အတွဲများ-

- `dashboards/alerts/sorafs_gateway_rules.yml` — ဂိတ်ဝေးရရှိနိုင်မှု၊ TTFB၊ အထောက်အထားပျက်ကွက်မှုများ
- `dashboards/alerts/sorafs_fetch_rules.yml` — သံစုံတီးဝိုင်း ပျက်ကွက်မှုများ/ ပြန်ကြိုးစားခြင်း/ စတိုးဆိုင်များ၊ `scripts/telemetry/test_sorafs_fetch_alerts.sh`၊ `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`၊ `dashboards/alerts/tests/soranet_privacy_rules.test.yml`၊ နှင့် `dashboards/alerts/tests/soranet_policy_rules.test.yml` မှတဆင့် တရားဝင်အတည်ပြုထားသည်။
- `dashboards/alerts/sorafs_capacity_rules.yml` — စွမ်းရည်ဖိအားများအပြင် SLA/ backlog/ ငှားရမ်းခြင်း- သက်တမ်းကုန်ဆုံးမှုသတိပေးချက်များ နှင့် ပြုပြင်ထိန်းသိမ်းမှုအကွာအဝေးများအတွက် GC အရောင်းဆိုင်/ ပိတ်ဆို့ထားသော/ အမှားသတိပေးချက်များ။
- `dashboards/alerts/soranet_privacy_rules.yml` — ကိုယ်ရေးကိုယ်တာအဆင့်နှိမ့်ချမှု spikes၊ ဖိနှိပ်မှုနှိုးဆော်သံများ၊ စုဆောင်းသူ-အလုပ်မလုပ်ဘဲ ထောက်လှမ်းခြင်း နှင့် မသန်စွမ်းသူ-စုဆောင်းသူသတိပေးချက်များ (`soranet_privacy_last_poll_unixtime`၊ `soranet_privacy_collector_enabled`)။
- `dashboards/alerts/soranet_policy_rules.yml` — `sorafs_orchestrator_brownouts_total` သို့ ကြိုးတပ်ထားသော အမည်ဝှက် နှိုးဆော်သံများ။
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai ကြည့်ရှုသူ ပျံ့လွင့်ခြင်း/အသုံးပြုခြင်း/CEK နှိုးစက်များအပြင် SoraFS မှ ပံ့ပိုးပေးထားသည့် ကျန်းမာရေးဆိုင်ရာ ပြစ်ဒဏ်များ/အအေးမိခြင်းသတိပေးချက်အသစ်များ။

## ခြေရာခံဗျူဟာ

- OpenTelemetry ကို အဆုံးမှ အဆုံးထိ အသုံးပြုပါ-
  - တောင်းဆိုချက် ID များ၊ ထင်ရှားသော အချေအတင်များနှင့် တိုကင်နံပါတ်များ ဖြင့် မှတ်သားထားသော OTLP အပိုင်းများ (HTTP) ကို Gateways မှ ထုတ်လွှတ်ပါသည်။
  - သံစုံတီးဝိုင်းသည် `tracing` + `opentelemetry` ကို အသုံးပြု၍ ထုတ်ယူရန် ကြိုးပမ်းမှုများအတွက် အပိုင်းများကို ထုတ်ယူသည်။
  - PoR စိန်ခေါ်မှုများနှင့် သိုလှောင်မှုဆိုင်ရာ လုပ်ဆောင်ချက်များအတွက် SoraFS ဆုံမှတ်များ တင်ပို့မှုအပိုင်းများကို ထည့်သွင်းထားသည်။ အစိတ်အပိုင်းအားလုံးသည် `x-sorafs-trace` မှတစ်ဆင့် ပျံ့နှံ့နေသော ဘုံခြေရာခံ ID ကို မျှဝေပါသည်။
- `SorafsFetchOtel` သည် သံစုံတီးဝိုင်းမက်ထရစ်များကို OTLP ဟစ်စတိုဂရမ်များအဖြစ် တံတားထိုးပေးကာ `telemetry::sorafs.fetch.*` ဖြစ်ရပ်များသည် မှတ်တမ်းကိုဗဟိုပြုသည့်နောက်ကွယ်အတွက် ပေါ့ပါးသော JSON payload များကို ပံ့ပိုးပေးပါသည်။
- စုဆောင်းသူများ- Prometheus/Loki/Tempo (Tempo ဦးစားပေး) နှင့်အတူ OTEL စုဆောင်းသူများကို လုပ်ဆောင်ပါ။ Jaeger API တင်ပို့သူများသည် ရွေးချယ်ခွင့်ရှိသေးသည်။
- High-cardinality operations ကို နမူနာယူသင့်သည် (အောင်မြင်မှုလမ်းကြောင်းများအတွက် 10%၊ ကျရှုံးမှုအတွက် 100%)။

## TLS Telemetry Coordination (SF-5b)

- မက်ထရစ်ချိန်ညှိမှု-
  - TLS အလိုအလျောက်စနစ် `sorafs_gateway_tls_cert_expiry_seconds`၊ `sorafs_gateway_tls_renewal_total{result}` နှင့် `sorafs_gateway_tls_ech_enabled` သင်္ဘောများ။
  - TLS/Certificate panel အောက်ရှိ Gateway Overview ဒက်ရှ်ဘုတ်တွင် ဤအညွှန်းကိန်းများကို ထည့်သွင်းပါ။
- သတိပေးချက် ချိတ်ဆက်မှု-
  - TLS သက်တမ်းကုန်ဆုံးချိန်သတိပေးချက်များ မီးလောင်သောအခါ (≤14 ရက်ကျန်) သည် ယုံကြည်စိတ်ချရသောရရှိနိုင်မှု SLO နှင့် ဆက်စပ်နေပါသည်။
  - ECH မသန်စွမ်းမှုသည် TLS နှင့်ရရှိနိုင်မှုအကန့်နှစ်ခုလုံးကိုရည်ညွှန်းသည့်ဒုတိယသတိပေးချက်ကိုထုတ်လွှတ်သည်။
- ပိုက်လိုင်း- TLS အလိုအလျောက်စနစ်ဆိုင်ရာအလုပ်သည် တူညီသော Prometheus stack သို့ ဂိတ်ဝေးမက်ထရစ်များအဖြစ် တင်ပို့သည်။ SF-5b နှင့် ညှိနှိုင်းဆောင်ရွက်ခြင်းသည် ပွားနေသော ကိရိယာများကို သေချာစေသည်။

## မက်ထရစ်အမည်ပေးခြင်းနှင့် အညွှန်းသဘောတူချက်များ

- မက်ထရစ်အမည်များသည် I18NT000000317X သို့မဟုတ် `sorafs_*` နှင့် gateway တို့ကို အသုံးပြုသည့် ရှေ့နောက်ဆက်တွဲများအတိုင်း ဖြစ်သည်။
- အညွှန်းအစုံများကို စံပြုထားပါသည်။
  - `result` → HTTP ရလဒ် (`success`, `refused`, `failed`)။
  - `reason` → ငြင်းဆို/အမှားကုဒ် (`unsupported_chunker`၊ `timeout` စသည်ဖြင့်)။
  - `status` → ပြုပြင်ခြင်းလုပ်ငန်းအခြေအနေ (`queued`, `in_progress`, `completed`, `failed`, `escalated`)။
  - `outcome` → ပြုပြင်ငှားရမ်းခြင်း သို့မဟုတ် တုံ့ပြန်မှုရလဒ် (`requeued`၊ `escalated`၊ `completed`၊ `failed`)။
  - `provider` → hex-encoded ပံ့ပိုးပေးသူ အမှတ်အသား။
  - `manifest` → canonical manifest digest (Cardinality မြင့်မားသောအခါ ဖြတ်တောက်ထားသည်)။
  - `tier` → ကြေငြာလွှာ တံဆိပ်များ (`hot`၊ `warm`၊ `archive`)။
- Telemetry ထုတ်လွှတ်မှုအချက်များ
  - Gateway metrics များသည် `torii_sorafs_*` အောက်တွင် နေထိုင်ပြီး `crates/iroha_core/src/telemetry.rs` မှ ကွန်ဗင်းရှင်းများကို ပြန်သုံးပါ။
  - သံစုံတီးဝိုင်းသည် `sorafs_orchestrator_*` မက်ထရစ်များနှင့် `telemetry::sorafs.fetch.*` ဖြစ်ရပ်များ (ဘဝသံသရာ၊ ပြန်လည်ကြိုးစားမှု၊ ဝန်ဆောင်မှုပေးသူ ပျက်ကွက်မှု၊ အမှားအယွင်း၊ ကုပ်ကုပ်) ကို ထင်ရှားသော အနှစ်ချုပ်၊ အလုပ် ID၊ ဒေသနှင့် ဝန်ဆောင်မှုပေးသူ ခွဲခြားသတ်မှတ်မှုများဖြင့် တဂ်လုပ်ထားသည်။
  - Nodes မျက်နှာပြင် `torii_sorafs_storage_*`၊ `torii_sorafs_capacity_*` နှင့် `torii_sorafs_por_*`။
- အညွှန်းစာပါမျှော်လင့်ချက်များအပါအဝင် မျှဝေထားသော Prometheus အမည်ပေးခြင်းစာရွက်တွင် မက်ထရစ်ကတ်တလောက်ကို မှတ်ပုံတင်ရန် Observability နှင့် ညှိနှိုင်းပါ။

## ဒေတာပိုက်လိုင်း

- စုဆောင်းသူများသည် အစိတ်အပိုင်းတစ်ခုစီနှင့်အတူ OTLP ကို Prometheus (မက်ထရစ်များ) နှင့် Loki/Tempo (မှတ်တမ်းများ/ခြေရာခံများ) သို့ တင်ပို့သည်။
- ရွေးချယ်နိုင်သော eBPF (Tetragon) သည် gateways/nodes အတွက် အဆင့်နိမ့်ခြေရာခံခြင်းကို ကြွယ်ဝစေသည်။
- Torii အတွက် `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` ကိုသုံး၍ မြှုပ်သွင်းထားသော node များ၊ တီးမှုတ်သူသည် `install_sorafs_fetch_otlp_exporter` ကို ဆက်လက်ခေါ်ဆိုပါသည်။

## အထောက်အထားချိတ်

- stall metrics နှင့် privacy ကိုနှိပ်ကွပ်ခြင်းစစ်ဆေးမှုများနှင့်အတူ Prometheus သတိပေးချက်စည်းမျဉ်းများသည် lockstep တွင်ရှိနေကြောင်းသေချာစေရန် CI အတွင်း `scripts/telemetry/test_sorafs_fetch_alerts.sh` ကိုဖွင့်ပါ။
- Grafana ဒက်ရှ်ဘုတ်များကို ဗားရှင်းထိန်းချုပ်မှု (`dashboards/grafana/`) အောက်တွင် ထားရှိကာ အကန့်များပြောင်းလဲသည့်အခါ ဖန်သားပြင်ပုံများ/လင့်ခ်များကို အပ်ဒိတ်လုပ်ပါ။
- `scripts/telemetry/log_sorafs_drill.sh` မှတဆင့် Chaos လေ့ကျင့်ခန်းမှတ်တမ်းရလဒ်များ။ validation သည် `scripts/telemetry/validate_drill_log.sh` ([Operations Playbook](operations-playbook.md ကိုကြည့်ပါ))။