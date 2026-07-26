---
lang: am
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ መሳሪያ ላብ ያልተሳካ ቁፋሮ Runbook (AND6/AND7)

ይህ Runbook የአሰራር ሂደቱን፣ የማስረጃ መስፈርቶችን እና የግንኙነት ማትሪክስ ይይዛል
ውስጥ የተጠቀሰውን **የመሣሪያ-ላብራቶሪ ድንገተኛ እቅድ** ሲለማመዱ ጥቅም ላይ ይውላል
`roadmap.md` (§“የቁጥጥር የዕደ ጥበብ ማረጋገጫዎች እና የላብራቶሪ ድንገተኛ ሁኔታ”)። ያሟላል።
የቦታ ማስያዣ የስራ ፍሰት (`device_lab_reservation.md`) እና የአደጋው ምዝግብ ማስታወሻ
(`device_lab_contingency.md`) ተገዢዎችን፣ የህግ አማካሪዎችን እና SRE
ያልተሳካ ዝግጁነትን እንዴት እንደምናረጋግጥ አንድ የእውነት ምንጭ ይኑርዎት።

## ዓላማ እና Cadence

- አንድሮይድ StrongBox + አጠቃላይ የመሳሪያ ገንዳዎች ሊሳኩ እንደሚችሉ አሳይ
  ወደ ኋለኛው የፒክሴል መስመሮች፣ የጋራ ገንዳ፣ የFirebase ሙከራ ላብራቶሪ ፍንዳታ ወረፋ፣ እና
  ውጫዊ StrongBox retainer ያለ AND6/AND7 SLAs.
- ከETSI/FISC ማቅረቢያዎች ጋር ህጋዊ ሊያያይዘው የሚችለውን የማስረጃ ጥቅል አዘጋጅ
  ከፌብሩዋሪ ተገዢነት ግምገማ በፊት።
- ቢያንስ በሩብ አንድ ጊዜ ያሂዱ፣ በተጨማሪም በማንኛውም ጊዜ የላብራቶሪ ሃርድዌር ዝርዝር ሲቀየር
  (አዲስ መሣሪያዎች፣ ጡረታ ወይም ጥገና ከ24 ሰዓት በላይ)።

| የመሰርሰሪያ መታወቂያ | ቀን | ሁኔታ | ማስረጃ ቅርቅብ | ሁኔታ |
|-------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | የፒክሰል8ፕሮ መስመር መቆራረጥ + የማረጋገጫ መዝገብ ከ AND7 ቴሌሜትሪ ልምምድ ጋር | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ ተጠናቋል - በ`docs/source/compliance/android/evidence_log.csv` ውስጥ የተመዘገበ የጥቅል ሀሽ። |
| DR-2026-05-Q2 | 2026-05-22 (የተያዘለት) | የ StrongBox ጥገና መደራረብ + Nexus ልምምድ | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(በመጠባበቅ ላይ)* - `_android-device-lab` ቲኬት **AND6-DR-202605** የተያዙ ቦታዎችን ይይዛል። ጥቅል ከቁፋሮ በኋላ ይሞላል። | 🗓 መርሐግብር የተያዘለት - የቀን መቁጠሪያ እገዳ ወደ "አንድሮይድ መሳሪያ ቤተ ሙከራ - ቦታ ማስያዣዎች" በAND6 ታክሏል። |

#ሂደት።

### 1. የቅድመ-ቁፋሮ ዝግጅት

1. በ `docs/source/sdk/android/android_strongbox_capture_status.md` ውስጥ የመነሻ አቅምን ያረጋግጡ.
2. ለታለመው የ ISO ሳምንት የቦታ ማስያዣ ካሌንደርን በ በኩል ይላኩ።
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. ፋይል `_android-device-lab` ትኬት
   `AND6-DR-<YYYYMM>` ወሰን ያለው ("የማይሳካለት መሰርሰሪያ")፣ የታቀዱ ክፍተቶች እና የተጎዱ
   የሥራ ጫና (ማስረጃ፣ CI ጭስ፣ የቴሌሜትሪ ትርምስ)።
