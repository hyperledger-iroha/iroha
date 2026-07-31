# Physical-iPhone Kagemusha candidate evidence

This lane proves the Taira-testnet offline-cash lifecycle on a paired physical
iPhone. It is candidate evidence, not a production release. The native build
contains `KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2`, has one
`ios-arm64` slice, and has no simulator or production-capability path.

The accepted testnet policy is
`taira-testnet-physical-ios-xcode-paired-v1`. App Attest and Secure Enclave
attestation are intentionally not part of this Taira-testnet policy because
they can require network access. Evidence must state `app_attest_used:false`;
it must never fabricate an attestation. A production policy must define its
own hardware-attestation requirement.

## Preconditions

- Exact Xcode 26.6 and XcodeGen 2.46.0.
- Rust 1.93.1 with the `aarch64-apple-ios` target.
- One paired, available physical iPhone with Developer Mode enabled.
- Airplane mode enabled, Wi-Fi disabled, and no wired network route. The test
  fails unless every real `NWPathMonitor` sample is `unsatisfied`.
- An Apple development team authorized to sign the disposable host and test
  bundles.
- The candidate record, standalone Norito manifest, finality roster, exact
  eight KRV4 artifacts, exact 33-file scenario, and independently reviewed
  dirty-source closure.
- All inputs and output parents are real, owner-private paths outside the
  repository. Signing keys remain runtime-only.

Do not use a simulator when the phone is disconnected. The runner treats
`tunnelState != connected`, unavailable DDI services, an unpaired device, or a
non-physical reality as a hard stop.

## Build the candidate-only native slice

Use the source identity from the newly reviewed closure after all source
changes have been sealed:

```sh
scripts/build_kagemusha_candidate_apple_native.sh \
  --candidate-record /private/inputs/candidate-v4.norito \
  --source-commit "$SOURCE_COMMIT" \
  --source-tree-sha256 "$SOURCE_TREE_SHA256" \
  --reviewed-source-closure /private/inputs/reviewed-source-closure-v1.json \
  --reviewed-source-closure-sha256 "$REVIEWED_SOURCE_CLOSURE_SHA256" \
  --target-dir /private/build/kagemusha-ios-cargo \
  --output-dir /private/build/kagemusha-ios-native
```

The builder source-seals before and after Cargo, compiles only
`aarch64-apple-ios` with `kagemusha-candidate-evidence-lab`, verifies the two
Apple phase symbols and do-not-ship marker, and writes a closed native build
manifest. Both `--target-dir` and `--output-dir` must name new paths beneath
existing real, owner-private parents; the builder refuses existing paths so a
stale native artifact cannot be reused. The device runner rehashes every
manifest-listed XCFramework input, compiles from a private staged copy,
rehashes it after Xcode completes, and retains the consumed plist, marker,
headers, module map, and static archive in the signed raw inventory.

## Run the two physical launches

Keep the raw device selector in process arguments only; never copy it into a
repository file or retained evidence:

```sh
scripts/run_kagemusha_candidate_ios_lab.sh \
  --device-id "$RUNTIME_DEVICE_SELECTOR" \
  --development-team "$APPLE_DEVELOPMENT_TEAM" \
  --candidate-record /private/inputs/candidate-v4.norito \
  --candidate-manifest /private/inputs/manifest-v4.norito \
  --topup-finality-roster /private/inputs/topup-finality-roster-v4.norito \
  --artifact-root /private/inputs/artifacts \
  --scenario-root /private/inputs/scenario \
  --reviewed-source-closure /private/inputs/reviewed-source-closure-v1.json \
  --native-build-root /private/build/kagemusha-ios-native \
  --evidence-root /private/evidence/kagemusha-ios-run
```

The runner performs these exact device operations:

1. `xcrun devicectl list devices --json-output <transient>` and fail-closed
   physical/paired/available checks.
2. `xcodebuild build-for-testing` for `platform=iOS,id=<runtime selector>`.
3. `xcrun devicectl device install app`.
4. `xcrun devicectl device copy to --domain-type appDataContainer` to stage
   the candidate, eight artifacts, and scenario outside the app bundle.
5. One `xcodebuild test-without-building` restricted to `testProofPhase`.
6. `xcrun devicectl device copy from` to retain the fsynced checkpoint and
   proof receipt, followed by exact external restaging.
7. A second `xcodebuild test-without-building` restricted to
   `testRestartPhase`.
8. A final `devicectl copy from` for the native transcript and restart
   receipt.

Raw CoreDevice JSON, provisioning profiles, XCResult bundles, and build logs
remain transient and are deleted. Retained device identity consists only of
SHA-256 values for UDID, ECID, serial number, identifier-for-vendor, and boot
session.

## What constitutes complete evidence

The native transcript must contain exactly 28 ordered causal events:
candidate install; init request/init; two append request/append pairs;
candidate reinstall after a distinct process restart; three result restores;
five branch/continuity validations; two proof-request/proof pairs; the
observed-branch duplicate request and exact `-311` rejection; and three
redeem-request/redeem pairs.

It must also bind:

- distinct process IDs and launch nonces;
- the same physical device, boot session, install identity, app identity,
  test identity, and native artifact identity;
- source commit, dirty source-tree hash, and nonzero reviewed-closure digest;
- the candidate record, Norito manifest, finality roster, scenario inventory,
  and ordered eight-artifact inventory;
- the fixed 6 GiB RSS ceiling;
- exactly two proof hops;
- initial value equal to first recipient plus second recipient plus sender
  change;
- redeemed value equal to initial value and final unspent value equal to zero;
- real `NWPathMonitor` samples that stay `unsatisfied` before, through, and
  after both launches; and
- a measured URL-loading request count of zero.

A simulator run, an XCTest/XCResult summary without the raw receipts and native
transcript, a caller-selected resource ceiling, or an unsigned JSON summary is
not evidence.

## Sign and verify

After the run, use the closed Ed25519 signer and validator:

```sh
scripts/sign_kagemusha_candidate_ios_evidence.py \
  --artifact-root /private/evidence/kagemusha-ios-run/raw \
  --private-key /run/secrets/kagemusha-ios-evidence-ed25519.pem \
  --public-key /run/secrets/kagemusha-ios-evidence-ed25519.pub.pem \
  --signer-key-id "$TRUSTED_KEY_ID" \
  --output /private/evidence/signed-evidence-v1.json

scripts/check_kagemusha_candidate_ios_evidence.py \
  --evidence /private/evidence/signed-evidence-v1.json \
  --artifact-root /private/evidence/kagemusha-ios-run/raw \
  --trusted-key-id "$TRUSTED_KEY_ID" \
  --trusted-public-key /run/secrets/kagemusha-ios-evidence-ed25519.pub.pem
```

The signed canonical payload excludes only
`signature` and `signature_payload_sha256`. The validator denies unknown
fields, verifies the trusted key identity and Ed25519 signature, rehashes the
entire exact raw-file inventory, and rechecks every cross-file invariant. It
implements strict RFC 8032 Ed25519 directly from immutable, no-follow PEM
snapshots, so a mutable `PATH` or a key-path replacement cannot select the
cryptographic result. Every retained JSON document must also use the exact
canonical byte envelope.

For promotion, copy only the signed JSON to the authenticated release as
`physical-device-benchmark.evidence`; retain the raw tree owner-private outside
the release directory. Arrange the external trees as
`$KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT/<final-manifest-sha256>/raw` and pass that
root plus the trusted key id/public key to
`ci/check_kagemusha_production_readiness.sh promotion`. The corridor accepts
the slot only after the signed raw candidate digest equals Kagami's immutable
candidate reconstructed from that finalized release.
