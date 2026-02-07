---
id: node-plan
lang: am
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

SF-3 Iroha/Torii ሂደትን ወደ SoraFS ማከማቻ አቅራቢ የሚቀይረውን `sorafs-node` crate ያቀርባል። ማቅረቢያዎችን በቅደም ተከተል ሲያስቀምጡ ይህንን እቅድ ከ[መስቀለኛ ማከማቻ መመሪያ](node-storage.md)፣ [የአቅራቢዎች መግቢያ ፖሊሲ](provider-admission-policy.md) እና [የማከማቻ አቅም የገበያ ቦታ መንገድ ካርታ](storage-capacity-marketplace.md) ጋር ይጠቀሙ።

## የዒላማ ወሰን (ሚልስቶን M1)

1. **Chunk Store ውህደት።** `sorafs_car::ChunkStore` በተዋቀረው የውሂብ መዝገብ ውስጥ ቸንክ ባይት፣ መግለጫዎች እና የPoR ዛፎችን በሚያከማች ቀጣይነት ባለው ጀርባ ይሸፍኑ።
2. **የጌትዌይ የመጨረሻ ነጥቦች።** የI18NT0000002X HTTP የመጨረሻ ነጥቦችን ለፒን ማቅረቢያ፣ ቸንክ ፈልሳፊ፣ የPoR ናሙና እና የማከማቻ ቴሌሜትሪ በTorii ሂደት ውስጥ ያጋልጡ።
3. **የማዋቀር ቧንቧ።** በ`iroha_config`፣ `iroha_core`፣ እና `iroha_torii` በኩል የተገጠመ የ`SoraFsStorage` ውቅር (የነቃ ባንዲራ፣ አቅም፣ ማውጫዎች፣ የኮንፈረንስ ገደቦች) ያክሉ።
4. **ኮታ/መርሐግብር።** በኦፕሬተር የተገለጸውን የዲስክ/ትይዩነት ገደቦችን እና የወረፋ ጥያቄዎችን ከኋላ ግፊት ጋር ያስፈጽሙ።
5. **ቴሌሜትሪ።** ሜትሪክስ/ሎግ ለፒን ስኬት፣ ቸንክ ማምጣት መዘግየት፣ የአቅም አጠቃቀም እና የPoR ናሙና ውጤቶች።

##የስራ መፈራረስ

### A. Crate & Module Structure

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| `crates/sorafs_node` በሞጁሎች ይፍጠሩ፡ `config`፣ `store`፣ `gateway`፣ `scheduler`፣ `telemetry`። | የማከማቻ ቡድን | ለ Torii ውህደት እንደገና ጥቅም ላይ ሊውሉ የሚችሉ ዓይነቶችን ወደ ውጭ መላክ። |
| ከ `SoraFsStorage` (ተጠቃሚ → ትክክለኛ → ነባሪዎች) የተቀረፀውን `StorageConfig` ይተግብሩ። | የማከማቻ ቡድን / ውቅር WG | የNorito/`iroha_config` ንብርብሮች የሚወስኑ መሆናቸውን ያረጋግጡ። |
| የ`NodeHandle` ፊት ለፊት Torii ፒን/ማስጠፊያዎችን ለማስገባት ይጠቅማል። | የማከማቻ ቡድን | የማጠራቀሚያ ውስጠ-ቁሳቁሶችን እና ያልተመሳሰሉ የቧንቧ መስመሮችን ይሸፍኑ. |

### B. የማያቋርጥ ቸንክ መደብር

