---
lang: am
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha ሰነድ

日本語版の概要は [`README.ja.md`](./README.ja.md)

የስራ ቦታው ከተመሳሳይ ኮድ ቤዝ ሁለት የመልቀቂያ መስመሮችን ይላካል፡- **Iroha 2** (በራስ የሚስተናገዱ ማሰማራት) እና
** Iroha 3 / SORA Nexus** (ነጠላው ዓለም አቀፍ Nexus መጽሐፍ)። ሁለቱም ተመሳሳይ I18NT0000015X ምናባዊ ማሽን (IVM) እና
Kotodama የመሳሪያ ሰንሰለት፣ ስለዚህ ኮንትራቶች እና ባይትኮድ በማሰማራት ዒላማዎች መካከል ተንቀሳቃሽ ሆነው ይቆያሉ። ሰነድ ተፈጻሚ ይሆናል።
ለሁለቱም ካልሆነ በስተቀር.

በ[ዋና I18NT0000016X ሰነድ](https://docs.iroha.tech/) ታገኛላችሁ፡-

- [መመሪያ ጀምር](https://docs.iroha.tech/get-started/)
- [ኤስዲኬ አጋዥ ስልጠናዎች](https://docs.iroha.tech/guide/tutorials/) ለ Rust፣ Python፣ Javascript፣ እና Java/Kotlin
- [ኤፒአይ ማጣቀሻ](https://docs.iroha.tech/reference/torii-endpoints.html)

የሚለቀቁት ልዩ ነጭ ወረቀቶች እና ዝርዝሮች፡-

- [Iroha 2 ነጭ ወረቀት](./source/iroha_2_whitepaper.md) - በራሱ የሚሰራ የአውታረ መረብ ዝርዝር መግለጫ።
- [Iroha 3 (SORA Nexus) ነጭ ወረቀት](./source/iroha_3_whitepaper.md) - Nexus ባለብዙ መስመር እና የውሂብ-ቦታ ንድፍ።
- [የውሂብ ሞዴል እና አይኤስአይ ዝርዝር (ተግባራዊ-የተገኘ)](./source/data_model_and_isi_spec.md) - የተገላቢጦሽ የምህንድስና ባህሪ ማጣቀሻ።
- [ZK Envelopes (Norito)](./source/zk_envelopes.md) - ቤተኛ IPA/STARK Norito ፖስታዎች እና አረጋጋጭ የሚጠበቁ።

## አካባቢያዊነት

ጃፓንኛ (`*.ja.*`)፣ ዕብራይስጥ (I18NI0000049X)፣ ስፓኒሽ (`*.es.*`)፣ ፖርቱጋልኛ
(`*.pt.*`)፣ ፈረንሳይኛ (`*.fr.*`)፣ ሩሲያኛ (`*.ru.*`)፣ አረብኛ (`*.ar.*`) እና ኡርዱ
(`*.ur.*`) የሰነድ ወረቀቶች ከእያንዳንዱ የእንግሊዝኛ ምንጭ ፋይል አጠገብ ይኖራሉ። ተመልከት
[`docs/i18n/README.md`](./i18n/README.md) ስለ ማመንጨት እና ዝርዝሮች
ትርጉሞችን ማቆየት እና በ ውስጥ አዳዲስ ቋንቋዎችን ለመጨመር መመሪያ
ወደፊት.

## መሳሪያዎች

በዚህ ማከማቻ ውስጥ ለIroha 2 መሳሪያዎች ሰነዶችን ማግኘት ይችላሉ፡-

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) ማክሮዎች ለማዋቀር መዋቅር (የ`config_base` ባህሪን ይመልከቱ)
- [የግንባታ ደረጃዎች](./profile_build.md) ቀርፋፋ `iroha_data_model` የማጠናቀር ተግባራትን ለመለየት

## ስዊፍት / iOS SDK ማጣቀሻዎች

- [የስዊፍት ኤስዲኬ አጠቃላይ እይታ](./source/sdk/swift/index.md) - የቧንቧ መስመር ረዳቶች፣ የፍጥነት መቀየሪያዎች እና Connect/WebSocket APIs።
- [ፈጣን ማስጀመሪያን ያገናኙ](./connect_swift_ios.md) - ኤስዲኬ-የመጀመሪያው የእግር ጉዞ እና የCryptoKit ማጣቀሻ።
- [Xcode ውህደት መመሪያ](./connect_swift_integration.md) - የ NoritoBridgeKit ሽቦን ማገናኘት/ወደ አፕሊኬሽኑ ከ ChaChaPoly እና ከፍሬም ረዳቶች ጋር ያገናኙ።
- [SwiftUI ማሳያ አስተዋጽዖ አበርካች መመሪያ](./norito_demo_contributor.md) - የiOS ማሳያን ከአካባቢያዊ I18NT0000024X መስቀለኛ መንገድ እና የማጣደፍ ማስታወሻዎች ጋር ማሄድ።
- Swift artifacts ወይም Connect ለውጦችን ከማተምዎ በፊት `make swift-ci` ን ያሂዱ; የቋሚ እኩልነት፣ ዳሽቦርድ ምግቦች እና Buildkite I18NI0000061X ሜታዳታ ያረጋግጣል።

## Norito (ተከታታይ ኮድ)

Norito የስራ ቦታ ተከታታይ ኮዴክ ነው። `parity-scale-codec` አንጠቀምም።
(ስኬል) ሰነዶች ወይም መመዘኛዎች ከ SCALE ጋር ሲወዳደሩ፣ ለ ብቻ ነው።
አውድ; ሁሉም የምርት መንገዶች I18NT0000005X ይጠቀማሉ። `norito::codec::{Encode, Decode}`
ኤፒአይዎች የራስጌ የሌለው ("ባሬ") Norito ክፍያ ለሃሺንግ እና ለሽቦ ይሰጣሉ
ቅልጥፍና - እሱ I18NT0000007X እንጂ SCALE አይደለም።

የቅርብ ጊዜ ሁኔታ፡

- በቋሚ ራስጌ (አስማት፣ ስሪት፣ 16-ባይት ሼማ፣ መጭመቂያ፣ ርዝመት፣ CRC64፣ ባንዲራዎች) ቆራጥ ኢንኮዲንግ/መግለጽ።
- CRC64-XZ ቼክ ድምር በአሂድ ጊዜ-የተመረጠ ማጣደፍ፡-
  - x86_64 PCLMULQDQ (ያለ ማባዛት) + ባሬት መቀነስ፣ በ32-ባይት ጥራጊዎች ላይ ተጣጥፎ።
  - aarch64 PMULL ከተዛማጅ ማጠፍ ጋር።
  - ለተንቀሳቃሽነት መቆራረጥ - በ 8 እና በመጠኑ መውደቅ።
- ምደባዎችን ለመቀነስ በዲሪቭስ እና በዋና ዓይነቶች የተተገበሩ ኢንኮድ ርዝመት ፍንጮች።
- ትላልቅ የዥረት ቋቶች (64 ኪቢ) እና ተጨማሪ የCRC ዝማኔ ኮድ በሚፈታበት ጊዜ።
- አማራጭ zstd መጭመቂያ; የጂፒዩ ማጣደፍ በባህሪይ የተገጠመ እና የሚወስን ነው።
- የሚለምደዉ መንገድ ምርጫ፡- `norito::to_bytes_auto(&T)` ከቁ መካከል ይመርጣል
  መጭመቂያ፣ ሲፒዩ zstd፣ ወይም ጂፒዩ-የተጫነ zstd (ሲጠናቀር እና ሲገኝ)
  የመጫኛ መጠን እና የተሸጎጡ የሃርድዌር ችሎታዎች ላይ በመመስረት። ምርጫ ብቻ ነው የሚነካው።
  አፈጻጸም እና የራስጌው I18NI0000065X ባይት; የክፍያ ትርጉሞች አልተቀየሩም።

ለተመጣጣኝ ሙከራዎች፣ መመዘኛዎች እና የአጠቃቀም ምሳሌዎች `crates/norito/README.md` ይመልከቱ።

ማስታወሻ፡ አንዳንድ ንዑስ ስርዓት ሰነዶች (ለምሳሌ፡ IVM acceleration and ZK circuits) በሂደት ላይ ናቸው። ተግባራዊነት ያልተሟላ ከሆነ, ፋይሎቹ የቀረውን ስራ እና የጉዞ አቅጣጫን ይጠራሉ.

የኹናቴ የመጨረሻ ነጥብ ኢንኮዲንግ ማስታወሻዎች
- Torii I18NI0000067X አካል Norito በነባሪነት የራስጌ የሌለው ("ባዶ") ጭነትን ይጠቀማል። ደንበኞች መጀመሪያ I18NT0000009X መፍታት መሞከር አለባቸው።
- አገልጋዮች ሲጠየቁ JSON መመለስ ይችላሉ; I18NI0000068X `application/json` ከሆነ ደንበኞች ወደ JSON ይመለሳሉ።
- የሽቦ ቅርፀቱ Norito እንጂ SCALE አይደለም። የ `norito::codec::{Encode,Decode}` ኤፒአይዎች ለባዶ ልዩነት ጥቅም ላይ ይውላሉ።