---
lang: am
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## ማጣደፍ እና Norito የሂዩሪስቲክስ ማጣቀሻ

በ `iroha_config` ክሮች ውስጥ ያለው የ `[accel]` እገዳ
`crates/irohad/src/main.rs:1895` ወደ `ivm::set_acceleration_config`። እያንዳንዱ አስተናጋጅ
ቪኤምን ከመፍጠን በፊት ተመሳሳይ ቁልፎችን ይተገብራል፣ ስለዚህ ኦፕሬተሮች በትክክል መወሰን ይችላሉ።
scalar/SIMD መውደቅ ሲኖር የትኞቹ የጂፒዩ መደገፊያዎች እንደሚፈቀዱ ይምረጡ።
ስዊፍት፣ አንድሮይድ እና ፓይዘን ማሰሪያዎች በድልድዩ ንብርብር በኩል አንድ አይነት አንጸባራቂ ይጫናሉ፣ ስለዚህ
እነዚህን ነባሪዎች መመዝገብ WP6-Cን በሃርድዌር-ማጣደፍ የኋላ መዝገብ ውስጥ ያቆማል።

### `accel` (የሃርድዌር ማጣደፍ)

ከታች ያለው ሠንጠረዥ `docs/source/references/peer.template.toml` እና የ
`iroha_config::parameters::user::Acceleration` ፍቺ፣ አካባቢን ማጋለጥ
እያንዳንዱን ቁልፍ የሚሽር ተለዋዋጭ።

| ቁልፍ | Env var | ነባሪ | መግለጫ |
|-------------|----|------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX አፈፃፀምን ያነቃል። `false` በሚሆንበት ጊዜ ቪኤም ለቬክተር ኦፕስ እና ለ Merkle hashing ቆራጥነት የተመጣጠነ ቀረጻዎችን ለማቃለል scalar backends ያስገድዳል። |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | የCUDA ጀርባን ያነቃል እና ሲጠናቀር እና የአሂድ ጊዜ ሁሉንም ወርቃማ-ቬክተር ቼኮችን አልፏል። |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | በ macOS ግንቦች ላይ የብረታ ብረት ጀርባን ያነቃል። እውነት ቢሆንም እንኳ፣ የብረታ ብረት ራስን መፈተሽ የተመጣጣኝ አለመጣጣም ከተከሰተ በሂደት ላይ ያለውን የጀርባውን መጨረሻ ሊያሰናክለው ይችላል። |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (ራስ-ሰር) | የሩጫ ጊዜ ማስጀመሪያው ስንት አካላዊ ጂፒዩዎችን ያሳያል። `0` ማለት "ተዛማጅ ሃርድዌር ማራገቢያ" እና በ `GpuManager` ተጣብቋል። |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | ወደ ጂፒዩ ከመጫንዎ በፊት ከመርክሌ ቅጠል በፊት የሚፈለጉት አነስተኛ ቅጠሎች። ከዚህ ገደብ በታች ያሉት ዋጋዎች PCIe ከአናት (`crates/ivm/src/byte_merkle_tree.rs:49`) ለማስቀረት በሲፒዩ ላይ ማሽቆልቆላቸውን ይቀጥላሉ. |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (ዓለም አቀፍ ውርስ) | ለጂፒዩ ገደብ ብረት-ተኮር መሻር። `0` ሲኖር ሜታል `merkle_min_leaves_gpu` ይወርሳል። |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (ዓለም አቀፍ ውርስ) | ለጂፒዩ ገደብ CUDA-ተኮር መሻር። `0`፣ CUDA `merkle_min_leaves_gpu` ይወርሳል። |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32768 በውስጥ) | የ ARMv8 SHA-2 መመሪያዎች በጂፒዩ ሃሺንግ ላይ የሚያሸንፉበትን የዛፉን መጠን ይሸፍናል። `0` የተቀናበረውን የ`32_768` ቅጠሎች (`crates/ivm/src/byte_merkle_tree.rs:59`) ነባሪ ያቆያል። |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (32768 በውስጥ) | ከላይ ካለው ጋር ተመሳሳይ ነገር ግን SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) በመጠቀም ለ x86/x86_64 አስተናጋጆች። |

