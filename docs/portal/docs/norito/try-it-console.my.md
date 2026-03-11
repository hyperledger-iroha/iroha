---
lang: my
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5246118a539e2031dcafb8cf384ac7d20b8abc28b67ee1555e1b1211779fe390
source_last_modified: "2026-01-22T16:26:46.508367+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
---

Portal သည် Torii သို့အသွားအလာကိုပြန်ပို့သည့်အပြန်အလှန်အကျိုးသက်ရောက်သောမျက်နှာပြင်သုံးခုကိုစုစည်းထားသည်။

- `/reference/torii-swagger` တွင် **Swagger UI** သည် လက်မှတ်ရေးထိုးထားသော OpenAPI spec ကို ထုတ်ပေးပြီး `TRYIT_PROXY_PUBLIC_URL` ကို သတ်မှတ်သောအခါ ပရောက်စီမှတစ်ဆင့် တောင်းဆိုချက်များကို အလိုအလျောက် ပြန်လည်ရေးသားပါသည်။
- `/reference/torii-rapidoc` တွင် **RapiDoc** သည် `application/x-norito` အတွက် ကောင်းမွန်စွာအလုပ်လုပ်နိုင်သော ဖိုင်အပ်လုဒ်များနှင့် အကြောင်းအရာအမျိုးအစားရွေးချယ်ပေးသည့်စနစ်ဖြင့် တူညီသောအစီအစဥ်ကို ဖော်ထုတ်သည်။
- **စမ်းသုံးကြည့်ပါ sandbox** တွင် Norito ခြုံငုံသုံးသပ်ချက် စာမျက်နှာတွင် ad-hoc REST တောင်းဆိုမှုများနှင့် OAuth-device logins အတွက် ပေါ့ပါးသောပုံစံကို ပေးပါသည်။

ဝစ်ဂျက်သုံးခုစလုံးသည် ဒေသတွင်း **Try-It proxy** (`docs/portal/scripts/tryit-proxy.mjs`) သို့ တောင်းဆိုမှုများ ပေးပို့သည်။ ပရောက်စီသည် `static/openapi/torii.json` တွင် `static/openapi/manifest.json` တွင် ရေးထိုးထားသော အချေအတင်နှင့် ကိုက်ညီကြောင်း အတည်ပြုပြီး နှုန်းကန့်သတ်ချက်ကို တွန်းအားပေးသည်၊ မှတ်တမ်းများတွင် `X-TryIt-Auth` ခေါင်းစီးများကို ပြန်လည်ပြင်ဆင်ပြီး အထက်ရေစီးကြောင်းခေါ်ဆိုမှုတိုင်းကို `X-TryIt-Client` ဖြင့် တဂ်ပေးပါသည်။ သို့မှသာ I1800 ၏ အရင်းအမြစ်များ အသုံးပြုနိုင်မည်ဖြစ်သည်။

