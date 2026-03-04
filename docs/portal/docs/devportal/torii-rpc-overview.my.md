---
lang: my
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
translator: machine-google-reviewed
---

#Norito-RPC ခြုံငုံသုံးသပ်ချက်

Norito-RPC သည် Torii APIs အတွက် ဒွိသယ်ယူပို့ဆောင်ရေးဖြစ်သည်။ ၎င်းသည် တူညီသော HTTP လမ်းကြောင်းများကို ပြန်လည်အသုံးပြုသည်။
`/v1/pipeline` အနေဖြင့်၊ သို့သော် schema ပါဝင်သော Norito-framed payload များကို လဲလှယ်သည်
hashes နှင့် checksums များ။ တိကျသေချာသော၊ အတည်ပြုထားသော တုံ့ပြန်မှုများ လိုအပ်သည့်အခါ သို့မဟုတ် ၎င်းကို အသုံးပြုပါ။
ပိုက်လိုင်း JSON တုံ့ပြန်မှုများသည် ပိတ်ဆို့မှုဖြစ်လာသောအခါ။

##ဘာလို့ပြောင်းတာလဲ။
- CRC64 ဖြင့် အဆုံးအဖြတ်ပေးသောဘောင်နှင့် schema hash များသည် ကုဒ်ဖော်ပြခြင်းအမှားများကို လျှော့ချပေးသည်။
- SDKs တစ်လျှောက်တွင် မျှဝေထားသော Norito အကူအညီပေးသူများသည် သင့်အား လက်ရှိဒေတာမော်ဒယ်အမျိုးအစားများကို ပြန်လည်အသုံးပြုနိုင်စေပါသည်။
- Torii သည် Norito ဆက်ရှင်များကို တယ်လီမက်ထရီတွင် တဂ်ထားပြီးဖြစ်သောကြောင့် အော်ပရေတာများက စောင့်ကြည့်နိုင်သည်
ပေးထားသော ဒက်ရှ်ဘုတ်များဖြင့် မွေးစားခြင်း။

## တောင်းဆိုချက်တစ်ခု ပြုလုပ်ခြင်း။

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. သင်၏ payload ကို Norito codec (`iroha_client`၊ SDK ကူညီသူများ၊ သို့မဟုတ်
   `norito::to_bytes`)။
2. `Content-Type: application/x-norito` ဖြင့် တောင်းဆိုချက်ကို ပေးပို့ပါ။
3. `Accept: application/x-norito` ကို အသုံးပြု၍ Norito တုံ့ပြန်မှုကို တောင်းဆိုပါ။
4. ကိုက်ညီသော SDK အကူအညီပေးသူကို အသုံးပြု၍ တုံ့ပြန်မှုကို ကုဒ်လုပ်ပါ။

SDK ၏ သီးခြားလမ်းညွှန်ချက်-
- **Rust**: `iroha_client::Client` သည် Norito ကို သင်သတ်မှတ်သောအခါ အလိုအလျောက်ညှိနှိုင်းသည်
  `Accept` ခေါင်းစီး။
- **Python**- `iroha_python.norito_rpc` မှ `NoritoRpcClient` ကို အသုံးပြုပါ။
- **Android**- `NoritoRpcClient` နှင့် `NoritoRpcRequestOptions` ကို အသုံးပြုပါ။
  Android SDK
- **JavaScript/Swift**- အကူအညီပေးသူများကို `docs/source/torii/norito_rpc_tracker.md` တွင် ခြေရာခံထားသည်
  NRPC-3 ၏ အစိတ်အပိုင်းအဖြစ် ဆင်းသက်မည်ဖြစ်သည်။

## အဲဒါကို console နမူနာကို စမ်းကြည့်ပါ။

ဆော့ဖ်ဝဲရေးသားသူပေါ်တယ်သည် Try It ပရောက်စီကို ပို့ဆောင်ပေးသောကြောင့် သုံးသပ်သူများသည် Norito ကို ပြန်လည်ပြသနိုင်သည်
စိတ်ကြိုက် script များမရေးဘဲ payload များ။

1. [ပရောက်စီကို စတင်ပါ](./try-it.md#start-the-proxy-locally) နှင့် သတ်မှတ်ပါ။
   `TRYIT_PROXY_PUBLIC_URL` ထို့ကြောင့် ဝစ်ဂျက်များသည် အသွားအလာပေးပို့ရမည့်နေရာကို သိသည်။
2. ဤစာမျက်နှာရှိ **စမ်းသုံးကြည့်ပါ** ကတ် သို့မဟုတ် `/reference/torii-swagger` ကိုဖွင့်ပါ။
   အကန့်နှင့် `POST /v1/pipeline/submit` ကဲ့သို့သော အဆုံးမှတ်တစ်ခုကို ရွေးပါ။
3. **Content-Type** ကို `application/x-norito` သို့ပြောင်းပါ၊ **Binary** ကို ရွေးပါ။
   တည်းဖြတ်ပြီး `fixtures/norito_rpc/transfer_asset.norito` ကို အပ်လုဒ်လုပ်ပါ။
   (သို့) တွင်ဖော်ပြထားသော payload တစ်ခုခု
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`)။
4. OAuth ကိရိယာ-ကုဒ်ဝစ်ဂျက် သို့မဟုတ် လက်စွဲပါ တိုကင်မှတစ်ဆင့် ကိုင်ဆောင်သူ တိုကင်တစ်ခု ပေးပါ။
   အကွက် (proxy သည် `X-TryIt-Auth` ဖြင့် configure လုပ်သောအခါ overrides လက်ခံသည်
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`)။
5. တောင်းဆိုချက်ကို တင်သွင်းပြီး Torii တွင်ဖော်ပြထားသော `schema_hash` ကို သံယောင်လိုက်ကြောင်း အတည်ပြုပါ
   `fixtures/norito_rpc/schema_hashes.json`။ ကိုက်ညီသော hashe များသည် အတည်ပြုသည်။
   Norito ခေါင်းစီးသည် browser/proxy hop မှလွတ်မြောက်ခဲ့သည်။

