---
lang: am
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ፖርታሉ ትራፊክን ወደ I18NT0000009X የሚያስተላልፍ ሶስት መስተጋብራዊ ንጣፎችን ያጠቃልላል፡

- **Swagger UI** በ `/reference/torii-swagger` የተፈረመውን OpenAPI spec ያቀርባል እና `TRYIT_PROXY_PUBLIC_URL` ሲቀናበር በፕሮክሲው በኩል በራስ ሰር ይጽፋል።
- **RapiDoc** በ `/reference/torii-rapidoc` ላይ ለ`/reference/torii-rapidoc` በደንብ የሚሰሩ የፋይል ሰቀላዎች እና የይዘት አይነት መምረጫዎችን ያጋልጣል።
- **ማጠሪያ ሞክረው** በI18NT0000004X አጠቃላይ እይታ ገጽ ለአድ-ሆክ REST ጥያቄዎች እና ለOAuth-device መግቢያዎች ቀላል ክብደት ያለው ቅጽ ያቀርባል።

ሶስቱም መግብሮች ለአካባቢው **ሙከራ-ተኪ** (`docs/portal/scripts/tryit-proxy.mjs`) ጥያቄዎችን ይልካሉ። ፕሮክሲው I18NI0000026X በ`static/openapi/torii.json` ከተፈረመው መፈጨት ጋር የሚዛመድ መሆኑን ያረጋግጣል፣ተመን የሚገድብ መሆኑን ያስፈጽማል፣በምዝግብ ማስታወሻዎች ውስጥ የ`X-TryIt-Auth` አርዕስትን ይቀይራል እና እያንዳንዱን የወዲያ ጥሪ በ`X-TryIt-Client` ከዋኝ ኦፕሬተር ኦዲት ማድረግ ይችላል።

## ፕሮክሲውን ያስጀምሩ

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

- `TRYIT_PROXY_TARGET` የአካል ብቃት እንቅስቃሴ ማድረግ የሚፈልጉት የ Torii መሰረት ዩአርኤል ነው።
- `TRYIT_PROXY_ALLOWED_ORIGINS` ኮንሶሉን መክተት ያለበትን እያንዳንዱን የፖርታል መነሻ (አካባቢያዊ ዴቭ አገልጋይ፣ የምርት አስተናጋጅ ስም፣ ቅድመ እይታ URL) ማካተት አለበት።
- `TRYIT_PROXY_PUBLIC_URL` በI18NI0000033X ተበላ እና በ `customFields.tryIt` በኩል ወደ መግብሮች ገብቷል።
- `TRYIT_PROXY_BEARER` `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ሲጭን ብቻ; አለበለዚያ ተጠቃሚዎች የራሳቸውን ማስመሰያ በኮንሶል ወይም OAuth መሳሪያ ፍሰት በኩል ማቅረብ አለባቸው።
- `TRYIT_PROXY_CLIENT_ID` በእያንዳንዱ ጥያቄ ላይ የሚደረገውን የI18NI0000038X መለያ ያዘጋጃል።
  ከአሳሹ `X-TryIt-Client` ማቅረብ ይፈቀዳል ነገር ግን እሴቶች ተቆርጠዋል
  እና የቁጥጥር ቁምፊዎችን ከያዙ ውድቅ ተደርጓል።

በሚነሳበት ጊዜ ፕሮክሲው `verifySpecDigest` ያሂዳል እና አንጸባራቂው የቆየ ከሆነ በማሻሻያ ፍንጭ ይወጣል። አዲሱን የI18NT0000012X ስፔስፊኬሽን ለማውረድ `npm run sync-openapi -- --latest` ያሂዱ ወይም ለአደጋ ጊዜ መሻር `TRYIT_PROXY_ALLOW_STALE_SPEC=1` ማለፍ።

የአካባቢ ፋይሎችን በእጅ ሳያርትዑ የተኪ ኢላማውን ለማዘመን ወይም ወደነበረበት ለመመለስ ረዳቱን ይጠቀሙ፡-

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## መግብሮችን ሽቦ ያድርጉ

ተኪው ካዳመጠ በኋላ ፖርታሉን ያገልግሉ፡-

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` የሚከተሉትን እንቡጦች ያጋልጣል፡

