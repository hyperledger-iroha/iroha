---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

#SoraFS ဝန်ဆောင်မှုပေးသူ ကြော်ငြာထည့်ရေး အစီအစဉ်

ဤအစီအစဥ်သည် ခွင့်ပြုချက်ပေးသူထံမှ ဖြတ်တောက်ထားသော ကြော်ငြာများကို ပေါင်းစပ်ညှိနှိုင်းပေးသည်။
အရင်းအမြစ်အစုံအလင်အတွက် လိုအပ်သော အပြည့်အဝအုပ်ချုပ်မှု `ProviderAdvertV1` မျက်နှာပြင်
ပြန်လည်ဖော်ထုတ်ခြင်း။ ၎င်းသည် ပေးပို့နိုင်သော သုံးခုကို အာရုံစိုက်သည်-

- **အော်ပရေတာလမ်းညွှန်။** အဆင့်ဆင့်လုပ်ဆောင်မှုများကို သိုလှောင်မှုပံ့ပိုးပေးသူများသည် ပြီးမြောက်ရပါမည်။
  တံခါးတစ်ခုစီမပြောင်းမီ။
- **Telemetry လွှမ်းခြုံမှု။** Observability နှင့် Ops အသုံးပြုသည့် ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များ
  ကွန်ရက်မှ အတည်ပြုရန် ကိုက်ညီသော ကြော်ငြာများကိုသာ လက်ခံပါသည်။
ထုတ်လွှင့်မှုသည် [SoraFS ရွှေ့ပြောင်းခြင်းတွင် SF-2b/2c မှတ်တိုင်များနှင့် ကိုက်ညီသည်
လမ်းပြမြေပုံ](./migration-roadmap) နှင့် ဝင်ခွင့်မူဝါဒကို လက်ခံသည်။
[ပံ့ပိုးပေးသူ ဝင်ခွင့်မူဝါဒ](./provider-admission-policy) ပါဝင်ပြီးဖြစ်သည်။
အကျိုးသက်ရောက်မှု။

## လက်ရှိလိုအပ်ချက်များ

SoraFS သည် အုပ်ချုပ်မှု-ထုပ်ပိုးထားသော `ProviderAdvertV1` ပေးချေမှုများကိုသာ လက်ခံသည်။ ဟိ
ဝင်ခွင့်အတွက် အောက်ပါလိုအပ်ချက်များကို ပြဌာန်းထားပါသည်။

- Canonical `profile_aliases` ပါရှိသည့် `profile_id=sorafs.sf1@1.0.0`။
- `chunk_range_fetch` စွမ်းရည် ပေးချေမှုများသည် ရင်းမြစ်ပေါင်းစုံအတွက် ပါဝင်ရပါမည်။
  ပြန်လည်ဖော်ထုတ်ခြင်း။
- `signature_strict=true` ကြော်ငြာတွင် ပူးတွဲပါရှိသော ကောင်စီလက်မှတ်များ
  စာအိတ်။
- `allow_unknown_capabilities` ကို တိကျရှင်းလင်းသော GREASE လေ့ကျင့်ခန်းများတွင်သာ ခွင့်ပြုသည်
  နှင့် logged ရပါမည်။

## အော်ပရေတာစစ်ဆေးစာရင်း

1. **စာရင်းကြော်ငြာများ။** ထုတ်ဝေပြီးသော ကြော်ငြာတိုင်းနှင့် မှတ်တမ်းများကို စာရင်းပြုစုပါ-
   - အုပ်ချုပ်မှုစာအိတ်လမ်းကြောင်း (`defaults/nexus/sorafs_admission/...` သို့မဟုတ် ထုတ်လုပ်မှုနှင့်ညီမျှသည်)။
   - ကြော်ငြာ `profile_id` နှင့် `profile_aliases`။
   - စွမ်းဆောင်ရည်စာရင်း (အနည်းဆုံး `torii_gateway` နှင့် `chunk_range_fetch` ကို မျှော်လင့်ထားသည်)။
   - `allow_unknown_capabilities` အလံ (ရောင်းချသူ-ကြိုတင်မှာယူထားသည့် TLV များရှိနေသည့်အခါ လိုအပ်သည်)။
