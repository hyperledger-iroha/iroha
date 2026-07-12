# Kagemusha Android device-lab fixture

`slot-sample/` is a deterministic structural fixture for the Kagemusha
device-lab validator. It is not production evidence.

```text
slot-sample/
  telemetry/telemetry.json
  attestation/report.json
  queue/pending_queue.json
  logs/runtime.log
  sha256sum.txt
```

Validate the fixture with:

```bash
python3 scripts/check_android_device_lab_slot.py \
  --root fixtures/android/device_lab \
  --require-slot
```

Production slots live under `artifacts/android/device_lab/<slot-id>/` and must
also contain the closed `slot.json` schema, ABI 19 Kagemusha recursive-spend
probe results, StrongBox/KeyMint evidence, exact lifecycle transcripts, the
content-bound wallet artifact, and an Ed25519-signed
`evidence/signed-evidence.json`.

Validate a complete production matrix with:

```bash
python3 scripts/check_android_device_lab_slot.py \
  --root artifacts/android/device_lab \
  --require-slot \
  --require-kagemusha-production-evidence \
  --require-kagemusha-standard-matrix \
  --trusted-signer-public-key <lab-public-key.pem>
```

The signer key is a runtime-only lab input. It must never be copied into a slot
or written to validation output.
