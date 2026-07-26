---
lang: am
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
translator: machine-google-reviewed
---

# የግምገማ ፓነል ማጠቃለያ (MINFO-4a)

የመንገድ ካርታ ንጥል **MINFO-4a — ገለልተኛ ማጠቃለያ ጄኔሬተር** ተቀባይነት ያለው የአጀንዳ ፕሮፖዛል፣ የበጎ ፈቃደኞች አጭር ኮርፐስ እና የተረጋገጠው የ AI ልከኝነት ወደ ገለልተኛ የሪፈረንደም ማጠቃለያ የሚቀይር ሊባዛ የሚችል የስራ ሂደት ይፈልጋል። ማስረከብ ያለበት፡-

- ውጤቱን እንደ Norito መዋቅር (`ReviewPanelSummaryV1`) ይመዝግቡ ስለዚህ አስተዳደር ከማኒፌክት እና ከድምጽ መስጫ ወረቀቶች ጋር አብሮ በማህደር እንዲቀመጥ ያድርጉ።
- ምንጩን ይዘርጉ፣ የግምገማ ፓነል ሚዛናዊ ድጋፍ/የተቃውሞ ሽፋን ከሌለው ወይም እውነታዎች ጥቅሶች ሲቀሩ በፍጥነት አለመሳካቱ።
- የመመሪያው ዳኞች ድምጽ ከመስጠታችሁ በፊት ሁለቱንም አውቶማቲክ እና የሰው አውድ መመልከቱን በማረጋገጥ የ AI ዝርዝር መግለጫውን እና የፕሮፖዛል ማስረጃውን ቅርቅብ በሁሉም ድምቀቶች ያጣቅሱ።

## የ CLI አጠቃቀም

የስራ ፍሰቱ እንደ `cargo xtask` አካል ነው፡-

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

የሚያስፈልጉ ግብዓቶች፡-

1. `--proposal` - JSON ክፍያ ከ `AgendaProposalV1` ጋር የሚጣበቅ። ረዳቱ ማጠቃለያውን ከማፍለቁ በፊት ንድፉን ያረጋግጣል።
2. `--volunteer` - JSON ድርድር `docs/source/ministry/volunteer_brief_template.md` የሚከተሉ የበጎ ፈቃደኞች አጭር መግለጫ። ከርዕስ ውጪ ያሉ ግቤቶች በራስ ሰር ችላ ይባላሉ።
3. `--ai-manifest` - በአስተዳደር የተፈረመ `ModerationReproManifestV1` ይዘቱን ያጣሩትን AI ኮሚቴ ይገልፃል።
4. `--panel-round` - ለአሁኑ የግምገማ ዙር መለያ (`RP-YYYY-##`)።
5. `--output` - መድረሻ ፋይል ወይም `-` ወደ stdout ለመልቀቅ። የፕሮፖዛል ቋንቋውን ለመሻር `--language` እና `--generated-at` ታሪክን በሚሞሉበት ጊዜ የሚወስን የዩኒክስ የጊዜ ማህተም (ሚሊሰከንዶች) ይጠቀሙ።

አንድ ጊዜ ብቻውን ማጠቃለያው ከተፈጠረ፣ ያሂዱት
[`cargo xtask ministry-panel packet`](referendum_packet.md) ረዳት ለመሰብሰብ
የተሟላ ሪፈረንደም ዶሴ (`ReferendumPacketV1`)። ማቅረብ
`--summary-out` ወደ ፓኬት ትዕዛዝ ተመሳሳይ የማጠቃለያ ፋይል ይቆያል
ለታችኛው ተፋሰስ ሸማቾች በፓኬት እቃ ውስጥ መክተት።

### አውቶማቲክ በ `ministry-transparency ingest`

`cargo xtask ministry-transparency ingest`ን ለሩብ አመት የማስረጃ እሽጎች የሚያሄዱ ቡድኖች አሁን የግምገማ ፓነል ማጠቃለያውን በተመሳሳይ መስመር መስፋት ይችላሉ፡

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

አራቱም የ`--panel-*` ባንዲራዎች አንድ ላይ መቅረብ አለባቸው (እና `--volunteer` ያስፈልጋቸዋል)። ትዕዛዙ የግምገማ ፓነልን ማጠቃለያ ለ`--panel-summary-out` ያወጣል፣የተተነተነውን ጭነት በመግቢያ ቅጽበታዊ ገጽ እይታ ውስጥ አካቷል እና ቼክተም ይመዘግባል ስለዚህ የታችኛው ተፋሰስ መሳሪያዎች ማስረጃውን ይመሰክራል።

## ሊንቲንግ እና ውድቀት ሁነታዎች

`cargo xtask ministry-panel synthesize` ማጠቃለያውን ከመጻፍዎ በፊት የሚከተሉትን ተለዋዋጮች ያስፈጽማል፡