4. በ `device_lab_contingency.md` ውስጥ ያለውን የአደጋ ጊዜ ምዝግብ አብነት በኤ
   ቦታ ያዥ ረድፍ ለመሰርሰሪያው ቀን።

### 2. የውድቀት ሁኔታዎችን አስመስለው

1. በቤተ ሙከራ ውስጥ ዋናውን መስመር (`pixel8pro-strongbox-a`) ያሰናክሉ ወይም ያሰናክሉ
   መርሐግብር አዘጋጅ እና የቦታ ማስያዣ ግቤትን እንደ “ቁፋሮ” መለያ ይስጡት።
2. በፔጀርዱቲ (`AND6-device-lab` አገልግሎት) ውስጥ የማስመሰል ማስጠንቀቂያ ያስነሱ እና
   የማስረጃ ጥቅል የማሳወቂያ ወደ ውጭ መላክን ይያዙ።
3. በተለምዶ ሌይን የሚበሉትን የBuildkite ስራዎችን ያብራሩ
   (`android-strongbox-attestation`፣ `android-ci-e2e`) ከመሰርሰሪያ መታወቂያው ጋር።

### 3. ያልተሳካ አፈፃፀም1. የመመለሻ Pixel7 መስመርን ወደ ዋናው CI ኢላማ ያስተዋውቁ እና የጊዜ ሰሌዳውን ያስይዙ
   በእሱ ላይ የታቀዱ የሥራ ጫናዎች.
2. በ`firebase-burst` ሌይን በኩል የFirebase Test Lab burst Suiteን ቀስቅሰው
   የ StrongBox ሽፋን ወደ የጋራ ሲሸጋገር የችርቻሮ-Wallet ጭስ ይፈትሻል
   መስመር. ለኦዲት በቲኬቱ ውስጥ የCLI ጥሪን (ወይም ኮንሶል ወደ ውጭ መላክ) ይያዙ
   እኩልነት.
3. የውጭውን StrongBox ላብራቶሪ መያዣን ለአጭር ጊዜ የማረጋገጫ መጥረግ ያሳትፉ።
   የምዝግብ ማስታወሻ ዕውቅና ከዚህ በታች እንደተገለጸው.
4. ሁሉንም የBuildkite አሂድ መታወቂያዎች፣ የFirebase job URLs እና የማቆያ ግልባጮችን በ ውስጥ ይቅረጹ
   የ`_android-device-lab` ትኬት እና የማስረጃ ጥቅል መግለጫ።

### 4. ማረጋገጥ እና መመለስ

1. የማረጋገጫ/CI runtimesን ከመነሻ መስመር ጋር ያወዳድሩ። ባንዲራ deltas>10% ወደ
   የሃርድዌር ላብ መሪ።
2. ዋናውን መስመር ወደነበረበት ይመልሱ እና የአቅም ቅጽበቱን እና ዝግጁነቱን ያዘምኑ
   ማትሪክስ አንዴ ማረጋገጫ ካለፈ።
3. የመጨረሻውን ረድፍ ወደ `device_lab_contingency.md` በመቀስቀስ፣ በድርጊቶች፣
   እና ክትትል.
4. `docs/source/compliance/android/evidence_log.csv` ያዘምኑ በ፡
   የጥቅል መንገድ፣ SHA-256 አንጸባራቂ፣ የBuildkite አሂድ መታወቂያዎች፣ PagerDuty ወደ ውጪ መላክ ሃሽ እና
   ገምጋሚ ማቋረጥ።

## የማስረጃ ጥቅል አቀማመጥ

