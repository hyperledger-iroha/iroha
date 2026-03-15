---
lang: am
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b920e21b96436755f7d37f7b5577465cb3e30016d36340c50f7c6f3a9a46919
source_last_modified: "2025-12-29T18:16:35.116499+00:00"
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
- የአውታረ መረብ መዳረሻ ወደ I18NT0000012X ዝግጅት አካባቢ
- የአካል ብቃት እንቅስቃሴ ለማድረግ ያቀዱትን Torii መስመሮችን መደወል የሚችል ተሸካሚ ቶከን

ሁሉም የተኪ ውቅር የሚከናወነው በአካባቢ ተለዋዋጮች ነው። ከታች ያለው ሰንጠረዥ
በጣም አስፈላጊ የሆኑትን እንክብሎች ይዘረዝራል-

| ተለዋዋጭ | ዓላማ | ነባሪ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | ተኪው የሚያስተላልፈው Base Torii URL ወደ | ** ያስፈልጋል *** |
| `TRYIT_PROXY_LISTEN` | ለአካባቢ ልማት አድራሻ ያዳምጡ (ቅርጸት I18NI0000042X ወይም `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | ተኪ ሊጠሩ የሚችሉ በነጠላ ሰረዝ የተለዩ የመነሻ ዝርዝር | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | ለእያንዳንዱ የወራጅ ጥያቄ በI18NI0000048X የተቀመጠው መለያ | `docs-portal` |
| `TRYIT_PROXY_BEARER` | ነባሪ ተሸካሚ ማስመሰያ ወደ Torii ተላልፏል ባዶ_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | የመጨረሻ ተጠቃሚዎች የራሳቸውን ማስመሰያ በ`X-TryIt-Auth` በኩል እንዲያቀርቡ ይፍቀዱላቸው | `0` |
| `TRYIT_PROXY_MAX_BODY` | ከፍተኛው የጥያቄ የሰውነት መጠን (ባይት) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ወደላይ የሚያልፍበት ጊዜ በሚሊሰከንዶች | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | የተፈቀደላቸው ጥያቄዎች በእያንዳንዱ የዋጋ መስኮት በደንበኛ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | ለተመን ገደብ (ሚሴ) ተንሸራታች መስኮት | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | ለPrometheus አይነት ሜትሪክስ የመጨረሻ ነጥብ (`host:port` ወይም `[ipv6]:port`) አማራጭ የማዳመጥ አድራሻ | ባዶ (የተሰናከለ) _ |
| `TRYIT_PROXY_METRICS_PATH` | የኤችቲቲፒ ዱካ በሜትሪክስ መጨረሻ ነጥብ | `/metrics` |

ፕሮክሲው I18NI0000067Xን ያጋልጣል፣ የተዋቀሩ የJSON ስህተቶችን ይመልሳል እና
ተሸካሚ ምልክቶችን ከሎግ ውፅዓት ያድሳል።

ተኪውን ለሰነዶች ተጠቃሚዎች ሲያጋልጥ `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`ን ያንቁ Swagger እና
RapiDoc ፓነሎች በተጠቃሚ የሚቀርቡ ተሸካሚ ቶከኖችን ማስተላለፍ ይችላሉ። ተኪው አሁንም የዋጋ ገደቦችን ያስፈጽማል፣
ምስክርነቶችን ያድሳል፣ እና ጥያቄው ነባሪውን ማስመሰያ ወይም የጥያቄ መሻር መጠቀሙን ይመዘግባል።
`TRYIT_PROXY_CLIENT_ID` እንደ `X-TryIt-Client` ለመላክ ወደሚፈልጉት መለያ ያቀናብሩ
(የI18NI0000071X ነባሪዎች)። ተኪው በጠዋይ የቀረበን ያስተካክላል እና ያረጋግጣል
የ`X-TryIt-Client` እሴቶች፣ ወደዚህ ነባሪ በመመለስ መግቢያ መንገዶችን ማዘጋጀት ይችላሉ።
የአሳሽ ዲበ ውሂብን ሳያካትት የኦዲት ማረጋገጫ።

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
የተዋቀረው I18NT0000017X መነሻ።

ሶኬቱን ከማሰርዎ በፊት ስክሪፕቱ ያንን ያረጋግጣል
`static/openapi/torii.json` ከተመዘገበው የምግብ መፈጨት ጋር ይዛመዳል
`static/openapi/manifest.json`. ፋይሎቹ ከተንሸራተቱ ትዕዛዙ በኤ
ስህተት እና `npm run sync-openapi -- --latest` እንዲያሄዱ ያዝዝዎታል። ወደ ውጪ ላክ
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` ለአደጋ ጊዜ መሻር ብቻ; ተኪው ያደርጋል
በጥገና መስኮቶች ወቅት ማገገም እንዲችሉ ማስጠንቀቂያ ይመዝገቡ እና ይቀጥሉ።

## የፖርታል መግብሮችን ሽቦ ያድርጉ

የገንቢ ፖርታልን ሲገነቡ ወይም ሲያገለግሉ መግብሮቹ የሚያዘጋጁትን ዩአርኤል ያዘጋጁ
ለፕሮክሲው መጠቀም ያለበት፡-

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

የሚከተሉት ክፍሎች እነዚህን እሴቶች ከ`docusaurus.config.js` ያንብቡ፡

- ** Swagger UI *** - በ `/reference/torii-swagger` ላይ የቀረበ; ቅድመ-ይፈቅዳል
- **MCP reference** - `/reference/torii-mcp`; use this for JSON-RPC `/v2/mcp` agent workflows.
  ተሸካሚ እቅድ ማስመሰያ ሲኖር፣ ጥያቄዎችን በ`X-TryIt-Client` መለያ ይሰጣል፣
  `X-TryIt-Auth` ያስገባል እና ጥሪዎችን በፕሮክሲው በኩል በድጋሚ ይጽፋል
  `TRYIT_PROXY_PUBLIC_URL` ተቀናብሯል።
- ** RapiDoc *** - በ `/reference/torii-rapidoc` ላይ የቀረበ; የማስመሰያ መስክን ያንፀባርቃል ፣
  ከስዋገር ፓኔል ጋር ተመሳሳይ ራስጌዎችን በድጋሚ ይጠቀማል፣ እና ተኪውን ያነጣጥራል።
  ዩአርኤል ሲዋቀር በራስ ሰር።
- ** ኮንሶል ይሞክሩት *** - በኤፒአይ አጠቃላይ እይታ ገጽ ላይ የተካተተ; ብጁ እንድትልክ ያስችልሃል
  ጥያቄዎችን፣ ራስጌዎችን ይመልከቱ፣ እና የምላሽ አካላትን ይፈትሹ።

ሁለቱም ፓነሎች የሚያነበው ** ቅጽበተ-ፎቶ መራጭ** ላይ
`docs/portal/static/openapi/versions.json`. ያንን መረጃ ጠቋሚ በሕዝብ ይሙሉት።
`npm run sync-openapi -- --version=<label> --mirror=current --latest` እንዲሁ
ገምጋሚዎች በታሪካዊ ዝርዝሮች መካከል መዝለል ይችላሉ ፣ የተቀዳውን SHA-256 ይመልከቱ ፣
እና የመልቀቂያ ቅጽበታዊ ገጽ እይታ ከመጠቀምዎ በፊት የተፈረመ ሰነድ መያዙን ያረጋግጡ
በይነተገናኝ መግብሮች.

በማንኛውም መግብር ውስጥ ማስመሰያ መቀየር የአሁኑን የአሳሽ ክፍለ ጊዜ ብቻ ነው የሚነካው; የ
ፕሮክሲ በፍፁም አይቆይም ወይም የቀረበውን ማስመሰያ አይመዘግብም።

## የአጭር ጊዜ የ OAuth ቶከኖች

ለረጅም ጊዜ የቆዩ የTorii ቶከኖችን ለገምጋሚዎች ከማሰራጨት ለመዳን ይሞክሩት።
ኮንሶል ወደ የእርስዎ OAuth አገልጋይ። ከታች ያሉት የአካባቢ ተለዋዋጮች ሲገኙ
ፖርታሉ የመሣሪያ-ኮድ መግቢያ ምግብርን፣ አጭር ጊዜ የሚቆይ ተሸካሚ ማስመሰያዎችን ያቀርባል፣
እና በራስ-ሰር ወደ ኮንሶል ቅፅ ውስጥ ያስገባቸዋል.

| ተለዋዋጭ | ዓላማ | ነባሪ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | የOAuth መሣሪያ ፈቃድ የመጨረሻ ነጥብ (`/oauth/device/code`) | ባዶ (የተሰናከለ) _ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` የሚቀበል ማስመሰያ የመጨረሻ ነጥብ | ባዶ_ |
| `DOCS_OAUTH_CLIENT_ID` | ለሰነዶች ቅድመ እይታ የተመዘገበ የOAuth ደንበኛ ለዪ | ባዶ_ |
| `DOCS_OAUTH_SCOPE` | በመግቢያ ጊዜ የተጠየቁ በክፍተት የተገደቡ ወሰኖች | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | ማስመሰያውን ከ | ጋር ለማያያዝ አማራጭ የኤፒአይ ታዳሚዎች ባዶ_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | መጽደቅን ሲጠብቅ ዝቅተኛው የድምጽ ክፍተት | `5000` (እሴቶች <5000ms ውድቅ ናቸው) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | የመመለሻ መሳሪያ-የኮድ ማብቂያ መስኮት (ሰከንዶች) | `600` (በ 300 ዎቹ እና 900 ዎቹ መካከል መቆየት አለበት) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | የመውደቅ መዳረሻ-ቶከን የህይወት ዘመን (ሰከንዶች) | `900` (በ 300 ዎቹ እና 900 ዎቹ መካከል መቆየት አለበት) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth ማስፈጸሚያን ሆን ብለው ለዘለሉ የአገር ውስጥ ቅድመ ዕይታዎች ወደ `1` ያቀናብሩ | _ያልተዘጋጀ_ |

የምሳሌ ውቅር፡

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` ወይም `npm run build` ን ሲያሄዱ ፖርታሉ እነዚህን እሴቶች ያካትታል
በ `docusaurus.config.js`. በአካባቢያዊ ቅድመ እይታ ጊዜ ይሞክሩት ካርዱ ሀ
"በመሳሪያ ኮድ ይግቡ" ቁልፍ። ተጠቃሚዎች የሚታየውን ኮድ በእርስዎ OAuth ላይ ያስገባሉ።
የማረጋገጫ ገጽ; አንዴ የመሳሪያው ፍሰት መግብርን ከተሳካ በኋላ፡-

- የተሰጠውን ተሸካሚ ማስመሰያ ወደ ሞክሩት ኮንሶል መስክ ውስጥ ያስገባል ፣
- ጥያቄዎችን አሁን ባሉት `X-TryIt-Client` እና `X-TryIt-Auth` ራስጌዎች መለያ ይሰጣል፣
- የቀረውን የህይወት ዘመን ያሳያል, እና
- ጊዜው ሲያልቅ ማስመሰያውን በራስ-ሰር ያጸዳል።

የእጅ ተሸካሚው ግቤት እንዳለ ይቆያል—በማንኛውም ጊዜ የOAuth ተለዋዋጮችን ያስወግዱ
ገምጋሚዎች ጊዜያዊ ማስመሰያ ራሳቸው እንዲለጥፉ ወይም ወደ ውጭ እንዲልኩ ማስገደድ ይፈልጋሉ
`DOCS_OAUTH_ALLOW_INSECURE=1` ለገለልተኛ የአካባቢ ቅድመ እይታዎች የማይታወቅ መዳረሻ
ተቀባይነት አለው። ግንባታዎች ያለ OAuth መዋቀር አሁን በፍጥነት ወድቀዋል
DOCS-1b የመንገድ ካርታ በር።

📌 [የደህንነት ማጠንከሪያ እና የብዕር-ሙከራ ዝርዝር](./security-hardening.md) ይገምግሙ
ከላቦራቶሪ ውጭ ያለውን ፖርታል ከማጋለጥዎ በፊት; የአደጋውን ሞዴል ያዘጋጃል ፣
የCSP/የታመኑ አይነቶች መገለጫ፣ እና አሁን DOCS-1bን የሚያስገቡት የፔኔትሽን-ሙከራ ደረጃዎች።

## Norito-RPC ናሙናዎች

የNorito-RPC ጥያቄዎች ከJSON መንገዶች ጋር አንድ አይነት ፕሮክሲ እና የOAuth የቧንቧ መስመር ይጋራሉ።
በቀላሉ `Content-Type: application/x-norito` አዘጋጅተው ይልካሉ
በNRPC ዝርዝር ውስጥ የተገለፀው ቀድሞ የተቀመጠ I18NT0000007X ክፍያ
(`docs/source/torii/nrpc_spec.md`)።
ማከማቻው በ `fixtures/norito_rpc/` so portal ስር ቀኖናዊ ጭነትን ይልካል።
ደራሲዎች፣ የኤስዲኬ ባለቤቶች እና ገምጋሚዎች CI የሚጠቀመውን ትክክለኛ ባይት እንደገና ማጫወት ይችላሉ።

### የNorito ክፍያ ጭነት ከ Try It console ላክ

1. እንደ `fixtures/norito_rpc/transfer_asset.norito` አይነት መሳሪያ ይምረጡ። እነዚህ
   ፋይሎች ጥሬ Norito ፖስታዎች ናቸው; አታድርግ ** base64-encode እነሱን.
2. በSwagger ወይም RapiDoc ውስጥ የNRPC የመጨረሻ ነጥብ ያግኙ (ለምሳሌ፡
   `POST /v2/pipeline/submit`) እና **የይዘት አይነት** መራጩን ወደ
   `application/x-norito`.
3. የጥያቄ አካል አርታዒውን ወደ ** ሁለትዮሽ ** (የSwagger "ፋይል" ሁነታ ወይም) ቀይር
   የRapiDoc "ሁለትዮሽ/ፋይል" መራጭ) እና የ`.norito` ፋይልን ይስቀሉ። መግብር
   ባይት ያለ ለውጥ በፕሮክሲው በኩል ያሰራጫል።