| ተለዋዋጭ | ዓላማ |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | ዩአርኤል ወደ Swagger፣ RapiDoc እና ይሞክሩት ማጠሪያ ውስጥ ገብቷል። ያልተዘጋጁ ቅድመ-እይታዎች ጊዜ መግብሮችን ለመደበቅ እንዳልተዋቀሩ ይተዉት። |
| `TRYIT_PROXY_DEFAULT_BEARER` | በማህደረ ትውስታ ውስጥ የተከማቸ አማራጭ ነባሪ ማስመሰያ። `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` እና HTTPS-ብቻ CSP ጠባቂ (DOCS-1b) ይፈልጋል `DOCS_SECURITY_ALLOW_INSECURE=1` በአገር ውስጥ ካላለፉ። |
| `DOCS_OAUTH_*` | ገምጋሚዎች ፖርታሉን ሳይለቁ ለአጭር ጊዜ የሚቆዩ ቶከኖችን ማመንጨት እንዲችሉ የOAuth መሳሪያ ፍሰትን (`OAuthDeviceLogin` አካል) ያንቁ። |

የOAuth ተለዋዋጮች ባሉበት ጊዜ ማጠሪያው በተቀናበረው Auth አገልጋይ በኩል የሚሄድ **በመሳሪያ ኮድ ይግቡ** ቁልፍ ይሰጣል (ለትክክለኛው ቅርፅ `config/security-helpers.js` ይመልከቱ)። በመሳሪያው ፍሰት በኩል የተሰጡ ቶከኖች በአሳሽ ክፍለ ጊዜ ውስጥ ብቻ ተደብቀዋል።

## Norito-RPC ጭነት በመላክ ላይ

1. በCLI ወይም በ[Norito quickstart](./quickstart.md) የተገለጹትን የI18NI0000051X ክፍያ ጭነት ይገንቡ። ፕሮክሲው I18NI0000052X አካላትን ሳይለወጥ ያስተላልፋል፣ ስለዚህ እርስዎ በ`curl` የሚለጥፉትን ተመሳሳይ ቅርስ እንደገና መጠቀም ይችላሉ።
2. `/reference/torii-rapidoc` ክፈት (ለሁለትዮሽ ጭነት ይመረጣል) ወይም `/reference/torii-swagger`።
3. ከተቆልቋዩ ውስጥ የሚፈልጉትን I18NT0000013X ቅጽበታዊ ፎቶ ይምረጡ። ቅጽበተ-ፎቶዎች ተፈርመዋል; ፓኔሉ በ `static/openapi/manifest.json` ውስጥ የተቀዳውን አንጸባራቂ ማሟያ ያሳያል።
4. በ"ሙከራው" መሳቢያ ውስጥ የI18NI0000057X የይዘት አይነትን ምረጥ፣*ፋይልን ምረጥ** የሚለውን ተጫን እና ክፍያህን ምረጥ። ተኪው ጥያቄውን ለ`/proxy/v2/pipeline/submit` በድጋሚ ይጽፋል እና በ `X-TryIt-Client=docs-portal-rapidoc` መለያ ይሰጠዋል ።
5. የI18NT0000007X ምላሾችን ለማውረድ `Accept: application/x-norito` ያዘጋጁ። Swagger/RapiDoc የራስጌ መምረጡን በተመሳሳዩ መሳቢያ ውስጥ ያጋልጡ እና ሁለትዮሽውን በፕሮክሲው በኩል ይልቀቁ።

ለJSON-ብቻ መንገዶች የተከተተው ማጠሪያ ሞክር ብዙ ጊዜ ፈጣን ነው፡ መንገዱን አስገባ (ለምሳሌ፡ `/v2/accounts/i105.../assets`)፣ የኤችቲቲፒ ዘዴን ምረጥ፣ ሲያስፈልግ የJSON አካል ለጥፍ እና ራስጌን፣ የቆይታ ጊዜን እና የሚጫኑ ጭነቶችን በውስጥ መስመር ለመፈተሽ **ጥያቄ ላክ** የሚለውን ተጫን።

