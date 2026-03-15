---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sandbox ကိုစမ်းကြည့်ပါ။

developer portal သည် ရွေးချယ်နိုင်သော "စမ်းသုံးကြည့်ပါ" ကွန်ဆိုးလ်တစ်ခု ပို့ပေးသောကြောင့် သင်သည် Torii ကိုခေါ်ဆိုနိုင်ပါသည်။
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
| `TRYIT_PROXY_BEARER` | ပုံသေကိုင်ဆောင်သူ တိုကင် Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | သုံးစွဲသူများအား `X-TryIt-Auth` | မှတစ်ဆင့် ၎င်းတို့၏ကိုယ်ပိုင်တိုကင်ကို ထောက်ပံ့ပေးရန် ခွင့်ပြုပါ။ `0` |
| `TRYIT_PROXY_MAX_BODY` | အများဆုံး တောင်းဆိုချက် ကိုယ်ထည်အရွယ်အစား (ဘိုက်များ) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | မီလီစက္ကန့်အတွင်း | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | သုံးစွဲသူ IP | တစ်ခုနှုန်း ဝင်းဒိုးတစ်ခုနှုန်း တောင်းဆိုမှုများကို ခွင့်ပြုသည်။ `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | နှုန်းကန့်သတ်ချက် (ms) | `60000` |

ပရောက်စီသည် `GET /healthz` ကိုလည်း ဖော်ထုတ်ပေးသည်၊ ဖွဲ့စည်းတည်ဆောက်ထားသော JSON အမှားများကို ပြန်ပို့ပေးသည်၊
မှတ်တမ်းထွက်ရှိမှုမှ ကိုင်ဆောင်သူတိုကင်များကို ပြန်လည်ပြင်ဆင်သည်။

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

script သည် ဘောင်ခတ်ထားသောလိပ်စာကို မှတ်တမ်းတင်ပြီး `/proxy/*` မှ တောင်းဆိုချက်များကို ပေးပို့သည်။
Torii ဇာစ်မြစ်ကို ပြင်ဆင်ထားသည်။

## ပေါ်တယ်ဝစ်ဂျက်များကို ကြိုးတပ်ပါ။

ဆော့ဖ်ဝဲရေးသားသူပေါ်တယ်ကို တည်ဆောက်သည့်အခါ သို့မဟုတ် ဝန်ဆောင်မှုပေးသည့်အခါ၊ ဝစ်ဂျက်များဖြစ်သည့် URL ကို သတ်မှတ်ပါ။
proxy အတွက် သုံးသင့်သည်-

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

အောက်ပါ အစိတ်အပိုင်းများသည် `docusaurus.config.js` မှ ဤတန်ဖိုးများကို ဖတ်ပြသည်-

- **Swagger UI** — `/reference/torii-swagger` တွင် ပြန်ဆိုထားသည်။ တောင်းဆိုချက်ကိုအသုံးပြုသည်။
  ကိုင်ဆောင်သူတိုကင်များကို အလိုအလျောက် ပူးတွဲရန် ကြားဖြတ်ကိရိယာ။
- **RapiDoc** — `/reference/torii-rapidoc`၊ တိုကင်အကွက်ကို မှန်ပြောင်းကြည့်သည်။
  proxy နှင့် ဆန့်ကျင်ဘက် try-it တောင်းဆိုမှုများကို ပံ့ပိုးပေးသည်။
- **စမ်းသုံးကြည့်ပါ console** — API ခြုံငုံသုံးသပ်ချက် စာမျက်နှာတွင် ထည့်သွင်းထားသည်။ စိတ်ကြိုက်ပို့ပေးတယ်။
  တောင်းဆိုချက်များ၊ ခေါင်းစီးများကိုကြည့်ရှုပြီး တုံ့ပြန်မှုအဖွဲ့များကို စစ်ဆေးပါ။

မည်သည့်ဝစ်ဂျက်တွင်မဆို တိုကင်ကိုပြောင်းလဲခြင်းသည် လက်ရှိဘရောက်ဆာစက်ရှင်ကိုသာ အကျိုးသက်ရောက်သည်။ အဆိုပါ
ပရောက်စီသည် ပံ့ပိုးပေးထားသော တိုကင်ကို ဘယ်သောအခါမှ ဆက်မလုပ်ပါ သို့မဟုတ် မှတ်တမ်းမတင်ပါ။

## ကြည့်ရှုနိုင်မှုနှင့် လည်ပတ်မှုများ

တောင်းဆိုမှုတိုင်းကို နည်းလမ်း၊ လမ်းကြောင်း၊ မူရင်း၊ အထက်ပိုင်းအခြေအနေ၊ နှင့် တစ်ကြိမ်ဖြင့် မှတ်တမ်းတင်ထားသည်။
စစ်မှန်ကြောင်းအထောက်အထား (`override`၊ `default`၊ သို့မဟုတ် `client`)။ တိုကင်များသည် ဘယ်တော့မှ မဟုတ်ပါ။
သိမ်းဆည်းထားသည် — ထမ်းသူခေါင်းစီးများနှင့် `X-TryIt-Auth` တန်ဖိုးများကို မပြင်ရသေးမီ
သစ်ခုတ်ခြင်း—ဒါကြောင့် သင်သည် stdout ကို စိတ်ပူစရာမလိုဘဲ ဗဟိုစုဆောင်းသူထံ ပေးပို့နိုင်ပါသည်။
လျှို့ဝှက်ချက်များပေါက်ကြား။

### ကျန်းမာရေးစစ်ဆေးခြင်းနှင့် သတိပေးချက်အစုအပြုံလိုက်စုံစမ်းစစ်ဆေးမှုကို ဖြန့်ကျက်ချထားစဉ် သို့မဟုတ် အချိန်ဇယားအတိုင်း လုပ်ဆောင်ပါ-

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

ပတ်ဝန်းကျင် ခလုတ်များ-

- `TRYIT_PROXY_SAMPLE_PATH` — ရွေးချယ်နိုင်သော Torii လမ်းကြောင်း (`/proxy` မပါဘဲ)။
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` သို့ ပုံသေများ။ လမ်းကြောင်းများရေးသားရန်အတွက် `POST` ဟုသတ်မှတ်ထားသည်။
- `TRYIT_PROXY_PROBE_TOKEN` — နမူနာခေါ်ဆိုမှုအတွက် ယာယီကိုင်ဆောင်သူ တိုကင်ကို ထိုးသွင်းသည်။
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — ပုံသေ 5s အချိန်ကုန်ဆုံးမှုကို အစားထိုးသည်။
- `TRYIT_PROXY_PROBE_METRICS_FILE` — ရွေးချယ်နိုင်သော Prometheus `probe_success`/`probe_duration_seconds` အတွက် စာသားဖိုင် ဦးတည်ရာ။
- `TRYIT_PROXY_PROBE_LABELS` — ကော်မာ-ခြားထားသော `key=value` အတွဲများကို မက်ထရစ်များတွင် ထည့်ထားသည် (ပုံသေများမှာ `job=tryit-proxy` နှင့် `instance=<proxy URL>`)။

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