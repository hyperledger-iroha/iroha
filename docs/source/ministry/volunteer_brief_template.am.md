---
lang: am
direction: ltr
source: docs/source/ministry/volunteer_brief_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cae4747782524b545fdcd52e7523cce0f5b60ddb85f32c747c5f57a63f85ccdc
source_last_modified: "2025-12-29T18:16:35.984008+00:00"
translation_last_reviewed: 2026-02-07
title: Volunteer Brief Template
summary: Structured template for roadmap item MINFO-3a covering balanced briefs, fact tables, conflict disclosures, and moderation tags.
translator: machine-google-reviewed
---

# የበጎ ፈቃደኞች አጭር አብነት (MINFO-3a)

የመንገድ ካርታ ማጣቀሻ፡- **MINFO-3a — ሚዛናዊ አጭር አብነቶች እና የግጭት መግለጫ።**

የበጎ ፈቃደኞች አጭር ማቅረቢያ የዜጎች ፓነሎች አስተዳደር እንዲከለስላቸው የሚፈልጓቸውን የጥቁር መዝገብ ለውጦች ወይም ሌሎች የሚኒስቴር ማስፈጸሚያ ቅስቀሳዎች ሲቀርቡ አቋሞችን ያጠቃልላል። MINFO-3a እያንዳንዱ አጭር መግለጫ የሚወስን መዋቅር እንዲከተል ይፈልጋል ስለዚህ የግልጽነት ቧንቧ መስመር (1) ተመጣጣኝ የሐቅ ሠንጠረዦችን ማቅረብ፣ (2) የፍላጎት ግጭቶች መገለጣቸውን ማረጋገጥ እና (3) ከርዕስ ውጪ የሚቀርቡ ግቤቶችን መጣል ወይም መጠቆም ይችላል። ይህ ገጽ በ`cargo xtask ministry-transparency` ውስጥ በተላኩ መሳሪያዎች የሚጠበቁ ቀኖናዊ መስኮችን፣ የCSV አይነት የእውነታ ሠንጠረዥ አቀማመጥ እና የአወያይ መለያዎችን ይገልጻል።

> ** Norito እቅድ፡** የ`iroha_data_model::ministry::VolunteerBriefV1` መዋቅር (ስሪት `1`) አሁን ለሁሉም ማቅረቢያዎች ስልጣን ያለው እቅድ ነው። አጭር መግለጫ ከማተም ወይም በፓናል ማጠቃለያ ላይ ከማጣቀስ በፊት የመሳሪያ እና የፖርታል አረጋጋጮች `VolunteerBriefV1::validate` ይደውላሉ።

## የማስረከቢያ ጭነት መዋቅር

