---
id: nexus-elastic-lane
lang: my
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/nexus_elastic_lane.md` ဖြစ်သည်။ ဘာသာပြန်မှု ပေါ်တယ်တွင် ပျံ့နှံ့သွားသည်အထိ မိတ္တူနှစ်ခုလုံးကို ချိန်ညှိထားပါ။
:::

# Elastic Lane Provisioning Toolkit (NX-7)

> ** လမ်းပြမြေပုံ အကြောင်းအရာ-** NX-7 — Elastic lane provisioning tooling  
> **အခြေအနေ-** Tooling ပြီးပါပြီ — မန်နီးဖက်စ်များ၊ ကတ်တလောက်အတိုအထွာများ၊ Norito ပေးချေမှုများ၊ မီးခိုးစမ်းသပ်မှုများ၊
> နှင့် load-test bundle helper သည် ယခုအခါ slot latency gating ကိုချုပ်လိုက်သည် + သက်သေပြသည် ထို့ကြောင့် validator
> load runs များကို စိတ်ကြိုက် scripting မပါဘဲ ထုတ်ဝေနိုင်ပါသည်။

ဤလမ်းညွှန်ချက်သည် အော်ပရေတာများကို အလိုအလျောက်လုပ်ဆောင်ပေးသည့် `scripts/nexus_lane_bootstrap.sh` အကူအညီပေးသည့်အသစ်ဖြင့် လမ်းလျှောက်ပေးသည်
လမ်းကြောဆိုင်ရာ ထင်ရှားသော မျိုးဆက်၊ လမ်းကြော/ဒေတာနေရာလွတ် ကတ်တလောက် အတိုအထွာများနှင့် သက်သေအထောက်အထားများ ထုတ်ပေးခြင်း။ ရည်ရွယ်ချက်ကတော့ ဖြစ်အောင်ပေါ့။
ဖိုင်များစွာကို လက်ဖြင့်တည်းဖြတ်ခြင်းမပြုဘဲ Nexus လမ်းကြောအသစ်များ (အများပြည်သူ သို့မဟုတ် သီးသန့်) လှည့်ရန် လွယ်ကူသည်
ကက်တလောက်ဂျီသြမေတြီကို လက်ဖြင့် ပြန်လည်ရယူသည်။

## 1. ကြိုတင်လိုအပ်ချက်များ

1. လမ်းကြောနံပတ်၊ ဒေတာနေရာလွတ်၊ တရားဝင်သတ်မှတ်မှု၊ အမှားခံနိုင်မှု (`f`) နှင့် ဖြေရှင်းရေးမူဝါဒအတွက် အုပ်ချုပ်မှုခွင့်ပြုချက်။
2. အပြီးသတ်အတည်ပြုချက်စာရင်း (အကောင့် IDs) နှင့် ကာကွယ်ထားသော namespace စာရင်း။
3. ထုတ်လုပ်ထားသော အတိုအထွာများကို ထပ်ဖြည့်နိုင်စေရန် node configuration repository သို့ ဝင်ရောက်ပါ။
4. Lane manifest registry အတွက် လမ်းကြောင်းများ (`nexus.registry.manifest_directory` ကိုကြည့်ပါ နှင့်
   `cache_directory`)။
5. Telemetry contacts/PagerDuty သည် လမ်းကြောအတွက် လက်ကိုင်ပါရှိသောကြောင့် လမ်းသွားသည်နှင့်တပြိုင်နက် သတိပေးချက်များကို ကြိုးတပ်နိုင်သည်
   အွန်လိုင်းလာပါသည်။

## 2. လမ်းသွားပစ္စည်းများကို ဖန်တီးပါ။

repository root မှ helper ကို run ပါ။

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

အဓိက အလံများ-

- `--lane-id` သည် `nexus.lane_catalog` ရှိ အသစ်ဝင်ရောက်မှု၏ အညွှန်းကိန်းနှင့် ကိုက်ညီရပါမည်။
- `--dataspace-alias` နှင့် `--dataspace-id/hash` သည် dataspace catalog entry ကို ထိန်းချုပ်သည် (မူရင်းတွင်၊
  ချန်လှပ်ထားသည့်အခါ လမ်းသွားအမှတ်အသား)။
- `--validator` ကို ထပ်ခါတလဲလဲ သို့မဟုတ် `--validators-file` မှ အရင်းအမြစ်ယူနိုင်သည်။
- `--route-instruction` / `--route-account` သည် အဆင်သင့်-ကူးထည့်ရန် လမ်းကြောင်းသတ်မှတ်ခြင်းစည်းမျဉ်းများကို ထုတ်လွှတ်သည်။
- `--metadata key=value` (သို့မဟုတ် `--telemetry-contact/channel/runbook`) runbook အဆက်အသွယ်များကိုဖမ်းယူပါ၊
  ဒက်ရှ်ဘုတ်များသည် မှန်ကန်သောပိုင်ရှင်များကို ချက်ချင်းစာရင်းပြုစုပါ။
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` မန်နီးဖက်စ်တွင် runtime-upgrade ချိတ်ကို ထည့်ပါ။
  လမ်းကြောတွင် တိုးချဲ့အော်ပရေတာ ထိန်းချုပ်မှုများ လိုအပ်သည့်အခါ။
