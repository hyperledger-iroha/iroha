---
lang: am
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T15:38:09.507154+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ ኤስዲኬ ኦፕሬሽንስ Runbook

ይህ Runbook አንድሮይድ ኤስዲኬን የሚያስተዳድሩ ኦፕሬተሮችን እና ድጋፍ ሰጪ መሐንዲሶችን ይደግፋል
ለብአዴን7 እና ከዚያ በላይ ማሰማራት። ለ SLA አንድሮይድ ድጋፍ Playbook ጋር ያጣምሩ
ትርጓሜዎች እና የመጨመር መንገዶች.

> **ማስታወሻ፡** የአደጋ ሂደቶችን ሲያዘምኑ፣ የተጋሩትንም ያድሱ
> መላ መፈለግ ማትሪክስ (`docs/source/sdk/android/troubleshooting.md`) ስለዚህ የ
> የትዕይንት ሠንጠረዥ፣ SLAs እና የቴሌሜትሪ ማጣቀሻዎች ከዚህ runbook ጋር አብረው ይቆያሉ።

## 0. Quickstart (ፔጄሮች ሲቃጠሉ)

ወደ ዝርዝር ውስጥ ከመግባትዎ በፊት ይህን ቅደም ተከተል ለSev1/Sev2 ማንቂያዎች ምቹ ያድርጉት
ከታች ያሉት ክፍሎች፡-

1. ** ገባሪውን አወቃቀሩን ያረጋግጡ፡** የ`ClientConfig` አንጸባራቂ ቼክ ድምርን ያንሱ
   በመተግበሪያ ጅምር ላይ የተለቀቀ እና ከተሰካው አንጸባራቂ ጋር ያወዳድሩ
   `configs/android_client_manifest.json`. ሃሽ ከተለያየ፣ ልቀቶችን ያቁሙ እና
   ቴሌሜትሪ/መሻርን ከመንካትዎ በፊት የማዋቀር ትኬት ያስገቡ (§1 ይመልከቱ)።
2. ** የመርሃግብር ልዩነት በርን ያሂዱ፡** `telemetry-schema-diff` CLIን በተቃራኒው ያስፈጽሙ
   ተቀባይነት ያለው ቅጽበታዊ ገጽ እይታ
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`)።
   ማንኛውንም የ`policy_violations` ውፅዓት እንደ Sev2 ያዙ እና ወደ ውጭ መላክን እስከ እግድ ያግዱ
   ልዩነት ተረድቷል (§2.6 ይመልከቱ)።
3. ** ዳሽቦርዶችን + ሁኔታ CLI ይመልከቱ:** የአንድሮይድ ቴሌሜትሪ ማሻሻያ እና ይክፈቱ
   ላኪ የጤና ሰሌዳዎች፣ ከዚያ አሂድ
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. ከሆነ
   ባለስልጣናት ከወለሉ በታች ናቸው ወይም ወደ ውጭ የመላክ ስህተት፣ ቅጽበታዊ ገጽ እይታዎችን ያንሱ እና
   ለክስተቱ ሰነድ የCLI ውጤት (§2.4–§2.5 ይመልከቱ)።
4. ** መሻርን ይወስኑ፡** ከላይ ከተጠቀሱት እርምጃዎች በኋላ ብቻ እና ከአደጋ/ባለቤቱ ጋር
   ተመዝግቧል፣ በ`scripts/android_override_tool.sh` በኩል የተወሰነ መሻር አውጣ
   እና `telemetry_override_log.md` ያስገቡ (§3 ይመልከቱ)። ነባሪ የማብቂያ ጊዜ፡ <24ሰ.
5. **በየዕውቂያ ዝርዝር ጨምር፡** የአንድሮይድ ጥሪ ላይ እና ታዛቢነት TL ገፅ
   (በ §8 ውስጥ ያሉ እውቂያዎች)፣ ከዚያም በ§4.1 ውስጥ ያለውን የማሳደግ ዛፍ ይከተሉ። ምስክር ከሆነ ወይም
   StrongBox ምልክቶች ይሳተፋሉ፣ የቅርብ ጊዜውን ጥቅል ይጎትቱ እና መታጠቂያውን ያሂዱ
   ወደ ውጭ መላክን እንደገና ከማንቃትዎ በፊት ከ§7 ቼኮች።

## 1. ማዋቀር እና ማሰማራት

- ** ClientConfig ምንጭ፡** የአንድሮይድ ደንበኞች Torii የመጨረሻ ነጥብ፣ TLS መጫኑን ያረጋግጡ።
  ፖሊሲዎች፣ እና ከ`iroha_config`-የመነጩ መገለጫዎች ቁልፎችን እንደገና ይሞክሩ። አረጋግጥ
  በመተግበሪያ ጅምር ጊዜ ዋጋዎች እና የነቃ አንጸባራቂ የምዝግብ ማስታወሻ።
  የትግበራ ማጣቀሻ: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  ክሮች `TelemetryOptions` ከ `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (ከመነጨው `TelemetryObserver` በተጨማሪ) ስለዚህ የተጠለፉ ባለስልጣናት በራስ-ሰር ይለቀቃሉ።
- ** ትኩስ ዳግም መጫን:** `iroha_config` ን ለመውሰድ የውቅረት መመልከቻውን ይጠቀሙ
  ያለ መተግበሪያ እንደገና ይጀምራል። ያልተሳኩ ድጋሚ ጭነቶች መልቀቅ አለባቸው
  `android.telemetry.config.reload` ክስተት እና ቀስቅሴ እንደገና መሞከር በትርፍ
  ወደኋላ መመለስ (ቢበዛ 5 ሙከራዎች)።
- ** የመመለስ ባህሪ፡** ውቅረት ሲጎድል ወይም የማይሰራ ከሆነ፣ ወደዚህ ይመለሱ
  ደህንነቱ የተጠበቀ ነባሪዎች (ማንበብ-ብቻ ሁነታ፣ ምንም በመጠባበቅ ላይ ያለ ወረፋ ማስረከብ) እና ተጠቃሚን በማሳየት ላይ
  የሚል ጥያቄ አቅርቧል። ክስተቱን ለክትትል ይመዝግቡ።

