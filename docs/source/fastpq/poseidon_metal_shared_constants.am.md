---
lang: am
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ፖሲዶን ሜታል የተጋሩ ኮንስታንትስ

የብረት ከርነሎች፣ CUDA kernels፣ the Rust prover እና እያንዳንዱ የኤስዲኬ እቃ ማጋራት አለባቸው
ሃርድዌር-የተጣደፈ ለማቆየት ትክክለኛ ተመሳሳይ የPoseidon2 መለኪያዎች
hashing deterministic. ይህ ሰነድ ቀኖናዊውን ቅጽበታዊ ገጽ እይታ ይመዘግባል፣ እንዴት
እንደገና ማመንጨት እና የጂፒዩ ቧንቧዎች እንዴት ውሂቡን እንደሚያስገቡ ይጠበቃል።

## ቅጽበታዊ ገጽ እይታ

መለኪያዎቹ እንደ `PoseidonSnapshot` RON ሰነድ ታትመዋል። ቅጂዎች ናቸው።
በስሪት ቁጥጥር ስር ስለሚቀመጡ የጂፒዩ የመሳሪያ ሰንሰለት እና ኤስዲኬዎች በግንባታ ጊዜ ላይ እንዳይመሰረቱ
ኮድ ማመንጨት.

| መንገድ | ዓላማ | SHA-256 |
|-------|--------|-----|
| `artifacts/offline_poseidon/constants.ron` | ከ `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` የመነጨ ቀኖናዊ ቅጽበታዊ ገጽ እይታ; ለጂፒዩ ግንባታዎች የእውነት ምንጭ። | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | ቀኖናዊውን ቅጽበታዊ ገጽ እይታን ያንጸባርቃል ስለዚህ የስዊፍት ዩኒት ሙከራዎች እና የ XCFramework የጢስ ማውጫ የብረት ከርነሎች የሚጠብቁትን ተመሳሳይ ቋሚዎች ይጭናሉ። | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | የአንድሮይድ/Kotlin መጫዎቻዎች ለተመጣጣኝ እና ለተከታታይነት ሙከራዎች ተመሳሳይ መግለጫዎችን ይጋራሉ። | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

እያንዳንዱ ሸማች ቋሚዎችን ወደ ጂፒዩ ከማስገባቱ በፊት ሃሹን ማረጋገጥ አለበት።
የቧንቧ መስመር. አንጸባራቂው ሲቀየር (አዲስ ግቤት ወይም መገለጫ)፣ SHA እና
የታችኛው ተፋሰስ መስተዋቶች በመቆለፊያ ደረጃ መዘመን አለባቸው።

## ዳግም መወለድ

አንጸባራቂው `xtask` በማሄድ ከሩስት ምንጮች የመነጨ ነው።
ረዳት ። ትዕዛዙ ሁለቱንም ቀኖናዊ ፋይል እና የኤስዲኬ መስተዋቶች ይጽፋል፡-

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

መድረሻዎቹን ለመሻር `--constants <path>`/`--vectors <path>` ይጠቀሙ ወይም
`--no-sdk-mirror` ቀኖናዊውን ቅጽበታዊ ገጽ እይታን ብቻ ሲያድስ። ረዳቱ ያደርጋል
ባንዲራ ሲቀር ቅርሶቹን ወደ ስዊፍት እና አንድሮይድ ዛፎች ያንጸባርቁ፣
ሃሽቹን ለ CI እንዲሰለፉ የሚያደርግ።

## ብረትን መመገብ/CUDA ይገነባል።

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` እና
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` ከ እንደገና መፈጠር አለበት።
  ሠንጠረዡ በሚቀየርበት ጊዜ ሁሉ ይግለጹ.
- የተጠጋጋ እና ኤምዲኤስ ቋሚዎች ወደ ተከታታይ `MTLBuffer`/`__constant` ተዘጋጅተዋል
  ከአንጸባራቂው አቀማመጥ ጋር የሚዛመዱ ክፍሎች፡ `round_constants[round][state_width]`
  ተከትሎ 3x3 MDS ማትሪክስ.
- `fastpq_prover::poseidon_manifest()` ቅጽበታዊ ገጽ እይታውን ይጭናል እና ያረጋግጣል
  የሩጫ ጊዜ (በብረታ ብረት ማሞቂያ ጊዜ) ስለዚህ የመመርመሪያ መሳሪያዎች የ
  የሻደር ቋሚዎች ከታተመው ሃሽ ጋር ይጣጣማሉ
  `fastpq_prover::poseidon_manifest_sha256()`.
- የኤስዲኬ ቋሚ አንባቢዎች (ስዊፍት `PoseidonSnapshot`፣ አንድሮይድ `PoseidonSnapshot`) እና
  የ Norito ከመስመር ውጭ መገልገያ መሳሪያዎች ጂፒዩ-ብቻን የሚከለክለው በተመሳሳይ መግለጫ ላይ ነው.
  መለኪያ ሹካዎች.

## ማረጋገጫ

1. አንጸባራቂውን እንደገና ካደጉ በኋላ `cargo test -p xtask` ን ያሂዱ
   የፖሲዶን ቋሚ ትውልድ አሃድ ሙከራዎች።
2. አዲሱን SHA-256 በዚህ ሰነድ እና በሚከታተል በማንኛውም ዳሽቦርድ ውስጥ ይመዝግቡ
   የጂፒዩ ቅርሶች.
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` ትንታኔዎች
   `poseidon2.metal` እና `fastpq_cuda.cu` በግንባታ ጊዜ እና አስረግጠው ያረጋግጣሉ
   ተከታታይ ቋሚዎች የCUDA/የብረት ሰንጠረዦችን በመጠበቅ ከማንፀባረቁ ጋር ይዛመዳሉ
   ቀኖናዊው ቅጽበታዊ ገጽ እይታ በመቆለፊያ ደረጃ።አንጸባራቂውን ከጂፒዩ ግንባታ መመሪያዎች ጋር ማቆየት ሜታል/CUDAን ይሰጣል
የስራ ፍሰቶች ወሳኙ የእጅ መጨባበጥ፡ ከርነሎች የማስታወስ ችሎታቸውን ለማመቻቸት ነፃ ናቸው።
የተጋሩ ቋሚዎች ነጠብጣብ እስከ ገቡ እና ሃሽውን እስካጋለጡ ድረስ አቀማመጥ
ቴሌሜትሪ ለተመጣጣኝ ቼኮች.