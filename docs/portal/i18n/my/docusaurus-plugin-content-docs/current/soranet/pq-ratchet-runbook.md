---
id: pq-ratchet-runbook
lang: my
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

##ရည်ရွယ်ချက်

ဤအပြေးစာအုပ်သည် SoraNet ၏အဆင့်လွန် ကွမ်တမ် (PQ) အမည်ဝှက်ခြင်းမူဝါဒအတွက် မီးလေ့ကျင့်မှုအပိုင်းကို လမ်းညွှန်ပေးသည်။ အော်ပရေတာများသည် ပရိုမိုးရှင်းနှစ်ခုလုံးကို အစမ်းလေ့ကျင့်သည် (Stage A -> Stage B -> Stage C) နှင့် PQ ထောက်ပံ့မှု ကျဆင်းသွားသောအခါ Stage B/A သို့ ထိန်းချုပ်ထားသော ရွှေ့ဆိုင်းမှုကို ပြန်လည်ပြုလုပ်သည်။ လေ့ကျင့်ခန်းသည် တယ်လီမီတာချိတ်များ (`sorafs_orchestrator_policy_events_total`၊ `sorafs_orchestrator_brownouts_total`၊ `sorafs_orchestrator_pq_ratio_*`) ကို သက်သေပြပြီး အဖြစ်အပျက် အစမ်းလေ့ကျင့်မှုမှတ်တမ်းအတွက် ရှေးဟောင်းပစ္စည်းများကို စုဆောင်းပါသည်။

## လိုအပ်ချက်များ

- နောက်ဆုံးထွက် `sorafs_orchestrator` စွမ်းရည်-အလေးချိန် (`docs/source/soranet/reports/pq_ratchet_validation.md` တွင်ပြသထားသည့် လေ့ကျင့်မှုအကိုးအကား သို့မဟုတ် အပြီးတွင် ကတိပြုသည်)။
- I18NI000000019X ဝန်ဆောင်မှုပေးသော Prometheus/Grafana စတက်ခ်သို့ ဝင်ရောက်ခွင့်။
- အမည်ခံကိုယ်ရံတော်လမ်းညွှန် လျှပ်တစ်ပြက်။ လေ့ကျင့်ခန်းမလုပ်မီ မိတ္တူကို ရယူပြီး အတည်ပြုပါ-

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

အရင်းအမြစ်လမ်းညွှန်သည် JSON ကိုသာထုတ်ဝေပါက၊ လည်ပတ်မှုအကူအညီများကိုမလည်ပတ်မီ I18NI000000020X ဖြင့် Norito binary သို့ ပြန်ကုဒ်လုပ်ပါ။

- CLI ဖြင့် မက်တာဒေတာနှင့် အဆင့်မီထုတ်ပေးသူ လှည့်ခြင်းဆိုင်ရာ အနုပညာပစ္စည်းများကို ဖမ်းယူပါ-

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- ခေါ်ဆိုမှုအဖွဲ့များမှ ကွန်ရက်ချိတ်ဆက်ခြင်းနှင့် ကြည့်ရှုနိုင်မှုဆိုင်ရာ ဝင်းဒိုးကို ပြောင်းလဲပါ။

## ပရိုမိုးရှင်းအဆင့်

1. **အဆင့်စာရင်းစစ်**

   စတင်သည့်အဆင့်ကို မှတ်တမ်းတင်ပါ-

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   ပရိုမိုးရှင်းမတိုင်မီ `anon-guard-pq` ကိုမျှော်လင့်ပါ။

2. **အဆင့် B (Majority PQ)** သို့ မြှင့်တင်ပါ။

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - မန်နီးဖက်စ်များကို ပြန်လည်စတင်ရန် >=5 မိနစ်စောင့်ပါ။
   - Grafana (`SoraNet PQ Ratchet Drill` ဒက်ရှ်ဘုတ်) တွင် "မူဝါဒဖြစ်ရပ်များ" အကန့်မှ `outcome=met` အတွက် `stage=anon-majority-pq` ကိုပြသကြောင်းအတည်ပြုပါ။
   - ဖန်သားပြင်ဓာတ်ပုံ သို့မဟုတ် အကန့် JSON ကို ဖမ်းယူပြီး အဖြစ်အပျက်မှတ်တမ်းတွင် ၎င်းကို ပူးတွဲပါ။

