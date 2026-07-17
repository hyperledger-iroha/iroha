# Sumeragi v2 formal verification

This directory is the first-release formal corridor for the production
Sumeragi v2 consensus protocol. There is no legacy Sumeragi proof corridor.
The model fixes protocol revision 3 and is parameterized over arbitrary finite
frozen rosters; production separately enforces the release limit of 128
validators. Mechanization status is recorded per obligation in the proof
ledger.

## Modules

- `SumeragiV2Quorums.tla` and `SumeragiV2QuorumProofs.tla` define and prove
  strict count-and-power quorum intersection.
- `SumeragiV2Availability.tla`, `SumeragiV2CrashRecovery.tla`, and
  `SumeragiV2Reconfiguration.tla` define durable-body, WAL, restart, and frozen
  height-context boundaries. `SumeragiV2VocabularyProofs.tla` checks their
  small helper facts separately, keeping the executable vocabulary theorem-free
  for exact parameterized instancing in the indexed chain refinement.
- `SumeragiV2Core.tla` models addressed asynchronous delivery, durable intents,
  locks and highest PrepareQCs, grouped timeout certificates, future-view
  recovery, old-view CommitQCs, body recovery, decisions, and application.
- `SumeragiV2TimeoutDurability.tla`,
  `SumeragiV2TimeoutSigningInvariant.tla`,
  `SumeragiV2TimeoutViewInvariant.tla`, and
  `SumeragiV2TimeoutWireAuthorization.tla` prove the timeout-envelope frontier
  from WAL ownership through signing and honest transport. Every honest timeout
  is bound to the frozen voter roster, context and height, carries an
  authenticated PrepareQC high reference, and satisfies `highRank <= view`.
  These envelope modules do not by themselves discharge the historical
  TC-lock authorization or the dependent direct-or-installed-authorization
  timeout wrapper; the full action induction below discharges both.
- `SumeragiV2SafetyLemmas.tla`, `SumeragiV2AgreementLemmas.tla`,
  `SumeragiV2Inductive.tla`, `SumeragiV2InductiveProofs.tla`, and
  `SumeragiV2Proofs.tla` contain the action-by-action safety induction and its
  end-to-end theorems.
- `SumeragiV2ChainEpoch.tla`, `SumeragiV2ChainEpochProofs.tla`, and
  `SumeragiV2ChainEpochRefinement.tla` model prefix-comparable per-validator
  histories and frozen epoch routing from exact durable CommitQC decisions and
  exact local application receipts. The refinement contains both the selected-
  height safety product and an indexed family of dormant, admissible
  `AsyncSpecAt` instances. Exact application receipts join successors one node
  at a time through an ordered activation pipeline. The node joins only when an
  Applied or exact durable-tip Recovered path publishes a full-context token.
  Validators absent from an old roster use the production-shaped authenticated
  historical CommitQC/body service and explicit decision, body-recovery, store,
  validation, and application stages. Terminal observers record known
  application without creating a successor or join. Certification and local
  application do not use a global all-node barrier. The production trace
  mapping and temporal multi-height induction remain explicit proof debt.
- `SumeragiV2EffectiveLockAcquisition.tla` is an executable, height-scoped
  locked-body owner. It keeps a physical load ID and subject immutable across
  same-lock consumer rebinds, defers a higher different-subject replacement
  until the active load terminates, classifies stale/future/wrong/duplicate
  completions, and retries only after the exact certified body becomes durable.
  `SumeragiV2EffectiveLockAcquisitionProofs.tla` owns the separate deductive
  model obligation; the ordinary Rust worker/runtime refinement is a distinct
  asynchronous proof obligation.
