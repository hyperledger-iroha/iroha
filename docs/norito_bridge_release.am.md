---
lang: am
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge የሚለቀቅ ማሸጊያ

ይህ መመሪያ የ`NoritoBridge` Swift ማሰሪያዎችን ለማተም የሚያስፈልጉትን ደረጃዎች ይዘረዝራል።
ከSwift Package Manager እና CocoaPods ሊበላ የሚችል XCFramework። የ
የስራ ፍሰት የስዊፍት ቅርሶችን በቁልፍ እርምጃ ያቆያል የ Rust crate ያንን መርከብ ይለቀቃል
Iroha's I18NT0000001X ኮዴክ። የታተመውን አጠቃቀም በተመለከተ ከጫፍ እስከ ጫፍ መመሪያዎችን ለማግኘት
በመተግበሪያ ውስጥ ያሉ ቅርሶች (የXcode ፕሮጀክት ሽቦ፣ የቻቻፖሊ አጠቃቀም፣ ወዘተ)፣ ይመልከቱ
`docs/connect_swift_integration.md`.

> **ማስታወሻ:** ለዚህ ፍሰት CI አውቶሜትድ የማክሮስ ገንቢዎች ከተፈለገ በኋላ ያርፋል
> የአፕል መገልገያ መስመር ላይ ይመጣል (በመለቀቅ ኢንጂነሪንግ ማክኦኤስ መገንቢያ የኋላ መዝገብ ውስጥ ተከታትሏል)።
> እስከዚያ ድረስ ከታች ያሉት እርምጃዎች በልማት ማክ ላይ በእጅ መተግበር አለባቸው።

## ቅድመ ሁኔታዎች

- የቅርብ ጊዜ የተረጋጋ የ Xcode ትዕዛዝ መስመር መሳሪያዎች የተጫነ የማክሮስ አስተናጋጅ።
- ከስራ ቦታ `rust-toolchain.toml` ጋር የሚዛመድ ዝገት የመሳሪያ ሰንሰለት።
- ፈጣን የመሳሪያ ሰንሰለት 5.7 ወይም ከዚያ በላይ።
- CocoaPods (በ Ruby Gems በኩል) ወደ ማዕከላዊ ዝርዝሮች ማከማቻ ከታተመ።
- የስዊፍት ቅርሶችን ለመሰየም የ I18NT0000000X Iroha የመልቀቂያ መፈረሚያ ቁልፎችን መድረስ።

## የስሪት ሞዴል

1. ለ Norito ኮዴክ (`crates/norito/Cargo.toml`) የ Rust crate ሥሪቱን ይወስኑ።
2. የስራ ቦታውን በሚለቀቅ ለዪ (ለምሳሌ `v2.1.0`) መለያ ይስጡ።
3. ለስዊፍት ጥቅል እና ለ CocoaPods podspec ተመሳሳይ የትርጉም ስሪት ይጠቀሙ።
4. የ Rust crate ስሪቱን ሲጨምር, ሂደቱን ይድገሙት እና ተዛማጅ ያትሙ
   ፈጣን ቅርስ። ስሪቶች በመሞከር ጊዜ የሜታዳታ ቅጥያዎችን (ለምሳሌ `-alpha.1`) ሊያካትቱ ይችላሉ።

## ደረጃዎችን ይገንቡ

1. ከማከማቻ ስር፣ የXCFrameworkን ለመሰብሰብ የረዳት ስክሪፕቱን ጥራ፡

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   ስክሪፕቱ የRust bridge ላይብረሪውን ለiOS እና ለማክሮስ ኢላማዎች ያጠናቅራል እና እነዚህን ይጠቀለላል
   በአንድ XCFramework ማውጫ ስር የተገኙ የማይንቀሳቀሱ ቤተ-መጻሕፍት።
   እንዲሁም `dist/NoritoBridge.artifacts.json` ያመነጫል, የድልድዩን ስሪት እና
   በፕላትፎርም SHA-256 hashes (ስሪቱን በI18NI0000019X ይሽሩት
   ያስፈልጋል)።

