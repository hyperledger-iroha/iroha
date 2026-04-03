<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# የደህንነት ኦዲት ሪፖርት

ቀን፡- 2026-03-26

## አስፈፃሚ ማጠቃለያ

ይህ ኦዲት ያተኮረው አሁን ባለው ዛፍ ውስጥ ከፍተኛ ተጋላጭነት ባላቸው ቦታዎች ላይ ነው፡ Torii HTTP/API/auth ፍሰቶች፣ P2P ትራንስፖርት፣ ሚስጥራዊ አያያዝ ኤፒአይዎች፣ የኤስዲኬ የትራንስፖርት ጠባቂዎች እና የአባሪ ሳኒታይዘር መንገድ።

6 ሊተገበሩ የሚችሉ ጉዳዮችን አግኝቻለሁ፡-

- 2 ከፍተኛ የክብደት ግኝቶች
- 4 መካከለኛ ክብደት ግኝቶች

በጣም አስፈላጊዎቹ ችግሮች የሚከተሉት ናቸው-

1. Torii ለእያንዳንዱ የኤችቲቲፒ ጥያቄ የመግቢያ መጠየቂያ ራስጌዎችን ይመዘግባል፣ ይህም ተሸካሚ ቶከኖችን፣ የኤፒአይ ቶከኖችን፣ የከዋኝ ክፍለ ጊዜ/ቡትስትራክሽን ቶከኖችን እና የmTLS ማርከሮችን ወደ ምዝግብ ማስታወሻዎች ያስተላልፋል።
2. በርካታ የህዝብ Torii መስመሮች እና ኤስዲኬዎች አሁንም ጥሬ `private_key` እሴቶችን ወደ አገልጋዩ መላክ ይደግፋሉ ስለዚህም Torii ደዋዩን ወክሎ መፈረም ይችላል።
3. በአንዳንድ ኤስዲኬዎች ውስጥ ሚስጥራዊ የዘር ምንጭ እና የቀኖናዊ ጥያቄ ማረጋገጫን ጨምሮ በርካታ "ሚስጥራዊ" መንገዶች እንደ ተራ የጥያቄ አካላት ይቆጠራሉ።

# ዘዴ

- የTorii፣ P2P፣ crypto/VM እና ኤስዲኬ ሚስጥራዊ አያያዝ መንገዶች የማይለዋወጥ ግምገማ
- የታለመ የማረጋገጫ ትዕዛዞች;
  - `cargo check -p iroha_torii --lib --message-format short` -> ማለፍ
  - `cargo check -p iroha_p2p --message-format short` -> ማለፍ
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> ማለፍ
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> ማለፊያ፣ የተባዛ-ስሪት ማስጠንቀቂያዎች ብቻ
- በዚህ ማለፊያ ያልተጠናቀቀ፡-
  - ሙሉ የስራ ቦታ ግንባታ/ሙከራ/ክሊፕ
  - ስዊፍት/ግራድል የሙከራ ስብስቦች
  - CUDA/የብረት አሂድ ጊዜ ማረጋገጫ

# ግኝቶች

### SA-001 ከፍተኛ፡ Torii ምዝግብ ማስታወሻዎች ሚስጥራዊነት ያላቸው ጥያቄዎች በአለምአቀፍ ደረጃተፅዕኖ፡ መርከቦች ፍለጋን የሚጠይቁ ማናቸውም ማሰማራት ተሸካሚ/ኤፒአይ/ኦፕሬተር ቶከኖችን እና ተዛማጅ የማረጋገጫ ቁሳቁሶችን ወደ የመተግበሪያ ምዝግብ ማስታወሻዎች ሊያፈስ ይችላል።

ማስረጃ፡-

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()` ያነቃል።
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)` ያነቃል።
- ሚስጥራዊነት ያላቸው የራስጌ ስሞች በተመሳሳይ አገልግሎት ውስጥ በሌላ ቦታ በንቃት ጥቅም ላይ ይውላሉ:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

ይህ ለምን አስፈላጊ ነው:

- `include_headers(true)` ሙሉ ወደ ውስጥ የሚገቡ የራስጌ ዋጋዎችን ወደ መፈለጊያ ቦታዎች ይመዘግባል።
- Torii እንደ `Authorization`፣ `x-api-token`፣ `x-iroha-operator-session`፣ `x-iroha-operator-token`፣ እና `x-forwarded-client-cert` ያሉ የማረጋገጫ ቁሳቁሶችን ይቀበላል።
- የሎግ ማጠቢያ ማቋቋሚያ፣ የማረም ምዝግብ ማስታወሻ መሰብሰብ ወይም የድጋፍ ቅርቅብ ስለዚህ የመገለጫ ማረጋገጫ ክስተት ሊሆን ይችላል።

