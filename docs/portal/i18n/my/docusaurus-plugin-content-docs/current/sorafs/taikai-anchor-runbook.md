---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Taikai Anchor Observability Runbook

ဤပေါ်တယ် မိတ္တူသည် canonical runbook ကို ရောင်ပြန်ဟပ်သည်။
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md)။
SN13-C routing-manifest (TRM) ကျောက်ဆူးများကို SoraFS/SoraNet အား အစမ်းလေ့ကျင့်သည့်အခါ ၎င်းကို အသုံးပြုပါ။
အော်ပရေတာများသည် spool artefacts၊ Prometheus telemetry နှင့် အုပ်ချုပ်မှုတို့ကို ဆက်စပ်နိုင်သည်
portal preview build ကို ချန်မထားဘဲ အထောက်အထား။

## နယ်ပယ်နှင့် ပိုင်ရှင်များ

- **အစီအစဉ်-** SN13-C — Taikai နှင့် SoraNS ကျောက်ဆူးများကို ပြသသည်။
- **ပိုင်ရှင်များ-** မီဒီယာပလပ်ဖောင်း WG၊ DA ပရိုဂရမ်၊ ကွန်ရက်ချိတ်ဆက်ခြင်း TL၊ Docs/DevRel။
- **ပန်းတိုင်-** Sev1/Sev2 သတိပေးချက်များ၊ တယ်လီမီတာအတွက် အဆုံးအဖြတ်ပေးသော ကစားစာအုပ်တစ်အုပ် ပေးပါ။
  Taikai လမ်းကြောင်းအတိုင်း ရှေ့သို့ တိုးထွက်နေချိန်တွင် အတည်ပြုခြင်းနှင့် သက်သေအထောက်အထားများ ဖမ်းယူခြင်း။
  နာမည်တူများ။

## အမြန်စတင်ခြင်း (Sev1/Sev2)

1. **spool artefacts များကို ဖမ်းယူပါ** — နောက်ဆုံးထွက်ကို ကူးယူပါ။
   `taikai-anchor-request-*.json`၊ `taikai-trm-state-*.json` နှင့်
   `taikai-lineage-*.json` မှ ဖိုင်များ
   အလုပ်သမားများ ပြန်လည်မစတင်မီ `config.da_ingest.manifest_store_dir/taikai/`။
2. **Dump `/status` telemetry** — မှတ်တမ်းတင်ပါ
   မည်သည့် manifest window ဖြစ်သည်ကို သက်သေပြရန် `telemetry.taikai_alias_rotations` array
   အသက်ဝင်သည်-
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များကို စစ်ဆေးပါ** — တင်ပါ။
   `dashboards/grafana/taikai_viewer.json` (cluster + stream filters) တို့ကို မှတ်သားပါ။
   စည်းကမ်းရှိသလား
   `dashboards/alerts/taikai_viewer_rules.yml` (`TaikaiLiveEdgeDrift`၊
   `TaikaiIngestFailure`၊ `TaikaiCekRotationLag`၊ SoraFS အထောက်အထား- ကျန်းမာရေးဆိုင်ရာ ဖြစ်ရပ်များ)။
4. **Prometheus** ကိုစစ်ဆေးပါ — အတည်ပြုရန် §“မက်ထရစ်အကိုးအကား” တွင် မေးမြန်းချက်များကို လုပ်ဆောင်ပါ။
   သုံးစွဲနေစဉ် latency/ drift နှင့် alias-rotation counter များသည် မျှော်လင့်ထားသည့်အတိုင်း လုပ်ဆောင်သည်။ အရှိန်မြှင့်
   အကယ်၍ `taikai_trm_alias_rotations_total` သည် ပြတင်းပေါက်များစွာအတွက် သို့မဟုတ် လျှင်
   အမှားကောင်တာများ တိုးလာသည်။

## မက်ထရစ်ကိုးကား

