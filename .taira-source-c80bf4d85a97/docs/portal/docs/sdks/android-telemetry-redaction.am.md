---
lang: am
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c9ce5a256683152440986a028d1e57ed298bbbb189196af866128962228caa5
source_last_modified: "2026-01-05T09:28:11.847834+00:00"
translation_last_reviewed: 2026-02-07
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
slug: /sdks/android-telemetry
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ ቴሌሜትሪ ማሻሻያ ዕቅድ (AND7)

## ወሰን

ይህ ሰነድ የታቀደውን የቴሌሜትሪ ማሻሻያ ፖሊሲ እና ማስቻልን ይይዛል
በሮድ ካርታ ንጥል **AND7** እንደተፈለገ ለአንድሮይድ ኤስዲኬ ቅርሶች። ያስተካክላል
በሂሳብ አያያዝ ወቅት የሞባይል መሳሪያ ከ Rust node baseline ጋር
መሣሪያ-ተኮር የግላዊነት ዋስትናዎች። ውጤቱ ለቅድመ-ንባብ ሆኖ ያገለግላል
ፌብሩዋሪ 2026 SRE የአስተዳደር ግምገማ።

ዓላማዎች፡-

- የጋራ ታዛቢነት ላይ የሚደርሰውን እያንዳንዱን አንድሮይድ የሚወጣ ምልክት ካታሎግ
  የኋላ (OpenTelemetry traces፣ Norito-የተመሰጠሩ ምዝግብ ማስታወሻዎች፣መለኪያዎች ወደ ውጭ መላክ)።
- ከ Rust መነሻ መስመር እና ከሰነድ ማሻሻያ የሚለያዩ መስኮችን መድብ ወይም
  የማቆያ መቆጣጠሪያዎች.
- የድጋፍ ቡድኖች ምላሽ እንዲሰጡ የማስቻል ስራን እና የሙከራ ስራዎችን ይግለጹ
  በቁርጠኝነት ወደ ማሻሻያ-ነክ ማንቂያዎች።

## የሲግናል ክምችት (ረቂቅ)

የታቀደ መሳሪያ በሰርጥ ተመድቧል። ሁሉም የመስክ ስሞች አንድሮይድ ይከተላሉ
የኤስዲኬ ቴሌሜትሪ እቅድ (`org.hyperledger.iroha.android.telemetry.*`)። አማራጭ
መስኮች በ I18NI0000023X ምልክት ይደረግባቸዋል.