2. ለስርጭት የ XCFramework ዚፕ፡-

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. ወደ አዲሱ ለመጠቆም የስዊፍት ጥቅል ዝርዝር መግለጫ (`IrohaSwift/Package.swift`) ያዘምኑ።
   ስሪት እና ቼክ;

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   የሁለትዮሽ ኢላማውን ሲገልጹ ቼክሱን በ I18NI0000021X ይመዝግቡ።

4. `IrohaSwift/IrohaSwift.podspec`ን በአዲሱ ስሪት፣ ቼክሰም እና ማህደር አዘምን
   URL.

5. ** ድልድዩ አዲስ ኤክስፖርት ካገኘ ራስጌዎችን ያድሱ።** የስዊፍት ድልድይ አሁን አጋልጧል።
   `connect_norito_set_acceleration_config` ስለዚህ `AccelerationSettings` ብረት መቀየር ይችላል /
   የጂፒዩ መደገፊያዎች። `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` ያረጋግጡ
   ዚፕ ከመደረጉ በፊት `crates/connect_norito_bridge/include/connect_norito_bridge.h` ይዛመዳል።

6. መለያ ከመስጠትዎ በፊት የስዊፍት ማረጋገጫ ስብስብን ያሂዱ፡-

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   የመጀመሪያው ትእዛዝ የስዊፍት ጥቅል (`AccelerationSettings`ን ጨምሮ) መቆየቱን ያረጋግጣል።
   አረንጓዴ; ሁለተኛው የቋሚነት እኩልነትን ያረጋግጣል፣ የፓርቲ/CI ዳሽቦርዶችን ይሰጣል፣ እና
   በBuildkite (የእ.ኤ.አ.ን ጨምሮ) የተተገበሩትን ተመሳሳይ የቴሌሜትሪ ፍተሻዎችን ይሠራል
   `ci/xcframework-smoke:<lane>:device_tag` ሜታዳታ መስፈርት)።

7. የተፈጠሩትን ቅርሶች በሚለቀቅበት ቅርንጫፍ ውስጥ አስገባ እና ቃልህን መለያ ስጥ።

## ማተም

### የስዊፍት ጥቅል አስተዳዳሪ

- መለያውን ወደ ይፋዊ የጂት ማከማቻ ይግፉት።
- መለያው በጥቅል መረጃ ጠቋሚ (አፕል ወይም የማህበረሰብ መስታወት) ሊደረስበት የሚችል መሆኑን ያረጋግጡ።
- ሸማቾች አሁን በ `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")` ላይ ሊመኩ ይችላሉ።

### CocoaPods

1. ፖድውን በአካባቢው ያረጋግጡ፡-

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. የዘመነውን podspec ን ይጫኑ፡-

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. በ CocoaPods ኢንዴክስ ውስጥ አዲሱን ስሪት እንደታየ ያረጋግጡ።

## CI ግምት

- የማሸጊያ ስክሪፕቱን የሚያሄድ፣ ቅርሶችን የሚያከማች እና የሚሰቀል የማክሮስ ስራ ይፍጠሩ
  የተፈጠረ ቼክ እንደ የስራ ፍሰት ውጤት።
- ጌት በስዊፍት ማሳያ መተግበሪያ ሕንፃ ላይ አዲስ ከተመረተው ማዕቀፍ ጋር ይለቀቃል።
- ውድቀቶችን ለመመርመር የሚረዱ የግንባታ ምዝግብ ማስታወሻዎችን ያከማቹ።

## ተጨማሪ ራስ-ሰር ሀሳቦች

- ሁሉም አስፈላጊ ኢላማዎች ከተጋለጡ በኋላ `xcodebuild -create-xcframework` በቀጥታ ይጠቀሙ።
- ከገንቢ ማሽኖች ውጭ ለማሰራጨት ፊርማ/ማስታወሻን ያዋህዱ።
- SPMን በመሰካት ከታሸገው ስሪት ጋር የመዋሃድ ሙከራዎችን በደረጃ ያቆዩ
  የመልቀቂያ መለያ ላይ ጥገኛ.