- `SumeragiV2AsyncNetwork.tla`,
  `SumeragiV2AsyncFairnessRefinementProofs.tla`,
  `SumeragiV2LivenessProofs.tla`, and `SumeragiV2AsyncLivenessProofs.tla`
  model the production scheduler and transport abstractions, own the typed
  fair-action-to-`AsyncNext` refinement proof, and state the conditional
  progress obligations after GST.
  The volatile vote pool is the delivery-epoch witness: an exact vote crosses
  once while present, and TC installation clears the local pool. If the
  resulting lock has the node's exact durable Commit intent, acknowledgement
  queues that intent for re-signing; signature completion reconstructs the
  local pool before sending only to the other voters. Peer retransmission of
  that exact locked Commit enters reserved progress admission in the new
  reducer generation. Every Commit delivery, including a current-view one,
  additionally requires the recipient's exact durable lock. A pre-lock current
  Commit is a recoverable ignore; after `LockAndCommit` acknowledgement applies
  the lock, the adapter advances its locked-Commit consumer epoch and may admit
  that exact vote once. The acknowledgement prunes the superseded historical
  pool before releasing the current Commit signature. Retained outbound control
  remains peer-delivery evidence, not a sufficient local witness because
  broadcast excludes the sender. The protected deferred lane has one slot per
  locked-Commit signer, one slot per TimeoutVote signer, and one class-wide
  PrepareQC, CommitQC, and TC slot each: exactly
  `2 * |ValidatorIds| + 3` owners. Exact duplicates coalesce, while a distinct
  same-owner item retries without displacing another protected slot. Immutable
  authenticated history remains separate from this consumer state. Fair
  transport ingress requires at least `3 * |ValidatorIds| + 1` entries. The
  potential separately reserves an empty or non-timeout-progress-deficient
  validator source, every validator's missing TimeoutVote, and the continuation
  required when servicing a lane would recreate either reservation; anonymous
  and non-roster senders share the final untrusted slot. The
  inductive invariant also records that every individual source lane is at
  most the aggregate ingress capacity, matching the runtime admission gate;
  this makes one-item removal decrease the counted depth by exactly one even
  when preservation is checked from an arbitrary invariant state. Each
  authenticated validator source also leaves a distinct configured 64 KiB
  timeout-vote reserve unavailable to ordinary traffic (in addition to body
  envelope headroom). That isolated region exceeds the conservative 4 KiB
  maximum valid timeout-vote envelope, including a 128-signer PrepareQC. A
  validator lane owns at most one distinct queued TimeoutVote in that region;
  transport copies coalesce while that lane owns the exact envelope, and the
  authenticated delivery record continues coalescing while the corresponding
  candidate is deferred, causal, queued, or completion-owned. A new occurrence
  starts only after scheduler ownership ends. Timeout delivery additionally
  requires the complete canonical envelope domain before inspecting its
  authorization fields; the augmented-record counterexample and typed async
  adapter refinement are TLAPS-proved. Thus auxiliary byte saturation or a
  field-compatible non-canonical record cannot invalidate the message-level
  timeout admission boundary.
  The production reducer's shared Rust/Verus refinement gate checks the local
  EnterView selection boundary: the persisted TC, pre-install lock,
  post-install durable lock, effect-carried lock, and immediately following
  recovery fetch must agree on the effective maximum lock. The executable
  acquisition owner makes the body-rebind and recovery state machine explicit,
  but its deductive `EffectiveLockAcquisitionModelObligation` remains
  `specified_unproved`. Proving that every production executor, runtime,
  worker, request, byte, and queue owner refines and fairly services that model
  remains the separate
  `EffectiveLockBodyAcquisitionProductionRefinementObligation`, also
  `specified_unproved`.
  Generation-scoped vote delivery is ledgered `tlaps_proved`. The one-height
  `AsyncSpecAt` type-closure wrapper has a checked source proof body but remains
  ledgered `specified_unproved` because it consumes
  `AsyncRunnerStepPreservesSchedulerType`, which has not passed a fresh pinned
  strict proof on the current source. Deadlock freedom now requires an enabled
  productive step that grows height evidence, consumes concrete deadline debt,
  or decreases/exits a protected candidate or Serve-occurrence rank. The weaker
  scheduler-enabled lemma cannot discharge it, so the productive obligation
  remains explicit proof debt. `StarvationFreedomObligation` likewise
  has a source proof body but cannot be promoted ahead of its still-unproved
  service-rank prerequisite. The durable progress
  witness and the remaining stable-suffix liveness declarations are likewise
  explicit debt, so this is not a machine-checked completion claim.
  The Core vote-delivery relation and normalized trace replay encode the exact
  durable-lock Commit gate and post-WAL pool pruning. The repaired full strict
  induction discharged all 7,826 obligations, and the downstream Core safety
  wrapper discharged all 565 obligations. That closes only the historical
  TC-lock and timeout-protection ledger entries; the asynchronous liveness
  proof-premise repairs remain outstanding.
  Logical views are unbounded in the deductive liveness abstraction; finite
  TLC configurations remain counterexample searches only.
