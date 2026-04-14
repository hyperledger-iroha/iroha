<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus ክሮስ-ዳታስፔስ የአካባቢ አውታረ መረብ ማረጋገጫ

ይህ Runbook የNexus ውህደት ማረጋገጫን ይሰራል፡-

- ባለ 4-አቻ የአካባቢ መረብን በሁለት የተከለከሉ የግል የመረጃ ቦታዎች (`ds1`፣ `ds2`) ያስነሳል።
- ወደ እያንዳንዱ የውሂብ ቦታ የመለያ ትራፊክ መንገዶች ፣
- በእያንዳንዱ የውሂብ ቦታ ውስጥ ንብረት ይፈጥራል ፣
- በሁለቱም አቅጣጫዎች የአቶሚክ ስዋፕ አሰፋፈርን በውሂብ ቦታዎች ላይ ያከናውናል ፣
- በገንዘብ ያልተደገፈ እግር በማቅረብ እና ቀሪ ሂሳቦችን በመፈተሽ የመመለሻ ትርጉምን ያረጋግጣል።

ቀኖናዊው ፈተና፡-
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## ፈጣን ሩጫ

የመጠቅለያውን ስክሪፕት ከማጠራቀሚያ ስር ይጠቀሙ፡-

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

ነባሪ ባህሪ፡

- የውሂብ ቦታ ማረጋገጫ ሙከራን ብቻ ይሰራል ፣
- ስብስቦች `NORITO_SKIP_BINDINGS_SYNC=1`,
- ስብስቦች `IROHA_TEST_SKIP_BUILD=1` ፣
- `--test-threads=1` ይጠቀማል፣
- `--nocapture` ያልፋል።

## ጠቃሚ አማራጮች

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` ጊዜያዊ የአቻ ማውጫዎችን (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) ለፎረንሲክስ ያቆያል።
- `--all-nexus` የማስረጃ ሙከራውን ብቻ ሳይሆን `mod nexus::` (ሙሉ Nexus ውህደትን) ይሰራል።

## CI በር

CI ረዳት፡

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

ኢላማ አድርግ፡

```bash
make check-nexus-cross-dataspace
```

ይህ በር የመወሰኛ ማረጋገጫ መጠቅለያውን ያከናውናል እና የዳታ ቦታ አቋራጭ አቶሚክ ከሆነ ስራውን አይሳካለትም።
swap scenario regresses.

## በእጅ አቻ ትዕዛዞች

የታለመ የማረጋገጫ ሙከራ;

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

ሙሉ Nexus ንዑስ ስብስብ:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## የሚጠበቁ የማረጋገጫ ምልክቶች- ፈተናው ያልፋል።
- አንድ የሚጠበቀው ማስጠንቀቂያ ሆን ተብሎ ያልተሳካ የገንዘብ ድጋፍ የሚደረግለት የሰፈራ እግር ይታያል፡
  `settlement leg requires 10000 but only ... is available`.
- የመጨረሻ ሚዛን ማረጋገጫዎች ከሚከተሉት በኋላ ተሳክተዋል-
  - የተሳካ ሽግግር ፣
  - የተሳካ የተገላቢጦሽ መለዋወጥ;
  - በገንዘብ ያልተደገፈ መለዋወጥ (ያልተቀየሩ ቀሪ ሒሳቦች መመለስ) አልተሳካም።

## የአሁን የማረጋገጫ ቅጽበታዊ ገጽ እይታ

ከ **ፌብሩዋሪ 19፣ 2026** ጀምሮ ይህ የስራ ሂደት በሚከተሉት አለፈ፡-

የታለመ ሙከራ: `1 passed; 0 failed`,
- ሙሉ Nexus ንዑስ ስብስብ: `24 passed; 0 failed`.