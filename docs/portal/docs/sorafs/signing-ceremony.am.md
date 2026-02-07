---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a4be274ad087f292559c4b83a120ad20316a1e1dfe0ccbfb9aad42235ac136b
source_last_modified: "2026-01-05T09:28:11.909870+00:00"
translation_last_reviewed: 2026-02-07
id: signing-ceremony
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
translator: machine-google-reviewed
---

> ፍኖተ ካርታ፡ **SF-1b — የሶራ ፓርላማ ቋሚ ማጽደቆች።**

ለSoraFS chunker fixtures ጥቅም ላይ የዋለው በእጅ የመፈረም ሥነ ሥርዓት ጡረታ ወጥቷል። ሁሉም
ማጽደቆች አሁን በ **Sora ፓርላማ** በኩል ይፈስሳሉ፣ በአደራደር ላይ የተመሰረተው DAO ያ
Nexus ያስተዳድራል። የፓርላማ አባላት XOR ዜግነት ለማግኘት፣ ይሽከረከራሉ።
ፓነሎች፣ እና የሰንሰለት ድምጾችን ያጸድቁ፣ የማይቀበሉ ወይም የሚመልሱ ዕቃዎችን ይስጡ
ይለቀቃል. ይህ መመሪያ የሂደቱን እና የገንቢ መሳሪያዎችን ያብራራል.

## የፓርላማ አጠቃላይ እይታ

- ** ዜግነት *** - እንደ ዜጋ ለመመዝገብ ኦፕሬተሮች አስፈላጊውን XOR ያስተሳሰራሉ
  ለመደርደር ብቁ መሆን።
- ** ፓነሎች *** - ኃላፊነቶች በሚሽከረከሩ ፓነሎች (መሠረተ ልማት ፣
  ልከኝነት፣ ግምጃ ቤት፣…) የመሠረተ ልማት ፓነል የSoraFS መግጠሚያ አለው።
  ማጽደቆች.
- ** መደርደር እና ማሽከርከር *** — የፓነል መቀመጫዎች በተገለጸው ገለጻ እንደገና ይሳሉ
  የፓርላማው ሕገ መንግሥት አንድም ቡድን በሞኖፖሊስ አይፀድቅም።

## ቋሚ የማጽደቅ ፍሰት

1. **የሃሳብ አቅርቦት**
   - የ Tooling WG እጩውን `manifest_blake3.json` ጥቅል ፕላስ ይሰቅላል
     fixture ልዩነት በ `sorafs.fixtureProposal` በኩል በሰንሰለት መዝገብ ላይ።
   - ፕሮፖዛሉ የBLAKE3 መፍጨትን፣ የትርጉም እትምን እና ማስታወሻዎችን ይቀይራል።
2. ** ግምገማ እና ድምጽ መስጠት ***
   - የመሠረተ ልማት ፓነል ምደባውን በፓርላማው በኩል ይቀበላል
     ወረፋ
   - የፓነል አባላት የCI artefactsን ይመረምራሉ፣ የተመጣጠነ ሙከራዎችን ያካሂዳሉ እና ክብደታቸውን ይወስዳሉ
     በሰንሰለት ላይ ድምጾች.
3. **ማጠናቀቂያ**
   - አንድ ጊዜ ምልአተ ጉባኤው ከተጠናቀቀ፣ የሩጫ ጊዜው የማረጋገጫ ክስተትን ያመነጫል።
     ቀኖናዊ አንጸባራቂ መፍጨት እና Merkle ቁርጠኝነት ለቋሚ ክፍያ ጭነት።
   - ዝግጅቱ ደንበኞች ማምጣት እንዲችሉ በ SoraFS መዝገብ ውስጥ ተንጸባርቋል
     የቅርብ ጊዜ ፓርላማ የጸደቀ ዝርዝር መግለጫ።
4. **መከፋፈል**
   - የ CLI ረዳቶች (`cargo xtask sorafs-fetch-fixture`) የተፈቀደውን አንጸባራቂ ይጎትቱ
     ከ Nexus RPC. የማከማቻው JSON/TS/Go ቋሚዎች በሥምረት ይቆያሉ።
     `export_vectors` እንደገና በማስኬድ እና በሰንሰለቱ ላይ ያለውን የምግብ መፍጫ ሂደት በማረጋገጥ ላይ
     መዝገብ.

