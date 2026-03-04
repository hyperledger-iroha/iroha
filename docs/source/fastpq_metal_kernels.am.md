---
lang: am
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ ሜታል ከርነል Suite

የ Apple Silicon ደጋፊ እያንዳንዱን የያዘ አንድ `fastpq.metallib` ይልካል።
Metal Shading Language (MSL) በ prover የሚለማመዱ ከርነል. ይህ ማስታወሻ ያብራራል
የሚገኙትን የመግቢያ ነጥቦች፣ የክር ቡድናቸው ገደቦች እና ቆራጥነት
የጂፒዩ መንገዱን ከስክላር ውድቀት ጋር እንዲለዋወጥ የሚያደርግ ዋስትና።

ቀኖናዊው አተገባበር ስር ይኖራል
`crates/fastpq_prover/metal/kernels/` እና የተጠናቀረው በ
`crates/fastpq_prover/build.rs` `fastpq-gpu` በ macOS ላይ በነቃ ቁጥር።
የአሂድ ጊዜ ሜታዳታ (`metal_kernel_descriptors`) ከዚህ በታች ያለውን መረጃ ያንጸባርቃል
መመዘኛዎች እና ምርመራዎች ተመሳሳይ እውነታዎችን ሊያሳዩ ይችላሉ። በፕሮግራማዊ መንገድ።【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## የከርነል ክምችት| የመግቢያ ነጥብ | ኦፕሬሽን | የክር ቡድን ካፕ | የሰድር ደረጃ ካፕ | ማስታወሻ |
| -------- | -------- | ------------ | ------------- | --- |
| `fastpq_fft_columns` | በክትትል አምዶች ላይ FFT አስተላልፍ | 256 ክሮች | 32 ደረጃዎች | ለመጀመሪያዎቹ ደረጃዎች የጋራ ማህደረ ትውስታ ንጣፎችን ይጠቀማል እና እቅድ አውጪው የIFFT ሁነታን ሲጠይቅ የተገላቢጦሽ ልኬትን ይተገበራል።【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262
| `fastpq_fft_post_tiling` | የሰድር ጥልቀት ከተደረሰ በኋላ FFT/IFFT/LDE ያጠናቅቃል | 256 ክሮች | - | የቀሩትን ቢራቢሮዎች ከመሳሪያው ማህደረ ትውስታ በቀጥታ ያሰራጫል እና ወደ አስተናጋጁ ከመመለሱ በፊት የመጨረሻውን ኮሴት/ተገላቢጦሽ ሁኔታዎችን ይቆጣጠራል።
| `fastpq_lde_columns` | ዝቅተኛ-ዲግሪ ቅጥያ በአምዶች | 256 ክሮች | 32 ደረጃዎች | Coefficients ወደ የግምገማ ቋት ይገለበጣል፣ ከተዋቀረው ኮሴት ጋር የተደረደሩ ደረጃዎችን ያስፈጽማል፣ እና የመጨረሻውን ደረጃ ለ `fastpq_fft_post_tiling` ሲያስፈልግ ይተወዋል።
| `poseidon_trace_fused` | ሃሽ አምዶች እና ጥልቀት-1 ወላጆችን በአንድ ማለፊያ ያሰሉ | 256 ክሮች | - | ልክ እንደ `poseidon_hash_columns` ተመሳሳይ መምጠጥ/መምጠጥ ያካሂዳል፣ ቅጠሉ ተፈጭቶ በቀጥታ ወደ ውፅዓት ቋት ውስጥ ያከማቻል እና ወዲያውኑ እያንዳንዱን `(left,right)` ጥንድ በ `fastpq:v1:trace:node` ጎራ ስር በማጠፍ `(⌈columns / 2⌉)` ወላጆች ከቅጠሉ በኋላ ያርፋሉ። ጎዶሎ አምድ ቆጠራዎች የመጨረሻውን ቅጠል በመሣሪያው ላይ በማባዛት ተከታዩን ከርነል እና የሲፒዩ ውድቀትን ለመጀመሪያው Merkle ንብርብር ያስወግዳል።【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src/metal
| `poseidon_permute` | Poseidon2 permutation (STATE_WIDTH = 3) | 256 ክሮች | - | የክር ቡድኖች ክብ ቋሚዎችን/ኤምዲኤስ ረድፎችን በክር ቡድን ማህደረ ትውስታ ውስጥ መሸጎጥ፣ የኤምዲኤስ ረድፎችን በየክር መዝገቦች ገልብጠው፣ እና ሂደቱ በ4-ግዛት ክፍልፍሎች ይገለጻል ስለዚህ እያንዳንዱ ዙር ቋሚ ማምጣቱ ወደ ብዙ ግዛቶች እንደገና ጥቅም ላይ ይውላል። ዙሮቹ ሙሉ በሙሉ እንዳልተከፈቱ ይቆያሉ እና እያንዳንዱ መስመር አሁንም በበርካታ ግዛቶች ይራመዳል፣ ይህም በእያንዳንዱ መላኪያ ≥4096 ምክንያታዊ ክሮች ዋስትና ይሰጣል። `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` የማስጀመሪያውን ስፋት እና ባለ መስመር ባች እንደገና ሳይገነባ ይሰኩት ሜታልሊብ።

