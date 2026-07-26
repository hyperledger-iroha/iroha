---
lang: az
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

# GPU Benchmark Çəkmə Tarixi (FASTPQ WP5-B)

Bu fayl `python3 scripts/fastpq/update_benchmark_history.py` tərəfindən yaradılıb.
Hər bükülmüş GPU-nu izləməklə çatdırıla bilən FASTPQ Stage 7 WP5-B-ni təmin edir
etalon artefakt, Poseidon mikrobench manifest və köməkçi süpürmələr
`benchmarks/`. Əsas çəkilişləri yeniləyin və yeni olduqda skripti yenidən işə salın
bağlama torpaqları və ya telemetriya yeni sübutlara ehtiyac duyur.

## Əhatə və Yeniləmə Prosesi

- Yeni GPU çəkilişlərini hazırlayın və ya sarın (`scripts/fastpq/wrap_benchmark.py` vasitəsilə),
  onları tutma matrisinə əlavə edin və bu generatoru yeniləmək üçün yenidən işə salın
  masalar.
- Poseidon microbench məlumatları mövcud olduqda, onu ilə ixrac edin
  `scripts/fastpq/export_poseidon_microbench.py` və manifestdən istifadə edərək yenidən qurun
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- JSON çıxışlarını altında saxlayaraq Merkle həddi süpürgələrini qeyd edin
  `benchmarks/merkle_threshold/`; bu generator məlum faylları siyahıya alır, beləliklə auditlər aparır
  CPU və GPU mövcudluğuna çarpaz istinad edə bilər.

## FASTPQ Mərhələ 7 GPU Testləri

| Paket | Backend | Rejim | GPU backend | GPU mövcuddur | Cihaz sinfi | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | bəli | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metal | gpu | heç biri | bəli | alma-m4 | Apple GPU 40 nüvəli | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metal | gpu | metal | bəli | alma-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metal | gpu | metal | bəli | alma-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metal | gpu | metal | bəli | alma-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | bəli | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> Sütunlar: `Backend` paketin adından götürülüb; `Mode`/`GPU backend`/`GPU available`
> CPU ehtiyatlarını və ya çatışmayan GPU-nu aşkar etmək üçün bükülmüş `benchmarks` blokundan kopyalanır.
> kəşf (məsələn, `gpu_backend=none` `Mode=gpu` baxmayaraq). SU = sürətləndirmə nisbəti (CPU/GPU).

## Poseidon Microbench Snapshots

`benchmarks/poseidon/manifest.json` defolt-skalar Poseidon-u birləşdirir
hər bir Metal paketdən ixrac edilən mikrobench qaçışları. Aşağıdakı cədvəl yenilənir
generator skripti, buna görə də CI və idarəetmə rəyləri tarixi sürətlənmələri fərqləndirə bilər
bükülmüş FASTPQ hesabatlarını açmadan.

| Xülasə | Paket | Vaxt möhürü | Defolt ms | Skalyar ms | Sürətləndirmə |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle Həddi Süpürürvasitəsilə toplanmış istinadlar
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` altında yaşayır. Siyahı girişləri ev sahibi olub olmadığını göstərir
süpürgə işlədiyi zaman açıq Metal cihazlar; GPU ilə aktivləşdirilmiş çəkilişlər hesabat verməlidir
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon ələ keçirmə (`takemiyacStudio.lan_25.0.0_arm64`) `docs/source/benchmarks.md`-də istifadə edilən kanonik GPU bazasıdır; macOS 14 girişləri Metal cihazları ifşa edə bilməyən mühitlər üçün yalnız CPU əsasları olaraq qalır.

## Sıra İstifadəsi Snapshotları

`scripts/fastpq/check_row_usage.py` vasitəsilə tutulan şahid deşifrələri köçürməni sübut edir
qadcetin sıra səmərəliliyi. JSON artefaktlarını `artifacts/fastpq_benchmarks/` altında saxlayın
və bu generator auditorlar üçün qeydə alınmış transfer əmsallarını ümumiləşdirəcək.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — partiyalar=2, transfer_nisbəti orta=0,629 (min=0,625, maks.=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — partiyalar=2, transfer_nisbəti orta=0,619 (min=0,613, maks.=0,625)