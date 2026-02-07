---
lang: am
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የብረታ ብረት እና የኒዮን ማጣደፍ እቅድ (ፈጣን እና ዝገት)

ይህ ሰነድ የሚወስን ሃርድዌርን ለማንቃት የጋራ እቅድን ይይዛል
ማጣደፍ (የብረት ጂፒዩ + NEON/ሲምዲ + የስትሮንግቦክስ ውህደትን ያፋጥኑ) በመላ
የ Rust የስራ ቦታ እና ስዊፍት ኤስዲኬ። ክትትል የሚደረግባቸውን የመንገድ ካርታ እቃዎች ይመለከታል
በ ** የሃርድዌር ማጣደፍ የስራ ዥረት (macOS/iOS)** ስር እና የእጅ ማጥፋት ያቀርባል
ቅርስ ለ Rust IVM ቡድን፣ የስዊፍት ድልድይ ባለቤቶች እና የቴሌሜትሪ መሣሪያዎች።

> መጨረሻ የዘመነው: 2026-01-12  
> ባለቤቶች፡ IVM አፈጻጸም TL፣ Swift SDK Lead

# ግቦች

1. የ Rust GPU kernels (Poseidon/BN254/CRC64) በአፕል ሃርድዌር በብረት በኩል እንደገና ይጠቀሙ።
   ከሲፒዩ ዱካዎች ጋር ቆራጥነት ያላቸውን ጥላዎች ያሰሉ ።
2. የፍጥነት መቀያየርን (`AccelerationConfig`) ከጫፍ እስከ ጫፍ ስለዚህ የስዊፍት መተግበሪያዎችን ያጋልጡ
   የ ABI/parity ዋስትናዎችን እየጠበቁ ወደ ብረት/NEON/StrongBox መርጠው መግባት ይችላሉ።
3. መሳሪያ CI + ዳሽቦርዶች ወደ ላይ ላዩን እኩልነት/ቤንችማርክ መረጃ እና ባንዲራ
   በሲፒዩ vs ጂፒዩ/ሲኤምዲ ዱካዎች ላይ ያሉ ለውጦች።
4. StrongBox/አስተማማኝ-የማጠቃለያ ትምህርቶችን በአንድሮይድ (AND2) እና በስዊፍት መካከል ያካፍሉ።
   (IOS4) ፍሰቶችን በቆራጥነት መፈረሙን ለማስቀጠል።

