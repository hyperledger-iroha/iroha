---
lang: my
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM Configuration ပြောင်းရွှေ့ခြင်း။

# SM ဖွဲ့စည်းမှုပုံစံ ပြောင်းရွှေ့ခြင်း။

SM2/SM3/SM4 အင်္ဂါရပ်အစုံကို ထုတ်ပေးခြင်းသည် စုစည်းမှုထက် ပိုမိုလိုအပ်ပါသည်။
`sm` လုပ်ဆောင်ချက် အလံ။ Nodes များသည် အလွှာ၏နောက်ကွယ်တွင် လုပ်ဆောင်နိုင်စွမ်းကို ဂိတ်ပေါက်သည်။
`iroha_config` ပရိုဖိုင်များ နှင့် ဥပါဒ် သရုပ်ကို တထပ်တည်း သယ်ဆောင်ရန် မျှော်လင့်ပါသည်။
ပုံသေ။ ဤမှတ်စုသည် အကြံပြုထားသည့် လုပ်ငန်းအသွားအလာကို မြှင့်တင်သည့်အခါ ဖမ်းယူပါသည်။
“Ed25519-only” မှ “SM-enabled” အထိ ရှိပြီးသားကွန်ရက်။

## 1. Build Profile ကို အတည်ပြုပါ။

- binaries များကို `--features sm` ဖြင့် စုစည်းပါ။ သင်ရှိမှသာ `sm-ffi-openssl` ကိုထည့်ပါ။
  OpenSSL/Tongsuo အစမ်းကြည့်လမ်းကြောင်းကို ကျင့်သုံးရန် စီစဉ်ပါ။ `sm` မပါဘဲ တည်ဆောက်သည်။
  config ကို ဖွင့်ထားသော်လည်း ဝင်ခွင့်လက်ခံစဉ်အတွင်း `sm2` လက်မှတ်များကို ငြင်းပယ်ခြင်း
  သူတို့ကို။
- CI သည် `sm` ပစ္စည်းများ ထုတ်ဝေပြီး အတည်ပြုခြင်း အဆင့်များအားလုံးကို အတည်ပြုသည် (`ကုန်တင်ကုန်ချ၊
  စမ်းသပ်မှု -p iroha_crypto --features sm`၊ ပေါင်းစည်းမှု ကိရိယာများ၊ fuzz suites) ကျော်
  သင်အသုံးပြုရန် ရည်ရွယ်ထားသည့် အတိအကျ binaries များပေါ်တွင်

## 2. Layer Configuration Overrides

`iroha_config` သည် အဆင့်သုံးဆင့် သက်ရောက်သည်- `defaults` → `user` → `actual`။ SM ကို ပို့ပါ။
အော်ပရေတာများသည် တရားဝင်သူများထံ ဖြန့်ဝေပေးသည့် `actual` ပရိုဖိုင်တွင် အစားထိုးမှုများ၊
`user` ကို Ed25519-only တွင်ထားခဲ့ပါ ထို့ကြောင့် developer ပုံသေများမှာ မပြောင်းလဲပါ။

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

တူညီသောဘလောက်ကို `kagami ဥပါဒ်မှတစ်ဆင့် `defaults/genesis` မန်နီးဖက်စ်သို့ ကူးယူပါ
လိုအပ်ပါက …` (add `--ခွင့်ပြုထားသော-လက်မှတ်ရေးထိုးခြင်း sm2 --default-hash sm3-256` ကို ထုတ်လုပ်ပါ
overrides) ထို့ကြောင့် `parameters` ပိတ်ဆို့ပြီး ထိုးသွင်းထားသော မက်တာဒေတာသည် ၎င်းနှင့်သဘောတူသည်။
runtime ဖွဲ့စည်းမှု။ သရုပ်ဖော်ခြင်းနှင့် config လုပ်သည့်အခါတွင် ရွယ်တူများသည် ငြင်းဆန်ကြသည်။
လျှပ်တစ်ပြက်ပုံများ ကွဲပြားသည်။

## 3. Genesis Manifests ကို ပြန်ထုတ်ပါ။

