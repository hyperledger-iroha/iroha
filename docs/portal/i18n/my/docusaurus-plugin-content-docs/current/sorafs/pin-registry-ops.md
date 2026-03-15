---
id: pin-registry-ops
lang: my
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

## ခြုံငုံသုံးသပ်ချက်

ဤ runbook သည် SoraFS pin registry နှင့် ၎င်း၏ ထပ်တူပြုခြင်းဝန်ဆောင်မှုအဆင့် သဘောတူညီချက်များ (SLAs) ကို စောင့်ကြည့်ပြီး စမ်းသပ်နည်းကို မှတ်တမ်းတင်ထားသည်။ မက်ထရစ်များကို `iroha_torii` မှ မြစ်ဖျားခံပြီး I18NI000000019X အမည်စကွက်အောက်တွင် Prometheus မှတဆင့် တင်ပို့သည်။ Torii သည် နောက်ခံ 30 စက္ကန့်ကြားကာလတွင် registry state ကိုနမူနာယူသည်၊ ထို့ကြောင့် `/v2/sorafs/pin/*` အဆုံးမှတ်များကို စစ်တမ်းကောက်ယူနေသည့် အော်ပရေတာများမရှိသည့်တိုင် ဒက်ရှ်ဘုတ်များသည် လက်ရှိရှိနေပါသည်။ အောက်ဖော်ပြပါ ကဏ္ဍများသို့ တိုက်ရိုက်မြေပုံပြုလုပ်နိုင်သော အဆင်သင့်အသုံးပြုနိုင်သော Grafana အပြင်အဆင်အတွက် ရွေးချယ်ထားသော ဒက်ရှ်ဘုတ် (`docs/source/grafana_sorafs_pin_registry.json`) ကို တင်သွင်းပါ။

## မက်ထရစ်အကိုးအကား

| မက်ထရစ် | တံဆိပ်များ | ဖော်ပြချက် |
| ------| ------| ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | သက်ကယ်စက်ဝန်းအခြေအနေဖြင့် On-chain စာရင်းအင်းကို ထင်ရှားစေသည်။ |
| `torii_sorafs_registry_aliases_total` | — | မှတ်ပုံတင်မှုတွင် မှတ်တမ်းတင်ထားသော လက်ရှိ ထင်ရှားသောအမည်တူများ အရေအတွက်။ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | ကူးယူမှု အမှာစာ မှတ်တမ်းကို အခြေအနေအရ ပိုင်းခြားထားသည်။ |
| `torii_sorafs_replication_backlog_total` | — | အဆင်ပြေမှု gauge mirroring `pending` အော်ဒါများ။ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA စာရင်းကိုင်- `met` သည် နောက်ဆုံးသတ်မှတ်ရက်အတွင်း ပြီးမြောက်သော မှာယူမှုများကို ရေတွက်သည်၊ `missed` သည် နောက်ကျပြီးစီးမှုများ + သက်တမ်းကုန်ဆုံးခြင်း၊ `pending` သည် ထူးထူးခြားခြား အော်ဒါများကို ထင်ဟပ်စေသည်။ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | စုစည်းပြီးစီးမှု ကြာမြင့်ချိန် (ထုတ်ပေးခြင်းနှင့် ပြီးစီးမှုကြားကာလများ)။ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | ဆိုင်းငံ့ထားသော-အမှာစာ ပျော့ကွက်ပြတင်းပေါက်များ (နောက်ဆုံးရက် အနုတ်လက္ခဏာ ထုတ်ပေးသည့် ကာလ)။ |

လျှပ်တစ်ပြက်ဆွဲခြင်းတိုင်းတွင် တိုင်းတာမှုအားလုံးကို ပြန်လည်သတ်မှတ်ထားသောကြောင့် ဒက်ရှ်ဘုတ်များသည် `1m` cadence သို့မဟုတ် ထို့ထက်ပိုမိုမြန်ဆန်သင့်သည်။

## Grafana ဒိုင်ခွက်

ဒက်ရှ်ဘုတ် JSON သည် အော်ပရေတာ အလုပ်အသွားအလာများကို ဖုံးအုပ်ထားသည့် အကန့် ခုနစ်ခုဖြင့် ပေးပို့သည်။ စိတ်ကြိုက်ဇယားများကို တည်ဆောက်လိုပါက အမြန်ကိုးကားရန်အတွက် အောက်တွင်ဖော်ပြထားသော မေးခွန်းများဖြစ်သည်။