`enable_simd` በተጨማሪም RS16 erasure codeing (Torii DA ingest + tooling) ይቆጣጠራል። እሱን አሰናክል
ውጤቶቹ በሃርድዌር ላይ እንዲወስኑ ሲያደርጉ scalar perity ትውልድን ያስገድዱ።

የምሳሌ ውቅር፡

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

የመጨረሻዎቹ አምስት ቁልፎች ዜሮ እሴቶች "የተጠናቀረ ነባሪ አቆይ" ማለት ነው። አስተናጋጆች መሆን የለባቸውም
የሚጋጩ መሻሮችን ያቀናብሩ (ለምሳሌ፡ CUDAን በማሰናከል CUDA-ብቻ ገደቦችን በማስገደድ)
አለበለዚያ ጥያቄው ችላ ይባላል እና የጀርባው አካል የአለም አቀፍ ፖሊሲን መከተሉን ይቀጥላል.

### የሩጫ ጊዜ ሁኔታን በመፈተሽ ላይየተተገበረውን ቅጽበተ ፎቶ ለማየት `cargo xtask acceleration-state [--format table|json]` ያሂዱ
ከMetal/CUDA የሩጫ ጊዜ የጤና ቢት ጋር ውቅር። ትዕዛዙ ይጎትታል
የአሁኑ `ivm::acceleration_config`፣ የተመጣጣኝነት ሁኔታ እና ተለጣፊ የስህተት ሕብረቁምፊዎች (ከሆነ
backend ተሰናክሏል) ስለዚህ ክዋኔዎች ውጤቱን በቀጥታ ወደ እኩልነት ይመገባሉ።
ዳሽቦርዶች ወይም የክስተት ግምገማዎች።

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

ቅጽበተ-ፎቶው በራስ-ሰር (JSON) ወደ ውስጥ መግባት ሲፈልግ `--format json` ይጠቀሙ
በሰንጠረዡ ውስጥ የሚታዩትን ተመሳሳይ መስኮች ይዟል).

`acceleration_runtime_errors()` ለምን SIMD ወደ scalar ተመልሶ እንደወደቀ ጠራ።
`disabled by config`፣ `forced scalar override`፣ `simd unsupported on hardware`፣ ወይም
`simd unavailable at runtime` ማግኘቱ ሲሳካ ግን አፈጻጸም አሁንም ይሰራል
ያለ ቬክተሮች. መሻሩን ማጽዳት ወይም መመሪያውን እንደገና ማንቃት መልእክቱን ይጥላል
SIMD በሚደግፉ አስተናጋጆች ላይ።

### የፓሪቲ ቼኮች

ወሳኙን ውጤት ለማረጋገጥ `AccelerationConfig`ን በሲፒዩ-ብቻ እና በ accel-on መካከል ገልብጥ።
የ `poseidon_instructions_match_across_acceleration_configs` regression ያካሂዳል
Poseidon2/6 ሁለት ጊዜ ኦፕኮዶችን ያደርጋል—መጀመሪያ በ `enable_cuda`/`enable_metal` ወደ `false` ተቀናብሯል፣ከዚያም
ከሁለቱም የነቃ - እና ጂፒዩዎች በሚገኙበት ጊዜ ተመሳሳይ ውጤቶችን እና የCUDA እኩልነትን ያረጋግጣል።【crates/ivm/tests/crypto.rs:100】
የኋለኛውን ለመቅዳት `acceleration_runtime_status()`ን ከሩጫው ጋር ይያዙ
በቤተ ሙከራ ምዝግብ ማስታወሻዎች ውስጥ ተዋቅረዋል/ ይገኛሉ።

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### የጂፒዩ ነባሪዎች እና ሂዩሪስቲክስ