## ပရောက်စီကိုဖွင့်ပါ။

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` သည် သင်လေ့ကျင့်လိုသော Torii အခြေခံ URL ဖြစ်သည်။
- `TRYIT_PROXY_ALLOWED_ORIGINS` တွင် ကွန်ဆိုးလ်ကို ထည့်သွင်းသင့်သော ပေါ်တယ်မူရင်းတိုင်း (ဒေသခံ dev server၊ ထုတ်လုပ်ရေး hostname၊ preview URL) ပါဝင်ရပါမည်။
- `TRYIT_PROXY_PUBLIC_URL` ကို `docusaurus.config.js` ဖြင့် စားသုံးပြီး `customFields.tryIt` မှတစ်ဆင့် ဝစ်ဂျက်များအတွင်းသို့ ထိုးသွင်းသည်။
- `TRYIT_PROXY_BEARER` သည် `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` တွင်သာဖွင့်သည်။ သို့မဟုတ်ပါက အသုံးပြုသူများသည် ကွန်ဆိုးလ် သို့မဟုတ် OAuth စက်စီးဆင်းမှုမှတစ်ဆင့် ၎င်းတို့၏ကိုယ်ပိုင်တိုကင်ကို ပေးဆောင်ရမည်ဖြစ်သည်။
- `TRYIT_PROXY_CLIENT_ID` သည် တောင်းဆိုမှုတိုင်းတွင် ယူဆောင်လာသော `X-TryIt-Client` ကို သတ်မှတ်ပေးသည်။
  ဘရောင်ဇာမှ `X-TryIt-Client` ကို ထောက်ပံ့ပေးခြင်းကို ခွင့်ပြုသော်လည်း တန်ဖိုးများကို ဖြတ်တောက်ထားသည်
  ထိန်းချုပ်ဇာတ်ကောင်များပါ၀င်ပါက ငြင်းပယ်ပါ။

စတင်ချိန်တွင် proxy သည် `verifySpecDigest` ကို run ပြီး manifest ပျက်နေပါက ပြန်လည်ပြင်ဆင်ခြင်း အရိပ်အမြွက်ဖြင့် ထွက်သည်။ နောက်ဆုံးပေါ် Torii သတ်မှတ်ချက်ကို ဒေါင်းလုဒ်လုပ်ရန် သို့မဟုတ် အရေးပေါ်အခြေအနေများအတွက် `TRYIT_PROXY_ALLOW_STALE_SPEC=1` ကို ကျော်ဖြတ်ရန် `npm run sync-openapi -- --latest` ကိုဖွင့်ပါ။

ပတ်ဝန်းကျင်ဖိုင်များကို လက်ဖြင့်မတည်းဖြတ်ဘဲ ပရောက်စီပစ်မှတ်ကို အပ်ဒိတ် သို့မဟုတ် ပြန်လှည့်ရန်၊ အကူအညီပေးသူကို အသုံးပြုပါ-

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## ဝစ်ဂျက်များကို ကြိုးတပ်ပါ။

ပရောက်စီကို နားထောင်ပြီးနောက် ပေါ်တယ်ကို ဝန်ဆောင်မှုပေးသည်-

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` သည် အောက်ပါ ခလုတ်များကို ဖော်ထုတ်သည်-

| ပြောင်းလဲနိုင်သော | ရည်ရွယ်ချက် |
| ---| ---|
| `TRYIT_PROXY_PUBLIC_URL` | URL ကို Swagger၊ RapiDoc နှင့် Try it sandbox ထဲသို့ ထိုးသွင်းလိုက်ပါသည်။ ခွင့်ပြုချက်မရှိဘဲ အစမ်းကြည့်ရှုမှုများအတွင်း ဝစ်ဂျက်များကို ဝှက်ထားရန် သတ်မှတ်မထားပါ။ |
| `TRYIT_PROXY_DEFAULT_BEARER` | မမ်မိုရီတွင် သိမ်းဆည်းထားသည့် ရွေးချယ်နိုင်သော မူရင်းတိုကင်။ `DOCS_SECURITY_ALLOW_INSECURE=1` ကို စက်တွင်းတွင် သင်မကျော်ပါက `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` နှင့် HTTPS သီးသန့် CSP အစောင့် (DOCS-1b) လိုအပ်ပါသည်။ |
| `DOCS_OAUTH_*` | သုံးသပ်သူများသည် ပေါ်တယ်မှမထွက်ခွာဘဲ ခဏတာတိုကင်များကို ထုတ်နှုတ်နိုင်စေရန် OAuth စက်ပစ္စည်းစီးဆင်းမှု (`OAuthDeviceLogin` အစိတ်အပိုင်း) ကိုဖွင့်ပါ။ |

OAuth variable များရှိနေသောအခါ sandbox သည် ပြင်ဆင်ထားသော Auth ဆာဗာမှတဆင့်သွားသော ** စက်ကုဒ်ဖြင့် အကောင့်ဝင်ရန်** ခလုတ်တစ်ခု (ပုံသဏ္ဍာန်အတိအကျအတွက် `config/security-helpers.js` ကိုကြည့်ပါ)။ ကိရိယာစီးဆင်းမှုမှတဆင့်ထုတ်ပေးသည့်တိုကင်များကို ဘရောက်ဆာစက်ရှင်တွင်သာ သိမ်းဆည်းထားသည်။