3. **အဆင့် C (Strict PQ)** သို့ မြှင့်တင်ပါ။

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` လမ်းကြောင်းကို 1.0 သို့ အတည်ပြုပါ။
   - အညိုရောင်ကောင်တာသည် ပြားချပ်ချပ်ဖြစ်နေကြောင်း အတည်ပြုပါ။ သို့မဟုတ်ပါက ရာထူးမှနုတ်ထွက်ခြင်း အဆင့်များကို လိုက်နာပါ။

## Demotion / brownout drill

1. **ဓာတု PQ ပြတ်လပ်မှုကို ဖြစ်ပေါ်စေသည်**

   အစောင့်လမ်းညွှန်ကို ဂန္တဝင်ဝင်ပေါက်များသာအဖြစ် ချုံ့ခြင်းဖြင့် ကစားကွင်းပတ်ဝန်းကျင်ရှိ PQ relays များကို ပိတ်ပါ၊ ထို့နောက် သံစုံတီးဝိုင်း ကက်ရှ်ကို ပြန်လည်စတင်ပါ-

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **အညိုရောင် တယ်လီမီတာကို စောင့်ကြည့်ပါ**

   - ဒက်ရှ်ဘုတ်- အကန့် "Brownout Rate" သည် 0 အထက်တွင် တက်လာသည်။
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` သည် `anonymity_reason="missing_majority_pq"` ဖြင့် `anonymity_outcome="brownout"` အစီရင်ခံသင့်သည်။

3. ** အဆင့် B / အဆင့် A ** အဆင့်သို့ နှိမ့်ချရန်၊

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ ထောက်ပံ့မှု မလုံလောက်ပါက `anon-guard-pq` သို့ နှိမ့်ချပါ။ ထီပေါက်သည့်ကောင်တာများ အခြေချပြီးသည်နှင့် ပရိုမိုးရှင်းများကို ပြန်လည်လျှောက်ထားနိုင်ပါသည်။

4. **ကိုယ်ရံတော်လမ်းညွှန်ကို ပြန်လည်ရယူပါ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## တယ်လီမီတာနှင့် ပစ္စည်းများ

- **ဒိုင်ခွက်-** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus သတိပေးချက်များ-** `sorafs_orchestrator_policy_events_total` အညိုရောင်ထွက်ခြင်းသတိပေးချက်သည် ပြင်ဆင်သတ်မှတ်ထားသော SLO (10 မိနစ်အတွင်း မည်သည့်ဝင်းဒိုးတွင်မဆို <5%) ရှိနေကြောင်း သေချာပါစေ။
- **ဖြစ်ရပ်မှတ်တမ်း-** ဖမ်းယူထားသော တယ်လီမီတာအတိုအထွာများနှင့် အော်ပရေတာမှတ်စုများကို `docs/examples/soranet_pq_ratchet_fire_drill.log` တွင် ထည့်သွင်းပါ။
- **လက်မှတ်ရေးထိုးထားသော ဖမ်းယူမှု-** လေ့ကျင့်ခန်းမှတ်တမ်းနှင့် အမှတ်စာရင်းဘုတ်အား `artifacts/soranet_pq_rollout/<timestamp>/` သို့ ကူးယူကာ BLAKE3 အချေအတင်များကို တွက်ချက်ပြီး `rollout_capture.json` ကို ထုတ်လုပ်ရန် `cargo xtask soranet-rollout-capture` ကို အသုံးပြုပါ။

ဥပမာ-

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ထုတ်လုပ်ထားသော မက်တာဒေတာနှင့် လက်မှတ်ကို အုပ်ချုပ်မှုပက်ကတ်တွင် ပူးတွဲပါ။

## နောက်ပြန်ဆုတ်

လေ့ကျင့်ခန်းသည် အမှန်တကယ် PQ ပြတ်တောက်မှုများကို ဖော်ထုတ်ပါက Stage A တွင် ဆက်လက်ရှိနေကာ Networking TL ကို အကြောင်းကြားပြီး စုဆောင်းထားသော မက်ထရစ်များနှင့် အစောင့်အကြပ်လမ်းညွှန်ကွဲပြားမှုများကို အဖြစ်အပျက်ခြေရာခံကိရိယာသို့ ပူးတွဲပါ။ ပုံမှန်ဝန်ဆောင်မှုကို ပြန်လည်ရယူရန် အစောပိုင်းက ရိုက်ကူးထားသော အစောင့်လမ်းညွှန်ထုတ်ယူမှုကို အသုံးပြုပါ။

:::tip Regression Coverage
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` သည် ဤအစမ်းလေ့ကျင့်မှုကို ပံ့ပိုးပေးသည့် ဓာတုတရားဝင်အတည်ပြုချက်ကို ပံ့ပိုးပေးသည်။
:::