# KAGEMUSHA verification checklist

Status: working checklist, 2026-10-06, for proposal revision 2026-10-04 (split
lineage). This document records useful checks for
[the single protocol](kagemusha_single_design_proposal.md). It is not an
approval process and does not block use, production integration or deployment.
The filename is retained for existing links. Section 8 records current
component checks; complete protocol and physical-device qualification remain open.

Runtime signature, proof, authorization, replay and durability checks remain
part of the protocol. Removing a release gate does not make an invalid
payment valid or make an unimplemented security property an established fact.

## 1. What the evidence should distinguish

A state package contains its step proof `σ` and provider receipt `τ`. Send,
Unload and Retiring commit only from a folded head and their packages also
carry the lineage wrap `Ω` of that predecessor; Bootstrap, Load, Receive,
ArchiveSent and RefreshPolicy may commit from an unfolded head and carry no
`Ω`. The operation tag fixes the receipt's `proof_digest` domain. Before
`Advance`, the wallet natively verifies the incoming packages, proves `σ` and
natively verifies it. The provider then executes atomic, durable
`Advance(expected_head, new_head, proof_digest, operation_id, recovery_capsule)`
and produces `τ`, which it natively verifies before release. In the background
the wallet proves the lineage proof `Λ` and its wrap `Ω` and natively verifies
`Ω`; verifiers reject a Send, Unload or Retiring package, or a fee claim, that
lacks its predecessor `Ω`. The current receipt is checked natively; the lineage
proof covering a step verifies its own and incoming receipts recursively. Tests
should follow that order and bind the receipt to the exact proof digest and
state transition.

A committed Send irreversibly debits `amount + fee`. Recovery delivers the
same exact Payment bytes to the bound receiver, which credits them once.
Timeout, interruption, rejection, missing delivery evidence or sender regret
never restores the debit. No step or lineage relation has a reversal operation.

The single target uses a software provider and marker in the released wallet
on a stock vendor OS. The phone must be unrooted, not jailbroken and free of
rootkits or other OS compromise. Enrollment verifies available platform
evidence, including locked verified boot where attested. Continued absence of
runtime compromise is a security assumption, not a fact proven by enrollment.
Record that assumption with every result. Stronger hardware enforcement
against a hostile OS is optional research, not a prerequisite for this design.

Platform attestation records particular signed facts about a key, app or boot.
It does not establish that the running OS is uncompromised at payment time.
A valid transition proof does not, by itself, establish that its predecessor
was never used elsewhere. Record the component that enforces that property.

The checklist follows the completed-payment properties in the proposal:

| Property | What to check |
|---|---|
| P1a | Accepted value is durably owned at completion, subject only to the P4 exception. |
| P1b | Once the receiver's local fold reaches its current head, which covers the crediting head, with no network, counterparty or approval, the value is spendable onward offline and can be unloaded. |
| P2 | The payer cannot spend it again under the provider's stated security assumptions. |
| P3 | Completion needs no later reconciliation, approval or settlement. |
| P4 | Later discovery of payer misconduct does not revoke an honest recipient's accepted value. The sole exception burns only an incoming Payment that passed native checks but fails in the receiver's lineage proof. |
| P5 | Only an explicitly enabled regulatory control requires connectivity. |
| PC | Native verification, the receiver's step proof, the durable commit, recovery data and fold witnesses finish before the receiver reports completion; only the local lineage fold follows, and it changes ownership only by the P4 burn branch. |

## 2. Records and independently runnable work

Use a record per check: **not run**, **observed as expected**, **deviation**,
**source reading**, or **not accessible**. Include the steps, expected result,
actual result and limits of the conclusion. None of these labels grants or
withholds permission to use or integrate the implementation.

Identify the protocol revision, source commit, dependency lock, build hashes,
step and lineage relations and wrap, provider and firmware, attestation policy,
device model, OS build, security patch and carrier. Keep the raw signed bytes,
proofs, key hashes, logs, timings and fault traces alongside the reproducible
procedure.
Where raw evidence contains identifying data, retain it in controlled storage
and record its digest and access location. Pin external source code by commit;
record vendor-document retrieval dates and content hashes.

The work below can start independently. Primitive discovery and relation
design can proceed together. Storage probes and carrier measurements need no
prover; label their stand-in payloads explicitly. Complete proof and device
checks naturally need the corresponding implementation, but are not
prerequisites for other integration work. Long-duration experiments can run
beside development; there is no mandatory 90-day wait.

## 3. Provider behavior and platform claims

- Capture actual Android TEE and StrongBox attestations independently. Record
  the chain, origin, security levels, hardware/software-enforced fields,
  use-limit tags, app identity, boot and patch claims, and evidence freshness.
  A feature flag or another vendor's result does not describe this device.
- Capture Apple evidence separately. App Attest authenticates its own key and
  supplied transcript; it does not independently attest a separate payment
  key or report payment-time OS integrity.
- Identify which trusted component owns the current head, enforces
  `expected_head`, commits the successor and retains the recovery outcome.
  Document how a verifier authenticates the provider and its contract.
- Interrupt signing before its result returns. If the successor was selected,
  retry or recovery must yield that same committed package offline. A selected
  head with no recoverable outcome is a durability defect, not merely latency.

The following hostile-OS experiments are optional research beyond the target
assumption. They do not gate integration or production use. Their results
must not be reported as guarantees of the software provider:

- Bypass the app and OS key-store service where research access permits.
  Begin concurrent operations from one head, varying messages, cancellation
  and completion order. Look for two different receipts accepted for the same
  predecessor. Record whether the interface itself serialized the attempts.
