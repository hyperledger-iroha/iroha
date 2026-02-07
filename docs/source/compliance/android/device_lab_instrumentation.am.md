---
lang: am
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ መሳሪያ ላብ መሳሪያ መንጠቆዎች (AND6)

ይህ ማመሳከሪያ የመንገድ ካርታ እርምጃን ይዘጋዋል “የቀረውን መሳሪያ-ላብራቶሪ ደረጃ /
የመሳሪያ መንጠቆዎች ከ AND6 kickoff በፊት። እያንዳንዱ እንዴት እንደተያዘ ያብራራል።
የመሳሪያ-ላብ ማስገቢያ ቴሌሜትሪ፣ ወረፋ እና የማረጋገጫ ቅርሶችን መያዝ አለበት።
የብአዴን6 ተገዢነት ማረጋገጫ ዝርዝር፣ የማስረጃ መዝገብ እና የአስተዳደር እሽጎች ተመሳሳይ ናቸው።
የሚወስን የስራ ፍሰት. ይህንን ማስታወሻ ከቦታ ማስያዝ ሂደት ጋር ያጣምሩ
(`device_lab_reservation.md`) እና ልምምዶችን ሲያቅዱ ያልተሳካው የሩጫ መጽሐፍ።

## ግቦች እና ወሰን

- ** ቁርጥ ያለ ማስረጃ *** - ሁሉም የመሳሪያ ውጤቶች በስር ይኖራሉ
  `artifacts/android/device_lab/<slot-id>/` ከ SHA-256 ጋር ኦዲተሮችን ያሳያል
  መመርመሪያዎቹን እንደገና ሳያስኬዱ ጥቅሎችን ሊለያይ ይችላል።
- ** ስክሪፕት-የመጀመሪያው የስራ ፍሰት *** - ያሉትን ረዳቶች እንደገና ይጠቀሙ
  (`ci/run_android_telemetry_chaos_prep.sh`፣
  `scripts/android_keystore_attestation.sh`፣ `scripts/android_override_tool.sh`)
  በድብቅ የአድቢ ትዕዛዞች ፈንታ።
- ** የፍተሻ ዝርዝሮች በተመሳሳይ ጊዜ ይቆያሉ *** - እያንዳንዱ ሩጫ ይህንን ሰነድ ከ
  AND6 የፍተሻ ዝርዝርን አሟልቷል እና ቅርሶቹን ወደ ላይ ጨምሯል።
  `docs/source/compliance/android/evidence_log.csv`.

## አርቲፊሻል አቀማመጥ

1. ከቦታ ማስያዣ ትኬቱ ጋር የሚዛመድ ልዩ ማስገቢያ መለያ ይምረጡ፣ ለምሳሌ
   `2026-05-12-slot-a`.
2. ደረጃውን የጠበቀ ማውጫ ዘርዝር፡

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. እያንዳንዱን የትዕዛዝ መዝገብ በተዛማጅ አቃፊ ውስጥ ያስቀምጡ (ለምሳሌ፡.
   `telemetry/status.ndjson`፣ `attestation/pixel8pro.log`)።
4. SHA-256 ይቅረጹ ማስገቢያው አንዴ ከተዘጋ፡-

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## የመሳሪያ ማትሪክስ

