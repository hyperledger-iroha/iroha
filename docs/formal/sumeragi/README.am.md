<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi መደበኛ ሞዴል (TLA+ / Apalache)

ይህ ማውጫ ለSumeragi የቁርጥ-መንገድ ደህንነት እና ሕያውነት የታሰረ መደበኛ ሞዴል ይዟል።

## ወሰን

ሞዴሉ የሚከተሉትን ይይዛል-
- የደረጃ እድገት (`Propose`፣ `Prepare`፣ `CommitVote`፣ `NewView`፣ `Committed`)፣
- ድምጽ እና የኮረም ገደቦች (`CommitQuorum`፣ `ViewQuorum`)፣
- ለNPoS አይነት የጥብቅና ጥበቃዎች (`StakeQuorum`) ክብደት ያለው የካስማ ኮረም፣
- RBC መንስኤ (`Init -> Chunk -> Ready -> Deliver`) ከአርዕስት/የመፍጨት ማስረጃ ጋር፣
- GST እና ደካማ የፍትሃዊነት ግምቶች በታማኝነት የእድገት እርምጃዎች ላይ።

ሆን ብሎ የሽቦ ቅርጸቶችን፣ ፊርማዎችን እና ሙሉ የአውታረ መረብ ዝርዝሮችን ያስወግዳል።

## ፋይሎች

- `Sumeragi.tla`: የፕሮቶኮል ሞዴል እና ባህሪያት.
- `Sumeragi_fast.cfg`: አነስ CI-ተስማሚ መለኪያ ስብስብ.
- `Sumeragi_deep.cfg`: ትልቅ የጭንቀት መለኪያ ስብስብ.

## ንብረቶች

ተለዋዋጮች
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

ጊዜያዊ ንብረት;
- `EventuallyCommit` (`[] (gst => <> committed)`)፣ ከጂኤስቲ በኋላ ፍትሃዊነት በኮድ
  በ `Next` ውስጥ የሚሰራ (የጊዜ ማብቂያ/የስህተት ቅድመ-መከላከያ ነቅቷል)
  የእድገት እርምጃዎች). ይሄ ሞዴሉን ከ Apalache 0.52.x ጋር መፈተሽ እንዲችል ያደርገዋል, ይህም
  በተረጋገጡ ጊዜያዊ ንብረቶች ውስጥ የ`WF_` የፍትሃዊነት ኦፕሬተሮችን አይደግፍም።

## በመሮጥ ላይ

ከማከማቻ ስር፡

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### ሊባዛ የሚችል የአካባቢ ማዋቀር (አይ18NT00000003X አያስፈልግም)በዚህ ማከማቻ ጥቅም ላይ የዋለውን የተሰካውን የአካባቢ Apalache መሣሪያ ሰንሰለት ጫን፡-

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

ሯጩ ይህንን ጭነት በሚከተለው ላይ በራስ-ሰር ያውቀዋል።
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
ከተጫነ በኋላ, `ci/check_sumeragi_formal.sh` ያለ ተጨማሪ env vars መስራት አለበት:

```bash
bash ci/check_sumeragi_formal.sh
```

Apalache በ`PATH` ውስጥ ካልሆነ፣ የሚከተሉትን ማድረግ ይችላሉ፦

- `APALACHE_BIN` ወደ ተፈፃሚው መንገድ ያዘጋጁ ፣ ወይም
- የDocker ውድቀትን ይጠቀሙ (`docker` ሲገኝ በነባሪነት የነቃ)
  - ምስል፡ `APALACHE_DOCKER_IMAGE` (ነባሪ `ghcr.io/apalache-mc/apalache:latest`)
  - የሚያሄድ Docker ዴሞን ይፈልጋል
  - በ`APALACHE_ALLOW_DOCKER=0` መመለስን ያሰናክሉ።

ምሳሌዎች፡-

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## ማስታወሻዎች

- ይህ ሞዴል የ Rust ሞዴል ሙከራዎችን ያሟላል (አይተካም)
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  እና
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ቼኮች በ `.cfg` ፋይሎች ውስጥ በቋሚ እሴቶች የተገደቡ ናቸው።
- PR CI እነዚህን ቼኮች በ `.github/workflows/pr.yml` በኩል ያካሂዳል
  `ci/check_sumeragi_formal.sh`.