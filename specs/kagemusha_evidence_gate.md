# KAGEMUSHA verification checklist

Status: working checklist, 2026-10-03. This document records useful checks for
[the single protocol](kagemusha_single_design_proposal.md). It is not an
approval process and does not block use, production integration or deployment.
The filename is retained for existing links. No check below has been run for
the consolidated design.

Runtime signature, proof, authorization, replay and durability checks remain
part of the protocol. Removing a release gate does not make an invalid
payment valid or make an unimplemented security property an established fact.

## 1. What the evidence should distinguish

A state package contains its transition proof `π` and provider receipt `τ`.
The proof verifies the predecessor package and any incoming packages. After
proving, the provider executes atomic, durable
`Advance(expected_head, new_head, hash(π), operation_id, recovery_capsule)` and produces
`τ`. The current receipt is checked natively; a successor proof verifies the
predecessor receipt recursively. Tests should follow that order and bind the
receipt to the exact proof and state transition.

A committed Send irreversibly debits `amount + fee`. Recovery delivers the
same exact Payment bytes to the bound receiver, which credits them once.
Timeout, interruption, rejection, missing delivery evidence or sender regret
never restores the debit. The proof relation has no reversal operation.

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
| P1 | Accepted value is durable and immediately spendable onward offline. |
| P2 | The payer cannot spend it again under the provider's stated security assumptions. |
| P3 | Completion needs no later reconciliation, approval or settlement. |
| P4 | Later discovery of payer misconduct does not revoke an honest recipient's accepted value. |
| P5 | Only an explicitly enabled regulatory control requires connectivity. |
| PC | Processing needed for these properties finishes before the relevant endpoint reports completion. |

## 2. Records and independently runnable work

Use a record per check: **not run**, **observed as expected**, **deviation**,
**source reading**, or **not accessible**. Include the steps, expected result,
actual result and limits of the conclusion. None of these labels grants or
withholds permission to use or integrate the implementation.

Identify the protocol revision, source commit, dependency lock, build hashes,
proof relation, provider and firmware, attestation policy, device model, OS
build, security patch and carrier. Keep the raw signed bytes, proofs, key
hashes, logs, timings and fault traces alongside the reproducible procedure.
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

Check the written authority contract and the implemented transition relation
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
  lost delivery evidence leaves that debit unchanged. The proof relation has no
  operation that restores it on those grounds; reject retired refund/refusal
  monetary objects and attempts to recredit the sender through other operations.
- Receive proves nonmembership of the exact `credit_id` in
  `consumed_credit_root`, credits once and inserts the ID with its complete
  Payment digest in the same committed transition. Entries are permanent.
  Deliver a delayed Payment after unrelated receives and then replay it
  repeatedly, including after restart and delivery
  evidence cleanup: only the first valid Receive changes the balance.
- Each voucher and redemption is consumed at most once. Funding and retirement
  cannot recreate a previously consumed voucher or reverse a committed Send.
- A nonzero fee is earned at Send. Its exact retained Payment authorizes one
  fee payout, keyed by `credit_id`, only to its fixed beneficiary. Claim the fee
  before Receive and retry after Receive: there is one payout total. Failed
  delivery or decline neither cancels the fee entitlement nor restores its
  debit. Map leaves and immutable capsule bodies do not contain the very
  receipt or final package that authenticates them.
- Optional Credited evidence is the committed Receive package or a read-only
  `CreditStatus` proof of membership from the receiver's current state. Verify
  the receiver's complete current package, its proof and provider receipt, and
  the membership binding to the exact Payment digest, recipient and scope.
  Another Payment, an unconsumed credit or an invalid current receipt fails.
  `ArchiveSent` verifies this evidence inside its transition relation and removes
  only the corresponding retained outbox entry, without a monetary effect.
  Evidence loss cannot change either balance or remove a permanent
  consumed-credit entry. The flow creates no acknowledgement chain,
  receiver pruning obligation or prerequisite for onward spending.
