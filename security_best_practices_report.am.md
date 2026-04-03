<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# የደህንነት ምርጥ ልምዶች ሪፖርት

ቀን፡- 2026-03-25

## አስፈፃሚ ማጠቃለያ

የቀደመውን የTorii/Soracloud ዘገባ አሁን ባለው የስራ ቦታ ላይ አድሼዋለሁ
ኮድ እና ግምገማውን በከፍተኛ ስጋት ባለው አገልጋይ ኤስዲኬ እና አራዘመ
crypto/ serialization ንጣፎች. ይህ ኦዲት በመጀመሪያ ሶስት አረጋግጧል
የመግቢያ/የማረጋገጫ ጉዳዮች፡- ሁለት ከፍተኛ ክብደት እና አንድ መካከለኛ ክብደት።
እነዚያ ሦስት ግኝቶች አሁን ባለው ዛፍ ላይ በማስተካከል ተዘግተዋል።
ከዚህ በታች ተብራርቷል. የክትትል ትራንስፖርት እና የውስጥ መላኪያ ግምገማ ተረጋግጧል
ዘጠኝ ተጨማሪ መካከለኛ-ክብደት ጉዳዮች፡ አንድ ወደ ውጭ የወጣ P2P ማንነት-ማሰር
ክፍተት፣ አንድ ወደ ውጭ የሚወጣ P2P TLS-የማውረድ ነባሪ፣ ሁለት Torii እምነት-ወሰን ሳንካዎች በ ውስጥ
የዌብ መንጠቆ አቅርቦት እና የኤምሲፒ ውስጣዊ መላኪያ፣ አንድ የኤስዲኬ ሚስጥራዊነት ያለው መጓጓዣ
በስዊፍት፣ ጃቫ/አንድሮይድ፣ ኮትሊን እና ጄኤስ ደንበኞች ላይ ክፍተት፣ አንድ SoraFS
የታመነ-ተኪ/ደንበኛ-IP ፖሊሲ ክፍተት፣ አንድ SoraFS የአካባቢ-ተኪ
የተሳሰረ/የማረጋገጫ ክፍተት፣ አንድ የአቻ-ቴሌሜትሪ ጂኦ-መፈለጊያ ግልጽ የጽሑፍ ውድቀት፣
እና አንድ ኦፕሬተር-አውዝ የርቀት-IP አልተሳካም-ክፍት መቆለፊያ/ተመን ቁልፍ ክፍተት። እነዚያ
በኋላ ላይ የተገኙ ግኝቶችም አሁን ባለው ዛፍ ውስጥ ተዘግተዋል.አራቱ ቀደም ሲል ስለ ጥሬ የግል ቁልፎች የ Soracloud ግኝቶችን ሪፖርት አድርገዋል
ኤችቲቲፒ፣ የውስጥ-ብቻ የአካባቢ-የተነበበ ተኪ አፈፃፀም፣ የማይለካ የህዝብ-አሂድ ጊዜ
ውድቀት፣ እና የርቀት-IP አባሪ ተከራይ አሁን አሁን ባለው ኮድ መኖር አይችሉም።
እነዚያ በተዘመነ የኮድ ማጣቀሻዎች ከታች የተዘጉ/የተተኩ ናቸው።ይህ ከተሟላ የቀይ ቡድን ልምምድ ይልቅ ኮድን ያማከለ ኦዲት ሆኖ ቆይቷል።
በውጭ ሊደረስ የሚችል Torii መግቢያ እና የጥያቄ ማረጋገጫ መንገዶችን ቅድሚያ ሰጥቻለሁ።
በቦታ የተረጋገጠ IVM፣ `iroha_crypto`፣ `norito`፣ ስዊፍት/አንድሮይድ/JS ኤስዲኬ
የጥያቄ ፊርማ አጋሮች፣ የአቻ-ቴሌሜትሪ ጂኦሜትሪ መንገድ እና SoraFS
የስራ ቦታ ተኪ አጋዦች እና የSoraFS ፒን/ጌትዌይ ደንበኛ-IP ፖሊሲ
ገጽታዎች. ምንም የቀጥታ የተረጋገጠ ጉዳይ ከዚያ መግቢያ/አውት፣ የመውጣት ፖሊሲ፣
የአቻ-ቴሌሜትሪ ጂኦግራፊ፣ የናሙና የP2P ትራንስፖርት ነባሪዎች፣ MCP-dispatch፣ ናሙና የተደረገ ኤስዲኬ
ትራንስፖርት፣ ኦፕሬተር-አውዝ መቆለፊያ/ተመን ቁልፍ፣ SoraFS የታመነ-ተኪ/ደንበኛ-አይፒ
ፖሊሲ፣ ወይም የአካባቢ ተኪ ቁራጭ በዚህ ሪፖርት ውስጥ ካሉት ጥገናዎች በኋላ ይቀራል።
የክትትል ማጠንከሪያው ያልተሳካው የተዘጋ ጅምር እውነትን አሰፋ
ናሙና IVM CUDA/የብረት ማፍጠኛ መንገዶች; ሥራው አዲስ ነገር አላረጋገጠም
ያልተሳካ ክፍት ጉዳይ. በናሙና የተሰራው ብረት Ed25519
ብዙ ref10 ተንሸራታች ከተስተካከለ በኋላ የፊርማ መንገድ አሁን በዚህ አስተናጋጅ ላይ ወደነበረበት ተመልሷል
በብረታ ብረት/CUDA ወደቦች ውስጥ ያሉ ነጥቦች፡- በማረጋገጫ ላይ አወንታዊ ነጥብ አያያዝ፣
የ `d2` ቋሚ፣ ትክክለኛው `fe_sq2` የመቀነሻ መንገድ፣ የባዘነው የመጨረሻ
`fe_mul` መሸከም ደረጃ፣ እና የጎደለው የድህረ-op መስክ መደበኛነት እጅና እግርን የሚፈቅድ
ድንበሮች በስክላር መሰላል ላይ ይንሸራተታሉ። አሁን ያተኮረ የብረት መመለሻ ሽፋን
የፊርማ ቧንቧው እንዲነቃ ያደርገዋል እና `[true, false]` በ ላይ ያረጋግጣልበሲፒዩ ማመሳከሪያ መንገድ ላይ አፋጣኝ. በናሙና የቀረበው የጅምር እውነት አሁን ተቀምጧል
እንዲሁም የቀጥታ ቬክተርን (`vadd64`፣ `vand`፣ `vxor`፣ `vor`) እና
ነጠላ-ዙር AES ባች ከርነሎች በሁለቱም በብረት እና በCUDA ከጀርባዎቹ በፊት
እንደነቃ መቆየት። በኋላ የጥገኝነት ቅኝት ሰባት የቀጥታ የሶስተኛ ወገን ግኝቶችን አክሏል።
ወደ ኋላ መዝገብ, ነገር ግን አሁን ያለው ዛፍ ሁለቱንም ንቁ `tar` አስወግዷል
የ `xtask` Rust `tar` ጥገኝነትን በመጣል እና በመተካት ምክሮች
`iroha_crypto` `libsodium-sys-stable` interop ሙከራዎች በክፍት ኤስኤስኤል የሚደገፍ
ተመጣጣኝ. የአሁኑ ዛፍ ቀጥተኛ የ PQ ጥገኛዎችን ተክቷል
በዚያ ጠራርጎ፣ `soranet_pq`፣ `iroha_crypto`፣ እና `ivm` በመሰደድ ላይ
`pqcrypto-dilithium` / `pqcrypto-kyber` ወደ
`pqcrypto-mldsa` / `pqcrypto-mlkem` ያለውን ML-DSA በመጠበቅ ላይ ሳለ /
ML-KEM ኤፒአይ ወለል። በኋላ በተመሳሳይ ቀን ጥገኝነት ማለፊያ የስራ ቦታውን ሰካ
`reqwest`/`rustls` ስሪቶች ወደ ተለጣፊ ልቀቶች፣ ይህም ያስቀምጣል።
`rustls-webpki` በቋሚ `0.103.10` መስመር ላይ አሁን ባለው መፍትሄ። ብቸኛው
የቀሩት የጥገኝነት ፖሊሲ ልዩ ሁኔታዎች ሁለቱ ተሻጋሪ ያልተጠበቁ ናቸው።
ማክሮ ሣጥኖች (`derivative`፣ `paste`)፣ እነዚህም አሁን በ ውስጥ በግልጽ ይቀበላሉ
`deny.toml` ምንም አስተማማኝ ማሻሻያ ስለሌለ እና እነሱን ማስወገድ ይጠይቃል
የበርካታ የላይ ዥረት ቁልሎችን በመተካት ወይም በመሸጥ ላይ። የቀሪው የማፍጠን ስራ ነው።
የተንጸባረቀውን CUDA መጠገን እና የተዘረጋው የCUDA እውነት የሩጫ ጊዜ ማረጋገጫ
የቀጥታ የCUDA አሽከርካሪ ድጋፍ ያለው አስተናጋጅ፣ የተረጋገጠ ትክክለኛነት ወይም አለመሳካት አይደለም።
አሁን ባለው ዛፍ ላይ ችግር.

#ከፍተኛ ከባድነት

### SEC-05፡ የመተግበሪያ ቀኖናዊ ጥያቄ ማረጋገጫ ባለብዙ ሲግ ገደቦችን ማለፍ (ዝግ 2026-03-24)

ተጽዕኖ፡

- ባለብዙ ሲግ ቁጥጥር የሚደረግበት መለያ ማንኛውም ነጠላ አባል ቁልፍ ሊፈቅድ ይችላል።
  ገደብ ወይም ክብደት ያስፈልጋቸዋል የተባሉ መተግበሪያዎችን የሚመለከቱ ጥያቄዎች
  ምልአተ ጉባኤ
- ይህ `verify_canonical_request` የሚያምን እያንዳንዱን የመጨረሻ ነጥብ ይነካል።
  Soracloud ሚውቴሽን መግባት፣ የይዘት መዳረሻ እና የተፈረመበት መለያ ZK ፈርሟል
  አባሪ ተከራይ.

ማስረጃ፡-

- `verify_canonical_request` ባለብዙ ሲግ መቆጣጠሪያን ወደ ሙሉ አባል ያሰፋዋል።
  የህዝብ ቁልፍ ዝርዝር እና ጥያቄውን የሚያረጋግጥ የመጀመሪያውን ቁልፍ ይቀበላል
  ፊርማ፣ ገደብ ወይም የተጠራቀመ ክብደት ሳይገመገም፡
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- ትክክለኛው የባለብዙ ሲግ ፖሊሲ ሞዴል ሁለቱንም `threshold` እና ሚዛን ይይዛል
  አባላት፣ እና ገደባቸው ከጠቅላላ ክብደት የሚበልጥ ፖሊሲዎችን ውድቅ ያደርጋል፡-
  `crates/iroha_data_model/src/account/controller.rs:92-95`፣
  `crates/iroha_data_model/src/account/controller.rs:163-178`፣
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- ረዳቱ በሶራክሎድ ሚውቴሽን ወደ ውስጥ ለመግባት በፈቃድ መንገድ ላይ ነው።
  `crates/iroha_torii/src/lib.rs:2141-2157`፣ የይዘት የተፈረመ የመለያ መዳረሻ
  `crates/iroha_torii/src/content.rs:359-360`፣ እና የአባሪ ተከራይ ውል በ
  `crates/iroha_torii/src/lib.rs:7962-7968`.

ይህ ለምን አስፈላጊ ነው:- የጥያቄው ፈራሚ ለኤችቲቲፒ ለመግባት እንደ መለያ ባለስልጣን ይቆጠራል ፣
  ግን አተገባበሩ የባለብዙ ሲግ አካውንቶችን በጸጥታ ወደ “ማንኛውም ነጠላ ዝቅ ያደርገዋል
  አባል ብቻውን ሊሠራ ይችላል."
- ይህ የመከላከያ ጥልቀት ያለው የኤችቲቲፒ ፊርማ ንብርብር ወደ ፍቃድ ይለውጠዋል
  ለብዙ ሲግ-የተጠበቁ መለያዎች ማለፍ።

ምክር፡-