| ፍሰት | ትዕዛዝ(ዎች) | የውጤት ቦታ | ማስታወሻ |
|-------------|--------|
| ቴሌሜትሪ ማሻሻያ + የሁኔታ ጥቅል | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | የ ማስገቢያ መጀመሪያ እና መጨረሻ ላይ አሂድ; CLI stdoutን ከ `status.log` ጋር ያያይዙ። |
| በመጠባበቅ ላይ ያለ ወረፋ + ትርምስ መሰናዶ | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`፣ `queue/*.json`፣ `queue/*.sha256` | መስተዋቶች ScenarioD ከ `readiness/labs/telemetry_lab_01.md`; ማስገቢያ ውስጥ ለእያንዳንዱ መሣሪያ env var ዘርጋ. |
| የሒሳብ መፍጨትን ይሽሩ | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | ምንም መሻር በማይሠራበት ጊዜ እንኳን የሚፈለግ; የዜሮ ሁኔታን ያረጋግጡ. |
| StrongBox / TEE ማረጋገጫ | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | ለእያንዳንዱ የተያዘ መሳሪያ ይድገሙ (በ `android_strongbox_device_matrix.md` ውስጥ ያሉ ተዛማጅ ስሞች)። |
| CI መታጠቂያ ማረጋገጫ regression | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI ሰቀላዎችን የሚያሳዩ ተመሳሳይ ማስረጃዎችን ይይዛል; ለሲሜትሜትሪ በእጅ ሩጫ ውስጥ ያካትቱ። |
| Lint / ጥገኝነት መነሻ | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | በቀዝቃዛው መስኮት አንድ ጊዜ ያሂዱ; ማጠቃለያውን በተጣጣመ ፓኬቶች ውስጥ ጥቀስ። |

## መደበኛ ማስገቢያ ሂደት1. ** ቅድመ በረራ (T-24 ሰ)** - የቦታ ማስያዣ ትኬቱን ይህንን ያረጋግጡ
   ሰነድ፣ የመሳሪያውን ማትሪክስ ግቤት ያዘምኑ እና የአርቲፊክ ሩትን ዘር።
2. ** በመግቢያው ወቅት ***
   - መጀመሪያ የቴሌሜትሪ ቅርቅቡን + ወረፋ ወደ ውጭ መላክ ትዕዛዞችን ያስኪዱ። ማለፍ
     `--note <ticket>` ወደ `ci/run_android_telemetry_chaos_prep.sh` ስለዚህ ምዝግብ ማስታወሻው
     የክስተቱን መታወቂያ ይጠቅሳል።
   - በእያንዳንዱ መሣሪያ የማረጋገጫ ስክሪፕቶችን ያስነሱ። ማሰሪያው ሲያመርት
     `.zip`፣ ወደ አርቲፊክ ሩት ይቅዱት እና Git SHA የታተመውን ይቅረጹ
     የስክሪፕቱ መጨረሻ.
   - CI ቢሆንም እንኳ `make android-lint` በተሻረው የማጠቃለያ መንገድ ያስፈጽሙ
     ቀድሞውኑ ሮጡ; ኦዲተሮች በየቦታው ሎግ ይጠብቃሉ።
3. **ከድኅረ ሩጫ**
   - በመክተቻው ውስጥ `sha256sum.txt` እና `README.md` (የነጻ ቅጽ ማስታወሻዎችን) ይፍጠሩ
     የተፈጸሙትን ትዕዛዞች ማጠቃለያ አቃፊ.
   - አንድ ረድፍ ከ `docs/source/compliance/android/evidence_log.csv` ጋር ያያይዙ
     ማስገቢያ መታወቂያ፣ የሃሽ አንጸባራቂ መንገድ፣ የBuildkite ማጣቀሻዎች (ካለ) እና የቅርብ ጊዜው
     የመሣሪያ-ላብራቶሪ አቅም መቶኛ ከተያዘው የቀን መቁጠሪያ ወደ ውጪ መላክ።
   - ማስገቢያ ማህደርን በ `_android-device-lab` ቲኬት ፣ AND6 ያገናኙ
     የማረጋገጫ ዝርዝር፣ እና `docs/source/android_support_playbook.md` የመልቀቂያ ሪፖርት።

## አለመሳካት አያያዝ እና መጨመር

- ማንኛውም ትእዛዝ ካልተሳካ የ stderr ውፅዓት በ `logs/` ስር ይያዙ እና ይከተሉ
  የማሳደግ መሰላል በ `device_lab_reservation.md` §6.
- የወረፋ ወይም የቴሌሜትሪ ጉድለቶች ወዲያውኑ የመሻር ሁኔታን ልብ ይበሉ
  `docs/source/sdk/android/telemetry_override_log.md` እና ማስገቢያ መታወቂያ ማጣቀሻ
  ስለዚህ አስተዳደር ቁፋሮውን መከታተል ይችላል.
- የማረጋገጫ ድጋፎች መመዝገብ አለባቸው
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ያልተሳኩ የመሳሪያዎች ተከታታይ እና ከላይ ከተመዘገቡት የጥቅል መንገዶች ጋር.

## የሪፖርት ማመሳከሪያ ዝርዝር

ክፍተቱ እንደተጠናቀቀ ምልክት ከማድረግዎ በፊት፣ የሚከተሉት ማጣቀሻዎች መዘመንዎን ያረጋግጡ።

- `docs/source/compliance/android/and6_compliance_checklist.md` - ምልክት ያድርጉበት
  instrumentation ረድፍ ተጠናቀቀ እና ማስገቢያ መታወቂያ ልብ ይበሉ.
- `docs/source/compliance/android/evidence_log.csv` - ግቤትን ይጨምሩ / ያዘምኑ
  የ ማስገቢያ ሃሽ እና የአቅም ንባብ.
- `_android-device-lab` ቲኬት - የጥበብ ማያያዣዎችን እና የBuildkite የስራ መታወቂያዎችን ያያይዙ።
- `status.md` - በሚቀጥለው የአንድሮይድ ዝግጁነት መፍጨት አጭር ማስታወሻ ያካትቱ።
  የመንገድ ካርታ አንባቢዎች የትኛው ማስገቢያ የቅርብ ጊዜ ማስረጃ እንዳቀረበ ያውቃሉ።

ይህን ሂደት ተከትሎ የብአዴን6 "የመሳሪያ-ላብ + የመሳሪያ መንጠቆዎችን" ያቆያል
ወሳኝ ደረጃ ኦዲት ሊደረግ የሚችል እና በእጅ በማስያዝ፣ በማስፈጸም መካከል ያለውን ልዩነት ይከላከላል፣
እና ሪፖርት ማድረግ.