- `proof_coverage.json` is the checked-in theorem/trust-boundary status
  declaration, not independent proof authority. The structural checker binds
  every status to the exact source theorem, and release status additionally
  requires fresh generated evidence under `target/formal/sumeragi_v2/`.
  Backend obligation counts belong only in that generated evidence.
- `SumeragiV2ResumeVoteWitness.tla` and
  `resume_locked_commit_witness.cfg` ask TLC to produce the bounded
  lock-and-Commit recovery trace: a timed-out view-0 Commit remains the exact
  active lock, a TC advances the node to view 1, a crash/restart contributes the
  second generation increment, and `ResumeVote` reconstructs the historical
  signing request. The deliberately violated negated predicate is a regression
  witness, not a release invariant or deductive proof.

## Exact protocol abstractions

`ContextRecord` binds the chain and protocol identities, semantic parent
finality, height, epoch, canonical roster and powers, lane/DA commitments, and
the already-computed production leader start. Certificates must satisfy both
`3 * signer_count > 2 * voter_count` and
`3 * signer_power > 2 * total_power`; observers never pad either threshold.

Honest Proposal, Prepare, Commit, and Timeout signatures require their matching
acknowledged WAL intent. A timeout vote carries the full highest durable
PrepareQC, and a TC contains disjoint signer groups whose union independently
satisfies both quorum thresholds. Installing a TC may move one validator to
`tc.view + 1`; it does not require other validators to install first. Prepare
intent replay remains current-view and timeout-fenced. An already-durable
Commit intent may resume signing after a timeout or later TC only when it still
matches the validator's exact active lock round and subject; unrelated
historical Commit intents remain fenced. TC acknowledgement performs the same
exact-lock check before queuing a Commit re-sign. If the TC instead promotes an
exact lock for which this node lacks Commit intent, installation does not sign.
After exact body storage and current-generation validation, the normal
`BeginLockCommit`/WAL path may create that historical intent only when no
higher conflicting-subject local Prepare intent or known PrepareQC exists;
higher same-subject reproposals do not block reconstruction. The local
signature completion inserts the vote directly into the new volatile pool
because the P2P broadcast excludes its sender. An old-view CommitQC remains
decisive after a view change. All received Commit votes require the exact active
durable lock. A premature current-view Commit stutters recoverably; once the
matching `LockAndCommit` acknowledgement applies the newer lock, it prunes the
superseded historical Commit pool before signing and advances the adapter's
consumer epoch so that exact vote may enter once.

A certified chain slot is created only from the exact valid CommitQC in a
durable decision receipt for the canonical parent context. Each validator
fetches, reconstructs, validates, durably applies, and advances independently
from its own exact receipt. Lagging validators remain on their certified
prefix and cannot be advanced by another node's receipt.

The asynchronous model includes pre-GST loss, duplication, reordering, crash,
and replay. After GST it models bounded authenticated per-source transport,
normal/progress/completion ingress reserves, view-indexed absolute timeout
priority, periodic retransmission, command-service debt, cyclic class-aware
dispatch with FIFO order inside each class, stale-completion rejection,
manifest and chunk recovery, validation, and independent application. The
scheduler choice matches the source-linked production kernel:

1. the current view's absolute timeout;
2. an owed class-aware command;
3. one periodic retransmission;
4. the first command selected by the cyclic class cursor; or
5. idle service.

Textual TLA+ disjunction order is never treated as priority; the selected-work
operator makes the branches mutually exclusive.

Local admission has a separate two-source cursor for producer completions and
causal work. If a producer turn is taken while causal work is waiting, the
model records sticky causal-admission debt and advances the cursor. The debt is
cleared only by selecting the causal source; once its head is admissible, debt
makes that source the deterministic preference under the existing fair
`RunNode` action. Thus a continuously replenished producer cannot erase the
causal source's turn.

