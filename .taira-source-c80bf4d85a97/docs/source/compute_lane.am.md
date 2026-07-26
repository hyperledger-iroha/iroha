---
lang: am
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የስሌት መስመር (SSC-1)

የስሌት መስመር ወሳኙ የኤችቲቲፒ አይነት ጥሪዎችን ይቀበላል፣ በKotodama ላይ ያሰራቸዋል
የመግቢያ ነጥቦች፣ እና ለሂሳብ አከፋፈል እና የአስተዳደር ግምገማ መለኪያ/ደረሰኞችን ይመዘግባል።
ይህ RFC የአንጸባራቂውን እቅድ፣ የጥሪ/ደረሰኝ ኤንቨሎፕ፣ የአሸዋ ሳጥን መከላከያ መንገዶችን፣
እና ለመጀመሪያው ልቀት የውቅረት ነባሪዎች።

## ይገለጣል

- እቅድ፡ `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` ከ `1` ጋር ተያይዟል; መገለጫዎች ከተለየ ስሪት ጋር ውድቅ ተደርገዋል።
  በማረጋገጥ ጊዜ.
- እያንዳንዱ መንገድ እንዲህ ይላል:
  - `id` (`service`፣ `method`)
  - `entrypoint` (Kotodama የመግቢያ ነጥብ ስም)
  - የኮዴክ ፍቃድ ዝርዝር (`codecs`)
  - ቲቲኤል/ጋዝ/ጥያቄ/የምላሽ መያዣዎች (`ttl_slots`፣ `gas_budget`፣ `max_*_bytes`)
  - የመወሰን/የአፈፃፀም ክፍል (`determinism`፣ `execution_class`)
  - SoraFS ማስገቢያ/ሞዴል ገላጭ (`input_limits`፣ አማራጭ `model`)
  - የዋጋ አሰጣጥ ቤተሰብ (`price_family`) + የንብረት መገለጫ (`resource_profile`)
  - የማረጋገጫ ፖሊሲ (`auth`)
- የአሸዋ ቦክስ የጥበቃ መንገዶች በአንጸባራቂው `sandbox` ብሎክ ውስጥ ይኖራሉ እና በሁሉም ይጋራሉ።
  መንገዶች (ሁነታ/ዘፈቀደ/ማከማቻ እና የማይወሰን syscall አለመቀበል)።

ምሳሌ፡ `fixtures/compute/manifest_compute_payments.json`.

## ጥሪዎች፣ ጥያቄዎች እና ደረሰኞች

