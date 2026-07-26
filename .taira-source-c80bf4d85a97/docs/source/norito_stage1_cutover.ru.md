---
lang: ru
direction: ltr
source: docs/source/norito_stage1_cutover.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f02eb4d7d5fa1a5ffef5c285cd1722799dcd0fd1fce852cd12eb6b9cf477f51
source_last_modified: "2026-01-03T18:07:57.754211+00:00"
translation_last_reviewed: 2026-01-30
---

## Norito Stage-1 Cutover (March 2026)

This note captures the latest Stage-1 (JSON structural index) and CRC64 GPU
threshold tuning. Benchmarks were gathered on Apple Silicon using:

```
cargo run -p norito --example stage1_cutover --release --features bench-internal \
  > benchmarks/norito_stage1/cutover.csv
```

The helper emits `bytes,scalar_ns,kernel_ns,ns_per_byte_*` rows. Key findings:
- Scalar vs SIMD crossover now occurs around **6–8 KiB**; at ~4 KiB SIMD is ~5%
  slower, but from 8 KiB upward it is ~2× faster.
- GPU Stage-1 launch overhead amortises around **~192 KiB** on this host with
  chunked CRC64/Stage-1 kernels; keeping the cutoff above that avoids thrashing
  while letting large documents use the helper dylibs when present.
- CRC64 GPU helpers benefit from the same 192 KiB guard and now support an env
  override for the helper path so tests and downstream tooling can inject
  stubbed accelerators.

Resulting defaults:
- `build_struct_index` small-input cutoff raised to **4096 bytes**.
- Stage-1 GPU minimum: **192 KiB** (`NORITO_STAGE1_GPU_MIN_BYTES` to override).
- CRC64 GPU minimum: **192 KiB** (`NORITO_GPU_CRC64_MIN_BYTES` to override).
- CRC64 GPU loader now accepts `NORITO_CRC64_GPU_LIB` to point at a custom
  helper (used by the stub-backed parity test).

See `benchmarks/norito_stage1/cutover.csv` for the raw measurements.