- `--encode-space-directory` သည် `cargo xtask space-directory encode` ကို အလိုအလျောက် ခေါ်ဆိုသည်။ ၎င်းကိုတွဲပါ။
  သင်ကုဒ်လုပ်ထားသော `.to` ဖိုင်ကို ပုံသေမဟုတ်သည့် အခြားနေရာတွင် လိုချင်သောအခါ `--space-directory-out`။

ဇာတ်ညွှန်းသည် `--output-dir` အတွင်းရှိ အနုပညာပစ္စည်းများ (၃) ခုကို ထုတ်လုပ်ပေးသည် (လက်ရှိ လမ်းညွှန်တွင် ပုံသေများ)၊
ကုဒ်သွင်းခြင်းကို ဖွင့်ထားသည့်အခါ ရွေးချယ်နိုင်သော စတုတ္ထမြောက်

1. `<slug>.manifest.json` — မှန်ကန်သော အထမြောက်သော၊ ကာကွယ်ထားသော အမည်နေရာများနှင့် ပါဝင်သော လမ်းကြောများ
   ရွေးချယ်နိုင်သော runtime-upgrade hook metadata။
2. `<slug>.catalog.toml` — `[[nexus.lane_catalog]]`၊ `[[nexus.dataspace_catalog]]` ပါသော TOML အတိုအထွာတစ်ခု၊
   နှင့် တောင်းဆိုထားသော လမ်းကြောင်းစည်းမျဉ်းများ။ `fault_tolerance` ကို dataspace ထည့်သွင်းမှုတွင် အရွယ်အစားအဖြစ် သတ်မှတ်ထားကြောင်း သေချာပါစေ။
   လမ်းကြောဖြတ်ကျော်ရေးကော်မတီ (`3f+1`)။
3. `<slug>.summary.json` — ဂျီသြမေတြီ (ပက်ကျိကျိ၊ အပိုင်းများ၊ မက်တာဒေတာ) အပေါင်းကို ဖော်ပြသည့် စာရင်းစစ်အကျဉ်းချုပ်
   လိုအပ်သော လုပ်ဆောင်ချက်အဆင့်များနှင့် `cargo xtask space-directory encode` အတိအကျ (အောက်
   `space_directory_encode.command`)။ အထောက်အထားအတွက် ဤ JSON ကို စတင်ပြေးဆွဲသည့် လက်မှတ်တွင် ပူးတွဲပါ ။
4. `<slug>.manifest.to` — `--encode-space-directory` ကို သတ်မှတ်သောအခါ ထုတ်လွှတ်သည်။ Torii's အတွက် အဆင်သင့်ဖြစ်နေပါပြီ။
   `iroha app space-directory manifest publish` စီးဆင်းမှု။