### 1.1 የዳግም ጭነት ምርመራዎችን ያዋቅሩ- የማዋቀሪያው ጠባቂ `android.telemetry.config.reload` ምልክቶችን ያስወጣል።
  `source`፣ `result`፣ `duration_ms`፣ እና አማራጭ `digest`/`error` መስኮች (ይመልከቱ።
  `configs/android_telemetry.json` እና
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  በተግባራዊ አንጸባራቂ አንድ ነጠላ `result:"success"` ክስተት ይጠብቁ; ተደግሟል
  የ`result:"error"` መዝገቦች ተመልካቹ 5 የኋሊት ሙከራዎችን እንዳሟጠጠ ያመለክታሉ።
  ከ 50ms ጀምሮ.
- በአደጋ ጊዜ የቅርብ ጊዜውን የመጫን ምልክት ከአሰባሳቢው ይያዙ
  (OTLP/span ማከማቻ ወይም የማሻሻያ ሁኔታ መጨረሻ ነጥብ) እና `digest` + ይመዝገቡ
  `source` በተፈጠረው ክስተት ዶክ. የምግብ መፍጫውን ከ ጋር ያወዳድሩ
  `configs/android_client_manifest.json` እና የተለቀቀው አንጸባራቂ ተሰራጭቷል።
  ኦፕሬተሮች.
- ተመልካቹ ስህተቶችን ማውጣቱን ከቀጠለ ለማባዛት የታለመውን መታጠቂያ ያሂዱ
  ከተጠርጣሪው አንጸባራቂ ጋር የመተንተን አለመሳካቱ፡-
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  የሙከራ ውጤቱን እና ያልተሳካውን አንጸባራቂ ከክስተቱ ጥቅል ጋር ያያይዙ ስለዚህ SRE
  ከተጠበሰ የውቅር ንድፍ ጋር ሊለያይ ይችላል።
- ቴሌሜትሪ እንደገና መጫን ሲጠፋ፣ ገባሪውን `ClientConfig` መያዙን ያረጋግጡ
  የቴሌሜትሪ ማጠቢያ እና የ OTLP ሰብሳቢ አሁንም ይቀበላል
  `android.telemetry.config.reload` መታወቂያ; አለበለዚያ እንደ Sev2 ቴሌሜትሪ ይያዙት
  ሪግሬሽን (እንደ §2.4 ተመሳሳይ መንገድ) እና ምልክቱ እስኪመለስ ድረስ ልቀቶችን ለአፍታ ያቁሙ።

### 1.2 ቆራጥ ቁልፍ ወደ ውጭ የሚላኩ ጥቅሎች
- የሶፍትዌር ኤክስፖርት አሁን v3 ቅርቅቦችን ወደ ውጭ መላክ ጨው + ኖንስ፣ `kdf_kind` እና `kdf_work_factor` ያወጣል።
  ላኪው አርጎን2ይድ (64 ሚቢ፣ 3 ድግግሞሽ፣ ትይዩ = 2) ይመርጣል እና ወደ
  PBKDF2-HMAC-SHA256 ከ350 ኪ ተደጋጋሚ ወለል ጋር Argon2id በመሳሪያው ላይ በማይገኝበት ጊዜ። ጥቅል
  AAD አሁንም ከተለዋጭ ስም ጋር ይያያዛል; የይለፍ ሐረጎች ለv3 ወደ ውጭ ለሚላኩ እና የ. ቢያንስ 12 ቁምፊዎች መሆን አለባቸው
  አስመጪ ሁሉንም-ዜሮ ጨው/የማይገኝ ዘር ውድቅ ያደርጋል።
  `KeyExportBundle.decode(Base64|bytes)`፣ ከዋናው የይለፍ ሐረግ አስመጣ እና ወደ v3 እንደገና ላክ
  ወደ ማህደረ ትውስታ-ጠንካራ ቅርጸት ይሂዱ. አስመጪው ሙሉ በሙሉ ዜሮ ወይም እንደገና ጥቅም ላይ የዋለ ጨው/ያልሆኑ ጥንዶችን ውድቅ ያደርጋል። ሁልጊዜ
  በመሣሪያዎች መካከል የቆዩ ወደ ውጭ የሚላኩ ምርቶችን እንደገና ከመጠቀም ይልቅ ጥቅሎችን ማሽከርከር።
- በ `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests` ውስጥ አሉታዊ-መንገድ ሙከራዎች
  አለመቀበል። ከተጠቀሙ በኋላ የይለፍ ሐረግ ቻር ድርድሮችን ያጽዱ እና ሁለቱንም የጥቅል ስሪት እና `kdf_kind` ይያዙ
  መልሶ ማግኘቱ ሳይሳካ ሲቀር በአጋጣሚ ማስታወሻዎች.

## 2. ቴሌሜትሪ እና ማሻሻያ

> ፈጣን ማጣቀሻ: ተመልከት
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> በማንቃት ጊዜ ጥቅም ላይ የዋለው ለተጨመቀው የትእዛዝ/የመግቢያ ማረጋገጫ ዝርዝር
> ክፍለ-ጊዜዎች እና የአደጋ ድልድዮች።- ** የምልክት ክምችት፡** ወደ `docs/source/sdk/android/telemetry_redaction.md` ያጣቅሱ
  ለሙሉ የተለቀቁ ስፋቶች/ሜትሪክስ/ክስተቶች ዝርዝር እና
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  ለባለቤት/ማረጋገጫ ዝርዝሮች እና አስደናቂ ክፍተቶች።
- ** ቀኖናዊ schema diff: ** የጸደቀው AND7 ቅጽበተ ፎቶ ነው።
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  ገምጋሚዎች ማየት እንዲችሉ እያንዳንዱ አዲስ የCLI ሩጫ ከዚህ ስነ-ጥበብ ጋር መወዳደር አለበት።
  ተቀባይነት ያለው `intentional_differences` እና `android_only_signals` አሁንም
  ከተመዘገቡት የመመሪያ ሰንጠረዦች ጋር ይዛመዳል
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. CLI አሁን ይጨምራል
  ማንኛውም ሆን ተብሎ የተደረገ ልዩነት ሲጠፋ `policy_violations` ሀ
  `status:"accepted"`/`"policy_allowlisted"` (ወይም የአንድሮይድ-ብቻ መዝገቦች ሲጠፉ
  ተቀባይነት ያለው ደረጃቸው)፣ ስለዚህ ባዶ ያልሆኑ ጥሰቶችን እንደ Sev2 ይያዙ እና ያቁሙ
  ወደ ውጭ መላክ ። ከታች ያሉት የ`jq` ቅንጥቦች በማህደር የተቀመጠ የንጽህና ማረጋገጫ ሆነው ይቀራሉ
  ቅርሶች፡-
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  ከእነዚህ ትእዛዞች የሚገኘውን ማንኛውንም ውፅዓት ሀ የሚያስፈልገው እንደ ሼማ ሪግሬሽን ይያዙ
  የቴሌሜትሪ ኤክስፖርት ከመቀጠሉ በፊት AND7 ዝግጁነት ሳንካ; `field_mismatches`
  በ `telemetry_schema_diff.md` §5 ባዶ መቆየት አለበት። ረዳቱ አሁን ይጽፋል
  `artifacts/android/telemetry/schema_diff.prom` በራስ-ሰር; ማለፍ
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (ወይም አዘጋጅ
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) በመድረክ/በምርት አስተናጋጆች ላይ ሲሰራ
  ስለዚህ የ `telemetry_schema_diff_run_status` መለኪያ ወደ `policy_violation` ይገለበጣል
  CLI ተንሳፋፊነትን ካወቀ በራስ-ሰር።
- ** CLI አጋዥ: ** `scripts/telemetry/check_redaction_status.py` ይመረምራል
  `artifacts/android/telemetry/status.json` በነባሪ; `--status-url` ወደ
  የአካባቢያዊ ቅጂውን ከመስመር ውጭ ለማደስ እና `--write-cache`
  ልምምዶች. `--min-hashed 214` ይጠቀሙ (ወይም አዘጋጅ
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) አስተዳደርን ለማስፈጸም
  በእያንዳንዱ የሁኔታ የሕዝብ አስተያየት ወቅት በተከለከሉ ባለስልጣናት ላይ ወለል።
- ** ስልጣንን ማጥመድ፡** ሁሉም ባለስልጣኖች Blake2b-256ን በመጠቀም ተጠልፈዋል
  በአስተማማኝ ሚስጥሮች ማከማቻ ውስጥ የተከማቸ የሩብ አመት ሽክርክሪት ጨው. ሽክርክሪቶች በ ላይ ይከሰታሉ
  በእያንዳንዱ ሩብ የመጀመሪያ ሰኞ በ 00:00 UTC. ላኪው መወሰዱን ያረጋግጡ
  አዲሱን ጨው የ `android.telemetry.redaction.salt_version` መለኪያን በመፈተሽ.
- **የመሳሪያ መገለጫ ባልዲዎች፡** `emulator`፣ `consumer`፣ እና `enterprise` ብቻ
  ደረጃዎች ወደ ውጭ ይላካሉ (ከኤስዲኬ ዋና ስሪት ጋር)። ዳሽቦርዶች እነዚህን ያወዳድራሉ
  ከ Rust baselines ጋር ይቆጥራል; ልዩነት> 10% ማንቂያዎችን ያነሳል.
- **የአውታረ መረብ ሜታዳታ፡** አንድሮይድ `network_type` እና `roaming` ባንዲራዎችን ብቻ ወደ ውጭ ይላካል።
  ተሸካሚ ስሞች በጭራሽ አይለቀቁም; ኦፕሬተሮች ተመዝጋቢውን መጠየቅ የለባቸውም
  በክስተቶች ምዝግብ ማስታወሻዎች ውስጥ መረጃ. የጸዳው ቅጽበታዊ ገጽ እይታ እንደ የተለቀቀው ነው።
  `android.telemetry.network_context` ክስተት፣ ስለዚህ መተግበሪያዎች መመዝገባቸውን ያረጋግጡ ሀ
  `NetworkContextProvider` (በወይ
  `ClientConfig.Builder.setNetworkContextProvider(...)` ወይም ምቾቱ
  `enableAndroidNetworkContext(...)` አጋዥ) ከTorii ጥሪዎች ከመድረሳቸው በፊት።
** Grafana ጠቋሚ፡** `Android Telemetry Redaction` ዳሽቦርድ
  ከላይ ላለው የCLI ውፅዓት ቀኖናዊ ምስላዊ ፍተሻ - አረጋግጥ
  `android.telemetry.redaction.salt_version` ፓነል አሁን ካለው የጨው ዘመን ጋር ይዛመዳል
  እና የ `android_telemetry_override_tokens_active` መግብር በዜሮ ላይ ይቆያል
  ምንም ልምምዶች ወይም ክስተቶች በማይሄዱበት ጊዜ። የትኛውም ፓነል ከተንሳፈፈ ከፍ አድርግ
  የ CLI ስክሪፕቶች ወደ ኋላ መመለስን ከመዘገባቸው በፊት።

### 2.1 የቧንቧ መስመር የስራ ፍሰት ወደ ውጭ ይላኩ1. ** ማከፋፈያ ማዋቀር።** `ClientConfig.telemetry.redaction` በክር ተሰርዟል ከ
   `iroha_config` እና ትኩስ-እንደገና በ `ConfigWatcher` ተጭኗል። እያንዳንዱ ዳግም መጫን ምዝግብ ማስታወሻዎች
   አንጸባራቂ መፍጨት እና የጨው ዘመን - በአደጋዎች እና ጊዜ ውስጥ ያንን መስመር ይያዙ
   ልምምዶች.
2. **መሳሪያ።** የኤስዲኬ ክፍሎች ርዝመቶችን/ሜትሪክቶችን/ክስተቶችን ወደ
   `TelemetryBuffer`. መያዣው እያንዳንዱን ጭነት በመሳሪያው መገለጫ እና መለያ ይሰጣል
   አሁን ያለው የጨው ዘመን ስለዚህ ላኪው የሃሺንግ ግብዓቶችን በቆራጥነት ማረጋገጥ ይችላል።
3. ** የማሻሻያ ማጣሪያ።** `RedactionFilter` hashes `authority`፣ `alias`፣ እና
   መሳሪያውን ከመውጣታቸው በፊት የመሳሪያ መለያዎች. አለመሳካቶች ይለቃሉ
   `android.telemetry.redaction.failure` እና ወደ ውጭ የመላክ ሙከራን አግድ።
4. ** ላኪ + ሰብሳቢ።** የንጽህና መጠበቂያ ጭነቶች በአንድሮይድ በኩል ይላካሉ
   ክፍት ቴሌሜትሪ ላኪ ወደ `android-otel-collector` ማሰማራት። የ
   ሰብሳቢ ደጋፊዎች ወደ ዱካዎች (ቴምፖ)፣ መለኪያዎች (Prometheus) እና Norito ያወጣሉ።
   የምዝግብ ማስታወሻዎች.
5. ** ታዛቢነት መንጠቆዎች።** `scripts/telemetry/check_redaction_status.py` ያነባል።
   ሰብሳቢ ቆጣሪዎች (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) እና የሁኔታ ቅርቅቡን ያዘጋጃል።
   በዚህ የሩጫ መጽሐፍ ውስጥ ተጠቅሷል።

### 2.2 የማረጋገጫ በሮች

- ** የመርሃግብር ልዩነት:** ሩጫ
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  በሚገለጽበት ጊዜ ሁሉ ለውጥ. ከእያንዳንዱ ሩጫ በኋላ, እያንዳንዱን ያረጋግጡ
  `intentional_differences[*]` እና `android_only_signals[*]` ግቤት ማህተም ተደርጓል
  `status:"accepted"` (ወይም `status:"policy_allowlisted"` ለሃሽድ/ባልዲ)
  መስኮች) ከማያያዝዎ በፊት በ `telemetry_schema_diff.md` §3 ውስጥ እንደሚመከር
  ለአደጋዎች እና ሁከት ላብራቶሪ ሪፖርቶች artefact. የተፈቀደውን ቅጽበታዊ ገጽ እይታ ተጠቀም
  (`android_vs_rust-20260305.json`) እንደ መከላከያ ሀዲድ እና አዲስ የሚወጣውን ንጣፍ
  JSON ከመመዝገቡ በፊት፡-
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  `$LATEST` ጋር ያወዳድሩ
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  የተፈቀደ ዝርዝሩ ሳይለወጥ መቆየቱን ለማረጋገጥ። የጠፋ ወይም ባዶ `status`
  ግቤቶች (ለምሳሌ በ `android.telemetry.redaction.failure` ወይም
  `android.telemetry.redaction.salt_version`) አሁን እንደ ሪግሬሽን እና
  ግምገማው ከመዘጋቱ በፊት መፈታት አለበት; CLI የተቀበለውን ወለል ያሳያል
  በቀጥታ ይግለጹ፣ ስለዚህ መመሪያው §3.4 ተሻጋሪ ማጣቀሻ የሚተገበረው መቼ ነው።
  `accepted` ያልሆነ ሁኔታ ለምን እንደሚታይ በማብራራት።

  ** ቀኖናዊ AND7 ምልክቶች (2026-03-05 ቅጽበተ-ፎቶ)**| ሲግናል | ቻናል | ሁኔታ | የአስተዳደር ማስታወሻ | ማረጋገጫ መንጠቆ |
  |--------|--------|--------|
  | `android.telemetry.redaction.override` | ክስተት | `accepted` | መስተዋቶች መገለጫዎችን ይሽራሉ እና ከ`telemetry_override_log.md` ግቤቶች ጋር መዛመድ አለባቸው። | `android_telemetry_override_tokens_active` ይመልከቱ እና የማህደር መግለጫዎችን በ§3። |
  | `android.telemetry.network_context` | ክስተት | `accepted` | አንድሮይድ ሆን ብሎ የአገልግሎት አቅራቢ ስሞችን ያስተካክላል; ወደ ውጭ የሚላኩት `network_type` እና `roaming` ብቻ ናቸው። | መተግበሪያዎች `NetworkContextProvider` መመዝገባቸውን ያረጋግጡ እና የዝግጅቱ መጠን Torii በ`Android Telemetry Overview` ላይ ያለውን ትራፊክ መከተሉን ያረጋግጡ። |
  | `android.telemetry.redaction.failure` | ቆጣሪ | `accepted` | ሀሺንግ ካልተሳካ ይወጣል; አስተዳደር አሁን በ schema diff artefact ውስጥ ግልጽ የሆነ የሁኔታ ሜታዳታ ያስፈልገዋል። | የ`Redaction Compliance` ዳሽቦርድ ፓኔል እና የCLI ውፅዓት ከ`check_redaction_status.py` በልምምድ ጊዜ ካልሆነ በስተቀር በዜሮ መቆየት አለባቸው። |
  | `android.telemetry.redaction.salt_version` | መለኪያ | `accepted` | ላኪው አሁን ያለውን የሩብ ወር የጨው ዘመን እየተጠቀመ መሆኑን ያረጋግጣል። | የGrafana የጨው መግብርን ከሚስጥር-ቮልት ዘመን ጋር ያወዳድሩ እና የሼማ ልዩነት የ`status:"accepted"` ማብራሪያን እንደያዘ ያረጋግጡ። |

  ከላይ ባለው ሠንጠረዥ ውስጥ ያለ ማንኛውም ግቤት `status` ን ከጣለ ልዩነቱ አርቲፊሻል መሆን አለበት
  የታደሰው ** እና *** `telemetry_schema_diff.md` ከ AND7 በፊት ዘምኗል
  የአስተዳደር ፓኬት ተሰራጭቷል። የታደሰውን JSON በ ውስጥ ያካትቱ
  `docs/source/sdk/android/readiness/schema_diffs/` እና ከ
  ዳግም መጀመሩን የቀሰቀሰው ክስተት፣ ትርምስ ቤተ ሙከራ ወይም የማስቻል ሪፖርት።
- ** CI / ክፍል ሽፋን: ** `ci/run_android_tests.sh` ከዚህ በፊት ማለፍ አለበት
  የሕትመት ግንባታዎች; ስዊቱ የአካል ብቃት እንቅስቃሴ በማድረግ ሃሺንግ/መሻር ባህሪን ያስገድዳል
  የቴሌሜትሪ ላኪዎች ከናሙና ጭነት ጋር።
- ** ኢንጀክተር ንጽህና ፍተሻዎች: ** ይጠቀሙ
  ከልምምድ በፊት `scripts/telemetry/inject_redaction_failure.sh --dry-run`
  አለመሳካቱን ለማረጋገጥ መርፌው እንደሚሰራ እና ጠባቂዎችን በሚጥሉበት ጊዜ እሳትን ያስታውቃል
  ተሰናክለዋል ። ሁል ጊዜ መርፌውን አንዴ ከተረጋገጠ በ`--clear` ያፅዱ
  ያጠናቅቃል.

### 2.3 ሞባይል ↔ Rust telemetry perity checklist