| የሲግናል መታወቂያ | ቻናል | ቁልፍ መስኮች | PII/PHI ምደባ | ማደስ / ማቆየት | ማስታወሻ |
|--------|--------|--------|
| `android.torii.http.request` | የመከታተያ ስፋት | `authority_hash`፣ `route`፣ `status_code`፣ `latency_ms` | ስልጣን የህዝብ ነው; መንገድ ሚስጥር የለውም | ወደ ውጭ ከመላኩ በፊት የተጨበጠ ባለስልጣን (I18NI0000029X) ለ 7 ቀናት ይቆዩ | መስተዋቶች ዝገት I18NI0000030X; hashing የሞባይል ተለዋጭ ስም ግላዊነትን ያረጋግጣል። |
| `android.torii.http.retry` | ክስተት | `route`፣ `retry_count`፣ `error_code`፣ `backoff_ms` | የለም | ምንም ማሻሻያ የለም; 30 ቀናት ይቆዩ | ለዳግም ሙከራ ኦዲት ጥቅም ላይ ይውላል; ከ Rust መስኮች ጋር ተመሳሳይ። |
| `android.pending_queue.depth` | መለኪያ መለኪያ | `queue_type`, `depth` | የለም | ምንም ማሻሻያ የለም; 90 ቀናት ይቆዩ | ግጥሚያዎች ዝገት `pipeline.pending_queue_depth`. |
| `android.keystore.attestation.result` | ክስተት | `alias_label`፣ `security_level`፣ `attestation_digest`፣ `device_brand_bucket` | ተለዋጭ ስም (የተገኘ)፣ የመሣሪያ ሜታዳታ | ተለዋጭ ስም በተሰየመ መለያ ይተኩ፣ የምርት ስም ወደ ባልዲ ይቀይሩ | ለ AND2 ምስክርነት ዝግጁነት ያስፈልጋል; የዝገት ኖዶች የመሳሪያውን ዲበ ውሂብ አያወጡም። |
| `android.keystore.attestation.failure` | ቆጣሪ | `alias_label`, `failure_reason` | ከቅፅል ስም በኋላ የለም | ምንም ማሻሻያ የለም; 90 ቀናት ይቆዩ | ትርምስ ልምምዶችን ይደግፋል; ቅጽል_ስያሜ ከሃሽድ ተለዋጭ ስም የተገኘ። |
| `android.telemetry.redaction.override` | ክስተት | `override_id`፣ `actor_role_masked`፣ `reason`፣ `expires_at` | የተዋናይ ሚና እንደ PII | የሜዳ ኤክስፖርት ጭምብል የሚና ምድብ; 365 ቀናት በኦዲት መዝገብ መያዝ | ዝገት ውስጥ የለም; ኦፕሬተሮች መሻሮችን በድጋፍ ፋይል ማድረግ አለባቸው። |
| `android.telemetry.export.status` | ቆጣሪ | `backend`, `status` | የለም | ምንም ማሻሻያ የለም; 30 ቀናት ይቆዩ | ከዝገት ላኪ ሁኔታ ቆጣሪዎች ጋር እኩልነት። |
| `android.telemetry.redaction.failure` | ቆጣሪ | `signal_id`, `reason` | የለም | ምንም ማሻሻያ የለም; 30 ቀናት ይቆዩ | Rust `streaming_privacy_redaction_fail_total`ን ለማንፀባረቅ ያስፈልጋል። |
| `android.telemetry.device_profile` | መለኪያ | `profile_id`፣ `sdk_level`፣ `hardware_tier` | የመሣሪያ ዲበ ውሂብ | ሸካራ ባልዲዎች (ኤስዲኬ ሜጀር፣ የሃርድዌር ደረጃ) አስወጡ። 30 ቀናት ይቆዩ | የኦሪጂናል ዕቃ አምራች ዝርዝሮችን ሳያጋልጥ የተመጣጣኝነት ዳሽቦርዶችን ያነቃል። |
| `android.telemetry.network_context` | ክስተት | `network_type`, `roaming` | ተሸካሚ PII | ሊሆን ይችላል። `carrier_name` ሙሉ በሙሉ ጣል; ሌሎች መስኮችን ለ 7 ቀናት ያቆዩ | `ClientConfig.networkContextProvider` መተግበሪያዎች የተመዝጋቢ ውሂብን ሳያጋልጡ የአውታረ መረብ አይነት + ዝውውርን እንዲያሰራጩ የጸዳ ቅጽበታዊ ገጽ እይታን ያቀርባል። የፓርቲ ዳሽቦርዶች ምልክቱን እንደ ሞባይል አናሎግ ለ Rust I18NI0000069X አድርገው ይመለከቱታል። |
| `android.telemetry.config.reload` | ክስተት | `source`፣ `result`፣ `duration_ms` | የለም | ምንም ማሻሻያ የለም; 30 ቀናት ይቆዩ | መስተዋቶች የዝገት ውቅር ድጋሚ የመጫን ክፍተቶች። |
| `android.telemetry.chaos.scenario` | ክስተት | `scenario_id`፣ `outcome`፣ `duration_ms`፣ `device_profile` | የመሣሪያ መገለጫ በባልዲ ተጭኗል | ልክ እንደ `device_profile`; 30 ቀናት ይቆዩ | ለብአዴን 7 ዝግጁነት በሚያስፈልገው ትርምስ ልምምዶች ወቅት ተመዝግቧል። |
| `android.telemetry.redaction.salt_version` | መለኪያ | `salt_epoch`, `rotation_id` | የለም | ምንም ማሻሻያ የለም; 365 ቀናት መቆየት | የ Blake2b የጨው ሽክርክሪት ይከታተላል; የአንድሮይድ ሃሽ ዘመን ከዝገት ኖዶች ሲለያይ ተመሳሳይ ማንቂያ። |
| `android.crash.report.capture` | ክስተት | `crash_id`፣ `signal`፣ `process_state`፣ `has_native_trace`፣ `anr_watchdog_bucket` | የብልሽት አሻራ + ሂደት ሜታዳታ | Hash `crash_id` ከተጋራው የማሻሻያ ጨው, ባልዲ ጠባቂ ሁኔታ, ወደ ውጭ ከመላኩ በፊት የተቆለሉ ፍሬሞችን ይጥሉ; 30 ቀናት ይቆዩ | I18NI0000090X ሲጠራ በራስ ሰር ነቅቷል፤ መሣሪያን የሚለዩ ምልክቶችን ሳያጋልጥ የፓርቲ ዳሽቦርዶችን ይመገባል። |
| `android.crash.report.upload` | ቆጣሪ | `crash_id`፣ `backend`፣ `status`፣ `retry_count` | የብልሽት አሻራ | Hashed I18NI0000096X እንደገና ጥቅም ላይ ማዋል፣ የመልቀቂያ ሁኔታን ብቻ; 30 ቀናት ይቆዩ | በ`ClientConfig.crashTelemetryReporter()` ወይም I18NI0000098X አስወጣ ስለዚህ ሰቀላዎች እንደሌሎች ቴሌሜትሪ ተመሳሳይ Sigstore/OLTP ዋስትናዎችን ያካፍላሉ። |

### የትግበራ መንጠቆዎች

- `ClientConfig` አሁን ክሮች ከማንፀባረቅ የተገኘ የቴሌሜትሪ መረጃን በ በኩል ነው።
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`፣ በራስ ሰር በመመዝገብ ላይ
  `TelemetryObserver` በጣም የተጠለፉ ባለስልጣናት እና የጨው መለኪያዎች ያለምንም ተመልካቾች ይፈስሳሉ።
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` ይመልከቱ
  እና ተጓዳኝ ክፍሎች በታች
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- ማመልከቻዎች መደወል ይችላሉ
  `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` ለመመዝገብ
  በማንፀባረቅ ላይ የተመሰረተ `AndroidNetworkContextProvider`፣ እሱም `ConnectivityManager` በሂደት ጊዜ ይጠይቃል
  እና የተጠናቀረ አንድሮይድ ሳያስተዋውቅ የ`android.telemetry.network_context` ክስተትን ይለቃል።
  ጥገኝነቶች.
- ክፍል `TelemetryOptionsTests` እና I18NI0000110X ሙከራዎች
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) hashing ን ይጠብቁ
  ረዳቶች እና የClientConfig ውህደት መንጠቆ ስለዚህ መመለሻዎች ወዲያውኑ ይገለጣሉ።
