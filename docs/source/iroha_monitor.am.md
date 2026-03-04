---
lang: am
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2025-12-29T18:16:35.968850+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha ማሳያ

የታደሰው Iroha ሞኒተሪ ቀላል ክብደት ያለውን ተርሚናል ዩአይ ከአኒሜሽን ጋር ያጣምራል።
ፌስቲቫል ASCII ጥበብ እና ባህላዊ የኢትንራኩ ጭብጥ።  በሁለት ላይ ያተኩራል።
ቀላል የስራ ሂደቶች;

- ** ስፓውን-ላይት ሁነታ *** - እኩዮችን የሚመስሉ የፍጥነት ደረጃ/የመለኪያ ግጥሞችን ይጀምሩ።
- ** የማያያዝ ሁነታ *** - ማሳያውን አሁን ባለው Torii HTTP የመጨረሻ ነጥቦች ላይ ያመልክቱ።

UI በእያንዳንዱ ማደስ ላይ ሶስት ክልሎችን ይሰጣል፡-

1. **Torii የሰማይ መስመር ራስጌ** - የታነመ የቶሪ በር፣ ፉጂ ተራራ፣ ኮይ ሞገዶች እና ኮከብ
   ከማደስ ቃና ጋር በማመሳሰል የሚሸብለል መስክ።
2. ** ማጠቃለያ ስትሪፕ *** - የተዋሃዱ ብሎኮች / ግብይቶች / ጋዝ እና የማደስ ጊዜ።
3. **የአቻ ጠረጴዛ እና የበዓል ሹክሹክታ** - የእኩያ ረድፎች በግራ በኩል፣ የሚሽከረከር ክስተት
   ማስጠንቀቂያዎችን የሚይዘው በቀኝ በኩል ይግቡ (የጊዜ ማብቂያዎች፣ ከመጠን በላይ የሚጫኑ ጭነቶች፣ ወዘተ)።
4. ** አማራጭ የጋዝ አዝማሚያ *** - `--show-gas-trend` ብልጭታ እንዲሰፍር ያንቁ
   በሁሉም እኩዮች ላይ አጠቃላይ የጋዝ አጠቃቀምን ማጠቃለል።

በዚህ ማሻሻያ ውስጥ አዲስ፡-

- የታነመ የጃፓን አይነት ASCII ትዕይንት ከ koi፣ torii እና laterns ጋር።
- ቀለል ያለ የትዕዛዝ ወለል (`--spawn-lite` ፣ `--attach` ፣ `--interval`)።
- የጋጋኩ ጭብጥ (ውጫዊ MIDI) አማራጭ የድምጽ መልሶ ማጫወት ያለው የመግቢያ ሰንደቅ
  የመድረክ/የድምጽ ቁልል ሲደግፈው አጫዋች ወይም አብሮ የተሰራው ለስላሳ ሲንት)።
- `--no-theme` / `--no-audio` ባንዲራዎች ለ CI ወይም ፈጣን የጭስ ማውጫዎች።
- የቅርብ ጊዜውን ማስጠንቀቂያ፣ የቁርጥ ቀን ወይም የሰአት ጊዜን የሚያሳይ የፔር “ስሜት” አምድ።

## ፈጣን ጅምር

ማሳያውን ይገንቡ እና ግትር ከሆኑ እኩዮቻቸው ጋር ያካሂዱት፡

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

ከነባሩ Torii የመጨረሻ ነጥቦች ጋር ያያይዙ፡

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

ለCI-ተስማሚ ጥሪ (የመግቢያ አኒሜሽን እና ኦዲዮ ዝለል)፦

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI ባንዲራዎች

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## ጭብጥ መግቢያ

በነባሪ፣ ጅምር አጭር የASCII እነማ ሲጫወት የኢተራኩ ውጤት ነው።
ይጀምራል።  የድምጽ ምርጫ ቅደም ተከተል፡-

1. `--midi-player` ከተሰጠ፣ ማሳያውን MIDI ይፍጠሩ (ወይም `--midi-file` ይጠቀሙ)
   እና ትዕዛዙን ያበቅላል.
2. ያለበለዚያ፣ በ macOS/Windows (ወይም ሊኑክስ ከ `--features iroha_monitor/linux-builtin-synth` ጋር)
   ውጤቱን አብሮ በተሰራው ጋጋኩ ለስላሳ ሲንት (ውጫዊ ኦዲዮ የለም)
   አስፈላጊ ንብረቶች).
3. ኦዲዮ ከተሰናከለ ወይም ማስጀመር ካልተሳካ መግቢያው አሁንም ያትማል
   አኒሜሽን እና ወዲያውኑ ወደ TUI ይገባል.

