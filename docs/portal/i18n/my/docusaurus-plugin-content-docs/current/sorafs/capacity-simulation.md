---
id: capacity-simulation
lang: my
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

ဤအပြေးစာအုပ်သည် SF-2c စွမ်းရည်စျေးကွက် သရုပ်ဖော်ကိရိယာကို မည်သို့လုပ်ဆောင်ရမည်ကို ရှင်းပြထားပြီး ရလဒ်မက်ထရစ်များကို မြင်သာစေသည်။ ၎င်းသည် `docs/examples/sorafs_capacity_simulation/` တွင် အဆုံးအဖြတ်ပေးသော ပွဲစဉ်များကို အသုံးပြု၍ ခွဲတမ်းညှိနှိုင်းမှု၊ ရှုံးနိမ့်မှု ကိုင်တွယ်မှုနှင့် အဆုံးအဖြတ်ပေးမှုကို ဖြတ်တောက်ခြင်းတို့ကို အဆုံးအဖြတ်ပေးသည်။ Capacity payloads များသည် `sorafs_manifest_stub capacity` ကို အသုံးပြုနေဆဲဖြစ်သည်။ manifest/CAR ထုပ်ပိုးမှုစီးဆင်းမှုအတွက် `iroha app sorafs toolkit pack` ကိုသုံးပါ။

## 1. CLI လက်ရာများကို ဖန်တီးပါ။

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` သည် `sorafs_manifest_stub capacity` အား Norito ပေးချေမှုများ၊ base64 blobs၊ Torii တောင်းဆိုချက်ကောင်များနှင့် JSON အနှစ်ချုပ်များအတွက်-

- ခွဲတမ်းညှိနှိုင်းမှုအခြေအနေတွင်ပါဝင်သော ပံ့ပိုးပေးသူကြေငြာချက်သုံးခု။
- ထိုဝန်ဆောင်မှုပေးသူများတစ်လျှောက် အဆင့်လိုက်ဖော်ပြချက်များကို ခွဲဝေပေးသည့် ကူးယူမှုတစ်ခု။
- ပြတ်တောက်မှုအကြိုအခြေခံလိုင်း၊ ပြတ်တောက်မှုကြားကာလနှင့် ပျက်ကွက်ပြန်လည်ရယူခြင်းအတွက် တယ်လီမီတာဓာတ်ပုံများ။
- simulated ပြတ်တောက်ပြီးနောက် ဖြတ်တောက်ရန် တောင်းဆိုသည့် အငြင်းပွားမှုတစ်ခု။

ပစ္စည်းအားလုံးသည် `./artifacts` အောက်တွင် ရှိသည် (ပထမအငြင်းအခုံအဖြစ် မတူညီသောလမ်းညွှန်တစ်ခုကို ကျော်သွားခြင်းဖြင့် အစားထိုးသည်)။ လူသားဖတ်နိုင်သော အကြောင်းအရာအတွက် `_summary.json` ဖိုင်များကို စစ်ဆေးပါ။

## 2. ရလဒ်များကို စုစည်းပြီး မက်ထရစ်များကို ထုတ်လွှတ်သည်။

```bash
./analyze.py --artifacts ./artifacts
```

ခွဲခြမ်းစိတ်ဖြာသူသည် ထုတ်လုပ်သည်-

- `capacity_simulation_report.json` - စုစည်းထားသော ခွဲဝေချထားမှုများ၊ မအောင်မြင်သော မြစ်ဝကျွန်းပေါ်ဒေသများနှင့် အငြင်းပွားမှု မက်တာဒေတာ။
- `capacity_simulation.prom` - Prometheus textfile metrics (`sorafs_simulation_*`) သည် node-exporter textfile collector သို့မဟုတ် standalone ခြစ်သည့်အလုပ်အတွက်သင့်လျော်သည်။

ဥပမာ Prometheus ခြစ်ဖွဲ့စည်းမှုပုံစံ-

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

စာသားဖိုင်စုဆောင်းသူကို `capacity_simulation.prom` တွင်ညွှန်ပြပါ (node-exporter ကိုအသုံးပြုသောအခါ `--collector.textfile.directory` မှတဆင့်ကူးယူထားသောလမ်းညွှန်ထဲသို့ ၎င်းကိုကူးယူပါ)။

## 3. Grafana ဒက်ရှ်ဘုတ်ကို တင်သွင်းပါ။

1. Grafana တွင်၊ `dashboards/grafana/sorafs_capacity_simulation.json` ကိုတင်သွင်းပါ။
2. `Prometheus` datasource variable ကို အထက်ဖော်ပြပါ ခြစ်ပစ်ရန် စီစဉ်ထားသော ပစ်မှတ်သို့ ချည်နှောင်ပါ။
3. အကန့်များကို စစ်ဆေးပါ-
   - **Quota Allocation (GiB)** သည် ဝန်ဆောင်မှုပေးသူတိုင်းအတွက် ကတိကဝတ်/သတ်မှတ်ထားသော လက်ကျန်ငွေများကို ပြသသည်။
   - **Failover Trigger** သည် ပြတ်တောက်မှု မက်ထရစ်များ စီးဝင်လာသောအခါ *Failover Active* သို့ ပြောင်းသည်။
   - ** ပြတ်တောက်စဉ်အတွင်း Uptime Drop** သည် ဝန်ဆောင်မှုပေးသူ `alpha` အတွက် ရာခိုင်နှုန်းဆုံးရှုံးမှုကို ဇယားကွက်။
   - **တောင်းဆိုထားသော မျဉ်းစောင်းရာခိုင်နှုန်း** သည် အငြင်းပွားမှုဖြေရှင်းချက်မှ ထုတ်နုတ်ထားသော ပြန်လည်ပြင်ဆင်မှုအချိုးကို မြင်ယောင်သည်။

## 4. မျှော်လင့်ထားသောစစ်ဆေးမှုများ

- `sorafs_simulation_quota_total_gib{scope="assigned"}` သည် `600` နှင့် ညီမျှသည်
- `sorafs_simulation_failover_triggered` သည် `1` နှင့် အစားထိုးပေးသူမက်ထရစ်သည် `beta` ကို မီးမောင်းထိုးပြသည်။
- `sorafs_simulation_slash_requested` သည် `0.15` (15% မျဉ်းစောင်းများ) ကို `alpha` ဝန်ဆောင်မှုပေးသူသတ်မှတ်သူအတွက် အစီရင်ခံသည်။

ပွဲစဉ်များကို CLI အစီအစဉ်မှ လက်ခံဆဲဖြစ်ကြောင်း အတည်ပြုရန် `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` ကိုဖွင့်ပါ။