- የነቃ ኪት/ላብራቶሪዎች ይህን ሰነድ በመያዝ ከሐሰት ኮድ ይልቅ የኮንክሪት ኤፒአይዎችን ይጠቅሳሉ
  Runbook ከመርከብ ኤስዲኬ ጋር የተስተካከለ።

> ** የክወናዎች ማስታወሻ፡** ባለቤቱ/ሁኔታ ሉህ ይኖራል
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` እና መሆን አለበት።
> በእያንዳንዱ የ AND7 የፍተሻ ቦታ ከዚህ ሠንጠረዥ ጎን ተዘምኗል።

## የፓሪቲ የፈቃድ ዝርዝሮች እና schema-diff የስራ ፍሰት

አንድሮይድ ወደ ውጭ መላክ መቼም ለዪዎች እንዳይወጣ አስተዳደር ባለሁለት ፍቃድ ዝርዝር ያስፈልገዋል
ያ ዝገት አገልግሎት ሆን ተብሎ ብቅ ይላል። ይህ ክፍል የሩጫ መጽሐፍ መግቢያን ያንጸባርቃል
(`docs/source/android_runbook.md` §2.3) ግን የ AND7 ማሻሻያ ዕቅድን ይጠብቃል
እራሱን የቻለ.

| ምድብ | አንድሮይድ ላኪዎች | ዝገት አገልግሎቶች | ማረጋገጫ መንጠቆ |
|-------------|-----------
| ባለስልጣን / መንገድ አውድ | Hash `authority`/`alias` በ Blake2b-256 እና ወደ ውጭ ከመላኩ በፊት ጥሬ Torii የአስተናጋጅ ስሞችን ይጥሉ; የጨው መዞርን ለማረጋገጥ `android.telemetry.redaction.salt_version` መልቀቅ። | ለግንኙነት ሙሉ የTorii የአስተናጋጅ ስሞች እና የአቻ መታወቂያዎችን አምጡ። | `android.torii.http.request` vs `torii.http.request` ግቤቶችን በቅርብ ጊዜ በ `docs/source/sdk/android/readiness/schema_diffs/` ስር ያወዳድሩ እና የጨው ዘመንን ለማረጋገጥ `scripts/telemetry/check_redaction_status.py`ን ያሂዱ። |
| መሳሪያ እና የፈራሚ ማንነት | ባልዲ `hardware_tier`/`device_profile`፣ የሃሽ ተቆጣጣሪ ተለዋጭ ስሞች፣ እና የመለያ ቁጥሮችን በጭራሽ ወደ ውጪ መላክ። | ኢሚት አረጋጋጭ `peer_id`፣ መቆጣጠሪያ `public_key`፣ እና ወረፋ ሃሽ በቃል። | ከ`docs/source/sdk/mobile_device_profile_alignment.md` ጋር አሰልፍ፣ በ`java/iroha_android/run_tests.sh` ውስጥ የአካል ብቃት እንቅስቃሴ ተለዋጭ ስም hashing ሙከራዎች እና በቤተ ሙከራ ጊዜ የወረፋ መርማሪ ውጤቶች። |
| የአውታረ መረብ ሜታዳታ | `network_type` + `roaming` ብቻ ይላኩ; ጣል `carrier_name`. | የአቻ አስተናጋጅ ስም/TLS የመጨረሻ ነጥብ ሜታዳታ ያቆዩ። | እያንዳንዱን የመርሃግብር ልዩነት በ`readiness/schema_diffs/` ውስጥ ያከማቹ እና የGrafana "Network Context" መግብር ተሸካሚ ሕብረቁምፊዎችን ካሳየ ያስጠነቅቁ። |
| መሻር / ትርምስ ማስረጃ | Emit `android.telemetry.redaction.override`/I18NI0000132X ጭምብል ከተሸፈኑ የተዋናይ ሚናዎች ጋር። | ያልተሸፈኑ የመሻር ማጽደቆችን ያመርቱ; ልዩ ትርምስ የለም። | ማስመሰያዎችን + የተመሰቃቀለ ቅርሶችን ከማያሸሸጉት የዝገት ዝግጅቶች ጎን መቀመጡን ለማረጋገጥ ከልምምዶች በኋላ `docs/source/sdk/android/readiness/and7_operator_enablement.md`ን አቋርጡ። |

የስራ ፍሰት፡

1. ከእያንዳንዱ አንጸባራቂ/ ላኪ ለውጥ በኋላ አሂድ
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` እና JSON ን በ `docs/source/sdk/android/readiness/schema_diffs/` ስር አስቀምጠው።
2. ከላይ ካለው ሰንጠረዥ አንጻር ያለውን ልዩነት ይከልሱ። አንድሮይድ ዝገት-ብቻ መስክን ከለቀቀ
   (ወይም በተገላቢጦሽ)፣ የ AND7 ዝግጁነት ስህተት ያስገቡ እና ሁለቱንም ይህንን እቅድ እና የ
   runbook.
3. በሳምንታዊ የኦፕስ ግምገማዎች ወቅት, ያስፈጽሙ
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   እና የጨው ዘመንን እና schema-diff የጊዜ ማህተምን ዝግጁነት ሉህ ውስጥ ያስገቡ።
4. በ `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` ውስጥ ማናቸውንም ልዩነቶች ይመዝግቡ
   ስለዚህ የአስተዳደር እሽጎች እኩል ውሳኔዎችን ይይዛሉ.

