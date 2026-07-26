---
lang: am
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
translator: machine-google-reviewed
---

# ግልጽነት እና የኦዲት ሪፖርቶች (MINFO-8)

የመንገድ ካርታ ማጣቀሻ፡- **MINFO-8 — ግልጽነት እና የኦዲት ሪፖርቶች** እና **MINFO-8a — ግላዊነትን የሚጠብቅ የመልቀቂያ ሂደት**

የማስታወቂያ ሚኒስቴር ህብረተሰቡ የልከኝነትን ውጤታማነት፣ የይግባኝ አያያዝ እና የጥቁር መዝገብ መዝገብን ኦዲት ማድረግ እንዲችል ወሳኙ የግልጽነት ቅርሶችን ማተም አለበት። ይህ ሰነድ MINFO-8ን ከQ32026 ዒላማው በፊት ለመዝጋት የሚያስፈልጉትን ወሰን፣ ቅርሶች፣ የግላዊነት ቁጥጥር እና የስራ ሂደት ይገልጻል።

## ግቦች እና ሊደርሱ የሚችሉ

- የ AI ልከኝነት ትክክለኛነትን ፣ የይግባኝ ውጤቶችን ፣ የክህደት ቃላቶችን ፣ የበጎ ፈቃደኞች ፓነል እንቅስቃሴን እና ከMINFO በጀቶች ጋር የተሳሰሩ የግምጃ ቤት እንቅስቃሴዎችን የሚያጠቃልሉ የሩብ ወር የግልጽነት ፓኬቶችን ያመርቱ።
- ጥሬ ዳታ ቅርቅቦችን (Norito JSON + CSV) እና ዳሽቦርዶችን በማጓጓዝ ዜጎች የማይለዋወጥ ፒዲኤፎችን ሳይጠብቁ መለኪያዎችን ይቆርጣሉ።
- ማንኛውም የውሂብ ስብስብ ከመታተሙ በፊት የግላዊነት ዋስትናዎችን (የተለያዩ የግላዊነት + አነስተኛ ቆጠራ ህጎች) እና የተፈረሙ ማረጋገጫዎችን ያስፈጽሙ።
- ታሪካዊ ቅርሶች የማይለወጡ እና በተናጥል ሊረጋገጡ የሚችሉ ሆነው እንዲቀጥሉ እያንዳንዱን እትም በአስተዳደር DAG እና Grafana ውስጥ ይመዝግቡ።

### Artefact ማትሪክስ

