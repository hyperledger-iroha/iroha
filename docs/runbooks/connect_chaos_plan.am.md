---
lang: am
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ትርምስ እና ስህተት የመልመጃ እቅድን ያገናኙ (IOS3 / IOS7)

ይህ የመጫወቻ መጽሐፍ IOS3/IOS7ን የሚያረኩ ሊደገሙ የሚችሉ ትርምስ ልምምዶችን ይገልጻል
የመንገድ ካርታ ተግባር _“የጋራ ትርምስ መለማመጃ እቅድ”_ (`roadmap.md:1527`)። ጋር ያጣምሩት።
የግንኙነት ቅድመ እይታ runbook (`docs/runbooks/connect_session_preview_runbook.md`)
ክሮስ-ኤስዲኬ ማሳያዎችን ሲያዘጋጁ።

## ግቦች እና የስኬት መስፈርቶች
- የጋራ ግንኙነትን እንደገና ሞክር/መመለስ ፖሊሲን፣ ከመስመር ውጭ የወረፋ ገደቦችን እና
  የቴሌሜትሪ ላኪዎች የምርት ኮድን ሳይቀይሩ ቁጥጥር በሚደረግባቸው ጥፋቶች ውስጥ።
- የሚወስኑ ቅርሶችን ያንሱ (`iroha connect queue inspect` ውፅዓት፣
  `connect.*` ሜትሪክስ ቅጽበተ-ፎቶዎች፣ የስዊፍት/አንድሮይድ/JS SDK ምዝግብ ማስታወሻዎች) አስተዳደር እንዲቻል
  እያንዳንዱን ልምምድ ኦዲት ያድርጉ ።
- የኪስ ቦርሳዎች እና dApps የአክብሮት ውቅረት ለውጦችን ያረጋግጡ (አንጸባራቂ ተንሸራታቾች ፣ ጨው
  ማሽከርከር፣ የማረጋገጫ ውድቀቶች) ቀኖናዊውን `ConnectError` በመግጠም
  ምድብ እና ማሻሻያ-ደህንነቱ የተጠበቀ የቴሌሜትሪ ክስተቶች.

