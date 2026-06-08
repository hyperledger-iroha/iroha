# Android device-lab sample slot

Sample artefacts mirrored from the AND6 device-lab workflow. The slot below is
intentionally minimal and is not production evidence: each folder contains
placeholder files plus a `sha256sum.txt` manifest so CI and documentation
examples have a deterministic fixture to validate against.

Real slots captured from the device lab should follow the same layout under
`artifacts/android/device_lab/<slot-id>/`. Production Kagemusha slots must also
include `slot.json` and pass the strict Kagemusha matrix mode described below.

```
slot-sample/
  telemetry/telemetry.json          -- synthetic metrics capture
  attestation/report.json           -- placeholder StrongBox attestation summary
  queue/pending_queue.json          -- queued transaction example
  logs/runtime.log                  -- short log excerpt for the run
  sha256sum.txt                     -- hashes of the files above
```

Run `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab --require-slot`
to validate the sample slot. Point the script at real runs under
`artifacts/android/device_lab/<slot-id>/` with `--json-out` to capture a summary
for compliance packets. If `--slot` is used, pass only a single slot directory
name under the lab root; path separators, traversal, and secret-looking labels
are rejected before validation.

For production Kagemusha readiness, run:

```
python3 scripts/check_android_device_lab_slot.py \
  --root artifacts/android/device_lab \
  --require-slot \
  --require-kagemusha-production-evidence \
  --require-kagemusha-standard-matrix \
  --trusted-signer-public-key <lab-public-key.pem>
```

After a real device slot has telemetry, attestation, queue, log files, and
`slot.json`, generate its signed evidence artifact with:

```
python3 scripts/sign_android_device_lab_evidence.py \
  --slot artifacts/android/device_lab/<slot-id> \
  --private-key <runtime-only-lab-private-key.pem> \
  --public-key <lab-public-key.pem> \
  --signer-key-id <lab-signer-id>
```

The helper signs the canonical evidence payload, refreshes
`signed_evidence_artifact_path`, `signed_evidence_artifact_sha256`, and
`sha256sum.txt`, then runs the same production metadata validation used by the
readiness checker. The private key path is a runtime-only input and is never
written to slot metadata, the signed artifact, or JSON summaries.

For the release-level rollup, run:

```
python3 scripts/kagemusha_production_readiness.py \
  --device-lab-root artifacts/android/device_lab \
  --trusted-signer-public-key <lab-public-key.pem> \
  --min-signed-at-utc 2026-06-06T00:00:00Z \
  --max-signed-at-future-skew-seconds 300 \
  --summary-out dist/kagemusha-production-readiness.json
```

The rollup combines the ABI-6 Reserved-lineage manifest check, ABI-7
recursive-compact fail-closed contract, signed Android slot validation, and
standard family coverage. It exits non-zero with explicit blockers until every
required production evidence row is present, valid, signed on or after the
release cutoff, and not future-dated beyond the validator clock-skew allowance.
It also rejects secret-looking `--repo-root`, `--device-lab-root`,
`--trusted-signer-public-key`, or `--summary-out` path arguments before writing
summaries, so operator-local tokens are not persisted in release evidence.
Successful summaries carry a local device-lab root label rather than the
absolute lab filesystem path, and the rollup does not print the absolute
summary output path.