| Artefact | መግለጫ | ቅርጸት | ማከማቻ |
|-------|------------|-------|-----|
| ግልጽነት ማጠቃለያ | ሰው ሊነበብ የሚችል ሪፖርት ከአስፈፃሚ ማጠቃለያ፣ ድምቀቶች፣ ከአደጋ እቃዎች ጋር | Markdown → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
| የውሂብ አባሪ | ቀኖናዊ Norito ጥቅል ከጽዳት ጠረጴዛዎች ጋር (`ModerationLedgerBlockV1`፣ ይግባኞች፣ ጥቁር መዝገብ ዴልታስ) | `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data` (ወደ SoraFS CID መስታወት) |
| መለኪያዎች CSV | ለዳሽቦርዶች አመቺ ሲኤስቪ ወደ ውጭ መላክ (AI FP/FN፣ ይግባኝ SLA፣ የከዳተኛ ሹርን) | `.csv` | ተመሳሳይ ማውጫ፣ የተፈረመ እና የተፈረመ |
| ዳሽቦርድ ቅጽበተ ፎቶ | Grafana JSON የ `ministry_transparency_overview` ፓነሎች ወደ ውጭ መላክ + የማንቂያ ደንቦች | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
| Provenance አንጸባራቂ | Norito አንጸባራቂ ማሰር የምግብ መፍጫ አካላት፣ SoraFS CID፣ ፊርማዎች፣ የመልቀቅ ጊዜ ማህተም | `.json` + የተነጠለ ፊርማ | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)` (በተጨማሪም ከአስተዳደር ድምጽ ጋር ተያይዟል) |

## የመረጃ ምንጮች እና የቧንቧ መስመር

| ምንጭ | ምግብ | ማስታወሻ |
|--------|-------|------|
| የአወያይ መዝገብ (`docs/source/sorafs_transparency_plan.md`) | በየሰዓቱ `ModerationLedgerBlockV1` ወደ ውጭ የሚላከው በ CAR ፋይሎች ውስጥ | ቀድሞውኑ ለ SFM-4c መኖር; ለሩብ ወሩ እንደገና ጥቅም ላይ የዋለ. |
| AI ልኬት + የውሸት አዎንታዊ ተመኖች | `docs/source/sorafs_ai_moderation_plan.md` ቋሚዎች + የመለኪያ ማሳያዎች (`docs/examples/ai_moderation_calibration_*.json`) | መለኪያዎች በፖሊሲ፣ በክልል እና በሞዴል መገለጫ የተዋሃዱ። |
| ይግባኝ መዝገብ | Norito `AppealCaseV1` በMINFO-7 የግምጃ ቤት መሳሪያዎች የወጡ ክስተቶች | የአክሲዮን ማስተላለፎችን፣ የፓነል ዝርዝር ዝርዝርን፣ SLA ጊዜ ቆጣሪዎችን ይዟል። |
| Denylist churn | `MinistryDenylistChangeV1` ክስተቶች ከመርክሌ መዝገብ ቤት (MINFO-6) | ሃሽ ቤተሰቦችን፣ ቲቲኤልን፣ የአደጋ ጊዜ ቀኖና ባንዲራዎችን ያካትታል። |
| የግምጃ ቤት ፍሰት | `MinistryTreasuryTransferV1` ክስተቶች (ይግባኝ ተቀማጭ, የፓነል ሽልማቶች) | ከ `finance/mminfo_gl.csv` ጋር የተመጣጠነ። |የአደጋ ጊዜ ቀኖና አስተዳደር፣ የቲቲኤል ገደቦች እና የግምገማ መስፈርቶች አሁን ይኖራሉ
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md)፣ በማረጋገጥ
የችርቻው ሜትሪክስ ደረጃውን (`standard`፣ `emergency`፣ `permanent`)፣ ቀኖና መታወቂያ፣
እና Torii በጭነት ጊዜ የሚያስፈጽመውን የመጨረሻ ጊዜ ይገምግሙ።

የማስኬጃ ደረጃዎች፡-
1. ** Ingest ** ጥሬ ክስተቶች ወደ `ministry_transparency_ingest` (የግልጽነት ደብተር ኢንጌስተር የሚያንፀባርቅ የዝገት አገልግሎት)። በሌሊት ይሮጣል ፣ ኃይለኛ።
2. ** ድምር ** በአንድ ሩብ ከ `ministry_transparency_builder` ጋር። ከግላዊነት ማጣሪያዎች በፊት የNorito ውሂብ አባሪ እና በየሜትሪክ ሰንጠረዦችን ያወጣል።
3. **ንጽህና አድርግ ** መለኪያዎችን በ`cargo xtask ministry-transparency sanitize` (ወይም `scripts/ministry/dp_sanitizer.py`) እና የCSV/JSON ቁርጥራጭን በሜታዳታ ልቀቅ።
4. **ፓኬጅ** ቅርሶች፣ በ`ministry_release_signer` ይፈርሙ እና ወደ SoraFS + አስተዳደር DAG ይስቀሉ።

## 2026-Q3 የማጣቀሻ መለቀቅ

- የመክፈቻው አስተዳደር-የተከለለ ጥቅል (2026-Q3) የተመረተው በ2026-10-07 በ`make check-ministry-transparency` ነው። ቅርሶች በ`artifacts/ministry/transparency/2026-Q3/` ውስጥ ይኖራሉ—`sanitized_metrics.json`፣ `dp_report.json`፣ `summary.md`፣ `summary.md`፣ `checksums.sha256`፣ `transparency_manifest.json`፣ እና `transparency_manifest.json`፣ እና I1800000060X፣ እና 010X SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`.
- የሕትመት ዝርዝሮች፣ የመለኪያ ሠንጠረዦች እና ማጽደቆች በ`docs/source/ministry/reports/2026-Q3.md` ውስጥ ተይዘዋል፣ይህም አሁን የQ3 መስኮትን ለሚመለከቱ ኦዲተሮች ቀኖናዊ ማጣቀሻ ሆኖ ያገለግላል።
- CI ከመልቀቃቸው በፊት `ci/check_ministry_transparency.sh`/`make check-ministry-transparency`ን ይተገብራል፣የእርጥፈት ውህዶችን፣Grafana/ማንቂያ ሃሽ እና ሜታዳታን ያሳያል ስለዚህ እያንዳንዱ የወደፊት ሩብ ተመሳሳይ የማስረጃ መንገድ ይከተላል።

