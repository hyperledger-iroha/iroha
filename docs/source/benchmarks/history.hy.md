---
lang: hy
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

# GPU հենանիշի գրավման պատմություն (FASTPQ WP5-B)

Այս ֆայլը ստեղծվել է `python3 scripts/fastpq/update_benchmark_history.py`-ի կողմից:
Այն բավարարում է FASTPQ Stage 7 WP5-B առաքումը` հետևելով յուրաքանչյուր փաթաթված GPU-ին
հենանիշային արտեֆակտ, Պոսեյդոնի միկրոբենչ մանիֆեստ և օժանդակ նյութեր
`benchmarks/`. Թարմացրեք հիմքում ընկած նկարները և նորից գործարկեք սցենարը, երբ նորը լինի
փաթեթային հողերը կամ հեռաչափությունը թարմ ապացույցների կարիք ունեն:

## Շրջանակ և թարմացման գործընթաց

- Արտադրել կամ փաթաթել նոր GPU նկարահանումներ (`scripts/fastpq/wrap_benchmark.py`-ի միջոցով),
  կցեք դրանք գրավման մատրիցին և նորից գործարկեք այս գեներատորը՝ թարմացնելու համար
  սեղաններ.
- Երբ առկա են Poseidon microbench-ի տվյալները, արտահանեք դրանք
  `scripts/fastpq/export_poseidon_microbench.py` և վերակառուցեք մանիֆեստը՝ օգտագործելով
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Գրանցեք Merkle-ի շեմի ավլում՝ պահպանելով նրանց JSON ելքերը տակ
  `benchmarks/merkle_threshold/`; այս գեներատորը թվարկում է հայտնի ֆայլերը, որպեսզի ստուգի
  կարող է խաչաձև հղում կատարել պրոցեսորի ընդդեմ GPU-ի առկայության:

## FASTPQ 7 փուլի GPU հենանիշներ

| Փաթեթ | Backend | Ռեժիմ | GPU backend | GPU հասանելի | Սարքի դաս | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|------------|--------------|-------- ------|-----|-------------------------------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | տարբեր | gpu | cuda-sm80 | այո | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | մետաղական | gpu | ոչ մեկը | այո | խնձոր-մ4 | Apple GPU 40 միջուկային | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | մետաղական | gpu | մետաղական | այո | խնձոր-մ2-ուլտրա | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | մետաղական | gpu | մետաղական | այո | խնձոր-մ2-ուլտրա | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | մետաղական | gpu | մետաղական | այո | խնձոր-մ2-ուլտրա | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | այո | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> Սյունակներ. `Backend`-ը բխում է փաթեթի անվանումից; `Mode`/`GPU backend`/`GPU available`
> պատճենվում են փաթաթված `benchmarks` բլոկից՝ CPU-ի հետադարձ կապերը կամ բացակայող GPU-ն բացահայտելու համար
> բացահայտում (օրինակ՝ `gpu_backend=none` չնայած `Mode=gpu`): SU = արագացման հարաբերակցություն (CPU/GPU):

## Poseidon Microbench Snapshots

`benchmarks/poseidon/manifest.json`-ը միավորում է լռելյայն ընդդեմ սկալյար Պոսեյդոնը
Մետաղական յուրաքանչյուր փաթեթից արտահանվող միկրոբենչ: Ստորև բերված աղյուսակը թարմացվում է
գեներատորի սցենարը, այնպես որ CI և կառավարման ակնարկները կարող են տարբերել պատմական արագությունները
առանց փաթաթված FASTPQ-ի հաշվետվությունները բացելու:

| Ամփոփում | Փաթեթ | Ժամացույց | Կանխադրված ms | Scalar ms | Արագացում |
|---------|--------|-----------|------------|----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Մերկլի շեմը մաքրում էՏեղեկանք գրավում է հավաքվել միջոցով
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
ապրում է `benchmarks/merkle_threshold/`-ի ներքո: Ցանկի գրառումները ցույց են տալիս, թե արդյոք հյուրընկալողը
Մետաղական սարքերի բացահայտում, երբ ավլումը վազեց. GPU-ով միացված նկարահանումները պետք է զեկուցվեն
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon Capture-ը (`takemiyacStudio.lan_25.0.0_arm64`) հանդիսանում է կանոնական GPU ելակետ, որն օգտագործվում է `docs/source/benchmarks.md`-ում; macOS 14-ի գրառումները մնում են որպես միայն պրոցեսորի ելակետային գծեր այն միջավայրերի համար, որոնք չեն կարող բացահայտել մետաղական սարքերը:

## Շարքի օգտագործման ակնարկներ

`scripts/fastpq/check_row_usage.py`-ի միջոցով ֆիքսված վկաների վերծանումները ապացուցում են փոխանցումը
գաջեթի շարքի արդյունավետությունը: Պահպանեք JSON արտեֆակտները `artifacts/fastpq_benchmarks/`-ում
և այս գեներատորը կամփոփի աուդիտորների համար գրանցված փոխանցման գործակիցները:

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — խմբաքանակ=2, փոխանցման_հարաբերակցություն միջին=0,629 (min=0,625, առավելագույնը՝ 0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — խմբաքանակ=2, փոխանցման_հարաբերակցություն միջին=0,619 (min=0,613, առավելագույնը՝ 0,625)