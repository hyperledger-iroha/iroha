---
lang: my
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha ဥပမာများ ချိတ်ဆက်ပါ (Rust App/Wallet)

Torii node ဖြင့် Rust နမူနာနှစ်ခုကို အဆုံးမှအဆုံးထိ လုပ်ဆောင်ပါ။

လိုအပ်ချက်များ
- I18NI000000010X တွင် `connect` ပါသော Torii node
- Rust toolchain (တည်ငြိမ်)။
- `iroha-python` ပက်ကေ့ဂျ်နှင့်အတူ Python 3.9+ (အောက်ပါ CLI အကူအညီပေးသူအတွက်)။

ဥပမာများ
- အက်ပ်ဥပမာ- `crates/iroha_torii_shared/examples/connect_app.rs`
- Wallet ဥပမာ- `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI အကူအညီပေးသူ- `python -m iroha_python.examples.connect_flow`

စတင်ခြင်း၏အမိန့်
1) Terminal A — အပလီကေးရှင်း (sid + တိုကင်များကို ပရင့်ထုတ်ပြီး၊ WS ချိတ်ဆက်ပြီး၊ SignRequestTx ပေးပို့သည်)။

    ကုန်တင်ပြေး -p iroha_torii_shared --ဥပမာ ချိတ်ဆက်_အက်ပ် -- --node http://127.0.0.1:8080 --role အက်ပ်

   နမူနာ အထွက်-

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS ချိတ်ဆက်ထားသည်။
    အက်ပ်- SignRequestTx ပေးပို့ခဲ့သည်။
    (အဖြေကို စောင့်မျှော်နေပါသည်)

2) Terminal B — Wallet (token_wallet နှင့် ချိတ်ဆက်ပါ၊ SignResultOk ဖြင့် စာပြန်သည်)။

    ကုန်တင်ကုန်ချ run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   နမူနာ အထွက်-

    ပိုက်ဆံအိတ်- WS ချိတ်ဆက်ထားသည်။
    ပိုက်ဆံအိတ်- SignRequestTx len=3 seq 1 တွင်
    ပိုက်ဆံအိတ်- SignResultOk ပို့လိုက်သည်။

3) အက်ပ်ဂိတ်သည် ရလဒ်ကို ပရင့်ထုတ်သည်-

    အက်ပ်- SignResultOk algo=ed25519 sig=deadbeef ရရှိသည်။

  `connect_norito_decode_envelope_sign_result_alg` အကူအညီအသစ်ကို အသုံးပြုပါ (နှင့်
  ဤဖိုင်တွဲရှိ Swift/Kotlin ထုပ်ပိုးခြင်း) algorithm string ကိုရယူရန်၊
  payload ကို decoding လုပ်ခြင်း။

မှတ်စုများ
- ဥပမာများသည် `sid` မှ သရုပ်ပြရုပ်မြင်သံကြားများ ဆင်းသက်လာသောကြောင့် အက်ပ်/ပိုက်ဆံအိတ် အလိုအလျောက် အပြန်အလှန်လုပ်ဆောင်သည်။ ထုတ်လုပ်မှုတွင် အသုံးမပြုရ။
- SDK သည် AEAD AAD binding နှင့် seq-as-nonce ကို ပြဌာန်းသည် ။ အတည်ပြုပြီးနောက် ထိန်းချုပ်မှုဘောင်များကို ကုဒ်ဝှက်ထားသင့်သည်။
- Swift သုံးစွဲသူများအတွက်၊ `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` ကို ကြည့်ပါ၊ `make swift-ci` ဖြင့် တရားဝင်အောင်ပြုလုပ်ထားသောကြောင့် ဒက်ရှ်ဘုတ်တယ်လီမီတာစနစ်သည် Rust နမူနာများနှင့် လိုက်လျောညီထွေဖြစ်ပြီး Buildkite မက်တာဒေတာ (`ci/xcframework-smoke:<lane>:device_tag`) သည် နဂိုအတိုင်းရှိနေပါသည်။
- Python CLI အကူအသုံးပြုမှု-

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  CLI သည် ရိုက်နှိပ်ထားသော စက်ရှင်အချက်အလက်ကို ပရင့်ထုတ်ပြီး ချိတ်ဆက်မှုအခြေအနေ လျှပ်တစ်ပြက်ရိုက်ချက်အား စွန့်ပစ်ကာ Norito-ကုဒ်လုပ်ထားသော `ConnectControlOpen` ဖရိမ်ကို ထုတ်လွှတ်သည်။ ပေးဆောင်မှုအား Torii သို့ ပြန်တင်ရန် `--send-open` ကို ကျော်ဖြတ်ပါ၊ အကြမ်းဘိုက်များရေးရန် `--frame-output-format binary`၊ Base64-ဖော်ရွေသော JSON blob အတွက် `--frame-json-output`၊ နှင့် `--status-json-output` တို့ကို ရိုက်နှိပ်ရန် လိုအပ်သည့်အခါတွင် အလိုအလျောက်ရိုက်ထည့်ပါ။ `name`၊ `url` နှင့် `icon_hash` အကွက်များပါရှိသော `--app-metadata-file metadata.json` မှတဆင့် အပလီကေးရှင်းမက်တာဒေတာကို JSON ဖိုင်မှလည်း တင်နိုင်သည် (`python/iroha_python/src/iroha_python/examples/connect_app_metadata.json` ကိုကြည့်ပါ)။ `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` ဖြင့် အသစ်သော ပုံစံတစ်ခုကို ဖန်တီးပါ။ တယ်လီမီတာဖြင့်သာ လုပ်ဆောင်ခြင်းအတွက်၊ သင်သည် `--status-only` ဖြင့် စက်ရှင်ဖန်တီးမှုကို လုံးလုံးကျော်သွားနိုင်ပြီး `--status-json-output status.json` မှတစ်ဆင့် JSON ကို ရွေးချယ်နိုင်သည်။