| ፋይል | መግለጫ |
|-------------|
| `README.md` | ማጠቃለያ (የቁፋሮ መታወቂያ፣ ወሰን፣ ባለቤቶች፣ የጊዜ መስመር)። |
| `bundle-manifest.json` | በጥቅሉ ውስጥ ላለው እያንዳንዱ ፋይል የSHA-256 ካርታ። |
| `calendar-export.{ics,json}` | የ ISO-ሳምንት ቦታ ማስያዣ ቀን መቁጠሪያ ከኤክስፖርት ስክሪፕት። |
| `pagerduty/incident_<id>.json` | PagerDuty ወደ ውጪ መላክ የማስጠንቀቂያ + የዕውቅና ጊዜ መስመርን ያሳያል። |
| `buildkite/<job>.txt` | ለተጎዱ ስራዎች ዩአርኤሎችን እና ምዝግብ ማስታወሻዎችን ያሂዱ። |
| `firebase/burst_report.json` | የFirebase ሙከራ ቤተ ሙከራ የፍንዳታ ማስፈጸሚያ ማጠቃለያ። |
| `retainer/acknowledgement.eml` | ከውጪው StrongBox ቤተ ሙከራ ማረጋገጫ። |
| `photos/` | ሃርድዌር በድጋሚ በኬብል ከተሰራ የአማራጭ ፎቶዎች/የላብ ቶፖሎጂ ቅጽበታዊ ገጽ እይታዎች። |

ጥቅሉን በ
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` እና መዝገብ
ከማስረጃ መዝገብ ውስጥ ያለው አንጸባራቂ ቼክ እና የብአዴን6 ተገዢነት ማረጋገጫ ዝርዝር።

## ዕውቂያ እና መጨመር ማትሪክስ

| ሚና | ዋና ግንኙነት | ቻናል(ዎች) | ማስታወሻ |
|-------------|--------|-------|
| የሃርድዌር ላብ አመራር | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | በጣቢያው ላይ ያሉ ድርጊቶች እና የቀን መቁጠሪያ ዝማኔዎች ባለቤት ናቸው። |
| Device Lab Ops | Mateo Cruz | `_android-device-lab` ወረፋ | የቦታ ማስያዣ ትኬቶችን + ጥቅል ሰቀላዎችን ያስተባብራል። |
| የምህንድስና ልቀቅ | Alexei Morozov | መልቀቅ Eng Slack · `release-eng@iroha.org` | የBuildkite ማስረጃን ያረጋግጣል + hashes ያትማል። |
| ውጫዊ StrongBox ቤተ ሙከራ | Sakura መሣሪያዎች NOC | `noc@sakura.example` · +81-3-5550-9876 | የማቆያ ግንኙነት; በ6 ሰአት ውስጥ መገኘቱን ያረጋግጡ። |
| Firebase ፍንዳታ አስተባባሪ | ቴሳ ራይት | `@android-ci` Slack | ውድቀት ሲያስፈልግ የFirebase ሙከራ ላብ አውቶማቲክን ያነሳሳል። |

አንድ መሰርሰሪያ የማገድ ችግሮችን ካወቀ በሚከተለው ቅደም ተከተል ጨምር፡
1. የሃርድዌር ላብ መሪ
2. አንድሮይድ ፋውንዴሽን TL
3. የፕሮግራም መሪ / የመልቀቅ ምህንድስና
4. ተገዢነት አመራር + የህግ አማካሪ (መሰርሰሪያ የቁጥጥር ስጋትን ካሳየ)

## ሪፖርት ማድረግ እና ክትትል- በማንኛውም ጊዜ በማጣቀሻ ጊዜ ይህንን የሩጫ መጽሐፍ ከቦታ ማስያዣ ሂደት ጋር ያገናኙት።
  አለመሳካት ዝግጁነት በ`roadmap.md`፣ `status.md` እና የአስተዳደር ፓኬቶች።
- የሩብ ዓመቱን የድጋሚ መግለጫ ወደ Compliance + Legal ከማስረጃ ጥቅል ጋር ኢሜል ያድርጉ
  የሃሽ ጠረጴዛ እና የ`_android-device-lab` ትኬት ወደ ውጭ መላክን ያያይዙ።
- ቁልፍ መለኪያዎችን ያንጸባርቁ (ጊዜ-ወደ-ውድቀት፣ የስራ ጫናዎች ወደ ነበሩበት ተመልሰዋል፣ አስደናቂ እርምጃዎች)
  በ `status.md` እና በ AND7 የሙቅ ዝርዝር መከታተያ ውስጥ ገምጋሚዎች መከታተል እንዲችሉ
  ተጨባጭ ልምምድ ላይ ጥገኛ መሆን.