| မက်ထရစ် | ရည်ရွယ်ချက် |
| ---| ---|
| `taikai_ingest_segment_latency_ms` | CMAF သည် အစုအစည်း/စီးကြောင်းတစ်ခုစီတွင် တုံ့ပြန်နေချိန် ဟစ်စတိုဂရမ် (ပစ်မှတ်- p95<750ms၊ p99<900ms)။ |
| `taikai_ingest_live_edge_drift_ms` | ကုဒ်ပြောင်းကိရိယာနှင့် ကျောက်ဆူးလုပ်သားများကြားတွင် တိုက်ရိုက်ပျံ့လွင့်နေသည် (စာမျက်နှာ 99>1.5 စက္ကန့်တွင် 10 မိနစ်)။ |
| `taikai_ingest_segment_errors_total{reason}` | အကြောင်းပြချက်ဖြင့် အမှားကောင်တာများ (`decode`၊ `manifest_mismatch`၊ `lineage_replay`၊ …)။ မည်သည့်တိုးမြှင့်မှုမဆို `TaikaiIngestFailure` ကို အစပျိုးသည်။ |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v1/da/ingest` သည် နာမည်တူတစ်ခုအတွက် TRM အသစ်ကို လက်ခံသည့်အခါတိုင်း တိုးများ; rotation cadence ကိုအတည်ပြုရန် `rate()` ကိုသုံးပါ။ |
| `/status → telemetry.taikai_alias_rotations[]` | `window_start_sequence`၊ `window_end_sequence`၊ `manifest_digest_hex`၊ `rotations_total` နှင့် အထောက်အထားအတွဲများအတွက် အချိန်တံဆိပ်ရိုက်နှိပ်ထားသော JSON လျှပ်တစ်ပြက်။ |
| `taikai_viewer_*` (rebuffer၊ CEK လည်ပတ်မှုအသက်၊ PQ ကျန်းမာရေး၊ သတိပေးချက်များ) | ကျောက်ဆူးများအတွင်း CEK လည်ပတ်မှု + PQ ဆားကစ်များ ကျန်းမာနေစေရန် ကြည့်ရှုသူဘက်မှ KPI များ။ |

### PromQL အတိုအထွာများ

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များ

- **Grafana ကြည့်ရှုရန်ဘုတ်-** `dashboards/grafana/taikai_viewer.json` — p95/p99
  latency၊ တိုက်ရိုက်-အနားသတ်ပျံ့လွင့်မှု၊ အပိုင်းအမှားအယွင်းများ၊ CEK လည်ပတ်မှုအသက်၊ ကြည့်ရှုသူသတိပေးချက်များ။
- **Grafana ကက်ရှ်ဘုတ်-** `dashboards/grafana/taikai_cache.json` — ပူ/ပူ/အေး
  alias windows လှည့်သည့်အခါ ပရိုမိုးရှင်းများနှင့် QoS ငြင်းဆိုမှုများ။
- ** သတိပေးမန်နေဂျာ စည်းမျဉ်းများ-** `dashboards/alerts/taikai_viewer_rules.yml` — ပျံ့လွင့်ခြင်း။
  စာမျက်နှာ၊ ထည့်သွင်းမှု ပျက်ကွက်မှု သတိပေးချက်များ၊ CEK လည်ပတ်မှု နောက်ကျခြင်းနှင့် SoraFS အထောက်အထား-ကျန်းမာရေး
  ပြစ်ဒဏ်များ/အအေးပေးမှုများ။ ထုတ်လုပ်မှုအစုတိုင်းအတွက် လက်ခံကိရိယာများ ရှိနေကြောင်း သေချာပါစေ။

## အထောက်အထား အစုအဝေး စစ်ဆေးရန်စာရင်း

- Spool artefacts (`taikai-anchor-request-*`၊ `taikai-trm-state-*`၊
  `taikai-lineage-*`)။
- ဆိုင်းငံ့ထားသော/ပေးပို့ထားသော စာအိတ်များ၏ လက်မှတ်ရေးထိုးထားသော JSON စာရင်းကို ထုတ်လွှတ်ကာ တောင်းဆိုချက်/SSM/TRM/မျိုးရိုးဖိုင်များကို ကူးယူကာ drill အတွဲသို့ ကူးယူရန် `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` ကိုဖွင့်ပါ။ မူရင်း spool လမ်းကြောင်းသည် `torii.toml` မှ `storage/da_manifests/taikai` ဖြစ်သည်။
- `/status` `telemetry.taikai_alias_rotations` ကို ဖုံးအုပ်ထားသော လျှပ်တစ်ပြက်ရိုက်ချက်။
- အဖြစ်အပျက်ဝင်းဒိုးရှိ အထက်ဖော်ပြပါ မက်ထရစ်များအတွက် Prometheus တင်ပို့မှု (JSON/CSV)။
- စစ်ထုတ်မှုများပါရှိသော Grafana ဖန်သားပြင်ဓာတ်ပုံများ။
- သက်ဆိုင်ရာ စည်းကမ်းကို ကိုးကားသော Alertmanager IDs များ။
- ဖော်ပြထားသည့် `docs/examples/taikai_anchor_lineage_packet.md` သို့ လင့်ခ်
  canonical အထောက်အထားထုပ်ပိုး။

## ဒက်ရှ်ဘုတ်ကို ထင်ဟပ်စေခြင်း နှင့် တူးခြင်း အချိုးအကွေ့

SN13-C လမ်းပြမြေပုံလိုအပ်ချက်ကို ကျေနပ်စေခြင်းသည် Taikai ကို သက်သေပြခြင်းပင်ဖြစ်သည်။
ကြည့်ရှုသူ/ကက်ရှ်ဒက်ရှ်ဘုတ်များကို ပေါ်တယ် **နှင့်** ကျောက်ဆူးတွင် ထင်ဟပ်စေသည်။
အထောက်အထား လေ့ကျင့်ခန်းသည် ကြိုတင်ခန့်မှန်းနိုင်သော လမ်းကြောင်းပေါ်တွင် လည်ပတ်နေသည်။

1. **Portal mirroring.** `dashboards/grafana/taikai_viewer.json` အခါတိုင်း သို့မဟုတ်
   `dashboards/grafana/taikai_cache.json` အပြောင်းအလဲများ၊ မြစ်ဝကျွန်းပေါ်ဒေသများကို အကျဉ်းချုပ်ပါ။
   `sorafs/taikai-monitoring-dashboards` (ဤပေါ်တယ်) နှင့် JSON ကိုမှတ်သားပါ။
   portal PR ဖော်ပြချက်တွင် checksums များ။ အကန့်အသစ်များ/အဆင့်သတ်မှတ်ချက်များကို မီးမောင်းထိုးပြပါ။
   သုံးသပ်သူများသည် စီမံခန့်ခွဲထားသော Grafana ဖိုင်တွဲနှင့် ဆက်စပ်နိုင်သည်။
2. **လစဉ်လေ့ကျင့်ရေး။**
   - လစဉ် 15:00UTC တွင်လတိုင်း၏ပထမအင်္ဂါနေ့တွင်လေ့ကျင့်ခန်းကိုလုပ်ဆောင်ပါ။
     SN13 အုပ်ချုပ်မှု ချိန်ကိုက်မှု မတိုင်မီက မြေနေရာများ။
   - အတွင်းဘက်တွင် spool artefacts၊ `/status` telemetry နှင့် Grafana ဖန်သားပြင်ဓာတ်ပုံများကို ရိုက်ကူးပါ။
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`။
   - ကွပ်မျက်မှုကိုမှတ်တမ်းတင်ပါ။
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`။
3. **ပြန်လည်သုံးသပ်ခြင်းနှင့်ထုတ်ဝေခြင်း။** 48 နာရီအတွင်း၊ သတိပေးချက်များ/မှားယွင်းသောအပြုသဘောများကို ပြန်လည်သုံးသပ်ပါ။
   DA ပရိုဂရမ် + NetOps၊ လေ့ကျင့်မှုမှတ်တမ်းရှိ နောက်ဆက်တွဲအရာများကို မှတ်တမ်းတင်ပြီး ၎င်းကို ချိတ်ဆက်ပါ။
   `docs/source/sorafs/runbooks-index.md` မှ အုပ်ချုပ်မှုပုံးကို အပ်လုဒ်လုပ်ပါ။

ဒက်ရှ်ဘုတ်များ သို့မဟုတ် လေ့ကျင့်ခန်းများ နောက်ကွယ်မှ ပြုတ်ကျပါက SN13-C သည် 🈺 မထွက်နိုင်ပါ။ ဒါကို သိမ်းထားပါ။
အရှိန်အဟုန် သို့မဟုတ် အထောက်အထား မျှော်လင့်ချက်များ ပြောင်းလဲသည့်အခါတိုင်း အပိုင်းသည် နောက်ဆုံးပေါ်ဖြစ်သည်။

## အသုံးဝင်သောအမိန့်များ

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Taikai အချိန်တိုင်းတွင် ဤပေါ်တယ်ကော်ပီကို Canonical runbook နှင့် ထပ်တူပြုထားပါ။
ကျောက်ချခြင်း တယ်လီမီတာ၊ ဒက်ရှ်ဘုတ်များ သို့မဟုတ် အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထား လိုအပ်ချက်များ ပြောင်းလဲခြင်း။