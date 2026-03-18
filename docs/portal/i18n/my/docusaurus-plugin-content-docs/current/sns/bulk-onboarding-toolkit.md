---
lang: my
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id- အစုလိုက်စတင်အသုံးပြုခြင်း-ကိရိယာအစုံ
ခေါင်းစဉ်- SNS Bulk Onboarding Toolkit
sidebar_label- အစုလိုက် စတင်အသုံးပြုသည့် ကိရိယာအစုံ
ဖော်ပြချက်- SN-3b မှတ်ပုံတင်သူ လုပ်ဆောင်ခြင်းအတွက် မှတ်ပုံတင်ရန်NameRequestV1 အလိုအလျောက်လုပ်ဆောင်ခြင်း CSV။
---

::: Canonical Source ကို သတိပြုပါ။
Mirrors `docs/source/sns/bulk_onboarding_toolkit.md` ဖြစ်တာကြောင့် ပြင်ပအော်ပရေတာတွေမှာ တွေ့ရမှာပါ။
repository ကို clone မလုပ်ဘဲ တူညီသော SN-3b လမ်းညွှန်ချက်။
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

** လမ်းပြမြေပုံ ရည်ညွှန်းချက်-** SN-3b "အစုလိုက် စတင်အသုံးပြုခြင်း ကိရိယာတန်ဆာပလာ"  
** ပစ္စည်းများ-** `scripts/sns_bulk_onboard.py`၊ `scripts/tests/test_sns_bulk_onboard.py`၊
`docs/portal/scripts/sns_bulk_release.sh`

မှတ်ပုံတင်အရာရှိကြီးများသည် `.sora` သို့မဟုတ် `.nexus` ရာနှင့်ချီသော မှတ်ပုံတင်မှုများကို ကြိုတင်လုပ်ဆောင်လေ့ရှိသည်
တူညီသောအုပ်ချုပ်မှုခွင့်ပြုချက်များနှင့်အခြေချရထားလမ်းများနှင့်အတူ။ JSON ကို ကိုယ်တိုင်ဖန်တီးခြင်း။
ဝန်ဆောင်ခများ သို့မဟုတ် CLI ကို ပြန်လည်လည်ပတ်ခြင်းမှာ အတိုင်းအတာမဟုတ်သောကြောင့် SN-3b သည် အဆုံးအဖြတ်ပေးသော သင်္ဘောဖြစ်သည်။
CSV မှ Norito အတွက် `RegisterNameRequestV1` အဆောက်အဦများကို ပြင်ဆင်ပေးသည်
Torii သို့မဟုတ် CLI။ ကူညီသူသည် ရှေ့အတန်းတိုင်းကို သက်သေပြပြီး နှစ်ခုလုံးကို ထုတ်လွှတ်သည်။
စုစည်းထားသော manifest နှင့် optional line-delimited JSON တို့ကို ပေးပို့နိုင်ပါသည်။
စာရင်းစစ်များအတွက် ဖွဲ့စည်းထားသည့် ပြေစာများကို မှတ်တမ်းတင်နေစဉ် payload သည် အလိုအလျောက် တက်လာပါသည်။

## 1. CSV schema

parser သည် အောက်ပါ ခေါင်းစီးအတန်း လိုအပ်သည် (အမှာစာသည် ပြောင်းလွယ်ပြင်လွယ်ဖြစ်သည်)

