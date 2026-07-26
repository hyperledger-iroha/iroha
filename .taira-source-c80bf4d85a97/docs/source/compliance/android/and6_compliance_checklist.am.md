---
lang: am
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ AND6 ተገዢነት ማረጋገጫ ዝርዝር

ይህ የማረጋገጫ ዝርዝር የወሳኝ ኩነት ጊዜውን የሚያሟላ የታዛዥነት አቅርቦትን ይከታተላል **AND6 -
CI & Compliance Hardening ***። የተጠየቁትን የቁጥጥር ቅርሶች ያጠናክራል።
በ `roadmap.md` እና የማከማቻውን አቀማመጥ በስር ይገልፃል
`docs/source/compliance/android/` ስለዚህ ምህንድስና፣ ድጋፍ እና ህጋዊ ልቀቅ
የአንድሮይድ ልቀቶችን ከማጽደቁ በፊት የተቀመጡትን ተመሳሳይ ማስረጃዎች መጥቀስ ይችላል።

## ስፋት እና ባለቤቶች

| አካባቢ | የሚላኩ | ዋና ባለቤት | ምትኬ / ገምጋሚ ​​|
|------|-------------|-----------|--------|
| የአውሮፓ ህብረት የቁጥጥር ጥቅል | ETSI EN 319 401 የደህንነት ኢላማ፣ GDPR DPIA ማጠቃለያ፣ የኤስቢኦኤም ማረጋገጫ፣ የማስረጃ መዝገብ | ተገዢነት እና ህጋዊ (ሶፊያ Martins) | መልቀቂያ ምህንድስና (አሌክሲ ሞሮዞቭ) |
| የጃፓን የቁጥጥር ጥቅል | የ FISC ደህንነት ቁጥጥር ዝርዝር፣ የሁለት ቋንቋ ተናጋሪ StrongBox ማረጋገጫ ቅርቅቦች፣ የማስረጃ መዝገብ | ተገዢነት እና ህጋዊ (ዳንኤል ፓርክ) | አንድሮይድ ፕሮግራም መሪ |
| የመሣሪያ ቤተ ሙከራ ዝግጁነት | የአቅም መከታተያ፣ የአደጋ ጊዜ ቀስቅሴዎች፣ የከፍታ መዝገብ | የሃርድዌር ላብ አመራር | አንድሮይድ ታዛቢነት TL |

