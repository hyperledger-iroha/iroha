# Experimental device runtime fixture

This standalone crate links the current canonical Rust provider and complete experimental engine. It is installed only in disposable test application namespaces. Its public fixture issuer keys and synthetic ledger events cannot select production funds.

`./build-apple.sh` builds the real iPhone static archive in the persistent `target/aarch64-apple-ios/release/` lane. The explicit iOS 18 deployment floor applies to Rust and C dependencies. The Apple app project lives in `../ios/runtime-project.yml`; it consumes `SPARTAN_DEVICE_ARCHIVE` and `SPARTAN_DEVICE_ARTIFACTS`. The C callbacks are specified in `spartan_device_fixture.h`.

The final host recursive-chain test emits `program-bundle.norito`, `parameter-manifest.norito`, and `parameter-signature.bin` when `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS` names an output directory. They contain the actual eight fixed-point verifier programs and independent fixture-root authentication. No placeholder artifact is accepted. Five-kind device fixtures also carry `world-manifest-digest.bin`, the 32 raw bytes of the independently selected `manifest_digest` from the prepared fixture World. The initial launch and every resumed owner authenticate that selection before opening custody.

The run performs real native enrollment custody and existing Ed25519 account admission, actual hardware Bootstrap, restart recovery, a fixture-genesis BLS-certified synthetic Load100 event, and a Send10 to the fixture's software counterparty wallet B (B signs the Request), confirmed through the platform authentication UI. B then Bootstraps, verifies and Receives that Payment, and pays the same 10 back under a Request signed by this wallet's hardware key through the canonical Request domain. The return Payment is retained in `incoming-payment.bin`, and the launch that proved B's operations stops there (report `stage 1 of 2`): B's proving leaves memory resident that a receiving phone would not hold. The next launch, a fresh process, performs this wallet's Receive from the retained bytes to a verified, durable output in platform custody; that is the measured operation. The completed-Request relation refuses payer == receiver, so the earlier self-transfer cannot be prepared. B's key is a public fixture scalar and B has no platform custody; its three operations are proved in the same process by the same engine and installed programs and are measured separately (`Counterparty*`). Each custody operation checks retained custody before preparing. Re-running after process termination must return the same retained Payments without another Send. After the Receive, a new owner of the same custody (fresh parameter authentication, provider, runtime, C session and account admission, as after a process restart) must return the Receive's exact completion and the exact outgoing Payment through the C output entry, and the verified new head must show the credit and an unchanged burned total. Fresh Native admission refuses invalid Payments before selection. The recursive relation also supports historical Burn outcomes. The generated `complete-payment.bin` (this wallet's outgoing Payment) and `incoming-payment.bin` are complete peer objects; their actual sizes are reported separately from performance or qualification claims. `device-measurements.json` (schema v2) records every phase's wall time and its own peak RSS: exact on Linux/Android (VmHWM reset before the phase), a 5 ms sample maximum elsewhere (`phase_peak_method`; one extra fixture thread). Phases named `*_drill` are fixture checks; `receive_normal_flow_us` is the observed Receive minus its drills and measurement overhead.

The iPhone fixture shows the full Native asset, amount, fee, receiver account digest and wallet.
The operator chooses **Authenticate this Send** before Native opens its own Secure Enclave
user-presence prompt. Canceling either step produces no confirmed Send. The fixture uses
the same marker-pinned key and receipt path as the production bridge.

On iPhone the SHA-256 of the embedded `native-device/parameter-manifest.norito`
selects the fixture’s protected Native custody directory, Secure Enclave payment-key
tags and rollback-anchor service. Relaunch the same app to resume a generation.
The older unsuffixed storage and its keys remain available to the earlier fixture.

After each completed launch, copy the manifest-specific report into a fresh local
launch directory. Use the signed diagnostic app and the identifier of the phone
reserved for the run. The report includes its manifest SHA-256 and the operation result.
The Native file `device-measurements.jsonl` records the ordered launch history.

```sh
codesign --verify --deep --strict "$FIXTURE_APP" || exit 1
manifest_sha=$(shasum -a 256 "$FIXTURE_APP/native-device/parameter-manifest.norito" | cut -d' ' -f1)
app_id=jp.co.soramitsu.kagemusha.spartanfixture
xcrun devicectl device copy from --device "$FIXTURE_DEVICE" --domain-type appDataContainer \
  --domain-identifier "$app_id" --source "Documents/spartan-runtime-result-$manifest_sha.json" \
  --destination "$LAUNCH_DIRECTORY/result.json" || exit 1
xcrun devicectl device copy from --device "$FIXTURE_DEVICE" --domain-type appDataContainer \
  --domain-identifier "$app_id" \
  --source "Library/Application Support/kagemusha-wallet-v1/spartan-runtime-fixture-$manifest_sha/device-measurements.jsonl" \
  --destination "$LAUNCH_DIRECTORY/device-measurements.jsonl"
```

