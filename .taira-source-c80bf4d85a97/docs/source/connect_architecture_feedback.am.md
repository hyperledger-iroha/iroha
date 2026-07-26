---
lang: am
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2025-12-29T18:16:35.934098+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! የአርክቴክቸር ግብረመልስ ማረጋገጫ ዝርዝርን ያገናኙ

ይህ የማረጋገጫ ዝርዝር ከግንኙነት ክፍለ ጊዜ አርክቴክቸር ክፍት ጥያቄዎችን ይይዛል
ከ አንድሮይድ እና ጃቫ ስክሪፕት ግብዓት የሚያስፈልገው strawman ከ በፊት
ፌብሩዋሪ 2026 የኤስዲኬ አቋራጭ አውደ ጥናት። አስተያየቶችን በማይመሳሰል መልኩ ለመሰብሰብ ይጠቀሙበት፣ ይከታተሉ
ባለቤትነት፣ እና የዎርክሾፑን አጀንዳ ያንሱ።

> ሁኔታ/ማስታወሻዎች አምድ እንደ አንድሮይድ እና JS ይመራል የመጨረሻ ምላሾችን ይዟል
> የየካቲት 2026 የቅድመ ወርክሾፕ ማመሳሰል፤ አዲስ የክትትል ጉዳዮችን በውስጥ መስመር ያገናኙ ውሳኔዎች
> በዝግመተ ለውጥ።

## የክፍለ ጊዜ የህይወት ዑደት እና መጓጓዣ

| ርዕስ | የአንድሮይድ ባለቤት | JS ባለቤት | ሁኔታ / ማስታወሻዎች |
|------------------
| WebSocket የኋሊት ማጥፋት ስትራቴጂን እንደገና ያገናኙ (ገላጭ ከካፕድ መስመራዊ) | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ በ60ዎቹ ዕድሜ ላይ በተያዘው ገላጭ የኋላ መውጣት ላይ ተስማማ። JS መስተዋት ለአሳሽ/መስቀለኛ መንገድ ተመሳሳይ ቋሚዎች። |
| ከመስመር ውጭ ቋት አቅም ነባሪዎች (የአሁኑ ገለባ፡ 32 ፍሬሞች) | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ የተረጋገጠ ባለ 32-ፍሬም ነባሪ ከውቅረት መሻር ጋር; አንድሮይድ በ`ConnectQueueConfig` በኩል ይቀጥላል፣ JS `window.connectQueueMax` ያከብራል። |
| የግፋ-ቅጥ ዳግም ግንኙነት ማሳወቂያዎች (FCM/APNS ከምርጫ ጋር) | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ አንድሮይድ ለኪስ አፕሊኬሽኖች አማራጭ የFCM መንጠቆን ያጋልጣል። JS የአሳሽ ግፊት ገደቦችን በማሳየት በምርጫ ላይ የተመሰረተ ከሰፊ የኋላ መጥፋት ጋር ይቆያል። |
| ለሞባይል ደንበኞች የፒንግ/pong cadence መከላከያ መንገዶች | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ ደረጃውን የጠበቀ የ30ዎቹ ፒንግ ከ 3× miss tolerance ጋር; አንድሮይድ የዶዝ ተጽእኖን ያስተካክላል፣ JS የአሳሽ መዘጋትን ለማስቀረት ወደ ≥15s ይቆማል። |

## ምስጠራ እና ቁልፍ አስተዳደር

| ርዕስ | የአንድሮይድ ባለቤት | JS ባለቤት | ሁኔታ / ማስታወሻዎች |
|------------------
| X25519 ቁልፍ ማከማቻ የሚጠበቁ (StrongBox፣ WebCrypto ደህንነቱ የተጠበቀ አውድ) | አንድሮይድ Crypto TL | JS መሪ | ✅ አንድሮይድ X25519 በ StrongBox ውስጥ ያከማቻል (ወደ TEE ይመለሳል)። JS ደህንነቱ የተጠበቀ አውድ WebCrypto ለ dApps ያዛል፣ ወደ ቤተኛ `iroha_js_host` ድልድይ ተመልሶ በመስቀለኛ መንገድ። |
| ChaCha20-Poly1305 ያልሆነ አስተዳደር መጋራት በመላው ኤስዲኬ | አንድሮይድ Crypto TL | JS መሪ | ✅ የተጋራ `sequence` ቆጣሪ ኤፒአይ ከ64-ቢት መጠቅለያ እና የጋራ ሙከራዎች ጋር መቀበል፤ JS የዝገት ባህሪን ለማዛመድ BigInt ቆጣሪዎችን ይጠቀማል። |
| በሃርድዌር የተደገፈ የማረጋገጫ ጭነት እቅድ | አንድሮይድ Crypto TL | JS መሪ | ✅ የተጠናቀቀው እቅድ፡ `attestation { platform, evidence_b64, statement_hash }`; JS አማራጭ (አሳሽ)፣ መስቀለኛ መንገድ HSM plug-in መንጠቆን ይጠቀማል። |
| ለጠፉ የኪስ ቦርሳዎች የመልሶ ማግኛ ፍሰት (ቁልፍ ማዞሪያ የእጅ መጨባበጥ) | አንድሮይድ Crypto TL | JS መሪ | ✅ የWallet ሽክርክር እጅ መጨባበጥ ተቀባይነት አለው፡- dApp ጉዳዮች `rotate` ቁጥጥር፣ የኪስ ቦርሳ በአዲስ pubkey + የተፈረመ እውቅና; JS የዌብክሪፕቶ ቁሳቁሶችን ወዲያውኑ ይከፍታል። |

