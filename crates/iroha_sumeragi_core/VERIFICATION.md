# Sumeragi v2 reducer verification

## Pinned environment

- Verus `0.2026.05.31.5dd6d83`
- Rust `1.95.0-aarch64-apple-darwin` (or the matching host triple)
- `vstd = 0.0.0-2026-05-31-0205`

Run `scripts/verify_sumeragi_v2.sh` from the workspace root after placing the
official pinned Verus release binaries in `PATH`. The script rejects a different
Verus or vstd version, verifies the official macOS arm64 or Linux x86_64 binary
checksums (other platforms must supply the two pinned checksum variables), and
rejects the wrong bundled Rust toolchain or proof escape hatches in the
production reducer and formal proof modules. It also rejects any external
`#[path]` from the production module, then runs exactly eleven deterministic
adversarial network simulations before invoking Verus.
The script enables the crate's `verus` feature explicitly; normal production
builds do not compile or link `vstd`. Verifier output is streamed to the caller
and retained at `target/formal/sumeragi_v2/verus.log` for CI to archive; shell
`pipefail` keeps a failed verification from being masked by log capture.

The dependency-free reducer sources are authoritative under
`crates/iroha_core/src/sumeragi/v2_core/`; this excluded crate is a formal
harness over those same package-local files. The script copies both into a
disposable workspace, generates that workspace's lockfile, and runs the eleven
loss/duplication/reordering, symmetric/asymmetric partition, proposal/locked-body
crash, corrupt-body, withheld-evidence, historical-certificate
consumer-tag/redelivery, divergent-view, and accelerated chain-prefix
simulations with `--locked
--offline` and one test thread.
The repository `Cargo.lock` is never read for resolution or modified.

The script uses a clean Cargo target by default. After the simulations, it
forwards `--no-cheating` only to the selected root crate with Cargo Verus's
`--fwd-verus-args-to roots`; dependencies are still verified, but pinned
`vstd` is allowed to use its reviewed trusted specifications. Verus, vstd, and
the solver are therefore part of the proof TCB. The scanned production reducer
and proof modules contain no `assume`, `admit`, external body, or external
specification escape hatch.

The following primitive source-link receipt was produced with the official
pinned macOS arm64 release. It predates the current proposal-origin changes and
was not rerun for this source, so its counts are historical evidence only and
must not be cited as discharge of the changed obligations:

```text
$ scripts/verify_sumeragi_v2.sh  # official pinned release already in PATH
verification results:: 1690 verified, 0 errors  # pinned vstd dependency
verification results:: 157 verified, 0 errors   # iroha_sumeragi_core root obligations
```

Evidence for the source-link edit itself uses the isolated harness because
`iroha_sumeragi_core` is intentionally excluded from the root workspace:

```text
bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  118 unit/reducer/WAL/refinement tests passed
bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
  8 model-trace replay tests passed
bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  11 deterministic network simulations passed
bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k
  50,000 permissioned + 50,000 NPoS heights passed
  400,000 validator finalizations; 0 failures; 91.29 seconds
bash scripts/formal/run_sumeragi_v2_harness.sh --clippy
  passed
```

The current Rust runs exercise the source-shared reducer/WAL/refinement logic,
model replay, all eleven named fast-network scenarios, and the deterministic
chaos schedule. They are not a Verus discharge. A fresh pinned run is required
before the changed primitive-to-derived-fact or production commit-gate
obligations can be marked verified. Unverified `std` collection code,
cryptography, and adapter contracts remain outside Verus in any case; the
remaining boundary is listed explicitly below.

## TLC trace replay against production

`tests/model_trace_replay.rs` replays a normalized 95-action TLC witness against
this crate's exact public `Reducer::step` API. It is freshly emitted from the
`SumeragiV2TraceWitness.tla` behavior with a nonresponsive view-zero leader.
Three timeout votes form and durably install a TC, all four reducers enter view
one, the rotated leader proposes subject A, and distinct three-validator
Prepare and Commit quorums produce a durable decision. The replay drives actual
Persist, Sign, Broadcast, FetchBody, StoreBody, ValidateBody, EnterView, and
Apply effects; it does not call a test reference reducer.

The witness module selects one representative production-replay schedule; its
selection operators are not Core behavior or proof premises. It starts at GST,
does not refetch an exact body after durable storage, uses the view leader as
the single projected PrepareQC aggregator, and emits one projected PrepareQC
and CommitQC per round and subject. One designated non-preparing validator is
kept below a local Prepare quorum and delays body validation until QC
observation begins, which makes the finite trace cross the `ObservePrepare` WAL
boundary. It remains a voting validator in the four-validator model. These
selection rules apply only to Prepare scheduling and QC projection. They never
restrict Commit vote delivery or retransmission: the same signed Commit
envelope can be attempted before a lock and admitted after intervening lock
persistence.

