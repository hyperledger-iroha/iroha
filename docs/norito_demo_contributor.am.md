---
lang: am
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito SwiftUI ማሳያ አስተዋዋቂ መመሪያ

ይህ ሰነድ የSwiftUI ማሳያን ከ ሀ ጋር ለማሄድ የሚያስፈልጉትን በእጅ የማዋቀር እርምጃዎችን ይይዛል
የአካባቢ Torii መስቀለኛ መንገድ እና የማስመሰያ ደብተር። `docs/norito_bridge_release.md` በ
በዕለት ተዕለት የልማት ተግባራት ላይ ማተኮር. ለማዋሃድ ጥልቅ የእግር ጉዞ
Norito ድልድይ/ቁልል ወደ Xcode ፕሮጀክቶች ያገናኙ፣ `docs/connect_swift_integration.md` ይመልከቱ።

## የአካባቢ ማዋቀር

1. በ `rust-toolchain.toml` ውስጥ የተገለጸውን የ Rust Toolchain ጫን።
2. Swift 5.7+ እና Xcode የትእዛዝ መስመር መሳሪያዎችን በ macOS ላይ ይጫኑ።
3. (የግድ ያልሆነ) ጫን [SwiftLint] (https://github.com/realm/SwiftLint) linting.
4. መስቀለኛ መንገድ በአስተናጋጅዎ ላይ መጨመሩን ለማረጋገጥ I18NI0000020X ን ያሂዱ።
5. I18NI0000021X ወደ I18NI0000022X ገልብጦ ማስተካከል
   ከአካባቢዎ ጋር የሚዛመዱ እሴቶች። መተግበሪያው ሲጀመር እነዚህን ተለዋዋጮች ያነባል፡-
   - `TORII_NODE_URL` — ቤዝ REST URL (የዌብሶኬት ዩአርኤሎች ከእሱ የተወሰዱ ናቸው)።
   - `CONNECT_SESSION_ID` — ባለ 32-ባይት ክፍለ ጊዜ መለያ (base64/base64url)።
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` - በ I18NI0000027X የተመለሱ ቶከኖች።
   - `CONNECT_CHAIN_ID` - በቁጥጥር መጨባበጥ ወቅት የሰንሰለት መለያ ታወቀ።
   - `CONNECT_ROLE` - ነባሪ ሚና በ UI (`app` ወይም `wallet`) ውስጥ አስቀድሞ ተመርጧል።
   - በእጅ ለመሞከር አማራጭ ረዳቶች፡ `CONNECT_PEER_PUB_B64`፣ `CONNECT_SHARED_KEY_B64`፣
     `CONNECT_APPROVE_ACCOUNT_ID`፣ `CONNECT_APPROVE_PRIVATE_KEY_B64`፣
     `CONNECT_APPROVE_SIGNATURE_B64`.

## Bootstrapping I18NT0000005X + የማስመሰል ደብተር

የመረጃ ቋቱ Torii መስቀለኛ መንገድን ከውስጥ ማስታወሻ ደብተር ጋር የሚጀምሩ የረዳት ስክሪፕቶችን ይልካል።
በማሳያ መለያዎች ተጭኗል፡-

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

ስክሪፕቱ ያወጣል፡-

- Torii መስቀለኛ መንገድ ወደ I18NI0000037X።
- የመመዝገቢያ መለኪያዎች (Prometheus ቅርጸት) ወደ `artifacts/metrics.prom`።
- የደንበኛ መዳረሻ ቶከኖች ወደ `artifacts/torii.jwt`።

`start.sh` `Ctrl+C` ን እስኪጫኑ ድረስ ማሳያውን ያቆያል። ዝግጁ-ግዛት ይጽፋል
ቅጽበተ-ፎቶ ወደ I18NI0000042X (ለሌሎች ቅርሶች የእውነት ምንጭ)
የ Prometheus መፋቅ እስኪሆን ድረስ ንቁውን I18NT0000008X stdout ሎግ ፣የምርጫ `/metrics` ይገለብጣል።
ይገኛል እና የተዋቀሩ መለያዎችን ወደ `torii.jwt` (የግል ቁልፎችን ጨምሮ) ያቀርባል
ውቅሩ ሲያቀርብላቸው)። ውጤቱን ለመሻር ስክሪፕቱ I18NI0000045X ይቀበላል
ማውጫ፣ `--telemetry-profile` ብጁ I18NT0000009X ውቅሮችን ለማዛመድ፣ እና
`--exit-after-ready` መስተጋብራዊ ላልሆኑ CI ስራዎች።

በ`SampleAccounts.json` ውስጥ ያለው እያንዳንዱ ግቤት የሚከተሉትን መስኮች ይደግፋል።

- `name` (ሕብረቁምፊ፣ አማራጭ) - እንደ መለያ ሜታዳታ `alias` ተከማችቷል።
- `public_key` (multihash string, ያስፈልጋል) - እንደ መለያ ፈራሚ ጥቅም ላይ ይውላል.
- `private_key` (አማራጭ) - በ`torii.jwt` ውስጥ የተካተተ ለደንበኛ ምስክርነት።
- `domain` (አማራጭ) - ከተተወ የንብረቱ ጎራ ነባሪዎች።
- `asset_id` (ሕብረቁምፊ፣ ያስፈልጋል) - ለመለያው ሚንት የንብረት ትርጉም።
- `initial_balance` (ሕብረቁምፊ፣ ያስፈልጋል) - በሂሳብ ውስጥ የገባ የቁጥር መጠን።

## የSwiftUI ማሳያን በማሄድ ላይ

1. በ`docs/norito_bridge_release.md` ላይ እንደተገለፀው የXCFrameworkን ይገንቡ እና ይጠቅልሉት
   ወደ ማሳያ ፕሮጀክት (ማጣቀሻዎች በፕሮጀክቱ ውስጥ `NoritoBridge.xcframework` ይጠብቃሉ
   ሥር)።
2. የ I18NI0000059X ፕሮጀክትን በ Xcode ይክፈቱ።
3. የI18NI0000060X እቅድን ይምረጡ እና የ iOS ሲሙሌተርን ወይም መሳሪያን ኢላማ ያድርጉ።
4. የI18NI0000061X ፋይል በእቅዱ የአካባቢ ተለዋዋጮች በኩል መጠቀሱን ያረጋግጡ።
   በ`/v1/connect/session` ወደ ውጭ የተላኩትን የI18NI0000062X እሴቶችን በብዛት ይሰብስቡ ስለዚህ UI ነው
   መተግበሪያው ሲጀምር አስቀድሞ ተሞልቷል።
5. የሃርድዌር ማጣደፍ ነባሪዎችን ያረጋግጡ፡ `App.swift` ጥሪዎች
   `DemoAccelerationConfig.load().apply()` ስለዚህ ማሳያው ሁለቱንም ያነሳል።
   `NORITO_ACCEL_CONFIG_PATH` አካባቢ መሻር ወይም ጥቅል
   `acceleration.{json,toml}`/`client.{json,toml}` ፋይል። እነዚህን ግብዓቶች ካስወገዱ/ያስተካክሏቸው
   ከመሮጥዎ በፊት የሲፒዩ ውድቀትን ማስገደድ ይፈልጋሉ።
6. አፕሊኬሽኑን ይገንቡ እና ያስጀምሩት። የመነሻ ስክሪን ለ Torii URL/ ማስመሰያ ካልሆነ ይጠይቃል
   አስቀድሞ በ `.env` በኩል ተዘጋጅቷል።
7. ለመለያ ዝመናዎች ለመመዝገብ ወይም ጥያቄዎችን ለማጽደቅ "አገናኝ" ክፍለ ጊዜን ያስጀምሩ።
8. የ IRH ማስተላለፍን አስገባ እና በስክሪኑ ላይ ያለውን የምዝግብ ማስታወሻ ከTorii ምዝግብ ማስታወሻዎች ጋር መርምር።

### የሃርድዌር ማጣደፍ መቀየሪያዎች (ብረት / NEON)

`DemoAccelerationConfig` ገንቢዎች የአካል ብቃት እንቅስቃሴ ማድረግ እንዲችሉ የ Rust node ውቅርን ያንጸባርቃል
የብረታ ብረት/NEON ዱካዎች ያለ ሃርድ-ኮድ ጣራዎች። ጫኚው የሚከተለውን ይፈልጋል
የሚጀመርባቸው ቦታዎች፡-

1. `NORITO_ACCEL_CONFIG_PATH` (በI18NI0000072X/የዕቅድ ክርክሮች የተገለፀ) - ፍጹም መንገድ ወይም
   `tilde`-የተዘረጋ ጠቋሚ ወደ I18NI0000074X JSON/TOML ፋይል።
2. `acceleration.{json,toml}` ወይም `client.{json,toml}` የተሰየሙ የተዋቀሩ ፋይሎች።
3. የትኛውም ምንጭ ከሌለ ነባሪ ቅንጅቶች (`AccelerationSettings()`) ይቀራሉ።

ምሳሌ `acceleration.toml` ቅንጫቢ፡

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

መስኮችን መልቀቅ I18NI0000079X የስራ ቦታ ነባሪዎች ይወርሳል። አሉታዊ ቁጥሮች ችላ ይባላሉ,
እና የጠፉ `[accel]` ክፍሎች ወደ ወሳኙ የሲፒዩ ባህሪ ይመለሳሉ። ሲሮጥ
ብረት የሌለበት አስመሳይ ደጋፊ ድልድዩ በፀጥታ ስኩላር መንገዱን ያቆያል
የማዋቀር ጥያቄዎች ብረት.

## የውህደት ሙከራዎች

- የውህደት ሙከራዎች በ `Tests/NoritoDemoTests` ውስጥ ይኖራሉ (ማክኦኤስ ሲ ሲ ሲጨምር መታከል አለበት)
  ይገኛል)።
- ሙከራዎች ከላይ ያሉትን ስክሪፕቶች በመጠቀም Torii ይሽከረከራሉ እና የዌብሶኬት ምዝገባዎችን ይለማመዱ ፣ ቶከን
  ሚዛኖች እና የዝውውር ፍሰቶች በስዊፍት ጥቅል በኩል።
- ከሙከራ ሩጫዎች የተገኙ ምዝግብ ማስታወሻዎች በ `artifacts/tests/<timestamp>/` ውስጥ ከሜትሪዎች ጋር ተከማችተዋል እና
  የናሙና ደብተር መጣያ.

## CI እኩልነት ማረጋገጫዎች

- ማሳያውን ወይም የጋራ መገልገያውን የሚነካ PR ከመላክዎ በፊት `make swift-ci` ን ያሂዱ። የ
  ኢላማ የቋሚ እኩልነት ፍተሻዎችን ይፈጽማል፣ የዳሽቦርድ ምግቦችን ያጸድቃል እና ያቀርባል
  በአካባቢው ማጠቃለያዎች. በCI ውስጥ ተመሳሳይ የስራ ፍሰት በBuildkite ሜታዳታ ላይ የተመሰረተ ነው።
  (`ci/xcframework-smoke:<lane>:device_tag`) ስለዚህ ዳሽቦርዶች ውጤቱን ለ
  ትክክለኛ ሲሙሌተር ወይም StrongBox ሌይን - ካስተካከሉ ሜታዳታ መኖሩን ያረጋግጡ
  የቧንቧ መስመር ወይም ወኪል መለያዎች.
- `make swift-ci` ሳይሳካ ሲቀር፣ በ`docs/source/swift_parity_triage.md` ውስጥ ያሉትን ደረጃዎች ይከተሉ።
  እና የትኛውን መስመር እንደሚያስፈልግ ለማወቅ የተሰራውን የI18NI0000087X ውጤት ይገምግሙ
  እንደገና መወለድ ወይም የአደጋ ክትትል.

## መላ መፈለግ

- ማሳያው ከTorii ጋር መገናኘት ካልቻለ የመስቀለኛ መንገዱን URL እና TLS ቅንብሮችን ያረጋግጡ።
- የJWT ማስመሰያ (ከተፈለገ) የሚሰራ እና ጊዜው ያለፈበት መሆኑን ያረጋግጡ።
- ለአገልጋይ-ጎን ስህተቶች `artifacts/torii.log` ያረጋግጡ።
- ለዌብሶኬት ጉዳዮች የደንበኛ ሎግ መስኮቱን ወይም የ Xcode ኮንሶል ውፅዓትን ይመርምሩ።