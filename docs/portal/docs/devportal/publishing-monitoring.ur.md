---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: SoraFS اشاعت اور مانیٹرنگ
sidebar_label: اشاعت اور مانیٹرنگ
description: SoraFS پورٹل ریلیزز کے لئے end-to-end مانیٹرنگ فلو کو ریکارڈ کریں تاکہ DOCS-3c کے پاس deterministic probes، ٹیلیمیٹری، اور evidence bundles ہوں۔
---

روڈ میپ آئٹم **DOCS-3c** صرف پیکیجنگ چیک لسٹ سے زیادہ مانگتا ہے: ہر SoraFS اشاعت کے بعد ہمیں
مسلسل ثابت کرنا ہوتا ہے کہ developer portal، Try it proxy، اور gateway bindings صحت مند ہیں۔
یہ صفحہ [deployment guide](./deploy-guide.md) کے ساتھ آنے والی مانیٹرنگ سطح کو دستاویز کرتا ہے
تاکہ CI اور on call انجینئرز وہی چیکس چلا سکیں جو Ops SLO نافذ کرنے کے لئے استعمال کرتی ہے۔

## Pipeline recap

1. **Build اور sign** - [deployment guide](./deploy-guide.md) کے مطابق
   `npm run build`, `scripts/preview_wave_preflight.sh`, اور Sigstore + manifest submission steps چلائیں۔
   preflight اسکرپٹ `preflight-summary.json` بناتا ہے تاکہ ہر preview کے پاس build/link/probe metadata ہو۔
2. **Pin اور verify** - `sorafs_cli manifest submit`, `verify-sorafs-binding.mjs`,
   اور DNS cutover plan governance کے لئے deterministic artefacts فراہم کرتے ہیں۔
3. **Evidence archive** - CAR summary، Sigstore bundle، alias proof، probe output، اور
   `docs_portal.json` ڈیش بورڈ snapshots کو `artifacts/sorafs/<tag>/` کے تحت محفوظ کریں۔

## Monitoring channels

### 1. Publishing monitors (`scripts/monitor-publishing.mjs`)

نیا `npm run monitor:publishing` کمانڈ portal probe، Try it proxy probe، اور bindings verifier
کو ایک CI-friendly check میں جوڑتا ہے۔ JSON config فراہم کریں
(CI secrets یا `configs/docs_monitor.json` میں محفوظ) اور چلائیں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` (اور اختیاری
`--prom-job docs-preview`) شامل کریں تاکہ Prometheus text-format metrics نکلیں
جو Pushgateway یا staging/production میں براہ راست scrapes کے لئے مناسب ہوں۔
یہ metrics JSON summary کو mirror کرتی ہیں تاکہ SLO dashboards اور alert rules
portal، Try it، bindings اور DNS کی صحت کو evidence bundle parse کئے بغیر ٹریک کر سکیں۔

ضروری knobs اور متعدد bindings کے ساتھ config مثال:

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
    "samplePath": "/proxy/v1/accounts/wonderland@wonderland/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/manifest",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiaff84aef0aaaf6a7c246c8ca1889e62d69c8d9b20d94933cb7b09902f3",
      "manifest": "8b8f3d2a4a7e92abdb17e5fafd4f9d67c6c7a8547ff985bb0d71f87209c1444d",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "openapi",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/openapi",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeidevopenapi",
      "manifest": "dad4b9fd48e35297c7fd71cd15b52c4ff0bb62dd8a1da4c5c2c1536ae2732b55",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "portal-sbom",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/portal-sbom",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiportalssbom",
      "manifest": "e2b2790f9f4c1ecbc8f1bdb9f8ba3fd65fd687e9e5e4de3c3d67c3d3192b79c8",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
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

مانیٹر ایک JSON summary (S3/SoraFS friendly) لکھتا ہے اور جب کوئی probe fail ہو تو non-zero کے ساتھ
ختم ہوتا ہے، جس سے یہ Cron jobs، Buildkite steps، یا Alertmanager webhooks کے لئے موزوں بنتا ہے۔
`--evidence-dir` دینے سے `summary.json`, `portal.json`, `tryit.json`, اور `binding.json`
`checksums.sha256` manifest کے ساتھ محفوظ ہوتے ہیں تاکہ governance reviewers probes دوبارہ چلائے بغیر
نتائج compare کر سکیں۔

> **TLS guardrail:** `monitorPortal` `http://` base URLs کو رد کرتا ہے جب تک
> `allowInsecureHttp: true` config میں نہ ہو۔ production/staging probes کو HTTPS پر رکھیں؛
> یہ اختیار صرف local previews کے لئے ہے۔

ہر binding entry `Sora-Name`, `Sora-Proof`, اور `Sora-Content-CID`
(headers اور payload) کے ساتھ `expectHost` guard لاگو کرتی ہے تاکہ DNS promotion
(`docs.sora` بمقابلہ `docs-preview.sora.link`) pin registry میں درج alias سے نہ ہٹے۔
اگر gateway `Sora-Content-CID`/`Sora-Proof` headers لگانا بند کر دے،
proof میں غلط base64 آجائے، یا advertised manifest/CID pinned payloads
(سائٹ، OpenAPI، اور SBOM) سے ہٹ جائیں تو checks فوراً fail ہو جاتے ہیں۔