የ አንድሮይድ ላኪዎችን እና የዝገት መስቀለኛ መንገድ አገልግሎቶችን በማክበር እንዲሰለፉ ያድርጉ
የተለያዩ የማሻሻያ መስፈርቶች ተመዝግበው ይገኛሉ
`docs/source/sdk/android/telemetry_redaction.md`. ከታች ያለው ሰንጠረዥ እንደ
በ AND7 የመንገድ ካርታ ግቤት ውስጥ የተጠቀሰው ባለሁለት ፍቃድ ዝርዝር - በማንኛውም ጊዜ ያዘምኑት።
schema diff መስኮችን ያስተዋውቃል ወይም ያስወግዳል።| ምድብ | አንድሮይድ ላኪዎች | ዝገት አገልግሎቶች | ማረጋገጫ መንጠቆ |
|-------------|-----------
| ባለስልጣን / መንገድ አውድ | Hash `authority`/`alias` በ Blake2b-256 እና ወደ ውጭ ከመላኩ በፊት ጥሬ Torii የአስተናጋጅ ስሞችን ይጥሉ; የጨው መዞርን ለማረጋገጥ `android.telemetry.redaction.salt_version` መልቀቅ። | ለግንኙነት ሙሉ የTorii የአስተናጋጅ ስሞችን እና የአቻ መታወቂያዎችን አምጡ። | `android.torii.http.request` vs `torii.http.request` ግቤቶችን በቅርብ ጊዜ በ `readiness/schema_diffs/` ስር ያወዳድሩ እና `android.telemetry.redaction.salt_version` `scripts/telemetry/check_redaction_status.py`ን በማሄድ ከክላስተር ጨው ጋር ይዛመዳል። |
| መሳሪያ እና የፈራሚ ማንነት | ባልዲ `hardware_tier`/`device_profile`፣ የሃሽ ተቆጣጣሪ ተለዋጭ ስሞች፣ እና የመለያ ቁጥሮችን በጭራሽ ወደ ውጪ መላክ። | ምንም የመሣሪያ ዲበ ውሂብ የለም; አንጓዎች የሚያመነጭ `peer_id` እና መቆጣጠሪያ `public_key` በቃል። | በ`docs/source/sdk/mobile_device_profile_alignment.md` ውስጥ ያሉትን ካርታዎች ያንጸባርቁ፣ የ`PendingQueueInspector` ውጤቶቹን በቤተ ሙከራ ጊዜ ኦዲት ያድርጉ እና በ`ci/run_android_tests.sh` ውስጥ ያሉ ተለዋጭ የሃሽ ሙከራዎች አረንጓዴ መሆናቸውን ያረጋግጡ። |
| የአውታረ መረብ ሜታዳታ | `network_type` + `roaming` booleans ብቻ ወደ ውጭ ይላኩ; `carrier_name` ተጥሏል። | ዝገት የአቻ አስተናጋጅ ስሞችን እና ሙሉ የTLS የመጨረሻ ነጥብ ዲበ ውሂብን ይይዛል። | የቅርብ ጊዜውን ልዩነት JSON በ`readiness/schema_diffs/` ውስጥ ያከማቹ እና የአንድሮይድ ጎን አሁንም `carrier_name` እንደተወ ያረጋግጡ። የGrafana's "Network Context" መግብር ማናቸውንም የድምጸ ተያያዥ ሞደም ሕብረቁምፊዎች ካሳየ አስጠንቅቅ። |
| መሻር / ትርምስ ማስረጃ | Emit `android.telemetry.redaction.override` እና `android.telemetry.chaos.scenario` ክስተቶች ጭምብል ከተሸፈኑ የተዋንያን ሚናዎች ጋር። | የዝገት አገልግሎቶች ያለ ሚና መሸፈኛ እና ልዩ ውዥንብር ሳይኖር ማጽደቆችን ያስወጣሉ። | ቶከኖች እና ምስቅልቅል ቅርሶች መሻራቸውን ለማረጋገጥ ከእያንዳንዱ ልምምድ በኋላ `docs/source/sdk/android/readiness/and7_operator_enablement.md` ን አቋርጥ ቼክ ካላደረጉት የዝገት ሁነቶች ጋር። |

የተመጣጣኝ የስራ ሂደት;

1. ከእያንዳንዱ አንጸባራቂ ወይም ላኪ ለውጥ በኋላ፣ አሂድ
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   ስለዚህ የJSON አርቴፋክት እና የተንጸባረቀው ሜትሪክስ ሁለቱም በማስረጃ ጥቅል ውስጥ ይገኛሉ
   (ረዳቱ አሁንም በነባሪነት `artifacts/android/telemetry/schema_diff.prom` ይጽፋል).
2. ከላይ ካለው ሰንጠረዥ ጋር ያለውን ልዩነት ይከልሱ; አንድሮይድ አሁን መስክ ከለቀቀ
   ዝገት ላይ ብቻ ነው የሚፈቀደው (ወይም በተቃራኒው)፣ የ AND7 ዝግጁነት ሳንካ ያስገቡ እና ያዘምኑ
   የማሻሻያ ዕቅድ.
3. በየሳምንቱ ቼኮች, ሩጫ
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   የጨው ዘመን ከGrafana መግብር ጋር የሚጣጣም መሆኑን ለማረጋገጥ እና በዘመኑ ያለውን ዘመን ልብ ይበሉ።
   የጥሪ መጽሔት.
4. ማንኛውንም ዴልታ ወደ ውስጥ ይመዝግቡ
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` እንዲሁ
   አስተዳደር የተመጣጣኝነት ውሳኔዎችን መመርመር ይችላል።

### 2.4 ታዛቢነት ዳሽቦርዶች እና የማንቂያ ገደቦች

ዳሽቦርዶችን እና ማንቂያዎችን ከ AND7 schema diff ማጽደቂያዎች ጋር ያቆዩ
`scripts/telemetry/check_redaction_status.py` ውፅዓት መገምገም፡-

- `Android Telemetry Redaction` - የጨው ዘመን መግብር፣ የማስመሰያ መለኪያን መሻር።
- `Redaction Compliance` — `android.telemetry.redaction.failure` ቆጣሪ እና
  injector አዝማሚያ ፓነሎች.
- `Exporter Health` — `android.telemetry.export.status` ተመን ብልሽቶች።
- `Android Telemetry Overview` - የመሳሪያ መገለጫ ባልዲዎች እና የአውታረ መረብ አውድ መጠን።

የሚከተሉት ገደቦች የፈጣን ማጣቀሻ ካርዱን ያንፀባርቃሉ እና መተግበር አለባቸው
በአጋጣሚ ምላሽ እና ልምምድ ወቅት፡-| ሜትሪክ / ፓነል | ገደብ | ድርጊት |
|------------|-------|
| `android.telemetry.redaction.failure` (`Redaction Compliance` ሰሌዳ) | > 0 በሚንከባለል 15 ደቂቃ መስኮት ላይ | ያልተሳካ ሲግናልን መርምር፣ ኢንጀክተር ንፁህ አሂድ፣ የ CLI ውፅዓት + Grafana ቅጽበታዊ ገጽ እይታን ይመዝግቡ። |
| `android.telemetry.redaction.salt_version` (`Android Telemetry Redaction` ሰሌዳ) | ከሚስጥር ይለያል-የጨው ዘመን | ልቀቶችን አቁም፣ ከሚስጥር መሽከርከር ጋር አስተባባሪ፣ የ AND7 ማስታወሻ ፋይል ያድርጉ። |
| `android.telemetry.export.status{status="error"}` (`Exporter Health` ሰሌዳ) | > 1% የወጪ ንግድ | የሰብሳቢ ጤናን ይመርምሩ፣ የCLI ምርመራዎችን ይያዙ፣ ወደ SRE ከፍ ይበሉ። |
| `android.telemetry.device_profile{tier="enterprise"}` vs Rust parity (`Android Telemetry Overview`) | ልዩነት>10% ከዝገት መነሻ መስመር | የፋይል አስተዳደር ክትትል፣ የቋሚ ገንዳዎችን አረጋግጥ፣ schema diff artefact ማብራሪያ። |
| `android.telemetry.network_context` ጥራዝ (`Android Telemetry Overview`) | Torii ትራፊክ እያለ ወደ ዜሮ ይወርዳል | የ`NetworkContextProvider` ምዝገባን አረጋግጥ፣ መስኮች አለመለወጣቸውን ለማረጋገጥ የሼማ ልዩነትን እንደገና አስጀምር። |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | ከፀደቀ ውጪ ዜሮ ያልሆነ የመሻሪያ/መሰርሰሪያ መስኮት | ከአንድ ክስተት ጋር ማስመሰያ ማሰር፣ መፈጨትን እንደገና ማመንጨት፣ በ§3 ውስጥ በስራ ሂደት መሻር። |

### 2.5 የኦፕሬተር ዝግጁነት እና የማስቻል ዱካ

የRoadmap ንጥል AND7 ራሱን የቻለ ኦፕሬተር ሥርዓተ ትምህርትን ይጠራል ስለዚህ ይደግፉ፣ SRE እና
የመልቀቅ ባለድርሻ አካላት runbook ከመሄዱ በፊት ከላይ ያሉትን እኩልነት ሰንጠረዦች ይረዳሉ
ጂኤ. ዝርዝሩን በ ውስጥ ይጠቀሙ
`docs/source/sdk/android/telemetry_readiness_outline.md` ለቀኖናዊ ሎጅስቲክስ
(አጀንዳ፣ አቅራቢዎች፣ የጊዜ መስመር) እና `docs/source/sdk/android/readiness/and7_operator_enablement.md`
ለዝርዝር ማረጋገጫ ዝርዝር፣ የማስረጃ ማገናኛዎች እና የድርጊት ምዝግብ ማስታወሻ። የሚከተለውን ያስቀምጡ
የቴሌሜትሪ ዕቅዱ በሚቀየርበት ጊዜ የሚመሳሰሉ ደረጃዎች፡-| ደረጃ | መግለጫ | ማስረጃ ጥቅል | ዋና ባለቤት |
|-------------
| አስቀድሞ የተነበበ ስርጭት | የመመሪያውን ቅድመ-የተነበበ `telemetry_redaction.md` እና ፈጣን ማመሳከሪያ ካርዱን ቢያንስ ከአምስት የስራ ቀናት በፊት ይላኩ። ምስጋናዎችን በዝርዝሩ comms ምዝግብ ማስታወሻ ውስጥ ይከታተሉ። | `docs/source/sdk/android/telemetry_readiness_outline.md` (Session Logistics + Communications Log) እና በ `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` ውስጥ የተመዘገበው ኢሜል። | ሰነዶች/ድጋፍ አስተዳዳሪ |
| የቀጥታ ዝግጁነት ክፍለ ጊዜ | የ60 ደቂቃ ስልጠናውን (የፖሊሲ ጥልቅ ዳይቭ፣ runbook walkthrough፣ dashboards፣ chaos lab demo) ያቅርቡ እና ቀረጻው ለተመሳሳይ ተመልካቾች እንዲሄድ ያድርጉ። | በ `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` ስር የተከማቹ ስላይዶች ቀረጻ በ§2 ከተያዙ ማጣቀሻዎች ጋር። | LLM (የብአዴን7 ባለቤት) |
| ትርምስ ቤተ ሙከራ አፈጻጸም | ከቀጥታ ክፍለ ጊዜ በኋላ ወዲያውኑ ቢያንስ C2 (መሻር) + C6 (የወረፋ መልሶ ማጫወት) ከ `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` ያሂዱ እና ምዝግብ ማስታወሻዎችን/ስክሪፕቶችን ከማስቻል ኪት ጋር ያያይዙ። | የScenario ሪፖርቶች እና ቅጽበታዊ ገጽ እይታዎች በ`docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` እና `/screenshots/<YYYY-MM>/`። | አንድሮይድ ታዛቢነት TL + SRE በጥሪ ላይ |
| የእውቀት ማረጋገጫ እና ተገኝነት | የጥያቄ ማቅረቢያዎችን ይሰብስቡ፣ ማንኛውም ሰው <90% ያስመዘገበውን ያስተካክሉ እና የመገኘት/የጥያቄ ስታቲስቲክስን ይመዝግቡ። ፈጣን የማመሳከሪያ ጥያቄዎች ከተመጣጣኝ ማረጋገጫ ዝርዝሩ ጋር እንዲጣጣሙ ያድርጉ። | ጥያቄዎችን ወደ ውጭ በመላክ `docs/source/sdk/android/readiness/forms/responses/`፣ ማጠቃለያ Markdown/JSON በ`scripts/telemetry/generate_and7_quiz_summary.py` የተሰራ፣ እና በ`and7_operator_enablement.md` ውስጥ ያለው የመገኘት ጠረጴዛ። | የምህንድስና ድጋፍ |
| ማህደር እና ክትትል | የማነቃቂያ ኪት የድርጊት ምዝግብ ማስታወሻን ያዘምኑ፣ ቅርሶችን ወደ ማህደሩ ይስቀሉ እና በ`status.md` ውስጥ መጠናቀቁን ልብ ይበሉ። በክፍለ-ጊዜው የተሰጠ ማሻሻያ ወይም መሻር ወደ `telemetry_override_log.md` መቅዳት አለበት። | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (የድርጊት መዝገብ)፣ `.../archive/<YYYY-MM>/checklist.md`፣ እና የተሻረው ምዝግብ በ§3 ውስጥ ተጠቅሷል። | LLM (የብአዴን7 ባለቤት) |

ሥርዓተ ትምህርቱ እንደገና ሲካሄድ (በሩብ ወይም ከዋና ዋና ንድፍ ለውጦች በፊት) ያድሱ
ዝርዝሩ ከአዲሱ ክፍለ ጊዜ ጋር፣ የተሳታፊውን ዝርዝር ወቅታዊ ያድርጉት፣ እና
የአስተዳደር እሽጎች እንዲችሉ የጥያቄ ማጠቃለያ JSON/Markdown ቅርሶችን እንደገና ማደስ
ወጥነት ያለው ማስረጃ ማጣቀስ። የ AND7 `status.md` ግቤት ከ ጋር ማገናኘት አለበት።
እያንዳንዱ የማነቃቂያ sprint ከተዘጋ በኋላ የቅርብ ጊዜ ማህደር።

### 2.6 የመርሃግብር ልዩነት ፈቃዶች እና የፖሊሲ ማረጋገጫዎች

ፍኖተ ካርታው ባለሁለት ፍቀድ ፖሊሲን በግልፅ ይጠራል (የሞባይል ማሻሻያ vs
ዝገት ማቆየት) በ `telemetry-schema-diff` CLI የሚተገበረው ስር
`tools/telemetry-schema-diff`. እያንዳንዱ ልዩ ልዩ ቅርስ ተመዝግቧል
`docs/source/sdk/android/readiness/schema_diffs/` የትኞቹ መስኮች እንደሆኑ መመዝገብ አለበት።
በአንድሮይድ ላይ hashed/bucketed፣የትኞቹ መስኮች ዝገት ላይ ሳይነኩ እንደቀሩ፣ እና እንደሆነ
ማንኛውም ያልተፈቀደ ምልክት ወደ ግንባታው ገባ። እነዚያን ውሳኔዎች ይያዙ
በመሮጥ በቀጥታ በJSON ውስጥ፦

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```የመጨረሻው `jq` ሪፖርቱ ንፁህ በሆነበት ጊዜ ወደ ምንም-op ይገመግማል። ማንኛውንም ውጤት ማከም
ከዚያ ትእዛዝ እንደ Sev2 ዝግጁነት ሳንካ፡ በህዝብ የተሞላ `policy_violations`
ድርድር ማለት CLI በአንድሮይድ-ብቻ ዝርዝር ውስጥ የሌለ ምልክት አግኝቷል ማለት ነው።
ወይም ዝገት-ብቻ ነፃ የመውጣት ዝርዝር ላይ በሰነድ ውስጥ
`docs/source/sdk/android/telemetry_schema_diff.md`. ይህ ሲከሰት ያቁሙ
ወደ ውጪ መላክ፣ የ AND7 ትኬት አስገባ እና ልዩነቱን ከፖሊሲው ሞጁል በኋላ ብቻ እንደገና አስጀምር
እና አንጸባራቂ ቅጽበተ-ፎቶዎች ተስተካክለዋል። የተገኘውን JSON ወደ ውስጥ ያከማቹ
`docs/source/sdk/android/readiness/schema_diffs/` ከቀን ቅጥያ እና ማስታወሻ ጋር
በአደጋው ውስጥ ያለው መንገድ ወይም የላብራቶሪ ሪፖርት ስለዚህ አስተዳደር ቼኮችን እንደገና ማጫወት ይችላል።

