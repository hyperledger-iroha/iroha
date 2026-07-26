---
lang: my
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Kagami Iroha3 ပရိုဖိုင်များ

Kagami သည် Iroha 3 ကွန်ရက်များအတွက် ကြိုတင်သတ်မှတ်မှုများကို ပို့ဆောင်ပေးသောကြောင့် အော်ပရေတာများသည် အဆုံးအဖြတ်ကို တံဆိပ်တုံးထုနိုင်သည်
ဥပါဒ် သည် ကွန်ရက် တစ်ခု နှင့် တစ်ခု ခလုတ်များကို လှည့်ခြင်း မပြုဘဲ ထင်ရှားသည်။

- ပရိုဖိုင်များ- `iroha3-dev` (ကွင်းဆက် `iroha3-dev.local`၊ စုဆောင်းသူများ k=1 r=1၊ NPoS ကိုရွေးချယ်သည့်အခါ ကွင်းဆက် ID မှရရှိသော VRF မျိုးစေ့များ)၊ `iroha3-taira` (ကွင်းဆက် `iroha3-taira`၊ စုဆောင်းသူ = 3,300 လိုအပ်သည် NPoS ကိုရွေးချယ်သောအခါ `--vrf-seed-hex`)၊ `iroha3-nexus` (ကွင်းဆက် `iroha3-nexus`၊ စုဆောင်းသူများ k=5 r=3၊ NPoS ကိုရွေးချယ်သည့်အခါ `--vrf-seed-hex` လိုအပ်သည်)။
- အများသဘောတူချက်- Sora ပရိုဖိုင်ကွန်ရက်များ (Nexus + dataspaces) သည် NPoS လိုအပ်ပြီး အဆင့်ခွဲဖြတ်တောက်မှုများကို ခွင့်မပြုပါ။ ခွင့်ပြုထားသော Iroha3 ဖြန့်ကျက်မှုများကို Sora ပရိုဖိုင်မပါဘဲ လုပ်ဆောင်ရပါမည်။
မျိုးဆက်- `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`။ Nexus အတွက် `--consensus-mode npos` ကိုသုံးပါ။ `--vrf-seed-hex` သည် NPoS အတွက်သာ အကျုံးဝင်သည် (taira/nexus အတွက် လိုအပ်သည်)။ Kagami သည် Iroha3 လိုင်းတွင် DA/RBC ပင်ထိုးပြီး အနှစ်ချုပ် (ကွင်းဆက်၊ စုဆောင်းသူများ၊ DA/RBC၊ VRF မျိုးစေ့၊ လက်ဗွေ) ကို ထုတ်လွှတ်သည်။
- အတည်ပြုခြင်း- `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` သည် ပရိုဖိုင်မျှော်လင့်ချက်များကို ပြန်လည်ပြသသည် (ကွင်းဆက် ID၊ DA/RBC၊ စုဆောင်းသူများ၊ PoP လွှမ်းခြုံမှု၊ သဘောတူညီမှု လက်ဗွေ)။ taira/nexus အတွက် NPoS manifest ကိုစစ်ဆေးသောအခါမှသာ `--vrf-seed-hex` ကို ပံ့ပိုးပေးပါသည်။
- နမူနာအစုအဝေးများ- ကြိုတင်ထုတ်လုပ်ထားသောအတွဲများသည် `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json၊ config.toml၊ docker-compose.yml၊ verify.txt၊ README) အောက်တွင် နေထိုင်ပါသည်။ `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` ဖြင့် ပြန်ထုတ်ပါ။
- Mochi- `mochi`/`mochi-genesis` လက်ခံ `--genesis-profile <profile>` နှင့် `--vrf-seed-hex <hex>` (NPoS သီးသန့်)၊ ၎င်းတို့ကို Kagami သို့ ပေးပို့ပြီး Kagami တွင် အသုံးပြုထားသည့် တူညီသော Kagami မှ ပရိုဖိုင်ကို ထုတ်ယူသည့်အခါတွင် ပုံနှိပ်ပါ။

အစုအဝေးများသည် BLS PoPs များကို topology entries များနှင့်အတူ ထည့်သွင်းထားသောကြောင့် `kagami verify` အောင်မြင်သည်
box ထဲက; Local အတွက် လိုအပ်သလို configs ရှိ ယုံကြည်ရသော ရွယ်တူ/ဆိပ်ကမ်းများကို ချိန်ညှိပါ။
မီးခိုးထွက်တယ်။