## Artefact ማትሪክስ| አርቲፊሻል | መግለጫ | የማጠራቀሚያ መንገድ | አድስ Cadence | ማስታወሻ |
|-------|-------------|---------|
| ETSI EN 319 401 የደህንነት ኢላማ | ለአንድሮይድ ኤስዲኬ ሁለትዮሽ የደህንነት አላማዎችን/ግምቶችን የሚገልጽ ትረካ። | `docs/source/compliance/android/eu/security_target.md` | እያንዳንዱን የGA + LTS ልቀትን እንደገና ያረጋግጡ። | ለሚለቀቀው ባቡር የግንባታ ፕሮቬንሽን ሃሽ መጥቀስ አለበት። |
| GDPR DPIA ማጠቃለያ | የቴሌሜትሪ/የሎግ ግኝቶችን የሚሸፍን የመረጃ ጥበቃ ተጽዕኖ ግምገማ። | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | ዓመታዊ + ከቁስ ቴሌሜትሪ ለውጦች በፊት። | የማጣቀሻ ማሻሻያ ፖሊሲ በ`sdk/android/telemetry_redaction.md`። |
| SBOM ማረጋገጫ | ለግራድል/ማቨን ቅርሶች የተፈረመ SBOM እና SLSA provenance። | `docs/source/compliance/android/eu/sbom_attestation.md` | እያንዳንዱ GA መለቀቅ. | CycloneDX ሪፖርቶችን ለማመንጨት `scripts/android_sbom_provenance.sh <version>` ን ያሂዱ፣ ጥቅሎችን እና ቼኮችን ይፍጠሩ። |
| FISC የደህንነት ቁጥጥር ዝርዝር | የተጠናቀቀ የማረጋገጫ ዝርዝር የኤስዲኬ ቁጥጥሮች ወደ FISC መስፈርቶች። | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | ዓመታዊ + ከጄፒ አጋር አብራሪዎች በፊት። | የሁለት ቋንቋ ርዕሶችን ያቅርቡ (EN/JP)። |
| StrongBox የማረጋገጫ ጥቅል (ጄፒ) | በመሣሪያ የማረጋገጫ ማጠቃለያ + ሰንሰለት ለJP ተቆጣጣሪዎች። | `docs/source/compliance/android/jp/strongbox_attestation.md` | አዲስ ሃርድዌር ወደ ገንዳው ሲገባ. | በ`artifacts/android/attestation/<device>/` ስር ወደ ጥሬ እቃዎች ያመልክቱ። |
| ህጋዊ የመግቢያ ማስታወሻ | የETSI/GDPR/FISC ወሰን፣ የግላዊነት አቀማመጥ እና ተያያዥ ቅርሶችን የሚሸፍን የምክር ማጠቃለያ። | `docs/source/compliance/android/eu/legal_signoff_memo.md` | የቅርስ ቅርቅቡ በተቀየረ ቁጥር ወይም አዲስ ስልጣን በተጨመረ ቁጥር። | ማስታወሻ ከማስረጃ ምዝግብ ማስታወሻው ሃሾችን ይጠቅሳል እና ከመሣሪያ-ላብራቶሪ የመጠባበቂያ ጥቅል ጋር ያገናኛል። |
| የማስረጃ መዝገብ | ከሃሽ/የጊዜ ማህተም ሜታዳታ ጋር የገቡ ቅርሶች ማውጫ። | `docs/source/compliance/android/evidence_log.csv` | ማንኛውም ከላይ ግቤት ሲቀየር ይዘምናል። | የBuildkite አገናኝ + ገምጋሚ ​​ማቋረጥን ያክሉ። |
| የመሣሪያ-ላብራቶሪ መሣሪያ ጥቅል | በ`device_lab_instrumentation.md` ውስጥ ከተገለጸው ሂደት ጋር የተመዘገቡ ስሎ-ተኮር ቴሌሜትሪ፣ ወረፋ እና የማረጋገጫ ማስረጃ። | `artifacts/android/device_lab/<slot>/` (`docs/source/compliance/android/device_lab_instrumentation.md` ይመልከቱ) | እያንዳንዱ የተጠበቀ ማስገቢያ + failover መሰርሰሪያ. | SHA-256 አንጸባራቂዎችን ይቅረጹ እና የመግቢያ መታወቂያውን በማስረጃ መዝገብ + የማረጋገጫ ዝርዝር ውስጥ ያመልክቱ። |
| የመሣሪያ-ላብራቶሪ ማስያዣ መዝገብ | የስትሮንግቦክስ ገንዳዎችን በበረዶ ጊዜ ≥80% ለማቆየት ስራ ላይ የሚውሉትን የስራ ፍሰት፣ ማፅደቆች፣ የአቅም ቅጽበታዊ እይታዎች እና የማሳደግ መሰላል። | `docs/source/compliance/android/device_lab_reservation.md` | የተያዙ ቦታዎች ሲፈጠሩ/ሲቀየሩ ያዘምኑ። | በሂደቱ ውስጥ የተመለከቱትን የ`_android-device-lab` የቲኬት መታወቂያዎችን እና ሳምንታዊ የቀን መቁጠሪያ ወደ ውጭ መላክን ይመልከቱ። |
| የመሣሪያ-ላብራቶሪ ውድቀት runbook & ቦረቦረ ጥቅል | የሩብ ጊዜ የመለማመጃ እቅድ እና የቅርጻቅርጽ አንጸባራቂ የኋሊት መሄጃ መስመሮችን፣ የፋየር ቤዝ ፍንዳታ ወረፋ እና የውጭ StrongBox ማቆያ ዝግጁነት። | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | በየሩብ ዓመቱ (ወይም ከሃርድዌር ዝርዝር ለውጦች በኋላ)። | በማረጃ መዝገብ ውስጥ የመሰርሰሪያ መታወቂያዎችን ይመዝገቡ እና በ runbook ውስጥ የተገለጸውን የሰነድ ሃሽ + PagerDuty ወደ ውጭ መላክን አያይዙ። |

