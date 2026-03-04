# Android device-lab sample slot

Sample artefacts mirrored from the AND6 device-lab workflow. The slot below is
intentionally minimal: each folder contains a single placeholder file plus a
`sha256sum.txt` manifest so CI and documentation examples have a deterministic
fixture to validate against.

Real slots captured from the device lab should follow the same layout under
`artifacts/android/device_lab/<slot-id>/`.

```
slot-sample/
  telemetry/telemetry.json          -- synthetic metrics capture
  attestation/report.json           -- placeholder StrongBox attestation summary
  queue/pending_queue.json          -- queued transaction example
  logs/runtime.log                  -- short log excerpt for the run
  sha256sum.txt                     -- hashes of the files above
```

Run `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab --require-slot` to
validate the sample slot or point the script at real runs under
`artifacts/android/device_lab/<slot-id>/` with `--json-out` to capture a summary
for compliance packets.
