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
  catch-up, old-view CommitQCs, body recovery, decisions, and application.
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
  at a time. Validators absent from an old roster use the production-shaped
  authenticated historical CommitQC/body service and ordinary local
  application receipt to catch up and join; certification and local application
  do not use a global all-node barrier. Its temporal multi-height induction
  remains explicit proof debt.
- `SumeragiV2AsyncNetwork.tla`, `SumeragiV2LivenessProofs.tla`, and
  `SumeragiV2AsyncLivenessProofs.tla` model the production scheduler and
  transport and state the exact conditional progress obligations after GST.
  The volatile vote pool is the delivery-epoch witness: an exact vote crosses
  once while present, TC installation clears the local pool, and a retained
  exact locked Commit vote enters reserved progress admission in the new
  reducer generation. The protected deferred lane has one slot per locked-vote
  signer plus dominant PrepareQC, CommitQC, and TC slots; same-slot updates
  coalesce without evicting another protected slot. Immutable authenticated
  history remains separate from this consumer state. Fair transport ingress
  requires at least `2 * |ValidatorIds| + 1` entries: an empty validator keeps
  both a first-message and progress reservation, while a singleton progress
  entry keeps a continuation reservation so service cannot invalidate the
  bound; anonymous and non-roster senders share the final untrusted slot. The
  inductive invariant also records that every individual source lane is at
  most the aggregate ingress capacity, matching the runtime admission gate;
  this makes one-item removal decrease the counted depth by exactly one even
  when preservation is checked from an arbitrary invariant state. Each
  authenticated validator source also leaves a distinct configured 64 KiB
  timeout-vote reserve unavailable to ordinary traffic (in addition to body
  envelope headroom). That isolated region exceeds the conservative 4 KiB
  maximum valid timeout-vote envelope, including a 128-signer PrepareQC. A
  validator lane owns at most one distinct queued TimeoutVote in that region;
  exact retransmissions coalesce, and a newer vote retries after fair service
  releases the owner. Thus auxiliary byte saturation cannot invalidate the
  first free-reserve timeout-vote admission proved by the message-level model.
  The one-height `AsyncSpecAt` type-closure wrapper and generation-scoped vote
  delivery obligation have TLAPS proof bodies. The runner-preservation leaf
  and temporal liveness obligations remain ledgered `specified_unproved`, so
  this is not a machine-checked completion claim. Logical views are unbounded
  in the deductive liveness abstraction; finite TLC configurations remain
  counterexample searches only.
- `proof_coverage.json` is the authoritative theorem/trust-boundary ledger.
  Tool output and obligation counts belong only in generated evidence under
  `target/formal/sumeragi_v2/`.
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
historical Commit intents remain fenced. An old-view CommitQC remains decisive
after a view change.

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
sign-once behavior, external validity, certified-body availability, lock and
timeout protection, agreement, absence of conflicting CommitQCs, crash/restart
preservation, chain-prefix safety, and epoch-context isolation. Their exact
mechanization status is recorded per obligation in `proof_coverage.json`.
Crash/restart preservation is a behavior-scoped temporal contract: under the
selected core specification, every crash or restart preserves the durable
projection, interrupted writes stay unacknowledged, and stale generations are
rejected.

Liveness is necessarily conditional. FLP rules out unconditional deterministic
consensus termination in a fully asynchronous network. The post-GST theorem
therefore has explicit premises: a non-crashing honest set independently meets
both quorum thresholds; authenticated retransmissions and serialized service
have declared finite representable bounds; the monotonic clock and run loop
continue; and admitted fsync, signature, reconstruction, deterministic
validation, and local application work terminate. The immutable view-zero
deadline grows linearly as `base * (view + 1)`, while retransmission retains its
fixed base interval. Consequently some post-GST view exceeds the complete
bounded service rank without assuming in advance that one configured fixed
deadline is already adequate. Under those premises, failed views form and
install TCs, rotation reaches a responsive honest leader, a safe round decides,
every responsive validator eventually applies the certified body, and each
local chain advances.