> ** ጠቃሚ ምክር:** ፒዲኤፎችን ወይም በውጭ የተፈረሙ ቅርሶችን ሲያያይዙ አጭር ያከማቹ
> ከማይለወጥ የጥበብ ስራ ጋር የሚያገናኘው በጠረጴዛው መንገድ ላይ የማርክ መጠቅለያ
> የአስተዳደር ድርሻ። ይህ ሬፖውን በሚጠብቅበት ጊዜ ክብደቱን ቀላል ያደርገዋል
> የኦዲት መንገድ።

## የአውሮፓ ህብረት የቁጥጥር ፓኬት (ETSI/GDPR)የአውሮፓ ህብረት ፓኬት ከላይ ያሉትን ሶስት ቅርሶች እና የህግ ማስታወሻውን አንድ ላይ ያገናኛል፡-

- `security_target.md`ን በመልቀቂያ መለያ ያዘምኑ፣ Torii አንጸባራቂ ሃሽ፣
  እና SBOM መፍጨት ስለዚህ ኦዲተሮች ሁለትዮሽዎችን ከተገለጸው ወሰን ጋር ማዛመድ ይችላሉ።
- የDPIA ማጠቃለያ ከቅርብ ጊዜው የቴሌሜትሪ ማሻሻያ ፖሊሲ እና ጋር እንዲጣጣም ያድርጉ
  በ`docs/source/sdk/android/telemetry_redaction.md` ውስጥ የተጠቀሰውን የNorito ልዩነት ያያይዙ።
- የ SBOM ማረጋገጫ ግቤት የሚከተሉትን ማካተት አለበት: CycloneDX JSON hash, provenance
  ጥቅል ሃሽ፣ ኮሲንግ መግለጫ እና እነሱን ያመነጨው የBuildkite የስራ ዩአርኤል።
- `legal_signoff_memo.md` አማካሪውን / ቀንን መያዝ አለበት ፣ እያንዳንዱን ቅርስ ይዘርዝሩ +
  SHA-256፣ ማናቸውንም የማካካሻ ቁጥጥሮችን ይዘረዝራል፣ እና ከማስረጃ መዝገብ ረድፍ ጋር ያገናኙ
  እንዲሁም መጽደቁን የተከታተለው የፔጀርዱቲ ቲኬት መታወቂያ።

## የጃፓን የቁጥጥር ፓኬት (FISC/StrongBox)

የጃፓን ተቆጣጣሪዎች ከሁለት ቋንቋ ተናጋሪ ሰነዶች ጋር ትይዩ የሆነ ጥቅል ይጠብቃሉ፡-

- `fisc_controls_checklist.md` ኦፊሴላዊ የተመን ሉህ ያንጸባርቃል; ሁለቱንም ሙላ
  EN እና JA አምዶች እና የ `sdk/android/security.md` የተወሰነ ክፍል ያጣቅሱ
  ወይም እያንዳንዱን ቁጥጥር የሚያረካ የ StrongBox ማረጋገጫ ጥቅል።
- `strongbox_attestation.md` የቅርብ ጊዜ ሩጫዎችን ያጠቃልላል
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (በአንድ መሣሪያ JSON + Norito ፖስታዎች)። አገናኞችን ወደ የማይለወጡ ቅርሶች ክተት
  በ `artifacts/android/attestation/<device>/` ስር እና የማሽከርከር ቃላቱን ያስተውሉ.
- ከውስጥ ከማስገባት ጋር የሚላከው የሁለት ቋንቋ የሽፋን ደብዳቤ አብነት ይመዝግቡ
  `docs/source/compliance/android/jp/README.md` ድጋፍ እንደገና ሊጠቀምበት ይችላል።
- የማረጋገጫ ምዝግብ ማስታወሻውን የማረጋገጫ ዝርዝሩን በሚጠቅስ ነጠላ ረድፍ ያዘምኑ
  የማረጋገጫ ጥቅል ሃሽ፣ እና ማንኛውም የJP አጋር ቲኬት መታወቂያ ከማድረስ ጋር የተሳሰሩ።

## የማስረከቢያ የስራ ፍሰት

