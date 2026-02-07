---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6066e49abf73a788490f9d6a738d90376b13dde4150509e81431508ffaaaf916
source_last_modified: "2025-12-29T18:16:35.199423+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
---

# AI አወያይነት ሪፖርት - የካቲት 2026

ይህ ሪፖርት ለ **MINFO-1** የመጀመሪያ ደረጃ ማስተካከያ ቅርሶችን ያጠቃልላል። የ
የውሂብ ስብስብ፣ መግለጫ እና የውጤት ሰሌዳ በ2026-02-05 ተዘጋጅቷል፣ በ የተገመገመ
የሚኒስቴር ምክር ቤት በ2026-02-10፣ እና በአስተዳደር DAG በከፍተኛ ደረጃ ላይ ተቀምጧል
`912044`.

## የውሂብ ስብስብ መግለጫ

- ** የውሂብ ስብስብ ማጣቀሻ: ** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- ** ስሉግ: *** `ai-moderation-calibration-202602`
- ** ግቤቶች፡** አንጸባራቂ 480፣ ቸንክ 12,800፣ ሜታዳታ 920፣ ኦዲዮ 160
- ** የመለያ ድብልቅ: *** ደህንነቱ የተጠበቀ 68% ፣ ተጠርጣሪ 19% ፣ 13% ይጨምራል
- ** Artefact መፍጨት: ** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- ** ስርጭት: ** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

ሙሉ አንጸባራቂው በI18NI0000008X ውስጥ ይኖራል
እና በሚለቀቅበት ጊዜ የተያዘውን የአስተዳደር ፊርማ እና ሯጭ ሃሽ ይዟል
ጊዜ.

## የውጤት ሰሌዳ ማጠቃለያ

ካሊብሬሽኖች በኦፕሴት 17 እና በወሳኙ የዘር ቧንቧ መስመር ሄዱ። የ
የተሟላ የውጤት ሰሌዳ JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
የ hashes እና የቴሌሜትሪ መፍጫዎችን ይመዘግባል; ከታች ያለው ሰንጠረዥ በጣም ጎላ አድርጎ ያሳያል
አስፈላጊ መለኪያዎች.

| ሞዴል (ቤተሰብ) | ብሪየር | ECE | አውሮክ | ትክክለኛነት @ ኳራንቲን | አስታውስ@Escalate |
| ------------- | --- | --- | --- | ------------------- | ------------ |
| ViT-H/14 ደህንነት (ራዕይ) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B ደህንነት (መልቲሞዳል) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| የማስተዋል ስብስብ (የማስተዋል) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

የተዋሃዱ መለኪያዎች: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. ፍርዱ
በመለኪያ መስኮቱ ላይ ያለው ስርጭት 91.2%፣ ማቆያ 6.8%፣
በአንጸባራቂው ውስጥ ከተመዘገቡት የመመሪያ ፍላጎቶች ጋር በማዛመድ 2.0% ይጨምራል
ማጠቃለያ የውሸት አወንታዊ መዝገብ ዜሮ ላይ ቀርቷል፣ እና የተንሸራታች ውጤቱ (7.1%)
ከ20% የማንቂያ ገደብ በታች በደንብ ወድቋል።

## ገደቦች እና ዘግተው መውጣት

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- የአስተዳደር እንቅስቃሴ: `MINFO-2026-02-07`
- በ `ministry-council-seat-03` የተፈረመ በ I18NI0000017X

CI የተፈረመውን ጥቅል በ`artifacts/ministry/ai_moderation/2026-02/` ውስጥ አከማችቷል።
ከመካከለኛው ሯጭ ሁለትዮሽ ጎን ለጎን። አንጸባራቂው መፈጨት እና የውጤት ሰሌዳ
ኦዲት በሚደረግበት ጊዜ እና ይግባኝ በሚቀርብበት ጊዜ ከላይ ያሉት ሃሽቶች መጠቀስ አለባቸው።

## ዳሽቦርዶች እና ማንቂያዎች

አወያይ SREዎች Grafana ዳሽቦርድን በ ላይ ማስመጣት አለባቸው
`dashboards/grafana/ministry_moderation_overview.json` እና ተዛማጅ
Prometheus የማንቂያ ደንቦች በI18NI0000020X
(የሙከራ ሽፋን በI18NI0000021X ስር ይኖራል)።
እነዚህ ቅርሶች ድንኳኖችን ወደ ውስጥ ለማስገባት ማንቂያዎችን ያሰራጫሉ፣ የሚንሸራተቱ ሹልፎች እና የኳራንቲን
የወረፋ እድገት፣ የተጠሩትን የክትትል መስፈርቶች ማሟላት
የ [AI Moderation Runner Specification](I18NU0000002X)።