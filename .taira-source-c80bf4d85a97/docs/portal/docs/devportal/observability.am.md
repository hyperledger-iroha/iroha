---
lang: am
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5f6d17a605e4b90d9bb9cc041055c43b2f1b384fd13f732c0a56e5de5fe78bbd
source_last_modified: "2025-12-29T18:16:35.105743+00:00"
translation_last_reviewed: 2026-02-07
id: observability
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
translator: machine-google-reviewed
---

የ DOCS-I18NT0000003X ፍኖተ ካርታ ትንታኔን፣ ሰው ሰራሽ መመርመሪያዎችን እና የተሰበረ ግንኙነትን ይፈልጋል።
ለእያንዳንዱ ቅድመ እይታ ግንባታ አውቶማቲክ። ይህ ማስታወሻ አሁን ያለውን የቧንቧ ስራ ይመዘግባል።
ኦፕሬተሮች ጎብኝዎችን ሳያፈስ ክትትል እንዲያደርጉ ፖርታሉን ይዘው ይጓዛሉ
ውሂብ.

## መለያ መስጠትን ይልቀቁ

- `DOCS_RELEASE_TAG=<identifier>` ያቀናብሩ (ወደ `GIT_COMMIT` ወይም `dev` ይመለሳል)
  ፖርታሉን መገንባት. እሴቱ ወደ `<meta name="sora-release">` ገብቷል።
  ስለዚህ መመርመሪያዎች እና ዳሽቦርዶች ማሰማራትን መለየት ይችላሉ።
- `npm run build` I18NI00000011 ኤክስ ያወጣል (የተፃፈ
  `scripts/write-checksums.mjs`) መለያውን፣ የጊዜ ማህተምን እና አማራጭን የሚገልጽ
  `DOCS_RELEASE_SOURCE`. ተመሳሳዩ ፋይል ወደ ቅድመ እይታ ቅርሶች እና
  በአገናኝ አረጋጋጭ ዘገባ ተጠቅሷል።