1. ** ረቂቅ *** - ባለቤቱ አርቲፊኬቱን ያዘጋጃል ፣ የታቀደውን የፋይል ስም ይመዘግባል
   ከላይ ያለው ሰንጠረዥ፣ እና የዘመነውን ማርክዳውን ስቱብ እና ሀን የያዘ PR ይከፍታል።
   የውጭ ተያያዥነት ማረጋገጫ.
2. **ግምገማ *** - የተለቀቀው ምህንድስና የፕሮቬንሽን ሃሽ ከተዘጋጀው ጋር እንደሚመሳሰል ያረጋግጣል
   ሁለትዮሽ; ማክበር የቁጥጥር ቋንቋን ያረጋግጣል; ድጋፍ SLAs ያረጋግጣል እና
   የቴሌሜትሪ ፖሊሲዎች በትክክል ተጠቅሰዋል።
3. ** ዘግተህ መውጣት *** - አጽዳቂዎች ስማቸውን እና ቀናቸውን ወደ `Sign-off` ሰንጠረዥ ይጨምራሉ።
   በታች። የማስረጃ ምዝግብ ማስታወሻው በPR URL እና Buildkite ሩጫ ተዘምኗል።
4. ** አትም *** - የኤስአርአይ አስተዳደር ከተፈረመ በኋላ ቅርሱን ወደ ውስጥ ያገናኙ
   `status.md` እና የአንድሮይድ ድጋፍ Playbook ማጣቀሻዎችን ያዘምኑ።

### የመግቢያ ምዝግብ ማስታወሻ

| Artefact | የተገመገመ በ | ቀን | PR / ማስረጃ |
-------------
| (በመጠባበቅ ላይ)* | - | - | - |

## የመሣሪያ ቤተ ሙከራ ቦታ ማስያዝ እና ድንገተኛ እቅድ

በፍኖተ ካርታው ላይ የተጠራውን **የመሣሪያ ላብራቶሪ አቅርቦትን** አደጋ ለመቀነስ፡-- ሳምንታዊ አቅምን በ `docs/source/compliance/android/evidence_log.csv` ይከታተሉ
  (አምድ `device_lab_capacity_pct`)። ካለ ማንቂያ መልቀቅ ምህንድስና
  ለሁለት ተከታታይ ሳምንታት ከ 70% በታች ይወርዳል.
- የተጠባባቂ StrongBox/አጠቃላይ መስመሮችን ይከተላል
  `docs/source/compliance/android/device_lab_reservation.md` ከሁሉም በፊት
  ጥያቄዎችን፣ ማፅደቆችን እና ቅርሶችን ማሰር፣ መለማመድ ወይም ተገዢነትን መጥረግ
  በ `_android-device-lab` ወረፋ ውስጥ ተይዘዋል. የተገኙትን የቲኬት መታወቂያዎች ያገናኙ
  የአቅም ቅጽበተ-ፎቶዎችን በሚመዘግቡበት ጊዜ በማስረጃ መዝገብ ውስጥ።
- ** የመመለሻ ገንዳዎች: *** መጀመሪያ ወደ የጋራ ፒክስል ገንዳ ፈነዱ; አሁንም ከጠገበ፣
  የጊዜ ሰሌዳ የFirebase Test Lab ጭስ ለCI ማረጋገጫ ይሰራል።
- **የውጭ ላብራቶሪ መያዣ:** መያዣውን ከስትሮንግቦክስ አጋር ጋር ያቆዩት።
  በማቀዝቀዣው መስኮቶች ጊዜ ሃርድዌርን እንድናስይዝ ላብራቶሪ (ቢያንስ የ7-ቀን እርሳስ)።
- ** መሻሻል: *** ሁለቱም በፔጀርዱቲ ውስጥ የ `AND6-device-lab` ክስተትን ያሳድጉ
  የመጀመሪያ ደረጃ እና የመውደቅ ገንዳዎች ከ 50 % አቅም በታች ይወድቃሉ። የሃርድዌር ላብ መሪ
  መሣሪያዎችን ዳግም ለማስቀደም ከSRE ጋር ያስተባብራል።