## ቅድመ ሁኔታዎች
1. **አካባቢያዊ ቦት ማንጠልጠያ**
   - ማሳያ Torii ቁልል ጀምር: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - ቢያንስ አንድ የኤስዲኬ ናሙና አስጀምር (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`፣
     `examples/ios/NoritoDemo`፣ አንድሮይድ `demo-connect`፣ JS `examples/connect`)።
2. **መሳሪያ**
   - የኤስዲኬ ምርመራን አንቃ (`ConnectQueueDiagnostics`፣ `ConnectQueueStateTracker`፣
     `ConnectSessionDiagnostics` በስዊፍት; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     አንድሮይድ/ጄኤስ ውስጥ አቻ)።
   - የ CLI `iroha connect queue inspect --sid <sid> --metrics` መፍትሔዎችን ያረጋግጡ
     በኤስዲኬ የተሰራው የወረፋ መንገድ (`~/.iroha/connect/<sid>/state.json` እና
     `metrics.ndjson`).
   - ሽቦ ቴሌሜትሪ ላኪዎች የሚከተሉት የሰዓት ተከታታዮች በ ውስጥ ይታያሉ
     Grafana እና በ`scripts/swift_status_export.py telemetry`፡ `connect.queue_depth`፣
     `connect.queue_dropped_total`፣ `connect.reconnects_total`፣
     `connect.resume_latency_ms`፣ `swift.connect.frame_latency`፣
     `android.telemetry.redaction.salt_version`.
3. ** የማስረጃ ማህደሮች *** - `artifacts/connect-chaos/<date>/` ይፍጠሩ እና ያከማቹ:
   - ጥሬ ምዝግብ ማስታወሻዎች (`*.log`) ፣ ሜትሪክስ ቅጽበታዊ ገጽ እይታዎች (`*.json`) ፣ ዳሽቦርድ ወደ ውጭ መላክ
     (`*.png`)፣ የCLI ውጤቶች እና የፔጀርዱቲ መታወቂያዎች።

## ሁኔታ ማትሪክስ| መታወቂያ | ስህተት | የመርፌ እርምጃዎች | የሚጠበቁ ምልክቶች | ማስረጃ |
|----|--------|------------|
| C1 | WebSocket መቋረጥ እና ዳግም ማገናኘት | `/v1/connect/ws`ን ከፕሮክሲ ጀርባ (ለምሳሌ `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) ወይም አገልግሎቱን ለጊዜው አግድ (`kubectl scale deploy/torii --replicas=0` ለ ≤60s)። ከመስመር ውጭ ወረፋዎች እንዲሞሉ ክፈፎችን መላክ እንዲቀጥል የኪስ ቦርሳውን ያስገድዱት። | `connect.reconnects_total` ጭማሪዎች፣ `connect.resume_latency_ms` ስፒሎች ግን - የዳሽቦርድ ማብራሪያ ለማቋረጥ መስኮት።- የናሙና የምዝግብ ማስታወሻ ከዳግም ግንኙነት + የማፍሰሻ መልእክቶች ጋር። |
| C2 | ከመስመር ውጭ ወረፋ ሞልቷል / ቲቲኤል ጊዜው አልፎበታል | የወረፋ ገደቦችን ለማጥበብ ናሙናውን ያስተካክሉት (ፈጣን፡ ፈጣን `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` በ`ConnectSessionDiagnostics` ውስጥ፤ አንድሮይድ/JS ተጓዳኝ ግንባታዎችን ይጠቀማሉ)። dApp ጥያቄዎችን እየጠራ ባለበት ጊዜ ቦርሳውን ለ≥2× `retentionInterval` አንጠልጥለው። | `connect.queue_dropped_total{reason="overflow"}` እና `{reason="ttl"}` ጭማሪ፣ `connect.queue_depth` አምባ በአዲሱ ገደብ፣ ኤስዲኬዎች ላዩን `ConnectError.QueueOverflow(limit: 4)` (ወይም `.QueueExpired`)። `iroha connect queue inspect` `state=Overflow` ከ `warn/drop` የውሃ ምልክቶች በ100% ያሳያል። | - የሜትሪክ ቆጣሪዎች ቅጽበታዊ ገጽ እይታ።- የ CLI JSON ውፅዓት የትርፍ ፍሰትን ይይዛል።- `ConnectError` መስመር የያዘ የስዊፍት/አንድሮይድ ሎግ ቅንጣቢ። |
| C3 | ተንሸራታች / የመግቢያ አለመቀበልን ያሳያል | የማገናኛ ማኒፌክትን በኪስ ቦርሳዎች (ለምሳሌ `docs/connect_swift_ios.md` ናሙና መግለጫ ቀይር፣ ወይም Torii በ `--connect-manifest-path` ጀምር `chain_id` ወይም `permissions` ቅጂ ላይ)። የdApp ጥያቄ ይሁንታ ያግኙ እና ቦርሳው በፖሊሲው ውድቅ ማድረጉን ያረጋግጡ። | Torii `HTTP 409` ለ`/v1/connect/session` በ `manifest_mismatch` ይመልሳል፣ ኤስዲኬዎች `ConnectError.Authorization.manifestMismatch(manifestVersion)` ያመነጫሉ፣ ቴሌሜትሪ `connect.manifest_mismatch_total`ን ያሳድጋል፣እና 000800777X ን ያመነጫል። | - Torii ምዝግብ ማስታወሻ ያልተዛመደ ፈልጎ ማግኘትን ያሳያል።- የ SDK ቅጽበታዊ ገጽ እይታ ስህተት።- በሙከራ ጊዜ ምንም የተሰለፉ ክፈፎች እንደሌለ የሚያረጋግጥ የመለኪያ ቅጽበታዊ ገጽ እይታ። |
| C4 | ቁልፍ ማሽከርከር / ጨው-ስሪት ጎድጎድ | የግንኙን ጨው ወይም የ AEAD ቁልፍን መካከለኛ ክፍለ ጊዜ አሽከርክር። በዴቭ ቁልል ውስጥ፣ Toriiን በ`CONNECT_SALT_VERSION=$((old+1))` እንደገና ያስጀምሩ (በ`docs/source/sdk/android/telemetry_schema_diff.md` ውስጥ የአንድሮይድ ማሻሻያ የጨው ሙከራን ያሳያል)። የጨው ሽክርክር እስኪያልቅ ድረስ የኪስ ቦርሳውን ከመስመር ውጭ ያቆዩት እና ከዚያ ይቀጥሉ። | የመጀመሪው የቆመበት ሙከራ በ`ConnectError.Authorization.invalidSalt` አልተሳካም ፣ ወረፋዎች ፈሰሰ (dApp የተሸጎጡ ፍሬሞችን በምክንያት `salt_version_mismatch` ይጥላል) ፣ ቴሌሜትሪ `android.telemetry.redaction.salt_version` (አንድሮይድ) እና `swift.connect.session_event{event="salt_rotation"}` ያወጣል። ከSID ማደስ በኋላ ሁለተኛ ክፍለ ጊዜ ተሳክቷል። | - ዳሽቦርድ ማብራሪያ ከጨው ዘመን በፊት/በኋላ።- ልክ ያልሆነ-ጨው ስህተት እና ቀጣይ ስኬት የያዙ ምዝግብ ማስታወሻዎች።- `iroha connect queue inspect` ውፅዓት `state=Stalled` እና ትኩስ `state=Active` ያሳያል። || C5 | ማረጋገጫ / StrongBox አለመሳካት | በአንድሮይድ ቦርሳዎች ላይ የ`attachments[]` + StrongBox ማረጋገጫን ለማካተት `ConnectApproval` ያዋቅሩ። ለdApp ከመስጠትዎ በፊት የማረጋገጫ ማሰሪያውን ይጠቀሙ (`scripts/android_keystore_attestation.sh` ከ`--inject-failure strongbox-simulated` ጋር) ወይም JSON ማረጋገጫውን ያበላሹ። | DApp መጽደቁን በ`ConnectError.Authorization.invalidAttestation`፣ Torii መዝግቦ በመዝገቡ የውድቀቱን ምክንያት፣ ላኪዎች `connect.attestation_failed_total` ደበደቡት እና ወረፋው የሚያስከፋውን ግቤት ያጸዳል። Swift/JS dApps ክፍለ-ጊዜውን በህይወት እያለ ስህተቱን ይመዘግባል። | - የሃርነስ ሎግ በመርፌ ውድቀት መታወቂያ።- ኤስዲኬ የስህተት መዝገብ + የቴሌሜትሪ ቆጣሪ ቀረጻ።- ወረፋው መጥፎ ፍሬሙን እንዳስወገደው የሚያሳይ ማስረጃ (`recordsRemoved > 0`)። |

