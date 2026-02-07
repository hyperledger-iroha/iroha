---
lang: am
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2025-12-29T18:16:35.924530+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ መሳሪያ ቤተ ሙከራ ቦታ ማስያዝ ሂደት (AND6/AND7)

ይህ የመጫወቻ መጽሐፍ የአንድሮይድ ቡድን እንዴት እንደሚጽፍ፣ እንደሚያረጋግጥ እና መሣሪያውን እንደሚመረምር ይገልጻል
የላብራቶሪ ጊዜ ለትልልቅ ደረጃዎች **AND6** (CI & compliance hardening) እና **AND7**
(የታዛቢነት ዝግጁነት). የአደጋ ጊዜ መግቢያን ያሟላል።
አቅምን በማረጋገጥ `docs/source/compliance/android/device_lab_contingency.md`
በመጀመሪያ ደረጃ ጉድለቶች ይወገዳሉ.

## 1. ግቦች እና ወሰን

- የ StrongBox + አጠቃላይ የመሳሪያ ገንዳዎችን ከመንገድ ካርታው 80% በላይ ያቆዩ
  በበረዶ መስኮቶች ውስጥ የአቅም ዒላማ.
- የሚወስን የቀን መቁጠሪያ ያቅርቡ ስለዚህ CI፣ የማረጋገጫ ጠራርጎ እና ትርምስ
  ልምምዶች ለተመሳሳይ ሃርድዌር በጭራሽ አይወዳደሩም።
- የሚመገብ ኦዲት ሊደረግ የሚችል ዱካ (ጥያቄዎች፣ ማፅደቆች፣ ከሂደቱ በኋላ ማስታወሻዎች) ይያዙ
  የብአዴን6 ተገዢነት ማረጋገጫ ዝርዝር እና የማስረጃ መዝገብ።

ይህ አሰራር የወሰኑ የፒክሰል መስመሮችን፣ የጋራ መመለሻ ገንዳውን እና ይሸፍናል።
በመንገድ ካርታው ላይ የተጠቀሰው የውጭ StrongBox ላብራቶሪ መያዣ። አድሆክ ኢሚሌተር
አጠቃቀም ወሰን ውጭ ነው።

## 2. የመጠባበቂያ ዊንዶውስ

| ገንዳ / ሌይን | ሃርድዌር | ነባሪ ማስገቢያ ርዝመት | የቅድሚያ ጊዜ ማስያዝ | ባለቤት |
|------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4 ሰ | 3 የስራ ቀናት | የሃርድዌር ላብ አመራር |
| `pixel8a-ci-b` | Pixel8a (CI አጠቃላይ) | 2 ሰ | 2 የስራ ቀናት | አንድሮይድ መሠረቶች TL |
| `pixel7-fallback` | Pixel7 የጋራ ገንዳ | 2 ሰ | 1 የስራ ቀን | የምህንድስና ልቀቅ |
| `firebase-burst` | Firebase ሙከራ ላብ ጭስ ወረፋ | 1 ሰ | 1 የስራ ቀን | አንድሮይድ መሠረቶች TL |
| `strongbox-external` | ውጫዊ StrongBox የላብራቶሪ መያዣ | 8 ሰ | 7 የቀን መቁጠሪያ ቀናት | የፕሮግራም መሪ |

ቦታዎች UTC ውስጥ የተያዙ ናቸው; ተደራራቢ ቦታ ማስያዣዎች ግልጽ ማረጋገጫ ያስፈልጋቸዋል
ከሃርድዌር ላብ መሪ.

## 3. የስራ ፍሰት ይጠይቁ

1. ** አውድ አዘጋጅ ***
   - `docs/source/sdk/android/android_strongbox_device_matrix.md` ጋር አዘምን
     የአካል ብቃት እንቅስቃሴ ለማድረግ ያቀዷቸው መሳሪያዎች እና ዝግጁነት መለያ
     (`attestation`፣ `ci`፣ `chaos`፣ `partner`)።
   - የቅርብ ጊዜውን የአቅም ቅጽበታዊ ገጽ እይታ ይሰብስቡ
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **ጥያቄ አስገባ**
   - አብነቱን በመጠቀም በ`_android-device-lab` ወረፋ ውስጥ ትኬት ያስገቡ
     `docs/examples/android_device_lab_request.md` (ባለቤት፣ ቀኖች፣ የስራ ጫናዎች፣
     የመውደቅ መስፈርት).
   - ማንኛውንም የቁጥጥር ጥገኞችን ያያይዙ (ለምሳሌ AND6 የማረጋገጫ መጥረግ፣ AND7
     የቴሌሜትሪ መሰርሰሪያ) እና ከተዛማጅ የመንገድ ካርታ ግቤት ጋር ያገናኙ።
3. ** ማጽደቅ ***
   - በአንድ የስራ ቀን ውስጥ የሃርድዌር ላብ አመራር ግምገማዎች, ውስጥ ማስገቢያ ያረጋግጣል
     የተጋራ የቀን መቁጠሪያ (`Android Device Lab – Reservations`) እና ያዘምናል።
     `device_lab_capacity_pct` አምድ ውስጥ
     `docs/source/compliance/android/evidence_log.csv`.
