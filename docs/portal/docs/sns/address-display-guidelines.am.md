---
lang: am
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 91a1116d82eb9845a26805effed00ad8cd13c1ebd034aef3827aebfa4e1eb846
source_last_modified: "2026-01-28T17:11:30.700223+00:00"
translation_last_reviewed: 2026-02-07
id: address-display-guidelines
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for I105 vs I105 Sora address presentation (ADDR-6).
translator: machine-google-reviewed
---

ExplorerAddressCard ከ'@site/src/components/ExplorerAddressCard' አስመጣ፤

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/sns/address_display_guidelines.md`ን ያንጸባርቃል እና አሁን ያገለግላል
እንደ ቀኖናዊ ፖርታል ቅጂ. የምንጭ ፋይሉ ለትርጉም PRs ዙሪያ ተጣብቋል።
::

የኪስ ቦርሳዎች፣ አሳሾች እና የኤስዲኬ ናሙናዎች የመለያ አድራሻዎችን የማይለዋወጥ አድርገው መያዝ አለባቸው
ሸክሞች. የአንድሮይድ የችርቻሮ ቦርሳ ናሙና በ ውስጥ
`examples/android/retail-wallet` አሁን የሚፈለገውን UX ጥለት ያሳያል፡-

- ** ባለሁለት ቅጂ ኢላማዎች።** ሁለት ግልጽ የቅጂ አዝራሮችን ይላኩ-I105 (ተመራጭ) እና
  የታመቀ የሶራ-ብቻ ቅጽ (`i105`፣ ሁለተኛ-ምርጥ)። I105 ከውጪ ለመጋራት ሁልጊዜ ደህንነቱ የተጠበቀ ነው።
  እና የQR ጭነትን ያበረታታል። የታመቀው ተለዋጭ የውስጥ መስመር ማካተት አለበት።
  ማስጠንቀቂያ በSora-aware መተግበሪያዎች ውስጥ ብቻ ስለሚሰራ። የአንድሮይድ ችርቻሮ
  የኪስ ቦርሳ ናሙና ሽቦዎች ሁለቱም የቁሳቁስ ቁልፎች እና የመሳሪያ ፍንጮቻቸው
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`፣ እና
  የ iOS SwiftUI ማሳያ በ `AddressPreviewCard` በኩል ተመሳሳይ UX ያንጸባርቃል
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- ** ሞኖስፔስ፣ ሊመረጥ የሚችል ጽሑፍ።** ሁለቱንም ሕብረቁምፊዎች በሞኖስፔስ ቅርጸ-ቁምፊ እና
  `textIsSelectable="true"` ተጠቃሚዎች IME ሳይጠሩ እሴቶችን መፈተሽ ይችላሉ።
  አርትዖት ሊደረጉ የሚችሉ መስኮችን ያስወግዱ፡ አይ ኤም ኢዎች ቃናን እንደገና መፃፍ ወይም የዜሮ ስፋት ኮድ ነጥቦችን ማስገባት ይችላሉ።
- ** ስውር ነባሪ የጎራ ፍንጮች።** መራጩ በተዘዋዋሪ ሲጠቁም።
  `default` ጎራ፣ ኦፕሬተሮችን የሚያስታውስ የመግለጫ ጽሁፍ ላይ ምንም ቅጥያ አያስፈልግም።
  አሳሾች ደግሞ መራጩ ጊዜ ቀኖናዊ ጎራ መለያ ማጉላት አለባቸው
  መፈጨትን ያስቀምጣል።
- **I105 QR ጭነት።** QR ኮዶች የI105 ሕብረቁምፊ መመስጠር አለባቸው። QR ትውልድ ከሆነ
  አልተሳካም፣ ከባዶ ምስል ይልቅ ግልጽ የሆነ ስህተት አሳይ።