- እቅድ፡ `ComputeRequest`፣ `ComputeCall`፣ `ComputeCallSummary`፣ `ComputeReceipt`፣
  `ComputeMetering`፣ `ComputeOutcome` ውስጥ
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` ቀኖናዊ ጥያቄ ሃሽ ያዘጋጃል (ራስጌዎች ይቀመጣሉ)
  በቆራጥነት `BTreeMap` እና ክፍያው እንደ `payload_hash` ተሸክሟል)።
- `ComputeCall` የስም ቦታ/መንገድ፣ ኮዴክ፣ ቲቲኤል/ጋዝ/ምላሽ ቆብ፣
  የንብረት መገለጫ + የዋጋ ቤተሰብ፣ auth (`Public` ወይም UAID-የተሳሰረ)
  `ComputeAuthn`)፣ ቆራጥነት (`Strict` vs `BestEffort`)፣ የማስፈጸሚያ ክፍል
  ፍንጮች (ሲፒዩ/ጂፒዩ/TEE)፣ SoraFS የግቤት ባይት/ቺንክ፣ አማራጭ ስፖንሰር ታውቋል
  በጀት, እና ቀኖናዊ ጥያቄ ፖስታ. የጥያቄው ሃሽ ጥቅም ላይ ይውላል
  እንደገና ማጫወት ጥበቃ እና መስመር.
- መንገዶች አማራጭ SoraFS ሞዴል ማጣቀሻዎችን እና የግቤት ገደቦችን ሊያካትት ይችላል
  (የመስመር / ቻንክ ካፕ); አንጸባራቂ ማጠሪያ ደንቦች የጂፒዩ/TEE ፍንጮች።
- `ComputePriceWeights::charge_units` የመለኪያ መረጃን ወደ ሂሳብ ማስላት ይለውጣል
  በዑደቶች እና በመውጣት ባይት ላይ በጣራው ክፍፍል በኩል ክፍሎች።
- `ComputeOutcome` ሪፖርት `Success`፣ `Timeout`፣ `OutOfMemory`፣
  `BudgetExhausted`፣ ወይም `InternalError` እና እንደ አማራጭ የምላሽ ሃሽዎችን ያካትታል/
  መጠኖች / ኮዴክ ለኦዲት.

ምሳሌዎች፡-
- ይደውሉ: `fixtures/compute/call_compute_payments.json`
- ደረሰኝ: `fixtures/compute/receipt_compute_payments.json`

## ማጠሪያ እና የንብረት መገለጫዎች- `ComputeSandboxRules` በነባሪነት የአፈፃፀም ሁነታን ወደ `IvmOnly` ይቆልፋል ፣
  ዘሮች ከጥያቄው ሃሽ የሚወስን በዘፈቀደ፣ ተነባቢ-ብቻ SoraFS ይፈቅዳል።
  መድረስ፣ እና የማይወስኑ syscalsን ውድቅ ያደርጋል። የጂፒዩ/TEE ፍንጮች የተከለሉ ናቸው።
  `allow_gpu_hints`/`allow_tee_hints` አፈጻጸምን ለመወሰን።
- `ComputeResourceBudget` በየመገለጫ ዑደቶች ፣ መስመራዊ ማህደረ ትውስታ ፣ ቁልል ላይ በየመገለጫ ያዘጋጃል
  መጠን፣ IO በጀት፣ እና መውጫ፣ እና ለጂፒዩ ፍንጮች እና የWASI-ላይት ረዳቶች መቀያየር።
- ነባሪዎች ሁለት መገለጫዎችን (`cpu-small`፣ `cpu-balanced`) በስር ይልካሉ።
  `defaults::compute::resource_profiles` ከመወሰኛ ውድቀት ጋር።

## የዋጋ አሰጣጥ እና የሂሳብ አከፋፈል ክፍሎች

- ዋጋ ቤተሰቦች (`ComputePriceWeights`) የካርታ ዑደቶች እና መውጣት ባይት ወደ ስሌት
  ክፍሎች; ነባሪዎች `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` ያስከፍላሉ
  `unit_label = "cu"`. ቤተሰቦች በ`price_family` በማኒፌስት እና
  መግቢያ ላይ ተፈጻሚ.
- የመለኪያ መዛግብት `charged_units` እና ጥሬ ዑደት/የመግባት/የመውጣት/የቆይታ ጊዜን ይይዛሉ።
  ጠቅላላ ለእርቅ. ክፍያዎች በአፈፃፀም-ክፍል እና
  determinism multipliers (`ComputePriceAmplifiers`) እና በ ተሸፍኗል
  `compute.economics.max_cu_per_call`; egress በ ተጨምቆ ነው።
  `compute.economics.max_amplification_ratio` ወደ የታሰረ ምላሽ ማጉላት።
- የስፖንሰር በጀቶች (`ComputeCall::sponsor_budget_cu`) ተፈጻሚ ሆነዋል
  በእያንዳንዱ ጥሪ / ዕለታዊ ካፕ; የተከፈሉ ክፍሎች ከተገለጸው የስፖንሰር በጀት መብለጥ የለባቸውም።
- የአስተዳደር ዋጋ ማሻሻያ የአደጋ-ክፍል ወሰኖችን ይጠቀማሉ
  `compute.economics.price_bounds` እና የመሠረታዊ ቤተሰቦች ተመዝግበዋል
  `compute.economics.price_family_baseline`; መጠቀም
  `ComputeEconomics::apply_price_update` ዴልታዎችን ከማዘመን በፊት ለማረጋገጥ
  ንቁ የቤተሰብ ካርታ. የ Torii ማዋቀር ዝመናዎች ይጠቀማሉ
  `ConfigUpdate::ComputePricing`፣ እና ኪሶ ከተመሳሳዩ ወሰኖች ጋር ይተገበራል
  የአስተዳደር አርትዖቶችን ቆራጥነት ያቆዩ።

## ማዋቀር

አዲስ የስሌት ውቅር በ`crates/iroha_config/src/parameters` ውስጥ ይኖራል፡

- የተጠቃሚ እይታ፡- `Compute` (`user.rs`) ከኤንቭ መሻር ጋር፡
  - `COMPUTE_ENABLED` (ነባሪ `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- የዋጋ አሰጣጥ / ኢኮኖሚክስ: `compute.economics` ቀረጻዎች
  `max_cu_per_call`/`max_amplification_ratio`፣ የክፍያ ክፍፍል፣ የስፖንሰር ካፕ
  (በየጥሪ እና በየቀኑ CU)፣ የዋጋ የቤተሰብ መነሻ መስመሮች + ለአደጋ ክፍሎች/ገደቦች
  የአስተዳደር ዝማኔዎች፣ እና የማስፈጸሚያ-ክፍል አባዢዎች (ጂፒዩ/ቲኢ/ምርጥ-ጥረት)።