TLA2Tools 1.7.4 is checksum pinned. That release does not support
`-dumpTrace json`, so the comparator uses supported `-tool` message framing and
extracts the trace-only scalar `witnessAction` record. Reproduce and compare the
witness with:

```text
scripts/formal/check_sumeragi_v2_replay_trace.sh
TLC replay witness matches 100 checked-in production actions and the production reducer

CARGO_TARGET_DIR=/tmp/codex-sumeragi-model-trace-target \
  cargo test --locked -p iroha_sumeragi_core --test model_trace_replay
8 passed; 0 failed
```

The normalizer rejects malformed tool-message framing, a mismatched seed or
invariant, non-contiguous states, duplicate or non-scalar marker fields,
unknown actions, invalid four-validator parameters, and any trace not ending
at `PersistDecision`. The Rust trace parser additionally rejects malformed
fields, stale/wrong leaders, missing durable intent boundaries, and
Prepare/Commit certificate formation without three distinct delivered voters.
Adversarial production tests recover every prefix of the witness WALs and
confirm that the combined trace crosses all seven record classes. They also
cover exact signed Commit-envelope redelivery across the pre-lock/post-lock
consumer boundary, crash after acknowledged intent, exact WAL resume,
stale-generation completion, duplicate and overlapping certificate signers, Prepare-vote and
full-high-QC timeout equivocation, and invalid body validation withholding
Prepare. The Python normalizer suite adds 15 positive and fail-closed parser
cases.

This is executable refinement evidence, not deductive proof. The checked-in
witness uses one four-validator permissioned/count context; unequal-stake
traces remain in `network_simulation.rs`, not in this TLC fixture. The model's
genesis height zero maps to production height one, model validator integers map
to deterministic 32-byte IDs, and model subject atoms map to deterministic
hash fixtures. The TLA model exposes ObservePrepare then LockCommit while the
production WAL atomically persists highest PrepareQC, lock, and Commit intent;
the harness explicitly accepts only that stronger `LockAndCommit` mapping.
The raw TLC witness has no crash action, so crash/stale-completion coverage is a
derived adversarial replay. Signature verification, Norito decoding, physical
WAL framing/fsync, and the asynchronous runtime adapter remain outside this
pure-reducer harness.

## Accelerated 100,000-height chaos gate

`tests/network_simulation.rs` includes an explicit ignored release gate that
finalizes two independent 50,000-height chains: one permissioned and one with
unequal NPoS voting power. Every height runs four production reducers, rotates
through certified views, varies delivery order, accepts delayed old-view
`CommitQC`s, rejects stale completions, periodically injects an under-quorum
decision, applies the exact certified body, consumes a matching durable Kura
receipt, and binds the resulting `CommitQC` as the next height's parent.

Run the workspace-excluded harness directly, or use the clean-source launcher
through the nightly workflow's independent chaos job (the strict formal job
remains separately failure-blocking):

```text
CARGO_TARGET_DIR=/tmp/sumeragi-v2-chaos \
  bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k

bash scripts/run_sumeragi_v2_100k_chaos.sh
```

The 2026-07-13 macOS arm64 run completed all 100,000 heights in 57.53 seconds
with no conflicting decision or chain-prefix failure. This is a deterministic
long-run implementation test, not a substitute for the deductive safety or
conditional-liveness proofs.
A final-source rerun on 2026-07-16 completed in 52.97 seconds with both chain
prefixes intact. This focused harness result does not replace the release
profile's checkout-manifest-bound evidence rerun.
The ignored test emits one exact `SUMERAGI_V2_CHAOS_COMPLETED` marker only after
both 50,000-height prefixes close. The source-bound launcher and aggregate
receipt require that marker as well as the one-test libtest result. The harness
still supplies valid certificates directly, so it does not prove network quorum
formation. Local adapter effects are queued one per deterministic scheduler
rank. Every 64th height rotates through restart after Decision-WAL append,
FetchBody, StoreBody, validation, and application; the recovered reducer must
reject the captured old-generation completion. The gate also pins duplicate
and reordered certificate delivery plus insufficient count-only and power-only
NPoS certificates. Its schema-v2 marker binds the exact schedule and counters.
These are bounded reducer/recovery faults with terminating fixture work, not a
substitute for the real-network seed matrix or Taira-profile soak. An exact
schema-v2 harness run on 2026-07-17 completed all 100,000 heights in 57.52
seconds and matched every pinned counter. The source-attested wrapper correctly
refuses a dirty worktree, so the final checkout-manifest-bound rerun remains
required after these changes are in a signed clean commit.
A 2026-07-21 run against the current proposal-origin source completed the
50,000-height permissioned prefix and 50,000-height unequal-power NPoS prefix,
400,000 validator finalizations, and zero failures in 91.29 seconds. This is a
mutable-working-tree harness result, not a source-sealed release receipt or a
Verus proof.

