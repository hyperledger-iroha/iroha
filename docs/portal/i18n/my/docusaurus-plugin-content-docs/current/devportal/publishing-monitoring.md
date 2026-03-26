---
id: publishing-monitoring
lang: my
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

လမ်းပြမြေပုံပါ ပစ္စည်း **DOCS-3c** သည် ထုပ်ပိုးမှုစစ်ဆေးသည့်စာရင်းထက် ပိုမိုလိုအပ်သည်- တိုင်းပြီးနောက်
SoraFS ထုတ်ဝေသူ developer ပေါ်တယ်ကို ကျွန်ုပ်တို့ စဉ်ဆက်မပြတ် သက်သေပြရမည်၊ စမ်းသုံးကြည့်ပါ
proxy၊ နှင့် gateway bindings များသည် ကျန်းမာနေပါသည်။ ဤစာမျက်နှာသည် စောင့်ကြည့်မှုကို မှတ်တမ်းတင်ထားသည်။
[deployment guide](./deploy-guide.md) သို့ CI နှင့် ပါသော မျက်နှာပြင်
ခေါ်ဆိုသော အင်ဂျင်နီယာများသည် SLO ကို ကျင့်သုံးရန် Ops အသုံးပြုသည့် အလားတူစစ်ဆေးမှုများကို လုပ်ဆောင်နိုင်သည်။

## ပိုက်လိုင်းပြန်ချုပ်

1. **တည်ဆောက်ပြီး လက်မှတ်ရေးထိုးခြင်း** – လုပ်ဆောင်ရန် [deployment guide](./deploy-guide.md) ကို လိုက်နာပါ။
   `npm run build`၊ `scripts/preview_wave_preflight.sh` နှင့် Sigstore +
   တင်ပြမှုအဆင့်များကို ဖော်ပြပါ။ preflight script သည် `preflight-summary.json` ကို ထုတ်လွှတ်သည်။
   ထို့ကြောင့် အစမ်းကြည့်ရှုမှုတိုင်းတွင် build/link/probe metadata ပါရှိသည်။
2. **ပင်ထိုး၍ အတည်ပြုပါ** – `sorafs_cli manifest submit`၊ `cargo xtask soradns-verify-binding`၊
   နှင့် DNS ဖြတ်တောက်ခြင်းအစီအစဉ်သည် အုပ်ချုပ်ရေးအတွက် အဆုံးအဖြတ်ပေးသည့်အရာများကို ပေးဆောင်သည်။
3. **သိမ်းဆည်းထားသော အထောက်အထား** – CAR အနှစ်ချုပ်၊ Sigstore အတွဲ၊ နာမည်တူ အထောက်အထား၊
   probe output နှင့် `docs_portal.json` ဒက်ရှ်ဘုတ်အောက်ရှိ လျှပ်တစ်ပြက်ဓာတ်ပုံများ
   `artifacts/sorafs/<tag>/`။

## စောင့်ကြည့်ရေးလိုင်းများ

### 1. ထုတ်ဝေရေး မော်နီတာများ (`scripts/monitor-publishing.mjs`)