- Save and replay key blobs and host database rows after use. Upgrade the same
  old blob twice and exercise every returned copy. Record which component
  prevents a second authorized successor.
- For a counter candidate, attempt chosen and repeated counters, concurrent
  assertions and assertions after rollback. A failed hook shows only that
  the attempted attack failed; it does not prove counter unforgeability.
- Record how a rooted, unlocked or research device differs from the ordinary
  device. Do not transfer results across changed enforcement without an
  explicit supporting argument. Inaccessible privileged tests remain
  inaccessible, rather than evidence of security.

## 4. State machine, proof storage and crash recovery

Check the written authority contract and the implemented transition relations
against the same fields: scheme, asset, predecessor, operation, counterparty,
amounts, successor, proof digest and recovery capsule. Trace the durable
linearization point before release of a usable Payment.

Exercise sends, receives, loads, unloads, delivery retries and phone replacement
under message loss, duplication, reordering, concurrency, process death,
power loss and restoration of older host files.

Here, migration means phone replacement through an ordinary offline payment,
not a separate monetary operation. A change of scheme uses voluntary unload
and load through the ledger, with no required change to an existing wallet.

Expected invariants:

- Loads and transfers conserve value. Arithmetic cannot overflow, underflow
  or cross assets or schemes. The proof and native verifier agree.
- A Request is a signed setup quote bound to both wallets, the payer's next
  send ordinal, amount, fee terms and nonce. It creates no monetary state,
  exclusive receive slot or receiver ordinal. A stale or declined quote can
  prevent a new Send; cancellation, replacement or later expiry of the quote
  cannot invalidate an already committed Payment.
- Each Send consumes its send ordinal and debits `amount + fee` exactly once.
  After its commit, every timeout, crash, transport error, receiver decline and
  lost delivery evidence leaves that debit unchanged. No step or lineage
  relation has an operation that restores it on those grounds; reject retired
  refund/refusal monetary objects and attempts to recredit the sender through
  other operations.
- Receive checks nonmembership of the exact `credit_id` against the
  consumed-credit store authenticated by the head's `consumed_credit_root`,
  inside the serialized `Advance` section, credits once and inserts
  `credit_id → (amount, receive sequence)` in the same committed transition.
  Its receipt binds the complete Payment digest, which no state commitment or
  `σ_recv` statement contains; `Λ_recv` later proves the update and the digest
  in-circuit and records the digest in the credit-digest root. Entries are
  permanent. Deliver a delayed Payment after unrelated receives and then replay
  it repeatedly, including after restart and delivery evidence cleanup: only
  the first valid Receive changes the balance. Repeat with a restored stale
  native store, which fails authentication against the head roots, and with
  duplicate delivery on two carriers during a `σ_recv` re-prove, which credits
  once. A `σ` whose successor root differs from the root recomputed from the
  authenticated store is discarded before commit.
- An incoming Payment that passes every native check but fails in `Λ_recv`,
  including a duplicate `credit_id`, takes the burn branch: its `credit_id`
  stays consumed, `Λ`'s `burned_total` grows by its amount, the credit-digest
  root records it as burned, no accumulator of the burned Payment enters `Ω`,
  the receiver's other value stays spendable and reserve accounting counts the
  burned credit. Only that Payment is burned. Every later Send, Unload and
  Retiring `σ` takes `burned_total` from `Ω(pred)`; a `σ` that uses the stale
  core value is rejected natively and in `Λ`.
- Send, Unload and Retiring packages carry their predecessor `Ω`; Send,
  Unload, fee claims, retirement and device moves from an unfolded head are
  rejected. A wallet's next Send waits until its current head is folded, also
  after its own previous Send. Crash and restore between commit and fold
  resume the fold from retained witnesses; loss of a fold witness or of a
  native authenticated store without a recoverable copy is reported as custody
  loss, never a silent wait.
- A Payment whose Request names a different payer wallet than `Ω(pred)`, or
  whose carried payment key or credential digest differs from `Ω(pred)`'s, is
  rejected before mutation, and so is a payer credential from the Offer whose
  digest differs.
- With each regulatory control enabled, the receiver's verification of
  `σ_send` rejects a Send that violates it, and a `σ_send` proved under the
  verifying key for a different enabled-controls mask than `Ω(pred)`'s.
- Each voucher and redemption is consumed at most once. Funding and retirement
  cannot recreate a previously consumed voucher or reverse a committed Send.
- A nonzero fee is earned at Send. Its exact retained Payment authorizes one
  fee payout, keyed by `credit_id`, only to its fixed beneficiary. Claim the fee
  before Receive and retry after Receive: there is one payout total. Failed
  delivery or decline neither cancels the fee entitlement nor restores its
  debit. Map leaves and immutable capsule bodies do not contain the very
  receipt or final package that authenticates them.
- Optional Credited evidence is the committed Receive package or a read-only
  `CreditStatus` {statement, `proof_digest`, receipt, `Ω`, opening} against a
  folded receiver head. Verify the Receive package's step proof and provider
  receipt, which binds the exact Payment digest, or decide the folded head's
  `Ω`, check its receipt and the opening of `credit_id → (Payment digest,
  burned flag)` in `Ω`'s credit-digest root, recipient and scope. Another
  Payment, an unconsumed credit or an invalid receipt fails; a burned credit
  reports **delivered, burned**. `ArchiveSent` verifies this evidence natively
  before `Advance` and in `Λ_archive`, and removes only the corresponding
  retained outbox entry, without a monetary effect. If the in-circuit check
  fails, `Λ_archive` takes the no-op branch: the entry stays pending in `Ω`'s
  pending-outgoing root, its Payment bytes are kept, and it can be archived
  again after a later Send, Unload or Retiring. The payer deletes the delivered
  Payment bytes only after a durable `Ω` covers the ArchiveSent step on its
  archive branch.
  Evidence loss cannot change either balance or remove a permanent
  consumed-credit entry. The flow creates no acknowledgement chain,
  receiver pruning obligation or prerequisite for onward spending.
