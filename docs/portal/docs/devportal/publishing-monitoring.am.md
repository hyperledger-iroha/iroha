---
lang: am
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f68e8cc639bd6a780c33fd14ab4e25df1c6e9381595c7a4c44ff577fea02400d
source_last_modified: "2026-01-22T16:26:46.494851+00:00"
translation_last_reviewed: 2026-02-07
id: publishing-monitoring
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
---

የመንገድ ካርታ ንጥል ** DOCS-3c** ከማሸጊያ ዝርዝር በላይ ይፈልጋል፡ ከእያንዳንዱ በኋላ
SoraFS ማተም የገንቢውን ፖርታል፣ ይሞክሩት የሚለውን በተከታታይ ማረጋገጥ አለብን
ተኪ፣ እና የጌትዌይ ማሰሪያዎች ጤናማ ሆነው ይቆያሉ። ይህ ገጽ ክትትልን ይመዘግባል
ከ[የማሰማሪያ መመሪያ](./deploy-guide.md) እና CI እና በርቷል ጋር ያለው ወለል
የጥሪ መሐንዲሶች Ops SLO ን ለማስፈጸም የሚጠቀምባቸውን ተመሳሳይ ፍተሻዎች መጠቀም ይችላሉ።

## የቧንቧ መስመር ዳግመኛ

1. ** ይገንቡ እና ይፈርሙ *** - ለማሄድ [የማሰማሪያ መመሪያ](./deploy-guide.md) ይከተሉ።
   `npm run build`፣ `scripts/preview_wave_preflight.sh`፣ እና Sigstore +
   ግልጽ የማስረከቢያ ደረጃዎች. የቅድመ በረራ ስክሪፕት `preflight-summary.json` ያወጣል።
   ስለዚህ እያንዳንዱ ቅድመ እይታ የግንባታ/አገናኝ/መመርመሪያ ሜታዳታን ይይዛል።
2. ** ይሰኩት እና ያረጋግጡ *** - `sorafs_cli manifest submit`፣ `cargo xtask soradns-verify-binding`፣
   እና የዲ ኤን ኤስ የመቁረጥ እቅድ ለአስተዳደር ቆራጥ የሆኑ ቅርሶችን ያቀርባል።
3. **የማህደር ማስረጃ** - የCAR ማጠቃለያውን ያከማቹ፣ Sigstore ጥቅል፣ ተለዋጭ ማስረጃ፣
   የመመርመሪያ ውፅዓት፣ እና I18NI0000021X ዳሽቦርድ ቅጽበተ-ፎቶዎች ስር
   `artifacts/sorafs/<tag>/`.

## የመከታተያ ቻናሎች

### 1. የህትመት ማሳያዎች (`scripts/monitor-publishing.mjs`)

አዲሱ የ`npm run monitor:publishing` ትዕዛዝ የፖርታል መፈተሻውን ይጠቀልላል፣ ይሞክሩት።
proxy probe፣ እና አስገዳጅ አረጋጋጭ ወደ ነጠላ CI-ተስማሚ ቼክ። አቅርቡ ሀ
JSON config (ወደ CI ሚስጥሮች ወይም `configs/docs_monitor.json` ምልክት የተደረገባቸው) እና ያሂዱ፡

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` ያክሉ (እና እንደ አማራጭ
`--prom-job docs-preview`) I18NT0000000X የጽሑፍ-ቅርጸት መለኪያዎችን ለመልቀቅ ተስማሚ
የPushgateway ሰቀላዎች ወይም ቀጥታ I18NT0000001X ጥራዞች በማዘጋጀት/በምርት ላይ። የ
የ SLO ዳሽቦርዶች እና የማንቂያ ደንቦች መከታተል እንዲችሉ ሜትሪክስ የJSON ማጠቃለያን ያንፀባርቃል
ፖርታል፣ ይሞክሩት፣ ማሰሪያ እና የዲኤንኤስ ጤና የማስረጃ ጥቅሉን ሳይተነተኑ።

የምሳሌ ማዋቀር ከሚያስፈልጉት ቁልፎች እና በርካታ ማሰሪያዎች ጋር፡-

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

ተቆጣጣሪው የJSON ማጠቃለያ (S3/SoraFS ወዳጃዊ) ይጽፋል እና ዜሮ ያልሆነው ሲወጣ ይወጣል
ማንኛውም መጠይቅ አልተሳካም፣ ለክሮን ስራዎች፣ Buildkite ደረጃዎች፣ ወይም ተስማሚ ያደርገዋል
የማንቂያ አስተዳዳሪ የድር መንጠቆዎች። `--evidence-dir` ማለፍ ይቀጥላል I18NI0000029X፣
`portal.json`፣ `tryit.json`፣ እና `binding.json` ከ `checksums.sha256` ጋር
የአስተዳደር ገምጋሚዎች የመቆጣጠሪያውን ውጤት ሳያስፈልጋቸው ሊለያዩ እንደሚችሉ ማሳየት
መመርመሪያዎችን እንደገና ያሂዱ.

> ** TLS የጥበቃ ሀዲድ፡** `monitorPortal` ካላቀናበሩ በቀር `http://` ቤዝ ዩአርኤሎችን ውድቅ ያደርጋል።
> `allowInsecureHttp: true` በማዋቀር። የማምረቻ/የማዘጋጀት መመርመሪያዎችን እንደበራ ያቆዩት።
> HTTPS; መርጦ መግባቱ ለአካባቢያዊ ቅድመ-እይታዎች ብቻ አለ።