`npm run monitor:publishing` command အသစ်သည် portal probe ကို ခြုံပြီး စမ်းကြည့်ပါ။
proxy probe၊ နှင့် CI-ဖော်ရွေသော စစ်ဆေးချက်တစ်ခုတွင် binding verifier တစ်ခုပေးပါ။
JSON config (CI လျှို့ဝှက်ချက်များ သို့မဟုတ် `configs/docs_monitor.json` သို့စစ်ဆေးထားသည်) နှင့် run-

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` (နှင့် ရွေးချယ်နိုင်သည်
`--prom-job docs-preview`) အတွက် သင့်လျော်သော Prometheus စာသားဖော်မတ်မက်ထရစ်များကို ထုတ်လွှတ်ရန်
Pushgateway အပ်လုဒ်များ သို့မဟုတ် Prometheus အပိုင်းအစများ/ထုတ်လုပ်ခြင်းတွင် တိုက်ရိုက်ခြစ်ခြင်း။ ဟိ
မက်ထရစ်များသည် JSON အကျဉ်းကို ထင်ဟပ်စေသောကြောင့် SLO ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်စည်းမျဉ်းများကို ခြေရာခံနိုင်သည်။
portal၊ စမ်းသုံးကြည့်ပါ၊ စည်းနှောင်ခြင်းနှင့် အထောက်အထားအစုအဝေးကို ခွဲခြမ်းစိတ်ဖြာခြင်းမပြုဘဲ DNS ကျန်းမာရေးကို လုပ်ဆောင်ပါ။

လိုအပ်သော အဖုများနှင့် ချိတ်ဆက်မှုများစွာပါရှိသော နမူနာ config-

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/<i105-account-id>/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

မော်နီတာသည် JSON အနှစ်ချုပ် (S3/SoraFS အဆင်ပြေသည်) ရေးပြီး သုညမဟုတ်သည့်အခါ ထွက်သည်
မည်သည့် probe မဆို ပျက်ကွက်ပြီး ၎င်းကို Cron အလုပ်များ၊ Buildkite အဆင့်များ သို့မဟုတ် သင့်လျော်အောင် ပြုလုပ်ထားသည်။
သတိပေးချက်မန်နေဂျာ webhooks။ Passing `--evidence-dir` ဆက်ရှိနေပါက `summary.json`၊
`portal.json`၊ `tryit.json` နှင့် `binding.json` `checksums.sha256`
ထို့ကြောင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် စောင့်ကြည့်စစ်ဆေးမှုရလဒ်များကို ကွဲပြားစေနိုင်သောကြောင့် ထင်ရှားပါသည်။
probes ကို ပြန်လည်လုပ်ဆောင်ပါ။

> **TLS guardrail-** `monitorPortal` သည် သင်သတ်မှတ်မထားပါက `http://` အခြေခံ URL များကို ငြင်းပယ်သည်
config တွင် > `allowInsecureHttp: true`။ ထုတ်လုပ်မှု/ အဆင့်စစ်ဆေးမှုများကို ဆက်လက်ဖွင့်ထားပါ။
> HTTPS; ရွေးချယ်မှုသည် ဒေသတွင်း အကြိုကြည့်ရှုမှုများအတွက်သာ တည်ရှိပါသည်။

binding entry တစ်ခုစီသည် `cargo xtask soradns-verify-binding` ကို ဖမ်းယူထားသည့် ဆန့်ကျင်ဘက်တွင် လုပ်ဆောင်သည်။
`portal.gateway.binding.json` အစုအဝေး (နှင့် ရွေးချယ်နိုင်သော `manifestJson`) ထို့ကြောင့် နာမည်တူ၊
သက်သေအခြေအနေနှင့် အကြောင်းအရာ CID သည် ထုတ်ပြန်ထားသော အထောက်အထားများနှင့် ကိုက်ညီနေပါသည်။ ဟိ
ရွေးချယ်နိုင်သော `hostname` guard သည် alias မှရရှိသော canonical host နှင့်ကိုက်ညီကြောင်းအတည်ပြုသည်
သင်မြှင့်တင်ရန် ရည်ရွယ်ထားသည့် gateway host သည် ၎င်းမှ ပျံ့လွင့်လာသော DNS ဖြတ်တောက်မှုများကို တားဆီးပေးသည်။
မှတ်တမ်းတင် ချိတ်ဆွဲခြင်း။

ရွေးချယ်နိုင်သော `dns` ဘလောက်ဝိုင်ယာကြိုးများကို DOCS-7 ၏ SoraDNS တစ်ခုတည်းသော မော်နီတာတွင် ထုတ်လွှင့်သည်။
ထည့်သွင်းမှုတစ်ခုစီသည် hostname/record-type pair တစ်ခုကို ဖြေရှင်းသည် (ဥပမာ
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) ကိုလည်းကောင်း၊
အဖြေများကို `expectedRecords` သို့မဟုတ် `expectedIncludes` နှင့် ကိုက်ညီကြောင်း အတည်ပြုသည်။ ဒုတိယ
ဟာ့ဒ်ကုဒ်များ အထက်ဖော်ပြပါ အတိုအထွာများတွင် ထည့်သွင်းခြင်းဖြင့် ထုတ်လုပ်ထားသော canonical hashed hostname ကို
`cargo xtask soradns-hosts --name docs-preview.sora.link`; မော်နီတာသည် ယခု သက်သေပြနေပါသည်။
လူသားဖော်ရွေသော နံမည်နှင့် ကျမ်းဂန်အခေါ်အဝေါ် (`igjssx53…gw.sora.id`)
pinned pretty host ကိုဖြေရှင်းပါ။ ၎င်းသည် DNS မြှင့်တင်ရေး အထောက်အထားကို အလိုအလျောက် ဖြစ်စေသည်-
HTTP ချိတ်ဆက်ထားဆဲဖြစ်သည့်တိုင် host နှစ်ခုလုံး လွင့်သွားပါက မော်နီတာသည် ပျက်သွားမည်ဖြစ်သည်။
မှန်ကန်သော သရုပ်ကို အခြေခံပါ။