| ကော်လံ | လိုအပ်သည် | ဖော်ပြချက် |
|--------|----------|-------------|
| `label` | ဟုတ်တယ် | တောင်းဆိုထားသော အညွှန်း (ရောနှောထားသော ဖြစ်ရပ်ကို လက်ခံသည်၊ Norm v1 နှင့် UTS-46 အရ ကိရိယာကို ပုံမှန်ဖြစ်စေသည်)။ |
| `suffix_id` | ဟုတ်တယ် | ကိန်းဂဏာန်း နောက်ဆက်တွဲ အမှတ်အသား (ဒဿမ သို့မဟုတ် `0x` hex)။ |
| `owner` | ဟုတ်တယ် | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | ဟုတ်တယ် | ကိန်းပြည့် `1..=255`။ |
| `payment_asset_id` | ဟုတ်တယ် | ပိုင်ဆိုင်မှု (ဥပမာ `xor#sora`)။ |
| `payment_gross` / `payment_net` | ဟုတ်တယ် | ပိုင်ဆိုင်မှု-ဇာတိ ယူနစ်များကို ကိုယ်စားပြုသည့် လက်မှတ်မထိုးထားသော ကိန်းပြည့်များ။ |
| `settlement_tx` | ဟုတ်တယ် | ငွေပေးချေမှု သို့မဟုတ် hash ကိုဖော်ပြသည့် JSON တန်ဖိုး သို့မဟုတ် ပကတိစာတန်း။ |
| `payment_payer` | ဟုတ်တယ် | ငွေပေးချေမှုကို ခွင့်ပြုထားသည့် အကောင့်အိုင်ဒီ။ |
| `payment_signature` | ဟုတ်တယ် | JSON သို့မဟုတ် ဘဏ္ဍာစိုး သို့မဟုတ် ဘဏ္ဍာတိုက်ဆိုင်ရာ လက်မှတ်အထောက်အထား ပါဝင်သော စာတန်း။ |
| `controllers` | ရွေးချယ်ခွင့် | Semicolon- သို့မဟုတ် ကော်မာ-ခြားထားသော ထိန်းချုပ်သူအကောင့်လိပ်စာများစာရင်း။ ချန်လှပ်ထားသောအခါတွင် ပုံသေသည် `[owner]` ဖြစ်သည်။ |
| `metadata` | ရွေးချယ်ခွင့် | Inline JSON သို့မဟုတ် `@path/to/file.json` သည် ဖြေရှင်းပေးသည့် အရိပ်အမြွက်များ၊ TXT မှတ်တမ်းများ စသည်တို့ကို ပံ့ပိုးပေးပါသည်။ ပုံသေများသည် `{}` သို့ဖြစ်သည်။ |
| `governance` | ရွေးချယ်ခွင့် | Inline JSON သို့မဟုတ် `@path` သည် `GovernanceHookV1` ကိုညွှန်ပြသည်။ `--require-governance` သည် ဤကော်လံကို ပြဋ္ဌာန်းသည်။ |

မည်သည့်ကော်လံမဆို ဆဲလ်တန်ဖိုး `@` ဖြင့် ပြင်ပဖိုင်ကို ရည်ညွှန်းနိုင်သည်။
လမ်းကြောင်းများကို CSV ဖိုင်နှင့် ဆက်စပ်ဖြေရှင်းထားသည်။

## 2. အကူအညီပေးသူကို လုပ်ဆောင်ခြင်း။

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

အဓိကရွေးချယ်စရာများ-

- `--require-governance` သည် အုပ်ချုပ်မှုအချိတ်အဆက်မရှိဘဲ အတန်းများကို ငြင်းပယ်သည် (အတွက်အသုံးဝင်သည်
  ပရီမီယံလေလံပွဲများ သို့မဟုတ် သီးသန့်တာဝန်များ)။
- `--default-controllers {owner,none}` သည် controller ဆဲလ်အလွတ်ရှိမရှိ ဆုံးဖြတ်သည်။
  ပိုင်ရှင်အကောင့်သို့ ပြန်ရောက်သွားပါသည်။
- `--controllers-column`၊ `--metadata-column` နှင့် `--governance-column` အမည်ပြောင်း
  အထက်စီးကြောင်း တင်ပို့မှုများနှင့် အလုပ်လုပ်သောအခါ ရွေးချယ်နိုင်သော ကော်လံများ။

အောင်မြင်မှုတွင် ဇာတ်ညွှန်းသည် စုစည်းဖော်ပြချက်တစ်ခုကို ရေးသားသည်-

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

`--ndjson` ကို ပေးထားပါက၊ `RegisterNameRequestV1` တစ်ခုစီကိုလည်း ရေးထားသည်
တစ်ကြောင်းတည်းသော JSON စာရွက်စာတမ်းသည် အလိုအလျောက်စနစ်ဖြင့် တောင်းဆိုမှုများကို တိုက်ရိုက်ထုတ်လွှင့်နိုင်သည်။
Torii-

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. အလိုအလျောက်တင်ပြမှုများ

### 3.1 Torii REST မုဒ်