** ሃሺንግ እና ማቆየት ማትሪክስ ***

| ሲግናል.መስክ | አንድሮይድ አያያዝ | ዝገት አያያዝ | የተፈቀደ ዝርዝር መለያ |
|-------------|----------------------|-----------|
| `torii.http.request.authority` | Blake2b-256 hashed (`representation: "blake2b_256"`) | ለመከታተል በቃላት የተከማቸ | `policy_allowlisted` (ሞባይል ሃሽ) |
| `attestation.result.alias` | Blake2b-256 hashed | ግልጽ የጽሑፍ ቅጽል (የማስረጃ መዛግብት) | `policy_allowlisted` |
| `attestation.result.device_tier` | ባልዲ (`representation: "bucketed"`) | ተራ ደረጃ ሕብረቁምፊ | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | የለም - አንድሮይድ ላኪዎች መስኩን ሙሉ በሙሉ ጣሉ | ያለ ማሻሻያ ያቅርቡ | `rust_only` (የ `telemetry_schema_diff.md` §3 ውስጥ የተመዘገበ) |
| `android.telemetry.redaction.override.*` | ጭንብል ከተሸፈኑ ተዋናዮች ጋር የአንድሮይድ-ብቻ ምልክት | ምንም ተመሳሳይ ምልክት አልወጣም | `android_only` (መቆየት ያለበት `status:"accepted"`) |

አዲስ ምልክቶች ሲታዩ ወደ የሼማ ዲፍ ፖሊሲ ሞጁል **እና** ያክሏቸው
ከላይ ያለው ሰንጠረዥ ስለዚህ runbook በ CLI ውስጥ የሚላከውን የማስፈጸሚያ አመክንዮ ያንጸባርቃል።
ማንኛውም አንድሮይድ-ብቻ ሲግናል ግልጽ የሆነ `status` ካስቀረ ወይም ካለ Schema አሁን አይሰራም
የ`policy_violations` ድርድር ባዶ አይደለም፣ ስለዚህ ይህን የፍተሻ ዝርዝር ከ ጋር በማመሳሰል ያቆዩት።
`telemetry_schema_diff.md` §3 እና የቅርብ ጊዜዎቹ የJSON ቅጽበተ-ፎቶዎች በ ውስጥ ተጠቅሰዋል
`telemetry_redaction_minutes_*.md`.

## 3. የስራ ፍሰት ይሽሩ

መልሶ ማገገሚያዎችን ወይም ግላዊነትን በሚሻሩበት ጊዜ መሻሮች የ"ሰበር ብርጭቆ" አማራጭ ናቸው።
ማንቂያዎች ደንበኞችን ያግዳሉ. ሙሉውን የውሳኔ ዱካ ከተመዘገበ በኋላ ብቻ ይተግብሩ
በተፈጠረው ክስተት ዶክ.1. ** ተንሸራታች እና ወሰን ያረጋግጡ።** የፔጄርዱቲ ማንቂያውን ወይም የመርሃግብር ልዩነትን ይጠብቁ።
   ወደ እሳቱ በር, ከዚያም ሩጡ
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` ወደ
   ያልተዛመዱ ባለስልጣናትን ያረጋግጡ ። የ CLI ውፅዓት እና Grafana ቅጽበታዊ ገጽ እይታዎችን ያያይዙ
   ወደ ክስተቱ መዝገብ.
2. **የተፈረመ ጥያቄን ያዘጋጁ።** የህዝብ ብዛት
   `docs/examples/android_override_request.json` ከቲኬቱ መታወቂያ፣ ጠያቂ፣
   ጊዜው ያለፈበት, እና ጽድቅ. ፋይሉን ከተከሰቱት ቅርሶች ቀጥሎ ያከማቹ
   ተገዢነት ግብዓቶችን ኦዲት ማድረግ ይችላል.
3. ** መሻርን ያውጡ።** ይደውሉ
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   ረዳቱ የተሻረውን ቶከን ያትማል፣ አንጸባራቂውን ይጽፋል እና ረድፍ ጨምሯል።
   ወደ Markdown ኦዲት መዝገብ. ምልክቱን በቻት ውስጥ በጭራሽ አይለጥፉ; በቀጥታ ያቅርቡ
   መሻርን ለሚተገበሩ የTorii ኦፕሬተሮች።
4. **ውጤቱን ይቆጣጠሩ።** በአምስት ደቂቃ ውስጥ አንድ ነጠላ ያረጋግጡ
   `android.telemetry.redaction.override` ክስተት ተለቀቀ, ሰብሳቢው
   የሁኔታ መጨረሻ ነጥብ `override_active=true` ያሳያል፣ እና ክስተቱ ሰነድ እነዚህን ይዘረዝራል።
   ጊዜው ያበቃል. የአንድሮይድ ቴሌሜትሪ አጠቃላይ እይታ ዳሽቦርዱን “ቶከኖችን ይሽሩ
   ንቁ” ፓነል (`android_telemetry_override_tokens_active`) ለተመሳሳይ
   ማስመሰያ ቆጠራ እና ድረስ በየ 10 ደቂቃው CLI ሁኔታን ማስኬዱን ይቀጥሉ
   hashing stabilis.
5. ** ይሽሩ እና ማህደር ያድርጉ።** ማቃለያው እንዳረፈ፣ ሩጡ
  `scripts/android_override_tool.sh revoke --token <token>` ስለዚህ የኦዲት መዝገብ
  የስረዛ ሰዓቱን ይይዛል፣ ከዚያ ያስፈጽማል
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  አስተዳደር የሚጠብቀውን የጸዳ ቅጽበታዊ ገጽ እይታን ለማደስ። ያያይዙት።
  አንጸባራቂ፣ የJSON፣ የCLI ግልባጮችን፣ Grafana ቅጽበተ-ፎቶዎችን እና የNDJSON ምዝግብ ማስታወሻ
  በ `--event-log` ወደ
  `docs/source/sdk/android/readiness/screenshots/<date>/` እና አቋራጭ
  መግቢያ ከ `docs/source/sdk/android/telemetry_override_log.md`.

ከ24 ሰአታት በላይ መሻር የኤስአርአይ ዳይሬክተር እና ተገዢነትን ማጽደቅ እና ይፈልጋል
በሚቀጥለው ሳምንታዊ የብአዴን 7 ግምገማ ማድመቅ አለበት።

### 3.1 የማሳደግ ማትሪክስ ይሽሩ

| ሁኔታ | ከፍተኛ ቆይታ | አጽዳቂዎች | አስፈላጊ ማሳወቂያዎች |
|--------|-------------|-----------|
| ነጠላ ተከራይ ምርመራ (የሃሽድ ባለስልጣን አለመመጣጠን፣ የደንበኛ Sev2) | 4 ሰዓታት | ድጋፍ መሐንዲስ + SRE ላይ-ጥሪ | ቲኬት `SUP-OVR-<id>`፣ `android.telemetry.redaction.override` ክስተት፣ የአደጋ መዝገብ |
| ፍሊት-ሰፊ የቴሌሜትሪ መቋረጥ ወይም SRE-የተጠየቀ መራባት | 24 ሰዓታት | SRE በጥሪ + የፕሮግራም መሪ | PagerDuty ማስታወሻ፣ የምዝግብ ማስታወሻ ግቤትን መሻር፣ በ`status.md` ማዘመን |
| ተገዢነት/የፎረንሲክስ ጥያቄ ወይም ከ24ሰአት በላይ የሆነ ጉዳይ | በግልጽ እስካልተሻረ ድረስ | SRE ዳይሬክተር + ተገዢነት አመራር | የአስተዳደር ደብዳቤ ዝርዝር፣ የተሻረ መዝገብ፣ AND7 ሳምንታዊ ሁኔታ |

#### የሚና ሀላፊነቶች| ሚና | ኃላፊነቶች | SLA / ማስታወሻዎች |
|-------------|-----------|
| አንድሮይድ ቴሌሜትሪ በመደወል (የአደጋ አዛዥ) | የማሽከርከር ማወቂያ፣ የመሻር መሳሪያ ስራን ያስፈጽሙ፣ በአደጋው ​​ሰነድ ውስጥ ማጽደቆችን ይመዝግቡ፣ እና መሻሩ ጊዜው ከማለፉ በፊት መሆኑን ያረጋግጡ። | በ5ደቂቃ ውስጥ PagerDutyን እውቅና ይስጡ እና በየ15ደቂቃው እድገትን ይመዝግቡ። |
| አንድሮይድ ታዛቢነት TL (Haruka Yamamoto) | የተንሸራታች ሲግናሉን ያረጋግጡ፣ ላኪ/ሰብሳቢ ሁኔታን ያረጋግጡ፣ እና የመሻሪያውን አንጸባራቂ ለኦፕሬተሮች ከመሰጠቱ በፊት ይመዝገቡ። | በ 10 ደቂቃ ውስጥ ድልድዩን ይቀላቀሉ; የማይገኝ ከሆነ ለዝግጅት ክላስተር ባለቤት ውክልና መስጠት። |
| የኤስአርአይ ግንኙነት (ሊያም ኦኮነር) | አንጸባራቂውን ለአሰባሳቢዎች ይተግብሩ፣ የኋላ መዝገብን ይቆጣጠሩ እና ከለቀቅ ኢንጂነሪንግ ጋር ለTorii ጎን ቅነሳዎች ያስተባብሩ። | በለውጥ ጥያቄ ውስጥ እያንዳንዱን የ`kubectl` እርምጃ ይመዝገቡ እና በክስተቱ ሰነድ ውስጥ የትዕዛዝ ግልባጮችን ለጥፍ። |
| ተገዢነት (ሶፊያ Martins / ዳንኤል ፓርክ) | ከ30ደቂቃ በላይ መሻሮችን አጽድቅ፣የኦዲት መዝገብ ረድፉን አረጋግጥ እና ስለተቆጣጣሪ/ደንበኛ መልእክት መላላኪያ ምክር። | በ `#compliance-alerts` ውስጥ እውቅና ይለጥፉ; ለምርት ዝግጅቶች መሻሩ ከመውጣቱ በፊት የማክበር ማስታወሻ ያስገቡ። |
| ሰነዶች/የድጋፍ አስተዳዳሪ (Priya Deshpande) | ማህደር በ`docs/source/sdk/android/readiness/…` ስር ይገለጻል/የ CLI ውፅዓት፣ የተሻረውን ምዝግብ ማስታወሻ ንፁህ ያድርጉት፣ እና ክፍተቶቹ ከታዩ የክትትል ቤተ ሙከራዎችን ያቅዱ። | ክስተቱን ከመዘጋቱ በፊት የማስረጃ ማቆየት (13 ወራት) እና የ AND7 ክትትሎች ያረጋግጣል። |

ማንኛውም የመሻር ማስመሰያ ያለ ሀ ጊዜው የሚያበቃበት ከሆነ ወዲያውኑ ከፍ ይበሉ
የሰነድ መሻር እቅድ.

## 4. የአደጋ ምላሽ

- ** ማንቂያዎች፡** የፔጄርዱቲ አገልግሎት `android-telemetry-primary` ማሻሻያ ይሸፍናል
  አለመሳካቶች፣ ላኪዎች መቆራረጥ እና የባልዲ ተንሸራታች። SLA መስኮቶች ውስጥ እውቅና
  (የድጋፍ ማጫወቻ መጽሐፍን ይመልከቱ)።