## የሁኔታ ዝርዝሮች

### C1 - የዌብሶኬት መቋረጥ እና እንደገና መገናኘት
1. Toriiን ከፕሮክሲ (toxiproxy፣Envoy፣ ወይም `kubectl port-forward`) ጀርባ መጠቅለል
   ሙሉውን መስቀለኛ መንገድ ሳይገድሉ ተገኝነትን መቀየር ይችላሉ.
2. የ45 ሰከንድ መቋረጥ አስነሳ፡
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. የቴሌሜትሪ ዳሽቦርዶችን እና `ስክሪፕቶችን/የፈጣን_ሁኔታ_የመላክ.py ቴሌሜትሪ ይመልከቱ
   --json-out artifacts/connect-chaos//c1_metrics.json`።
4. ከተቋረጠ በኋላ ወዲያውኑ የወረፋ ሁኔታን ይጥሉ፡-
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. ስኬት = አንድ ነጠላ ዳግም የማገናኘት ሙከራ፣ የታሰረ የወረፋ እድገት እና አውቶማቲክ
   ተኪው ካገገመ በኋላ ያፈስሱ.

### C2 - ከመስመር ውጭ ወረፋ ሞልቷል / ቲቲኤል የአገልግሎት ጊዜው ያበቃል
1. በአከባቢ ግንባታ የወረፋ ጣራዎችን አሳንስ፡-
   - ስዊፍት፡ የ `ConnectQueueJournal` ማስጀመሪያውን በናሙናዎ ውስጥ ያዘምኑ
     (ለምሳሌ፡ `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` ለማለፍ.
   - አንድሮይድ/ጄኤስ፡- በሚገነቡበት ጊዜ ተመጣጣኝ የማዋቀር ነገርን ያስተላልፉ
     `ConnectQueueJournal`.
2. የኪስ ቦርሳውን (የሲሙሌተር ዳራ ወይም የመሳሪያ አውሮፕላን ሁነታ) ለ ≥60 ሰ
   dApp `ConnectClient.requestSignature(...)` ጥሪዎችን ሲያወጣ።
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (ስዊፍት) ወይም JS ይጠቀሙ
   የማስረጃ ጥቅል ወደ ውጭ ለመላክ የምርመራ ረዳት (`state.json` ፣ `journal/*.to` ፣
   `metrics.ndjson`).
4. ስኬት = የተትረፈረፈ ቆጣሪዎች መጨመር፣ የኤስዲኬ ወለሎች `ConnectError.QueueOverflow`
   አንድ ጊዜ, እና የኪስ ቦርሳው ከቀጠለ በኋላ ወረፋው ይመለሳል.

### C3 - ተንሸራታች / የመግቢያ አለመቀበልን ያሳያል
1. የመግቢያ መግለጫውን ቅጂ ይስሩ፣ ለምሳሌ፡-
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii በ `--connect-manifest-path /tmp/manifest_drift.json` (ወይም) አስጀምር
   ለመሰርሰሪያው docker compose/k8s ውቅር አዘምን)።
