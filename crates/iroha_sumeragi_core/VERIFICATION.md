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
production reducer and formal proof module. It also rejects any external
`#[path]` from the production module, then runs exactly seven deterministic
adversarial network simulations before invoking Verus.
The script enables the crate's `verus` feature explicitly; normal production
builds do not compile or link `vstd`. Verifier output is streamed to the caller
and retained at `target/formal/sumeragi_v2/verus.log` for CI to archive; shell
`pipefail` keeps a failed verification from being masked by log capture.

The dependency-free reducer sources are authoritative under
`crates/iroha_core/src/sumeragi/v2_core/`; this excluded crate is a formal
harness over those same package-local files. The script copies both into a
disposable workspace, generates that workspace's lockfile, and runs the seven
loss/duplication/reordering, partition, crash, corrupt-body, withheld-evidence,
divergent-view, and accelerated chain-prefix simulations with `--locked
--offline` and one test thread.
The repository `Cargo.lock` is never read for resolution or modified.

The script uses a clean Cargo target by default. After the simulations, it
forwards `--no-cheating` only to the selected root crate with Cargo Verus's
`--fwd-verus-args-to roots`; dependencies are still verified, but pinned
`vstd` is allowed to use its reviewed trusted specifications. Verus, vstd, and
the solver are therefore part of the proof TCB. The scanned production reducer
and proof module contain no `assume`, `admit`, external body, or external
specification escape hatch.

The primitive source-link kernel was checked with the official pinned macOS
arm64 release:

```text
$ scripts/verify_sumeragi_v2.sh  # official pinned release already in PATH
verification results:: 1690 verified, 0 errors  # pinned vstd dependency
verification results:: 76 verified, 0 errors    # iroha_sumeragi_core root obligations
```

Evidence for the source-link edit itself:

```text
CARGO_TARGET_DIR=/tmp/codex-wal-exact-target \
  cargo test -p iroha_sumeragi_core -- --nocapture
  64 unit/reducer/WAL tests, 7 model-trace replays, and 7 deterministic network simulations passed
CARGO_TARGET_DIR=/tmp/codex-wal-exact-target \
  cargo clippy -p iroha_sumeragi_core --lib -- -D warnings
  passed
PATH=<pinned-verus> CARGO_TARGET_DIR=/tmp/codex-wal-exact-verus-target \
  scripts/verify_sumeragi_v2.sh
  1690 dependency obligations and 76 root obligations verified, 0 errors
```

The successful run discharges the abstract reducer/WAL obligations, the
primitive-to-derived-fact kernel, and the production commit-gate obligations.
It does not turn unverified `std` collection code, cryptography, or adapter
contracts into verified code; the remaining boundary is listed explicitly
below.

## TLC trace replay against production

`tests/model_trace_replay.rs` replays a normalized 101-action TLC witness
against this crate's exact public `Reducer::step` API. The witness comes from
`SumeragiV2TraceWitness.tla` and the `LivenessSpec` corridor with a
nonresponsive view-zero leader. Three timeout votes form and durably install a
TC, all four reducers enter view one, the rotated leader proposes subject A,
and distinct three-validator Prepare and Commit quorums produce a durable
decision. The replay drives actual Persist, Sign, Broadcast, FetchBody,
StoreBody, ValidateBody, EnterView, and Apply effects; it does not call a test
reference reducer.

TLC 1.8.0 is checksum pinned because earlier TLC releases cannot emit JSON
traces. Reproduce and compare the witness with:

```text
TLA2TOOLS_JAR=<pinned-1.8.0-jar> \
  scripts/formal/check_sumeragi_v2_replay_trace.sh
TLC replay witness matches 101 checked-in production actions

CARGO_TARGET_DIR=/tmp/codex-sumeragi-model-trace-target \
  cargo test --locked -p iroha_sumeragi_core --test model_trace_replay
7 passed; 0 failed
```

The normalizer rejects unknown `ReliableNext` state deltas and non-contiguous
TLC states. The Rust trace parser additionally rejects unknown actions,
malformed fields, stale/wrong leaders, missing durable intent boundaries, and
Prepare/Commit certificate formation without three distinct delivered voters.
Adversarial production tests recover every prefix of the witness WALs and
confirm that the combined trace crosses all seven record classes. They also
cover crash after acknowledged intent, exact WAL resume, stale-generation
completion, duplicate and overlapping certificate signers, Prepare-vote and
full-high-QC timeout equivocation, and invalid body validation withholding
Prepare.

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

Run it directly or through the nightly formal workflow:

```text
CARGO_TARGET_DIR=/tmp/sumeragi-v2-chaos \
  cargo test --locked -p iroha_sumeragi_core \
  --test network_simulation \
  accelerated_100_000_block_chaos_preserves_chain_prefix \
  -- --ignored --nocapture
```