## መለኪያዎች እና ዳሽቦርዶች

የGrafana ዳሽቦርድ (`dashboards/grafana/ministry_transparency_overview.json`) የሚከተሉትን ፓነሎች ያጋልጣል፡

- የ AI ልከኝነት ትክክለኛነት፡ በአንድ ሞዴል FP/FN ተመን፣ ተንሸራታች vs የካሊብሬሽን ዒላማ፣ እና የማንቂያ ጣራዎች ከ`docs/source/sorafs/reports/ai_moderation_calibration_*.md` ጋር የተሳሰሩ።
- ይግባኝ የሕይወት ዑደት: ግቤቶች, SLA ተገዢነት, ተገላቢጦሽ, ማስያዣ ይቃጠላል, በየደረጃው መዝገብ.
- ውድቅ ማድረግ፡ መጨመር/ማስወገድ በአንድ ሃሽ ቤተሰብ፣ የቲቲኤል ማብቂያ ጊዜ፣ የአደጋ ጊዜ ቀኖና ጥሪዎች።
- የበጎ ፈቃደኞች አጭር መግለጫዎች እና የፓናል ልዩነት፡ በየቋንቋው የሚቀርቡ ግጭቶች፣ የፍላጎት ግጭት መግለጫዎች፣ የህትመት መዘግየት። ሚዛናዊ አጫጭር መስኮች በ`docs/source/ministry/volunteer_brief_template.md` ውስጥ ተገልጸዋል፣የእውነታ ሠንጠረዦች እና የአወያይ መለያዎች ማሽን ሊነበቡ የሚችሉ መሆናቸውን ያረጋግጣል።
- የግምጃ ቤት ሒሳቦች፡ ተቀማጮች፣ ክፍያዎች፣ የላቀ ተጠያቂነት (ምግቦች MINFO-7)።

የማንቂያ ደንቦች (በ`dashboards/alerts/ministry_transparency_rules.yml` ውስጥ የተስተካከለ) ሽፋን፡-
- የኤፍፒ/ኤፍኤን ልዩነት>25% ከመለኪያ መነሻ መስመር ጋር ሲነጻጸር።
- ይግባኝ SLA ማጣት መጠን> 5% ሩብ.
- ከፖሊሲ በላይ የቆዩ የአደጋ ጊዜ ቀኖናዎች።
- የህትመት መዘግየት> 14 ቀናት ሩብ ከተዘጋ በኋላ።

## የግላዊነት እና የልቀት መከላከያ መንገዶች (MINFO-8a)| መለኪያ ክፍል | መካኒዝም | መለኪያዎች | ተጨማሪ ጠባቂዎች |
|-------------|-----------|-------------|
| ቆጠራዎች (ይግባኝ፣ የተከለከሉ ዝርዝር ለውጦች፣ የበጎ ፈቃደኞች አጭር መግለጫ) | የላፕላስ ጫጫታ | ε = 0.75 በሩብ, δ = 1e-6 | የድህረ-ጫጫታ እሴት ያላቸውን ባልዲዎች ማፈን <5; ቅንጥብ አስተዋጽዖ ለ1 ተዋናይ በአንድ ሩብ። |
| AI ትክክለኛነት | Gaussian ጫጫታ በቁጥር / መለያ ላይ | ε=0.5, δ=1e-6 | የጸዳ ናሙና ሲቆጠር ≥50 (የ`min_accuracy_samples` ወለል) ብቻ ይልቀቁ እና የመተማመን ክፍተቱን ያትሙ። |
| የግምጃ ቤት ፍሰት | ምንም ድምፅ የለም (ቀድሞውንም የህዝብ በሰንሰለት ላይ) | - | ከግምጃ ቤት መታወቂያዎች በስተቀር ጭምብል መለያ ስሞች; Merkle ማስረጃዎችን ያካትቱ. |