1. **Manifest lifecycle** – `torii_sorafs_registry_manifests_total` (`status` ဖြင့် အုပ်စုဖွဲ့ထားသည်)။
2. **Alias ​​catalog trend** – `torii_sorafs_registry_aliases_total`။
3. **အခြေအနေအရ မှာယူရန် တန်းစီခြင်း** – `torii_sorafs_registry_orders_total` (`status` ဖြင့် အုပ်စုဖွဲ့)။
4. **Backlog နှင့် သက်တမ်းကုန်သွားသော အမှာစာများ** – `torii_sorafs_replication_backlog_total` နှင့် `torii_sorafs_registry_orders_total{status="expired"}` ကို မျက်နှာပြင် ရွှဲစိုစေရန် ပေါင်းစပ်ထားသည်။
5. **SLA အောင်မြင်မှုအချိုး** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latency vs deadline slack** – ထပ်ဆင့် `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` နှင့် `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`။ ဥပမာအားဖြင့် သင်သည် လုံးဝ slack ကြမ်းပြင်ကို လိုအပ်သောအခါ `min_over_time` ကြည့်ရှုမှုများကို ထည့်ရန် Grafana အသွင်ပြောင်းမှုများကို အသုံးပြုပါ။

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **လွတ်သွားသော အော်ဒါများ (1 နာရီနှုန်း)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## သတိပေးချက်အဆင့်များ

- ** SLA အောင်မြင်မှု < 0.95 15 မိနစ် **
  - အဆင့်သတ်မှတ်ချက်- `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - လုပ်ဆောင်ချက်- စာမျက်နှာ SRE; ပုံတူကူးခြင်း backlog triage ကိုစတင်ပါ။
- ** 10 နှင့်အထက် ဆိုင်းငံ့ထားသော မှတ်တမ်း**
  - အဆင့်သတ်မှတ်ချက်- `torii_sorafs_replication_backlog_total > 10` ကို 10 မိနစ်ကြာ ထိန်းထားသည်။
  - လုပ်ဆောင်ချက်- ဝန်ဆောင်မှုပေးသူရရှိနိုင်မှုနှင့် Torii စွမ်းရည်အချိန်ဇယားကို စစ်ဆေးပါ။
- ** သက်တမ်းလွန်အော်ဒါများ > 0**
  - အဆင့်သတ်မှတ်ချက်- `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - လုပ်ဆောင်ချက်- ဝန်ဆောင်မှုပေးသူကို အတည်ပြုရန် အုပ်ချုပ်ရေးဆိုင်ရာ သရုပ်ပြမှုများကို စစ်ဆေးပါ။