- ትክክለኛ/ነባሪዎች፡- `actual.rs`/`defaults.rs::compute` አጋልጧል
  `Compute` ቅንብሮች (ስም ቦታዎች፣ መገለጫዎች፣ የዋጋ ቤተሰቦች፣ ማጠሪያ)።
- ልክ ያልሆኑ ውቅሮች (ባዶ የስም ቦታዎች፣ ነባሪ መገለጫ/ቤተሰብ ጠፍቷል፣ የቲቲኤል ካፕ
  ተገላቢጦሽ) በመተንተን ወቅት እንደ `InvalidComputeConfig` ተዘርግተዋል።

## ሙከራዎች እና መለዋወጫዎች

- ቆራጥ ረዳቶች (`request_hash`፣ ዋጋ አወጣጥ) እና የቋሚ ጉዞዎች በ ውስጥ ይኖራሉ
  `crates/iroha_data_model/src/compute/mod.rs` (`fixtures_round_trip` ይመልከቱ፣
  `request_hash_is_stable`፣ `pricing_rounds_up_units`)።
- የJSON መጫዎቻዎች በ`fixtures/compute/` ውስጥ ይኖራሉ እና በመረጃ-ሞዴል ነው የሚሰሩት
  ለድጋሚ ሽፋን ሙከራዎች.

## SLO መታጠቂያ እና በጀት- የ`compute.slo.*` ውቅር የመግቢያ መንገዱን SLO ቁልፎችን ያጋልጣል (በበረራ ላይ ወረፋ)
  ጥልቀት፣ የ RPS ካፕ እና የመዘግየት ዒላማዎች) ውስጥ
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. ነባሪዎች፡ 32
  በበረራ ላይ፣ በየመንገድ 512 ወረፋ፣ 200 RPS፣ p50 25ms፣ p95 75ms፣ p99 120ms
- የ SLO ማጠቃለያዎችን እና ጥያቄ/መውጣትን ለመያዝ ቀላል ክብደት ያለውን የቤንች ማሰሪያ ያሂዱ
  ቅጽበታዊ ገጽ እይታ፡ `የጭነት አሂድ -p xtask --bin compute_gateway -- አግዳሚ ወንበር [ማኒፌስት_መንገድ]
  [ተደጋጋሚ] [concurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`፣
  128 ድግግሞሾች፣ ተጓዳኝ 16፣ ውጤቶች በታች
  `artifacts/compute_gateway/bench_summary.{json,md}`). አግዳሚ ወንበር ይጠቀማል
  የሚወስኑ ክፍያዎች (`fixtures/compute/payload_compute_payments.json`) እና
  የአካል ብቃት እንቅስቃሴ በሚያደርጉበት ጊዜ ግጭቶችን ለማስቀረት በየራስ መጠየቂያዎች
  `echo`/`uppercase`/`sha3` የመግቢያ ነጥቦች።

## የኤስዲኬ/CLI እኩልነት መጫዎቻዎች

- ቀኖናዊ መጫዎቻዎች በ`fixtures/compute/` ስር ይኖራሉ፡ መግለጫ፣ ጥሪ፣ ጭነት እና
  የጌትዌይ አይነት ምላሽ/ደረሰኝ አቀማመጥ። የመጫኛ ሃሽ ከጥሪው ጋር መዛመድ አለበት።
  `request.payload_hash`; የረዳቱ ጭነት ይኖራል
  `fixtures/compute/payload_compute_payments.json`.
- CLI `iroha compute simulate` እና `iroha compute invoke` ይልካል።

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` በቀጥታ
  `javascript/iroha_js/src/compute.js` ከእንደገና ሙከራዎች በታች
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` ተመሳሳይ መገልገያዎችን ይጭናል, የመጫኛ ሃሽዎችን ያረጋግጣል,
  እና የመግቢያ ነጥቦቹን ከሙከራዎች ጋር ያስመስላል
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- የ CLI/JS/Swift ረዳቶች ሁሉም ተመሳሳይ Norito መጫዎቻዎችን ይጋራሉ ስለዚህ ኤስዲኬዎች እንዲችሉ
  ሀ ሳይመታ የጥያቄ ግንባታ እና ከመስመር ውጭ አያያዝን ያረጋግጡ
  የሩጫ መግቢያ.