Body transport is typed syntactically over all `Subjects`, not only
`ValidSubjects`. This keeps authenticated-but-invalid reconstructed bodies in
the adversarial state space. Deterministic validation is the semantic boundary:
a valid proposal body records current-view/current-generation validation,
whereas an invalid proposal body follows `RejectBody` and cannot authorize a
Prepare vote. An invalid body for an already durable decision remains a
fail-closed error, matching the production progress-witness check.

Every available, durable, validated, and invalid body record binds its exact
view. The only view-independent body state is `retainedLockedBodies`, created
when a Commit lock is persisted and accepted only by `RebindRetainedBody`.
Rebinding stages an exact target-view available record; target-view storage and
a generation-bound validation marker must then complete. When the exact
canonical bytes already have a durable validation witness in an earlier view
of the same height context, production promotes its deterministic execution
commitment into that new marker instead of executing the body again. Retained
authority or an old marker alone cannot authorize voting or application, and
decision application checks durable and validated evidence at the CommitQC's
exact view.

The production authority boundary is stricter than signature validity alone.
An individual Vote may consume an execution commitment only after the exact
round and subject are bound by a local validated receipt, verified WAL replay,
or quorum-authenticated QC evidence; a signed Vote cannot create that binding.
Unbound Votes remain recoverable input rather than local fail-stop evidence.
Body-availability rebind requires the installed destination tag, preflights the
source and destination ownership sets, and either moves one exact source or
coalesces it into one exact destination. An uninstalled destination is a
recoverable caller-contract rejection with no mutation. Conflicting or
duplicate ownership fails closed, and every pipeline or Decision retirement
invariant is checked transactionally before mutation. Conflicting certificate
evidence received through body-recovery request/response transport is rejected
nonfatally and leaves retry ownership available.

The timeout service budget charges at most three class-aware dispatches for
each of the `AsyncQueueCapacity` same-class positions of a protected admitted
occurrence.  Each dispatch is separated by the conservative runner-cycle bound
`AsyncQueueCapacity + 2 * AsyncIngressCapacity + 3`; the extra ingress term is
the sound over-approximation implied by the scalar runner-budget type rather
than an assumption that every reachable phase starts at its tight reset value.

Shared-config projection version 2 binds this pacemaker rule into the handshake
fingerprint. A retired fixed-timeout binary therefore cannot silently
participate in the same height and supply premature timeout votes against the
view-growing liveness argument.

## Theorem scope and FLP boundary

Safety is asynchronous: it permits arbitrary delay, loss, duplication,
reordering, Byzantine messages within authenticated identities, and crashes at
effect boundaries. The safety argument and release obligations cover durable
sign-once behavior, external validity, certified-body availability, lock
protection, agreement, absence of conflicting CommitQCs, crash/restart
preservation, chain-prefix safety, epoch-context isolation, and both the
grouped-timeout kernel and the corrected timeout disjunction: a formed TC
either directly protects a potential Commit quorum or exposes an honest
timeout/Commit intersection witness with durable installed-TC authorization.
Their exact mechanization status is recorded per obligation in
`proof_coverage.json`;
inclusion in this scope is not itself a claim that the obligation is proved.
Crash/restart preservation is a behavior-scoped temporal contract: under the
selected core specification, every crash or restart preserves the durable
projection, interrupted writes stay unacknowledged, and stale generations are
rejected.

Ten arbitrary-context Core safety wrappers are TLAPS-proved over
`CoreSpecAt(initialContext)`: durable-vote uniqueness, lock monotonicity,
external validity, certified-body availability, certificate uniqueness,
agreement, conflicting-CommitQC exclusion, crash recovery, historical TC-lock
Commit authorization, and the dependent direct-or-installed-authorization
timeout wrapper. The narrower grouped-timeout kernel for Commit intents already
present at timeout remains proved as well. The authoritative
receipt-driven chain model also TLAPS-proves chain-prefix comparability and
epoch-boundary isolation; those two obligations are not redirected to the
one-height Core wrapper.