## ፈቃዶች እና የማረጋገጫ ቅርቅቦች| ርዕስ | የአንድሮይድ ባለቤት | JS ባለቤት | ሁኔታ / ማስታወሻዎች |
|------------------
| ዝቅተኛ የፍቃድ እቅድ (ዘዴዎች/ክስተቶች/ሃብቶች) ለ GA | አንድሮይድ ዳታ ሞዴል TL | JS መሪ | ✅ GA መነሻ መስመር፡ `methods`፣ `events`፣ `resources`፣ `constraints`; JS የTyScript አይነቶችን ከ Rust manifest ጋር ያስተካክላል። |
| የኪስ ቦርሳ ውድቅ የማድረግ ጭነት (`reason_code`፣ የተተረጎሙ መልዕክቶች) | አንድሮይድ ኔትወርክ TL | JS መሪ | ✅ ኮዶች ተጠናቀዋል (`user_declined`፣ `permissions_mismatch`፣ `compliance_failed`፣ `internal_error`) ከአማራጭ `localized_message` ጋር። |
| የጥቅል አማራጭ መስኮች (ተገዢነት/KYC አባሪዎች) | አንድሮይድ ዳታ ሞዴል TL | JS መሪ | ✅ ሁሉም ኤስዲኬዎች አማራጭ `attachments[]` (Norito `AttachmentRef`) እና `compliance_manifest_id` ይቀበላሉ; የባህሪ ለውጥ አያስፈልግም። |
| አሰላለፍ በNorito JSON እቅድ እና በድልድይ-የተፈጠሩ መዋቅሮች | አንድሮይድ ዳታ ሞዴል TL | JS መሪ | ✅ ውሳኔ፡- በድልድይ የተሰሩ መዋቅሮችን እመርጣለሁ። የJSON መንገድ ለማረም ብቻ ይቀራል፣ JS `Value` አስማሚን ያቆያል። |

## የኤስዲኬ የፊት ገጽታዎች እና የኤፒአይ ቅርፅ

| ርዕስ | የአንድሮይድ ባለቤት | JS ባለቤት | ሁኔታ / ማስታወሻዎች |
|------------------
| ከፍተኛ-ደረጃ ያልተመሳሰሉ በይነገጾች (`Flow`፣ async iterators) እኩልነት | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ አንድሮይድ `Flow<ConnectEvent>` አጋልጧል; JS `AsyncIterable<ConnectEvent>` ይጠቀማል; ሁለቱም ካርታ `ConnectEventKind` ይጋራል። |
| የታክሶኖሚ ካርታ ስራ ላይ ስህተት (`ConnectError`፣ የተተየቡ ንዑስ ክፍሎች) | አንድሮይድ ኔትወርክ TL | JS መሪ | ✅ የተጋራ ቁጥርን {`Transport`፣ `Codec`፣ `Authorization`፣ `Timeout`፣ `QueueOverflow`፣ `Internal`፣ `Internal`፣ `Internal`}ን ይቀበሉ። |
| የበረራ ውስጥ ምልክት ጥያቄዎችን ስረዛ ትርጓሜ | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ `cancelRequest(hash)` መቆጣጠሪያ አስተዋወቀ; ሁለቱም የኤስዲኬዎች ወለል ሊሰረዙ የሚችሉ ኮርቲኖች/የኪስ ቦርሳ እውቅናን በተመለከተ ቃል ገብተዋል። |
| የተጋሩ ቴሌሜትሪ መንጠቆዎች (ክስተቶች፣ የመለኪያዎች ስያሜ) | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ የመለኪያ ስሞች የተሰለፉ፡ `connect.queue_depth`፣ `connect.latency_ms`፣ `connect.reconnects_total`; የናሙና ላኪዎች ተመዝግቧል። |

## ከመስመር ውጭ ጽናት እና ጋዜጠኝነት