လမ်းပြမြေပုံအထောက်အထားအတွက်၊ Try It ဖန်သားပြင်ဓာတ်ပုံကို အပြေးတစ်ခုနှင့် တွဲချိတ်ပါ။
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`။ ဇာတ်ညွှန်းတွေဖြစ်ပါတယ်။
`cargo xtask norito-rpc-verify` မှ JSON အကျဉ်းချုပ်ကို ရေးသည်။
`artifacts/norito_rpc/<timestamp>/` နှင့် တူညီသော ပွဲစဉ်များကို ဖမ်းယူသည်။
portal လောင်တယ်။

## ပြဿနာဖြေရှင်းခြင်း။

| ရောဂါလက္ခဏာ | ပေါ်လာသည့်နေရာ | ဖြစ်ဖွယ်ရှိ | ပြင်ရန် |
| ---| ---| ---| ---|
| `415 Unsupported Media Type` | Torii တုံ့ပြန်မှု | `Content-Type` ခေါင်းစီး | ပျောက်နေသည် သို့မဟုတ် မမှန်ပါ။ payload ကိုမပို့မီ `Content-Type: application/x-norito` ကို သတ်မှတ်ပါ။ |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii တုံ့ပြန်မှုကိုယ်ထည်/ခေါင်းစီးများ | Fixture schema hash သည် Torii build နှင့် ကွဲပြားသည် | `cargo xtask norito-rpc-fixtures` ဖြင့် ပြင်ဆင်ပြီး `fixtures/norito_rpc/schema_hashes.json` တွင် hash ကို အတည်ပြုပါ။ အဆုံးမှတ်သည် Norito ကို မဖွင့်ရသေးပါက JSON သို့ ပြန်သွားပါ။ |
| `{"error":"origin_forbidden"}` (HTTP 403) | Proxy တုံ့ပြန်မှု | တောင်းဆိုချက်သည် `TRYIT_PROXY_ALLOWED_ORIGINS` | တွင်ဖော်ပြထားခြင်းမရှိသော မူရင်းမှလာပါသည်။ ပေါ်တယ်မူရင်း (ဥပမာ၊ `https://docs.devnet.sora.example`) ကို env var တွင်ထည့်ကာ proxy ကို ပြန်လည်စတင်ပါ။ |
| `{"error":"rate_limited"}` (HTTP 429) | Proxy တုံ့ပြန်မှု | Per-IP ခွဲတမ်းသည် `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` ဘတ်ဂျက်ကို ကျော်လွန်သွားသည် | အတွင်းဝန်စမ်းသပ်ခြင်းအတွက် ကန့်သတ်ချက်ကို တိုးမြှင့်ပါ သို့မဟုတ် ဝင်းဒိုးပြန်လည်သတ်မှတ်သည်အထိ စောင့်ပါ (JSON တုံ့ပြန်မှုတွင် `retryAfterMs` ကိုကြည့်ပါ)။ |
| `{"error":"upstream_timeout"}` (HTTP 504) သို့မဟုတ် `{"error":"upstream_error"}` (HTTP 502) | Proxy တုံ့ပြန်မှု | Torii အချိန်ကုန်သွားသည် သို့မဟုတ် proxy သည် configured backend သို့ မရောက်ရှိနိုင် | `TRYIT_PROXY_TARGET` ကို ရရှိနိုင်ကြောင်း အတည်ပြုပါ၊ Torii ကျန်းမာရေးကို စစ်ဆေးပါ သို့မဟုတ် ပိုကြီးသော `TRYIT_PROXY_TIMEOUT_MS` ဖြင့် ထပ်စမ်းကြည့်ပါ။ |

နောက်ထပ် စမ်းသပ်ကြည့်ပါက ရောဂါရှာဖွေမှုများနှင့် OAuth အကြံပြုချက်များ တိုက်ရိုက်ပါဝင်ပါသည်။
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples)။

## နောက်ထပ်အရင်းအမြစ်များ
- သယ်ယူပို့ဆောင်ရေး RFC: `docs/source/torii/norito_rpc.md`
- အမှုဆောင်အနှစ်ချုပ်- `docs/source/torii/norito_rpc_brief.md`
- လုပ်ဆောင်ချက် ခြေရာခံ- `docs/source/torii/norito_rpc_tracker.md`
- Try-It proxy ညွှန်ကြားချက်များ- `docs/portal/docs/devportal/try-it.md`