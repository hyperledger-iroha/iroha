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
   current-source native artifacts; Swift, Kotlin, mirrored Java and remaining
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

## Requested device scope

The requested first-release families are iPhone, Samsung, Huawei, Google, and
Meizu. No model/OS/firmware/provider tuple has been qualified by this assessment.

| Family | Integration work and evidence required |
| --- | --- |
| iPhone | Swift/native integration, authorized provisioned secure-element service, exact supported model/OS/territory, and physical qualification. |
| Samsung | Kotlin/Android native integration and an authorized service implementing the complete non-forking device contract; qualify each model/firmware. |
| Huawei | Establish the exact Android or HarmonyOS application/runtime and authorized service, then build and qualify its native integration. Android coverage does not establish HarmonyOS coverage. |
| Google | Kotlin/Android native integration and an authorized non-forking service; qualify exact Pixel/model/firmware profiles. |
| Meizu | Establish model/OS/native runtime and authorized service availability, then run the same full qualification. |

Apple's NFC & SE platform requires an agreement and entitlement; access alone
does not establish KAGEMUSHA contract compliance. See Apple's
[platform requirements](https://developer.apple.com/support/nfc-se-platform/).
Android's rollback-resistant **key deletion** semantics do not specify an
application's aggregate-balance compare-and-swap journal. The latter remains a
separate KAGEMUSHA requirement; see the Android
[KeyProtection contract](https://android.googlesource.com/platform/frameworks/base/+/master/keystore/java/android/security/keystore/KeyProtection.java)
and [device bridge contract](kagemusha_device_bridge_v1.md).

## Security findings and implementation work

- **KGM-01 — High, monetary relation incomplete.** The original MintFold private
  recipient credential and credit opening were retained by Core but not fully
  constrained to the receiving lane and verified authorization in the composite
  relation. Host validation cannot replace these circuit constraints. The
  correction now constrains the recipient and opening bytes and routes State SHA
  messages through the mandatory authenticated ordered claim fold. Focused Rust
  checks and actual artifact/resource gates are still required before closure.
  Sources: [recipient/opening constraint](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L2624)
  and [claim consumer](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L1705).
- **KGM-02 — High, operation recovery integration incomplete.** Swift exposed
  operations still allocated retry identities internally while Core had moved to
  caller-owned IDs; some SDK provider calls still used the retired allocator
  signature and untagged sender reservation bytes. A lost native return must not
  allocate a second monetary operation on retry. The caller-ID and canonical
  reservation changes now have focused Kotlin/Java/C# coverage, with Swift
  typechecking. C# also now rejects missing-state re-bootstrap, journal rollback
  and recovery equivocation. Current-source native execution is still required.
  Sources: [Core reservation](../crates/iroha_core/src/zk/kagemusha_v1_state/coordinator_operation_store.rs#L274)
  and [C# ID admission/recovery](../csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/KagemushaWalletV1.cs#L1014).
- **KGM-03 — Medium, bridge response substitution.** Outbox release admitted a
  structurally valid response for a different canonical installed envelope.
  Match the exact request envelope before returning backend results and preserve
  cleared C outputs on rejection. The fix and regressions are implemented; Rust
  execution awaits resolution of the current workspace compilation errors.
  Source: [native response validation](../crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs#L553).
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
  substitution tests and the isolated projector test pass; the stable-source
  combined run is being validated. Actual admitted OEM verifiers, roots and
  physical runs remain required for every enabled profile. See the
  [exact closure contract](kagemusha_v1_physical_evidence.md) and
  [release verifier](../scripts/verify_kagemusha_v1_release_evidence.py#L2117).
- **KGM-06 — Medium, JavaScript model mutation.** A public internal-value getter
  exposed mutable WeakMap backing data behind frozen canonical models. The
  getter is removed and only internal encoders access backing values. Public
  projections are defensive, with nested mutation/canonical byte regressions
  passing. Source: [model backing boundary](../javascript/iroha_js/src/kagemusha.js).

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