የመልቀቂያ መስፈርቶች፡-
- የልዩነት የግላዊነት ሪፖርቶች የኤፒሲሎን/ዴልታ መዝገብ እና የ RNG ዘር ቁርጠኝነት (`blake3(seed)`) ያካትታሉ።
- ሚስጥራዊነት ያላቸው ምሳሌዎች (የማስረጃ ሃሽ) በይፋዊ Merkle ደረሰኞች ካልሆነ በስተቀር ተቀይሯል።
- ሁሉንም የተወገዱ መስኮችን እና ማረጋገጫዎችን በሚገልጽ ማጠቃለያ ላይ የማሻሻያ መዝገብ ተያይዟል።

## የስራ ፍሰት እና የጊዜ መስመር ማተም

| T-መስኮት | ተግባር | ባለቤት(ዎች) | ማስረጃ |
-------------
| T+3d ከሩብ በኋላ | ወደ ውጭ የሚላኩ ጥሬ ዕቃዎችን ማገድ፣ የመደመር ሥራን ቀስቅሷል | ሚኒስቴር ታዛቢነት TL | `ministry_transparency_ingest.log`, የቧንቧ ሥራ መታወቂያ |
| T+7d | ጥሬ መለኪያዎችን ይገምግሙ፣ የዲፒ ሳኒታይዘር ደረቅ ሩጫን ያካሂዱ | የውሂብ እምነት ቡድን | የሳኒታይዘር ዘገባ (`artifacts/.../dp_report.json`) |
| T+10d | ረቂቅ ማጠቃለያ + የውሂብ አባሪ | ሰነዶች/DevRel + የፖሊሲ ተንታኝ | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T+12d | ቅርሶችን ይፈርሙ፣ ማኒፌክት ያዘጋጁ፣ ወደ SoraFS ይስቀሉ | ኦፕስ / የአስተዳደር ሴክሬታሪያት | `manifest.json(.sig)`፣ SoraFS CID |
| T+14d | ዳሽቦርዶችን + ማንቂያዎችን ያትሙ፣ የአስተዳደር ማስታወቂያ | ታዛቢነት + Comms | Grafana ወደ ውጭ መላክ፣ የማስጠንቀቂያ ደንብ ሃሽ፣ የአስተዳደር ድምጽ ማገናኛ |

እያንዳንዱ ልቀት በዚህ መጽደቅ አለበት፡-
1. ሚኒስቴር ታዛቢነት TL (የውሂብ ታማኝነት)
2. የአስተዳደር ምክር ቤት ግንኙነት (ፖሊሲ)
3. ሰነዶች/Comms መሪ (የወል ቃል)