- ** የክሊፕቦርድ መልእክት።** የታመቀውን ቅጽ ከገለበጡ በኋላ ቶስት ይልቀቁ ወይም
  የሶራ-ብቻ እና ለአይኤምኢ ማንግሊንግ የተጋለጠ መሆኑን ለተጠቃሚዎች የሚያስታውስ መክሰስ ባር።

እነዚህን የጥበቃ መንገዶች መከተል የዩኒኮድ/IME ሙስናን ይከላከላል እና ያረካል
ADDR-6 የመንገድ ካርታ ተቀባይነት መስፈርት ለኪስ ቦርሳ/አሳሽ UX።

## ቅጽበታዊ ገጽ እይታዎች

የአዝራር መለያዎችን ለማረጋገጥ በትርጉም ግምገማ ወቅት የሚከተሉትን መገልገያዎች ተጠቀም።
የመሳሪያ ምክሮች እና ማስጠንቀቂያዎች በመድረኮች ላይ ይቆያሉ፡

- አንድሮይድ ማጣቀሻ: `/img/sns/address_copy_android.svg`

  ![አንድሮይድ ባለሁለት ቅጂ ማጣቀሻ](/img/sns/address_copy_android.svg)

- የ iOS ማጣቀሻ: `/img/sns/address_copy_ios.svg`

  ![iOS ባለሁለት ቅጂ ማጣቀሻ](/img/sns/address_copy_ios.svg)

## የኤስዲኬ ረዳቶች

እያንዳንዱ ኤስዲኬ I105 (ተመራጭ) እና የታመቀ (`sora`፣ ሁለተኛ-ምርጥ) የሚመልስ ምቹ ረዳትን ያጋልጣል።
የUI ንብርብሮች ወጥነት እንዲኖራቸው ከማስጠንቀቂያ ህብረቁምፊው ጋር ይመሰርታሉ፡

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
ጃቫ ስክሪፕት መርማሪ፡- `inspectAccountId(...)` የታመቀውን ማስጠንቀቂያ ይመልሳል
  ደዋዮች `i105` ባቀረቡ ቁጥር ሕብረቁምፊ እና ከ`warnings` ጋር ያያይዙታል።
  በጥሬው፣ ስለዚህ አሳሾች/የኪስ ቦርሳ ዳሽቦርዶች የሶራ-ብቻ ማሳሰቢያውን ሊያሳዩ ይችላሉ።
  በሚፈጥሩት ጊዜ ብቻ ሳይሆን በመለጠፍ/በማረጋገጫ ፍሰቶች ወቅት
  የታመቀ ቅጽ እራሳቸው.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- ስዊፍት: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

የኢኮድ አመክንዮ በUI ንብርብሮች ውስጥ እንደገና ከመተግበር ይልቅ እነዚህን ረዳቶች ይጠቀሙ።
የጃቫ ስክሪፕት ረዳት በ`selector` ክፍያ በI18NI0000038X ላይ ያጋልጣል
(`tag`፣ `digest_hex`፣ `registry_id`፣ `label`) UIs
መምረጡ በሎካል-12 ወይም በመመዝገቢያ የተደገፈ ጥሬ ጭነት እንደገና ሳይተነተን።

## Explorer instrumentation ማሳያ

<ExplorerAddressCard />

አሳሾች የኪስ ቦርሳ ቴሌሜትሪ እና የተደራሽነት ስራን ማንጸባረቅ አለባቸው፡

