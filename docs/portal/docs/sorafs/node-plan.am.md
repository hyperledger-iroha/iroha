---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c516f77520bb3562ff1d06794ed8fcf14e6a3f98d7e2a6ca99ae0ff2b8f4536d
source_last_modified: "2026-01-05T09:28:11.898207+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
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

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `GET /v1/sorafs/pin`, `POST /v1/sorafs/pin/register`, `GET /v1/sorafs/pin/{digest_hex}` | Read the pin registry, register paid manifest pins, and fetch bounded manifest pin details. | Validate chunker profiles, manifest payloads, pin policy, fee receipt context, aliases, and successor links before queueing the signed transaction. |
| `POST /v1/sorafs/storage/pin`, `POST /v1/sorafs/storage/fetch`, `POST /v1/sorafs/storage/token` | Store payload bytes for an approved manifest, fetch content ranges, and issue storage access tokens. | Enforce quotas, token policy, provider capability checks, and scheduler/back-pressure limits. |
| `GET /v1/sorafs/storage/manifest/{manifest_id}`, `GET /v1/sorafs/storage/plan/{manifest_id}`, `GET /v1/sorafs/storage/car/{manifest_id}`, `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}` | Serve bounded manifest metadata, deterministic chunk plans, CAR bytes, and individual chunk bytes. | Keep readback arrays bounded while preserving total counts and verify digest/path bindings before streaming bytes. |
| `GET /v1/sorafs/storage/peers`, `GET /v1/sorafs/storage/state`, `POST /v1/sorafs/storage/por-sample` | Report peer/storage state and request bounded local PoR samples. Proof and verdict admission use the authenticated capacity lifecycle; direct storage mutation routes are not mounted. | Reuse chunk-store sampling, update telemetry, and preserve governance-verdict replay state. |

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
- Torii exposes the current `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` route surface and passes integration tests.
- ሰነድ ([መስቀለኛ ማከማቻ መመሪያ](node-storage.md)) በማዋቀር ነባሪዎች + CLI ምሳሌዎች የዘመነ; ከዋኝ runbook ይገኛል.  
- ቴሌሜትሪ በፕላስተር ዳሽቦርዶች ውስጥ ይታያል; ለአቅም ሙሌት እና ለPoR ውድቀቶች የተዋቀሩ ማንቂያዎች።

## ሰነዶች እና ኦፕስ ማቅረቢያዎች

- [የመስቀለኛ ማከማቻ ማጣቀሻ](node-storage.md) በማዋቀር ነባሪዎች፣ የCLI አጠቃቀም እና የመላ መፈለጊያ ደረጃዎችን ያዘምኑ።  
- SF-3 እየተሻሻለ ሲመጣ [መስቀለኛ ኦፕሬሽኖች runbook](node-operations.md) ከትግበራው ጋር እንዲጣጣም ያድርጉ።  
- Keep API reference for `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` endpoints aligned with the OpenAPI manifest.