## Current refinement model

`crates/iroha_sumeragi_core/src/verus_proofs.rs` contains one safety projection
for the production WAL and reducer rather than independent protocol examples.
`crates/iroha_sumeragi_core/src/effective_lock_verus_proofs.rs` isolates the
solver queries for exact-body ownership, retirement accounting, and runtime
service selection while instantiating the same production macros.

The current source projection separates the reducer lifecycle owner from two
wire identities. `proposal_round` is the immutable proposal/body/header origin;
Vote/QC `round` is the Prepare or Commit certification round. Prepare requires
the two to be equal. Commit permits a later certification view in the same
context and height. Body recovery, header association, validation, and
application bind the proposal origin. Later-view Commit recovery re-signs that
origin in the active finality round, selects only the newest durable Commit
intent for `(proposal_round, subject)`, and retires older same-origin pools
after the replacement WAL acknowledgement. A locked value is committed
directly; equal bytes at a new proposal origin are not admitted.

Pending-WAL and boundary capability projections bind the primary proposal
origin separately from the lifecycle owner and bind the auxiliary proposal
origin of any embedded certificate. Begin, acknowledgement, requested effect,
and reconstructed grant must match both. Revision 4 has no legacy decoder or
missing-origin inference.

The production timer/FIFO arbiter in
`crates/iroha_core/src/sumeragi/v2_core/scheduler.rs` is also source linked.
Ordinary Rust and Verus instantiate the same macro-expanded branch relation,
so absolute-timeout priority, one-slot periodic delay, FIFO debt, ordinary FIFO
service, and idle selection cannot drift through a separately transcribed
proof model. The runtime clock and task-invocation premises remain part of the
post-GST host-service contract; the choice made at each invocation is verified.

The inner runtime `completion -> progress -> normal` selector is now source
linked in the same way. Production `BoundedIngress::pop_next` calls the typed
three-class kernel, while Verus proves that every selected class is ready,
invalid cursors select nothing, and continuously ready classes form one exact
three-invocation cycle. This is a bounded arbitration theorem only. It does not
assert that the host invokes the runtime again or that external disk, network,
validation, or application work terminates.

Exact-body ownership now has a second source-linked kernel at the production
executor/runtime boundary. Production passes typed tag, `(round, subject)`,
manifest, lane-owner-count, and byte-counter identities; it cannot pass an
authorization boolean. The checked relation permits a certified Fetch to fill
an absent manifest identity once, then requires the same owner through
`BodyAvailable`, `StoreBody`, and `ValidateBody`. Rebinding requires a strictly
newer view and generation at the same height and preserves the exact key and
manifest. Runtime ingress and the Busy-deferred lane jointly admit only one
exact completion owner. Higher-lock and certified-view retirement uses the
same sequential subtraction relation proved to leave exactly the original
counter minus every retired owner, without an overflowing intermediate sum.
Before either EnterView cleanup or direct locked-body reconciliation, the
executor also requires both counters to equal the complete serialized
ready/retained/store owner sets; even exact lock repetition cannot bypass that
aggregate equality check.

Authenticated consensus-message ownership uses the same generic production
bridge. `deferred_authenticated_message_owner` encodes the complete candidate
envelope and compares those bytes with the deferred occurrence's retained
`authenticated_wire_identity`; runtime admission repeats the exact comparison
after authentication. The QC-named lookup wrappers are `cfg(test)` regression
conveniences only. They are not a second, reducer-event-only production
ownership rule and are not counted as production-refinement evidence.
Collection lookups, manifest hashing, and external service acknowledgements
remain ordinary Rust extraction/adapter boundaries; temporal service remains
the conditional liveness obligation.

`src/wal.rs` also owns a dependency-free executable mapping contract for the
physical WAL. The adapter supplies only the 32-byte hash function (BLAKE3 in
production) and the three ordered I/O operations. The core implementation:

- encodes the exact `SUMV2WAL` header and `S2FR` frames used by production;
- binds the header to format revision, protocol version, chain hash, and local
  consensus-key hash;
- validates the monotonic physical sequence, 16 MiB record bound,
  previous-frame link, and complete-frame checksum;
- returns the sole safe truncation boundary for an incomplete final append,
  without exposing that tail as a recovered record;
- fails closed on complete corruption before or at the final frame;
- calls `write_all`, `flush`, and `sync_data` in that order, advances the
  sequence/hash state only after all three succeed, and poisons the append
  instance after any I/O error; and
