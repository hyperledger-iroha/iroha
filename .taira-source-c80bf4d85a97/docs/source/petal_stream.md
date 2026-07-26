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
- **Katakana presets:** `--channel katakana-base94` accepts
  `--katakana-preset balanced|distance-safe` when `--grid-size 0` lets the CLI
  choose geometry automatically. `balanced` is the default and prefers grid size
  `41` or larger; `distance-safe` prefers grid size `33` or larger for larger
  per-cell boxes at longer camera distances. An explicit `--grid-size` always
  overrides the preset floor.

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

- `encode` for deterministic PNG output of the decode-critical binary Petal
  grid or deterministic Katakana-base94 command tiles plus `manifest.json`.
  Feature-gated GIF output for both channels, including bounded multi-frame
  animation via `--animation-frames`, is also available when `iroha_cli` is
  built with `--features offline-visual-codecs`.
- `eval-capture` for replaying binary-grid or Katakana-base94 PNG frames, plus
  GIF manifests when built with `--features offline-visual-codecs`, through the
  deterministic Petal decoder. It can optionally apply deterministic capture
  perturbation and gate on a basis-point or decimal success ratio.
- `simulate-realtime` for replaying binary-grid or Katakana-base94 PNG frames,
  plus feature-gated GIF manifests, in deterministic loop order, optionally
  applying deterministic capture perturbation, and writing the first recovered
  payload.
- `score-styles`, which exercises the deterministic Petal capture scorer for
  binary-grid or Katakana-base94 style candidates.

