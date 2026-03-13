---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ማጠሪያውን ይሞክሩት።

Torii መደወል እንዲችሉ የገንቢው ፖርታል አማራጭ የሆነውን “ሞክሩት” ኮንሶል ይልካል።
ሰነዶቹን ሳይለቁ የመጨረሻ ነጥቦች. ኮንሶሉ ጥያቄዎችን ያስተላልፋል
አሳሾች አሁንም የCORS ገደቦችን ማለፍ እንዲችሉ በተጠቀጠቀ ፕሮክሲ በኩል
የዋጋ ገደቦችን እና ማረጋገጫን ማስፈጸም።

## ቅድመ ሁኔታዎች

- Node.js 18.18 ወይም ከዚያ በላይ (ከፖርታል ግንባታ መስፈርቶች ጋር ይዛመዳል)
- ወደ Torii የማዘጋጀት አካባቢ የአውታረ መረብ መዳረሻ
- የአካል ብቃት እንቅስቃሴ ለማድረግ ያቀዱትን Torii መስመሮችን መደወል የሚችል ተሸካሚ ቶከን

ሁሉም የተኪ ውቅር የሚከናወነው በአካባቢ ተለዋዋጮች ነው። ከታች ያለው ሰንጠረዥ
በጣም አስፈላጊ የሆኑትን እንክብሎች ይዘረዝራል-

