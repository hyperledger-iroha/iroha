---
lang: he
direction: rtl
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-11-21T15:32:12.874170+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/benchmarks/history.md -->

# היסטוריית לכידות בנצ'מרק GPU ‏(FASTPQ WP5-B)

קובץ זה נוצר על‑ידי `python3 scripts/fastpq/update_benchmark_history.py`.
הוא ממלא את deliverable של FASTPQ Stage 7 WP5-B באמצעות מעקב אחר כל ארטיפקט
בנצ'מרק GPU עטוף, מניפסט ה‑Poseidon microbench, וסריקות עזר תחת `benchmarks/`.
עדכנו את הלכידות הבסיסיות והריצו מחדש את הסקריפט בכל פעם שחבילה חדשה מגיעה
או שהטלמטריה דורשת ראיות רעננות.

## תחום ותהליך עדכון

- הפיקו או עטפו לכידות GPU חדשות (באמצעות `scripts/fastpq/wrap_benchmark.py`),
  הוסיפו אותן למטריצת הלכידות והריצו מחדש את הגנרטור כדי לרענן את הטבלאות.
- כאשר נתוני Poseidon microbench קיימים, ייצאו אותם עם
  `scripts/fastpq/export_poseidon_microbench.py` ובנו מחדש את המניפסט באמצעות
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- תעדו סריקות Merkle threshold באמצעות אחסון פלטי JSON תחת
  `benchmarks/merkle_threshold/`; הגנרטור מציג את הקבצים הידועים כדי שביקורות
  יוכלו להשוות זמינות CPU מול GPU.

## FASTPQ Stage 7 GPU Benchmarks

| חבילה | Backend | Mode | GPU backend | GPU available | Device class | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | yes | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metal | gpu | none | yes | apple-m4 | Apple GPU 40-core | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | yes | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> עמודות: `Backend` נגזר משם החבילה; `Mode`/`GPU backend`/`GPU available`
> מועתקים מבלוק `benchmarks` העטוף כדי לחשוף fallback‑ים של CPU או גילוי GPU חסר
> (למשל, `gpu_backend=none` למרות `Mode=gpu`). ‏SU = יחס האצה (CPU/GPU).

## Poseidon Microbench Snapshots

`benchmarks/poseidon/manifest.json` מאגד את ריצות ה‑microbench של Poseidon
בין ברירת המחדל לגרסת scalar שיוצאות מכל חבילת Metal. הטבלה למטה מתרעננת
על‑ידי סקריפט הגנרטור, כך ש‑CI וביקורות ממשל יוכלו לבצע diff להאצות היסטוריות
בלי לפרוק את דוחות FASTPQ העטופים.

| Summary | Bundle | Timestamp | Default ms | Scalar ms | Speedup |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle Threshold Sweeps

לכידות ייחוס שנאספו באמצעות
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
נמצאות תחת `benchmarks/merkle_threshold/`. רשומות מציינות אם המארח חשף
התקני Metal בזמן הריצה; לכידות עם GPU צריכות לדווח
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

לכידת Apple Silicon (`takemiyacStudio.lan_25.0.0_arm64`) היא בסיס ה‑GPU הקנוני
שמשמש ב‑`docs/source/benchmarks.md`; רשומות macOS 14 נשארות בסיסים של CPU בלבד
לסביבות שאינן יכולות לחשוף התקני Metal.

## Row-Usage Snapshots

פענוחי witness שנלכדו באמצעות `scripts/fastpq/check_row_usage.py` מוכיחים את
יעילות השורות של ה‑transfer gadget. שמרו את ארטיפקטי ה‑JSON תחת
`artifacts/fastpq_benchmarks/` והגנרטור יסכם את יחסי ההעברה שהוקלטו עבור בודקים.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — batches=2, transfer_ratio avg=0.629 (min=0.625, max=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — batches=2, transfer_ratio avg=0.619 (min=0.613, max=0.625)

</div>
