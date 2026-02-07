---
lang: my
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 အဆင့်-ဂိတ် အစီရင်ခံစာ (T?_→T?_)

> တင်ပြခြင်းမပြုမီ နေရာယူထားသော နေရာတိုင်း (ထောင့်ကွင်းစကွက်များ) ကို အစားထိုးပါ။ စောင့်ရှောက်ပါ။
> အပိုင်းခေါင်းစီးများသည် ဖိုင်ကို အုပ်ချုပ်မှု အလိုအလျောက်စနစ်ဖြင့် ခွဲခြမ်းစိတ်ဖြာနိုင်သောကြောင့် ဖြစ်သည်။

## 1. မက်တာဒေတာ

| လယ် | တန်ဖိုး |
|---------|-------|
| ပရိုမိုးရှင်း | `<T0→T1 or T1→T2>` |
| အစီရင်ခံပြတင်းပေါက် | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Relays in scope | `<count + IDs or “see appendix A”>` |
| ပင်မအဆက်အသွယ် | `<name/email/Matrix handle>` |
| တင်ပြချက် မော်ကွန်း | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| SHA-256 | `<sha256:...>` |

## 2. မက်ထရစ်အကျဉ်းချုပ်

| မက်ထရစ် | စောင့်ကြည့်လေ့လာ | တံခါးခုံ | ဖြတ်သွားမလား? | အရင်းအမြစ် |
|--------|----------|-----------|------|--------|
| တိုက်နယ်အောင်မြင်မှုအချိုး | `<0.000>` | ≥0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| အကျိူးအညိုကွက်အချိုး | `<0.000>` | ≤0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR ရောနှောကွဲလွဲမှု | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 စက္ကန့် | `<0.0 s>` | ≤3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| Latency p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ အချိုး (avg) | `<0.00>` | ≥ ပစ်မှတ် | ☐ / ☑ | `telemetry/pq_summary.json` |

**ဇာတ်ကြောင်း-** `<summaries of anomalies, mitigations, overrides>`

## 3. တူးဖော်ခြင်းနှင့် အဖြစ်အပျက်မှတ်တမ်း

| Timestamp (UTC) | တိုင်းဒေသကြီး | ရိုက် | သတိပေးချက် ID | လျော့ပါးရေး အကျဉ်းချုပ် |
|--------------------|--------|------|----------------|--------------------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Attachments နှင့် hashes များ

| Artefact | မဂ် | SHA-256 |
|----------|------|---------|
| မက်ထရစ်များ လျှပ်တစ်ပြက်ရိုက်ချက် | `reports/metrics-window.json` | `<sha256>` |
| မက်ထရစ်အစီရင်ခံစာ | `reports/metrics-report.json` | `<sha256>` |
| အစောင့်အလှည့် မှတ်တမ်းများ | `evidence/guard_rotation/*.log` | `<sha256>` |
| ချည်နှောင်ခြင်းကို ထင်ရှားစေခြင်း | `evidence/exit_bonds/*.to` | `<sha256>` |
| သစ်လုံးတူး | `evidence/drills/*.md` | `<sha256>` |
| MASQUE စေတနာ (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| နောက်ပြန်ဆွဲမည့် အစီအစဉ် (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. အတည်ပြုချက်များ

| အခန်းကဏ္ဍ | အမည် | (Y/N) | လက်မှတ်ရေးထိုးခဲ့သည်။ မှတ်စုများ |
|------|------|-----------------|------|
| ကွန်ရက်ချိတ်ဆက်ခြင်း TL | `<name>` | ☐ / ☑ | `<comments>` |
| အုပ်ချုပ်ရေးကိုယ်စားလှယ် | `<name>` | ☐ / ☑ | `<comments>` |
| SRE ကိုယ်စားလှယ် | `<name>` | ☐ / ☑ | `<comments>` |

## နောက်ဆက်တွဲ A — Relay စာရင်းဇယား

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## နောက်ဆက်တွဲ B — အဖြစ်အပျက် အကျဉ်းချုပ်များ

```
<Detailed context for any incidents or overrides referenced above.>
```