Liveness is necessarily conditional. FLP rules out unconditional deterministic
consensus termination in a fully asynchronous network. The post-GST paper
argument therefore has explicit premises: a non-crashing honest set
independently meets both quorum thresholds; authenticated retransmissions and
serialized service have declared finite representable bounds; the monotonic
clock and run loop continue; and admitted fsync, signature, reconstruction,
deterministic validation, and local application work terminate. The immutable view-zero
deadline grows linearly as `base * (view + 1)`, while retransmission retains its
fixed base interval. Consequently some post-GST view exceeds the complete
bounded service rank without assuming in advance that one configured fixed
deadline is already adequate. Under those premises, failed views form and
install TCs. An undecided execution either decides early or reaches a view in
which the responsive honest scheduled leader itself is active, and that leader
state leads to responsive decisions. Each responsive validator's durable
decision independently leads to certified-body recovery, validation, and
application, after which its local chain advances.

The scheduler-owned protected rank includes Completion and Progress work plus
a canonical constructor-shaped Normal proposal/Prepare slice: initial or
post-TC `AssembleBody`, causal `BeginPrepare`, and the frozen Normal delivery
shape for Proposal, PrepareVote, and CommitVote items. Reachable delivery
ownership originates at authenticated ingress, but the admitted class remains
protected after view movement even if dynamic classification would now call the
same CommitVote historical Progress. Authenticated TimeoutVote/DeliverTimeout
uses its own signer-keyed protected Progress slot. Each accepted certified-body
or Commit-certificate recovery request receives a fresh live Serve nonce whose
FIFO position is its occurrence-level rank, so equal request values remain
distinct. This intentionally over-approximates reachable constructor families
without promising service to authenticated junk. The composite rank and
starvation obligations remain `specified_unproved`.

All 18 weak-fairness targets are complete transition actions, not inner
scheduler fragments. Four exact outer-frame categories bind the Core height
context, crash/recovery state, and runner service deadline according to the
variables each action may change. `AsyncFairActionAt` inventories the same
quantified actions as `AsyncFairnessAt`. `AsyncFairActionsRefineAsyncNext` is
the typed source claim; the dedicated
`SumeragiV2AsyncFairnessRefinementProofs!AsyncFairActionsRefineAsyncNextObligation`
theorem proves that every member is one canonical `AsyncNext` transition at a
Core-plus-scheduler typed state. Its fully decomposed strict run is green at
1,143/1,143 obligations. The Core transition relation is deliberately not
conjoined inside each `WF` target: doing so makes TLC re-search unrelated Core
branches while evaluating `ENABLED`. The structural checker pins the four
frames, all 18 action classifications, both quantifier inventories, the typed
claim, and the exact dedicated theorem inventory. The finite TLC specs and
deductive specs share the same `AsyncAllVars` and `AsyncFairnessAt`; no
TLC-only fairness relation exists. This promotes only the fair-action
refinement entry: the 46-entry ledger still has 15 `specified_unproved`
obligations and keeps `machine_checked_completion: false`.

The target statement is exactly: after GST, with a responsive dual quorum and
terminating local work, every height eventually decides and every responsive
validator eventually applies it. It makes no termination claim during an
unbounded partition, without a responsive dual quorum, or while admitted disk,
signing, reconstruction, validation, or application work does not terminate.

This is the conditional consensus-height progress target, not a completed
machine-checked liveness theorem and not a transaction-fairness claim. A valid
empty heartbeat can satisfy progress. Transaction inclusion, mempool fairness,
and censorship resistance are explicitly out of scope in the proof ledger. It
must remain described as a paper argument while
`machine_checked_completion` is false.

