---
lang: am
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM ውቅር ስደት

# SM ውቅር ፍልሰት

የSM2/SM3/SM4 ባህሪ ስብስብን መልቀቅ ከ ጋር ከማጠናቀር በላይ ይጠይቃል
`sm` ባህሪ ባንዲራ። አንጓዎች ከተነባበረው በስተጀርባ ያለውን ተግባር ይከፍታሉ
`iroha_config` መገለጫዎች እና የዘፍጥረት አንጸባራቂ ተዛማጅነት እንዲኖረው ይጠብቁ
ነባሪዎች. ይህ ማስታወሻ ሀን ሲያስተዋውቅ የተመከረውን የስራ ሂደት ይይዛል
አሁን ያለው አውታረ መረብ ከ"Ed25519-ብቻ" ወደ "SM-የነቃ"።

## 1. የግንባታ መገለጫውን ያረጋግጡ

- ሁለትዮሾችን በ `--features sm` ያጠናቅቁ; እርስዎ ሲሆኑ ብቻ `sm-ffi-openssl` ይጨምሩ
  የOpenSSL/Tongsuo ቅድመ እይታ መንገድን ለመጠቀም እቅድ ማውጣቱ። ያለ `sm` ይገነባል።
  ባህሪው ምንም እንኳን አወቃቀሩ ቢነቃቅም የ `sm2` ፊርማዎችን ውድቅ ያድርጉ
  እነርሱ።
- CI የ `sm` ቅርሶችን እና ሁሉንም የማረጋገጫ ደረጃዎች (`ጭነት) ማተምን ያረጋግጡ
  test -p iroha_crypto - ባህሪያት sm`፣ የውህደት ዕቃዎች፣ fuzz suites) ማለፍ
  ለማሰማራት ባሰቡት ትክክለኛ ሁለትዮሽ ላይ።

## 2. የንብርብር ውቅር ይሽራል።

`iroha_config` ሶስት እርከኖችን ይተገበራል፡ `defaults` → `user` → `actual`። ኤስኤምኤስ ይላኩ።
ኦፕሬተሮች ወደ አረጋጋጮች የሚያሰራጩትን የ`actual` መገለጫ ይሽራል።
የገንቢ ነባሪዎች ሳይለወጡ እንዲቆዩ `user` በ Ed25519-ብቻ ይተዉት።

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

ተመሳሳዩን ብሎክ ወደ `defaults/genesis` አንጸባራቂ በካጋሚ ዘፍጥረት በኩል ይቅዱ
ማመንጨት…` (add `--የተፈቀደ-መፈረም sm2 --default-hash sm3-256` ከፈለጉ
ይሻራል) ስለዚህ የ `parameters` እገዳ እና የተከተተ ሜታዳታ በ
የአሂድ ጊዜ ውቅር. አንጸባራቂው እና ሲዋቀሩ እኩዮች ለመጀመር ፈቃደኛ አይደሉም
ቅጽበተ-ፎቶዎች ይለያያሉ.

## 3. የዘፍጥረት መግለጫዎችን እንደገና ማደስ

- ለእያንዳንዱ ሰው `kagami genesis generate --consensus-mode <mode>` ን ያሂዱ
  አካባቢን እና የዘመነውን JSON ከTOML መሻሮች ጋር ያድርጉ።
- መግለጫውን (`kagami genesis sign …`) ይፈርሙ እና `.nrt` ክፍያን ያሰራጩ።
  ካልተፈረመ JSON አንጸባራቂ የሚነኩ አንጓዎች የሩጫ ጊዜ ክሪፕቶ ናቸው።
  ውቅር በቀጥታ ከፋይሉ - አሁንም ለተመሳሳይ ወጥነት ተገዢ ነው።
  ቼኮች.

## 4. ከትራፊክ በፊት ያረጋግጡ

- የዝግጅት ክላስተር ከአዲሱ ሁለትዮሽ እና ማዋቀር ጋር ያቅርቡ፣ ከዚያ ያረጋግጡ፡
  - `/status` አንዴ እኩዮች እንደገና ከጀመሩ `crypto.sm_helpers_available = true` ያጋልጣል።
  - Torii መግቢያ አሁንም የSM2 ፊርማዎችን ውድቅ ያደርጋል `sm2` በሌለበት
    `allowed_signing` እና የተቀላቀሉ Ed25519/SM2 ስብስቦችን ዝርዝሩን ይቀበላል
    ሁለቱንም ስልተ ቀመሮችን ያካትታል።
  - `iroha_cli tools crypto sm2 export …` የዙር ጉዞዎች ቁልፍ ቁሳቁስ በአዲሱ በኩል ተዘርቷል።
    ነባሪዎች.
- SM2 መወሰኛ ፊርማዎችን የሚሸፍኑ የውህደት ጭስ ስክሪፕቶችን ያሂዱ እና
  የአስተናጋጅ/VM ወጥነት ለማረጋገጥ SM3 hashing።

## 5. የመመለሻ እቅድ- የተገላቢጦሹን ሰነድ ይመዝግቡ-`sm2`ን ከ `allowed_signing` ያስወግዱ እና ወደነበረበት ይመልሱ
  `default_hash = "blake2b-256"`. ለውጡን በተመሳሳዩ `actual` ይግፉት
  የመገለጫ ቧንቧው እያንዳንዱ አረጋጋጭ በብቸኝነት ይገለበጣል።
- የኤስ.ኤም. መግለጫዎችን በዲስክ ላይ ያስቀምጡ; ያልተዛመደ ውቅር እና ዘፍጥረት የሚያዩ እኩዮች
  ውሂብ ለመጀመር እምቢ ማለት ነው፣ ይህም ከፊል መልሶ መመለስን ይከላከላል።
- የOpenSSL/Tongsuo ቅድመ-እይታ ከተሳተፈ፣የማሰናከል ደረጃዎችን ያካትቱ
  `crypto.enable_sm_openssl_preview` እና የተጋሩ ነገሮችን ከ
  Runtime አካባቢ.

## የማጣቀሻ ቁሳቁስ

- [`docs/genesis.md`] (../../genesis.md) - የጄኔሲስ አንጸባራቂ አወቃቀር እና
  የ `crypto` እገዳ.
- [`docs/source/references/configuration.md`](../references/configuration.md) -
  የ `iroha_config` ክፍሎች እና ነባሪዎች አጠቃላይ እይታ።
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) - እስከ መጨረሻ
  የመጨረሻ ኦፕሬተር ማረጋገጫ ዝርዝር ለመላክ SM ምስጠራ።