** አዘምን (CRC64 + ደረጃ-1 አድስ):** CRC64 ጂፒዩ ረዳቶች አሁን ወደ Prometheus በ192KiB ነባሪ መቆራረጥ (በ`NORITO_GPU_CRC64_MIN_BYTES` መሻር ወይም ግልጽ የረዳት `NORITO_CRC64_GPU_LIB`) እና የመውደቅ ሲምዲ እየያዙ ነው። JSON Stage‑1 መቁረጫዎች እንደገና ቤንችማርክ ተደርጎላቸዋል (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`)፣ scalar cutover 4KiB ላይ እንዲቆይ እና የStage‑1 ጂፒዩ ነባሪውን ከ192KiB (`NORITO_STAGE1_GPU_MIN_BYTES`) ጋር በማጣጣም (`NORITO_STAGE1_GPU_MIN_BYTES` ትልቅ ዋጋ ያስከፍላል)።

## የሚላኩ እና ባለቤቶች

| ወሳኝ ምዕራፍ | ሊደርስ የሚችል | ባለቤት(ዎች) | ዒላማ |
|--------|-------------|-------|-------|
| ዝገት WP2-A / B | የ CUDA አስኳሎች የሚያንጸባርቁ የብረት ሼደር በይነገጾች | IVM Perf TL | የካቲት 2026 |
| ዝገት WP2-C | ሜታል BN254 እኩልነት ሙከራዎች & CI ሌይን | IVM Perf TL | Q2 2026 |
| ስዊፍት IOS6 | የድልድይ መቀየሪያዎች ባለገመድ (`connect_norito_set_acceleration_config`) + ኤስዲኬ ኤፒአይ + ናሙናዎች | ስዊፍት ድልድይ ባለቤቶች | ተከናውኗል (ጥር 2026) |
| ስዊፍት IOS5 | የውቅር አጠቃቀምን የሚያሳዩ መተግበሪያዎች/ሰነዶች ናሙና | Swift DX TL | Q2 2026 |
| ቴሌሜትሪ | ዳሽቦርድ ምግቦች ከፍጥነት እኩልነት + የቤንችማርክ መለኪያዎች | ስዊፍት ፕሮግራም PM / ቴሌሜትሪ | አብራሪ ውሂብ Q2 2026 |
| CI | XCFramework ጭስ ታጥቆ ሲፒዩ vs Metal/NEON በመሣሪያ ገንዳ | ስዊፍት QA መሪ | Q2 2026 |
| StrongBox | በሃርድዌር የተደገፈ የፊርማ እኩልነት ፈተናዎች (የተጋሩ ቬክተሮች) | አንድሮይድ Crypto TL / ስዊፍት ደህንነት | Q3 2026 |

## በይነገጽ እና የኤፒአይ ኮንትራቶች### ዝገት (`ivm::AccelerationConfig`)
- ያሉትን መስኮች አቆይ (`enable_simd`፣ `enable_metal`፣ `enable_cuda`፣ `max_gpus`፣ ጣራዎች)።
- ለመጀመሪያ ጊዜ ጥቅም ላይ የሚውል መዘግየትን ለማስወገድ ግልጽ የሆነ የብረት ማሞቂያ (ዝገት #15875) ይጨምሩ።
- ለዳሽቦርዶች የተመጣጣኝ ኤፒአይዎችን የመመለሻ ሁኔታ/መመርመሪያ ያቅርቡ፡
  - ለምሳሌ. `ivm::vector::metal_status()` -> {ነቅቷል፣ እኩልነት፣ የመጨረሻ_ስህተት}።
- የውጤት ቤንችማርኪንግ ሜትሪክስ (Merkle tree timings፣ CRC throughput) በ በኩል
  ቴሌሜትሪ መንጠቆዎች ለ `ci/xcode-swift-parity`.
- የብረታ ብረት አስተናጋጅ አሁን የተጠናቀረውን `fastpq.metallib` ይጭናል፣ FFT/IFFT/LDE ይልካል።
  እና Poseidon kernels፣ እና በማንኛውም ጊዜ ወደ ሲፒዩ ትግበራ ይመለሳል
  metallib ወይም የመሣሪያ ወረፋ አይገኝም።

### C FFI (`connect_norito_bridge`)
- አዲስ መዋቅር `connect_norito_acceleration_config` (የተጠናቀቀ)።
- የጌተር ሽፋን አሁን `connect_norito_get_acceleration_config` (config ብቻ) እና `connect_norito_get_acceleration_state` (config + parity)ን ያካትታል።
- ለ SPM/CocoaPods ሸማቾች በርዕስ አስተያየቶች ውስጥ አቀማመጥን ያዋቅራል።

### ስዊፍት (`AccelerationSettings`)
- ነባሪዎች፡ ብረት ነቅቷል፣ CUDA ተሰናክሏል፣ ገደብ ዜሮ (ውርስ)።
- አሉታዊ እሴቶች ችላ ተብለዋል; `apply()` በቀጥታ በ`IrohaSDK` ተጠርቷል።
- `AccelerationSettings.runtimeState()` አሁን `connect_norito_get_acceleration_state` ላይ ይታያል
  የክፍያ ጭነት (የማዋቀር + ብረት/CUDA እኩልነት ሁኔታ) ስለዚህ ስዊፍት ዳሽቦርዶች ተመሳሳይ ቴሌሜትሪ ያመነጫሉ
  እንደ ዝገት (`supported/configured/available/parity`)። ረዳቱ ሲመለስ `nil`
  ፈተናዎችን ተንቀሳቃሽ ለማድረግ ድልድይ የለም።
- `AccelerationBackendStatus.lastError` የማሰናከል/የስህተት ምክንያት ከ
  `connect_norito_get_acceleration_state` እና ሕብረቁምፊው አንዴ ከሆነ ቤተኛ ቋት ነጻ ያወጣል።
  የተንቀሳቃሽ ስልክ እኩልነት ዳሽቦርዶች ሜታል/CUDA ለምን እንደተሰናከለ ሊገልጽ ይችላል።
  እያንዳንዱ አስተናጋጅ.
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`፣
  ፈተናዎች በ `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) አሁን
  የፈታ ኦፕሬተር እንደ Norito ማሳያ፡ ክብር በተመሳሳይ የቅድሚያ ቅደም ተከተል ያሳያል
  `NORITO_ACCEL_CONFIG_PATH`፣ ፍለጋ `acceleration.{json,toml}`/`client.{json,toml}`፣
  የተመረጠውን ምንጭ ይመዝግቡ እና ወደ ነባሪዎች ይመለሱ። መተግበሪያዎች ከአሁን በኋላ ሎጀሮች አያስፈልጋቸውም።
  የ Rust `iroha_config` ንጣፍን ያንጸባርቁ።
- መቀያየሪያዎችን እና የቴሌሜትሪ ውህደትን ለማሳየት የናሙና መተግበሪያዎችን እና README ያዘምኑ።

### ቴሌሜትሪ (ዳሽቦርዶች + ላኪዎች)
- የተመጣጣኝ ምግብ (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {ነቅቷል፣ እኩልነት፣ perf_delta_pct}።
  - የ `perf_delta_pct` መነሻ ሲፒዩ ከጂፒዩ ንፅፅርን ተቀበል።
  - `acceleration.metal.disable_reason` መስተዋቶች `AccelerationBackendStatus.lastError`
    ስለዚህ ስዊፍት አውቶሜሽን ልክ እንደ Rust ታማኝነት የአካል ጉዳተኛ ጂፒዩዎችን ሊያመለክት ይችላል።
    ዳሽቦርዶች.
- CI ምግብ (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {ሲፒዩ፣ ብረት}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> ድርብ.
- ላኪዎች ከ Rust benchmarks ወይም CI runs (ለምሳሌ፣ መሮጥ) መለኪያዎችን ማግኘት አለባቸው።
  ሜታል/ሲፒዩ ማይክሮቤንች እንደ `ci/xcode-swift-parity` አካል)።### የማዋቀር ቁልፎች እና ነባሪዎች (WP6-C)
- `AccelerationConfig` ነባሪዎች፡ `enable_metal = true` በማክሮስ ግንባታዎች ላይ፣ `enable_cuda = true` የCUDA ባህሪው ሲጠናቀር፣ `max_gpus = None` (ካፕ የለውም)። የስዊፍት `AccelerationSettings` መጠቅለያ በ`connect_norito_set_acceleration_config` በኩል ተመሳሳይ ነባሪዎችን ይወርሳል።
- Norito Merkle ሂዩሪስቲክስ (ጂፒዩ vs ሲፒዩ)፡ `merkle_min_leaves_gpu = 8192` ≥8192 ቅጠሎች ላሏቸው ዛፎች ጂፒዩ ማሽኮርመም ያስችላል። የኋለኛው ይሽራል (`merkle_min_leaves_metal`፣ `merkle_min_leaves_cuda`) በነባሪነት ለተመሳሳይ ገደብ በግልፅ ካልተዘጋጀ በስተቀር።
- የሲፒዩ ምርጫ ሂዩሪስቲክስ (SHA2 ISA በአሁኑ): በሁለቱም AArch64 (ARMv8 SHA2) እና x86/x86_64 (SHA-NI) የሲፒዩ መንገድ እስከ `prefer_cpu_sha2_max_leaves_* = 32_768` ቅጠሎች ድረስ ተመራጭ ሆኖ ይቆያል። ከዚያ በላይ የጂፒዩ ገደብ ተፈጻሚ ይሆናል። እነዚህ እሴቶች በ`AccelerationConfig` በኩል የሚዋቀሩ ናቸው እና መስተካከል ያለባቸው በቤንችማርክ ማስረጃ ብቻ ነው።

## የሙከራ ስልት

1. **የአሃድ እኩልነት ሙከራዎች (ዝገት)**፡- የብረት ከርነሎች ከሲፒዩ ውፅዓቶች ጋር እንደሚዛመዱ ያረጋግጡ።
   የሚወስኑ ቬክተሮች; በ `cargo test -p ivm --features metal` ስር ያሂዱ።
   `crates/fastpq_prover/src/metal.rs` አሁን የማክሮስ-ብቻ ተመሳሳይነት ፈተናዎችን ይልካል።
   የአካል ብቃት እንቅስቃሴ FFT/IFFT/LDE እና Poseidon ከስክላር ማመሳከሪያው ጋር።
2. **ፈጣን የጭስ ማሰሪያ**፡- CPU vs Metal ለማስፈጸም የIOS6 የሙከራ ሯጭን ዘርጋ
   በሁለቱም emulators እና StrongBox መሳሪያዎች ላይ ኢንኮዲንግ (Merkle/CRC64); አወዳድር
   ውጤቶች እና የምዝግብ ማስታወሻ እኩልነት ሁኔታ.
3. **CI**፡ አዘምን `norito_bridge_ios.yml` (ቀድሞውንም `make swift-ci` ይደውላል) ለመግፋት
   የፍጥነት መለኪያዎች ወደ ቅርሶች; ሩጫው Buildkiteን ያረጋግጣል
   የመታጠቂያ ለውጦችን ከማተምዎ በፊት `ci/xcframework-smoke:<lane>:device_tag` ሜታዳታ፣
   እና እኩልነት/ቤንችማርክ ተንሸራታች ላይ ያለውን መስመር ወድቋል።
4. ** ዳሽቦርዶች *** አዳዲስ መስኮች አሁን በ CLI ውፅዓት ውስጥ ይሰጣሉ። ላኪዎች መመረታቸውን ያረጋግጡ
   ዳሽቦርዶችን በቀጥታ ከመገልበጥዎ በፊት ውሂብ።

## WP2-ኤ ሜታል ሻደር ፕላን (Poseidon Pipelines)

የመጀመሪያው የWP2 ምእራፍ ለPoseidon Metal kernels የእቅድ ስራን ይሸፍናል።
የCUDA ትግበራን የሚያንፀባርቅ። እቅዱ ጥረቱን ወደ ከርነሎች ይከፍላል ፣
የአስተናጋጅ መርሐግብር፣ እና የተጋራ የማያቋርጥ ዝግጅት በኋላ ሥራ ላይ ብቻ እንዲያተኩር
ትግበራ እና ሙከራ.

### የከርነል ስፋት

1. `poseidon_permute`: permutes `state_count` ነጻ ግዛቶች. እያንዳንዱ ክር
   የ `STATE_CHUNK` (4 ግዛቶች) ባለቤት ሲሆን ሁሉንም የ`TOTAL_ROUNDS` ድግግሞሾችን በመጠቀም ይሰራል
   በክርክር ቡድን የተጋሩ ክብ ቋሚዎች በመላክ ጊዜ ተዘጋጅተዋል።
2. `poseidon_hash_columns`፡ ስፓርስ `PoseidonColumnSlice` ካታሎግ እና
   የእያንዳንዱን አምድ ለመርክል ተስማሚ ሃሽንግ ያከናውናል (ከሲፒዩ ጋር የሚዛመድ
   `PoseidonColumnBatch` አቀማመጥ). ተመሳሳይ የፈትል ቡድን ቋሚ ቋት ይጠቀማል
   እንደ permute kernel ግን በ `(states_per_lane * block_count)` ላይ ቀለበቶች
   ከርነሉ የወረፋ ማስገባቶችን እንዲያስተካክል ያስወጣል።
3. `poseidon_trace_fused`፡ ለክትትል ጠረጴዛው የወላጅ/የቅጠል ውህዶችን ያሰላል።
   በአንድ ማለፊያ. የተዋሃደ ከርነል `PoseidonFusedArgs` ይበላል ስለዚህ አስተናጋጁ
   ተላላፊ ያልሆኑ ክልሎችን እና `leaf_offset`/`parent_offset`ን መግለጽ ይችላል፣ እና
   ሁሉንም ክብ/ኤምዲኤስ ሠንጠረዦችን ከሌሎች ከርነሎች ጋር ይጋራል።

### የትእዛዝ መርሐግብር እና የአስተናጋጅ ኮንትራቶች- እያንዳንዱ የከርነል መላኪያ በ`MetalPipelines::command_queue` በኩል ያልፋል
  አስማሚውን መርሐግብር (ዒላማ ~2 ሚሴ) እና የወረፋ ደጋፊ-ውጭ መቆጣጠሪያዎችን ያስፈጽማል
  በ `FASTPQ_METAL_QUEUE_FANOUT` እና ተጋልጧል
  `FASTPQ_METAL_COLUMN_THRESHOLD`. በ `with_metal_state` ውስጥ ያለው የማሞቂያ መንገድ
  ሦስቱንም የፖሲዶን ከርነሎች ከፊት ለፊት ያጠናቅራል ስለዚህ የመጀመሪያው መላክ አይሰራም
  የቧንቧ መስመር መፈጠር ቅጣትን ይክፈሉ.
- የክር ቡድን መጠን ነባሩን የብረታ ብረት FFT/LDE ነባሪዎች ያንጸባርቃል፡ ዒላማው ነው።
  በአንድ ግቤት 8,192 ክሮች በቡድን 256 ክሮች ጠንካራ ካፕ። የ
  አስተናጋጁ ዝቅተኛ ኃይል ላላቸው መሳሪያዎች የ `states_per_lane` ማባዣን ሊቀንስ ይችላል።
  አካባቢን መደወል ይሻራል (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  በ WP2-B ውስጥ መጨመር) የሻደር አመክንዮ ሳይሻሻል.
- የአምድ አቀማመጥ አስቀድሞ FFT ጥቅም ላይ የዋለውን ተመሳሳይ ባለ ሁለት-ማቆያ ገንዳ ይከተላል
  የቧንቧ መስመሮች. የፖሲዶን አስኳሎች ጥሬ ጠቋሚዎችን ወደዚያ የማዘጋጃ ቋት ይቀበላሉ።
  እና ዓለም አቀፋዊ ክምር ምደባዎችን በጭራሽ አይንኩ ፣ ይህም የማስታወስ-ቆራጥነትን ይጠብቃል።
  ከ CUDA አስተናጋጅ ጋር የተስተካከለ።

### የተጋሩ ቋሚዎች

- የተገለጸው `PoseidonSnapshot` አንጸባራቂ
  `docs/source/fastpq/poseidon_metal_shared_constants.md` አሁን ቀኖናዊ ነው።
  ለክብ ቋሚዎች እና ለኤምዲኤስ ማትሪክስ ምንጭ. ሁለቱም ሜታል (`poseidon2.metal`)
  እና CUDA (`fastpq_cuda.cu`) አስኳሎች እንደገና መፈጠር አለባቸው አንጸባራቂው
  ለውጦች.
- WP2-B አንጸባራቂውን በሂደት ጊዜ የሚያነብ ትንሽ አስተናጋጅ ጫኝ ይጨምራል
  SHA-256ን ወደ ቴሌሜትሪ (`acceleration.poseidon_constants_sha`) ያመነጫል።
  የፓርቲ ዳሽቦርዶች የሻደር ቋሚዎች ከታተሙት ጋር እንደሚዛመዱ ማረጋገጥ ይችላሉ።
  ቅጽበታዊ ገጽ እይታ
- በማሞቅ ጊዜ የ `TOTAL_ROUNDS x STATE_WIDTH` ቋሚዎችን ወደ ሀ
  `MTLBuffer` እና በመሳሪያ አንድ ጊዜ ይስቀሉት። እያንዳንዱ ከርነል ውሂቡን ይገለበጣል
  ቁርጥራጮቹን ከማስኬዱ በፊት ወደ ክር ቡድን ማህደረ ትውስታ ውስጥ መግባት ፣ ይህም ቆራጥነትን ያረጋግጣል
  ብዙ የትዕዛዝ ማቋረጫዎች በበረራ ውስጥ ሲሄዱ እንኳን ማዘዝ።

### የማረጋገጫ መንጠቆዎች

- የክፍል ሙከራዎች (`cargo test -p fastpq_prover --features fastpq-gpu`) ያድጋሉ።
  የተካተቱትን የሻደር ቋሚዎች ያሸበረቀ እና ከነሱ ጋር የሚያነጻጽር ማረጋገጫ
  የጂፒዩ መግጠሚያ ስብስብን ከመተግበሩ በፊት የማሳያውን SHA።
- አሁን ያለው የከርነል ስታቲስቲክስ መቀያየር (`FASTPQ_METAL_TRACE_DISPATCH`፣
  `FASTPQ_METAL_QUEUE_FANOUT`፣ ወረፋ ጥልቀት ቴሌሜትሪ) አስፈላጊ ማስረጃ ሆነ
  ለ WP2 መውጫ፡ እያንዳንዱ የፈተና ሩጫ መርሐግብር አውጪው ፈጽሞ እንደማይጥስ ማረጋገጥ አለበት።
  የተዋቀረ ደጋፊ-ውጭ እና የተዋሃደ መከታተያ ከርነል ወረፋውን ከ በታች ያቆየዋል።
  የሚለምደዉ መስኮት.
- Swift XCFramework የጢስ ማውጫ እና የ Rust benchmark ሯጮች ይጀምራሉ
  `acceleration.poseidon.permute_p90_ms{cpu,metal}` ወደ ውጭ መላክ ስለዚህ WP2-D ቻርት ማድረግ ይችላል።
  አዲስ የቴሌሜትሪ ምግቦች እንደገና ሳይፈጠሩ ሜታል-ቪኤስ-ሲፒዩ ዴልታዎች።

## WP2-B ፖሲዶን አንጸባራቂ ጫኚ እና የራስ-ሙከራ ተመሳሳይነት- `fastpq_prover::poseidon_manifest()` አሁን አካትቶ ይተነትናል።
  `artifacts/offline_poseidon/constants.ron`፣ SHA-256 ያሰላል
  (`poseidon_manifest_sha256()`)፣ እና በሲፒዩ ላይ ያለውን ቅጽበታዊ ገጽ እይታ ያረጋግጣል
  ማንኛውም የጂፒዩ ሥራ ከመጀመሩ በፊት የፖሲዶን ጠረጴዛዎች። `build_metal_context()` መዝገቦች
  የቴሌሜትሪ ላኪዎች ማተም እንዲችሉ በማሞቅ ጊዜ መፈጨት
  `acceleration.poseidon_constants_sha`.
- አንጸባራቂው ተንታኝ ያልተዛመደውን ስፋት/ተመን/ክብ-ቁጥር ቱፕልስ እና ውድቅ ያደርጋል
  አንጸባራቂው MDS ማትሪክስ ከስካላር አተገባበር ጋር እኩል መሆኑን ያረጋግጣል፣ ይከላከላል
  ቀኖናዊ ጠረጴዛዎች ሲታደሱ ጸጥ ያለ መንሳፈፍ።
- ታክሏል `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`, ይህም
  በ `poseidon2.metal` ውስጥ የተካተቱትን የፖሲዶን ጠረጴዛዎች ይተነትናል እና
  `fastpq_cuda.cu` እና ሁለቱም አስኳሎች በትክክል አንድ አይነት መሆናቸውን አስረግጠው ተናግረዋል
  ቋሚዎች እንደ አንጸባራቂ. የሆነ ሰው shader/CUDAን ካስተካክል CI አሁን አይሳካም።
  ቀኖናዊ አንጸባራቂውን እንደገና ሳያሳድጉ ፋይሎች።
- የወደፊት ተመሳሳይነት መንጠቆዎች (WP2-C/D) ደረጃውን ለመደርደር `poseidon_manifest()` ን እንደገና መጠቀም ይችላሉ
  ቋሚ ቋሚዎችን ወደ ጂፒዩ ቋት እና የምግብ መፍጫውን በNorito በኩል ለማጋለጥ
  ቴሌሜትሪ ምግቦች.

## WP2-C BN254 የብረት ቱቦዎች እና የመመሳሰል ሙከራዎች- ** ወሰን እና ክፍተት፡** አስተናጋጅ ላኪዎች፣ እኩልነት መታጠቂያዎች እና `bn254_status()` ቀጥታ ስርጭት ላይ ናቸው፣ እና `crates/fastpq_prover/metal/kernels/bn254.metal` አሁን የሞንትጎመሪ ፕሪሚቲቭስ እና ከክር ቡድን ጋር የተመሳሰለ FFT/LDE loops ተግባራዊ ያደርጋል። እያንዳንዱ መላኪያ አንድ ሙሉ አምድ በአንድ ክር ቡድን ውስጥ በየደረጃው መሰናክሎች ያካሂዳል፣ ስለዚህ ከርነሎች የተደረደሩትን መገለጫዎች በትይዩ ይለማመዳሉ። ቴሌሜትሪ አሁን ባለገመድ ሆኗል እና መርሐግብር ሰጪዎች የተከበሩ ናቸው ስለዚህ ለጎልድሎክስ ከርነሎች በምንጠቀምበት ተመሳሳይ ማስረጃ ነባሪውን መልቀቅ እንችላለን።
- ** የከርነል መመዘኛዎች፡** ✅ የተደረደሩትን ትዊድል/ኮሴት መገለጫዎች እንደገና ተጠቀም፣ ግብዓቶችን/ውጤቶችን አንድ ጊዜ ቀይር፣ እና ሁሉንም የራዲክስ-2 ደረጃዎች በእያንዳንዱ አምድ ክር ቡድን ውስጥ አስፈጽም ስለዚህ ባለብዙ መላኪያ ማመሳሰል አያስፈልገንም። የሞንትጎመሪ ረዳቶች በFFT/LDE መካከል እንደተጋሩ ይቆያሉ ስለዚህ የ loop ጂኦሜትሪ ብቻ ተቀየረ።
- ** አስተናጋጅ የወልና:** ✅ `crates/fastpq_prover/src/metal.rs` ደረጃዎች ቀኖናዊ እጅና እግር, ዜሮ-ሙላ LDE ቋት, አንድ ነጠላ ክር ቡድን በአንድ አምድ ይመርጣል, እና gating `bn254_status()` ያጋልጣል. ለቴሌሜትሪ ምንም ተጨማሪ የአስተናጋጅ ለውጦች አያስፈልጉም።
- **ጠባቂዎችን ይገንቡ፡** `fastpq.metallib` የታሸገውን እንክርዳድ ይልካል። ማንኛውም የወደፊት ተስፋዎች የጊዜ መቀየሪያዎችን ከማጠናቀር ይልቅ ከቴሌሜትሪ/የባህሪ በሮች በስተጀርባ ይቆያሉ።
- ** የፓሪቲ መጫዎቻዎች:** ✅ `bn254_parity` ሙከራዎች የጂፒዩ FFT/LDE ውጤቶችን ከሲፒዩ መጫዎቻዎች ጋር ማወዳደር ይቀጥላሉ እና አሁን በቀጥታ በብረት ሃርድዌር ይሰራሉ። አዲስ የከርነል ኮድ ዱካዎች ከታዩ የተበላሹ - ገላጭ ሙከራዎችን ያስታውሱ።
- **ቴሌሜትሪ እና መመዘኛዎች፡** `fastpq_metal_bench` አሁን ይለቃል፡
  - አንድ `bn254_dispatch` ብሎክ በእያንዳንዱ-ተላኪ ክር ቡድን ስፋቶችን ፣ ሎጂካዊ ክር ብዛትን እና የቧንቧ መስመር ገደቦችን ለ FFT/LDE ነጠላ-አምድ ስብስቦች ማጠቃለያ። እና
  - ለሲፒዩ መነሻ መስመር `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` የሚመዘግብ የ`bn254_metrics` ብሎክ እና የየትኛው የጂፒዩ ጀርባ ሩጫ።
  የቤንችማርክ መጠቅለያው ሁለቱንም ካርታዎች ወደ እያንዳንዱ የታሸጉ ቅርሶች ስለሚገለብጥ WP2-D ዳሽቦርዶች የጥሬ ኦፕሬሽን ድርድርን ሳይቀይሩ የተሰየሙትን መዘግየት/ጂኦሜትሪ ያስገባሉ። `FASTPQ_METAL_THREADGROUP` አሁን ለ BN254 FFT/LDE መላኪያዎች ይተገበራል፣ ይህም ቋጠሮው ለ perf እንዲውል ያደርገዋል። ጠረገ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【ስክሪፕቶች/fastpq/wrap:_1

## ክፍት ጥያቄዎች (ግንቦት 2027 ተፈትቷል)1. ** የብረታ ብረት ማጽጃ:** `warm_up_metal()` ክር-አካባቢያዊ እንደገና ይጠቀማል.
   `OnceCell` እና አሁን idempotence/regression ሙከራዎች አሉት
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`)፣ ስለዚህ የመተግበሪያ የሕይወት ዑደት ሽግግሮች
   ያለማፍሰስ ወይም ሁለቴ ሳይጀምር የማሞቂያውን መንገድ በደህና መጥራት ይችላል።
