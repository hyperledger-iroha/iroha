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
- **Katakana presets:** when `--channel katakana-base94` is used and the operator
  leaves both `--chunk-size` and `--grid-size` at defaults, the encoder applies
  a deterministic preset selected by `--katakana-preset`:
  - `balanced` (default): `chunk_size=176`, `grid_size>=41` with `41` preferred.
  - `distance-safe`: `chunk_size=96`, `grid_size>=33` with `33` preferred for
    larger per-cell boxes at longer camera distances. When `--parity-group` is not
    forced, this preset also defaults to `parity_group=4` for stronger recovery in
    camera-capture conditions.

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

## 6. CLI status

The implemented commands in the current CLI are:

- `encode` for deterministic single-frame PNG output of the decode-critical
  binary Petal grid plus `manifest.json`. Single-frame binary-grid GIF output
  is also available when `iroha_cli` is built with
  `--features offline-visual-codecs`.
- `eval-capture` for replaying binary-grid PNG frames through the deterministic
  Petal decoder, optionally applying deterministic capture perturbation, and
  gating on a basis-point or decimal success ratio.
- `simulate-realtime` for replaying binary-grid PNG frames in deterministic loop
  order, optionally applying deterministic capture perturbation, and writing the
  first recovered payload.
- `score-styles`, which exercises the core deterministic Petal grid and capture
  scorer.

Renderer-backed commands for multi-frame animated GIF output and Katakana visual
channels are planned but not wired yet.

Implemented binary-grid PNG `encode` example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format png --style sora-temple --channel binary-grid --dimension 1024
```

The encode manifest uses schema `iroha.offline.petal.encode.v1` and records the
input path, output directory, payload size, format, style, channel, fps,
dimension, requested/resolved grid options, and rendered frame paths.

Feature-gated binary-grid GIF `encode` example:

```bash
cargo run -p iroha_cli --bin iroha --features offline-visual-codecs -- --machine offline petal encode --input payload.bin --output ./petal_out --format gif --style sora-temple --channel binary-grid --dimension 1024 --fps 24
```

Without `offline-visual-codecs`, `--format gif` fails before rendering and tells
the operator which feature to enable.

Planned Katakana base94 balanced example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format png --channel katakana-base94 --style sora-temple-command --dimension 1024
```

Planned Katakana base94 distance-safe example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format png --channel katakana-base94 --katakana-preset distance-safe --style sora-temple-command --dimension 1024
```

Implemented binary-grid `eval-capture` example:

```bash
iroha offline petal eval-capture --input-dir ./petal_out/png --channel binary-grid --profile default --min-success-ratio 0.95 --output-report ./petal_out/capture_eval.json
```

The current `eval-capture` path finds the encode `manifest.json`, samples each
PNG frame at deterministic cell centers, decodes the Petal frame, and fails if
the success ratio drops below `--min-success-ratio` or
`--min-success-ratio-bps`. It reports both executed attempts (`attempts`) and
total scheduled attempts (`planned_attempts`) plus `aborted_early=true` when the
remaining frames can no longer satisfy the gate. Manifest-free PNG directories
are supported only when `--grid-size` is supplied.

Perturbed binary-grid `eval-capture` example:

```bash
iroha offline petal eval-capture --input-dir ./petal_out/png --channel binary-grid --profile default --perturb-capture --capture-seed 42 --capture-attempts 12 --min-success-ratio-bps 9500 --output-report ./petal_out/capture_eval_perturbed.json
```

With `--perturb-capture`, the sampled binary grid is re-rendered through the
same deterministic capture profile used by `score-styles`. The report records
`perturb_capture=true`, `capture_seed`, `capture_attempts_per_frame`, and the
effective capture profile. Profile override flags (`--capture-attempts`,
`--capture-dark-luma`, `--capture-light-luma`, and
`--capture-luminance-jitter`) fail closed unless `--perturb-capture` is present.
Renderer-backed capture evaluation still needs distance downscale, blur, motion
blur, exposure/noise shifts, and Katakana visual-channel decoding.

Implemented binary-grid `simulate-realtime` example:

```bash
iroha offline petal simulate-realtime --input-dir ./petal_out/png --channel binary-grid --profile default --simulate-fps 24 --realtime-loops 3 --output-payload ./petal_out/realtime_decoded.bin --output-report ./petal_out/realtime_report.json
```

The current `simulate-realtime` path replays rendered binary-grid PNG frames in
manifest order with deterministic looped playback via `--realtime-loops <n>`.
It reports `loop_index` and `source_index` per attempt, records the first
successful source frame, and writes `--output-payload` only after a frame decodes
successfully.
When `--perturb-capture` is enabled, realtime attempts expand to
`loop * source frame * capture attempt`; the report adds
`capture_attempt_index` and records the first successful capture-attempt index.
For distance-safe katakana validation, keep encode dimension at `1024` so realtime results
reflect the larger-box operating point.

Implemented `score-styles` example (repeatable style ranking report):

```bash
iroha offline petal score-styles --input payload.bin --output-report ./petal_out/style_score.json --profile default --fps 24 --target-effective-bps 3000
```

`score-styles` is implemented as the core deterministic capture/report gate for the
published `sora-temple-default` style set. The report includes the selected profile,
seed, requested and resolved grid options, capture attempts/successes,
`capture_success_ratio_bps`, `effective_payload_bytes_per_second`,
`effective_payload_bits_per_second`, `throughput_score_bps`, `overall_score_bps`,
and `recommended_style`. The `--target-effective-bps` threshold is evaluated in
bits per second; the byte/sec field is included for operator readability.

The default deterministic CLI baseline for the `sora-temple-capture-baseline`
payload is `recommended_style=sora-temple`, 12/12 successful decode attempts
(`capture_success_ratio_bps=10000`), `requested_grid_size=0`,
`resolved_grid_size=33`, `effective_payload_bits_per_second=5376`, and
`overall_score_bps=10000` against the `9500` capture gate and `3000` bps
throughput gate. The low-contrast adversarial profile (`dark_luma=128`,
`light_luma=129`) is pinned at 0/4 successful attempts so the gate fails closed
when luminance separation collapses.

For operator-facing QR transport presets (`ecc`/dimension/fps) in noisy camera conditions, see
`docs/source/offline_qr_operator_runbook.md`.