4. ** መፈጸም ***
   - የታቀዱ ስራዎችን ያካሂዱ; የBuildkite አሂድ መታወቂያዎችን ወይም የመሳሪያ ምዝግብ ማስታወሻዎችን ይመዝግቡ።
   - ማናቸውንም ልዩነቶች (የሃርድዌር መለዋወጥ፣ ከመጠን በላይ መጨናነቅ) አስተውል።
5. ** መዘጋት ***
   - በቲኬቱ ላይ በአርቲፊክስ/አገናኞች አስተያየት ይስጡ።
   - ሩጫው ከማክበር ጋር የተያያዘ ከሆነ ያዘምኑ
     `docs/source/compliance/android/and6_compliance_checklist.md` እና ረድፍ ጨምር
     ወደ `evidence_log.csv`.

በአጋር ማሳያዎች (AND8) ላይ ተጽእኖ የሚያሳድሩ ጥያቄዎች አጋር ምህንድስናን CC ማድረግ አለባቸው።

## 4. ለውጥ እና መሰረዝ- ** እንደገና መርሐግብር ያስይዙ: *** ዋናውን ትኬት እንደገና ይክፈቱ ፣ አዲስ ማስገቢያ ያቅርቡ እና ያዘምኑ
  የቀን መቁጠሪያ ግቤት. አዲሱ ማስገቢያ በ 24h ውስጥ ከሆነ, ፒንግ ሃርድዌር Lab Lead + SRE
  በቀጥታ.
- ** የአደጋ ጊዜ ስረዛ:** የአደጋ ጊዜ ዕቅዱን ይከተሉ
  (`device_lab_contingency.md`) እና ቀስቅሴ/ድርጊት/ተከታይ ረድፎችን ይመዝግቡ።
- ** ተትረፍርፏል፡** ሩጫው ከሽግግሩ በ>15 ደቂቃ ካለፈ፣ ማሻሻያ ይለጥፉ እና ያረጋግጡ
  የሚቀጥለው ቦታ ማስያዝ መቀጠል ይችል እንደሆነ; አለበለዚያ ወደ ውድቀት እጅ ይስጡ
  ገንዳ ወይም Firebase ፍንዳታ መስመር.

## 5. ማስረጃ እና ኦዲት

| Artefact | አካባቢ | ማስታወሻ |
|-------------|---|
| የቦታ ማስያዣ ትኬቶች | `_android-device-lab` ወረፋ (ጂራ) | ሳምንታዊ ማጠቃለያ ወደ ውጭ ላክ; የቲኬት መታወቂያዎች በማስረጃ መዝገብ ውስጥ። |
| የቀን መቁጠሪያ ወደ ውጭ መላክ | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | በእያንዳንዱ አርብ `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` ን ያሂዱ; ረዳቱ የተጣራውን `.ics` ፋይል እና የJSON ማጠቃለያ ለ ISO ሳምንት ያድናል ስለዚህ ኦዲቶች ሁለቱንም ቅርሶች በእጅ ሳይወርዱ ማያያዝ ይችላሉ። |
| የአቅም ቅጽበታዊ እይታዎች | `docs/source/compliance/android/evidence_log.csv` | ከእያንዳንዱ ቦታ ማስያዝ/መዘጋት በኋላ ያዘምኑ። |
| ድህረ-ማስታወሻዎች | `docs/source/compliance/android/device_lab_contingency.md` (አደጋ ከሆነ) ወይም የቲኬት አስተያየት | ለኦዲት ያስፈልጋል። |

በየሩብ ዓመቱ የማክበር ግምገማዎች፣ የቀን መቁጠሪያውን ወደ ውጭ መላክ፣ የቲኬት ማጠቃለያ፣
እና ከብአዴን 6 የማረጋገጫ መዝገብ ላይ የተወሰደ።

### የቀን መቁጠሪያ ወደ ውጭ መላክ አውቶማቲክ

1. የ ICS ምግብ ዩአርኤል ያግኙ (ወይም `.ics` ፋይል ያውርዱ) ለ "አንድሮይድ መሳሪያ ቤተ ሙከራ - ቦታ ማስያዝ"።
2. መፈጸም

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   ስክሪፕቱ ሁለቱንም `artifacts/android/device_lab/<YYYY-WW>-calendar.ics` ይጽፋል
   እና `...-calendar.json`, የተመረጠውን ISO ሳምንት በመያዝ.
3. የተፈጠሩትን ፋይሎች ከሳምንታዊው የማስረጃ ፓኬት ጋር ይስቀሉ እና ማጣቀሻውን ያጣሩ
   JSON ማጠቃለያ በ `docs/source/compliance/android/evidence_log.csv` መቼ
   የመመዝገቢያ መሳሪያ-የላብራቶሪ አቅም.

## 6. Escalation መሰላል

