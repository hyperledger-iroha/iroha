---
lang: my
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox ကိုစမ်းကြည့်ပါ။

developer portal သည် ရွေးချယ်နိုင်သော “စမ်းသုံးကြည့်ပါ” ကွန်ဆိုးလ်တစ်ခု ပို့ပေးသောကြောင့် သင်သည် Torii သို့ခေါ်ဆိုနိုင်သည်
စာရွက်စာတမ်းကို မချန်ဘဲ အဆုံးမှတ်များ။ ကွန်ဆိုးလ်သည် တောင်းဆိုချက်များကို ထပ်ဆင့်ပို့သည်။
ဘရောက်ဆာများသည် CORS ကန့်သတ်ချက်များကို ဖြတ်ကျော်နိုင်စေရန် အစုအဝေးရှိ ပရောက်စီမှတစ်ဆင့် ငြိမ်နေပါ။
နှုန်းကန့်သတ်ချက်နှင့် စစ်မှန်ကြောင်း အတည်ပြုခြင်းတို့ကို ပြဋ္ဌာန်းထားသည်။

## လိုအပ်ချက်များ

- Node.js 18.18 သို့မဟုတ် အသစ်များ (ပေါ်တယ်တည်ဆောက်မှုလိုအပ်ချက်များနှင့် ကိုက်ညီသည်)
- Torii အဆင့်သတ်မှတ်ပတ်ဝန်းကျင်သို့ ကွန်ရက်ဝင်ရောက်ခွင့်
- သင်လေ့ကျင့်ရန်စီစဉ်ထားသော Torii လမ်းကြောင်းများကိုခေါ်ဆိုနိုင်သော ကိုင်ဆောင်သူတိုကင်

ပရောက်စီဖွဲ့စည်းမှုအားလုံးကို ပတ်၀န်းကျင် ကိန်းရှင်များမှတစ်ဆင့် လုပ်ဆောင်သည်။ အောက်ပါဇယား
အရေးအကြီးဆုံး ခလုတ်များကို စာရင်းပြုစုသည်။

