---
lang: am
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2025-12-29T18:16:35.986892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI ጥቅል መሣሪያ

MOCHI ቀላል ክብደት ባለው የማሸጊያ የስራ ፍሰት ስለሚልክ ገንቢዎች ሀ
ተንቀሳቃሽ የዴስክቶፕ ቅርቅብ ያለ ሽቦ የ CI ስክሪፕቶች። `xtask`
ንዑስ ትእዛዝ ማጠናቀርን፣ አቀማመጥን፣ hashingን እና (በአማራጭ) ማህደርን ይቆጣጠራል
በአንድ ምት ውስጥ መፍጠር.

## ጥቅል በማመንጨት ላይ

```bash
cargo xtask mochi-bundle
```

በነባሪ ትዕዛዙ የመልቀቂያ ሁለትዮሾችን ይገነባል ፣ ጥቅሉን ከስር ይሰበስባል
`target/mochi-bundle/`፣ እና `mochi-<os>-<arch>-release.tar.gz` ማህደር ያወጣል
ከመወሰኛ `manifest.json` ጋር። አንጸባራቂው እያንዳንዱን ፋይል ይዘረዝራል።
መጠኑ እና SHA-256 hash ስለዚህ CI ቧንቧ መስመሮች ማረጋገጫን እንደገና ማካሄድ ወይም ማተም ይችላሉ።
ማረጋገጫዎች. ረዳቱ ሁለቱንም `mochi` የዴስክቶፕ ሼል እና የ
የስራ ቦታ `kagami` ሁለትዮሽ ይገኛሉ ስለዚህ የዘር ማመንጨት ከ
ሳጥን.

### ባንዲራዎች

| ባንዲራ | መግለጫ |
---------------------------------------------------------------------------------------|
| `--out <dir>` | የውጤት ማውጫውን ይሽሩ (የ`target/mochi-bundle` ነባሪዎች)።         |
| `--profile <name>` | በአንድ የተወሰነ የካርጎ መገለጫ ይገንቡ (ለምሳሌ፡ `debug` ለፈተናዎች)።              |
| `--no-archive` | የተዘጋጀውን አቃፊ ብቻ በመተው የ`.tar.gz` ማህደርን ይዝለሉ።               |
| `--kagami <path>` | `iroha_kagami` ከመገንባት ይልቅ ግልጽ የሆነ `kagami` ሁለትዮሽ ይጠቀሙ።         |
| `--matrix <path>` | የጥቅል ዲበ ውሂብን ለCI provenance መከታተያ ወደ JSON ማትሪክስ ያክሉ።         |
| `--smoke` | `mochi --help`ን ከጥቅል ጥቅል እንደ መሰረታዊ የማስፈጸሚያ በር ያሂዱ።      |
| `--stage <dir>` | የተጠናቀቀውን ጥቅል (እና በማህደር ያስቀምጡ፣ ሲገኝ) ወደ ማዘጋጃ ማህደር ይቅዱ። |

`--stage` እያንዳንዱ የግንባታ ወኪል ለሚሰቅልበት ለ CI ቧንቧዎች የታሰበ ነው።
ቅርሶች ወደ የጋራ ቦታ። ረዳቱ የጥቅል ማውጫውን እና እንደገና ይፈጥራል
ስራዎችን ማተም እንዲችሉ የተፈጠረውን ማህደር ወደ የዝግጅት ማውጫው ይገለበጣል
ያለ ሼል ስክሪፕት በመድረክ ላይ የተወሰኑ ውጤቶችን ሰብስብ።

በጥቅሉ ውስጥ ያለው አቀማመጥ ሆን ተብሎ ቀላል ነው፡-

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### የአሂድ ጊዜ ይሽራል።

የታሸገው `mochi` ፈጻሚው የትእዛዝ መስመር መሻሮችን በብዛት ይቀበላል
የጋራ ተቆጣጣሪ ቅንብሮች. ከማርትዕ ይልቅ እነዚህን ባንዲራዎች ተጠቀም
ሲሞክር `config/local.toml`፡

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

ማንኛውም የ CLI እሴት ከ `config/local.toml` ግቤቶች እና አከባቢዎች ቅድሚያ ይሰጣል
ተለዋዋጮች.

## ቅጽበታዊ አውቶማቲክ

`manifest.json` የትውልዱን የጊዜ ማህተም፣ የታለመ ሶስት እጥፍ፣ የካርጎ ፕሮፋይል፣
እና የተሟላ የፋይል ክምችት. የቧንቧ መስመሮች መቼ እንደሆነ ለመለየት አንጸባራቂውን ሊለያዩ ይችላሉ።
አዲስ ቅርሶች ታዩ፣ JSON ከተለቀቁት ንብረቶች ጋር ይስቀሉ ወይም ኦዲት ያድርጉ
ጥቅልን ወደ ኦፕሬተሮች ከማስተዋወቅዎ በፊት hashes።

ረዳት አቅም ያለው ነው፡ ትዕዛዙን እንደገና ማስኬድ አንጸባራቂውን ያዘምናል።
`target/mochi-bundle/` ነጠላ አድርጎ በመያዝ የቀደመውን ማህደር ይተካል።
አሁን ባለው ማሽን ላይ ላለው የቅርብ ጊዜ ጥቅል የእውነት ምንጭ።