2. **የቤንችማርክ መነሻ መስመሮች፡** የብረት መስመሮች ከሲፒዩ 20% ውስጥ መቆየት አለባቸው
   የ FFT/IFFT/LDE መነሻ መስመር እና በ15% ውስጥ ለPoseidon CRC/Merkle ረዳቶች;
   `acceleration.*_perf_delta_pct > 0.20` (ወይም ሲጎድል) ማንቂያው መቀጣጠል አለበት
   በሞባይል እኩልነት ምግብ ውስጥ. በ20k የክትትል ጥቅል ውስጥ የIFFT ድግግሞሾች ተስተውለዋል።
   አሁን በWP2-D ላይ በተጠቀሰው የወረፋ መሻር መጠገን ተዘግተዋል።
3. ** StrongBox fallback:** ስዊፍት የአንድሮይድ መመለሻ ደብተር በ
   በድጋፍ runbook ውስጥ የማረጋገጫ ውድቀቶችን መመዝገብ
   (`docs/source/sdk/swift/support_playbook.md`) እና በራስ ሰር ወደ መቀየር
   በ HKDF የተደገፈ የሶፍትዌር መንገድ ከኦዲት ምዝገባ ጋር; እኩልነት ቬክተሮች
   በነባሩ የOA መጫዎቻዎች በኩል እንደተጋሩ ይቆዩ።
