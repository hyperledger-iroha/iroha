# Parliament private ballot requirements and launch decision

Status: final requirements and launch decision, 2026-10-04. No owner decisions
remain pending. A deployable cryptographic construction has not been established;
this document does not claim a completed cryptographic protocol or release evidence.
The binding-governance mainnet launch is blocked until the construction and its
qualification satisfy these requirements. Source baseline: `cd618edb4b`.

The implementation inventory is [governance_pipeline.md](governance_pipeline.md).
This document specifies its replacement requirements. The first release has one
canonical V1 protocol: replace the unqualified timed-OVN implementation, its
schemas and clients together. Do not introduce V2, dual operation, compatibility
decoders, old-ballot fallback, or a migration prerequisite.

## 1. Final decisions

The user has explicitly required:

- No validator may hold ballot decryption keys.
- No other party may hold ballot decryption keys either. A separate trustee
  committee is not the selected architecture.
- Parliament membership is not known in advance and may initially be only a few
  people. Consequently this design does not impose a 500-person operating minimum.
- Backward compatibility is unnecessary for this first release.

The supplied assessment additionally requires post-quantum ballot confidentiality,
completion despite a voter disappearing after acceptance, and isolation of
governance failures from consensus. Public commit/reveal and penalties for
withholding do not satisfy those requirements.

Do not interpret the key restriction as permission to rename an equivalent
secret as a recovery share, mask-opening credential, or MPC state. A candidate
must disclose exactly what every party can recover, alone and in a coalition,
and obtain a decision on any departure from this restriction.

The six implementation choices are closed as follows:

| Choice | Decision |
| --- | --- |
| Ballot key custody | No validator, separate trustee, or juror may hold another voter's ballot-opening key or equivalent recovery material. A voter may know their own choice and private casting randomness. |
| Tally threshold | No threshold-decryption committee is adopted; `n-f` and `f+1` are not ballot parameters. Consensus keeps its separate quorum rules. |
| Internal exact counts | No designated party is authorized to learn exact counts. Reveal only the permitted result and context below. |
| Public verification | Require a publicly verifiable, post-quantum sound zero-knowledge proof of the complete-corpus decision. A quorum signature alone does not establish tally correctness. |
| Jury size | Use actual sealed membership, with the existing 500-seat Policy target as a cap, not a minimum; retain the 1,000-ballot resource ceiling and floor of three accepted participants. Quorum is based on the frozen actual roster. |
| Launch before private voting is ready | Do not launch binding Parliament governance or dependent mainnet capabilities before qualification. No temporary public ballot, classical confidentiality fallback, trustee takeover, or interactive all-voter finalization is selected. |

Post-quantum confidentiality, no decryption custodians, and continued contribution
after an accepted voter disappears remain hard requirements. The missing
construction is a research and implementation blocker, not a request for another
owner decision. Small membership does not relax any of those requirements.
Enrollment remains bond-only: the electorate consists of bonded accounts, with
no claim of one person per account. Later compromise of private casting material,
receipt freeness and coercion resistance are outside the privacy claim.

SCCP uses the agreed Parliament standing pause panel, drawn once per epoch
outside governance attempts. It does not gain a validator emergency authority.
Section 8 fixes that interface.

## 2. Why the assessment's threshold sketch is not the design

Giving validators, trustees, or jurors threshold decryption material violates the
selected key-custody requirement. Consequently `t = n - f`, a second tallier
committee, and exact counts disclosed to talliers are not adopted parameters.

There are also independent defects in the sketch:

1. A proof that committed shares encode one valid vote does not establish that
   their ciphertexts deliver those shares. A complete encryption-consistency
   relation would be necessary. Acknowledgements or optimistic complaints do not
   make acceptance unconditional and can require monitoring or voter return.
2. Salted hash commitments are not homomorphic. An aggregate proof must establish
   the sum over every entry of the exact frozen corpus; adding commitments is not
   an implementation of that relation.
3. A sound STARK is not automatically zero knowledge. The current FASTPQ relation
   and its parameters do not establish a general-purpose private ballot prover.
4. Threshold participation alone does not prove a correct result. An independent
   verifier needs the ballot-validity, corpus-completeness, aggregation and
   decision relations, including every public input binding.