After copying all three launches, verify the Send, Receive and restart in order.
The verifier matches each report to the packaged programs, checks consecutive
Native histories and retained Payment hashes, and prints the measured Receive
time and process lifetime peak RSS. Keep the physical device identity and the
signed app with the original collection records.

```sh
python3 ../ios/verify-returned-launches.py --app "$FIXTURE_APP" \
  "$SEND_LAUNCH_DIRECTORY" "$RECEIVE_LAUNCH_DIRECTORY" "$RESTART_LAUNCH_DIRECTORY"
```

Host run (Mac or Linux, software P-256 keys from the canonical simulator, no user confirmation): `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS=<bundle dir> KAGEMUSHA_SPARTAN_DEVICE_ROOT=<new empty dir> cargo test --offline --release --lib -- --ignored --nocapture two_wallet_device_flow_on_host`. It makes the same three launches as a phone run: stage 1, the measured Receive, then a launch on the same custody that must return every output retained, with no new execution or confirmation. `device-measurements-launch-N.json` keeps each launch's record.

After fixture-root parameter authentication, the same exclusive provider enters the real bridge registry and Session through an isolated test-only constructor. Account admission, Request issuance and monetary operations call the production C exports. The fixture exercises C output allocation/free, close rejection, unused-review discard, exact execute-reply retry and retained-output checks. Its host path also checks the signed regulatory blacklist during preparation and selection and exercises CreditStatus issuance and verification through the C API. The fixture also rejects stale preparation on repeated launches. This exercises C dispatch on both phones; the Android entry uses JNI to reach that fixture. It does not exercise the production `NativeWalletRuntime` installation-to-experimental-owner transfer, nor claim that the fixed test issuer is a production installation authority. A separate native proof reader shares the immutable authenticated parameters to verify the C Payment projection without duplicating proving or parameter storage.

The manifest v4 experimental profile authenticates four pairs of verifier programs for Bootstrap, Send, Receive and Control. Control handles Load, signed RefreshTimeAnchor and Unload. The two-wallet fixture exercises Bootstrap, Load, Send and Receive with time controls disabled. A time-controlled Send separately requires a protected direct-exchange time observation. [Native Control contracts](../../NATIVE_BACKEND.md)

### Signed time-anchor recovery on the host

From the Iroha checkout, set `FUNDED_FIXTURE` to the completed three-launch software fixture wallet directory and `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS` to its signed program directory. The following run advances a private copy. Each command starts a new process. The last process checks the two re-encoded invalid inputs and recovers the exact successful completion without a second state change. [Recorded results](../../output/production-20261119/wallet-owned-public-refresh-result.json)

```sh
: "${FUNDED_FIXTURE:?set FUNDED_FIXTURE to the completed host fixture}"
: "${KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS:?set the signed fixture program directory}"
test -f "$FUNDED_FIXTURE/summary-launch-3.txt" || exit 1
KAGEMUSHA_SPARTAN_DEVICE_ROOT=$(mktemp -d) || exit 1
export KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS KAGEMUSHA_SPARTAN_DEVICE_ROOT
cp -R "$FUNDED_FIXTURE/." "$KAGEMUSHA_SPARTAN_DEVICE_ROOT/" || exit 1
for stage in refresh-prepare refresh-execute refresh-recover; do
  KAGEMUSHA_SPARTAN_HOST_LAUNCH="$stage" cargo iroha-fast -- test \
    --manifest-path experiments/kagemusha-spartan/device/runtime/Cargo.toml \
    --release --locked --offline --lib host_tests::two_wallet_device_flow_on_host \
    -- --exact --ignored --nocapture --test-threads=1 || exit 1
done
```

Apple E5 uses the real App Attest service and key-binding assertion, with effect dispatches and returned originals retained through the actual EnrollmentOwner. Android uses the actual nonexportable hardware key and its original KeyMint attestation chain. This fixture issuer **does not independently verify FI attestation policy before signing E6**. Therefore these runs test native proof/key custody and recovery, not production FI admission, pinned-root/revocation qualification, or physical cross-device payment. The separate host chain covers two wallet identities. No successful full device run is claimed until its actual output is preserved.

Android instrumentation entry: `KagemushaSpartanRuntimeDeviceTest`. Package the actual `libkagemusha_spartan_device_fixture.so` as an androidTest JNI library and the fixture originals under androidTest assets `spartan-artifacts/`. A five-kind fixture includes the independent World selection alongside the bundle, manifest and signature. Use the explicit disposable application namespace through the existing `kagemushaEnrollmentProbeApplicationId` test property. The dedicated nonexported Activity presents the retail SDK's amount, fee, asset and recipient review. The SDK signs the exact Native challenge with the admitted slot's Android Keystore P-256 confirmation key through AndroidX BiometricPrompt and its CryptoObject. Native verifies the signature, its platform origin and the exact review before the device debit can be selected and completed.

