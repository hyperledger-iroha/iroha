# ZK SDK native qualification — 2026-09-30

This continuation separates the interrupted Apple candidate from the current
integrated source. It does not close ZK07 or qualify a signed release, a physical
device, replacement RAM-LFE encryption, or an independent cryptographic review.

## Apple packaging correction

The previous ABI-25 build completed all five native Apple archives and C consumer
links, with no source drift or dependency-audit errors, but failed packaging after
17,912.822 seconds. The C/Rust export gate still required the retired Parliament
`CASTING_PROOF_PAGE_RESULT_BYTES_V1` name; Rust and the header both define
`CASTING_PROOF_PAGE_SUMMARY_BYTES_V1 = 41`. The gate now checks that actual name
and width. Four explicit mutations reject a changed Rust/C width and either
retired name. A stale reserve-finality signature mutation now corrupts the
current checkpoint length instead of the removed trusted height parameter.

`bash ci/check_connect_norito_bridge_header.sh --self-test` passes the positive
C11/C++17 header checks and all 53 negative controls. The first self-test attempt
exposed the stale height mutation; no failed assertion was removed or bypassed.

The normal retry authenticates all 502 retained local compiler outputs before
Cargo reuse. It preserves the original failed build, reruns every normal target
build and artifact admission step, and lets the builder own the new manifest and
Swift hash pins. This frozen candidate is the preceding `ce9a01d` component
snapshot plus the recorded amendments; it is not the current integrated source.
Its amended 20,739-entry source manifest has SHA-256
`ccda6f20067780025e04aaaf05846479dba32c1d9417ea947f669d4a51790efc`.

## Superseded integrated candidate

A separate Apple source clone is pinned to
`89efeb5f734e6782f818e1bf98c21870c7888e9b`, the same commit captured by the current
Core/Kagami runner. Its only amendment is the export-gate correction above.
Rust, SDK and lockfile inputs match that capture. Its complete source manifest
has SHA-256
`aeeef8b8ed101dbabe869ea3b492efdf8cb419947e3a61d6e9fc0e0656d1578f`.
The user subsequently required all work to use only the primary checkout on
`optimizations`. No new work uses this clone. Historical predecessor compiler
success does not qualify the current checkout; no terminal Swift test result is
claimed for the interrupted predecessor runner.

## Primary-checkout continuation

The first primary five-slice attempt is retained in
`sdk-qualification/optimizations-apple-run3/`. It ran normal Cargo directly in
`/Users/takemiyamakoto/devstuff/iroha` on `optimizations`. It uses the standard
owner-only `target/norito-bridge-local/` layout, retains compiler messages and
checks the built-in source seal. Concurrent relevant fixes must pass a normal
rebuild before any package or consumer result qualifies. After 2,099 seconds,
the first host slice failed on two calls to the test-only issuer-admission
deadline accessor; no XCFramework was published. Its 58 retained local compiler
outputs pass provenance checks, while the full source seal also records the
concurrent source amendments. Neither a host slice nor a package is marked passing.

Both qualification calls now use the production `require_live` API, which checks
the original journal selection and continuous deadline. The handoff regression
also checks fresh qualification acceptance and expired rejection. The prepared
`run-optimizations-apple-retry.py` authenticates the primary predecessor outputs,
runs ordinary Cargo against the amended source and requires a new unchanged
source seal through all five slices and packaging. This retry waits for the
coordinated X509 test amendments before capturing its source.

`run-primary-swift-consumers.py` requires successful authenticated native output
before running 68 policy controls, two actual confidential-redemption tests and
the public executable example. Its separate device-preparation action discovers
the actual Xcode package scheme and compiles generic iOS tests before requesting
a physical connection. A device run must execute both native tests without skips.
The read-only preflight found one Apple development signing identity and no
connected physical device.

## Kotlin consumers

Current Kotlin no longer references the retired
`native_execution_evidence_{1,4}_lanes_v1.json` files or their retired consumers.
The previous handoff runner is therefore not a valid current-source inventory.
Current finality fixture ownership and `kotlin-fixture-gen` remain with the
integrated native producer.

The primary-checkout genuine fixture producer now passes all ten encoder modes.
Its retained executable has SHA-256
`58bc6337cc288aa921f62febe0afebebc622fd0a77fd05066cd28fc23c6eb2aa`,
and evidence is in `primary-kotlin-fixture-retry1/`. Its synthetic status/lane
encodings are wire fixtures, not finalized network execution evidence. Current
ABI-25 JNI and the dependent Kotlin consumers remain pending.

