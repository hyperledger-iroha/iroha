## Active implementation — 2026-09-22

Work remains confined to `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. The following combines source inspection with the scoped
component validation below; it is not a security audit or hardware qualification.

The stored IPA implementation now contains the consuming assignment, coefficient,
lookup, product, vanishing, quotient-numerator, inverse-transform,
quotient-commitment and scalar stages. Its multiopening continuation now retains
guarded P, its derived blind and the original typed x3 with the complete original
proof owner. It preserves ordinary source grouping, evaluations, polynomial/blind
folds and transcript order; guarded inner IPA remains unfinished. The authenticated
Core indexed-key owner and encrypted polynomial store are present. These components
remain separate from the normal production proof entry points: the six direct
`create_proof` calls and two `create_proof_consuming` calls in
[generation.rs](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs)
use dense `ProvingKey` owners. The stored continuation and Core
`capture_indexed_proving_key` have only test callers in the inspected source.
Scalar evaluation, lazy H reconstruction, original advice-blind folding and the
guarded P continuation have the scoped evidence below. A separate satisfiable
fixture verifies six whole-PLONK cases using a test-only ordinary inner-IPA
handoff; it does not complete the production stored prover. Guarded inner IPA and
authenticated Core producer/key integration remain required. Incremental scratch
bounds do not establish whole-process memory or latency compliance. Sources:
[stored owner](../vendor/halo2-axiom/src/plonk/prover/stored.rs),
[guarded P continuation](../vendor/halo2-axiom/src/plonk/prover/stored/proof_evaluations/opening.rs),
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

The next completion sequence is: finish and connect the full stored proof path;
qualify real funded recursive payments and complete durable native ownership;
rebuild and exercise SDK/native artifacts; then close release, physical-device
and independent-review gates. Exact OEM services/profiles for the requested
brands remain external dependencies. Release authentication and fail-closed
activation must stay enforced throughout this work.