`--submit-torii-url` နှင့် `--submit-token` သို့မဟုတ်
`--submit-token-file` သည် manifest entry တိုင်းကို Torii သို့ တိုက်ရိုက်တွန်းရန်-

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- အကူအညီပေးသူက တောင်းဆိုချက်တစ်ခုလျှင် `POST /v1/sns/registrations` ကိုထုတ်ပေးပြီး ဖျက်ပစ်လိုက်သည်။
  ပထမဆုံး HTTP အမှား။ တုံ့ပြန်မှုများကို NDJSON အဖြစ် မှတ်တမ်းလမ်းကြောင်းတွင် ထည့်သွင်းထားသည်။
  မှတ်တမ်းများ
- `--poll-status` တစ်ခုစီပြီးနောက် `/v1/sns/registrations/{selector}` ပြန်မေးသည်
  မှတ်တမ်းကို အတည်ပြုရန် တင်ပြချက် (`--poll-attempts` အထိ၊ မူရင်း 5)၊
  မြင်နိုင်သည်။ `--suffix-map` (`suffix_id` ၏ JSON မှ `"suffix"` တန်ဖိုးများ) သို့ ပေးပါ။
  ကိရိယာသည် မဲရုံအတွက် `{label}.{suffix}` စာလုံးများကို ထုတ်ယူနိုင်သည်။
- အသံချဲ့စက်များ- `--submit-timeout`၊ `--poll-attempts` နှင့် `--poll-interval`။

### 3.2 iroha CLI မုဒ်

CLI မှတဆင့် manifest entry တစ်ခုစီကို လမ်းကြောင်းပြရန် binary path ကို ပံ့ပိုးပါ-

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- ထိန်းချုပ်ကိရိယာများသည် `Account` ထည့်သွင်းမှုများ (`controller_type.kind = "Account"`) ဖြစ်ရမည်။
  အဘယ်ကြောင့်ဆိုသော် CLI သည် လက်ရှိတွင် အကောင့်အခြေပြု ထိန်းချုပ်ကိရိယာများကိုသာ ထုတ်ဖော်ပြသထားသောကြောင့်ဖြစ်သည်။
- တောင်းဆိုချက်တစ်ခုအတွက် မက်တာဒေတာနှင့် အုပ်ချုပ်မှု blobs များကို ယာယီဖိုင်များသို့ စာရေးပြီးဖြစ်သည်။
  `iroha sns register --metadata-json ... --governance-json ...` သို့ ထပ်ဆင့်ပို့သည်။
- CLI stdout နှင့် stderr အပေါင်း ထွက်ပေါက်ကုဒ်များကို မှတ်တမ်းတင်ထားသည်။ သုညမဟုတ်သော ထွက်ပေါက်ကုဒ်များကို ဖျက်သိမ်းလိုက်ပါ။
  ပြေးသည်။

တင်သွင်းမှုမုဒ်နှစ်ခုစလုံးသည် (Torii နှင့် CLI) အပြန်အလှန်စစ်ဆေးသည့် မှတ်ပုံတင်အရာရှိထံသို့ အတူတကွ လုပ်ဆောင်နိုင်သည်
ဖြန့်ကျက်ခြင်း သို့မဟုတ် ဆုတ်ယုတ်မှုများကို အစမ်းလေ့ကျင့်ပါ။

### 3.3 တင်ပြမှုပြေစာများ

`--submission-log <path>` ကို ပေးသောအခါ၊ script သည် NDJSON entries များကို ထည့်သွင်းသည်
ရိုက်ကူးခြင်း-

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

အောင်မြင်သော Torii မှ ထုတ်နုတ်ထားသော ဖွဲ့စည်းပုံအကွက်များ ပါဝင်ပါသည်။
`NameRecordV1` သို့မဟုတ် `RegisterNameResponseV1` (ဥပမာ `record_status`၊
`record_pricing_class`, `record_owner`, `record_expires_at_ms`၊
`registry_event_version`၊ `suffix_id`၊ `label`) ထို့ကြောင့် ဒက်ရှ်ဘုတ်များနှင့် အုပ်ချုပ်မှု
အစီရင်ခံစာများသည် အခမဲ့ပုံစံ စာသားကို မစစ်ဆေးဘဲ မှတ်တမ်းကို ပိုင်းခြားနိုင်သည်။ ဤမှတ်တမ်းကို ပူးတွဲပါ။
ပြန်လည်ထုတ်လုပ်နိုင်သော အထောက်အထားများအတွက် ဖော်ပြချက်နှင့်အတူ မှတ်ပုံတင်အရာရှိလက်မှတ်များ။