- ** ምርመራዎች: *** ለመሰብሰብ `scripts/telemetry/check_redaction_status.py` ን ያሂዱ
  የወቅቱ ላኪ ጤና፣ የቅርብ ጊዜ ማንቂያዎች እና የሃሽድ ባለስልጣን መለኪያዎች። ያካትቱ
  በአደጋው የጊዜ መስመር (`incident/YYYY-MM-DD-android-telemetry.md`) ውስጥ ውፅዓት።
- ** ዳሽቦርዶች፡** የአንድሮይድ ቴሌሜትሪ ማስተካከያ፣ አንድሮይድ ቴሌሜትሪ ይቆጣጠሩ
  አጠቃላይ እይታ፣ ማሻሻያ ማክበር እና ላኪ ጤና ዳሽቦርዶች። ያንሱ
  ለአደጋ መዝገቦች ቅጽበታዊ ገጽ እይታዎች እና ማንኛውንም የጨው ስሪት ወይም መሻርን ያብራሩ
  ክስተትን ከመዝጋትዎ በፊት የማስመሰያ ልዩነቶች።
- ** ቅንጅት: *** ለላኪዎች ጉዳዮች ፣ ማክበርን ይሳተፉ
  ለመሻር/PII ጥያቄዎች፣ እና የፕሮግራም መሪ ለሴቭ 1 ክስተቶች።

### 4.1 Escalation ፍሰት

የአንድሮይድ ክስተቶች ልክ እንደ አንድሮይድ ተመሳሳይ የክብደት ደረጃዎችን በመጠቀም ይለያያሉ።
Playbookን ይደግፉ (§2.1)። ከታች ያለው ሠንጠረዥ ማን እና እንዴት መሆን እንዳለበት ጠቅለል አድርጎ ያሳያል
በፍጥነት እያንዳንዱ ምላሽ ሰጪ ድልድዩን ይቀላቀላል ተብሎ ይጠበቃል።| ከባድነት | ተጽዕኖ | ዋና ምላሽ ሰጪ (≤5ደቂቃ) | ሁለተኛ ደረጃ መጨመር (≤10ደቂቃ) | ተጨማሪ ማሳወቂያዎች | ማስታወሻ |
|------------|--------|
| ሴቭ1 | የደንበኛ መቋረጥ፣ የግላዊነት ጥሰት ወይም የውሂብ መፍሰስ | አንድሮይድ ቴሌሜትሪ በጥሪ (`android-telemetry-primary`) | Torii በጥሪ + የፕሮግራም መሪ | ተገዢነት + SRE አስተዳደር (`#sre-governance`)፣ የክላስተር ባለቤቶች (`#android-staging`) | የጦር ክፍሉን ወዲያውኑ ይጀምሩ እና ለትእዛዝ ምዝግብ ማስታወሻ የጋራ ሰነድ ይክፈቱ። |
| Sev2 | ፍሊት መበላሸት፣ አላግባብ መጠቀምን መሻር፣ ወይም ረዘም ያለ መልሶ ማጫወት | አንድሮይድ ቴሌሜትሪ በጥሪ | አንድሮይድ መሠረቶች TL + ሰነዶች/ድጋፍ አስተዳዳሪ | የፕሮግራም መሪ፣ የመልቀቅ የምህንድስና ግንኙነት | መሻር ከ24ሰአታት በላይ ከሆነ ወደ ተገዢነት ከፍ ይበሉ። |
| Sev3 | የነጠላ ተከራይ ጉዳይ፣ የላብራቶሪ ልምምድ ወይም የምክር ማስጠንቀቂያ | ድጋፍ ኢንጂነር | አንድሮይድ በመደወል (አማራጭ) | ሰነዶች/ድጋፍ ለግንዛቤ | ወሰን ከተስፋፋ ወይም ብዙ ተከራዮች ከተጎዱ ወደ Sev2 ይለውጡ። |

| መስኮት | ድርጊት | ባለቤት(ዎች) | ማስረጃ/ማስታወሻ |
|--------|---
| 0–5 ደቂቃ | PagerDutyን እውቅና ይስጡ፣ የአደጋ አዛዥ (IC) ይመድቡ እና `incident/YYYY-MM-DD-android-telemetry.md` ይፍጠሩ። አገናኙን እና የአንድ መስመር ሁኔታን በ`#android-sdk-support` ውስጥ ጣል ያድርጉ። | በጥሪ ላይ SRE / ድጋፍ መሐንዲስ | ከሌሎች የአደጋ ምዝግብ ማስታወሻዎች ጎን የተፈጸመ የፔጀርዱቲ ack + የክስተት ግትር ቅጽበታዊ ገጽ እይታ። |
| 5–15 ደቂቃ | `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` ን ያሂዱ እና ማጠቃለያውን በክስተቱ ሰነድ ውስጥ ይለጥፉ። ፒንግ አንድሮይድ ታዛቢነት TL (Haruka Yamamoto) እና የድጋፍ መሪ (Priya Deshpande) ለእውነተኛ ጊዜ የእጅ ማጥፋት። | IC + አንድሮይድ ታዛቢነት TL | የCLI ውፅዓት JSONን ያያይዙ፣ የተከፈቱ ዳሽቦርድ ዩአርኤሎች እና የማን ምርመራ ባለቤት እንደሆኑ ምልክት ያድርጉ። |
| 15–25 ደቂቃ | በ`android-telemetry-stg` ላይ ለመራባት የዝግጅት ክላስተር ባለቤቶችን (Haruka Yamamoto ለታዛቢነት፣ Liam O'Connor for SRE) ያሳትፉ። የምልክት ተመሳሳይነት ለማረጋገጥ በ`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` ዘር መጫን እና ከ Pixel + emulator ወረፋ ያንሱ። | የክላስተር ባለቤቶች ዝግጅት | የጸዳ `pending.queue` + `PendingQueueInspector` ውፅዓት ወደ ክስተቱ አቃፊ ይስቀሉ። |
| 25–40 ደቂቃ | መሻርን ይወስኑ Torii ስሮትሊንግ ወይም StrongBox ውድቀት። PII መጋለጥ ወይም የማይወሰን ሃሽ ከተጠረጠረ ገጽ Compliance (Sofia Martins, Daniel Park) በ`#compliance-alerts` በኩል እና የፕሮግራም መሪን በተመሳሳይ የክስተት ክር ያሳውቁ። | IC + ተገዢነት + የፕሮግራም መሪ | አገናኝ መሻር ቶከኖች፣ Norito መገለጫዎች እና የጸደቀ አስተያየቶች። |
| ≥40 ደቂቃ | የ30 ደቂቃ የሁኔታ ዝማኔዎችን ያቅርቡ (PagerDuty note + `#android-sdk-support`)። የጦርነት ክፍል ድልድይ ገና ገቢር ካልሆነ ፣የመቀነስ ኢቲኤ ሰነድ እና የተለቀቀው ኢንጂነሪንግ (አሌክሲ ሞሮዞቭ) ሰብሳቢ/ኤስዲኬ ቅርሶችን ለመንከባለል በተጠባባቂ ላይ መሆኑን ያረጋግጡ። | IC | በጊዜ ማህተም የተደረገባቸው ዝማኔዎች እና የውሳኔ ምዝግብ ማስታወሻዎች በአደጋው ​​ፋይል ውስጥ የተከማቹ እና በ`status.md` ውስጥ በሚቀጥለው ሳምንታዊ እድሳት ወቅት ተጠቃለዋል። |- ሁሉም ፍጥነቶች ከአንድሮይድ የድጋፍ ፕሌይቡክ የ"የባለቤት/የሚቀጥለው የዝማኔ ጊዜ" ሠንጠረዥን በመጠቀም በክስተቱ ሰነድ ውስጥ መንጸባረቅ አለባቸው።
- ሌላ ክስተት ቀድሞውኑ ክፍት ከሆነ አሁን ያለውን የጦርነት ክፍል ይቀላቀሉ እና አዲስ ከማሽከርከር ይልቅ የአንድሮይድ አውድ ያክሉ።
- ክስተቱ የ runbook ክፍተቶችን ሲነካ በ AND7 JIRA epic ውስጥ የመከታተያ ስራዎችን ይፍጠሩ እና `telemetry-runbook` መለያ ያድርጉ።

## 5. ትርምስ እና ዝግጁነት መልመጃዎች

- በዝርዝር የተገለጹትን ሁኔታዎች ያከናውኑ
  `docs/source/sdk/android/telemetry_chaos_checklist.md` በየሩብ ዓመቱ እና ከዚያ በፊት
  ዋና የተለቀቁ. ውጤቶቹን በላብራቶሪ ዘገባ አብነት ይመዝግቡ።
- ማስረጃዎችን (ቅጽበታዊ ገጽ እይታዎችን, ምዝግቦችን) ከስር ያከማቹ
  `docs/source/sdk/android/readiness/screenshots/`.
- የማሻሻያ ትኬቶችን በ AND7 epic በ `telemetry-lab` መለያ ይከታተሉ።
- የትዕይንት ካርታ፡ C1 (የማሻሻያ ስህተት)፣ C2 (መሻር)፣ C3 (የላኪ ቡኒ ማውጣት)፣ C4
  (የተንሸራታች ውቅር ያለው `run_schema_diff.sh` በመጠቀም የschema diff በር)፣ C5
  (የመሣሪያ-መገለጫ skew በ`generate_android_load.sh`)፣ C6 (Torii ጊዜ አልቋል)
  + ወረፋ እንደገና ማጫወት) ፣ C7 (የማረጋገጫ ውድቅ)። ይህን ቁጥር መቁጠር ከ ጋር እንዲስማማ ያድርጉት
  ልምምዶች ሲጨመሩ `telemetry_lab_01.md` እና ትርምስ ማረጋገጫ ዝርዝር።

### 5.1 የማሻሻያ ተንሸራታች እና መሰርሰሪያ (C1/C2)

1. የሃሺንግ አለመሳካትን በ
   `scripts/telemetry/inject_redaction_failure.sh` እና PagerDuty ይጠብቁ
   ማንቂያ (`android.telemetry.redaction.failure`)። የ CLI ውፅዓት ከ ያንሱ
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` ለ
   የክስተቱ መዝገብ.
2. አለመሳካቱን በ `--clear` ያጽዱ እና ማንቂያው መፍትሄውን ያረጋግጡ
   10 ደቂቃዎች; የጨው/የስልጣን ፓነሎች Grafana ቅጽበታዊ ገጽ እይታዎችን ያያይዙ።
3. በመጠቀም የተፈረመ የመሻር ጥያቄ ይፍጠሩ
   `docs/examples/android_override_request.json`፣ ጋር ይተግብሩ
   `scripts/android_override_tool.sh apply`፣ እና ያልታሸገውን ናሙና በ
   በማዘጋጀት ላይ ያለውን የላኪውን ጭነት መፈተሽ (መመልከት።
   `android.telemetry.redaction.override`)።
4. መሻርን በ`scripts/android_override_tool.sh revoke --token <token>`፣
   የመሻር ማስመሰያ ሃሽ እና የቲኬት ማጣቀሻን ጨምር
   `docs/source/sdk/android/telemetry_override_log.md`፣ እና mint a diest JSON
   በ `docs/source/sdk/android/readiness/override_logs/`. ይህ ይዘጋል
   C2 ሁኔታ በግርግር ማመሳከሪያ ዝርዝር ውስጥ እና የአስተዳደር ማስረጃውን ትኩስ አድርጎ ያስቀምጣል።

### 5.2 ላኪ ቡኒ እና ወረፋ መልሶ ማጫወት (C3/C6)1. የመድረክ ሰብሳቢውን ወደታች (`kubectl ሚዛን
   ማሰማራት/android-otel-collector --replicas=0`) ላኪን ለማስመሰል
   ቡኒ መውጣት. የማቋቋሚያ መለኪያዎችን በሁኔታ CLI ይከታተሉ እና ማንቂያዎችን በ ላይ ያረጋግጡ
   የ 15 ደቂቃ ምልክት.
2. ሰብሳቢውን ወደነበረበት ይመልሱ, የኋለኛውን ፍሳሽ ያረጋግጡ እና ሰብሳቢውን መዝገብ ያስቀምጡ
   የድጋሚ ማጫወት ማጠናቀቅን የሚያሳይ ቅንጣቢ።
3. በሁለቱም የዝግጅት አቀራረብ Pixel እና emulator ላይ፣ ScenarioC6 ን ይከተሉ፡ ጫን
   `examples/android/operator-console`፣ የአውሮፕላን ሁነታን ቀይር፣ ማሳያውን አስረክብ
   ያስተላልፋል፣ ከዚያ የአውሮፕላን ሁነታን ያሰናክሉ እና የወረፋ ጥልቀት መለኪያዎችን ይመልከቱ።
4. እያንዳንዱን በመጠባበቅ ላይ ያለውን ወረፋ ይጎትቱ (`adb shell run- as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   : ኮር: ክፍሎች >/dev/null`), and run `java -cp ግንባታ/ክፍሎች
   org.hyperledger.iroha.android.tools.PendingQueueInspector --ፋይል
   /tmp/.queue --json > ወረፋ-እንደገና መጫወት-.json`። ዲኮድ ያያይዙ
   ኤንቨሎፕ እና ሃሽዎችን ወደ ቤተ ሙከራ ሎግ ይድገሙት።
5. ትርምስ ሪፖርቱን ከላኪ መቋረጥ ቆይታ ጋር ያዘምኑ፣ ከወረፋው ጥልቀት በፊት/በኋላ፣
   እና `android_sdk_offline_replay_errors` 0 መቆየቱን ማረጋገጫ።

### 5.3 የክላስተር ትርምስ ስክሪፕት (android-telemetry-stg)

የክላስተር ባለቤቶች ሃሩካ ያማሞቶ (አንድሮይድ ታዛቢነት TL) እና ሊያም ኦኮነር
(SRE) የልምምድ ሩጫ በተያዘበት ጊዜ ሁሉ ይህንን ስክሪፕት ይከተሉ። ቅደም ተከተል ይቆያል
ተሳታፊዎች ከቴሌሜትሪ ትርምስ ማመሳከሪያ ዝርዝር ጋር ተጣጥመው ለዚያ ዋስትና ሲሰጡ
ቅርሶች ለአስተዳደር ተይዘዋል.

** ተሳታፊዎች ***

| ሚና | ኃላፊነቶች | ያግኙን |
|-------------|--------|
| አንድሮይድ ላይ-ጥሪ IC | መሰርሰሪያውን ያሽከረክራል፣ የፔጀርዱቲ ማስታወሻዎችን ያስተባብራል፣ የትእዛዝ ሎግ ባለቤት ነው | PagerDuty `android-telemetry-primary`, `#android-sdk-support` |
| የክላስተር ባለቤቶች (Haruka, Liam) | መስኮቶችን ይቀይሩ፣ `kubectl` ድርጊቶችን ያሂዱ፣ ቅጽበታዊ ክላስተር ቴሌሜትሪ | `#android-staging` |
| ሰነዶች/የድጋፍ አስተዳዳሪ (Priya) | ማስረጃ ይቅረጹ፣ የላብራቶሪ ዝርዝርን ይከታተሉ፣ የክትትል ትኬቶችን ያትሙ | `#docs-support` |

