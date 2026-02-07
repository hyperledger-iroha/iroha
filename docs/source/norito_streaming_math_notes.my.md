---
lang: my
direction: ltr
source: docs/source/norito_streaming_math_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dc286dbb17fc181508995b95ef494b6ab4a0c1fc23686e48021589121c08eb9f
source_last_modified: "2026-02-04T16:48:08.681885+00:00"
translation_last_reviewed: 2026-02-07
---

# Norito Streaming Math Roadmap (Non-Normative)

This note captures potential math upgrades for the NSC baseline codec. The ideas
here are intentionally optional and must preserve determinism across hardware.
They do not change the on-wire format unless explicitly stated.

## Video Coding

- **Integer motion compensation**: add deterministic block matching
  (integer-pel, diamond/hex search) with bounded search windows to reduce
  residual energy without introducing non-determinism.
- **Rate-model RDO for rANS**: replace the fixed 24-bit token estimate with
  context-aware rate costs derived from rANS symbol tables (`-log2(p)` lookup),
  keeping the DP optimizer integer-only.
- **Adaptive quantization**: scale the quant matrix per block using block
  variance/contrast sensitivity in integer math to target perceptual quality
  without changing the bitstream grammar.
- **Deterministic post-filter**: add an in-loop deblock/dering filter with fixed
  coefficients (no floating point) to reduce ringing at low bitrates.

## Audio Coding

- **LPC + Rice residuals**: deterministic prediction + Rice coding can
  outperform ADPCM while keeping the implementation small and fixed-point.
- **Fixed-point MDCT**: a short-window MDCT with coarse psychoacoustic weighting
  yields better quality than ADPCM and remains hardware-stable when implemented
  with integer arithmetic.

## Acceleration

- **SIMD/GPU**: add SIMD kernels for the fixed-point DCT/IDCT and bundle rANS
  paths behind feature flags to improve throughput while preserving bit-exact
  output.

## Determinism Requirements

- No data-dependent parallel reductions.
- No floating-point math in the new paths.
- Feature flags must preserve deterministic fallbacks.
