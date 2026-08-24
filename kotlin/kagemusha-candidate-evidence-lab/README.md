# Kagemusha candidate evidence lab (never ship)

This module is a marker-bearing Android application used only to gather real
native-bridge ABI-23 evidence from a physical ARM64 device before a candidate
is promoted.
Here, ABI-23 names the native bridge; the Kagemusha protocol envelope remains
ABI-21/V4.
It is not in the normal Gradle project graph, has no publication, has no
`INTERNET` permission, disables its release variant, and stores every build
intermediate under the exact candidate directory:

```text
artifacts/kagemusha-candidate-evidence/<candidate-record-sha256>/
```

The Rust bridge must be built with the candidate-lab-only feature and stored as
`evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so`. The Gradle lab
renames it inside the APK so it cannot masquerade as the production library. Both it and the
APK carry `KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2`. Production mobile
artifact checks reject that marker and every `candidate_lab` native symbol.

## Input contract

The candidate directory must contain the exact immutable, unsigned, clean
candidate record and manifest, the eight canonical KRV4 artifacts, and a
scenario generated from that same candidate:

```text
evidence/candidate/candidate-v4.norito
evidence/candidate/manifest-v4.norito
evidence/candidate/artifacts/step-eq.params-ipa.krv4
evidence/candidate/artifacts/step-eq.proving-key.krv4
evidence/candidate/artifacts/step-eq.verifying-key.krv4
evidence/candidate/artifacts/step-eq.bootstrap-witness.krv4
evidence/candidate/artifacts/step-ep.params-ipa.krv4
evidence/candidate/artifacts/step-ep.proving-key.krv4
evidence/candidate/artifacts/step-ep.verifying-key.krv4
evidence/candidate/artifacts/step-ep.bootstrap-witness.krv4
scenario/init-top-up-anchor-v4.norito
scenario/init-top-up-finality-proof-v2.norito
scenario/init-top-up-finality-roster-artifact-v2.norito
scenario/init-opening-v2.norito
scenario/init-output-membership-v4.norito
scenario/transfer-verifier-commitment-v2.bin
scenario/append-hop-01-recipient-request-v2.norito
scenario/append-hop-01-recipient-opening-v2.norito
scenario/append-hop-01-change-opening-v2.norito
scenario/append-hop-01-output-membership-v4.norito
scenario/append-hop-01-operation-id.bin
scenario/append-hop-01-block-height.txt
scenario/append-hop-01-verified-at-ms.txt
scenario/append-hop-02-recipient-request-v2.norito
scenario/append-hop-02-recipient-opening-v2.norito
scenario/append-hop-02-change-opening-v2.norito
scenario/append-hop-02-output-membership-v4.norito
scenario/append-hop-02-operation-id.bin
scenario/append-hop-02-block-height.txt
scenario/append-hop-02-verified-at-ms.txt
scenario/redeem-recipient-account-id.txt
scenario/unshield-verifier-commitment-v2.bin
scenario/redeem-hop-01-operation-id.bin
scenario/redeem-hop-01-block-height.txt
scenario/redeem-hop-02-operation-id.bin
scenario/redeem-hop-02-block-height.txt
scenario/redeem-sender-change-operation-id.bin
scenario/redeem-sender-change-block-height.txt
scenario/duplicate-input-recipient-request-v2.norito
scenario/duplicate-input-output-membership-v4.norito
scenario/duplicate-input-operation-id.bin
scenario/duplicate-input-block-height.txt
scenario/duplicate-input-verified-at-ms.txt
evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so
```

These are primitive construction inputs, never precomputed lifecycle requests
or results. On the device, the native builder creates init from the exact
anchor/finality/roster/opening inputs. The first append is built from the exact
bundle, provenance, and membership witness projected from that init result.
The second append is then built from the exact change projected from the first
append. Both appends must produce change.