| ርዕስ | የአንድሮይድ ባለቤት | JS ባለቤት | ሁኔታ / ማስታወሻዎች |
|------------------
| ለተሰለፉ ክፈፎች የማከማቻ ቅርጸት (ሁለትዮሽ Norito vs JSON) | አንድሮይድ ዳታ ሞዴል TL | JS መሪ | ✅ ሁለትዮሽ Norito (`.to`) በሁሉም ቦታ ያከማቹ; JS IndexedDB `ArrayBuffer` ይጠቀማል። |
| የጆርናል ማቆያ ፖሊሲ እና የመጠን መያዣዎች | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ ነባሪ ማቆየት 24 ሰአት እና 1ሚቢ በአንድ ክፍለ ጊዜ; በ `ConnectQueueConfig` በኩል ሊዋቀር የሚችል። |
| ሁለቱም ወገኖች ክፈፎችን ሲደግሙ የግጭት አፈታት | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ `sequence` + `payload_hash` ይጠቀሙ; ብዜቶች ችላ ተብለዋል፣ ግጭቶች `ConnectError.Internal` በቴሌሜትሪ ክስተት ያስነሳሉ። |
| ቴሌሜትሪ ለወረፋ ጥልቀት እና መልሶ ማጫወት ስኬት | አንድሮይድ አውታረ መረብ TL | JS መሪ | ✅ Emit `connect.queue_depth` መለኪያ እና `connect.replay_success_total` ቆጣሪ; ሁለቱም ኤስዲኬዎች ወደ የተጋራ Norito የቴሌሜትሪ እቅድ ይያያዛሉ። |

## የትግበራ ስፒሎች እና ማጣቀሻዎች- ** የዝገት ድልድይ እቃዎች፡** `crates/connect_norito_bridge/src/lib.rs` እና ተያያዥ ሙከራዎች በእያንዳንዱ ኤስዲኬ የሚጠቀሙትን ቀኖናዊ ኮድ/መግለጫ መንገዶችን ይሸፍናሉ።
- ** ፈጣን ማሳያ መታጠቂያ፡** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` መልመጃዎች የክፍለ-ጊዜ ፍሰቶችን በተሳለቁ መጓጓዣዎች ያገናኙ።
- **Swift CI gating:** ከሌሎች ኤስዲኬዎች ጋር ከማጋራትዎ በፊት የቋሚ ቅርሶችን፣ ዳሽቦርድ ምግቦችን እና Buildkite `ci/xcframework-smoke:<lane>:device_tag` ሜታዳታን ለማረጋገጥ ቅርሶችን ሲያዘምኑ `make swift-ci` ያሂዱ።
- ** የጃቫ ስክሪፕት ኤስዲኬ ውህደት ሙከራዎች፡** `javascript/iroha_js/test/integrationTorii.test.js` የግንኙነት ሁኔታ/የክፍለ ጊዜ አጋሮችን ከTorii ጋር አረጋግጧል።
- **የአንድሮይድ ደንበኛን የመቋቋም ችሎታ ማስታወሻዎች፡** `java/iroha_android/README.md:150` የወረፋ/የኋላ-ማጥፋት ነባሪዎችን ያነሳሱትን ወቅታዊ የግንኙነት ሙከራዎችን ይመዘግባል።

## ወርክሾፕ መሰናዶ ዕቃዎች

- [x] አንድሮይድ፡ ከላይ ለእያንዳንዱ የሠንጠረዥ ረድፍ ሰው መድቡ።
- [x] JS: ከላይ ለእያንዳንዱ የጠረጴዛ ረድፍ ነጥብ ሰው ይመድቡ።
- [x] ወደ ነባር የትግበራ እሾህ ወይም ሙከራዎች አገናኞችን ሰብስብ።
- [x] ከፌብሩዋሪ 2026 ምክር ቤት በፊት የቅድመ ሥራ ግምገማን ያቅዱ (ለ2026-01-29 15፡00 UTC በአንድሮይድ TL፣ JS Lead፣ Swift Lead) የተያዘ)።
- [x] `docs/source/connect_architecture_strawman.md` ከተቀበሉት መልሶች ጋር ያዘምኑ።

## ቅድመ-የተነበበ ጥቅል

- ✅ ቅርቅብ በ`artifacts/connect/pre-read/20260129/` ተመዝግቧል (በ `make docs-html` በኩል ገለባውን ፣ የኤስዲኬ መመሪያዎችን እና ይህንን የማረጋገጫ ዝርዝር ካደሰ በኋላ)።
- 📄 ማጠቃለያ + የስርጭት ደረጃዎች በቀጥታ በ `docs/source/project_tracker/connect_architecture_pre_read.md`; አገናኙን በየካቲት 2026 ወርክሾፕ ግብዣ እና በ`#sdk-council` አስታዋሽ ውስጥ ያካትቱ።
- 🔁 ጥቅሉን በሚያድስበት ጊዜ መንገዱን ያዘምኑ እና በቅድመ-የተነበበው ማስታወሻ ውስጥ ሀሽ እና ማስታወቂያውን በ`status.md` በ IOS7/AND7 ዝግጁነት ምዝግብ ማስታወሻዎች ውስጥ ያስቀምጡ።