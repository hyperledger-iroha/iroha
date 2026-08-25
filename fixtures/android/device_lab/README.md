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
also contain the candidate-bound V2 `slot.json` schema, mobile bridge ABI22 / Kagemusha
data ABI V4 candidate-lab results, StrongBox/KeyMint evidence, exact lifecycle transcripts,
the clean candidate and exact ordered eight KRV4 artifacts, the lab native
library, the distinct marker-bearing candidate-lab APK, the independently
attested wallet APK, and an Ed25519-signed
`evidence/signed-evidence.json`. V1 evidence is retained only for historical
parsing and cannot satisfy the production-evidence gate.

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