## Norito-RPC payload များ ပို့ခြင်း။

1. [Norito အမြန်စတင်ခြင်း](./quickstart.md) တွင် ဖော်ပြထားသော CLI သို့မဟုတ် အတိုအထွာများဖြင့် `.norito` ပေးချေမှုတစ်ခုကို တည်ဆောက်ပါ။ ပရောက်စီသည် `application/x-norito` ကောင်များကို မပြောင်းလဲဘဲ ပေးပို့သောကြောင့် `curl` ဖြင့် သင်တင်မည့် အလားတူပစ္စည်းကို ပြန်သုံးနိုင်သည်။
2. `/reference/torii-rapidoc` (binary payloads အတွက် ပိုနှစ်သက်သည်) သို့မဟုတ် `/reference/torii-swagger` ကိုဖွင့်ပါ။
3. drop-down မှ လိုချင်သော Torii လျှပ်တစ်ပြက်ရိုက်ချက်ကို ရွေးပါ။ လျှပ်တစ်ပြက်များကို လက်မှတ်ရေးထိုးထားသည်။ အကန့်သည် `static/openapi/manifest.json` တွင်မှတ်တမ်းတင်ထားသော manifest digest ကိုပြသသည်။
4. "စမ်းကြည့်ပါ" အံဆွဲရှိ `application/x-norito` အကြောင်းအရာအမျိုးအစားကို ရွေးပါ၊ **ဖိုင်ကိုရွေးချယ်ပါ** ကိုနှိပ်ပါ၊ နှင့် သင်၏ပေးချေမှုအား ရွေးချယ်ပါ။ ပရောက်စီသည် တောင်းဆိုချက်ကို `/proxy/v1/pipeline/submit` သို့ ပြန်ရေးပြီး `X-TryIt-Client=docs-portal-rapidoc` ဖြင့် တဂ်ပေးသည်။
5. Norito တုံ့ပြန်မှုများကို ဒေါင်းလုဒ်လုပ်ရန်၊ `Accept: application/x-norito` ကို သတ်မှတ်ပါ။ Swagger/RapiDoc သည် header selector ကို တူညီသောအံဆွဲတွင် ဖော်ထုတ်ပြီး binary ကို proxy မှတဆင့် ပြန်လွှင့်ပါ။

ထည့်သွင်းထားသော JSON သီးသန့်လမ်းကြောင်းများအတွက် Try it sandbox သည် မကြာခဏ ပိုမြန်သည်- လမ်းကြောင်း (ဥပမာ၊ `/v1/accounts/i105.../assets`) ကို ရိုက်ထည့်ပါ၊ HTTP နည်းလမ်းကို ရွေးချယ်ပါ၊ လိုအပ်သည့်အခါတွင် JSON စာကိုယ်ကို ကူးထည့်ကာ ခေါင်းစီးများ၊ ကြာချိန်နှင့် ပေးဆောင်မှုများကို စစ်ဆေးရန် **Send Request** ကို နှိပ်ပါ။

## ပြဿနာဖြေရှင်းခြင်း။

