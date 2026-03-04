---
lang: kk
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

# GPU бенчмарк түсіру тарихы (FASTPQ WP5-B)

Бұл файлды `python3 scripts/fastpq/update_benchmark_history.py` жасайды.
Ол әрбір оралған GPU бақылау арқылы жеткізілетін FASTPQ Stage 7 WP5-B деңгейін қанағаттандырады.
эталондық артефакт, Посейдон микробенч манифесті және қосалқы сыпырғыштар
`benchmarks/`. Негізгі түсірулерді жаңартыңыз және жаңа болған кезде сценарийді қайта іске қосыңыз
топтама жерлер немесе телеметрия жаңа дәлелдерді қажет етеді.

## Ауқым және жаңарту процесі

- Жаңа GPU түсірілімдерін жасаңыз немесе ораңыз (`scripts/fastpq/wrap_benchmark.py` арқылы),
  оларды түсіру матрицасына қосыңыз және жаңарту үшін осы генераторды қайта іске қосыңыз
  кестелер.
- Poseidon microbench деректері болған кезде оны арқылы экспорттаңыз
  `scripts/fastpq/export_poseidon_microbench.py` және манифестті пайдалану арқылы қайта жасаңыз
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- JSON шығыстарын сақтау арқылы Merkle шекті сыпыруды жазыңыз
  `benchmarks/merkle_threshold/`; бұл генератор белгілі файлдарды тізімдейді, осылайша тексереді
  CPU және GPU қол жетімділігін салыстыра алады.

## FASTPQ 7 кезең GPU көрсеткіштері

| Бума | Backend | Режим | GPU сервері | GPU қол жетімді | Құрылғы класы | GPU | LDE мс (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | иә | xeon-rtx | NVIDIA RTX 6000 Ada | 1512,9/880,7/1,72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | металл | gpu | ешқайсысы | иә | алма-m4 | Apple GPU 40 ядролы | 785,6/735,6/1,07 | 1803,8/1897,5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | металл | gpu | металл | иә | алма-м2-ультра | Apple M2 Ultra | 1581,1/1604,5/0,98 | 3589,9/3697,3/0,97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | металл | gpu | металл | иә | алма-м2-ультра | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | металл | gpu | металл | иә | алма-м2-ультра | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | иә | neoverse-mi300 | AMD Instinct MI300A | 4518,5/688,9/6,56 | 2780,4/905,6/3,07 |

> Бағандар: `Backend` бума атынан алынған; `Mode`/`GPU backend`/`GPU available`
> процессордың резервтік мүмкіндіктерін немесе жоқ GPU-ны көрсету үшін оралған `benchmarks` блогынан көшіріледі.
> табу (мысалы, `gpu_backend=none` `Mode=gpu` қарамастан). SU = жылдамдықты арттыру коэффициенті (CPU/GPU).

## Poseidon Microbench суреттері

`benchmarks/poseidon/manifest.json` әдепкі және скаляр Посейдонды біріктіреді
әрбір металл бумасынан экспортталатын микробенч жұмысы. Төмендегі кестені жаңартқан
генератор сценарийі, сондықтан CI және басқару шолулары тарихи жылдамдықтарды ажырата алады
оралған FASTPQ есептерін ашпастан.

| Түйіндеме | Бума | Уақыт белгісі | Әдепкі мс | Скалярлық мс | Жылдамдату |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167,7 | 2152,2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990,5 | 1994,5 | 1.00 |

## Merkle табалдырығын сыпыруАнықтамалық түсірілімдер арқылы жиналған
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` астында өмір сүреді. Тізім жазбалары хосттың бар-жоғын көрсетеді
сыпыру кезінде ашық металл құрылғылар; GPU қосылған түсірулер есеп беруі керек
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon түсіру (`takemiyacStudio.lan_25.0.0_arm64`) `docs/source/benchmarks.md` жүйесінде қолданылатын канондық GPU базалық сызығы; macOS 14 жазбалары металл құрылғыларын көрсете алмайтын орталар үшін тек процессорға арналған негізгі сызықтар ретінде қалады.

## Жолды пайдалану суреттері

`scripts/fastpq/check_row_usage.py` арқылы түсірілген куәгердің декодтары тасымалдауды дәлелдейді
гаджет жолының тиімділігі. JSON артефактілерін `artifacts/fastpq_benchmarks/` астында сақтаңыз
және бұл генератор аудиторлар үшін жазылған аударым коэффициенттерін қорытындылайды.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — партиялар=2, тасымалдау_қатысы орташа =0,629 (мин=0,625, макс=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — партиялар=2, тасымалдау_қатысы орташа =0,619 (мин=0,613, макс=0,625)