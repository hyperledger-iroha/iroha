---
lang: am
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# የግንኙነት ክፍለ ጊዜ ቅድመ እይታ Runbook (IOS7 / JS4)

ይህ Runbook የማዘጋጀት፣ የማረጋገጥ እና የማረጋገጥ ሂደትን ከጫፍ እስከ ጫፍ መዝግቧል
በRoadmap ችካሎች በሚፈለገው መሰረት የግንኙነት ቅድመ እይታ ክፍለ ጊዜዎችን ማፍረስ **IOS7**
እና ** JS4** (`roadmap.md:1340`፣ `roadmap.md:1656`)። እነዚህን እርምጃዎች በማንኛውም ጊዜ ይከተሉ
የ Connect strawman (`docs/source/connect_architecture_strawman.md`) አሳይ
በኤስዲኬ የመንገድ ካርታዎች ውስጥ ቃል የተገባውን የወረፋ/የቴሌሜትሪ መንጠቆዎችን ይለማመዱ ወይም ይሰብስቡ
ማስረጃ ለ `status.md`.

## 1. የቅድመ በረራ ማረጋገጫ ዝርዝር

| ንጥል | ዝርዝሮች | ዋቢዎች |
|-------|---------|-----------|
| Torii የመጨረሻ ነጥብ + የግንኙነት ፖሊሲ | የTorii መሰረት URLን፣ `chain_id` እና የግንኙነት ፖሊሲን (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) ያረጋግጡ። የJSON ቅጽበታዊ ገጽ እይታን በ runbook ቲኬት ያንሱ። | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| ቋሚ + ድልድይ ስሪቶች | የምትጠቀመውን የNorito ቋሚ ሃሽ እና የድልድይ ግንባታ (Swift `NoritoBridge.xcframework` ይፈልጋል፣ JS `@iroha/iroha-js` ይፈልጋል `bootstrapConnectPreviewSession` የላከው ስሪት)። | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| ቴሌሜትሪ ዳሽቦርዶች | `connect.queue_depth`፣ `connect.queue_overflow_total`፣ `connect.resume_latency_ms`፣ `swift.connect.session_event`፣ ወዘተ የሚይዙት ዳሽቦርዶች ሊደረስባቸው የሚችሉ መሆናቸውን ያረጋግጡ (Grafana Grafana Prometheus ቅጽበተ-ፎቶዎች)። | `docs/source/connect_architecture_strawman.md`፣ `docs/source/sdk/swift/telemetry_redaction.md`፣ `docs/source/sdk/js/quickstart.md` |
| የማስረጃ ማህደሮች | እንደ `docs/source/status/swift_weekly_digest.md` (ሳምንት መፍጨት) እና `docs/source/sdk/swift/connect_risk_tracker.md` (የአደጋ መከታተያ) ያሉ መድረሻን ይምረጡ። የማከማቻ ምዝግብ ማስታወሻዎች፣ ሜትሪክስ ቅጽበታዊ ገጽ እይታዎች እና ምስጋናዎች በ`docs/source/sdk/swift/readiness/archive/<date>/connect/` ስር። | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. የቅድመ እይታ ክፍለ ጊዜን ማስነሳት

1. ** ፖሊሲ + ኮታዎችን ያረጋግጡ።** ይደውሉ፡-
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   `queue_max` ወይም TTL ካቀዱት ውቅረት የሚለይ ከሆነ ሩጫውን ያጥፉ።
   ፈተና
2. ** የሚወስን SID/URS ይፍጠሩ።** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` አጋዥ የSID/URI ትውልድን ከTorii ጋር ያገናኛል
   የክፍለ ጊዜ ምዝገባ; ስዊፍት የዌብሶኬት ንብርብርን በሚያሽከረክርበት ጊዜም ይጠቀሙበት።
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false` ወደ ደረቅ-አሂድ QR/ጥልቅ-አገናኝ ሁኔታዎች አቀናብር።
   - የተመለሰውን `sidBase64Url`፣ ጥልቅ አገናኝ URLs እና `tokens` ብሎብ በ
     ማስረጃ አቃፊ; የአስተዳደር ግምገማው እነዚህን ቅርሶች ይጠብቃል።