## 4. Docs portal သည် အလိုအလျောက်စနစ်ဖြင့် ထုတ်လွှတ်သည်။

CI နှင့် portal jobs များဖြစ်သည့် `docs/portal/scripts/sns_bulk_release.sh` များဖြစ်ပါတယ်။
အကူအညီပေးသူက `artifacts/sns/releases/<timestamp>/` အောက်တွင် ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းသည်-

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

ဇာတ်ညွှန်း-

1. `registrations.manifest.json`၊ `registrations.ndjson` ကိုတည်ဆောက်ပြီး မိတ္တူကူးပါ။
   မူရင်း CSV ထုတ်ဝေမှုလမ်းညွှန်သို့။
2. Torii နှင့်/သို့မဟုတ် CLI (ပြင်ဆင်သတ်မှတ်ထားသောအခါ) ကိုအသုံးပြု၍ မန်နီးဖက်စ်ကို တင်သွင်းပါ
   အထက်ဖော်ပြပါ ပြေစာများနှင့်အတူ `submissions.log`။
3. ထုတ်ဝေမှုကို ဖော်ပြသည့် `summary.json` ကို ထုတ်လွှတ်သည် (လမ်းကြောင်းများ၊ Torii URL၊ CLI လမ်းကြောင်း၊
   timestamp) ထို့ကြောင့် portal automation သည် bundle ကို artefact storage သို့ အပ်လုဒ်လုပ်နိုင်ပါသည်။
4. ပါဝင်သော `metrics.prom` (`--metrics` မှတဆင့် override) ကို ထုတ်လုပ်သည်
   စုစုပေါင်းတောင်းဆိုမှုများ၊ နောက်ဆက်တွဲဖြန့်ဖြူးမှုအတွက် Prometheus-ဖော်မတ်ကောင်တာများ၊
   ပိုင်ဆိုင်မှုစုစုပေါင်းနှင့် တင်ပြမှုရလဒ်များ။ အကျဉ်းချုပ် JSON သည် ဤဖိုင်သို့ လင့်ခ်ချိတ်သည်။

Workflows သည် ယခုထွက်ရှိထားသော directory ကို တစ်ခုတည်းသော artefact အဖြစ် သိမ်းဆည်းထားသည်။
စာရင်းစစ်ခြင်းအတွက် အုပ်ချုပ်မှုလိုအပ်ချက်အားလုံးပါရှိသည်။

## 5. တယ်လီမီတာနှင့် ဒက်ရှ်ဘုတ်များ

`sns_bulk_release.sh` မှ ထုတ်ပေးသော မက်ထရစ်ဖိုင်သည် အောက်ပါတို့ကို ဖော်ပြသည်။
စီးရီး-

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

သင်၏ Prometheus ဆိုက်ကားထဲသို့ `metrics.prom` ကို ကျွေးပါ (ဥပမာ Promtail သို့မဟုတ် မှတဆင့်
အစုလိုက်တင်သွင်းသူ) မှတ်ပုံတင်အရာရှိများ၊ ဘဏ္ဍာစိုးများနှင့် အုပ်ချုပ်မှုလုပ်ဖော်ကိုင်ဖက်များကို ညီညွတ်အောင်ထားရန်၊
အစုလိုက်တိုးတက်မှု။ Grafana ဘုတ်
`dashboards/grafana/sns_bulk_release.json` သည် တူညီသောဒေတာကို အကန့်များဖြင့် မြင်ယောင်သည်။
နောက်ဆက်တွဲအရေအတွက်၊ ငွေပေးချေမှုပမာဏနှင့် တင်ပြမှုအောင်မြင်/မအောင်မြင်မှုအချိုးများအတွက်။
ဘုတ်အဖွဲ့သည် `release` ဖြင့် စစ်ထုတ်သောကြောင့် စာရင်းစစ်များသည် CSV တစ်ခုတည်းကို လည်ပတ်နိုင်မည်ဖြစ်သည်။