4. **የቴሌሜትሪ ማከማቻ፡** የፍጥነት ቀረጻዎች እና የመሳሪያ ገንዳ ማረጋገጫዎች ናቸው።
   በ`configs/swift/` (ለምሳሌ፦
   `configs/swift/xcframework_device_pool_snapshot.json`)፣ እና ላኪዎች
   ተመሳሳይ አቀማመጥ ማንጸባረቅ አለበት (`artifacts/swift/telemetry/acceleration/*.json`
   ወይም `.prom`) ስለዚህ የBuildkite ማብራሪያዎች እና የፖርታል ዳሽቦርዶች
   ያለ አድሆክ መቧጨር ይመገባል።

## ቀጣይ ደረጃዎች (የካቲት 2026)

- [x] ዝገት፡ የመሬት ሜታል አስተናጋጅ ውህደት (`crates/fastpq_prover/src/metal.rs`) እና
      የከርነል በይነገጽ ለስዊፍት ያጋልጣል; doc እጅ-ጠፍቷል ከጎን ተከታትሏል
      የስዊፍት ድልድይ ማስታወሻዎች።
- [x] ስዊፍት፡ የኤስዲኬ-ደረጃ ማጣደፍ ቅንብሮችን አጋልጥ (ጥር 2026 ተከናውኗል)።
- [x] ቴሌሜትሪ፡ `scripts/acceleration/export_prometheus.py` አሁን ይቀየራል።
      `cargo xtask acceleration-state --format json` ወደ Prometheus ውፅዓት
      የጽሑፍ ፋይል (ከአማራጭ `--instance` መለያ ጋር) ስለዚህ CI ሩጫዎች ጂፒዩ/ሲፒዩን ማያያዝ ይችላሉ።
      ማስቻል፣ ገደቦች እና እኩልነት/ምክንያቶችን ወደ ጽሑፍ ፋይል በቀጥታ ማሰናከል
      ሰብሳቢዎች ያለ መፋቅ.