| ပြောင်းလဲနိုင်သော | ရည်ရွယ်ချက် | ပုံသေ |
| ---| ---| ---|
| `TRYIT_PROXY_TARGET` | Proxy မှ ထပ်ဆင့်တောင်းဆိုသော | **လိုအပ်သည်** |
| `TRYIT_PROXY_LISTEN` | ဒေသဖွံ့ဖြိုးရေးအတွက် လိပ်စာ နားထောင်ရန် (ဖော်မက် `host:port` သို့မဟုတ် `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | ပရောက်စီ | ဟုခေါ်ဆိုနိုင်သော ကော်မာ-ခြားထားသော မူရင်းစာရင်း `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | အထက်စီးကြောင်းတောင်းဆိုမှုတိုင်းအတွက် | `docs-portal` |
| `TRYIT_PROXY_BEARER` | ပုံသေကိုင်ဆောင်သူ တိုကင် Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | သုံးစွဲသူများအား `X-TryIt-Auth` | မှတစ်ဆင့် ၎င်းတို့၏ကိုယ်ပိုင်တိုကင်ကို ထောက်ပံ့ပေးရန် ခွင့်ပြုပါ။ `0` |
| `TRYIT_PROXY_MAX_BODY` | အများဆုံး တောင်းဆိုချက် ကိုယ်ထည်အရွယ်အစား (ဘိုက်များ) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | မီလီစက္ကန့်အတွင်း | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | သုံးစွဲသူ IP | တစ်ခုနှုန်း ဝင်းဒိုးတစ်ခုနှုန်း တောင်းဆိုမှုများကို ခွင့်ပြုသည်။ `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | နှုန်းကန့်သတ်ချက် (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus စတိုင် မက်ထရစ်များ အဆုံးမှတ် (`host:port` သို့မဟုတ် `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | မက်ထရစ်များ အဆုံးမှတ် | ဖြင့် ဆောင်ရွက်ပေးသော HTTP လမ်းကြောင်း `/metrics` |

ပရောက်စီသည် `GET /healthz` ကိုလည်း ဖော်ထုတ်ပေးသည်၊ ဖွဲ့စည်းထားသော JSON အမှားများကို ပြန်ပို့ပေးသည်၊
မှတ်တမ်းထွက်ရှိမှုမှ ကိုင်ဆောင်သူတိုကင်များကို ပြန်လည်ပြင်ဆင်သည်။

Docs အသုံးပြုသူများထံသို့ proxy ကိုပြသသောအခါ Swagger နှင့် `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` ကိုဖွင့်ပါ
RapiDoc အကန့်များသည် သုံးစွဲသူမှ ပံ့ပိုးပေးထားသော ကိုင်ဆောင်သူ တိုကင်များကို ပေးပို့နိုင်သည်။ ပရောက်စီသည် နှုန်းထားကန့်သတ်ချက်များကို ဆက်လက်ကျင့်သုံးဆဲ၊
အထောက်အထားများကို ပြန်လည်ပြင်ဆင်ပြီး တောင်းဆိုချက်တစ်ခုသည် မူရင်းတိုကင်ကို အသုံးပြုသည်ရှိမရှိ သို့မဟုတ် တောင်းဆိုချက်တစ်ခုချင်းအပေါ် ထပ်လောင်းခြင်းရှိမရှိ မှတ်တမ်းတင်သည်။
`TRYIT_PROXY_CLIENT_ID` ကို `X-TryIt-Client` အဖြစ် သင်ပေးပို့လိုသော အညွှန်းသို့ သတ်မှတ်ပါ
(ပုံသေ `docs-portal`)။ ပရောက်စီသည် ခေါ်ဆိုသူပေးဆောင်ထားသော ဖြတ်တောက်ပြီး မှန်ကန်ကြောင်း အတည်ပြုသည်။
`X-TryIt-Client` တန်ဖိုးများသည် ဤပုံသေသို့ ပြန်ကျသွားသောကြောင့် အဆင့်လိုက် gateways များလုပ်နိုင်သည်
ဘရောက်ဆာ မက်တာဒေတာကို မဆက်စပ်ဘဲ စစ်ဆေးခြင်း

## ပရောက်စီကို စက်တွင်းတွင် စတင်ပါ။

ပေါ်တယ်ကို သင်ပထမဆုံးစဖွင့်သောအခါတွင် မှီခိုအားထားမှုများကို ထည့်သွင်းပါ-

```bash
cd docs/portal
npm install
```

ပရောက်စီကိုဖွင့်ပြီး သင်၏ Torii စံနမူနာတွင် ညွှန်ပြပါ-

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

script သည် ဘောင်ခတ်ထားသော လိပ်စာကို မှတ်တမ်းတင်ပြီး `/proxy/*` မှ တောင်းဆိုချက်များကို ပေးပို့သည်။
Torii ဇာစ်မြစ်ကို ပြင်ဆင်သတ်မှတ်ထားသည်။

socket ကို binding မလုပ်ခင် script က အဲဒါကို validate လုပ်ပါတယ်။
`static/openapi/torii.json` တွင် မှတ်တမ်းတင်ထားသော အတိုကောက်နှင့် ကိုက်ညီပါသည်။
`static/openapi/manifest.json`။ ဖိုင်တွေ လွင့်နေရင်၊ command က an နဲ့ ထွက်ပါတယ်။
error နှင့် `npm run sync-openapi -- --latest` ကို run ရန်သင့်အားညွှန်ကြားသည်။ တင်ပို့ခြင်း။
အရေးပေါ်အခြေအနေအတွက်သာ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` proxy ပေးပါလိမ့်မယ်။
သတိပေးချက်ကို မှတ်တမ်းတင်ပြီး ပြုပြင်ထိန်းသိမ်းမှုဝင်းဒိုးများအတွင်း ပြန်လည်ရယူနိုင်စေရန် ဆက်လက်လုပ်ဆောင်ပါ။

## ပေါ်တယ်ဝစ်ဂျက်များကို ကြိုးတပ်ပါ။

ဆော့ဖ်ဝဲရေးသားသူပေါ်တယ်ကို တည်ဆောက်သည့်အခါ သို့မဟုတ် ဝန်ဆောင်မှုပေးသည့်အခါ၊ ဝစ်ဂျက်များဖြစ်သည့် URL ကို သတ်မှတ်ပါ။
proxy အတွက် သုံးသင့်သည်-

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

အောက်ပါ အစိတ်အပိုင်းများသည် `docusaurus.config.js` မှ ဤတန်ဖိုးများကို ဖတ်ပြသည်-

- **Swagger UI** — `/reference/torii-swagger` တွင် ပြန်ဆိုထားသည်။ ကြိုတင်ခွင့်ပြုပေးသည်။
  တိုကင်တစ်ခုရှိသည့်အခါ ကိုင်ဆောင်သူအစီအစဥ်၊ `X-TryIt-Client` ဖြင့် တဂ်များတောင်းဆိုမှုများ၊
  `X-TryIt-Auth` ကို ထိုးသွင်းပြီး ပရောက်စီမှတစ်ဆင့် ခေါ်ဆိုမှုများကို ပြန်လည်ရေးသားသည့်အခါ၊
  `TRYIT_PROXY_PUBLIC_URL` ကို သတ်မှတ်ထားသည်။
- **RapiDoc** — `/reference/torii-rapidoc` တွင် ပြန်ဆိုထားသည်။ တိုကင်အကွက်ကို မှန်ကြည့်၊
  Swagger panel ကဲ့သို့ တူညီသော ခေါင်းစီးများကို ပြန်လည်အသုံးပြုပြီး proxy ကို ပစ်မှတ်ထားသည်။
  URL ကို configure လုပ်သောအခါအလိုအလျောက်။
- **စမ်းသုံးကြည့်ပါ console** — API ခြုံငုံသုံးသပ်ချက် စာမျက်နှာတွင် ထည့်သွင်းထားသည်။ စိတ်ကြိုက်ပို့ပေးတယ်။
  တောင်းဆိုချက်များ၊ ခေါင်းစီးများကိုကြည့်ရှုပြီး တုံ့ပြန်မှုအဖွဲ့များကို စစ်ဆေးပါ။

အကန့်နှစ်ခုလုံးသည် ဖတ်ပြသော **snapshot ရွေးချယ်မှု** တစ်ခုပေါ်လာသည်။
`docs/portal/static/openapi/versions.json`။ ထိုအညွှန်းကို ဖြည့်ပါ။
`npm run sync-openapi -- --version=<label> --mirror=current --latest` ဆိုတော့
သုံးသပ်သူများသည် သမိုင်းဆိုင်ရာ သတ်မှတ်ချက်များကြားတွင် ခုန်ဆင်းနိုင်ပြီး၊ မှတ်တမ်းတင်ထားသော SHA-256 အချေအတင်ကို ကြည့်ပါ၊
နှင့် ထုတ်ဝေမှု လျှပ်တစ်ပြက်ရိုက်ချက်သည် အသုံးမပြုမီ လက်မှတ်ရေးထိုးထားသော မန်နီးဖက်စ်ပါရှိမရှိ အတည်ပြုပါ။
အပြန်အလှန်အကျိုးပြုသောဝစ်ဂျက်များ။

မည်သည့်ဝစ်ဂျက်တွင်မဆို တိုကင်ကိုပြောင်းလဲခြင်းသည် လက်ရှိဘရောက်ဆာစက်ရှင်ကိုသာ အကျိုးသက်ရောက်သည်။ အဆိုပါ
ပရောက်စီသည် ပံ့ပိုးပေးထားသော တိုကင်ကို ဘယ်သောအခါမှ ဆက်မလုပ်ပါ သို့မဟုတ် မှတ်တမ်းမတင်ပါ။

## ခဏတာ OAuth တိုကင်များ

ဝေဖန်သုံးသပ်သူများထံ သက်တမ်းရှည် Torii တိုကင်များ ဖြန့်ဝေခြင်းကို ရှောင်ရှားရန် စမ်းသုံးကြည့်ပါ
သင်၏ OAuth ဆာဗာအတွက် ကွန်ဆိုးလ်။ အောက်ဖော်ပြပါ ပတ်၀န်းကျင် ကိန်းရှင်များ ရှိနေသောအခါ
ပေါ်တယ်သည် စက်ပစ္စည်း-ကုဒ် လော့ဂ်အင်ဝစ်ဂျက်ကို ထုတ်ပေးသည်၊၊ ခဏတာ ကိုင်ဆောင်သူ တိုကင်များ၊
နှင့် ၎င်းတို့ကို console ပုံစံထဲသို့ အလိုအလျောက် ထိုးသွင်းသည်။

| ပြောင်းလဲနိုင်သော | ရည်ရွယ်ချက် | ပုံသေ |
| ---| ---| ---|
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth စက်ပစ္စည်း၏ ခွင့်ပြုချက် အဆုံးမှတ် (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | လက်ခံသော တိုကင်အဆုံးမှတ် _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | Docs အစမ်းကြည့်ရှုခြင်း | _empty_ |
| `DOCS_OAUTH_SCOPE` | လက်မှတ်ထိုးဝင်စဉ် | ကန့်သတ်ထားသော နယ်ပယ်များကို တောင်းဆိုထားသည်။ `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | တိုကင်ကို | နှင့် ချိတ်ရန် ရွေးချယ်နိုင်သော API ပရိသတ် _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | အတည်ပြုချက်ကို စောင့်ဆိုင်းနေစဉ် အနည်းဆုံး စစ်တမ်းကာလ (ms) | `5000` (တန်ဖိုး <5000ms ကို ငြင်းပယ်ထားသည်) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Fallback စက်-ကုဒ် သက်တမ်းကုန်ဆုံးဝင်းဒိုး (စက္ကန့်) | `600` (300s နှင့် 900s ကြားတွင် ရှိနေရမည်) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | အစားထိုးဝင်ရောက်ခွင့်-တိုကင်သက်တမ်း (စက္ကန့်) | `900` (300s နှင့် 900s ကြားတွင် ရှိနေရမည်) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth စိုးမိုးမှုကို ရည်ရွယ်ချက်ရှိရှိ ကျော်သွားသည့် ပြည်တွင်းအကြိုကြည့်ရှုမှုများအတွက် `1` သို့ သတ်မှတ်မည် | _unset_ |

နမူနာဖွဲ့စည်းပုံ-

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

သင် `npm run start` သို့မဟုတ် `npm run build` ကိုဖွင့်သောအခါ၊ ပေါ်တယ်သည် ဤတန်ဖိုးများကို မြှုပ်နှံထားသည်
`docusaurus.config.js` တွင်။ စက်တွင်း အစမ်းကြည့်ရှုမှုအတွင်း Try it ကတ်တွင် တစ်ခုပြသည်။
“စက်ပစ္စည်းကုဒ်ဖြင့် လက်မှတ်ထိုးဝင်ပါ” ခလုတ်ကိုနှိပ်ပါ။ အသုံးပြုသူများသည် သင်၏ OAuth တွင် ဖော်ပြထားသော ကုဒ်ကို ထည့်သွင်းပါ။
အတည်ပြုစာမျက်နှာ; စက်ပစ္စည်းစီးဆင်းမှု အောင်မြင်သည်နှင့် ဝစ်ဂျက်-

- ထုတ်ပေးထားသော ကိုင်ဆောင်သူတိုကင်ကို Try it console အကွက်ထဲသို့ ထိုးထည့်ပါ၊
- ရှိပြီးသား `X-TryIt-Client` နှင့် `X-TryIt-Auth` ခေါင်းစီးများဖြင့် တောင်းဆိုမှုများ၊
- ကျန်ရှိသော သက်တမ်းကို ပြသပေးပြီး၊
- သက်တမ်းကုန်ဆုံးသည့်အခါ တိုကင်ကို အလိုအလျောက်ရှင်းပေးသည်။

Manual Bearer ထည့်သွင်းမှုသည် ဆက်လက်ရရှိနိုင်သည်—သင်သည် အချိန်တိုင်းတွင် OAuth ကိန်းရှင်များကို ချန်လှပ်ထားသည်။
ဝေဖန်သုံးသပ်သူများကို ယာယီတိုကင်ကို ကိုယ်တိုင်ကူးထည့်ရန် သို့မဟုတ် ထုတ်ယူရန် အတင်းအကျပ် ခိုင်းစေလိုသည်။
အမည်မသိဝင်ရောက်အသုံးပြုနိုင်သည့် သီးခြားဒေသတွင်း အကြိုကြည့်ရှုခြင်းများအတွက် `DOCS_OAUTH_ALLOW_INSECURE=1`
လက်ခံနိုင်ဖွယ်ရှိသည်။ OAuth မပါဘဲ တည်ဆောက်မှုများသည် ယခုအခါ ကျေနပ်အားရစေရန် အမြန်ပျက်ကွက်ပါသည်။
DOCS-1b လမ်းပြမြေပုံဂိတ်။

📌 [Security hardening & pen-test checklist](./security-hardening.md) ကို သုံးသပ်ပါ။
ဓာတ်ခွဲခန်းအပြင်ဘက်တွင် ပေါ်တယ်ကို မဖော်ထုတ်မီ၊ ၎င်းသည် ခြိမ်းခြောက်မှုပုံစံကို မှတ်တမ်းတင်ခြင်း၊
CSP/Trusted Types ပရိုဖိုင်နှင့် ယခု DOCS-1b ဂိတ်ပေါက်သည့် ထိုးဖောက်မှု-စမ်းသပ်မှု အဆင့်များ။

## Norito-RPC နမူနာများ

Norito-RPC တောင်းဆိုချက်များသည် JSON လမ်းကြောင်းများကဲ့သို့တူညီသော proxy နှင့် OAuth ပိုက်လိုင်းကို မျှဝေသည်၊
သူတို့က ရိုးရိုး `Content-Type: application/x-norito` ကို သတ်မှတ်ပြီး ပို့ပေးပါတယ်။
NRPC သတ်မှတ်ချက်တွင်ဖော်ပြထားသော ကြိုတင်ကုဒ်လုပ်ထားသော Norito ပေးဆောင်မှု
(`docs/source/torii/nrpc_spec.md`)။
repository သည် `fixtures/norito_rpc/` အောက်တွင် canonical payload များကို ပို့ဆောင်ပေးပါသည်။
စာရေးဆရာများ၊ SDK ပိုင်ရှင်များနှင့် သုံးသပ်သူများသည် CI အသုံးပြုသည့် ဘိုက်အတိအကျကို ပြန်ဖွင့်နိုင်ပါသည်။

### Try It console မှ Norito payload တစ်ခုကို ပို့ပါ။

1. `fixtures/norito_rpc/transfer_asset.norito` ကဲ့သို့သော ခံစစ်မှူးကို ရွေးပါ။ ဒါတွေ
   ဖိုင်များသည် အကြမ်းထည် Norito စာအိတ်များဖြစ်သည်။ **မလုပ်ပါ** base64-encode လုပ်ပါ။
2. Swagger သို့မဟုတ် RapiDoc တွင် NRPC အဆုံးမှတ်ကို ရှာပါ (ဥပမာ
   `POST /v1/pipeline/submit`) နှင့် **အကြောင်းအရာ-အမျိုးအစား** ရွေးချယ်ကိရိယာသို့ ပြောင်းပါ။
   `application/x-norito`။
3. တောင်းဆိုချက်ကိုယ်ထည်တည်းဖြတ်သူကို **binary** သို့ပြောင်းပါ (Swagger ၏ "File" မုဒ် သို့မဟုတ်
   RapiDoc ၏ "Binary/File" ရွေးပေးသူ) နှင့် `.norito` ဖိုင်ကို အပ်လုဒ်လုပ်ပါ။ ဝဒ်
   ပြောင်းလဲခြင်းမရှိဘဲ ဘိုက်များကို ပရောက်စီမှတဆင့် ထုတ်လွှင့်သည်။
4. တောင်းဆိုချက်ကိုတင်ပြပါ။ အကယ်၍ Torii သည် `X-Iroha-Error-Code: schema_mismatch` ပြန်တက်လာပါက၊
   binary payloads နှင့် binary payloads ကို လက်ခံသည့် endpoint ကို သင်ခေါ်ဆိုနေကြောင်း စစ်ဆေးပါ။
   `fixtures/norito_rpc/schema_hashes.json` တွင်မှတ်တမ်းတင်ထားသော schema hash ကိုအတည်ပြုပါ။
   သင်ရိုက်နေသော Torii နှင့် ကိုက်ညီပါသည်။

ကွန်ဆိုးလ်သည် လတ်တလောဖိုင်ကို မမ်မိုရီတွင် သိမ်းဆည်းထားသောကြောင့် အလားတူ ပြန်လည်ပေးပို့နိုင်ပါသည်။
ကွဲပြားခြားနားသော ခွင့်ပြုချက်တိုကင်များ သို့မဟုတ် Torii တန်ဆာပလာများကို အသုံးပြုနေစဉ် payload။ ထည့်ပေးခြင်း။
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` မှ သင့်လုပ်ငန်းအသွားအလာကို ထုတ်လုပ်သည်။
NRPC-4 မွေးစားခြင်းအစီအစဉ် (မှတ်တမ်း + JSON အကျဉ်းချုပ်) တွင် ကိုးကားထားသော အထောက်အထားအတွဲ၊
သုံးသပ်ချက်များအတွင်း Try It တုံ့ပြန်မှုကို ဖန်သားပြင်ဓာတ်ပုံရိုက်ခြင်းနှင့် ကောင်းမွန်စွာတွဲဖက်ပါသည်။

### CLI ဥပမာ (curl)

အသုံးဝင်သော `curl` မှတစ်ဆင့် ပေါ်တယ်အပြင်ဘက်တွင် အလားတူပစ္စည်းများကို ပြန်လည်ပြသနိုင်သည်၊
ပရောက်စီကို တရားဝင်စစ်ဆေးသည့်အခါ သို့မဟုတ် ဂိတ်ဝေးတုံ့ပြန်မှုများကို အမှားရှာစစ်ဆေးသည့်အခါ-

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

`transaction_fixtures.manifest.json` တွင်ဖော်ပြထားသော မည်သည့် entry အတွက်မဆို အံဝင်ခွင်ကျကို လဲလှယ်ပါ။
သို့မဟုတ် `cargo xtask norito-rpc-fixtures` ဖြင့် သင့်ကိုယ်ပိုင် payload ကို ကုဒ်လုပ်ပါ။ Torii တုန်းက
Canary မုဒ်တွင် သင် `curl` ကို try-it proxy တွင် ညွှန်နိုင်သည်
(`https://docs.sora.example/proxy/v1/pipeline/submit`) လေ့ကျင့်ခန်းအတူတူပါပဲ။
ပေါ်တယ်ဝစ်ဂျက်များ အသုံးပြုသည့် အခြေခံအဆောက်အဦ။

## မြင်နိုင်စွမ်းနှင့် လည်ပတ်မှုများတောင်းဆိုမှုတိုင်းကို နည်းလမ်း၊ လမ်းကြောင်း၊ မူရင်း၊ အထက်ပိုင်းအခြေအနေ၊ နှင့် တစ်ကြိမ်ဖြင့် မှတ်တမ်းတင်ထားသည်။
စစ်မှန်ကြောင်းအထောက်အထား (`override`၊ `default`၊ သို့မဟုတ် `client`)။ တိုကင်များသည် ဘယ်တော့မှ မဟုတ်ပါ။
သိမ်းဆည်းထားသည် — ထမ်းသူခေါင်းစီးများနှင့် `X-TryIt-Auth` တန်ဖိုးများကို မပြင်ဆင်ရသေးမီ
သစ်ခုတ်ခြင်း—ဒါကြောင့် သင်သည် stdout ကို စိတ်ပူစရာမလိုဘဲ ဗဟိုစုဆောင်းသူထံ ပေးပို့နိုင်ပါသည်။
လျှို့ဝှက်ချက်များပေါက်ကြား။

### ကျန်းမာရေးစစ်ဆေးခြင်းနှင့် သတိပေးချက်

အစုအပြုံလိုက်စုံစမ်းစစ်ဆေးမှုကို ဖြန့်ကျက်ချထားစဉ် သို့မဟုတ် အချိန်ဇယားအတိုင်း လုပ်ဆောင်ပါ-

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

ပတ်ဝန်းကျင် ခလုတ်များ-

- `TRYIT_PROXY_SAMPLE_PATH` — ရွေးချယ်နိုင်သော Torii လမ်းကြောင်း (`/proxy` မပါဘဲ)။
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` သို့ ပုံသေများ။ လမ်းကြောင်းများရေးသားရန်အတွက် `POST` ဟုသတ်မှတ်ထားသည်။
- `TRYIT_PROXY_PROBE_TOKEN` — နမူနာခေါ်ဆိုမှုအတွက် ယာယီကိုင်ဆောင်သူ တိုကင်ကို ထိုးသွင်းသည်။
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — မူရင်း 5s အချိန်ကုန်ဆုံးမှုကို အစားထိုးသည်။
- `TRYIT_PROXY_PROBE_METRICS_FILE` — ရွေးချယ်နိုင်သော Prometheus `probe_success`/`probe_duration_seconds` အတွက် စာသားဖိုင် ဦးတည်ရာ။
- `TRYIT_PROXY_PROBE_LABELS` — ကော်မာ-ခြားထားသော `key=value` အတွဲများကို မက်ထရစ်များတွင် ထည့်ထားသည် (ပုံသေများမှာ `job=tryit-proxy` နှင့် `instance=<proxy URL>`)။
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` ကို ဖွင့်ထားသောအခါ အောင်မြင်စွာ တုံ့ပြန်ရမည့် မက်ထရစ်အဆုံးမှတ် URL (ဥပမာ၊ `http://localhost:9798/metrics`)။

ရလဒ်များကို စာသားဖိုင်စုဆောင်းသူထံ ညွှန်ပြခြင်းဖြင့် ရလဒ်များကို ရေးသွင်းနိုင်သည်။
လမ်းကြောင်း (ဥပမာ၊ `/var/lib/node_exporter/textfile_collector/tryit.prom`) နှင့်
စိတ်ကြိုက်တံဆိပ်များထည့်ခြင်း-

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

ဇာတ်ညွှန်းသည် မက်ထရစ်ဖိုင်ကို အက်တမ်ပုံစံဖြင့် ပြန်လည်ရေးသားပေးသောကြောင့် သင်၏စုဆောင်းသူသည် အမြဲဖတ်သည်။
ပြီးပြည့်စုံသော payload။

`TRYIT_PROXY_METRICS_LISTEN` ကို ပြင်ဆင်သတ်မှတ်သည့်အခါ၊ သတ်မှတ်ပါ။
`TRYIT_PROXY_PROBE_METRICS_URL` သည် မက်ထရစ်များ အဆုံးမှတ်သို့ ရောက်သောကြောင့် စုံစမ်းစစ်ဆေးမှု မြန်ဆန်စွာ မအောင်မြင်ပါ။
ခြစ်ထားသော မျက်နှာပြင် ပျောက်သွားပါက (ဥပမာ၊ ပုံသွင်းမှု မှားယွင်းစွာ ဝင်ရောက်ခြင်း သို့မဟုတ် ပျောက်ဆုံးနေခြင်း
Firewall စည်းမျဉ်းများ)။ ပုံမှန်ထုတ်လုပ်မှု သတ်မှတ်ချက်တစ်ခုဖြစ်သည်။
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`။

ပေါ့ပါးသောသတိပေးချက်အတွက်၊ သင်၏စောင့်ကြည့်ရေးစဥ်တွင် စုံစမ်းစစ်ဆေးမှုအား ကြိုးတပ်ပါ။ Prometheus
နှစ်ကြိမ်ဆက်တိုက် ကျရှုံးပြီးနောက် စာမျက်နှာများ ဥပမာ-

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### မက်ထရစ်အဆုံးမှတ်များနှင့် ဒက်ရှ်ဘုတ်များ

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (သို့မဟုတ် မည်သည့် host/port pair) ကိုမဆို အရင်သတ်မှတ်ပါ။
Prometheus ဖော်မက်လုပ်ထားသည့် မက်ထရစ်များ အဆုံးမှတ်ကို ဖော်ထုတ်ရန် ပရောက်စီကို စတင်သည်။ လမ်းစဉ်
ပုံသေ `/metrics` သို့ သော်လည်းကောင်း မှတဆင့် အစားထိုးနိုင်ပါသည်။
`TRYIT_PROXY_METRICS_PATH=/custom`။ ခြစ်လိုက်တိုင်းသည် နည်းလမ်းတစ်ခုအတွက် ကောင်တာများကို ပြန်ပေးသည်။
စုစုပေါင်းတောင်းဆိုမှု၊ နှုန်းကန့်သတ်ချက်၊ ရေစီးကြောင်းအမှားအယွင်းများ/အချိန်ကုန်သွားခြင်း၊ ပရောက်စီရလဒ်များ၊
နှင့် latency အနှစ်ချုပ်များ-

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

သင်၏ Prometheus/OTLP စုဆောင်းသူများကို မက်ထရစ်များအဆုံးမှတ်တွင် ညွှန်ပြပြီး ၎င်းကို ပြန်လည်အသုံးပြုပါ။
ရှိပြီးသား `dashboards/grafana/docs_portal.json` အကန့်များ SRE သည် အမြီးကို စောင့်ကြည့်နိုင်သည်။
မှတ်တမ်းများကို ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ latencies နှင့် rejection များ တိုးလာသည်။ ပရောက်စီသည် အလိုအလျောက်ဖြစ်သည်။
ပြန်လည်စတင်မှုများကို အော်ပရေတာများသိရှိနိုင်ရန် ကူညီရန် `tryit_proxy_start_timestamp_ms` ကို ထုတ်ဝေသည်။

### အလိုအလျောက်ပြန်လှည့်ခြင်း။

ပစ်မှတ် Torii URL ကို အပ်ဒိတ် သို့မဟုတ် ပြန်လည်ရယူရန် စီမံခန့်ခွဲမှုအကူကို အသုံးပြုပါ။ ဇာတ်ညွှန်း
`.env.tryit-proxy.bak` တွင် ယခင် configuration ကို သိမ်းဆည်းထားသောကြောင့် rollbacks များသည်
command တစ်ခုတည်း။

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

သင်၏အသုံးပြုမှုရှိပါက `--env` သို့မဟုတ် `TRYIT_PROXY_ENV` ဖြင့် env ဖိုင်လမ်းကြောင်းကို အစားထိုးပါ။
configuration ကို အခြားနေရာတွင် သိမ်းဆည်းပါ။