## ግላዊነትን የሚጠብቅ ትንታኔ

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` ወደ ያዋቅሩ
  ቀላል ክብደት መከታተያ አንቃ። የሚጫኑ ጭነቶች `{ ክስተት፣ መንገድ፣ አካባቢ፣
  መልቀቅ፣ ts }` with no referrer or IP metadata, and `navigator.sendBeacon`
  አሰሳዎችን ማገድን ለማስወገድ በሚቻልበት ጊዜ ሁሉ ጥቅም ላይ ይውላል።
- ናሙናዎችን በI18NI0000016X (0-1) ይቆጣጠሩ። መከታተያው ያከማቻል
  የመጨረሻው የተላከው መንገድ እና ለተመሳሳይ አሰሳ የተባዙ ክስተቶችን በጭራሽ አያወጣም።
- አተገባበሩ በ I18NI0000017X ውስጥ ይኖራል እና ነው።
  በአለምአቀፍ ደረጃ በ `src/theme/Root.js` በኩል ተጭኗል።

## ሰው ሰራሽ መመርመሪያዎች

- `npm run probe:portal` በጋራ መንገዶች ላይ የGET ጥያቄዎችን ያወጣል።
  (`/`፣ `/norito/overview`፣ `/reference/torii-swagger`፣ ወዘተ) እና ያረጋግጣል።
  `sora-release` ሜታ መለያ ከ `--expect-release` ጋር ይዛመዳል (ወይም
  `DOCS_RELEASE_TAG`)። ምሳሌ፡-

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

አለመሳካቶች በየመንገዱ ሪፖርት ይደረጋሉ፣ ይህም ሲዲ በምርመራ ስኬት ላይ በቀላሉ ለመግባት ያስችላል።

## የተሰበረ-አገናኝ አውቶማቲክ

- `npm run check:links` `build/sitemap.xml` ስካን ያደርጋል፣ እያንዳንዱን ካርታ ወደ አንድ ግቤት ያረጋግጣል።
  አካባቢያዊ ፋይል (`index.html` fallbacks በመፈተሽ) እና ይጽፋል
  `build/link-report.json` የሚለቀቀውን ሜታዳታ፣ ጠቅላላ፣ ውድቀቶች፣
  እና የSHA-256 የ I18NI0000030X አሻራ (እንደ `manifest.id` የተጋለጠ)
  ስለዚህ እያንዳንዱ ዘገባ ከሥነ ጥበብ መግለጫው ጋር ሊያያዝ ይችላል።
- አንድ ገጽ ሲጠፋ ስክሪፕቱ ከዜሮ ውጭ ይወጣል፣ ስለዚህ CI ልቀቶችን ማገድ ይችላል።
  የቆዩ ወይም የተሰበሩ መንገዶች. ሪፖርቶች የተሞከሩትን የእጩ መንገዶችን ይጠቅሳሉ፣
  ወደ ዶክ ዛፉ የማዞሪያ መንገዶችን ለመከታተል የሚረዳ።

## Grafana ዳሽቦርድ እና ማንቂያዎች

- `dashboards/grafana/docs_portal.json` ** የሰነዶች ፖርታል ሕትመትን ያትማል **
  Grafana ሰሌዳ. የሚከተሉትን ፓነሎች ይልካል።
  - *የጌትዌይ እምቢታ (5ሜ)* `torii_sorafs_gateway_refusals_total` ይጠቀማል
    `profile`/`reason` ስለዚህ SREዎች መጥፎ የፖሊሲ ግፊቶችን ወይም የቶከን ውድቀቶችን ለይተው ማወቅ ይችላሉ።
  - *Alias Cache Refresh results* እና *Alias Proof Age p90* ትራክ
    `torii_sorafs_alias_cache_*` ዲ ኤን ኤስ ከመቆረጡ በፊት ትኩስ ማረጋገጫዎች መኖራቸውን ለማረጋገጥ
    በላይ።
  - *የፒን መዝገብ ቤት መግለጫ ብዛት* እና *የገቢር ተለዋጭ ስም ብዛት* ስታቲስቲክስ
    የፒን መዝገብ መዝገብ እና አጠቃላይ ተለዋጭ ስሞች አስተዳደር እያንዳንዱን እትም ኦዲት ማድረግ ይችላል።
  - *ጌትዌይ TLS የሚያበቃበት (ሰዓታት)* የማተሚያ መግቢያ በር TLS ጊዜ ድምቀቶች
    የተረጋገጠ አቀራረቦች ጊዜው ያበቃል (የማስጠንቀቂያ ገደብ በ 72 ሰ)።
  - * ማባዛት SLA ውጤቶች * እና * ማባዛት Backlog * ይከታተሉ
    ሁሉም ቅጂዎች GA መገናኘታቸውን ለማረጋገጥ `torii_sorafs_replication_*` ቴሌሜትሪ
    ባር ከታተመ በኋላ.
- አብሮ የተሰራውን የአብነት ተለዋዋጮችን ይጠቀሙ (`profile`፣ `reason`)
  `docs.sora` መገለጫን ማተም ወይም በሁሉም የመግቢያ መንገዶች ላይ ሹልቶችን መርምር።
- PagerDuty ራውቲንግ ዳሽቦርድ ፓነሎችን እንደ ማስረጃ ይጠቀማል፡ ማንቂያዎች ተሰይመዋል
  `DocsPortal/GatewayRefusals`፣ `DocsPortal/AliasCache`፣ እና
  `DocsPortal/TLSExpiry` እሳት ተጓዳኝ ተከታታዮች ሲጥሱ
  ገደቦች. የጥሪ መሐንዲሶች እንዲችሉ የማንቂያውን ማስኬጃ መጽሐፍ ከዚህ ገጽ ጋር ያገናኙ
  ትክክለኛውን I18NT0000000X መጠይቆችን እንደገና ያጫውቱ።

## አንድ ላይ በማጣመር

1. በ `npm run build` ጊዜ፣ የመልቀቂያ/ትንታኔ የአካባቢ ተለዋዋጮችን እና
   ከግንባታ በኋላ ያለው እርምጃ `checksums.sha256`፣ `release.json`፣ እና
   `link-report.json`.
2. `npm run probe:portal`ን በቅድመ እይታ አስተናጋጅ ስም ያሂዱ
   `--expect-release` በተመሳሳዩ መለያ ላይ ተጣብቋል። stdout ለህትመት ያስቀምጡ
   የማረጋገጫ ዝርዝር.
3. በተበላሹ የጣቢያ ካርታ ግቤቶች እና ማህደር ላይ በፍጥነት እንዳይሳካ `npm run check:links` ን ያሂዱ
   የመነጨው JSON ሪፖርት ከቅድመ-ዕይታ ቅርሶች ጋር። CI ን ይጥላል
   የቅርብ ጊዜ ሪፖርት በ `artifacts/docs_portal/link-report.json` ስለዚህ አስተዳደር ይችላል።
   የማስረጃውን ጥቅል በቀጥታ ከግንባታ ምዝግብ ማስታወሻዎች ያውርዱ።
4. የትንታኔውን የመጨረሻ ነጥብ ወደ ግላዊነት ጥበቃ ሰብሳቢዎ ያስተላልፉ (ተጨባጭ፣
   በራስ የሚስተናገደው OTEL ኢንጀስት፣ ወዘተ.) እና የናሙና ተመኖች በእያንዳንዱ መመዝገባቸውን ያረጋግጡ
   መልቀቅ ስለዚህ ዳሽቦርዶች በትክክል ይቆጥራሉ.
5. CI እነዚህን ደረጃዎች አስቀድሞ በቅድመ-እይታ/የስራ ፍሰቶችን በማሰማራት ሽቦዎች ሰጥቷቸዋል።
   (`.github/workflows/docs-portal-preview.yml`፣
   `.github/workflows/docs-portal-deploy.yml`)፣ ስለዚህ የአካባቢ ደረቅ ሩጫዎች ብቻ ያስፈልጋቸዋል
   ሚስጥራዊ-ተኮር ባህሪን ይሸፍኑ።