## አውቶሜሽን እና ማስረጃ ማከማቻ- የሩብ ዓመቱን ቅጽበተ-ፎቶ ከጥሬ መኖዎች (መመዝገቢያዎች፣ ይግባኞች፣ ውድቅ መዝገብ፣ ግምጃ ቤት፣ በጎ ፈቃደኞች) ለመገንባት `cargo xtask ministry-transparency ingest` ይጠቀሙ። ከማተምዎ በፊት የዳሽቦርድ መለኪያዎችን JSON እና የተፈረመውን አንጸባራቂ ለመልቀቅ በ`cargo xtask ministry-transparency build` ይከታተሉ።
- የቀይ ቡድን ትስስር፡ አንድ ወይም ከዚያ በላይ የ`--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` ፋይሎችን ወደ ማስገቢያ ደረጃ ያስተላልፉ ስለዚህ የግልጽነት ቅጽበታዊ ገጽ እይታ እና የጸዳ ሜትሪክስ የመሰርሰሪያ መታወቂያዎችን፣ የትዕይንት ክፍሎችን፣ የማስረጃ ጥቅል መንገዶችን እና ዳሽቦርድ SHAዎችን ከመመዝገቢያ/ይግባኝ/ ውድቅ መዝገብ ጋር። ይህ MINFO-9 መሰርሰሪያ ቁፋሮ በእያንዳንዱ የግልጽነት ፓኬት ውስጥ ያለ በእጅ አርትዖቶች እንዲንጸባረቅ ያደርገዋል።
- የበጎ ፈቃደኞች ማስረከቦች `docs/source/ministry/volunteer_brief_template.md` መከተል አለባቸው (ለምሳሌ `docs/examples/ministry/volunteer_brief_template.json`)። የማስገቢያ እርምጃው የእነዚያን ነገሮች የJSON ድርድር ይጠብቃል፣ የ`moderation.off_topic` ግቤቶችን በራስ ሰር ያጣራል፣ የገለጻ ማረጋገጫዎችን ያስፈጽማል እና የእውነታ-ጠረጴዛ ሽፋንን ይመዘግባል በዚህም ዳሽቦርዶች የጎደሉ ጥቅሶችን እንዲያጎላ።
- ተጨማሪ አውቶማቲክ በ `scripts/ministry/` ስር ይኖራል። `dp_sanitizer.py` wraps the `cargo xtask ministry-transparency sanitize` command, while `transparency_release.py` (added alongside the provenance tooling) now packages artefacts, derives the SoraFS CID from the `sorafs_cli car pack|proof verify` summary (or an explicit `--sorafs-cid`)፣ እና ሁለቱንም `transparency_manifest.json` እና `transparency_release_action.json` ይጽፋል (የ `TransparencyReleaseV1` የአስተዳደር ክፍያ ጭነት አንጸባራቂ መፈጨትን፣ SoraFS CID እና dashboards git SHA)። `--governance-dir <path>` ወደ `transparency_release.py` (ወይም `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>` ን ያሂዱ) የNorito ክፍያን ኮድ ለማድረግ እና ከማተምዎ በፊት (የJSON ማጠቃለያውን ጨምሮ) ወደ አስተዳደር DAG ማውጫ ውስጥ ያስገቡት። ተመሳሳዩ ባንዲራ በ `<governance-dir>/publisher/head_requests/ministry_transparency/` ስር የ`<governance-dir>/publisher/head_requests/ministry_transparency/` ጥያቄን ያወጣል፣ ሩብ አመቱን፣ SoraFS CIDን፣ የገለፃ መንገዶችን እና የIPNS ቁልፍ ተለዋጭ ስም (በ`--ipns-key`) በመጥቀስ። `--auto-head-update` ጥያቄውን ወዲያውኑ በ`publisher_head_updater.py` በኩል ያቅርቡ፣ እንደ አማራጭ `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'` በማለፍ IPNS በተመሳሳይ ጊዜ መታተም አለበት። ያለበለዚያ `scripts/ministry/publisher_head_updater.py --governance-dir <path>`ን በኋላ ያሂዱ (ከተፈለገ ከተመሳሳዩ አብነት ጋር) ወረፋውን ለማፍሰስ ፣ `publisher/head_updates.log` ን ይጨምሩ ፣ `publisher/ipns_heads/<key>.json`ን ያዘምኑ እና የተቀነባበረውን JSON በ `head_requests/ministry_transparency/processed/` ውስጥ ያስቀምጡ።
- በ `artifacts/ministry/transparency/<YYYY-Q>/` ስር የተከማቹ ቅርሶች በመልቀቂያ ቁልፉ የተፈረመ የ`checksums.sha256` ፋይል። ዛፉ አሁን የማመሳከሪያ ጥቅል በ`artifacts/ministry/transparency/2026-Q3/` (የጸዳ ሜትሪክስ፣ DP ሪፖርት፣ ማጠቃለያ፣ መግለጫ፣ የአስተዳደር እርምጃ) በመያዝ መሐንዲሶች የመሳሪያውን አሰራር ከመስመር ውጭ መሞከር እንዲችሉ፣ እና `scripts/ministry/check_transparency_release.py` ዲጀስት/ሩብ ሜታዳታ በአገር ውስጥ ሲያረጋግጥ `ci/check_ministry_transparency.sh` ከመስቀሉ በፊት ማስረጃው ትክክለኛ ነው አረጋጋጩ አሁን የተመዘገቡትን የዲፒ በጀቶች (ε≤0.75 ለቁጥራጮች፣ ε≤0.5 ለትክክለኛነት፣ δ≤1e−6) እና `min_accuracy_samples` ወይም የጭቆና ጣራ በሚንሳፈፍበት ጊዜ፣ ወይም ባልዲው ከወለሎቹ በታች እሴት ሲፈስ ይወድቃል። ስክሪፕቱን በፍኖተ ካርታው (MINFO-8) እና CI መካከል ያለው ውል አድርገው ይያዙት፡ የግላዊነት መመዘኛዎች ሁሌም ከተቀየሩ ሁለቱንም ከላይ ያለውን ሰንጠረዥ እና አረጋጋጩን ያስተካክሉ።- የአስተዳደር መልህቅ፡ የ`TransparencyReleaseV1` ድርጊት አንጸባራቂ መፍጨትን፣ SoraFS CID እና ዳሽቦርድ git SHA ይፍጠሩ (`iroha_data_model::ministry::TransparencyReleaseV1` ቀኖናዊ ክፍያን ይገልጻል)።