- ወይም በመተግበሪያ-አውዝ ንብርብር ላይ ባለብዙ ሲግ-ቁጥጥር መለያዎችን እስከ ሀ
  ትክክለኛው የምሥክርነት ቅርጸት አለ፣ ወይም ፕሮቶኮሉን ያራዝመዋል ስለዚህ የኤችቲቲፒ ጥያቄ
  ደፍ የሚያረካ ሙሉ ባለብዙ ሲግ ምስክር ስብስብ ተሸክሞ ያረጋግጣል
  ክብደት.
- የ Soracloud ሚውቴሽን መካከለኛ ዌርን፣ የይዘት ማረጋገጫን እና ZKን የሚሸፍኑ ድግግሞሾችን ያክሉ
  ከገደብ በታች ባለ ብዙ ሲግ ፊርማዎች ማያያዣዎች።

የማስተካከያ ሁኔታ፡

- በባለብዙ ሲግ ቁጥጥር ስር ባሉ መለያዎች ላይ ዝግ ባለመሆኑ በአሁኑ ኮድ ተዘግቷል።
  `crates/iroha_torii/src/app_auth.rs`.
- አረጋጋጩ ከአሁን በኋላ "ማንኛውም አባል መፈረም ይችላል" የትርጉም ጽሑፎችን አይቀበልም።
  ባለብዙ ሲግ HTTP ፈቃድ; የባለብዙ ሲግ ጥያቄዎች እስከ ሀ
  ደረጃን የሚያረካ የምሥክርነት ቅርጸት አለ።
- የመመለሻ ሽፋን አሁን የተወሰነ ባለብዙ ሲግ ውድቅ የማድረግ ጉዳይን ያካትታል
  `crates/iroha_torii/src/app_auth.rs`.

#ከፍተኛ ከባድነት

### SEC-06፡ የመተግበሪያ ቀኖናዊ ጥያቄ ፊርማዎች ላልተወሰነ ጊዜ እንደገና መጫወት የሚችሉ ነበሩ (ዝግ 2026-03-24)

ተጽዕኖ፡- የተያዘ ትክክለኛ ጥያቄ እንደገና ሊጫወት ይችላል ምክንያቱም የተፈረመው መልእክት ቁ
  የጊዜ ማህተም፣ ምንም፣ ጊዜው ያበቃል ወይም መሸጎጫውን እንደገና ያጫውቱ።
- ይህ ሁኔታን የሚቀይሩ የሶራክሎድ ሚውቴሽን ጥያቄዎችን መድገም እና እንደገና ማውጣት ይችላል።
  ከዋናው ደንበኛ ከረጅም ጊዜ በኋላ በሂሳብ ላይ የተመሰረተ ይዘት/አባሪ ስራዎች
  አሰበባቸው።

ማስረጃ፡-

- Torii የመተግበሪያውን ቀኖናዊ ጥያቄ እንደ ብቻ ይገልጻል
  `METHOD + path + sorted query + body hash` ውስጥ
  `crates/iroha_torii/src/app_auth.rs:1-17` እና
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- አረጋጋጩ `X-Iroha-Account` እና `X-Iroha-Signature` ብቻ ይቀበላል እና ያደርጋል።
  ትኩስነትን አለማስገደድ ወይም የመድገም መሸጎጫ አለመያዝ፡-
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- የጄኤስ፣ ስዊፍት እና አንድሮይድ ኤስዲኬ ረዳቶች አንድ አይነት መልሶ ማጫወት የሚጋለጥ ራስጌ ያመነጫሉ።
  ያለጊዜ/የጊዜ ማህተም መስኮች ያጣምሩ፡
  `javascript/iroha_js/src/canonicalRequest.js:50-82`፣
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`፣ እና
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- የTorii ኦፕሬተር-ፊርማ መንገድ ቀድሞውኑ ጠንካራውን ስርዓተ-ጥለት ይጠቀማል
  መተግበሪያን የሚመለከት ዱካ ይጎድላል፡ የጊዜ ማህተም፣ ኖንስ እና መሸጎጫውን እንደገና አጫውት።
  `crates/iroha_torii/src/operator_signatures.rs:1-21` እና
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

ይህ ለምን አስፈላጊ ነው:

- HTTPS ብቻ በተገላቢጦሽ ፕሮክሲ፣ ማረም ሎገር፣
  የተጠለፈ የደንበኛ አስተናጋጅ፣ ወይም ማንኛውም አማላጅ ትክክለኛ ጥያቄዎችን መመዝገብ ይችላል።
- ተመሳሳይ እቅድ በሁሉም ዋና ደንበኛ ኤስዲኬዎች ውስጥ ስለሚተገበር፣ ድጋሚ ማጫወት
  ድክመት ከአገልጋይ-ብቻ ይልቅ ስልታዊ ነው።

ምክር፡-- የተፈረመ አዲስነት ነገርን ወደ መተግበሪያ-አውድ ጥያቄዎች በትንሹ የጊዜ ማህተም ይጨምሩ
  እና ምንም፣ እና የቆዩ ወይም እንደገና ጥቅም ላይ የዋሉ ቱፕልሎችን በታሰረ የድጋሚ ማጫወቻ መሸጎጫ አትቀበሉ።
- Torii እና ኤስዲኬዎች እንዲችሉ የመተግበሪያውን ቀኖናዊ-ጥያቄ ቅርጸት በግልፅ ያውጡ
  የድሮውን ባለ ሁለት ራስጌ እቅድ በደህና ያስወግዱት።
- ለ Soracloud ሚውቴሽን፣ ይዘት የድጋሚ አጫውት አለመቀበልን የሚያረጋግጡ ድግግሞሾችን ያክሉ
  መዳረሻ, እና አባሪ CRUD.

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. Torii አሁን ባለአራት-ራስጌ እቅድ ያስፈልገዋል
  (`X-Iroha-Account`፣ `X-Iroha-Signature`፣ `X-Iroha-Timestamp-Ms`፣
  `X-Iroha-Nonce`) እና ምልክቶች/ያረጋግጣሉ
  `METHOD + path + sorted query + body hash + timestamp + nonce` ውስጥ
  `crates/iroha_torii/src/app_auth.rs`.
- ትኩስነት ማረጋገጫ አሁን የታሰረ የሰዓት-skew መስኮትን ያስፈጽማል፣ ያረጋግጣል
  ምንም ቅርጽ የለውም፣ እና በድጋሚ ጥቅም ላይ የዋለ noncesን ከውስጠ-ትውስታ ድጋሚ አጫውት መሸጎጫ ጋር ውድቅ ያደርጋል
  ቁልፎች በ `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` በኩል ተዘርግተዋል።
- የጄኤስ፣ ስዊፍት እና አንድሮይድ ረዳቶች አሁን ተመሳሳይ ባለአራት-ራስጌ ቅርጸት በ ውስጥ ይለቃሉ
  `javascript/iroha_js/src/canonicalRequest.js`፣
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`፣ እና
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- የመልሶ ማቋቋም ሽፋን አሁን አዎንታዊ የፊርማ ማረጋገጫን እና እንደገና ማጫወትን ያጠቃልላል።
  ጊዜ ያለፈበት ማህተም፣ እና የጎደሉ-ትኩስነት ውድቅ ጉዳዮች በ ውስጥ
  `crates/iroha_torii/src/app_auth.rs`.

## መካከለኛ ክብደት

### SEC-07፡ mTLS ማስፈጸሚያ የሚታመነው የሚተላለፍ ራስጌ ነው (ዝግ 2026-03-24)

ተጽዕኖ፡- በ `require_mtls` ላይ የተመሰረቱ ማሰማራት Torii በቀጥታ ከሆነ ሊታለፍ ይችላል
  ሊደረስበት የሚችል ወይም የፊት ተኪ በደንበኛ የቀረበውን አያራግፍም።
  `x-forwarded-client-cert`.
- ጉዳዩ በማዋቀር ላይ የተመሰረተ ነው፣ ሲቀሰቀስ ግን ይገባኛል ጥያቄን ይቀየራል።
  የደንበኛ ሰርተፍኬት መስፈርት ወደ ግልጽ ራስጌ ቼክ።

ማስረጃ፡-