- **ሚዛናዊ አቋሞች፡** ቢያንስ አንድ የድጋፍ አጭር እና አንድ ተቃዋሚ አጭር መገኘት አለበት። የጠፋ ሽፋን ሩጫውን ገላጭ በሆነ ስህተት ያጠናቅቃል።
- ** የጥቅስ ሽፋን፡** ድምቀቶች የሚዘጋጁት ጥቅሶችን ካካተቱ የእውነት ረድፎች ብቻ ነው። የጎደሉ ጥቅሶች ግንቡን በጭራሽ አያግደውም ነገር ግን እያንዳንዱ የተነካ አጭር መግለጫ በውጤቱ ውስጥ በ`warnings[]` ተዘርዝሯል።
- **በየድምቀት ማጣቀሻዎች፡** እያንዳንዱ ድምቀት ለ(ሀ) የፍቃደኛ እውነታ ረድፍ(ዎች)፣ (ለ) የ AI መግለጫ መታወቂያ እና (ሐ) ከፕሮፖዛሉ የተገኘ የመጀመሪያ ማስረጃ አባሪ ማጣቀሻዎችን ያካትታል ስለዚህ ፓኬጁ ሁል ጊዜ ወደ ፊርማዎቹ ቅርሶች ይመለሳል።ማንኛውም ቼክ ካልተሳካ ትዕዛዙ ዜሮ ባልሆነ ሁኔታ ይወጣል እና በችግር መዝገብ ላይ ይጠቁማል። የተሳካላቸው ሩጫዎች ከ`ReviewPanelSummaryV1` እቅድ ጋር የሚዛመድ እና በአስተዳደር መግለጫዎች ውስጥ ሊካተት የሚችል የJSON ፋይል ይጽፋሉ።

## የውጤት መዋቅር

`ReviewPanelSummaryV1` የሚኖረው በ`crates/iroha_data_model/src/ministry/mod.rs` ሲሆን ለእያንዳንዱ ሸማች በ`iroha_data_model` ሳጥን በኩል ይገኛል። ዋና ክፍሎች የሚከተሉትን ያካትታሉ:

- `overview` – ርዕስ፣ ገለልተኛ ማጠቃለያ ዓረፍተ ነገር እና የውሳኔ አውድ ለፖሊሲ ዳኞች ፓኬት።
- `stance_distribution` - አጭር መግለጫዎች ብዛት እና የእውነታ ረድፎች በአንድ አቋም። የታችኛው ዳሽቦርዶች ከማተምዎ በፊት ሽፋንን ለማረጋገጥ ይህንን ያንብቡ።
- `highlights` - በአንድ አቋም እስከ ሁለት እውነታዎች ማጠቃለያዎች ሙሉ ብቃት ካላቸው ጥቅሶች ጋር።
- `ai_manifest` – ከዳግም መራባት አንጸባራቂ የወጣ ሜታዳታ (አንጸባራቂ UUID፣ ሯጭ ስሪት፣ ገደቦች)።
- `volunteer_references` - ለአጭር ጊዜ ስታቲስቲክስ (ቋንቋ, አቋም, ረድፎች, የተጠቀሱ ረድፎች) ለኦዲት.
- `warnings` – ነፃ ቅጽ የተዘለሉ ዕቃዎችን የሚገልጹ ሊንት መልዕክቶች (ለምሳሌ፣ የጎደሉ ጥቅሶች ያሉት የእውነታ ረድፎች)።

#ምሳሌ

`docs/examples/ministry/review_panel_summary_example.json` ከረዳት ጋር የተሰራ ሙሉ ናሙና ይዟል. የተመጣጠነ ድጋፍ/የመቃወም ሽፋን፣ የጥቅስ መስመር ዝርጋታ፣ አንጸባራቂ ማጣቀሻዎች እና የማስጠንቀቂያ ሕብረቁምፊዎችን ለዕውነታ ረድፎች ወደ ድምቀቶች ከፍ ማድረግ ላልቻሉ ያሳያል። ገለልተኛ ማጠቃለያውን መጠቀም የሚያስፈልጋቸው ዳሽቦርዶች፣ የአስተዳደር መግለጫዎች ወይም የኤስዲኬ መሣሪያ ሲራዘም ይጠቀሙበት።

> ** ጠቃሚ ምክር፡** የመነጨውን ማጠቃለያ ከተፈረመው የ AI ዝርዝር መግለጫ ጋር ያካትቱ እና በሪፈረንደም ማስረጃ ጥቅል ውስጥ የበጎ ፈቃደኞች አጭር መግለጫ በማካተት በግምገማ ፓነል የተጠቀሰውን እያንዳንዱን ቅርስ የፖሊሲ ዳኞች ማረጋገጥ ይችላሉ።