- `Advance` retries recover the same committed outcome. Once a Payment is
  assembled and released, every retry returns its exact original bytes,
  including proof and receipt. Crash before release must finish the selected
  Send, retain its complete Payment and deliver that Payment; it cannot select
  another successor, regenerate a different released Payment or restore value.
  Loss of every retained copy of released bytes reports delivery-data loss;
  generating a new receipt or proof is not recovery.
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
  wallet upgrades. Exercise the unchanged relation and verification interface
  within the scheme without an unconfigured requirement to reconnect.
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
proof relation together, identifying which component discharges each
obligation instead of having each assume the other enforces it.

## 5. Real proofs and protocol interoperability

Generate real zero-state, load and recursive transition proofs for a complete
load → A → B → C → unload lineage. Include split/change, delayed and duplicate
delivery, receive-then-send, phone replacement and cyclic transfers. Retain
witness fixtures, proofs, verification keys and component resource measurements.

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

Compare native and recursive verification of the same package. Check that
current receipts are verified before credit and that successor proofs bind
those receipts to the exact predecessor and incoming packages. Check that
an invalid package cannot become valid merely by wrapping it in another proof.

The scheme keeps its fixed proof relation and verification interface for its
lifetime. Test voluntary transfers to a different scheme separately; breaking
changes cannot require old wallets to update or reconnect to keep using their
existing value.

## 6. Complete device exchanges

Run real proofs and the software provider on stock phones for each claimed
device and ordered sender/receiver/carrier combination. Label runs with
padded proofs so their timings are not attributed to a real prover. Record
untested combinations without claiming platform or vendor universality.

- A pays B; immediately after B reports complete, B pays all received value
  to C with radios off. Repeat after restart, temporary storage unavailability
  and recoverable faults. Record total private-recovery-byte erasure as
  custody loss, not successful recovery from a digest. Inspect both endpoints:
  the payer reports its durable Send independently and reports delivery confirmed
  only from valid receiver completion evidence.
- After A commits Send, interrupt delivery before B receives it; also exercise
  a decline, capacity failure and a lost Credited message. A's amount and fee
  never return. B can receive other payments before the original exact Payment
  is retried, and can then credit that Payment once. Withhold optional Credited
  evidence and confirm that B's accepted value stays onward-spendable. Recover
  that evidence later through `CreditStatus` without changing B's state.
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
- Measure every package-carrying message against the **10,000-byte** bound,
  and each Offer against **2,048 bytes**. Measure the **2-second p95** target
  from payer confirmation after the Request to the receiver's durable,
  onward-ready completion. Include intervening framing, retries, carrier
  setup, both proofs and provider durability. Measure the payer's later
  delivery confirmation separately. State sample counts and the percentile
  estimator before measurement; retain raw samples, failures, cold/warm
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

Re-run affected checks after changes to provider, firmware, OS, proof relation
or storage behavior. Record what results still apply and how existing offline
packages interoperate. Use the proposal's retirement map and actual caller
inventory when consolidating implementations; verification work does not
create a separate approval condition for integration or production use.

Starting points, not evidence that this protocol has passed:

- Android: `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt`; `AndroidKeyMintOneUseSelectionCandidateV1.kt` and `KeyMintRestartDiagnosticV1.kt` in the same directory. Generate and inspect keys even when feature flags are negative.
- Apple: `examples/ios/KagemushaAppAttestProbe/`.
- Attestation: `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/attestation/` and `python/iroha_app_attestation/src/iroha_app_attestation/`.
- Formal work: `formal/kagemusha_v1/`; check its assumed hardware contract before reusing results.
- Evidence records: `specs/kagemusha_v1_physical_evidence.md` and `scripts/verify_kagemusha_v1_physical_device.py`; adapt their provider assumptions explicitly.
- [Pinned AOSP characteristics](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/common/src/tag.rs), [operations](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/ta/src/operation.rs) and [upgrades](https://android.googlesource.com/platform/system/keymint/+/fda4e68d32f8dfc103e0283b4bfc41503ecfb19f/ta/src/keys.rs): reference behavior, not vendor-firmware evidence.
- [Android attestation](https://developer.android.com/privacy-and-security/security-key-attestation), [Apple validation](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server) and [Apple fraud-risk guidance](https://developer.apple.com/documentation/devicecheck/assessing-fraud-risk): capture the exact statements and their limits in each evidence record.
