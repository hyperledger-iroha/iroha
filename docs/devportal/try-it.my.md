---
lang: my
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

developer portal သည် Torii REST API အတွက် "စမ်းကြည့်ပါ" ကွန်ဆိုးလ်တစ်ခု ပေးပို့သည်။ ဒီလမ်းညွှန်
ပံ့ပိုးပေးသည့် ပရောက်စီကို မည်သို့စတင်ရန်နှင့် ကွန်ဆိုးလ်အား ဇာတ်ညွှန်းတစ်ခုသို့ ချိတ်ဆက်နည်းကို ရှင်းပြသည်။
အထောက်အထားများကို မဖော်ပြဘဲ တံခါးပေါက်။

## လိုအပ်ချက်များ

- Iroha repository checkout (workspace root)။
- Node.js 18.18+ (ပေါ်တယ်အခြေခံလိုင်းနှင့် ကိုက်ညီသည်)။
- Torii အဆုံးမှတ်ကို သင်၏ workstation (staging သို့မဟုတ် local) မှ ရယူနိုင်သည်။

## 1. OpenAPI လျှပ်တစ်ပြက်ရိုက်ချက် ဖန်တီးပါ (ချန်လှပ်ထားနိုင်သည်)

ကွန်ဆိုးလ်သည် တူညီသော OpenAPI payload ကို portal ရည်ညွှန်းစာမျက်နှာများအဖြစ် ပြန်လည်အသုံးပြုသည်။ အကယ်လို့
သင်သည် Torii လမ်းကြောင်းများကို ပြောင်းလဲပြီး လျှပ်တစ်ပြက်ကို ပြန်လည်ထုတ်ပါ-

```bash
cargo xtask openapi
```

အလုပ်က `docs/portal/static/openapi/torii.json` လို့ရေးတယ်။

## 2. Try It proxy ကို စတင်ပါ။

repository root မှ

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### ပတ်ဝန်းကျင် ပြောင်းလဲမှု

| ပြောင်းလဲနိုင်သော | ဖော်ပြချက် |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii အခြေခံ URL (လိုအပ်သည်)။ |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | ပရောက်စီကို အသုံးပြုရန် ခွင့်ပြုထားသော မူရင်းများစာရင်း (`http://localhost:3000`)။ |
| `TRYIT_PROXY_BEARER` | ပရောက်ဆီထည့်ထားသော တောင်းဆိုမှုအားလုံးတွင် ရွေးချယ်နိုင်သော ပုံသေကိုင်ဆောင်သူ တိုကင်ကို အသုံးပြုထားသည်။ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | ခေါ်ဆိုသူ၏ `Authorization` ခေါင်းစီးစကားလုံးကို ထပ်ဆင့်ပေးပို့ရန် `1` သို့ သတ်မှတ်ပါ။ |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | မမ်မိုရီအတွင်းနှုန်းကန့်သတ်ဆက်တင်များ (မူရင်းများ- 60s လျှင် တောင်းဆိုမှု 60)။ |
| `TRYIT_PROXY_MAX_BODY` | အများဆုံး တောင်းဆိုချက် ပေးချေမှုအား လက်ခံသည် (ဘိုက်များ၊ မူရင်း 1MiB)။ |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii တောင်းဆိုမှုများအတွက် ရေစီးကြောင်းတွင် အချိန်ကုန်ခြင်း (မူလ 10000ms)။ |

ပရောက်စီသည် ဖော်ထုတ်သည်-

- `GET /healthz` — အဆင်သင့်စစ်ဆေးမှု။
- `/proxy/*` — proxyed တောင်းဆိုမှုများ၊ လမ်းကြောင်းနှင့် query string ကို ထိန်းသိမ်းထားသည်။

## 3. ပေါ်တယ်ကိုဖွင့်ပါ။

