---
lang: my
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
---

# SNS Metrics & Onboarding Kit

လမ်းပြမြေပုံ အကြောင်းအရာ **SN-8** သည် ကတိနှစ်ခုကို စုစည်းထားသည်-

1. မှတ်ပုံတင်ခြင်း၊ သက်တမ်းတိုးခြင်း၊ ARPU၊ အငြင်းပွားမှုများနှင့် ထုတ်ဖော်ပြသသည့် ဒက်ရှ်ဘုတ်များကို ထုတ်ဝေပါ။
   `.sora`၊ `.nexus` နှင့် `.dao` အတွက် ပြတင်းပေါက်များကို အေးခဲထားပါ။
2. မှတ်ပုံတင်သူများနှင့် ဘဏ္ဍာစိုးများသည် DNS၊ ဈေးနှုန်းနှင့် ချိတ်ဆက်နိုင်စေရန်အတွက် onboarding kit တစ်ခု ပို့ပေးပါ
   နောက်ဆက်တွဲတစ်ခုမှ တိုက်ရိုက်မလွှင့်မီ APIs များကို တသမတ်တည်း လုပ်ဆောင်ပါ။

ဤစာမျက်နှာသည် အရင်းအမြစ်ဗားရှင်းကို ထင်ဟပ်စေသည်။
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
ထို့ကြောင့် ပြင်ပသုံးသပ်သူများသည် တူညီသောလုပ်ငန်းစဉ်ကို လိုက်နာနိုင်သည်။

## 1. မက်ထရစ်အတွဲ

### Grafana ဒက်ရှ်ဘုတ်နှင့် ပေါ်တယ်ကို ထည့်သွင်းထားသည်။

- `dashboards/grafana/sns_suffix_analytics.json` ကို Grafana သို့ တင်သွင်းပါ
  စံ API မှတဆင့် ခွဲခြမ်းစိတ်ဖြာမှုလက်ခံသူ)

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- တူညီသော JSON သည် ဤပေါ်တယ်စာမျက်နှာ၏ iframe ကို စွမ်းအားပေးသည် (**SNS KPI Dashboard** ကိုကြည့်ပါ)။
  ဒက်ရှ်ဘုတ်ကို ထိလိုက်တိုင်း ပြေးပါ။
  `npm run build && npm run serve-verified-preview` အတွင်းရှိ `docs/portal` သို့
  Grafana နှင့် embed နှစ်ခုလုံးသည် ထပ်တူကျနေစေရန် အတည်ပြုပါ။

### အကွက်များနှင့် အထောက်အထားများ

| Panel | မက်ထရစ်များ | အုပ်ချုပ်မှု အထောက်အထား |
|---------|---------|---------------------|
| မှတ်ပုံတင်ခြင်းနှင့် သက်တမ်းတိုးခြင်း | `sns_registrar_status_total` (အောင်မြင်မှု + သက်တမ်းတိုးဖြေရှင်းမှု အညွှန်းများ) | Per-suffix throughput + SLA ခြေရာခံခြင်း။ |
| ARPU / အသားတင်ယူနစ် | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | ဘဏ္ဍာရေးသည် မှတ်ပုံတင်အရာရှိ၏ ဖော်ပြချက်များကို ဝင်ငွေနှင့် ယှဉ်နိုင်သည်။ |
| အငြင်းပွားမှုများ & အေးခဲခြင်း | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | တက်ကြွစွာ အေးခဲခြင်း၊ ခုံသမာဓိ စီရင်ဆုံးဖြတ်ခြင်း နှင့် အုပ်ထိန်းသူ အလုပ်တာဝန်တို့ကို ပြသသည်။ |
| SLA/အမှားနှုန်းများ | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | ၎င်းတို့သည် သုံးစွဲသူများအပေါ် မသက်ရောက်မီ API ဆုတ်ယုတ်မှုများကို မီးမောင်းထိုးပြသည်။ |
| အစုလိုက် ထင်ရှားသော ခြေရာခံကိရိယာ | `sns_bulk_release_manifest_total`၊ `manifest_id` တံဆိပ်များဖြင့် ငွေပေးချေမှု မက်ထရစ်များ | CSV အစက်များကို ငွေပေးချေမှုလက်မှတ်များသို့ ချိတ်ဆက်သည်။ |

