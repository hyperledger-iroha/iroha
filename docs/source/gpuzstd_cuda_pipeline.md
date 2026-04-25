# GPU Zstd (CUDA) Pipeline

This document describes the deterministic CUDA helper used by Norito for zstd
compression on non-Apple GPU hosts. The helper exports the same C ABI as the
Metal helper:

- `gpu_zstd_compress(src_ptr, src_len, level, dst_ptr, dst_len)`
- `gpu_zstd_decompress(src_ptr, src_len, dst_ptr, dst_len)`

## Goals

- Use CUDA for the expensive match-finding and sequence-generation pass.
- Emit standard zstd frames that CPU zstd decoders can read.
- Keep encoded bytes deterministic for the same input and helper build.
- Report `gpu_unavailable` when CUDA kernels are not built or no CUDA device is
  present, so Norito falls back to the CPU backend honestly.
- Never perform CPU zstd encoding inside `gpuzstd_cuda`; CUDA or frame assembly
  failures are reported to Norito so fallback happens outside the helper.

## Encoding Pipeline

1. The Rust helper partitions input into fixed 32 KiB chunks.
2. `gpuzstd_cuda_count_sequences` copies the input to device memory and launches
   one deterministic scanner per chunk to count zstd sequences.
3. The host computes chunk sequence offsets.
4. `gpuzstd_cuda_write_sequences` launches the same scanner and writes
   `(literal length, match length, offset)` records into a device sequence
   buffer, then copies them back to the host.
5. The shared zstd frame encoder assembles literals, Huffman/FSE data, block
   headers, and the frame header.

The CUDA kernels do not use atomics or scheduling-dependent reductions for
output bytes. Each chunk owns its hash table and sequence range, so launch order
does not affect the emitted stream.

Norito attempts to load the CUDA helper directly and relies on the helper
self-test to prove CUDA availability; it does not require `nvidia-smi` to be in
`PATH`.

## Decoding Pipeline

The helper decodes with the shared frame decoder and falls back to the
CPU zstd decoder for standard frames not covered by that decoder. This keeps the
C ABI useful for Norito roundtrips while preserving the rule that compression is
only registered as a GPU backend when CUDA compression succeeds.

## JSON Stage-1 CUDA Helper

`jsonstage1_cuda` now builds CUDA kernels by default when `nvcc` is available.
The helper classifies structural characters, quotes, and backslashes into
32-byte masks on the GPU. The host finalizer then applies quote/backslash parity
across block boundaries and emits the structural offset tape. If CUDA kernels
are not built or a CUDA device is not present, the helper returns
`gpu_unavailable`; Norito's scalar/SIMD Stage-1 path remains the fallback.

The same helper also exposes `norito_crc64_cuda`, which computes CRC64-XZ using
CUDA chunk kernels and host-side GF(2) combination. CRC64 returns unavailable
instead of silently doing CPU work when CUDA cannot run.

## Validation

Focused checks:

- `cargo test -p gpuzstd_cuda`
- `cargo test -p jsonstage1_cuda`
- `cargo test -p norito gpu_zstd --features gpu-compression`
- `cargo test -p norito stage1_helper --features cuda-stage1,stage1-validate`

CUDA-host checks that fail instead of skipping if kernels or devices are
missing:

- `GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture`
- `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture`
- `GPUZSTD_CUDA_REQUIRE=1 cargo test -p norito required_cuda_backend_is_registered_when_requested --features gpu-compression -- --nocapture`
- `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p norito cuda_stage1_backend_matches_scalar_when_required_or_available --features cuda-stage1,stage1-validate -- --nocapture`

On hosts without `nvcc`, the helper crates compile without CUDA kernels and the
CUDA-only assertions skip after observing `gpu_unavailable`. On CUDA hosts, the
same tests exercise the kernels and compare decoded payloads or Stage-1 tapes
against the CPU references.