ဖိုင်များရေးသားခြင်းမပြုဘဲ JSON/အတိုအထွာများကို အစမ်းကြည့်ရှုရန် `--dry-run` ကိုသုံး၍ ထပ်ရေးရန် `--force`
ရှိပြီးသားပစ္စည်းများ။

## 3. ပြောင်းလဲမှုများကို အသုံးချပါ။

1. Manifest JSON ကို configure လုပ်ထားသော `nexus.registry.manifest_directory` (နှင့် cache ထဲသို့ ကူးယူပါ။
   registry သည် remote bundle များကိုကြည့်လျှင် directory ဖြစ်သည်)။ မန်နီးဖက်စ်များကို ဗားရှင်းဖြင့် ထည့်သွင်းပါက ဖိုင်ကို ထည့်သွင်းပါ။
   သင်၏ configuration repo ။
2. ကတ်တလောက်အတိုအထွာကို `config/config.toml` (သို့မဟုတ် သင့်လျော်သော `config.d/*.toml`) တွင် ပေါင်းထည့်ပါ။ သေချာပါတယ်။
   `nexus.lane_count` သည် အနည်းဆုံး `lane_id + 1` ဖြစ်ပြီး ၎င်း `nexus.routing_policy.rules` ကို အပ်ဒိတ်လုပ်ပါ။
   လမ်းသစ်ကို ညွှန်သင့်တယ်။
3. ကုဒ်နံပါတ် (`--encode-space-directory`) ကို ကျော်သွားပါက မန်နီးဖက်စ်ကို Space Directory သို့ ထုတ်ဝေပါ
   အနှစ်ချုပ် (`space_directory_encode.command`) တွင်ဖမ်းယူထားသော command ကိုအသုံးပြုခြင်း။ ၎င်းသည်ထုတ်လုပ်သည်။
   `.manifest.to` payload Torii သည် စာရင်းစစ်များအတွက် အထောက်အထားများကို မျှော်လင့်ပြီး မှတ်တမ်းတင်ပါသည်။ မှတဆင့်တင်ပြပါ။
   `iroha app space-directory manifest publish`။
4. `irohad --sora --config path/to/config.toml --trace-config` ကို run ပြီး trace output ကို သိမ်းဆည်းပါ။
   ဖြန့်ချိရေးလက်မှတ်။ ၎င်းသည် ဂျီသြမေတြီအသစ်သည် ထုတ်လုပ်ထားသော ပက်ကျိ/kura အပိုင်းများနှင့် ကိုက်ညီကြောင်း သက်သေပြသည်။
5. မန်နီးဖက်စ်/ကတ်တလောက် အပြောင်းအလဲများကို အသုံးချပြီးသည်နှင့် လမ်းသွယ်တွင် သတ်မှတ်ပေးထားသည့် validators ကို ပြန်လည်စတင်ပါ။ စောင့်ရှောက်ပါ။
   အနာဂတ်စာရင်းစစ်များအတွက် လက်မှတ်ရှိ JSON အကျဉ်းချုပ်။

## 4. registry distribution bundle တစ်ခုကို တည်ဆောက်ပါ။

အော်ပရေတာများသည် လမ်းကြောဆိုင်ရာ အုပ်ချုပ်မှုဒေတာကို မလိုအပ်ဘဲ ဖြန့်ဝေနိုင်စေရန် ထုတ်လုပ်ထားသော မန်နီးဖက်စ်နှင့် ထပ်ဆင့်လွှာကို ထုပ်ပိုးပါ။
host တိုင်းတွင် configs တည်းဖြတ်ခြင်း။ အစုအဝေးအကူအညီပေးသူက မိတ္တူများကို canonical layout တွင်ထင်ရှားစေသည်၊
`nexus.registry.cache_directory` အတွက် စိတ်ကြိုက်ရွေးချယ်နိုင်သော အုပ်ချုပ်မှုကတ်တလောက်ထပ်တင်မှုကို ထုတ်လုပ်ပြီး တစ်ခုထုတ်လွှတ်နိုင်သည်
အော့ဖ်လိုင်းလွှဲပြောင်းမှုများအတွက် tarball

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

