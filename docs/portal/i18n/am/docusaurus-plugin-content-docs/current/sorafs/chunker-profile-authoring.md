---
id: chunker-profile-authoring
lang: am
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS Chunker መገለጫ ደራሲ መመሪያ

ይህ መመሪያ ለSoraFS አዲስ chunker መገለጫዎችን እንዴት እንደሚያቀርቡ እና እንደሚታተም ያብራራል።
እሱ የሕንፃውን RFC (SF-1) እና የመመዝገቢያ ማጣቀሻ (SF-2a) ያሟላል።
በተጨባጭ የአጻጻፍ መስፈርቶች፣ የማረጋገጫ ደረጃዎች እና የፕሮፖዛል አብነቶች።
ለቀኖናዊ ምሳሌ፣ ተመልከት
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
እና ተጓዳኝ በደረቅ አሂድ ግባ
`docs/source/sorafs/reports/sf1_determinism.md`.

## አጠቃላይ እይታ

ወደ መዝገቡ የሚገባ እያንዳንዱ መገለጫ፡

- የሚወስኑ የሲዲሲ መለኪያዎችን እና የመልቲሃሽ ቅንብሮችን በመላ ተመሳሳይ ያስተዋውቁ
  አርክቴክቸር;
- እንደገና ሊጫወቱ የሚችሉ ዕቃዎችን (Rust/Go/TS JSON + fuzz corpora + PoR ምስክሮችን) መላክ
  የታችኛው ኤስዲኬዎች ያለ ምንም መሳሪያ መሳሪያ ማረጋገጥ ይችላሉ፤
- ለአስተዳደር ዝግጁ የሆነ ሜታዳታ (ስም ቦታ፣ ስም፣ ሴቨር) እና ፍልሰትን ይጨምራል
- ከምክር ቤቱ ግምገማ በፊት የልዩነት ልዩነትን ማለፍ።

እነዚያን ደንቦች የሚያሟላ ፕሮፖዛል ለማዘጋጀት ከዚህ በታች ያለውን የማረጋገጫ ዝርዝር ይከተሉ።

## የመመዝገቢያ ቻርተር ቅጽበታዊ ገጽ እይታ

ፕሮፖዛል ከማዘጋጀትዎ በፊት፣ ከተተገበረው የመመዝገቢያ ቻርተር ጋር የሚስማማ መሆኑን ያረጋግጡ
በ`sorafs_manifest::chunker_registry::ensure_charter_compliance()`፡

- የመገለጫ መታወቂያዎች ያለ ክፍተቶች በብቸኝነት የሚጨምሩ አዎንታዊ ኢንቲጀር ናቸው።
- ቀኖናዊው እጀታ (`namespace.name@semver`) በተለዋጭ ስም ዝርዝር ውስጥ መታየት አለበት
  እና ** የመጀመሪያው ግቤት መሆን አለበት.
- ማንኛውም ተለዋጭ ስም ከሌላ ቀኖናዊ እጀታ ጋር መጋጨት ወይም ከአንድ ጊዜ በላይ ሊታይ አይችልም።
- ተለዋጭ ስሞች ባዶ ያልሆኑ እና በነጭ ቦታ የተቆረጡ መሆን አለባቸው።

ምቹ የ CLI ረዳቶች፡-

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

እነዚህ ትዕዛዞች ከመዝገቡ ቻርተር ጋር የተጣጣሙ ሀሳቦችን ያቆያሉ እና ያቀርባል
በአስተዳደር ውይይቶች ውስጥ ቀኖናዊ ሜታዳታ ያስፈልጋል።

## የሚፈለግ ዲበ ውሂብ