እያንዳንዱ አስገዳጅ ግቤት I18NI0000037X ከተያዘው ጋር ይሰራል
`portal.gateway.binding.json` ጥቅል (እና አማራጭ I18NI0000039X) ስለዚህ ተለዋጭ ስም፣
የማረጋገጫ ሁኔታ፣ እና የይዘት CID ከታተሙ ማስረጃዎች ጋር እንደተጣመረ ይቆያሉ። የ
አማራጭ `hostname` ጠባቂ ተለዋጭ ስም የተገኘ ቀኖናዊ አስተናጋጅ ከ
ለማስተዋወቅ ያሰቡትን ጌትዌይ አስተናጋጅ፣ከዚህ የሚንሳፈፉ የዲ ኤን ኤስ መቆራረጦችን ይከላከላል
የተመዘገበ ማሰሪያ.

የአማራጭ `dns` የማገጃ ሽቦዎች DOCS-7's SoraDNS ልቀት ወደ ተመሳሳይ ማሳያ።
እያንዳንዱ ግቤት የአስተናጋጅ ስም/የመዝገብ አይነት ጥንድን ይፈታል (ለምሳሌ የ
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) እና
መልሶቹን I18NI0000044X ወይም I18NI0000045X መመሳሰልን ያረጋግጣል። ሁለተኛው
ከሃርድ-ኮዶች በላይ ባለው ቅንጣቢ ውስጥ የገባው ቀኖናዊው ሃሽድ አስተናጋጅ ስም በ
`cargo xtask soradns-hosts --name docs-preview.sora.link`; ማሳያው አሁን ያረጋግጣል
ሁለቱም ለሰው ተስማሚ ተለዋጭ ስም እና ቀኖናዊው ሃሽ (`igjssx53…gw.sora.id`)
ለተሰካው ቆንጆ አስተናጋጅ መፍታት። ይህ የዲ ኤን ኤስ ማስተዋወቂያ ማስረጃን በራስ ሰር ያደርገዋል፡-
የኤችቲቲፒ ማሰሪያው አሁንም ቢሆን ተቆጣጣሪው አንዱም አስተናጋጅ ቢንሸራተት አይሳካም።
ዋና ዋና አንጸባራቂ.

### 2. I18NT0000004X ስሪት አንጸባራቂ ጠባቂ

የDOCS-2b "የተፈረመ OpenAPI አንጸባራቂ" መስፈርት አሁን አውቶማቲክ ጠባቂ ይልካል።
`ci/check_openapi_spec.sh` ጥሪዎች `npm run check:openapi-versions` ጥሪዎች
`scripts/verify-openapi-versions.mjs` ለመሻገር
`docs/portal/static/openapi/versions.json` ከትክክለኛው Torii ዝርዝሮች እና
ይገለጣል። ጠባቂው ይህንን ያረጋግጣል፡-

- በI18NI0000052X ውስጥ የተዘረዘረው እያንዳንዱ እትም ከዚህ በታች ተዛማጅ ማውጫ አለው።
  `static/openapi/versions/`.
- የእያንዳንዱ ግቤት I18NI0000054X እና I18NI0000055X መስኮች በዲስክ ላይ ካለው ዝርዝር ፋይል ጋር ይዛመዳሉ።
- የ`latest` ተለዋጭ ስም የ`current` ግቤትን ያንፀባርቃል (መፍጨት/መጠን/ፊርማ ዲበ ዳታ)
  ስለዚህ ነባሪው ማውረድ ሊንሸራተት አይችልም።
- የተፈረሙ ግቤቶች `artifact.path` ወደ እሱ የሚያመለክት አንጸባራቂን ይጠቅሳሉ
  ተመሳሳይ ዝርዝር እና የማን ፊርማ/የህዝብ ቁልፍ የአስራስድስትዮሽ እሴቶች ከማንፀባረቂያው ጋር ይዛመዳሉ።

አዲስ ዝርዝር በሚያንጸባርቁበት ጊዜ ጠባቂውን በአካባቢው ያሂዱ፡-

