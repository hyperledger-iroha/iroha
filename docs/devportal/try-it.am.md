---
lang: am
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

የገንቢው ፖርታል ለTorii REST API የ"ሞክሩት" ኮንሶል ይልካል። ይህ መመሪያ
ደጋፊ ፕሮክሲውን እንዴት ማስጀመር እና ኮንሶሉን ከመድረክ ጋር ማገናኘት እንደሚቻል ያብራራል።
ምስክርነቶችን ሳያጋልጥ መግቢያ.

## ቅድመ ሁኔታዎች

- Iroha ማከማቻ ፍተሻ (የስራ ቦታ ስር)።
- Node.js 18.18+ (ከፖርታል መነሻ መስመር ጋር ይዛመዳል)።
- Torii የመጨረሻ ነጥብ ከእርስዎ የስራ ጣቢያ (የማዘጋጀት ወይም የአካባቢ) ሊደረስበት የሚችል።

## 1. የOpenAPI ቅጽበታዊ ገጽ እይታን ይፍጠሩ (አማራጭ)

ኮንሶሉ ልክ እንደ ፖርታል ማመሳከሪያ ገፆች ተመሳሳይ OpenAPI ክፍያን እንደገና ይጠቀማል። ከሆነ
የ Torii መንገዶችን ቀይረሃል፣ ቅጽበተ-ፎቶውን እንደገና አሳድገው፡

```bash
cargo xtask openapi
```

ተግባሩ `docs/portal/static/openapi/torii.json` ይጽፋል።

## 2. ሞክር ፕሮክሲውን ይጀምሩ

ከማከማቻ ስር፡-

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### የአካባቢ ተለዋዋጮች

| ተለዋዋጭ | መግለጫ |
|-------|-----------|
| `TRYIT_PROXY_TARGET` | Torii ቤዝ ዩአርኤል (የሚያስፈልግ)። |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | በነጠላ ሰረዝ የተለዩ የመነሻዎች ዝርዝር ተኪውን እንዲጠቀም ተፈቅዶለታል (የ `http://localhost:3000` ነባሪዎች)። |
| `TRYIT_PROXY_BEARER` | አማራጭ ነባሪ ተሸካሚ ማስመሰያ በሁሉም የተኪ ጥያቄዎች ላይ ተተግብሯል። |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | የደዋዩን `Authorization` ራስጌ በቃል ለማስተላለፍ ወደ `1` ያቀናብሩ። |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | የማህደረ ትውስታ መጠን ገዳቢ ቅንብሮች (ነባሪዎች፡ በ60ዎቹ 60 ጥያቄዎች)። |
| `TRYIT_PROXY_MAX_BODY` | ከፍተኛው የጥያቄ ጭነት ተቀባይነት አለው (ባይት፣ ነባሪ 1ሚቢ)። |
| `TRYIT_PROXY_TIMEOUT_MS` | ለI18NT0000009X ጥያቄዎች (ነባሪ 10000ms) የወቅቱ ጊዜ ማብቂያ ጊዜ። |

ፕሮክሲው ያጋልጣል፡-

- `GET /healthz` - ዝግጁነት ማረጋገጥ.
- `/proxy/*` - የተኪ ጥያቄዎች፣ መንገዱን እና የመጠይቅ ሕብረቁምፊን በመጠበቅ።

## 3. መግቢያውን ያስጀምሩ

በተለየ ተርሚናል ውስጥ፡-

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` ይጎብኙ እና ይሞክሩት ኮንሶል ይጠቀሙ። ተመሳሳይ
የአካባቢ ተለዋዋጮች Swagger UI እና RapiDoc መክተቻዎችን ያዋቅራሉ።

## 4. የሩጫ ክፍል ሙከራዎች

ተኪው ፈጣን በመስቀለኛ መንገድ ላይ የተመሰረተ የሙከራ ስብስብ ያጋልጣል፡

```bash
npm run test:tryit-proxy
```

ፈተናዎቹ የአድራሻ መተንተንን፣ የመነሻ አያያዝን፣ ተመን መገደብን እና ተሸካሚን ይሸፍናሉ።
መርፌ.

## 5. የመርማሪ አውቶማቲክ እና መለኪያዎች

`/healthz` እና የናሙና የመጨረሻ ነጥብ ለማረጋገጥ የተጠቀለለውን ምርመራ ይጠቀሙ፡-

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

የአካባቢ ቁልፎች;

- `TRYIT_PROXY_SAMPLE_PATH` — አማራጭ Torii መንገድ (ያለ `/proxy`) የአካል ብቃት እንቅስቃሴ።
- `TRYIT_PROXY_SAMPLE_METHOD` - ለ `GET` ነባሪዎች; ለመጻፍ መንገዶች ወደ `POST` ተዘጋጅቷል።
- `TRYIT_PROXY_PROBE_TOKEN` - ለናሙና ጥሪ ጊዜያዊ ተሸካሚ ማስመሰያ ያስገባል።
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ነባሪውን የ 5s ጊዜ ማብቂያ ይሽራል።
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus የጽሑፍ ፋይል መድረሻ ለI18NI0000044X/I18NI0000045X።
- `TRYIT_PROXY_PROBE_LABELS` — በነጠላ ሰረዝ የተለዩ `key=value` ጥንዶች በመለኪያዎቹ ላይ ተያይዘዋል (የ`job=tryit-proxy` እና `instance=<proxy URL>` ነባሪዎች)።

`TRYIT_PROXY_PROBE_METRICS_FILE` ሲዋቀር ስክሪፕቱ ፋይሉን እንደገና ይጽፋል
በአቶሚካል ስለዚህ የእርስዎ node_exporter/textፋይል ሰብሳቢ ሁል ጊዜ የተሟላ ሆኖ እንዲያይ
ጭነት. ምሳሌ፡-

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

የተገኙትን መለኪያዎች ወደ Prometheus ያስተላልፉ እና የናሙና ማንቂያውን እንደገና ይጠቀሙ
ገንቢ-ፖርታል ሰነዶች `probe_success` ወደ `0` ሲወርድ።

## 6. የምርት ማጠንከሪያ ዝርዝር

ከአካባቢ ልማት በላይ ፕሮክሲውን ከማተምዎ በፊት፡-

- TLSን ከፕሮክሲው ቀድመው ያቋርጡ (ተገላቢጦሽ ተኪ ወይም የሚተዳደር መግቢያ በር)።
- የተዋቀረ ምዝግብ ማስታወሻን ያዋቅሩ እና ወደ ታዛቢነት ቧንቧዎች ያስተላልፉ።
- ተሸካሚዎችን ያሽከርክሩ እና በሚስጥር አስተዳዳሪዎ ውስጥ ያከማቹ።
- የተኪውን `/healthz` የመጨረሻ ነጥብ ይቆጣጠሩ እና የቆይታ ጊዜ መለኪያዎችን ያዋህዱ።
- የዋጋ ገደቦችን ከእርስዎ Torii የማዘጋጃ ኮታዎች ጋር ያስተካክሉ። `Retry-After` አስተካክል
  ጉሮሮዎችን ከደንበኞች ጋር የመግባባት ባህሪ ።