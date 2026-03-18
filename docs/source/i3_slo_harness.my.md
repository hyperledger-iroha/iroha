---
lang: my
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO ကြိုးစည်း

Iroha 3 ထုတ်လွှတ်မှုမျဉ်းသည် အရေးကြီးသော Nexus လမ်းကြောင်းများအတွက် ရှင်းလင်းပြတ်သားသော SLO များကို သယ်ဆောင်သည်-

- နောက်ဆုံးအဆင့် အထိုင်ကြာချိန် (NX-18 cadence)
- အထောက်အထားစိစစ်ခြင်း (အသိအမှတ်ပြုလက်မှတ်များ၊ JDG အထောက်အထားများ၊ တံတားအထောက်အထားများ)
- အထောက်အထားအဆုံးမှတ်ကို ကိုင်တွယ်ခြင်း (အတည်ပြုချိန်နေချိန်မှတဆင့် Axum လမ်းကြောင်းပရောက်စီ)
- အခကြေးငွေနှင့် လောင်းကြေးလမ်းကြောင်းများ (ပေးဆောင်သူ/စပွန်ဆာနှင့် ငွေချေးစာချုပ်/မျဉ်းစောင်းများ)

## ဘတ်ဂျက်

ဘတ်ဂျက်များသည် `benchmarks/i3/slo_budgets.json` တွင်နေထိုင်ပြီး ခုံတန်းလျားသို့ တိုက်ရိုက်မြေပုံဆွဲပါ။
I3 suite ရှိ မြင်ကွင်းများ။ ရည်ရွယ်ချက်များသည် ခေါ်ဆိုမှုတိုင်းတွင် p99 ပစ်မှတ်များဖြစ်သည်-

- အခကြေးငွေ/လောင်းကြေး- ဖုန်းခေါ်ဆိုမှုတစ်ခုလျှင် 50ms (`fee_payer`၊ `fee_sponsor`၊ `staking_bond`၊ `staking_slash`)
- Commit cert / JDG / bridge verify: 80ms (`commit_cert_verify`, `jdg_attestation_verify`၊
  `bridge_proof_verify`)
- အသိအမှတ်ပြုလက်မှတ် တပ်ဆင်ခြင်း- 80ms (`commit_cert_assembly`)
- ဝင်သုံးရန် အချိန်ဇယား- 50ms (`access_scheduler`)
- အထောက်အထားအဆုံးမှတ်ပရောက်စီ- 120ms (`torii_proof_endpoint`)

လောင်ကျွမ်းမှုနှုန်း အရိပ်အမြွက်များ (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0 ကို ကုဒ်နံပါတ်
စာမျက်နှာတစ်ခုနှင့် လက်မှတ်သတိပေးချက်များအတွက် ဝင်းဒိုးများစွာအချိုးများ။

##ကြိုးသိုင်း

`cargo xtask i3-slo-harness` မှတဆင့် ကြိုးကြိုးကို အသုံးပြုပါ

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

အထွက်များ-

- `bench_report.json|csv|md` — အကြမ်း I3 bench suite ရလဒ်များ (git hash + scenarios)
- `slo_report.json|md` — ပစ်မှတ်တစ်ခုလျှင် pass/fail/budget-ratio ဖြင့် SLO အကဲဖြတ်ခြင်း

ကြိုးသည် ဘတ်ဂျက်ဖိုင်ကို သုံးစွဲပြီး `benchmarks/i3/slo_thresholds.json` ကို ပြဋ္ဌာန်းသည်။
ခုံတန်းလျားအတွင်း ပြေးနေစဉ် ပစ်မှတ်တစ်ခု နောက်ပြန်ဆုတ်သွားသောအခါ မြန်ဆန်စွာ ကျရှုံးစေရန်။

## တယ်လီမီတာနှင့် ဒက်ရှ်ဘုတ်များ

- နောက်ဆုံးအဆင့်- `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- အထောက်အထားစိစစ်ခြင်း- `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana စတင်သည့် အကန့်များသည် `dashboards/grafana/i3_slo.json` တွင် နေထိုင်ပါသည်။ Prometheus
လောင်ကျွမ်းမှုနှုန်းသတိပေးချက်များကို `dashboards/alerts/i3_slo_burn.yml` ဖြင့် ပေးထားသည်။
အထက်တွင်ထည့်သွင်းထားသောဘတ်ဂျက်များ (နောက်ဆုံးအဆင့် 2s၊ အထောက်အထား 80ms ကိုအတည်ပြုပါ၊ အဆုံးမှတ်ပရောက်စီကိုသက်သေပြပါ
120ms)။

## လုပ်ငန်းဆောင်ရွက်မှုမှတ်စု

- ညအိပ်ရာဝင်ချိန်တွင် ကြိုးကို ပြေးပါ။ `artifacts/i3_slo/<stamp>/slo_report.md` ထုတ်ဝေသည်။
  အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားအတွက် ခုံတန်းလျားများနှင့်အတူ
- ဘတ်ဂျက်မအောင်မြင်ပါက၊ ဇာတ်ညွှန်းကိုဖော်ထုတ်ရန် ခုံတန်းလျားအမှတ်အသားကိုသုံးပါ၊ ထို့နောက် တူးပါ။
  တိုက်ရိုက်တိုင်းတာမှုများနှင့်ဆက်စပ်ရန် Grafana အကန့်/သတိပေးချက်သို့။
- လမ်းကြောင်းတစ်လျှောက်တွင် ရှောင်ရှားရန် အထောက်အထားပြသည့် အဆုံးမှတ် SLO များသည် အတည်ပြုချိန်နေချိန်ကို ပရောက်စီအဖြစ် အသုံးပြုသည်။
  cardinality မှုတ်ထုတ်ခြင်း; စံမှတ်ပစ်မှတ် (120ms) သည် ထိန်းသိမ်းထားမှု/DoS နှင့် ကိုက်ညီသည်။
  အထောက်အထား API ပေါ်ရှိ guardrails