### 2. OpenAPI ဗားရှင်း manifest guard

DOCS-2b ၏ "လက်မှတ်ထိုးထားသော OpenAPI manifest" လိုအပ်ချက်သည် ယခုအခါ အလိုအလျောက် အစောင့်တစ်ဦးကို ပို့ဆောင်ပေးသည်-
`ci/check_openapi_spec.sh` သည် `npm run check:openapi-versions` ကိုခေါ်ဆိုသည်၊
`scripts/verify-openapi-versions.mjs` ကို အပြန်အလှန်စစ်ဆေးရန်
`docs/portal/static/openapi/versions.json` သည် အမှန်တကယ် Torii specs များနှင့်
ထင်ရှားသည်။ အစောင့်က စစ်ဆေးပါတယ်-

- `versions.json` တွင်ဖော်ပြထားသော ဗားရှင်းတိုင်းတွင် ကိုက်ညီသောလမ်းညွှန်တစ်ခုရှိသည်။
  `static/openapi/versions/`။
- ထည့်သွင်းမှုတစ်ခုစီ၏ `bytes` နှင့် `sha256` အကွက်များသည် on-disk spec ဖိုင်နှင့် ကိုက်ညီပါသည်။
- `latest` alias သည် `current` ထည့်သွင်းမှုကို ထင်ဟပ်စေသည် (အသေးစိတ်/ အရွယ်အစား/ လက်မှတ် မက်တာဒေတာ)
  ထို့ကြောင့် မူရင်းဒေါင်းလုဒ်သည် မပျံ့လွင့်နိုင်ပါ။
- လက်မှတ်ရေးထိုးထားသော ထည့်သွင်းမှုများသည် `artifact.path` ကို ပြန်ညွှန်ထားသည့် မန်နီးဖက်စ်ကို ရည်ညွှန်းသည်။
  တူညီသော spec နှင့် ၎င်း၏ လက်မှတ်/အများပြည်သူသုံးသော့ hex တန်ဖိုးများသည် မန်နီးဖက်စ်နှင့် ကိုက်ညီပါသည်။

spec အသစ်တစ်ခုကို သင်ထင်မြင်သည့်အခါတိုင်း အစောင့်အကြပ်ကို စက်တွင်းတွင် လုပ်ဆောင်ပါ-

```bash
cd docs/portal
npm run check:openapi-versions
```

ပျက်ကွက်သောမက်ဆေ့ချ်များတွင် stale-file အရိပ်အမြွက် (`npm run sync-openapi -- --latest`) ပါဝင်သည်
ထို့ကြောင့် ပေါ်တယ်မှ ပံ့ပိုးကူညီသူများသည် လျှပ်တစ်ပြက်ရိုက်ချက်များကို မည်သို့ ပြန်လည်ဆန်းသစ်ရမည်ကို သိသည်။ အစောင့်အကြပ်တွေ ချထားတယ်။
လက်မှတ်ထိုးထားသော မန်နီးဖက်စ်နှင့် ထုတ်ဝေသည့် အနှစ်ချုပ်နေရာတွင် ပေါ်တယ်ထုတ်လွှတ်မှုကို CI တားဆီးသည်။
ထပ်တူကျသည်။

### 2. ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များ

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c အတွက် မူလဘုတ်အဖွဲ့။ ပြားများ
  ခြေရာခံ `torii_sorafs_gateway_refusals_total`၊ ပုံတူ SLA လွတ်သွားသည်၊ စမ်းကြည့်ပါ။
  ပရောက်စီ အမှားအယွင်းများနှင့် စုံစမ်းစစ်ဆေးမှု တုံ့ပြန်ချိန် (`docs.preview.integrity` ထပ်ဆင့်)။ တင်ပို့ပါ။
  ထုတ်ဝေမှုတိုင်းပြီးနောက် ဘုတ်အဖွဲ့အား စစ်ဆင်ရေးလက်မှတ်တွင် ပူးတွဲပါရှိသည်။