**የቅድመ-በረራ ቅንጅት**

- ከስልጠናው 48 ሰአታት በፊት፣ የታቀደውን የሚዘረዝር የለውጥ ጥያቄ ያቅርቡ
  ሁኔታዎች (C1–C7) እና ሊንኩን በ`#android-staging` ይለጥፉ ስለዚህ የክላስተር ባለቤቶች
  የግጭት ማሰማራቶችን ማገድ ይችላል።
- የቅርብ ጊዜውን `ClientConfig` hash እና `kubectl ይሰብስቡ - አውድ ዝግጅት ፖድ
  -n android-telemetry-stg` ውፅዓት የመነሻ ሁኔታን ለመመስረት፣ ከዚያ ያከማቹ
  ሁለቱም በ `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- የመሣሪያ ሽፋን (Pixel + emulator) ያረጋግጡ እና ያረጋግጡ
  `ci/run_android_tests.sh` በቤተ ሙከራ ጊዜ ጥቅም ላይ የዋሉ መሳሪያዎችን አጠናቅቋል
  (`PendingQueueInspector`፣ ቴሌሜትሪ ኢንጀክተሮች)።

** ማስፈጸሚያ ኬላዎች ***

- በ `#android-sdk-support` ውስጥ "ሁከት መጀመሩን" አስታውቅ፣ የድልድዩን ቀረጻ ጀምር፣
  እና `docs/source/sdk/android/telemetry_chaos_checklist.md` እንዲታይ ያቆዩት።
  እያንዳንዱ ትእዛዝ ለጸሐፊው ይተረካል።
- እያንዳንዱ መርፌ እርምጃ (`kubectl scale` ፣ ላኪ) የዝግጅት ባለቤት መስታወት ይኑርዎት
  እንደገና ይጀምራል, የጭነት ማመንጫዎች) ስለዚህ ሁለቱም ታዛቢነት እና SRE ደረጃውን ያረጋግጣሉ.
- ውጤቱን ከ`ስክሪፕት/ቴሌሜትሪ/Check_redaction_status.py ያንሱ
  --status-url https://android-telemetry-stg/api/redaction/status` ከእያንዳንዱ በኋላ
  ሁኔታ እና ወደ ክስተቱ ሰነድ ይለጥፉ።

** ማገገም ***- ሁሉም መርፌዎች እስኪጸዱ ድረስ ከድልድዩ አይውጡ (`inject_redaction_failure.sh --clear` ፣
  `kubectl scale ... --replicas=1`) እና Grafana ዳሽቦርዶች አረንጓዴ ሁኔታን ያሳያሉ።
- ሰነዶች/የድጋፍ ማህደሮች ወረፋ መጣል፣የ CLI ምዝግብ ማስታወሻዎች እና ቅጽበታዊ ገጽ እይታዎች ስር
  `docs/source/sdk/android/readiness/screenshots/<date>/` እና ማህደሩን ምልክት ያደርጋል
  የለውጥ ጥያቄው ከመዘጋቱ በፊት የማረጋገጫ ዝርዝር።
- ለማንኛውም ሁኔታ የክትትል ትኬቶችን በ `telemetry-chaos` መለያ ይመዝገቡ
  ያልተሳኩ ወይም ያልተጠበቁ መለኪያዎችን አዘጋጅተው በ`status.md` ውስጥ ያጣቅሷቸው።
  በሚቀጥለው ሳምንታዊ ግምገማ ወቅት.

| ጊዜ | ድርጊት | ባለቤት(ዎች) | Artefact |
|-------|-------
| ቲ-30 ደቂቃ | የ`android-telemetry-stg` ጤናን ያረጋግጡ፡ `kubectl --context staging get pods -n android-telemetry-stg`፣ በመጠባበቅ ላይ ያሉ ማሻሻያዎችን ያረጋግጡ እና ሰብሳቢ ስሪቶችን ያስተውሉ። | ሀሩካ | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| ቲ-20 ደቂቃ | የዘር መነሻ መስመር ጭነት (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) እና ስቶዶትን ይያዙ። | ሊያም | `readiness/labs/reports/<date>/load-generator.log` |
| ቲ-15 ደቂቃ | `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` ወደ `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md` ቅዳ፣ የሚሄዱባቸውን ሁኔታዎች ዘርዝር (C1–C7) እና ጸሃፊዎችን ይመድቡ። | Priya Deshpande (ድጋፍ) | ልምምድ ከመጀመሩ በፊት የክስተት ምልክት ተደረገ። |
| ቲ-10 ደቂቃ | Pixel + emulatorን በመስመር ላይ ያረጋግጡ፣ የቅርብ ጊዜ ኤስዲኬ መጫኑን እና `ci/run_android_tests.sh` `PendingQueueInspector` አጠናቅሯል። | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T-5 ደቂቃ | የማጉላት ድልድይ ይጀምሩ፣ ስክሪን መቅዳት ይጀምሩ እና በ`#android-sdk-support` ውስጥ “የግርግር መጀመር”ን ያሳውቁ። | IC / ሰነዶች / ድጋፍ | ቀረጻ በ`readiness/archive/<month>/` ተቀምጧል። |
| +0ደቂቃ | የተመረጠውን ሁኔታ ከ`docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (በተለምዶ C2 + C6) ያስፈጽሙ። የላብራቶሪ መመሪያው እንዲታይ ያቆዩት እና እንደሚከሰቱ የትዕዛዝ ጥሪዎችን ይደውሉ። | Haruka drives, Liam መስታወት ውጤቶች | በቅጽበት ከክስተቱ ፋይል ጋር የተያያዙ ምዝግብ ማስታወሻዎች። |
| +15 ደቂቃ | መለኪያዎችን ለመሰብሰብ ባለበት ያቁሙ (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) እና Grafana ቅጽበታዊ ገጽ እይታዎችን ይያዙ። | ሀሩካ | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 ደቂቃ | ማንኛቸውም የተወጉ ውድቀቶችን ወደነበሩበት ይመልሱ (`inject_redaction_failure.sh --clear`፣ `kubectl scale ... --replicas=1`)፣ ወረፋዎችን እንደገና ያጫውቱ እና ማንቂያዎች መዘጋታቸውን ያረጋግጡ። | ሊያም | `readiness/labs/reports/<date>/recovery.log` |
| +35 ደቂቃ | አጭር መግለጫ፡ የክስተቱን ሰነድ በማለፍ/ከሽምቅ በአንድ scenario ያዘምኑ፣ ተከታታዮችን ይዘርዝሩ እና ቅርሶችን ወደ git ይግፉ። የማህደር ማረጋገጫ ዝርዝሩ ሊጠናቀቅ እንደሚችል ሰነዶችን ያሳውቁ/ድጋፍ ያድርጉ። | IC | የክስተት ሰነድ ተዘምኗል፣ `readiness/archive/<month>/checklist.md` ምልክት ተደርጎበታል። |

- ላኪዎች ጤናማ እስኪሆኑ እና ሁሉም ማንቂያዎች እስኪጸዱ ድረስ የዝግጅት ባለቤቶችን በድልድዩ ላይ ያቆዩ።
- ጥሬ ወረፋ ቆሻሻዎችን በ`docs/source/sdk/android/readiness/labs/reports/<date>/queues/` ውስጥ ያከማቹ እና ሀሽቻቸውን በአደጋው ​​መዝገብ ውስጥ ያጣቅሱ።
- አንድ ሁኔታ ካልተሳካ ወዲያውኑ `telemetry-chaos` የሚል የJIRA ትኬት ይፍጠሩ እና ከ`status.md` ያገናኙት።
- አውቶሜሽን አጋዥ፡- `ci/run_android_telemetry_chaos_prep.sh` የመጫኛ ጀነሬተርን፣ የሁኔታ ቅጽበተ-ፎቶዎችን እና የወረፋ ወደ ውጭ መላክ የቧንቧ ስራን ያጠቃልላል። የማስተዳደሪያ መዳረሻ ሲኖር `ANDROID_TELEMETRY_DRY_RUN=false` ያቀናብሩ እና `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (ወዘተ) ስለዚህ ስክሪፕቱ እያንዳንዱን የወረፋ ፋይል ይገለበጣል፣ `<label>.sha256` ያወጣል እና `PendingQueueInspector`ን ያሂዳል Prometheus የJSON ልቀት መዝለል ሲገባው ብቻ `ANDROID_PENDING_QUEUE_INSPECTOR=false` ይጠቀሙ (ለምሳሌ፣ JDK የለም)። **ሁልጊዜ ረዳቱን ከማስኬድዎ በፊት የሚጠበቁትን የጨው መለያዎች ወደ ውጭ ይላኩ** `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` እና `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` በማቀናበር የተቀረፀው ቴሌሜትሪ ከ Rust baseline የሚለይ ከሆነ የተከተተው `check_redaction_status.py` ጥሪዎች በፍጥነት ይከሽፋሉ።

## 6. ሰነድ እና ማንቃት- ** የኦፕሬተር ማስቻያ ኪት፡** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  የ runbookን፣ የቴሌሜትሪ ፖሊሲን፣ የቤተ ሙከራ መመሪያን፣ የማህደር ማረጋገጫ ዝርዝርን እና እውቀትን ያገናኛል።
  ወደ ነጠላ AND7-ዝግጁ ጥቅል ይፈትሻል። SRE ሲያዘጋጁ ያመልክቱ
  አስተዳደር የሩብ ዓመቱን መታደስ አስቀድሞ ያነባል ወይም መርሐግብር ማስያዝ።
- **የማስቻል ክፍለ ጊዜዎች፡** የ60 ደቂቃ የማንቃት ቀረጻ በ2026-02-18 ላይ ይሰራል።
  ከሩብ እድሳት ጋር። ቁሶች ስር ይኖራሉ
  `docs/source/sdk/android/readiness/`.
**የእውቀት ፍተሻዎች፡** ሰራተኞች ≥90% በዝግጁነት ፎርም ማስቆጠር አለባቸው። ማከማቻ
  ውጤቶች `docs/source/sdk/android/readiness/forms/responses/`.
- ** ዝማኔዎች፡** በማንኛውም ጊዜ የቴሌሜትሪ ንድፎች፣ ዳሽቦርዶች፣ ወይም ፖሊሲዎችን ሲሽሩ
  ቀይር፣ ይህን የሩጫ መጽሐፍ፣ የድጋፍ ማጫወቻ ደብተር እና `status.md` በተመሳሳይ መልኩ አዘምን
  PR.
- ** ሳምንታዊ ግምገማ: ** ከእያንዳንዱ የዝገት መለቀቅ እጩ በኋላ (ወይም ቢያንስ በየሳምንቱ) ያረጋግጡ
  `java/iroha_android/README.md` እና ይህ Runbook አሁንም የአሁኑን አውቶማቲክ ያንጸባርቃል፣
  ቋሚ የማሽከርከር ሂደቶች እና የአስተዳደር የሚጠበቁ. ግምገማውን በ ውስጥ ይያዙ
  `status.md` ስለዚህ የፋውንዴሽን ማይልስ ኦዲት የሰነድ ትኩስነት መከታተል ይችላል።

## 7. የጠንካራ ቦክስ ማረጋገጫ መታጠቂያ- ** ዓላማ: ** መሣሪያዎችን ወደ ውስጥ ከማስተዋወቅዎ በፊት በሃርድዌር የተደገፉ የማረጋገጫ ቅርቅቦችን ያረጋግጡ
  StrongBox ገንዳ (AND2/AND6)። ማሰሪያው የተያዙ የምስክር ወረቀቶችን ይበላል እና ያረጋግጣቸዋል።
  የምርት ኮድ የሚያስፈጽመውን ተመሳሳይ መመሪያ በመጠቀም ከታመኑ ሥሮች ጋር።