`MerkleTree` ጂፒዩ ማውረድ በነባሪ በ `8192` ይጀምራል እና ሲፒዩ SHA-2
ምርጫ ገደቦች በእያንዳንዱ አርክቴክቸር `32_768` ቅጠሎች ላይ ይቆያሉ። መቼ CUDA ወይም
ሜታል በጤና ፍተሻዎች ይገኛል ወይም ተሰናክሏል፣ VM በራስ-ሰር ይወድቃል
ወደ SIMD/scalar hashing ይመለሱ እና ከላይ ያሉት ቁጥሮች በቆራጥነት ላይ ተጽዕኖ አያሳርፉም።

`max_gpus` የመዋኛ ገንዳውን መጠን ወደ `GpuManager` ያቆማል። `max_gpus = 1` በማቀናበር ላይ
የብዝሃ-ጂፒዩ አስተናጋጆች ማፋጠንን በሚፈቅዱበት ጊዜ ቴሌሜትሪ ቀላል ያደርገዋል። ኦፕሬተሮች ይችላሉ።
የተቀሩትን መሳሪያዎች ለ FASTPQ ወይም CUDA Poseidon ስራዎች ለማስያዝ ይህንን ማብሪያ / ማጥፊያ ይጠቀሙ።

### ቀጣይ የፍጥነት ዒላማዎች እና በጀቶች

የቅርብ ጊዜው የ FastPQ ብረት ዱካ (`fastpq_metal_bench_20k_latest.json`፣ 32K ረድፎች × 16
አምዶች፣ 5 iters) የPoseidon አምድ ሃሽንግ የZK የስራ ጫናዎችን እንደሚቆጣጠር ያሳያል፡

- `poseidon_hash_columns`: ሲፒዩ አማካኝ **3.64s** vs.ጂፒዩ አማካኝ **3.55s** (1.03×)።
- `lde`: ሲፒዩ አማካኝ **1.75s** vs.ጂፒዩ አማካኝ **1.57s** (1.12×)።

IVM/Crypto በሚቀጥለው የአሲል መጥረግ እነዚህን ሁለት እንክብሎች ኢላማ ያደርጋል። የመነሻ በጀቶች፡-

- scalar/SIMD ን ከሲፒዩ በላይ ካለው ወይም በታች ያቆዩት እና ይያዙ
  `acceleration_runtime_status()` ከእያንዳንዱ ሩጫ ጎን ለጎን የብረታ ብረት/CUDA ተገኝነት
  በበጀት ቁጥሮች ተመዝግቧል.
- ዒላማ ≥1.3× የፍጥነት ፍጥነት ለ`poseidon_hash_columns` እና ≥1.2× ለ `lde` አንዴ የተስተካከለ ሜታል
  እና CUDA የከርነል መሬት፣ የውጤት ወይም የቴሌሜትሪ መለያዎችን ሳይቀይሩ።

የJSON ዱካውን እና `cargo xtask acceleration-state --format json` ቅጽበተ ፎቶን አያይዝ
ወደፊት ላብራቶሪ ይሰራል CI/regressions ሁለቱንም በጀቶችን እና ጤናን በሚደግፍበት ጊዜ ማረጋገጥ ይችላል።
ሲፒዩ-ብቻ ከ accel-on ሩጫዎች ጋር ማወዳደር።