2. **ပံ့ပိုးသူကိရိယာဖြင့် ပြန်လည်ထုတ်လုပ်ပါ။**
   - သေချာစေရန် သင်၏ပံ့ပိုးပေးသူ ကြော်ငြာထုတ်ဝေသူနှင့် payload ကို ပြန်လည်တည်ဆောက်ပါ-
     - `profile_id=sorafs.sf1@1.0.0`
     - သတ်မှတ်ထားသော `max_span` ဖြင့် `capability=chunk_range_fetch`
     - GREASE TLV များရှိနေသောအခါ - `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` နှင့် `sorafs_fetch` မှတဆင့် တရားဝင်စစ်ဆေးပါ။ အမည်မသိအကြောင်း သတိပေးချက်များ
     စွမ်းရည်များကို စမ်းသပ်ရပါမည်။
3. **ရင်းမြစ်ပေါင်းစုံ အဆင်သင့်ဖြစ်မှုကို အတည်ပြုပါ။**
   - `sorafs_fetch` ကို `--provider-advert=<path>` ဖြင့် လုပ်ဆောင်ပါ။ CLI က အခုပျက်နေတယ်။
     `chunk_range_fetch` ပျောက်ဆုံးသွားသည့်အခါ လျစ်လျူရှုထားသော အမည်မသိများအတွက် သတိပေးချက်များကို ပရင့်ထုတ်ပါ။
     စွမ်းရည်များ။ JSON အစီရင်ခံစာကို ဖမ်းယူပြီး လည်ပတ်မှုမှတ်တမ်းများဖြင့် သိမ်းဆည်းပါ။
4. **အဆင့် သက်တမ်းတိုးခြင်း**
   - `ProviderAdmissionRenewalV1` စာအိတ်များကို အနည်းဆုံး ရက် 30 မတိုင်မီ တင်သွင်းပါ။
     သက်တမ်းကုန်ဆုံး။ သက်တမ်းတိုးခြင်းများသည် canonical handle and capability set ကို ထိန်းသိမ်းထားရပါမည်။
     အစုရှယ်ယာများ၊ အဆုံးမှတ်များ သို့မဟုတ် မက်တာဒေတာကိုသာ ပြောင်းလဲသင့်သည်။
5. **မှီခိုအဖွဲ့များနှင့် ဆက်သွယ်ပါ။**
   - SDK ပိုင်ရှင်များသည် အော်ပရေတာများသို့ သတိပေးချက်များကို ဖော်ပြသည့် ဗားရှင်းများကို ထုတ်ပြန်ရပါမည်။
     ကြော်ငြာတွေကို ပယ်ချပါတယ်။
   - DevRel သည် အဆင့်အကူးအပြောင်းတစ်ခုစီကို ကြေညာသည်။ ဒက်ရှ်ဘုတ်လင့်ခ်များနှင့် ပါဝင်သည်။
     အောက်တွင် threshold logic။
6. ** ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များကို တပ်ဆင်ပါ။**
   - Grafana ကို တင်သွင်းပြီး **SoraFS / Provider အောက်တွင် ထားလိုက်ပါ။
     ဒက်ရှ်ဘုတ် UID `sorafs-provider-admission` ဖြင့် စတင်ဖြန့်ချိသည်။
   - မျှဝေထားသော `sorafs-advert-rollout` ကိုညွှန်ပြသည့် သတိပေးချက်စည်းမျဉ်းများကို သေချာပါစေ။
     အဆင့်မြှင့်တင်ခြင်းနှင့် ထုတ်လုပ်ခြင်းအတွက် အသိပေးချက်ချန်နယ်။

## တယ်လီမီတာနှင့် ဒက်ရှ်ဘုတ်များ

