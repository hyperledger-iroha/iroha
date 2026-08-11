# Kagemusha Android production matrix

This matrix is the physical-device release gate for the single Kagemusha
offline-cash protocol. Measurements are collected from a clean, unsigned V4
candidate before release finalization, through the opt-in
`kagemusha-candidate-evidence-lab` build only. Every accepted V2 slot is bound
to native bridge ABI 22, the exact candidate record and source tree, the
ordered eight recursive artifacts, the lab native library and APK, the real
recursive-spend lifecycle transcript, the application signing certificate,
and the wallet policy. The ordinary production capability must remain false
throughout the lab run.

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

`slot.json`, `evidence/signed-evidence.json`, and
`evidence/candidate-binding-v2.json` are closed V2 schemas. V1 evidence cannot
satisfy the production-evidence gate. V2 binds:

- the canonical device family, model, codename, fingerprint, OS build, and
  minimum OS;
- the app package, signing-certificate digest, Kagemusha wallet artifact and
  policy digests;
- the StrongBox/KeyMint certificate chain and challenge;
- the clean candidate record, manifest, source commit/tree, generation, native
  bridge ABI 22, exact Eq/Ep KRV4 framed and payload identities, and the
  native-accepted inventory digest;
- the marker-bearing lab native library and APK, while proving the production
  capability stayed false;
- exact atomic-value conservation, multi-hop verification, independent branch
  redemption, duplicate rejection, restart recovery, and zero peer-transfer
  network requests in `evidence/lifecycle-transcript-v2.json`;
- one-use key rotation and rollback rejection;
- QR, NFC HCE, and nearby-offline transfer transcripts;
- the exact raw test commands and every referenced artifact digest.

The candidate-lab APK is a separate, marker-bearing application and must be
bound by `candidate_lab_apk_path`/`candidate_lab_apk_sha256`. It must never be
substituted for the wallet APK bound by
`kagemusha_wallet_apk_path`/`kagemusha_wallet_apk_sha256`; the latter remains
the independently measured StrongBox, rotation, rollback, and D2D wallet
artifact.

The lifecycle evidence must prove exact fractional value conservation, sender
change, recursive multihop spending, durable receiver acknowledgement,
independent branch redemption, duplicate rejection, and zero network traffic
during peer transfers. Artifact paths must remain inside the slot and every
digest must be canonical lowercase SHA-256.

The required raw commands are the canonical values exported by
`scripts/check_android_device_lab_slot.py`. They build the current SDK plus the
nonshipping candidate-lab APK, run the two AndroidJUnitRunner lifecycle/export
classes on the physical device, and export only the files and measurements
observed during that run. The candidate-lab feature, symbols, marker, APK, and
native library are forbidden from every production AAR/XCFramework/package.

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