- Norito-RPC gating `require_mtls` በመደወል ያስፈጽማል
  `norito_rpc_mtls_present`፣ አለመሆኑን ብቻ የሚያረጋግጥ
  `x-forwarded-client-cert` አለ እና ባዶ አይደለም፡
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- ኦፕሬተር-አውዝ ቡትስትራክ/የመግቢያ ፍሰቶች `check_common` ይደውላሉ፣ይህም ብቻ ውድቅ ያደርጋል።
  `mtls_present(headers)` ውሸት ሲሆን
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` እንዲሁ ባዶ ያልሆነ `x-forwarded-client-cert` ቼክ ብቻ ነው።
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- እነዚያ ከዋኝ-አውዝ ተቆጣጣሪዎች አሁንም እንደ መንገድ ተጋልጠዋል
  `crates/iroha_torii/src/lib.rs:16658-16672`.

ይህ ለምን አስፈላጊ ነው:

- የተላለፈ ራስጌ ኮንቬንሽን ታማኝ የሚሆነው Torii ከኋላ ሲቀመጥ ብቻ ነው።
  ራስጌውን ነቅሎ እንደገና የሚጽፍ ጠንካራ ፕሮክሲ። ኮዱ አያረጋግጥም።
  ያ የማሰማራት ግምት ራሱ።
- በተገላቢጦሽ ንፅህና ላይ በፀጥታ የሚወሰኑ የደህንነት ቁጥጥሮች ቀላል ናቸው።
  በመድረክ፣ በካናሪ ወይም በአደጋ ምላሽ የማዞሪያ ለውጦች ወቅት የተሳሳተ ውቅረት።

ምክር፡-- ከተቻለ ቀጥተኛ የትራንስፖርት-ግዛት ማስፈጸሚያን ይምረጡ። ተኪ መሆን ካለበት
  ጥቅም ላይ የዋለ፣ የተረጋገጠ ፕሮክሲ-ወደ-Torii ሰርጥ እመኑ እና የፈቃድ ዝርዝር ጠይቅ
  ወይም በጥሬ ራስጌ መገኘት ፈንታ ከዚያ ተኪ የተፈረመ ማረጋገጫ።
- `require_mtls` በቀጥታ በተጋለጡ Torii አድማጮች ላይ ደህንነቱ ያልተጠበቀ መሆኑን ሰነድ።
- ለተጭበረበሩ `x-forwarded-client-cert` ግቤት በNorito-RPC ላይ አሉታዊ ሙከራዎችን ያክሉ
  እና ከዋኝ-አውት ቡትስትራፕ መንገዶች።

የማስተካከያ ሁኔታ፡

- የተላለፈ-ራስጌ እምነትን ወደ የተዋቀረው ፕሮክሲ በማሰር በአሁኑ ኮድ ተዘግቷል።
  በጥሬ ራስጌ መገኘት ፈንታ CIDRs።
- `crates/iroha_torii/src/limits.rs` አሁን የተጋራውን ያቀርባል
  `has_trusted_forwarded_header(...)` በር፣ እና ሁለቱም Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) እና ከዋኝ-አዋጅ
  (`crates/iroha_torii/src/operator_auth.rs`) ከደዋዩ TCP እኩያ ጋር ይጠቀሙበት
  አድራሻ.
- `iroha_config` አሁን `mtls_trusted_proxy_cidrs` ለሁለቱም ያጋልጣል
  ኦፕሬተር-አውዝ እና Norito-RPC; ነባሪዎች loopback-ብቻ ናቸው።
- የድጋሚ ሽፋን አሁን የተጭበረበረ `x-forwarded-client-cert` ግቤትን ውድቅ ያደርጋል
  የማይታመን የርቀት መቆጣጠሪያ በሁለቱም ኦፕሬተር-አውት እና የተጋራ ገደብ ረዳት።

## መካከለኛ ክብደት

### SEC-08፡ ወደ ውጪ የሚሄዱ የP2P መደወያዎች የተረጋገጠውን ቁልፍ ከታሰበው የአቻ መታወቂያ ጋር አላሰሩም (ዝግ 2026-03-25)

ተጽዕኖ፡- ለአቻ `X` ወደ ውጭ የሚወጣ መደወያ እንደማንኛውም እኩያ `Y` ቁልፉ ሊጠናቀቅ ይችላል
  የመተግበሪያ-ንብርብሩን መጨባበጥ በተሳካ ሁኔታ ፈርመዋል፣ ምክንያቱም መጨባበጥ
  "በዚህ ግኑኝነት ላይ ያለ ቁልፍ" አረጋግጧል ነገር ግን ቁልፉ መሆኑን በጭራሽ አላጣራም።
  ለመድረስ የታሰበው የአውታረ መረብ ተዋናይ የእኩያ መታወቂያ።
- በተፈቀደ ተደራቢዎች የኋለኛው ቶፖሎጂ/የፍቀድ ዝርዝር ቼኮች አሁንም ይጥላሉ
  የተሳሳተ ቁልፍ፣ ስለዚህ ይህ በዋነኛነት ምትክ/ተደራሽነት ስህተት ነበር።
  ከቀጥታ ስምምነት የማስመሰል ስህተት ይልቅ። በሕዝብ ተደራቢዎች ውስጥ ሀ
  የተጠለፈ አድራሻ፣ የዲ ኤን ኤስ ምላሽ ወይም የማስተላለፊያ የመጨረሻ ነጥብ ሌላ ይተካሉ።
  ወደ ውጭ በሚወጣ መደወያ ላይ የተመልካች ማንነት.

ማስረጃ፡-

- የወጪው አቻ ግዛት የታሰበውን `peer_id` ውስጥ ያከማቻል
  `crates/iroha_p2p/src/peer.rs:5153-5179` ፣ ግን የድሮው የእጅ መጨባበጥ ፍሰት
  ከፊርማው ማረጋገጫ በፊት ያንን ዋጋ ወርዷል።
- `GetKey::read_their_public_key` የተፈረመ የእጅ መጨባበጥ ጭነት እና አረጋግጧል
  ከዚያ ወዲያውኑ `Peer` ከማስታወቂያው የርቀት የህዝብ ቁልፍ ገንብቷል
  በ `crates/iroha_p2p/src/peer.rs:6266-6355` ውስጥ, ከ
  `peer_id` በመጀመሪያ ለ `connecting(...)` ቀርቧል።
- ተመሳሳይ የትራንስፖርት ቁልል የTLS/QUIC ሰርተፍኬትን በግልፅ ያሰናክላል
  ማረጋገጫ ለ P2P በ `crates/iroha_p2p/src/transport.rs` ፣ ስለዚህ ማሰር
  የመተግበሪያ-ንብርብር የተረጋገጠ ቁልፍ ለታሰበው የአቻ መታወቂያ ወሳኝ ነው።
  ወደ ውጪ በሚደረጉ ግንኙነቶች ላይ የማንነት ማረጋገጫ.

ይህ ለምን አስፈላጊ ነው:- ዲዛይኑ ሆን ብሎ የአቻ ማረጋገጫን ከመጓጓዣው በላይ ይገፋል
  ንብርብር፣ ይህም የመጨባበጥ ቁልፍ ብቸኛው ዘላቂ የማንነት ማሰሪያ መሆኑን ያረጋግጣል
  ወደ ውጭ በሚወጡ መደወያዎች ላይ።
- ያ ቼክ ከሌለ የአውታረ መረብ ንብርብር በጸጥታ " በተሳካ ሁኔታ ማከም ይችላል።
  አንዳንድ እኩዮችን አረጋግጧል" ከ "የደወልንለት እኩያ ጋር ደረስን" ከሚለው ጋር እኩል ነው።
  ይህም ደካማ ዋስትና ነው እና ቶፖሎጂ / መልካም ስም ሁኔታ ሊያዛባ ይችላል.

ምክር፡-

- የታሰበውን ወደ ውጭ የሚወጣ `peer_id` በተፈረሙ የእጅ መጨባበጥ ደረጃዎች እና
  የተረጋገጠው የርቀት ቁልፍ ካልተዛመደ አልተዘጋም።
- በትክክል የተፈረመ የእጅ መጨባበጥ የሚያረጋግጥ በትኩረት ተደግፎ ይያዙ
  የተለመደው የተፈረመ የእጅ መጨባበጥ ሲሳካ የተሳሳተ ቁልፍ ውድቅ ይደረጋል።

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `ConnectedTo` እና የታችኛው ተፋሰስ የእጅ መጨባበጥ ግዛቶች አሁን
  የሚጠበቀውን ወደ ውጭ መውጣት `PeerId`, እና
  `GetKey::read_their_public_key` ያልተዛመደ የተረጋገጠ ቁልፍን ውድቅ ያደርጋል
  `HandshakePeerMismatch` በ `crates/iroha_p2p/src/peer.rs`።
- ያተኮረ የተሃድሶ ሽፋን አሁን ያካትታል
  `outgoing_handshake_rejects_unexpected_peer_identity` እና ያለው
  አዎንታዊ `handshake_v1_defaults_to_trust_gossip` ዱካ
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09፡ HTTPS/WSS የዌብ መንጠቆ ማቅረቢያ በድጋሚ የተፈቱ የተረጋገጡ የአስተናጋጅ ስሞች በግንኙነት ጊዜ (ዝግ 2026-03-25)

ተጽዕኖ፡- ደህንነቱ የተጠበቀ የድር መንጠቆ ማቅረቢያ የተረጋገጠ የመድረሻ ዲ ኤን ኤስ መልሶች ከዌብ መንጠቆው ጋር
  የመውጣት ፖሊሲ፣ ግን ከዚያ በኋላ እነዚያን የተረጋገጡ አድራሻዎች ተጥሎ ለደንበኛው መፍቀድ
  ቁልል የአስተናጋጁን ስም እንደገና በ HTTPS ወይም WSS ግንኙነት ጊዜ ይፍቱ።
- በማረጋገጫ እና በማገናኘት ጊዜ መካከል ዲ ኤን ኤስ ላይ ተጽዕኖ የሚያደርግ አጥቂ ይችላል።
  ከዚህ ቀደም የተፈቀደውን የአስተናጋጅ ስም ወደ የታገደ የግል ወይም እንደገና ማያያዝ ይችላል።
  ከዋኝ-ብቻ መድረሻ እና በCIDR ላይ የተመሰረተ የድር መንጠቆ ጥበቃን ማለፍ።

ማስረጃ፡-

- የኢግሬስ ጠባቂው እጩ መድረሻ አድራሻዎችን ይፈታል እና ያጣራል።
  `crates/iroha_torii/src/webhook.rs:1746-1829` ፣ እና ደህንነቱ የተጠበቀ የመላኪያ መንገዶች
  እነዚያን የተረጋገጡ የአድራሻ ዝርዝሮችን ወደ HTTPS/WSS ረዳቶች ያስተላልፉ።
- የድሮው HTTPS ረዳት ከዋናው ዩአርኤል አንጻር አጠቃላይ ደንበኛ ገንብቷል።
  በ `crates/iroha_torii/src/webhook.rs` ውስጥ አስተናጋጅ እና ግንኙነቱን አላሰረም።
  ወደ ተጣራው የአድራሻ ስብስብ, ይህም ማለት የዲ ኤን ኤስ መፍታት በውስጡ እንደገና ተከስቷል
  የኤችቲቲፒ ደንበኛ።
- የድሮው WSS ረዳት `tokio_tungstenite::connect_async(url)` ተብሎም ይጠራል
  ከዋናው የአስተናጋጅ ስም ጋር ይቃረናል፣ እሱም እንዲሁም አስተናጋጁን በምትኩ እንደገና ፈትቷል።
  አስቀድሞ የተፈቀደውን አድራሻ እንደገና መጠቀም።

ይህ ለምን አስፈላጊ ነው:

- የመድረሻ ፍቃድ ዝርዝሮች የሚሰሩት የተፈተሸ አድራሻ ከሆነ ብቻ ነው።
  ደንበኛው በትክክል ይገናኛል.
- ከፖሊሲ ፈቃድ በኋላ እንደገና መፍታት የዲ ኤን ኤስ መልሶ ማገናኘት / TOCTU ክፍተትን በ ሀ
  ኦፕሬተሮች ለSSRF-style መያዣ የሚያምኑበት መንገድ።

ምክር፡-- በማቆየት ጊዜ የተጣራ የዲ ኤን ኤስ መልሶችን በትክክለኛው የኤችቲቲፒኤስ የግንኙነት መንገድ ላይ ይሰኩ።
  ለ SNI / የምስክር ወረቀት ማረጋገጫ ዋናው የአስተናጋጅ ስም።
- ለ WSS፣ የTCP ሶኬትን በቀጥታ ከተረጋገጠ አድራሻ ጋር ያገናኙ እና TLSን ያሂዱ
  የዌብሶኬት መጨባበጥ የአስተናጋጅ ስም-ተኮር ከመጥራት ይልቅ በዚያ ዥረት ላይ
  ምቹ አያያዥ.

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_torii/src/webhook.rs` አሁን ያገኛል
  `https_delivery_dns_override(...)` እና
  `websocket_pinned_connect_addr(...)` ከተጣራው አድራሻ ስብስብ።
- HTTPS መላኪያ አሁን `reqwest::Client::builder().resolve_to_addrs(...)` ይጠቀማል
  ስለዚህ ዋናው የአስተናጋጅ ስም TCP ግኑኙነቱ እያለ ለTLS ይታያል
  አስቀድሞ ከጸደቁ አድራሻዎች ጋር ተያይዟል።
- WSS መላኪያ አሁን ጥሬውን `TcpStream` ለተረጋገጠ አድራሻ ይከፍታል እና ያከናውናል
  `tokio_tungstenite::client_async_tls_with_config(...)` በዚያ ዥረት ላይ፣
  ከፖሊሲ ማረጋገጫ በኋላ ሁለተኛ የዲ ኤን ኤስ ፍለጋን ያስወግዳል።
- የመመለሻ ሽፋን አሁን ያካትታል
  `https_delivery_dns_override_pins_vetted_domain_addresses`፣
  `https_delivery_dns_override_skips_ip_literals`፣ እና
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` ውስጥ
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10፡ የኤምሲፒ የውስጥ መስመር ማተም የታተመ loopback እና የተወረሰ የፍቃድ ዝርዝር ልዩ መብት (ዝግ 2026-03-25)

ተጽዕኖ፡- Torii ኤምሲፒ ሲነቃ፣ የውስጥ መሣሪያ መላክ እያንዳንዱን ጥያቄ እንደሚከተለው ጻፈ።
  ትክክለኛው ደዋይ ምንም ይሁን ምን loopback. ደዋይ CIDRን የሚያምኑባቸው መንገዶች
  ልዩ መብት ወይም ስሮትልንግ ማለፊያ የኤምሲፒ ትራፊክን እንደ ማየት ይችላል።
  `127.0.0.1`.
ኤምሲፒ በነባሪነት ስለተሰናከለ እና ጉዳዩ በማዋቀር-የተዘጋ ነበር።
  ጉዳት የደረሰባቸው መንገዶች አሁንም በተፈቀደ ዝርዝር ወይም ተመሳሳይ የሎፕ ጀርባ እምነት ላይ ይወሰናሉ።
  ፖሊሲ፣ ነገር ግን ኤምሲፒን አንዴ ወደ ልዩ ጥቅም የሚያሰፋ ድልድይ ቀይሮታል።
  ቁልፎች አንድ ላይ ነቅተዋል።

ማስረጃ፡-

- `dispatch_route(...)` በ `crates/iroha_torii/src/mcp.rs` ከዚህ ቀደም ገብቷል
  `x-iroha-remote-addr: 127.0.0.1` እና ሠራሽ loopback `ConnectInfo` ለ
  እያንዳንዱ የውስጥ የተላከ ጥያቄ.
- `iroha.parameters.get` በ MCP ገጽ ላይ በተነባቢ-ብቻ ሁነታ ተጋልጧል፣ እና
  `/v1/parameters` የደዋዩ አይፒ አባል በሚሆንበት ጊዜ መደበኛውን ማረጋገጫ ያልፋል
  በ`crates/iroha_torii/src/lib.rs:5879-5888` ውስጥ የተዋቀረ የፍቃድ ዝርዝር።
- `apply_extra_headers(...)` እንዲሁም የዘፈቀደ `headers` ግቤቶችን ተቀብሏል ከ
  የMCP ደዋይ፣ ስለዚህ የተጠበቁ የውስጥ እምነት ራስጌዎች እንደ
  `x-iroha-remote-addr` እና `x-forwarded-client-cert` በግልጽ አልነበሩም
  የተጠበቀ።

ይህ ለምን አስፈላጊ ነው:- የውስጥ ድልድይ ንብርብሮች የመጀመሪያውን የመተማመን ወሰን መጠበቅ አለባቸው። መተካት
  loopback ያለው እውነተኛው ደዋይ እያንዳንዱን የMCP ደዋይ በብቃት እንደ
  ጥያቄው ድልድዩን ካቋረጠ በኋላ የውስጥ ደንበኛ።
- ስህተቱ ረቂቅ ነው ምክንያቱም በውጫዊ የሚታየው MCP መገለጫ አሁንም ሊመስል ይችላል።
  የውስጣዊ የኤችቲቲፒ መስመር የበለጠ ልዩ መብት ያለው ምንጭ ሲያይ ማንበብ ብቻ።

ምክር፡-

- የውጪው `/v1/mcp` ጥያቄ የተቀበለውን ደዋይ አይፒን ጠብቅ
  ከ Torii የርቀት አድራሻ መካከለኛ ዌር እና `ConnectInfo` ከ
  ከ loopback ይልቅ ያ እሴት።
- እንደ `x-iroha-remote-addr` እና ያሉ የመግቢያ-ብቻ እምነት ራስጌዎችን ያክሙ
  `x-forwarded-client-cert` እንደ የተጠበቁ የውስጥ ራስጌዎች ስለዚህ MCP ደዋዮች አይችሉም
  በ `headers` ክርክር በኩል ማሸጋገር ወይም መሻር።

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_torii/src/mcp.rs` አሁን ያገኘዋል።
  በውስጥ የተላከ የርቀት አይፒ ከውጪው ጥያቄ መርፌ
  `x-iroha-remote-addr` ራስጌ እና `ConnectInfo` ከተጨባጭ ያዋህዳል
  ከ loopback ይልቅ ደዋይ አይፒ።