## 6. အတည်ပြုခြင်းနှင့် ပျက်ကွက်ခြင်းမုဒ်များ

- **Label canonicalization-** input များကို Python IDNA plus ဖြင့် ပုံမှန်ပြုလုပ်ထားပါသည်။
  စာလုံးအသေးနှင့် Norm v1 အက္ခရာ စစ်ထုတ်မှုများ။ မမှန်ကန်သော အညွှန်းများသည် မည်သည့်အရာမဆို လျင်မြန်စွာ ပျက်ကွက်ပါသည်။
  ကွန်ရက်ခေါ်ဆိုမှုများ။
- **ဂဏန်းအကာအရံများ-** နောက်ဆက်တွဲ အိုင်ဒီများ၊ သက်တမ်းနှစ်များနှင့် ဈေးနှုန်း အရိပ်အမြွက်များ ကျသင့်သည်
  `u16` နှင့် `u8` ဘောင်အတွင်း။ ငွေပေးချေမှုအကွက်များသည် ဒဿမ သို့မဟုတ် ဆဋ္ဌမကိန်းလုံးကို လက်ခံသည်။
  `i64::MAX` အထိ။
- ** မက်တာဒေတာ သို့မဟုတ် အုပ်ချုပ်မှုပိုင်းခြားစိတ်ဖြာခြင်း-** လိုင်း JSON ကို တိုက်ရိုက်ခွဲခြမ်းစိတ်ဖြာသည်။ ဖိုင်
  ကိုးကားချက်များကို CSV တည်နေရာနှင့် ဆက်စပ်ဖြေရှင်းထားသည်။ အရာဝတ္ထုမဟုတ်သော မက်တာဒေတာ
  အတည်ပြုချက်အမှားကို ထုတ်ပေးသည်။
- **ထိန်းချုပ်သူများ-** ဆဲလ်အလွတ်များသည် `--default-controllers` ကို ဂုဏ်ပြုပါသည်။ ပြတ်သားစွာဖော်ပြပါ။
  ပိုင်ရှင်မဟုတ်သူများကို လွှဲအပ်သည့်အခါ ထိန်းချုပ်သူစာရင်းများ (ဥပမာ `i105...;i105...`)
  သရုပ်ဆောင်များ။

ပျက်ကွက်မှုများကို အကြောင်းအရာအလိုက် အတန်းနံပါတ်များဖြင့် အစီရင်ခံသည် (ဥပမာ
`error: row 12 term_years must be between 1 and 255`)။ ဇာတ်ညွှန်းက ထွက်ပါတယ်။
CSV လမ်းကြောင်းပျောက်နေချိန်တွင် `1` သည် တရားဝင်မှုအမှားများနှင့် `2` တွင် ကုဒ်နံပါတ်

## 7. စမ်းသပ်ခြင်းနှင့်သက်သေ

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` သည် CSV ခွဲခြမ်းစိတ်ဖြာခြင်းကို အကျုံးဝင်သည်၊
  NDJSON ထုတ်လွှတ်မှု၊ အုပ်ချုပ်မှုအာဏာနှင့် CLI သို့မဟုတ် Torii တင်သွင်းမှု
  လမ်းကြောင်းများ။
- ကူညီသူသည် Python အစစ်အမှန်ဖြစ်သည် (နောက်ထပ်မှီခိုမှုများမရှိ) နှင့်မည်သည့်နေရာတွင်မဆိုအလုပ်လုပ်သည်။
  `python3` ကို ရရှိနိုင်ပါသည်။ Commit history ကို CLI နှင့်တွဲပြီး ခြေရာခံပါသည်။
  မျိုးပွားနိုင်စွမ်းအတွက် အဓိကသိုလှောင်ရာနေရာ။

ထုတ်လုပ်မှုလုပ်ဆောင်ခြင်းအတွက်၊ ထုတ်လုပ်ထားသော မန်နီးဖက်စ်နှင့် NDJSON အစုအဝေးကို ပူးတွဲပါ။
မှတ်ပုံတင်အရာရှိလက်မှတ်သည် ဘဏ္ဍာစိုးများသည် တင်သွင်းထားသော ပေးဆောင်မှုအတိအကျကို ပြန်လည်ပြသနိုင်စေရန်
Torii သို့။