The target statement is exactly: after GST, with a responsive dual quorum and
terminating local work, every height eventually decides and every responsive
validator eventually applies it. It makes no termination claim during an
unbounded partition, without a responsive dual quorum, or while admitted disk,
signing, reconstruction, validation, or application work does not terminate.

The theorem is consensus-height progress, not transaction fairness. A valid
empty heartbeat can satisfy progress. Transaction inclusion, mempool fairness,
and censorship resistance are explicitly out of scope in the proof ledger.

The mechanization boundary is narrower than the argument above. The universal
`AsyncTypeInvariantObligation` now has a checked proof body, while its concrete
runner-preservation leaf and the timeout-view, rotating-leader, and application
liveness obligations over `AsyncSpecAt(initialContext)` remain exact release
declarations with `specified_unproved` status. The concrete genesis chain
product separately records its first-successor handoff, when a successor
height exists, as `GenesisHeightSuccessorHandoffObligation`, also
`specified_unproved`. At the terminal finite horizon that handoff is explicitly
vacuous rather than manufacturing a successor instance.
The chain refinement now models an indexed family of authoritative
`AsyncSpecAt` instances and exposes the exact
`SumeragiV2ChainEpochRefinement!HeightLivenessObligation`. That theorem remains
explicit `specified_unproved` debt until instance activation, exact-action
fairness on the all-joined suffix, authenticated historical catch-up fairness
for successor validators absent from an old roster, and finite-height temporal
induction are discharged. Its exact completion predicate requires application
at terminal `MaxHeight`; at every nonterminal context it requires each
responsive validator to advance into a successor context. Catch-up copies an
already canonical exact QC and a
certified-signer-held body, then refines the ordinary decision and application
receipt boundaries; it cannot create finality. Dormant non-genesis instances
retain their exact `InitAt` parent receipt internally, but only current-context
and explicit catch-up receipts enter the global ChainEpoch projection, so the
indexed genesis is non-vacuous. The first-release model does not restore a
favourable-network relation, global asynchronous shadow state, or a second
consensus transition relation to stand in for that proof.

The ledger also names the previously implicit intermediate obligations.
Generation-scoped delivery now has a checked proof body. Durable
progress-witness preservation, post-GST deadlock freedom, strict
protected-service-rank decrease, starvation freedom, and the async runner's
remaining scheduler-type leaf intentionally remain `specified_unproved`;
adding the vocabulary is not machine proof.

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
`machine_checked_completion=true` is accepted for release only when no such
entry remains and fresh source-bound evidence validates.

Run the full gate from the repository root:

```bash
bash scripts/formal/install_sumeragi_v2_tlapm.sh
bash scripts/formal/install_sumeragi_v2_tla2tools.sh
bash scripts/formal/install_sumeragi_v2_verus.sh
bash ci/check_sumeragi_formal.sh
```

TLC requires exact no-counterexample completion from six bounded configurations
and the exact deliberately violated invariant from the locked-Commit recovery
witness; neither outcome changes proof status. Its liveness configuration uses
finite `65535` timeout and view ceilings, above the configured complete service
budget and within the
pinned TLC 1.7.4 integer evaluator; the deductive model keeps these constants
symbolic. Before those searches, the gate runs three explicit scheduler
mutation pairs. Equal-value replacement and same-source head-only ingress each
have a pinned temporal counterexample, while queued-envelope coalescing and
indexed oldest-admissible removal close their respective lassos. An overlong
lane also gives a two-state counterexample to the old aggregate ingress
capacity invariant: removing its only progress item exposes a reservation
without decreasing the capped depth. The explicit per-lane capacity bound
rejects that state and preserves the repaired two-state model. These bounded
mutations are regression witnesses, not deductive proof. The model-trace
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
remain part of the implementation proof TCB. Generated evidence attests that
the configured tools accepted the exact sources; it is not a cryptographic
attestation of the host that ran them.
