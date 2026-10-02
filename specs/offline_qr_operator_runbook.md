## Offline QR Operator Runbook

This runbook defines practical `ecc`/dimension/fps presets for camera-noisy
environments when using offline QR transport.

### Recommended presets

| Environment | Style | ECC | Dimension | FPS | Chunk size | Parity group | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Controlled lighting, short range | `sakura` | `M` | `360` | `12` | `360` | `0` | Highest throughput, minimal redundancy. |
| Typical mobile camera noise | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Preferred balanced preset (`~3 KB/s`) for mixed devices. |
| High glare, motion blur, low-end cameras | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Lower throughput, strongest decode resilience. |

### Encode/decode checklist

1. Encode with explicit transport knobs.
2. Validate with scanner-loop capture before rollout.
3. Pin the same style profile in SDK playback helpers to keep preview parity.

Example:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 3 \
  --in payload.bin \
  --out out_dir
```

### Scanner-loop validation (sakura-storm 3 KB/s profile)

Use the same transport profile across all capture paths:

- `chunk_size=336`
- `parity_group=3`
- `fps=12`
- `style=sakura-storm`

Validation targets:

- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Browser/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Acceptance:

- Full payload reconstruction succeeds with one dropped data frame per parity group.
- No checksum/payload-hash mismatches in the normal capture loop.

### Petal Stream (custom optical transport)

When a standard QR scanner is not required, `iroha offline petal` plays the same payloads as an animated
`天` / katakana / dotted-ring stream (`specs/petal_stream.md`). It reads on phones where an animated QR does not:
its dot and polarity lanes survive defocus of 3–4 px and 480p previews that defeat QR modules.

| Environment | Display fps | Expect |
| --- | --- | --- |
| Sharp camera (720p+, in focus, steady hand) | 8–12 | Katakana turbo lane readable: about 110 B per frame, a 7.5 KB payment in about 9 s. |
| Slightly soft 720p camera (about 1.8 px blur) | 8 | Katakana lane in about half of the frames: the same payment in about 12 s. |
| Defocused or 480p camera | 8 (up to 12) | Only the polarity and dot lanes read: about 28 B per frame, the same payment in about 35 s. |

Rehearse with the simulator before choosing a rate for a device class:

```bash
iroha offline petal simulate --camera legacy --bytes 7552 --fps 8 --trials 4
iroha offline petal encode --input payment.bin --output out/ --kind 2 --fps 8 --format gif
```

Keep the whole square visible, full screen brightness, nothing bright within one finder diameter of the corner blossoms,
and do not run the animation faster than a third of the camera frame rate.

On the scanning phone, use a 1280×720 preview where the device keeps up, and lower the exposure by about one stop (two on old
cameras) or lock it once the code is in view: automatic exposure over-exposes a mostly black screen. The decoder copes with
2–3× over-exposure, veiling light and shadows at 720p, but a correct exposure keeps every lane. Tilt the screen away from
windows and lamps: a deep glare hides whatever it covers.