## መላ መፈለግ

| ምልክት | ምክንያት | ማሻሻያ |
| --- | --- | --- |
| የአሳሽ ኮንሶል የCORS ስህተቶችን ያሳያል ወይም ማጠሪያው ተኪ ዩአርኤል እንደጎደለ ያስጠነቅቃል። | ተኪ እየሰራ አይደለም ወይም መነሻው በተፈቀደላቸው ዝርዝር ውስጥ አልተመዘገበም። | ተኪውን ይጀምሩ፣ `TRYIT_PROXY_ALLOWED_ORIGINS` የእርስዎን ፖርታል አስተናጋጅ እንደሚሸፍን ያረጋግጡ እና I18NI0000063Xን እንደገና ያስጀምሩ። |
| `npm run tryit-proxy` በ"መፍጨት አለመመጣጠን" ይወጣል። | የTorii I18NT0000002X ቅርቅብ ወደ ላይ ተቀይሯል። | `npm run sync-openapi -- --latest` (ወይም I18NI0000066X) ያሂዱ እና እንደገና ይሞክሩ። |
| መግብሮች `401` ወይም I18NI0000068X ይመለሳሉ። | ማስመሰያ ይጎድላል፣ ጊዜው ያለፈበት ወይም በቂ ያልሆነ ወሰን። | የOAuth መሳሪያ ፍሰትን ተጠቀም ወይም የሚሰራ ተሸካሚ ማስመሰያ ወደ ማጠሪያ ሳጥን ውስጥ ለጥፍ። ለስታቲክ ቶከኖች `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ወደ ውጭ መላክ አለብህ። |
| `429 Too Many Requests` ከተኪ። | የአይፒ ተመን ገደብ ታልፏል። | `TRYIT_PROXY_RATE_LIMIT`/I18NI0000072X ለታመኑ አካባቢዎች ወይም ስሮትል የሙከራ ስክሪፕቶች ያሳድጉ። ሁሉም ተመኖች-ገደብ ውድቅ ጭማሪ `tryit_proxy_rate_limited_total`. |

# ታዛቢነት

- `npm run probe:tryit-proxy` (በ`scripts/tryit-proxy-probe.mjs` አካባቢ መጠቅለያ) ወደ `/healthz` ይደውላል ፣ እንደ አማራጭ የናሙና መንገድ ይሠራል እና Prometheus የጽሑፍ ፋይሎችን ለ`probe_success` / I18NI7000 ያወጣል። ከ node_exporter ጋር ለመዋሃድ I18NI0000079X አዋቅር።
- ቆጣሪዎችን (`tryit_proxy_requests_total`፣ `tryit_proxy_rate_limited_total`፣ `tryit_proxy_upstream_failures_total`) እና የቆይታ ሂስቶግራምን ለማጋለጥ `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` አዘጋጅ። የ`dashboards/grafana/docs_portal.json` ሰሌዳ DOCS-SORA SLOsን ለማስፈጸም እነዚህን መለኪያዎች ያነባል።
- የሩጫ ጊዜ ምዝግብ ማስታወሻዎች በ stdout ላይ ይኖራሉ። እያንዳንዱ ግቤት የጥያቄውን መታወቂያ፣ የላይ ዥረት ሁኔታ፣ የማረጋገጫ ምንጭ (`default`፣ `override`፣ ወይም I18NI0000087X) እና የቆይታ ጊዜን ያካትታል። ሚስጥሮች ከመልቀቃቸው በፊት ተስተካክለዋል.

የ`application/x-norito` ክፍያ Torii ሳይለወጥ መድረሱን ማረጋገጥ ካስፈለገዎት Jest suite (`npm test -- tryit-proxy`) ያሂዱ ወይም በ `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` ስር ያሉትን እቃዎች ይፈትሹ። የድጋሚ ፈተናዎቹ የተጨመቁ Norito ሁለትዮሾችን፣ የተፈረሙ OpenAPI መግለጫዎችን እና የNRPC ልቀቶች ቋሚ የማስረጃ ዱካዎችን ይሸፍናሉ።