PQKryvos is a useful public-tally-hiding reference, not an adopted construction:
it violates the selected custody requirement and adds an all-tallier availability
dependency. Tallier availability and the accepted-voter dropout guarantee are
separate properties. Its theorem does not transfer to a replacement protocol.
The [paper and artifact](https://petsymposium.org/popets/2026/popets-2026-0164.php)
are the primary reference. This document makes no universal claim that all
post-quantum voting research lacks fault tolerance.

## 3. Construction and release gate

No concrete, reviewed construction satisfying all the selected requirements has
been established in this review. This is not a theorem that such cryptography
is impossible, and it is not merely a missing benchmark or audit. Do not start
production ballot integration around a placeholder cryptographic interface and
label it a finished design.

The crypto workstream must deliver one complete candidate meeting the acceptance
conditions below. Until it does, the release verdict is **no-go for private
binding Parliament governance and capabilities that depend on it**. Independent
work on consensus isolation, retry correctness and the standing pause panel may
continue. None of that work changes the ballot release verdict.

Before calling the ballot design complete, its specification must define:

- Setup, generation, casting, acceptance, aggregation, result production,
  verification and deletion algorithms, including every party's private state.
- How an accepted ballot contributes after its author disappears, without
  another party obtaining prohibited ballot-opening material.
- Exact assumptions and a privacy experiment conditioned on the intended public
  outputs and adversarial voters' own inputs; separate soundness and liveness
  assumptions, cumulative compromise and abort/retry leakage.
- Fixed algorithms, field arithmetic, byte encodings, transcript domains,
  randomness requirements, proof geometry, bounded work and public inputs.
- The full proof that only permitted outputs are revealed. Classical pairing,
  BLS or discrete-log privacy is not a PQ construction.

Public time-lock opening of each recorded vote would reveal individual ballots.
Recovery secrets that survivors can use to open ballots would reintroduce
custody; storing ciphertext without that capability does not by itself do so.
An interactive protocol that labels a ballot accepted only after the collective
tally completes changes the acceptance guarantee and is excluded from this
release design; it does not solve the existing guarantee by definition.

The intended public result contains only the immutable roster size, public
accepted-ballot count, quorum status, decision, narrow-approval flag,
context roots and proof references. Emergency approval thresholds are evaluated
inside the decision relation without revealing an additional predicate. Participation is
public and linkable. Exact Aye/Nay/Abstain counts are not intended public outputs.
Neither an unsalted count hash nor a result root containing hidden small counts
is safe: an observer can enumerate the possible tallies.

The security claim remains conditional on an authentic finalized bulletin board.
An independently verified proof can reject a false result relative to that board;
it cannot make a wholly corrupt consensus committee publish an authentic board,
include a ballot, or remain live.

The existing BLS jury beacon is classical. Even a PQ ballot construction would
not make jury selection post-quantum unpredictable: breaking that beacon can
expose future draws and undermine capture resistance. Retaining it carries an
explicit classical unpredictability assumption. The selected scope is PQ ballot
confidentiality under intact chain integrity and that beacon assumption.
Consensus, authentication and beacon migration are separate workstreams;
using ML-DSA elsewhere does not close these dependencies.

## 4. Membership, quorum and unavoidable inference

Separate three quantities: eligible citizens, actually sealed seats `S`, and
accepted ballots `M`. Configured body targets are caps, not promised membership.
Neither validator count nor the existing Policy target of 500 determines `S`.
Core freezes the actual roster before voting; later absence never reduces its
quorum denominator. Retain the current corpus ceiling of 1,000 as a resource
ceiling pending a measured replacement profile.

Preserve the current private-ballot floor of three accepted participants as the
first-release rule, not as an anonymity theorem. Zero, one or two eligible
participants cannot silently switch to a public binding ballot. They produce
typed insufficient-capacity state for that attempt while the chain continues.
The lifecycle accommodates a three-seat binding body; its cryptographic
viability remains unresolved, and this does not establish enough population for
every body or a disjoint Confirmation Jury. Its privacy leakage must be stated
explicitly. A different floor requires an explicit policy change.

No protocol can conceal information already implied by its output. Unanimity,
known votes, coerced disclosures, or all-but-one collusion can reveal a choice
even with a large jury. The claim is confidentiality beyond the permitted result
and adversarial knowledge, not guaranteed anonymity of each voter. Receipt
freeness, coercion resistance, one-human-one-account and protection of a
compromised voter device are not provided by this ballot design.

Preserve the current decision arithmetic, using checked integers:

```text
Q = ceil(2*S/3)
A + N + X = M, with each count in [0,M] and M <= S
capacity = M >= 3
quorum = M >= Q
ordinary_approved = capacity and quorum and A > N
emergency_policy_approved = ordinary_approved and A >= Q
narrow = final_policy_approved and 100*(A-N) < 5*(A+N)
```

Abstentions count toward quorum; a tie rejects. Confirmation is required only
for an approved narrow Policy result. Its electorate excludes all sealed Policy
members, not just voters. A small population may therefore be unable to seat a
fresh Confirmation Jury. Preserve atomic `ConfirmationJuryCapacityUnavailable`
rather than overlap the juries, lower the floor or certify without Confirmation.

When `M` already proves insufficient capacity or no quorum, no private tally is
needed. A future construction must not release intermediate tallies, prefixes,
or overlapping subsets to work around a failed attempt.

## 5. Sortition waits belong to governance

The current implementation makes Parliament pulse demand consensus-mandatory in
`iroha_core::sumeragi::epoch_beacon::{required,capture}` and its producer. Daemon
startup also preflights Parliament TLE custody. Those are replacement targets,
not desired behavior.

Define two separate paths:

- Consensus-native epoch/NPoS beacon obligations retain their own Sumeragi rules.
- Parliament sortition is pending governance work. Missing Parliament material
  must not prevent block production, state replay, node startup or an unrelated
  transaction from executing.

A sortition request freezes its complete eligible snapshot, proposal content,
attempt and election identities, exact sequence, request height and one future
logical pulse slot. The target slot is computed by checked addition from the
committed delay. It cannot be changed by a relayer, timeout, key rotation,
restart, membership change or manager action.

At the slot's canonical height, retain the authenticated historical beacon
context that would authorize that pulse, including the exact active key session
and roster. A late Parliament pulse is verified against this retained context,
not against the current key pointer or a latest-pulse monotonicity shortcut.
If the slot has not yet been reached, only its deterministic resolution rule is
frozen; no caller guesses a future key. Rotation cannot change an already
resolved slot. Initial bodies still consume one complete simultaneous batch.

```text
Requested -> AwaitingExactPulse -> Drawing -> AcceptingInvitations
                                      -> Sealed | ObjectiveRosterFailure
```

`AwaitingExactPulse` has no deadline that authorizes new entropy. It can wait
indefinitely. No fallback pulse, fresh height, alternate source, cancellation
followed by an automatic redraw, or unavailable-pulse certificate may substitute
another draw. This is an explicit exception to bounded ballot-phase deadlines.

Historical custody and recovery are service obligations. Pending requests must
retain their public context, and key retirement must account for outstanding
slots; missing local custody is a governance availability fault, not a reason to
reject an otherwise valid chain or block node startup. A permanently lost pulse
can permanently stall that request. Do not promise recovery without the material
needed to reconstruct that unique pulse.

Bound pending requests and snapshot bytes at admission using committed policy.
Retain the existing 65,536-citizen and 8-MiB snapshot ceilings until deliberately
replaced. Admission limits prevent unbounded allocation, but do not guarantee
that unavailable requests eventually free capacity. Worker retries use local
timers and bounded queues; an unresolved request is not permission to manufacture
empty blocks. Consensus-visible progress uses ordinary finalized transactions.

Keep bond-retention consequences visible: an indefinitely pending election can
retain candidate bonds. Releasing them while preserving frozen eligibility is
a distinct economic-policy change, not an incidental worker optimization.

## 6. Ballot lifecycle and finality contract

The eventual construction must realize this external state machine:

```text
Prepared -> Casting -> FrozenCorpus -> ResultProduction -> Decided
    |          |            |                |
    +----------+------------+----------------+-> NoResult
```

`Prepared` is not permission to install a decryption custodian. Its exact work
must be supplied by the construction passing section 3. Remove registration/survivor/TLE phases unless
the chosen replacement proves that a particular phase is necessary.

Acceptance means a valid, authorized, uniquely counted ballot and all data needed
for its future contribution are finalized. A local submission receipt or a
promise of later delivery is only pending. A candidate may specify bounded
provisional checks before final acceptance, but must disclose their censorship
and availability assumptions and cannot disguise an all-voter dependency as
completed acceptance. After final acceptance, no acknowledgement, complaint,
later voter action or unavailable off-chain plaintext may revoke that guarantee.
Authenticated authority and eligibility checks precede
expensive proof work. The ballot binds the full network, proposal, attempt,
body, roster, ballot sequence, policy and proof-profile context.

Freeze one canonical ordered corpus from all accepted ballots in the casting
window. No manager supplies a subset. One authority has one immutable accepted
ballot per attempt. Retransmitting identical bytes is idempotent; replacing the
choice, omitting a finalized ballot or accepting a late ballot is invalid.

Use the agreed SCCP governance clock: committed millisecond deadlines anchored
at block start, transaction intents, and deterministic transitions in the
block-start pass. Windows are half-open. The first block whose authenticated
protocol timestamp reaches the casting deadline freezes the earlier corpus
before transactions execute; a ballot in that block is late. The first block
at or after the result deadline records failure if no valid result was already
finalized. Local wall clocks never choose consensus outcomes. Preserve the
binding ballot's opening anchor `s0` for the SCCP lift rule.

`governance_due(parent, height, timestamp)` feeds the shared keepalive mechanism;
G2 ranks 4–8 remain reserved for ballot phases. The keepalive work and admission
bounds must be specified and qualified with the SCCP integration. No external
transaction must land at exactly one height, and no ad hoc empty-block production
or node-local consensus toggle is introduced. Until the shared mechanism is
qualified, elapsed wall time alone is not a claim of finalized progress.

Valid proof bytes, complete available data and matching immutable context are
required before mutation. Invalid work is rejected without changing the corpus
or recording a fabricated failure. Deterministic timers terminalize unavailable
work. Claimed network outages or caller-provided failure reasons are not evidence.

Ballot failure never becomes rejection of a candidate consensus block solely
because the ballot service lacks private material. Failures have typed reasons,
an immutable evidence root, and either one legal bounded retry or a terminal
governance outcome. No `Active` attempt may remain after every legal retry is
exhausted.

A ballot retry uses the same seated jury, a fresh attempt context and fresh
private randomness. It is not an automatic jury redraw. Successful decisions
cannot be retried to obtain a preferable outcome. Proposal-content retry and
entropy budgets persist across transport retries and governance attempts.
Different semantically equivalent proposals remain a potential grinding channel;
fingerprints alone do not establish a global capture bound.

## 7. NoRoster, Confirmation and certificates

Retain the reviewed liveness reducer and planner contract: the planner derives
the complete retry generation for every active `NoRoster` body together, with
each exact next sequence and a fresh shared slot. Core rechecks the candidate
snapshot, body targets, containing height and remaining budget, and persistence
validates the successor. SCCP retries are permissionless; other proposal kinds
retain their existing authorization. The ballot redesign does not reopen that
patch or add standing-panel draws to its attempt-local sortition chain. A future
payload-minimal API is optional cleanup, not a prerequisite for the liveness fix.

Unseated candidates are released after objective `NoRoster`; members of other
sealed bodies retain their attempt-local obligations. Snapshot restoration must
reconstruct the same state and locks. Preserve public-finding quorum and split
failure rules for nonbinding bodies.

Confirmation admission is atomic with accepting the Policy result. Check fresh
capacity and proposal-wide entropy budget before committing a Policy binding
that requires Confirmation. The source baseline already terminalizes the
redraw-ceiling case; retain
`narrow_policy_at_randomness_redraw_ceiling_persists_terminal_no_result` as a
regression, including its restore assertions.

Certificates bind the verified permitted result, full proof/corpus context,
public findings, exact proposal effect and compare-and-set subject head. Remove
exact private counts from certificates, deterministic result-root preimages,
events, queries, SDK projections and snapshots, not merely UI fields.
Automatic delayed enactment, supersession and rollback-isolated execution
failure remain Core-owned. A standalone referendum cannot authorize a Parliament
proposal.

## 8. SCCP policy boundary

Pause authority remains with Parliament under
[SCCP D1/D2](sccp.md#14-decisions-binding-2026-09-26). The selected fast path is
a standing Parliament pause panel and disjoint backup, drawn once per epoch
separately from governance attempts. A pause reads the already seated panel;
nothing is redrawn during the attempt. A missing epoch pulse retains the
previous panel. Panel seating must not register an attempt-local Parliament
pulse demand or make that missing material a chain-wide obligation.

The SCCP implementation owns the panel's public endorsements, roster/quorum
validation, bounded hold and expiry. Endorsements bind the route, current head,
panel identity, nonce and incident commitment. The path is pause-only: it cannot
transfer value, raise caps, register routes, clear faults or authorize permanent
policy changes. Hold lifetime is at least the configured full-track phase
windows times one plus the permitted retry count. This finite bound excludes
unbounded exact-pulse waits; it cannot guarantee that full-track governance
finishes before expiry. There is no re-pause cooldown. The full-track lift rule
uses `s0`; expiry and head-bound replay checks remain mandatory.

SCCP consumes the 32-byte `GovernanceCertificateId::derive_v1`, the certificate's
`effect_preimage_hash`, and deterministic rollback-isolated block-start enactment.
Its 191-byte control leaf binds track, certificate ID and effect hash. Destinations
verify finalized control evidence and do not parse private-ballot internals or
hold a Parliament decryption key. Ballot proof and certificate binding changes
must preserve this interface. The standing panel does not make an unqualified
binding ballot ready for mainnet.

## 9. Implementation order and replacement inventory

1. **Isolate governance availability.** Split Parliament from native beacon
   demand and daemon preflight; retain historical slot verification; make phase
   progress deterministic and preserve the reviewed retry generation. Prove unrelated
   transactions and restart remain live while the exact pulse is absent.
2. **Discharge the construction gate.** Complete section 3 under the fixed decisions.
   Do not start a nominal PQ deployment using threshold custody, classical
   timed-OVN, a toy proof or public ballots to conceal this gap.
3. **Prototype the entire proof and data path.** Measure smallest supported
   electorates and actual 500/1,000-member cases, every admitted committee or
   participant geometry, native phone CPU/RSS/energy, verifier work, block data,
   full replay and restore. Do not describe operation counts as timings or
   estimates as device measurements. Freeze enforceable bounds before release.
4. **Replace canonical V1 surfaces together.** Retire `iroha_crypto::timed_ovn`,
   `iroha_core_timed_ovn`, Parliament-only TLE release/custody, old reducer phases,
   proof archives, session instructions/configuration, Torii routes, FFI,
   Swift/Kotlin wallet paths, CLI flows, fixtures and schema captures. Preserve
   independently required consensus beacon primitives. Kotlin owns JVM/mobile
   work; add no duplicate Java implementation. No Wasm target is introduced.
5. **Qualify one candidate before launch.** Complete proof review, adversarial lifecycle tests,
   formal model, real-peer restart/rollback, SDK parity and release evidence.
   Update public documentation in `iroha-docs` when implementation claims change.

Use Norito with explicit canonical layouts. New runtime settings flow through
`iroha_config`; consensus decisions use genesis/committed policy. Hardware
acceleration may optimize identical deterministic computations but cannot choose
a different relation, result, gas schedule or acceptance rule.

## 10. Required validation

These are obligations for the implementation, not tests passed by this design:

| Case | Required observation |
| --- | --- |
| Voter vanishes immediately after final acceptance | Its vote still contributes; no recall or plaintext recovery |
| Garbage delivery paired with a valid-looking commitment | Rejected before final acceptance |
| Missing Parliament pulse for many heights | Unrelated blocks and restart succeed; exactly the original pulse remains pending |
| Delayed pulse after beacon rotation | Original historical authorization is checked; fresh-key substitution fails |
| Missed phase trigger or result at the deadline | Deterministic single terminal outcome; no exact-height transaction dependency |
| Omitted, duplicated, reordered or replayed ballot | Complete-corpus verification rejects |
| Small electorate and known other votes | Document actual inference; do not claim an anonymity theorem |
| Narrow result without disjoint Confirmation capacity | Atomic terminal failure with no partially certified Policy result |
| Final ballot/roster/entropy retry exhausted | Terminal status survives Norito restore; bonds follow specified retention rules |
| Corrupt snapshot/cache or count-bearing result hash | Full replay rejects; public state cannot enumerate a hidden tally |
| Proof/ciphertext growth and invalid-proof floods | Admission, work, storage and restore bounds hold on measured hardware |
| Pure CPU versus supported acceleration | Identical verification and consensus outputs |

Changes to Sumeragi safety/liveness require named deterministic simulator
regressions and mutations under `specs/sumeragi.md` section 13. Real-peer tests
use a legal `3f+1` validator committee with at least four validators; this does
not impose the same size requirement on Parliament.

Primitive references do not qualify their composition:
[ML-KEM](https://csrc.nist.gov/pubs/fips/203/final),
[SHA-3](https://csrc.nist.gov/pubs/fips/202/final), and
[zero knowledge for STARKs](https://eprint.iacr.org/2024/1037).
The present FASTPQ boundary is documented in
[its protocol contract](fastpq_deep_protocol_contract.md).