1. የሃርድዌር ላብራቶሪ መሪ (ዋና)
2. አንድሮይድ ፋውንዴሽን TL
3. የፕሮግራም መሪ/ልቀት ምህንድስና (ለበረዶ መስኮቶች)
4. ውጫዊ StrongBox የላብራቶሪ ግንኙነት (ያያዘው ሲጠራ)

ጭማሪዎች በቲኬቱ ውስጥ ገብተው በየሳምንቱ አንድሮይድ ውስጥ መንጸባረቅ አለባቸው
ሁኔታ ደብዳቤ.

## 7. ተዛማጅ ሰነዶች

- `docs/source/compliance/android/device_lab_contingency.md` - የክስተት መዝገብ
  የአቅም ጉድለቶች.
- `docs/source/compliance/android/and6_compliance_checklist.md` - ዋና
  ሊደርስ የሚችል ዝርዝር.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` - ሃርድዌር
  ሽፋን መከታተያ.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  በ AND6/AND7 የተጠቀሰ የስትሮንግቦክስ ማረጋገጫ ማስረጃ።

ይህንን የቦታ ማስያዝ ሂደት ማቆየት የፍኖተ ካርታ እርምጃ ንጥልን ያሟላል።
የመሣሪያ-ላብራቶሪ ማስያዣ ሂደት” እና አጋርን የሚመለከቱ ተገዢ የሆኑ ቅርሶችን ያስቀምጣል።
ከተቀረው የአንድሮይድ ዝግጁነት እቅድ ጋር በማመሳሰል።

## 8. ያልተሳካ የመሰርሰሪያ ሂደት እና እውቂያዎች

የRoadmap ንጥል AND6 እንዲሁ በየሩብ አመቱ ያልተሳካ ልምምድ ያስፈልገዋል። ሙሉ፣
የደረጃ በደረጃ መመሪያዎች በቀጥታ ውስጥ
`docs/source/compliance/android/device_lab_failover_runbook.md`, ግን ከፍተኛ
ደረጃ የስራ ሂደት ከዚህ በታች ተጠቃሏል ስለዚህ ጠያቂዎች ልምምዶችን አብረው ማቀድ ይችላሉ።
መደበኛ ቦታ ማስያዝ.1. ** መሰርሰሪያውን መርሐግብር ያውጡ:** የተጎዱትን መስመሮች አግድ (`pixel8pro-strongbox-a`,
   የመውደቅ ገንዳ፣ `firebase-burst`፣ ውጫዊ StrongBox retainer) በጋራ
   ካላንደር እና `_android-device-lab` ወረፋ ቢያንስ 7 ቀናት ቀደም ብሎ።
2. **የማቋረጥን አስመስሎ፡** ዋናውን ሌይን አጣርተህ ፔጀርዱቲውን ቀስቅሰው
   (`AND6-device-lab`) ክስተት፣ እና ጥገኛ Buildkite ስራዎችን አብራራ
   በ runbook ውስጥ የተጠቀሰው የመሰርሰሪያ መታወቂያ።
3. ** አልተሳካም: ** የ Pixel7 መመለሻ መስመርን ያስተዋውቁ፣ የFirebase ፍንዳታ ያስጀምሩ
   ስብስብ፣ እና የውጭውን የስትሮንግቦክስ አጋር በ6ሰአታት ውስጥ ያሳትፉ። ያንሱ
   Buildkite ዩአርኤሎችን፣ የFirebase ወደ ውጭ መላክን እና የማቆያ እውቅናዎችን ያስኬዳል።
4. ** አረጋግጥ እና እነበረበት መልስ፡** የምስክርነት ማረጋገጫ + CI runtimes አረጋግጥ፣ ወደነበረበት መልስ
   ኦሪጅናል መስመሮች፣ እና `device_lab_contingency.md` እና የማስረጃ ምዝግብ ማስታወሻውን ያዘምኑ
   ከጥቅል መንገድ + ቼኮች ጋር።

### የእውቂያ እና የመጨመር ማጣቀሻ

| ሚና | ዋና ግንኙነት | ቻናል(ዎች) | የመጨመር ትዕዛዝ |
|-------------|------------|
| የሃርድዌር ላብ አመራር | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | 1 |
| Device Lab Ops | Mateo Cruz | `_android-device-lab` ወረፋ | 2 |
| አንድሮይድ መሠረቶች TL | ኤሌና ቮሮቤቫ | `@android-foundations` Slack | 3 |
| የምህንድስና ልቀቅ | Alexei Morozov | `release-eng@iroha.org` | 4 |
| ውጫዊ StrongBox ቤተ ሙከራ | Sakura መሣሪያዎች NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

መሰርሰሪያው የማገጃ ችግሮችን ከገለጠ ወይም ማንኛውም ውድቀት ካለ በቅደም ተከተል አስፋ
መስመር በ 30 ደቂቃ ውስጥ መስመር ላይ ማምጣት አይቻልም። ሁልጊዜ መጨመሩን ይመዝግቡ
በ `_android-device-lab` ትኬት ውስጥ ማስታወሻዎችን እና በድንገተኛ መዝገብ ውስጥ ያንጸባርቁዋቸው.