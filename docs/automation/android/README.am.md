---
lang: am
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ ዶክመንቴሽን አውቶሜሽን ቤዝላይን (AND5)

የRoadmap ንጥል AND5 ሰነድ፣ አካባቢ ማድረግ እና ማተም ያስፈልገዋል
አውቶሜሽን AND6 (CI & Compliance) ከመጀመሩ በፊት ኦዲት ይደረጋል። ይህ አቃፊ
AND5/AND6 የሚያመለክተውን ትዕዛዞችን፣ ቅርሶችን እና የማስረጃ አቀማመጥን ይመዘግባል፣
የተያዙትን እቅዶች በማንፀባረቅ
`docs/source/sdk/android/developer_experience_plan.md` እና
`docs/source/sdk/android/parity_dashboard_plan.md`.

## የቧንቧ መስመሮች እና ትዕዛዞች

| ተግባር | ትዕዛዝ(ዎች) | የሚጠበቁ ቅርሶች | ማስታወሻ |
|-------------|--------|------|
| ለትርጉም ስቱብ ማመሳሰል | `python3 scripts/sync_docs_i18n.py` (በአማራጭ I18NI0000006X በሩጫ ማለፍ) | በ I18NI0000007X ስር የተከማቸ የምዝግብ ማስታወሻ እና የተተረጎመው stub ፈጸመ | `docs/i18n/manifest.json` ከተተረጎሙ ማሰሪያዎች ጋር በማመሳሰል ያቆያል; የምዝግብ ማስታወሻው የቋንቋ ኮዶችን በመንካት እና በመነሻ መስመር ላይ የተቀረፀውን የጊት ቃል ይመዘግባል። |
| Norito ቋሚ + ተመሳሳይነት ማረጋገጫ | `ci/check_android_fixtures.sh` (ጥቅል `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | የተፈጠረውን ማጠቃለያ JSON ወደ I18NI0000011X | `java/iroha_android/src/test/resources` የሚጫኑ ጭነቶችን፣ የሰነድ መግለጫዎችን እና የተፈረመ የቋሚ ርዝመቶችን ያረጋግጣል። ማጠቃለያውን በ`artifacts/android/fixture_runs/` ስር ካለው የድጋፍ ማስረጃ ጋር አያይዘው። |
| የናሙና አንጸባራቂ እና የህትመት ማረጋገጫ | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (ሙከራዎችን + SBOM + provenance ያካሂዳል) | የፕሮቨንስ ጥቅል ሜታዳታ እና የተገኘው `sample_manifest.json` ከ I18NI0000016X በ`docs/automation/android/samples/<version>/` ውስጥ ተከማችቷል | የ AND5 ናሙና መተግበሪያዎችን ያገናኛል እና አውቶማቲክን አንድ ላይ ይልቀቁ - የመነጨውን አንጸባራቂ፣ SBOM hash እና የፕሮቨንስ ሎግ ለቅድመ-ይሁንታ ግምገማ ይያዙ። |
| ፓሪቲ ዳሽቦርድ ምግብ | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` ተከትሎ I18NI0000019X | የ `metrics.prom` ቅጽበታዊ ገጽ እይታን ወይም Grafana JSON ወደ I18NI0000021X ወደ ውጪ መላክ | የ AND5/AND7 አስተዳደር ልክ ያልሆኑ የማስረከቢያ ቆጣሪዎችን እና የቴሌሜትሪ ጉዲፈቻን ማረጋገጥ እንዲችል የዳሽቦርዱን እቅድ ይመገባል። |

## ማስረጃ ቀረጻ

1. **የጊዜ ማህተም ሁሉንም ነገር።** የዩቲሲ ጊዜ ማህተሞችን በመጠቀም ፋይሎችን ይሰይሙ
   (`YYYYMMDDTHHMMSSZ`) ስለዚህ እኩልነት ዳሽቦርዶች፣ የአስተዳደር ደቂቃዎች እና ታትመዋል
   ሰነዶች ተመሳሳይ ሩጫን በቆራጥነት ሊጠቅሱ ይችላሉ።
2. **ማጣቀሻ ይፈጸማል።** እያንዳንዱ ሎግ የሩጫውን ጊት መፈጸም ሃሽ ማካተት አለበት።
   እንዲሁም ማንኛውም ተዛማጅ ውቅር (ለምሳሌ፣ `ANDROID_PARITY_PIPELINE_METADATA`)።
   ግላዊነት ማሻሻያ ሲፈልግ፣ ማስታወሻ ያካትቱ እና ወደ ደህንነቱ ካዝና የሚወስድ አገናኝ።
3. ** ትንሹን አውድ በማህደር አስቀምጥ።** የምንፈትሽው የተዋቀሩ ማጠቃለያዎችን ብቻ ነው (JSON፣
   `.prom`፣ `.log`)። ከባድ ቅርሶች (ኤፒኬ ቅርቅቦች፣ ቅጽበታዊ ገጽ እይታዎች) ውስጥ መቆየት አለባቸው
   `artifacts/` ወይም የነገር ማከማቻ በምዝግብ ማስታወሻው ውስጥ ከተመዘገበ ሃሽ ጋር።
4. **የሁኔታ ግቤቶችን ያዘምኑ።** AND5 ችካሎች በ`status.md` ሲገፉ፣ ይጥቀሱ
   ተዛማጅ ፋይል (ለምሳሌ፣ `docs/automation/android/parity/20260324T010203Z-summary.json`)
   ስለዚህ ኦዲተሮች የ CI ምዝግብ ማስታወሻዎችን ሳይቆርጡ የመነሻ መስመርን መከታተል ይችላሉ።

ይህንን አቀማመጥ መከተል ለ "ዶክመንቶች/አውቶሜትድ መሰረታዊ መስመሮችን ያሟላል።
ኦዲት” ቅድመ ሁኔታ AND6 ጠቅሶ የአንድሮይድ ዶክመንቴሽን ፕሮግራም ያስቀመጠ
ከታተሙት እቅዶች ጋር በመቆለፊያ ውስጥ.