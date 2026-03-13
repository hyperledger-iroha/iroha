---
lang: am
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-12-29T18:16:35.156432+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

ይህ ክፍል ለI18NT0000003X "እንደ ዝርዝር አንብበው" ማቴሪያሎችን ያጠቃልላል። እነዚህ ገፆች እንደዚ እንኳን ተረጋግተው ይቆያሉ።
መመሪያዎች እና አጋዥ ስልጠናዎች ይሻሻላሉ.

## ዛሬ ይገኛል።

- ** Norito የኮዴክ አጠቃላይ እይታ *** - `reference/norito-codec.md` በቀጥታ ወደ ባለስልጣኑ ያገናኛል
  የፖርታል ጠረጴዛው እየሞላ እያለ `norito.md` ዝርዝር መግለጫ።
- **Torii I18NT0000000X** - I18NI0000010X የቅርብ ጊዜውን የ Torii REST መግለጫን በመጠቀም ያቀርባል
  እንደገና። ዝርዝሩን በI18NI0000011X ያድሱ (አክል
  ቅጽበተ-ፎቶውን ወደ ተጨማሪ ታሪካዊ ስሪቶች ለመቅዳት `--mirror=<label>`)።
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v2/mcp`.
- ** የማዋቀሪያ ሠንጠረዦች *** - የሙሉ መለኪያ ካታሎግ ተቀምጧል
  `docs/source/references/configuration.md`. ፖርታሉ ራስ-ማስመጣት እስከሚልክ ድረስ፣ ያንን ያጣቅሱ
  ለትክክለኛ ነባሪዎች እና የአካባቢ መሻሮች ምልክት ማድረጊያ ፋይል።
- ** የሰነዶች ስሪት *** - የ navbar ስሪት ተቆልቋይ የቀዘቀዙ ቅጽበተ-ፎቶዎችን ያጋልጣል
  `npm run docs:version -- <label>`፣ በመልቀቂያዎች ላይ መመሪያዎችን ማወዳደር ቀላል ያደርገዋል።

## በቅርብ ቀን

- **Torii REST ማጣቀሻ *** - OpenAPI ፍቺዎች በዚህ ክፍል በኩል ይመሳሰላሉ
  ቧንቧው አንዴ ከነቃ `docs/portal/scripts/sync-openapi.mjs`።
- ** CLI የትዕዛዝ መረጃ ጠቋሚ ** - የመነጨ የትዕዛዝ ማትሪክስ (የሚያንጸባርቅ `crates/iroha_cli/src/commands`)
  ከቀኖናዊ ምሳሌዎች ጋር እዚህ ያርፋል።
- **IVM ABI ሰንጠረዦች** - የጠቋሚው አይነት እና ሲስካል ማትሪክስ (በ`crates/ivm/docs` የተቀመጠ)
  የዶክ ማመንጨት ሥራ ከተጣበቀ በኋላ ወደ ፖርታል ይቀርባል።

## ይህን ኢንዴክስ ወቅታዊ ማድረግ

አዲስ የማመሳከሪያ ቁሳቁስ ሲታከል - የመነጨ የኤፒአይ ሰነዶች፣ የኮዴክ ዝርዝሮች፣ የውቅር ማትሪክስ - ቦታ
በ `docs/portal/docs/reference/` ስር ያለውን ገጽ እና ከላይ ያገናኙት። አንድ ገጽ በራስ-ሰር የተፈጠረ ከሆነ፣ እባክዎን ልብ ይበሉ
አስተዋጽዖ አበርካቾች እንዴት እንደሚያድሱት እንዲያውቁ የማመሳሰል ስክሪፕት። ይህ የማጣቀሻ ዛፉ እስከ እ.ኤ.አ. ድረስ ጠቃሚ ያደርገዋል
ሙሉ በሙሉ በራስ-ሰር የተፈጠረ የአሰሳ መሬቶች።