- ** ዋቢ፡** ሙሉውን `docs/source/sdk/android/strongbox_attestation_harness_plan.md` ይመልከቱ
  ቀረጻ ኤፒአይ፣ ተለዋጭ ስም የሕይወት ዑደት፣ CI/Buildkite ሽቦ እና የባለቤትነት ማትሪክስ። ያንን እቅድ እንደ
  አዲስ የላቦራቶሪ ቴክኒሻኖችን ሲሳፈሩ ወይም የፋይናንስ/ተገዢነት ቅርሶችን ሲያዘምኑ የእውነት ምንጭ።
- ** የስራ ሂደት:
  1. በመሳሪያ ላይ የማረጋገጫ ጥቅል ይሰብስቡ (ቅፅል ስም፣ `challenge.hex`፣ እና `chain.pem` ከ
     ቅጠል → ሥር ቅደም ተከተል) እና ወደ የሥራ ቦታው ይቅዱት.
  2. `ስክሪፕቶችን/android_keystore_attestation.sh --bundle-dir  --trust-root ን ያሂዱ
     [--trust-root-dir ] --require-strongbox --output ` ተገቢውን በመጠቀም
     Google/Samsung root (መምሪያዎቹ ሙሉ የሻጭ ቅርቅቦችን እንዲጭኑ ያስችሉዎታል)።
  3. የJSON ማጠቃለያውን በማህደር ያስቀምጡ ከጥሬ ማስረጃው ጋር
     `artifacts/android/attestation/<device-tag>/`.
