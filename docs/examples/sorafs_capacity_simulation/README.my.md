---
lang: my
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#SoraFS Capacity Simulation Toolkit

ဤလမ်းညွှန်သည် SF-2c စွမ်းရည်စျေးကွက်အတွက် ပြန်လည်ထုတ်လုပ်နိုင်သော ပစ္စည်းများကို ပို့ဆောင်ပေးပါသည်။
သရုပ်သကန်။ ကိရိယာအစုံသည် ခွဲတမ်းညှိနှိုင်းမှု၊ မအောင်မြင်သည့် ကိုင်တွယ်မှုနှင့် ဖြတ်တောက်ခြင်းကို လေ့ကျင့်ပေးသည်။
ထုတ်လုပ်မှု CLI အကူအညီများနှင့် ပေါ့ပါးသော ခွဲခြမ်းစိတ်ဖြာမှု script ကို အသုံးပြု၍ ပြန်လည်ပြင်ဆင်ခြင်း။

## လိုအပ်ချက်များ

- အလုပ်ခွင်အဖွဲ့ဝင်များအတွက် `cargo run` ကို run နိုင်သော Rust toolchain။
- Python 3.10+ (စံပြစာကြည့်တိုက်သာ)။

## အမြန်စတင်ပါ။

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` ဇာတ်ညွှန်းသည် တည်ဆောက်ရန် `sorafs_manifest_stub capacity` ကို တောင်းဆိုသည်-

- ခွဲတမ်းညှိနှိုင်းမှု အစုံအလင်အတွက် သတ်မှတ်ထားသော ပံ့ပိုးပေးသူ၏ ကြေငြာချက်များ။
- ညှိနှိုင်းမှုအခြေအနေနှင့်ကိုက်ညီသော ပုံတူကူးယူမှု။
- ပျက်ကွက်ဝင်းဒိုးအတွက် Telemetry လျှပ်တစ်ပြက်ဓာတ်ပုံများ။
- ဖြတ်တောက်ထားသော တောင်းဆိုချက်ကို ဖမ်းယူထားသော အငြင်းပွားမှုတစ်ခု။

ဇာတ်ညွှန်းသည် Norito bytes (`*.to`), base64 payloads (`*.b64`), Torii တောင်းဆိုချက်
ရွေးချယ်ထားသော ပစ္စည်းအောက်တွင် လူဖတ်နိုင်သော အနှစ်ချုပ်များ (`*_summary.json`)၊
လမ်းညွှန်။

`analyze.py` သည် ထုတ်လုပ်ထားသော အနှစ်ချုပ်များကို စားသုံးပြီး စုစည်းထားသော အစီရင်ခံစာကို ထုတ်လုပ်သည်
(`capacity_simulation_report.json`) နှင့် Prometheus စာသားဖိုင်ကို ထုတ်လွှတ်သည်
တင်ဆောင်လာသော (`capacity_simulation.prom`)

- `sorafs_simulation_quota_*` ညှိနှိုင်းစွမ်းရည်နှင့် ခွဲဝေပေးဝေမှုကို ဖော်ပြသည့် တိုင်းထွာများ
  ဝန်ဆောင်မှုပေးသူအလိုက် မျှဝေပါ။
- `sorafs_simulation_failover_*` စက်ရပ်ချိန် မြစ်ဝကျွန်းပေါ်ဒေသများနှင့် ရွေးချယ်ထားသည့်အရာများကို မီးမောင်းထိုးပြသည့် တိုင်းတာမှုများ
  အစားထိုးပေးသူ။
- `sorafs_simulation_slash_requested` ထုတ်ယူထားသော ပြန်လည်ကုစားမှု ရာခိုင်နှုန်းကို မှတ်တမ်းတင်ခြင်း။
  အငြင်းပွားမှုမှ payload။

`dashboards/grafana/sorafs_capacity_simulation.json` တွင် Grafana အတွဲကို တင်သွင်းပါ
ထုတ်ပေးထားသော စာသားဖိုင်ကို ခြစ်ထုတ်သည့် Prometheus ဒေတာရင်းမြစ်တစ်ခုတွင် ညွှန်ပြပါ (အတွက်
node-exporter textfile collector မှတဆင့် ဥပမာ)။ runbook မှာ
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` အပြည့် ဖြတ်သွားသည်
Prometheus ဖွဲ့စည်းမှုဆိုင်ရာ အကြံပြုချက်များ အပါအဝင် အလုပ်အသွားအလာ။

## တန်ဆာပလာများ

- `scenarios/quota_negotiation/` — ဝန်ဆောင်မှုပေးသူ၏ ကြေငြာသတ်မှတ်ချက်များနှင့် ပုံတူကူးယူမှု အမှာစာ။
- `scenarios/failover/` — မူလပြတ်တောက်မှုနှင့် ပျက်ကွက်သောဓာတ်လှေကားအတွက် တယ်လီမီတာပြတင်းပေါက်များ။
- `scenarios/slashing/` — တူညီသောပုံတူပွားမှုအမှာစာအား ကိုးကားသည့် အငြင်းပွားမှု spec ။

ဤပြင်ဆင်မှုများကို `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` တွင် အတည်ပြုထားသည်။
၎င်းတို့သည် CLI schema နှင့် ထပ်တူကျကြောင်း အာမခံရန်။