- `Advance` retries recover the same committed outcome. Once a Payment is
  assembled and released, every retry returns its exact original bytes,
  including proof and receipt. Crash before release must finish the selected
  Send, retain its complete Payment and deliver that Payment; it cannot select
  another successor, regenerate a different released Payment or restore value.
  Loss of every retained copy of released bytes reports delivery-data loss;
  generating a new receipt or proof is not recovery. Inject a faulty receipt
  signature before first release: it fails native self-verification and is
  signed again, and no failing receipt is released.
- Phone replacement uses the ordinary payment path. Interruptions preserve
  its exact outcome; the old phone retains unresolved obligations, with no
  copied balance or separate migration authority.
- Crashes preserve authenticated recovery bytes sufficient to reconstruct
  the complete package and prove its next transition offline. Temporary host
  unavailability permits retry. Deliberate total erasure of the private
  recovery bytes is custody loss: a digest cannot reconstruct those bytes.
- Every message, per-operation buffer and recovery object respects the
  proposal's bounds. Exercise zero, the limit and one over it. Preflight
  available storage before accepting an operation. Unresolved operations can
  accumulate indefinitely. Permanent consumed-credit entries and their map
  openings also consume storage; do not prune them by age or claim globally
  bounded wallet storage.
- Capacity pressure is handled before acceptance. Preserve already committed
  Payments and distinguish a capacity wait from loss of accepted value. Test
  repeated, cyclic and branching histories and migrations, including growth
  of unresolved operations, rather than only a short acyclic payment chain.
- An older wallet can verify and spend its admitted packages after another
  wallet upgrades. Exercise the unchanged step relations, lineage relation,
  wrap, verifying-key allowlist and verification interface within the scheme
  without an unconfigured requirement to reconnect.
- Retirement: race load issuance with the atomic ledger instruction that closes
  new loads. Previously issued vouchers stay recoverable and new loads after
  closure debit nothing. Deliver an already committed Payment while the receiver
  is retiring; it remains receivable once. Also issue a quote before Retiring,
  commit its Send afterwards and deliver it: retiring cannot cancel that credit.
  Retirement stops issuing new quotes and preserves old receive custody.
  Neither a zero balance nor a receipt proves
  that no delayed incoming Payment exists. Intentional custody deletion reports
  the loss of late incoming value; it is not a safe monetary drain certificate.
- Interrupted enrollment: try issuing a voucher before the ledger records the
  completed Bootstrap receipt; issuance fails without debit. Abandon is permitted
  only while the original enrollment marker remains selected and Bootstrap has
  never committed. Once Bootstrap commits, including an uncertain activation
  response, resume that incarnation and use Retiring; do not select Abandon.
  If a voucher is funded, load the original once without cancelling its
  obligation, refunding its mint, resetting ordinals or initializing a second
  authority. Abandoning an unused enrollment changes no monetary balance.
- Time anchors: delay the issuer's response to near the response-age bound and
  sign it just before a lease end; the wallet must treat the lease as ended.
  Sleep the device for hours while anchored; elapsed time must include the
  sleep. A Send whose time interval touches two windows is counted in both.
- Marker coverage: at every step of enrollment, a normal transition,
  retirement and abandonment, the key is covered by exactly one durable current
  marker or a terminal marker.

Use a finite-state model for adversarial schedules and an inductive argument
for unbounded executions. State the bounds of each model check; a bounded
search does not prove unbounded-hop safety. Review the provider contract and
proof relations together, identifying which component discharges each
obligation instead of having each assume the other enforces it.

## 5. Real proofs and protocol interoperability

Generate real zero-state and load proofs, step proofs, lineage proofs and wraps
for a complete load → A → B → C → unload lineage. Include split/change, delayed
and duplicate delivery, receive-then-send, phone replacement and cyclic
transfers. Retain witness fixtures, proofs, verification keys and component
resource measurements, including lineage fold time, peak memory and energy.

Try forked predecessors, changed amounts, duplicate and reordered credits,
forged or substituted provider receipts, altered proof digests, missing
ancestor authorization, mixed schemes, wrong assets, malformed encodings and
mismatched relation identifiers. Record the trust boundary rejecting each
case. Mutation tests should show a named failure when a monetary invariant
is removed; ordinary valid fixtures alone do not exercise that invariant.
Include mutations that restore a committed Send after timeout or decline,
omit consumed-credit insertion, permit its later removal, or replace the
retained Payment bytes during recovery. Each must be rejected or violate a
named invariant in the test. Replace a Payment's P-256 signature `s` with
`n - s`, and vary signature encodings or verification dependencies. Native
and recursive checks must reject altered bytes; they must not normalize an
incoming Payment into a different digest or insert an alternate consumed entry.
Signing normalizes before freezing the original canonical object.