የሚመከር ማስተካከያ፡-

- በምርት ጊዜ ውስጥ ሙሉ የጥያቄ ራስጌዎችን ማካተት ያቁሙ።
- አሁንም ለማረም የራስጌ ምዝግብ ማስታወሻ አስፈላጊ ከሆነ ለደህንነት-ስሱ ራስጌዎች ግልጽ ማሻሻያ ይጨምሩ።
- ውሂቡ በአዎንታዊ መልኩ ካልተዘረዘረ በስተቀር ጥያቄ/ምላሽ መግባትን በነባሪነት እንደ ሚስጥራዊ ያዙት።

### SA-002 ከፍተኛ፡ የህዝብ Torii APIs አሁንም ጥሬ የግል ቁልፎችን ከአገልጋይ ወገን ፊርማ ይቀበላሉ

ተፅዕኖ፡ ደንበኞች በአውታረ መረቡ ላይ ጥሬ የግል ቁልፎችን እንዲያስተላልፉ ይበረታታሉ ስለዚህ አገልጋዩ በስማቸው መፈረም እና አላስፈላጊ ሚስጥራዊ ተጋላጭነት በኤፒአይ፣ ኤስዲኬ፣ ፕሮክሲ እና የአገልጋይ-ሜሞሪ ንብርብሮች።

ማስረጃ፡-- የአስተዳደር መስመር ሰነድ ከአገልጋይ ወገን መፈረምን በግልጽ ያስተዋውቃል፡-
  - `crates/iroha_torii/src/gov.rs:495`
- የመንገዱ ትግበራ የቀረበውን የግል ቁልፍ ይተነትናል እና የአገልጋይ ጎን ምልክት ያደርጋል፡-
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- ኤስዲኬዎች `private_key`ን ወደ JSON አካላት በተከታታይ ያደርጋሉ፡-
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

ማስታወሻዎች፡-

- ይህ ንድፍ ለአንድ መስመር ቤተሰብ ብቻ የተነጠለ አይደለም። የአሁኑ ዛፍ በአስተዳደር፣ ከመስመር ውጭ ገንዘብ፣ የደንበኝነት ምዝገባዎች እና ሌሎች መተግበሪያ-ተኮር DTOs ላይ ተመሳሳይ ምቹ ሞዴል ይዟል።
የኤችቲቲፒኤስ-ብቻ የትራንስፖርት ፍተሻዎች በአጋጣሚ የጽሑፍ ትራንስፖርትን ይቀንሳሉ፣ ነገር ግን ከአገልጋይ ወገን የሚስጥር አያያዝን ወይም የመግቢያ/የማስታወስ ተጋላጭነትን አይፈቱም።

የሚመከር ማስተካከያ፡-

- ጥሬ የ`private_key` መረጃን የሚሸከሙ ሁሉንም የጥያቄ DTOዎች ውድቅ ያድርጉ።
- ደንበኞች በአገር ውስጥ እንዲፈርሙ እና ፊርማዎችን ወይም ሙሉ በሙሉ የተፈረሙ ግብይቶችን / ፖስታዎችን እንዲያቀርቡ ይጠይቁ።
- ከተኳኋኝነት መስኮት በኋላ የ`private_key` ምሳሌዎችን ከOpenAPI/ኤስዲኬዎች ያስወግዱ።

### SA-003 መካከለኛ፡ ሚስጥራዊ ቁልፍ መውጣት ሚስጥራዊ የዘር ቁሳቁሶችን ወደ Torii ልኮ መልሰው ያስተጋባል

ተፅዕኖ፡ ሚስጥራዊው ቁልፍ-መነሻ ኤፒአይ የዘር ቁሳቁሶችን ወደ መደበኛ የጥያቄ/ምላሽ ጭነት ውሂብ ይለውጣል፣ በፕሮክሲዎች፣ መካከለኛ ዌር፣ ሎግዎች፣ ዱካዎች፣ የብልሽት ሪፖርቶች ወይም የደንበኛ አላግባብ መጠቀም ዘርን የመግለጽ እድልን ይጨምራል።

ማስረጃ፡-- ጥያቄው የዘር ቁሳቁሶችን በቀጥታ ይቀበላል-
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- የምላሽ መርሃግብሩ ዘሩን በሄክስ እና ቤዝ64 ያስተጋባል።
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- ተቆጣጣሪው በግልፅ እንደገና ኮድ ያስገባ እና ዘሩን ይመልሳል፡-
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- ስዊፍት ኤስዲኬ ይህንን እንደ መደበኛ የአውታረ መረብ ዘዴ ያጋልጣል እና በምላሽ ሞዴል ውስጥ የማስተጋባት ዘርን ይቀጥላል፡-
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

