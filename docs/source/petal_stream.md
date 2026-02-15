---
title: Petal Stream Transport
---

## 1. Overview

Petal Stream is an optional, custom optical transport for offline payloads. It reuses the
`QrStreamFrame` bytes produced by the existing QR stream encoder but renders them as a
stylized optical frame instead of a rigid QR grid. Petal Stream **requires a custom
scanner** on each platform; standard QR scanners will not decode it.

## 2. Frame header (PS1)

Each Petal Stream frame carries a short header before the payload bytes:

| Field | Size | Notes |
| --- | --- | --- |
| magic | 2 bytes | ASCII `PS` (`0x50 0x53`) |
| version | 1 byte | `1` |
| payload_len | 2 bytes | Little-endian payload length in bytes |
| payload_crc32 | 4 bytes | CRC32 of payload bytes (same CRC32 as QR stream) |

The payload bytes are the raw `QrStreamFrame::encode()` output.

## 3. Grid layout

Frames are mapped into a square grid of `grid_size × grid_size` cells.

- **Border (dark):** the outermost ring of cells is always `1` (dark).
- **Anchors:** 3×3 blocks inside the border are reserved for calibration:
  - Top-left and bottom-left anchors are **dark** (`1`).
  - Top-right and bottom-right anchors are **light** (`0`).
- **Data cells:** all remaining cells are filled row-major (x then y) with the header
  bits followed by payload bits (MSB-first per byte).
- **Stream sizing:** choose a single `grid_size` for the whole stream based on the
  largest frame so scanners do not have to rescale between frames. The reference
  encoder uses the canonical size ladder `33..69` in steps of 4 (see
  `PETAL_STREAM_GRID_SIZES`).

If the header + payload bits exceed capacity, the encoder must choose a larger grid size
or fail.

## 4. Decoding and calibration

The decoder assumes the Petal Stream frame fills a square crop (similar to QR scanning).

1. Convert the crop to luminance.
2. Sample each cell by averaging a small sub-grid of pixels.
3. Compute `dark_avg` from the dark anchors and `light_avg` from the light anchors.
4. Classify each cell as `dark` if `sample < (dark_avg + light_avg) / 2`.
5. Reconstruct header + payload bytes; verify the CRC32 before accepting.

If CRC fails, the frame should be rejected and the QR stream assembler can recover using
parity frames.

If the grid size is not known ahead of time, attempt the canonical size ladder and
accept the first size that yields a valid header + CRC.

## 5. Animation guidance

The `sora-temple` renderer keeps all payload bits in Petal grid cells, and adds
data-derived ornamentation:

- Data cells are rendered as katakana tiles (iroha ordering, including archaic forms).
- The central SORA logo silhouette (`天`) is composed from high-density data tiles.
- Three concentric ring bands are dotted with redundant, data-driven symbols.
- Anchor cells remain high-contrast for threshold calibration.

Use luminance contrast (not hue alone) for the decode-critical layers so camera color
pipelines do not collapse bit separation.

## 6. CLI preview

`encode` example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format gif --fps 24 --style sora-temple
```

`eval-capture` example (distance/motion robustness gate):

```bash
iroha offline petal eval-capture --input-dir ./petal_out/png --profile default --min-success-ratio 0.95 --output-report ./petal_out/capture_eval.json
```

`eval-capture` applies deterministic perturbations (distance downscale, blur, motion blur,
jitter, exposure/noise shifts), decodes each perturbed frame, and fails if the success ratio
drops below the configured threshold.

For operator-facing QR transport presets (`ecc`/dimension/fps) in noisy camera conditions, see
`docs/source/offline_qr_operator_runbook.md`.