## ተግባሮችን እና ቀጣይ እርምጃዎችን ይክፈቱ

| ተግባር | ሁኔታ | ማስታወሻ |
|-------|--------|------|
| `ministry_transparency_ingest` + ግንበኛ ስራዎችን ተግብር | 🈺 በሂደት ላይ | `cargo xtask ministry-transparency ingest|build` አሁን የስፌት ደብተር/ይግባኝ/ መካድ/የግምጃ ቤት ምግቦች; ቀሪው የስራ ሽቦዎች የዲፒ ሳኒታይዘር + የመልቀቂያ ስክሪፕት ቧንቧ መስመር። |
| Grafana ዳሽቦርድ + ማንቂያ ጥቅል ያትሙ | 🈴 ተጠናቀቀ | ዳሽቦርድ + የማንቂያ ፋይሎች በ `dashboards/grafana/ministry_transparency_overview.json` እና `dashboards/alerts/ministry_transparency_rules.yml`; በታቀደው ጊዜ ወደ PagerDuty `ministry-transparency` ያገናኙዋቸው። |
| የዲፒ ሳኒታይዘር + የፕሮቬንሽን አንጸባራቂ አውቶማቲክ | 🈴 ተጠናቀቀ | `cargo xtask ministry-transparency sanitize` (መጠቅለያ፡ `scripts/ministry/dp_sanitizer.py`) የጸዳ ሜትሪክስ + DP ሪፖርት ያወጣል፣ እና `scripts/ministry/transparency_release.py` አሁን `checksums.sha256` እና `transparency_manifest.json` ለፕሮቨንስ ይጽፋል። |
| የሩብ ዓመት ሪፖርት አብነት ይፍጠሩ (`reports/<YYYY-Q>.md`) | 🈴 ተጠናቀቀ | አብነት `docs/source/ministry/reports/2026-Q3-template.md` ላይ ታክሏል; በየሩብ ዓመቱ ይቅዱ/ይሰይሙ እና ከማተምዎ በፊት `{{...}}` ቶከኖችን ይተኩ። |
| ሽቦ አስተዳደር DAG መልህቅ | 🈴 ተጠናቀቀ | `TransparencyReleaseV1` የሚኖረው በ`iroha_data_model::ministry` ነው፣ `scripts/ministry/transparency_release.py` የJSON ክፍያን ያስለቅቃል፣ እና `cargo xtask ministry-transparency anchor` የ `cargo xtask ministry-transparency anchor` የ `.to` አርቴፋክትን ወደ የተዋቀረው አስተዳደር ማተም DAG ማውጫን በራስ ሰር መልቀቅ ይችላል። |

ሰነዱን፣ ዳሽቦርድ ዝርዝርን እና የስራ ፍሰትን ማድረስ MINFO-8ን ከ🈳 ወደ 🈺 ያንቀሳቅሳል። ቀሪ የምህንድስና ስራዎች (ስራዎች፣ ስክሪፕቶች፣ ማንቂያ ሽቦዎች) ከላይ ባለው ሠንጠረዥ ውስጥ ክትትል ይደረግባቸዋል እና ከመጀመሪያው Q32026 ህትመት በፊት መዘጋት አለባቸው።