- ** የጥቅል ቅርጸት:** `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` ተከተል
  ለሚፈለገው የፋይል አቀማመጥ (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **የታመኑ ሥሮች፡** ከመሣሪያው ቤተ ሙከራ ሚስጥሮች ማከማቻ በሻጭ የሚቀርቡ PEMs ያግኙ። ብዙ ማለፍ
  `--trust-root` ክርክሮች ወይም `--trust-root-dir` ወደ መልህቆቹ በሚይዝበት ጊዜ ጠቁም
  ሰንሰለቱ ጎግል ባልሆነ መልህቅ ውስጥ ያበቃል።
- ** CI መታጠቂያ:** በማህደር የተቀመጡ ቅርቅቦችን በቡድን ለማረጋገጥ `scripts/android_strongbox_attestation_ci.sh` ይጠቀሙ
  በላብራቶሪ ማሽኖች ወይም በ CI ሯጮች ላይ. ስክሪፕቱ `artifacts/android/attestation/**` ይቃኛል እና ጥሪውን ያቀርባል
  የታደሰ `result.json` በመጻፍ ለእያንዳንዱ ማውጫ የተመዘገቡ ፋይሎች
  በቦታው ላይ ማጠቃለያዎች.
- ** CI ሌይን፡** አዳዲስ ጥቅሎችን ካመሳስሉ በኋላ የተገለጸውን የBuildkite እርምጃ ያሂዱ
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`)።
  ስራው `scripts/android_strongbox_attestation_ci.sh` ያስፈጽማል, ማጠቃለያ ያመነጫል
  `scripts/android_strongbox_attestation_report.py`፣ ሪፖርቱን ወደ `artifacts/android_strongbox_attestation_report.txt` ሰቅሎታል፣
  እና ግንባታውን እንደ `android-strongbox/report` ያብራራል። ማንኛውንም ውድቀቶች ወዲያውኑ ይመርምሩ እና
  የግንባታ ዩአርኤልን ከመሳሪያው ማትሪክስ ያገናኙ።
- ** ሪፖርት ማድረግ:** የJSON ውፅዓትን ከአስተዳደር ግምገማዎች ጋር ያያይዙ እና የመሳሪያውን ማትሪክስ ግቤት ያዘምኑ
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ከማረጋገጫው ቀን ጋር።
- ** የማሾፍ ልምምድ: ** ሃርድዌር በማይኖርበት ጊዜ `scripts/android_generate_mock_attestation_bundles.sh` ን ያሂዱ
  (`scripts/android_mock_attestation_der.py` የሚጠቀመው) ሚንት የሚወስኑ የሙከራ ቅርቅቦችን እና የጋራ የማስመሰያ ስርን ለማድረግ CI እና ሰነዶች ከጫፍ እስከ ጫፍ ያለውን ታጥቆ መጠቀም ይችላሉ።
- ** የውስጠ-ኮድ መከላከያዎች፡** `ci/አሂድ_android_tests.sh -- ሙከራዎች
  org.hyperledger.iroha.android.crypto.keystore.የቁልፍ ማከማቻ ቁልፍ አቅራቢ ሙከራዎች ከተጣሩ ጋር ባዶ ይሸፍናል
  የማረጋገጫ ዳግም መወለድ (StrongBox/TEE ሜታዳታ) እና `android.keystore.attestation.failure` ያወጣል
  አዲስ ቅርቅቦችን ከመላኩ በፊት መሸጎጫ/ቴሌሜትሪ ሪግሬሽን ተይዟል።

## 8. እውቂያዎች

- ** የድጋፍ ምህንድስና ጥሪ ላይ:** `#android-sdk-support`
- ** SRE አስተዳደር: ** `#sre-governance`
- ** ሰነዶች / ድጋፍ: ** `#docs-support`
- ** የሚያድግ ዛፍ፡** የአንድሮይድ ድጋፍ መጫወቻ መጽሐፍ §2.1 ይመልከቱ

## 9. ሁኔታዎችን መላ መፈለግየመንገድ ካርታ ንጥል AND7-P2 በተደጋጋሚ ገጹን የሚያሳዩ ሶስት የአደጋ ክፍሎችን ይጠራል
አንድሮይድ ላይ-ጥሪ፡- Torii/የአውታረ መረብ ጊዜ ማብቂያዎች፣የስትሮንግቦክስ ማረጋገጫ ውድቀቶች፣እና
`iroha_config` አንጸባራቂ ተንሸራታች። ከማቅረቡ በፊት በሚመለከተው ዝርዝር ውስጥ ይስሩ
Sev1/2 ክትትሎች እና ማስረጃዎችን በ `incident/<date>-android-*.md` ውስጥ ያስቀምጡ.

### 9.1 Torii እና የአውታረ መረብ ጊዜ ማብቂያዎች

** ምልክቶች ***

- ማንቂያዎች በ`android_sdk_submission_latency`፣ `android_sdk_pending_queue_depth`፣
  `android_sdk_offline_replay_errors`፣ እና Torii `/v2/pipeline` የስህተት መጠን።
- `operator-console` ፍርግሞች (ምሳሌዎች/አንድሮይድ) የቆመ የወረፋ ፍሳሽ ወይም
  በገለፃ ጀርባ ላይ ተጣብቆ ይሞክራል።

** ፈጣን ምላሽ ***

1. PagerDuty (`android-networking`) እውቅና ይስጡ እና የክስተቱን መዝገብ ይጀምሩ።
2. የ Grafana ቅጽበተ-ፎቶዎችን (የማስረከቢያ መዘግየት + ወረፋ ጥልቀት) የሚሸፍነውን ያንሱ
   የመጨረሻ 30 ደቂቃዎች.
3. ገባሪውን `ClientConfig` ሃሽ ከመሳሪያው ምዝግብ ማስታወሻ ይቅረጹ (`ConfigWatcher`
   ዳግም መጫን ሲሳካ ወይም ሳይሳካ ሲቀር አንጸባራቂውን ያትማል)።

** ምርመራ ***

- ** ወረፋ ጤና፡** የተዋቀረውን የወረፋ ፋይል ከማስተናገጃ መሳሪያ ወይም ከ
  emulator (`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`)። ፖስታዎቹን በ
  `OfflineSigningEnvelopeCodec` እንደተገለጸው
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` ለማረጋገጥ
  የኋላ ሎግ ከዋኞች ከሚጠበቁት ጋር ይዛመዳል። ዲኮድ የተደረገውን ሃሽ ከ
  ክስተት.
- ** Hash inventory:** የወረፋውን ፋይል ካወረዱ በኋላ የተቆጣጣሪውን ረዳት ያሂዱ
  ለክስተቱ ቅርሶች ቀኖናዊ ሃሽ/ተለዋጭ ስሞችን ለመያዝ፡-

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  `queue-inspector.json` እና በቆንጆ የታተመውን ስቶዶትን ከክስተቱ ጋር ያያይዙ
  እና ከ AND7 የላብራቶሪ ዘገባ ለ Scenario D ያገናኙት።
- ** Torii ግንኙነት:** ኤስዲኬን ለማስቀረት የኤችቲቲፒ ማጓጓዣ ማሰሪያውን በአካባቢው ያሂዱ
  regressions: `ci/run_android_tests.sh` ልምምዶች
  `HttpClientTransportTests`፣ `HttpClientTransportHarnessTests`፣ እና
  `ToriiMockServerTests`. እዚህ ያሉት አለመሳካቶች ከሀ ይልቅ የደንበኛ ስህተትን ያመለክታሉ
  Torii መቋረጥ።
- ** የስህተት መርፌ ልምምድ፡** በማዘጋጀት ላይ ፒክስል (ስትሮንግቦክስ) እና AOSP
  emulator፣ በመጠባበቅ ላይ ያለ ወረፋ እድገትን ለማራባት ግንኙነትን ይቀያይሩ፡
  `adb shell cmd connectivity airplane-mode enable` → ሁለት ማሳያ አስገባ
  በኦፕሬተር-ኮንሶል → `adb shell cmd የግንኙነት አውሮፕላን-ሞድ በኩል የሚደረግ ግብይት
  አሰናክል` → verify the queue drains and `android_sdk_ከመስመር ውጭ_ጨዋታ_ስህተቶችን
  ቀሪዎች 0. በድጋሚ የተጫወቱትን ግብይቶች hashes ይመዝግቡ።
- ** የማንቂያ እኩልነት:** ገደቦችን ሲያስተካክሉ ወይም ከ Torii ለውጦች በኋላ ያስፈጽሙ
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` ስለዚህ Prometheus ህጎች ይቆያሉ
  ከዳሽቦርዶች ጋር የተስተካከለ.

** ማገገም ***

1. Torii ከተቀነሰ፣ የTorii ጥሪን ያሳትፉ እና እንደገና ማጫወትዎን ይቀጥሉ።
   ወረፋ አንዴ `/v2/pipeline` ትራፊክ ይቀበላል።
2. የተጎዱ ደንበኞችን በተፈረመ `iroha_config` መግለጫዎች በኩል ብቻ እንደገና ማዋቀር። የ
   `ClientConfig` ትኩስ ጫን ተመልካች ክስተቱ በፊት የስኬት ምዝግብ ማስታወሻ ማውጣት አለበት
   መዝጋት ይችላል።
3. ክስተቱን ከወረፋ መጠን ጋር ያዘምኑት ከድጋሚ ጨዋታ በፊት/በኋላ እና ሃሽ
   ማንኛውም የወደቀ ግብይቶች.

### 9.2 StrongBox እና የማረጋገጫ ውድቀቶች

** ምልክቶች ***- ማንቂያዎች በ `android_sdk_strongbox_success_rate` ወይም
  `android.keystore.attestation.failure`.
- `android.keystore.keygen` ቴሌሜትሪ አሁን የተጠየቀውን ይመዘግባል
  `KeySecurityPreference` እና ጥቅም ላይ የዋለው መንገድ (`strongbox`፣ `hardware`፣
  `software`) ከ `fallback=true` ባንዲራ ጋር የ StrongBox ምርጫ ሲያርፍ
  ቲኢ/ሶፍትዌር። STRONGBOX_REQUIRED ጥያቄዎች አሁን ከዝምታ ይልቅ በፍጥነት ወድቀዋል
  የ TEE ቁልፎችን መመለስ.
- የድጋፍ ትኬቶች `KeySecurityPreference.STRONGBOX_ONLY` መሣሪያዎች
  ወደ የሶፍትዌር ቁልፎች መመለስ.

** ፈጣን ምላሽ ***

1. PagerDuty (`android-crypto`) እውቅና ይስጡ እና የተጎዳውን ተለዋጭ ስም ይያዙ
   (የጨው ሃሽ) እና የመሳሪያ መገለጫ ባልዲ።
2. ለመሣሪያው የማረጋገጫ ማትሪክስ ግቤትን ያረጋግጡ
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` እና
   የመጨረሻውን የተረጋገጠ ቀን ይመዝግቡ.

** ምርመራ ***

- ** የጥቅል ማረጋገጫ: ** አሂድ
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  አለመሳካቱ በመሳሪያው ምክንያት መሆኑን ለማረጋገጥ በማህደር የተመዘገበ ማረጋገጫ ላይ
  የተሳሳተ ውቅረት ወይም የፖሊሲ ለውጥ። የተፈጠረውን `result.json` ያያይዙ።
- ** ፈታኝ regen:** ተግዳሮቶች አልተሸጎጡም። እያንዳንዱ የፈተና ጥያቄ አዲስ ያድሳል
  ማረጋገጫ እና መሸጎጫዎች በ `(alias, challenge)`; ፈታኝ ያልሆኑ ጥሪዎች መሸጎጫውን እንደገና ይጠቀሙ። የማይደገፍ
- ** CI መጥረግ:** `scripts/android_strongbox_attestation_ci.sh` ን ያስፈጽሙ
  የተከማቸ ጥቅል እንደገና ተረጋግጧል; ይህ ከስርዓታዊ ችግሮች ይጠብቃል
  በአዲስ እምነት መልህቆች.
- ** የመሣሪያ መሰርሰሪያ: ** StrongBox በሌለበት ሃርድዌር ላይ (ወይም ኢምፓዩን በማስገደድ)
  ኤስዲኬን StrongBox ብቻ እንዲፈልግ ያዘጋጁ፣ የማሳያ ግብይት ያስገቡ እና ያረጋግጡ
  የቴሌሜትሪ ላኪው የ `android.keystore.attestation.failure` ክስተትን ያወጣል።
  ከሚጠበቀው ምክንያት ጋር. ይህንን ለማረጋገጥ በ StrongBox አቅም ባለው ፒክሴል ላይ ይድገሙት
  ደስተኛ መንገድ አረንጓዴ ይቆያል.
- ** SDK regression check:** `ci/run_android_tests.sh` ን ያሂዱ እና ይክፈሉ።
  በምስክርነት ላይ ያተኮሩ ስብስቦች (`AndroidKeystoreBackendDetectionTests`፣
  `AttestationVerifierTests`፣ `IrohaKeyManagerDeterministicExportTests`፣
  `KeystoreKeyProviderTests` ለመሸጎጫ/ለመለያየት። እዚህ አለመሳካቶች
  የደንበኛ-ጎን መመለሻን ያመልክቱ.

** ማገገም ***

1. ሻጩ የምስክር ወረቀቶችን ካዞረ ወይም የ
   መሣሪያ በቅርቡ ዋና ኦቲኤ ተቀብሏል።
2. የታደሰውን ጥቅል ወደ `artifacts/android/attestation/<device>/` እና ይስቀሉ።
   የማትሪክስ ግቤትን በአዲሱ ቀን ያዘምኑ።
3. StrongBox በምርት ውስጥ የማይገኝ ከሆነ፣ የሚሻረውን የስራ ሂደት ይከተሉ
   ክፍል 3 እና የመመለሻ ጊዜን መመዝገብ; የረጅም ጊዜ ቅነሳ ያስፈልገዋል
   የመሳሪያ ምትክ ወይም የሻጭ ማስተካከያ.

### 9.2a ቆራጥ ወደ ውጭ የመላክ መልሶ ማግኛ

- ** ቅርጸቶች፡** የአሁን ወደ ውጭ የሚላኩ ምርቶች v3 ናቸው (በአንድ ላኪ ጨው/ኖንስ + Argon2id፣ እንደ ተመዝግቧል)
- ** የይለፍ ሐረግ ፖሊሲ፡** v3 ≥12 ቁምፊ የይለፍ ሐረጎችን ያስፈጽማል። ተጠቃሚዎች አጭር የሚያቀርቡ ከሆነ
  የይለፍ ሐረጎች፣ በታዛዥ የይለፍ ሐረግ እንደገና ወደ ውጭ እንዲልኩ ያስተምሯቸው። v0/v1 ከውጭ የሚገቡ ናቸው።
  ነፃ ግን ከውጪ እንደመጣ ወዲያውኑ እንደ v3 መጠቅለል አለበት።
- **ጠባቂዎችን ታምፐር/እንደገና መጠቀም፡** ዲኮደሮች ዜሮ/አጭር ጨው ወይም ርዝመቶችን ውድቅ ያደርጋሉ እና ይደገማሉ
  ጨው/ያልሆኑ ጥንዶች ወለል እንደ `salt/nonce reuse` ስህተቶች። ለማጽዳት ወደ ውጭ መላክን ያድሱ
  ጠባቂው; እንደገና ጥቅም ላይ ለማዋል አይሞክሩ.
  ቁልፉን እንደገና ለማጠጣት `SoftwareKeyProvider.importDeterministic(...)` ፣ ከዚያ
  `exportDeterministic(...)` የዴስክቶፕ መጠቀሚያ አዲሱን KDF ይመዘግባል v3 ጥቅል ለመልቀቅ
  መለኪያዎች.### 9.3 አንጸባራቂ እና አለመዛመጃዎችን ያዋቅሩ

** ምልክቶች ***

- `ClientConfig` ዳግም መጫን አለመሳካቶች፣ ያልተዛመደ Torii የአስተናጋጅ ስሞች ወይም ቴሌሜትሪ
  schema diffs በ AND7 diff መሳሪያ ተጠቁሟል።
- ኦፕሬተሮች በተመሳሳዩ መሳሪያዎች ላይ የተለያዩ የድጋሚ ሙከራዎችን ሪፖርት የሚያደርጉ
  መርከቦች.

** ፈጣን ምላሽ ***

1. በአንድሮይድ ምዝግብ ማስታወሻዎች ላይ የታተመውን `ClientConfig` ዲጀስትን ያንሱ እና
   ከሚለቀቀው አንጸባራቂ የሚጠበቀው መፈጨት።
2. ለማነጻጸር የሩጫ መስቀለኛ መንገድን ጣል ያድርጉ፡
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

** ምርመራ ***

- ** የመርሃግብር ልዩነት፡** ስክሪፕቶችን/ቴሌሜትሪ/run_schema_diff.sh --android-config ያሂዱ
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  የNorito ልዩነት ዘገባ ለማመንጨት፣ የPrometheus የጽሑፍ ፋይል ያድሱ እና ያያይዙ
  JSON artefact እና ለክስተቱ የመለኪያ ማስረጃዎች እና የ AND7 ቴሌሜትሪ ዝግጁነት ምዝግብ ማስታወሻ።
- ** ማረጋገጫን አሳይ:** `iroha_cli runtime capabilities` (ወይም የአሂድ ሰዓቱን ይጠቀሙ)
  የኦዲት ትዕዛዝ) የመስቀለኛ መንገድ ማስታወቂያ crypto/ABI hashes ሰርስሮ ለማውጣት እና ለማረጋገጥ
  እነሱ ከሞባይል መግለጫው ጋር ይጣጣማሉ። አለመመጣጠን መስቀለኛ መንገድ ወደ ኋላ ተንከባሎ እንደነበር ያረጋግጣል
  አንድሮይድ መግለጫውን እንደገና ሳያወጡ።
- ** የኤስዲኬ መመለሻ ማረጋገጫ: ** `ci/run_android_tests.sh` ሽፋኖች
  `ClientConfigNoritoRpcTests`፣ `ClientConfig.ValidationTests`፣ እና
  `HttpClientTransportStatusTests`. አለመሳካቶች የተላከው ኤስዲኬ እንደማይችል ያመለክታሉ
  አሁን የተዘረጋውን አንጸባራቂ ቅርጸት ተንትን።

** ማገገም ***

1. አንጸባራቂውን በተፈቀደው የቧንቧ መስመር በኩል ያድሱ (ብዙውን ጊዜ
   `iroha_cli runtime Capabilities` → የተፈረመ Norito አንጸባራቂ → ማዋቀር ጥቅል) እና
   በኦፕሬተር ቻናል በኩል እንደገና ማሰማራት. `ClientConfig` በጭራሽ አያርትዑ
   በመሣሪያው ላይ ይሽራል።
2. አንዴ ከታረመ አንጸባራቂ መሬቶች፣ `ConfigWatcher` “ዳግም መጫን እሺ”ን ይመልከቱ።
   በእያንዳንዱ የጦር መርከቦች ደረጃ ላይ መልእክት እና ክስተቱን ከቴሌሜትሪ በኋላ ብቻ ይዝጉ
   schema diff ሪፖርቶች እኩልነት.
3. አንጸባራቂውን ሃሽ፣ schema diff artefact ዱካ እና የአደጋ ማገናኛን ይመዝግቡ
   `status.md` ለአንድሮይድ ክፍል ለኦዲትነት።

## 10. ኦፕሬተር ማስቻል ካሪኩለም

የመንገድ ካርታ ንጥል **AND7** ሊደገም የሚችል የሥልጠና ጥቅል ስለሚያስፈልገው ኦፕሬተሮች፣
የድጋፍ መሐንዲሶች፣ እና SRE የቴሌሜትሪ/የማሻሻያ ዝማኔዎችን ያለሱ ሊቀበል ይችላል።
ግምት. ይህንን ክፍል ከ ጋር ያጣምሩ
`docs/source/sdk/android/readiness/and7_operator_enablement.md`፣ በውስጡ የያዘው።
ዝርዝር የማረጋገጫ ዝርዝር እና የጥበብ አገናኞች።

### 10.1 የክፍለ ጊዜ ሞጁሎች (የ60 ደቂቃ አጭር መግለጫ)

1. **የቴሌሜትሪ አርክቴክቸር (15 ደቂቃ)** በላኪው ቋት ውስጥ ይራመዱ፣
   ማሻሻያ ማጣሪያ፣ እና የሼማ ልዩነት መሣሪያ። ማሳያ
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` ሲደመር
   `scripts/telemetry/check_redaction_status.py` ስለዚህ ተሰብሳቢዎች እኩልነት ምን ያህል እንደሆነ ያያሉ።
   ተፈጻሚ ነው።
2. **Runbook + chaos labs (20min)** የዚህን ሩጫ መጽሐፍ ክፍል 2–9 ያድምቁ።
   ከ`readiness/labs/telemetry_lab_01.md` አንድ ሁኔታን ይለማመዱ እና እንዴት እንደሆነ ያሳዩ
   በ`readiness/labs/reports/<stamp>/` ስር ያሉ ቅርሶችን በማህደር ለማስቀመጥ።
3. ** መሻር + ተገዢነት የስራ ፍሰት (10 ደቂቃ)።** ክፍል 3 ገምግሟል፣
   አሳይ `scripts/android_override_tool.sh` (ማመልከት/መሻር/መፍጨት)፣ እና
   አዘምን `docs/source/sdk/android/telemetry_override_log.md` እና የቅርብ
   JSON መፈጨት
4. **ጥያቄ እና መልስ/የእውቀት ማረጋገጫ (15ደቂቃ)** ፈጣን ማመሳከሪያ ካርዱን ወደ ውስጥ ይጠቀሙ
   `readiness/cards/telemetry_redaction_qrc.md` ጥያቄዎችን ወደ መልህቅ፣ እንግዲያውስ
   በ `readiness/and7_operator_enablement.md` ውስጥ ክትትልን ይያዙ.### 10.2 የንብረት ማረጋገጫ እና ባለቤቶች

| ንብረት | Cadence | ባለቤት(ዎች) | የማህደር ቦታ |
|-------|--------|
| የተመዘገበ የእግር ጉዞ (ማጉላት/ቡድኖች) | ሩብ ወይም ከእያንዳንዱ የጨው ሽክርክሪት በፊት | አንድሮይድ ታዛቢነት TL + ሰነዶች/ድጋፍ አስተዳዳሪ | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (መቅዳት + የማረጋገጫ ዝርዝር) |
| የተንሸራታች ወለል እና ፈጣን ማጣቀሻ ካርድ | መመሪያ/ runbook ሲቀየር አዘምን | ሰነዶች/ድጋፍ አስተዳዳሪ | `docs/source/sdk/android/readiness/deck/` እና `/cards/` (PDF + Markdown ወደ ውጪ መላክ) |
| የእውቀት ማረጋገጫ + የመከታተያ ወረቀት | ከእያንዳንዱ የቀጥታ ክፍለ ጊዜ በኋላ | የምህንድስና ድጋፍ | `docs/source/sdk/android/readiness/forms/responses/` እና `and7_operator_enablement.md` የመገኘት እገዳ |
| የጥያቄ እና መልስ መዝገብ / የድርጊት ማስታወሻ | ማንከባለል; ከእያንዳንዱ ክፍለ ጊዜ በኋላ የዘመነ | LLM (ትወና DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 ማስረጃዎች እና የግብረመልስ ምልልስ

- የክፍለ-ጊዜ ቅርሶችን (ቅጽበታዊ ገጽ እይታዎች፣ የአደጋ ልምምዶች፣ የፈተና ጥያቄዎች ወደ ውጭ መላክ) በ ውስጥ ያከማቹ
  አስተዳደር ሁለቱንም ኦዲት ማድረግ ይችላል።
  ዝግጁነት አንድ ላይ ይከታተላል.
- አንድ ክፍለ ጊዜ ሲጠናቀቅ `status.md` (የአንድሮይድ ክፍል) ከሚከተለው አገናኞች ጋር አዘምን
  የማህደር ማውጫውን እና ማንኛውንም ክፍት ክትትልን ያስተውሉ.
- ከቀጥታ ጥያቄ እና መልስ የሚመጡ ድንቅ ጥያቄዎች ወደ ጉዳዮች ወይም ሰነድ መቀየር አለባቸው
  በአንድ ሳምንት ውስጥ ጥያቄዎችን ይጎትቱ; በ ውስጥ ያለውን የመንገድ ካርታ epics (AND7/AND8) ያጣቅሱ
  የቲኬት መግለጫ ባለቤቶች እንዲሰለፉ።
- SRE ያመሳስላል የማህደር ማመሳከሪያ ዝርዝሩን እና በ ውስጥ የተዘረዘሩትን የሼማ ልዩነት አርቲፊኬት ይገመግማል
  ክፍል2.3 ሥርዓተ ትምህርቱ ለሩብ ዓመት መዘጋቱን ከማወጁ በፊት።