4. ጥያቄውን ያቅርቡ. Torii `X-Iroha-Error-Code: schema_mismatch` ከመለሰ፣
   ሁለትዮሽ ጭነቶችን የሚቀበል የመጨረሻ ነጥብ እየጠሩ መሆንዎን ያረጋግጡ እና
   የሼማ ሃሽ በ`fixtures/norito_rpc/schema_hashes.json` ውስጥ መመዝገቡን ያረጋግጡ
   እየመታህ ካለው Torii ግንባታ ጋር ይዛመዳል።

መሥሪያው በጣም የቅርብ ጊዜውን ፋይል በማህደረ ትውስታ ውስጥ ያቆያል ስለዚህ ተመሳሳዩን እንደገና ማስገባት ይችላሉ።
የተለያዩ የፍቃድ ቶከኖች ወይም Torii አስተናጋጆችን በሚጠቀሙበት ወቅት የሚጫኑት። በማከል ላይ
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ወደ የእርስዎ የስራ ፍሰት ምርቶች
በNRPC-4 የጉዲፈቻ ዕቅድ (ሎግ + JSON ማጠቃለያ) ውስጥ የተጠቀሰው የማስረጃ ጥቅል
በግምገማዎች ጊዜ ምላሹን ይሞክሩት ከቅጽበታዊ ገጽ እይታ ጋር በጥሩ ሁኔታ የተጣመረ።

