# KAGEMUSHA V1 production readiness

Source/security assessment started 2026-09-05. KAGEMUSHA is **not production
qualified**. This record tracks implementation and validation work; it is not an
independent cryptographic audit, hardware certification, or authorization to
enable an offline monetary profile.

## Release goals

1. Close monetary proof authority: constrain original recipient credentials,
   plaintext openings, finalized reserve credits, normalized hardware guards,
   replay insertion, and successor state inside the actual recursive relation.
   Generate and verify real proofs using the authenticated final artifacts.
2. Finish the durable product coordinator: connect private state/proof snapshots,
   authenticated history, sealed preparation, hardware commit, inbox, outbox,
   finality, and recovery. Every caller retry must recover the original operation
   and every exposed byte; absence of qualified hardware must remain unavailable.
3. Complete SDK interoperability: caller-persisted request/payment/mint/redemption
   identities; exact canonical reservation bytes; identical response binding;
   current-source native artifacts; Swift, Kotlin, Java consumers and remaining
   SDK conformance. A syntax check or mocked provider does not qualify native use.
4. Close security and performance evidence: 1,024 real recursive handoffs,
   1,000 independently funded balances through aggregate spend and redemption,
   four-validator settlement/restart/replay, complete adversarial crash matrix,
   fuzzing, workspace tests/Clippy, independent review, and measured device limits.
5. Qualify and enable each exact device profile only after all preceding goals
   pass for the immutable release candidate. Do not infer support from brand,
   operating system, successful signing, or application installation.

Proof ceilings remain 6,528 paired-proof bytes and 9,211/12,288 complete raw/text
exchange bytes. Device gates retain 128 MiB process RSS, 10 s proving p95,
1 s verification p95, and 30 s handoff p95. These are required acceptance limits,
not current achieved measurements or a claim of optimality.

## Active implementation — 2026-09-22