- mints `WalRetirementAuthorization` only from `FinalizedHeight`, which is
  available only after application and verification of the exact durable Kura
  block-and-`CommitQC` receipt.

The Verus projection covers exact header identity, complete-prefix extension,
incomplete-tail stuttering, complete-corruption fail closure, append receipt
ordering, and retirement prerequisites. The byte loop, supplied hash function,
and adapter filesystem implementation remain outside Verus; the uncompleted
production call-site mapping is stated precisely in gap 5 below. Header and
complete-frame acceptance, append acknowledgement, and retirement authority
use the same macro-expanded predicates in ordinary Rust and Verus, so an
accepted core frame cannot bypass a separately transcribed proof guard.

The WAL projection enumerates all seven production `WalRecord` variants:

| Production record | Projected guards and post-state |
| --- | --- |
| `ProposalIntent` | contiguous frame, frozen context, current view, expected local leader, safe parent/TC justification, open view, and one immutable proposal subject |
| `PrepareIntent` | valid local Prepare, current/open view, and one immutable Prepare subject |
| `ObservePrepare` | valid PrepareQC no later than the current view, compatible equal-view subject, and strictly-higher-only highest-QC replacement |
| `LockAndCommit` | valid matching PrepareQC and local Commit, current/open view, non-regressing lock, immutable Commit subject, and atomic lock-plus-intent installation |
| `TimeoutIntent` | frozen context/height, current view, in-roster local signer, exact durable full high-QC evidence identity, and one immutable timeout intent |
| `InstallTimeout` | valid TC, non-regressing certified view, no counter overflow, compatible selected PrepareQC, monotone lock, and entry into exactly `tc_view + 1` |
| `Decision` | valid CommitQC and an absent or identical durable decision |

`PrepareIntent` no longer crosses the abstract WAL boundary with a
caller-supplied `local_vote_valid` boolean. Its projection carries the vote's
context, height, phase, signer, view, and subject, while the replay projection
carries the frozen roster size and local-validator index. Admissibility derives
the same context/height/Prepare-phase/local-signer/roster checks performed by
`DurableState::validate_local_vote`. The proof
`prepare_intent_guard_is_derived_from_vote_and_frozen_context` makes those
primitive consequences explicit.

`TimeoutIntent` likewise no longer carries either `local_vote_valid` or
`high_reference_matches` across the abstract WAL boundary. Its projection
carries context, height, view, signer, and the optional full PrepareQC. The
guard derives context/height/current-view/local-signer/roster membership and
compares both the semantic certificate reference and a fixed evidence identity
for its signer/signature bytes. `timeout_intent_guard_is_derived_from_vote_and_frozen_context`
makes those consequences explicit. The executable replay path now performs the
same frozen-roster membership check. Missing high QCs and same-reference QCs
with different signer evidence are rejected transactionally by the Rust test.

Proposal, observed-QC, Commit-intent, TC, and decision predicate compression
remains part of the projection-extraction gap listed below. The proof-level
certificate evidence identity is still extracted by ordinary Rust and is not a
proof of the cryptographic bytes or their hash function.

The public `DurableState::apply` clone-and-swap behavior is represented by an
accepted path and a rejected path. The rejected path preserves every projected
field. Sequence exhaustion and TC-view overflow are explicit rejected cases.

The reducer input projection enumerates all sixteen production `Event` variants
and includes the `(height, view, generation)` tag. Its path relation represents:

- tag rejection and pending-WAL backpressure as state-preserving paths;
- accepted body availability, storage, and validation progress;
- every source event that can reach each `start_persistence` call site;
- acknowledgement of the sole matching frame through the exact WAL relation;
- successful application acknowledgement for the exact durable decision;
- generation overflow as a state-preserving failure instead of a partial TC
  installation in the reducer projection; and
- `ResumeAfterReplay` as a recovery-authenticated, fully tagged, one-shot
  transition whose stale and duplicate deliveries stutter without effects;
  all other current-tag inputs remain behind a `RecoveryPending` fence until
  that transition commits.

The possible persistence records are phase constrained. A Prepare vote or QC
can lead only to `ObservePrepare` or `LockAndCommit`; a Commit vote or QC can
lead only to `Decision`; a timeout vote or TC can lead only to
`InstallTimeout`; and a successful validation can lead only to Prepare or
Commit intent for the same view and subject.

The reducer projection keeps two different body facts deliberately:

- monotone historical durable/validated tokens record the adapter ordering
  contract needed to authorize proposal and Prepare signatures, including
  replay of an already-authorized intent; and
- an application-ready set mirrors the current `BodyState::Validated` boundary
  and is cleared on TC generation change and crash recovery.

This distinction prevents a replayed WAL record from manufacturing a body
durability fact that is not present in the record itself.