اختیاری `dns` بلاک DOCS-7 کے SoraDNS rollout کو اسی monitor میں جوڑتا ہے۔ ہر entry
hostname/record-type جوڑی resolve کرتی ہے (مثلاً CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) اور تصدیق کرتی ہے
کہ جواب `expectedRecords` یا `expectedIncludes` سے ملتے ہیں۔ اوپر کے snippet میں دوسری entry
`cargo xtask soradns-hosts --name docs-preview.sora.link` سے بننے والا canonical hashed hostname فکس کرتی ہے؛
اب monitor ثابت کرتا ہے کہ friendly alias اور canonical hash (`igjssx53...gw.sora.id`) دونوں
pinned pretty host پر resolve ہوتے ہیں۔ اس سے DNS promotion evidence خودکار ہو جاتی ہے:
اگر کسی host میں drift ہو تو monitor fail ہوگا، چاہے HTTP bindings درست manifest stapling کر رہے ہوں۔

### 2. OpenAPI version manifest guard

DOCS-2b کی "signed OpenAPI manifest" ضرورت اب automated guard کے ساتھ آتی ہے:
`ci/check_openapi_spec.sh` `npm run check:openapi-versions` چلاتا ہے، جو
`scripts/verify-openapi-versions.mjs` کے ذریعے
`docs/portal/static/openapi/versions.json` کو Torii specs اور manifests سے cross-check کرتا ہے۔
یہ guard چیک کرتا ہے کہ:

- `versions.json` میں ہر version کے لئے `static/openapi/versions/` کے نیچے matching directory ہو۔
- `bytes` اور `sha256` fields on-disk spec file سے match کریں۔
- `latest` alias `current` entry (digest/size/signature metadata) کو reflect کرے تاکہ default download drift نہ کرے۔
- signed entries ایسے manifest کو refer کریں جن کا `artifact.path` اسی spec کی طرف اشارہ کرے اور
  signature/public key hex values manifest سے match کریں۔

نئی spec mirror کرنے پر guard مقامی طور پر چلائیں:

```bash
cd docs/portal
npm run check:openapi-versions
```

Failure messages میں stale-file hint (`npm run sync-openapi -- --latest`) شامل ہوتا ہے
تاکہ portal contributors جان سکیں کہ snapshots کیسے refresh کرنے ہیں۔
CI میں guard رکھنے سے ایسے portal releases رک جاتے ہیں جہاں signed manifest اور published digest sync سے باہر ہوں۔

### 2. Dashboards اور alerts

- **`dashboards/grafana/docs_portal.json`** - DOCS-3c کے لئے primary board۔ panels
  `torii_sorafs_gateway_refusals_total`, replication SLA misses, Try it proxy errors,
  اور probe latency (`docs.preview.integrity` overlay) کو track کرتے ہیں۔ ہر release کے بعد board export کریں
  اور operations ticket میں attach کریں۔
- **Try it proxy alerts** - Alertmanager rule `TryItProxyErrors` sustained
  `probe_success{job="tryit-proxy"}` drops یا `tryit_proxy_requests_total{status="error"}` spikes پر fire ہوتی ہے۔
- **Gateway SLO** - `DocsPortal/GatewayRefusals` اس بات کو یقینی بناتا ہے کہ alias bindings
  pinned manifest digest advertise کرتے رہیں؛ escalations `verify-sorafs-binding.mjs` CLI transcript کی طرف اشارہ کرتی ہیں
  جو publish کے دوران capture ہوا تھا۔

### 3. Evidence trail

ہر monitoring run میں یہ شامل ہونا چاہئے:

- `monitor-publishing` evidence bundle (`summary.json`, per-section files, اور `checksums.sha256`).
- Grafana screenshots for `docs_portal` board release window کے دوران۔
- Try it proxy change/rollback transcripts (`npm run manage:tryit-proxy` logs).
- `scripts/verify-sorafs-binding.mjs` سے alias verification output.

انہیں `artifacts/sorafs/<tag>/monitoring/` میں رکھیں اور release issue میں link کریں
تاکہ audit trail CI logs کے ختم ہونے کے بعد بھی موجود رہے۔

## Operational checklist

1. Deployment guide کو Step 7 تک چلائیں۔
2. `npm run monitor:publishing` کو production configuration کے ساتھ چلائیں؛ JSON output archive کریں۔
3. Grafana panels (`docs_portal`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capture کریں اور release ticket میں attach کریں۔
4. Recurring monitors (سفارش: ہر 15 منٹ) production URLs پر اسی config کے ساتھ schedule کریں تاکہ DOCS-3c SLO gate پورا ہو۔
5. Incidents کے دوران monitor command کو `--json-out` کے ساتھ دوبارہ چلائیں تاکہ before/after evidence ریکارڈ ہو اور postmortem میں attach ہو۔

یہ لوپ follow کرنے سے DOCS-3c بند ہو جاتا ہے: portal build flow، publishing pipeline، اور monitoring stack
اب ایک ہی playbook میں ہیں جس میں reproducible commands، sample configs، اور telemetry hooks شامل ہیں۔