> ** የመርሃግብር ማመሳከሪያ፡** ቀኖናዊ መስክ መለያዎች የሚመነጩት ከ ነው።
> `android_telemetry_redaction.proto` (በአንድሮይድ ኤስዲኬ ግንባታ ወቅት ቁስ አካል የተደረገ
> ከ Norito ገላጭዎች ጋር)። መርሃግብሩ `authority_hash`ን ያጋልጣል፣
> `alias_label`፣ `attestation_digest`፣ `device_brand_bucket`፣ እና
> `actor_role_masked` መስኮች አሁን በመላው ኤስዲኬ እና ቴሌሜትሪ ላኪዎች ጥቅም ላይ ይውላሉ።

`authority_hash` የተመዘገበው የI18NT0000015X ባለስልጣን ዋጋ ቋሚ ባለ 32-ባይት መፍጨት ነው።
በፕሮቶ ውስጥ. `attestation_digest` ቀኖናዊውን የማረጋገጫ መግለጫ ይይዛል
የጣት አሻራ፣ `device_brand_bucket` ጥሬውን የአንድሮይድ የምርት ስም ሕብረቁምፊ በካርታው ላይ ሲያስቀምጥ
የጸደቀው ኢነም (I18NI0000147X፣ `oem`፣ `enterprise`)። `actor_role_masked` ይሸከማል
በ
ጥሬ የተጠቃሚ መለያ።

### የብልሽት ቴሌሜትሪ ወደ ውጭ መላክ አሰላለፍየብልሽት ቴሌሜትሪ አሁን ተመሳሳይ የOpenTelemetry ላኪዎችን እና ፕሮቬንሽን ይጋራል።
የቧንቧ መስመር እንደ I18NT0000016X የኔትወርክ ምልክቶች, የአስተዳደር ክትትልን ይዘጋዋል.
የተባዙ ላኪዎች. የብልሽት ተቆጣጣሪው `android.crash.report.capture` ይመገባል።
ክስተት ከተጠለፈ I18NI0000155X (Blake2b-256 ቀድሞውኑ የመቀየሪያውን ጨው በመጠቀም
ክትትል በ `android.telemetry.redaction.salt_version`) ፣ በሂደት-ግዛት ባልዲዎች ፣
እና የጸዳ የኤኤንአር ጠባቂ ዲበ ውሂብ። ቁልል ዱካዎች በመሣሪያው ላይ ይቀራሉ እና ብቻ ናቸው።
ከዚህ በፊት ወደ `has_native_trace` እና `anr_watchdog_bucket` መስኮች ተጠቃሏል
ወደ ውጭ መላክ ስለዚህ ምንም PII ወይም OEM ሕብረቁምፊዎች መሣሪያውን አይተዉም.

ብልሽትን መስቀል የ `android.crash.report.upload` ቆጣሪ ግቤት ይፈጥራል፣
ስለእሱ ምንም ሳይማር SRE የጀርባ አስተማማኝነትን ኦዲት እንዲያደርግ መፍቀድ
ተጠቃሚ ወይም ቁልል ዱካ. ሁለቱም ምልክቶች Torii ላኪውን እንደገና ስለሚጠቀሙ ይወርሳሉ
ተመሳሳዩ Sigstore ፊርማ፣ ማቆየት ፖሊሲ እና ማንቂያ መንጠቆዎች አስቀድሞ ተገልጸዋል
ለብአዴን7። የድጋፍ runbooks ስለዚህ የሃሽድ ብልሽት መለያን ማዛመድ ይችላሉ።
ያለ የብልሽት ቧንቧ መስመር በአንድሮይድ እና Rust ማስረጃዎች መካከል።

ተቆጣጣሪውን በI18NI0000160X አንድ ጊዜ ያንቁት
የቴሌሜትሪ አማራጮች እና ማጠቢያዎች ተዋቅረዋል; የብልሽት ሰቀላ ድልድዮች እንደገና ጥቅም ላይ ሊውሉ ይችላሉ።
`ClientConfig.crashTelemetryReporter()` (ወይም `CrashTelemetryHandler.recordUpload`)
በተመሳሳዩ የተፈረመ የቧንቧ መስመር የጀርባ ውጤቶችን ለማስለቀቅ.

## ፖሊሲ ዴልታስ vs ዝገት ቤዝላይን።

በአንድሮይድ እና Rust telemetry ፖሊሲዎች መካከል ያሉ ልዩነቶች ከመቀነሱ እርምጃዎች ጋር።

