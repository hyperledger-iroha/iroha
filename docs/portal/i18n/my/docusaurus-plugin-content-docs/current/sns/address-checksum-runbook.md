---
id: address-checksum-runbook
lang: my
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/sns/address_checksum_failure_runbook.md` ဖြစ်သည်။ မွမ်းမံ
အရင်းအမြစ်ဖိုင်ကို ဦးစွာပထမ၊ ထို့နောက် ဤမိတ္တူကို စင့်ခ်လုပ်ပါ။
:::

Checksum ကျရှုံးမှုများသည် `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) အဖြစ် ပေါ်လာသည်
Torii၊ SDK နှင့် wallet/explorer clients ယခု ADDR-6/ADDR-7 လမ်းပြမြေပုံ ပစ္စည်းများ
checksum သတိပေးချက်များ သို့မဟုတ် ပံ့ပိုးမှုပြုလုပ်သည့်အခါတိုင်း အော်ပရေတာများသည် ဤ runbook ကို လိုက်နာရန် လိုအပ်သည်။
လက်မှတ်မီး

## ဘယ်အချိန်မှာ ကစားရမလဲ

- **သတိပေးချက်များ-** `AddressInvalidRatioSlo` (သတ်မှတ်ထားသည်။
  `dashboards/alerts/address_ingest_rules.yml`) ခရီးစဉ်များနှင့် မှတ်ချက်များစာရင်း
  `reason="ERR_CHECKSUM_MISMATCH"`။
- **Fixture drift:** `account_address_fixture_status` Prometheus textfile သို့မဟုတ်
  Grafana ဒက်ရှ်ဘုတ်သည် မည်သည့် SDK မိတ္တူအတွက်မဆို checksum မကိုက်ညီကြောင်း အစီရင်ခံပါသည်။
- ** ထပ်လောင်းပြောဆိုမှုများကို ပံ့ပိုးပေးသည်-** Wallet/explorer/SDK အဖွဲ့များသည် checksum အမှားများကို ကိုးကား၍ IME
  အကျင့်ပျက်ခြစားမှု သို့မဟုတ် ကုဒ်ဖော်ပြခြင်းမပြုတော့သော ကလစ်ဘုတ်စကန်ဖတ်ခြင်းများ။
- **လက်ဖြင့်လေ့လာခြင်း-** Torii မှတ်တမ်းများသည် `address_parse_error=checksum_mismatch` ထပ်ခါတလဲလဲ ပြသည်
  ထုတ်လုပ်မှုအဆုံးမှတ်များအတွက်။

အကယ်၍ အဆိုပါဖြစ်ရပ်သည် Local-8/Local-12 ယာဉ်တိုက်မှုများအကြောင်း အတိအကျပြောပါက၊ လိုက်နာပါ။
`AddressLocal8Resurgence` သို့မဟုတ် `AddressLocal12Collision` အစား စာအုပ်များ။

## သက်သေစာရင်း

| အထောက်အထား | Command / Location | မှတ်စုများ |
|----------|--------------------------------|------|
| Grafana လျှပ်တစ်ပြက်ရိုက်ချက် | `dashboards/grafana/address_ingest.json` | မမှန်ကန်သော အကြောင်းပြချက် ပျက်ယွင်းမှုများနှင့် သက်ရောက်မှုရှိသော အဆုံးမှတ်များကို ရိုက်ကူးပါ။ |
| သတိပေးချက် payload | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | အကြောင်းအရာ အညွှန်းများနှင့် အချိန်တံဆိပ်များ ထည့်သွင်းပါ။ |
| Fixture ကျန်းမာရေး | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK မိတ္တူများသည် `fixtures/account/address_vectors.json` မှ လွင့်သွားခြင်း ရှိမရှိ သက်သေပြပါသည်။ |
| PromQL မေးမြန်းမှု | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | အဖြစ်အပျက်စာရွက်စာတမ်းအတွက် CSV ကို ထုတ်ယူပါ။ |
| မှတ်တမ်းများ | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (သို့မဟုတ် မှတ်တမ်းပေါင်းစည်းခြင်း) | မမျှဝေမီ PII ကို ပွတ်တိုက်ပါ။ |
| Fixture စိစစ်ခြင်း | `cargo xtask address-vectors --verify` | Canonical generator ကို အတည်ပြုပြီး JSON သဘောတူသည်။ |
| SDK တူညီမှုစစ်ဆေးခြင်း | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | သတိပေးချက်များ/လက်မှတ်များတွင် အစီရင်ခံထားသော SDKတိုင်းအတွက် လုပ်ဆောင်ပါ။ |
| ကလစ်ဘုတ်/IME စိတ်ဖောက်ပြန်မှု | `iroha tools address inspect <literal>` | ဝှက်ထားသော ဇာတ်ကောင်များ သို့မဟုတ် IME ပြန်လည်ရေးသားမှုများကို ရှာဖွေတွေ့ရှိသည်။ `address_display_guidelines.md` ကို ကိုးကားပါ။ |