The mechanization boundary is narrower than the argument above. The universal
historical TC-lock authorization and its dependent direct-or-installed-
authorization timeout wrapper are now ledgered `tlaps_proved`, together with
the narrower grouped-timeout kernel for Commit intents already present when
timeout votes were made. The universal
`AsyncTypeInvariantObligation` has a checked source proof body but remains
ledgered `specified_unproved` because its concrete runner-preservation
prerequisite remains `specified_unproved` without a fresh pinned strict proof;
the timeout-view,
rotating-leader, and application liveness declarations remain
`specified_unproved` as well. The rotating-leader declaration is a two-stage
claim: reach a view where the responsive honest scheduled leader itself is
active (or decide first), then decide from that leader state. The application
declaration contains an independent post-GST decision-to-application leadsto
for each responsive validator, plus the aggregate clause used by height
composition. The concrete genesis chain product separately
records its first-successor handoff, when a successor height exists, as
`GenesisHeightSuccessorHandoffObligation`. That theorem also has a source proof
body but remains `specified_unproved` until the strict proof succeeds after
rotating-leader, application liveness, and the explicit
`SuccessorActivationStarvationFreedomObligation` and
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`
are proved. At the terminal finite
horizon the handoff is explicitly vacuous rather than manufacturing a
successor instance. The activation obligation uses the minimal scalar carrier
`0..19`: the pre-failure path adds 9 to the remaining pipeline distance, so a
fail-closed reset at any stage still strictly descends before authenticated
recovery resumes the one-shot suffix. Its rank, enabledness, fairness, and
starvation clauses range over responsive validators only. An honest validator
outside `Responsive` may retain work queued before GST without violating the
conditional production target; the model does not manufacture local-worker
fairness for that validator.
The chain refinement now models an indexed family of authoritative
`AsyncSpecAt` instances and exposes the exact
`SumeragiV2ChainEpochRefinement!HeightLivenessObligation`. Its source proof body
now composes responsive-validator instance activation and exact-action fairness
on the all-joined
suffix, authenticated exact recovery for responsive validators absent from
an old roster, and finite-height temporal induction. The whole theorem remains
explicit `specified_unproved` debt until a fresh pinned strict proof succeeds
after rotating-leader, application liveness, successor-activation starvation,
and that production-refinement seam are proved. Its exact completion
predicate requires application
at terminal `MaxHeight`; at every nonterminal context it requires each
responsive validator to advance into a successor context. Exact recovery opens
one per-instance scheduler target and imports only an authenticated canonical
CommitQC into the ordinary reducer. It then follows the same decision,
body-fetch, store, validation, and application transitions as live consensus;
it cannot create finality or skip a consumer boundary. Nonterminal
application queues the ordinary activation pipeline without joining, while
terminal application uses `RecordKnownApplication` and creates no successor
work. Dormant non-genesis instances
retain their exact `InitAt` parent receipt internally, but only current-context
receipts enter the global ChainEpoch projection, so the indexed genesis is
non-vacuous. The first-release model does not restore a
favourable-network relation, global asynchronous shadow state, or a second
consensus transition relation to stand in for that proof.

The ledger also names the previously implicit intermediate obligations.
Generation-scoped delivery is ledgered `tlaps_proved`. The executable
height-scoped owner now gives the body-rebind/recovery kernel its own
`EffectiveLockAcquisitionModelObligation`; bounded TLC explores it but does not
prove it. The end-to-end executor/runtime/worker/request/byte/queue mapping and
fair-service composition remains separately ledgered as
`EffectiveLockBodyAcquisitionProductionRefinementObligation`. Both entries are
`specified_unproved`. Historical TC-lock Commit authorization and the
direct-or-installed-authorization timeout induction are `tlaps_proved` from
the full action induction. Post-GST deadlock freedom excludes a bare clock,
runner, or view-change step: it requires current-height evidence growth,
concrete deadline-debt decrease, or protected-rank decrease/exit. Its remaining
stage, packet, and zero-deadline cases are still `specified_unproved`. Durable
progress-witness preservation, strict protected-service-rank decrease, and
starvation freedom intentionally remain `specified_unproved`; adding the
vocabulary is not machine proof.

## Evidence and release gate

The operator-facing conditional guarantee, liveness snapshot, watchdog
classifications, and executable PR/release commands are documented in
[`../../source/sumeragi_v2_liveness.md`](../../source/sumeragi_v2_liveness.md).

The release gate uses TLAPM commit
`763bf3c1826d77a4cf206f43d5aa16775da1da33`, the immutable TLA2Tools 1.7.4
release, and Verus `0.2026.05.31.5dd6d83`. The rolling TLA2Tools 1.8.0
pre-release is deliberately not used because upstream replaces its release
asset with master builds, which makes a fixed archive checksum unreproducible.
The TLC and replay entry points also verify the exact SHA-256 digests of the
`Functions.tla` and `Folds.tla` files from that pinned TLAPM commit before
adding them to `TLA-Library`; an inherited alternate standard library cannot
change bounded-search semantics.
The strict TLAPS runner checks every deductive module,
then generates evidence bound to the exact ordered module list, every proof
log, the pinned tool identity, and a SHA-256 manifest of every TLA+ source.
The checked-in ledger cannot contain stale tool-run counts.

The structural checker rejects top-level TLA+ assumptions/axioms, unledgered
omitted proofs, Verus assume/admit/trusted-body escapes, non-theorem ledger
targets, retired Sumeragi paths, and the former favourable-network liveness
corridor. A proofless release theorem is accepted only at its exact pinned
module and symbol while the ledger records it as `specified_unproved`.
Every validation mode rejects `machine_checked_completion=true` while any such
entry remains. Promotion order is also explicit: async type closure depends on
proved runner scheduler preservation, deadlock freedom depends on proved type
closure, starvation freedom depends on the proved service-rank theorem, and
genesis handoff and indexed height liveness each depend on proved
rotating-leader, application liveness, successor-activation starvation, and the
exact-recovery production refinement.
Release mode additionally requires fresh source-bound evidence.

Before network startup, the executable wrapper inventories 168 named tests
across 14 Rust modules. The inventory includes five native-AMX lane-work
capacity regressions, adapter/runner/watchdog successor-activation boundaries,
exact recovery-derived successor identity, authenticated exact historical
recovery, and the exact
`3N+1`/`2N+3` admission boundaries in addition to exact-lock,
completion-ownership, future-acquisition rejection, rebound durable retry, and
executor-batch boundaries. It also runs exact mocked contracts for active Git
operation rejection, detached source sealing, the 128-run matrix launcher, the
source-bound 100,000-height chaos receipt, provisional Taira evidence
promotion, and the aggregate release receipt. These execution contracts are
not deductive proof. Strict proof completion, the complete PR corridor, the
source-bound chaos run, and the 24-hour Taira-profile soak remain pending.

Production release execution accepts only a clean committed HEAD, reproduces
it in a detached read-only worktree, and records both the original checkout
manifest and the permission-aware sealed manifest. Manifest modes cover
enumerated file/symlink entries; a separate seal walk checks directories and
rejects source symlink escapes, writable-output targets, and hard-linked regular
files. Child builds and evidence bind the sealed manifest actually compiled;
the aggregate receipt additionally binds original HEAD/tree/`Cargo.lock`, all
29 pre-network legs and the exact 168-test inventory, the pinned harness lock
and resolved toolchain, the formal ledger/evidence/log, all matrix logs, chaos
log, and exact-identity soak evidence. The chmod
seal is a cooperative ordinary-write guard rather than a same-UID security
boundary. The complete operator contract is documented in
[`../../source/sumeragi_v2_liveness.md`](../../source/sumeragi_v2_liveness.md).

The release transcript contract provides cooperative self-consistency only.
It rejects malformed, incomplete, cross-source, semantically mismatched, or
digest-mismatched evidence observed during receipt generation; it is not a
cryptographic attestation of the host or runner. A same-UID actor can synthesize
a fully self-consistent transcript and recompute its hashes. Cargo/rustc are
resolved to the repository-pinned 1.93.1 toolchain, run with sanitized semantic
environment overrides and an isolated configuration-free `CARGO_HOME`, and
their exact paths, versions, and hashes are retained in the corridor receipt.
Java resolution rejects a non-working launcher such as the macOS
`/usr/bin/java` stub, honors an explicit `JAVA_BIN` only when it executes, and
otherwise selects a canonical working JDK from `JAVA_HOME`, `PATH`, the
repository-local JDK, or stable package-manager links. The selected binary's
canonical path and SHA-256 remain receipt-bound.

Run the full gate from the repository root:

```bash
bash scripts/formal/install_sumeragi_v2_tlapm.sh
bash scripts/formal/install_sumeragi_v2_tla2tools.sh
bash scripts/formal/install_sumeragi_v2_verus.sh
bash ci/check_sumeragi_formal.sh
```

TLC requires exact exhaustive completion from two bounded configurations, one
exact deterministic-simulation transcript from each of four configurations,
and the exact deliberately violated invariant from the locked-Commit recovery
witness; none of these outcomes changes proof status. Its liveness configuration uses
finite `65535` timeout and view ceilings, above the configured complete service
budget and within the
pinned TLC 1.7.4 integer evaluator; the deductive model keeps these constants
symbolic. The ChainEpoch simulation uses a separate full-state harness:
`ChainEpochTlcInit` initializes inherited Core state, `ChainEpochTlcVars`
subscripts it with the receipt variables, and `ChainEpochTlcNext` freezes Core
while a directly constructed receipt relation advances. The deductive
`ChainEpochNext` and `ChainEpochSpec` are unchanged, and
`ChainEpochTlcReceiptNextRefinesChainEpochNext` checks that every optimized
harness receipt is an ordinary deductive step. Its focused strict slice is
green at 5/5 obligations. Simulation
transcripts must contain one seeded header, one initial-state marker, at least
one progress marker, one exact single- or multi-unit duration footer, status
zero, and no TLC error. Before those searches, the gate runs the original eight
explicit scheduler mutation/repair pairs:

1. equal-value replacement versus exact queued-envelope coalescing;
2. deferred-owner replacement versus scheduler-wide coalescing;
3. strict deferred-class priority versus the cyclic deferred-class cursor;
4. Busy Completion requeue without cursor advance versus cursor advance;
5. same-source head-only ingress versus oldest-admissible indexed removal;
6. aggregate-only ingress capacity versus the explicit per-lane bound;
7. conflated pending-work/completion capacity versus separate capacities; and
8. producer-first local admission versus sticky causal debt and the alternating
   local-source cursor.

The protected-rank follow-up checks four additional adversarial families:

9. four causal-capacity refill lassos versus sticky class-specific debt, plus
   the exact duplicate fast path;
10. blind causal-successor replacement versus scheduler-wide exact coalescing;
11. recurring Commit-certificate discovery inside `RunNode` versus its own
    fairly scheduled auxiliary action; and
12. indexing every physical I/O job versus indexing only Consensus owners.

An exhaustive one-validator ownership configuration separately checks
`AsyncProgressOwnershipInvariant` over 42,817 generated states (6,208 distinct,
depth 45). The expanded graph covers the independent non-timeout-progress and
TimeoutVote ingress reservations. The deliberately broken configurations must produce their exact
counterexamples; all repaired configurations must complete without error.

The producer-first buggy configuration has the pinned three-state fair lasso and exits
TLC with status 13; the repaired alternating configuration exhausts its
seven-state bounded graph with no error and exits with status 0. The ingress
capacity mutation separately exposes the two-state invariant failure that its
per-lane bound rejects. These bounded mutations are regression witnesses and
counterexample searches, not deductive proof and not grounds for changing any
ledger status.

Two focused seam models exercise the stronger vocabulary. An unprotected
Normal proposal/Prepare candidate has a fair starvation counterexample, and a
dynamic delivery-class mutation loses a stored CommitVote after TC; the frozen
constructor inventory closes both. Separately, a scheduler-only deadlock claim
accepts a bare tick, whereas the productive claim rejects it until a concrete
deadline, evidence, rank, or decision repair exists. These bounded checks do
not discharge the productive release obligation.

The model-trace
replayer drives the exact production
reducer API. The source-linked Verus harness proves the reducer/WAL and
scheduler kernels, runs the required adversarial simulations, and retains its
log under `target/formal/sumeragi_v2/`. The ignored 100,000-height chaos test is
an additional implementation stress gate, not deductive evidence.

## Trusted computing boundary

The proof ledger keeps the remaining premises explicit:

- signature authenticity and collision resistance;
- faithful complete-frame `fsync` acknowledgement;
- deterministic reconstruction, validation, and execution;
- a responsive honest dual quorum after GST;
- bounded post-GST authenticated transport; and
- continuing clock/run-loop service plus termination of admitted local work.

TLAPM, its backends, Verus, vstd, the SMT solver, the Rust compiler, the
production-to-proof extraction code, the operating system, and the hardware
remain part of the implementation proof TCB. Generated evidence records the
cooperative, hash-bound claim that the configured tools accepted the exact
sources; it does not authenticate either the host or the runner that produced
that claim.