The executable commit gate additionally projects an explicit reducer action
class (`Stutter`, `BeginWal`, `AcknowledgeWal`, `BodyProgress`,
`VolatileProtocol`, `CompleteApplication`, or `ResumeAfterReplay`) and the
exact participating WAL record class. Successful signature completions
distinguish proposal, Prepare, Commit, and timeout messages. This removes the
previous ambiguity where the same collection of booleans could describe
several unrelated source branches.
The action classifier is decomposed into the same small executable predicates
in production and Verus, including a separately checked validation-completion
effect predicate. This keeps the solver query modular without changing the
runtime decision or raising its resource limit.

Each side of the transition also carries fixed-width cardinalities for the
candidate/body work, pending and known PrepareQCs, active vote and current-view
timeout pools, locally formed certificates, retained outbound controls,
signature FIFO/in-flight slot, and replay-resume flag. The gate enforces:

- at most two active vote pools and `2 * validator_count` entries: current
  Prepare plus the latest durable Commit finality pool for the exact locked
  proposal origin. Every Commit requires that origin and subject;
  acknowledging a later-finality `LockAndCommit` prunes all older same-origin
  Commit pools and only then releases its signature;
- at most one timeout pool and `validator_count` timeout entries;
- at most two locally formed QCs, one locally formed TC, and seven retained
  outbound control classes;
- every pending PrepareQC is known, with at most the durable highest/lock pair
  additionally known;
- body work is bounded by pending certified bodies plus candidate/decision
  identities; and
- the signature FIFO plus its sole in-flight item cannot exceed the durable
  signable-intent bound.

Acknowledging `InstallTimeout` additionally requires the exact production
reset: candidate/body/pending/vote/timeout/formed pools are empty, at most two
durable PrepareQCs remain known, and at most the three permitted post-TC
control classes remain retained.

A pre-lock current Commit is a recoverable ignore rather than a third pool.
The adapter advances its locked-Commit consumer epoch after the matching
acknowledgement, allowing that exact authenticated vote to cross once after the
lock exists. Retained outbound Commit control serves peers, but is not a
sufficient local progress witness because broadcast excludes the sender. These
source-linked reducer constraints preserve the fixed-width cardinality gate;
they do not by themselves supply the fresh strict evidence required for the
promoted TLAPS liveness targets.

The source-shared locked-Commit progress kernel additionally models validation
finishing after a TC-promoted lock's active finality view has durably timed
out. It accepts the exact current durable timeout as a recovery witness only
for that historical lock; this does not authorize `LockAndCommit` or signing
in the closed view. The next installed TC restarts exact-origin recovery in an
open finality view. Stale, wrong-signer, volatile-only, and non-exact timeout
projections are rejected by executable mutation tests, and the same expression
is instantiated by the Verus theorem
`locked_commit_progress_witness_is_valid`.

## Encoded proof obligations

The module contains transition-by-transition proof functions for:

- the exact production scheduler choice, absolute-timeout priority, and the
  two-invocation bound when periodic retransmission precedes ready FIFO work;
- strict count and voting-power quorum arithmetic;
- exact physical-WAL header identity;
- complete-frame sequence/hash-chain extension and fail-closed corruption;
- incomplete final-frame recovery as an unacknowledged state-preserving tail;
- write/flush/sync ordering before append receipt and hash-state advance;
- exact Kura block-and-certificate evidence before WAL retirement;
- accepted-WAL invariant preservation;
- transactional rejection of malformed, non-contiguous, overflowed, or
  otherwise inadmissible WAL frames;
- derivation of every local `PrepareIntent` authenticity check from the vote
  primitives and frozen replay context, without a validity-bit premise;
- derivation of every local `TimeoutIntent` context, height, current-view,
  signer/roster, and exact full-high-QC-evidence check from primitives, without
  validity or high-QC-match bit premises;
- immutable proposal, Prepare, Commit, and timeout intents;
- the postcondition of every individual WAL record variant;
- atomic `LockAndCommit` installation;
- lock, view, highest-PrepareQC, and decision monotonicity;
- body storage before validation and application readiness;
- reducer invariant preservation for every reducer path;
- persistence-before-signing, Decision-before-Apply, and TC-before-EnterView
  effect fences;
- reducer-level vote uniqueness, lock monotonicity, and decision uniqueness;
- bounded volatile-evidence summaries on both sides of every committed step
  and the exact persisted-TC reset;
- consistency of reducer action/WAL/signature discriminants; and
- an explicit production macro-step map to named `SumeragiV2Core.tla`
  ingress, formation, begin/persist, body, signature, replay-resume, and apply
  actions, with a checked safety-state delta for the durable boundary; and
