---
lang: am
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 መገለጫዎች

Kagami ለ Iroha 3 አውታረ መረቦች ቅድመ-ቅምጦችን መርከቦች ኦፕሬተሮች ቆራጥነትን ማተም ይችላሉ
ዘፍጥረት የሚገለጠው በአውታረ መረብ ቁልፎች ላይ ሳይጣበቁ ነው።

- መገለጫዎች፡ `iroha3-dev` (ሰንሰለት `iroha3-dev.local`፣ ሰብሳቢዎች k=1 r=1፣ NPoS ሲመረጥ ከሰንሰለት መታወቂያ የተገኘ የVRF ዘር)፣ `iroha3-testus` (ሰንሰለት `iroha3-testus` (ሰንሰለት `iroha3-testus`፣00000011X፣00000011X፣0000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣3000001X፣30000001X፣3000001X፣3000001X፣1 r=02002 NPoS ሲመረጥ), `iroha3-nexus` (ሰንሰለት `iroha3-nexus`, ሰብሳቢዎች k=5 r=3, NPoS ሲመረጥ `--vrf-seed-hex` ያስፈልገዋል).
- የጋራ ስምምነት: የሶራ ፕሮፋይል ኔትወርኮች (Nexus + dataspaces) NPoS ያስፈልጋቸዋል እና የተደረደሩ መቁረጫዎችን አይፍቀዱ; የተፈቀደ Iroha3 ማሰማራቶች ያለሶራ መገለጫ መሮጥ አለባቸው።
- ትውልድ: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. ለ Nexus `--consensus-mode npos` ይጠቀሙ; `--vrf-seed-hex` የሚሰራው ለNPoS ብቻ ነው (ለ testus/nexus ያስፈልጋል)። Kagami ፒን DA/RBC በ Iroha3 መስመር ላይ እና ማጠቃለያ (ሰንሰለት፣ ሰብሳቢዎች፣ ዳ/አርቢሲ፣ ቪአርኤፍ ዘር፣ ​​የጣት አሻራ) ያወጣል።
- ማረጋገጫ፡ `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` የመገለጫ የሚጠበቁትን ይደግማል (የሰንሰለት መታወቂያ፣ DA/RBC፣ ሰብሳቢዎች፣ የፖፕ ሽፋን፣ የጋራ ስምምነት አሻራ)። አቅርቦት `--vrf-seed-hex` ብቻ NPoS አንጸባራቂ ለ testus/nexus ሲያረጋግጥ።
- የናሙና ቅርቅቦች፡- ቀድሞ የተፈጠሩ ጥቅሎች በ`defaults/kagami/iroha3-{dev,testus,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README) ስር ይኖራሉ. በ`cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` ያድሱ።
- Mochi: `mochi`/`mochi-genesis` `--genesis-profile <profile>` እና `--vrf-seed-hex <hex>` (NPoS ብቻ) ይቀበሉ፣ ወደ Kagami አስተላልፏቸው፣ እና ተመሳሳይ Kagami ሲደርስ ያትሙ።

ጥቅሎቹ BLS ፖፕዎችን ከቶፖሎጂ ግቤቶች ጋር አካተዋል ስለዚህም `kagami verify` እንዲሳካ
ከሳጥኑ ውስጥ; ለአካባቢው እንደ አስፈላጊነቱ የታመኑ አቻዎችን/ወደቦችን በውቅሮች ውስጥ ያስተካክሉ
ጭስ ይሮጣል.