3. ከኪስ ቦርሳ ውስጥ አንድ ክፍለ ጊዜ ለመጀመር ሙከራ; HTTP 409 ይጠብቁ።
4. Torii + SDK ምዝግብ ማስታወሻዎችን እና `connect.manifest_mismatch_total` ከ
   የቴሌሜትሪ ዳሽቦርድ.
5. ስኬት = ያለ ወረፋ እድገት ውድቅ ማድረግ፣ በተጨማሪም የኪስ ቦርሳው የተጋራውን ያሳያል
   የታክሶኖሚ ስህተት (`ConnectError.Authorization.manifestMismatch`)።### C4 - ቁልፍ ማሽከርከር / የጨው እብጠት
1. የአሁኑን የጨው ስሪት ከቴሌሜትሪ ይመዝግቡ፡-
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Toriiን በአዲስ ጨው (`CONNECT_SALT_VERSION=$((OLD+1))` ወይም ማዘመን
   ማዋቀር ካርታ)። ዳግም ማስጀመር እስኪጠናቀቅ ድረስ የኪስ ቦርሳውን ከመስመር ውጭ ያቆዩት።
3. የኪስ ቦርሳውን ከቆመበት ቀጥል; የመጀመሪያው ከቆመበት ቀጥል ልክ ባልሆነ የጨው ስህተት መክሸፍ አለበት።
   እና `connect.queue_dropped_total{reason="salt_version_mismatch"}` ጭማሪዎች።
4. የክፍለ ጊዜ ማውጫውን በመሰረዝ መተግበሪያው የተሸጎጡ ፍሬሞችን እንዲጥል ያስገድዱት
   (`rm -rf ~/.iroha/connect/<sid>` ወይም መድረክ-ተኮር መሸጎጫ ግልጽ)፣ ከዚያ
   ክፍለ-ጊዜውን በአዲስ ምልክቶች እንደገና ያስጀምሩ።
5. ስኬት = ቴሌሜትሪ የጨው እብጠትን ያሳያል፣ ልክ ያልሆነው ከቆመበት ቀጥል ክስተት ተመዝግቧል
   አንድ ጊዜ, እና ቀጣዩ ክፍለ ጊዜ በእጅ ጣልቃ ሳይገባ ይሳካል.

### C5 - ማረጋገጫ / StrongBox አለመሳካት።
1. `scripts/android_keystore_attestation.sh` በመጠቀም የማረጋገጫ ጥቅል ይፍጠሩ
   (የፊርማውን ቢት ለመገልበጥ `--inject-failure strongbox-simulated` ያዘጋጁ)።
2. የኪስ ቦርሳው ይህን ጥቅል በTorii ኤፒአይ በኩል እንዲያያይዝ ያድርጉ። dApp
   ክፍያውን ማረጋገጥ እና ውድቅ ማድረግ አለበት.
3. ቴሌሜትሪ ያረጋግጡ (`connect.attestation_failed_total`፣ ስዊፍት/አንድሮይድ ክስተት
   መለኪያዎች) እና ወረፋው የተመረዘውን ግቤት እንደጣለ ያረጋግጡ።
4. ስኬት = አለመቀበል ለመጥፎ ፍቃድ ብቻ ነው, ወረፋዎች ጤናማ ሆነው ይቆያሉ,
   እና የማረጋገጫው መዝገብ ከቁፋሮው ማስረጃ ጋር ተከማችቷል.

## የማስረጃ ማረጋገጫ ዝርዝር
- `artifacts/connect-chaos/<date>/c*_metrics.json` ወደ ውጭ ይላካል
  `scripts/swift_status_export.py telemetry`.
- የ CLI ውጤቶች (`c*_queue.txt`) ከ `iroha connect queue inspect`።
- ኤስዲኬ + Torii ምዝግብ ማስታወሻዎች በጊዜ ማህተም እና በSID hashes።
- ዳሽቦርድ ቅጽበታዊ ገጽ እይታዎች ለእያንዳንዱ ሁኔታ ማብራሪያዎች።
Sev1/2 ማንቂያዎች ከተተኮሱ PagerDuty / የአደጋ መታወቂያዎች።

ሙሉውን ማትሪክስ በሩብ አንድ ጊዜ ማጠናቀቅ የፍኖተ ካርታ በርን ያሟላል።
Swift/Android/JS Connect ትግበራዎች ቆራጥ ምላሽ እንደሚሰጡ ያሳያል
በከፍተኛ አደጋ ውድቀት ሁነታዎች ላይ።