| መስክ | መግለጫ | ምሳሌ (`sorafs.sf1@1.0.0`) |
|-------|-------------|--------------|
| `namespace` | ለተዛማጅ መገለጫዎች አመክንዮአዊ ስብስብ። | `sorafs` |
| `name` | በሰው ሊነበብ የሚችል መለያ። | `sf1` |
| `semver` | ለመለኪያ ስብስብ የትርጓሜ ስሪት ሕብረቁምፊ። | `1.0.0` |
| `profile_id` | ሞኖቶኒክ ቁጥራዊ መለያ መገለጫው አንዴ ከተመደበ። የሚቀጥለውን መታወቂያ ያስይዙ ነገር ግን ያሉትን ቁጥሮች እንደገና አይጠቀሙ። | `1` |
| `profile_aliases` | በድርድር ወቅት ለደንበኞች የተጋለጡ የአማራጭ ተጨማሪ መያዣዎች. ሁልጊዜ ቀኖናዊውን እጀታ እንደ መጀመሪያው ግቤት ያካትቱ. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | በባይት ውስጥ ያለው ዝቅተኛ ቁራጭ ርዝመት። | `65536` |
| `profile.target_size` | የዒላማ ቁራጭ ርዝመት በባይት። | `262144` |
| `profile.max_size` | በባይት ውስጥ የሚፈቀደው ከፍተኛ ቁራጭ ርዝመት። | `524288` |
| `profile.break_mask` | በሮሊንግ ሃሽ (ሄክስ) ጥቅም ላይ የሚውል አስማሚ ጭንብል። | `0x0000ffff` |
| `profile.polynomial` | Gear ፖሊኖሚል ቋሚ (ሄክስ)። | `0x3da3358b4dc173` |
| `gear_seed` | የ 64KiB የማርሽ ጠረጴዛን ለማግኘት የሚያገለግል ዘር። | `sorafs-v1-gear` |
| `chunk_multihash.code` | የመልቲሃሽ ኮድ ለእያንዳንዱ ቸንክ መፍጨት። | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | የቀኖና ዕቃዎች ስብስብ መፍጨት። | `13fa...c482` |
| `fixtures_root` | የታደሱ ዕቃዎችን የያዘ አንጻራዊ ማውጫ። | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | ዘር ለሚወስነው የPoR ናሙና (`splitmix64`)። | `0xfeedbeefcafebabe` (ምሳሌ) |

ዲበ ውሂቡ በሁለቱም በፕሮፖዛል ሰነድ ውስጥ እና በተፈጠረው ውስጥ መታየት አለበት።
መጫዎቻዎች ስለዚህ መዝገቡ፣ CLI Tooling እና የአስተዳደር አውቶሜሽን ማረጋገጥ እንዲችሉ
በእጅ ያለ ማመሳከሪያ ዋጋዎች. በሚጠራጠሩበት ጊዜ, ቸንክ-መደብሩን ያሂዱ እና
የተሰላውን ዲበ ዳታ ወደ ግምገማ ለመልቀቅ ከ`--json-out=-` ጋር አንጸባራቂ CLIs
ማስታወሻዎች.

### CLI እና Registry Touchpoints

- `sorafs_manifest_chunk_store --profile=<handle>` - ቁራጭ ሜታዳታን እንደገና ያሂዱ ፣
  አንጸባራቂ መፍጨት፣ PoR ከታቀዱት መለኪያዎች ጋር ይፈትሻል።
- `sorafs_manifest_chunk_store --json-out=-` - የ chunk-store ሪፖርትን ወደ ዥረት ያሰራጩ
  stdout ለ አውቶሜትድ ንጽጽሮች.
- `sorafs_manifest_stub --chunker-profile=<handle>` - መግለጫዎችን እና CARን ያረጋግጡ
  ዕቅዶች ቀኖናዊውን እጀታ እና ተለዋጭ ስሞችን አካተዋል።
- `sorafs_manifest_stub --plan=-` - የቀደመውን I18NI0000048X መልሰው ይመግቡ
  ከለውጥ በኋላ ማካካሻዎችን/መፍጨትን ለማረጋገጥ።

በፕሮፖዛሉ ውስጥ የትዕዛዙን ውፅዓት (መፈጨት፣ የPoR roots፣ manifest hashes) ይመዝግቡ
ስለዚህ ገምጋሚዎች በቃላት ሊባዙዋቸው ይችላሉ።

## ቆራጥነት እና ማረጋገጫ ዝርዝር

1. ** መጋጠሚያዎችን እንደገና ማደስ ***
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. ** የፓርቲ ስብስብን ያሂዱ ** - `cargo test -p sorafs_chunker` እና
   የቋንቋ ልዩነት መታጠቂያ (`crates/sorafs_chunker/tests/vectors.rs`) መሆን አለበት።
   አረንጓዴ ከአዳዲስ እቃዎች ጋር.
3. ** fuzz/back-pressure corpora ን እንደገና አጫውት *** - `cargo fuzz list` ን ያስፈጽሙ እና
   የዥረት ማሰሪያ (`fuzz/sorafs_chunker`) ከታደሱ ንብረቶች ጋር።
4. **የማስረጃ-ማስረጃ ምስክሮችን አረጋግጥ *** - አሂድ
   `sorafs_manifest_chunk_store --por-sample=<n>` የታቀደውን መገለጫ በመጠቀም እና
   ሥሮቹ ከቋሚ አንጸባራቂው ጋር እንደሚዛመዱ ያረጋግጡ።
5. ** CI ደረቅ ሩጫ *** - በአካባቢው `ci/check_sorafs_fixtures.sh` ይደውሉ; ስክሪፕቱ
   በአዲሶቹ መጫዎቻዎች እና አሁን ባለው `manifest_signatures.json` ስኬታማ መሆን አለበት።