The first normal current `:core-jvm:test` selection compiles production Kotlin,
test Kotlin and Java consumers with the JDK-8 API guards intact. Of 15 selected
profile, parser and tag tests, five pass and ten fail because the ABI-25 native
address validator is unavailable. These parsers also validate public keys, so
they require the real native bridge even when testing JSON fields. There are no
skips, and every captured Kotlin/fixture input is unchanged. Retry requires the
current rebuilt bridge; no fallback validator or weakened assertion is added.

The subsequent normal primary host slice completes with ABI 25 and features
`privacy-production-enabled`. The immutable JNI capture has SHA-256
`5135d02bb3245e1815b8a7ae0a08d1ea05020abaf92e8c220d8611e3d19cf6c2`;
`current-host-jni/receipt.json` binds its compiler stream, 66 authenticated local
outputs, source seal and live ABI probe. The other Apple slices remain in progress.

Using that capture, the complete `:core-jvm:test` run passes all 1,522 tests with
zero failures, errors or skips. This includes all four native confidential-wallet
tests and current Kotlin/Java-source fixture consumers. Direct generator parity
covers account registration, claim identifiers, transfers and contract lifecycle.
`:core-jvm:confidentialRedemptionExample` produces and locally verifies a
13,741-byte, one-input full redemption. `:tools:test :tools:installDist` passes all
25 tests. The commands retain JDK-8 API compilation guards and use JDK 21;
source, generator and native-library guards remain unchanged. Exact commands,
JUnit and terminal results are in `current-kotlin-consumers/`.

The first Android run passes 178 client and 32 wallet managed tests, and 70 of 71
macOS host-JNI tests. The failed enrollment nonce-substitution test expected
`IllegalStateException` where the frame validator raises `IllegalArgumentException`
after native dispatch. The test now checks the exact rejection, one native close,
closed-handle recovery and retry rejection without redispatch, and idempotent
explicit owner closure. Production code is unchanged. The complete rerun passes
all 281 tests with zero failures, errors or skips in 23.381 seconds. Source and
native guards match; the active Apple source snapshot also remains byte-identical.
`current-android-managed-host/` retains the failure;
`current-android-managed-host-retry1/` retains the passing rerun.

## C# host consumers

The current C# SDK and test assembly build in Release with zero warnings or errors.
All 5,865 unit tests pass with no skips, including the four required native wallet
controls against the same ABI-25 host capture. The public example then proves and
locally verifies both a 14,215-byte redemption with change and a 13,741-byte full
redemption. Source and native-library guards match throughout. The pinned .NET
8.0.419 SDK was already available inside this checkout; all 4,704 distributed
files were rechecked against its retained official SHA-512-authenticated archive.
`current-csharp-consumers/` retains exact commands, build output, xUnit XML, native
bindings and the example. This does not qualify a signed multi-platform NuGet
package or any physical device.

## Evidence and remaining work

Raw commands, compiler streams, JUnit output, source manifests and receipts are
retained under `dist/zk-remediation/2026-09-30/sdk-qualification/`. The original
Apple failure remains under
`dist/zk-remediation/2026-09-29/swift-current-abi25/run1/`.

The current integrated five-slice build, Swift consumers, current installed Python
and JavaScript consumers, direct device-test preparation and actual connected-device
execution remain pending. Device discovery reports no connected iPhone.
Cross-target archive creation and C linkage do not establish device execution.

## Historical continuation handles

- `run-swift-retry.py` owns the predecessor packaging and 68 host controls;
  terminal receipt: `swift-run2/complete-receipt.json`.
- `queue-current-apple.py` waits for a passing predecessor packaging receipt,
  then invokes `run-current-apple.py` using the frozen current source.
- `current-apple-run1/complete-receipt.json` will record native packaging and
  policy controls. `wallet-workflow-receipt.json` separately records the two
  real native wallet tests and runnable example.
- The current driver also captures `xcodebuild -list -json` to establish a
  direct package scheme for physical-device follow-up. No device test is
  scheduled automatically, and no signing or device evidence is fabricated.

All runner names above are historical references relative to the retained SDK
qualification directory. Their clone is `current-apple-source/`; it must not be
used for follow-on work. The primary-checkout continuation above owns all new
implementation and validation.