- ** ပြီးစီးမှု p95 > နောက်ဆုံးနေ့ slack avg**
  - အဆင့်သတ်မှတ်ချက်- `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - လုပ်ဆောင်ချက်- ပံ့ပိုးပေးသူများသည် သတ်မှတ်ရက်မတိုင်မီ ကျူးလွန်ကြောင်း အတည်ပြုပါ။ ပြန်လည်တာဝန်ပေးအပ်ရန် စဉ်းစားပါ။

### ဥပမာ Prometheus စည်းမျဉ်းများ

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triage အလုပ်အသွားအလာ

1. **အကြောင်းရင်းကို ဖော်ထုတ်ပါ**
   - SLA သည် backlog နိမ့်နေချိန်တွင် spike လွတ်သွားပါက၊ ဝန်ဆောင်မှုပေးသူ၏စွမ်းဆောင်ရည် (PoR ကျရှုံးမှုများ၊ နောက်ကျမှုများ) ကိုအာရုံစိုက်ပါ။
   - တည်ငြိမ်သော လွဲချော်မှုများဖြင့် နောက်ကျကျန်နေပါက ကောင်စီ၏အတည်ပြုချက်ကို စောင့်ဆိုင်းနေသော ထင်ရှားသည့်ရလဒ်များကို အတည်ပြုရန် ဝင်ခွင့်စစ်ဆေးခြင်း (`/v2/sorafs/pin/*`)။
2. **ဝန်ဆောင်မှုပေးသူ အခြေအနေကို အတည်ပြုပါ**
   - `iroha app sorafs providers list` ကို run ပြီး ကြော်ငြာထားသော စွမ်းရည်များ ထပ်တူထပ်မျှ လိုအပ်ချက်များ ကိုက်ညီကြောင်း စစ်ဆေးပါ။
   - ပံ့ပိုးပေးထားသော GiB နှင့် PoR အောင်မြင်ကြောင်း အတည်ပြုရန် `torii_sorafs_capacity_*` တိုင်းထွာများကို စစ်ဆေးပါ။
3. **ပုံတူကူးခြင်းကို ပြန်လည်သတ်မှတ်ရန်**
   - backlog slack (`stat="avg"`) သည် 5 ကြိမ်အောက် ကျဆင်းသွားသောအခါတွင် `sorafs_manifest_stub capacity replication-order` မှတစ်ဆင့် အမှာစာအသစ်များ ထုတ်ပေးပါသည်။
   - နာမည်တူများသည် တက်ကြွသောထင်ရှားသည့်တွဲနှောင်မှုများမရှိလျှင် (`torii_sorafs_registry_aliases_total` မမျှော်လင့်ဘဲကျဆင်းသွားသည်)။
4. **စာရွက်စာတမ်းရလဒ်**
   - SoraFS လည်ပတ်မှုမှတ်တမ်းတွင် အဖြစ်အပျက်မှတ်စုများကို အချိန်တံဆိပ်တုံးများနှင့် သက်ရောက်မှုရှိသော ဖော်ပြချက်အချက်များကို မှတ်တမ်းတင်ပါ။
   - ချို့ယွင်းချက်မုဒ်အသစ်များ သို့မဟုတ် ဒက်ရှ်ဘုတ်များကို မိတ်ဆက်ပေးပါက ဤ runbook ကို အပ်ဒိတ်လုပ်ပါ။

## အစီအစဉ် ရေးဆွဲခြင်း။

ထုတ်လုပ်ရေးတွင် alias cache မူဝါဒကို ဖွင့်ရန် သို့မဟုတ် တင်းကျပ်သည့်အခါ ဤအဆင့်သတ်မှတ်ထားသော လုပ်ထုံးလုပ်နည်းကို လိုက်နာပါ-

1. **ဖွဲ့စည်းပုံပြင်ဆင်ခြင်း**
   - `torii.sorafs_alias_cache` ကို `iroha_config` (အသုံးပြုသူ → အမှန်တကယ်) တွင် သဘောတူထားသည့် TTLs များနှင့် ကျေးဇူးတော်ဝင်းဒိုးများ- `positive_ttl`၊ `refresh_window`၊ `hard_expiry`, I180NI0200, I180NI0800, `rotation_max_age`၊ `successor_grace` နှင့် `governance_grace`။ မူရင်းများသည် `docs/source/sorafs_alias_policy.md` ရှိ မူဝါဒနှင့် ကိုက်ညီပါသည်။
   - SDK များအတွက်၊ ၎င်းတို့၏ဖွဲ့စည်းပုံအလွှာများ (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` in Rust / NAPI / Python bindings) မှတဆင့် တူညီသောတန်ဖိုးများကို ဖြန့်ဝေပေးသောကြောင့် client enforcement သည် gateway နှင့်ကိုက်ညီပါသည်။
2. ** Dry-run in stage**
   - ထုတ်လုပ်မှုဆိုင်ရာ topology ကိုထင်ဟပ်စေသော အဆင့်သတ်မှတ်ထားသော အစုအဝေးတစ်ခုသို့ config ပြောင်းလဲမှုကို အသုံးချပါ။
   - ကုဒ်နှင့်အသွားအပြန်ရှိနေဆဲဖြစ်သော canonical alias များကိုအတည်ပြုရန် `cargo xtask sorafs-pin-fixtures` ကိုဖွင့်ပါ။ မကိုက်ညီမှုမှန်သမျှကို ဦးစွာဖြေရှင်းရမည်ဖြစ်ပြီး ရေစီးကြောင်းပေါ်ရှိ manifest drift ကို ဆိုလိုသည်။
   - လတ်ဆတ်သော၊ ပြန်လည်ဆန်းသစ်-ဝင်းဒိုး၊ သက်တမ်းကုန်သွားသော၊ နှင့် ခက်ခဲသောသက်တမ်းကုန်ဆုံးသွားသောကိစ္စများကို ပေါင်းစပ်ထားသော ဓာတုအထောက်အထားများဖြင့် `/v2/sorafs/pin/{digest}` နှင့် `/v2/sorafs/aliases` ကို လေ့ကျင့်ပါ။ HTTP အခြေအနေကုဒ်များ၊ ခေါင်းစီးများ (`Sora-Proof-Status`၊ `Retry-After`၊ `Warning`) နှင့် JSON ကိုယ်ထည်အကွက်များကို ဤ runbook နှင့် ကိုက်ညီအောင် စစ်ဆေးပါ။
3. **ထုတ်လုပ်မှုတွင်ဖွင့်ပါ**
   - စံပြောင်းလဲမှု ဝင်းဒိုးမှတဆင့် ဖွဲ့စည်းမှုအသစ်ကို စတင်ပါ။ ၎င်းကို Torii တွင် ဦးစွာအသုံးပြုပါ၊ ထို့နောက် node မှ မူဝါဒအသစ်ကို မှတ်တမ်းများတွင် အတည်ပြုပြီးသည်နှင့် ဂိတ်ဝေးများ/SDK ဝန်ဆောင်မှုများကို ပြန်လည်စတင်ပါ။
   - `docs/source/grafana_sorafs_pin_registry.json` ကို Grafana (သို့မဟုတ် ရှိပြီးသား ဒက်ရှ်ဘုတ်များကို အပ်ဒိတ်လုပ်ပါ) နှင့် alias cache refresh panels များကို NOC workspace တွင် ချိတ်ပါ။
4. ** ဖြန့်ကျက်ပြီးနောက် အတည်ပြုခြင်း**
   - `torii_sorafs_alias_cache_refresh_total` နှင့် `torii_sorafs_alias_cache_age_seconds` ကို မိနစ် 30 စောင့်ကြည့်ပါ။ `error`/`expired` မျဉ်းကွေးများတွင် နှိုက်နှိုက်ချွတ်ချွတ်များသည် မူဝါဒပြန်လည်စတင်သည့် windows များနှင့် ဆက်စပ်နေသင့်သည်။ မမျှော်လင့်ထားသော တိုးတက်မှုဆိုလိုသည်မှာ အော်ပရေတာများသည် ဆက်မလုပ်မီ နာမည်တူအထောက်အထားများနှင့် ပံ့ပိုးပေးသူကျန်းမာရေးကို စစ်ဆေးရပါမည်။
   - client-side logs များသည် တူညီသောမူဝါဒဆုံးဖြတ်ချက်များကိုပြသကြောင်းအတည်ပြုပါ (အထောက်အထားပျက်သွားသောအခါ သို့မဟုတ် သက်တမ်းကုန်သွားသောအခါတွင် SDK သည် အမှားအယွင်းများပေါ်လာပါမည်)။ ကလိုင်းယင့်သတိပေးချက်များမရှိခြင်းသည် မှားယွင်းသောဖွဲ့စည်းမှုတစ်ခုကို ညွှန်ပြသည်။
5. **နောက်ပြန်ဆွဲ**
   - နာမည်တူထုတ်ပေးခြင်းနောက်ကျသွားကာ ပြန်လည်ဆန်းသစ်သည့်ဝင်းဒိုးခရီးစဉ်များ မကြာခဏပြုလုပ်ပါက၊ Config တွင် `refresh_window` နှင့် `positive_ttl` ကို တိုးမြှင့်ခြင်းဖြင့် မူဝါဒကို ယာယီဖြေလျှော့ပါ၊ ထို့နောက် ပြန်လည်အသုံးပြုပါ။ `hard_expiry` ကို နဂိုအတိုင်းထားပါ၊ ထို့ကြောင့် အမှန်တကယ် ဟောင်းနွမ်းနေသော အထောက်အထားများကို ငြင်းပယ်ဆဲဖြစ်သည်။
   - တယ်လီမီတာမှ မြင့်မားသော `error` အရေအတွက်များကို ဆက်လက်ပြသနေပါက ယခင် `iroha_config` လျှပ်တစ်ပြက်ကို ပြန်ယူခြင်းဖြင့် ယခင်ဖွဲ့စည်းပုံသို့ ပြန်ပြောင်းကာ alias မျိုးဆက်နှောင့်နှေးမှုများကို ခြေရာခံရန် အဖြစ်အပျက်တစ်ခုကို ဖွင့်ပါ။

##ဆက်စပ်ပစ္စည်းများ

- `docs/source/sorafs/pin_registry_plan.md` — အကောင်အထည်ဖော်ရေး လမ်းပြမြေပုံနှင့် အုပ်ချုပ်မှုဆိုင်ရာ အကြောင်းအရာ။
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — သိုလှောင်မှုဝန်ထမ်း လုပ်ဆောင်ချက်များသည် ဤစာရင်းသွင်းစာအုပ်ကို ဖြည့်စွက်ပေးသည်။