- አዝራሮችን ለመቅዳት `data-copy-mode="i105|qr"` ያመልክቱ የፊት-ጫፎቹ የአጠቃቀም ቆጣሪዎችን ያመነጫሉ
  ከ Torii-side I18NI0000044X መለኪያ ጋር። ከላይ ያለው የማሳያ ክፍል ይልካል።
  አንድ የ`iroha:address-copy` ክስተት ከ`{mode,timestamp}` ጋር—ይህንን ወደ የእርስዎ ትንታኔ/ቴሌሜትሪ ያገናኙት።
  የቧንቧ መስመር (ለምሳሌ፡ ወደ ክፍል ወይም በNORITO የሚደገፍ ሰብሳቢ) ስለዚህ ዳሽቦርዶች አገልጋይን ማዛመድ እንዲችሉ
  የአድራሻ-ቅርጸት አጠቃቀም ከደንበኛ ቅጂ ባህሪ ጋር። እንዲሁም የTorii የጎራ ቆጣሪዎችን ያንጸባርቁ
  (`torii_address_domain_total{domain_kind}`) በተመሳሳይ ምግብ ውስጥ የአካባቢ-12 የጡረታ ግምገማዎች እንዲችሉ
  የ30 ቀን I18NI0000048X የዜሮ አጠቃቀም ማረጋገጫ በቀጥታ ከ`address_ingest` ወደ ውጪ ላክ
  Grafana ሰሌዳ.
- እያንዳንዱን መቆጣጠሪያ ከተለየ I18NI0000050X/`aria-describedby` ፍንጮች ጋር ያጣምሩ
  በቀጥታ ለማጋራት ደህንነቱ የተጠበቀ ነው (`I105`) ወይም Sora-only (የተጨመቀ `sora`)። ስውር-ጎራ መግለጫ ጽሑፍን ያካትቱ
  መግለጫው አጋዥ ቴክኖሎጂ በምስላዊ የሚታየውን ተመሳሳይ አውድ ያሳያል።
- የቀጥታ ክልልን ያጋልጡ (ለምሳሌ፡ `<output aria-live="polite">…</output>`) የቅጂ ውጤቶችን እና
  ማስጠንቀቂያዎች፣ የVoiceOver/TalkBack ባህሪን አሁን ከስዊፍት/አንድሮይድ ናሙናዎች ጋር በማዛመድ።

ይህ መሳሪያ ADDR-6bን ያሟላል ኦፕሬተሮች ሁለቱንም Torii መግባታቸውን እና ማየት እንደሚችሉ በማረጋገጥ
የአካባቢ መራጮች ከመጥፋታቸው በፊት የደንበኛ-ጎን ቅጂ ሁነታዎች።

## የአካባቢ → የአለምአቀፍ ፍልሰት መሳሪያ ስብስብ

ራስ-ሰር ለማድረግ [Local → Global Toolkit](local-to-global-toolkit.md) ይጠቀሙ
የJSON ኦዲት ሪፖርት እና የተለወጠው ተመራጭ I105/ሁለተኛ-ምርጥ የታመቀ (`sora`) ኦፕሬተሮች የሚያያይዙት ዝርዝር
ወደ ዝግጁነት ትኬቶች፣ ተጓዳኝ ራንቡክ I18NT0000001X ሲያገናኝ
ጥብቅ ሁነታ መቁረጥን የሚከፍቱ ዳሽቦርዶች እና Alertmanager ህጎች።

## የሁለትዮሽ አቀማመጥ ፈጣን ማጣቀሻ (ADDR-1a)

ኤስዲኬዎች የላቀ የአድራሻ መሣሪያን ሲያዩ (ተቆጣጣሪዎች፣ የማረጋገጫ ፍንጮች፣
አንጸባራቂ ግንበኞች)፣ በተያዘው ቀኖናዊ የሽቦ ቅርጸት ላይ ገንቢዎችን ጠቁም።
`docs/account_structure.md`. አቀማመጡ ሁልጊዜ ነው
`header · selector · controller`፣ የራስጌ ቢትስ ያሉበት፡-

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits7-5) ዛሬ; ዜሮ ያልሆኑ እሴቶች የተጠበቁ እና አለባቸው
  `AccountAddressError::InvalidHeaderVersion` ከፍ ያድርጉ።
- `addr_class` ነጠላ (I18NI0000061X) vs multisig (`1`) መቆጣጠሪያዎችን ይለያል።
- `norm_version = 1` የ Normv1 መምረጫ ደንቦችን ይደብቃል። የወደፊት ደንቦች እንደገና ጥቅም ላይ ይውላሉ
  ተመሳሳይ ባለ 2-ቢት መስክ.