- [x] ስዊፍት QA: `scripts/acceleration/acceleration_matrix.py` ብዙዎችን ያጠቃልላል
      acceleration-state በመሳሪያ ቁልፍ ወደ JSON ወይም Markdown ሰንጠረዦች ይቀርጻል።
      መለያ፣ የጭስ ማሰሪያውን የሚወስን የ"CPU vs Metal/CUDA" ማትሪክስ
      ከናሙና-መተግበሪያው ጭስ ጋር አብሮ ለመስቀል። የ Markdown ውፅዓት ያንፀባርቃል
      ዳሽቦርዶች ተመሳሳዩን አርቲፊኬት ወደ ውስጥ ማስገባት እንዲችሉ Buildkite የማስረጃ ቅርጸት።
- [x] `irohad` ወረፋውን/ዜሮ ሙላ ላኪዎችን ስለሚልክ እና ሁኔታን ያዘምኑ።
      የ env/config የማረጋገጫ ሙከራዎች የብረት ወረፋውን ይሸፍናሉ፣ ስለዚህ WP2-D
      ቴሌሜትሪ + ማሰሪያዎች የቀጥታ ማስረጃዎች ተያይዘዋል።【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546

ቴሌሜትሪ/የመላክ አጋዥ ትዕዛዞች፡-

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D ልቀት ቤንችማርክ እና ማያያዣ ማስታወሻዎች- **20k-ረድፍ ልቀት ቀረጻ:** በ macOS14 ላይ ትኩስ ሜታል vs ሲፒዩ መለኪያ ተመዝግቧል
  (ክንድ64፣ የሌይን-ሚዛን መለኪያዎች፣ የታሸገ ባለ 32,768-ረድፍ ዱካ፣ ሁለት አምድ ባች) እና
  የJSON ቅርቅቡን ወደ `fastpq_metal_bench_20k_release_macos14_arm64.json` ፈትሽ።
  ማመሳከሪያው የየስራ ጊዜዎችን እና የPoseidon ማይክሮቤንች ማስረጃዎችን ወደ ውጭ ይልካል።
  WP2-D ከአዲሱ የብረታ ብረት ወረፋ ሂዩሪስቲክስ ጋር የተሳሰረ የGA-ጥራት ያለው ቅርስ አለው። ርዕስ
  ዴልታስ (ሙሉ ሠንጠረዥ በ`docs/source/benchmarks.md` ውስጥ ይኖራል)

  | ኦፕሬሽን | ሲፒዩ አማካኝ (ms) | ሜታል አማካኝ (ኤምኤስ) | ፍጥነት |
  |-------------
  | FFT (32,768 ግብዓቶች) | 12.741 | 10.963 | 1.16× |
  | IFFT (32,768 ግብዓቶች) | 17.499 | 25.688 | 0.68× *(regression: queue fan-out throttled to keep determinism; የክትትል ማስተካከያ ያስፈልገዋል)* |
  | LDE (262,144 ግብዓቶች) | 68.389 | 65.701 | 1.04× |
  | የፖሲዶን ሃሽ አምዶች (524,288 ግብዓቶች) | 1,728.835 | 1,447.076 | 1.19× |

  እያንዳንዱ የቀረጻ ምዝግብ ማስታወሻዎች `zero_fill` ጊዜ (9.651ms ለ 33,554,432 ባይት) እና
  `poseidon_microbench` ግቤቶች (ነባሪ መስመር 596.229ms vs scalar 656.251ms፣
  1.10× የፍጥነት ፍጥነት) ስለዚህ ዳሽቦርድ ሸማቾች ከወረፋው ግፊት ጋር ሊለያዩ ይችላሉ።
  ዋና ስራዎች.