##ချက်​ချင်းတုံ့ပြန်​ခြင်း။

1. သတိပေးချက်ကို အသိအမှတ်ပြုပါ၊ အဖြစ်အပျက်တွင် Grafana လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် PromQL အထွက်ကို လင့်ခ်ချိတ်ပါ။
   thread၊ နှင့် Torii ဆက်စပ်မှုများကို သက်ရောက်မှုရှိကြောင်း သတိပြုပါ။
2. ထင်ရှားသော ပရိုမိုးရှင်းများကို ရပ်ထား/ SDK သည် ထိထိမိမိ ခွဲခြမ်းစိတ်ဖြာသည့် လိပ်စာကို ထုတ်ပြန်သည်။
3. ဒက်ရှ်ဘုတ် လျှပ်တစ်ပြက်ပုံများကို သိမ်းဆည်းပြီး ထုတ်ပေးထားသော Prometheus စာသားဖိုင်လက်ရာများ
   အဖြစ်အပျက်ဖိုင်တွဲ (`docs/source/sns/incidents/YYYY-MM/<ticket>/`)။
4. `checksum_mismatch` ပေးဆောင်မှုများအား ပြသသော မှတ်တမ်းနမူနာများကို ဆွဲယူပါ။
5. နမူနာပေးချေမှုများဖြင့် SDK ပိုင်ရှင်များ (`#sdk-parity`) ကို အကြောင်းကြားရန် ၎င်းတို့အား စမ်းသပ်နိုင်သည်။

## အမြစ်-အကြောင်းရင်းကို ခွဲထုတ်ခြင်း။

### မီးစက် (သို့) မီးစက် ပျံ့

- `cargo xtask address-vectors --verify` ကို ပြန်ဖွင့်ပါ။ အဆင်မပြေရင် ပြန်ထုတ်ပါ။
- `ci/account_fixture_metrics.sh` (သို့မဟုတ် တစ်ဦးချင်းလုပ်ဆောင်ပါ။
  SDK တစ်ခုစီအတွက် `scripts/account_fixture_helper.py check`) အစုအဝေးကို အတည်ပြုရန်
  ပြင်ဆင်မှုများသည် Canonical JSON နှင့် ကိုက်ညီသည်။

### သုံးစွဲသူ ကုဒ်နံပါတ်များ / IME ဆုတ်ယုတ်မှုများ

- အနံ သုညကို ရှာရန် `iroha tools address inspect` မှတစ်ဆင့် သုံးစွဲသူမှပေးသော စာများကို စစ်ဆေးပါ
  ချိတ်ဆက်မှုများ၊ kana ပြောင်းလဲမှုများ သို့မဟုတ် ဖြတ်တောက်ထားသော ပေးဆောင်မှုများ။
- အပြန်အလှန်စစ်ဆေးသောပိုက်ဆံအိတ် / ရှာဖွေသူနှင့်အတူစီးဆင်း
  `docs/source/sns/address_display_guidelines.md` (မိတ္တူနှစ်ထပ်ပစ်မှတ်များ၊ သတိပေးချက်များ၊
  QR ကူညီပေးသူများ) ၎င်းတို့သည် အတည်ပြုထားသော UX ကို လိုက်နာကြောင်း သေချာစေရန်။

### Manifest သို့မဟုတ် registry ပြဿနာများ