- `apply_extra_headers(...)` አሁን ሁለቱንም `x-iroha-remote-addr` እና
  `x-forwarded-client-cert` እንደ የተጠበቁ የውስጥ ራስጌዎች፣ ስለዚህ የኤምሲፒ ደዋዮች
  በመሳሪያ ክርክሮች በኩል loopback / ingress-proxy እምነትን ማመንጨት አይችልም።
- የመመለሻ ሽፋን አሁን ያካትታል
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`፣ እና
  `apply_extra_headers_blocks_reserved_internal_headers` ውስጥ
  `crates/iroha_torii/src/mcp.rs`.### SEC-11፡ የኤስዲኬ ደንበኞች ደህንነታቸው ባልተጠበቀ ወይም በአቋራጭ አስተናጋጅ መጓጓዣዎች ላይ ሚስጥራዊነት ያለው የጥያቄ ቁሳቁስ ፈቅደዋል (ዝግ 2026-03-25)

ተጽዕኖ፡

- ለናሙና የቀረበው ስዊፍት፣ ጃቫ/አንድሮይድ፣ ኮትሊን እና JS ደንበኞች አላደረጉም።
  ሁሉንም ሚስጥራዊነት ያላቸው የጥያቄ ቅርጾችን እንደ መጓጓዣ-ስሱ አድርገው ይያዙ።
  በእርዳታ ሰጪው ላይ በመመስረት፣ ደዋዮች ተሸካሚ/ኤፒአይ-ቶከን ራስጌዎችን፣ ጥሬዎችን መላክ ይችላሉ።
  `private_key*` JSON መስኮች፣ ወይም የመተግበሪያ ቀኖናዊ-የመፈረሚያ ቁሳቁስ በላይ
  ግልጽ `http` / `ws` ወይም በአቋራጭ አስተናጋጅ ፍፁም ዩአርኤል ይሽራል።
- በJS ደንበኛ በተለይ `canonicalAuth` ራስጌዎች ከተጨመሩ በኋላ
  `_request(...)` የትራንስፖርት ፍተሻውን ጨርሷል፣ እና አካል-ብቻ `private_key`
  JSON እንደ ሚስጥራዊነት መጓጓዣ አልቆጠረም።

ማስረጃ፡-- ስዊፍት አሁን ጠባቂውን ወደ ውስጥ ያማክራል።
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` እና ከ ይተገበራል
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`፣
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`፣ እና
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; ከዚህ ማለፊያ በፊት
  እነዚህ ረዳቶች አንድ የትራንስፖርት ፖሊሲ በር አልተጋሩም።
- ጃቫ/አንድሮይድ አሁን ተመሳሳይ ፖሊሲን ያማከለ ነው።
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  እና ከ `NoritoRpcClient.java`፣ `ToriiRequestBuilder.java`፣
  `OfflineToriiClient.java`፣ `SubscriptionToriiClient.java`፣
  `stream/ToriiEventStreamClient.java`፣ እና
  `websocket/ToriiWebSocketClient.java`.
- ኮትሊን አሁን ያንን ፖሊሲ ያንፀባርቃል
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  እና ከተዛማጅ የJVM ደንበኛ/ጥያቄ-ገንቢ/የክስተት-ዥረት/ ይተገበራል።
  የዌብሶኬት ወለሎች.
- JS `ToriiClient._request(...)` አሁን `canonicalAuth` እና JSON አካላትን ያስተናግዳል።
  የ `private_key*` መስኮችን እንደ ሚስጥራዊ የመጓጓዣ ቁሳቁስ የያዘ
  `javascript/iroha_js/src/toriiClient.js`፣ እና የቴሌሜትሪ ክስተት ቅርፅ በ ውስጥ
  `javascript/iroha_js/index.d.ts` አሁን `hasSensitiveBody` / መዝግቧል
  `hasCanonicalAuth` `allowInsecure` ጥቅም ላይ ሲውል.

ይህ ለምን አስፈላጊ ነው:

- ሞባይል፣ አሳሽ እና የአካባቢ-ዴቭ ረዳቶች ብዙ ጊዜ የሚቀያየር ዴቭ/ዝግጅት ላይ ይጠቁማሉ
  ቤዝ URLs. ደንበኛው ሚስጥራዊነት ያላቸው ጥያቄዎችን ወደ የተዋቀረው ካልሰካ
  እቅድ/አስተናጋጅ፣ ምቹ የሆነ ፍጹም ዩአርኤል መሻር ወይም ግልጽ-ኤችቲቲፒ መሰረት ማድረግ ይችላል።
  ኤስዲኬን ወደ ሚስጥራዊ-ማጣራት ወይም ወደ ፊርማ-ጥያቄ የማውረድ መንገድ ይለውጡት።
- አደጋው ከተሸካሚ ምልክቶች የበለጠ ሰፊ ነው። ጥሬ `private_key` JSON እና ትኩስ
  ቀኖናዊ-አውት ፊርማዎች እንዲሁ በሽቦ ላይ ደህንነትን የሚነኩ ናቸው።
  የትራንስፖርት ፖሊሲን በዝምታ ማለፍ የለበትም።

ምክር፡-- በእያንዳንዱ ኤስዲኬ ውስጥ የትራንስፖርት ማረጋገጫን ያማከለ እና ከአውታረ መረብ I/O በፊት ይተግብሩ
  ለሁሉም ሚስጥራዊነት ያላቸው የጥያቄ ቅርጾች፡ የቃል ራስጌዎች፣ የመተግበሪያ ቀኖናዊ-የማስተማር ፊርማ፣
  እና ጥሬ `private_key*` JSON.
- `allowInsecure`ን እንደ ግልጽ የአካባቢ/ዴቭ ማምለጫ ፍንጣቂ ብቻ አቆይ እና ልቀቅ
  ደዋዮች መርጠው ሲገቡ ቴሌሜትሪ።
- ላይ ብቻ ሳይሆን በጋራ ጥያቄ ግንበኞች ላይ ያተኮሩ ድጋፎችን ይጨምሩ
  ከፍተኛ-ደረጃ ምቹ ዘዴዎች, ስለዚህ የወደፊት ረዳቶች ተመሳሳይ ጠባቂ ይወርሳሉ.

የማስተካከያ ሁኔታ፡- አሁን ባለው ኮድ ተዘግቷል. በናሙና የቀረበው ስዊፍት፣ ጃቫ/አንድሮይድ፣ ኮትሊን እና ጄኤስ
  ደንበኞች አሁን ደህንነታቸው ያልተጠበቀ ወይም አቋራጭ ማጓጓዣዎችን ለስሜታዊነት አይቀበሉም።
  ደዋዩ ወደ ሰነዱ ዴቭ-ብቻ ካልገባ በስተቀር ቅርጾችን ጠይቅ
  ደህንነቱ ያልተጠበቀ ሁነታ.
- ስዊፍት ያተኮሩ ሪግሬሽን አሁን ደህንነታቸው ያልተጠበቀ የNorito-RPC ማረጋገጫ ራስጌዎችን ይሸፍናል፣
  ደህንነቱ ያልተጠበቀ የግንኙነት ዌብሶኬት ትራንስፖርት፣ እና ጥሬ-`private_key` Torii ጥያቄ
  አካላት.
- Kotlin ያተኮሩ ሪግሬሽን አሁን ደህንነታቸው ያልተጠበቀ Norito-RPC የቃል ራስጌዎችን ይሸፍናል፣
  ከመስመር ውጭ/የደንበኝነት ምዝገባ `private_key` አካላት፣ SSE auth ራስጌዎች እና የድር ሶኬት
  auth ራስጌዎች.
- በጃቫ/አንድሮይድ ላይ ያተኮረ ድግግሞሾች አሁን ደህንነቱ ያልተጠበቀ Norito-RPC ማረጋገጫን ይሸፍናሉ
  ራስጌዎች፣ ከመስመር ውጭ/የደንበኝነት ምዝገባ `private_key` አካላት፣ የኤስኤስኢ ማረጋገጫ ራስጌዎች፣ እና
  የዌብሶኬት auth ራስጌዎች በተጋራው የግራድል ማሰሪያ በኩል።