Work remains confined to `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. The following combines source inspection with the scoped
component validation below; it is not a security audit or hardware qualification.

The September 25 native installer now derives its complete release and finality
configuration from a threshold-authenticated mobile bootstrap token. The bounded
canonical package pins network, asset/reserve, release, first context and freshness;
its independent native policy cannot be selected by a downloaded response. The
installer checks a suspend-inclusive token deadline before journal access and
publication. A signed checkpoint still requires native trusted time and retained
replay state, approved deployment signatures and actual app-startup integration.
The scoped bridge suite passes 355 tests with one existing ignored test. The new
fixed five-slice arithmetic and shared dense MSM suite pass 40 tests with two
benchmark-only cases ignored, including both-parity full-capacity, empty-slice,
padding and source-binding controls. The dense witness now derives a complete
offset bound from its active additions; the previously rejecting 256-offset
case emits a valid full trace in both fields. Each arithmetic slice configures
55 advice, three fixed and 21 permutation columns at k15. This excludes global
source authentication and recursive joins, so it is not a phone RSS pass.

The stored IPA implementation now continues from consuming assignment through
scalar evaluation, guarded outer multiopening and guarded inner IPA to final c/f
writes. `finish_guarded_ipa` returns a closed internal completed-proof owner while
retaining the original key, parameters, RNG, transcript and stored-source receipts.
The inner stage reuses P as b, retains guarded S→P' and uses a guarded MSM join;
its minimum initialized field arrays contain 2n + floor(n/2) + 2 elements and its
owned affine arrays contain n + floor(n/2) + 2 elements. Admission charges actual
capacities and explicit headers. Backend representations and worker scratch are
excluded; these bounds do not establish whole-process erasure, RSS or latency.
The final 27-function selection passes in both default and no-multicore builds,
including seven inner tests and six satisfiable generic whole-PLONK cases through
the guarded inner implementation. The verifier rejects zero round challenges;
empty/singleton FFT identities and the k0 IPA basis empty product now pass their
regressions. These are scoped component results, not monetary release evidence.

The crate-private [indexed reader](../vendor/halo2-axiom/src/plonk/structured_key/indexed/reads.rs)
now fills bounded intervals directly from mask coefficients, constant/bitset/raw
fixed columns and permutation target IDs. It uses constant-size decoding scratch
without allocating a full column, and clears the entire output on invalid input,
I/O failure or unwind. Original-source authentication and failure poisoning remain
the consuming owner's responsibility. The additional coefficient reader reuses a
caller-owned guarded column and the original normalized transform. A private
adapter seals that column into an exact fixed/permutation/mask storage role;
role identity and a sealed snapshot alone supply no key-source authority. Stored
proving and normal Core generation still retain dense keys pending consuming
indexed-owner integration.

The authenticated Core indexed-key owner and encrypted polynomial store are
present but remain separate from normal production proof entry points: the six
direct `create_proof` calls and two `create_proof_consuming` calls in
[generation.rs](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs)
use dense `ProvingKey` owners. The stored continuation and Core
`capture_indexed_proving_key` have only test callers in the inspected source.
Closed internal proof completion does not supply authenticated Core producer/key
integration. Sources: [stored owner](../vendor/halo2-axiom/src/plonk/prover/stored.rs),
[guarded outer continuation](../vendor/halo2-axiom/src/plonk/prover/stored/proof_evaluations/opening.rs),
[guarded inner IPA](../vendor/halo2-axiom/src/plonk/prover/stored/proof_evaluations/opening/inner_ipa.rs),
[indexed artifact owner](../crates/iroha_core/src/zk/kagemusha_v1_recursion/artifacts/stored_key.rs).

Normal native activation remains unavailable. The only in-repository
`KagemushaCoreCoordinatorBackendV1` implementation and install calls are in its
unit tests. The exported C open/invoke functions return `-312` when that backend
is absent. Enrollment, enrolled-session, registry, deadline and startup ownership
kernels remain behind `cfg(test)`. The stock device dispatcher has 22 lifecycle
operations; each can return only malformed or unavailable, never a monetary
success. A qualified backend must connect these pieces before release. Sources:
[coordinator contract](../crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs),
[C activation boundary](../crates/connect_norito_bridge/src/lib.rs),
[stock device dispatch](../crates/connect_norito_bridge/src/kagemusha_device_bridge_v1.rs).

Swift's native coordinator adapter delegates to that C boundary. Android's
registered hardware-provider factory requires exactly one external native Core
coordinator factory; no production implementation/registration of that second
factory is present in the Kotlin sources. SDK framing, APDU transport and provider
interfaces do not supply the missing qualified backend. Current-source native
artifacts and cross-SDK executions remain required; `dist/NoritoBridge.xcframework`
is absent in this checkout at this assessment. Sources:
[Swift bridge](../IrohaSwift/Sources/IrohaSwift/KagemushaCoreCoordinatorBridgeV1.swift),
[Android factory](../kotlin/kagemusha-wallet-android/src/main/java/org/hyperledger/iroha/sdk/offline/wallet/KagemushaAndroidAuthenticatedHardwareProviderFactoryV1.kt).

Durable history, coordinator WAL, response-evidence retention and recovery
checkpoint code exist. The authenticated Guard verifier can delegate checkpoint
CAS and current-checkpoint checks to a release-bound hardware transaction
verifier. That verifier authenticates an independently generated challenge and
exact signed checkpoint/journal-prefix values. Production native ownership still
has to connect it to the held journal descriptors, original response evidence,
latest checkpoint selection and byte-identical recovery. `restore_from_disk_history`
explicitly remains unwired to the product coordinator's hardware session. A
signed historical projection or a replayable host WAL alone must not authorize
restoration. Sources:
[Guard delegation](../crates/iroha_core/src/zk/kagemusha_v1_recursion/guard_verifier.rs),
[transaction verification](../crates/iroha_core/src/zk/kagemusha_v1_recursion/hardware_transactions.rs),
[durable restoration](../crates/iroha_core/src/zk/kagemusha_v1_state/mod.rs),
[response archive](../crates/iroha_core/src/zk/kagemusha_v1_state/response_evidence_archive.rs).

The real 1,024-handoff qualification test remains ignored and explicitly fails
because the positive-value MintFold → SendSplit → Payment → ReceiveFold generator
is not wired. The payment-corridor source still marks terminal/payment integration
as incomplete. These fixtures do not establish 1,000 funded merchant balances,
a live four-validator settlement/recovery run, genuine final-artifact proof
acceptance, or device resource ceilings. The mint-authority reader also retains
a bootstrap structure fixed-point qualification TODO. Sources:
[handoff gate](../crates/iroha_core/src/zk/kagemusha_v1_recursion/real_handoff_qualification_tests.rs),
[payment corridor](../crates/iroha_core/src/zk/kagemusha_v1_recursion/real_payment_corridor.rs),
[native verifier](../crates/iroha_core/src/zk/kagemusha_v1_recursion/native_backend.rs).

The next completion sequence is: qualify and connect the full stored proof path;
qualify real funded recursive payments and complete durable native ownership;
rebuild and exercise SDK/native artifacts; then close release, physical-device
and independent-review gates. Exact OEM services/profiles for the requested
brands remain external dependencies. Release authentication and fail-closed
activation must stay enforced throughout this work.

## Current validation boundary — 2026-09-23

A September 23 focused `connect_norito_bridge` library check reached
`iroha_core` and exposed two typed SHA-bit provenance errors in the staged
terminal-recovery relation. Both call sites now use the typed bit decomposition;
the check and associated new regression tests still need to rerun after the
concurrent signed Taira release build clears the machine. This is source repair,
not a passing current-root build.

The fresh Kotlin/JVM KAGEMUSHA selection executes **57 tests: 33 pass, 24 fail,
zero skipped**, with all 699 captured SDK/build/fixture inputs unchanged. Every
failure reports unavailable ABI-23 native account-address validation; these are
not evidence of 24 distinct product assertion defects. The
[Kotlin validation note](../docs/history/2026-09-22/kagemusha-readiness/kotlin-core-validation.md)
records the actual JDK 21 command, all JUnit results and exact hashes. A
current-source native bridge rebuild and rerun are required; this snapshot does
not bind the Rust native dependency graph or qualify JNI/device execution.

The current indexed-key batch passes the same **79 functions in default and
no-multicore builds (158 executions)**, with zero failures or ignored tests. All
267 captured inputs match across both test windows and the passing 3.569 s
non-test library check. Coverage includes the installed n−1→n permutation-coset
regression, coefficient conversion, guarded snapshot errors/unwinds, exact key
roles, advice-role rejection and the prior guarded-inner proof selection. The
[indexed-key batch note](../docs/history/2026-09-22/kagemusha-readiness/indexed-key-batch-validation.md)
records exact scopes and hashes. The two new actual Core store-role tests are
installed but await the separately coordinated root-graph checks. This does not
qualify consuming indexed-owner integration, Core monetary proofs or hardware.

The earlier 18-function indexed-reader selection remains a separate captured
candidate in the [reader note](../docs/history/2026-09-22/kagemusha-readiness/indexed-reader-validation.md).

The preceding guarded-inner candidate passes the same **27 distinct test functions in
default and no-multicore builds: 54 executions, zero failures or ignored tests**.
All 260 captured source/build inputs match across both windows and the separate
non-test vendor-library check, which passes in 1.124 s; each test executable remains
unchanged during its window. The selection includes guarded arithmetic, owner
failure/erasure controls, ordinary IPA/multiopening regressions, zero-challenge
rejection, empty/singleton FFT identities and the k0 IPA basis empty product.

The satisfiable square/lookup/copy fixture reaches real whole-PLONK acceptance
through guarded inner IPA, with dense proof-byte, transcript and next-RNG agreement
in both Pasta fields and three instance modes (six generic cases). This does not
qualify final KAGEMUSHA recursive monetary proofs. These checks use the standalone
vendor manifest/lockfile, not the root-workspace/Core dependency graph.

The [inner validation note](../docs/history/2026-09-22/kagemusha-readiness/inner-validation.md)
retains every earlier failure: the first window executed 11 functions (10 passed,
one failed), the accidental unchanged-candidate second window executed 24 (23
passed, one failed), and the third executed 26 (25 passed, one failed). The third
window installed the FFT fix but exposed a separate k0 empty-product assertion in
`compute_s`; the final candidate fixes that assertion and adds its regression.
The earlier selector/setup failures remain recorded; they are not overall passes.

The earlier guarded-P candidate records 43 distinct passing functions across two
default windows and 42 passing no-multicore functions, retaining the original
failed assertion and interrupted first no-multicore attempt. Its positive fixture
uses a test-only ordinary inner IPA; the new guarded-inner positive fixture above
is separate. Its deliberately unsatisfied inverse fixture's byte equality and
opening verification are not positive whole-PLONK acceptance. The
[opening note](../docs/history/2026-09-22/kagemusha-readiness/opening-validation.md)
preserves that candidate's exact scope. Earlier default and no-multicore builds
each passed the same 27 scalar/blind functions (54 executions, 27 distinct); the
[scalar/blind note](../docs/history/2026-09-22/kagemusha-readiness/scalar-blind-validation.md)
retains that separate candidate. These earlier receipts do not qualify later
inner-IPA source changes or final recursive monetary proofs.

The replaced active/current sections and root status row are preserved
byte-for-byte in the
[dated archive](../docs/history/2026-09-22/kagemusha-readiness/README.md).
The previously cited `target/kagemusha-validation/stored-prover-next-window-20260912`,
`target/kagemusha-validation/20260912-source-window`,
`target/kagemusha-main-native-jvm-validation-r5` and
`target/kagemusha-sdk-security-parity-validation-r1` directories are absent.
Their recorded historical test totals cannot be independently rechecked here or
used to qualify today's source. Finding entries and test counts below preserve
earlier scoped work records; they are not fresh validation of this source.

Fresh qualification must retain the exact candidate, dependency/lock inputs,
compiled artifacts, commands and results. Focused vendor or host tests cannot
replace authenticated Core/SDK integration, final monetary proofs, workspace
checks, canonical release provenance, measured full-process resources, qualified
hardware, or independent review.

## Requested device scope

The priority families are iPhone, Samsung, Huawei, Google, and Meizu. Other major
brands follow the same exact-profile qualification path. No model/OS/firmware/
provider tuple has been qualified by this assessment.

A physical iPhone 17 Pro Max running iOS 26.7 completed a nonmonetary App Attest
probe on 2026-09-23. Its development attestation had 164-byte authenticator
data, flag `0x40`, counter zero, and no extension suffix. Two same-key assertions
had 37-byte authenticator data, flag `0x40`, no suffix, and counters one and two;
discarding the first assertion did not prevent the hardware counter from
advancing. The saved attestation passed Swift verification of Apple's pinned
root, certificate chain, nonce, key ID, App ID, AAGUID and public key. Both
assertions passed the documented P-256-SHA256 signature check against that
attested key: Apple forms `nonce = SHA256(authenticatorData || clientDataHash)`
and ECDSA-SHA256 signs that nonce. The exact same-run raw objects pass a
23-test isolated Swift validation harness, including attestation and both
assertion signatures.
The device supplied no signed validation-category or bundle-version measurement,
so this iOS 26.7 result cannot establish per-release identity. All raw objects
remain in an untracked, app-private diagnostic record; no monetary proof was
admitted. The connected Pixel 6 is now authorized for `adb`; its physical
Keystore probe reports neither `android.hardware.keystore.single_use_key` nor
`limited_use_key`, and its attestation has no hardware rollback-resistance tag.
The mandatory Pixel 6 profile was additionally tested with forced StrongBox
P-256 `setMaxUsageCount(1)` on Android 16. Its attestation identified StrongBox
for the key and KeyMint but put use-limit tag 405 in `softwareEnforced`, with
neither tag 405 nor rollback-resistance tag 303 in `hardwareEnforced`. The
first signature verified and the second failed with
`KeyPermanentlyInvalidatedException`; that is software-limited use, not
hardware no-fork evidence. Its internal eSE is connected, but no access rule
for the current applet was observed. An app-accessible, provisioned hardware
counter/checkpoint profile or another physically proven no-fork primitive is
required to support this device for production offline money.
The connected Pixel 6 was rechecked on an Android 17 user build
(`google/oriole/oriole:17/CP2A.260705.006/15641320`) with locked, green
verified boot. It still advertises neither hardware single-use nor limited-use
Keystore support, and does not advertise the hardware Identity Credential
feature. On 2026-09-24, Android `PackageManager.hasSystemFeature` on that
API-37 device returned `FEATURE_KEYSTORE_SINGLE_USE_KEY=false` and
`FEATURE_KEYSTORE_LIMITED_USE_KEY=false`; the hardware one-use qualification
test rejected with `hardware single-use feature absent`. A fresh nonmonetary
StrongBox one-use instrumentation
test passed on that build: the new key's attestation reported StrongBox security
level 2 for both attestation and KeyMint, hardware tag 303 absent, hardware tag
405 absent, software tag 303 absent, and software tag 405 equal to one. Its first signature verified and
its second signing attempt failed with `KeyPermanentlyInvalidatedException`.
On 2026-09-25, the three-stage physical restart probe also passed after a
fresh device boot and PIN unlock: the consumed alias could not sign again.
This demonstrates software-enforced one-use only; it does not qualify a
hardware no-fork monetary ratchet. `eSE1` is connected, but the 2026-09-24
ordinary-app OMAPI discovery returned `ONLINE_ONLY` for the current applet AID;
no app access rule was observed and applet SELECT/recovery remains untested.
Android's [setMaxUsageCount contract](https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder#setMaxUsageCount(int))
explicitly allows software enforcement when secure hardware lacks the feature;
the [PackageManager feature definitions](https://developer.android.com/reference/android/content/pm/PackageManager#FEATURE_KEYSTORE_SINGLE_USE_KEY)
identify hardware support. A successful first signature and rejected second
signature therefore do not override the missing feature flags and hardware
attestation tags.
The same Pixel 6 advertises neither `android.hardware.identity_credential`
nor `android.hardware.identity_credential_direct_access` in `pm list
features`, so the [hardware Identity Credential feature](https://developer.android.com/reference/android/content/pm/PackageManager#FEATURE_IDENTITY_CREDENTIAL_HARDWARE)
cannot supply this device's missing ratchet. That API's authentication-key
use count is an app-readable replacement indicator; its presentation request
[allows exhausted-key reuse by default](https://developer.android.com/reference/android/security/identity/CredentialDataRequest.Builder#setAllowUsingExhaustedKeys(boolean))
and can even skip incrementing the count. The
[device-signed presentation](https://developer.android.com/reference/android/security/identity/CredentialDataResult)
authenticates session and credential data, not a monotonic count. Android's
ordinary passkey API likewise cannot presently qualify this profile: the
[FIDO2 selection criteria](https://developers.google.com/android/reference/com/google/android/gms/fido/fido2/api/common/AuthenticatorSelectionCriteria.Builder)
do not require a hardware-backed non-backup credential or nonzero counter,
and [WebAuthn permits a constant-zero signature counter](https://www.w3.org/TR/webauthn-3/#sctn-sign-counter).
An experimental Pixel 6 StrongBox observation collector and testnet-only app
entry point retain exact selection/network/release binding and local lost-result
freezing. On 2026-09-24, both Android JNI slices were built and source-sealed
at commit `95acc0ac374fc00f2a7fdbb8e69289cd2ae89b7d`, Android source
final Android source fingerprint
`67f7c4e912d0de7d97980fbc1584e14ec9c9586a66ae6a65d740e5349cbc91dd`.
The APK was installed on the locked/verified-boot Pixel 6; the physical
StrongBox observation and native retained-proof export each passed one
instrumentation test. Its leaf-first public attestation chain is retained in
an owner-only artifact outside this repository. Independent parsing found
StrongBox levels 2/2 and software-enforced tag 405 equal to one, while hardware
tags 303 and 405 remain absent. Three additional nonmonetary instrumentation
stages passed: an unused key persisted into a new app process, that process
made one verified signature, and after reboot the consumed alias was absent
and a second signing attempt failed. This is observed Android service behavior,
not a hardware-enforced no-fork guarantee. The collector now checks the exact
460-byte Core S before touching its intent store or one-use key; its 16 focused
Android JVM tests and the Core/JVM KeyMint verifier tests pass. The final
post-edit Android seal verified again, and both physical instrumented checks
passed after reinstalling that APK (SHA-256
`2ba9f08755ad99731d465e261817d398ff90c28b48f1d855ba12637208bb1f3e`).
Observation output is explicitly non-qualified and is not a production
monetary certificate.
The captured self-signed root is DER-identical to Google's first
[previously issued Android attestation root](https://developer.android.com/privacy-and-security/security-key-attestation#root_certificates).
Google advises retaining that factory chain's trust despite certificate expiry
only with a successful revocation check. The current application verifier pins
the two newer published roots and has not admitted this older chain; adding an
unverified root or skipping the missing hardware tags would be unsound.

| Family | Integration work and evidence required |
| --- | --- |
| iPhone | Bind an app-release policy without claiming unavailable iOS 26 per-assertion version measurement, then complete the exact signed-counter/one-use proof fold, lost-assertion recovery and physical qualification for each supported OS/profile. HCE, NFC and QR are byte transports, not monetary authority. |
| Samsung | Kotlin/Android KeyMint attestation and one-use-key ratchet integration; physically verify hardware enforcement of rollback resistance and single use for each admitted model/firmware. |
| Huawei | Establish the exact Android or HarmonyOS app/runtime and available attested key service, then run the same non-forking proof and physical qualification. Android coverage does not establish HarmonyOS coverage. |
| Google | Pixel 6 is mandatory and cannot use the tested KeyMint one-use profile. Obtain and qualify an app-accessible internal hardware counter/checkpoint service or a distinct no-fork primitive on that exact device, then bind it into the recursive monetary proof. |
| Meizu | Establish model/OS/runtime and attested one-use key availability, then run the same full qualification. |

The ordinary-app profile does not require Apple's Secure Element Credential
entitlement or an external card. [App Attest](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server)
provides an attested app key and signed assertions; physical enforcement of
strict-next counter behavior and sound consumed-but-lost recovery are separate
release obligations. HCE entitlement affects only a transfer transport.
Android's KeyMint attestation can describe a hardware-enforced one-use key and
rollback resistance, but those tags must be present in each admitted device's
actual chain; app-hosted counters and rollback-resistant key deletion alone do
not establish the monetary ratchet. See the [Android attestation
contract](https://source.android.com/docs/security/features/keystore/attestation)
and [phone algorithm](kagemusha_v1_phone_algorithm.md).

The Android SDK now has typed method-12 enrollment framing over the existing
native coordinator. A rebuilt ABI-23 host bridge runs nine focused Android
host-native tests, including original-ticket phase-6 cancellation retry after a
lost response or a locally poisoned proof response. The adapter retains one
phase-1 selection in process, can read its byte-identical response after a lost
return through the same live owner, and rejects issuer completion after a changed proof
result. It does not create the missing qualified native backend or admit Android
money.
The iOS app's native coordinator also dispatches method 12 through the exact
schema-2 frame validator. It retains the original ticket, phase order, signed
preparation selection and exact response/proof retries before exposure.
Nine focused tests passed on the connected iPhone 17 Pro Max, including exact
phase-2 response correlation, phase-3 challenge identity, and lost phase-6
cancellation reply retry even after local response poisoning. This uses the
development-signed diagnostic bridge;
the app still lacks an installed qualified enrollment components factory and a
source-sealed monetary XCFramework.

## Security findings and implementation work

- **KGM-22 — Corrected; focused retry-archive validation passes.** The Kotlin
  intent and historical qualification decoders previously entered generic
  decompression before rejecting noncanonical compressed archives. Both now
  bound and snapshot input, inspect the header without decompression, require
  uncompressed zero-layout archives, and retain schema/checksum/complete-decode
  and exact re-encoding checks against that same snapshot. All nine existing
  intent tests and four added regression tests pass. Small malformed-header
  cases verify early rejection; enclosing decode-state and defensive-copy
  behavior are preserved. Historical qualification decoding still confers no
  hardware authentication. Source:
  [retry archive codec](../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaOperationIntentV1.kt).
- **KGM-23 — Fixed; 54 Core recovery tests passed.** Outgoing-index
  reconciliation alone did not pair a separately supplied coordinator WAL with
  the checkpoint-selected journal when only Reserve/BeginIntent records existed.
  Core now requires the selected frame count, hash and byte boundary to occur in
  the held, fully replayed WAL before serving it. Ownership/generation checks
  remain mandatory even for a cached match; valid appended suffixes remain
  recoverable. Creation rejects before touching a new path once selected history
  has advanced beyond the initializer. Three new regressions and 51 retained
  tests passed on the fresh shared Core harness. This source finding does not establish an
  exported-ABI or monetary exploit, and the fix does not supply hardware
  freshness or qualified speculative-suffix authentication. Sources:
  [coordinator pairing](../crates/iroha_core/src/zk/kagemusha_v1_state/coordinator_operation_store.rs)
  and [owned journal ancestry](../crates/iroha_core/src/zk/kagemusha_v1_state/private_journal.rs).
- **KGM-01 — High, monetary relation incomplete.** The original MintFold private
  recipient credential and credit opening were retained by Core but not fully
  constrained to the receiving lane and verified authorization in the composite
  relation. Host validation cannot replace these circuit constraints. The
  correction now constrains the recipient and opening bytes and routes State SHA
  messages through the mandatory authenticated ordered claim fold. Focused Rust
  checks and actual artifact/resource gates are still required before closure.
  Sources: [recipient/opening constraint](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L2612)
  and [claim consumer](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L1705).
- **KGM-02 — High, operation recovery integration incomplete.** Swift exposed
  operations still allocated retry identities internally while Core had moved to
  caller-owned IDs; some SDK provider calls still used the retired allocator
  signature and untagged sender reservation bytes. A lost native return must not
  allocate a second monetary operation on retry. The caller-ID and canonical
  reservation changes now have focused Kotlin/Java/C# coverage, and 22 Swift
  coordinator tests pass against the pinned native host library. C# also now rejects missing-state re-bootstrap, journal rollback
  and recovery equivocation. Current-source native execution is still required.
  Sources: [Core reservation](../crates/iroha_core/src/zk/kagemusha_v1_state/coordinator_operation_store.rs#L274)
  and [C# ID admission/recovery](../csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/KagemushaWalletV1.cs#L1014).
- **KGM-03 — Medium, bridge response substitution.** Outbox release admitted a
  structurally valid response for a different canonical installed envelope.
  Match the exact request envelope before returning backend results and preserve
  cleared C outputs on rejection. The fix and regressions are implemented; Rust
  passed in the initial 20-test native coordinator suite. The expanded typed
  archive/input/receipt boundary subsequently passed the pinned 32-test native
  coordinator suite, with all recorded sources unchanged.
  Source: [native response validation](../crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs#L552).
- **KGM-04 — Medium, physical clock rollback evidence missing.** The physical
  transcript could pass without exercising a host clock rollback and rejection
  of an expired request. The verifier now requires four explicit boundaries and
  the release report requires `clock_rollback`, including for signed reports.
  Focused mutation tests pass. Source:
  [clock boundary verification](../scripts/verify_kagemusha_v1_physical_device.py#L774).
- **KGM-05 — Release blocker, physical provenance closure.** The release manifest
  now requires the full raw transcript, OEM attestation, governed trust roots,
  independently pinned observer policy and native OEM verification report. It
  authenticates and reruns the fixed physical checker, binds the exact candidate
  and OEM challenge, and rejects changed sources before publication. Focused
  substitution tests and the isolated projector test pass; an earlier stable-source
  five-file run after mandatory governed provider-issuer authorization
  passed 216 tests and 50 subtests. The latest native-fixture closure passes 240
  tests and 138 subtests with all 19 recorded inputs unchanged.
  Actual admitted OEM verifiers, roots and
  physical runs remain required for every enabled profile. See the
  [exact closure contract](kagemusha_v1_physical_evidence.md) and
  [release verifier](../scripts/verify_kagemusha_v1_release_evidence.py#L2109).
- **KGM-06 — Medium, JavaScript model mutation.** A public internal-value getter
  exposed mutable WeakMap backing data behind frozen canonical models. The
  getter is removed and only internal encoders access backing values. Public
  projections are defensive, with nested mutation/canonical byte regressions
  passing. Source: [model backing boundary](../javascript/iroha_js/src/kagemusha.js#L195).
- **KGM-07 — High, sender-context and response-evidence integration.** Core and
  the bridge used the same sender-context schema name for different field sets,
  so full public-input digests disagreed; the coordinator also discarded the
  device response's original signature. Core now owns the shared context,
  including its authenticated Core key reference, and method 3 carries the
  original low-S signature as its fifth field. The new ten-field contract rejects
  the retired projection. Rust canonical fixtures pass Kotlin/Java parity and
  full provider tests retain the original authenticator. Native session admission
  and fresh Rust Core regression execution remain required before closure.
- **KGM-08 — Medium, canonical SDK and retained-operation boundaries.** Existing
  mobile context/receipt codecs differed from Rust's declared alias-field layout.
  Actual Rust archive fixtures exposed and corrected the mismatch. The native
  boundary now rejects opaque/wrong-schema archives, changed nested preparation
  or recovery IDs, mismatched input digests, and invalid complete signed release
  commands. Android terminal-envelope bounds and historical policy/key rotation
  cleanup passed focused Kotlin/Java tests. Native recovery now requires the exact
  installed bytes and terminal identity selected by its canonical device reply;
  those checks passed the pinned native coordinator suite, and six sender tests
  passed context/preimage parity, signed zero-amount rejection and receipt retry
  checks. Public archive validity
  never grants a durable operation, admitted signing key or finality capability.
- **KGM-09 — Release blocker, incompatible profile identities.** Native loader
  admission compared its digest of the exact circuit layout to a release digest
  of a different evidence report. An authenticated release therefore could not
  satisfy normal native loading. The signed validation receipt now carries a
  separate native-layout digest, independently derived by the report verifier
  and checked before native artifact reads. Current Python release/provenance
  tests pass. The pinned focused Rust build now passes 31 release tests, including
  the governed provider registry, canonical issuer signing bytes, signed approval
  chain and native-layout substitution. Actual generated-artifact loading and
  the final combined release still need execution. See
  [native profile binding](kagemusha_v1_native_profile_binding.md).
- **KGM-10 — High, incomplete Guard credential history.** The Guard consumer
  expected a two-cell credential projection after the credential circuit had
  moved to a complete 40-cell statement with SHA history. The consumer now binds
  all columns, folds both current credential proofs and their carried histories,
  and shares the exact audits across both proof parities. A native verifier now
  verifies complete monetary Guard proofs against immutable release-authenticated
  protocols and Core's caller-derived statement. The loader also rejects an
  empty-effect sentinel belonging to another release. Eleven Guard archive tests
  and the native Guard protocol-binding regression pass in the frozen lane;
  genuine proof acceptance remains pending. These monetary proofs cannot certify independent hardware journal
  transactions.
- **KGM-11 — Critical, unanchored credential policy authority.** The credential
  relation proves membership under its supplied hardware-policy Merkle root,
  but native release admission authenticates a different digest of the enabled
  profile list. The new Guard verifier must not treat proof validity under a
  caller-selected root as provider authorization. Native monetary Guard, State,
  mint authorization and terminal entry points now fail closed until the exact policy root is
  bound into the actual proof relation and authenticated governance evidence.
  Merely checking local Core inputs would not protect direct terminal admission.
  An enabled profile name, a nonzero policy digest, or
  self-consistent credential proofs do not supply that authority.
  The receipt now authenticates a bounded exact provider inventory and derives
  its Merkle root independently. Both Rust and Python require the exact hardware
  profile issuer's low-S P-256 signature over each public provider commitment and
  registry position. Python verifies the same root against the raw physical
  transcript and OEM report, and OpenSSL interoperability plus refreshed-approval
  substitution regressions pass. The fixed-root circuit and mint-authorization
  integration pass three provider-root and five mint generation/column/transport
  tests in the frozen dependency lane. Genuine-proof execution remains required.
  See [provider policy binding](kagemusha_v1_provider_policy_binding.md).
- **KGM-12 — High, detached sender credential validity.** Terminal authorization
  reconstructed private credential statements but compared only the normalized
  Guard digest, which omits credential issuance. Sender expiry therefore lacked
  an authenticated proof opening. Guard now exposes both complete credential
  digests at public cells `6..10`, with its history at `10..44`.
  Terminal and aggregate consumers bind those exact verified cells; terminal
  constraints hash the canonical compact credential and governed profile and
  require the original trusted commit, or entire half-open lease, inside both
  validity intervals. The issuance field is the exact compact credential ID.
  Native decoders reject the old Guard shape, and all descendant keys require
  regeneration. All six Eq/Ep canonical opening, substitution and complete-window
  regressions pass in the frozen dependency lane; genuine terminal proof
  generation remains unverified.
  New top-ups also enforce validity at the certified reserve commit time;
  exact retries authenticate the stored receipt at its original time and do not
  mint again after expiry. The 22 focused DataModel instruction tests pass,
  including both new boundary tests. All three new Core lifetime/retry regressions
  pass. All four corrected finality retry/cache/anchor/conflict fixture tests now
  pass in the second frozen Core run; actual qualified-device timing remains open.
- **KGM-13 — High, detached recursive assignment context.** The aggregate
  allocated Guard history cells through the builder after its virtual context
  had moved into the recursive loader. Finalization replaces that detached pool.
  Guard history and credential limbs now allocate in the loader's active context.
  A regression exercises the actual context transfer and rejects credential and
  history substitutions in both fields. That regression passes in the frozen
  dependency lane; real aggregate proofs remain required before closure.
- **KGM-14 — Release blocker, sender lifetime admission before hardware commit.**
  The new terminal validity checks also require the provider to reject an expired
  sender credential/profile or straddling lease before irreversibly advancing
  hardware. The current public op7 codec authenticates preparation identity but
  cannot supply that qualified admission. Otherwise hardware could commit a
  transition whose terminal proof is impossible. The device contract now requires
  atomic lifetime checks before mutation and historical recovery after expiry.
  The actual native structural exporter and five normal parser tests now pass,
  including replay of 18 canonical send/redemption commands across three fixture
  contexts and re-signed candidate substitution rejection. All source, binary and
  fixture hashes remained unchanged. The final five Python evidence suites pass
  240 tests and 138 subtests with all 19 recorded inputs unchanged. Both CLI
  boundary tests and actual binary replay of all 18 complete projections pass
  with unchanged source/binary/fixture hashes. These synthetic fixtures do not qualify
  service behavior or physical evidence. Stock dispatch still rejects, so this
  review did not demonstrate an enabled monetary bypass. See
  [the precommit contract](kagemusha_device_bridge_v1.md) and
  [physical qualification requirements](kagemusha_v1_physical_evidence.md).
- **KGM-15 — Release blocker, canonical prepared-transfer hash mismatch.**
  The terminal circuit placed the amount after both sender-state digests, while
  the canonical model places it before those digests. The valid prepared-transfer
  constraint regression failed. The correction must preserve the model's exact
  bytes and all component substitutions; a changed expected digest alone would
  hide the incompatibility. The corrected both-field regression now passes with
  exact model bytes, all component substitutions and rejection of the old byte
  order. This fixed snapshot took 624.28 seconds for the full MockProver test;
  genuine terminal proof generation and refreshed keys remain pending.
- **KGM-16 — Release blocker, missing claim-fold challenge bus witnesses.**
  Mint claim RLC start rows installed challenge state but omitted the same values
  from their permutation bus cells. Nonzero distinct challenges therefore violated
  existing gates and copy constraints. The correction must populate the exact
  original bus witnesses and retain both-field substitution rejection, with no
  constraint relaxation. All three corrected RLC tests now pass in the frozen
  validation lane, including both start challenges, results and padding in both
  fields. The exact claim geometry and processed small-key size tests also pass;
  genuine claim-proof execution remains pending.
- **KGM-17 — Tracked-lock advisory vulnerabilities cleared; release artifacts unqualified.**
  The current tracked workspace `Cargo.lock` does not contain `smallstr`;
  `smallstr 0.3.1` ([RUSTSEC-2026-0215](https://rustsec.org/advisories/RUSTSEC-2026-0215.html))
  survives only in ignored historical tool/fuzz/sample lockfiles in this checkout.
  The tracked lock previously contained `lru 0.16.4`, which has a conditional
  panic-safety defect ([RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html))
  and was pulled by the vendored `concread` manifest's optional `arcache` feature.
  Core requests only `ebr`, `maps`, and `foldhash`, so the affected LRU code is
  disabled in the inspected Core/mobile graph. The vendored dependency and tracked
  lock now select patched `lru 0.18.2`. A fresh RustSec database also found
  `rustls 0.23.40` affected by
  [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html), so
  the tracked lock now selects patched `rustls 0.23.45` and `rustls-webpki 0.103.15`.
  On 2026-09-24, `cargo audit` against 1,267 advisories reports zero vulnerabilities;
  denying transitive unsound and yanked warnings also passes. Ten unmaintained
  warnings remain allowed by that invocation. The optional `concread` feature
  compile, exact release-target dependency graphs, signed bundled binaries and
  final release audit remain unqualified while compiled builds are held.
- **KGM-18 — Corrected; focused durable-finality validation passes.** A valid staged reserve
  receipt could fail promotion after finality framing changed nested alignment.
  The four-times-wire allocation estimate missed 17,496 bytes of additional
  alignment-copy charges in the reproduced receipt. The helper now uses Norito's
  owned-graph allocation envelope, capped at the unchanged 4 MiB, with independent
  sequence, element, field and depth limits. Exact staged/final decoding, repeated
  promotion, restart, unchanged persisted bytes and malformed-graph rejection
  regressions pass in the third retained Core diagnostic run. The maximum
  casting-corpus capacity fixture was separately corrected to use a real canonical
  registration. The isolated follow-up still rejects the maximum 1,000-binding
  corpus at the unchanged 4 MiB allocation limit (4,194,322 attempted bytes).
  A bounded allocation trace reaches only binding 507 of 1,000; the total deficit
  is not merely 18 bytes. The subsequent fixed-V1 borrowed decoder avoids those
  enclosing-field copies while retaining canonical decoding and every existing
  budget. A fresh isolated build passes wire equivalence across all casting phases,
  hostile field/length/variant/trailing-byte checks, outer-budget enforcement,
  finality receipt restart, membership/path substitution, and the complete
  1,000-binding staging/promotion/retry/restart case in 5.04 seconds. The 4 MiB
  ceiling and required binding count are unchanged. This closes the reproduced
  defect in that source-recorded snapshot; full current-source release validation
  remains separate.
  Sources: [sidecar decode limits](../crates/iroha_core/src/kura.rs) and
  [borrowed finality decoder](../crates/iroha_core/src/kura/kagemusha_finality_decode.rs).
- **KGM-19 — Release blocker, complete Terminal key/resource budget.** The earlier
  Terminal configuration used five Table8 SHA lanes and four dense accumulator
  lanes. Its 110 original selector bitmaps require 901,120 bytes at k=16, already
  exceeding the unchanged 65,536-byte verification-key limit. Even before Base
  and materialized selector columns, the fixed/permutation polynomial inventory
  gives a conservative 225,298,502-byte proving-key floor, above 64 MiB. These are
  configuration/serialization bounds, not measured generated keys. Selector
  compression retains the original bitmaps; disabling it increases the proving-key
  requirement. An early configure-only rejection now precedes parameter and scalar
  graph construction. Both configure-only rejection tests and the real small-k6
  serialized-key agreement test pass in the isolated build. The next source snapshot
  replaces all inline Terminal SHA jobs with a required authenticated complete claim,
  retaining candidate/Guard history, merging the full claim ancestry and binding all
  14 carrier values across the reciprocal audits. Its typed producer uses all 26
  original messages and authenticates four release-pinned claim/shard protocols.
  The pinned follow-up build now passes ten focused tests: all four helper pins,
  both-field history/shape rejection, dense geometry and early preflight, complete
  Send/Redeem queues, independence from later proof outputs, and changed semantic
  intent. The new reciprocal claim-tail tests and actual complete proofs remain
  pending. The dense-only auxiliary floor is now 27,263,214 PK bytes, but the complete Base
  graph, generated keys and proofs remain unqualified. Four dense lanes still use
  148 advice columns, or 296 MiB for one k16 polynomial vector, before other memory.
  The original key and mobile limits remain; SHA removal alone does not close this gate.
  Terminal now checks the distinct source indices referenced by each compact
  reciprocal audit against the exact four-lane k16 scheduler before allocating
  its consuming Base graph. This rejects impossible jobs early; it does not
  reduce key size or qualify the graph.
  Sources: [resource inventory](../crates/iroha_core/src/zk/kagemusha_v1_recursion/artifact_resource_preflight.rs)
  and [generation preflight](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs).
- **KGM-20 — Corrected; focused reciprocal-audit validation passes.** Terminal
  reused a State-specific helper that selected audit positions 48/50, which are
  history cells in Terminal's public column. Terminal's Eq circuit must instead
  bind the Ep audit at 41/42, and its Ep circuit the Eq audit at 39/40. The
  correction selects those exact Terminal cells and retains the existing audit
  transcript and dense curve equations. Both-curve tests cover changed audit
  limbs, relocation to the old positions, malformed columns and a recomputed
  digest containing a false curve equation. Static review passes. The first
  isolated regression aborts on the default test thread stack. The unchanged
  harness then passes all reciprocal cases with a recorded 32 MiB test stack in
  869.07 seconds. The fresh-source normal regression then passes in 1,025.14
  seconds using a test-local named 32 MiB thread and no environment override.
  Source/binary hashes remain unchanged. This closes the reproduced positional
  defect; final genuine Terminal proofs and release qualification remain blocked
  separately by resource/authority gates.
  Source: [Terminal reciprocal audits](../crates/iroha_core/src/zk/kagemusha_v1_recursion/terminal_authorization.rs).
- **KGM-21 — Release blocker, typed-SHA claim key convergence.** The actual
  supervised State diagnostic fails before State proving while generating its
  reusable typed-SHA claim artifacts. The convergence graph has 264 advice
  columns, 145 selectors (142 materialized), three instance columns and 163
  permutation columns. Its predicted PK is 1,316,114,662 bytes, above even the
  1,073,741,824-byte host diagnostic allowance. The 1,197,834-byte predicted VK
  is below that diagnostic allowance but does not satisfy mobile helper limits.
  The source-stable run exits after 1,123.99 seconds with peak RSS 1,950,744,576
  bytes; it creates no qualifying State proof. A sound claim representation and
  converged key graph are required, not an increase to the diagnostic budget.
  A fresh both-parity configure-only regression confirms that even one legal
  Base gate plus one lookup request yields 109 advice columns, seven configured
  fixed columns, two selectors and eight permutation columns: its predicted
  77,611,726-byte PK exceeds the 64 MiB release helper limit. An independent
  arithmetic floor excluding every selector representation still reaches
  69,206,654 bytes. Neither bound is a generated artifact measurement.
  A subsequent reviewed source change shares the identical 15-bit Base/RLC
  table and retains independent ownership when Base has no matching table.
  Its expected minimum is six configured fixed columns and a 73,417,382-byte
  PK, still above 64 MiB. The old selector-free impossibility bound no longer
  applies after that saving. The sharing change now compiles and passes its
  ownership, both-consumer overflow and actual serialized-key regressions. Compact
  Eq/Ep PKs shrink from 44,056,974 to 39,862,630 bytes, and VKs from 16,682 to
  16,650 bytes. The minimum production configuration bound remains 73,417,382;
  these compact fixture keys do not prove full claim-key convergence.
  The next genuine State profile includes shared range ownership and exact Base
  packing. It exits at the same claim prerequisite after 1,221.907 seconds, with
  1,928,134,656 bytes of peak enforced memory. Its actual graph has 7,687,503
  Base cells, 118 Base advice columns, eight lookup columns, 234 total advice
  columns, six configured fixed columns, 118 materialized selectors and 133
  permutation columns. The predicted PK falls to 1,085,204,558 bytes but still
  exceeds the unchanged 1 GiB diagnostic limit by 11,462,734 bytes; its predicted
  VK is 974,890 bytes. The production limits remain 64 MiB PK and 64 KiB VK.
  Exact packing succeeds; no State proof is produced. The observed reciprocal
  job contains 749 sources, so both dense lanes are necessary: one lane admits
  at most 504 sources at the existing k16 domain. Neither removing a required
  lane nor increasing diagnostic limits closes the production resource gate.
  The next applied change chooses compressed or direct selector encoding from
  exact synthesized inventories while requiring a single candidate to satisfy
  both key limits. A degree-threshold lower bound replaces an unsound inactive
  greedy estimate that could reject feasible keys before synthesis. On the
  measured graph, direct encoding removes only 966,656 bitmap bytes: the VK
  becomes 8,234 bytes and the PK remains 1,084,237,902 bytes, still over budget.
  A reviewed GLV change also removes one redundant modular carry per source,
  preserving the final canonical scalar equality and all curve/segment bindings.
  The isolated build passes twelve focused tests, including actual small proofs
  in both fields and encodings, checked key round trips, the greedy counterexample,
  rational normalization and valid/invalid carry witnesses. Eleven cases per field
  confirm 260 fewer Base cells and 60 fewer lookup entries per source. The full
  749-source extrapolation is 194,740 cells and 44,940 lookups; guarded full-graph
  execution remains pending. The first prior reciprocal matrix has passed and
  its full claim-tail matrix continues. These results do not establish production
  key or mobile closure.
  A subsequent isolated native Poseidon Claim integration passes 44 focused tests.
  It preserves the exact raw generator and all 65 rounds, domain/padding/order,
  source and public bindings, and six Base copies per permutation. Four genuine
  k12 proof cases pass with checked reloaded keys and changed witnesses under the
  same key; wrong public inputs, digests and corrupted proofs reject. Actual Claim
  configuration and maximum-source queue checks pass alongside all 22 resource
  regressions and nine packing/transport tests. This reviewed batch is applied
  after exact preimage checks and compiler-hold release. Its auxiliary PK floor is
  85,984,030 bytes and minimum legal Base profile is 119,555,166 bytes, both above the unchanged 64 MiB
  release cap. The explicit test-only diagnostic envelope now reaches the same
  early configuration guard as consuming keygen. A guarded run measured successive
  373-, 710- and 858-source convergence graphs at 2,360,000, 5,094,505 and 6,322,041
  Base cells before its memory-accounting guard stopped during a companion process's
  natural exit. No final converged key or State proof completed. The companion's full
  reciprocal-tail mutation matrix passed both tests with frozen inputs. Genuine
  State/Terminal/CommitWrapper closure remains outstanding.
  A subsequent isolated candidate passes 57 focused tests, including four actual
  k12 proof cases with the shared native Poseidon equality column, both-field
  protocol identity mutation tests, and exact GLV integer/remainder regressions.
  The latter remove 110 Base cells and 21 lookup entries per source. All twelve
  loaded artifact types keep authenticated fields private to the recursion module;
  all 24 full-library doctests pass, and an external compilation against the actual
  library rejects access to all 162 fields while accepting opaque storage/passing.
  The tested minimum legal
  Claim key estimate is now 98,583,446 bytes, which still exceeds the 64 MiB release
  cap. Its minimum legal 116 advice columns also require 232 MiB of scalar payload
  in one k16 advice vector. Both Claim key-consuming paths use the consuming
  prover, which already transfers owned values and removes eager duplicate
  placeholders but retains the final scalar advice vector. This is a source
  allocation count, not measured device RSS; the
  complete circuit/prover working set must be reduced and measured against the
  unchanged 128 MiB whole-process gate independently of key serialization.
  The reviewed seven-file batch is applied to the shared checkout after exact
  preimage and timestamp checks; its shared build remains separate from the isolated
  evidence. The subsequent current native bridge build passes 137 tests with all
  seven source hashes and its retained executable unchanged; its synthetic catalog
  does not qualify hardware or monetary admission. A corrected complete-fold native
  transcript follow-up passes full Core metadata compilation and 45 unique focused
  tests (28 actual Core and 17 exact native-queue tests), preserving the shared BUS,
  existing geometry, every original fold challenge/byte/equation and rejection
  behavior. Eight small real IPA proof cases pass. It remains isolated pending
  full proof measurements and shared-source application. A subsequent actual
  Core run passes all 64 focused tests using an optimized field dependency with
  debug and overflow checks retained. The subsequent optimized Core executable
  also passes all 64 focused tests with all bound inputs unchanged. Its fixed
  full State diagnostic fails after 319 seconds because a test-only emitted-row
  assertion omits the two new native folds (20,394 actual rows versus 15,444
  expected). The first Eq graph has 387,738 fewer Base cells before reciprocal
  auditing. Inputs remain unchanged and the child exits cleanly; the narrow count
  correction passes two extracted actual geometry tests and is installed in the
  next isolated snapshot. The corrected full Core build passes all 64 focused
  checks; its fixed State diagnostic passes the original failing assertion in
  both fields but times out after 2,700.103 seconds. Both fields complete the
  858-source graph at 5,253,683 Base cells and 81 Base advice columns. Peak owned
  RSS is 1,425,096,704 bytes; inputs remain unchanged and the reviewed guard
  reaps its child correctly. No final key or proof completes. A short CPU sample
  identifies repeated dense-MSM witness inversions, and the linked `pasta_curves`
  artifact remains at O0. The private inversion-reuse candidate and parser
  correction pass five extracted-source tests, including actual BGH19 reads and
  both-field counter equivalence. Complete row/terminal-point comparisons then
  pass in both curves, including accumulator handoff across split lanes; seven
  distinct follow-up checks now pass. Both candidates are installed in the
  phase19 private snapshot of 5,244 sources and five build inputs. Its full Core
  build and all 71 focused checks now pass, including eight small real IPA
  cases and the seven parser/counter checks. Actual Core, Pasta and wrapper
  artifacts are O3 with debug and overflow checks retained; all bound sources,
  installation and the retained executable remain unchanged. The fixed guarded
  State preflight succeeds, but the diagnostic times out at 2,700.076 seconds
  under its unchanged 24 GiB/2,700-second envelope. Peak owned RSS is
  6,654,443,520 bytes; all 5,249 inputs, executable, guard and supervisor remain
  unchanged, and the reviewed guard reaps its child correctly. Reusable typed-SHA generation
  completes with each Claim PK/VK at 799,022,510/6,058 bytes and each shard PK/VK
  at 8,533,246/13,290 bytes. These are actual diagnostic artifacts from the
  then-current `Processed` serializer; that Claim PK fails the unchanged 64 MiB
  release limit. The later structured-v1 serializer has not been measured for
  this Claim graph, so the historical 799,022,510-byte result is not its size.
  The run reaches
  credential typed-SHA proving but provides no full State pass. Its cached-key
  path uses the borrowed prover, whose eager advice buffers are separate from
  the already improved consuming-key path. Reducing either allocation alone
  cannot establish the unchanged 128 MiB device gate.
  The current structured-v1 resource preflight configures the Claim circuit
  with a strict minimum legal Base profile and computes an 8,668,355-byte
  proving-key bound, but its 96 advice columns require a 192 MiB dense advice
  basis at k16. Those are source-level bounds, not a generated full Claim key
  or a measured device RSS result; the 128 MiB whole-process gate remains open.
  The next private candidate moves selected complete ordinary transcripts into
  the existing two native Poseidon lanes after reserving mandatory work. Static
  comparison preserves the original complete ordinary/hybrid verifier bodies;
  compilation, challenge/stream/equation equivalence, constraint mutation tests,
  genuine proof verification and key convergence remain outstanding. The
  nonzero-view restart fixture now restores real WAL registry and body custody.
  The retained Broadcast and recovered-Apply carriers use heap storage, with live
  Broadcast allocation reserved before publication. Core56 passes the genuine
  nonzero-view production-services restart and inline-carrier size regressions on
  the default stack. This closes the observed registry-insertion stack failure.
  Broader startup/recovery and four-validator settlement qualification remain
  open; these focused passes do not establish a monetary or production release.
  All 19 current cases
  mapped from the original 18 failures pass on the preceding executable; 17 are
  additional unique tests, bringing its focused Core total to 81. The full proof
  remains unverified and historical failed runs retain their original results.
  Optimized diagnostic compilation changes no production configuration or
  resource limit.
  The preceding State diagnostic on the phase16 validation copy times out after
  2,700 seconds under its unchanged exclusive guard. Both fields complete the
  866-source graph at 5,693,567 Base cells and 87 Base advice columns, with peak
  owned RSS of 1,465,925,632 bytes. Inputs and executable remain unchanged; the
  cleanup authentication error and absent child exit code are preserved, and
  subsequent process checks find both owned processes absent. No final key or
  State proof is produced.
  Source: [typed-SHA key generation](../crates/iroha_core/src/zk/kagemusha_v1_recursion/mint_hash_generation.rs).

The platform credential circuit already constrains its positive hardware epoch
inside the proof. The shared credential-assignment helper now enforces the same
local invariant for Guard and MintAuthorization; direct both-field zero/positive
regressions pass in the retained diagnostic run. This strengthens a local invariant: the review did not
find an accepting full zero-generation proof through the existing authenticated
PlatformCredential relation. Guard and MintAuthorization keys must be regenerated
and their final geometry measured. Bootstrap's inactive predecessor generation
and encoding-only zero templates remain valid construction inputs; they do not
authorize a live credential.

The diagnostic State milestone now retains the original sender openings and
extends through actual paired TerminalAuthorization artifact/proof generation.
Its persisted candidate must match the State transport protocol and complete
public column; the inner State protocol, proof and history are independently
verified and never relabeled. The diagnostic compiles and its cheap boundary
tests pass; actual proof execution remains pending. It uses
test-provider secrets and structural commit evidence, so even a successful run
would not qualify an OEM commit. The full State/TerminalAuthorization/CommitWrapper
key graph must still close before payment, sender installation and ReceiveFold
qualification.

The frozen validation checkouts shared Cargo artifacts with the current checkout,
and a concurrent mobile build observed stale local-crypto metadata. Retained test
results and unchanged source hashes are diagnostic observations; they do not
establish isolated dependency provenance. Further frozen builds must use an
independent persistent target and revalidate their exact dependency graph before
serving as production evidence. The first corrected isolated snapshot now records
5,211 compiler/source/fixture files, five build inputs and 59 local artifacts. Its
compile-start test and 14 follow-up tests pass; the reciprocal test stack abort and
maximum casting-corpus allocation rejection remain failed results. A subsequent
5,212-source snapshot compiles the borrowed-decoder correction and passes its
equivalence test plus ten focused regressions, including the complete maximum
casting corpus and corrected reciprocal audit. These results validate their recorded pinned snapshots, not the
entire current checkout or physical hardware. The following 5,217-source snapshot
passes ten tests and retains three failed Terminal fixture filters. Carrier
encoding tests in both Pasta fields measure advice cells of 76/586/154 for
scalar/point/source-commitment encoding, down from 376/1,222/790; bounds,
coordinated witness mutations and cell-cache identity also pass. This is local
constraint evidence, not complete proof performance. The next frozen snapshot
contains corrected Terminal packing/reservation fixtures, active SendSplit queue
coverage and the shared range table; all nine tests pass, including all three
previously failed filters. Every recorded input and binary hash remains unchanged.

The private recursive State checkpoint codec is implemented, with proof decisions
and exact expected-state binding. Nine structural tests pass, but actual
capture/restore reaches the mandatory native provider-policy authority gate
before proof verification. A positive genuine-proof round trip and same-length
proof-substitution checks therefore remain unverified; structural acceptance
does not satisfy that gate. The remaining native coordinator dependencies are concrete:
a qualified service that seals one atomic latest checkpoint across
state/proofs/WAL/accepted replies, native response admission and challenge
consumption, integration of credential/Guard proof production, and finality
resolution. No production backend currently implements the four distinct mint
reservation, mint staging, peer staging and recovery-anchor certificates. Public
device replies do not contain those native-private records, so a response
  signature cannot supply them. Startup also needs a nonauthorizing challenge
journal before the authenticated lane's operation WAL can exist; qualification
currently reserves an operation before Core bootstrap.
The current stock backend intentionally reports unavailable. These are outstanding
implementation and qualification goals, not deployment switches.

Concrete source locations and completed test results are recorded with the
implementation in `status.md`. No active exploitation or qualified production
deployment was established by this source review. Rust/Swift/Kotlin circuit and
device findings come from direct code inspection; the generic security skill
does not provide language-specific audit coverage for those components.

## Physical clock-rollback transcript contract

After byte-identical outbox recovery and before the backup/restore cycle, the
observer must record `clock_rollback_begin`, `clock_rollback_applied`,
`expired_request_rejected`, and `clock_rollback_end` in that order. All four
bind one unique control and the active hardware boot. Begin/rejection bind the
same request digest, current aggregate state, logical counter and epoch.

The host clock must start strictly after request expiry, move strictly before
it, remain before it during the failed sender attempt, and be restored to at
least its initial value. Hardware trusted time must remain strictly past expiry
and nondecreasing throughout. The attempt must return
`expired_request_rejected` without advancing monetary state; its operation ID
cannot duplicate another operation. Observer event time remains monotonic and
is distinct from the intentionally changed host clock. Missing boundaries,
reused controls, substituted request/state/epoch/counter, accepted attempts, or
rollback of trusted time fail validation even with fresh observer signatures.

This negative sender exercise does not change delayed receiver admission:
payments already committed within their original request window remain
receivable after expiry.
