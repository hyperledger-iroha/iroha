---
lang: my
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Norito လွှင့်ထုတ်ခြင်း။

Norito Streaming သည် ဝါယာကြိုးဖော်မတ်၊ ထိန်းချုပ်မှုဘောင်များနှင့် ရည်ညွှန်းကုဒ်ဒက်ကို သတ်မှတ်သည်
Torii နှင့် SoraNet တစ်လျှောက် တိုက်ရိုက်မီဒီယာစီးဆင်းမှုများအတွက် အသုံးပြုသည်။ Canonical spec သည် နေထိုင်သည်။
workspace root တွင် `norito_streaming.md`; ဤစာမျက်နှာသည် ထိုအပိုင်းအစများကို ခွဲထုတ်သည်။
အော်ပရေတာများနှင့် SDK စာရေးဆရာများသည် configuration touch point များနှင့်အတူ လိုအပ်ပါသည်။

## ဝိုင်ယာပုံစံနှင့် လေယာဉ်ထိန်းချုပ်မှု

- **ဖော်ပြချက်များနှင့်ဘောင်များ။** `ManifestV1` နှင့် `PrivacyRoute*` သည် အပိုင်းကို ဖော်ပြသည်
  အချိန်ဇယား၊ အပိုင်းအစများ ဖော်ပြချက်များနှင့် လမ်းကြောင်း အရိပ်အမြွက်များ။ ထိန်းချုပ်ဘောင်များ (`KeyUpdate`၊
  `ContentKeyUpdate`၊ နှင့် cadence တုံ့ပြန်ချက်) သည် manifest နှင့်အတူ နေထိုင်သည်
  ကုဒ်မုဒ်မထည့်မီ ကြည့်ရှုသူများသည် ကတိကဝတ်များကို အတည်ပြုနိုင်သည်။
- **အခြေခံကုဒ်ဒက်။** `BaselineEncoder`/`BaselineDecoder` မိုနိုတိုနစ်ကို အသုံးပြုရန်
  chunk ids၊ အချိန်တံဆိပ်တုံးဂဏန်းသင်္ချာနှင့် ကတိကဝတ်အတည်ပြုခြင်း အိမ်ရှင်တွေ ခေါ်ရမယ်။
  ကြည့်ရှုသူများ သို့မဟုတ် ထပ်ဆင့်များကို မထမ်းဆောင်မီ `EncodedSegment::verify_manifest`။
- ** အင်္ဂါရပ်များ။** ညှိနှိုင်းမှုစွမ်းရည် `streaming.feature_bits` ကြော်ငြာများ
  (မူရင်း `0b11` = အခြေခံလိုင်း တုံ့ပြန်ချက် + ကိုယ်ရေးကိုယ်တာ လမ်းကြောင်း ပံ့ပိုးပေးသူ) ထို့ကြောင့် တစ်ဆင့်ခံများကို လည်းကောင်း၊
  ဖောက်သည်များသည် ကိုက်ညီသောစွမ်းရည်များကို အဆုံးအဖြတ်မပေးဘဲ ရွယ်တူများကို ငြင်းပယ်နိုင်သည်။

## သော့များ၊ အစုံများနှင့် ကခုန်နှုန်းများ

- **Identity လိုအပ်ချက်။** Streaming ထိန်းချုပ်မှုဘောင်များဖြင့် အမြဲတမ်း လက်မှတ်ထိုးထားသည်။
  Ed25519။ သီးသန့်သော့များမှတဆင့် ပေးပို့နိုင်ပါသည်။
  `streaming.identity_public_key`/`streaming.identity_private_key`; မဟုတ်ရင်
  node အထောက်အထားကို ပြန်သုံးသည်။