- JS ያተኮረ ሪግሬሽን አሁን ደህንነቱ ያልተጠበቀ እና አስተናጋጅ ይሸፍናል።
  `private_key`-የሰውነት ጥያቄዎች እና ደህንነታቸው ያልተጠበቀ `canonicalAuth` ጥያቄዎች በ ውስጥ
  `javascript/iroha_js/test/transportSecurity.test.js`፣ እያለ
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` አሁን አዎንታዊውን ይሰራል
  ቀኖናዊ-አውት መንገድ በአስተማማኝ መሠረት ዩአርኤል ላይ።

### SEC-12፡ SoraFS የሀገር ውስጥ QUIC ፕሮክሲ የባለጉዳይ ማረጋገጫ ሳይኖር የloopback ማሰሪያዎችን ተቀብሏል (ዝግ 2026-03-25)

ተጽዕኖ፡- `LocalQuicProxyConfig.bind_addr` ከዚህ ቀደም ወደ `0.0.0.0` ሊዋቀር ይችላል
  LAN IP፣ ወይም ሌላ ማንኛውም መልሶ የማይመለስ አድራሻ፣ እሱም "አካባቢያዊ"ን ያጋለጠው።
  የመሥሪያ ቦታ ፕሮክሲ እንደ በርቀት ሊደረስበት የሚችል የQUIC አድማጭ።
- ያ አድማጭ ደንበኞችን አላረጋገጠም። ሊደረስበት የሚችል ማንኛውም እኩያ
  የQUIC/TLS ክፍለ ጊዜን ያጠናቅቁ እና ከስሪት ጋር የተዛመደ የእጅ መጨባበጥ ይላኩ።
  ከዚያ በየትኛው ላይ በመመስረት `tcp` ፣ `norito` ፣ `car` ፣ ወይም `kaigi` ዥረቶችን ይክፈቱ።
  የድልድይ ሁነታዎች ተዋቅረዋል።
- በ`bridge` ሁነታ፣ ያ የኦፕሬተርን የተሳሳተ ውቅረት ወደ የርቀት TCP ቀይሮታል።
  በአሠሪው የሥራ ቦታ ላይ ቅብብል እና የአካባቢ-ፋይል ዥረት ወለል።

ማስረጃ፡-

- `LocalQuicProxyConfig::parsed_bind_addr(...)` ውስጥ
  `crates/sorafs_orchestrator/src/proxy.rs` ከዚህ ቀደም ሶኬቱን ብቻ ተንትኗል
  አድራሻ እና የloopback ያልሆኑ በይነገጾችን አልተቀበለም።
- `spawn_local_quic_proxy(...)` በተመሳሳይ ፋይል የQUIC አገልጋይ ይጀምራል
  በራስ የተፈረመ የምስክር ወረቀት እና `.with_no_client_auth()`።
- `handle_connection(...)` `ProxyHandshakeV1` ማንኛውንም ደንበኛ ተቀብሏል
  ሥሪት ከተደገፈው ነጠላ ፕሮቶኮል ሥሪት ጋር ይዛመዳል እና ወደ ውስጥ ገባ
  የመተግበሪያ ዥረት loop.
- `handle_tcp_stream(...)` የዘፈቀደ `authority` ዋጋዎችን ይደውላል
  `TcpStream::connect(...)`፣ `handle_norito_stream(...)` ሳለ፣
  `handle_car_stream(...)`፣ እና `handle_kaigi_stream(...)` የአካባቢ ፋይሎችን ያሰራጫሉ።
  ከተዋቀሩ spool/መሸጎጫ ማውጫዎች።

ይህ ለምን አስፈላጊ ነው:- በራስ የተፈረመ የምስክር ወረቀት የአገልጋይ ማንነትን የሚጠብቀው ደንበኛው ከሆነ ብቻ ነው።
  ለማረጋገጥ ይመርጣል። ደንበኛውን አያረጋግጥም። አንዴ ፕሮክሲው
  ከloopback ውጪ ሊደረስበት የሚችል ነበር፣ የመጨባበጥ መንገዱ የስሪት-ብቻ ነበር።
  መግቢያ
- ኤፒአይ እና ሰነዶች ይህንን ረዳት እንደ የአካባቢ የስራ ቦታ ተኪ ይገልጹታል።
  አሳሽ/ኤስዲኬ ውህደቶች፣ ስለዚህ በርቀት ሊደረስባቸው የሚችሉ ማሰሪያ አድራሻዎችን መፍቀድ ነበር።
  የታመነ የድንበር አለመዛመድ እንጂ የታሰበ የርቀት አገልግሎት ሁነታ አይደለም።

ምክር፡-

- በማንኛውም loopback ላይ ያልተዘጋ `bind_addr` ስለዚህ የአሁኑ ረዳት ሊሆን አይችልም
  ከአካባቢው የሥራ ቦታ በላይ መጋለጥ.
- የርቀት ተኪ መጋለጥ የምርት መስፈርት ከሆነ አስተዋውቁ
  ግልጽ የደንበኛ ማረጋገጫ/ችሎታ መጀመሪያ ከመግባት ይልቅ
  ማሰሪያውን ዘና ማድረግ.

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/sorafs_orchestrator/src/proxy.rs` አሁን
  ከ `ProxyError::BindAddressNotLoopback` ጋር የማይመለስ ማሰር አድራሻዎችን ውድቅ ያደርጋል
  የQUIC አድማጭ ከመጀመሩ በፊት።
- የውቅር መስክ ሰነድ በ ውስጥ
  `docs/source/sorafs/developer/orchestrator.md` እና
  `docs/portal/docs/sorafs/orchestrator-config.md` አሁን ሰነዶች
  `bind_addr` እንደ loopback-ብቻ።
- የመመለሻ ሽፋን አሁን ያካትታል
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` እና ያለው
  አዎንታዊ የአካባቢ ድልድይ ሙከራ
  `proxy::tests::tcp_stream_bridge_transfers_payload` ውስጥ
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13፡ ወደ ውጪ የሚወጣ P2P TLS-over-TCP በጸጥታ ወደ ግልጽ ጽሑፍ በነባሪነት ወርዷል (በ2026-03-25 ተዘግቷል)

ተጽዕኖ፡- `network.tls_enabled=true` ን ማንቃት TLS-ብቻን አላስከበረም።
  ኦፕሬተሮች ካላገኙ እና ካልተዘጋጁ በስተቀር ወደ ውጭ መጓጓዣ
  `tls_fallback_to_plain=false`.
- ማንኛውም TLS መጨባበጥ አለመሳካት ወይም መውጫ መንገድ ላይ ጊዜ ማብቂያ ስለዚህ
  መደወያውን በነባሪነት ወደ ግልጽ ጽሑፍ TCP ዝቅ አደረገ፣ ይህም መጓጓዣን አስወገደ
  በመንገድ ላይ ባሉ አጥቂዎች ላይ ምስጢራዊነት እና ታማኝነት
  መካከለኛ ሳጥኖች.
- የተፈረመው መተግበሪያ መጨባበጥ አሁንም የአቻ ማንነትን አረጋግጧል፣ ስለዚህ
  ይህ ከአቻ-አስመሳይ ማለፊያ ይልቅ የትራንስፖርት-ፖሊሲ ማሽቆልቆል ነበር።

ማስረጃ፡-

- `tls_fallback_to_plain` በ `true` ውስጥ ነባሪ ተደርጓል
  `crates/iroha_config/src/parameters/user.rs`፣ ስለዚህ መውደቅ ንቁ ነበር።
  ኦፕሬተሮች በ config ውስጥ በግልፅ ካልሻሩት በስተቀር።
- `Connecting::connect_tcp(...)` በ `crates/iroha_p2p/src/peer.rs` ሙከራዎች
  `tls_enabled` በተቀናበረ ቁጥር TLS ይደውሉ፣ ነገር ግን በTLS ስህተቶች ላይ ወይም ጊዜው አልፎበታል።
  ማስጠንቀቂያ ይመዘግባል እና በማንኛውም ጊዜ ወደ ግልጽ TCP ይመለሳል
  `tls_fallback_to_plain` ነቅቷል።
- በ `crates/iroha_kagami/src/wizard.rs` ውስጥ ያለው ከዋኝ ፊት ለፊት ያለው የናሙና ማዋቀር እና
  በ `docs/source/p2p*.md` ውስጥ ያለው የህዝብ P2P ትራንስፖርት ሰነዶች እንዲሁ ማስታወቂያ ተሰጥቷል
  ግልጽ ጽሁፍ መመለስ እንደ ነባሪ ባህሪ።

ይህ ለምን አስፈላጊ ነው:አንዴ ኦፕሬተሮች TLS ን ካበሩ በኋላ ደህንነቱ የተጠበቀው ተስፋ ተዘግቷል፡ TLS ከሆነ
  ክፍለ-ጊዜ መመስረት አይቻልም, መደወያው በጸጥታ ሳይሆን ውድቀት አለበት
  የማጓጓዣ መከላከያዎችን ማፍሰስ.
- ማሽቆልቆሉን በነባሪነት መተው ስምምነቱን አሳሳቢ ያደርገዋል
  የአውታረ መረብ-መንገድ ኩርኩሮች፣ የተኪ ጣልቃ ገብነት እና ንቁ የእጅ መጨባበጥ በ ሀ
  በታቀደው ጊዜ በቀላሉ የሚጠፋበት መንገድ።

ምክር፡-

- ግልጽ የጽሑፍ ውድቀትን እንደ ግልጽ የተኳኋኝነት ቁልፍ ያቆዩት ፣ ግን ነባሪ ያድርጉት
  `false` ስለዚህ `network.tls_enabled=true` ማለት ኦፕሬተሩ ካልመረጠ በስተቀር TLS-ብቻ ነው
  ወደ ዝቅጠት ባህሪ.

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_config/src/parameters/user.rs` አሁን
  ነባሪዎች `tls_fallback_to_plain` ወደ `false`።
- ነባሪው-የማዋቀር ቅጽበታዊ ገጽ እይታ፣ Kagami ናሙና ውቅር እና ነባሪ መሰል
  P2P/Torii የሙከራ ረዳቶች አሁን ያንን የጠንካራ የአሂድ ጊዜ ነባሪ ያንፀባርቃሉ።
- የተባዙት `docs/source/p2p*.md` ሰነዶች አሁን ግልጽ የጽሑፍ ውድቀትን እንደሚከተለው ይገልጹታል
  ከተላከው ነባሪ ይልቅ ግልጽ የሆነ መርጦ መግባት።

### SEC-14፡ አቻ ቴሌሜትሪ ጂኦአፕ በፀጥታ ወደ ግልጽ የሶስተኛ ወገን HTTP ወደቀ (ተዘጋ 2026-03-25)

ተጽዕኖ፡- ግልጽ የሆነ የመጨረሻ ነጥብ ሳይፈጠር `torii.peer_geo.enabled=true` ማንቃት
  Torii የአቻ አስተናጋጅ ስሞችን ወደ አብሮገነብ ግልጽ ጽሑፍ ለመላክ
  `http://ip-api.com/json/...` አገልግሎት።
- ያ የተለቀቀው የአቻ ቴሌሜትሪ ኢላማው ያልተረጋገጠ የሶስተኛ ወገን HTTP ላይ ነው።
  ጥገኝነት እና ማንኛውም የጎዳና ላይ አጥቂ ወይም የተጠለፈ የመጨረሻ ነጥብ እንዲመግብ ይፍቀዱ
  የአካባቢ ሜታዳታ ወደ Torii ይመለሱ።
- ባህሪው መርጦ መግባት ነበር፣ ነገር ግን የህዝብ ቴሌሜትሪ ሰነዶች እና የናሙና ውቅር
  አብሮ የተሰራውን ነባሪ አስተዋውቋል፣ ይህም ደህንነቱ ያልተጠበቀ የስምሪት ንድፍ አድርጓል
  ኦፕሬተሮች አንድ ጊዜ የአቻ ጂኦ ፍለጋዎችን ካነቁ በኋላ ሊሆን ይችላል።

ማስረጃ፡-

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` ቀደም ሲል የተገለፀው
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` እና
  `construct_geo_query(...)` ያንን ነባሪ በማንኛውም ጊዜ ተጠቅሟል
  `GeoLookupConfig.endpoint` `None` ነበር።