- နောက်ဆုံးပေါ် manifest အစုအဝေးကို ပြန်လည်စစ်ဆေးရန် `address_manifest_ops.md` ကို လိုက်နာပါ
  Local-8 ရွေးစရာများ ထပ်ပေါ်မလာကြောင်း သေချာပါစေ။
  payloads တွင်ပေါ်လာသည်။

### အန္တရာယ်ရှိသော သို့မဟုတ် ပုံစံမမှန်သော လမ်းကြောင်း

- Torii မှတ်တမ်းများနှင့် `torii_http_requests_total` မှတစ်ဆင့် စော်ကားသော IPs/app ID များကို ပိုင်းခြားပါ။
- လုံခြုံရေး / အုပ်ချုပ်မှုနောက်ဆက်တွဲအတွက် အနည်းဆုံး 24 နာရီမှတ်တမ်းများကိုသိမ်းဆည်းပါ။

## လျော့ပါးရေးနှင့် ပြန်လည်ထူထောင်ရေး

| ဇာတ်လမ်း | လုပ်ဆောင်ချက်များ |
|----------|---------|
| Fixture ပျံ့ | `fixtures/account/address_vectors.json` ကို ပြန်ထုတ်ပါ၊ `cargo xtask address-vectors --verify` ကို ပြန်ဖွင့်ပါ၊ SDK အစုအဝေးများကို အပ်ဒိတ်လုပ်ကာ လက်မှတ်တွင် `address_fixture.prom` လျှပ်တစ်ပြက်ရိုက်ချက်များကို ပူးတွဲပါ။ |
| SDK/Client ဆုတ်ယုတ်မှု | Canonical fixture + `iroha tools address inspect` output နှင့် SDK parity CI ၏နောက်ကွယ်တွင် gate releases ပြဿနာများ (ဥပမာ `ci/check_address_normalize.sh`)။ |
| မလိုလားအပ်သော တင်ပြချက်များ | အဆင့်သတ်မှတ်ခြင်း သို့မဟုတ် စော်ကားသောကျောင်းအုပ်များကို ပိတ်ဆို့ပါ၊ သင်္ချိုင်းတွင်းရွေးချယ်ပေးသူများ လိုအပ်ပါက အုပ်ချုပ်မှုသို့ တိုးမြှင့်ပါ။ |

သက်သာရာရပြီးသည်နှင့် အတည်ပြုရန် အထက်ဖော်ပြပါ PromQL မေးခွန်းကို ပြန်ဖွင့်ပါ။
`ERR_CHECKSUM_MISMATCH` သည် အနည်းဆုံး သုည (`/tests/*` မပါဝင်) တွင် ရှိနေသည်
အဖြစ်အပျက်ကို အဆင့်မချမီ မိနစ် ၃၀ အလို။

## ပိတ်တယ်။

1. Archive Grafana လျှပ်တစ်ပြက်ရိုက်ချက်များ၊ PromQL CSV၊ မှတ်တမ်းကောက်နုတ်ချက်များနှင့် `address_fixture.prom`။
2. tooling/docs ရှိပါက `status.md` (ADDR အပိုင်း) နှင့် လမ်းပြမြေပုံအတန်းအား အပ်ဒိတ်လုပ်ပါ
   ပြောင်းလဲခဲ့သည်။
3. သင်ခန်းစာအသစ်များရသောအခါ `docs/source/sns/incidents/` အောက်တွင် အဖြစ်အပျက်လွန်မှတ်စုများကို ရေးပါ။
   ပေါ်ထွက်လာ
4. SDK ထုတ်ဝေမှုမှတ်စုတွင် အကျုံးဝင်သည့်အချိန်တွင် ချက်လက်မှတ်ပြင်ဆင်မှုများကို ဖော်ပြထားသည်ကို သေချာပါစေ။
5. သတိပေးချက်သည် 24 နာရီကြာ စိမ်းနေမည်ကို အတည်ပြုပြီး မီးခြစ်စစ်ဆေးမှုများသည် အစိမ်းရောင်ရှိနေဆဲဖြစ်ကြောင်း အတည်ပြုပါ။
   ဖြေရှင်းခြင်း။