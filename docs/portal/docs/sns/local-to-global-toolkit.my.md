---
lang: my
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

ဤစာမျက်နှာသည် [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
mono-repo မှ။ လမ်းပြမြေပုံအကြောင်းအရာ **ADDR-5c** မှ လိုအပ်သော CLI အထောက်အပံများနှင့် runbook များကို ထုပ်ပိုးထားသည်။

## ခြုံငုံသုံးသပ်ချက်

- ထုတ်လုပ်ရန် `scripts/address_local_toolkit.sh` သည် `iroha` CLI ကို ထုပ်ပိုးထားသည်-
  - `audit.json` — `iroha tools address audit --format json` မှ ဖွဲ့စည်းပုံအထွက်။
  - `normalized.txt` — Local-domain ရွေးပေးသူတိုင်းအတွက် နှစ်သက်ရာ I105 / ဒုတိယအကောင်းဆုံးချုံ့ထားသော (`sora`) literals များကို ပြောင်းထားသည်။
- လိပ်စာထည့်သွင်းထားသော ဒက်ရှ်ဘုတ် (`dashboards/grafana/address_ingest.json`) နှင့် ဇာတ်ညွှန်းကို တွဲပါ
  Local-8/ ကိုသက်သေပြရန် နှင့် Alertmanager စည်းမျဉ်းများ (`dashboards/alerts/address_ingest_rules.yml`)
  Local-12 cutover သည် ဘေးကင်းပါသည်။ Local-8 နှင့် Local-12 ယာဉ်တိုက်မှုအကန့်များ ပေါင်း၍ ကြည့်ရှုပါ။
  `AddressLocal8Resurgence`၊ `AddressLocal12Collision` နှင့် `AddressInvalidRatioSlo` မတိုင်မီ သတိပေးချက်များ
  ထင်ရှားသောပြောင်းလဲမှုများကို မြှင့်တင်ခြင်း။
- [လိပ်စာပြသမှုလမ်းညွှန်ချက်များ](address-display-guidelines.md) နှင့်
  UX နှင့် အဖြစ်အပျက်-တုံ့ပြန်မှုဆိုင်ရာ အကြောင်းအရာအတွက် [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md)။

##အသုံးပြုမှု

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

ရွေးချယ်စရာများ-

- I105 အစား `i105` အထွက်အတွက် `--format i105`။
- စာလုံးဗလာများကိုထုတ်လွှတ်ရန် `domainless output (default)`။
- ပြောင်းလဲခြင်းအဆင့်ကို ကျော်ရန် `--audit-only`။
ပုံမမှန်သောအတန်းများပေါ်လာသောအခါ (CLI အပြုအမူနှင့် ကိုက်ညီသည်)။

script သည် run ၏အဆုံးတွင် artefact paths များကိုရေးသားသည်။ ဖိုင်နှစ်ခုလုံးကို ပူးတွဲပါ။
သုညကို သက်သေပြသော Grafana ဖန်သားပြင်ဓာတ်ပုံနှင့်အတူ သင်၏ပြောင်းလဲမှု-စီမံခန့်ခွဲမှုလက်မှတ်
Local-8 ထောက်လှမ်းမှုနှင့် Local-12 တိုက်မိမှု သုည ≥30 ရက်။

## CI ပေါင်းစည်းခြင်း။

1. သီးသန့်အလုပ်တစ်ခုတွင် script ကို run ပြီး ၎င်း၏ output များကို အပ်လုဒ်လုပ်ပါ။
2. `audit.json` သည် Local selectors (`domain.kind = local12`) ကို အစီရင်ခံသောအခါ ပေါင်းစပ်မှုများကို ပိတ်ဆို့ပါ။
   ၎င်း၏မူလ `true` တန်ဖိုးတွင် (dev/test clusters များတွင်သာ `false` သို့ ပြောင်းသည့်အခါတွင်သာ
   ဆုတ်ယုတ်မှုများကို အဖြေရှာခြင်း) နှင့် ပေါင်းထည့်ပါ။
   `iroha tools address normalize` သည် CI ဖြစ်သောကြောင့် ဆုတ်ယုတ်သွားသည်။
   ထုတ်လုပ်မှုကို မစမီ ကြိုးစားမှု မအောင်မြင်ပါ။

နောက်ထပ်အသေးစိတ်အချက်အလက်များအတွက် အရင်းအမြစ်စာရွက်စာတမ်း၊ နမူနာအထောက်အထားစစ်ဆေးမှုစာရင်းများနှင့် ဖောက်သည်များအား ဖြတ်တောက်မှုကို ကြေငြာသည့်အခါ သင်ပြန်လည်အသုံးပြုနိုင်သည့် ထုတ်ပြန်ချက်-မှတ်စုအတိုအထွာကို ကြည့်ရှုပါ။