- crash/replay preservation of the complete WAL safety projection while
  discarding volatile application readiness; and
- exact replay-resume effect classes: proposal, Prepare, Commit, or timeout
  Sign; certified decided-body Fetch; or no effect for an empty WAL.

### Replay-resumption proof ledger

| Obligation | Status | Evidence |
| --- | --- | --- |
| No public lifecycle bypass | Implemented and tested | The only API is tagged `Event::ResumeAfterReplay` through `Reducer::step`; a fresh reducer and duplicates stutter |
| Stale height/view/generation cannot consume recovery | Implemented and tested | Adversarial reducer tests preserve the complete pre-state and emit no effects |
| Exact one-shot state/effect relation | Encoded in the production gate | `ACTION_RESUME_AFTER_REPLAY` checks false-to-true, unchanged durable state, and the exact Sign/Fetch/empty effect class |
| Abstract reducer refinement | Encoded | `ReducerPathProjection::ResumeAfterReplay` preserves WAL, application, and effect fences |
| Named TLA+ action map | Encoded and spelling-gated | Proposal/vote/timeout resumption maps to the existing `ResumeProposal`, `ResumeVote`, and `ResumeTimeout` actions; decided replay maps to `FetchBody` |
| Pinned Verus discharge of the changed obligations | **Pending rerun** | The historical 1690-dependency/157-root receipt predates the proposal-origin source changes; the combined source contract expects 1690 dependency and 158 root obligations |

## Exact production commit gate

`crates/iroha_core/src/sumeragi/v2_core/refinement.rs` defines a small
executable transition gate. Its decision expressions are shared textually with
the Verus module by macros; the verifier does not check a separately copied
reference implementation. The normal Rust build instantiates those expressions
in `refinement::accepts`, while
`crates/iroha_sumeragi_core/src/verus_proofs.rs` instantiates the same
expressions in Verus proof functions. Their discharge for the current source is
pending the fresh pinned run noted above.

`Reducer::step` now performs the real transition on a private clone and invokes
the gate before replacing caller-visible state. Successful, ignored, and error
paths all pass the gate. A rejected candidate returns
`ReducerError::RefinementViolation` and leaves the original reducer unchanged.

The gate receives the complete effect vector as a fixed eight-slot trace. The
bound is structural: seven retained control-message classes plus at most one
Decision body-pipeline effect (FetchBody, StoreBody, ValidateBody, or Apply)
fill eight slots; a ninth fails closed. Every active slot
contains its exact vector position and two fixed-width primitive capability
keys: one requested by the concrete effect and one independently reconstructed
from the event and candidate state. The shared proof kernel computes
authorization
by comparing every key field; callers no longer supply `authorized=true`.
Invariant truth, state equality, tag matching, busy-fence state, action class,
WAL class, continuation class, and boundary exactness are likewise derived
inside the kernel from concrete state identities, event primitives, violation
counts, and requested/granted boundary keys. When discharged, the relation
proves:

- every active effect slot has identical nonempty requested and granted
  capability identities (the collection-extraction caveat below remains);
- every stale, recovery-fenced, or busy input is an exact full-`Reducer`
  stutter, not merely a transition with unchanged collection cardinalities;
- Persist, Sign, Apply, EnterView, StoreBody, and ValidateBody occur at most
  once per transition;
- a Persist effect cannot share a transition with Sign, Apply, EnterView, or a
  body-pipeline advance;
- Sign is the last effect, so decision/view/body effects precede the next
  asynchronous signing completion;
- StoreBody and ValidateBody are single-effect transitions when caused by
  their exact predecessor completion;
- a Decision retransmission emits retained control messages followed by at
  most one exact FetchBody, StoreBody, ValidateBody, or Apply owner, and the
  pipeline effect must be final;
- an EnterView effect is possible only on acknowledgement of an
  InstallTimeout continuation; and
- the ignored/busy, begin-persist, acknowledge-persist, and non-durable action
  families preserve their required state fields and effect fences; and
- only a successfully recovered reducer can change `replay_resumed` from
  false to true, exactly once, through a matching `(height, view, generation)`
  `Reducer::step` event.

The same gate rejects an action/WAL mismatch (for example, an InstallTC
continuation attached to a Decision record), an invented successful-signature
class, an over-capacity volatile summary, or any stale/busy transition that
changes a projected volatile cardinality. These are production checks, not
debug assertions.

The concrete grant reconstruction checks exact pending WAL entries and
continuations, re-applies an acknowledged frame to a cloned `DurableState`,
reconstructs Sign and Broadcast keys from durable proposal/vote/timeout
intents, reconstructs Apply from the durable CommitQC and validated body,
reconstructs EnterView from the persisted TC, and reconstructs body-pipeline
keys from the exact predecessor event and candidate work state. These checks
execute unconditionally in production; they are not debug assertions. The
ordinary Rust collection lookups that produce those concrete primitives are
not themselves verified, which remains gap 1 below, but no authorization or
action-exactness boolean crosses the shared proof-kernel boundary.