- ** ማሰሪያዎች/ሰነዶች ማቋረጫ አገናኝ፡** `docs/source/benchmarks.md` አሁን የሚያመለክተው
  JSON ን እና አራቢ ትእዛዝን ይልቀቁ፣ የብረታ ብረት ወረፋ መሻሮች ተረጋግጠዋል
  በ`iroha_config` env/ማሳያ ሙከራዎች፣ እና `irohad` በቀጥታ ያትማል።
  `fastpq_metal_queue_*` መለኪያዎች ስለዚህ ዳሽቦርዶች ባንዲራ IFFT ያለ regressions
  አድ-ሆክ ሎግ መቧጨር። የስዊፍት `AccelerationSettings.runtimeState` ያጋልጣል
  ተመሳሳይ የቴሌሜትሪ ክፍያ ጭነት በJSON ጥቅል ውስጥ ተልኳል፣ WP2-Dን ይዘጋል።
  አስገዳጅ/የሰነድ ክፍተት ሊባዛ ከሚችል ተቀባይነት መነሻ መስመር ጋር።【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT ወረፋ ማስተካከል:** የተገላቢጦሽ የኤፍኤፍቲ ስብስቦች አሁን ባለብዙ ወረፋ መላክን በማንኛውም ጊዜ ይዝለሉ።
  የስራ ጫናው ከደጋፊ መውጣት ገደብ ጋር እምብዛም አያሟላም (በሌይን-ሚዛናዊ ላይ 16 አምዶች
  መገለጫ)፣ በማስቀመጥ ላይ እያለ ከላይ የተጠራውን Metal-vs-CPU regression በማስወገድ
  ለ FFT/LDE/Poseidon ባለ ብዙ ወረፋ መንገድ ላይ ትልቅ-አምድ የስራ ጫናዎች።