### CLI ምሳሌ (ከርል)

ተመሳሳይ መጫዎቻዎች ከፖርታሉ ውጭ በ `curl` በኩል ሊጫወቱ ይችላሉ ፣ ይህ ጠቃሚ ነው
ተኪውን ሲያረጋግጡ ወይም የጌትዌይ ምላሾችን ሲያርሙ፡-

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v2/pipeline/submit"
```

በ`transaction_fixtures.manifest.json` ውስጥ ለተዘረዘረው ማንኛውም ግቤት እቃውን ይቀይሩት።
ወይም የእራስዎን የክፍያ ጭነት በ `cargo xtask norito-rpc-fixtures`. መቼ Torii
በካናሪ ሁነታ ላይ ነው `curl` በሙከራ ፕሮክሲው ላይ ማመልከት ይችላሉ
(`https://docs.sora.example/proxy/v2/pipeline/submit`) ተመሳሳይ ልምምድ ለማድረግ
የፖርታል መግብሮች የሚጠቀሙባቸው መሠረተ ልማት.

## ታዛቢነት እና ተግባራትእያንዳንዱ ጥያቄ አንድ ጊዜ በዘዴ፣ በዱካ፣ በመነሻ፣ በከፍታ ሁኔታ እና በ
የማረጋገጫ ምንጭ (`override`፣ `default`፣ ወይም `client`)። ማስመሰያዎች በጭራሽ አይደሉም
ተከማችተዋል - ሁለቱም የተሸካሚ ራስጌዎች እና `X-TryIt-Auth` እሴቶች ከዚህ በፊት ተስተካክለዋል
ሎግ (ሎግ)—ስለዚህ ሳይጨነቁ stdoutን ወደ ማዕከላዊ ሰብሳቢ ማስተላለፍ ይችላሉ።
ሚስጥሮች የሚያፈሱ.