### Norito ሂዩሪስቲክስ (የተጠናቀረ ጊዜ ነባሪዎች)የNorito አቀማመጥ እና መጭመቂያ ሂዩሪስቲክስ በ`crates/norito/src/core/heuristics.rs` ውስጥ ይኖራሉ
እና በእያንዳንዱ ሁለትዮሽ ውስጥ ይሰበሰባሉ. በማሽከርከር ጊዜ የሚዋቀሩ አይደሉም፣ ግን የሚያጋልጡ ናቸው።
ግብዓቶቹ ኤስዲኬ እና ኦፕሬተር ቡድኖች Norito አንዴ ጂፒዩ እንዴት እንደሚያሳዩ ለመተንበይ ይረዳል
መጭመቂያ አስኳሎች ነቅተዋል።
የስራ ቦታው አሁን Norito በነባሪ የነቃው የ `gpu-compression` ባህሪን ይገነባል።
ስለዚህ የጂፒዩ zstd ጀርባዎች ተሰብስበዋል; የአሂድ ጊዜ መገኘት አሁንም በሃርድዌር ላይ የተመሰረተ ነው,
የረዳት ቤተ-መጽሐፍት (`libgpuzstd_*`/`gpuzstd_cuda.dll`) እና `allow_gpu_compression`
አዋቅር ባንዲራ. የብረት ረዳትን በ `cargo build -p gpuzstd_metal --release` እና
በጫኚው መንገድ ላይ `libgpuzstd_metal.dylib` ያስቀምጡ. የአሁኑ ሜታል ረዳት ጂፒዩ ይሰራል
ግጥሚያ-ግኝት/ተከታታይ ማመንጨት እና የውስጠ-crate መወሰኛ zstd ፍሬም ይጠቀማል
ኢንኮደር (Huffman/FSE + ፍሬም ስብሰባ) በአስተናጋጁ ላይ; ዲኮድ የውስጠ-ክሬት ፍሬሙን ይጠቀማል
ላልተደገፉ ክፈፎች ከሲፒዩ zstd ውድቀት ጋር ዲኮደር የጂፒዩ ብሎክ መፍታት እስኪገባ ድረስ።