အထွက်များ-

1. `manifests/<slug>.manifest.json` — အဲဒါတွေကို configure လုပ်ပြီး ကော်ပီကူးပါ။
   `nexus.registry.manifest_directory`။
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory` သို့ဆင်းပါ။ `--module` တိုင်း
   entry သည် governance-module swap-outs (NX-2) ကို enable လုပ်နိုင်သော pluggable module အဓိပ္ပါယ်တစ်ခုဖြစ်လာသည်
   `config.toml` ကို တည်းဖြတ်ခြင်းအစား ကက်ရှ်အလွှာကို အပ်ဒိတ်လုပ်ခြင်း။
3. `summary.json` — ဟက်ရှ်များ၊ ထပ်ဆင့် မက်တာဒေတာနှင့် အော်ပရေတာ ညွှန်ကြားချက်များ ပါဝင်သည်။
4. ရွေးချယ်နိုင်သော `registry_bundle.tar.*` — SCP, S3, သို့မဟုတ် artifact trackers အတွက် အဆင်သင့်။

လမ်းကြောင်းတစ်ခုလုံး (သို့မဟုတ် မော်ကွန်းတိုက်) တစ်ခုလုံးကို အတည်ပြုပေးသူတစ်ခုစီသို့ စင့်ခ်လုပ်ပါ၊ လေဝင်လေထွက်ရှိသော တန်ဆာပလာများကို ထုတ်ယူပြီး မိတ္တူကူးပါ။
Torii ကို ပြန်လည်မစတင်မီ ၎င်းတို့၏ registry လမ်းကြောင်းများထဲသို့ manifests + cache ထပ်တင်ခြင်း။

## 5. Validator မီးခိုးစမ်းသပ်မှုများ

Torii ပြန်လည်စတင်ပြီးနောက်၊ လမ်းကြောဆိုင်ရာအစီရင်ခံစာများ `manifest_ready=true` ကိုစစ်ဆေးရန် မီးခိုးအကူအသစ်ကို ဖွင့်ပါ။
မက်ထရစ်များသည် မျှော်လင့်ထားသည့် လမ်းသွားအရေအတွက်ကို ဖော်ထုတ်ပေးပြီး အလုံပိတ် တိုင်းထွာသည် ရှင်းပါသည်။ ထင်ရှားသောလမ်းများ
အချည်းနှီးမဟုတ်သော `manifest_path` ကို ဖော်ထုတ်ရပါမည်။ လမ်းကြောင်းပျောက်နေတဲ့အခါ ကူညီပေးသူဟာ အခုချက်ချင်း ပျက်သွားတယ်။
NX-7 ဖြန့်ကျက်မှုမှတ်တမ်းတိုင်းတွင် လက်မှတ်ရေးထိုးထားသော ထင်ရှားသောအထောက်အထားများ ပါဝင်သည်-

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

ကိုယ်တိုင်ရေးထိုးထားသော ပတ်ဝန်းကျင်များကို စမ်းသပ်သောအခါ `--insecure` ကို ထည့်ပါ။ လမ်းသွားဖြစ်ပါက script သည် သုညမဟုတ်သော ထွက်ပါသည်။
ပျောက်ဆုံးနေသော၊ အလုံပိတ် သို့မဟုတ် မက်ထရစ်များ/တယ်လီမီတာများ မျှော်မှန်းထားသော တန်ဖိုးများမှ ပျံ့လွင့်နေသည်။ ကိုသုံးပါ။
`--min-block-height`၊ `--max-finality-lag`၊ `--max-settlement-backlog` နှင့်
`--max-headroom-events` ခလုတ်များ
သင်၏လုပ်ငန်းလည်ပတ်မှုစာအိတ်များအတွင်း၊ `--max-slot-p95` / `--max-slot-p99` ဖြင့် ၎င်းတို့ကို ပူးတွဲပါ
အကူအညီပေးသူကိုမထွက်ခွာဘဲ NX-18 အပေါက်-ကြာချိန်ပစ်မှတ်များကို တွန်းအားပေးရန် (+ `--min-slot-samples`)။

လေအကွာအဝေးအတည်ပြုချက်များ (သို့မဟုတ် CI) အတွက် တိုက်ရိုက်ထုတ်လွှင့်မှုကို ရိုက်မည့်အစား ဖမ်းယူထားသော Torii တုံ့ပြန်မှုကို ပြန်ဖွင့်နိုင်သည်
အဆုံးမှတ်-

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` အောက်တွင် မှတ်တမ်းတင်ထားသော ပစ္စည်းများသည် bootstrap မှထုတ်လုပ်ထားသော ပစ္စည်းများကို ထင်ဟပ်စေသည်
စိတ်ကြိုက်ဇာတ်ညွှန်းရေးခြင်းမရှိဘဲ သရုပ်ဖော်ပုံအသစ်များကို စီစဥ်ထားနိုင်စေရန် အကူအညီပေးသည်။ CI လေ့ကျင့်ခန်းမှတဆင့် တူညီသောစီးဆင်းမှု
`ci/check_nexus_lane_smoke.sh` နှင့် `ci/check_nexus_lane_registry_bundle.sh`
NX-7 မီးခိုးအကူသည် ထုတ်ဝေထားသောစာနှင့် လိုက်လျောညီထွေရှိနေကြောင်း သက်သေပြရန် (အမည်များ- `make check-nexus-lanes`)
payload format နှင့် bundle digests/overlays များသည် မျိုးပွားနိုင်စေကြောင်း သေချာစေရန်။