- `ext_flag` ሁልጊዜም I18NI0000065X ነው—የተቀመጡ ቢትስ የማይደገፉ የክፍያ ማራዘሚያዎችን ያመለክታሉ።

መራጩ ወዲያውኑ ራስጌውን ይከተላል፡-

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

የዩአይ እና ኤስዲኬ ወለሎች የመምረጡን አይነት ለማሳየት ዝግጁ መሆን አለባቸው፡-

- `0x00` = ስውር ነባሪ ጎራ (ክፍያ የለም)።
- `0x01` = የአካባቢ መፍጨት (12-ባይት `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = ዓለም አቀፍ የመዝገብ መግቢያ (ትልቅ-ኤንዲያን `registry_id:u32`)።

የኪስ ቦርሳ መጠቀሚያ በሰነዶች/ሙከራዎች ውስጥ ሊያገናኝ ወይም ሊካተት የሚችል ቀኖናዊ ሄክስ ምሳሌዎች፡-

| መራጭ አይነት | ቀኖናዊ ሄክስ |
|------------|-----------|
| ስውር ነባሪ | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| የአካባቢ መፍጨት (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| ዓለም አቀፍ መዝገብ ቤት (I18NI0000074X) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

ለሙሉ መራጭ/ግዛት `docs/source/references/address_norm_v1.md` ይመልከቱ
ሠንጠረዥ እና `docs/account_structure.md` ለተሟላ ባይት ንድፍ።

## ቀኖናዊ ቅርጾችን ማስፈጸም

ሕብረቁምፊዎች በ ADDR-5 የተመዘገበውን የCLI የስራ ፍሰት መከተል አለባቸው፡

1. `iroha tools address inspect` አሁን የተዋቀረ JSON ማጠቃለያን ከ I105 ጋር አውጥቷል፣
   የታመቀ፣ እና ቀኖናዊ የሄክስ ጭነቶች። ማጠቃለያው `domain`ንም ያካትታል
   ከ `kind`/`warning` መስኮች ያለው ነገር እና ማንኛውንም የቀረበውን ጎራ በ
   `input_domain` መስክ. `kind` `local12` ሲሆን CLI ማስጠንቀቂያ ያትማል ለ
   stderr እና JSON ማጠቃለያ የCI ቧንቧዎች እና ኤስዲኬዎች ተመሳሳይ መመሪያ ያስተጋባል
   ሊገለጽ ይችላል. የተቀየረውን በፈለጉት ጊዜ `legacy  suffix` ይለፉ
   ኢንኮዲንግ እንደ `<i105>@<domain>` እንደገና ተጫውቷል።
2. ኤስዲኬዎች በጃቫስክሪፕት ረዳት በኩል ተመሳሳይ ማስጠንቀቂያ/ማጠቃለያ ሊያወጡ ይችላሉ፡-

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  እርስዎ ካልሆነ በስተቀር ረዳቱ ከትክክለኛው የተገኘውን የI105 ቅድመ ቅጥያ ይጠብቃል።
  በግልጽ I18NI0000087X ያቅርቡ፣ ስለዚህ ነባሪ ላልሆኑ አውታረ መረቦች ማጠቃለያዎች ይሰጣሉ።
  በጸጥታ እንደገና በነባሪ ቅድመ ቅጥያ አያቅርቡ።

3. `i105.value` ወይም `i105` እንደገና በመጠቀም ቀኖናዊውን ክፍያ ይለውጡ።
   መስኮች ከማጠቃለያው (ወይም በ `--format` በኩል ሌላ ኢንኮዲንግ ይጠይቁ)። እነዚህ
   ሕብረቁምፊዎች አስቀድመው በውጭ ለመጋራት ደህና ናቸው።
4. መግለጫዎችን፣ መዝገቦችን እና ደንበኛን የሚመለከቱ ሰነዶችን በ
   ቀኖናዊ ቅፅ እና የአካባቢ መራጮች እንደሚሆኑ ተጓዳኝዎችን ያሳውቁ
   ማቋረጡ እንደተጠናቀቀ ውድቅ ተደርጓል።
5. ለጅምላ የውሂብ ስብስቦች, አሂድ
   `iroha tools address audit --input addresses.txt --network-prefix 753`. ትዕዛዙ
   በአዲስ መስመር የተከፋፈሉ ቀጥተኛ ቃላትን ያነባል (ከ`#` የሚጀምሩ አስተያየቶች ችላ ተብለዋል እና
   `--input -` ወይም ምንም ባንዲራ STDIN አይጠቀምም) የJSON ዘገባ ከ ጋር ያቀርባል
   ቀኖናዊ/ተመራጭ I105/ሁለተኛ-ምርጥ የታመቀ (`sora`) ማጠቃለያ ለእያንዳንዱ ግቤት፣ እና ሁለቱንም ትንተና ይቆጥራል።
   ቆሻሻ ረድፎችን እና የበር አውቶማቲክን ከI18NI0000095X ጋር ያካተቱ ቆሻሻዎች
   አንዴ ኦፕሬተሮች በCI ውስጥ የአካባቢ መራጮችን ለማገድ ዝግጁ ከሆኑ።
