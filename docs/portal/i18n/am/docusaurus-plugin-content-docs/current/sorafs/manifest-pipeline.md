---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Chunking → አንጸባራቂ ቧንቧ

ይህ የፈጣን ጅምር ጓደኛ ወደ ጥሬ የሚለወጠውን ከጫፍ እስከ ጫፍ ያለውን የቧንቧ መስመር ይከታተላል
ባይት ወደ I18NT0000000X ለSoraFS ፒን መዝገብ ተስማሚ ያሳያል። ይዘቱ ነው።
ከ [`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) የተስተካከለ;
ያንን ሰነድ ለቀኖናዊው ዝርዝር መግለጫ እና ለውጥ ሎግ ያማክሩ።

## 1. ቁርጥ ቁርጥ ቁርጥ

SoraFS የ SF-1 (`sorafs.sf1@1.0.0`) መገለጫን ይጠቀማል፡ በFastCDC አነሳሽነት የሚሽከረከር
ሃሽ በትንሹ 64ኪባ ቁራጭ መጠን፣ 256ኪባ ዒላማ፣ ከፍተኛ 512ኪባ እና ሀ
`0x0000ffff` መስበር ጭንብል። መገለጫው በ ውስጥ ተመዝግቧል
`sorafs_manifest::chunker_registry`.

### ዝገት ረዳቶች

- `sorafs_car::CarBuildPlan::single_file` – የተቆራረጡ ማካካሻዎችን፣ ርዝመቶችን እና
  CAR ሜታዳታ በማዘጋጀት ላይ እያለ BLAKE3 ይዋጣል።
- `sorafs_car::ChunkStore` - የሚጫኑ ጭነቶችን ያሰራጫል፣ የተቆራረጠ ሜታዳታ እና
  64ኪቢ/4ኪቢ መልሶ ማግኘት የሚቻልበትን ማረጋገጫ (PoR) የናሙና ዛፍ ያገኛል።
- `sorafs_chunker::chunk_bytes_with_digests` - ከሁለቱም CLIዎች በስተጀርባ የቤተ መፃህፍት ረዳት።

### የ CLI መገልገያ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON የታዘዙ ማካካሻዎችን፣ ርዝመቶችን እና ቁርጥራጭ መጭመቂያዎችን ይዟል። በጽናት
መግለጫዎች ወይም ኦርኬስትራ ማምለጫ ዝርዝሮችን ሲገነቡ እቅድ ያውጡ።

### የPoR ምስክሮች

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` እና
`--por-sample=<count>` ስለዚህ ኦዲተሮች ቆራጥ የምሥክርነት ስብስቦችን መጠየቅ ይችላሉ። ጥንድ
JSON ለመቅዳት እነዚያ ባንዲራዎች ከ `--por-proof-out` ወይም I18NI0000019X ጋር።

## 2. አንጸባራቂ መጠቅለል

`ManifestBuilder` ቁራጭ ሜታዳታን ከአስተዳደር አባሪዎች ጋር ያጣምራል።

- Root CID (dag-cbor) እና CAR ቁርጠኝነት።
- ተለዋጭ ማስረጃዎች እና የአቅራቢዎች ችሎታ ይገባኛል ጥያቄዎች።
- የምክር ቤት ፊርማዎች እና አማራጭ ሜታዳታ (ለምሳሌ የግንባታ መታወቂያዎች)።

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

ጠቃሚ ውጤቶች፡-

- `payload.manifest` – Norito የተመሰጠረ አንጸባራቂ ባይት።
- `payload.report.json` - የሰው / አውቶሜትድ ሊነበብ የሚችል ማጠቃለያ, ጨምሮ
  `chunk_fetch_specs`፣ `payload_digest_hex`፣ CAR መፈጨት እና ቅጽል ሜታዳታ።
- `payload.manifest_signatures.json` - አንጸባራቂ BLAKE3 የያዘ ፖስታ
  መፍጨት፣ ቸንክ-ፕላን SHA3 መፍጨት፣ እና የ Ed25519 ፊርማዎችን ደርድር።

በውጫዊ የቀረቡ ኤንቨሎፖችን ለማረጋገጥ `--manifest-signatures-in` ይጠቀሙ
ፈራሚዎች መልሰው ከመጻፍዎ በፊት, እና `--chunker-profile-id` ወይም
የመዝገብ ምርጫን ለመቆለፍ `--chunker-profile=<handle>`።

## 3. አትም እና ፒን

1. ** የአስተዳደር ግቤት *** - አንጸባራቂውን መፈጨት እና ፊርማ ያቅርቡ
   ፒኑ እንዲገባ ኤንቨሎፕ ወደ ምክር ቤቱ። የውጭ ኦዲተሮች አለባቸው
   የ chunk-plan SHA3 መፍጨት ከማንፀባረቂያው መፈጨት ጋር ያከማቹ።
2. ** የሚጫኑ ጭነቶችን ይሰኩ** - የተጠቀሰውን የCAR ማህደር (እና አማራጭ የCAR ኢንዴክስ) ይስቀሉ
   በማንፀባረቂያው ውስጥ ወደ ፒን መዝገብ ቤት. አንጸባራቂውን ያረጋግጡ እና CAR ማጋራቱን ያረጋግጡ
   ተመሳሳይ ስር CID.
3. ** ቴሌሜትሪ ይቅረጹ *** - የJSON ዘገባን፣ የPoR ምስክሮችን እና ማንኛውንም ማምጣት
   በመለቀቂያ ቅርሶች ውስጥ መለኪያዎች. እነዚህ መዝገቦች ከዋኝ ዳሽቦርዶች እና
   ትላልቅ ጭነቶችን ሳያወርዱ ችግሮችን እንደገና ለማራባት ያግዙ።

## 4. ባለብዙ አቅራቢ አስመሳይ

`የጭነት ሩጫ -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --አቅራቢ=አልፋ=አቅራቢዎች/አልፋ.ቢን --አቅራቢ=ቤታ=አቅራቢዎች/beta.bin#4@3 \
  --output=የክፍያ ጭነት.bin --json-out=fetch_report.json`

- `#<concurrency>` በአንድ አቅራቢ ትይዩ ይጨምራል (`#4` ከላይ)።
- `@<weight>` ዜማዎች መርሐግብር አድልዎ; ነባሪዎች ወደ 1.
- `--max-peers=<n>` አንድ አሂድ ጊዜ መርሐግብር አቅራቢዎች ብዛት caps
  ግኝቱ ከተፈለገው በላይ ብዙ እጩዎችን ይሰጣል።
- `--expect-payload-digest` እና `--expect-payload-len` ከዝምታ ይጠብቁ
  ሙስና.
- `--provider-advert=name=advert.to` የአቅራቢውን አቅም ከዚህ በፊት ያረጋግጣል
  በማስመሰል ውስጥ እነሱን መጠቀም.
- `--retry-budget=<n>` በእያንዳንዱ-ቻንክ የድጋሚ ሙከራ ቆጠራን ይሽራል (ነባሪ፡ 3) ስለዚህ CI
  የብልሽት ሁኔታዎችን በሚፈትሽበት ጊዜ ድግግሞሾችን በፍጥነት ማየት ይችላል።

`fetch_report.json` የወለል ንጣፎች የተዋሃዱ መለኪያዎች (`chunk_retry_total`፣
`provider_failure_rate`, ወዘተ) ለ CI ማረጋገጫዎች እና ታዛቢነት ተስማሚ.

## 5. የመዝገብ ማሻሻያ እና አስተዳደር

አዲስ chunker መገለጫዎችን ሲያቀርቡ፡-

1. ገላጭውን ደራሲ በ `sorafs_manifest::chunker_registry_data`.
2. `docs/source/sorafs/chunker_registry.md` እና ተዛማጅ ቻርተሮችን ያዘምኑ።
3. የቤት ዕቃዎችን እንደገና ማመንጨት (`export_vectors`) እና የተፈረሙ መግለጫዎችን ይያዙ።
4. የቻርተሩን ተገዢነት ሪፖርት ከአስተዳደር ፊርማ ጋር ያቅርቡ.

አውቶሜሽን ቀኖናዊ እጀታዎችን (`namespace.name@semver`) እና መውደቅን መምረጥ አለበት።