- የአቻ ቴሌሜትሪ ሞኒተር በማንኛውም ጊዜ `collect_geo(...)` ይፈልቃል
  `geo_config.enabled` እውነት ነው።
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`፣ ስለዚህ ግልጽ ጽሑፉ
  መመለስ ከሙከራ-ብቻ ኮድ ይልቅ በተላኩ የሩጫ ጊዜ ኮድ ማግኘት ይቻላል።
- ውቅሩ በ `crates/iroha_config/src/parameters/defaults.rs` እና
  `crates/iroha_config/src/parameters/user.rs` `endpoint` እንዳልተዋቀረ ይተወዋል፣ እና
  የተባዙ የቴሌሜትሪ ሰነዶች በ`docs/source/telemetry*.md` ሲደመር
  `docs/source/references/peer.template.toml` በግልፅ ሰነዱ
  ኦፕሬተሮች ባህሪውን ሲያነቁ አብሮ የተሰራ መውደቅ።

ይህ ለምን አስፈላጊ ነው:- አቻ ቴሌሜትሪ የእኩያ አስተናጋጅ ስሞችን በግልፅ ጽሑፍ HTTP ላይ በፀጥታ ማውጣት የለበትም
  ለሶስተኛ ወገን አገልግሎት ኦፕሬተሮች በምቾት ባንዲራ ላይ ሲገለብጡ።
- የተደበቀ ደህንነቱ ያልተጠበቀ ነባሪ ለውጥ ግምገማን ያዳክማል፡ ኦፕሬተሮች ይችላሉ።
  ውጫዊ እንዳስገቡ ሳያውቁ ጂኦአፕን ያንቁ
  የሶስተኛ ወገን ዲበ ውሂብ ይፋ ማድረግ እና ያልተረጋገጠ ምላሽ አያያዝ።

ምክር፡-

- አብሮ የተሰራውን የጂኦ መጨረሻ ነጥብ ነባሪ ያስወግዱ።
- የአቻ ጂኦ ፍለጋዎች ሲነቁ እና ግልጽ የሆነ የኤችቲቲፒኤስ የመጨረሻ ነጥብ ይጠይቁ
  አለበለዚያ ፍለጋዎችን ይዝለሉ.
- የጎደሉ ወይም የኤችቲቲፒኤስ የመጨረሻ ነጥቦች አለመሳካታቸውን የሚያረጋግጡ በትኩረት የተደረጉ ለውጦችን ይቀጥሉ።

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  አሁን የጎደሉትን የመጨረሻ ነጥቦችን በ`MissingEndpoint` ውድቅ ያደርጋል፣ ኤችቲቲፒኤስ ያልሆኑትን ውድቅ ያደርጋል።
  የመጨረሻ ነጥቦችን በ`InsecureEndpoint`፣ እና በምትኩ የአቻ ጂኦአፕ ፍለጋን ይዘላል።
  በፀጥታ ወደ ግልጽ ጽሑፍ አብሮገነብ አገልግሎት መመለስ።
- `crates/iroha_config/src/parameters/user.rs` ከአሁን በኋላ ስውር አይወጋም።
  የመጨረሻ ነጥብ በመተንተን ጊዜ፣ ስለዚህ ያልተቀናበረው የውቅር ሁኔታ ግልጽ ሆኖ ይቆያል
  ወደ Runtime ማረጋገጫ መንገድ.
- የተባዙ የቴሌሜትሪ ሰነዶች እና የቀኖናዊው ናሙና ውቅር በ ውስጥ
  `docs/source/references/peer.template.toml` አሁን ይገልፃል።
  `torii.peer_geo.endpoint` ከኤችቲቲፒኤስ ጋር በግልፅ መዋቀር አለበት
  ባህሪ ነቅቷል።
- የመመለሻ ሽፋን አሁን ያካትታል
  `construct_geo_query_requires_explicit_endpoint`፣
  `construct_geo_query_rejects_non_https_endpoint`፣
  `collect_geo_requires_explicit_endpoint_when_enabled`፣ እና
  `collect_geo_rejects_non_https_endpoint` ውስጥ
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15፡ SoraFS ፒን እና ፍኖተ መንገድ ፖሊሲ የታመነ-ተኪ-አዋቂ ደንበኛ-IP ጥራት የለውም (ዝግ 2026-03-25)

ተጽዕኖ፡

- የተገላቢጦሽ Torii ማሰማራት ከዚህ ቀደም የተለየ SoraFS ሊፈርስ ይችላል
  የማከማቻ-ፒን ሲዲአርን ሲገመግሙ ወደ ተኪ ሶኬት አድራሻ ደዋዮች
  ፍቀድ-ዝርዝሮች፣ ማከማቻ-ፒን በደንበኛ ስሮትል እና መግቢያ ዌይ ደንበኛ
  የጣት አሻራዎች.
- ያ በ `/v1/sorafs/storage/pin` እና በSoraFS ላይ የተዳከመ የመጎሳቆል መቆጣጠሪያዎች
  ጌትዌይ የማውረድ ወለሎችን በጋራ ተቃራኒ-ተኪ ቶፖሎጂዎች በማድረግ
  ብዙ ደንበኞች አንድ ባልዲ ወይም አንድ የተፈቀደ ዝርዝር ማንነት ይጋራሉ።
- ነባሪ ራውተር አሁንም የውስጥ የርቀት ሜታዳታን ያስገባል፣ ስለዚህ ይህ ሀ አልነበረም
  አዲስ ያልተረጋገጠ መግቢያ ማለፊያ፣ ነገር ግን እውነተኛ እምነት-ወሰን ክፍተት ነበር።
  ለፕሮክሲ አውቆ ማሰማራት እና ተቆጣጣሪ-ወሰን ከመጠን በላይ መተማመን
  የውስጥ የርቀት-IP ራስጌ.

ማስረጃ፡-- `crates/iroha_config/src/parameters/user.rs` እና
  `crates/iroha_config/src/parameters/actual.rs` ከዚህ ቀደም አጠቃላይ አልነበረም
  `torii.transport.trusted_proxy_cidrs` ማዞሪያ፣ ስለዚህ ተኪ-አዋቂ ቀኖናዊ ደንበኛ
  የአይፒ ጥራት በአጠቃላይ Torii መግቢያ ወሰን ላይ ሊዋቀር አልቻለም።
- `inject_remote_addr_header(...)` በ `crates/iroha_torii/src/lib.rs`
  ከዚህ ቀደም የውስጥ `x-iroha-remote-addr` ራስጌን ከ
  `ConnectInfo` ብቻ፣ ይህም የታመነ የደንበኛ-IP ዲበ ውሂብን ከዚህ ወርዷል።
  እውነተኛ የተገላቢጦሽ ፕሮክሲዎች።
- `PinSubmissionPolicy::enforce(...)` ውስጥ
  `crates/iroha_torii/src/sorafs/pin.rs` እና
  `gateway_client_fingerprint(...)` በ `crates/iroha_torii/src/sorafs/api.rs`
  የታመነ-ተኪ-አዋቂ ቀኖናዊ-IP ጥራት ደረጃን በ ላይ አላጋራም።
  ተቆጣጣሪ ወሰን.
- በ`crates/iroha_torii/src/sorafs/pin.rs` ውስጥ የማጠራቀሚያ-ፒን ስሮትል እንዲሁ ተከፍቷል።
  ማስመሰያ በተገኘ ቁጥር በ bearer token ብቻ፣ ይህ ማለት ብዙ ማለት ነው።
  አንድ ትክክለኛ የፒን ቶከን የሚጋሩ ተኪ ደንበኞች ወደ ተመሳሳይ መጠን ተገድደዋል
  ባልዲ የደንበኞቻቸው አይፒዎች ከተለዩ በኋላም እንኳ።

ይህ ለምን አስፈላጊ ነው:

- የተገላቢጦሽ ፕሮክሲዎች ለTorii መደበኛ የማሰማራት ንድፍ ናቸው። የሩጫ ጊዜ ከሆነ
  የታመነ ተኪን ከማይታመን ደዋይ፣ አይፒ በቋሚነት መለየት አይችልም።
  የፍቃድ ዝርዝሮች እና የደንበኛ ስሮትሎች ኦፕሬተሮች ምን እንደሚያስቡ ይቆማሉ
  ማለት ነው።
- SoraFS ፒን እና መግቢያ ዱካዎች በግልጽ አላግባብ መጠቀምን የሚነኩ ንጣፎች ናቸው፣ ስለዚህ
  ደዋዮችን ወደ ተኪ አይፒ ውስጥ መውደቅ ወይም ከመጠን በላይ መታመን የተላለፈ
  የመሠረት መንገዱ አሁንም ቢሆን ሜታዳታ በአሠራር ላይ ጠቃሚ ነው።
  ሌላ መግቢያ ያስፈልገዋል.

ምክር፡-- አጠቃላይ Torii `trusted_proxy_cidrs` ውቅር ገጽን ይጨምሩ እና ችግሩን ይፍቱ
  ቀኖናዊ ደንበኛ አይፒ አንድ ጊዜ ከ`ConnectInfo` እና ማንኛውም አስቀድሞ የተላለፈ
  ራስጌ የሶኬት አቻ በዚያ የተፈቀደ ዝርዝር ውስጥ ሲሆን ብቻ።
- ያንን ቀኖናዊ-IP ጥራት ከ SoraFS ተቆጣጣሪ ዱካዎች ውስጥ እንደገና ይጠቀሙ
  በጭፍን ውስጣዊ ራስጌን ማመን.
- የተጋራ ማስመሰያ ማከማቻ-ሚስማር ስሮትሎችን በቶከን እና ቀኖናዊ ደንበኛ አይፒን ወሰን
  ሁለቱም ሲገኙ.

የማስተካከያ ሁኔታ፡

- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_config/src/parameters/defaults.rs`፣
  `crates/iroha_config/src/parameters/user.rs`፣ እና
  `crates/iroha_config/src/parameters/actual.rs` አሁን ተጋልጧል
  `torii.transport.trusted_proxy_cidrs`፣ ወደ ባዶ ዝርዝር ነባሪ በማድረግ።
- `crates/iroha_torii/src/lib.rs` አሁን ቀኖናዊውን ደንበኛ አይፒን ይፈታል።
  `limits::ingress_remote_ip(...)` በመግቢያው መካከለኛ ዌር ውስጥ እና እንደገና ይጽፋል
  የውስጥ `x-iroha-remote-addr` ራስጌ ከታመኑ ፕሮክሲዎች ብቻ።
- `crates/iroha_torii/src/sorafs/pin.rs` እና
  `crates/iroha_torii/src/sorafs/api.rs` አሁን ቀኖናዊ ደንበኛ አይፒዎችን ፈትቷል።
  በ `state.trusted_proxy_nets` በተቆጣጣሪው ወሰን ለማከማቻ-ሚስማር
  የፖሊሲ እና የጌትዌይ ደንበኛ የጣት አሻራ ስራ፣ ስለዚህ ቀጥተኛ ተቆጣጣሪ ዱካዎች አይችሉም
  ከመጠን በላይ መተማመን የቆየ የአይፒ ዲበ ውሂብ።
- የማከማቻ-ፒን ስሮትል አሁን ቁልፎች የተጋሩ ተሸካሚ ቶከኖች በ `ቶከን + ቀኖናዊ
  ደንበኛ IP` ሁለቱም ሲገኙ፣ የደንበኛ ባልዲዎችን ለጋራ ማቆየት።
  የፒን ምልክቶች.