በሲፒኤል የተጎላበተ ሲንት በራስ-ሰር በማክኦኤስ እና በዊንዶውስ ላይ ያነቃል። በሊኑክስ ላይ ነው።
የስራ ቦታ በሚገነቡበት ጊዜ የ ALSA/Pulse ራስጌዎችን እንዳያመልጡ መርጠው ይግቡ። አንቃው።
ስርዓትዎ ሀ የሚያቀርብ ከሆነ በ`--features iroha_monitor/linux-builtin-synth`
የሚሰራ የድምጽ ቁልል.

በCI ወይም ጭንቅላት በሌላቸው ዛጎሎች ውስጥ ሲሰሩ `--no-theme` ወይም `--no-audio` ይጠቀሙ።

ለስላሳው ሲንት አሁን በ*MIDI synth design in ውስጥ የተያዘውን ዝግጅት ይከተላል
Rust.pdf*፡ hichiriki እና ryūteki ሾው እያለ ሄትሮፎኒክ ዜማ ይጋራሉ።
በሰነዱ ውስጥ የተገለጹትን የaitake pads ያቀርባል.  በጊዜ የተያዘው የማስታወሻ ውሂብ ይኖራል
በ `etenraku.rs`; ሁለቱንም የ CPAL መልሶ ጥሪ እና የመነጨውን ማሳያ MIDI ኃይል ይሰጣል።
የድምጽ ውፅዓት በማይገኝበት ጊዜ ተቆጣጣሪው መልሶ ማጫወትን ይዘላል ነገር ግን አሁንም ያቀርባል
የ ASCII እነማ.

## የዩአይ አጠቃላይ እይታ- ** የራስጌ ጥበብ *** - እያንዳንዱ ፍሬም በ `AsciiAnimator` የተፈጠረ; ኮይ ፣ ቶሪ መብራቶች ፣
  እና ሞገዶች የማያቋርጥ እንቅስቃሴ ለመስጠት ይንቀሳቀሳሉ.
- ** ማጠቃለያ** - የመስመር ላይ እኩዮችን ያሳያል፣ የተዘገበ የአቻ ብዛት፣ አጠቃላይ ድምር፣
  ባዶ ያልሆኑ የማገጃ ድምር፣ tx ማጽደቆች/አለመቀበላቸው፣ የጋዝ አጠቃቀም እና የማደስ መጠን።
- ** የአቻ ሠንጠረዥ *** - ተለዋጭ ስም / የመጨረሻ ነጥብ ፣ ብሎኮች ፣ ግብይቶች ፣ የወረፋ መጠን ፣
  የጋዝ አጠቃቀም፣ መዘግየት እና የ"ስሜት" ፍንጭ (ማስጠንቀቂያዎች፣ የቁርጠኝነት ጊዜ፣ የስራ ሰዓት)።
- ** የበዓሉ ሹክሹክታ *** - የሚሽከረከር የማስጠንቀቂያ መዝገብ (የግንኙነት ስህተቶች ፣ የክፍያ ጭነት
  ጥሰቶችን ይገድቡ ፣ ዘገምተኛ የመጨረሻ ነጥቦች)።  መልእክቶች ተቀልብሰዋል (የቅርብ ጊዜ ከላይ)።

የቁልፍ ሰሌዳ አቋራጮች;

- `n` / ቀኝ / ታች - ትኩረትን ወደ ቀጣዩ እኩያ ያንቀሳቅሱ።
- `p` / ግራ / ወደላይ - ትኩረትን ወደ ቀድሞው እኩያ ያንቀሳቅሱ።
- `q` / Esc / Ctrl-C - ይውጡ እና ተርሚናሉን ወደነበረበት ይመልሱ።

ተቆጣጣሪው ተሻጋሪ ቃል + ራታቱኢን በተለዋጭ ማያ ገጽ ቋት ይጠቀማል። በመውጣት ላይ
ጠቋሚውን ወደነበረበት ይመልሳል እና ማያ ገጹን ያጸዳል።

## የጭስ ሙከራዎች

ሣጥኑ ሁለቱንም ሁነታዎች እና የኤችቲቲፒ ገደቦችን የሚለማመዱ የውህደት ሙከራዎችን ይልካል።

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

የማሳያ ሙከራዎችን ብቻ ያሂዱ፡-

```bash
cargo test -p iroha_monitor -- --nocapture
```

የስራ ቦታው የበለጠ ከባድ የውህደት ሙከራዎች (`cargo test --workspace`) አለው። መሮጥ
የመቆጣጠሪያው ሙከራዎች በተናጥል አሁንም ሲያደርጉ ፈጣን ማረጋገጫ ጠቃሚ ነው።
ሙሉውን ስብስብ አያስፈልግም.

## ቅጽበታዊ ገጽ እይታዎችን በማዘመን ላይ

የሰነዶች ማሳያው አሁን የሚያተኩረው በቶሪ ሰማይ መስመር እና በእኩያ ጠረጴዛ ላይ ነው።  ለማደስ
ንብረቶች፣ አሂድ