အောက်ပါ မက်ထရစ်များကို `iroha_telemetry` မှတစ်ဆင့် ဖော်ထုတ်ပြီးဖြစ်သည်-

- `torii_sorafs_admission_total{result,reason}` — အရေအတွက်များကို လက်ခံသည်၊ ငြင်းပယ်သည်၊
  နှင့်သတိပေးရလဒ်များ။ အကြောင်းရင်းများတွင် `missing_envelope`၊ `unknown_capability`၊
  `stale` နှင့် `policy_violation`။

Grafana ထုတ်ယူခြင်း- [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)။
ဖိုင်ကို မျှဝေထားသော ဒက်ရှ်ဘုတ်များ သိုလှောင်ရာသို့ တင်သွင်းပါ (`observability/dashboards`)
ထုတ်ဝေခြင်းမပြုမီ ဒေတာအရင်းအမြစ် UID ကိုသာ အပ်ဒိတ်လုပ်ပါ။

ဘုတ်အဖွဲ့သည် Grafana ဖိုင်တွဲ **SoraFS/Provider Rollout** ဖြင့် ထုတ်ဝေသည်
တည်ငြိမ်သော UID `sorafs-provider-admission`။ သတိပေးချက်စည်းကမ်းများ
`sorafs-admission-warn` (သတိပေးချက်) နှင့် `sorafs-admission-reject` (စိုးရိမ်ရ) တို့မှာ၊
`sorafs-advert-rollout` အသိပေးချက်မူဝါဒကို အသုံးပြုရန် ကြိုတင်ပြင်ဆင်ထားပါသည်။ ညှိ
ဦးတည်ရာစာရင်းကို တည်းဖြတ်ခြင်းထက် ပြောင်းလဲပါက အဆက်အသွယ်အမှတ်
ဒက်ရှ်ဘုတ် JSON။

အကြံပြုထားသည့် Grafana အကန့်များ-

| Panel | မေးမြန်းမှု | မှတ်စုများ |
|---------|------|-------|
| **ဝင်ခွင့် ရလဒ်** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | လက်ခံ vs သတိပေး vs ငြင်းပယ်မှုကို မြင်ယောင်ရန် စတန်းစီဇယား။ သတိပေးချက် > 0.05 * စုစုပေါင်း (သတိပေးချက်) သို့မဟုတ် ငြင်းပယ်သည့်အခါ သတိပေးချက် > 0 (စိုးရိမ်ရ)။ |
| **သတိပေးချက် အချိုး** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | ပေဂျာအဆင့်ကို ဖြည့်ပေးသည့် တစ်ကြောင်းချင်း အကြိမ်များ (5% သတိပေးမှုနှုန်း 15 မိနစ်အတွင်း)။ |
| **ငြင်းဆိုခြင်း** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Runbook triage ကို Drives များ၊ လျော့ပါးရေးအဆင့်များသို့ လင့်ခ်များ ပူးတွဲပါ ။ |
| **အကြွေးကို ပြန်လည်ဆန်းသစ်ခြင်း** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ဝန်ဆောင်မှုပေးသူများသည် ပြန်လည်စတင်သည့် နောက်ဆုံးရက်တွင် ပျောက်ဆုံးနေကြောင်း ညွှန်ပြသည်။ ရှာဖွေတွေ့ရှိမှု ကက်ရှ်မှတ်တမ်းများဖြင့် အပြန်အလှန်ကိုးကားခြင်း။ |

Manual dashboards အတွက် CLI artefacts

- `sorafs_fetch --provider-metrics-out` သည် `failures`၊ `successes` နှင့်
  ပံ့ပိုးသူတစ်ဦးလျှင် `disabled` ကောင်တာများ။ စောင့်ကြည့်ရန် ad-hoc ဒက်ရှ်ဘုတ်များထဲသို့ ထည့်သွင်းပါ။
  ထုတ်လုပ်မှုပံ့ပိုးပေးသူများကို မပြောင်းမီ တီးမှုတ်သူသည် ခြောက်သွေ့သောအပြေးများ။