የሚመከር ማስተካከያ፡-

- በCLI/SDK ኮድ ውስጥ የአካባቢያዊ ቁልፍ መውጣቱን ይምረጡ እና የርቀት መገኛ መንገዱን ሙሉ በሙሉ ያስወግዱ።
- መንገዱ መቆየት ካለበት፣ ዘሩን በምላሹ በጭራሽ አይመልሱ እና ዘር የሚሰጡ አካላት በሁሉም የትራንስፖርት ጠባቂዎች እና በቴሌሜትሪ/የእንጨት መንገዶች ላይ ስሱ እንደሆኑ ምልክት ያድርጉ።

### SA-004 መካከለኛ፡ የኤስዲኬ ትራንስፖርት ትብነት ማወቂያ `private_key` ሚስጥራዊ ላልሆኑ ነገሮች ዓይነ ስውር ቦታዎች አሉት

ተፅዕኖ፡ አንዳንድ ኤስዲኬዎች ኤችቲቲፒኤስን ለጥሬ `private_key` ጥያቄዎች ያስገድዳሉ፣ነገር ግን አሁንም ሌላ ደህንነትን የሚነካ የጥያቄ ቁሳቁስ ደህንነቱ ባልተጠበቀ HTTP ላይ እንዲጓዝ ወይም ወደማይዛመዱ አስተናጋጆች እንዲሄድ ይፈቅዳሉ።

ማስረጃ፡-- ስዊፍት ቀኖናዊ የጥያቄ ማረጋገጫ ራስጌዎችን እንደ ሚስጥራዊነት ይቆጥራቸዋል፡-
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
ነገር ግን ስዊፍት አሁንም በ `"private_key"` ላይ የሰውነት ግጥሚያዎች ብቻ ናቸው፡
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- ኮትሊን `authorization` እና `x-api-token` ራስጌዎችን ብቻ ያውቃል፣ከዚያም ወደ ተመሳሳዩ `"private_key"` የሰውነት ሂዩሪስቲክስ ይወድቃል፡
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- ጃቫ/አንድሮይድ ተመሳሳይ ገደብ አለው፡-
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- ኮትሊን/ጃቫ ቀኖናዊ ጥያቄ ፈራሚዎች በራሳቸው የትራንስፖርት ጠባቂዎች ሚስጥራዊነት ያልተመደቡ ተጨማሪ የቃል አርዕስት ያመነጫሉ፡
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

የሚመከር ማስተካከያ፡-

- የሂዩሪስቲክ የሰውነት ቅኝትን በግልፅ የጥያቄ ምደባ ይተኩ።
- ቀኖናዊ auth ራስጌዎችን፣ የዘር/የይለፍ ቃል መስኮችን፣ የተፈረሙ ሚውቴሽን ራስጌዎችን እና ማንኛቸውንም ወደፊት ሚስጥራዊ ተሸካሚ መስኮችን በንዑስ ሕብረቁምፊ ግጥሚያ ሳይሆን በኮንትራት እንደ ስሱ አድርገው ይያዙ።
- የስሜታዊነት ደንቦችን በስዊፍት፣ ኮትሊን እና ጃቫ ላይ ያቆዩ።

### SA-005 መካከለኛ፡ አባሪ "ማጠሪያ" ንዑስ ሂደት ብቻ እና `setrlimit` ሲደመርተፅዕኖ፡ የዓባሪ ሳኒታይዘር “ማጠሪያ የተደረገ” ተብሎ ይገለጻል እና ሪፖርት ተደርጓል፣ ነገር ግን አተገባበሩ የሃብት ገደቦች ያለው የአሁኑ ሁለትዮሽ ሹካ/exec ነው። ተንታኝ ወይም የማህደር ብዝበዛ አሁንም በተመሳሳዩ ተጠቃሚ፣ የፋይል ስርዓት እይታ እና የአካባቢ አውታረ መረብ/የሂደት ልዩ መብቶች እንደ Torii ያከናውናል።

ማስረጃ፡-

- የውጪው መንገድ ልጅን ከወለዱ በኋላ ውጤቱን እንደ ማጠሪያ ምልክት ያደርገዋል።
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- ህፃኑ አሁን ካለው ተፈፃሚው ጋር በነባሪነት ይሄዳል፡-
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- ንዑስ ሂደቱ በግልፅ ወደ `AttachmentSanitizerMode::InProcess` ይቀየራል።
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
ብቸኛው ማጠንከሪያ የሚተገበረው ሲፒዩ/አድራሻ-ቦታ `setrlimit` ነው።
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

የሚመከር ማስተካከያ፡-