### የጤና ምርመራ እና ማስጠንቀቂያ

በማሰማራት ጊዜ ወይም በጊዜ መርሐግብር የታሸገውን ምርመራ ያሂዱ፡-

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
- `TRYIT_PROXY_PROBE_METRICS_FILE` — አማራጭ Prometheus የጽሑፍ ፋይል መድረሻ ለI18NI0000135X/`probe_duration_seconds`።
- `TRYIT_PROXY_PROBE_LABELS` — በነባሪ ሰረዝ የተለዩ `key=value` ጥንዶች በመለኪያዎቹ ላይ ተያይዘዋል (የ`job=tryit-proxy` እና `instance=<proxy URL>` ነባሪዎች)።
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` ሲነቃ በተሳካ ሁኔታ ምላሽ መስጠት ያለበት የአማራጭ መለኪያዎች የመጨረሻ ነጥብ ዩአርኤል (ለምሳሌ `http://localhost:9798/metrics`)።

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

`TRYIT_PROXY_METRICS_LISTEN` ሲዋቀር, አዘጋጅ
`TRYIT_PROXY_PROBE_METRICS_URL` ወደ ሜትሪክስ መጨረሻ ነጥብ ስለዚህ መፈተሻው በፍጥነት አይሳካም።
የተቧጨረው ገጽ ከጠፋ (ለምሳሌ፣ የተሳሳተ ውቅር መግባት ወይም ጠፍቷል
የፋየርዎል ደንቦች). የተለመደው የምርት ቅንብር ነው
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

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