လမ်းကြောတစ်ခုအား အမည်ပြောင်းသည့်အခါ `nexus.lane.topology` တယ်လီမီတာ ဖြစ်ရပ်များကို ဖမ်းယူပါ (ဥပမာအားဖြင့်၊
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) ထဲသို့ ပြန်ဝင်ပါ။
ဆေးလိပ်သောက်သူ။ `--telemetry-file/--from-telemetry` အလံသည် မျဉ်းအသစ်-ပိုင်းခြားထားသော မှတ်တမ်းကို လက်ခံသည်။
`--require-alias-migration old:new` က `alias_migrated` ဖြစ်ရပ်သည် အမည်ပြောင်းခြင်းကို မှတ်တမ်းတင်ထားသည်-

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` fixture သည် canonical rename sample အစုအဝေးကို CI စစ်ဆေးနိုင်စေရန်
တိုက်ရိုက် node ကို မဆက်သွယ်ဘဲ telemetry ပိုင်းခြားသည့်လမ်းကြောင်း။

## Validator load tests (NX-7 အထောက်အထားများ)

လမ်းပြမြေပုံ **NX-7** သည် ပြန်လည်ထုတ်လုပ်နိုင်သော validator load run ကို ပို့ဆောင်ရန် လမ်းအသစ်တိုင်း လိုအပ်သည်။ သုံးပါ။
မီးခိုးစစ်ဆေးမှုများ၊ အပေါက်-ကြာချိန်တံခါးများနှင့် အပေါက်အတွဲများကို ချုပ်ရန် `scripts/nexus_lane_load_test.py`
အုပ်ချုပ်ရေးကို ပြန်လည်ပြသနိုင်သည့် တစ်ခုတည်းသော ပစ္စည်းအစုံတွင် ဖော်ပြပါ-

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

ကူညီသူသည် တူညီသော DA အထွတ်အထိပ်၊ oracle၊ အခြေချနေထိုင်မှုကြားခံ၊ TEU နှင့် အသုံးပြုထားသော အပေါက်-ကြာချိန်ဂိတ်များကို တွန်းအားပေးသည်
မီးခိုးအကူပေးသူက `smoke.log`၊ `slot_summary.json`၊ slot bundle manifest ကိုရေးပြီး၊
ရွေးချယ်ထားသော `--out-dir` ထဲသို့ `load_test_manifest.json` ဖြစ်သောကြောင့် load run များကို တိုက်ရိုက်တွဲနိုင်သည်
စိတ်ကြိုက် ဇာတ်ညွှန်းရေးခြင်းမရှိဘဲ လက်မှတ်များထုတ်ပေးခြင်း။

## 6. Telemetry & governance နောက်ဆက်တွဲများ

- လမ်းသွားဒိုင်ခွက်များ (`dashboards/grafana/nexus_lanes.json` နှင့် ဆက်စပ်သော ထပ်ဆင့်များ) ကို အပ်ဒိတ်လုပ်ပါ။
  လမ်းသွားအိုင်ဒီနှင့် မက်တာဒေတာအသစ်။ ထုတ်လုပ်လိုက်သော မက်တာဒေတာသော့များ (`contact`၊ `channel`၊ `runbook` စသည်ဖြင့်) ပြုလုပ်သည်
  တံဆိပ်များကို ကြိုတင်ဖြည့်ရန် ရိုးရှင်းပါသည်။
- ဝင်ခွင့်မဖွင့်မီ လမ်းသွယ်အသစ်အတွက် Wire PagerDuty/Alertmanager စည်းမျဉ်းများ။ `summary.json`
  နောက်အဆင့်များ array သည် [Nexus operations](./nexus-operations) ရှိ checklist ကို ထင်ဟပ်စေသည်။
- တရားဝင်သတ်မှတ်သူသည် တိုက်ရိုက်ထုတ်လွှင့်သည်နှင့်တစ်ပြိုင်နက် Space Directory တွင် manifest အစုအဝေးကို မှတ်ပုံတင်ပါ။ အတူတူသုံးပါ။
  အကူအညီပေးသူက ထုတ်ပေးသော JSON ကို ထင်ရှားစွာပြပြီး အုပ်ချုပ်မှုဆိုင်ရာစာအုပ်တွင် ရေးထိုးထားသည်။
- မီးခိုးစမ်းသပ်မှုများ (FindNetworkStatus၊ Torii အတွက် [Sora Nexus အော်ပရေတာ စတင်ခြင်း](./nexus-operator-onboarding) ကို လိုက်နာပါ
  လက်လှမ်းမှီနိုင်မှု) နှင့် အထက်ဖော်ပြပါ ပစ္စည်းအစုံအလင်ဖြင့် အထောက်အထားများကို ဖမ်းယူပါ။

## 7. Dry-run ဥပမာ

ဖိုင်များမရေးဘဲ ရှေးဟောင်းပစ္စည်းများကို အစမ်းကြည့်ရှုရန်-

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --dry-run
```

ညွှန်ကြားချက်သည် JSON အနှစ်ချုပ်နှင့် TOML အတိုအထွာကို stdout တွင် print ထုတ်ပြီး ကာလအတွင်း အမြန်ပြန်ဆိုခြင်းကို ခွင့်ပြုသည်
စီစဉ်ခြင်း။

---

နောက်ထပ်အကြောင်းအရာအတွက် ကြည့်ပါ-- [Nexus operations](./nexus-operations) — လုပ်ငန်းလည်ပတ်မှု စစ်ဆေးစာရင်းနှင့် တယ်လီမီတာ လိုအပ်ချက်များ။
- [Sora Nexus အော်ပရေတာ စတင်အသုံးပြုခြင်း](./nexus-operator-onboarding) — အသေးစိတ်အချက်အလက်များကို ကိုးကားသော စတင်ဝင်ရောက်ခြင်းအစီအစဥ်
  အကူအညီအသစ်။
- [Nexus လမ်းသွားမော်ဒယ်](./nexus-lane-model) — ကိရိယာမှအသုံးပြုသော လမ်းသွားဂျီသြမေတြီ၊ ပက်ကျိများနှင့် သိုလှောင်မှုပုံစံ။