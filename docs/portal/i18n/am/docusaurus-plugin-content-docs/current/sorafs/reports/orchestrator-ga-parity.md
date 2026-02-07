---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS ኦርኬስትራ GA የፓሪቲ ሪፖርት

የመለኪያ መሐንዲሶች ያንን ማረጋገጥ እንዲችሉ ቆራጥ ባለብዙ-አመጣጣኝ እኩልነት አሁን በእያንዳንዱ ኤስዲኬ ክትትል ይደረግበታል።
የመጫኛ ባይት፣ የተቆራረጡ ደረሰኞች፣ የአቅራቢዎች ሪፖርቶች እና የውጤት ሰሌዳ ውጤቶች በአንድ ላይ ተሰልፈው ይቆያሉ።
አተገባበር. እያንዳንዱ ማሰሪያ ከስር ያለውን ቀኖናዊ ባለብዙ አቅራቢ ጥቅል ይበላል
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`፣የSF1 ዕቅድን፣ አቅራቢውን የሚያጠቃልለው
ሜታዳታ፣ ቴሌሜትሪ ቅጽበተ-ፎቶ እና ኦርኬስትራ አማራጮች።

## Rust Baseline

- ** ትዕዛዝ: *** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- ** ወሰን፡** የ`MultiPeerFixture` እቅድን በሂደት ላይ ባለው ኦርኬስትራ በኩል ሁለት ጊዜ ያካሂዳል፣ በማረጋገጥ
  የተገጣጠሙ የተጫኑ ባይት፣ የተቆራረጡ ደረሰኞች፣ የአቅራቢዎች ሪፖርቶች እና የውጤት ሰሌዳ ውጤቶች። መሳሪያ
  እንዲሁም ከፍተኛውን ተመጣጣኝ እና ውጤታማ የስራ-ስብስብ መጠን (`max_parallel × max_chunk_length`) ይከታተላል።
- ** የአፈፃፀም ጠባቂ: *** እያንዳንዱ ሩጫ በ CI ሃርድዌር ላይ በ 2 ሴ ውስጥ ማጠናቀቅ አለበት.
- ** የሚሰራ ጣሪያ:** በ SF1 መገለጫ መታጠቂያው `max_parallel = 3` ያስገድዳል ፣ ይህም ይሰጣል
  ≤196608ባይት መስኮት።

የምዝግብ ማስታወሻ ውፅዓት ናሙና

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## ጃቫ ስክሪፕት ኤስዲኬ ታጥቆ

- ** ትዕዛዝ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** ወሰን:** የክፍያ ጭነቶችን በማነፃፀር በ `iroha_js_host::sorafsMultiFetchLocal` በኩል አንድ አይነት ማጠናከሪያን እንደገና ያጫውታል።
  ደረሰኞች፣ የአቅራቢዎች ሪፖርቶች እና የውጤት ሰሌዳ ቅጽበተ-ፎቶዎች በተከታታይ ሩጫዎች።
- ** የአፈፃፀም ጠባቂ: *** እያንዳንዱ አፈፃፀም በ 2 ሰከንድ ውስጥ ማጠናቀቅ አለበት; መታጠቂያው የሚለካውን ያትማል
  ቆይታ እና የተጠበቀው-ባይት ጣሪያ (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

ምሳሌ ማጠቃለያ መስመር፡-

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ስዊፍት ኤስዲኬ ታጥቆ

- ** ትዕዛዝ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** ወሰን:** በ `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` ውስጥ የተገለጸውን ተመሳሳይነት ስብስብ ያካሂዳል ፣
  በ Norito ድልድይ (`sorafsLocalFetch`) በኩል የ SF1 መሣሪያን ሁለት ጊዜ እንደገና ማጫወት። ማሰሪያው ያረጋግጣል
  የመጫኛ ባይት፣ የተቆራረጡ ደረሰኞች፣ የአቅራቢዎች ሪፖርቶች እና የውጤት ሰሌዳ ግቤቶች ተመሳሳይ መወሰኛ በመጠቀም
  አቅራቢ ሜታዳታ እና ቴሌሜትሪ ቅጽበተ-ፎቶዎች እንደ Rust/JS ስብስቦች።
- ** የድልድይ ቦት ማንጠልጠያ:** ማጠፊያው `dist/NoritoBridge.xcframework.zip` በፍላጎት እና በጭነት ይከፍታል
  የ macOS ቁራጭ በ I18NI0000019X። xcframework ሲጎድል ወይም የSoraFS ማሰሪያዎች ሲጎድል፣
  ወደ I18NI0000020X ተመልሶ ይወድቃል እና ይቃወማል
  `target/release/libconnect_norito_bridge.dylib`፣ ስለዚህ በCI ውስጥ በእጅ ማዋቀር አያስፈልግም።
- ** የአፈፃፀም ጠባቂ: *** እያንዳንዱ አፈፃፀም በ CI ሃርድዌር ላይ በ 2 ሰ ውስጥ ማለቅ አለበት; መታጠቂያው ያትማል
  የሚለካው ቆይታ እና የተጠበቀው-ባይት ጣሪያ (`max_parallel = 3`፣ `peak_reserved_bytes ≤ 196 608`)።

ምሳሌ ማጠቃለያ መስመር፡-

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## የፓይዘን ማያያዣዎች ማሰሪያ

- ** ትዕዛዝ: *** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- ** ወሰን:** ከፍተኛ-ደረጃ `iroha_python.sorafs.multi_fetch_local` መጠቅለያውን እና የተተየበው ይለማመዳል
  የመረጃ ክፍሎች ስለዚህ ቀኖናዊው ቋሚ ሸማቾች በሚጠሩት ኤፒአይ በኩል ይፈስሳሉ። ፈተናው
  የአቅራቢውን ሜታዳታ ከ`providers.json` እንደገና ይገነባል፣ የቴሌሜትሪ ቅጽበተ-ፎቶውን ያስገባ እና ያረጋግጣል።
  የመጫኛ ባይት፣ የተቆራረጡ ደረሰኞች፣ የአቅራቢዎች ሪፖርቶች እና የውጤት ሰሌዳ ይዘት ልክ እንደ Rust/JS/Swift
  ስብስቦች.
- ** ቅድመ-ተጠያቂ:** `maturin develop --release` ን ያሂዱ (ወይም ጎማውን ይጫኑ) ስለዚህ I18NI0000028X ያጋልጣል
  ፒትስት ከመጥራት በፊት `sorafs_multi_fetch_local` ማሰር; ማሰሪያው በሚታሰርበት ጊዜ መታጠቂያው በራስ-ሰር ይዘለላል
  አይገኝም።
- ** የአፈጻጸም ጠባቂ:** ልክ እንደ Rust suite ተመሳሳይ ≤2s በጀት; pytest የተሰበሰበውን ባይት ቆጠራ ይመዘግባል
  እና የአቅራቢው ተሳትፎ ማጠቃለያ ለተለቀቀው አርቲፊክት።

የመልቀቂያ ጌቲንግ ማጠቃለያውን ከእያንዳንዱ ማሰሪያ (Rust, Python, JS, Swift) መያዝ አለበት ስለዚህ
በማህደር የተቀመጠ ሪፖርት ግንባታን ከማስተዋወቅዎ በፊት የክፍያ ደረሰኞችን እና መለኪያዎችን በአንድነት ሊለያይ ይችላል። ሩጡ
`ci/sdk_sorafs_orchestrator.sh` እያንዳንዱን ተመሳሳይ ስብስብ (Rust, Python bindings, JS, Swift) ለማስፈጸም
አንድ ማለፊያ; የሲአይ ቅርሶች ከዛ ረዳት የተወሰደውን የምዝግብ ማስታወሻ እና የተፈጠረውን ማያያዝ አለባቸው
`matrix.md` (ኤስዲኬ/ሁኔታ/የቆይታ ሠንጠረዥ) ወደ መልቀቂያ ትኬት ገምጋሚዎች እኩልነትን ኦዲት ማድረግ እንዲችሉ
ማትሪክስ ክፍሉን በአካባቢው እንደገና ሳያስኬድ።