- JSON အစီရင်ခံစာ၏ `chunk_retry_rate` နှင့် `provider_failure_rate` အကွက်များ
  ဝင်ခွင့်မပြုမီ ပိတ်ဆို့ခြင်း သို့မဟုတ် ဟောင်းနွမ်းနေသော ပေးဆောင်မှု လက္ခဏာများကို မီးမောင်းထိုးပြပါ။
  ပယ်ချခြင်း။

### Grafana ဒက်ရှ်ဘုတ် အပြင်အဆင်

Observability သည် သီးခြားဘုတ်အဖွဲ့တစ်ခုကို ထုတ်ဝေသည် — **SoraFS ပံ့ပိုးသူဝင်ခွင့်
စတင်ရောင်းချခြင်း** (`sorafs-provider-admission`) — **SoraFS / ဝန်ဆောင်မှုပေးသူ ရုပ်သိမ်းခြင်း** အောက်တွင်
အောက်ပါ canonical panel ID များဖြင့်-

- အကန့် 1 — *ဝင်ခွင့်ရလဒ်နှုန်း* (စုထားသောဧရိယာ၊ ယူနစ် “ops/min”)။
- အကန့် 2 — *သတိပေးချက် အချိုး* (စီးရီးတစ်ခုတည်း)၊ စကားရပ်ကို ထုတ်လွှတ်သည်။
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`။
- အကန့် 3 — *ငြင်းပယ်ခြင်းအကြောင်းရင်း* (`reason` ဖြင့် အုပ်စုဖွဲ့ထားသော အချိန်စီးရီး)၊
  `rate(...[5m])`။
- အကန့် 4 — *ကြွေးမြီကို ပြန်လည်စတင်ရန်* (စာရင်းဇယား)၊ အထက်ဇယားရှိ စုံစမ်းမေးမြန်းချက်အား ပြန်လှန်ကြည့်ပါ။
  ပြောင်းရွှေ့မှုစာရင်းဂျာမှ ဆွဲယူထားသော နောက်ဆုံးရက်များကို ပြန်လည်ဆန်းသစ်သည့် ကြော်ငြာဖြင့် အမှတ်အသားပြုထားသည်။

အခြေခံအဆောက်အဦ ဒက်ရှ်ဘုတ်များ repo ရှိ JSON အရိုးစုကို ကူးယူ (သို့မဟုတ်) ဖန်တီးပါ။
`observability/dashboards/sorafs_provider_admission.json` ကိုသာ update လုပ်ပါ။
ဒေတာအရင်းအမြစ် UID; panel ID များနှင့် သတိပေးချက်စည်းမျဉ်းများကို runbooks များမှကိုးကားပါသည်။
အောက်တွင်ဖော်ပြထားသော၊ ထို့ကြောင့် ဤစာရွက်စာတမ်းအား ပြန်လည်မပြင်ဆင်ဘဲ ၎င်းတို့ကို နံပါတ်ပြန်ထည့်ခြင်းမှ ရှောင်ကြဉ်ပါ။

အဆင်ပြေစေရန်အတွက် repository သည် ရည်ညွှန်း dashboard အဓိပ္ပါယ်ဖွင့်ဆိုချက်ကို ယခုပေးပို့ပါသည်။
`docs/source/grafana_sorafs_admission.json`; အကယ်၍ ၎င်းကို သင်၏ Grafana ဖိုဒါသို့ ကူးယူပါ။
ပြည်တွင်းစမ်းသပ်မှုအတွက် သင်စမှတ်တစ်ခုလိုသည်။

### Prometheus သတိပေးချက် စည်းမျဉ်းများ

အောက်ပါ စည်းကမ်းအုပ်စုကို `observability/prometheus/sorafs_admission.rules.yml` သို့ ထည့်ပါ။
(၎င်းသည် ပထမဆုံး SoraFS စည်းမျဉ်းအုပ်စုဖြစ်လျှင် ဖိုင်ကိုဖန်တီးပါ) နှင့် ၎င်းကို ထည့်သွင်းပါ။
သင်၏ Prometheus ဖွဲ့စည်းမှု။ `<pagerduty>` ကို အမှန်တကယ် routing ဖြင့် အစားထိုးပါ။
သင်၏ခေါ်ဆိုမှုအလှည့်အတွက် အညွှန်း။

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` ကိုဖွင့်ပါ။
syntax သည် `promtool check rules` ကို ကျော်သွားကြောင်း သေချာစေရန် အပြောင်းအလဲများကို မတွန်းမီ။