| ክፍል | መስኮች | መስፈርቶች |
|--------|--------|-------------|
| ** ፖስታ *** | `version` (u16) | `1` መሆን አለበት። የስሪት ጠባቂው ሚኒስቴር መሥሪያ ቤቱን ያለምንም ጥርጣሬ እንዲያሻሽል ያስችለዋል። |
| ** ማንነት እና አቋም** | `brief_id` (ሕብረቁምፊ፣ በቀን መቁጠሪያ ዓመት ልዩ)፣ `proposal_id` (ወደ ጥቁር መዝገብ ወይም የፖሊሲ እንቅስቃሴ አገናኞች)፣ `language` (BCP-47)፣ `stance` (`support`/`oppose`/`context`)፣ `submitted_at` (RFC3339) | ሁሉም መስኮች ያስፈልጋሉ። `stance` ዳሽቦርዶችን ይመገባል እና ከተፈቀደው የቃላት ዝርዝር ጋር መዛመድ አለበት። |
| **የደራሲ መረጃ** | `author.name`፣ `author.organization` (አማራጭ)፣ `author.contact`፣ `author.no_conflicts_certified` (ቦል) | `author.contact` ከህዝባዊ ዳሽቦርድ ተሰርዟል ነገር ግን በጥሬ ዕቃው ውስጥ ተከማችቷል። `no_conflicts_certified: true` ያዋቅሩት ደራሲው ምንም ይፋዊ መግለጫዎች እንደማይተገበሩ ካረጋገጡ ብቻ ነው። |
| **ማጠቃለያ** | `summary.title`፣ `summary.abstract`፣ `summary.requested_action` | የጽሑፍ አጠቃላይ እይታ ከእውነታው ሠንጠረዥ አጠገብ ወጣ። `summary.abstract` ወደ ≤2000 ቁምፊዎች ገድብ። |
| **የእውነታ ሰንጠረዥ** | `fact_table` ድርድር (ቀጣዩን ክፍል ይመልከቱ) | ለአጭር አጭር መግለጫዎች እንኳን ያስፈልጋል። የ CLI እና የግልጽነት ስራ ያለመረጃ ሠንጠረዥ አቅርቦቶችን ውድቅ ያደርጋል። |
| ** መግለጫዎች *** | `disclosures` ድርድር ወይም `author.no_conflicts_certified: true` | እያንዳንዱ የመግለጫ ረድፍ `type` (`financial`፣ `employment`፣ `governance`፣ `family`፣ `other`፣ I18000000038X፣ I18009 `relationship`፣ እና `details`። |
| ** ልከኝነት ሜታዳታ** | `moderation.off_topic` (bool), `moderation.tags` (የ enum ሕብረቁምፊዎች ድርድር), `moderation.notes` | ኮከብ ቆጣሪዎችን ወይም ተዛማጅ ያልሆኑ ግቤቶችን ለማፈን በገምጋሚዎች ጥቅም ላይ ይውላል። ከርዕስ ውጪ ያሉ ግቤቶች ለዳሽቦርዶች አስተዋጽዖ አያደርጉም። |

## የእውነታ ሰንጠረዥ መግለጫ

እያንዳንዱ `fact_table` ረድፍ በማሽን ሊነበብ የሚችል የይገባኛል ጥያቄን ይይዛል። ረድፎቹን ከሚከተሉት መስኮች ጋር እንደ JSON ነገሮች ያከማቹ፡| መስክ | መግለጫ |
|-------|-----------|
| `claim_id` | የተረጋጋ ለዪ (ለምሳሌ፡ `VB-2026-04-F1`)። |
| `claim` | የአንድ-ዓረፍተ ነገር እውነታ ወይም ተጽዕኖ መግለጫ። |
| `status` | ከ `corroborated`፣ `disputed`፣ `context-only` አንዱ። |
| `impact` | አንድ ወይም ከዚያ በላይ `governance`፣ `technical`፣ `compliance`፣ `community` የያዘ ድርድር። |
| `citations` | ባዶ ያልሆኑ የሕብረቁምፊዎች ስብስብ። ዩአርኤሎች፣ Torii የጉዳይ መታወቂያዎች ወይም የCID ማጣቀሻዎች ተቀባይነት አላቸው። |
| `evidence_digest` | አማራጭ BLAKE3 የድጋፍ ሰነዶች ቼክ ድምር። |

ራስ-ሰር ማስታወሻዎች:
- የህትመት ውጤት ካርዶችን ለመገንባት የሚያስገባው ስራ `fact_rows` እና `fact_rows_with_citation` ይቆጥራል። ጥቅሶች የሌላቸው ረድፎች በሰው ሊነበቡ በሚችሉት ሠንጠረዥ ውስጥ አሁንም ይታያሉ ነገር ግን እንደ የጎደሉ ማስረጃዎች ክትትል ይደረግባቸዋል።
- የይገባኛል ጥያቄዎችን አጠር አድርገው ያስቀምጡ እና በአስተዳደር ሀሳቦች ውስጥ ጥቅም ላይ የሚውሉትን ተመሳሳይ መለያዎችን ያጣቅሱ ስለዚህ ማገናኘት የሚወስን ነው።

## የግጭት መግለጫ መስፈርቶች

1. የገንዘብ፣የስራ፣የአስተዳደር ወይም የቤተሰብ ትስስር ሲኖር ቢያንስ አንድ ይፋዊ መግቢያ ያቅርቡ።
2. “ያልታወቁ ግጭቶች” ለማለት `author.no_conflicts_certified: true` ይጠቀሙ። ማስረከቦች ይፋ ማውጣት ወይም የ`true` ማረጋገጫን ማካተት አለባቸው። ያለበለዚያ ፣ በሚጠጡበት ጊዜ ምልክት ይደረግባቸዋል ።
3. ይፋዊ ሰነዶች በሚኖሩበት ጊዜ ሁሉ `disclosures[i].evidence`ን ያካትቱ (ለምሳሌ፣ የድርጅት ሰነዶች፣ የDAO ድምጾች)። ለ"ምንም" ማረጋገጫዎች ማስረጃ አማራጭ ነው ነገር ግን በጥብቅ የሚመከር።

## የአወያይ መለያዎች እና ከርዕስ ውጪ አያያዝ

የአወያይ ገምጋሚዎች ወደ ግልፅነት ቧንቧው ከመግባታቸው በፊት ማቅረቢያዎችን መሰየም ይችላሉ፡-

- `moderation.off_topic: true` የ `off_topic_rejections` ቆጣሪን በመጨመር ግቤቱን ከድምር ቆጠራ ያስወግዳል። ረድፉ አሁንም ለኦዲት በጥሬ መዛግብት ይገኛል።
- `moderation.tags` የቁጥር እሴቶችን ይቀበላል፡ `duplicate`፣ `needs-translation`፣ `needs-follow-up`፣ `spam`፣ `astroturf`፣ I180000 ሙሉ አጭር መግለጫውን ሳያነቡ መለያዎች የታችኛው ተፋሰስ ገምጋሚዎች እንዲለዩ ያግዛሉ።
- `moderation.notes` ለሽምግልና ውሳኔ (≤512 ቁምፊዎች) አጭር ማረጋገጫ ያከማቻል።

## የማስረከቢያ ዝርዝር

1. ይህንን አብነት ወይም ከዚህ በታች የተገለጸውን የረዳት CLI በመጠቀም የJSON ክፍያን ይሙሉ።
2. ቢያንስ አንድ የእውነታ ሠንጠረዥ ረድፍ ብዛት; ለእያንዳንዱ ረድፍ ጥቅሶችን ያካትቱ.
3. ይፋዊ መግለጫዎችን ያቅርቡ ወይም `author.no_conflicts_certified: true` በግልፅ ያዘጋጁ።
4. ገምጋሚዎች በፍጥነት መለየት እንዲችሉ የአወያይ ሜታዳታ (ነባሪው `off_topic: false`) ያያይዙ።
5. ከመጫንዎ በፊት ክፍያውን በ`cargo xtask ministry-transparency ingest --volunteer <file>` ወይም በማንኛውም Norito አረጋጋጭ ያረጋግጡ።

## ማረጋገጫ CLI (MINFO-3)

ማከማቻው አሁን ለፈቃደኛ አጭር መግለጫዎች ራሱን የቻለ አረጋጋጭ ይልካል።

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input docs/examples/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

ቁልፍ ባህሪ፡- የግለሰብ JSON ዕቃዎችን * ወይም * የአጭር መግለጫዎችን ይቀበላል; በአንድ ሩጫ ውስጥ ብዙ ፋይሎችን ለመደርደር `--input` ብዙ ጊዜ ማለፍ።
- የስህተቶችን እና የማስጠንቀቂያዎችን ብዛት የሚያሳይ አጭር ማጠቃለያ ያወጣል; ማስጠንቀቂያዎች ባዶ የጥቅስ ዝርዝሮችን ወይም ረጅም ማስታወሻዎችን ያጎላሉ, ስህተቶች ግን ህትመቶችን ያግዳሉ.
- የሚፈለጉ መስኮች (`brief_id`፣ `proposal_id`፣ `stance`፣ የእውነታ ሠንጠረዥ ይዘቶች፣ ይፋ መግለጫዎች ወይም `no_conflicts_certified`) ከዚህ አብነት ጋር እንደሚዛመዱ እና የቁጥር እሴቶች በሰነድ መዝገበ-ቃላት ውስጥ እንደሚቆዩ ያረጋግጣል።
- `--json-output <path>` ሲዋቀር አረጋጋጩ በማሽን የሚነበብ አንጸባራቂ ይጽፋል እያንዳንዱን አጭር ማጠቃለያ (የፕሮፖዛል መታወቂያ፣ አቋም፣ አቋም፣ ስህተቶች/ማስጠንቀቂያዎች)። የፖርታሉ `npm run generate:volunteer-lint` ትእዛዝ ከእያንዳንዱ የፕሮፖዛል ገጽ ቀጥሎ ያለውን ግልጽ ሁኔታ ለማሳየት ይህንን አንጸባራቂ ይበላል።

የበጎ ፈቃደኞች ማቅረቢያዎች ግልጽነት ወደ ውስጥ ከመግባታቸው በፊት ከ **MINFO-3** ጋር ተገዢ እንዲሆኑ ትዕዛዙን ወደ ፖርታል የስራ ፍሰቶች ወይም CI ያዋህዱ።

## ምሳሌ ጭነት

የእውነታ ሰንጠረዥ ረድፎችን፣ መግለጫዎችን እና የአወያይ መለያዎችን ጨምሮ ሙሉ ለሙሉ ለተሞላ ምሳሌ `docs/examples/ministry/volunteer_brief_template.json` ይመልከቱ። የታችኛው ዳሽቦርዶች ጥሬውን JSON ይበላሉ እና በራስ ሰር ያሰላሉ፡-

- `total_briefs` (ከርዕስ ውጪ ማስገባቶች አልተካተቱም)
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

አዳዲስ መስኮች ከተፈለጉ፣ ይህንን ሰነድ እና የመግቢያ ማጠቃለያውን (`xtask/src/ministry.rs`) ያዘምኑ ስለዚህ የአስተዳደር ማስረጃው እንደገና ሊባዛ ይችላል።

## ሕትመት SLA እና ፖርታል ወለል (MINFO-3)

የዜጎችን ማስረከቦች ግልፅነት ለመጠበቅ ፖርታሉ አሁን ማረጋገጫውን ካለፉ በኋላ ስለ ቋሚ ቃላቶች አጭር መግለጫዎችን ያትማል፡-

1. **T+0–6ሰዓት፡** መሬት በበጎ ፈቃደኝነት ቅበላ ቅፅ ወይም `cargo xtask ministry-transparency ingest` በኩል ያቀርባል። አረጋጋጮች `VolunteerBriefV1::validate` ን ያሂዳሉ፣ የተበላሹ ሸክሞችን ውድቅ ያደርጋሉ እና የተዘበራረቁ ሪፖርቶችን ያሰራጫሉ (የጎደሉ መግለጫዎች፣ የተባዙ የእውነታ መታወቂያዎች፣ ወዘተ)።
2. **T+6–24ሰአታት፡** ተቀባይነት ያላቸው አጭር መግለጫዎች ለትርጉም/ለተለያዩ ተሰልፈዋል። የአወያይ መለያዎች (`needs-translation`፣ `duplicate`፣ `policy-escalation`፣ …) ተተግብረዋል፣ እና ከርዕስ ውጪ ያሉ ግቤቶች በማህደር ተቀምጠዋል ነገር ግን ከድምር ቆጠራዎች የተገለሉ ናቸው።
3. **T+24–48ሰአታት፡** ፖርታሉ አጭር ዘገባውን ከተዛማጅ ፕሮፖዛል ገጽ ጋር ያትማል። እያንዳንዱ የታተመ ፕሮፖዛል አሁን ከ"ፍቃደኛ አስተያየቶች" ጋር ይገናኛል ስለዚህ ገምጋሚዎች ጥሬ JSON ሳይከፍቱ የድጋፍ/መቃወም/የአውድ አጭር መግለጫዎችን ማንበብ ይችላሉ።

አንድ ግቤት `policy-escalation` ወይም `astroturf` ምልክት ከተደረገበት፣ SLA ወደ **12ሰአታት** ያጠነክራል ስለዚህ አስተዳደር በፍጥነት ምላሽ መስጠት ይችላል። ኦፕሬተሮች SLA ኦዲት ማድረግ ይችላሉ ** የበጎ ፈቃደኞች አጭር መግለጫ ** ገጽ በሰነዶች ፖርታል (`docs/portal/docs/ministry/volunteer-briefs.md`) ፣ እሱም የቅርብ ጊዜዎቹን የሕትመት መስኮቶች ፣ የ lint ሁኔታ እና ከ Norito ቅርሶች ጋር ይዘረዝራል።