6. ከአዲስ መስመር ወደ አዲስ መስመር እንደገና መፃፍ ሲፈልጉ ይጠቀሙ
  ለአካባቢ-መራጮች ማሻሻያ የተመን ሉሆች ይጠቀሙ
  ቀኖናዊ ኢንኮዲንግ፣ ማስጠንቀቂያዎች እና ድክመቶችን በአንድ ማለፊያ የሚያጎላ `input,status,format,…` CSV ወደ ውጭ ለመላክ።
   ረዳቱ አካባቢያዊ ያልሆኑ ረድፎችን በነባሪ ይዘላል፣ የቀረውን ግቤት ሁሉ ይለውጣል
   ወደ ተጠየቀው ኢንኮዲንግ (I105 ተመራጭ/የተጨመቀ (`sora`) ሁለተኛ-ምርጥ/ሄክስ/JSON)፣ እና ያስቀምጣል።
   `legacy  suffix` ሲዋቀር ኦሪጅናል ጎራ። ከ `--allow-errors` ጋር ያጣምሩት።
   የቆሻሻ መጣያ የተበላሹ ፊደሎችን ቢይዝም መቃኘትን ለመቀጠል።
7. CI/lint አውቶሜሽን `ci/check_address_normalize.sh`ን ማስኬድ ይችላል፣ ይህም የሚያወጣው
   ከ `fixtures/account/address_vectors.json` ውስጥ ያሉ የአካባቢያዊ መምረጫዎች, ይቀየራሉ
   እነሱን በ `iroha tools address normalize` ፣ እና ድጋሚ ማጫወት
   `iroha tools address audit` ልቀቶች ከአሁን በኋላ እንደማይለቀቁ ለማረጋገጥ
   የአካባቢ መሟጠጥ.`torii_address_local8_total{endpoint}` ሲደመር