| ምድብ | ዝገት ቤዝላይን | አንድሮይድ ፖሊሲ | ቅነሳ / ማረጋገጫ |
|------------------|
| ባለስልጣን / የአቻ መለያዎች | ግልጽ ባለስልጣን ሕብረቁምፊዎች | `authority_hash` (Blake2b-256, የተሽከረከረ ጨው) | በ `iroha_config.telemetry.redaction_salt` በኩል የታተመ የጋራ ጨው; እኩልነት ፈተና ለድጋፍ ሰጪ ሰራተኞች ሊቀለበስ የሚችል የካርታ ስራን ያረጋግጣል። |
| አስተናጋጅ / የአውታረ መረብ ሜታዳታ | መስቀለኛ አስተናጋጅ ስሞች/አይፒዎች ወደ ውጭ ተልከዋል | የአውታረ መረብ አይነት + ሮሚንግ ብቻ | የአውታረ መረብ ጤና ዳሽቦርዶች ከአስተናጋጅ ስሞች ይልቅ የተገኝነት ምድቦችን ለመጠቀም ተዘምነዋል። |
| የመሣሪያ ባህሪያት | N/A (አገልጋይ-ጎን) | የተከተተ መገለጫ (ኤስዲኬ 21/23/29+፣ ደረጃ `emulator`/`consumer`/I18NI0000167X) | ትርምስ ልምምዶች ባልዲ ካርታን ያረጋግጣሉ; በጣም ጥሩ ዝርዝር ሲያስፈልግ runbook ሰነዶችን የመጨመር መንገድን ይደግፉ። |
| ማሻሻያ ይሽራል | አይደገፍም | በNorito መዝገብ (`actor_role_masked`፣ `reason`) ውስጥ የተከማቸ በእጅ መሻር ማስመሰያ | መሻር የተፈረመ ጥያቄ ያስፈልገዋል; የኦዲት መዝገብ ለ 1 ዓመት ተይዟል. |
| የማረጋገጫ አሻራዎች | የአገልጋይ ማረጋገጫ በSRE ብቻ | ኤስዲኬ የጸዳ የማረጋገጫ ማጠቃለያ ያወጣል። የዝገት ማረጋገጫ አረጋጋጭ ላይ የማረጋገጫ ሀሾችን አቋርጥ። ሃሽድ ተለዋጭ ስም መፍሰስን ይከላከላል። |

የማረጋገጫ ዝርዝር፡

- የማሻሻያ አሃድ ከዚህ በፊት ለያንዳንዱ ምልክት ሃሽድ/ጭምብል የተደረገባቸው መስኮችን ያረጋግጣል
  ላኪ ማቅረብ.
- የ Schema diff መሳሪያ (ከ Rust nodes ጋር የተጋራ) የመስክ እኩልነትን ለማረጋገጥ በምሽት ያካሂዳል።
- ትርምስ የመለማመጃ ስክሪፕት ልምምዶች የስራ ሂደትን ይሽረዋል እና የኦዲት ምዝገባን ያረጋግጣል።

## የትግበራ ተግባራት (ቅድመ-SRE አስተዳደር)

1. **የኢንቬንቶሪ ማረጋገጫ** - ከላይ ካለው አንድሮይድ ኤስዲኬ ጋር አቋራጭ ያረጋግጡ
   የመሳሪያዎች መንጠቆዎች እና I18NT0000006X የመርሃግብር ትርጓሜዎች። ባለቤቶች: አንድሮይድ
   ታዛቢነት TL፣ LLM
2. **የቴሌሜትሪ ንድፍ ልዩነት** — የተጋራውን የልዩነት መሣሪያ ከሩስት መለኪያዎች ጋር ያሂዱ።
   ለ SRE ግምገማ ተመሳሳይ ቅርሶችን ማምረት። ባለቤት፡ የSRE ግላዊነት መሪ።
3. ** Runbook ረቂቅ (የተጠናቀቀው 2026-02-03)** — `docs/source/android_runbook.md`
   አሁን ከጫፍ እስከ ጫፍ የመሻር የስራ ሂደትን (ክፍል 3) እና የተስፋፋውን ሰነድ ዘግቧል
   escalation ማትሪክስ እና ሚና ኃላፊነቶች (ክፍል3.1)፣ CLI ን በማያያዝ
   ወደ አስተዳደር ፖሊሲው የሚመለሱ ረዳቶች፣ የክስተቶች ማስረጃዎች እና ትርምስ ስክሪፕቶች።
   ባለቤቶች፡ LLM ከሰነዶች/ድጋፍ አርትዖት ጋር።
4. **የማስቻል ይዘት** — አጭር መግለጫ ስላይዶችን፣ የላብራቶሪ መመሪያዎችን እና
   ለፌብሩዋሪ 2026 ክፍለ ጊዜ የእውቀት-ፈትሽ ጥያቄዎች። ባለቤቶች፡ ሰነዶች/ድጋፍ
   ሥራ አስኪያጅ፣ የኤስአርኢ ማስቻል ቡድን።

## የስራ ፍሰት እና Runbook መንጠቆዎችን ማንቃት

