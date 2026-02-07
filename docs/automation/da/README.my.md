---
lang: my
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Data-Availability Threat-Model Automation (DA-1)

လမ်းပြမြေပုံ အကြောင်းအရာ DA-1 နှင့် `status.md` သည် အဆိုပါ အဆုံးအဖြတ်ပေးသော အလိုအလျောက်စနစ် လည်ပတ်မှုကို တောင်းဆိုသည်
Norito PDP/PoTR ခြိမ်းခြောက်မှုပုံစံ အနှစ်ချုပ်များကို ထုတ်လုပ်သည်
`docs/source/da/threat_model.md` နှင့် Docusaurus မှန်။ ဒီလမ်းညွှန်
ကိုးကားထားသော ပစ္စည်းများကို ဖမ်းယူသည်-

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (`scripts/docs/render_da_threat_model_tables.py` အလုပ်လုပ်သည်)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## စီးဆင်းမှု

1. **အစီရင်ခံစာကို ဖန်တီးပါ**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON အနှစ်ချုပ်သည် အတုယူထားသော ပုံတူပွားမှု မအောင်မြင်မှုနှုန်း၊ chunker ကို မှတ်တမ်းတင်သည်။
   သတ်မှတ်ချက်များနှင့် PDP/PoTR ကြိုးဝိုင်းမှ တွေ့ရှိသော မူဝါဒချိုးဖောက်မှုများ
   `integration_tests/src/da/pdp_potr.rs`။
2. ** Markdown ဇယားများကို Render လုပ်ပါ**
   ```bash
   make docs-da-threat-model
   ```
   ၎င်းသည် `scripts/docs/render_da_threat_model_tables.py` ကို ပြန်လည်ရေးသားရန် လုပ်ဆောင်သည်။
   `docs/source/da/threat_model.md` နှင့် `docs/portal/docs/da/threat-model.md`။