That strict mode requires every standard Android device family in the release
matrix to have a slot with valid hashes, StrongBox/KeyMint metadata, rollback and
one-use key evidence, ABI-6 recursive spend probe success, and ABI-7 recursive
compact evidence with `abi7_recursive_compact_jni_probe = one_hop_verified` and
`abi7_recursive_compact_prover_state = multi_hop_proof_composition_unavailable`.
The `minimum_os` field must match the release matrix for that device family.
`slot.json` must also bind
`app_package_name`, `app_signing_certificate_sha256`,
`attestation_challenge_sha256`, `offline_wallet_policy_sha256`,
`offline_wallet_apk_path`, `offline_wallet_apk_sha256`,
`d2d_payment_transcript_path`, `d2d_payment_transcript_sha256`,
`wallet_integrity_transcript_path`, `wallet_integrity_transcript_sha256`, and
`native_bridge_abi_version` so lab attestation cannot be replayed across app
builds, wallet policies, APK bytes, D2D handoff evidence, wallet rollback
evidence, or stale native bridge surfaces. The release APK path, D2D transcript
path, and wallet integrity transcript path must stay inside the slot, each
SHA-256 must match those bytes, the D2D transcript path must stay under
`handoff/`, and the wallet integrity transcript path must stay under `wallet/`.
The native bridge ABI version is pinned to the ABI-7 surface used by the
recursive compact fail-closed probes. The
production `attestation/result.json` summary must repeat those app, challenge,
policy, device fingerprint, OS build, slot id, and StrongBox/KeyMint bindings.
If both `slot` and `slot_id` are present, they must match each other and the
slot directory.
It is also a closed schema, and every SHA-256 field in the summary must use
canonical lowercase hex.
Production `slot.json` is a closed schema; unexpected fields are rejected before
the validator accepts the slot or the signer helper writes evidence. `slot.json`
must include both `signed_evidence_artifact_path` and
`signed_evidence_artifact_sha256`; the validator checks that the referenced file
exists under `evidence/` and that its bytes match the declared SHA-256 digest. The
referenced signed evidence artifact
must use schema `iroha.android.device_lab.kagemusha.signed_evidence.v1`, repeat
the slot identity fields from `slot.json`, carry signer/signature metadata, and
include `artifact_digests` entries that match the required telemetry,
attestation, queue, log, wallet integrity, and D2D handoff files. It must repeat
the StrongBox, physical device attestation, one-use key, rollback, ABI probe,
D2D transcript, wallet integrity transcript, and raw command claims from
`slot.json`, so those claims are covered by the signature. The attestation summary must also report
`physical_device_attestation: true` plus the attestation certificate chain path
and SHA-256, so emulator, simulator, or summary-only lab runs cannot satisfy
production evidence. The chain path must reference a non-empty `.pem` or `.der`
artifact under `attestation/`; PEM files must carry certificate boundaries and
DER files must start with an ASN.1 SEQUENCE byte. The D2D transcript
must use schema `iroha.android.device_lab.kagemusha.d2d_payment.v1`, bind the
same slot/app/APK/policy values as `slot.json`, prove both wallets stayed
offline, prove the sent and received payload hashes match, prove receiver redeem
acceptance and duplicate-spend rejection, hash-bind the transport session,
one-use key id, and receiver ACK, prove payer/payee wallet-state hashes changed,
bind `queue_after_sha256` to `queue/pending_queue.json`, and be referenced from
`slot.json` under `handoff/`. The wallet
integrity transcript must use schema
`iroha.android.device_lab.kagemusha.wallet_integrity.v1`, bind the same
slot/app/APK/policy and attestation-chain values as `slot.json`, prove one-use
key rotation, old-key invalidation, stale-snapshot rejection, changed key ids,
changed wallet state after rotation, and active-state preservation after the
rollback attempt is rejected. The raw command list must include
`:client-android:assembleRelease`, `:offline-wallet-android:assembleRelease`,
`connectedAndroidTest`, and
`org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest` plus
`OfflineNoteTransferHandoff`. Its
`signed_at_utc` value must use canonical UTC
`YYYY-MM-DDTHH:MM:SSZ`. It must also include
`signer_public_key_sha256`, `signature_payload_sha256`, and an Ed25519
`signature` that verifies against a trusted signer public key supplied with
`--trusted-signer-public-key`. The signer pin is the SHA-256 of the public key
DER emitted by `openssl pkey -pubin -pubout -outform DER`.

When `--json-out` is used, the summary includes the Kagemusha required,
covered, and missing device-family lists plus trusted signer public-key SHA-256
pins. The production readiness rollup also rejects copied matrix rows that reuse
the same device fingerprint or attestation challenge across multiple slots. It
records signer pins only; trusted signer key file paths and the
absolute device-lab root are not written to the summary. Manifest and artifact
paths that contain secret-looking material are reported with a redacted path
label in stderr and JSON summaries.