After the forced process restart, the harness restores the exact native result
archives and private openings from owner-only files in `noBackupFilesDir`. It
independently validates every restored branch against the installed candidate,
builds two recipient verification requests and three full-redemption requests
from those observed branches, and consumes the first recipient, second
recipient, and final sender change so the observed unspent value reaches zero.
The negative-test builder accepts one exact, already validated observed branch
and duplicates it internally; `nativeAppendV4` must reject that reuse. Static
append, verify, redeem, duplicate-input, and result archives are prohibited.
Native code authenticates the candidate record and every framed/payload
artifact binding; the Android harness does not accept a JVM substitute.
The eight KRV4 files are deliberately not APK assets: the two proving keys are
larger than Android's practical APK/ZIP installation corridor. After the small
marker-bearing APKs are installed and package data is cleared, the runner
streams each file with `adb shell -T run-as` into an owner-only incoming
directory below `noBackupFilesDir`, verifies its on-device size and SHA-256,
writes a candidate/stage/count binding, and atomically renames the complete set
into place. A free-space preflight reserves room for the external files, one
complete native spool, and 1 GiB of working space. The harness rejects symlinks,
non-`0600` files, wrong owners, extra or missing files, stale candidate/stage
bindings, size mismatches, and SHA-256 mismatches while it streams the same
bytes into native authentication. The Gradle staging tasks also open both final
APKs as ZIP archives, reject every KRV4 basename, and enforce a 64 MiB APK cap.
There is no asset fallback.
All openings and recipient material in this scenario must belong to disposable
candidate-only test notes. They are embedded in the unmistakably non-shipping
lab APK and must never be keys or openings for production funds.

## Physical-device run

Use `scripts/run_kagemusha_candidate_android_lab.sh`. It builds only the debug
lab application without KRV4 payloads, installs the marker-bearing APKs, stages
the exact external artifact set with
`scripts/stage_kagemusha_candidate_android_artifacts.py`, and launches these
exact classes in separate `AndroidJUnitRunner` processes:

```text
org.hyperledger.iroha.sdk.kagemusha.candidate.lab.KagemushaCandidateLifecycleInstrumentedTest
org.hyperledger.iroha.sdk.kagemusha.candidate.lab.KagemushaCandidateArtifactExportInstrumentedTest
```

The lab application is staged as
`evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-<candidate-record-sha256>-debug.apk`.
It is intentionally distinct from the production wallet APK that the device
slot binds for the separate StrongBox and D2D corridor.

Before Gradle runs, the runner invokes this exact current-source native build:

```bash
scripts/build_kagemusha_candidate_android_native.sh \
  --candidate-sha256 "$CANDIDATE_SHA256" \
  --stage-sha256 "$STAGE_SHA256" \
  --source-commit "$SOURCE_COMMIT" \
  --source-tree-sha256 "$SOURCE_TREE_SHA256" \
  --reviewed-source-closure "$REVIEWED_SOURCE_CLOSURE" \
  --reviewed-source-closure-sha256 "$REVIEWED_SOURCE_CLOSURE_SHA256"
```

That helper runs `cargo ndk -t arm64-v8a` with
`--features kagemusha-candidate-evidence-lab`, a candidate-scoped Cargo target,
and canonical full-source-tree seal checks before and after compilation. It
requires the independently reviewed canonical source-closure descriptor and
its SHA-256 pin to match the staged candidate validation report, accepts no
prebuilt or production `.so`, refuses a dirty or mismatched Git index/worktree
(including untracked files), validates the result as marker-bearing AArch64
ELF, and atomically writes only the candidate lab native path.

The device must have no active Android network. The first process dynamically
builds and performs real init and two-hop proving, then persists the observed
native result archives and the minimum private opening state in the app's
no-backup directory. Private state is mode `0600`, is never exported, and is
deleted after the restart phase. The second process re-installs the exact same
candidate, reprojects and independently validates those branches, dynamically
builds and performs real verify/redeem, proves value conservation, observes
duplicate-input rejection, and exports only:

```text
evidence/candidate-binding-v2.json
evidence/lifecycle-transcript-v2.json
```

Neither file asserts success on behalf of native code: all hashes, amounts,
hop counts, durations, identities, and rejection details come from calls made
on the physical device.