| ရောဂါလက္ခဏာ | ဖြစ်ဖွယ်ရှိ | ကုစား |
| ---| ---| ---|
| ဘရောင်ဇာကွန်ဆိုးလ်သည် CORS အမှားများကို ပြသသည် သို့မဟုတ် ပရောက်စီ URL ပျောက်ဆုံးနေကြောင်း sandbox မှ သတိပေးသည်။ | ပရောက်စီသည် အလုပ်မလုပ်ပါ သို့မဟုတ် မူရင်းကို တရားဝင်စာရင်းသွင်းမထားပါ။ | ပရောက်စီကို စတင်ပါ၊ `TRYIT_PROXY_ALLOWED_ORIGINS` သည် သင့် portal host ကို ဖုံးအုပ်ထားကြောင်း သေချာစေပြီး `npm run start` ကို ပြန်လည်စတင်ပါ။ |
| `npm run tryit-proxy` သည် "ဒိုင်းရှင်းမတူညီမှု" ဖြင့် ထွက်သည်။ | Torii OpenAPI အစုအဝေးသည် အထက်ပိုင်းသို့ ပြောင်းလဲသွားသည်။ | `npm run sync-openapi -- --latest` (သို့မဟုတ် `--version=<tag>`) ကိုဖွင့်ပြီး ထပ်စမ်းကြည့်ပါ။ |
| Widgets သည် `401` သို့မဟုတ် `403` သို့ ပြန်သွားသည်။ | တိုကင်ပျောက်နေခြင်း၊ သက်တမ်းကုန်သွားခြင်း သို့မဟုတ် အတိုင်းအတာများ မလုံလောက်ခြင်း။ | OAuth စက်ပစ္စည်းစီးဆင်းမှုကို အသုံးပြုပါ သို့မဟုတ် တရားဝင်ကိုင်ဆောင်သူ တိုကင်ကို sandbox ထဲသို့ ကူးထည့်ပါ။ အငြိမ်တိုကင်များအတွက် `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ကို တင်ပို့ရပါမည်။ |
| ပရောက်စီမှ `429 Too Many Requests`။ | Per-IP နှုန်းကန့်သတ်ချက်ကို ကျော်သွားပါပြီ။ | ယုံကြည်ရသောပတ်ဝန်းကျင်များ သို့မဟုတ် အခိုးအငွေ့စမ်းသပ်မှုစခရစ်များအတွက် `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` ကိုမြှင့်ပါ။ နှုန်းထားကန့်သတ်ချက်အားလုံးကို တိုးမြှင့်ခြင်း `tryit_proxy_rate_limited_total`။ |

## မြင်နိုင်စွမ်း

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs`) သည် `/healthz` ကိုခေါ်ဆိုပြီး၊ နမူနာလမ်းကြောင်းကို ရွေးချယ်နိုင်ပြီး Norito / `probe_success` / I180NI00 အတွက် Norito node_exporter နှင့် ပေါင်းစည်းရန် `TRYIT_PROXY_PROBE_METRICS_FILE` ကို စီစဉ်သတ်မှတ်ပါ။
- ကောင်တာများ (`tryit_proxy_requests_total`၊ `tryit_proxy_rate_limited_total`၊ `tryit_proxy_upstream_failures_total`) နှင့် latency histogram များကို ဖော်ထုတ်ရန် `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` ကို သတ်မှတ်ပါ။ `dashboards/grafana/docs_portal.json` ဘုတ်သည် DOCS-SORA SLO များကို ကျင့်သုံးရန် ဤမက်ထရစ်များကို ဖတ်သည်။
- Runtime မှတ်တမ်းများသည် stdout တွင် တိုက်ရိုက်ထုတ်လွှင့်သည်။ ထည့်သွင်းမှုတိုင်းတွင် တောင်းဆိုချက် ID၊ အထက်ပိုင်းအခြေအနေ၊ အထောက်အထားစိစစ်ခြင်းအရင်းအမြစ် (`default`၊ `override`၊ သို့မဟုတ် `client`) နှင့် ကြာချိန်တို့ ပါဝင်ပါသည်။ လျှို့ဝှက်ချက်များကို ထုတ်လွှတ်ခြင်းမပြုမီ ပြန်လည်ပြင်ဆင်သည်။

`application/x-norito` သည် Torii သို့မပြောင်းလဲကြောင်း အတည်ပြုရန် လိုအပ်ပါက Jest suite (`npm test -- tryit-proxy`) ကိုဖွင့်ပါ သို့မဟုတ် `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` အောက်တွင် တပ်ဆင်မှုများကို စစ်ဆေးပါ။ ဆုတ်ယုတ်မှုစမ်းသပ်မှုများသည် ဖိသိပ်ထားသော Norito binaries၊ ရေးထိုးထားသော OpenAPI သရုပ်ဖော်မှုများနှင့် ပရောက်စီအဆင့်နှိမ့်ချခြင်းလမ်းကြောင်းများကို ဖုံးအုပ်ထားသောကြောင့် NRPC စတင်ခြင်းများသည် အမြဲတမ်းသက်သေအဖြစ် ဆက်လက်တည်ရှိနေပါသည်။