| ተለዋዋጭ | ዓላማ | ነባሪ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | ተኪው የሚያስተላልፈው Base Torii URL ወደ | ** ያስፈልጋል *** |
| `TRYIT_PROXY_LISTEN` | ለአካባቢ ልማት አድራሻ ያዳምጡ (ቅርጸት `host:port` ወይም `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | ተኪ ሊጠሩ የሚችሉ በነጠላ ሰረዝ የተለዩ የመነሻ ዝርዝር | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | ነባሪ ተሸካሚ ማስመሰያ ወደ Torii ተላልፏል ባዶ_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | የመጨረሻ ተጠቃሚዎች የራሳቸውን ማስመሰያ በ`X-TryIt-Auth` በኩል እንዲያቀርቡ ይፍቀዱላቸው | `0` |
| `TRYIT_PROXY_MAX_BODY` | ከፍተኛው የጥያቄ የሰውነት መጠን (ባይት) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ወደላይ የሚያልፍበት ጊዜ በሚሊሰከንዶች | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | የተፈቀደላቸው ጥያቄዎች በእያንዳንዱ የዋጋ መስኮት በደንበኛ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | ለተመን ገደብ (ሚሴ) ተንሸራታች መስኮት | `60000` |

ፕሮክሲው `GET /healthz`ን ያጋልጣል፣ የተዋቀሩ የJSON ስህተቶችን ይመልሳል እና
ተሸካሚ ምልክቶችን ከሎግ ውፅዓት ያድሳል።

## ፕሮክሲውን በአገር ውስጥ ይጀምሩ

ፖርታሉን ለመጀመሪያ ጊዜ ሲያዘጋጁ ጥገኞችን ይጫኑ፡-

```bash
cd docs/portal
npm install
```

ፕሮክሲውን ያሂዱ እና በእርስዎ Torii ምሳሌ ላይ ያመልክቱ፡

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

ስክሪፕቱ የታሰረውን አድራሻ ይመዘግባል እና ጥያቄዎችን ከ `/proxy/*` ወደ
የተዋቀረው Torii መነሻ።

## የፖርታል መግብሮችን ሽቦ ያድርጉ

የገንቢ ፖርታልን ሲገነቡ ወይም ሲያገለግሉ መግብሮቹ የሚያዘጋጁትን ዩአርኤል ያዘጋጁ
ለፕሮክሲው መጠቀም ያለበት፡-

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

የሚከተሉት ክፍሎች እነዚህን እሴቶች ከ`docusaurus.config.js` ያንብቡ፡-

- ** Swagger UI *** - በ `/reference/torii-swagger` ላይ የቀረበ; ጥያቄ ይጠቀማል
  የተሸካሚ ምልክቶችን በራስ-ሰር ለማያያዝ interceptor።
- ** RapiDoc *** - በ `/reference/torii-rapidoc` ላይ የቀረበ; የማስመሰያ መስክን ያንጸባርቃል
  እና በፕሮክሲው ላይ መሞከርን ይደግፋል።
- ** ኮንሶል ይሞክሩት *** - በኤፒአይ አጠቃላይ እይታ ገጽ ላይ የተካተተ; ብጁ እንድትልክ ያስችልሃል
  ጥያቄዎችን፣ ራስጌዎችን ይመልከቱ፣ እና የምላሽ አካላትን ይፈትሹ።

በማንኛውም መግብር ውስጥ ማስመሰያ መቀየር የአሁኑን የአሳሽ ክፍለ ጊዜ ብቻ ነው የሚነካው; የ
ፕሮክሲ በፍፁም አይቆይም ወይም የቀረበውን ማስመሰያ አይመዘግብም።

## ታዛቢነት እና ተግባራት

እያንዳንዱ ጥያቄ አንድ ጊዜ በዘዴ፣ በዱካ፣ በመነሻ፣ በከፍታ ሁኔታ እና በ
የማረጋገጫ ምንጭ (`override`፣ `default`፣ ወይም `client`)። ማስመሰያዎች በጭራሽ አይደሉም
የተከማቸ - ሁለቱም ተሸካሚ ራስጌዎች እና `X-TryIt-Auth` እሴቶች ከዚህ በፊት ተስተካክለዋል
ሎግ (ሎግ)—ስለዚህ ሳይጨነቁ stdoutን ወደ ማዕከላዊ ሰብሳቢ ማስተላለፍ ይችላሉ።
ሚስጥሮች የሚያፈሱ.

### የጤና ምርመራ እና ማስጠንቀቂያበማሰማራት ጊዜ ወይም በጊዜ መርሐግብር የታሸገውን ምርመራ ያሂዱ፡-

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

የአካባቢ ቁልፎች;

- `TRYIT_PROXY_SAMPLE_PATH` — አማራጭ Torii መንገድ (ያለ `/proxy`) የአካል ብቃት እንቅስቃሴ።
- `TRYIT_PROXY_SAMPLE_METHOD` - ለ `GET` ነባሪዎች; ለመጻፍ መንገዶች ወደ `POST` ተዘጋጅቷል።
- `TRYIT_PROXY_PROBE_TOKEN` - ለናሙና ጥሪ ጊዜያዊ ተሸካሚ ማስመሰያ ያስገባል።
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ነባሪውን የ 5s ጊዜ ማብቂያ ይሽራል።
- `TRYIT_PROXY_PROBE_METRICS_FILE` — አማራጭ Prometheus የጽሑፍ ፋይል መድረሻ ለ`probe_success`/`probe_duration_seconds`።
- `TRYIT_PROXY_PROBE_LABELS` — በነባሪ ሰረዝ የተለዩ `key=value` ጥንዶች በመለኪያዎቹ ላይ ተያይዘዋል (የ`job=tryit-proxy` እና `instance=<proxy URL>` ነባሪዎች)።

መፈተሻውን በተፃፈ ጽሑፍ ላይ በመጠቆም ውጤቱን ወደ ጽሑፍ ፋይል ሰብሳቢ ይመግቡ
መንገድ (ለምሳሌ `/var/lib/node_exporter/textfile_collector/tryit.prom`) እና
ማንኛውንም ብጁ መለያዎች ማከል

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

ስክሪፕቱ የመለኪያ ፋይሉን በአቶሚክ በድጋሚ ይጽፋል ስለዚህ ሰብሳቢዎ ሁል ጊዜ ሀ
ሙሉ ክፍያ.

ለቀላል ክብደት ማንቂያ፣ ፍተሻውን በክትትል ቁልልዎ ላይ ሽቦ ያድርጉት። አንድ Prometheus
ለምሳሌ ያ ገጾች ከሁለት ተከታታይ ውድቀቶች በኋላ

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

### የጥቅልል አውቶማቲክ

ዒላማውን Torii URL ለማዘመን ወይም ወደነበረበት ለመመለስ የአስተዳደር አጋዥን ይጠቀሙ። ስክሪፕቱ
የቀደመውን ውቅር በ`.env.tryit-proxy.bak` ያከማቻል ስለዚህ መልሶ መመለሻዎች ሀ
ነጠላ ትዕዛዝ.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

ከተሰማሩ የኢንቪ ፋይል ዱካውን በ`--env` ወይም `TRYIT_PROXY_ENV` ይሽሩት
ውቅረትን በሌላ ቦታ ያከማቻል.