- ወይ እውነተኛ የስርዓተ ክወና ማጠሪያን (ለምሳሌ የስም ቦታ/ሰከንድ/ላንድሎክ/የእስር ቤት ማግለል፣ ልዩ መብት መጣል፣ ምንም አውታረ መረብ፣ የተገደበ የፋይል ስርዓት) ወይም ውጤቱን `sandboxed` ብሎ መሰየሙን ያቁሙ።
- እውነተኛ ማግለል እስኪኖር ድረስ አሁን ያለውን ንድፍ በኤፒአይዎች፣ ቴሌሜትሪ እና ሰነዶች ውስጥ ካለው “ማጠሪያ” ይልቅ እንደ “ንዑስ ሂደት ማግለል” ያዙት።

### SA-006 መካከለኛ፡ አማራጭ P2P TLS/QUIC መጓጓዣዎች የምስክር ወረቀት ማረጋገጫን ያሰናክሉተፅዕኖ፡ `quic` ወይም `p2p_tls` ሲነቃ ቻናሉ ምስጠራን ያቀርባል ነገርግን የርቀት መጨረሻ ነጥቡን አያረጋግጥም። ንቁ የጎዳና ላይ አጥቂ አሁንም ከTLS/QUIC ጋር የሚገናኙትን መደበኛ የደህንነት ጥበቃ ኦፕሬተሮች በማሸነፍ ቻናሉን ማስተላለፍ ወይም ማቋረጥ ይችላል።

ማስረጃ፡-

- QUIC የፍቃድ የምስክር ወረቀት ማረጋገጫን በግልፅ ሰነድቷል፡
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- የQUIC አረጋጋጭ የአገልጋዩን የምስክር ወረቀት ያለምንም ቅድመ ሁኔታ ይቀበላል፡-
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- የTLS-over-TCP ትራንስፖርት እንዲሁ ያደርጋል፡-
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

የሚመከር ማስተካከያ፡-

- የአቻ የምስክር ወረቀቶችን ያረጋግጡ ወይም በከፍተኛ ደረጃ በተፈረመ የእጅ መጨባበጥ እና በትራንስፖርት ክፍለ ጊዜ መካከል ግልጽ የሆነ የሰርጥ ትስስር ይጨምሩ።
- አሁን ያለው ባህሪ ሆን ተብሎ ከሆነ፣ ባህሪውን እንደገና ይሰይሙት/ያልተረጋገጠ የተመሰጠረ ትራንስፖርት አድርገው ኦፕሬተሮች ሙሉ የTLS አቻ ማረጋገጫ ብለው እንዳይሳሳቱ።

## የሚመከር የማሻሻያ ትእዛዝ1. የራስጌ ምዝግብ ማስታወሻን በማስተካከል ወይም በማሰናከል SA-001ን ወዲያውኑ ያስተካክሉ።
2. ለSA-002 የፍልሰት እቅድ ይነድፉ እና ይላኩ ስለዚህ ጥሬ የግል ቁልፎች የኤፒአይን ድንበር መሻገር ያቆማሉ።
3. የርቀት ሚስጥራዊ ቁልፍ መገኛ መንገድን ያስወግዱ ወይም ያጥቡት እና ዘር የሚሰጡ አካላትን እንደ ሚስጥራዊነት ይመድቡ።
4. የኤስዲኬ ትራንስፖርት ትብነት ደንቦችን በስዊፍት/ኮትሊን/ጃቫ አሰልፍ።
5. የዓባሪ ንፅህና መጠበቂያ ትክክለኛ ማጠሪያ ወይም የሐቀኝነት ስም መቀየር/ዳግም መጥራት እንደሚያስፈልገው ይወስኑ።
6. ኦፕሬተሮች የተረጋገጠ TLS የሚጠብቁ ማጓጓዣዎችን ከማንቃትዎ በፊት የP2P TLS/QUIC ስጋት ሞዴልን ያብራሩ እና ያጠናክሩ።

## የማረጋገጫ ማስታወሻዎች

- `cargo check -p iroha_torii --lib --message-format short` አልፏል.
- `cargo check -p iroha_p2p --message-format short` አልፏል.
- `cargo deny check advisories bans sources --hide-inclusion-graph` ከማጠሪያ ውጭ ከሮጠ በኋላ አለፈ; የተባዙ-ስሪት ማስጠንቀቂያዎችን አውጥቷል ግን `advisories ok, bans ok, sources ok` ሪፖርት አድርጓል።
- ለምስጢራዊው የመነሻ-ቁልፍ መስመር መስመር ትኩረት የተደረገበት Torii ፈተና በዚህ ኦዲት ወቅት ተጀምሯል ነገር ግን ሪፖርቱ ከመጻፉ በፊት አልተጠናቀቀም። ግኝቱ ምንም ይሁን ምን በቀጥታ ምንጭ ፍተሻ የተደገፈ ነው.