```bash
make monitor-screenshots
```

ይህ `scripts/iroha_monitor_demo.sh` ይጠቀልላል (spawn-lite ሁነታ፣ ቋሚ ዘር/መመልከቻ፣
ምንም መግቢያ/ድምጽ፣ የንጋት ቤተ-ስዕል፣ የጥበብ-ፍጥነት 1፣ ጭንቅላት የሌለው ካፕ 24) እና ይጽፋል
SVG/ANSI ፍሬሞች እና `manifest.json` እና `checksums.json` ወደ
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
ሁለቱንም የሲአይ ጥበቃዎች (`ci/check_iroha_monitor_assets.sh` እና
`ci/check_iroha_monitor_screenshots.sh`) ስለዚህ የጄነሬተር hashes፣ የገለጻ መስኮች፣
እና ቼኮች በማመሳሰል ውስጥ ይቆያሉ; የቅጽበታዊ ገጽ እይታው ፍተሻ እንዲሁ ይላካል
`python3 scripts/check_iroha_monitor_screenshots.py`. `--no-fallback` ወደ
ወደ ኋላ ከመውደቅ ይልቅ ቀረጻው እንዲወድቅ ከፈለጉ የማሳያ ስክሪፕቱ
የመቆጣጠሪያው ውፅዓት ባዶ በሚሆንበት ጊዜ የተጋገሩ ክፈፎች; መውደቅ ጥሬው ጥቅም ላይ ሲውል
የ`.ans` ፋይሎች በተጋገሩ ክፈፎች እንደገና ተጽፈዋል ስለዚህም መግለጫው/ቼክሱም ይቆያሉ
የሚወስን.

## ቆራጥ ቅጽበታዊ ገጽ እይታዎች

የተላኩት ቅጽበተ-ፎቶዎች በ`docs/source/images/iroha_monitor_demo/` ውስጥ ይኖራሉ፡

![አጠቃላይ እይታ](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![ቧንቧን ይቆጣጠሩ](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

በቋሚ መመልከቻ/ዘር ያባዟቸው፡-

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

የቀረጻው አጋዥ `LANG`/`LC_ALL`/`TERM`ን፣ ወደፊት ያስተላልፋል
`IROHA_MONITOR_DEMO_SEED`፣ ድምጸ-ከል ያደርጋል፣ እና የጥበብ ጭብጥ/ፍጥነቱን ይሰካል
ክፈፎች በመድረኮች ላይ በተመሳሳይ መልኩ ይሰጣሉ። እሱ `manifest.json` (ጄነሬተር) ይጽፋል
hashes + መጠኖች) እና `checksums.json` (SHA-256 ዲጀስት) ስር
`docs/source/images/iroha_monitor_demo/`; CI ይሰራል
`ci/check_iroha_monitor_assets.sh` እና `ci/check_iroha_monitor_screenshots.sh`
ንብረቶቹ ከተመዘገቡት አንጸባራቂዎች ሲንሳፈፉ ውድቀት.

## መላ መፈለግ- ** ምንም የድምጽ ውፅዓት የለም *** - ማሳያው ወደ ድምጸ-ከል መልሶ ማጫወት ተመልሶ ይቀጥላል።
- **ጭንቅላት የሌለው መመለስ ቀደም ብሎ ይወጣል *** - ማሳያው ያለ ጭንቅላት ወደ ጥንድ ይሮጣል
  ደርዘን ፍሬሞች (በነባሪው ክፍተት 12 ሰከንድ ያህል) መቀየር በማይችልበት ጊዜ
  ተርሚናል ወደ ጥሬ ሁነታ; እየሄደ እንዲቆይ `--headless-max-frames 0` ማለፍ
  ላልተወሰነ ጊዜ።
- **ከመጠን በላይ የሆኑ የሁኔታ ክፍያዎች** - የእኩዮች ስሜት አምድ እና የበዓሉ ምዝግብ ማስታወሻ
  ከተዋቀረው ገደብ (`128 KiB`) ጋር `body exceeds …` አሳይ።
- ** ዘገምተኛ እኩዮች *** - የክስተቱ ምዝግብ ማስታወሻ ጊዜ ማብቂያ ማስጠንቀቂያዎችን ይመዘግባል; ያንን እኩያ ላይ ማተኮር
  ረድፉን አጉልተው.

በበዓሉ ላይ ይደሰቱ!  ለተጨማሪ ASCII ጭብጦች ወይም
የመለኪያ ፓነሎች እንኳን ደህና መጡ—ስብስቦች ተመሳሳይ እንዲሆኑ ቆራጥ ያድርጓቸው
ፍሬም-በ-ፍሬም ምንም ይሁን ምን ተርሚናል.