##ဝင်ခွင့်ရလဒ်များ

- `chunk_range_fetch` စွမ်းရည် → `reason="missing_capability"` ဖြင့် ငြင်းပယ်ခြင်း။
- `allow_unknown_capabilities=true` မပါသော အမည်မသိ TLV များ → ငြင်းပယ်ခြင်း
  `reason="unknown_capability"`။
- `signature_strict=false` → ငြင်းပယ်ခြင်း (အထီးကျန်ရောဂါရှာဖွေခြင်းအတွက် သီးသန့်)။
- သက်တမ်းကုန် `refresh_deadline` → ငြင်းပယ်ခြင်း။

## ဆက်သွယ်ရေးနှင့် အဖြစ်အပျက် ကိုင်တွယ်ခြင်း။

- **အပတ်စဉ် အခြေအနေမေးလ်။** DevRel သည် ဝင်ခွင့်ဆိုင်ရာ အကျဉ်းချုပ်ကို ဖြန့်ဝေပါသည်။
  မက်ထရစ်များ၊ ထူးထူးခြားခြား သတိပေးချက်များနှင့် လာမည့်နောက်ဆုံးရက်များ။
- ** မတော်တဆ တုံ့ပြန်မှု။** `reject` သတိပေးချက် မီးလောင်ပါက၊ ဖုန်းဆက်ပြီး အင်ဂျင်နီယာများ-
  1. Torii ရှာဖွေတွေ့ရှိမှု (`/v2/sorafs/providers`) မှတစ်ဆင့် စော်ကားသောကြော်ငြာကို ရယူပါ။
  2. ပံ့ပိုးပေးသူပိုက်လိုင်းတွင် ကြော်ငြာအတည်ပြုချက်ကို ပြန်လည်လုပ်ဆောင်ပြီး နှင့် နှိုင်းယှဉ်ပါ။
     အမှားကိုပြန်ထုတ်ရန် `/v2/sorafs/providers`။
  3. နောက်တစ်ကြိမ် ပြန်လည်စတင်ခြင်းမပြုမီ ကြော်ငြာကိုလှည့်ရန် ဝန်ဆောင်မှုပေးသူနှင့် ညှိနှိုင်းပါ။
     နောက်ဆုံးနေ့။
- **ပြောင်းလဲမှုသည် အေးခဲသွားပါသည်။** R1/R2 ကာလအတွင်း မြေယာပြောင်းလဲခြင်းမှလွဲ၍ စွမ်းဆောင်ရည် schema မရှိပါ။
  ထုတ်ပေးရေး ကော်မတီမှ လက်မှတ် ရေးထိုးခြင်း၊ GREASE စမ်းသပ်မှုများကို အချိန်အတွင်း စီစဉ်ရပါမည်။
  အပတ်စဉ် ပြုပြင်ထိန်းသိမ်းမှု ဝင်းဒိုးနှင့် ရွှေ့ပြောင်းခြင်းဆိုင်ရာ လယ်ဂျာတွင် အကောင့်ဝင်ပါ။

## ကိုးကား

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [ပံ့ပိုးပေးသူဝင်ခွင့်မူဝါဒ](./provider-admission-policy)
- [ရွှေ့ပြောင်းနေထိုင်မှု လမ်းပြမြေပုံ](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)