သီးခြား terminal တွင်-

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` သို့ဝင်ရောက်ပြီး Try It console ကိုအသုံးပြုပါ။ အတူတူပါပဲ။
ပတ်ဝန်းကျင်ပြောင်းလဲမှုများသည် Swagger UI နှင့် RapiDoc မြှုပ်နှံမှုများကို စီစဉ်သတ်မှတ်ပေးသည်။

## 4. အပြေးယူနစ်စစ်ဆေးမှုများ

ပရောက်စီသည် လျင်မြန်သော Node-based စမ်းသပ်မှုအစုံကို ပြသသည်-

```bash
npm run test:tryit-proxy
```

စမ်းသပ်မှုများတွင် လိပ်စာခွဲခြမ်းစိတ်ဖြာခြင်း၊ မူရင်းကိုင်တွယ်ခြင်း၊ နှုန်းကန့်သတ်ခြင်းနှင့် ကိုင်ဆောင်ခြင်းတို့ ပါဝင်သည်။
ဆေးထိုး။

## 5. အလိုအလျောက်စနစ်နှင့် တိုင်းတာမှုများကို စစ်ဆေးပါ။

`/healthz` နှင့် နမူနာ အဆုံးမှတ်ကို စစ်ဆေးရန် အစုလိုက်အပြုံလိုက် စုံစမ်းစစ်ဆေးမှုကို အသုံးပြုပါ-

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

ပတ်ဝန်းကျင် ခလုတ်များ-

- `TRYIT_PROXY_SAMPLE_PATH` — ရွေးချယ်နိုင်သော Torii လမ်းကြောင်း (`/proxy` မပါဘဲ) လေ့ကျင့်ခန်း။
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` သို့ ပုံသေများ။ လမ်းကြောင်းများရေးသားရန်အတွက် `POST` ဟုသတ်မှတ်ထားသည်။
- `TRYIT_PROXY_PROBE_TOKEN` — နမူနာခေါ်ဆိုမှုအတွက် ယာယီကိုင်ဆောင်သူ တိုကင်ကို ထိုးသွင်းသည်။
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — ပုံသေ 5s အချိန်လွန်မှုကို အစားထိုးသည်။
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus `probe_success`/`probe_duration_seconds` အတွက် စာသားဖိုင် ဦးတည်ရာ။
- `TRYIT_PROXY_PROBE_LABELS` — ကော်မာ-ခြားထားသော `key=value` အတွဲများကို မက်ထရစ်များတွင် ထည့်ထားသည် (ပုံသေများမှာ `job=tryit-proxy` နှင့် `instance=<proxy URL>`)။

`TRYIT_PROXY_PROBE_METRICS_FILE` ကို သတ်မှတ်သောအခါ၊ script သည် ဖိုင်ကို ပြန်လည်ရေးသားသည်။
အက်တမ်နည်းအားဖြင့် သင်၏ node_exporter/textfile စုဆောင်းသူသည် ပြီးပြည့်စုံမှုကို အမြဲမြင်နေရသည်။
payload ဥပမာ-

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

ရလဒ်မက်ထရစ်များကို Prometheus သို့ ပေးပို့ပြီး နမူနာသတိပေးချက်ကို ပြန်သုံးပါ။
`probe_success` မှ `0` သို့ကျသွားသောအခါ developer-portal docs

## 6. ထုတ်လုပ်မှု တင်းမာမှု စစ်ဆေးရန်စာရင်း

ဒေသဆိုင်ရာ ဖွံ့ဖြိုးတိုးတက်မှုထက် ကျော်လွန်၍ ပရောက်စီကို မထုတ်ဝေမီ-

- ပရောက်စီ၏ရှေ့တွင် TLS (ပြောင်းပြန်ပရောက်စီ သို့မဟုတ် စီမံထားသော ဂိတ်ဝ) ကိုပိတ်ပါ။
- စနစ်တကျ သစ်ခုတ်ခြင်းကို စီစဉ်သတ်မှတ်ပြီး မြင်နိုင်စွမ်းရှိသော ပိုက်လိုင်းများဆီသို့ ပို့ဆောင်ပါ။
- ကိုင်ဆောင်သူတိုကင်များကို လှည့်၍ သင်၏လျှို့ဝှက်ချက်များမန်နေဂျာတွင် သိမ်းဆည်းပါ။
- ပရောက်စီ၏ `/healthz` အဆုံးမှတ်နှင့် ပေါင်းစည်း latency မက်ထရစ်များကို စောင့်ကြည့်ပါ။
- သင်၏ Torii အဆင့်သတ်မှတ်ခွဲတမ်းများနှင့် ကန့်သတ်နှုန်းများကို ချိန်ညှိပါ။ `Retry-After` ကို ချိန်ညှိပါ။
  ဖောက်သည်များနှင့် ဆက်သွယ်ရန် အတားအဆီးဖြစ်စေသော အပြုအမူ။