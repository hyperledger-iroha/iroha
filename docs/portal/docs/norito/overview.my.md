---
lang: my
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2025-12-29T18:16:35.153135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Norito ခြုံငုံသုံးသပ်ချက်

Norito သည် Iroha တွင်အသုံးပြုသော binary serialization အလွှာဖြစ်သည်- ဒေတာကိုမည်သို့သတ်မှတ်သည်
တည်ဆောက်ပုံများကို ဝါယာကြိုးပေါ်တွင် ကုဒ်ဝှက်ထားပြီး၊ ဒစ်ပေါ်တွင် ဆက်လက်တည်ရှိကာ အချင်းချင်း အပြန်အလှန်ဖလှယ်ကြသည်။
စာချုပ်များနှင့်အိမ်ရှင်များ။ အလုပ်ခွင်ရှိ သေတ္တာတိုင်းသည် Norito အစား Norito ကို အားကိုးသည်။
`serde` ထို့ကြောင့် မတူညီသော ဟာ့ဒ်ဝဲရှိ ရွယ်တူများသည် တူညီသော ဘိုက်များကို ထုတ်လုပ်သည်။

ဤခြုံငုံသုံးသပ်ချက်သည် ပင်မအပိုင်းအစများကို အကျဉ်းချုပ်ပြီး canonical ကိုးကားချက်များဆီသို့ လင့်ခ်များကို ပေးသည်။

## ဗိသုကာပညာကို တစ်ချက်ကြည့်လိုက်သည်။

- **Header + payload** – Norito မက်ဆေ့ဂျ်တစ်ခုစီသည် feature-ညှိနှိုင်းမှုဖြင့် စတင်သည်
  ခေါင်းစီး (အလံများ၊ ချက်ခ်ဆမ်း) ၏နောက်တွင် ဗလာပါယာဝန်ပါရှိသည်။ ထုပ်ပိုးထားသော လက်ကွက်များနှင့်
  compression ကို header bits မှတဆင့် ညှိနှိုင်းသည်။
- **Deterministic encoding** – `norito::codec::{Encode, Decode}` ကိုအကောင်အထည်ဖော်ပါ။
  ဗလာကုဒ်ပြောင်းခြင်း။ ခေါင်းစီးများတွင် payload များကို ထုပ်ပိုးသည့်အခါ တူညီသော အပြင်အဆင်ကို ပြန်သုံးပါသည်။
  hashing နှင့် signing သည် အဆုံးအဖြတ်အဖြစ် ဆက်လက်တည်ရှိနေပါသည်။
- **Schema + derives** – `norito_derive` သည် `Encode`၊ `Decode` နှင့်
  `IntoSchema` အကောင်အထည်ဖော်မှုများ။ ထုပ်ပိုးထားသော တည်ဆောက်ပုံ/အစီအစဥ်များကို မူရင်းအတိုင်း ဖွင့်ထားသည်။
  `norito.md` တွင် မှတ်တမ်းတင်ထားသည်။
- **Multicodec registry** – hash, key အမျိုးအစားများနှင့် payload အတွက် ခွဲခြားသတ်မှတ်မှုများ
  ဖော်ပြချက်များသည် `norito::multicodec` တွင် နေထိုင်ပါသည်။ ကျမ်းကိုးဇယား
  `multicodec.md` တွင် ထိန်းသိမ်းထားသည်။

## တန်ဆာပလာ

| တာဝန် | Command / API | မှတ်စုများ |
| ---| ---| ---|
| ခေါင်းစီး/အပိုင်းများကို စစ်ဆေးပါ | `ivm_tool inspect <file>.to` | ABI ဗားရှင်း၊ အလံများနှင့် ဝင်ခွင့်အမှတ်များကို ပြသသည်။ |
| Rust | တွင် ကုဒ်/ဝှက်ခြင်း `norito::codec::{Encode, Decode}` | core data-model အမျိုးအစားအားလုံးအတွက် အကောင်အထည်ဖော်ပါ။ |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | Norito တန်ဖိုးများဖြင့် ကျောထောက်နောက်ခံပြုထားသော ဆုံးဖြတ်ခြင်း JSON။ |
| docs/ specs | ထုတ်လုပ်ပါ။ `norito.md`, `multicodec.md` | repo root ရှိ အရင်းအမြစ်-of-truth စာရွက်စာတမ်း။ |

## ဖွံ့ဖြိုးတိုးတက်ရေးလုပ်ငန်းစဉ်

1. ** ထုတ်ယူမှုများကို ပေါင်းထည့်ပါ** – ဒေတာအသစ်အတွက် `#[derive(Encode, Decode, IntoSchema)]` ကို ဦးစားပေးပါ
   အဆောက်အဦများ။ မလိုအပ်ဘဲ လက်ဖြင့်ရေးထားသော အမှတ်စဉ်များကို ရှောင်ကြဉ်ပါ။
2. **ထုပ်ပိုးထားသော အပြင်အဆင်များကို သက်သေပြပါ** – `cargo test -p norito` ကို အသုံးပြုပါ (နှင့် ထုပ်ပိုးထားသော အရာများ
   အသစ်သေချာစေရန်အတွက် `scripts/run_norito_feature_matrix.sh`)
   အပြင်အဆင်များသည် တည်ငြိမ်နေပါသည်။
3. ** docs ကို ပြန်ထုတ်ပါ** – ကုဒ်ပြောင်းသွားသောအခါ၊ `norito.md` နှင့် ကို အပ်ဒိတ်လုပ်ပါ။
   multicodec ဇယား၊ ထို့နောက် portal စာမျက်နှာများ (`/reference/norito-codec`
   နှင့် ဤသုံးသပ်ချက်)။
4. **စမ်းသပ်မှုများ Norito ကို ထားရှိပါ** – ပေါင်းစည်းခြင်း စမ်းသပ်မှုများ Norito JSON ကို အသုံးပြုသင့်သည်
   `serde_json` အစား အကူအညီပေးသူများသည် ထုတ်လုပ်မှုကဲ့သို့ တူညီသောလမ်းကြောင်းများကို ကျင့်သုံးကြသည်။

## အမြန်လင့်များ

သတ်မှတ်ချက်- [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec တာဝန်များ- [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Feature matrix script- `scripts/run_norito_feature_matrix.sh`
- Packed-layout ဥပမာ- `crates/norito/tests/`

ဤခြုံငုံသုံးသပ်ချက်ကို တစ်ခုအတွက် အမြန်စတင်လမ်းညွှန် (`/norito/getting-started`) နှင့် တွဲပါ။
Norito ကို အသုံးပြုသည့် bytecode ပြုစုခြင်းနှင့် လုပ်ဆောင်ခြင်း၏ လက်ကမ်းတင်ပြချက်
ဝန်ဆောင်ခများ။