### 1. የአካባቢ + CI ጭስ ሽፋን

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` የ Torii ማጠሪያን ያሽከረክራል ፣ ቀኖናዊውን ባለብዙ ምንጭ SoraFS መጋጠሚያ (ለ`ci/check_sorafs_orchestrator_adoption.sh` ውክልና) እና ዘሮች ሰው ሰራሽ አንድሮይድ ቴሌሜትሪ ይደግማል።
  - የትራፊክ ማመንጨት በ`scripts/telemetry/generate_android_load.py` ነው የሚስተናገደው፣ይህም በ`artifacts/android/telemetry/load-generator.log` ስር የጥያቄ/ምላሽ ግልባጭ ይመዘግባል እና ራስጌዎችን፣ ዱካ መሻሮችን ወይም ደረቅ አሂድ ሁነታን ያከብራል።
  - ረዳቱ SoraFS የውጤት ሰሌዳ/ማጠቃለያ ወደ `${WORKDIR}/sorafs/` ገልብጧል ስለዚህ AND7 ልምምዶች የሞባይል ደንበኞችን ከመንካት በፊት የብዝሃ-ምንጭ እኩልነትን ያረጋግጣሉ።
- CI ተመሳሳዩን መሣሪያ እንደገና ይጠቀማል፡- `ci/check_android_dashboard_parity.sh` `scripts/telemetry/compare_dashboards.py`ን ከ `dashboards/grafana/android_telemetry_overview.json`፣ የ Rust ማጣቀሻ ዳሽቦርድ እና የአበል ፋይሉን በ`dashboards/data/android_rust_dashboard_allowances.json` ላይ ያካሂዳል፣የተፈረመውን ልዩነት ቅጽበተ-ፎቶ I181800
- ትርምስ ልምምዶች `docs/source/sdk/android/telemetry_chaos_checklist.md` ይከተላሉ; የናሙና-ኤንቪ ስክሪፕት እና የዳሽቦርድ እኩልነት ማረጋገጫ የብአዴን7ን የተቃጠለ ኦዲት የሚመግብ “ዝግጁ” የማስረጃ ጥቅል ይመሰርታሉ።

### 2. የመስጠት እና የኦዲት መንገድን መሻር

- `scripts/android_override_tool.py` የማሻሻያ ማሻሻያዎችን ለማውጣት እና ለመሻር ቀኖናዊው CLI ነው። `apply` የተፈረመ ጥያቄን ያስገባል፣የዝርዝር ጥቅሉን ያወጣል (በነባሪ `telemetry_redaction_override.to`) እና የተጠለፈ ማስመሰያ ረድፍ ወደ `docs/source/sdk/android/telemetry_override_log.md` ጨምሯል። `revoke` የስረዛ ጊዜ ማህተሙን በዚያው ረድፍ ላይ ያስቀምጠዋል፣ እና `digest` ለአስተዳደር የሚያስፈልገውን የጸዳ JSON ቅጽበተ ፎቶ ይጽፋል።
- በ`docs/source/android_support_playbook.md` ውስጥ ከተያዘው የተገዢነት መስፈርት ጋር የሚዛመድ የማርክዳው ሠንጠረዥ ራስጌ እስካልተገኘ ድረስ CLI የኦዲት ምዝግብ ማስታወሻውን ለማሻሻል ፈቃደኛ አይሆንም። በ `scripts/tests/test_android_override_tool_cli.py` ውስጥ ያለው የአሃድ ሽፋን የጠረጴዛ ተንታኝን፣ አንጸባራቂ አስመጪዎችን እና የስህተት አያያዝን ይከላከላል።
- ኦፕሬተሮች የመነጨውን አንጸባራቂ፣ የዘመነ የምዝግብ ማስታወሻ ክፍልን እና ** እና** ማሟያውን JSON በ `docs/source/sdk/android/readiness/override_logs/` በማንኛውም ጊዜ መሻር በሚደረግበት ጊዜ ያያይዙታል። ምዝግብ ማስታወሻው በዚህ እቅድ ውስጥ በተሰጠው የአስተዳደር ውሳኔ መሰረት የ 365 ቀናት ታሪክን ይይዛል.

### 3. ማስረጃ መያዝ እና ማቆየት።

- እያንዳንዱ ልምምድ ወይም ክስተት የሚከተሉትን የያዘ በ`artifacts/android/telemetry/` የተዋቀረ ጥቅል ያወጣል:
  - የሎድ-ጄነሬተር ግልባጭ እና ድምር ቆጣሪዎች ከ `generate_android_load.py`።
  - የዳሽቦርድ እኩልነት ልዩነት (`android_vs_rust-<stamp>.json`) እና አበል ሃሽ በ`ci/check_android_dashboard_parity.sh` የወጣ።
  - ሎግ ዴልታ (መሻር ከተሰጠ)፣ ተዛማጅ አንጸባራቂውን እና የታደሰውን JSONን ይሽሩ።
- የኤስአርአይ ማቃጠል ሪፖርት እነዚያን ቅርሶች እና በ`android_sample_env.sh` የተቀዳውን SoraFS የውጤት ሰሌዳ ይጠቅሳል፣ ለ AND7 ዝግጁነት ግምገማ ከቴሌሜትሪ ሃሽ → ዳሽቦርዶች → የመሻር ሁኔታን ይሰጣል።

## ክሮስ-ኤስዲኬ የመሣሪያ መገለጫ አሰላለፍ

ዳሽቦርዶች የአንድሮይድ `hardware_tier` ወደ ቀኖናዊው ይተረጉማሉ
`mobile_profile_class` የተገለፀው በ
`docs/source/sdk/mobile_device_profile_alignment.md` so AND7 እና IOS7 ቴሌሜትሪ
ተመሳሳይ ቡድኖችን ማወዳደር

- `lab` - እንደ `hardware_tier = emulator` የተለቀቀ ፣ ከስዊፍት ጋር የሚዛመድ
  `device_profile_bucket = simulator`.
- `consumer` — እንደ `hardware_tier = consumer` የተለቀቀ (ከኤስዲኬ-ዋና ቅጥያ ጋር)
  እና ከስዊፍት `iphone_small`/`iphone_large`/`ipad` ባልዲዎች ጋር ተቧድኗል።
- `enterprise` - እንደ `hardware_tier = enterprise` የተለቀቀ፣ ከስዊፍት ጋር የሚስማማ
  `mac_catalyst` ባልዲ እና ወደፊት የሚተዳደር/iOS ዴስክቶፕ አሂድ ጊዜዎች።

ማንኛውም አዲስ እርከን ወደ አሰላለፍ ሰነድ እና የንድፍ ልዩነት ቅርሶች መታከል አለበት።
ዳሽቦርዶች ከመብላታቸው በፊት።

## አስተዳደር እና ስርጭት

- **ቅድመ-የተነበበ ጥቅል** — ይህ ሰነድ እና አባሪ ቅርሶች (የእቅድ ልዩነት፣
  runbook diff፣ ዝግጁነት ዴክ ዝርዝር) ለ SRE አስተዳደር ይሰራጫል።
  የደብዳቤ መላኪያ ዝርዝር ከ **2026-02-05** ያልበለጠ።
- **የመልስ ምልልስ *** - በአስተዳደር ጊዜ የሚሰበሰቡ አስተያየቶች ወደ ውስጥ ይገባሉ።
  `AND7` JIRA epic; ማገጃዎች በI18NI0000211X እና በአንድሮይድ በየሳምንቱ ይታያሉ
  የቁም ማስታወሻዎች.
- ** ማተም *** - አንዴ ከጸደቀ፣ የመመሪያው ማጠቃለያ ከ ይገናኛል።
  `docs/source/android_support_playbook.md` እና በተጋራው ተጠቅሷል
  ቴሌሜትሪ FAQ በ I18NI0000213X።

## የኦዲት እና ተገዢነት ማስታወሻዎች

- ፖሊሲ የሞባይል ተመዝጋቢ ውሂብን በማስወገድ የGDPR/CCPA መስፈርቶችን ያከብራል።
  ወደ ውጭ ከመላክ በፊት; የሃሺድ ባለስልጣን ጨው በየሩብ ዓመቱ ይሽከረከራል እና ይከማቻል
  የጋራ ሚስጥሮች ማከማቻ ።
- የማንቃት ቅርሶች እና የ runbook ዝመናዎች በማክበር መዝገብ ውስጥ ገብተዋል።
- የሩብ ዓመት ግምገማዎች መሻር የተዘጋ-loop መቆየታቸውን ያረጋግጣሉ (ያረጀ መዳረሻ የለም)።

## የአስተዳደር ውጤት (2026-02-12)

በ*2026-02-12** የነበረው የSRE አስተዳደር ክፍለ ጊዜ የአንድሮይድ ማሻሻያ አጽድቋል
ፖሊሲ ያለ ማሻሻያ. ቁልፍ ውሳኔዎች (ተመልከት
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **የመመሪያ ተቀባይነት።** የሃሽድ ባለስልጣን፣ የመሣሪያ-መገለጫ ባልዲ እና የ
  የአገልግሎት አቅራቢ ስም መቅረት ጸድቋል። የጨው ሽክርክሪት መከታተያ በ በኩል
  `android.telemetry.redaction.salt_version` የሩብ ዓመት የኦዲት ንጥል ይሆናል።
- ** የማረጋገጫ እቅድ።
  የሩብ ዓመት ትርምስ ልምምዶች ተቀባይነት አግኝተዋል። የእርምጃ ንጥል፡ ዳሽቦርድ ያትሙ
  ከእያንዳንዱ ልምምድ በኋላ የፓርቲ ሪፖርት.
- ** አስተዳደርን ይሽሩ።** Norito የተቀዳ መሻሪያ ማስመሰያዎች ጸድቀዋል
  የ 365 ቀናት ማቆያ መስኮት. የድጋፍ ምህንድስና የመሻሪያ ምዝግብ ማስታወሻ ባለቤት ይሆናል።
  በወርሃዊ ክንዋኔዎች ማመሳሰል ወቅት ግምገማ።

## የመከታተያ ሁኔታ

1. **የመሣሪያ-መገለጫ አሰላለፍ (በ2026-03-01 መጨረሻ)።** ✅ ተጠናቀቀ - የተጋራው
   በ `docs/source/sdk/mobile_device_profile_alignment.md` ውስጥ ካርታ መሥራት እንዴት እንደሆነ ይገልጻል
   የአንድሮይድ `hardware_tier` እሴቶች ወደ ቀኖናዊው `mobile_profile_class`
   በተመጣጣኝ ዳሽቦርዶች እና በ schema diff tooling የሚበላ።

## መጪ የኤስአርአይ አስተዳደር አጭር መግለጫ (Q22026)የመንገድ ካርታ ንጥል **AND7** የሚቀጥለው SRE አስተዳደር ክፍለ ጊዜ ሀ
አጭር የአንድሮይድ ቴሌሜትሪ ማስተካከያ ቅድመ-ንባብ። ይህንን ክፍል እንደ ህያው ይጠቀሙ
አጭር; ከእያንዳንዱ የምክር ቤት ስብሰባ በፊት ማዘመን።

### የዝግጅት ዝርዝር

1. **የማስረጃ ጥቅል** — የቅርብ ጊዜውን የሼማ ልዩነት፣ ዳሽቦርድ ቅጽበታዊ ገጽ እይታዎችን ወደ ውጭ መላክ፣
   እና የምዝግብ ማስታወሻዎችን ይሽሩ (ከታች ያለውን ማትሪክስ ይመልከቱ) እና በቀኑ ስር ያስቀምጧቸው
   አቃፊ (ለምሳሌ
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) በፊት
   ግብዣውን በመላክ ላይ።
2. **የቁፋሮ ማጠቃለያ** — በጣም የቅርብ ጊዜውን ትርምስ የመልመጃ ምዝግብ ማስታወሻ እና የ
   `android.telemetry.redaction.failure` ሜትሪክ ቅጽበታዊ እይታ; Alertmanager ያረጋግጡ
   ማብራሪያዎች ተመሳሳይ የጊዜ ማህተም ይጠቅሳሉ።
3. ** ኦዲትን ይሽሩ *** - ሁሉም ንቁ መሻሮች በNorito ውስጥ መመዝገባቸውን ያረጋግጡ።
   መዝገብ ቤት እና በስብሰባው ወለል ውስጥ ተጠቃሏል. የማለቂያ ቀናትን እና የ
   ተዛማጅ የክስተት መታወቂያዎች።
4. **የአጀንዳ ማስታወሻ *** - የ SRE ሊቀመንበር ፒንግ ከ 48 ሰዓታት በፊት ከስብሰባው በፊት
   አጭር ማገናኛ፣ የሚፈለጉትን ማንኛውንም ውሳኔዎች ማድመቅ (አዲስ ምልክቶች፣ ማቆየት።
   ለውጦች ወይም የፖሊሲ ዝመናዎችን ይሽሩ)።

### ማስረጃ ማትሪክስ

| Artefact | አካባቢ | ባለቤት | ማስታወሻ |
|-------------|-------|------|
| Schema diff vs ዝገት | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | ቴሌሜትሪ መሣሪያ DRI | ከመገናኘቱ በፊት <72 ሰአታት በፊት መፈጠር አለበት። |
| ዳሽቦርድ diff ቅጽበታዊ ገጽ እይታዎች | `docs/source/sdk/android/readiness/dashboards/<date>/` | ታዛቢነት TL | `sorafs.fetch.*`፣ I18NI0000224X፣ እና Alertmanager ቅጽበተ-ፎቶዎችን ያካትቱ። |
| መፈጨትን ይሽሩ | `docs/source/sdk/android/readiness/override_logs/<date>.json` | የምህንድስና ድጋፍ | `scripts/android_override_tool.sh digest` (በዚያ ማውጫ ውስጥ README ይመልከቱ) ከአዲሱ `telemetry_override_log.md` ጋር ያሂዱ; ቶከኖች ከማጋራትዎ በፊት hashed ይቀራሉ። |
| ትርምስ የመለማመጃ መዝገብ | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA አውቶማቲክ | የKPI ማጠቃለያን ያያይዙ (የድንኳን ብዛት፣ ጥምርታ ጥምርታ፣ አጠቃቀምን መሻር)። |

### ለምክር ቤቱ ክፍት ጥያቄዎች

- የመሻር ማቆያ መስኮቱን ከ 365 ቀናት አሁን ማሳጠር አለብን?
  የምግብ መፍጫው አውቶማቲክ ነው?
- `android.telemetry.device_profile` አዲሱን የተጋራ መሆን አለበት።
  `mobile_profile_class` መለያዎች በሚቀጥለው ልቀት ላይ፣ ወይም Swift/JS ይጠብቁ
  ኤስዲኬዎች ተመሳሳይ ለውጥ ለመላክ?
- ለክልላዊ መረጃ ነዋሪነት ተጨማሪ መመሪያ አንድ ጊዜ Torii ያስፈልጋል
  Norito-RPC ክስተቶች በአንድሮይድ ላይ ያርፋሉ (NRPC-3 ክትትል)?

### የቴሌሜትሪ እቅድ ልዩነት አሰራር

schema diff መሳሪያውን ቢያንስ አንድ ጊዜ በእጩ እጩ ያሂዱ (እና በማንኛውም ጊዜ አንድሮይድ
የመሳሪያ ለውጦች) ስለዚህ የኤስአርአይ ካውንስል ከ ጋር አብሮ ትኩስ ተመሳሳይነት ያላቸውን ቅርሶች ይቀበላል
ዳሽቦርድ ልዩነት፡-

1. ለማነጻጸር የሚፈልጉትን የአንድሮይድ እና የሩስት ቴሌሜትሪ ንድፎችን ወደ ውጭ ይላኩ። ለ CI አወቃቀሮች በቀጥታ
   በ `configs/android_telemetry.json` እና `configs/rust_telemetry.json` ስር።
2. `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json` አስፈጽም.
   - በአማራጭ ማለፍ (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) ወደ
     ውቅሮቹን በቀጥታ ከ git ይጎትቱ; ስክሪፕቱ በሥነ ጥበብ ውስጥ ያሉትን hashes ይሰኩት።
3. የተፈጠረውን JSON ከዝግጁነት ጥቅል ጋር ያያይዙትና ከ`status.md` + ያገናኙት።
   `docs/source/telemetry.md`. ልዩነቱ የተጨመሩ/የተወገዱ መስኮችን እና የማቆየት ዴልታዎችን ያደምቃል
   ኦዲተሮች መሣሪያውን እንደገና ሳይጫወቱ የመቀያየር እኩልነትን ማረጋገጥ ይችላሉ።
4. ልዩነቱ የሚፈቀደውን ልዩነት ሲያሳይ (ለምሳሌ፣ የአንድሮይድ-ብቻ መሻሪያ ምልክቶች)፣
   የአበል ፋይል በ`ci/check_android_dashboard_parity.sh` ተጠቅሷል እና በ ውስጥ ያለውን ምክንያት ልብ ይበሉ
   የ schema-diff ማውጫ README.

> ** የማህደር ህጎች፡** አምስቱን የቅርብ ጊዜ ልዩነቶችን አቆይ
> `docs/source/sdk/android/readiness/schema_diffs/` እና የቆዩ ቅጽበተ-ፎቶዎችን ወደ ውሰድ
> `artifacts/android/telemetry/schema_diffs/` ስለዚህ የአስተዳደር ገምጋሚዎች ሁልጊዜ የቅርብ ጊዜውን ውሂብ ያያሉ።