```bash
cd docs/portal
npm run check:openapi-versions
```

ያልተሳካላቸው መልእክቶች የቆየ ፋይል ፍንጭ (`npm run sync-openapi -- --latest`) ያካትታሉ።
ስለዚህ የፖርታል አስተዋጽዖ አበርካቾች ቅጽበተ-ፎቶዎችን እንዴት ማደስ እንደሚችሉ ያውቃሉ። ጠባቂውን ወደ ውስጥ ማቆየት።
CI የተፈረመበት አንጸባራቂ እና የታተመ መፈጨትን በሚመለከት ፖርታል መልቀቅን ይከለክላል
ከመመሳሰል ውጣ።

### 2. ዳሽቦርዶች እና ማንቂያዎች

- ** `dashboards/grafana/docs_portal.json` *** - ለ DOCS-3c ዋና ሰሌዳ። ፓነሎች
  ትራክ `torii_sorafs_gateway_refusals_total`, ማባዛት SLA ናፈቀ, ይሞክሩት
  የተኪ ስህተቶች፣ እና የመመርመሪያ መዘግየት (`docs.preview.integrity` ተደራቢ)። ወደ ውጭ ላክ
  ከእያንዳንዱ ከተለቀቀ በኋላ ይሳፈሩ እና ከኦፕሬሽኖች ቲኬት ጋር ያያይዙት።
- ** ተኪ ማንቂያዎችን ይሞክሩት *** - የማስጠንቀቂያ አስተዳዳሪ ደንብ `TryItProxyErrors` ይቃጠላል
  ቀጣይነት ያለው `probe_success{job="tryit-proxy"}` ጠብታዎች ወይም
  `tryit_proxy_requests_total{status="error"}` ስፒሎች።
- ** ጌትዌይ SLO *** - `DocsPortal/GatewayRefusals` ተለዋጭ ስም ማሰር እንደሚቀጥል ያረጋግጣል
  የተሰካውን አንጸባራቂ መፍጨት ለማስተዋወቅ; escalations አገናኝ ወደ
  `cargo xtask soradns-verify-binding` CLI ግልባጭ በህትመት ጊዜ ተይዟል።

### 3. የማስረጃ ዱካ

እያንዳንዱ የክትትል ሂደት መያያዝ አለበት፡-

- `monitor-publishing` የማስረጃ ጥቅል (I18NI0000069X፣ በክፍል ፋይሎች፣ እና
  `checksums.sha256`)።
- Grafana ቅጽበታዊ ገጽ እይታዎች ለ I18NI0000071X ሰሌዳ በሚለቀቀው መስኮት ላይ።
- የተኪ ለውጥ/የመመለሻ ግልባጭ (`npm run manage:tryit-proxy` ምዝግብ ማስታወሻዎች) ይሞክሩት።
- ተለዋጭ ማረጋገጫ ውፅዓት ከ I18NI0000073X።

እነዚህን በ `artifacts/sorafs/<tag>/monitoring/` ውስጥ ያከማቹ እና በ ውስጥ ያገናኙዋቸው
የመልቀቅ ችግር ስለዚህ የኦዲት ዱካ የCI ምዝግብ ማስታወሻዎች ካለቀ በኋላ እንዲቆይ።

## የተግባር ማረጋገጫ ዝርዝር

1. የማሰማራት መመሪያውን በደረጃ 7 ያሂዱ።
2. I18NI0000075X በምርት ውቅር ያስፈጽም; ማህደር
   የ JSON ውጤት.
3. I18NT0000007X ፓነሎችን (`docs_portal`፣ `TryItProxyErrors`፣
   `DocsPortal/GatewayRefusals`) እና ከመልቀቂያ ትኬት ጋር አያይዟቸው።
4. ተደጋጋሚ ማሳያዎችን (የሚመከር፡ በየ 15 ደቂቃው) በ
   የDOCS-3c SLO በርን ለማርካት ተመሳሳይ ውቅር ያላቸው ዩአርኤሎች።
5. በአደጋዎች ጊዜ, ለመቅዳት የመቆጣጠሪያ ትዕዛዙን በ `--json-out` እንደገና ያሂዱ.
   ከማስረጃ በፊት/በኋላ እና ከድህረ ሞት ጋር ያያይዙት።

ይህንን ዑደት ተከትሎ DOCS-3c ይዘጋል፡ የፖርታል ግንባታ ፍሰት፣ የህትመት ቧንቧ መስመር፣
እና የክትትል ቁልል አሁን በአንድ ጨዋታ ደብተር ውስጥ ሊባዙ በሚችሉ ትዕዛዞች ይኖራሉ፣
የናሙና አወቃቀሮች፣ እና ቴሌሜትሪ መንጠቆዎች።