- တစ်ခုချင်းစီအတွက် `kagami genesis generate --consensus-mode <mode>` ကို run ပါ။
  ပတ်ဝန်းကျင်ကို TOML အစားထိုးမှုများနှင့်အတူ အပ်ဒိတ်လုပ်ထားသော JSON ကို ထည့်သွင်းပါ။
- မန်နီးဖက်စ် (`kagami genesis sign …`) ကို လက်မှတ်ရေးထိုးပြီး `.nrt` ပေးချေမှုအား ဖြန့်ဝေပါ။
  လက်မှတ်မထိုးထားသော JSON မန်နီးဖက်စ်မှ bootstrap ပါသော Node များသည် runtime crypto ကို ဆင်းသက်လာသည်။
  ဖိုင်မှ တိုက်ရိုက်ဖွဲ့စည်းပုံ- တူညီသော ကိုက်ညီမှုရှိနေဆဲဖြစ်သည်။
  စစ်ဆေးမှုများ။

## 4. Traffic မဖြစ်မီ အတည်ပြုပါ။

- binaries အသစ်များနှင့် config ဖြင့် အဆင့်သတ်မှတ်ထားသော အစုအဝေးတစ်ခုကို ပံ့ပိုးပေးပြီး အတည်ပြုပါ-
  - `/status` သည် `crypto.sm_helpers_available = true` ကို ရွယ်တူများ ပြန်လည်စတင်သည်နှင့် ဖော်ထုတ်သည်။
  - Torii ဝင်ခွင့်သည် SM2 လက်မှတ်များကို ငြင်းပယ်ဆဲဖြစ်ပြီး `sm2` တွင် မပါရှိပါ။
    `allowed_signing` နှင့် ရောစပ်ထားသော Ed25519/SM2 အတွဲများကို လက်ခံသည်
    algorithms နှစ်ခုလုံးပါဝင်သည်။
  - `iroha_cli tools crypto sm2 export …` အသွားအပြန် သော့ထွက်ပစ္စည်းအသစ်မှတဆင့် အမျိုးအနွယ်
    ပုံသေ။
- SM2 အဆုံးအဖြတ်ပေးသော လက်မှတ်များနှင့် အကျုံးဝင်သော ပေါင်းစပ်မီးခိုးအက္ခရာများကို လုပ်ဆောင်ပါ။
  host/VM ညီညွတ်မှုကို အတည်ပြုရန် SM3 ကို ဟတ်ချနေသည်။

## 5. Rollback Plan- ပြောင်းပြန်လှန်ခြင်းကို မှတ်တမ်းတင်ပါ- `sm2` ကို `allowed_signing` မှ ဖယ်ရှားပြီး ပြန်လည်ရယူပါ။
  `default_hash = "blake2b-256"`။ တူညီသော `actual` မှတဆင့်ပြောင်းလဲမှုကိုတွန်းပါ။
  profile pipeline သည် validator တိုင်းသည် monotonically လှန်သည်။
- SM manifests များကို disk တွင်ထားပါ။ မကိုက်ညီသော config နှင့် genesis ကိုမြင်သည့်ရွယ်တူများ
  တစ်စိတ်တစ်ပိုင်းပြန်လှည့်ခြင်းမှကာကွယ်ပေးသောဒေတာစတင်ရန်ငြင်းဆန်သည်။
- OpenSSL/Tongsuo အစမ်းကြည့်ရှုမှုတွင် ပါဝင်ပါက၊ ပိတ်ခြင်းအတွက် အဆင့်များကို ထည့်သွင်းပါ။
  `crypto.enable_sm_openssl_preview` နှင့် မျှဝေထားသော အရာဝတ္ထုများမှ ဖယ်ရှားခြင်း။
  runtime ပတ်ဝန်းကျင်။

## ရည်ညွှန်းပစ္စည်း

- [`docs/genesis.md`](../../genesis.md) - ဥပါဒ်ထင်ရှားခြင်း၏ဖွဲ့စည်းပုံနှင့်
  `crypto` ဘလောက်
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  `iroha_config` ကဏ္ဍများနှင့် မူရင်းများ ခြုံငုံသုံးသပ်ချက်။
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – အဆုံးထိ
  SM ကုဒ်ဝှက်ခြင်း ပို့ဆောင်ခြင်းအတွက် အဆုံးအော်ပရေတာ စစ်ဆေးချက်စာရင်း။