| መስክ | ነባሪ | ዓላማ |
|-------|---------|-----|
| `min_compress_bytes_cpu` | `256` ባይት | ከዚህ በታች፣ ከአቅም በላይ ጫናዎች ለማስቀረት zstdን ሙሉ በሙሉ ይዘላሉ። |
| `min_compress_bytes_gpu` | `1_048_576` ባይት (1ሚቢ) | `norito::core::hw::has_gpu_compression()` እውነት ሲሆን በዚህ ገደብ ወይም ከዚያ በላይ የሚጫኑ ጭነቶች ወደ ጂፒዩ zstd ይቀየራሉ። |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | የሲፒዩ መጭመቂያ ደረጃዎች ለ<32KiB እና ≥32KiB እንደቅደም ተከተላቸው። |
| `zstd_level_gpu` | `1` | የትዕዛዝ ወረፋዎችን በሚሞሉበት ጊዜ ዘግይቶ እንዲቆይ ለማድረግ ወግ አጥባቂ የጂፒዩ ደረጃ። |
| `large_threshold` | `32_768` ባይት | በ"ትንሽ" እና "ትልቅ" ሲፒዩ zstd ደረጃዎች መካከል ያለው የመጠን ወሰን። |
| `aos_ncb_small_n` | `64` ረድፎች | ከዚህ የረድፍ ቆጠራ በታች የሚለምደዉ ኢንኮደሮች ሁለቱንም የAoS እና የኤንሲቢ አቀማመጦችን ይመረምራሉ ትንሹን ክፍያ ለመምረጥ። |
| `combo_no_delta_small_n_if_empty` | `2` ረድፎች | 1-2 ረድፎች ባዶ ህዋሶችን ሲይዙ u32/id delta ኢንኮዲንግ ማንቃትን ይከለክላል። |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | ዴልታዎች ቢያንስ ሁለት ረድፎች ካሉ አንድ ጊዜ ብቻ ነው የሚገቡት። |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | ሁሉም የዴልታ ትራንስፎርሜሽን በነባሪነት ጥሩ ባህሪ ላላቸው ግብዓቶች ነቅተዋል። |
| `combo_enable_name_dict` | `true` | የተመታ ሬሾዎች የማህደረ ትውስታውን በላይ ሲያጸድቁ በአንድ-አምድ መዝገበ-ቃላት ይፈቅዳል። |
| `combo_dict_ratio_max` | `0.40` | ከ40% በላይ ረድፎች ሲለያዩ መዝገበ ቃላትን ያሰናክሉ። |
| `combo_dict_avg_len_min` | `8.0` | መዝገበ ቃላትን ከመገንባቱ በፊት አማካኝ የሕብረቁምፊ ርዝመት ≥8 ጠይቅ (አጭር ተለዋጭ ስሞች በውስጥ መስመር ይቆያሉ። |
| `combo_dict_max_entries` | `1024` | የተገደበ የማህደረ ትውስታ አጠቃቀምን ለማረጋገጥ በመዝገበ-ቃላት ግቤቶች ላይ ጠንካራ ሽፋን። |

እነዚህ ሂውሪስቲክስ በጂፒዩ የነቁ አስተናጋጆችን ከሲፒዩ-ብቻ አቻዎች ጋር ያቆያቸዋል፡ መራጩ
የሽቦ ቅርጸቱን የሚቀይር ውሳኔ በጭራሽ አያደርግም, እና ጣራዎቹ ተስተካክለዋል
በአንድ ልቀት. ፕሮፌሽናል ማድረግ የተሻሉ የመለያየት ነጥቦችን ሲያገኝ፣ Norito ያዘምናል
ቀኖናዊ `Heuristics::canonical` ትግበራ እና `docs/source/benchmarks.md` plus
`status.md` ለውጡን ከተቀየረው ማስረጃ ጋር ይመዘግባል።የጂፒዩ zstd አጋዥ በተመሳሳይ `min_compress_bytes_gpu` መቆራረጥን ያስፈጽማል
በቀጥታ ተጠርቷል (ለምሳሌ በ `norito::core::gpu_zstd::encode_all`) ፣ በጣም ትንሽ
የጂፒዩ መገኘት ምንም ይሁን ምን የክፍያ ጭነቶች ሁል ጊዜ በሲፒዩ መንገድ ላይ ይቆያሉ።

### መላ ፍለጋ እና እኩልነት ማረጋገጫ ዝርዝር

- ቅጽበታዊ የአሂድ ጊዜ ሁኔታ ከ`cargo xtask acceleration-state --format json` እና አቆይ
  ከማንኛውም ያልተሳኩ ምዝግብ ማስታወሻዎች ጋር የሚወጣው ውጤት; ሪፖርቱ የተዋቀሩ/የሚገኙ የኋላ መደገፊያዎችን ያሳያል
  ሲደመር እኩልነት/የመጨረሻ ስህተት ሕብረቁምፊዎች.
- መንሸራተትን ለማስቀረት የ accel parity regression በአካባቢው እንደገና ያስኪዱ፡
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (ሲፒዩ-ብቻ ከዚያም accel-on ይሰራል)። ለመሮጥ `acceleration_runtime_status()` ይመዝግቡ።
- የኋለኛ ክፍል ራስን መፈተሽ ካልተሳካ፣ መስቀለኛ መንገዱን በሲፒዩ-ብቻ ሁነታ ያቆዩት (`enable_metal =
  false`, `enable_cuda = false`) እና ከተያዘው ተመሳሳይ ውጤት ጋር አንድ ክስተት ይክፈቱ
  ጀርባውን ከማስገደድ ይልቅ. ውጤቶቹ በሁሉም ሁነታዎች የሚወሰኑ ሆነው መቆየት አለባቸው።
- ** CUDA እኩልነት ጭስ (ላብ NV ሃርድዌር): ** አሂድ
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  በsm_8x ሃርድዌር ላይ፣ `cargo xtask acceleration-state --format json`ን ያንሱ እና አያይዘው።
  የሁኔታ ቅጽበታዊ ገጽ እይታ (የጂፒዩ ሞዴል/ሹፌር ተካትቷል) ወደ ቤንችማርክ ቅርሶች።