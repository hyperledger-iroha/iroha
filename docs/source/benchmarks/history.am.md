---
lang: am
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# የጂፒዩ ቤንችማርክ ቀረጻ ታሪክ (FASTPQ WP5-B)

ይህ ፋይል የተፈጠረው በ`python3 scripts/fastpq/update_benchmark_history.py` ነው።
እያንዳንዱን የታሸገ ጂፒዩ በመከታተል ሊደርስ የሚችለውን FASTPQ Stage 7 WP5-B ያሟላል።
የቤንችማርክ አርቴፋክት፣ የፖሲዶን ማይክሮቤንች አንጸባራቂ እና ረዳት ጠረገ
`benchmarks/`. የስር ቀረጻዎችን ያዘምኑ እና አዲስ በሚሆንበት ጊዜ ስክሪፕቱን እንደገና ያስጀምሩ
ጥቅል መሬት ወይም ቴሌሜትሪ አዲስ ማስረጃ ያስፈልገዋል።

## ወሰን እና የማዘመን ሂደት

- አዲስ የጂፒዩ ቀረጻዎችን (በ`scripts/fastpq/wrap_benchmark.py` በኩል) ማምረት ወይም መጠቅለል
  ወደ ቀረጻ ማትሪክስ አያይዟቸው እና ይህን ጄኔሬተር ለማደስ እንደገና ያስጀምሩት።
  ጠረጴዛዎች.
- የፖሲዶን ማይክሮ ቤንች መረጃ በሚገኝበት ጊዜ ወደ ውጭ ይላኩት
  `scripts/fastpq/export_poseidon_microbench.py` እና አንጸባራቂውን በመጠቀም እንደገና ይገንቡ
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- የJSON ውጤቶቻቸውን ከስር በማከማቸት Merkle threshold ጠራሮችን ይመዝግቡ
  `benchmarks/merkle_threshold/`; ይህ ጄኔሬተር የታወቁ ፋይሎችን ይዘረዝራል ስለዚህ ኦዲት ያደርጋል
  ማጣቀሻ ሲፒዩ vs ጂፒዩ መገኘት ይችላል።

## FASTPQ ደረጃ 7 የጂፒዩ መመዘኛዎች

| ጥቅል | ጀርባ | ሁነታ | የጂፒዩ ጀርባ | ጂፒዩ አለ | የመሣሪያ ክፍል | ጂፒዩ | LDE ms (ሲፒዩ/ጂፒዩ/SU) | Poseidon ms (ሲፒዩ/ጂፒዩ/SU) |
|---
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | ኩዳ | ጂፒዩ | cuda-sm80 | አዎ | xeon-rtx | NVIDIA RTX 6000 አዳ | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | ብረት | ጂፒዩ | የለም | አዎ | apple-m4 | አፕል ጂፒዩ 40-ኮር | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | ብረት | ጂፒዩ | ብረት | አዎ | apple-m2-ultra | አፕል M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | ብረት | ጂፒዩ | ብረት | አዎ | apple-m2-ultra | አፕል M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | ብረት | ጂፒዩ | ብረት | አዎ | apple-m2-ultra | አፕል M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | ጂፒዩ | opencl | አዎ | neoverse-mi300 | AMD በደመ MI300A | 4518.5 / 688.9 / 6.56 | 2780.4/905.6/3.07 |

> አምዶች: `Backend` ከጥቅል ስም የተወሰደ ነው; `Mode`/`GPU backend`/`GPU available`
> የሲፒዩ ውድቀትን ወይም የጎደሉትን ጂፒዩ ለማጋለጥ ከተጠቀለለው `benchmarks` ብሎክ የተገለበጡ ናቸው።
> ግኝት (ለምሳሌ `gpu_backend=none` ምንም እንኳን `Mode=gpu`)። SU = የፍጥነት መጠን (ሲፒዩ/ጂፒዩ)።

## የፖሲዶን ማይክሮ ቤንች ቅጽበተ-ፎቶዎች

`benchmarks/poseidon/manifest.json` ነባሪውን-ከስካላር ፖሲዶን ያጠቃልላል
ማይክሮቤንች ከእያንዳንዱ የብረታ ብረት ጥቅል ወደ ውጭ ይላካል። ከዚህ በታች ያለው ሰንጠረዥ የሚታደሰው በ
የጄኔሬተሩ ስክሪፕት, ስለዚህ CI እና የአስተዳደር ግምገማዎች ታሪካዊ ፍጥነትን ሊለያዩ ይችላሉ
የታሸጉትን የ FASTPQ ሪፖርቶችን ሳይከፍቱ።

| ማጠቃለያ | ጥቅል | የጊዜ ማህተም | ነባሪ ms | Scalar ms | ፍጥነት |
|--------|----
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06: 11: 01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06: 04: 07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle Threshold ጠረገየማጣቀሻ ቀረጻዎች የተሰበሰቡ በ
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
በ `benchmarks/merkle_threshold/` ስር ይኖራሉ። የዝርዝር ግቤቶች አስተናጋጁ እንደሆነ ያሳያሉ
መጥረጊያው ሲሮጥ የተጋለጡ የብረት መሳሪያዎች; በጂፒዩ የነቁ ምስሎች ሪፖርት ማድረግ አለባቸው
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

የ Apple Silicon ቀረጻ (`takemiyacStudio.lan_25.0.0_arm64`) በ `docs/source/benchmarks.md` ውስጥ ጥቅም ላይ የዋለው ቀኖናዊ የጂፒዩ መነሻ መስመር ነው; የ macOS 14 ግቤቶች የብረታ ብረት መሳሪያዎችን ማጋለጥ ለማይችሉ አካባቢዎች እንደ ሲፒዩ-ብቻ መሰረት ሆነው ይቆያሉ።

## የረድፍ አጠቃቀም ቅጽበተ-ፎቶዎች

በ`scripts/fastpq/check_row_usage.py` የተያዙ ምስክሮች መፍታት ዝውውሩን ያረጋግጣሉ
የመግብር ረድፍ ቅልጥፍና. የJSON ቅርሶችን በ`artifacts/fastpq_benchmarks/` ስር ያቆዩ
እና ይህ ጄነሬተር ለኦዲተሮች የተመዘገቡትን የዝውውር ሬሾዎችን ያጠቃልላል.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — ባች=2፣ ማስተላለፍ_ሬሾ አማካኝ=0.629 (ደቂቃ=0.625፣ ከፍተኛ=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — ባች=2፣ ማስተላለፍ_ሬሾ አማካኝ=0.619 (ደቂቃ=0.613፣ ከፍተኛ=0.625)