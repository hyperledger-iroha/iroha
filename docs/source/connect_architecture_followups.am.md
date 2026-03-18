---
lang: am
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የአርክቴክቸር ክትትል ተግባራትን ያገናኙ

ይህ ማስታወሻ ከኤስዲኬ መስቀል የመጣውን የምህንድስና ክትትልን ይይዛል
የሕንፃ ግምገማን ያገናኙ። እያንዳንዱ ረድፍ ለአንድ ጉዳይ (የጂራ ቲኬት ወይም PR) ካርታ ሊኖረው ይገባል
ሥራ ከተያዘ በኋላ. ባለቤቶች የመከታተያ ትኬቶችን ሲፈጥሩ ሰንጠረዡን ያዘምኑ።| ንጥል | መግለጫ | ባለቤት(ዎች) | መከታተል | ሁኔታ |
|------|-------------|----------|------|-----|
| የተጋሩ የኋላ-ኦፍ ቋሚዎች | ገላጭ የኋላ ማጥፋት + ጂተር አጋዥዎችን (`connect_retry::policy`) ተግብር እና ለስዊፍት/አንድሮይድ/JS ኤስዲኬዎች አጋልጣቸው። | ስዊፍት ኤስዲኬ፣ አንድሮይድ አውታረ መረብ TL፣ JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | ተጠናቅቋል - `connect_retry::policy` በ deterministic splitmix64 ናሙና አረፈ; ስዊፍት (`ConnectRetryPolicy`)፣ አንድሮይድ እና JS ኤስዲኬዎች የሚያንጸባርቁ ረዳቶችን እና ወርቃማ ሙከራዎችን ይልካሉ። |
| ፒንግ/ፖንግ ማስፈጸሚያ | ሊዋቀር የሚችል የልብ ምት ማስፈጸሚያ ከተስማሙ የ30ዎቹ ቃላቶች እና የአሳሽ ዝቅተኛ መቆንጠጥ ጋር ይጨምሩ። የወለል መለኪያዎች (`connect.ping_miss_total`)። | ስዊፍት ኤስዲኬ፣ አንድሮይድ አውታረ መረብ TL፣ JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | ተጠናቅቋል — Torii አሁን የሚዋቀር የልብ ምት ክፍተቶችን (`ping_interval_ms`፣ `ping_miss_tolerance`፣ `ping_min_interval_ms`)፣ የ`connect.ping_miss_total` መለኪያን ያጋልጣል፣ እና የልብ መቆራረጥ መቆራረጥ ሜትሪክ። የኤስዲኬ ባህሪ ቅጽበታዊ ገጽ እይታዎች ለደንበኛዎች አዲሱን እንቡጦች ላይ ያሳያሉ። |
| ከመስመር ውጭ ወረፋ ጽናት | የተጋራውን እቅድ በመጠቀም Norito `.to` ጆርናል ፀሐፊዎችን/አንባቢዎችን ለግንኙነት ወረፋዎች (Swift `FileManager`፣አንድሮይድ ኢንክሪፕትድ ማከማቻ፣JS IndexedDB) ተግብር። | ስዊፍት ኤስዲኬ፣ አንድሮይድ ዳታ ሞዴል TL፣ JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | ተጠናቅቋል - ስዊፍት፣ አንድሮይድ እና JS አሁን የጋራውን `ConnectQueueJournal` + የምርመራ አጋሮችን በማቆየት/ትርፍ ፍተሻ ይላካሉ ስለዚህም የማስረጃ ጥቅሎች በሁሉም ላይ ቆራጥነት እንዲኖራቸው ኤስዲኬዎች።【IrohaSwift/ምንጮች/IrohaSwift/ConnectQueueJournal.swift:1】【ጃቫ/iroha_android/src/main/java/org/hype rledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox ማረጋገጫ ክፍያ | ክር `{platform,evidence_b64,statement_hash}` በ Wallet ማጽደቆች በኩል እና ማረጋገጫ ወደ dApp ኤስዲኬዎች ያክሉ። | አንድሮይድ Crypto TL፣ JS Lead | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | በመጠባበቅ ላይ |
| የማዞሪያ መቆጣጠሪያ ፍሬም | በሁሉም ኤስዲኬዎች ውስጥ `Control::RotateKeys` + `RotateKeysAck`ን ተግብር እና `cancelRequest(hash)`/ማሽከርከር ኤፒአይዎችን አጋልጥ። | ስዊፍት ኤስዲኬ፣ አንድሮይድ አውታረ መረብ TL፣ JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | በመጠባበቅ ላይ |
| ቴሌሜትሪ ላኪዎች | `connect.queue_depth`፣ `connect.reconnects_total`፣ `connect.latency_ms`፣እና ቆጣሪዎችን ወደ ነባር የቴሌሜትሪ ቧንቧዎች (OpenTelemetry) ያጫውቱ። | ቴሌሜትሪ WG፣ የኤስዲኬ ባለቤቶች | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | በመጠባበቅ ላይ |
| ስዊፍት CI gating | ከግንኙነት ጋር የተገናኙ የቧንቧ መስመሮች `make swift-ci` መጥራትን ያረጋግጡ ስለዚህ ቋሚ እኩልነት፣ ዳሽቦርድ ምግቦች እና Buildkite `ci/xcframework-smoke:<lane>:device_tag` ሜታዳታ በመላው ኤስዲኬዎች ላይ እንዲሰለፉ ያድርጉ። | ስዊፍት ኤስዲኬ መሪ፣ ኢንፍራ ገንቡ | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | በመጠባበቅ ላይ |
| የመውደቅ ክስተት ሪፖርት ማድረግ | ለጋራ ታይነት የXCFramework የጢስ ማውጫ ገጠመኞችን (`xcframework_smoke_fallback`፣ `xcframework_smoke_strongbox_unavailable`) በኮኔክተር ዳሽቦርድ ውስጥ ሽቦ ያድርጉ። | Swift QA Lead, Infra መገንባት | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | በመጠባበቅ ላይ || ተገዢነት አባሪዎች ያልፋል | ኤስዲኬዎች አማራጭ የ`attachments[]` + `compliance_manifest_id` የማረጋገጫ ክፍያዎችን ያለምንም ኪሳራ መቀበላቸውን እና ማስተላለፍዎን ያረጋግጡ። | ስዊፍት ኤስዲኬ፣ አንድሮይድ ዳታ ሞዴል TL፣ JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | በመጠባበቅ ላይ |
| የታክሶኖሚ አሰላለፍ ላይ ስህተት | የተጋራውን ቁጥር (`Transport`፣ `Codec`፣ `Authorization`፣ `Timeout`፣ `QueueOverflow`፣ `Internal`) ከመድረክ-speci.speci. | ስዊፍት ኤስዲኬ፣ አንድሮይድ አውታረ መረብ TL፣ JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | ተጠናቅቋል — ስዊፍት፣ አንድሮይድ እና JS ኤስዲኬዎች የጋራውን `ConnectError` መጠቅለያ + ቴሌሜትሪ አጋዥዎችን ከ README/TypeScript/Java ሰነዶች እና TLS/የጊዜ ማብቂያ/ኤችቲቲፒ/ኮዴክ/ወረፋን የሚሸፍኑ የድጋሚ ሙከራዎችን ይልካሉ። ጉዳዮች።【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/ምንጮች/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src /test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
| ወርክሾፕ ውሳኔ መዝገብ | ተቀባይነት ያላቸውን ውሳኔዎች ለምክር ቤቱ መዝገብ የሚያጠቃልለውን የተብራራውን ንጣፍ/ማስታወሻ ያትሙ። | የኤስዲኬ ፕሮግራም መሪ | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | በመጠባበቅ ላይ |

> የመከታተያ ለዪዎች ባለቤቶች ክፍት ቲኬቶች እንደ ይሞላሉ; ከችግር ሂደት ጎን ለጎን የ`Status` አምድ ያዘምኑ።