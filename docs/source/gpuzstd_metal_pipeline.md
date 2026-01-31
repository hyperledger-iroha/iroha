# GPU Zstd (Metal) Pipeline

This document describes the deterministic GPU pipeline used by the Metal helper
for zstd compression. It is a design and implementation guide for the
`gpuzstd_metal` helper that must produce the exact same bytes as the CPU zstd
implementation for the same input and parameters.

## Goals

- Bit-for-bit parity with CPU zstd at the same compression level and parameters.
- Deterministic outputs across hardware, drivers, and thread scheduling.
- Explicit bounds checks and predictable buffer lifetimes.

## Encoding pipeline (high level)

1. Input staging
   - Copy the input into a device buffer.
   - Partition into fixed-size chunks (for sequence generation) and blocks (for
     zstd frame assembly).
2. Match finding and sequence emission
   - GPU kernels scan each chunk and emit sequences (literal length, match
     length, offset).
   - Sequence ordering is stable and deterministic.
3. Literal preparation
   - Collect literals referenced by sequences.
   - Build literal histograms and select literal block mode (raw, RLE, or
     Huffman) deterministically.
4. Huffman tables (literals)
   - Generate code lengths from the histogram.
   - Build canonical tables with deterministic tie-breaking that matches CPU
     zstd output.
5. FSE tables (LL/ML/OF)
   - Normalize frequency counts.
   - Build FSE decoding/encoding tables deterministically.
6. Bitstream writer
   - Pack bits little-endian (LSB-first).
   - Flush on byte boundaries; pad with zeros only.
   - Enforce overflow and capacity checks.
7. Block and frame assembly
   - Emit block headers (type, size, last-block flag).
   - Serialize literals and sequences into compressed blocks.
   - Emit standard zstd frame headers and optional checksums.

## Decoding pipeline (high level)

1. Frame parse
   - Validate magic bytes, window settings, and frame header fields.
2. Bitstream reader
   - Read LSB-first bit sequences with strict bounds checks.
3. Literal decode
   - Decode literal blocks (raw, RLE, or Huffman) into the literal buffer.
4. Sequence decode
   - Decode LL/ML/OF values using FSE tables.
   - Reconstruct matches using the sliding window.
5. Output and checksum
   - Write reconstructed bytes into the output buffer.
   - Verify optional checksums when enabled.

## Buffer lifetimes and ownership

- Input buffer: host -> device, read-only.
- Sequence buffer: device, produced by match-finding and consumed by entropy
  coding; no cross-block reuse.
- Literal buffer: device, produced for each block and released after block
  emission.
- Output buffer: device, holds the final frame bytes until the host copies them
  out.
- Scratch buffers: reused across kernels, but always overwritten deterministically.

## Kernel responsibilities

- Match finding kernels: find matches and emit sequences (LL/ML/OF + literals).
- Huffman build kernels: derive code lengths and canonical tables.
- FSE build kernels: build LL/ML/OF tables and state machines.
- Block encode kernels: serialize literals and sequences into the bitstream.
- Block decode kernels: parse bitstream and reconstruct literals/sequences.

## Determinism and parity constraints

- Canonical table builds must use the same ordering and tie-breaking as CPU
  zstd.
- No atomics or reductions that depend on thread scheduling for any output byte.
- Bitstream packing is little-endian, LSB-first; byte alignment pads with zeros.
- All bounds checks are explicit; invalid inputs fail deterministically.

## Validation

- CPU golden vectors for the bitstream writer/reader.
- Corpus parity tests comparing GPU and CPU outputs.
- Fuzz coverage for malformed frames and boundary conditions.
