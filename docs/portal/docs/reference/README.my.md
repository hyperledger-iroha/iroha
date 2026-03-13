---
lang: my
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-12-29T18:16:35.156432+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

ဤအပိုင်းသည် Iroha အတွက် "၎င်းကို spec တစ်ခုအနေဖြင့် ဖတ်ပါ" ပစ္စည်းကို စုစည်းထားသည်။ ဤစာမျက်နှာများကဲ့သို့ပင် တည်ငြိမ်နေပါသည်။
လမ်းညွှန်များနှင့် သင်ခန်းစာများ တိုးတက်ပြောင်းလဲလာသည်။

## ယနေ့ ရရှိနိုင်ပါပြီ။

- **Norito codec ခြုံငုံသုံးသပ်ချက်** – `reference/norito-codec.md` သည် တရားဝင်စာထံသို့ တိုက်ရိုက်လင့်ခ်များ
  ပေါ်တယ်ဇယားကို လူဦးရေဖြည့်နေစဉ် `norito.md` သတ်မှတ်ချက်။
- **Torii OpenAPI** – `/reference/torii-openapi` သည် နောက်ဆုံးထွက် Torii REST သတ်မှတ်ချက်ကို အသုံးပြုပြီး ပြန်ဆိုသည်
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v2/mcp`.
  ပြန်ယူပါ။ spec ကို `npm run sync-openapi -- --version=current --latest` ဖြင့် ပြန်ထုတ်ပါ (ထည့်ပါ။
  လျှပ်တစ်ပြက်ရိုက်ချက်အား နောက်ထပ်သမိုင်းဝင်ဗားရှင်းများသို့ ကူးယူရန် `--mirror=<label>`)။
- **Configuration tables** – ကန့်သတ်ချက်အပြည့်အစုံကို ကတ်တလောက်တွင် သိမ်းဆည်းထားသည်။
  `docs/source/references/configuration.md`။ ပေါ်တယ်မှ အလိုအလျောက် တင်သွင်းမှုကို သင်္ဘောမတင်မချင်း ၎င်းကို ကိုးကားပါ။
  အတိအကျ ပုံသေများနှင့် ပတ်ဝန်းကျင်ကို ပြန်လှန်ရန်အတွက် Markdown ဖိုင်။
- **Docs ဗားရှင်းပြုလုပ်ခြင်း** - navbar ဗားရှင်း dropdown သည် အေးခဲနေသော လျှပ်တစ်ပြက်ရိုက်ချက်များဖြင့် ဖန်တီးထားသည်။
  `npm run docs:version -- <label>`၊ ထုတ်ဝေမှုများတစ်လျှောက် လမ်းညွှန်ချက်ကို နှိုင်းယှဉ်ရန် လွယ်ကူစေသည်။

## မကြာမီလာမည်

- **Torii REST ရည်ညွှန်းချက်** – OpenAPI အဓိပ္ပါယ်ဖွင့်ဆိုချက်များကို ဤကဏ္ဍမှတစ်ဆင့် ထပ်တူပြုပါမည်။
  ပိုက်လိုင်းကိုဖွင့်ပြီးသည်နှင့် `docs/portal/scripts/sync-openapi.mjs`။
- **CLI အမိန့်အညွှန်းကိန်း** - ထုတ်ပေးထားသော အမိန့်မက်ထရစ် (`crates/iroha_cli/src/commands`)
  Canonical ဥပမာများနှင့်အတူ ဤနေရာတွင် ဆင်းသက်ပါမည်။
- **IVM ABI tables** – pointer-type နှင့် syscall matrices (`crates/ivm/docs` အောက်တွင် ထိန်းသိမ်းထားသည်)
  doc မျိုးဆက်အလုပ်အား ကြိုးတပ်ပြီးသည်နှင့် portal သို့ ပြန်ဆိုပါမည်။

## ဤအညွှန်းကိန်းကို လက်ရှိထိန်းသိမ်းထားပါ။

ရည်ညွှန်းပစ္စည်းအသစ်ကို ပေါင်းထည့်သောအခါ—ထုတ်လုပ်ထားသော API စာရွက်စာတမ်းများ၊ codec သတ်မှတ်ချက်များ၊ ဖွဲ့စည်းမှုပုံစံများ—နေရာ
`docs/portal/docs/reference/` အောက်ရှိ စာမျက်နှာနှင့် အပေါ်ကလင့်ခ်ကို ကြည့်ပါ။ စာမျက်နှာတစ်ခုသည် အလိုအလျောက်ထုတ်ပေးပါက၊ မှတ်သားပါ။
ထပ်တူကျသော script များကို ပံ့ပိုးပေးသူများသည် ၎င်းကို မည်သို့ ပြန်လည်စတင်ရမည်ကို သိနိုင်စေရန်။ ၎င်းသည် အကိုးအကားသစ်ပင်ကို အသုံးမဝင်မချင်း ထိန်းသိမ်းထားသည်။
အပြည့်အဝ အလိုအလျောက်ထုတ်လုပ်ထားသော လမ်းကြောင်းပြမြေများ။
