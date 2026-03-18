---
lang: my
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Crypto မှီခိုမှုစစ်ဆေးမှု

## Streebog (`streebog` သေတ္တာ)

- **သစ်ပင်ရှိဗားရှင်း-** `0.11.0-rc.2` `vendor/streebog` (`gost` အင်္ဂါရပ်ကို ဖွင့်ထားသောအခါတွင် အသုံးပြုသည်)။
- **စားသုံးသူ-** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + မက်ဆေ့ချ်ကို တားဆီးခြင်း)။
- **အခြေအနေ-** ထုတ်ဝေသူ- ကိုယ်စားလှယ်လောင်းသာ။ RC မဟုတ်သော သေတ္တာများ လောလောဆယ်တွင် လိုအပ်သော API မျက်နှာပြင်ကို ပေးဆောင်ထားခြင်းမရှိပါ။
  ထို့ကြောင့် ကျွန်ုပ်တို့သည် နောက်ဆုံးထွက်ရှိမှုအတွက် ရေအထက်တွင် ခြေရာခံနေစဉ် စာရင်းစစ်နိုင်စေရန် သစ်သေတ္တာအတွင်းမှ ကြေးမုံပြင်ကို ရောင်ပြန်ဟပ်ပါသည်။
- ** စစ်ဆေးရေးဂိတ်များ ပြန်လည်စစ်ဆေးခြင်း-**
  - Wycheproof suite နှင့် TC26 fixtures များမှ တဆင့် hash output ကို စစ်ဆေးပြီး
    `cargo test -p iroha_crypto --features gost` (`crates/iroha_crypto/tests/gost_wycheproof.rs` ကိုကြည့်ပါ)။
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    Ed25519/Secp256k1 ကို လက်ရှိ မှီခိုမှုနှင့်အတူ TC26 မျဉ်းကွေးတိုင်းနှင့်အတူ လေ့ကျင့်ခန်းလုပ်သည်။
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    checked-in medians များနှင့် နှိုင်းယှဉ်ပါက ပိုသစ်လွင်သော တိုင်းတာမှုများ (CI တွင် `--summary-only` ကိုသုံးပါ၊ ထည့်ပါ
    ပြန်လည်စတင်သည့်အခါ `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`။
  - `scripts/gost_bench.sh` ခုံတန်းလျား + စစ်ဆေးစီးဆင်းမှုကို ထုပ်ပိုးထားသည်။ JSON ကို အပ်ဒိတ်လုပ်ရန် `--write-baseline` ကို ကျော်ဖြတ်ပါ။
    အဆုံးမှအဆုံး အလုပ်အသွားအလာအတွက် `docs/source/crypto/gost_performance.md` ကို ကြည့်ပါ။
- **လျော့ပါးစေခြင်း-** `streebog` သည် သော့များကို zeroise ဖြစ်သော အဆုံးအဖြတ်ပေးသော ထုပ်ပိုးမှုများဖြင့်သာ ခေါ်ဆိုပါသည်။
  ဆိုးရွားသော RNG ချို့ယွင်းမှုကို ရှောင်ရှားရန် လက်မှတ်ထိုးသူက OS entropy ဖြင့် အကာအကွယ်ပေးသည်။
- **နောက်ထပ်လုပ်ဆောင်ချက်များ-** RustCrypto ၏ streebog `0.11.x` ထွက်ရှိမှုကို လိုက်နာပါ။ tag ပြီးတာနဲ့ ဆက်ဆံပါ။
  စံမှီခိုမှုအဖုအထစ်တစ်ခုအဖြစ် အဆင့်မြှင့်ပါ ( checksum ကိုစစ်ဆေးပါ၊ ကွဲပြားမှုကို ပြန်လည်သုံးသပ်ပါ၊ သက်သေအထောက်အထား မှတ်တမ်းနှင့်
  ရောင်းတဲ့မှန်ကို လွှတ်ချလိုက်ပါ။)