Build the Pixel instrumentation APK from the Iroha root. Set
`ANDROID_NDK_HOME` to the installed NDK 30.0.16248370, `FIXTURE_INPUTS`
to the directory containing the three signed originals, and `FIXTURE_WORK`
to a fresh owner-private absolute directory. Set `FIXTURE_APP_ID` to the
designated disposable enrollment-probe namespace. The retained Pixel package
uses `org.hyperledger.iroha.kagemusha.enrollmentprobe.spartan_e5_20261118`.

```sh
repo="$PWD"
fixture="$repo/experiments/kagemusha-spartan/device/runtime"
resources="$FIXTURE_WORK/android-resources"
mkdir -p "$resources/assets/spartan-artifacts" "$resources/jniLibs/arm64-v8a"
for original in program-bundle.norito parameter-manifest.norito parameter-signature.bin; do
  cp "$FIXTURE_INPUTS/$original" "$resources/assets/spartan-artifacts/$original" || exit 1
done
if test -f "$FIXTURE_INPUTS/world-manifest-digest.bin"; then
  cp "$FIXTURE_INPUTS/world-manifest-digest.bin" "$resources/assets/spartan-artifacts/" || exit 1
fi
cargo ndk -t arm64-v8a --platform 26 build --release --locked --offline \
  --manifest-path "$fixture/Cargo.toml" --target-dir "$fixture/target" || exit 1
cp "$fixture/target/aarch64-linux-android/release/libkagemusha_spartan_device_fixture.so" \
  "$resources/jniLibs/arm64-v8a/" || exit 1
(
  cd "$repo/kotlin" || exit 1
  ./gradlew --offline -I "$repo/experiments/kagemusha-spartan/device/android/fixture.init.gradle" \
    -PspartanFixtureResourceDirectory="$resources" \
    -PkagemushaEnrollmentProbeApplicationId="$FIXTURE_APP_ID" \
    :kagemusha-wallet-android:assembleDebugAndroidTest
) || exit 1
cp "$repo/kotlin/kagemusha-wallet-android/build/outputs/apk/androidTest/debug/kagemusha-wallet-android-debug-androidTest.apk" \
  "$FIXTURE_WORK/spartan-androidTest.apk"
```

On Android each signed manifest owns its own fixture program directory,
native custody and report under `protected-debit-v1`. Earlier diagnostic runs of the same signed manifest retain their previous directories. A fresh launch must exercise the protected Send before recovery can reuse its outputs.
The instrumentation status reports `spartanManifestSha256`, `spartanReportPath`
and `spartanCustodyPath`. On the designated fixture Pixel, capture each
of the three launches in a distinct `LAUNCH_OUTPUT` file. Starting a new
process preserves the selected fixture custody. For the first Send, review
the displayed asset, amount, fee and recipients and confirm on the device.

```sh
adb -s "$FIXTURE_SERIAL" shell am force-stop "$FIXTURE_APP_ID" || exit 1
adb -s "$FIXTURE_SERIAL" shell am instrument -w -r \
  -e class 'org.hyperledger.iroha.sdk.offline.wallet.KagemushaSpartanRuntimeDeviceTest#completeNativeLifecycleAndExactRestartRecovery' \
  "$FIXTURE_APP_ID/androidx.test.runner.AndroidJUnitRunner" > "$LAUNCH_OUTPUT" || exit 1
```

From the Iroha root, retrieve that launch's report with the same
diagnostic APK, application ID and device serial.

```sh
python3 experiments/kagemusha-spartan/device/android/read-fixture-result.py \
  --apk "$FIXTURE_APK" --instrumentation "$LAUNCH_OUTPUT" \
  --package "$FIXTURE_APP_ID" --serial "$FIXTURE_SERIAL"
```

Select `--resource measurements` or `--resource events` to retain the
Native measurement JSON or event stream after each launch. The reader checks
the reported manifest and paths against the APK and the retained report
against its `spartanReportSha256` from that launch. Measurement retrieval also checks the SHA-256 printed by that launch.


### ArchiveSent on the governed five-family fixture

Preserve the first three launch reports for Send, Receive and cold recovery.
Launch the same build against the same private installation a fourth time.
It verifies the genuine CreditStatus, refuses a changed status, and archives
the delivered Send through the public wallet API. A fifth cold launch retries
the same operation. Its completion bytes, balance and sequence must match the
fourth launch. The reports name the ArchiveSent phases `execute` and
`exact_retry`. Both operations use the holder’s existing hardware wallet.