- የመመለሻ ሽፋን አሁን ያካትታል
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`፣
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`፣
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`፣
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`፣
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy` እና የ
  የማዋቀር እቃ
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16፡ የተወጋው የርቀት አይፒ ራስጌ በሚጠፋበት ጊዜ የኦፕሬተር ማረጋገጫ መቆለፊያ እና የዋጋ ገደብ ቁልፍ ወደ አንድ የጋራ ስም-አልባ ባልዲ ተመልሶ ወደቀ (ዝግ 2026-03-25)

ተጽዕኖ፡

- የኦፕሬተር ማረጋገጫ መቀበል ቀድሞውኑ ተቀባይነት ያለው ሶኬት አይፒን ተቀብሏል ፣ ግን እ.ኤ.አ
  የመቆለፊያ/ተመን-ገደብ ቁልፍ ችላ ብሎት እና ጥያቄዎችን ወደ አንድ የጋራ ሰብስብ
  የውስጥ `x-iroha-remote-addr` ራስጌ በነበረ ቁጥር `"anon"` ባልዲ
  የለም ።
- ያ በነባሪ ራውተር ላይ አዲስ የህዝብ መግቢያ መንገድ አልነበረም፣ ምክንያቱም
  ingress middleware እነዚህ ተቆጣጣሪዎች ከመሮጣቸው በፊት የውስጥ አርዕስቱን እንደገና ይጽፋል።
  አሁንም ለጠባቡ የውስጥ እውነተኛ ውድቀት-ክፍት እምነት-ወሰን ክፍተት ነበር።
  ተቆጣጣሪ ዱካዎች፣ ቀጥተኛ ሙከራዎች እና ማንኛውም ወደፊት የሚደርስ መንገድ
  `OperatorAuth` ከመርፌ መሃከል በፊት።
- በእነዚያ አጋጣሚዎች አንድ ደዋይ የታሪፍ ገደብ በጀት ሊወስድ ወይም ሌላ መቆለፍ ይችላል።
  በምንጭ አይፒ ተነጥለው መሆን የነበረባቸው ደዋዮች።

ማስረጃ፡-- `OperatorAuth::check_common(...)` ውስጥ
  `crates/iroha_torii/src/operator_auth.rs` አስቀድሞ ተቀብሏል።
  `remote_ip: Option<IpAddr>`፣ ግን ቀደም ሲል `auth_key(headers)` ተብሎ ይጠራል
  እና የማጓጓዣ አይፒን ሙሉ በሙሉ ጣለው.
- `auth_key(...)` በ `crates/iroha_torii/src/operator_auth.rs` ከዚህ ቀደም
  የተተነተነ `limits::REMOTE_ADDR_HEADER` ብቻ እና ያለበለዚያ `"anon"` ተመልሷል።
- በ `crates/iroha_torii/src/limits.rs` ውስጥ ያለው አጠቃላይ Torii ረዳት አስቀድሞ ነበረው
  በ `effective_remote_ip (ራስጌዎች፣
  remote)`፣ የተወጋውን ቀኖናዊ ራስጌ እየመረጥኩ ግን ወደ
  ተቀባይነት ያለው ሶኬት አይፒ ቀጥተኛ ተቆጣጣሪ ጥሪዎች መካከለኛ ዌርን ሲያልፉ።

ይህ ለምን አስፈላጊ ነው:

- የመቆለፊያ እና የዋጋ ገደብ ሁኔታ በተመሳሳዩ ውጤታማ የደዋይ ማንነት ላይ ቁልፍ መሆን አለበት።
  የተቀረው Torii ለፖሊሲ ውሳኔዎች የሚጠቀም። ወደ አንድ የጋራ መመለስ
  የማይታወቅ ባልዲ የጎደለውን የውስጥ ሜታዳታ መዝጊያ ወደ ደንበኛ ይለውጠዋል
  ውጤቱን ወደ እውነተኛው ደዋይ ከማውጣት ይልቅ ጣልቃ መግባት።
- ኦፕሬተር auth አላግባብ መጠቀምን የሚነካ ድንበር ነው፣ ስለዚህ መካከለኛ-ክብደት እንኳን
  ባልዲ-ግጭት ጉዳይ በግልፅ መዝጋት ተገቢ ነው።

ምክር፡-

- የኦፕሬተር-አውዝ ቁልፉን ከ `ገደቦች :: ውጤታማ_remote_ip ያውጡ (ራስጌዎች ፣
  remote_ip)` ስለዚህ የተወጋው ራስጌ አሁንም ሲገኝ ያሸንፋል፣ ግን በቀጥታ
  የተቆጣጣሪ ጥሪዎች ከ`"anon"` ይልቅ ወደ ትራንስፖርት አድራሻ ይመለሳሉ።
- `"anon"` እንደ የመጨረሻ ውድቀት ብቻ ያቆዩት ሁለቱም የውስጥ ራስጌ እና
  የማጓጓዣ አይፒው አይገኝም።

የማስተካከያ ሁኔታ፡- አሁን ባለው ኮድ ተዘግቷል. `crates/iroha_torii/src/operator_auth.rs` አሁን ይደውላል
  `auth_key(headers, remote_ip)` ከ `check_common(...)`፣ እና `auth_key(...)`
  አሁን የመቆለፍ/ተመን-ገደብ ቁልፉን ያገኘዋል።
  `limits::effective_remote_ip(headers, remote_ip)`.
- የመመለሻ ሽፋን አሁን ያካትታል
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` እና
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` ውስጥ
  `crates/iroha_torii/src/operator_auth.rs`.

## ከቀዳሚው ዘገባ የተዘጉ ወይም የተተኩ ግኝቶች

- ቀደም ሲል ጥሬ-የግል-ቁልፍ Soracloud ፍለጋ፡ ተዘግቷል። የአሁኑ ሚውቴሽን ወደ ውስጥ መግባት
  የመስመር ውስጥ `authority` / `private_key` መስኮችን ውድቅ ያደርጋል
  `crates/iroha_torii/src/soracloud.rs:5305-5308`፣ የኤችቲቲፒ ፈራሚውን ከ
  ሚውቴሽን ፕሮቬንሽን በ `crates/iroha_torii/src/soracloud.rs:5310-5315`፣ እና
  የተፈረመ ሰነድ ከማስገባት ይልቅ ረቂቅ የግብይት መመሪያዎችን ይመልሳል
  ግብይት በ `crates/iroha_torii/src/soracloud.rs:5556-5565`.
- ቀደም ብሎ የውስጥ-ብቻ የአካባቢ-የተነበበ ተኪ ማስፈጸሚያ፡ ተዘግቷል። የህዝብ
  የመንገድ መፍታት አሁን ይፋዊ ያልሆኑ እና የዝማኔ/የግል-ዝማኔ ተቆጣጣሪዎችን ወደ ውስጥ ይዘላል
  `crates/iroha_torii/src/soracloud.rs:8445-8463`፣ እና የሩጫ ሰዓቱ ውድቅ ያደርጋል
  ይፋዊ ያልሆኑ የአካባቢ-የተነበቡ መንገዶች
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- ቀደም ብሎ የህዝብ-አሂድ ጊዜ የማይለካ የውድቀት ግኝት፡ እንደተፃፈ ተዘግቷል። የህዝብ
  የሩጫ ጊዜ መግቢያ አሁን የዋጋ ገደቦችን እና የበረራ ጣራዎችን ያስገድዳል
  `crates/iroha_torii/src/lib.rs:8837-8852` የህዝብ መንገድን ከመፍታቱ በፊት
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- ቀደም ሲል የርቀት-አይፒ አባሪ-የተከራይና አከራይ ፍለጋ፡ ተዘግቷል። አባሪ ተከራይ አሁን
  የተረጋገጠ መለያ ያስፈልገዋል
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  አባሪ ተከራይ ቀደም ሲል SEC-05 እና SEC-06 ወርሷል; ያ ርስት
  ከላይ ባሉት የመተግበሪያ-አውት ማሻሻያዎች ተዘግቷል።## የጥገኝነት ግኝቶች- `cargo deny check advisories bans sources --hide-inclusion-graph` አሁን ይሰራል
  በቀጥታ ከተከታተለው `deny.toml` እና አሁን ሶስት ቀጥታ ስርጭትን ሪፖርት አድርጓል
  ከተፈጠረ የስራ ቦታ መቆለፊያ ፋይል የጥገኝነት ግኝቶች።
- የ`tar` ምክሮች ከአሁን በኋላ በንቃት ጥገኝነት ግራፍ ውስጥ አይገኙም።
  `xtask/src/mochi.rs` አሁን `Command::new("tar")` ከቋሚ ክርክር ጋር ይጠቀማል
  ቬክተር፣ እና `iroha_crypto` ለ `libsodium-sys-stable` አይጎትትም
  እነዚያን ቼኮች ወደ OpenSSL ከቀየሩ በኋላ Ed25519 interop ሙከራዎች።
- ወቅታዊ ግኝቶች:
  - `RUSTSEC-2024-0388`: `derivative` ያልተጠበቀ ነው.
  - `RUSTSEC-2024-0436`: `paste` ያልተጠበቀ ነው.
- ተጽዕኖ ልዩነት;
  - ቀደም ሲል የተዘገበው `tar` ምክሮች ለንቁ ዝግ ናቸው።
    ጥገኝነት ግራፍ. `cargo tree -p xtask -e normal -i tar`፣
    `cargo tree -p iroha_crypto -e all -i tar`፣ እና
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` አሁን ሁሉም አልተሳካም።
    በ"የጥቅል መታወቂያ ዝርዝር መግለጫ ... ከጥቅል ጋር አልተዛመደም" እና
    `cargo deny` ከአሁን በኋላ `RUSTSEC-2026-0067` ወይም
    `RUSTSEC-2026-0068`.
  - ቀደም ሲል ሪፖርት የተደረገው ቀጥተኛ PQ ምትክ ምክሮች አሁን ተዘግተዋል።
    አሁን ያለው ዛፍ. `crates/soranet_pq/Cargo.toml`፣
    `crates/iroha_crypto/Cargo.toml`፣ እና `crates/ivm/Cargo.toml` አሁን ጥገኛ ናቸው
    በ `pqcrypto-mldsa`/`pqcrypto-mlkem`፣ እና በተነካው ML-DSA/ML-KEM ላይ
    የሩጫ ጊዜ ሙከራዎች አሁንም ከስደት በኋላ ያልፋሉ።
  - ቀደም ሲል የተዘገበው `rustls-webpki` ምክር ከአሁን በኋላ ንቁ አይደለም።
    የአሁኑ ውሳኔ. የመስሪያ ቦታው አሁን `reqwest`/`rustls` ላይ ይሰካል`rustls-webpki` በ `0.103.10` ላይ የሚያቆየው የተለጠፈ patch ልቀቶች
    ከአማካሪ ክልል ውጪ።
  - `derivative` እና `paste` በቀጥታ በስራ ቦታ ምንጭ ውስጥ ጥቅም ላይ አይውሉም።
    በBLS / arkworks ቁልል ስር በመሸጋገሪያ በኩል ይገባሉ።
    `w3f-bls` እና ሌሎች በርካታ የተፋሰሱ ሳጥኖች፣ ስለዚህ እነሱን ማስወገድ ይጠይቃል
    ከአካባቢያዊ ማክሮ ጽዳት ይልቅ ወደላይ ወይም የጥገኝነት ቁልል ለውጦች።
    አሁን ያለው ዛፍ አሁን እነዚያን ሁለቱን ምክሮች በግልፅ ይቀበላል
    `deny.toml` ከተመዘገቡ ምክንያቶች ጋር።

## የሽፋን ማስታወሻዎች- አገልጋይ/የአሂድ ጊዜ/ውቅር/አውታረ መረብ፡ SEC-05፣ SEC-06፣ SEC-07፣ SEC-08፣ SEC-09፣
  SEC-10፣ SEC-12፣ SEC-13፣ SEC-14፣ SEC-15 እና SEC-16 የተረጋገጡት በዚህ ወቅት ነው።
  ኦዲቱ እና አሁን ባለው ዛፍ ውስጥ ተዘግተዋል. ተጨማሪ ማጠንከሪያ በ
  አሁን ያለው ዛፍ የግንኙነት ዌብሶኬት/የክፍለ ጊዜ መግቢያ እንዳይሳካ ያደርገዋል
  በምትኩ የውስጥ መርፌ የርቀት-IP ራስጌ ሲጠፋ ይዘጋል
  ያንን ሁኔታ ወደ loopback በመመለስ ላይ።
- IVM/crypto/ ተከታታይነት፡ ከዚህ ኦዲት ምንም ተጨማሪ የተረጋገጠ ግኝት የለም
  ቁራጭ። አወንታዊ ማስረጃ ሚስጥራዊ ቁልፍ የቁሳቁስ ዜሮ መሆንን ያጠቃልላል
  `crates/iroha_crypto/src/confidential.rs:53-60` እና ዳግም አጫውት-የሚያውቅ Soranet PoW
  በ`crates/iroha_crypto/src/soranet/pow.rs:823-879` ውስጥ የተፈረመ ትኬት ማረጋገጫ።
  የክትትል እልከኝነት አሁን ደግሞ የተበላሸ የፍጥነት ውፅዓት በሁለት አይቀበልም።
  Norito ዱካዎች ናሙና: `crates/norito/src/lib.rs` የተጣደፈ JSON ያረጋግጣል
  የደረጃ-1 ካሴቶች ከ`TapeWalker` የማረጋገጫ ማካካሻዎች በፊት እና አሁን ደግሞ ያስፈልገዋል
  በተለዋዋጭ የተጫኑ ሜታል/CUDA ደረጃ-1 ረዳቶች ከ ጋር ተመሳሳይነት ለማረጋገጥ
  ከማግበር በፊት ስካላር መዋቅራዊ-ኢንዴክስ ገንቢ, እና
  `crates/norito/src/core/gpu_zstd.rs` በጂፒዩ የተዘገበ የውጤት ርዝመት ያረጋግጣል
  የመቀየሪያ/የማስቀቢያ ቋቶችን ከመቁረጥ በፊት። `crates/norito/src/core/simd_crc64.rs`
  አሁን ደግሞ የራስ ሙከራዎች በተለዋዋጭ የጂፒዩ CRC64 ረዳቶችን ከ
  ከ`hardware_crc64` በፊት ቀኖናዊ ውድቀት ያምኗቸዋል፣ስለዚህ የተበላሸ
  Norito ቼክሰምን በዝምታ ከመቀየር ይልቅ አጋዥ ቤተ-መጻሕፍት አልተዘጉም።ባህሪ. ልክ ያልሆኑ የረዳት ውጤቶች አሁን በአስደንጋጭ መለቀቅ ፈንታ ወደ ኋላ ይመለሳሉ
  የቼክሰም እኩልነትን ይገነባል ወይም ይንሸራተታል። በIVM በኩል፣ ናሙና የተደረገ ማፍያ
  የማስጀመሪያ በሮች አሁን ደግሞ CUDA Ed25519 `signature_kernel`፣ CUDA BN254ን ይሸፍናሉ።
  add/sub/mul kernels፣ CUDA `sha256_leaves`/`sha256_pairs_reduce`፣ ቀጥታ ስርጭት
  CUDA vector/AES ባች ከርነሎች (`vadd64`፣ `vand`፣ `vxor`፣ `vor`፣
  `aesenc_batch`፣ `aesdec_batch`) እና ተዛማጅ ሜታል
  እነዚያ መንገዶች ከመታመናቸው በፊት `sha256_leaves`/vector/AES batch kernels። የ
  ናሙና የተደረገው ሜታል Ed25519 የፊርማ መንገድ አሁን ደግሞ ተመልሷል
  በዚህ አስተናጋጅ ላይ በተዘጋጀው የቀጥታ አፋጣኝ ውስጥ፡ የቀደመው እኩልነት ውድቀት ነበር።
  የተስተካከለው በ ref10 እጅና እግር ላይ የተጣበቀ መደበኛነትን በመለኪያ መሰላል ላይ ወደነበረበት በመመለስ፣
  እና ያተኮረው የብረት መመለሻ አሁን `[s]B`፣ `[h](-A)`፣
  የሁለት መሰረታዊ ነጥብ መሰላል እና ሙሉ የ`[true, false]` ባች ማረጋገጫ
  በሲፒዩ ማመሳከሪያ መንገድ ላይ በብረት ላይ. የተንጸባረቀው የCUDA ምንጭ ይለወጣል
  በ`--features cuda --tests` ማጠናቀር፣ እና የCUDA ጅምር እውነት አሁን ተቀናብሯል።
  የቀጥታ የመርክል ቅጠል/ጥንድ ከርነል ከሲፒዩ ከተንሳፈፈ አይዘጋም።
  የማጣቀሻ መንገድ. የአሂድ ጊዜ CUDA ማረጋገጫ በዚህ በአስተናጋጅ-የተገደበ ይቆያል
  አካባቢ.