`torii_address_collision_total{endpoint,kind="local12_digest"}`፣
`torii_address_collision_domain_total{endpoint,domain}` እና የ
Grafana ቦርድ `dashboards/grafana/address_ingest.json` ማስፈጸሚያውን ያቀርባል
ምልክት፡ አንዴ የምርት ዳሽቦርዶች ዜሮ ህጋዊ የአካባቢ ማስረከቦችን ያሳያሉ
ዜሮ የአካባቢ-12 ግጭቶች ለ30 ተከታታይ ቀናት፣ Torii Local-8ን ይገለብጣል።
በዋና ኔት ላይ ወደ ከባድ-ውድቀት በር ይሂዱ፣ አንዴ አለምአቀፍ ጎራዎች ካሉ በኋላ Local-12
ተዛማጅ የመመዝገቢያ ግቤቶች. የ CLI ውፅዓት ከዋኝ ፊት ለፊት ያለውን ማስታወቂያ አስቡበት
ለዚህ በረዶ-ተመሳሳይ የማስጠንቀቂያ ሕብረቁምፊ በኤስዲኬ የመሳሪያ ምክሮች እና ጥቅም ላይ ይውላል
አውቶሜሽን ከመንገድ ካርታው መውጫ መስፈርት ጋር ያለውን እኩልነት ለመጠበቅ። Torii አሁን ነባሪ ሆኗል።
ድግግሞሾችን ሲመረምሩ. `torii_address_domain_total{domain_kind}` ማንጸባረቅዎን ይቀጥሉ
ወደ Grafana (`dashboards/grafana/address_ingest.json`) ስለዚህ ADDR-7 ማስረጃ ጥቅል
`domain_kind="local12"` ለሚፈለገው የ30 ቀን መስኮት በዜሮ መቆየቱን ማረጋገጥ ይችላል
(`dashboards/alerts/address_ingest_rules.yml`) ሶስት የጥበቃ መንገዶችን ይጨምራል።

- አንድ አውድ አዲስ የአካባቢ-8 ሪፖርት ባደረገ ቁጥር `AddressLocal8Resurgence` ገጾች
  መጨመር. የጥብቅ ሁነታ ልቀቶችን አቁም፣ የሚያስከፋውን የኤስዲኬ ገጽ በ ውስጥ ያግኙ
  ምልክቱ ወደ ዜሮ እስኪመለስ ድረስ - ከዚያም ነባሪውን ወደነበረበት ይመልሱ (`true`).
- `AddressLocal12Collision` ሁለት Local-12 መለያዎች ወደ ተመሳሳይ ሃሽ ሲያደርጉ ያቃጥላል
  መፈጨት ። አንጸባራቂ ማስተዋወቂያዎችን ባለበት አቁም፣ ኦዲት ለማድረግ የአካባቢ → አለምአቀፍ መሣሪያ ስብስብን ያስኪዱ
  የምግብ መፍጫውን ካርታ እንደገና ከመውጣቱ በፊት ከ Nexus አስተዳደር ጋር ያስተባበሩ.
  የመመዝገቢያ መግቢያ ወይም የታችኛው ተፋሰስ ልቀቶችን እንደገና ማንቃት።
- `AddressInvalidRatioSlo` የበረራ-ሰፊው ልክ ያልሆነ ምጥጥን ሲያስጠነቅቅ (ሳይጨምር)
  የአካባቢ-8/ጥብቅ ሁነታ ውድቅ የተደረገ) ከ 0.1% SLO ለአስር ደቂቃዎች ይበልጣል። ተጠቀም
  `torii_address_invalid_total` ተጠያቂውን አውድ/ምክንያት እና
  ጥብቅ ሁነታን እንደገና ከማንቃትዎ በፊት ከባለቤቱ የኤስዲኬ ቡድን ጋር ማስተባበር።

### የልቀት ማስታወሻ ቅንጣቢ (የኪስ ቦርሳ እና አሳሽ)

በሚላክበት ጊዜ የሚከተለውን ጥይት በኪስ ቦርሳ/አሳሽ ውስጥ ያካትቱ
መቁረጫው:

> ** አድራሻዎች:** `iroha tools address normalize` ታክሏል።
> ረዳት እና ወደ CI (`ci/check_address_normalize.sh`) በሽቦ ሰራው
> Local-8/Local-12 በሜይንኔት ከመታገዱ በፊት። ወደ ውጭ የሚላኩ ማናቸውንም ያዘምኑ
> ትዕዛዙን ያሂዱ እና የተለመደውን ዝርዝር ከመልቀቂያ ማስረጃ ጥቅል ጋር ያያይዙ።