6. ** የማቋረጥ ጊዜ ማረጋገጫ ** - የ Go/TS ማሰሪያዎች የታደሰውን እንደሚበሉ ያረጋግጡ
   JSON እና ተመሳሳይ ቁርጥራጭ ድንበሮችን እና መፍጨትን ያስወጣሉ።

ትዕዛዞቹን እና የውጤት መግለጫዎችን በፕሮፖዛል ውስጥ ይመዝግቡ ስለዚህ የ Tooling WG
ያለ ግምት እንደገና ማስኬድ ይችላል።

### አንጸባራቂ / PoR ማረጋገጫ

የቤት ዕቃዎችን እንደገና ካዳበሩ በኋላ፣ CARን ለማረጋገጥ ሙሉውን አንጸባራቂ ቧንቧ ያስኪዱ
ሜታዳታ እና የPoR ማረጋገጫዎች ወጥነት አላቸው፡

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

የግቤት ፋይሉን በእርስዎ ቋሚዎች በሚጠቀሙት ማንኛውም ተወካይ አካል ይተኩ
(ለምሳሌ፣ 1ጂቢ መወሰኛ ዥረት) እና የተገኘውን የምግብ መፍጨት ከ
ፕሮፖዛል.

## የፕሮፖዛል አብነት

የውሳኔ ሃሳቦች የቀረቡት እንደ I18NI0000056X Norito መዝገቦች ተረጋግጠዋል
`docs/source/sorafs/proposals/`. ከታች ያለው የJSON አብነት የሚጠበቀውን ያሳያል
ቅርፅ (እሴቶቻችሁን እንደአስፈላጊነቱ ይተኩ)


የሚይዘው ተዛማጅ የማርክ ዳውን ሪፖርት (`determinism_report`) ያቅርቡ
የትዕዛዝ ውፅዓት፣ ቁርጥራጭ ማጭበርበሮች እና ማንኛውም በማረጋገጫ ወቅት ያጋጠሙ ልዩነቶች።

## የአስተዳደር የስራ ሂደት

1. ** PR ከፕሮፖዛል + ዕቃዎች ጋር ያቅርቡ።** የተፈጠሩ ንብረቶችን ያካትቱ፣
   Norito ፕሮፖዛል፣ እና ወደ `chunker_registry_data.rs` ዝማኔዎች።
2. **የመሳሪያዎች WG ግምገማ።** ገምጋሚዎች የማረጋገጫ ዝርዝሩን እንደገና ያሂዱ እና ያረጋግጡ
   ሀሳቡ ከመመዝገቢያ ደንቦች ጋር ይጣጣማል (መታወቂያ እንደገና ጥቅም ላይ አይውልም ፣ መወሰን ረክቷል)።
3. **የካውንስል ፖስታ** ከፀደቀ በኋላ የምክር ቤቱ አባላት የፕሮፖዛል መፍጫውን ይፈርማሉ
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) እና የእነሱን ያክሉ
   ከመሳሪያዎቹ ጎን ለጎን የተከማቸ የመገለጫ ፖስታ ላይ ፊርማዎች.
4. **የመዝገብ ቤት ህትመት።** ውህደት መዝገቡን፣ ሰነዶችን እና የቤት እቃዎችን ያበላሻል። የ
   አስተዳዳሪው እስኪያወጅ ድረስ ነባሪ CLI በቀድሞው መገለጫ ላይ ይቆያል
   ፍልሰት ዝግጁ ነው።
5. **የማቋረጡ ክትትል።** ከስደት መስኮቱ በኋላ መዝገቡን ያዘምኑ
   መጽሐፍ መዝገብ

## የደራሲ ምክሮች

- የጠርዝ መያዣን የመቁረጥ ባህሪን ለመቀነስ የሁለት ድንበሮችን እንኳን ኃይልን ይምረጡ።
- አንጸባራቂ እና መግቢያ መንገዱን ሳያስተባብሩ የመልቲሃሽ ኮድን ከመቀየር ይቆጠቡ
- የማርሽ ሰንጠረዥ ዘሮች በሰው ሊነበቡ የሚችሉ ነገር ግን ኦዲትን ለማቃለል በዓለም አቀፍ ደረጃ ልዩ ይሁኑ
  ዱካዎች.
- ማናቸውንም የቤንችማርኪንግ ቅርሶች (ለምሳሌ፣ የውጤት ንፅፅር) ስር ያከማቹ
  ለወደፊቱ ማጣቀሻ `docs/source/sorafs/reports/`.

በታቀደው ጊዜ ለሚጠበቀው የሥራ ክንውን የፍልሰት ደብተርን ተመልከት
(`docs/source/sorafs/migration_ledger.md`)። ለአሂድ ጊዜ መስማማት ደንቦችን ይመልከቱ
`docs/source/sorafs/chunker_conformance.md`.