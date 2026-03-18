---
lang: mn
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

# GPU жишиг зураг авалтын түүх (FASTPQ WP5-B)

Энэ файлыг `python3 scripts/fastpq/update_benchmark_history.py` үүсгэсэн.
Энэ нь ороосон GPU бүрийг хянах замаар FASTPQ 7-р шатлалын WP5-B-ийг хангадаг.
жишиг олдвор, Посейдоны бичил вандан манифест, туслах шүүр
`benchmarks/`. Үндсэн зураг авалтыг шинэчилж, скриптийг шинээр авах болгонд дахин ажиллуулаарай
багц газар эсвэл телеметрийн хувьд шинэ нотлох баримт хэрэгтэй.

## Хамрах хүрээ ба шинэчлэх үйл явц

- Шинэ GPU бичлэг хийх эсвэл боох (`scripts/fastpq/wrap_benchmark.py`-ээр дамжуулан),
  тэдгээрийг барих матрицад нэмж, энэ үүсгэгчийг дахин ажиллуулна
  хүснэгтүүд.
- Poseidon microbench-ийн өгөгдөл байгаа үед үүнийг ашиглан экспортлоорой
  `scripts/fastpq/export_poseidon_microbench.py` ба манифестыг ашиглан дахин бүтээнэ үү
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Тэдний JSON гаралтыг доор хадгалах замаар Merkle босго шүүрэлтийг тэмдэглэ
  `benchmarks/merkle_threshold/`; Энэ генератор нь мэдэгдэж буй файлуудыг жагсааж, аудит хийдэг
  CPU ба GPU-ийн хүртээмжийг хооронд нь холбож болно.

## FASTPQ 7-р шатны GPU жишиг

| Багц | Backend | Горим | GPU backend | GPU боломжтой | Төхөөрөмжийн ангилал | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | тиймээ | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | металл | gpu | байхгүй | тиймээ | алим-м4 | Apple GPU 40 цөмт | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | металл | gpu | металл | тиймээ | алим-м2-хэт | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | металл | gpu | металл | тиймээ | алим-м2-хэт | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | металл | gpu | металл | тиймээ | алим-м2-хэт | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | тиймээ | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> Багана: `Backend` нь багцын нэрнээс гаралтай; `Mode`/`GPU backend`/`GPU available`
> ороосон `benchmarks` блокоос хуулж, CPU-ийн нөхөн олговор эсвэл байхгүй GPU-г илрүүлэх боломжтой.
> нээлт (жишээ нь, `gpu_backend=none` `Mode=gpu` хэдий ч). SU = хурдны харьцаа (CPU/GPU).

## Poseidon Microbench-ийн агшин зуурын зургууд

`benchmarks/poseidon/manifest.json` нь анхдагч болон скаляр Посейдоныг нэгтгэдэг
Металл багц бүрээс экспортолж буй микро вандан гүйлт. Доорх хүснэгтийг шинэчилсэн болно
генераторын скрипт, тиймээс CI болон засаглалын тоймууд нь түүхэн хурдацыг өөрчилдөг
ороосон FASTPQ тайланг задлахгүйгээр.

| Дүгнэлт | Багц | Цагийн тэмдэг | Өгөгдмөл ms | Скаляр мс | Хурдлах |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Мерклийн босгыг шүүрдэгЛавлагааны зургийг ашиглан цуглуулсан
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` дор амьдардаг. Жагсаалтын оруулгууд нь хост байгаа эсэхийг харуулдаг
шүүрдэх үед ил гарсан Металл төхөөрөмж; GPU-г идэвхжүүлсэн зураг авалтыг мэдээлэх ёстой
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon capture (`takemiyacStudio.lan_25.0.0_arm64`) нь `docs/source/benchmarks.md`-д ашиглагддаг каноник GPU суурь юм; macOS 14-ийн оруулгууд нь Металл төхөөрөмжүүдийг ил гаргах боломжгүй орчинд зөвхөн CPU-н суурь үзүүлэлт хэвээр байна.

## Мөр ашиглалтын агшин зуурын зургууд

`scripts/fastpq/check_row_usage.py`-ээр авсан гэрчийн код тайлагдсан нь шилжүүлгийг нотолж байна
гаджетын эгнээний үр ашиг. JSON олдворуудыг `artifacts/fastpq_benchmarks/` доор хадгал
мөн энэ үүсгэгч нь аудиторуудад бүртгэгдсэн шилжүүлгийн харьцааг нэгтгэн дүгнэх болно.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — багц=2, дамжуулах_харьцаа дундаж=0,629 (мин=0,625, хамгийн ихдээ=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — багц=2, дамжуулах_харьцаа дундаж=0,619 (мин=0,613, хамгийн ихдээ=0,625)