## የገንቢ የስራ ፍሰት

- የቤት ዕቃዎችን በ:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- የተፈቀደውን ኤንቨሎፕ ለማውረድ የፓርላማ ረዳትን ይጠቀሙ፣ ያረጋግጡ
  ፊርማዎች እና የአካባቢ መገልገያዎችን ያድሱ። ነጥብ `--signatures` ላይ
  በፓርላማ የታተመ ፖስታ; ረዳቱ ተጓዳኝ አንጸባራቂውን ይፈታል ፣
  የ BLAKE3 መፈጨትን እንደገና ያሰላል እና ቀኖናዊውን ያስፈጽማል
  `sorafs.sf1@1.0.0` መገለጫ።

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

አንጸባራቂው በተለያየ ዩአርኤል ላይ የሚኖር ከሆነ `--manifest` ይለፉ። ያልተፈረሙ ፖስታዎች
`--allow-unsigned` ለአካባቢው ጭስ ሩጫ ካልተዋቀረ በስተቀር ውድቅ ተደርገዋል።

- በማስተናገጃ መግቢያ በር በኩል አንጸባራቂን በሚያረጋግጡበት ጊዜ ኢ18NT00000007X ሳይሆን ኢላማ ያድርጉ።
  የሀገር ውስጥ ሸክሞች፡-

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- የአካባቢ CI ከአሁን በኋላ `signer.json` ዝርዝር አያስፈልግም።
  `ci/check_sorafs_fixtures.sh` repo ሁኔታን ከቅርብ ጊዜው ጋር ያወዳድራል።
  በሰንሰለት ላይ ቁርጠኝነት እና ሲለያዩ ይወድቃሉ።

## የአስተዳደር ማስታወሻዎች

- የፓርላማው ሕገ መንግሥት ምልአተ ጉባኤን፣ ሽክርክርን፣ እና ጭማሪን ይቆጣጠራል—አይ
  የሣጥን ደረጃ ማዋቀር ያስፈልጋል።
- የአደጋ ጊዜ መልሶ ማቋቋሚያ በፓርላማ አወያይ በኩል ይካሄዳል። የ
  የመሠረተ ልማት ፓነል የቀደመውን አንጸባራቂ የሚያመለክት የተገላቢጦሽ ፕሮፖዛል ያቀርባል
  መፍጨት፣ ከተፈቀደ በኋላ ልቀቱን የሚተካ።
- ታሪካዊ ማፅደቆች በ SoraFS መዝገብ ውስጥ ለፎረንሲክ ይገኛሉ
  እንደገና አጫውት።

## የሚጠየቁ ጥያቄዎች

- ** `signer.json` የት ሄደ?  
  ተወግዷል። ሁሉም የፈራሚ ባህሪ በሰንሰለት ላይ ይኖራል; `manifest_signatures.json`
  በማጠራቀሚያው ውስጥ ከቅርብ ጊዜው ጋር መዛመድ ያለበት የገንቢ መሣሪያ ብቻ አለ።
  የማጽደቅ ክስተት.

- ** አሁንም የአካባቢ Ed25519 ፊርማዎችን እንፈልጋለን?**  
  ቁጥር፡ የፓርላማ ማጽደቂያዎች በሰንሰለት ላይ እንደ ቅርስ ተቀምጠዋል። የአካባቢ መገልገያዎች አሉ።
  ለዳግም መራባት ግን ከፓርላማ ውሣኔ አንፃር የተረጋገጡ ናቸው።

- **ቡድኖች ማጽደቆችን እንዴት ይቆጣጠራሉ?**  
  ለ`ParliamentFixtureApproved` ክስተት ይመዝገቡ ወይም መዝገቡን በ በኩል ይጠይቁ
  Nexus RPC የአሁኑን አንጸባራቂ መፍጨት እና የፓነል ጥቅል ጥሪን ለማውጣት።