ገላጭዎቹ በሂደት ጊዜ በ በኩል ይገኛሉ
`fastpq_prover::metal_kernel_descriptors()` ማሳየት ለሚፈልግ መሳሪያ
ተመሳሳይ ሜታዳታ.

## ቆራጥ ወርቃማ ሒሳብ- ሁሉም አስኳሎች በጎልድሎክስ መስክ ላይ ከተገለጹ ረዳቶች ጋር ይሰራሉ
  `field.metal` (ሞዱል አክል/mul/ንዑስ፣ ተገላቢጦሽ፣ `pow5`)።【crates/fastpq_prover/metal/kernels/field.metal:1】
- የኤፍኤፍቲ/ኤልዲኢ ደረጃዎች የሲፒዩ እቅድ አውጪ የሚያመነጨውን ተመሳሳዩን የትርቪድል ሰንጠረዦች እንደገና ይጠቀማሉ።
  `compute_stage_twiddles` በየደረጃው አንድ ትዊድል እና አስተናጋጁን አስቀድሞ ያሰላል
  ከእያንዳንዱ መላኪያ በፊት ድርድርን በመጠባበቂያ ማስገቢያ 1 ይሰቅላል፣ ይህም ዋስትና ይሰጣል
  የጂፒዩ ዱካ አንድ አይነት የአንድነት ስር ይጠቀማል።【crates/fastpq_prover/src/metal.rs:1527】
- ለኤልዲኢ ኮሴት ማባዛት በመጨረሻው ደረጃ ላይ ተቀላቅሏል ስለዚህ ጂፒዩ በጭራሽ
  ከሲፒዩ መከታተያ አቀማመጥ ይለያል; አስተናጋጁ የግምገማ ቋቱን ዜሮ ይሞላል
  ከመላኩ በፊት፣ የመደፊያ ባህሪን ወስኖ መጠበቅ።【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## የሜታሊብ ትውልድ

`build.rs` የግለሰቡን `.metal` ምንጮችን ወደ `.air` ነገሮች ያጠናቅራል እና ከዚያም
ከላይ የተዘረዘሩትን እያንዳንዱን የመግቢያ ነጥብ ወደ ውጭ በመላክ ወደ `fastpq.metallib` ያገናኛቸዋል።
`FASTPQ_METAL_LIB` ወደዚያ መንገድ ማዋቀር (የግንባታ ስክሪፕቱ ይህን ያደርጋል
በራስ-ሰር) የሩጫ ሰዓቱ ምንም ይሁን ምን ቤተ-መጽሐፍቱን በቆራጥነት እንዲጭን ያስችለዋል።
`cargo` የግንባታ ቅርሶችን ያስቀመጠበት።【crates/fastpq_prover/build.rs:45】

ከ CI ሩጫዎች ጋር ለመመሳሰል ቤተ መፃህፍቱን በእጅ ማደስ ይችላሉ፡-

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## የክር ቡድን መጠን ሂዩሪስቲክስ

`metal_config::fft_tuning` የመሳሪያውን የማስፈጸሚያ ስፋት እና ከፍተኛ ክሮች በአንድ
የክርክር ቡድን ወደ እቅድ አውጪው ስለዚህ የአሂድ ጊዜ መላኪያዎች የሃርድዌር ገደቦችን ያከብራሉ።
የምዝግብ ማስታወሻው መጠን ሲጨምር ነባሪው ወደ 32/64/128/256 መስመሮች ተጣብቋል፣ እና
የሰድር ጥልቀት አሁን በ `log_len ≥ 12` ላይ ከአምስት ደረጃዎች ወደ አራት ይራመዳል, ከዚያም ይቀጥላል.
የጋራ ትውስታ ማለፊያ ዱካው ከተሻገረ በኋላ ለ12/14/16 ደረጃዎች ገቢር ይሆናል።
`log_len ≥ 18/20/22` ስራን ለድህረ-ቲሊንግ ከርነል ከማስተላለፉ በፊት። ኦፕሬተር
ይሽራል (`FASTPQ_METAL_FFT_LANES`፣ `FASTPQ_METAL_FFT_TILE_STAGES`) ያልፋል
`FftArgs::threadgroup_lanes`/`local_stage_limit` እና በከርነሎች ይተገበራሉ
በላይኛው ሜታልሊብ ሳይገነባ።【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

የተፈቱ የማስተካከያ ዋጋዎችን ለመያዝ እና ያንን ለማረጋገጥ `fastpq_metal_bench` ይጠቀሙ
ባለብዙ ማለፊያ ኮርነሎች ተለማመዱ (`post_tile_dispatches` በ JSON) በፊት
የቤንችማርክ ጥቅል መላክ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】