Compare native and recursive verification of the same package. Native and
in-circuit verifiers must accept exactly the same set for every object that
`Λ` verifies after a native check: run differential and fuzz tests, in the
relation owners' CI, over `Ω` including its deferred values, `σ`, `τ`, the
Request, Credited evidence, load vouchers, fee schedules, certificates,
credentials, and policy, list, time and credential updates. Exercise the
poison-pill burn branch of `Λ_recv` and the no-op branch of `Λ_archive`; a
tampered deferred value; a mixed history; a wrong relation identity; a `σ`
under the wrong verifying key for its operation tag or enabled-controls mask;
an `Ω` whose head, credential digest or payment key does not match the `σ`
statement or `τ`; a `σ` whose `burned_total` or pending-outgoing input differs
from `Ω(pred)`'s; and a receipt whose `proof_digest` omits or swaps `Ω`, or
uses the domain of another operation tag. Check that current receipts are verified before credit and that
successor lineage proofs bind those receipts to the exact predecessor and
incoming packages. Check that an invalid package cannot become valid merely by
wrapping it in another proof.

The scheme keeps its fixed step relations, lineage relation, wrap,
verifying-key allowlist and verification interface for its lifetime. Test
voluntary transfers to a different scheme separately; breaking changes cannot
require old wallets to update or reconnect to keep using their existing value.

## 6. Complete device exchanges

Run real proofs and the software provider on stock phones for each claimed
device and ordered sender/receiver/carrier combination. Label runs with
padded proofs so their timings are not attributed to a real prover. Record
untested combinations without claiming platform or vendor universality.

- A pays B; after B reports complete and its local fold reaches its current
  head, B pays all received value to C with radios off. Before that fold, B
  shows the completed credit as **spendable after local proof** with its fold
  backlog, and Send and Unload wait for the fold. Repeat after restart, temporary
  storage unavailability and recoverable faults. Record total
  private-recovery-byte erasure as custody loss, not successful recovery from
  a digest. Inspect both endpoints: the payer reports its durable Send
  independently and reports **delivered** only from valid receiver completion
  evidence.
- After A commits Send, interrupt delivery before B receives it; also exercise
  a decline, capacity failure and a lost Credited message. A's amount and fee
  never return. B can receive other payments before the original exact Payment
  is retried, and can then credit that Payment once. Withhold optional Credited
  evidence and confirm that B's accepted value becomes onward-spendable after
  its fold without that evidence. Recover that evidence later through
  `CreditStatus` without changing B's state.
- Cut power at authority, proof-store, journal and release boundaries,
  including inside `Advance` and after commit but before return. Distinguish
  forced restart from actual loss of power to storage. Test locked,
  unavailable and full storage without treating transient errors as absence.
- Exercise platform backup, vendor transfer, reinstall and update paths.
  Repeat rollback attempts against the actual provider boundary, and record
  precisely which destructive events are outside its durability assumptions.
- After B accepts value from A, give B and C authenticated misconduct evidence
  about A. B's credit remains usable, and C accepts B's otherwise valid
  package. Check native and recursive verification.
- With regulatory controls off, pay, recover and spend onward offline after
  reboots, clock changes and long idle periods. Run each enabled control
  separately. Record the longest tested interval and any network dependency.
- On a device class that fails or has no published lineage budget,
  activation, Load, Receive and RefreshPolicy refuse before commit, and the
  voucher or Payment stays deliverable. On each passing class,
  measure fold time, peak memory and energy per operation, including folds
  interrupted by app suspension and resumed at the next app run. With a
  receiver's Request-issuance policy (minimum amount, rate limit, maximum
  unfolded backlog), new quotes stop at the configured limit and committed
  Payments stay receivable.
- Measure every package-carrying message against the **10,000-byte** bound,
  including the Lineage message, the single-parity `Ω` inside Payment and a
  CreditStatus, and each Offer with the payer credential against **2,048
  bytes**. If `Ω` cannot keep Payment, or CreditStatus cannot keep Credited,
  within 10,000 bytes, record a deviation; a fallback is a new owner decision.
  Report schemes with enabled controls separately. Measure
  the **2-second p95** target from payer confirmation after the Request to the
  receiver's durable completion (P1a). Include intervening framing, retries,
  carrier setup, the payer's remaining step-proof time, receiver verification,
  the receiver step proof and both durable commits. Report separately, per
  device class, the fold time until the value is ready to spend onward and
  tap-to-done including setup and the confirmation dwell. Measure the payer's
  later delivery confirmation separately. State sample counts and the
  percentile estimator before measurement; retain raw samples, failures, cold/warm
  results, peak memory, energy and thermal effects. These are requirements
  and targets, not measured results.

Zero observed failures does not establish zero failure probability. Under
independent, identically distributed Bernoulli trials, n clean trials give an
approximate 95% upper failure-rate bound of 3/n. Adaptive attacks, correlated
storage faults and different devices do not acquire that model merely by
repetition. An empirically successful delay is not a storage durability
contract. Record counterexamples and repair the underlying mechanism.

## 7. Integration and existing material

Compare node, issuer, wallet core, Swift and Kotlin implementations using the
same canonical vectors and real packages. Exercise ledger loads, unloads,
fees and migrations on a valid multi-peer network. Check reserve accounting,
replay indexes, finality and issuer/provider key separation. State the stock-OS
assumption with the deployed software provider; do not describe its marker
as hardware enforcement against an OS takeover.

Re-run affected checks after changes to provider, firmware, OS, step or
lineage relations, wrap or storage behavior. Record what results still apply
and how existing offline packages interoperate. Use the proposal's retirement map and actual caller
inventory when consolidating implementations; verification work does not
create a separate approval condition for integration or production use.

Starting points, not evidence that this protocol has passed:

