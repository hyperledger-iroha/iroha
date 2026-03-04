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
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Scanner-loop validation (sakura-storm 3 KB/s profile)

Use the same transport profile across all capture paths:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Validation targets:

- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Browser/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Acceptance:

- Full payload reconstruction succeeds with one dropped data frame per parity group.
- No checksum/payload-hash mismatches in the normal capture loop.
