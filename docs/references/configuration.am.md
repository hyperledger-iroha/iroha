---
lang: am
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ማፋጠን

የ`[accel]` ክፍል ለIVM እና አጋዥዎች አማራጭ የሃርድዌር ማጣደፍን ይቆጣጠራል። ሁሉም
የተጣደፉ ዱካዎች የሚወስኑ የሲፒዩ ውድቀቶች አሏቸው። አንድ ጀርባ ወርቃማ ካልተሳካ
በሂደት ጊዜ ራስን መሞከር በራስ-ሰር ተሰናክሏል እና አፈፃፀሙ በሲፒዩ ላይ ይቀጥላል።

- `enable_cuda` (ነባሪ፡ እውነት) - ሲዘጋጅ እና ሲገኝ CUDA ይጠቀሙ።
- `enable_metal` (ነባሪ፡ እውነት) - ሲገኝ ሜታልን በ macOS ላይ ይጠቀሙ።
- `max_gpus` (ነባሪ: 0) - ለመጀመር ከፍተኛው ጂፒዩዎች; `0` ማለት አውቶማቲክ/ካፕ የለም ማለት ነው።
- `merkle_min_leaves_gpu` (ነባሪ፡ 8192) - መርክልን ለማውረድ በትንሹ ቅጠሎች
  ቅጠል ወደ ጂፒዩ. ባልተለመደ ፈጣን ጂፒዩዎች ብቻ ዝቅ አድርግ።
- የላቀ (አማራጭ፤ ብዙውን ጊዜ አስተዋይ ነባሪዎች ይወርሳሉ)
  - `merkle_min_leaves_metal` (ነባሪ፡ `merkle_min_leaves_gpu` ይወርሳሉ)።
  - `merkle_min_leaves_cuda` (ነባሪ፡ `merkle_min_leaves_gpu` ይወርሳሉ)።
  - `prefer_cpu_sha2_max_leaves_aarch64` (ነባሪ፡ 32768) – ሲፒዩ SHA‑2 እስከዚህ ብዙ ቅጠሎችን በ ARMv8 ከSHA2 ጋር ይምረጡ።
  - `prefer_cpu_sha2_max_leaves_x86` (ነባሪ፡ 32768) – ሲፒዩ SHA‑NI እስከዚህ ድረስ በ x86/x86_64 ላይ ይምረጡ።

ማስታወሻዎች
- ቆራጥነት በመጀመሪያ: ማፋጠን የማይታዩ ውጤቶችን ፈጽሞ አይለውጥም; ጀርባዎች
  በመግቢያው ላይ ወርቃማ ሙከራዎችን ያሂዱ እና አለመዛመጃዎች ሲገኙ ወደ scalar/SIMD ይመለሱ።
- በ `iroha_config` በኩል ያዋቅሩ; በምርት ውስጥ የአካባቢ ተለዋዋጮችን ያስወግዱ.