- Android: `kotlin/kagemusha-wallet-android/src/main/java/org/hyperledger/iroha/sdk/offline/wallet/` (the Android `Advance` adapter, payment-key and backup rules) and `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaP256Codec.kt`. Generate and inspect keys even when feature flags are negative. The old KeyMint single-use probes of the retired protocol were deleted.
- Apple: `IrohaSwift/Sources/IrohaSwift/KagemushaWalletApplePlatformV1.swift` and `KagemushaWalletAppleAppAttestV1.swift` in the same directory (Secure Enclave payment key and App Attest enrollment evidence of step E5). The old `examples/ios/KagemushaAppAttestProbe/` app of the retired protocol was deleted.
- Attestation: `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/attestation/` and `python/iroha_app_attestation/src/iroha_app_attestation/`.
- Formal work: none at HEAD. The old `formal/kagemusha_v1/` model of the retired protocol was deleted; write a new model from the proposal.
- Evidence records: §8 below. The old physical-evidence record of the retired hardware protocol and its verifier script were deleted; record new device results here with their provider assumptions.
- [Pinned AOSP characteristics](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/common/src/tag.rs), [operations](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/ta/src/operation.rs) and [upgrades](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/ta/src/keys.rs): reference behavior, not vendor-firmware evidence.
- [Android attestation](https://developer.android.com/privacy-and-security/security-key-attestation), [Apple validation](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server) and [Apple fraud-risk guidance](https://developer.apple.com/documentation/devicecheck/assessing-fraud-risk): capture the exact statements and their limits in each evidence record.

## 8. Recorded results

### Current implementation checkpoint (2026-10-06)

The current `optimizations` checkout includes continuing uncommitted coordinated
work over merge `e5c89263b0`; a commit hash alone does not reproduce it. Earlier
component runs below used the dirty `bddb072b97` checkpoint; their source/binary
records remain attached to those runs, not retroactively to the merge. No authenticated artifact
set, full-protocol qualification, phone result or completed offline payment is
claimed. Earlier measurements of deleted implementations below do not qualify
this candidate. The [Λ/Ω construction](kagemusha_lambda_omega_v1.md#10-milestones-named-tests-and-thresholds)
defines the unchanged engineering limits and current shared-host method.

| Milestone | Current evidence | Remaining boundary |
|---|---|---|
| G1 revision-4 σ and controls | Native proof crate release suite: 90 passed, 41 intentionally ignored; statement gadget 5 unit and 8 integration cases passed. Shared Rust vectors, all 8 Send masks and both Receive selectors are pinned. Fixed64 usage, recorded blacklist, share expiry and time-span checks have negative tests. Real k14 proofs for masks 2 and 7 verify at 3,456 B; other descriptor lengths are 3,296 B at k12. | The same suite and real k12/k14 cases pass after PIPA-R migration. Authenticated artifacts, cross-language rebuilt consumers and exhaustive ignored sweeps remain open. No wallet protocol path consumes σ yet. |
| Envelope arithmetic | Canonical fixed Payment overhead is 1,723 B; largest current σ_send is 3,456 B, leaving at most 4,821 B for Ω. The earlier 4,736 B Ω was an estimate, not an implemented proof. The first generic Ω frame descriptor requires 11,392 B transport; bounded foreign arithmetic and direct S6 reduce it to 10,944 B. | Both generic Ω descriptors fail the hard cap. Compact layout work remains necessary. Actual complete Norito Payment/Status encodings must establish frozen bounds; no stand-in proof establishes a completed payment. |
| M3b carry/range binding | [Carry memo](kagemusha_ff_carry_v1.md) includes the four corrections, independent exact rederivation and reviewed source hashes. FF 20 existing plus 2 boundary tests and 4 shared-Q-layout tests passed. | Scoped engineering review, not an external cryptographic audit or proof-engine qualification. |
| M3 consuming witness / MSM budget | Native engine 228 tests passed (4 ignored); Pasta 102 passed (1 ignored). All 42 non-timing oracle cases retain their proof bytes after owned-buffer and quotient-evaluator changes. Owned and borrowed witness paths preserve proof bytes in parity tests. Shared nonblocking MSM reservations enforce a process-wide 64 MiB ceiling. | Performance gates still require current-candidate fresh-process qualification. |
| Retained native consumers | SoraFS PoP uses native PIPA-R with consuming witnesses, full opening verification and new pinned circuit/key identities. All 37 proof and 25 Node PoP consumer tests pass; canonical fixtures, signed inventory and strict lint pass. Native Kaigi passes 22 library tests, seven Core_zk Kaigi tests, two descriptor/key-carrier tests and 48 verifier/admission/guardrail tests. The rebuilt JavaScript host passes all 22 Kaigi tests. All three rebuilt Core Kaigi lifecycle/admission integration tests pass. The rebuilt Core real-proof release builder and native policy/gas/guardrail components pass; current Torii native-policy, exact-identity, allowlist and KAGEMUSHA route/finality tests pass. The key carrier binds the complete compiled descriptor and processed Vesta key; old keys and labels reject. The native development vote fixture passes reproducibility and adversarial checks; the native confidential host hash passes 35 regression tests. The confidential production implementation now uses consuming native PIPA-R with three new exact circuit/key identities and one native public column. Its activated Rust confidential suite passes 50 tests, including all three production-depth real proofs at 3,680 B each; the independent relation corpus passes ten host/adversarial cases. Focused SDK tests pass: Kotlin 19, C# 103, Python 12 and isolated Swift registry 10. The downstream Core/Torii/CLI test compilation passes before the final five integration-fixture migrations. | Packaged SDK and network qualification, final confidential consumer execution and tally/dispatch retirement remain open; shared vendor dependencies and the temporary oracle cannot yet be removed. The reviewed dependency-budget baseline includes the promoted native confidential dependencies and passes; the release source seal predates substantial concurrent work and still fails, so release packaging is not qualified. |
| Measurement controls | Fallible direct CPU/kernel RSS probes; verified 1/4-worker pools; two separately verified owned-witness proofs; source/binary and actual descriptor binding. Python runner 14 tests pass, including missing configurations, stale identity, malformed probes, cap overruns and retained-report revalidation. | The complete nine-process, three-block schedule has not qualified a candidate. Invalid attempts remain recorded; elapsed-time failures are never normalized. |
| PIPA-R / recursion | Native PIPA-R typed transcripts/proofs and PIPA-AS folds pass both-curve tests. The complete succinct circuit interpreter matches real source proofs. Total soft accumulator decoding, malformed-claim → burn → Trivial replacement → hard fold, and exceptional identity-correction rejection pass on both curves. Obligation tests cover the branch truth table and all 14 unsplit schedules, with four fixed Vesta fold slots including explicit trivial fillers. The [soundness argument](kagemusha_recursion_soundness_v1.md) records assumptions and outstanding review. | A genuine Q → A → Ω composition, full key continuity, operation relations and recursive mutations remain open. Component proofs do not establish unbounded-PCD soundness or a joint simulator. |
| Q signature relation | Five integration tests pass for exact ten-word slot binding, fixed/variable keys, malformed raw256 inputs, low-S and verdict rules, and witness-independent layout. The 5V/1F shape reaches 64,238 rows / 747,364 cells. A real one-slot native proof is 8,576 B and its opening decides. Raw256 bridge cell tampering and shared-table audits pass. | Signed-object semantics, issuer-role authorization and complete operation composition remain required. Q proofs are local, not the transported Ω. |
| Q sigma relation | Real Receive-k12 and incoming Send-k14 proofs are verified in Q with a hard three-slot local accumulation. The shared byte/ECC layout has 29 advice, 42 fixed, 22 equality columns and 12 lookups; largest row span 50,799 at k16. Its actual native Q proof is 10,496 B and verifies, with altered public chunks rejected. Native preparation binds exact proof lengths, class keys, selected claims and exported frames. | One four-worker synthesis/prove/self-verify diagnostic took 12.8297 s; this is neither an isolated proof timing nor qualification. Q is local, not the transported Omega proof. |
| A / Ω recursive frames | All 14 A variants share the tested 69-field frame. Same-tape statement/message mutations and isolated source k12/k14/k16 Ω frames pass. The corrected common-Q2/A4 Bootstrap/Load catalog rebuild passes exact descriptor and terminal-key equality after binding its actual Ω digest; both genuine outer proofs decide. The full controls-off Send chain passes all five source proofs and pending decisions at 8,480 B per A proof, with maximum assigned rows 50,098 / 56,018 / 57,868 / 61,864 / 60,051. It binds the own current Credential, direct Enrollment certificate and Receipt, full 320-byte public predecessor plus proof/P/V/σ digest, pending/fee/unchanged maps and every required opening. Stage/task omission, receipt substitution and internal-key misuse reject. | The common Bootstrap/Load generic Ω transports 11,360 B and fails the cap. Send is not yet added and rebound into the final uniform catalog; enabled Send masks, remaining operation composition and the final rooted Ω key remain open. The earlier three-stage Load lacked mandatory C4 authorization and is only a partial diagnostic. A three-bus Load diagnostic also fails k16 capacity. Frame fixtures alone do not authorize operations. |
| Compact Ω layout | The complete compact descriptor computes 4,768 transported bytes (11 advice, 11 fixed, 25 advice queries, 12 fixed queries, five equality columns, one lookup), 53 B below the derived cap. Its tagged `(width,value)` table has 65,528 rows for exact widths 3–15; one/two-bit widths use polynomial gates. Both-field alias/boundary/layout and full interpreter/differential tests pass. The narrowed unsigned Proper-product kernel passes both fields, Pasta moduli, all batches 1–8 and adversarial/cell mutation cases; wide retained paths also pass for all four moduli. | The genuine common-Q2/four-bus Bootstrap diagnostic passes the predicate but requires 97,137 range and 64,928 shared rows after exact-constant reuse and unsigned lazy reduction. Shared rows fit k16; range rows still exceed it. The earlier smaller three-bus Bootstrap diagnostic required 97,993 / 63,889 rows but does not establish a uniform Load-capable profile. Exact-constant cell reuse and the two-congruence unsigned reduction pass both-field boundary, layout and every-cell mutation tests; full compact differential tests pass after each change. Further bounded product/division admission is under review and is not included in these measurements. No compact Ω proof was produced. Further capacity reduction, complete catalog/root rebinding, actual proof bytes and loaded-host qualification remain open. |
| Signed-object constraints | The 14-case object suite and three additional delivery-evidence cases pass for all 11 exact signed-body schemas against Rust vectors, same-tape message/object digests and Request credit ID; malformed fields/version/key framing; total credential evidence/policy and exact renewal continuity; Send/Receive receipt substitutions; policy/voucher body bounds; Request overflow/pair checks; exact basis-point rounding, clamps and overflow before clamping; exact Load voucher/wallet binding; Send payer ownership and head-held fees (including zero-fee defaults); original 163-byte Payment transcript and transitive package binding, with every byte and nested component substitution rejected; total credit-opening parsing and root/credit/Payment binding reject sentinel, alias, next-key, sibling and burn-flag substitutions. Total incoming statements match hard semantics across all 14 variants and every field, and malformed statements remain false even when the receipt digest is recomputed to match. Dynamic CreditStatus heads and receipts use one fixed layout across all operation tags. Both Credited transcript forms match Rust vectors; CreditStatus and Receive evidence bind the original Request, receiver wallet/key, pinned relation, credit/amount and retained Payment, with renewal preserved and coherent foreign-relation/root/Payment substitutions rejected. These tests hash the original malformed transcripts and reject forged true verdicts; proof/signature decisions remain separate required obligations. | Component tests. Complete per-operation authorization, incoming package bindings and native-versus-circuit integration remain in progress; a standalone parsed object is not authenticated. |
| Indexed maps / state openings | Eight IMT tests pass, covering both fields, full-depth G1 vectors, authenticated empty insertion, removed-slot clearing, full integer key order, immutable first records and every-cell small-circuit tampering. The 25-case state/statement and operation-map suite passes (including Bootstrap, Load, Unload, Retiring and all five refresh state effects) for core33/rest8/statement26, all operation tags, recomputed-hash forgeries, exact control masks, identity/head continuity, OQ-3 and Archive no-op. Production-depth map components fit k16 (Send 31,746, Receive 35,446 Archive 31,524 and Unload recovery 17,094 maximum assigned rows). The obsolete indexed quota upsert/update/dummy-witness API is removed; six native tree tests pass with duplicate/zero rejection and final-slot exhaustion. Shared digest parity passes 17 tests (one long case ignored). Blacklist refresh insertion and Request-recorded history lookup pass, including hard route authentication with a soft pair verdict. The fixed64 quota rebuild passes at 51,652 sponge / 36,008 range / 17,670 glue rows, including full64 matching, first-share, expired/unused drops, exact signed window count in 1..=64, and recomputed-root reset/end/drop attacks; the same component passes on the recursive duplex. Independent source review found no local merge/state-effect gap. Shared hash framing passes both-field tests and all eight IMT tests after generic lane composition. | These constraints are components to be bound to authenticated operation inputs and recursive verdicts. They do not authorize a payment by themselves. |
| Wallet / integration | Shared Rust coordination retains exact replay, durable fold witnesses, Patricia indexes, source-bound checkpoints, CreditStatus and cooperative payment preemption around G2. Current G2/Advance tests pass 190/190 and wallet state/recovery tests pass 25/25. Collection preserves latest folded and pending-gap heads, permanent first-credit metadata, and unpaid fee Payment plus original Request until source-verified finalized payout; restart and interrupted publication/removal tests pass. Latest descriptor-relative backend tests 8/8 and lower-layer custody test passed, as did scoped library lint. The backend retains directory/staging identities through publication. | The real NativeProofs provider remains mandatory and depends on unfinished authenticated artifacts. Opaque C/JNI wallet handles and thin Swift/Kotlin clients cover commit/retry/resume/fold/credit; new collection and payout custody remain Rust-owner methods pending the real artifact/provider loader. Five native boundary tests and 37 Kotlin wallet tests pass; current Swift source typechecking and managed result-contract execution pass. The 172-test artifact inventory suite (233 subtests), 42 header/negative controls and shell packaging controls pass. Rebuilt native SDK execution, real-proof ledger integration, A → B → C → unload and four-validator qualification remain open. |
| Ledger services | Current rebuilt Core passes all 33 G6 tests for reserve accounting, exact-once unload/fee claims, enrollment/policy and finalized global-QC authorization. The daemon publisher passes both load-voucher tests; its test network identity is nonzero and production certificate checks remain intact. All 731 configuration and 24 ZK model tests pass. | These are component results. Real-proof wallet exchange/unload on at least four validators and current release artifacts remain required. |

The current repaired-harness **diagnostic**, binary SHA-256
`58b0368064ce8ab590d4e0a6bafce17e73d3aea9115b23ec41826db92903217b`, produced
two valid PIPA-R proofs in each configuration below on the shared Mac. It
includes shared A round selectors, reused quotient coset buffers and exact
DAG normalization. The worktree was not frozen for these runs; there was no
calibration/repetition qualification. Raw records are retained in
`target/qualification/m3-pilot-20261006-normalized-dag/`; earlier raw attempts
remain under the adjacent `m3-pilot-*` directories. RSS is the kernel lifetime
peak, including key generation, and is not phase-subtracted. Time is the slower
of the two proofs. Q is the 5-variable-key/1-fixed-key signature chip workload;
A is the IMT-like load, not a complete operation relation.

| Real workload | Workers | Judged diagnostic time | Peak RSS bytes | Gate observation |
|---|---:|---:|---:|---|
| Q chips | 1 | 29.9847 s CPU | 750,321,664 | Inside hard limits; lacks 10% time margin |
| Q chips | 4 | 10.1756 s elapsed | 674,955,264 | Exceeds 10 s; RSS below 0.75 GiB |
| A IMT load | 1 | 33.0941 s CPU | 764,788,736 | Inside hard limits; lacks 10% time margin |
| A IMT load | 4 | 10.0960 s elapsed (diagnostic) | 728,449,024 | RSS below 0.85 GiB; no A4 time gate is invented |

Q proofs are 7,936 bytes; the shared-selector A workload proofs are 7,584 bytes.
Those are local chip workload proofs, not transported Omega artifacts.

No synthetic-shape or phone result is inferred from these chip diagnostics.
Streaming/allocation changes or narrower/split workloads must meet the existing
limits before M3 is marked qualified.

### Earlier observations (retired layouts; not current qualification)

Records below follow §2. Source: `optimizations` at `df9c70cdb3` plus the
uncommitted working tree of 2026-10-03; host: Apple-silicon Mac, 20 CPUs, 128 GiB,
shared with other build jobs (timings ±25%). No phone, live network or payment was used.

| Check | Record | Result and limits |
|---|---|---|
| G1 canonical objects and transcripts | observed as expected | `iroha_data_model::kagemusha::kagemusha_wallet_v1` ([wire record](kagemusha_wallet_wire_v1.md)): 129 unit tests pass (`scripts/cargo_fast.sh --stable-local-metadata --incremental --target-slot kagemusha-wallet -- test -p iroha_data_model --lib kagemusha_wallet_v1`), including every-byte-flip, decode-order, bound (limit, limit+1) and Norito-tag tests. Not yet consumed by the proof relation, provider, bridge, node or Torii. |
| Low-S signature rule (§8) | observed as expected | Native codec accepts `s = floor(n/2)` and rejects `floor(n/2)+1`, `n - s`, `r = 0`, `s = 0`, `r = n`, `s = n`; signer DER and raw high-S output is normalized and verified before freezing. In-circuit enforcement used the old halo2 P-256 gadget `assert_p256_ecdsa_digest::<F, 256, true>`, deleted with the old recursion code; the PIPA-v1 relation must re-implement it, and no relation consumes the new objects yet. |
| Cross-language vectors | observed as expected | `fixtures/kagemusha/wallet_v1_vectors.json` (generated by the Rust test; `IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS=1` regenerates). Kotlin `KagemushaWalletVectorsV1Test` 14/14 pass. Swift `KagemushaWalletVectorsV1Tests` 15/15 pass in a scratch package; `swift test` in `IrohaSwift/` is blocked before compilation because the checked-out `NoritoBridge.xcframework` reports bridge ABI 21, not 25. JS, Python and C# consumers: not run (not written). |
| Payment, Credited and Offer size bounds | observed as expected (stand-in proofs) | Size tests assert the worst cases with stand-in proofs of the provisional caps (transition 6,016 B, CreditStatus 2,000 B). Stand-in proofs are structural only and are not completed payments. |
| In-circuit P-256 cost | measured | One full-width low-S verification: 1,466,624 advice cells = 23 advice + 4 lookup columns at k=16 in each Pasta field. A k=16 proof of that circuit alone: 10,112 B per parity; prove 12.5–13.0 s (20 threads), 20.7–24.2 s (4), 31.9–32.8 s (1); verify 0.26 s; peak RSS 0.79–0.95 GiB. |
| IPA proof scaling | measured | Test-only harness `crates/iroha_core_zk/src/g3_proof_scaling_measurement_tests.rs` (ignored tests, release; deleted with the old recursion code and recoverable from Git history). k=16 proof bytes = 1,976 + 358·(advice columns); prove ≈ 7.2 s + 0.54 s per column at 20 threads; peak RSS ≈ 257 MiB + 27 MiB per column; proving key ≈ 25.5 MB + 9.1 MB per column. Narrow no-lookup proofs: bytes = 768 + 352·W + 64·k (k=18–20, W=1–3: 2,272–3,104 B), prove 21–80 s, verify 1.0–3.2 s. One thread is only 2.5–3.3× slower than 20. |
| Recursive-verifier building blocks | measured | ≈ 2,170 advice cells per MSM source (`reciprocal_compact_batch_allocation_diagnostic_in_both_parities`, a deleted old-recursion diagnostic: 1,008 sources = 34 advice + 6 lookup columns at k=16); dense rows for 1,008 sources: 131,046 rows in 9.3 s. |
| R9 and 2 s p95 with the current construction | deviation (estimate from the measured models) | A Send transition needs ≥ 6 in-circuit P-256 verifications plus transcripts, maps and recursion (≈ 150–230 columns): ≈ 55–85 KB per parity at k=16, so Payment ≤ 10,000 B needs a narrow large-k outer layer, whose proving alone is ≈ 21–80 s per parity on this host. End-to-end 2 s p95 is not reachable on this path; proposal revision 2026-10-04 moves recursion off the payment path (split lineage). |
| Android Keystore absence semantics | source reading | AOSP `AndroidKeyStoreSpi` (android14-release): `containsAlias` returns false on any Keystore error, `aliases`/`size`/`getCertificate*` swallow errors, only `getKey` distinguishes `KEY_NOT_FOUND`; keystore2 `rebind_alias` replaces an existing alias on generation. Device confirmation: not run. |
| Android lock-screen removal | source reading | keystore2 android12–14 `reset_user(.., keep_non_super_encrypted_keys=true)` deletes certificate-only and super-encrypted entries and keeps plain non-auth keys; Android 15 depends on build flag `fix_unlocked_device_required_keys_v2`. Device confirmation: not run. |
| Android backup/restore | source reading | `BackupAgent` never backs up `getNoBackupFilesDir()`, but a full-data restore of an app without its own agent first clears all app data and its Keystore namespace (`FullRestoreEngine` → `clearApplicationUserDataLIF`, android14). The wallet's backup set must therefore be empty. Device confirmation: not run. |
| iPhone keychain passcode class | source reading | Apple Platform Security: `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly` items are never backed up, synced or escrowed and become unusable when the passcode is removed or reset. Restart and power-loss durability after a write: not run. |