လစဉ် KPI ကာလအတွင်း Grafana (သို့မဟုတ် ထည့်သွင်းထားသော iframe) မှ PDF/CSV ကို ထုတ်ယူပါ
ပြန်လည်သုံးသပ်ပြီး သက်ဆိုင်ရာ နောက်ဆက်တွဲ ထည့်သွင်းမှုအောက်တွင် ပူးတွဲပါရှိသည်။
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`။ ဘဏ္ဍာစိုးများသည် SHA-256 ကိုလည်း ဖမ်းယူထားသည်။
`docs/source/sns/reports/` အောက်တွင် တင်ပို့ထားသောအတွဲ၏ (ဥပမာ၊
`steward_scorecard_2026q1.md`) ထို့ကြောင့် စာရင်းစစ်များသည် အထောက်အထားလမ်းကြောင်းကို ပြန်ဖွင့်နိုင်သည်။

### နောက်ဆက်တွဲ အလိုအလျောက်စနစ်

နောက်ဆက်တွဲဖိုင်များကို ဒက်ရှ်ဘုတ် ထုတ်ယူမှုမှ တိုက်ရိုက်ထုတ်လုပ်ခြင်းဖြင့် သုံးသပ်သူများ ရရှိစေသည်။
တသမတ်တည်းအကြေ

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- အကူအညီပေးသူက ပို့ကုန်ကို hashes လုပ်ပြီး၊ UID/tags/panel count ကိုဖမ်းယူပြီး တစ်ခုရေးပါ။
  `docs/source/sns/reports/.<suffix>/<cycle>.md` အောက်တွင် Markdown နောက်ဆက်တွဲ (ကိုကြည့်ပါ။
  `.sora/2026-03` နမူနာကို ဤ doc နှင့်အတူ ပူးတွဲတင်ပြပါသည်။
- `--dashboard-artifact` ထဲသို့ ပို့ကုန်ကို မိတ္တူကူးသည်။
  `artifacts/sns/regulatory/<suffix>/<cycle>/` ထို့ကြောင့် နောက်ဆက်တွဲသည် ၎င်းကို ကိုးကားပါသည်။
  canonical အထောက်အထားလမ်းကြောင်း; သင်ထောက်ပြရန် လိုအပ်မှသာ `--dashboard-label` ကို အသုံးပြုပါ။
  တီးဝိုင်းပြင်ပ မှတ်တမ်းတွင်။
- အုပ်ချုပ်ရေးမှတ်စုတွင် `--regulatory-entry` အမှတ်။ အထောက်အကူပေးသူက (သို့မဟုတ်
  နောက်ဆက်တွဲလမ်းကြောင်းကိုမှတ်တမ်းတင်သော `KPI Dashboard Annex` ဘလောက်တစ်ခုကို အစားထိုးသည်၊
  အထောက်အထား၊ အချေအတင်နှင့် အချိန်တံဆိပ်တုံးများ ပြန်လည်လုပ်ဆောင်ပြီးနောက် အထောက်အထားများ ထပ်တူကျနေမည်ဖြစ်သည်။
- `--portal-entry` သည် Docusaurus မိတ္တူ (`docs/portal/docs/sns/regulatory/*.md`) ကို သိမ်းဆည်းသည်
  ချိန်ညှိထားသောကြောင့် သုံးသပ်သူများသည် သီးခြား နောက်ဆက်တွဲ အနှစ်ချုပ်များကို ကိုယ်တိုင် ကွဲပြားရန် မလိုအပ်ပါ။
- အကယ်၍ သင်သည် `--regulatory-entry`/`--portal-entry` ကိုကျော်သွားပါက ထုတ်လုပ်ထားသောဖိုင်ကို ပူးတွဲပါ
  မှတ်စုတိုကို ကိုယ်တိုင်ပြုလုပ်ပြီး Grafana မှ ရိုက်ကူးထားသော PDF/CSV လျှပ်တစ်ပြက်ရိုက်ချက်များကို အပ်လုဒ်လုပ်ဆဲဖြစ်သည်။
- ထပ်တလဲလဲ တင်ပို့မှုအတွက်၊ နောက်ဆက်တွဲ/သံသရာအတွဲများကို စာရင်းပြုစုပါ။
  `docs/source/sns/regulatory/annex_jobs.json` နဲ့ run လိုက်ပါ။
  `python3 scripts/run_sns_annex_jobs.py --verbose`။ အထောက်အကူပြုသူသည် ဝင်ပေါက်တိုင်းကို လျှောက်သွားသည်၊
  ဒက်ရှ်ဘုတ်တင်ပို့မှုကို မိတ္တူကူးသည် (`dashboards/grafana/sns_suffix_analytics.json` သို့ ပုံသေဖြစ်သည်။
  သတ်မှတ်မထားသည့်အချိန်တွင်) နှင့် စည်းမျဉ်းတစ်ခုစီအတွင်းရှိ နောက်ဆက်တွဲပိတ်ဆို့ကို ပြန်လည်စတင်သည် (နှင့်၊
  ရနိုင်သည့်အခါ၊ ပေါ်တယ်) လက်မှတ်တစ်ခုတွင် မှတ်စုတို။
- အလုပ်စာရင်းကို စီ/ခွဲထားသည်ကို သက်သေပြရန် `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (သို့မဟုတ် `make check-sns-annex`) ကိုဖွင့်ပါ၊ မှတ်စုတစ်ခုစီတွင် ကိုက်ညီသော `sns-annex` အမှတ်အသားကို သယ်ဆောင်ထားပြီး နောက်ဆက်တွဲ ဆောင်းပါးတိုလည်း ရှိနေပါသည်။ အကူအညီပေးသူက အုပ်ချုပ်မှုပက်ကတ်များတွင် အသုံးပြုသည့် ဒေသန္တရ/ hash အနှစ်ချုပ်များဘေးတွင် `artifacts/sns/annex_schedule_summary.json` ရေးသည်။
၎င်းသည် မိမိကိုယ်တိုင် မိတ္တူကူးထည့်ခြင်း အဆင့်များကို ဖယ်ရှားပြီး SN-8 နောက်ဆက်တွဲ အထောက်အထားများကို တသမတ်တည်း ရှိနေစေပါသည်။
အစောင့်အကြပ်အချိန်ဇယား၊ အမှတ်အသားနှင့် CI တွင် ပျံ့လွင့်နေသည်။

## 2. Onboarding Kit အစိတ်အပိုင်းများ

### နောက်ဆက်တွဲ ဝါယာကြိုး

- Registry schema + selector စည်းမျဉ်းများ-
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  နှင့် [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)။
- DNS အရိုးစုအကူ-
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  အစမ်းလေ့ကျင့်မှု စီးဆင်းမှုနှင့်အတူ
  [gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md)။
- မှတ်ပုံတင်သူ စတင်ခြင်းတိုင်းအတွက်၊ အောက်တွင် အတိုချုံးမှတ်စုကို ရေးပါ။
  `docs/source/sns/reports/` သည် ရွေးချယ်သူနမူနာများ၊ GAR အထောက်အထားများနှင့် DNS hashe များကို အကျဉ်းချုပ်ဖော်ပြထားသည်။

### စျေးနှုံး

| တံဆိပ်အရှည် | အခြေခံအခကြေးငွေ (USD equiv) |
|-----------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| ၆–၉ | $12 |
| 10+ | $8 |

Suffix coefficients- `.sora` = 1.0×, `.nexus` = 0.8×, `.dao` = 1.3×။  
သက်တမ်းမြှောက်ကိန်းများ- 2-year −5%, 5-year −12%; ကျေးဇူးတော်ပြတင်းပေါက် = ရက် 30၊ ရွေးနှုတ်ခြင်း။
= ရက် 60 (အခကြေးငွေ 20%၊ အနည်းဆုံး $5၊ အများဆုံး $200)။ ညှိနှိုင်းထားသော သွေဖည်မှုများကို မှတ်တမ်းတင်ပါ။
မှတ်ပုံတင်လက်မှတ်။

### ပရီမီယံလေလံပွဲများနှင့် သက်တမ်းတိုးခြင်းများ

1. **ပရီမီယံရေကန်** — တံဆိပ်ခတ်ထားသော လေလံကတိကဝတ်/ထုတ်ဖော်မှု (SN-3)။ လေလံများကို ခြေရာခံပါ။
   `sns_premium_commit_total` နှင့် manifest အောက်တွင် ထုတ်ဝေပါ။
   `docs/source/sns/reports/`။
2. **ဒတ်ခ်ျကို ပြန်ဖွင့်သည်** — ကျေးဇူးတော် + ရွေးနှုတ်မှု သက်တမ်းကုန်ပြီးနောက်၊ 7-ရက် ဒတ်ခ်ျရောင်းချမှုကို စတင်ပါ။
   တစ်နေ့လျှင် 15% ပျက်စီးသည်။ အညွှန်းသည် `manifest_id` ဖြင့် ထင်ရှားသည်။
   ဒက်ရှ်ဘုတ်သည် တိုးတက်မှုကို ပေါ်အောင်လုပ်နိုင်သည်။
3. **သက်တမ်းတိုးခြင်း** — စောင့်ကြည့် `sns_registrar_status_total{resolver="renewal"}` နှင့်
   အလိုအလျောက်သက်တမ်းတိုးစစ်ဆေးမှုစာရင်းကို ဖမ်းယူပါ (အကြောင်းကြားချက်များ၊ SLA၊ နောက်ပြန်ငွေပေးချေမှုသံလမ်းများ)
   မှတ်ပုံတင်လက်မှတ်ထဲမှာ။

### Developer APIs နှင့် အလိုအလျောက်စနစ်

- API စာချုပ်များ- [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)။
- အစုလိုက်အကူအညီပေးသူနှင့် CSV အစီအစဉ်-
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)။
- ဥပမာ command-

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

KPI ဒက်ရှ်ဘုတ် filter တွင် manifest ID (`--submission-log`) ကို ထည့်သွင်းပါ
ထို့ကြောင့် ငွေကြေးသည် ထုတ်ဝေမှုတိုင်းတွင် ဝင်ငွေအကန့်များကို ညှိနှိုင်းနိုင်ပါသည်။

### အထောက်အထား အတွဲ

1. အဆက်အသွယ်များ၊ နောက်ဆက်တွဲ နယ်ပယ်နှင့် ငွေပေးချေမှုလမ်းကြောင်းများပါရှိသော မှတ်ပုံတင်အရာရှိလက်မှတ်။
2. DNS/resolver အထောက်အထားများ (ဇုန်ဖိုင်အရိုးစုများ + GAR အထောက်အထားများ)။
3. စျေးနှုန်းသတ်မှတ်ရေးစာရွက် + အုပ်ချုပ်မှုမှ အတည်ပြုထားသော အစားထိုးမှုများ။
4. API/CLI မီးခိုးစမ်းသပ်မှုလက်ရာများ (`curl` နမူနာများ၊ CLI မှတ်တမ်းများ)။
5. KPI ဒက်ရှ်ဘုတ် ဖန်သားပြင်ဓာတ်ပုံ + CSV ထုတ်ယူမှု၊ လစဉ်နောက်ဆက်တွဲတွင် ပူးတွဲပါရှိသည်။

## 3. စစ်ဆေးရန်စာရင်းကို စတင်ပါ။

| အဆင့် | ပိုင်ရှင် | Artefact |
|--------|------|----------|
| ဒက်ရှ်ဘုတ် တင်သွင်း | ထုတ်ကုန်ပိုင်းခြားစိတ်ဖြာချက် | Grafana API တုံ့ပြန်မှု + ဒက်ရှ်ဘုတ် UID |
| Portal embed validated | Docs/DevRel | `npm run build` မှတ်တမ်းများ + အစမ်းကြည့်ခြင်း စခရင်ရှော့ |
| DNS အစမ်းလေ့ကျင့်မှု ပြီးပါပြီ | ကွန်ရက်ချိတ်ဆက်ခြင်း/ Ops | `sns_zonefile_skeleton.py` အထွက်များ + runbook မှတ်တမ်း |
| မှတ်ပုံတင်သူ အလိုအလျောက်စနစ် ခြောက်သွေ့ခြင်း | မှတ်ပုံတင်အရာရှိ Eng | `sns_bulk_onboard.py` တင်ပြချက်များ မှတ်တမ်း |
| အုပ်ချုပ်ရေးဆိုင်ရာ အထောက်အထားများ | အုပ်ချုပ်ရေးကောင်စီ | ထုတ်ယူထားသော ဒက်ရှ်ဘုတ် |

မှတ်ပုံတင်အရာရှိ သို့မဟုတ် နောက်ဆက်တွဲကို အသက်မသွင်းမီ စစ်ဆေးရန်စာရင်းကို အပြီးသတ်ပါ။ လက်မှတ်ရေးထိုးသည်။
အစုအဝေးသည် SN-8 လမ်းပြမြေပုံတံခါးကို ရှင်းလင်းပြီး စာရင်းစစ်များအား ရည်ညွှန်းသည့်အခါ တစ်ခုတည်းကို ပေးသည်။
စျေးကွက်မိတ်ဆက်မှုများကို ပြန်လည်သုံးသပ်ခြင်း။