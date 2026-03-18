---
lang: my
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Telemetry လိုအပ်ချက်များ

## Prometheus ပစ်မှတ်များ

အောက်ဖော်ပြပါ အညွှန်းများဖြင့် relay နှင့် orchestrator ကို ခြစ်ပါ။

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## လိုအပ်သော ဒက်ရှ်ဘုတ်များ

1. `dashboards/grafana/soranet_testnet_overview.json` *(ထုတ်ဝေရန်)* — JSON ကို တင်ပါ၊ ကိန်းရှင်များ `region` နှင့် `relay_id` ကို တင်သွင်းပါ။
2. `dashboards/grafana/soranet_privacy_metrics.json` *(လက်ရှိ SNNet-8 ပိုင်ဆိုင်မှု)* — ကိုယ်ရေးကိုယ်တာပုံးပြားများကို ကွက်လပ်မရှိဘဲ တင်ဆက်ကြောင်း သေချာပါစေ။

## သတိပေးချက်စည်းကမ်းများ

သတ်မှတ်ချက်များသည် playbook မျှော်မှန်းချက်နှင့် ကိုက်ညီရမည်-

- `soranet_privacy_circuit_events_total{kind="downgrade"}` တိုးသည် > 0 10 မိနစ်ကျော်သည် `critical` ကို အစပျိုးသည်။
- မိနစ် 30 လျှင် `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 သည် `warning` ကို အစပျိုးသည်။
- `up{job="soranet-relay"}` == 0 2 မိနစ်အတွက် `critical` အစပျိုးသည်။

`testnet-t0` လက်ခံကိရိယာဖြင့် Alertmanager တွင် သင့်စည်းမျဉ်းများကို ထည့်သွင်းပါ။ `amtool check-config` ဖြင့် အတည်ပြုပါ။

## မက်ထရစ်အကဲဖြတ်ခြင်း။

၁၄ ရက်ကြာ လျှပ်တစ်ပြက်ကို စုစည်းပြီး SNNet-10 အတည်ပြုသူထံ ပေးပို့ပါ-

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- တိုက်ရိုက်ဒေတာကို လုပ်ဆောင်နေချိန်တွင် သင်ထုတ်ထားသော လျှပ်တစ်ပြက်ရိုက်ချက်ဖြင့် နမူနာဖိုင်ကို အစားထိုးပါ။
- `status = fail` ရလဒ်သည် ပရိုမိုးရှင်းကို ပိတ်ဆို့ထားသည်။ ထပ်မစမ်းမီ မီးမောင်းထိုးပြထားသည့် စစ်ဆေးချက်(များ)ကို ဖြေရှင်းပါ။

## အစီရင်ခံခြင်း။

အပတ်တိုင်း အပ်လုဒ်လုပ်သည်-

- Query လျှပ်တစ်ပြက်ပုံများ (`.png` သို့မဟုတ် `.pdf`) PQ အချိုး၊ circuit အောင်မြင်မှုနှုန်း နှင့် PoW ဖြေရှင်းချက် histogram ကိုပြသထားသည်။
- Prometheus အတွက် `soranet_privacy_throttles_per_minute` အတွက် အသံသွင်းခြင်း စည်းမျဉ်း အထွက်။
- ပစ်လွှတ်သည့်သတိပေးချက်များနှင့် လျော့ပါးစေသည့်အဆင့်များ (အချိန်တံဆိပ်နှိပ်ခြင်းများပါ၀င်သည်) ကိုဖော်ပြသည့် အကျဉ်းချုပ်ဇာတ်ကြောင်း။