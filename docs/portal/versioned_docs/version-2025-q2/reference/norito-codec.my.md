---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito Codec အကိုးအကား

Norito သည် Iroha ၏ canonical serialization အလွှာဖြစ်သည်။ on-wire message တိုင်း၊ on-disk
payload နှင့် cross-component API သည် Norito ကိုအသုံးပြုသောကြောင့် nodes များသည် တူညီသောဘိုက်များကိုသဘောတူသည်
မတူညီတဲ့ hardware တွေပေါ်မှာ အလုပ်လုပ်နေရင်တောင်။ ဤစာမျက်နှာသည် ရွေ့လျားနေသော အစိတ်အပိုင်းများကို အကျဉ်းချုပ်ဖော်ပြသည်။
`norito.md` တွင် သတ်မှတ်ချက် အပြည့်အစုံကို ညွှန်ပြသည်။

## Core အပြင်အဆင်

| အစိတ်အပိုင်း | ရည်ရွယ်ချက် | အရင်းအမြစ် |
| ---| ---| ---|
| **ခေါင်းစီး** | Negotiates အင်္ဂါရပ်များ (ထုပ်ပိုးထားသော တည်ဆောက်ပုံများ/ sequences၊ ကျစ်လစ်သိပ်သည်းသော အရှည်များ၊ ဖိသိပ်မှုအလံများ) နှင့် CRC64 checksum ကို မြှုပ်နှံထားသောကြောင့် payload integrity ကို decode မလုပ်မီ စစ်ဆေးပါသည်။ | `norito::header` — `norito.md` ကိုကြည့်ပါ (“ခေါင်းစီးနှင့်အလံများ”၊ repository root) |
| **ဝန်ဆောင်ခ** | hashing/comparison အတွက် သုံးသော သတ်မှတ်တန်ဖိုး ကုဒ်နံပါတ် သယ်ယူပို့ဆောင်ရေးအတွက် တူညီသော အပြင်အဆင်ကို ခေါင်းစီးဖြင့် ပတ်ထားသည်။ | `norito::codec::{Encode, Decode}` |
| **ချုံ့** | ရွေးချယ်နိုင်သော Zstd (နှင့် စမ်းသပ်ထားသော GPU အရှိန်မြှင့်ခြင်း) ကို `compression` အလံ byte မှတစ်ဆင့် အသက်သွင်းခဲ့သည်။ | `norito.md`၊ "ချုံ့ညှိနှိုင်းမှု" |

အလံ မှတ်ပုံတင်ခြင်း အပြည့်အစုံ (packed-struct၊ packed-seq၊ ကျစ်လစ်သော အရှည်များ၊ ဖိသိပ်မှု)
`norito::header::flags` တွင်နေထိုင်သည်။ `norito::header::Flags` သည် အဆင်ပြေသည်
runtime စစ်ဆေးခြင်းအတွက် စစ်ဆေးမှုများ၊ သီးသန့် layout bits များကို decoders များမှ ပယ်ချပါသည်။

## ပံ့ပိုးမှုရယူပါ။

`norito_derive` သည် `Encode`၊ `Decode`၊ `IntoSchema` နှင့် JSON အကူအညီပေးသူထံမှ ရရှိသည်။
အဓိက သဘောတူညီချက်များ-

- `packed-struct` အင်္ဂါရပ်သည် ထုပ်ပိုးထားသော layouts များကို ကောက်နှုတ်ခြင်း/ စည်းမျဥ်းများ
  ဖွင့်ထားသည် (မူလ)။ အကောင်အထည်ဖော်မှုသည် `crates/norito_derive/src/derive_struct.rs` တွင် နေထိုင်ပါသည်။
  နှင့် အပြုအမူကို `norito.md` ("ထုပ်ပိုးထားသော လက်ကွက်များ") တွင် မှတ်တမ်းတင်ထားသည်။
- ထုပ်ပိုးထားသော စုစည်းမှုများကို v1 တွင် ပုံသေ အနံ အတွဲလိုက် ခေါင်းစီးများနှင့် အော့ဖ်ဆက်များကို အသုံးပြုသည်။ သာ
  တစ်ခုချင်းတန်ဖိုးအရှည် ရှေ့ဆက်များသည် `COMPACT_LEN` ဖြင့် သက်ရောက်မှုရှိသည်။
- JSON အကူအညီပေးသူများ (`norito::json`) သည် အဆုံးအဖြတ်ပေးသော Norito-ကျောထောက်နောက်ခံ JSON အတွက် ပံ့ပိုးပေးသည်
  APIs ကိုဖွင့်ပါ။ `norito::json::{to_json_pretty, from_json}` ကိုသုံးပါ — ဘယ်တော့မှ `serde_json` ကိုသုံးပါ။

## Multicodec & identifier ဇယားများ

Norito သည် ၎င်း၏ multicodec တာဝန်များကို `norito::multicodec` တွင် ထိန်းသိမ်းထားသည်။ အကိုးအကား
ဇယား (hashes၊ သော့အမျိုးအစားများ၊ payload ဖော်ပြချက်များ) ကို `multicodec.md` တွင် ထိန်းသိမ်းထားသည်။
repository root မှာ။ အထောက်အထားအသစ်တစ်ခုကို ထည့်သောအခါ-

1. `norito::multicodec::registry` ကို အပ်ဒိတ်လုပ်ပါ။
2. `multicodec.md` တွင် ဇယားကို တိုးချဲ့ပါ။
3. ၎င်းတို့သည် မြေပုံကို စားသုံးပါက ရေစုန်နှောင်ကြိုးများ (Python/Java) ကို ပြန်ထုတ်ပါ။

## စာရွက်စာတမ်းများနှင့် ဖိုင်များကို ပြန်လည်ထုတ်ပေးခြင်း။

စကားပြေအကျဉ်းချုပ်ကို လက်ရှိအသုံးပြုနေသည့် ပေါ်တယ်အနေဖြင့်၊ အထက်စီးကြောင်း Markdown ကို အသုံးပြုပါ။
အမှန်တရား၏အရင်းအမြစ်အဖြစ်၊

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **စံသတ်မှတ်ချက်များ**: `crates/norito/benches/`
- **ရွှေစမ်းသပ်မှုများ**: `crates/norito/tests/`

Docusaurus automation တိုက်ရိုက်လွှင့်သည့်အခါ၊ portal မှတဆင့် update လုပ်ပါမည်။
ဤအရာများမှဒေတာကိုဆွဲယူသောတစ်ပြိုင်တည်းချိန်ကိုက်ခြင်း script (`docs/portal/scripts/`)
ဖိုင်များ။ ထိုအချိန်အထိ၊ spec ပြောင်းလဲသည့်အခါတိုင်း၊ ဤစာမျက်နှာကို ကိုယ်တိုင်ချိန်ညှိထားပါ။