3. JSON အစီရင်ခံစာ (နှင့် စိတ်ကြိုက်ရွေးချယ်နိုင်သော CLI မှတ်တမ်း) ကို ကူးယူခြင်းဖြင့် **အနုပညာပစ္စည်းများကို သိမ်းဆည်းပါ**
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`။ ဘယ်တော့လဲ။
   အုပ်ချုပ်မှုဆိုင်ရာ ဆုံးဖြတ်ချက်များသည် တိကျသောလုပ်ဆောင်မှုတစ်ခုအပေါ် မူတည်ပြီး၊ git commit hash နှင့် ပါဝင်သည်။
   ပေါက်ဖော် `<timestamp>-metadata.md` ရှိ Simulator မျိုးစေ့။

## သက်သေမျှော်လင့်ချက်

- JSON ဖိုင်များသည် <100 KiB ရှိနေသင့်သည်၊ သို့မှသာ ၎င်းတို့သည် git တွင် နေထိုင်နိုင်သည်။ ပိုကြီးတဲ့ ကွပ်မျက်မှု
  သဲလွန်စများသည် ပြင်ပသိုလှောင်မှုတွင် ပါ၀င်သည်—မက်တာဒေတာတွင် ၎င်းတို့၏ လက်မှတ်ထိုးထားသော hash ကို ကိုးကားပါ။
  လိုအပ်ရင် မှတ်ထားပါ။
- သိမ်းဆည်းထားသောဖိုင်တစ်ခုစီတိုင်းသည် မျိုးစေ့၊ config လမ်းကြောင်းနှင့် simulator ဗားရှင်းတို့ကို စာရင်းပြုစုထားရပါမည်။
  DA ထုတ်ပေးသည့်ဂိတ်များကို စာရင်းစစ်သည့်အခါ အတိအကျပြန်ထုတ်ပေးနိုင်သည်။
- `status.md` မှ သိမ်းဆည်းထားသော ဖိုင်သို့ ပြန်လည် လင့်ခ် သို့မဟုတ် လမ်းပြမြေပုံ ဝင်ခွင့်ကို အချိန်တိုင်း
  DA-1 လက်ခံမှုစံနှုန်းများကို ကြိုတင်ပြင်ဆင်ထားပြီး၊ သုံးသပ်သူများသည် အဆိုပါအချက်အား အတည်ပြုနိုင်စေရန် သေချာစေပါသည်။
  ကြိုးကို ပြန်မတီးဘဲ အခြေခံလိုင်း။

## ကတိကဝတ် ပြန်လည်သင့်မြတ်ရေး (Sequencer Omission)

DA ထည့်သွင်းလက်ခံဖြတ်ပိုင်းများနှင့် နှိုင်းယှဉ်ရန် `cargo xtask da-commitment-reconcile` ကိုသုံးပါ။
DA ကတိကဝတ် မှတ်တမ်းများ၊ ဆက်တိုက် ပျက်ကွက်ခြင်း သို့မဟုတ် လက်ဆော့ခြင်းကို ဖမ်းယူခြင်း-

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito သို့မဟုတ် JSON ဖောင်နှင့် ကတိကဝတ်များမှ ပြေစာများကို လက်ခံသည်
  `SignedBlockWire`၊ `.norito` သို့မဟုတ် JSON အစုအဝေးများ။
- block log မှ လက်မှတ်ပျောက်ဆုံးသည့်အခါ သို့မဟုတ် hash များ ကွဲပြားသွားသည့်အခါ မအောင်မြင်ပါ။
  `--allow-unexpected` သည် သင် ရည်ရွယ်ချက်ရှိရှိ ကန့်သတ်လိုက်သောအခါတွင် ပိတ်ဆို့ခြင်းသီးသန့် လက်မှတ်များကို လျစ်လျူရှုသည်
  ပြေစာအစုံ။
- ပျက်ကွက်မှုအတွက် ထုတ်လွှတ်သော JSON ကို အုပ်ချုပ်မှုဆိုင်ရာ ပက်ကတ်များ/သတိပေးချက်မန်နေဂျာထံ ပူးတွဲပါ
  သတိပေးချက်များ; ပုံသေ `artifacts/da/commitment_reconciliation.json`။

## အထူးအခွင့်အရေးစာရင်းစစ် (သုံးလပတ်ဝင်ရောက်ကြည့်ရှုစစ်ဆေးခြင်း)

DA မန်နီးဖက်စ်/ပြန်ဖွင့်သည့်လမ်းညွှန်များကို စကင်န်ဖတ်ရန် `cargo xtask da-privilege-audit` ကိုသုံးပါ။
ပျောက်ဆုံးနေသော၊ လမ်းညွှန်မဟုတ်သော သို့မဟုတ် ကမ္ဘာရေး၍မရသော လမ်းကြောင်းများ (အပြင် ရွေးချယ်နိုင်သော အပိုလမ်းကြောင်းများ)
ထည့်သွင်းမှုများ-

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- ပေးထားသော Torii config မှ DA သွင်းလမ်းကြောင်းများကိုဖတ်ပြီး Unix ကိုစစ်ဆေးသည်
  ရရှိနိုင်သောခွင့်ပြုချက်များ။
- ပျောက်ဆုံးနေသော/မ-က-လမ်းညွှန်/ကမ္ဘာ-ရေးနိုင်သောလမ်းကြောင်းများကို အလံများနှင့် သုညမဟုတ်သော ထွက်ပေါက်ကို ပြန်ပေးသည်။
  ပြဿနာများရှိသောအခါကုဒ်။
- လက်မှတ်ရေးထိုးပြီး JSON အစုအဝေး (`artifacts/da/privilege_audit.json` by
  မူရင်း) သုံးလတစ်ကြိမ် ဝင်ရောက်-သုံးသပ်ခြင်း ပက်ကတ်များနှင့် ဒက်ရှ်ဘုတ်များ။