The 2026-07-13 macOS arm64 run completed all 100,000 heights in 57.53 seconds
with no conflicting decision or chain-prefix failure. This is a deterministic
long-run implementation test, not a substitute for the deductive safety or
conditional-liveness proofs.

## Current refinement model

`crates/iroha_sumeragi_core/src/verus_proofs.rs` contains one safety projection
for the production WAL and reducer rather than independent protocol examples.

The production timer/FIFO arbiter in
`crates/iroha_core/src/sumeragi/v2_core/scheduler.rs` is also source linked.
Ordinary Rust and Verus instantiate the same macro-expanded branch relation,
so absolute-timeout priority, one-slot periodic delay, FIFO debt, ordinary FIFO
service, and idle selection cannot drift through a separately transcribed
proof model. The runtime clock and task-invocation premises remain part of the
post-GST host-service contract; the choice made at each invocation is verified.

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
candidate/body work, pending and known PrepareQCs, current-view vote and
timeout pools, locally formed certificates, retained outbound controls,
signature FIFO/in-flight slot, and replay-resume flag. The gate enforces:

- at most two current-view vote pools and `2 * validator_count` entries;
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
| Pinned Verus discharge of the changed obligations | **Verified** | Official pinned workflow reports 1690 dependency and 76 root obligations verified with zero errors |

## Exact production commit gate

`crates/iroha_core/src/sumeragi/v2_core/refinement.rs` defines a small
executable transition gate. Its decision expressions are shared textually with
the Verus module by macros; the verifier does not check a separately copied
reference implementation. The normal Rust build instantiates those expressions
in `refinement::accepts`, while
`crates/iroha_sumeragi_core/src/verus_proofs.rs` instantiates the same
expressions in the verified functions.

`Reducer::step` now performs the real transition on a private clone and invokes
the gate before replacing caller-visible state. Successful, ignored, and error
paths all pass the gate. A rejected candidate returns
`ReducerError::RefinementViolation` and leaves the original reducer unchanged.

The gate receives the complete effect vector as a fixed eight-slot trace. The
bound is structural: seven retained control-message classes plus at most one
fetch/apply effect fill eight slots; a ninth fails closed. Every active slot
contains its exact vector position and two fixed-width primitive capability
keys: one requested by the concrete effect and one independently reconstructed
from the event and candidate state. The verified kernel computes authorization
by comparing every key field; callers no longer supply `authorized=true`.
Invariant truth, state equality, tag matching, busy-fence state, action class,
WAL class, continuation class, and boundary exactness are likewise derived
inside the kernel from concrete state identities, event primitives, violation
counts, and requested/granted boundary keys. The verified relation proves:

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
- StoreBody and ValidateBody are single-effect transitions caused by their
  exact predecessor completion;
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
action-exactness boolean crosses the verified kernel boundary.

The pinned verifier discharged all 76 root obligations with zero errors on a
clean target. The verification script rejects `assume`, `admit`, unreviewed
trusted bodies, and external function specifications in the package-local
reducer and proof modules throughout this crate. It also rejects reintroduction
of compressed `TimeoutIntent` validity/high-QC-match predicates and requires
the primitive-guard proof. It checks that every mapped TLA+ action name still
exists in both `SumeragiV2Core.tla` and the Verus mapping; this prevents name
drift but does not prove the independently parsed operator bodies equivalent.

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
   production commit decision is now the exact Verus-checked primitive kernel,
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
   into that identity remains in this gap.
   Closing this residual source-level gap requires Verus-compatible reducer
   collections or verified projection functions over reviewed external type
   specifications; neither is claimed here.
2. **Continuous pinned verification.** The source-link change passed the pinned
   workflow locally. The PR and nightly formal jobs invoke
   `scripts/verify_sumeragi_v2.sh` and retain its output with the other formal
   artifacts. Those jobs must succeed before their output is release evidence.
   Any syntax or solver failure must be fixed without weakening a guard or
   invariant.
3. **Volatile reducer contents.** Cardinalities, fixed protocol bounds, exact
   full-state stale/busy stuttering, durable-signature capacity, and the
   persisted-TC reset are now in the executable/verified gate. Exact key/value correspondence for
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
   actions also remain outside the executable `Reducer::step` map.
6. **Temporal liveness.** This module proves safety-style transition
   preservation only. Fair delivery, timeout-certificate progress, rotating
   honest leaders, and the post-GST commit bound remain TLAPS liveness
   obligations plus executable simulation/integration evidence.

Until all six items are discharged, the successful current run proves the
listed abstract obligations and the exact production commit-gate relation. It
does not prove every line of the inner reducer, the fact-extraction functions,
the cryptographic/filesystem trusted contracts, or the protocol liveness theorem.