An earlier pinned verifier run discharged 157 root obligations with zero errors
on its clean historical target. It predates the proposal-origin changes and was
not rerun for the combined 158-root source. The verification script rejects
`assume`, `admit`, unreviewed trusted bodies, and external function
specifications in the package-local reducer and proof modules throughout this
crate. It also rejects reintroduction
of compressed `TimeoutIntent` validity/high-QC-match predicates and requires
the primitive-guard proof. It checks that every mapped TLA+ action name still
exists in both `SumeragiV2Core.tla` and the Verus mapping; this prevents name
drift but does not prove the independently parsed operator bodies equivalent.

The cross-tool successor inventory additionally includes
`production_terminal_application_without_successor_activation_refines_indexed_terminal`.
Its shared production/Verus kernel binds the exact height context, application
receipt, finality artifact, block identity, and authenticated durable
predecessor while requiring that no successor activation is pending. The live
runner invokes that kernel immediately after predecessor authentication and
before `PendingSuccessorConstruction::begin`. This is an Apply-boundary
separation result, not a production `MaxHeight` rule; the projection deliberately
contains no finite-horizon input.

## Production WAL byte mapping

`iroha_core::sumeragi::safety_wal` now delegates the live filesystem path to
the core contract instead of carrying a second parser and append state machine:

- file creation uses `encode_wal_file_header` with production BLAKE3, followed
  by file and parent-directory synchronization;
- startup calls `recover_wal_file`, maps its typed integrity failures, truncates
  only the returned incomplete-tail boundary, synchronizes that truncation, and
  derives `WalAppendState` from the verified complete prefix;
- every live append implements `WalAppendIo` on the single open file and calls
  `WalAppendState::append`, so the adapter receives a receipt only after the
  shared write/flush/sync predicate succeeds and cannot retry a poisoned
  instance; and
- `SafetyWal::retire` requires `WalRetirementAuthorization`, derived at the
  call site only from the actual `FinalizedHeight` returned after the exact Kura
  block-and-`CommitQC` receipt closes the reducer and rechecked through the
  shared retirement predicate before I/O. File removal remains followed by
  parent-directory synchronization.

The real-file tests retain canonical-layout, reopen/replay, incomplete-tail,
complete-corruption, hash-chain, and identity-mismatch coverage and add an
injected read-only-handle failure proving that no append acknowledgement or
retry escapes the failed-closed state. Canonical Norito payload decoding stays
the adapter-to-`WalEntry` mapping. BLAKE3 collision resistance and truthful OS
file/directory synchronization remain explicit trusted contracts rather than
Verus claims.

## Remaining work before a production correctness claim

The following gaps are exact and intentional; each must be closed before the
production reducer can be described as deductively verified:

1. **Projection extraction and the inner reducer body.** The caller-visible
   production commit decision is now the exact shared primitive kernel targeted
   by Verus,
   and every `Reducer::step` exit invokes it. Direct `Reducer` and
   `DurableState` identities are compared by the kernel; effect and boundary
   authorization is derived from independently requested and reconstructed
   fixed-width keys; and invariant truth is derived from explicit violation
   counts. The collection lookups and protocol-object decomposition that
   construct those primitive keys and counts remain ordinary Rust because this
   pinned Verus toolchain cannot verify the current `std` collection-heavy
   reducer body directly. A correlated extraction defect that constructs the
   same wrong requested and granted key is therefore not excluded deductively.
   The abstract `PrepareIntent` and `TimeoutIntent` guards now consume
   decomposed vote/context primitives instead of admissibility booleans. The
   timeout guard additionally compares a full-certificate evidence identity,
   rather than trusting a high-QC-match bit. Rust extraction of validator
   identities into the frozen-roster index and of the exact QC evidence bytes
   into that identity remains in this gap. Exact-body owner binding, stage
   transfer, rebind, cross-lane uniqueness, retirement arithmetic, and runtime
   class selection now consume typed primitives through shared production/Verus
   expressions. Extraction of those primitives from `BTreeMap`, runtime queue,
   manifest-hash, and service-owned state is still ordinary Rust and remains in
   this gap.
   Closing this residual source-level gap requires Verus-compatible reducer
   collections or verified projection functions over reviewed external type
   specifications; neither is claimed here.
