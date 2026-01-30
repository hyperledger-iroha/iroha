---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Try It سینڈ باکس

ڈویلپر پورٹل ایک اختیاری "Try it" کنسول فراہم کرتا ہے تاکہ آپ دستاویزات چھوڑے بغیر Torii endpoints کال کر سکیں۔ کنسول بنڈل پروکسی کے ذریعے درخواستیں ری لے کرتا ہے تاکہ براؤزر CORS حدود بائی پاس کر سکیں جبکہ ریٹ لمٹس اور آتھنٹیکیشن نافذ رہیں۔

## شرائط

- Node.js 18.18 یا نیا (پورٹل build ضروریات کے مطابق)
- Torii staging ماحول تک نیٹ ورک رسائی
- ایسا bearer token جو آپ جن Torii routes کو ٹیسٹ کرنا چاہتے ہیں انہیں کال کر سکے

پروکسی کی تمام کنفیگریشن environment variables سے ہوتی ہے۔ نیچے اہم knobs کی فہرست ہے:

| Variable | مقصد | Default |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii کا base URL جس پر پروکسی درخواستیں فارورڈ کرتا ہے | **Required** |
| `TRYIT_PROXY_LISTEN` | لوکل ڈیولپمنٹ کے لئے listen address (`host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | origins کی comma-separated فہرست جو پروکسی کو کال کر سکتے ہیں | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | ہر upstream request میں `X-TryIt-Client` کے طور پر لگایا گیا identifier | `docs-portal` |
| `TRYIT_PROXY_BEARER` | ڈیفالٹ bearer token جو Torii کو فارورڈ ہوتا ہے | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو `X-TryIt-Auth` کے ذریعے اپنا token دینے کی اجازت | `0` |
| `TRYIT_PROXY_MAX_BODY` | request body کا زیادہ سے زیادہ سائز (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | upstream timeout (milliseconds) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | ہر client IP کے لئے فی window اجازت شدہ requests | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | rate limiting کے لئے sliding window (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus طرز metrics endpoint کے لئے اختیاری listen address (`host:port` یا `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | metrics endpoint کا HTTP path | `/metrics` |

پروکسی `GET /healthz` بھی فراہم کرتا ہے، structured JSON errors واپس کرتا ہے، اور bearer tokens کو log output سے redact کرتا ہے۔

`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` فعال کریں جب آپ پروکسی کو docs users کے لئے expose کریں تاکہ Swagger اور RapiDoc panels user-provided bearer tokens فارورڈ کر سکیں۔ پروکسی اب بھی rate limits نافذ کرتا ہے، credentials کو redact کرتا ہے، اور ریکارڈ کرتا ہے کہ request نے default token استعمال کیا یا per-request override۔ `TRYIT_PROXY_CLIENT_ID` کو اس label پر سیٹ کریں جو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(ڈیفالٹ `docs-portal`)۔ پروکسی caller-provided `X-TryIt-Client` values کو trim اور validate کرتا ہے اور اسی default پر واپس آتا ہے تاکہ staging gateways browser metadata سے correlate کئے بغیر provenance audit کر سکیں۔

## لوکل طور پر پروکسی چلائیں

پورٹل سیٹ اپ کرنے کی پہلی بار dependencies انسٹال کریں:

```bash
cd docs/portal
npm install
```

پروکسی چلائیں اور اسے اپنے Torii instance کی طرف پوائنٹ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

اسکرپٹ bound address لاگ کرتا ہے اور `/proxy/*` سے requests کو configured Torii origin کی طرف forward کرتا ہے۔

ساکٹ bind کرنے سے پہلے اسکرپٹ verify کرتا ہے کہ
`static/openapi/torii.json` کا digest `static/openapi/manifest.json` میں ریکارڈ شدہ digest سے میچ کرے۔ اگر فائلز drift ہو جائیں تو کمانڈ error کے ساتھ exit کر دیتا ہے اور `npm run sync-openapi -- --latest` چلانے کی ہدایت دیتا ہے۔ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی override کے لئے استعمال کریں؛ پروکسی warning لاگ کرے گا اور جاری رہے گا تاکہ maintenance windows میں ریکوری ہو سکے۔

## پورٹل widgets کو جوڑیں

جب آپ developer portal کو build یا serve کرتے ہیں تو وہ URL سیٹ کریں جسے widgets پروکسی کے لئے استعمال کریں:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

یہ components `docusaurus.config.js` سے values پڑھتے ہیں:

- **Swagger UI** - `/reference/torii-swagger` پر render ہوتا ہے؛ token موجود ہونے پر bearer scheme کو pre-authorise کرتا ہے، requests کو `X-TryIt-Client` سے tag کرتا ہے، `X-TryIt-Auth` inject کرتا ہے، اور `TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے پر calls کو پروکسی کے ذریعے rewrite کرتا ہے۔
- **RapiDoc** - `/reference/torii-rapidoc` پر render ہوتا ہے؛ token فیلڈ کو mirror کرتا ہے، Swagger panel جیسے headers reuse کرتا ہے، اور URL configure ہونے پر خودکار طور پر proxy کو target کرتا ہے۔
- **Try it console** - API overview page میں embedded ہے؛ custom requests بھیجنے، headers دیکھنے، اور response bodies inspect کرنے دیتا ہے۔

دونوں panels ایک **snapshot selector** دکھاتے ہیں جو
`docs/portal/static/openapi/versions.json` پڑھتا ہے۔ اس index کو
`npm run sync-openapi -- --version=<label> --mirror=current --latest` سے بھریں تاکہ reviewers historical specs میں جا سکیں، ریکارڈ شدہ SHA-256 digest دیکھ سکیں، اور interactive widgets استعمال کرنے سے پہلے تصدیق کر سکیں کہ release snapshot signed manifest لے کر آیا ہے۔

کسی بھی widget میں token بدلنا صرف موجودہ browser session پر اثر ڈالتا ہے؛ proxy کبھی بھی فراہم کردہ token کو persist یا log نہیں کرتا۔

## مختصر مدت والے OAuth tokens

طویل مدت والے Torii tokens reviewers کو دینے سے بچنے کے لئے Try it console کو اپنے OAuth server سے جوڑیں۔ جب نیچے دیے گئے environment variables موجود ہوں تو پورٹل device-code login widget دکھاتا ہے، مختصر مدت والے bearer tokens بناتا ہے، اور انہیں console form میں خودکار طور پر inject کرتا ہے۔

| Variable | مقصد | Default |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Device Authorization endpoint (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token endpoint جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` قبول کرتا ہے | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth client identifier جو docs preview کے لئے رجسٹر ہے | _empty_ |
| `DOCS_OAUTH_SCOPE` | sign-in کے دوران مانگے گئے space-delimited scopes | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | token کو باندھنے کے لئے اختیاری API audience | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم polling interval (ms) | `5000` (قدریں < 5000 ms رد ہوتی ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | device-code کی fallback expiration window (seconds) | `600` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | access-token کی fallback lifetime (seconds) | `900` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | لوکل previews کے لئے `1` جو OAuth enforcement جان بوجھ کر چھوڑتے ہیں | _unset_ |

Example configuration:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو پورٹل یہ values `docusaurus.config.js` میں embed کرتا ہے۔ لوکل preview کے دوران Try it card "Sign in with device code" بٹن دکھاتا ہے۔ صارفین دکھایا گیا code اپنی OAuth verification page پر درج کرتے ہیں؛ device flow کامیاب ہونے کے بعد widget:

- جاری شدہ bearer token کو Try it console فیلڈ میں inject کرتا ہے،
- موجودہ `X-TryIt-Client` اور `X-TryIt-Auth` headers کے ساتھ requests tag کرتا ہے،
- باقی ماندہ مدت دکھاتا ہے، اور
- token ختم ہونے پر خودکار طور پر صاف کر دیتا ہے۔

manual Bearer input دستیاب رہتا ہے - OAuth variables کو چھوڑ دیں جب آپ reviewers کو عارضی token خود paste کرنے پر مجبور کرنا چاہتے ہیں، یا `DOCS_OAUTH_ALLOW_INSECURE=1` export کریں تاکہ isolated لوکل previews میں anonymous access قابل قبول ہو۔ OAuth کے بغیر builds اب DOCS-1b roadmap gate پورا کرنے کے لئے فوراً fail ہو جاتے ہیں۔

Note: پورٹل کو لیب سے باہر expose کرنے سے پہلے [Security hardening & pen-test checklist](./security-hardening.md) دیکھیں؛ اس میں threat model، CSP/Trusted Types پروفائل، اور pen-test steps شامل ہیں جو اب DOCS-1b کو gate کرتے ہیں۔

## Norito-RPC نمونے

Norito-RPC requests اسی proxy اور OAuth plumbing کو شیئر کرتی ہیں جیسے JSON routes؛ یہ صرف `Content-Type: application/x-norito` سیٹ کرتی ہیں اور NRPC specification میں بیان کردہ pre-encoded Norito payload بھیجتی ہیں
(`docs/source/torii/nrpc_spec.md`)۔ ریپو میں `fixtures/norito_rpc/` کے تحت canonical payloads موجود ہیں تاکہ portal authors، SDK owners، اور reviewers وہی bytes replay کر سکیں جو CI استعمال کرتا ہے۔

### Try It console سے Norito payload بھیجیں

1. `fixtures/norito_rpc/transfer_asset.norito` جیسا fixture منتخب کریں۔ یہ فائلیں raw Norito envelopes ہیں؛ انہیں **base64 encode نہ کریں**۔
2. Swagger یا RapiDoc میں NRPC endpoint تلاش کریں (مثلاً `POST /v1/pipeline/submit`) اور **Content-Type** selector کو `application/x-norito` پر سوئچ کریں۔
3. request body editor کو **binary** پر ٹوگل کریں (Swagger کا "File" موڈ یا RapiDoc کا "Binary/File" selector) اور `.norito` فائل اپ لوڈ کریں۔ widget bytes کو proxy کے ذریعے بغیر تبدیلی کے stream کرتا ہے۔
4. request بھیجیں۔ اگر Torii `X-Iroha-Error-Code: schema_mismatch` واپس کرے تو تصدیق کریں کہ آپ ایسا endpoint کال کر رہے ہیں جو binary payloads قبول کرتا ہے اور `fixtures/norito_rpc/schema_hashes.json` میں ریکارڈ شدہ schema hash آپ کے Torii build سے میچ کرتا ہے۔

کنسول تازہ ترین فائل میموری میں رکھتا ہے تاکہ آپ مختلف authorization tokens یا Torii hosts کے ساتھ وہی payload دوبارہ بھیج سکیں۔ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کو workflow میں شامل کرنے سے NRPC-4 adoption plan میں حوالہ دیا گیا evidence bundle (log + JSON summary) تیار ہوتا ہے، جو reviews کے دوران Try It response کے screenshot کے ساتھ اچھی طرح جاتا ہے۔

### CLI مثال (curl)

وہی fixtures `curl` کے ذریعے پورٹل سے باہر بھی replay کیے جا سکتے ہیں، جو proxy validate کرنے یا gateway responses debug کرنے میں مددگار ہے:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں موجود کسی بھی entry کے ساتھ fixture بدلیں یا `cargo xtask norito-rpc-fixtures` سے اپنا payload encode کریں۔ جب Torii canary mode میں ہو تو آپ `curl` کو try-it proxy (`https://docs.sora.example/proxy/v1/pipeline/submit`) پر پوائنٹ کر سکتے ہیں تاکہ وہی infrastructure test ہو جو portal widgets استعمال کرتے ہیں۔

## Observability اور operations

ہر request ایک بار method، path، origin، upstream status، اور authentication source (`override`, `default`، یا `client`) کے ساتھ log ہوتی ہے۔ tokens کبھی store نہیں ہوتے - bearer headers اور `X-TryIt-Auth` values logging سے پہلے redact ہو جاتی ہیں، اس لئے آپ stdout کو central collector میں forward کر سکتے ہیں بغیر secrets leak ہونے کے خدشے کے۔

### Health probes اور alerting

deployments کے دوران یا schedule پر bundled probe چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Environment knobs:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری Torii route (بغیر `/proxy`) جسے چیک کرنا ہو۔
- `TRYIT_PROXY_SAMPLE_METHOD` - ڈیفالٹ `GET`؛ write routes کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - sample call کے لئے عارضی bearer token inject کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ڈیفالٹ 5 s timeout کو override کرتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus textfile destination۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے comma-separated metrics میں شامل ہوتے ہیں (ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختیاری metrics endpoint URL (مثلاً `http://localhost:9798/metrics`) جو `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی سے جواب دے۔

نتائج کو textfile collector میں فیڈ کریں، probe کو writeable path پر پوائنٹ کر کے
(مثلاً `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور custom labels شامل کر کے:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ metrics فائل کو atomically rewrite کرتا ہے تاکہ collector ہمیشہ مکمل payload پڑھے۔

جب `TRYIT_PROXY_METRICS_LISTEN` configure ہو تو `TRYIT_PROXY_PROBE_METRICS_URL` کو metrics endpoint پر سیٹ کریں تاکہ probe تیزی سے fail ہو اگر scrape surface غائب ہو جائے (مثلاً misconfigured ingress یا missing firewall rules)۔ ایک typical production setting ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

لائٹ ویٹ alerting کے لئے probe کو اپنے monitoring stack میں جوڑیں۔ Prometheus مثال جو دو مسلسل failures کے بعد page کرتی ہے:

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

### Metrics endpoint اور dashboards

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی host/port) سیٹ کریں اور پھر proxy start کریں تاکہ Prometheus-formatted metrics endpoint expose ہو۔ path ڈیفالٹ `/metrics` ہے لیکن `TRYIT_PROXY_METRICS_PATH=/custom` سے بدل سکتے ہیں۔ ہر scrape method-wise totals، rate-limit rejections، upstream errors/timeouts، proxy outcomes، اور latency summaries واپس کرتا ہے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

اپنے Prometheus/OTLP collectors کو metrics endpoint پر پوائنٹ کریں اور `dashboards/grafana/docs_portal.json` کے existing panels کو reuse کریں تاکہ SRE tail latencies اور rejection spikes کو logs parse کیے بغیر دیکھ سکے۔ proxy خودکار طور پر `tryit_proxy_start_timestamp_ms` publish کرتا ہے تاکہ operators restarts detect کر سکیں۔

### Rollback automation

management helper استعمال کریں تاکہ Torii target URL کو update یا restore کیا جا سکے۔ اسکرپٹ پچھلی configuration کو `.env.tryit-proxy.bak` میں محفوظ کرتا ہے تاکہ rollback ایک ہی کمانڈ ہو۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

اگر آپ کی deployment configuration کہیں اور محفوظ کرتی ہے تو `--env` یا `TRYIT_PROXY_ENV` سے env فائل کا راستہ override کریں۔