### መለኪያዎች የመጨረሻ ነጥብ እና ዳሽቦርዶች

ከዚህ በፊት `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ወይም ማንኛውንም አስተናጋጅ/ወደብ ጥንድ) አዘጋጅ
የPrometheus የተቀረፀውን የሜትሪክስ የመጨረሻ ነጥብ ለማጋለጥ ፕሮክሲውን በመጀመር። መንገዱ
የ `/metrics` ነባሪዎች ግን በ በኩል ሊሻሩ ይችላሉ።
`TRYIT_PROXY_METRICS_PATH=/custom`. እያንዳንዱ መቧጨር ለእያንዳንዱ ዘዴ ቆጣሪዎችን ይመልሳል
የጥያቄ ድምር፣ የዋጋ-ገደብ አለመቀበል፣ የተፋሰሱ ስህተቶች/ጊዜ ማብቂያዎች፣ የተኪ ውጤቶች፣
እና የዘገየ ማጠቃለያዎች፡-

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

የእርስዎን Prometheus/OTLP ሰብሳቢዎች በሜትሪክስ መጨረሻ ነጥብ ላይ ያመልክቱ እና እንደገና ይጠቀሙ
SRE ጅራትን ማየት እንዲችል ነባር `dashboards/grafana/docs_portal.json` ፓነሎች
የምዝግብ ማስታወሻዎችን ሳይተነተን መዘግየት እና ውድቅ ማድረጉ። ፕሮክሲው በራስ-ሰር
ኦፕሬተሮች ዳግም መጀመሩን እንዲያውቁ ለማገዝ `tryit_proxy_start_timestamp_ms` ያትማል።

### የጥቅልል አውቶማቲክ

ዒላማውን Torii URL ለማዘመን ወይም ወደነበረበት ለመመለስ የአስተዳደር አጋዥን ይጠቀሙ። ስክሪፕቱ
የቀደመውን ውቅር በI18NI0000153X ያከማቻል ስለዚህ መልሶ መመለሻዎች ሀ
ነጠላ ትዕዛዝ.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

ከተሰማሩ የኢንቪ ፋይል ዱካውን በ`--env` ወይም `TRYIT_PROXY_ENV` ይሽሩት
ውቅረትን በሌላ ቦታ ያከማቻል.