2. **Continuous pinned verification.** The checked-in pinned receipt predates
   the current proposal-origin source-link changes; a fresh local pinned run
   has not been recorded. The PR and nightly formal jobs invoke
   `scripts/verify_sumeragi_v2.sh` and retain its output with the other formal
   artifacts. Those jobs must succeed before their output is release evidence.
   Any syntax or solver failure must be fixed without weakening a guard or
   invariant.
3. **Volatile reducer contents.** Cardinalities, fixed protocol bounds, exact
   full-state stale/busy stuttering, durable-signature capacity, and the
   persisted-TC reset are now in the executable/Verus gate. Exact key/value
   correspondence for
   candidate selection, body-work states, vote signatures, known/pending QCs,
   retained retransmission payloads, and signature FIFO order is still
   ordinary Rust and is not deductively verified. Replay resumption no longer
   bypasses `Reducer::step`: its one-shot flag change, complete effect-vector
   class, stale-tag behavior, and durable effect authorization traverse the
   exact executable/Verus gate and map to `ResumeProposal`, `ResumeVote`,
   `ResumeTimeout`, or decided-body `FetchBody`. The extraction of exact queued
   message contents remains part of gap 1, and replay liveness remains outside
   this safety proof.
4. **Adapter token construction.** Authenticated-event, validated-certificate,
   durable-body, deterministic-validation, and Kura-receipt constructors need
   executable contracts proving that only the corresponding checked adapter
   path can create each token. Cryptographic soundness, hash collision
   resistance, executor determinism, and fsync truth remain documented proof
   assumptions.
5. **TLA+ action-body equivalence.** Verus now has an explicit named macro-step
   map (source, optional certificate formation, durable boundary), and the
   verification script prevents action-name drift. The boundary delta is
   proved against the production gate. No shared generated semantics or
   cross-tool theorem yet proves those Verus definitions equivalent to the
   independently parsed TLA+ operator bodies. Crash, restart, and epoch-boundary
   actions also remain outside the executable `Reducer::step` map. The live
   adapter now calls one source-bound `prepend_causal_continuation` kernel, and
   `production_reverse_push_front_refines_fifo` encodes its exact
   continuation-before-old-tail sequence relation. Separate Verus induction
   theorems prove that the projected first-owner filter excludes every prior
   owner, retains every emitted identity that was not previously owned, has
   unique values, and is a stable subsequence of the emitted batch.
   Given a unique old causal queue whose identities are all included in the
   supplied owner set, the batch theorem preserves that prefix and appends a
   disjoint unique suffix. Concrete inverted-owner and recursive-append
   witnesses pin the two corresponding semantic mutations. A token-aware
   source gate binds those contracts and proof bodies to real direct children
   of `verus!`, rejecting comments, literals, macros, and `cfg` replacements.
   `ProductionEffectToCandidateTraceProjection` now closes the concrete
   effect boundary. Each positional runtime sidecar hashes the complete
   adapter effect, immutable lifecycle owner and parent relation, and the
   route-neutral semantic TLA+ candidate. The executor recomputes that
   projection from the concrete effect, counts the corresponding owner across
   every retained asynchronous task, and calls
   `check_production_effect_to_candidate_transition` before retaining the
   batch. A first owner is installed once; exact retries are consumed without minting a second asynchronous owner, and an equal candidate under a
   replacement owner fails closed.

   The mirrored Verus theorem checks the closed effect-kind map, exact
   identities and positions, the three-successor bound, and first-owner versus
   coalesced-retry accounting. The Completion residual is composed with the
   root residual by `production_completion_capacity_product_rank_descends`:
   radix four makes one root descent dominate resetting the successor
   component to its maximum value of three. The source checker seals this
   Rust/Verus gate and its production mutation boundary together with
   `AsyncCandidateCausalOrigin`, `ExactAsyncCandidateIdentity`,
   `FreshCommandSuccessors`, `AppendCausalSuccessors`, and
   `FairStage6CompletionCapacityOpens`. This discharges the concrete causal
   FIFO/Completion-capacity mapping seam; the broader independently parsed
   TLA+ action-body equivalence described at the start of this item remains a
   separate boundary.
6. **Temporal liveness.** This module proves safety-style transition
   preservation only. Fair delivery, timeout-certificate progress, rotating
   honest leaders, repeated runtime invocation, terminating body service, and
   the post-GST commit bound have promoted TLAPS target statuses but still
   require fresh exact-source strict evidence plus executable
   simulation/integration evidence. The exact three-class cycle and immutable
   body-owner rebind theorem do not by themselves discharge this gap.

Until all six items are discharged, the current Rust harness results establish
only the tested executable behavior. The historical pinned receipt proves only
the sources it hashed; it does not prove the changed proposal-origin
commit-gate relation, every line of the inner reducer, the fact-extraction
functions, the cryptographic/filesystem trusted contracts, or the protocol
liveness theorem.