3. ** ሚስጥሮችን ያሰራጩ።** የጥልቅ አገናኝ URIን ከዋሌት ኦፕሬተር ጋር ያካፍሉ።
   (የፈጣን dApp ናሙና፣ አንድሮይድ ቦርሳ ወይም የQA መታጠቂያ)። ጥሬ ማስመሰያዎችን በጭራሽ አይለጥፉ
   ወደ ውይይት; በማንቃት ፓኬት ውስጥ የተመሰጠረውን ካዝና ይጠቀሙ።

## 3. ክፍለ ጊዜን መንዳት1. **የዌብሶኬትን ክፈት።** የስዊፍት ደንበኞች በተለምዶ፡-
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   ማጣቀሻ `docs/connect_swift_integration.md` ለተጨማሪ ማዋቀር (ድልድይ
   አስመጪዎች, ኮንኩሬተር አስማሚዎች).
2. ** አጽድቅ + የምልክት ፍሰቶችን።** DApps `ConnectSession.requestSignature(...)` ይደውሉ፣
   የኪስ ቦርሳዎች በ `approveSession` / `reject` በኩል ምላሽ ሲሰጡ. እያንዳንዱ ማጽደቅ መግባት አለበት።
   ከግንኙነት አስተዳደር ቻርተር ጋር ለማዛመድ የ hashed alias + ፍቃዶች።
3. **የአካል ብቃት እንቅስቃሴ ወረፋ + ከቆመበት ቀጥል ዱካዎች።** የአውታረ መረብ ግንኙነትን ቀይር ወይም አቁም
   የኪስ ቦርሳ የታሰረውን ወረፋ እና የድጋሚ ማጫወቻ መንጠቆዎች ምዝግብ ማስታወሻ ግቤቶችን ለማረጋገጥ። JS/አንድሮይድ
   ኤስዲኬዎች `ConnectQueueError.overflow(limit)` / ይለቃሉ
   ክፈፎች ሲጥሉ `.expired(ttlMs)`; ስዊፍት አንድ ጊዜ ተመሳሳይ ነገር ማክበር አለበት
   IOS7 ወረፋ ስካፎልዲንግ መሬቶች (`docs/source/connect_architecture_strawman.md`)።
   ቢያንስ አንድ ዳግም ግንኙነት ከቀረጹ በኋላ ያሂዱ
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (ወይም በ `ConnectSessionDiagnostics` የተመለሰውን ወደ ውጭ መላኪያ ማውጫ ያስተላልፉ) እና
   የተሰራውን ሰንጠረዥ/JSON ከ runbook ቲኬት ጋር ያያይዙ። CLI እንዲሁ ያነባል።
   `state.json` / `metrics.ndjson` ጥንድ `ConnectQueueStateTracker` የሚያመርተው፣
   ስለዚህ የአስተዳደር ገምጋሚዎች ያለ ምንም መሳሪያ መሳሪያ የመሰርሰሪያ ማስረጃን መፈለግ ይችላሉ።

## 4. ቴሌሜትሪ እና ታዛቢነት

- ** ለመቅረጽ መለኪያዎች: ***
  - `connect.queue_depth{direction}` መለኪያ (ከፖሊሲ ካፕ በታች መቆየት አለበት)።
  - `connect.queue_dropped_total{reason="overflow|ttl"}` ቆጣሪ (ዜሮ ያልሆነ ብቻ
    በስህተት-መርፌ ወቅት).
  - `connect.resume_latency_ms` ሂስቶግራም (ከተገደዱ በኋላ p95 ን ይቅዱ
    እንደገና መገናኘት).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - ስዊፍት-ተኮር `swift.connect.session_event` እና
    `swift.connect.frame_latency` ወደ ውጭ መላክ (`docs/source/sdk/swift/telemetry_redaction.md`)።