| ተግባር | ባለቤት(ዎች) | ማስታወሻ |
|-------------|---|
| የዲስክ ጀርባ መጠቅለያ `sorafs_car::ChunkStore` በዲስክ አንጸባራቂ መረጃ ጠቋሚ (`sled`/I18NI0000046X) ይገንቡ። | የማከማቻ ቡድን | ቆራጥ አቀማመጥ፡ `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| I18NI0000048X በመጠቀም የPoR ሜታዳታ (64KiB/4KiB ዛፎች) አቆይ። | የማከማቻ ቡድን | እንደገና ከተጀመረ በኋላ እንደገና ማጫወትን ይደግፉ; በሙስና ላይ በፍጥነት መውደቅ. |
| በሚነሳበት ጊዜ የታማኝነት ድግግሞሹን ይተግብሩ (መግለጫዎችን እንደገና ያሻሽሉ ፣ ያልተሟሉ ፒኖችን ይቁረጡ)። | የማከማቻ ቡድን | ድጋሚ ማጫወት እስኪያልቅ ድረስ Torii አግድ። |

### ሐ. ጌትዌይ የመጨረሻ ነጥቦች

| የመጨረሻ ነጥብ | ባህሪ | ተግባራት |
|-------------|------|
| `POST /sorafs/pin` | `PinProposalV1`ን ተቀበል፣ አንጸባራቂዎችን አረጋግጥ፣ ወረፋ መግባቱን፣ በአንጸባራቂ CID ምላሽ ይስጡ። | የ chunk መገለጫን ያረጋግጡ፣ ኮታዎችን ያስፈጽሙ፣ ውሂብን በchunk ማከማቻ ያሰራጩ። |
| `GET /sorafs/chunks/{cid}` + ክልል መጠይቅ | ከ`Content-Chunker` ራስጌዎች ጋር ቸንክ ባይት ያገልግሉ፤ አክብሮት ክልል ችሎታ spek. | የጊዜ መርሐግብር + የዥረት በጀቶችን ይጠቀሙ (ከ SF-2d ክልል አቅም ጋር ይገናኙ)። |
| `POST /sorafs/por/sample` | ለማንፀባረቂያ እና የመመለሻ ማረጋገጫ ጥቅል የPoR ናሙናን ያሂዱ። | የ chunk ማከማቻ ናሙና እንደገና ተጠቀም፣ በNorito JSON ጭነቶች ምላሽ ይስጡ። |
| `GET /sorafs/telemetry` | ማጠቃለያዎች፡ አቅም፣ የPoR ስኬት፣ የስህተት ቆጠራዎች። | ለዳሽቦርዶች/ኦፕሬተሮች ውሂብ ያቅርቡ። |

የአሂድ ቧንቧዎች በ `sorafs_node::por` በኩል የPoR ግንኙነቶችን ይከተላሉ፡ መከታተያው እያንዳንዱን `PorChallengeV1`፣ `PorProofV1` እና `AuditVerdictV1` ይመዘግባል ስለዚህ I18NI000000059X ሜትሪክስ `CapacityMeter` ሜትሪክስ ያለአስተዳደር ፍርዶች00 ያንፀባርቃል አመክንዮ።【crates/sorafs_node/src/scheduler.rs#L147】

የትግበራ ማስታወሻዎች፡-

- የToriiን የአክሱም ቁልል ከ`norito::json` ጭነቶች ጋር ይጠቀሙ።
- ለምላሾች Norito ንድፎችን ያክሉ (`PinResultV1`፣ `FetchErrorV1`፣ telemetry structs)።

- ✅ I18NI0000063X አሁን የኋለኛውን ጥልቀት እና በጣም ጥንታዊውን ዘመን/የመጨረሻ ጊዜ እና አጋለጠ።
  ለእያንዳንዱ አገልግሎት አቅራቢ የቅርብ ጊዜ የስኬት/የመውደቅ የጊዜ ማህተም፣ የተጎላበተ
  `sorafs_node::NodeHandle::por_ingestion_status` እና Torii መዝግቧል
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` መለኪያዎች ለ ዳሽቦርዶች።【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【telecrates/srcrates/srcrates

### D. መርሐግብር እና ኮታ ማስፈጸሚያ

| ተግባር | ዝርዝሮች |
|------|--------|
| የዲስክ ኮታ | በዲስክ ላይ ባይት ይከታተሉ; `max_capacity_bytes` ሲያልፍ አዲስ ፒኖችን ውድቅ ያድርጉ። ለወደፊት ፖሊሲዎች የማስወጣት መንጠቆዎችን ያቅርቡ። |
| ተጓዳኝ አምጡ | ግሎባል ሴማፎር (`max_parallel_fetches`) እና የአቅራቢዎች በጀቶች ከSF-2d ክልል ካፕ የወጡ። |
| የፒን ወረፋ | እጅግ በጣም ጥሩ የሆኑ የምግብ ስራዎችን ይገድቡ; ለወረፋ ጥልቀት የNorito ሁኔታ የመጨረሻ ነጥቦችን ያጋልጡ። |
| PoR cadence | የበስተጀርባ ሰራተኛ በ`por_sample_interval_secs` የሚመራ። |

