---
lang: my
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% အကြောင်းအရာ Hosting လမ်းသွယ်
% Iroha Core

# အကြောင်းအရာ Hosting လမ်းသွယ်

အကြောင်းအရာလမ်းကြောသည် ကွင်းဆက်တွင် အငြိမ်အစုအဝေးငယ်များ (tar archives) များကို သိမ်းဆည်းထားပြီး ဆောင်ရွက်ပေးသည်။
Torii မှ တိုက်ရိုက်ဖိုင်တစ်ခုစီ။

- **ထုတ်ဝေခြင်း**- `PublishContentBundle` ကို ကတ္တရာစေးမှတ်တမ်း၊ ရွေးချယ်နိုင်သော သက်တမ်းကုန်ဆုံးမှုဖြင့် တင်သွင်းပါ။
  အမြင့်၊ နှင့် ရွေးချယ်နိုင်သော မန်နီးဖက်စ်။ bundle ID သည် blake2b hash ဖြစ်သည်
  တာဘော Tar ထည့်သွင်းမှုများသည် ပုံမှန်ဖိုင်များဖြစ်ရမည်။ အမည်များသည် ပုံမှန် UTF-8 လမ်းကြောင်းများဖြစ်သည်။
  အရွယ်အစား/လမ်းကြောင်း/ဖိုင်အရေအတွက် ထုပ်များသည် `content` config (`max_bundle_bytes`၊
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`)။
  Manifest များတွင် Norito-အညွှန်း hash၊ dataspace/lane၊ cache policy တို့ ပါဝင်သည်
  (`max_age_seconds`, `immutable`), auth mode (`public`/`role:<role>`/
  `sponsor:<uaid>`)၊ ထိန်းသိမ်းမှုမူဝါဒနေရာပေးသူနှင့် MIME တို့ကို အစားထိုးမှုများ။
- **Deduping**- tar payloads များကို အတုံးလိုက်အခဲလိုက် (ပုံသေ 64KiB) နှင့် တစ်ကြိမ်လျှင် သိမ်းဆည်းသည်
  အကိုးအကားအရေအတွက်များနှင့်အတူ hash; အစုအဝေးတစ်ခုဆုတ်ခွာခြင်းနှင့် prunes အတုံးများ။
- **ဝန်ဆောင်မှု**- Torii သည် `GET /v1/content/{bundle}/{path}` ကို ဖော်ထုတ်သည်။ တုံ့ပြန်မှုစီးကြောင်း
  `ETag` = ဖိုင် hash၊ `Accept-Ranges: bytes`၊
  အပိုင်းအခြားပံ့ပိုးမှု၊ နှင့် Cache-Control သည် မန်နီးဖက်စ်မှ ဆင်းသက်လာသည်။ ဂုဏ်ပြုဖတ်ရှုသည်။
  သက်သေပြခြင်းမုဒ်- အခန်းကဏ္ဍမှ ကန့်သတ်ထားသော နှင့် ပံ့ပိုးကူညီမှုဖြင့် ကန့်သတ်ထားသော တုံ့ပြန်မှုများသည် စံသတ်မှတ်ချက်များ လိုအပ်သည်။
  လက်မှတ်ရေးထိုးထားသော တောင်းဆိုချက် ခေါင်းစီးများ (`X-Iroha-Account`၊ `X-Iroha-Signature`)
  အကောင့်; ပျောက်ဆုံး/သက်တမ်းကုန် အစုအဝေးများ 404 ကို ပြန်ပေးသည်။
- **CLI**: `iroha content publish --bundle <path.tar>` (သို့မဟုတ် `--root <dir>`) ယခု
  မန်နီးဖက်စ်ကို အလိုအလျောက်ထုတ်ပေးပြီး ရွေးချယ်နိုင်သော `--manifest-out/--bundle-out` နှင့်
  `--auth`၊ `--cache-max-age-secs`၊ `--dataspace`၊ `--lane`၊ `--immutable`၊
  နှင့် `--expires-at-height` ကို အစားထိုးသည်။ `iroha content pack --root <dir>` တည်ဆောက်သည်။
  ဘာကိုမှ မတင်ပြဘဲ အဆုံးအဖြတ်ပေးတဲ့ tarball + manifest ။
- **Config**- Cache/auth knob များသည် `iroha_config` တွင် `content.*` အောက်တွင် နေထိုင်သည်
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`၊
  `default_auth_mode`) နှင့် ထုတ်ဝေချိန်၌ ပြဋ္ဌာန်းထားသည်။
- **SLO + ကန့်သတ်ချက်များ**: `content.max_requests_per_second` / `request_burst` နှင့်
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` ဖတ်ရန်ဦးထုပ်
  ဖြတ်သန်းမှု; Torii သည် ဘိုက်များနှင့် ပို့ကုန်များ မထမ်းဆောင်မီ နှစ်ခုစလုံးကို တွန်းအားပေးသည်။
  `torii_content_requests_total`၊ `torii_content_request_duration_seconds` နှင့်
  ရလဒ်တံဆိပ်များပါရှိသော `torii_content_response_bytes_total` မက်ထရစ်များ။ ငံနေချိန်
  ပစ်မှတ်များသည် `content.target_p50_latency_ms` / အောက်တွင် နေထိုင်သည်
  `content.target_p99_latency_ms`/`content.target_availability_bps`။
- **အလွဲသုံးစားလုပ်ထိန်းချုပ်မှုများ**- အဆင့်သတ်မှတ်ပုံးများကို UAID/API တိုကင်/အဝေးထိန်း IP ဖြင့် သော့ခတ်ထားပြီး၊
  ရွေးချယ်နိုင်သော PoW guard (`content.pow_difficulty_bits`, `content.pow_header`) လုပ်နိုင်သည်
  မဖတ်မီ လိုအပ်သည်။ DA stripe layout ပုံသေများမှ လာပါသည်။
  `content.stripe_layout` နှင့် ပြေစာများ/မန်နီးဖက်စ် ဟက်ရှ်များတွင် ပဲ့တင်ထပ်ပါသည်။
- **ပြေစာများနှင့် DA အထောက်အထားများ**- အောင်မြင်သော တုံ့ပြန်မှုများ ပူးတွဲပါရှိသည်။
  သယ်ဆောင်လာသော `sora-content-receipt` (base64 Norito-framed `ContentDaReceipt`)
  `bundle_id`၊ `path`၊ `file_hash`၊ `served_bytes`၊ ထမ်းဆောင်ခဲ့သော ဘိုက်အကွာအဝေး၊
  `chunk_root` / `stripe_layout`၊ ရွေးချယ်နိုင်သော PDP ကတိကဝတ်နှင့် အချိန်တံဆိပ်တစ်ခု ထို့ကြောင့်
  ဖောက်သည်များသည် ကိုယ်ကိုပြန်မဖတ်ဘဲ ရယူခဲ့သည့်အရာကို ပင်ထိုးနိုင်သည်။

အဓိက ကိုးကားချက်များ-ဒေတာမော်ဒယ်- `crates/iroha_data_model/src/content.rs`
- ကွပ်မျက်မှု- `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ကိုင်တွယ်သူ- `crates/iroha_torii/src/content.rs`
- CLI ကူညီသူ- `crates/iroha_cli/src/content.rs`