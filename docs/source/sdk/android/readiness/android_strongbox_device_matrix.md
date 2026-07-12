# Kagemusha Android production matrix

This matrix is the physical-device release gate for the single Kagemusha
offline-cash protocol. Every accepted slot is bound to native bridge ABI 19,
the packaged Kagemusha recursive-spend prover, the application signing
certificate, the wallet policy, and the exact application artifact.

## Required device families

| Device family | Minimum OS | Required evidence |
| --- | --- | --- |
| Google Pixel 6 / 6a | Android 14 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |
| Google Pixel 7 / 7 Pro | Android 14 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |
| Google Pixel 8 / 8a / 8 Pro | Android 15 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |
| Google Pixel Fold / Tablet | Android 15 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |
| Samsung Galaxy S23 | Android 14 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |
| Samsung Galaxy S24 | Android 15 | StrongBox/KeyMint attestation and complete Kagemusha lifecycle |

One signed slot is required for every row. Slots must not reuse a device
fingerprint or attestation challenge. Emulator and summary-only results do not
satisfy this gate.

## Slot contract

`slot.json` and `evidence/signed-evidence.json` are closed schemas. They bind:

- the canonical device family, model, codename, fingerprint, OS build, and
  minimum OS;
- the app package, signing-certificate digest, Kagemusha wallet artifact and
  policy digests;
- the StrongBox/KeyMint certificate chain and challenge;
- native bridge ABI 19 and successful recursive-spend FFI/JNI/prover states;
- one-use key rotation and rollback rejection;
- QR, NFC HCE, and nearby-offline transfer transcripts;
- the exact raw test commands and every referenced artifact digest.

The lifecycle evidence must prove exact fractional value conservation, sender
change, recursive multihop spending, durable receiver acknowledgement,
independent branch redemption, duplicate rejection, and zero network traffic
during peer transfers. Artifact paths must remain inside the slot and every
digest must be canonical lowercase SHA-256.

The required raw commands are the canonical values exported by
`scripts/check_android_device_lab_slot.py`. They build the current
`core-jvm`/`client-android` SDK, run the Kagemusha recursive-spend prover test,
run the lifecycle instrumentation test, and export its bound evidence.

## Validation

Validate a complete production matrix with:

```bash
python3 scripts/check_android_device_lab_slot.py \
  --root artifacts/android/device_lab \
  --require-slot \
  --require-kagemusha-production-evidence \
  --require-kagemusha-standard-matrix \
  --trusted-signer-public-key <lab-public-key.pem>
```

The validator fails closed for missing families, unexpected fields, stale ABI,
invalid signatures, weak attestation, copied device bindings, unsafe paths,
artifact mutation, noncanonical values, or incomplete lifecycle evidence. Lab
private keys are runtime-only inputs and must never appear in metadata, logs,
or summaries.