- ** ዳሽቦርዶች: *** የማገናኛ ሰሌዳ ዕልባቶችን ከማብራሪያ ምልክቶች ጋር ያዘምኑ።
  ቅጽበታዊ ገጽ እይታዎችን (ወይም JSON ወደ ውጭ የሚላኩ) ከጥሬው ጋር ከማስረጃ ማህደር ጋር ያያይዙ
  OTLP/Prometheus ቅጽበታዊ ገጽ እይታዎች በቴሌሜትሪ ላኪው CLI በኩል ተስበው።
- ** ማንቂያ:** ማንኛውም Sev1/2 ገደቦች ቀስቅሴ ከሆነ (በ `docs/source/android_support_playbook.md` §5)።
  የኤስዲኬ ፕሮግራም ይመሩ እና የፔጀርዱቲ ክስተት መታወቂያውን በ runbook ውስጥ ይመዝግቡ
  ትኬት ከመቀጠልዎ በፊት.

## 5. ማፅዳት እና መመለሻ

1. **የታቀዱ ክፍለ-ጊዜዎችን ሰርዝ።** ሁልጊዜም የቅድመ እይታ ክፍለ ጊዜዎችን ሰርዝ ስለዚህ ጥልቀት ወረፋ
   ማንቂያዎች ጠቃሚ ሆነው ይቆያሉ
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   ለSwift-only test runs፣ በ Rust/CLI አጋዥ በኩል ተመሳሳዩን የመጨረሻ ነጥብ ይደውሉ።
2. ** መጽሔቶችን አጽዳ።** ማንኛቸውም የቆዩ የወረፋ መጽሔቶችን ያስወግዱ
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB መደብሮች, ወዘተ) ስለዚህ የ
   የሚቀጥለው ሩጫ ንጹህ ይጀምራል። ካስፈለገዎት ከመሰረዝዎ በፊት የፋይሉን ሃሽ ይቅዱ
   የመልሶ ማጫወት ችግርን ማረም.
3. **የአደጋ ማስታወሻዎችን ፋይል ያድርጉ።** በሚከተሉት ውስጥ ያለውን ሩጫ ያጠቃልሉት፡-
   - `docs/source/status/swift_weekly_digest.md` (ዴልታ ብሎክ) ፣
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2 ያጽዱ ወይም ይቀንሱ
     ቴሌሜትሪ አንዴ ከገባ)
   - አዲስ ባህሪ ከተረጋገጠ የJS SDK ለውጥ ሎግ ወይም የምግብ አሰራር።
4. ** ብልሽቶች መጨመር: **
   - ያለ ጥፋቶች ወረፋ ሞልቷል ⇒ የማንኛውን ኤስዲኬ ስህተት ይመዝግቡ
     ፖሊሲ ከTorii ተለያይቷል።
   - ስህተቶችን ከቆመበት ቀጥል ⇒ `connect.queue_depth` + `connect.resume_latency_ms` ማያያዝ
     ወደ ክስተቱ ዘገባ ቅጽበታዊ እይታዎች።
   - የአስተዳደር አለመመጣጠን (ቶከኖች እንደገና ጥቅም ላይ ውለዋል፣ ቲቲኤል አልፏል) ⇒ ከኤስዲኬ ጋር ያሳድጉ
     በሚቀጥለው ክለሳ ወቅት `roadmap.md` መርሀ ግብር ምራ እና አብራራ።

## 6. የማስረጃ ዝርዝር| Artefact | አካባቢ |
|-------------|
| SID/Deplink/Tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| ዳሽቦርድ ወደ ውጭ መላክ (`connect.queue_depth`, ወዘተ) | `.../metrics/` ንዑስ አቃፊ |
| PagerDuty / ክስተት መታወቂያዎች | `.../notes.md` |
| የጽዳት ማረጋገጫ (Torii ሰርዝ፣ ጆርናል መጥረግ) | `.../cleanup.log` |

ይህን የማረጋገጫ ዝርዝር ማጠናቀቅ የ"ሰነዶች/ runbooks የዘመነ" የመውጫ መስፈርትን ያሟላል።
ለ IOS7/JS4 እና የአስተዳደር ገምጋሚዎች ለእያንዳንዱ የሚወስን ዱካ ይሰጣቸዋል
የቅድመ እይታ ክፍለ ጊዜን ያገናኙ።