- **စမ်းသုံးကြည့်ပါ ပရောက်စီသတိပေးချက်များ** - သတိပေးမန်နေဂျာစည်းမျဉ်း `TryItProxyErrors` ပွင့်သည်
  `probe_success{job="tryit-proxy"}` ဆက်တိုက်ကျဆင်းသွားခြင်း သို့မဟုတ်
  `tryit_proxy_requests_total{status="error"}` ဆူးပေါက်များ။
- **Gateway SLO** – `DocsPortal/GatewayRefusals` သည် alias binding များကို ဆက်လက်သေချာစေသည်
  pinned manifest digest ကို ကြော်ငြာရန်၊ escalations မှ link ကို
  `cargo xtask soradns-verify-binding` CLI စာသားကို ထုတ်ဝေစဉ်အတွင်း ရိုက်ကူးခဲ့သည်။

### 3. သက်သေလမ်းကလေး

စောင့်ကြည့်မှုတစ်ခုစီတိုင်းကို ဖြည့်စွက်သင့်သည်-

- `monitor-publishing` အထောက်အထားအစုအဝေး (`summary.json`၊ အပိုင်းတစ်ပိုင်းဖိုင်များနှင့်၊
  `checksums.sha256`)။
- ထွက်လာသည့်ဝင်းဒိုးပေါ်ရှိ `docs_portal` ဘုတ်အတွက် Grafana ဖန်သားပြင်ဓာတ်ပုံများ။
- ၎င်းကို ပရောက်စီပြောင်းလဲမှု/ပြန်လှည့်မှတ်တမ်းများ (`npm run manage:tryit-proxy` မှတ်တမ်းများ) စမ်းကြည့်ပါ။
- `cargo xtask soradns-verify-binding` မှ Alias ​​အတည်ပြုခြင်းအထွက်။

၎င်းတို့ကို `artifacts/sorafs/<tag>/monitoring/` အောက်တွင် သိမ်းဆည်းပြီး ၎င်းတို့ကို ချိတ်ဆက်ပါ။
CI မှတ်တမ်းများ သက်တမ်းကုန်ပြီးနောက် စာရင်းစစ်လမ်းကြောင်းသည် ဆက်လက်ရှင်သန်နေစေရန် ပြဿနာကို ထုတ်ပြန်ပေးပါသည်။

## လည်ပတ်မှုစာရင်း

1. အဆင့် 7 မှတစ်ဆင့် ဖြန့်ကျက်ခြင်းလမ်းညွှန်ကို လုပ်ဆောင်ပါ။
2. ထုတ်လုပ်မှုပုံစံဖြင့် `npm run monitor:publishing` ကို အကောင်အထည်ဖော်ပါ။ တင်ထားပါတယ်။
   JSON အထွက်။
3. ရိုက်ကူးရန် Grafana အကန့်များ (`docs_portal`၊ `TryItProxyErrors`၊
   `DocsPortal/GatewayRefusals`) နှင့် ၎င်းတို့ကို ထုတ်ဝေခွင့်လက်မှတ်တွင် ပူးတွဲပါရှိသည်။
4. ထပ်တလဲလဲ မော်နီတာများကို အချိန်ဇယားဆွဲပါ (အကြံပြုထားသည်- 15 မိနစ်တိုင်း) ညွှန်ပြသည်
   DOCS-3c SLO ဂိတ်ကို ကျေနပ်စေရန် တူညီသော config ဖြင့် ထုတ်လုပ်သည့် URL များ။
5. အဖြစ်အပျက်များအတွင်း၊ မှတ်တမ်းတင်ရန် `--json-out` ဖြင့် မော်နီတာအမိန့်ကို ပြန်လည်လုပ်ဆောင်ပါ။
   သက်သေမပြမီ/ ပြီးနောက် ရင်ခွဲစစ်ဆေးချက်တွင် ပူးတွဲပါရှိသည်။

ဤကွင်းဆက်ပြီးနောက် DOCS-3c ကို ပိတ်လိုက်သည်- ပေါ်တယ်တည်ဆောက်စီးဆင်းမှု၊ ပိုက်လိုင်းထုတ်ဝေခြင်း၊
ဆက်လက်၍ ပြန်လည်ထုတ်လုပ်နိုင်သော အမိန့်များပါရှိသော တစ်ခုတည်းသော playbook တွင် ယခုအခါ စောင့်ကြည့်လေ့လာခြင်း stack တွင် နေထိုင်လျက်ရှိပါသည်။
နမူနာ configs နှင့် telemetry ချိတ်များ။