- ** ያልተሳካ ማስረጃዎች እሽጎች: ** እያንዳንዱን ልምምድ ስር ያከማቹ
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` ከቦታ ማስያዝ ጋር
  ጥያቄ፣ PagerDuty ወደ ውጪ መላክ፣ የሃርድዌር መግለጫ እና የመልሶ ማግኛ ግልባጭ። ማጣቀሻ
  ጥቅሉን ከ`device_lab_contingency.md` እና SHA-256 ወደ ማስረጃ ምዝግብ ማስታወሻ ያክሉ
  ስለዚህ የአደጋ ጊዜ የስራ ሂደት መከናወኑን ህጋዊ ያረጋግጣል።
- ** የሩብ ልምምዶች:** Runbookን ወደ ውስጥ ይለማመዱ
  `docs/source/compliance/android/device_lab_failover_runbook.md`፣ ያያይዙት።
  ወደ `_android-device-lab` ትኬት የተገኘ የጥቅል መንገድ + አንጸባራቂ ሃሽ፣ እና
  የመሰርሰሪያ መታወቂያውን በሁለቱም የድንገተኛ መዝገብ እና የማስረጃ መዝገብ ውስጥ ያንጸባርቁ።

የድንገተኛ እቅዱን ማግበር በ ውስጥ ይመዝግቡ
`docs/source/compliance/android/device_lab_contingency.md` (ቀንን ይጨምራል፣
ቀስቅሴ፣ ድርጊቶች እና ክትትሎች)።

## የማይንቀሳቀስ-ትንታኔ ፕሮቶታይፕ

- `make android-lint` ይጠቀልላል `ci/check_android_javac_lint.sh`፣ በማጠናቀር ላይ
  `java/iroha_android` እና የተጋሩ `java/norito_java` ምንጮች ከ ጋር
  `javac --release 21 -Xlint:all -Werror` (ከተጠቆሙት ምድቦች ጋር
- ከተጠናቀረ በኋላ ስክሪፕቱ የብአዴንን የጥገኝነት ፖሊሲ ያስፈጽማል
  `jdeps --summary`፣ ከተፈቀደው የተፈቀደ ዝርዝር ውጭ ያለ ማንኛውም ሞጁል ካልተሳካ
  (`java.base`፣ `java.net.http`፣ `jdk.httpserver`) ይታያል። ይህ ያስቀምጣል
  አንድሮይድ ገጽ ከኤስዲኬ ምክር ቤት “ምንም የተደበቁ የጄዲኬ ጥገኞች” ጋር ተስተካክሏል።
  ከ StrongBox ተገዢነት ግምገማዎች በፊት ያለው መስፈርት።
- CI አሁን ያው በር በኩል ይሰራል
  `.github/workflows/android-lint.yml`፣ የሚጠራው።
  `ci/check_android_javac_lint.sh` አንድሮይድ በሚነካው በእያንዳንዱ ግፊት/PR ላይ
  የተጋራ Norito የጃቫ ምንጮች እና ሰቀላዎች `artifacts/android/lint/jdeps-summary.txt`
  ስለዚህ የግምገማ ግምገማዎች የተፈረመ ሞጁል ዝርዝርን እንደገና ሳይሰሩ ሊጠቅሱ ይችላሉ።
  ስክሪፕት በአካባቢው.
- ጊዜያዊውን ማቆየት ሲፈልጉ `ANDROID_LINT_KEEP_WORKDIR=1` ያዘጋጁ
  የስራ ቦታ. ስክሪፕቱ አስቀድሞ የተፈጠረውን የሞዱል ማጠቃለያ ወደ ውስጥ ይቀዳል።
  `artifacts/android/lint/jdeps-summary.txt`; አዘጋጅ
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (ወይም ተመሳሳይ) ለኦዲት ተጨማሪ፣ የተሻሻለ አርቲፊክስ ሲፈልጉ።
  አንድሮይድ PRs ከማቅረቡ በፊት መሐንዲሶች አሁንም ትዕዛዙን በአገር ውስጥ ማስኬድ አለባቸው
  የጃቫ ምንጮችን የሚነኩ እና የተቀዳውን ማጠቃለያ/ሎግ ከማክበር ጋር ያያይዙ
  ግምገማዎች. ከተለቀቁት ማስታወሻዎች እንደ “አንድሮይድ ጃቫክ ሊንት + ጥገኝነት ይጠቅሱት።
  ቅኝት"

## CI ማስረጃ (ሊንት፣ ሙከራዎች፣ ማረጋገጫ)- `.github/workflows/android-and6.yml` አሁን ሁሉንም AND6 በሮች ይሰራል (javac lint +
  ጥገኝነት ቅኝት፣ የአንድሮይድ ሙከራ ስብስብ፣ የስትሮንግቦክስ ማረጋገጫ አረጋጋጭ እና
  የመሣሪያ-ላብ ማስገቢያ ማረጋገጫ) በእያንዳንዱ PR/የአንድሮይድ ገጽን በመንካት ግፋ።
- `ci/run_android_tests.sh` `ci/run_android_tests.sh` ተጠቅልሎ ያወጣል
  በ `artifacts/android/tests/test-summary.json` ላይ የሚወሰን ማጠቃለያ
  የኮንሶል ሎግ ወደ `artifacts/android/tests/test.log` በመቆየት። ሁለቱንም ያያይዙ
  የ CI ሩጫዎችን ሲያመለክቱ ወደ ተገዢነት ፓኬቶች ፋይሎች።
- `scripts/android_strongbox_attestation_ci.sh --summary-out` ያመርታል
  `artifacts/android/attestation/ci-summary.json`፣ የተጠቀለለውን በማረጋገጥ
  የማረጋገጫ ሰንሰለቶች `artifacts/android/attestation/**` ለ StrongBox እና
  TEE ገንዳዎች.
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  በ CI ውስጥ ጥቅም ላይ የዋለውን የናሙና ማስገቢያ (`slot-sample/`) ያረጋግጣል እና ሊጠቁም ይችላል
  እውነተኛ በ `artifacts/android/device_lab/<slot-id>/` ስር ይሰራል
  `--require-slot --json-out <dest>` የመሳሪያ ቅርቅቦች መከተላቸውን ለማረጋገጥ
  የሰነድ አቀማመጥ. CI የማረጋገጫ ማጠቃለያውን ይጽፋል
  `artifacts/android/device_lab/summary.json`; የናሙና ማስገቢያ ያካትታል
  ቦታ ያዥ ቴሌሜትሪ/ማስረጃ/ወረፋ/የምዝግብ ማስታወሻዎች እና የተቀዳ
  `sha256sum.txt` ሊባዛ ለሚችል ሃሽ።

## የመሣሪያ-ላብ መሣሪያ የስራ ፍሰት

እያንዳንዱ ቦታ ማስያዝ ወይም ያልተሳካ ልምምዶች መከተል አለባቸው
`device_lab_instrumentation.md` መመሪያ ስለዚህ ቴሌሜትሪ፣ ወረፋ እና ማረጋገጫ
ቅርሶች ከቦታ ማስያዣ መዝገብ ጋር ይሰለፋሉ፡-

1. ** ዘር ማስገቢያ artefacts.** ይፍጠሩ
   `artifacts/android/device_lab/<slot>/` ከመደበኛው ንዑስ አቃፊዎች ጋር እና አሂድ
   `shasum` ማስገቢያው ከተዘጋ በኋላ (የአዲሱን "የአርቲፊክ አቀማመጥ" ክፍልን ይመልከቱ)
   መመሪያ).
2. **የመሳሪያ ትዕዛዞችን ያሂዱ።** የቴሌሜትሪ/የወረፋ ቀረጻውን ያስፈጽሙ።
   መፈጨትን፣ StrongBox መታጠቂያ እና lint/ጥገኛ ቅኝትን መሻር በትክክል
   ውጤቶቹ CI ን በማንፀባረቅ ተጽፈዋል።
3. **የፋይል ማስረጃ።** አዘምን
   `docs/source/compliance/android/evidence_log.csv` እና የቦታ ማስያዣ ትኬቱ
   በ ማስገቢያ መታወቂያ፣ SHA-256 አንጸባራቂ መንገድ እና ተዛማጅ ዳሽቦርድ/Buildkite
   አገናኞች.

የ artefact ማህደርን እና የሃሽ ሰነዱን ከ AND6 የመልቀቂያ ፓኬት ጋር ያያይዙ
የተጎዳው የበረዶ መስኮት. የአስተዳደር ገምጋሚዎች የማረጋገጫ ዝርዝሮችን አይቀበሉም።
ማስገቢያ መለያ እና የመሳሪያ መመሪያውን አይጥቀሱ።

### ቦታ ማስያዝ እና አለመሳካቱ ዝግጁነት ማስረጃ

የመንገድ ካርታ ንጥል "የቁጥጥር የስነ-ጥበብ ማፅደቂያዎች እና የላብራቶሪ ተጠባቂነት" የበለጠ ይፈልጋል
ከመሳሪያዎች ይልቅ. እያንዳንዱ የ AND6 ፓኬት ንቁውን ማጣቀስ አለበት።
የቦታ ማስያዝ የስራ ፍሰት እና የሩብ ዓመት ውድቀት ልምምድ፡- ** ቦታ ማስያዝ መጫወቻ ደብተር (`device_lab_reservation.md`)** ቦታ ማስያዣውን ይከተሉ።
  ሠንጠረዥ (የመሪ ጊዜዎች ፣ ባለቤቶች ፣ የቦታ ርዝመት) ፣ የተጋራውን የቀን መቁጠሪያ በ በኩል ወደ ውጭ ይላኩ።
  `scripts/android_device_lab_export.py`፣ እና `_android-device-lab` ይቅረጹ
  የቲኬት መታወቂያዎች ከአቅም ቅጽበታዊ እይታዎች ጋር በ`evidence_log.csv`። የመጫወቻ መጽሐፍ
  የከፍታውን መሰላል እና የድንገተኛ ጊዜ ቀስቅሴዎችን ይጽፋል; እነዚያን ዝርዝሮች ይቅዱ
  የተያዙ ቦታዎች ሲንቀሳቀሱ ወይም አቅሙ ከስር ሲወድቅ ወደ የማረጋገጫ ዝርዝሩ ግቤት ውስጥ መግባት
  80% የመንገድ ካርታ ግብ.
- ** ያልተሳካለት መሰርሰሪያ runbook (`device_lab_failover_runbook.md`)።
  የሩብ ዓመት ልምምድ (የማቋረጥን አስመስሎ → የመመለሻ መንገዶችን ያስተዋውቁ → ይሳተፉ
  Firebase ፈነዳ + ውጫዊ StrongBox አጋር) እና ቅርሶች ስር አከማች
  `artifacts/android/device_lab_contingency/<drill-id>/`. እያንዳንዱ ጥቅል መሆን አለበት።
  አንጸባራቂውን፣ PagerDuty ወደ ውጪ መላክ፣ Buildkite አሂድ ማገናኛዎች፣ የፋየር ቤዝ ፍንዳታ ይዟል
  ሪፖርት, እና retainer እውቅና runbook ውስጥ ተጠቅሷል. ዋቢ
  መሰርሰሪያ መታወቂያ፣ SHA-256 መግለጫ እና የመከታተያ ትኬት በሁለቱም የማስረጃ መዝገብ እና
  ይህ የማረጋገጫ ዝርዝር.

እነዚህ ሰነዶች አንድ ላይ ሆነው የመሣሪያውን አቅም ማቀድ፣ የማቋረጥ ልምምዶች፣
እና የመሳሪያዎቹ ቅርቅቦች በ የተጠየቁትን የኦዲት ዱካ ይጋራሉ።
የመንገድ ካርታ እና የህግ ገምጋሚዎች.

## ክለሳ Cadence

- ** በየሩብ ዓመቱ *** - የአውሮፓ ህብረት / JP ቅርሶች ወቅታዊ መሆናቸውን ያረጋግጡ; ማደስ
  ማስረጃ ሎግ hashes; የፕሮቬንሽን ቀረጻን ይለማመዱ.
- **ቅድመ-መለቀቅ** - ይህንን የፍተሻ ዝርዝር በእያንዳንዱ የGA/LTS መቁረጫ ጊዜ ያሂዱ እና አያይዘው።
  የተጠናቀቀው ምዝግብ ማስታወሻ ወደ ተለቀቀው RFC.
- ** ከክስተቱ በኋላ *** - በሴቭ 1/2 ክስተት ቴሌሜትሪ ከነካ፣ መፈረም ወይም
  ማረጋገጫ፣ ተዛማጅ ቅርሶችን በማሻሻያ ማስታወሻዎች ያዘምኑ እና
  ማጣቀሻውን በማስረጃ መዝገብ ውስጥ ይያዙ።