- **HPKE suites.** `KeyUpdate` သည် အနိမ့်ဆုံး အသုံးများသော suites ကို ရွေးသည်၊ နံပါတ် ၁ ကတော့ suite ပါ။
  မဖြစ်မနေ (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`) တို့နဲ့၊
  ရွေးချယ်နိုင်သော `Kyber1024` အဆင့်မြှင့်တင်မှုလမ်းကြောင်း။ Suite Selection တွင် သိမ်းဆည်းထားသည်။
  session နှင့် update တိုင်းတွင် validated ။
- **လှည့်ခြင်း။** ထုတ်ဝေသူများသည် 64MiB သို့မဟုတ် 5 မိနစ်တိုင်းတွင် လက်မှတ်ရေးထိုးထားသော `KeyUpdate` ကို ထုတ်လွှတ်သည်။
  `key_counter` သည် တင်းကြပ်စွာ တိုးရမည်; ဆုတ်ယုတ်ခြင်းသည် ခက်ခဲသောအမှားတစ်ခုဖြစ်သည်။
  `ContentKeyUpdate` သည် လှိမ့်ထားသော အုပ်စုအကြောင်းအရာကီးကို ဖြန့်ဝေသည်၊၊
  ညှိနှိုင်းထားသော HPKE suite နှင့် ID + တရားဝင်မှုဖြင့် ဂိတ်များ အပိုင်းကို စာဝှက်ဖြည်ခြင်း။
  ပြတင်းပေါက်။
- **လျှပ်တစ်ပြက်များ။** `StreamingSession::snapshot_state` နှင့်
  `restore_from_snapshot` ဆက်နေသည် `{session_id၊ key_counter၊ suite၊ sts_root၊
  cadence အခြေအနေ}` under `streaming.session_store_dir` (မူရင်း
  `./storage/streaming`)။ သယ်ယူပို့ဆောင်ရေးသော့များကို ပြန်လည်ရယူသည့်အခါတွင် ပြန်လည်ရရှိသောကြောင့် ပျက်စီးသွားပါသည်။
  session လျှို့ဝှက်ချက်များကိုမပေါက်ကြားစေနှင့်။

## Runtime ဖွဲ့စည်းမှု

- **သော့ပစ္စည်း။** သီးသန့်သော့များဖြင့် ပေးဆောင်ပါ။
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519
  multihash) နှင့် ရွေးချယ်နိုင်သော Kyber ပစ္စည်းမှတဆင့်
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`။ လေးခုလုံး ဖြစ်ရမယ်။
  ပုံသေများကို ဦးစားပေးသောအခါတွင် ရှိနေသည်; `streaming.kyber_suite` လက်ခံသည်။
  `mlkem512|mlkem768|mlkem1024` (အမည်တူ `kyber512/768/1024`၊ မူရင်း
  `mlkem768`)။
- ** Codec guardrails.** တည်ဆောက်မှုအား ဖွင့်မပေးပါက CABAC သည် ဆက်လက်ပိတ်ထားမည်ဖြစ်သည်။
  စုစည်းထားသော rANS သည် `ENABLE_RANS_BUNDLES=1` လိုအပ်သည်။ တစ်ဆင့်ခံ ပြဋ္ဌာန်းပါ။
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` နှင့် ရွေးချယ်နိုင်သည်။
  စိတ်ကြိုက်စားပွဲများကို ထောက်ပံ့ပေးသည့်အခါ `streaming.codec.rans_tables_path`။ ထုပ်ပိုးထားသည်။
- **SoraNet လမ်းကြောင်းများ။** `streaming.soranet.*` သည် အမည်မသိ သယ်ယူပို့ဆောင်ရေးကို ထိန်းချုပ်သည်-
  `exit_multiaddr` (မူလ `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (မူလ 25ms), `access_kind` (`authenticated` vs `read-only`)၊ ချန်လှပ်ထားနိုင်သည်
  `channel_salt`၊ `provision_spool_dir` (မူရင်း
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (မူရင်း 0၊
  အကန့်အသတ်မရှိ)၊ `provision_window_segments` (မူရင်း 4) နှင့်
  `provision_queue_capacity` (မူရင်း 256)။
- **စင့်ခ်တံခါး။** `streaming.sync` သည် ရုပ်မြင်သံကြားအတွက် ပျံ့လွင့်မှုကို ခလုတ်ဖွင့်ပေးသည်
  စီးကြောင်းများ- `enabled`၊ `observe_only`၊ `ewma_threshold_ms` နှင့် `hard_cap_ms`
  အချိန်ကိုက်ပျံ့လွင့်မှုအတွက် အပိုင်းများကို ငြင်းပယ်သည့်အခါ အုပ်ချုပ်သည်။

## အတည်ပြုခြင်းနှင့် ပြင်ဆင်မှုများ

- Canonical အမျိုးအစား အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် အကူအညီပေးသူများ နေထိုင်ပါသည်။
  `crates/iroha_crypto/src/streaming.rs`။
- Integration coverage သည် HPKE ကို လက်ဆွဲနှုတ်ဆက်ခြင်း၊ အကြောင်းအရာ-သော့ဖြန့်ဖြူးခြင်းကို လေ့ကျင့်ခန်းလုပ်သည်၊
  နှင့် လျှပ်တစ်ပြက် ဘဝသံသရာ (`crates/iroha_crypto/tests/streaming_handshake.rs`)။
  တိုက်ရိုက်ထုတ်လွှင့်မှုကို အတည်ပြုရန် `cargo test -p iroha_crypto streaming_handshake` ကိုဖွင့်ပါ။
  ဒေသအလိုက် ပေါ်လာသည်။
- အပြင်အဆင်၊ အမှားအယွင်း ကိုင်တွယ်မှုနှင့် အနာဂတ် အဆင့်မြှင့်တင်မှုများကို နက်ရှိုင်းစွာ စေ့စေ့တွေးကြည့်ရန်၊ ဖတ်ရှုပါ။
  repository root ရှိ `norito_streaming.md`။