Implemented binary-grid PNG `encode` example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format png --style sora-temple --channel binary-grid --dimension 1024 --animation-frames 1
```

The encode manifest uses schema `iroha.offline.petal.encode.v1` and records the
input path, output directory, payload size, format, style, channel, Katakana
preset when applicable, fps, animation frame count, dimension,
requested/resolved grid options, rendered file paths, and each file's encoded
frame count. For PNG, `--animation-frames <n>` writes `n` deterministic PNG
files named `frame_0000.png`, `frame_0001.png`, and so on.
`--animation-frames` must be in `1..=120`.

Feature-gated GIF `encode` example:

```bash
cargo run -p iroha_cli --bin iroha --features offline-visual-codecs -- --machine offline petal encode --input payload.bin --output ./petal_out --format gif --style sora-temple --channel binary-grid --dimension 1024 --fps 24 --animation-frames 24
```

Without `offline-visual-codecs`, `--format gif` fails before rendering and tells
the operator which feature to enable. With the feature enabled, GIF output is a
single animated file whose manifest entry records `encoded_frame_count`.

Feature-gated Katakana base94 GIF example:

```bash
cargo run -p iroha_cli --bin iroha --features offline-visual-codecs -- --machine offline petal encode --input payload.bin --output ./petal_out --format gif --channel katakana-base94 --style sora-temple-command --dimension 1024 --fps 24 --animation-frames 24
```

Implemented Katakana base94 PNG example:

```bash
iroha offline petal encode --input payload.bin --output ./petal_out --format png --channel katakana-base94 --style sora-temple-command --dimension 1024 --katakana-preset distance-safe
```

The current Katakana renderer writes RGB PNG/GIF command tiles while preserving
the decode-critical luminance value at each sampled cell center and leaving
calibration cells solid. This lets `eval-capture` and `simulate-realtime` use
the same sample decoder as the binary-grid channel for PNG and feature-gated
GIF manifests. Use `--katakana-preset balanced` or omit the flag for ordinary
camera distances; use `--katakana-preset distance-safe` with a larger
`--dimension` for longer-distance validation. Passing `--grid-size <n>` keeps
that exact grid size and records the requested/resolved value in the manifest.

Implemented binary-grid `eval-capture` example:

```bash
iroha offline petal eval-capture --input-dir ./petal_out/png --channel binary-grid --profile default --min-success-ratio 0.95 --output-report ./petal_out/capture_eval.json
```

The current `eval-capture` path finds the encode `manifest.json`, samples each
PNG frame or feature-gated GIF internal frame at deterministic cell centers,
decodes the Petal frame, and fails if the success ratio drops below
`--min-success-ratio` or `--min-success-ratio-bps`. It reports both executed
attempts (`attempts`) and total scheduled attempts (`planned_attempts`) plus
`aborted_early=true` when the remaining frames can no longer satisfy the gate.
Manifest-free directories are supported only for PNG frames and only when
`--grid-size` is supplied.

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
Additional deterministic cell-grid perturbation flags are available under
`--perturb-capture`:

- `--capture-downscale-cells <1..=8>` averages sampled cells into square blocks
  before decoding to simulate a lower effective camera resolution.
- `--capture-blur-radius <0..=4>` applies deterministic box blur over sampled
  cells before decoding.
- `--capture-motion-blur-cells <0..=8>` applies deterministic horizontal motion
  blur over sampled cells before decoding.
- `--capture-noise-amplitude <0..=64>` adds deterministic seeded per-cell sensor
  noise before decoding.
- `--capture-exposure-offset <-255..=255>` shifts sampled luminance with
  saturating bounds before decoding.

These values are recorded in the JSON report and also apply to Katakana-base94
PNG/GIF replay because the renderer preserves decode-critical center luminance.
Renderer-specific Katakana style scoring beyond the current binary-grid style
sets remains planned.

Implemented PNG/GIF `simulate-realtime` example:

```bash
iroha offline petal simulate-realtime --input-dir ./petal_out/png --channel binary-grid --profile default --simulate-fps 24 --realtime-loops 3 --output-payload ./petal_out/realtime_decoded.bin --output-report ./petal_out/realtime_report.json
```

The current `simulate-realtime` path replays rendered binary-grid or
Katakana-base94 PNG frames, plus feature-gated GIF internal frames, in manifest
order with deterministic looped playback via `--realtime-loops <n>`.
It reports `loop_index` and `source_index` per attempt, records the first
successful source frame, and writes `--output-payload` only after a frame decodes
successfully.
When `--perturb-capture` is enabled, realtime attempts expand to
`loop * source frame * capture attempt`; the report adds
`capture_attempt_index` and records the first successful capture-attempt index.
For distance-safe Katakana validation, encode with `--katakana-preset
distance-safe` and keep `--dimension` large enough that realtime results reflect
the larger-box operating point.

Implemented `score-styles` example (repeatable style ranking report):

```bash
iroha offline petal score-styles --input payload.bin --output-report ./petal_out/style_score.json --profile default --fps 24 --target-effective-bps 3000
```

`score-styles` is implemented as the core deterministic capture/report gate for
the published `sora-temple-default` style set. The report includes the selected
profile, channel, Katakana preset when applicable, seed, requested and resolved
grid options, per-style channel/preset metadata, per-style capture profile,
capture attempts/successes, `capture_success_ratio_bps`,
`effective_payload_bytes_per_second`, `effective_payload_bits_per_second`,
`throughput_score_bps`, `overall_score_bps`, and `recommended_style`. The
`--target-effective-bps` threshold is evaluated in bits per second; the byte/sec
field is included for operator readability.

Use `--style-set sora-temple-expanded` to score the default `sora-temple`
candidate plus the deterministic `sora-temple-high-contrast` hardening
candidate. The high-contrast candidate preserves the capture-attempt and jitter
settings while widening the dark/light luminance separation with saturating
bounds. If both candidates tie, the report keeps `sora-temple` as the
recommendation for the default baseline; under a collapsed low-contrast profile
such as `dark_luma=128`, `light_luma=129`, `luminance_jitter=0`, the expanded
set recommends `sora-temple-high-contrast`.

For Katakana-base94 scoring, pass `--channel katakana-base94`. The default
candidate becomes `sora-temple-command`, `--katakana-preset` resolves the same
balanced or distance-safe grid floors used by `encode`, and
`--style-set sora-temple-expanded` adds
`sora-temple-command-high-contrast`. Under the same collapsed low-contrast
profile, the Katakana expanded set recommends the high-contrast command
candidate while preserving deterministic sample-decoder behavior.

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