- ኤስዲኬዎች/ምሳሌዎች፡- SEC-11 የተረጋገጠው በናሙና በተዘጋጀው መጓጓዣ ላይ ባተኮረ ጊዜ ነው።
  በስዊፍት፣ ጃቫ/አንድሮይድ፣ ኮትሊን እና ጄኤስ ደንበኞች ማለፍ እና ያማግኘት አሁን ባለው ዛፍ ውስጥ ተዘግቷል. ጄኤስ፣ ስዊፍት እና አንድሮይድ
  ቀኖናዊ-ጥያቄ ረዳቶች ወደ አዲሱ ተዘምነዋል
  ትኩስነትን የሚያውቅ ባለአራት-ራስጌ እቅድ።
  የናሙናው የQUIC ዥረት ትራንስፖርት ግምገማ እንዲሁ በቀጥታ ስርጭት አላስገኘም።
  አሁን ባለው ዛፍ ውስጥ የአሂድ ጊዜ ፍለጋ፡ `StreamingClient::connect(...)`፣
  `StreamingServer::bind(...)`፣ እና የችሎታ-ድርድር ረዳቶች ናቸው።
  በአሁኑ ጊዜ በ `crates/iroha_p2p` ውስጥ ካለው የሙከራ ኮድ ብቻ ነው የሚሰራው።
  `crates/iroha_core`፣ ስለዚህ ፈቃዱ በራሱ የተፈረመ አረጋጋጭ በዚያ ረዳት ውስጥ
  መንገዱ በአሁኑ ጊዜ ከተጓጓዥ ወለል ይልቅ የሙከራ/ረዳት-ብቻ ነው።
- ምሳሌዎች እና የሞባይል ናሙና መተግበሪያዎች የተገመገሙት በስፖት-ቼክ ደረጃ ብቻ እና ነው።
  እንደ ሙሉ ኦዲት ተደርጎ መታየት የለበትም።

## የማረጋገጫ እና የሽፋን ክፍተቶች- `cargo deny check advisories bans sources --hide-inclusion-graph` አሁን ይሰራል
  በቀጥታ ከተከታተለው `deny.toml` ጋር። አሁን ባለው የመርሃግብር ሂደት፣
  `bans` እና `sources` ንፁህ ሲሆኑ `advisories` ከአምስቱ ጋር አልተሳካም
  ከላይ የተዘረዘሩት የጥገኝነት ግኝቶች.
- የተዘጉ የ`tar` ግኝቶች ጥገኝነት-ግራፍ ማጽጃ ማረጋገጫ አልፏል፡-
  `cargo tree -p xtask -e normal -i tar`፣
  `cargo tree -p iroha_crypto -e all -i tar`፣ እና
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` አሁን ሁሉም ሪፖርት ያድርጉ
  “የጥቅል መታወቂያ ዝርዝር መግለጫ… ከጥቅል ጋር አልተዛመደም” እያለ
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`፣
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`፣
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`፣
  እና
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  ሁሉም አልፏል.
- `bash scripts/fuzz_smoke.sh` አሁን እውነተኛ የlibFuzzer ክፍለ ጊዜዎችን በ በኩል ይሰራል
  `cargo +nightly fuzz`፣ ግን የIVM የስክሪፕቱ ግማሽ አላለቀም።
  ይህ ማለፊያ ለ`tlv_validate` የመጀመሪያው የምሽት ግንባታ ገና ስለነበረ ነው።
  በእጃቸው ላይ እድገት ። ያ ግንባታ ከዚያን ጊዜ ጀምሮ ለማጠናቀቅ በበቂ ሁኔታ ተጠናቅቋል
  የመነጨ libFuzzer ሁለትዮሽ በቀጥታ፡
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  አሁን የlibFuzzer run loop ደርሷል እና ከ 200 ሩጫዎች በኋላ በንጽህና ይወጣል
  ባዶ ኮርፐስ. የ Norito ግማሹን ካስተካከለ በኋላ በተሳካ ሁኔታ ተጠናቀቀ
  መታጠቂያ/ተንሸራታች ገላጭ እና `json_from_json_equiv` fuzz-ዒላማ ማጠናቀር
  መስበር
- Torii ማሻሻያ ማረጋገጥ አሁን የሚከተሉትን ያጠቃልላል
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - ጠባብው `--no-default-features --features app_api,app_api_https` Torii
    የሙከራ ማትሪክስ አሁንም በDA / ውስጥ የማይዛመዱ ነባር የማጠናቀር ውድቀቶች አሉት
    Soracloud-gated ሊብ-ሙከራ ኮድ፣ ስለዚህ ይህ ማለፊያ የተላከውን አረጋግጧል
    ነባሪ ባህሪ ኤምሲፒ መንገድ እና የ`app_api_https` የድር መንጠቆ ዱካ ሳይሆን
    ሙሉ ዝቅተኛ-ባህሪ ሽፋን በመጠየቅ።
- የታመነ-ፕሮክሲ/SoraFS ማሻሻያ ማረጋገጥ አሁን የሚከተሉትን ያጠቃልላል
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- የP2P ማሻሻያ ማረጋገጫ አሁን የሚከተሉትን ያጠቃልላል
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS የአካባቢ-ተኪ ማሻሻያ ማረጋገጥ አሁን የሚከተሉትን ያካትታል፡-
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS-ነባሪ ማሻሻያ ማረጋገጫ አሁን የሚከተሉትን ያካትታል፡-
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- የኤስዲኬ-ጎን ማሻሻያ ማረጋገጫ አሁን የሚከተሉትን ያካትታል፡-
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito የክትትል ማረጋገጫ አሁን የሚከተሉትን ያጠቃልላል
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito ኢላማዎች `json_parse_string`፣
    `json_parse_string_ref`፣ `json_skip_value`፣ እና `json_from_json_equiv`ከታጠቁ/የዒላማ ጥገናዎች በኋላ አለፈ)
- IVM fuzz የክትትል ማረጋገጫ አሁን የሚከተሉትን ያጠቃልላል
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM የፍጥነት መቆጣጠሪያ ማረጋገጥ አሁን የሚከተሉትን ያጠቃልላል
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- ያተኮረ የCUDA ሊብ-ሙከራ አፈፃፀም በዚህ አስተናጋጅ ላይ በአከባቢው የተገደበ ይቆያል፡
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  የCUDA ሾፌር ምልክቶች (`cu*`) ስለማይገኙ አሁንም ማገናኘት አልቻለም።
- ትኩረት የተደረገ የብረታ ብረት አሂድ ጊዜ ማረጋገጫ አሁን በዚህ ላይ በአፋጣኝ ላይ ሙሉ በሙሉ ይሰራል
  አስተናጋጅ፡ ናሙና የተደረገው Ed25519 የፊርማ ቧንቧ መስመር በጅምር ነቅቷል።
  ራስን መፈተሽ፣ እና `metal_ed25519_batch_matches_cpu` `[true, false]` ያረጋግጣል።
  በቀጥታ ከሲፒዩ ማመሳከሪያ መንገድ ጋር በብረት ላይ።
- ሙሉ የስራ ቦታ የዝገት ሙከራን፣ ሙሉ `npm test` ወይም
  በዚህ የማገገሚያ ማለፊያ ጊዜ ሙሉ ስዊፍት/አንድሮይድ ስብስቦች።

## ቅድሚያ የሚሰጠው የማስተካከያ የኋላ መዝገብ

### ቀጣይ ትራክ- በግልጽ ተቀባይነት ላለው የሽግግር መለዋወጫ ወደ ላይ ይከታተሉ
  `derivative`/`paste` ማክሮ ዕዳ እና የ `deny.toml` ልዩ ሁኔታዎችን ያስወግዱ
  BLS / Halo2 / PQ / መረጋጋት ሳያስቀሩ አስተማማኝ ማሻሻያዎች ይገኛሉ
  የዩአይ ጥገኝነት ቁልል።
- ሙሉ የሌሊት IVM fuzz-ጭስ ስክሪፕት በሞቃት መሸጎጫ ላይ እንደገና ያሂዱ።
  `tlv_validate` / `kotodama_lower` ከሚከተሉት ቀጥሎ የተረጋጋ የተመዘገቡ ውጤቶች አሏቸው
  አሁን-አረንጓዴ Norito ኢላማዎች። የቀጥታ `tlv_validate` ሁለትዮሽ ሩጫ አሁን ተጠናቅቋል፣
  ግን ሙሉ ስክሪፕት የተደረገው የምሽት ጭስ አሁንም አስደናቂ ነው።
- ያተኮረውን CUDA ሊብ-ሙከራ የራስ-ሙከራ ቁራጭ ከCUDA ሾፌር ጋር በአስተናጋጅ ላይ እንደገና ያሂዱ
  ቤተ መፃህፍት ተጭነዋል፣ ስለዚህ የተዘረጋው የCUDA ጅምር እውነት ስብስብ ተረጋግጧል
  ከ `cargo check` ባሻገር እና የተንጸባረቀው Ed25519 መደበኛ ማስተካከያ እና
  አዲስ የቬክተር/AES ማስጀመሪያ መመርመሪያዎች በሂደት ጊዜ ይሠራሉ።
- ሰፋ ያለ JS/Swift/Android/Kotlin ስብስቦችን አንዴ ከማይዛመደው የስብስብ ደረጃ እንደገና አስጀምር
  በዚህ ቅርንጫፍ ላይ ያሉ ማገጃዎች ጸድተዋል, ስለዚህ አዲሱ ቀኖናዊ-ጥያቄ እና
  የመጓጓዣ-የደህንነት ጥበቃዎች ከላይ ከተጠቀሱት የረዳት ሙከራዎች በላይ የተሸፈኑ ናቸው.
- የረዥም ጊዜ የመተግበሪያ-አውት መልቲሲግ ታሪክ ይቆይ እንደሆነ ይወስኑ
  አልተሳካም-ዝግ ወይም አንደኛ ደረጃ HTTP multisig ምስክር ቅርጸት ያሳድጉ.

### ተቆጣጠር- የ `ivm` ሃርድዌር-ማጣደፍ/ደህንነታቸው ያልተጠበቁ መንገዶች ላይ ያተኮረ ግምገማ ይቀጥሉ
  እና ቀሪው `norito` ዥረት/crypto ድንበሮች። የJSON ደረጃ-1
  እና የጂፒዩ zstd አጋዥ የእጅ መውጫዎች አሁን ተዘግተው እንዳይቀሩ እልከኛ ሆነዋል
  ልቀት ይገነባል፣ እና ናሙናው IVM የፍጥነት ጅምር እውነት ስብስቦች አሁን ናቸው።
  ሰፋ ያለ፣ ነገር ግን ሰፋ ያለ ደህንነቱ ያልተጠበቀ/ቆራጥነት ግምገማ አሁንም ክፍት ነው።