### ኢ ቴሌሜትሪ እና ሎግ

መለኪያዎች (Prometheus)

- `sorafs_pin_success_total`፣ `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (ሂስቶግራም ከ I18NI0000073X መለያዎች ጋር)
- `torii_sorafs_storage_bytes_used`፣ `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`፣ `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`፣ `torii_sorafs_storage_por_samples_failed_total`

መዝገቦች / ክስተቶች

- የተዋቀረ I18NT0000007X ቴሌሜትሪ ለአስተዳደር ማስመጫ (`StorageTelemetryV1`)።
- ሲጠቀሙ ማንቂያዎች>90% ወይም የPoR አለመሳካት ደረጃ ከገደቡ ሲያልፍ።

### F. የሙከራ ስልት

1. ** የዩኒት ሙከራዎች።** ቸንክ ሱቅ ጽናት፣ ኮታ ስሌቶች፣ የጊዜ መርሐግብር ልዩነቶች (`crates/sorafs_node/src/scheduler.rs` ይመልከቱ)።  
2. ** የውህደት ሙከራዎች *** (`crates/sorafs_node/tests`). ፒን → የዙር ጉዞን አምጡ፣ መልሶ ማግኘትን እንደገና ያስጀምሩ፣ የኮታ ውድቅ ማድረግ፣ የPoR ናሙና ማረጋገጫ ማረጋገጫ።  
3. **Torii የመዋሃድ ሙከራዎች።** Torii ን ከማከማቻው ጋር ያሂዱ፣ የኤችቲቲፒ የመጨረሻ ነጥቦችን በ`assert_cmd` ያካሂዱ።  
4. ** ትርምስ የመንገድ ካርታ።** የወደፊት ልምምዶች የዲስክን ድካም፣ ቀርፋፋ አይኦ፣ የአቅራቢዎችን ማስወገድን ያስመስላሉ።

## ጥገኛዎች

- SF-2b የመግቢያ ፖሊሲ - አንጓዎች ከማስታወቂያ በፊት የመግቢያ ፖስታዎችን እንደሚያረጋግጡ ያረጋግጡ።  
- SF-2c የአቅም የገበያ ቦታ - ቴሌሜትሪ ወደ የአቅም መግለጫዎች መልሰው ማሰር።  
- SF-2d የማስታወቂያ ማራዘሚያዎች - አንዴ ከተገኘ የክልል አቅም + የዥረት በጀቶችን ይጠቀሙ።

## ወሳኝ ደረጃ መውጫ መስፈርት

- `cargo run -p sorafs_node --example pin_fetch` ከአካባቢያዊ መገልገያዎች ጋር ይሰራል።  
- Torii በ `--features sorafs-storage` ይገነባል እና የውህደት ፈተናዎችን አልፏል።  
- ሰነድ ([መስቀለኛ ማከማቻ መመሪያ](node-storage.md)) በማዋቀር ነባሪዎች + CLI ምሳሌዎች የዘመነ; ከዋኝ runbook ይገኛል.  
- ቴሌሜትሪ በፕላስተር ዳሽቦርዶች ውስጥ ይታያል; ለአቅም ሙሌት እና ለPoR ውድቀቶች የተዋቀሩ ማንቂያዎች።

## ሰነዶች እና ኦፕስ ማቅረቢያዎች

- [የመስቀለኛ ማከማቻ ማጣቀሻ](node-storage.md) በማዋቀር ነባሪዎች፣ የCLI አጠቃቀም እና የመላ መፈለጊያ ደረጃዎችን ያዘምኑ።  
- SF-3 እየተሻሻለ ሲመጣ [መስቀለኛ ኦፕሬሽኖች runbook](node-operations.md) ከትግበራው ጋር እንዲጣጣም ያድርጉ።  
- የኤፒአይ ማጣቀሻዎችን ለ`/sorafs/*` የመጨረሻ ነጥቦችን በገንቢ ፖርታል ውስጥ ያትሙ እና ወደ OpenAPI ማኒፌስት አንድ ጊዜ Torii ተቆጣጣሪዎች ያርቁዋቸው።