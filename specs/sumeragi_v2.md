# Sumeragi v2 consensus

Sumeragi v2 is the only live consensus protocol accepted by this release. It is a breaking
network revision: validators using another protocol version or consensus fingerprint are rejected
during admission, and an existing chain must restart from a fresh genesis rather than mix protocol
versions at one height. The first-release decoder accepts only the canonical revision-4 Norito
shape; it does not infer a missing proposal origin or fall back to a legacy vote, QC, status, or
finality layout.

Every nullable field in the authenticated revision-4 layout is an explicit slot. Height contexts,
block subjects, execution commitments, Native AMX participant leaves, timeout votes and groups,
certificate references, and proposal justifications encode either the exact value or `null`; JSON
omission never means `None`. Signed messages and their canonical signature-payload projections
reject unknown JSON fields, and shortened pre-release binary layouts fail typed decode rather than
being padded with implicit defaults.

The retained authenticated `QcVote` and `Qc` JSON objects likewise always carry
their nullable `highest_qc` slot, using `null` for `None`; those objects and
`QcAggregate` reject unknown keys. Their Norito encoding is unchanged, but a
shortened layout which omits the slot is not accepted.

The live `BlockMessage` wire enum contains only canonical `V2` messages, the independent lane-local
message family, and authenticated Kura replica adverts. Its retained numeric gaps are reserved,
not compatibility slots: every former global-v1 block-message discriminant fails typed decode and
the pre-allocation ingress classifier. The former `NetworkMessage::SumeragiControlFlow` evidence
tag likewise fails before ingress. There is no archive decoder on the live network type and no
fabricated message used to turn a malformed frame into an infallible decode result.

The durable `Evidence` model is likewise current-only: it contains one
`SumeragiV2EquivocationEvidence` value with the frozen height context,
roster-ordered BLS proofs of possession, and an exact conflicting signed pair.
Retired global-v1 kind/payload records (double-vote summaries, invalid-QC or
invalid-proposal claims, and censorship receipts) fail Norito decode; the node
does not reconstruct a historical roster or silently upgrade them.
The typed `SumeragiV2EquivocationEvidence` JSON object is closed and requires
all three of its fields; its nested current consensus types retain their
explicit nullable slots. `Evidence` itself remains a binary-only wrapper.

The persisted `EvidenceRecord` wrapper is exact as well: its penalty flags,
nullable penalty heights, and nullable consensus-admission height are all part
of the first-release binary layout, and a shortened prefix receives no defaults.
It is also binary-only; Torii constructs the bounded audit JSON projection
explicitly instead of exposing a second full-evidence object schema.

The protocol has one executable decision authority, the package-local
`iroha_core::sumeragi::v2_core::Reducer`. Networking,
signature verification, block construction, deterministic validation, payload storage, Kura, and
telemetry are adapters. They may perform work concurrently, but only the serialized reducer may
change consensus state or authorize a signature.

## Frozen height context

Every global consensus message is bound to a `HeightContextId`. The hashed context contains:

- chain and protocol identifiers;
- height, epoch, and the previous height's CommitQC;
- the ordered equal-vote committee;
- the complete Nexus/AMX consensus context, V1 process-local execution-policy identity, and DA
  layout, including the canonical active-lane lifecycle map of
  `(lane_id, incarnation, activation_height)` tuples sorted by lane id;
- the epoch leader seed and quorum policy.

The frozen voting committee has exact `n = 3f + 1` geometry for `1 <= f <= 10`:
4, 7, 10, …, 31 validators. This is not a local tuning knob. The same bound
limits proofs of possession, authenticated vote/equivocation retention,
certificate work, and per-view memory on every peer. A smaller, larger, or
non-`3f + 1` context is structurally invalid before consensus opens.

The context for height `h` is derived only from the finalized state at `h - 1`. A reconfiguration
committed at `h` may activate at `h + 1`; certificates from the old context cannot act in the new
one.

The next context and its view-zero proposal bind the parent CommitQC by semantic finality identity:
parent context, height, immutable proposal-origin round, Commit phase, block subject, and execution
commitment. The QC finality view, aggregate bytes, and signer subset are excluded because the same
safe proposal can acquire valid CommitQCs in multiple views or through independently collected
quorum subsets. The successor constructor verifies the locally carried QC against the exact durable
parent context and artifact before opening the height, and the adapter retains that parent context
and its proofs of possession so every alternate QC carried by a view-zero leader is also
cryptographically verified before reducer admission.

Genesis carries this projection as `sumeragi_v2.nexus_amx_context_hash`. The canonical projection
binds the validated lane and dataspace catalogs, routing, staking, fees, AXT, lane fusion and
autoscaling, DA policy, deterministic AMX budgets, the ordered active public-lane validator
records, and the complete retained lane-incarnation lineage. The lineage is keyed canonically by
lane ID and includes every active or retired lane's generation, incarnation, and activation
height, because recreation derives the next incarnation from that history. Local paths, worker
counts, caches, and telemetry are deliberately excluded. An unsigned
deployment template whose final roster is not known carries the config-only projection. `kagami
genesis sign` stages the complete genesis transaction, including validator activation, then
replaces the projection and consensus fingerprint with the exact staged values before it emits a
signed block. Generation-zero lane incarnations use the domain-separated `static:v2` projection of
the canonical lane catalog and lane definition; they deliberately exclude `NetworkId`, because the
exact `NetworkId` is the final signed-genesis header hash and including it here would require a
cryptographic fixed point. Later configuration and lifecycle incarnations remain network-bound,
and every live height context authenticates the exact `NetworkId` separately. After signing,
Kagami re-stages the final signed body under that final `NetworkId` and refuses to publish it unless
both context commitments reproduce exactly. A template is therefore not a deployable commitment
by itself.

Genesis also carries `sumeragi_v2.execution_policy_hash`. This is a separate, versioned identity
for boot configuration which is read from `State` during transaction admission, transaction and
trigger execution, block validation, replay, or snapshot recovery. Genesis signing derives it from
the fully staged state; the hash is then part of every `HeightContextId`, peer admission capability,
persisted finality artifact, and authenticated snapshot bootstrap. A node fails closed when its
locally loaded policy differs from the signed or recovered identity. There is no default-value or
legacy-field fallback.

The V1 inventory is:

| Policy family | Bound decision inputs |
| --- | --- |
| Pipeline | access prepass, overlay instruction/byte/chunk bounds, gas asset/rate routing, IVM cycle and decode bounds, quarantine count/cycle bounds, in-IVM query bounds, deterministic AMX budgets |
| Cryptography | default hash, allowed signing algorithms and curve IDs, SM2 distinguished ID |
| Oracle | history retention, reward/slash/bond economics, governance stage SLAs and thresholds, Twitter-binding identity and TTL/update guards |
| Fraud admission | enablement, required risk band, whether missing evidence is permitted, authenticated attester identities |
| Governance | ballot/tally keys, bonds and slashing, JDG and runtime-upgrade provenance, citizen/viral/SoraFS policy, voting thresholds, terms, committee sizes, and pipeline deadlines |
| Content | bundle/file/path/retention/chunk limits, publishing allow-list, cache and immutability effects, authorization mode, DA stripe defaults |
| Settlement | deterministic router windows and guard rails; offline-wallet capability is compiled universally and has no enablement, asset catalog, dataspace enrollment, or deployment-readiness input |
| Existing canonical sub-policies | Nexus plus loaded lane-manifest/compliance digests, ZK policy (including process-local SCCP admission/work limits) |

Only result-preserving operational choices are excluded: worker and cache sizes, parallel/GPU
backend selection, exact signature batching, prover thread count, tracing and telemetry, fraud
service endpoints and transport timeouts, content gateway quotas/SLO/PoW, and filesystem paths.
Chain-authoritative values such as the governed SCCP registry are also outside this immutable boot
identity: they are committed through world state and the existing confidential-feature/block
commitments, so legitimate on-chain evolution does not masquerade as local configuration drift.
Quarantine consensus acceptance is bounded by cycles and gas and never branches on host elapsed
time; there is no wall-clock validity-budget setting. Classification is also consensus-canonical:
only the exact Norito JSON boolean `true` at the `quarantine` key in authenticated transaction
metadata selects the lane, with no process-local classifier hook. Elapsed timings are observational
telemetry only. Changing an excluded value may affect throughput or diagnostics, but it cannot
change admission or ledger effects; if a future implementation makes one decision-affecting, it
must first be promoted into a new canonical policy projection.

The public Nexus sample remains intentionally non-deployable until an operator supplies the
canonical Nexus XOR asset-definition ID. Generation fails closed without
`--nexus-xor-asset-definition-id`; the Taira XOR identity must never be reused as a substitute.

Permissioned and NPoS modes both assign one consensus vote to every committee
member. NPoS stake selects and backs the finalized committee; it never weights
Prepare, Commit, timeout, or certificate votes. A wire certificate contains
exactly `2f + 1` distinct validators. A local collector may retain more valid
votes, but it deterministically projects the first `2f + 1` validators in
canonical signer order; certificate supersets are rejected. Observers are
never committee members and do not contribute to quorum.

Leadership rotates through the entire frozen roster:

```text
start        = H(epoch_seed, height) mod roster_len
leader(view) = roster[(start + view) mod roster_len]
```

Stake controls NPoS eligibility and backed committee seats, not vote weight or
leader frequency. The remaining seats use the finalized epoch seed's
deterministic PRF order.

NPoS epochs contain exactly the configured `3,600` finalized consensus heights.
Proposal production remains workload-driven. The shared semantic-work gate
admits a resultless or wire-empty carrier only when it proves state-derived
ledger-clock progress or authenticated external, autonomous, or other internal
work. A genuinely idle, semantic-work-free body is rejected before voting and
finality.
The old committee authenticates the complete next-epoch roster, proofs of
possession, equal-vote quorum, seed, and end height in the terminal height
context before the successor committee can activate. A configured validator
which is absent from that successor roster participates in the new global
height only as an observer and cannot sign successor-global consensus traffic.
Lane authority is independent: the node may sign bounded unfinished
predecessor sessions and current-height Nexus lane sessions only when that
session's exact frozen descriptor names its key. A current lane committee need
not be a subset of the successor global roster. This lets old lane quorums
finish and independently pinned lane committees keep producing across an epoch
boundary without granting removed validators successor-global votes or
requiring global-roster overlap.

## Shared runtime configuration

The production adapter obtains its immutable settings from
`Sumeragi::v2_config(block_cadence, mode)`. The cadence and mode arguments come from the signed
genesis/current `HeightContext`; the legacy local `consensus_mode` selector cannot override them.
The returned `SumeragiV2Config` is a versioned Norito value containing only fixed-width integers:
protocol and mode, cadence, the one round deadline and derived one-fifth retransmission interval,
finite transaction/body/queue/ready-work bounds, consensus-key policy, and (in NPoS mode) the
epoch, VRF, election, and reconfiguration policy. Its domain-separated hash is the canonical
shared-config fingerprint used by the adapter, peer gate, status API, and rollout checker.

Peer admission also checks a distinct, domain-separated genesis fingerprint. Its canonical Norito
projection contains the chain and protocol, genesis-selected mode, cadence, one round timeout,
finite block bound, signed DA/Nexus context, and (for NPoS) the epoch seed plus VRF, election, and
reconfiguration inputs. Legacy collectors, phase-specific/adaptive timeouts, the old global-DA
boolean, and mutable BLS-domain strings are excluded. The former full-parameter fingerprint is
available only to archival tooling and is never an input to live v2 admission.

The peer handshake carries the canonical `ivm::gas::schedule_hash()` as an
independent execution capability. Admission requires an exact match before a
peer can join the online set. A binary whose opcode, host-formula, staged-phase,
or syscall metering differs therefore cannot participate silently even when its
wire protocol and genesis fingerprint are otherwise identical.

Node role is deliberately outside this shared fingerprint because an observer must be able to use
the same chain configuration without voting. Worker counts, local scheduling budgets, paths,
caches, and telemetry are also local-only. The DA encoding, chunk geometry, maximum chunk count,
and Nexus/AMX commitment are already signed into the `HeightContext`; mutable RBC configuration is
not a second source of those values.

The signed DA layout must describe every payload up to its advertised maximum.
Revision 4 requires RS16 with non-zero data and parity shard counts and an even
chunk size; `Plain` is structurally invalid. `max_chunk_count` must accommodate
the complete encoded stripe count for `max_payload_size_bytes`. Protocol
admission caps the signed layout at 16 data shards, 16 parity shards, 32 total
shards, 256 KiB per chunk, 16 MiB per canonical payload, and 1,024 encoded
chunks. The complete encoded maximum, including parity, is capped at 32 MiB;
pre-manifest orphan acquisition uses the same byte ceiling. These bounds keep
RS16 matrix work and acquisition memory finite even for adversarial but
arithmetically valid wire values. Genesis and every derived height context
apply the same checked geometry validation, so an unusable layout is rejected
before height one rather than stalling proposal production.

A native AMX route plan is capped at 256 total legs including its single coordinator, so a receipt
can carry at most 255 participant legs. Request validation, block admission, and certified merge
replay apply that same coordinator-inclusive boundary; a 256-participant receipt is rejected before
committee or aggregate-signature work.

For authoritative NPoS validity, `sumeragi_npos_parameters` must exist in committed world state.
Genesis builders emit it for NPoS chains. VRF scheduling, evidence attribution, and slashing delay
are read from that committed snapshot; customized node-local fallback values cannot change a
candidate or follower result. The reserved parameter ID rejects malformed payloads, zero-length
epochs or VRF windows, windows that reach or exceed the epoch boundary, and zero evidence,
activation, or slashing bounds. At least one finalized pre-boundary block is therefore reserved
after the reveal cutoff. An NPoS v2 node that cannot load the committed snapshot fails closed.

### Authenticated NPoS VRF records

Authoritative v2 persists the exact signed commit and reveal messages behind every
`VrfParticipantRecord`.  Each proof includes the signed epoch, signer index,
commitment or reveal, and canonical signature bytes.  Candidate validation reconstructs the
domain-separated `VrfCommit`/`VrfReveal` preimage and verifies it against the signer at that index
in the frozen `HeightContext` roster.  A summary value without its matching proof, a proof replayed
from another chain/epoch/index, a commitment/reveal mismatch, duplicate signer, or non-canonical
ordering makes the candidate invalid.

At an epoch boundary, context construction requires the exact authenticated current-epoch record
already present in finalized pre-state. It revalidates the epoch, frozen seed, roster, window
geometry, canonical participant order, signatures, VRF proofs, and observation heights before
mixing the canonically signer-ordered on-time reveals into the immediate successor seed. Missing or
inconsistent pre-state fails context construction. Boundary-height and late reveals remain useful
for penalty accounting but cannot alter the already frozen successor seed.

The recorded first-observation height is not covered by the validator signature.  It is therefore
validated as monotonic admission metadata: a proof absent from committed pre-state must first
appear at the candidate's own height and in the active commit, reveal, or late-reveal phase.  A
candidate cannot backdate a proof or introduce a commitment and a late reveal together.  Existing
proof bytes are immutable across later record extensions.

The first block of every NPoS epoch must carry exactly one current-epoch record. That record freezes
the epoch length and commit/reveal deadlines from pre-block world state before any parameter update
in the same block executes. Later valid epoch/window updates are stored in world state for the next
epoch boundary and cannot move the active windows or make the next height unconstructable.

At `height == epoch_end_height`, every NPoS candidate must carry exactly one finalized seal for the
current epoch. Its seed, epoch length, window deadlines, roster length, boundary height, signed
participants, and exact absence partition must match the frozen context and committed pre-state.
At heights other than the epoch start and boundary, a candidate may omit the record or carry one
unfinalized monotonic extension. This prevents fabricated participant entries from entering the
committed observation history while retaining bounded proposal attachments.

Reveal inclusion is not yet backed by a quorum-certified accumulator. Consequently the current
release does not mix a proposer-carried reveal subset into consensus randomness: next-epoch seeds
advance by the fixed hash chain `H(current_seed)`, so including or omitting any valid reveal cannot
grind the leader schedule. Signed reveal proofs remain bounded telemetry and future beacon inputs.
Reveal mixing may be enabled only with a consensus-certified inclusion root and deterministic
completeness rule.

Commit-without-reveal and no-participation sets are proposer-observed absence summaries, not
quorum-certified evidence. A boundary proposer can omit an observation, so those sets are retained
only as diagnostics and can never authorize jail or slash actions. The next block deterministically
marks the absence record processed without changing validator status. Economic penalties require
self-contained, signature-verified equivocation evidence admitted by a prior committed block.

V2 configuration validation fails closed if transaction or body bounds are absent, any adapter
queue is zero, or a retired mode flip, phase-specific timeout, fast-finality cap, adaptive
resilience path, or consensus fault-injection switch is supplied. Collector routing, global RBC
state, adaptive pacemaker settings, and missing-QC recovery parameters are not part of the runtime
configuration projection. Older fields with those labels, where still exposed by observability
schemas, are non-authoritative counters and may remain zero. Validators retransmit
correctness-critical control messages to the whole voting roster.

Authenticated proposal/vote/timeout semantic keys are retained for the current view plus one full
roster rotation. Capacity is derived from the frozen roster at four keys per validator per retained
view (proposal, Prepare, Commit, timeout), rather than a smaller fixed table. Current-view traffic
may evict the oldest prior-view evidence key before backpressure, while complete QCs and TCs use a
separate reserved progress queue. Thus a valid old-view flood cannot consume the honest slots
needed to form the current QC or TC.

At the local P2P scheduler, authoritative v2 proposals, votes, QCs, timeout votes/certificates,
commit-certificate responses, and VRF commit/reveal messages use `ConsensusSafety`. This tag is
derived after decode and is not part of the wire format. It has independent bounded network-actor,
per-peer, encrypted-frame, deferred-send, inbound-dispatch, and relay-subscriber queues. Auxiliary
lane traffic uses `Consensus`; Torii proxy and streaming-control traffic use `Control`. Genesis is
a local trust-root input and has no peer request/response route. An auxiliary or control-plane flood
therefore cannot consume safety admission, while bounded burst scheduling still gives repair
traffic a turn.

The final Sumeragi handoff repeats that source isolation instead of collapsing authenticated
traffic into one FIFO. Each frozen-roster validator has a bounded ingress lane, authenticated
non-validator sources receive on-demand lanes capped by
`sumeragi.queues.authenticated_non_validator_sources = H`, and anonymous traffic retains its own
persistent lane. Each validator owns an ordinary first-message slot, an ordinary Progress slot,
a certified-fence-escape slot, a distinct signer-bounded TimeoutVote slot, and a
TransportCompletion slot. Each materialized authenticated non-validator lane owns a generic slot,
a certified-fence-escape slot, and a TransportCompletion slot; the anonymous
lane owns only the generic and TransportCompletion pair for a non-empty roster. Configuration must therefore provide at least
`5 * roster_len + 3 * H + 2` slots, or `3 * H + 1` for the no-roster diagnostic geometry, and body
bytes must isolate `(roster_len + H + 1) * body_source_bytes`. A semantic duplicate carrying an
alternate authenticated reply route attaches to its existing request before the new-lane `H` gate
is evaluated. Exact-output route capacity remains the independent effective
`network.max_total_connections` value `R`; root validation resolves any configured lane profile
before deriving `R` and rejects `H > R`. Authenticated non-validator lanes are removed when empty. A busy lane may borrow only
capacity which preserves those outstanding reservations, and the continuation potential cannot
increase when one item is serviced. Dequeue rotates one message per non-empty source. Height
rollover closes the queue, discards messages owned by the old immutable context, installs the
successor roster, and reopens only after WAL replay. A Byzantine validator or a swarm of
non-roster identities therefore cannot indefinitely exclude an honest retransmission at the
production ingress boundary.

A proof-carrying `LaneHistoricalRecoveryResponse` is the only completion whose
semantic origin may belong solely to a predecessor roster. It consumes that
authenticated hop's bounded `H` TransportCompletion owner and bytes; the lane
adapter accepts it only for an outstanding exact request and a responder named
by the frozen historical CommitQC or READY certificate. Other DA completions
still require a current-roster semantic origin.

Removal from that fair ingress is conditional on the exact next queue. A reducer-directed head
remains in its source lane unless the single runtime FIFO has room in that payload's Normal or
Progress prefix. TimeoutVote, Commit votes, QCs, TCs, payload chunks, certified-body requests and
responses, and Commit-certificate requests and responses all receive outer Progress ownership.
TC, direct CommitQC, and a Commit-certificate response carrying CommitQC additionally use the
isolated certified-fence-escape count and byte owner; TimeoutVote keeps its distinct signer-bounded
reservation. A commit-certificate response is also
charged to runtime Progress because successful authentication unwraps it into a CommitQC. A
current-height certified-body request remains queued until the ordered I/O FIFO has room in its
authenticated service prefix. One dequeue attempt examines at most one head from every ready
source. A source whose head cannot enter its downstream prefix rotates without losing that
message, so a blocked Normal head cannot hide later progress that still fits its reservation; a
full unsuccessful scan restores the ready order. The capacity check, fair rotation, and removal
occur under the ingress lock, and the runner is the sole downstream producer, so an admitted head
cannot become a pop-then-drop race between queues.

Restart keeps a productive leader-wire lifecycle's immutable logical token but not its volatile
queue position. Its exact retransmission therefore receives a fresh physical carrier after the
restored admission high-water mark. Equality between token admission and carrier admission
identifies a fresh lifecycle and receives no recovery authority. A direct authenticated TC or
CommitQC whose token is strictly older than its carrier may enter the isolated certified prefix;
the already-frozen absolute timeout still executes first. A Commit-certificate response has no
productive leader-wire token of its own and follows its ordinary authenticated projected-CommitQC
path.

TimeoutVote service uses a separate finite producer episode. The absolute timeout first persists
and dispatches its exact `BeginTimeout` owner `T`, freezing one TimeoutVote slot for every
authenticated roster source. Every restart-dormant token is restored into the shared ordinal source
before `T`. A pre-cut owner below `T` is rank descent: its original carrier has token admission equal
to physical admission and strictly below the frozen physical cut; a restored token remains below
that cut while its new carrier is at or above it. A first post-cut token above `T` may instead
increase the collected-vote count once for its frozen source slot; its equal token/carrier admission
is at or above the cut. This replenishment is not itself protocol progress. Runtime admission
projects those cases as exact `0→1` first admission, `1→1` same-owner coalescence, or `0→0`
non-candidate service, and rejects a different token or carrier for an occupied slot before reducer
refinement. A best-effort retransmit already frozen below `T` is atomically superseded by the
successful timeout transition. Fresh post-timeout retransmits remain enabled above `T` but cannot
take precedence over the finite vote episode. TimeoutVotes stay in ordinary Progress capacity,
receive no certified-fence credit or signature-fence authority, and retained terminals prevent
resurrection. The inclusive `<= T` completion prefix separately admits the timeout signer's callback
while an older transport response is retained; completions from fresh roots above `T` remain
excluded.

The serialized runtime FIFO has physical capacity `C`, Progress reserve `P`, and Completion reserve
`K`. Ordinary Normal work is bounded by `C-P-K-1`, ordinary non-Completion work by `C-K-1`, and all
ordinary work by `C-1`. The remaining physical slot is credited to at most one queued authenticated
TC, CommitQC, or `CommitCertificateResponse` carrying a CommitQC root; additional certificates consume ordinary Progress capacity. The credit is
derived from the immutable queued command, so a certificate which arrives before ordinary work
cannot make later Completion work lose any of the `K` reserved positions. The same accounting is
used by direct Completion admission and by the reserved `BodyAvailable` handoff. Publishing an
unmaterialized `BodyAvailable` reservation atomically retires every queued Proposal which conflicts
with that canonical body; capacity is never computed against the post-retirement queue while the
conflicting commands remain live.

If a process stops after the stage-7 `BodyAvailable` producer reservation is durable but before its
runtime handoff, restart restores the same logical lifecycle key and first-admission ordinal without
claiming a dormant FIFO position: the body bytes are still volatile. A fresh exact `FetchBody`
reconstruction reacquires those bytes, and its `BodyAvailable` completion spends exactly one new
physical Completion position. An exact retry reuses that unpublished position, and the completed
handoff removes the durable stage-7 record so a later restart cannot resurrect the consumed stage.
Specifically, claiming the restored reservation marks the pending handoff as volatile: it neither
stores a durable producer terminal nor advertises durable terminal evidence. Acknowledgement keeps
the exact process-local terminal, persists removal of the durable producer reservation, and only
then clears the restored-handoff metadata. A second same-height restart therefore has no stage-7
producer record to reopen.

Terminal supersession before reducer service uses the same persistence boundary. Before an
unpublished or materialized restored `BodyAvailable` owner can be removed, the runtime extracts
one exact `(causal lifecycle key, first-admission ordinal, producer stage 7)` tuple from the sole
serialized owner. The adapter accepts only the matching Reserved, volatile-body record which is
present in its process, durable, and restart-dormant indexes and has no deferred or pending-handoff
alias. It removes all three records and persists the new producer table before the runtime releases
the Completion token or queued command. A persistence failure restores all three in-memory records
and leaves the volatile runtime owner intact while the runtime fails closed. This ordering applies
to exact `BodyAvailable` retirement and whole body-pipeline retirement. The same transaction covers
producer reservations attached to Busy-deferred work: every exact process and durable alias is
validated, the complete producer-release batch is persisted, and only then may the deferred queues
lose their owners. Duplicate addresses, a missing Busy owner, or a failed store leaves both the
queue and all producer maps unchanged. None of those paths can leave a crash window in which the
runtime owner is gone but a second restart resurrects its producer.

A restored `FetchBody` may also become terminal before it has bytes with which to reserve a
`BodyAvailable` token. Restart deliberately gives that Fetch a fresh physical runtime lifecycle, so
its volatile ownership cannot claim the old producer key. After the runtime proves the exact Fetch
effect binding, the adapter resolves its persisted route-neutral `(context, height, round, subject)`
coordinates; a supplied manifest must reproduce the complete serviced-candidate identity, while a
manifest-less fetch may select only one unique dormant stage-7 record. The adapter persists that
record's retirement before the pending Fetch or certified-request owner is released. The absence of
an unpublished Completion token is therefore not permission to leave its restart-dormant parent
behind.

Restart performs an additional producer-frontier reconciliation after safety-WAL replay and before
runtime capacity is installed. Current-view Reserved producers remain eligible. Older-view Reserved
records are persistently pruned unless they are one of the four exact body-pipeline stages
(`LocalProposalReady`, `BodyAvailable`, `BodyStored`, or `ValidationCompleted`) for the durable
protected lock's proposal view and subject. Future-view Reserved records or inexact
process/durable/dormant aliases fail closed. This cut is restart-only: a live `EnterView` cannot
erase an older producer while its process owner may still be completing the explicit handoff.

A certified-body response retained after retryable backpressure owns one finite certificate-escape
episode. The episode starts `Fresh`, or `Charged` when a certificate already owns the physical
credit. `Fresh` may admit one new authenticated TC/CommitQC root (including the exact response
wrapper); `Charged` drains the already-owned
certificate prefix without admitting a replacement, and becomes `Spent` when the last credit
disappears while Completion admission remains closed. `Spent` cannot reset until the exact response
retires. The runner still services one already-owned pacemaker root on every retry, but fresh
network certificates cannot replenish the same physical credit forever and starve the retained
Completion handoff.

Trusted completion admission has a matching finite invariant. The shared configuration bounds
outstanding asynchronous effect work by the runtime completion reserve. The ordered I/O worker has
one physical FIFO with hierarchical total-length admission: authenticated certified-body service
can occupy only the configured auxiliary prefix, consensus signing/storage/validation/application
can also use a reserved consensus suffix, and one final slot is reserved for trusted candidate-load
and cleanup control. The worker always consumes the FIFO head, so the reservation cannot reorder
earlier work, while a Byzantine request flood cannot make later consensus work inadmissible. Its
completion channel covers the entire physical FIFO. The runner removes an I/O or reconstruction
completion only while the serialized reducer FIFO has an exact free completion slot and alternates
the two producer classes when both are ready. A finite simultaneous fsync/signature/validation
burst remains backpressured in its bounded producer queue; it cannot overflow the reducer FIFO or
turn valid work into a restart.

The shipped Taira profile sets `role = "validator"`, a 1,000 ms genesis cadence, a 10,000 ms round
deadline, bounded 96-transaction/16 MiB bodies, and the finalized NPoS stake-snapshot roster. An
observer changes only `role = "observer"`; it must not change the shared fingerprint.

Only a voting peer with a BLS-Normal consensus key opens the durable Native AMX sign-once guard.
Observers and other non-signing peers do not create or require that filesystem journal and can run
on non-Unix platforms. A voting validator still fails closed on a platform that cannot provide the
secure guard; there is no in-memory or insecure signing fallback.

## Round protocol

The global protocol has only Prepare and Commit votes.

Three round identities are deliberately distinct. The reducer owner tag
`(height, view, generation)` names the process-local lifecycle allowed to start or complete a
transition. A signed `proposal_round` names the immutable origin of the proposal bytes, manifest,
header, durable body, and validation receipt. A vote or QC `round` names the round in which that
Prepare or Commit evidence is certified. The lifecycle owner is never substituted for either wire
round. Prepare requires `proposal_round == round`; Commit requires the same context and height and
`proposal_round.view <= round.view`. Both rounds are authenticated by every Vote and QC signature.

1. An unlocked expected leader broadcasts a proposal and payload manifest. View zero is justified
   by the parent CommitQC. A later unlocked view may carry the previous view's TimeoutCertificate
   only when that certificate selects no PrepareQC. A Timeout-justified Proposal carrying any
   selected PrepareQC is structurally invalid. Validators instead install that certificate through
   the ordinary durable timeout path, retain its exact proposal origin as their lock, and commit
   that lock directly; they do not create a new proposal origin for equal block bytes. The Proposal
   signature authenticates the current leader and the proposal round.
2. A validator reconstructs the complete canonical body, checks all chunk and payload hashes,
   validates it deterministically against the certified parent, and stores it durably under the
   exact proposal origin. Only after the body and a Prepare sign-once record are durable may it
   release a Prepare signature; the Prepare vote and PrepareQC have that same round as their
   `proposal_round`.
3. A PrepareQC certifies validity and availability. Its honest signers have the exact body in the
   durable payload store and must serve it after restart.
4. On a valid PrepareQC, a validator with the exact validated body atomically persists its lock and
   a Commit sign-once record. The Commit vote uses the active finality round while retaining the
   PrepareQC's immutable `proposal_round`; only the persistence acknowledgement releases the
   signature. The exact TC-promoted lock case described below uses the same
   persistence-before-sign ordering after body recovery and validation. A received Commit vote
   enters a volatile pool only when its proposal origin and subject match the exact durable lock.
5. A CommitQC decides the subject. The node persists the decision before applying or publishing it.
   Its certification round may be later than its proposal origin. The canonical block header's
   view-change index, body fetch, deterministic validation, and application all bind the CommitQC's
   `proposal_round`, not its later certification view. A node missing the body records
   `PendingApply`, fetches that exact origin from certified signers, validates it, and does not vote
   at the next height until application completes.

Successful exact-artifact verification also installs a private transient capability on the
committed lifecycle object. Production State application accepts only that capability and derives
the commit-topology transition from its frozen `HeightContext.roster`; ordinary, signer-only, and
unchecked commits cannot mint it. Restart recreates the same capability only after Kura has
cryptographically verified the current V2 artifact and rebound the exact result-bearing block.

A validator prepares a proposal only when it is unlocked or when a duplicate proposal has the
exact same origin and subject as its lock. A TC-selected PrepareQC may supersede a lower lock, but
the selected proposal is then committed directly. Equal bytes at another view are a different
origin and are rejected instead of being re-proposed. Locks never move to a lower certificate.

## Certified view changes

The round deadline is absolute from view entry and is not reset by partial progress. The default
public-network profile uses a ten-second deadline and retransmits critical control messages every
two seconds.

On expiry, a validator persists one `TimeoutVote(height, view, highest_prepare_qc)`. That durable
record closes the ordinary voting path for the view: the validator can no longer create a Prepare
vote or an arbitrary Commit vote for it. The validator remains in the view until it receives a
valid TimeoutCertificate. The sole historical-vote exception is the exact TC-promoted locked Commit
reconstruction below; a timeout intent by itself does not authorize it.

After the TC is durably installed, the reducer discards the closed view's active individual-vote
pools. The adapter keeps semantic fingerprints and an equivocation-reported bit in a map separate
from delivery deduplication for one roster rotation, then evicts them deterministically. Complete
authenticated conflict pairs are not yet persisted for penalties. This bounded history does not
invalidate a complete earlier-view CommitQC, which remains admissible and decisive.

A current-view Commit which arrives before the matching `LockAndCommit` acknowledgement is ignored
recoverably. Once that acknowledgement makes the later-finality same-origin intent durable, the
adapter advances the locked-Commit consumer epoch and may admit the same exact authenticated vote
once. The reducer then removes every older Commit pool for that proposal origin before releasing
its local current Commit signature; it never removes the old reconstruction source before
successful persistence. Retained outbound Commit
control continues to serve peers, but cannot alone reconstruct the sender's local pool because
broadcast does not loop back to the sender.

A TimeoutCertificate contains individually verifiable votes, optionally aggregated in groups that
reported the same high QC. Groups are canonically sorted, their signer sets are disjoint, and their
union contains exactly `2f + 1` committee members. Local formation projects the canonical first
`2f + 1` signers before grouping them. The deterministic maximum valid PrepareQC is selected across
every group. A TC is persisted before entering the next view and any validator may form and
rebroadcast it; there is no correctness-critical collector.

Another valid TC for the immediately preceding timed-out round may arrive after view entry with a
PrepareQC that the first quorum omitted. The node persists this certificate only when its selected
Prepare origin is strictly newer than both the installed highest PrepareQC and the active lock. It
then upgrades the lock without advancing the lifecycle view again. A same-round replacement with
no high QC, or with an equal or lower Prepare origin, is rejected as a replay regression.

Installing the TC may make its selected highest PrepareQC the node's active durable lock even when
that node never created a Commit intent for the PrepareQC's round. TC installation does not itself
authorize a signature. The reducer first recovers, stores, and deterministically validates the exact
locked body at the PrepareQC's `proposal_round`. Validation may then append one `LockAndCommit`
whose Commit vote is signed in the active finality round but retains that historical proposal
origin; only the successful WAL acknowledgement releases the local Commit signature. Every other
proposal origin or subject remains behind the timeout fence, and this path never creates a Prepare
vote or a replacement proposal.

Validation can finish after the active finality round's `TimeoutIntent` is already durable. In that
ordering the reducer does not append or sign a Commit in the closed view. The exact acknowledged
current-view timeout is instead a typed recovery witness for the validated historical lock. The
next installed TC creates a new owner generation, reacquires and revalidates the same immutable
proposal origin, and may append `LockAndCommit` in the new open finality round. Stale, wrong-view,
wrong-signer, volatile-only, or non-exact timeout state is not a progress witness and still fails
closed.

WAL replay enforces the same boundary in record order. A historical `LockAndCommit` is valid only
after an installed TC has advanced the view while the exact same PrepareQC is the active lock.
Without that prerequisite, or when the proposal origin or subject differs, replay fails closed. An
`InstallTimeout` without the later exact `LockAndCommit` therefore resumes body reconstruction and
validation rather than inferring a Commit signature from the certificate alone.

An earlier-view CommitQC remains decisive even when its final shares are assembled or delivered
after a TC. The timeout fence prevents arbitrary new honest votes in the closed view. Its one exact
historical-Commit exception cannot change the installed lock. For every old value which can still
reach a Commit quorum, quorum intersection now proves the exact disjunction the implementation
supports: either the target TC's selected high directly protects that proposal origin and subject, or an
honest TC/Commit intersection signer has a non-strict timeout/Commit pair whose exact Commit keeps
durable installed-TC authorization. The target TC may have been formed before that late Commit, so
claiming that its own high must always protect the Commit would be false. The formal operator
`TCProtectsOrInstalledTcAuthorizesPotentialCommit` states the corrected safety property.
Installing a TC never lowers or clears a local lock: only a strictly higher PrepareQC can release it
for another subject. Because timeout votes transport that full QC to every voter, an omitted lock
becomes known to the honest quorum and is selected by a subsequent TC after GST without depending
on a correctness-critical collector.

## Payload availability

PrepareQC replaces global RBC READY and DELIVER as the consensus availability certificate. INIT,
deterministic erasure chunks, repair, certified body fetch, and block sync remain transport
mechanisms. Authenticated partial shards are retained only in bounded memory and are reacquired
after restart; once reconstruction succeeds, the exact canonical body crosses the durable body-store
boundary before validation or voting. This avoids a durability barrier per shard without weakening
the full-body voting gate. Chunk signatures bind epoch, height, view, context, parent, subject,
payload root, encoding, chunk index, and total chunk count, preventing replay or mixed
reconstruction.

A checksummed validation-marker file is never restart authority by itself. Startup quarantines all
recovered markers, reloads their exact signed bodies, and reproduces each execution commitment with
the production deterministic validator before the serialized runtime can restore Prepare or Commit
authority. Reproposal aliases of one exact body share one execution pass but retain independently
checked round-local bindings. If Kura already crossed the commit boundary, its cryptographically
verified finality artifact authenticates the exact subject and execution commitment instead of
replaying the candidate against an advanced world state. A substituted marker, mismatched
commitment, missing semantic dependency, or caller that skips this preflight fails closed before
network ingress.

The view projection partitions the bounded committee into Set A (`2f + 1`
members, with the leader first and proxy tail last) and Set B (`f` members).
Proposal control reaches the full committee, while the initial RS16 chunk
fanout targets Set A. Prepare and Commit votes reach the full committee, so any
validator can aggregate and broadcast the QC without making one projected
collector a liveness dependency. If the fast path does not complete,
retransmission expands chunks to Set B; any `2f + 1` equal votes form the QC.
Committee-wide timeout votes form a TC and cyclically rotate all roles. Every
voter must reconstruct, durably store, and deterministically validate the full
exact body before Prepare.

The reducer may bind and durably reconstruct lane proposals after a PrepareQC
locks one exact global body, but no current-height lane Prepare, Commit, QC, or
NewView signature is admitted or emitted until the reducer installs the exact
global Decision. This prevents a lane certificate for a locally locked carrier
from conflicting with a later higher-view global Decision. After Decision,
validators form the winning lane certificates asynchronously; a losing or
merely advertised global proposal cannot advance a lane. Global application
and successor activation do not wait for the lane CommitQC. Exact unfinished
sessions, safety locks, payload cursors, recovery ownership, and bounded
retransmission state move once into the successor and continue there as
historical work. Before a later block may consume the next lane-local height,
Kura must durably hold the exact lane certificate and either an application
receipt bound to the canonical global block and its transaction results while
those results are retained, or the exact hash-only snapshot anchor after
compaction. Restart repairs the narrow full-body
certificate-before-receipt crash boundary from those canonical artifacts.

Native AMX gives every participant leg a genuine lane-local finality object. Planning groups all
AMX sources touching the same active lane slot into one canonical, non-empty participant proposal;
if a lane is a coordinator for one source and a participant for another, those roles still share one
proposal rather than competing for the same height. The participant proposal carries its own lane
incarnation, predecessor, block height, and lane-local view. Its signed attestation body binds that
height and view, the exact participant proposal hash, and the exact participant settlement
commitment hash. None of those values is copied from the coordinator proposal or from the global
Sumeragi view. Empty participant proposals remain invalid because they cannot prove which committed
sources the participant committee authorized. Unlike ordinary asynchronous lane completion, a
carrier containing Native AMX work is admissible only after every participant leg supplies matching
Prepare and Commit QCs for that exact finality object.
If any Native source in one dependency-coupled candidate slice is still
pending, candidate assembly defers the complete Native cohort together before
refilling independent single-route work. It never recomputes a surviving
source against a different participant proposal while the first body is
signable. The coordinator also pins source and phase-neutral participant-slot
claims for the global view before local signing or request publication.

Each Native AMX QC carries an ordered validator set and exactly one historical
BLS proof of possession for every validator. The data model enforces this
one-to-one alignment during construction and during both binary and JSON
decoding, and exposes the material through read-only slices and paired
iteration. Signature, proof length, ordering, committee authority, bitmap, and
exact quorum-cardinality checks remain admission-time semantic validation.
Local formation validates every supplied vote and deterministically selects
the canonical first quorum; an otherwise valid signer superset is not a wire QC.

Grouped participant application is atomic and bounded to 1–4,096 ordered,
unique sources. The group must match the exact transaction count and timestamp,
contain the current source exactly once, and carry zero participant effects,
zero nested fee receipts, and zero nested Native receipts. A block in which one
route has mixed coordinator and participant roles uses the same block-wide
anchor and defers the role-sensitive check until the complete group is
available.

The versioned Native AMX signing guard durably records Commit decisions. Its source-session claim
binds the source ID, typed transaction entrypoint hash, plan digest, height context, authority
height, coordinator route/incarnation/planned height/view/proposal, and every participant
route/incarnation. The grouped participant slot claim is phase-neutral and binds the participant
proposal and settlement hashes for the exact lane height, lane view, and signer, so a durable
Commit also constrains every later Prepare retry. Prepare anti-equivocation is scoped to one global
view in volatile exact-body/source/slot maps. Commit claims are retained only for the current
certified global view: because both Native QCs bind the exact global block-creation round, a
strictly newer certified view atomically checkpoints and retires the older Commit prefix before
accepting replacement work. Reinstalling the same view is idempotent and preserves its claims.
Before the first Prepare signature in that view, the guard atomically fsyncs a
`last_prepare_view` anchor marker; reopening the same height quarantines
that view until the global safety WAL installs a strictly newer certified numeric view. This keeps
same-view crash safety without letting uncertified Prepare choices split honest validators forever
across view changes. V5 is the only accepted signing-journal layout; unsupported pre-release
journals fail closed before the canonical signer directory is created.
Before Prepare or Commit signing, and again at admission, the guard requires
the active incarnation, exact predecessor height and descriptor hash,
and the contiguous next lane height. Commit repeats the complete participant identity certified by
Prepare and carries the exact matching PrepareQC. A committed source, session, proposal,
predecessor, or settlement conflict fails closed across restart. Global round view monotonicity
remains independent from the participant lane view.

Participant finality is control-only. Exactly one coordinator ownership executes each AMX source
and commits its state transition; participant proposals are not executable-payload handoffs and
cannot appear as independent merge executions. Participant committees may deterministically
preflight the settlement against the frozen state, but preflight cannot mutate State. The signed
participant settlement binds the proposal, included sources, resulting effect commitment, and
participant coordinates. It does not recursively contain the Native AMX receipt whose leg commits
its hash, avoiding a receipt-to-settlement hash cycle.
One shared participant-application predicate is used by block validation,
Kura, State frontiers, recovery, diagnostics, drain, and retirement. A
coordinator leg on the same route is not a separate participant application
and produces no marker, receipt, latest-pointer update, diagnostic row, or
drain blocker.

The globally finalized execution commitment contains a canonical Native application manifest root.
Each route/incarnation leaf binds the predecessor, proposal, settlement, ordered source/result
membership, and canonical application-block identity. Kura retains each leaf and Merkle proof in
one immutable versioned manifest file named by participant height, and stores the matching
idempotent application receipt in a separate immutable versioned per-height file. Publication
creates and fsyncs a temporary, promotes without clobbering an existing stable identity, syncs the
descriptor-bound directory, and rereads the exact bytes. It makes participant finality and the
manifest durable first, then the receipt, then replaces the route/incarnation-bound exact-latest
pointer, and only then advances the replicated participant frontier. The canonical executed wire
remains available until that evidence is durable. After body pruning, validation uses the
QC-authenticated manifest root and proof; old hash-only evidence remains blocked unless the
canonical wire is recovered from authenticated storage or QC signers.

Every revision-4 execution commitment also carries the mandatory `merge_carrier` option. Ordinary
blocks encode it explicitly as JSON `null` (and as the canonical absent Option tag in Norito).
A merge carrier encodes the closed V1 object `{version: 1, entry_hash}`, where `entry_hash` is the
canonical hash of the complete merge-ledger entry. Missing options, unsupported versions,
malformed hashes, and unknown object fields are rejected; there is no revision-3 omission fallback.
Immediately after that option, the commitment carries mandatory non-zero
`executed_block_wire_len: u64` followed by `executed_block_wire_hash`. Validators reject a missing
or zero length; the pair binds the exact byte length and digest of the canonical result-bearing
block wire, with no implicit legacy default.

The standalone manifest and receipt histories use the configured Kura sidecar-retention count and
the existing shared Native sidecar aggregate-byte budget, with one bounded transient publication
slot. Compaction first fsyncs a versioned prune intent bound to lane, dataspace, incarnation, and
every `(artifact kind, participant height, artifact hash)` removal. Restart completes a
temporary-only or stable intent, resumes after each individual manifest/receipt unlink and after
the complete pair unlink, and reconciles identical stable plus temporary intents idempotently. The
exact pair named by the latest pointer cannot be pruned.

A crash after global application but before the receipt or latest pointer leaves the old frontier
blocked. Startup repair revalidates the block, checkpoint, finality, manifest root/proof, and exact
group under the sidecar publication guard, then idempotently completes the missing standalone
files without executing the transaction again. A valid lone publication temporary is promoted and
a byte-identical duplicate beside a stable file is removed; malformed, conflicting, or oversized
temporaries fail closed. Startup reconstructs the bounded exact-latest pointer explicitly;
steady-state lookup does not reverse-scan history. Obsolete dense Native data/index layouts and
unexpected, malformed, oversized, non-regular, hardlinked, or symlinked artifacts are rejected
before mutation.

Lane draining, retirement, archive, purge, disk accounting, and same-ID recreation all require the
same exact finality/manifest/receipt/latest-pointer join for the active incarnation. An unapplied or
unverifiable participant slot remains live work, so autoscaling cannot destroy its storage
generation or admit the next incarnation early.

A fresh lane height always starts at lane view zero, independently of the winning global proposal
view. Before a global body is locked, the deterministic height-rotated author for each active route
may reserve one bounded FIFO batch. The durable reservation keys, entrypoints, routing plans, and
Native AMX receipts are signed into one hint-free executable payload and persisted before fanout.
Ingress accepts only the exact scheduled author, active incarnation, view-zero proposal,
applied-or-snapshot predecessor, frozen committee, and canonical reservation identities. A second
producer or different payload for the same slot is rejected.

Hint-free means “awaiting its carrier,” not independently executable. The global leader may include
the exact autonomous envelope in its candidate; until the resulting global lock supplies that
anchor, validators keep the payload in the bounded pending set. The lock lets
Kura persist the exact payload and execution input, but READY, lane
Prepare/Commit, and autonomous NewView remain quiescent until the same carrier
has the exact global Decision. After Decision those phases use only the
persisted bytes and immutable origin proposal.
If the scheduled author is unavailable, the runner waits only for the configured bounded interval
and then permits ordinary carrier execution; it does not let another committee member replace the
author or reservation claim. The separate globally planned carrier path carries no autonomous
reservation metadata and is reconstructed only from its exact committed entrypoints.

The crash-atomic Kura lane-geometry marker is a universal active-segment boundary, not an
autonomous-payload special case. Globally anchored lane-ownership sidecars, autonomous payloads,
certified lane sessions, globally anchored or autonomous execution inputs, direct-execution
preflights, and `Current`, `DirectExecution`, and `MergeExecution` application receipts all require
the exact active lane, dataspace, and incarnation, with proposal height strictly after that
incarnation's activation, both when persisted and when served. Canonical ownership repair and
active-session, ownership, and direct-receipt snapshots apply the same marker check and revalidate
the active segment after checking canonical execution evidence. A same-ID lane recreation therefore
establishes a new durable storage generation
before accepting height one; delayed payloads, certified sessions, or cached execution artifacts
from the retired incarnation cannot replace or be read through the fresh lane slot. Only the
geometry retirement/archive scanners may read marker-mismatched artifacts, using the authenticated
historical binding while moving or proving retired storage. Committed-log receipt repair skips those
historical merge executions instead of repopulating them into the active segment.

Recovered execution evidence carries one canonical typed source. `GlobalBlock` contains the exact
`LaneBlockArtifact` committed by the readable global body; `AutonomousLane` contains the exact
network identity, epoch, and executable-payload hash authenticated by the lane-owned payload. The
same source value is copied into the durable execution input, direct preflight, and application
receipt. A proposal payload hint remains consensus scheduling evidence only: it is never promoted
into a global storage anchor for autonomous execution. In particular, a hint-free payload does not
invent a `HashOf<BlockHeader>` or proposal view zero, and the pre-release artifact-plus-three-option
execution-input layout is rejected rather than decoded or normalized.

An autonomous Prepare vote carries a second domain-separated READY signature over the exact
producer-authenticated payload. The resulting PrepareQC embeds the READY aggregate, historical
committee and PoPs. Kura must fsync this origin-Prepare availability certificate before the adapter
may consume a Commit request or release a Commit signature. A transient write failure leaves the
request and QC retryable; it cannot produce an undurable Commit lock.

Lane `NewView` certificates rotate only a synthetic, durable retransmission cursor. They never
retarget the proposal certified by Prepare, Commit, READY, the certified sidecar, or merge
execution. Every Commit signature before and after a cursor change is therefore byte-identical and
remains compatible with the crash-safe per-incarnation signing guard. The cursor chain is checked
for contiguous quorum-authorized transitions and may be compacted into a restart checkpoint. Lane
availability, drain, NewView, and lane-block wire certificates each carry exactly the canonical
quorum signer count; local aggregation projects the canonical first quorum and validation rejects
supersets. A later-view READY certificate is invalid. Failed cursor persistence is retried by
re-aggregating the retained quorum votes; installing a cursor simply re-fans the immutable origin
payload, proposal, votes, and QCs.

Fast global finality does not reset unfinished canonical lane consensus. At each global-height
boundary the runner carries only the bounded lane-session cache whose proposal identity matches the
exact Kura ownership artifact, whose incarnation remains active in the successor context, and whose
lane-local height has neither an application receipt nor a hash-only snapshot anchor. Matching
remote votes, a PrepareQC, pending certificate broadcasts, and signer commit locks survive intact;
the advisory global-block hint is normalized to Kura's exact anchor. Unanchored proposal siblings,
orphan evidence, applied slots, and inactive incarnations are pruned, while a quorum-certified
identity that conflicts with the canonical recovered proposal stops rollover before mutation. The
successor reapplies its configured ordinary-session bound without evicting protected commit evidence.
Ephemeral NewView votes and timeout markers do not cross that boundary; the successor fully
revalidates the durable cursor chain and latest certificate from Kura. Historical shared-lane
evidence is authenticated with the cryptographically verified V2 finality artifact, roster, and
PoPs for its original proposal height, never the structural context-recovery store or the
successor's mutable roster.

Startup enumerates validated autonomous artifacts before reconstructing missing payloads from
committed global anchors. It filters finalized and inactive work before applying the global session
cap, restores the immutable origin proposal and READY/Prepare evidence, hydrates an exact certified
sidecar when present, and recreates the local Prepare vote from durable bytes. A quorum restart
therefore does not depend on an external proposal replay to resume an unfinished lane height.

A complete lane session whose exact carrier is durable in Kura but not yet committed in WSV is a
deferred apply boundary, not a fatal certificate error. It remains in the bounded retry queue. The
runner attempts idempotent certificate-and-receipt persistence during every actor loop as well as at
terminal height rollover, so the session is published immediately after State reaches the carrier
and is then excluded from successor rollover.

The inverse crash boundary is also authenticated explicitly. If Kura contains the exact global
lane body and ownership sidecars but WSV commit did not finish, restart does not rerun mutable lane
planning. Recovery verifies the canonical block hash, exact ownership sidecars, active route and
incarnation, QC tag, committee, expected global leader, replay material, and applied-or-snapshot
predecessor, then still runs the normal complete block validator. Any drift fails closed. Missing
lane or AMX work is fetched or requeued. After a bounded work deferral or candidate rejection, the
runner arms one ordinary non-empty proposal retry; if no publishable work is available, that retry
retires and waits for work rather than manufacturing a carrier.

Fresh global proposal production is workload-driven. The signed block cadence is the earliest
view-zero proposal time, not an instruction to manufacture a body at every idle height. A leader
defers before signing or encoding when the bounded queue snapshot, autonomous provider, and
internal attachments contain no work. Internal work includes enabled, still-reachable time
triggers which require ledger-clock progress, DA and pin material, previous-roster audit evidence,
NPoS effects, SCCP commitments, certified merge work, and autonomous lane payloads. A resultless or
wire-empty carrier is therefore admissible only when the shared semantic-work gate proves
state-derived ledger-clock progress or authenticated external, autonomous, or other internal work;
genuinely idle bodies are rejected.

Merge-committee certificates are likewise not applied out of band. A complete certificate is
stored as a hash-addressed Kura sidecar bound to one global height, parent, and view. The matching
V2 proposal carries a compact certified reference, binds the certified execution batch to the
carrier application header and ledger time, and defers ordinary queue work at or after that time
or duplicating a certified batch entrypoint. Block validation resolves and revalidates the exact
sidecar, and Kura commits the carrier block and merge-log record in the same global order, with
the full entry staged before the block becomes irreversible and monotonic restart reconciliation
repairing any unpublished merge-log or sparse-carrier suffix. Old-view sidecars and signatures
cannot be rebound to a later proposal origin; an earlier-origin global block that reaches a
later-view CommitQC remains decisive only with its exact original carrier.

Only the exact global merge leader constructs the execution candidate. It advertises and serves
bounded chunks of that complete candidate; followers validate and deterministically reexecute the
embedded autonomous source bundles using committed State rather than selecting a local Kura subset.
Consequently harmless local producer-signature or synthetic-cursor variants cannot create follower
digest choices: the round has one leader-carried byte sequence, one durable merge signing decision,
and one quorum digest. A completed round rejects a second advert or transfer identity even from the
authenticated leader, and view/lock retirement immediately purges queued candidate traffic.

## Audited hash-only snapshot startup

An imported ledger whose historical block bodies predate the executable v2 chain uses one explicit
snapshot trust root; it is not treated as genesis and no authority is inferred from environment
variables, mutable peer configuration, or a self-signed first artifact. Operators must configure
the exact audited payload before startup:

```toml
[snapshot.bootstrap]
enabled = true
audited_sha256 = "<exact 64-hex SHA-256 of snapshot.data>"
audited_height = 12345
```

An enabled policy requires the digest and a non-zero height; a disabled or partially specified
policy rejects all authorization fields. The signed, digest-pinned snapshot carries a
`SnapshotV2BootstrapRecord` containing the exact first executable `HeightContext`, roster-aligned
BLS proofs of possession, and a `SnapshotBootstrapAnchor`. The anchor commits to the audited height,
terminal block hash, terminal block timestamp in milliseconds, and canonical WSV hash. The reader
also requires exact chain, commit-topology, live-key/PoP, and WSV agreement. Every pre-existing
Kura hash must match; audited bootstrap may append a missing prefix but never rewrite or truncate
existing history. Kura remains read-only
while the audited prefix is provisional; its runtime starts only after snapshot authentication and
the independent v2 replay-boundary check agree and the durable verified-tail marker is published.
The marker is discovery metadata, not an authentication capability: on every restart it opens Kura
read-only, and a normally signed snapshot must retain the original bootstrap record so startup can
match its lineage digest, complete block-hash vector, anchor, and first full finality artifact. Once
the initial import has completed, operators may disable the one-time digest bypass; doing so never
permits startup from the marker alone. Missing or substituted lineage, an unexpected anchor parent,
or a later snapshot above the anchor without the complete lineage-bound first finality artifact
stops startup before Kura writers, consensus, or network ingress open.

The token-consuming finalizer may complete deferred commit-manifest, retained-stage, finality, and
carrier recovery. Startup therefore never executes the replay plan computed while Kura was still
provisional: after finalization and State journal hydration it rereads the exact fallible durable
height, recomputes the whole plan, revalidates the restored State and audited boundary, and repeats
the complete body-range preflight. Geometry changes and replay begin only from that freshly
authenticated post-recovery image.

Before any generic replay, WAL, or network ingress, startup persists the exact snapshot context in
the immutable v2 context store and compares any first full-body artifact byte-for-byte with that
context and PoP vector. The first full block extends the anchored hash and derives canonical ledger
time as the maximum of `anchor_timestamp + committed_block_cadence` and every included transaction
timestamp plus one millisecond. Zero, fractional-millisecond, or overflowing cadence geometry fails
closed. This hash-only-parent profile is one-shot: after that block finalizes, every later context
must carry the ordinary parent CommitQC and a snapshot anchor is rejected. A crash before the first
finality sidecar can reopen only from the original anchor-height snapshot and the exact persisted
context, safety-WAL decision, body receipt, and semantically replayed validation
receipt; it never fetches, signs, broadcasts, changes view, or executes an
inferred context during recovery. A checksummed marker cannot select itself for
semantic replay: the authenticated WAL frontier names only the durable
lock/decision and the adapter's bounded first replay batch. Markers from older
views retain no restart vote authority and cannot force unbounded synchronous
execution. If one selected marker needs a certified merge sidecar, startup
retires that marker authority while retaining the exact body; only later live
reducer work may enter the existing bounded sidecar-fetch and validation retry
path. A later snapshot is written only after complete commit evidence exists
and is not accepted as a recovery root for that pre-finality window.

Before replay mutates WSV, startup preflights the entire requested height range. Every executable
height must have a locally retrievable canonical body; ordinary finalized body eviction remains
valid when the signed snapshot already covers that height, but a state behind an unavailable body
fails before any earlier block is applied. Zero-length local-snapshot placeholders are never
mistaken for an audited imported prefix without the typed retained lineage boundary.

If that exact sidecar is absent during deterministic body validation or later decided application,
the exact work identifier is retained rather than converted into a permanent rejection. The node
requests fixed-boundary, hash-addressed chunks only from the merge QC's authenticated signer set,
validates the canonical full entry against the compact reference and current global order, persists
it in Kura, and retries the same durable work. Before allocating a fetch, compact-QC preflight
requires the exact frozen roster, chain digest, hard count/byte caps, equal-vote quorum, canonical signer
PoPs, and a valid aggregate signature. Inbound sessions are keyed by both entry hash and the full
reference digest, so attacker-first conflicting length or execution metadata cannot poison an
honest decided body with the same claimed hash. Ordinary validation traffic leaves global and
per-holder session/byte headroom for the uniquely decided Apply dependency, and idle requests resume
strictly after a fairness cursor. A full outbound queue is detected before cryptographic preflight;
successful authentication is cached by the canonical QC identity for this height, while every
unsigned reference variant still reruns cheap shape and carrier checks. Returning an unsent request
to the idle set restores its holder cursor and timeout attempt count because no network attempt
occurred. Sidecar traffic otherwise has reserved, fairly drained capacity; active bounded responses
do not expire merely because a protocol-sized transfer spans many ticks. Capacity pressure,
transient Kura publication failure, and holder outages remain retryable. Registration failures
reject only their exact body/work tuple, while hash-wide rejection requires a fully decoded
reference-matching entry.

On a certified view transition, deferred work not protected by the exact proposal round and subject of the
TC's selected high PrepareQC is released from the executor and transport. Kura cleanup is cached by
certified carrier-state transition rather than rescanning its bounded store on every actor loop.
The same certified transition retires strictly older global control,
payload-chunk, and merge-share fanouts from the exact-output worker before
retry arbitration. A permanently unreachable topology target therefore cannot
accumulate one old-view owner per timeout until the shared corridor rejects the
current Proposal or TimeoutVote for every responsive peer. Height-only recovery
and epoch-wide VRF traffic are outside this view cut. Exact identical topology
retries reuse their incumbent worker owner, and each frozen target has one
separate pacemaker reservation so the TimeoutVote needed to certify the cleanup
view cannot itself be starved by ordinary Safety-class backlog.
With neither a lock nor a durable decision, Kura retains only entries eligible for the new exact
carrier round; the initial directive installs that retention before ingress opens. With a lock or
durable decision, cleanup waits for the durable body and retains only its exact compact reference,
including an immutable earlier origin view. A durable decision also stops new merge-candidate
production. Locked-body insertion may replace an uncertified same-slot proposal shell, but a
conflicting lane-local QC remains safety-protected; the complete next lane-session cache is resolved
before deleting losing durable sidecars. Once merge quorum formation and full State validation
succeed, the adapter transitions to a typed certified-publication stage, drops the duplicate large
candidate body, and retries the exact prevalidated entry on the normal retransmission cadence after
Kura failure. Before using the local consensus key for a merge share, the adapter fsyncs one exact
`(epoch, view, carrier height, parent, roster) -> digest` decision under the Kura root. A later relay,
successful sidecar staging, or process restart therefore cannot reopen that context for another
digest. Signing, remote-share admission, and share retransmission additionally require State and
Kura to expose the same exact parent frontier, so a block-first Kura-ahead crash image cannot reopen
merge work. Regular lane, Native AMX, and merge-share retransmissions rotate their first-service
class, including when the outbound queue has only one slot. Kura stages the exact full entry first
and then makes the canonical carrier block its first irreversible commit point; merge-log,
sparse-carrier, and transaction-index publication proceeds monotonically afterward. A crash or
write error in that suffix is repaired from the durable block and retained pending entry before WSV
application. Once that association is complete, idempotent replay does not recreate a pending
sidecar or depend on unrelated pending-store capacity. Finalized height rollover removes all
remaining losing sidecars. This keeps the in-memory and durable caps live without deleting an entry
that a delayed decisive CommitQC can still require.

## Finalized membership and lane observability

Static trusted peers are bootstrap seeds only. Once finalized world membership is non-empty, v2
advertises exactly that membership and evicts connected seeds which are no longer present. A local
node that was previously admitted and is later absent clears its queues, disconnects both network
and gossiper topology, and sets the process-local removed-membership telemetry
state; this also applies when the finalized peer set becomes empty. A
never-admitted observer may retain bootstrap connectivity while the world is
still empty.

`GET /v1/sumeragi/status` returns only the authoritative `SumeragiV2Status`.
It contains the reducer's protocol and build/configuration fingerprints, restart flag, frozen
height context, height, view, phase, leader, QC/TC references, body and persistence state, commit
frontier, and bounded liveness state. `GET /v1/sumeragi/status/sse` streams the same authoritative
shape. Lane sidecars, queue pressure, Native application evidence, and autonomous execution stages
do not appear on either status surface.

`GET /v1/sumeragi/diagnostics` returns the non-authoritative
`SumeragiDiagnosticsStatus`. It includes pipeline and queue pressure, lane
commitments and settlements, relay envelopes, payload ownership, committed
lane blocks, live lane sessions, governance state,
`native_amx_participant_applications`, and
`autonomous_lane_executions`. Nexus-disabled nodes return the lane-detail vectors empty. The Native
and autonomous evidence vectors are derived from State plus revalidated Kura
evidence (and the durable queue finalization view), rather than a process-local
completion cache. The Native vector is deterministically ordered and bounded to
one record per active route/incarnation; each record exposes the participant
height/view, predecessor, descriptor, proposal, settlement, grouped source
count, optional application block, and one of
`certified_pending_carrier`, `committed_evidence_pending`,
`durably_applied`, or `conflict`. Conflicting same-height identities are
reported as `conflict`, never selected by arrival or filesystem order. The
autonomous vector is restart-stable, bounded, and advances from durable
reservation evidence through lane certification,
bundle/merge/carrier/application evidence, and finally `queue_finalized`;
disagreement is `conflict`. Diagnostics describe durable progress but never
authorize it.

The status snapshot's latest durable CommitQC summary is evaluated under that certificate's own
retained height context, including exact signer and voting-power totals across an epoch boundary.
Its typed validator rejects impossible phases, body states, persistence identities, and certificate
contexts. The diagnostics producer and typed validators independently enforce
active-incarnation derivation, deterministic ordering, vector and group bounds,
complete application-block pairs, queue limits, recomputable payload
ownership, known execution labels, and possible certified quorums. Oversized or
inconsistent typed or JSON responses fail closed instead of turning operator
diagnostics into an unbounded allocation surface.

Every proposal-authenticating outbound-intent diagnostic carries its exact `proposal_round` in
addition to the intent's finality `round`; timeout intents carry no proposal round. Proposal and
Prepare intents require equal rounds, while Commit intents may retain an earlier authenticated
proposal origin after a view change.

Both JSON projections follow the Norito JSON contract exactly. Fixed byte arrays such as
`height_context.epoch_seed`, settlement `source_id`, and relay `manifest_root` are uppercase hex
strings of the exact byte width; they are not JSON byte arrays. Unit enums remain tagged objects:
for example mode is `{"mode":"permissioned","details":null}`, reducer phase is
`{"phase":"prepare","details":null}`, and body state is
`{"state":"validated","details":null}`. Settlement `u128` totals and receipt amounts are
projected as canonical unsigned decimal strings (zero is `"0"`; signs, whitespace, and leading
zeroes are rejected) so JavaScript clients do not lose precision. Liquidity and volatility use
their declared Norito tags, for example `{"profile":"Tier1","state":null}` and
`{"bucket":"Stable","state":null}`.

Each diagnostics `lane_relay_envelopes` entry exposes the actual relay record: `block_header`, nullable `qc`,
nullable `da_commitment_hash`, its settlement commitment and hash, and RBC byte total. Optional
`lane_block_descriptor_hash`, uppercase-hex `manifest_root`, and `fastpq_proof` are omitted when
absent. The JSON wire does not contain the retired synthetic `block_hash` or `commit_qc` keys.

## Safety WAL

The Sumeragi safety WAL is append-only and hash chained. It is bound to the chain, protocol version,
and consensus key. Each successful append includes `flush` and `sync_data`; only that success is a
durability acknowledgement to the reducer. Replay happens before consensus ingress opens. Recovery
streams and verifies one frame at a time, retaining each payload exactly once instead of buffering
the complete file and cloning its records. A height-local WAL is bounded to 8,192 complete records
and 32 MiB of aggregate payload bytes; append rejects the first record that would cross either
fixed first-release bound before file or hash-chain state changes, and startup fails closed on an
already oversized complete prefix. These are deterministic WAL-retention invariants, not process
memory limits.

The WAL records Prepare intent, observed PrepareQC/high QC, atomic lock plus Commit intent, timeout
intent, installed TC, and decision. A `LockAndCommit` whose proposal origin is older than the
replayed current view is accepted only when the preceding records leave its exact PrepareQC as the
active lock after view installation and no higher proposal-origin local Prepare intent or known
PrepareQC exists, including evidence for the same subject bytes. Its Commit round is the active
finality round; its proposal origin remains the lock's exact earlier round. Any higher Prepare
origin fences reconstruction rather than relabelling that origin or creating a replacement
proposal. The timeout fence still rejects every other historical vote. An incomplete
final frame is an unacknowledged crash tail and is discarded. A checksum failure, broken hash chain,
non-monotonic sequence, identity mismatch, or historical-lock mismatch before that tail fails
closed. Records are pruned only after the decided block and its certificate are durable in Kura.

Pending persistence and the production refinement boundary carry the WAL record's primary proposal
origin independently from the reducer owner tag. Records which embed a second certificate, such as
`LockAndCommit` or timeout evidence with a highest PrepareQC, also carry that certificate's
auxiliary proposal origin. Begin, acknowledgement, requested capability, and independently
reconstructed grant must agree on both origins. Mutating both sides to a different origin is still
rejected because the pending WAL record remains the authoritative identity.

Height-local I/O retirement is a nonblocking control handoff. After the typed
Kura finality receipt is validated, the context worker consumes its body-store
owner and tries once to enqueue a combined body/chunk retirement job to one
bounded runner-lifetime janitor. The consensus thread never joins a running
context worker or performs recursive filesystem deletion. A full or
disconnected janitor queue retains the files for startup reconciliation and
reports a typed warning; it cannot delay successor construction or create an
unbounded cleanup-thread pool. Finalized-height network output does not enter a detached
repair domain. Before the successor opens, the serialized runner retries
responsive targets, proves every remaining fanout reconstructible from Kura and
the typed finality artifact, atomically seals the exact-output corridor, and
moves the sole merge-sidecar journal owner into the successor. Backpressured
autonomous payload and NewView output is discarded only after exact durable
reconstruction and local retransmit authority are verified. No predecessor
thread or second journal writer survives the handoff. Finalized height-local
files are retained for restart reconciliation when cleanup cannot complete,
and that post-application file cleanup does not delay successor progress.
The successor responder reserves every current-roster stream plus one complete
31-validator immediate-predecessor committee. An identity absent from the
current roster must belong to Kura's exact durable predecessor context before
it can allocate in that corridor, so arbitrary older identities cannot starve
predecessor recovery or consume a live-roster slot. Structural, stale-generation,
duplicate, and per-source rate admission precedes expensive carrier lookup.
The durable requester-fair materialization scheduler then proves the exact
finalized carrier and requester membership before emitting bytes; a predecessor
member may request an older carrier only when it also belongs to that carrier's
frozen context. On restart, an older same-roster V3 lifecycle snapshot may
expand monotonically from `N` responder streams to `N + 31`; the persisted
generation and all stream/gate ownership remain unchanged, while shrinkage is
rejected. Kura extracts the carrier's compact merge reference while the
signed body is present and retains it with the canonical header and proposal/
executed-wire hashes. Local body eviction therefore cannot remove bounded
serving authority. The retained reference is not standalone consensus
evidence: the recipient still verifies the exact reference and merge QC against
its own canonical carrier. Kura accepts only the current version-3 retained
record. Pre-release version-2 bytes fail closed at direct read and startup;
operators must discard and rebuild that pre-release storage instead of asking
the node to synthesize fields that were never authenticated.

Runtime-ingress and Busy-deferred body completions share one exact ownership
domain. Deferred work retains its full manifest and durable/validated receipt,
including validation polarity. Only byte-for-byte equal trusted evidence may
coalesce; conflicting evidence or duplicate owners fail closed, and ownership
is checked before body availability can prune any queued proposal.
`LocalProposalReady` uses the reserved Completion class at both boundaries, so
Normal ingress saturation cannot strand a locally built, durably stored,
validated proposal. After signature verification, an individual Vote is
admissible only when its exact round/subject execution commitment is already
bound by a local validated receipt, verified WAL replay, or
quorum-authenticated QC evidence. A Vote cannot create that authority merely by
being individually signed. Unbound Votes are rejected recoverably; conflicting
commitment-bearing evidence is rejected before serialized runtime ownership.

Body-availability rebind requires the reducer's installed destination tag and
preflights both source and destination ownership before mutation. One exact
source moves to an empty destination or coalesces into one exact destination
owner. Coalescence first classifies persistent producer backing on both sides.
If only the source is persistent, the ordinary destination is retired and the
persistent source is retagged; if only the destination is persistent, source
ownership is validated against the already-persistent destination before the
ordinary volatile source disappears. Two
independent persistent roots fail closed before either side changes. Thus one
persistent root always survives a successful coalescence. An uninstalled
destination tag is a recoverable caller-contract rejection with no mutation;
conflicting or duplicate ownership fails closed without a partial move. If a
certified response has already reserved an
unpublished `BodyAvailable` position when its service handoff returns typed
`Retryable`, a protecting `EnterView` retags that same physical token together
with its `PendingFetch`. The token keeps its admission ordinal, lifecycle
owner, exact manifest, and restart backing; a later retry can publish exactly
once at the installed incarnation instead of colliding with its old-view
owner. That exact retry reclaims the already charged token before consulting
the reducer's new-view admission projection, which may legitimately differ
after `EnterView` but has no authority to remint the physical slot.
Body-pipeline and Decision retirement likewise
preflight all ingress and Busy-deferred owners transactionally before removing
any of them. Retiring a pending fetch also retires its exact unpublished
`BodyAvailable` token before releasing the request and pipeline owner; a TC
which protects another body or carries no body lock therefore cannot leave an
old-view Completion slot permanently occupied. If that token or a queued
completion replaces a restart-restored stage-7 producer, any required durable
producer removal is persisted before its runtime lane is changed. A failed
store rolls the producer maps and dormant index back and therefore cannot
partially coalesce or partially retire a body pipeline.

The generic productive leader-wire lifecycle gate advances from the same
durable safety frontier. After a certified `EnterView`, and again after the
first durable Decision, the service derives a monotone process-local recovery
authority. While holding the fair-ingress mirror lock it asks the persistent
gate to remove exactly the obsolete restart-dormant slots, publishes that
snapshot first, and only then removes the same mirror records. Persistence
failure restores the prior gate state and leaves the mirror untouched. Live
Ingress and Runtime owners are never pruned by this cut, admission rejects an
identity below the durable view (or every identity after Decision), and both
admission-ordinal high-watermarks survive retirement so the freed slot cannot
resurrect an old identity or reuse an ordinal.
At the transport boundary, a locally conflicting certified-body request is a
nonfatal remote rejection, while a conflicting Commit-certificate response
leaves discovery outstanding and retryable through another authenticated peer.

The production `SumeragiWorker` dispatches Sumeragi-v2 wire revision 4 directly
to the serialized height runner; it never executes the legacy actor under a
revision-4 handshake. For every height the runner replays context and WAL state before
opening ingress, drains all tagged effects, validates the typed Kura receipt
against the exact finality artifact, and only then builds the successor
context. WAL, body, chunk, or cleanup-worker retirement after that durability
boundary returns an ordered typed warning report. Those local cleanup warnings
remain operator-visible but cannot turn a committed block back into an error or
prevent successor-height progress.
The finalized-height type retains the safety WAL until canonical lane/output
rollover is durably reconstructible. A crash between global Kura finality and a
late lane certificate therefore reopens from the Decision WAL instead of
entering `PendingKuraApply` without any recoverable Decision-Fetch authority.

## Correctness claim and trusted boundary

Safety assumes authenticated signatures, collision-resistant hashes, deterministic validation,
at most `f` Byzantine validators in the `3f + 1` committee, and faithful durable-write
acknowledgements. The safety properties are agreement, chain-prefix finality, external validity,
vote uniqueness, crash/restart lock preservation, epoch isolation, and durable availability of a
decided body.

Liveness is a conditional target because an asynchronous network cannot guarantee termination. The
paper argument assumes that, after GST:

- at least `q = 2f + 1` frozen committee members are correct and responsive;
- authenticated per-source messages and retransmissions are serviced within the declared transport
  bound;
- the monotonic clock and serialized run loop continue, with timeout priority, FIFO debt, and
  normal/progress/completion queue reserves;
- one-shot timeout and periodic retransmission events use the trusted deferred lane, where duplicate
  ticks coalesce and an untrusted normal-message flood cannot discard an already emitted timeout;
- body transfer, reconstruction, validation, signing, certificate formation, application, and
  fsync terminate within the declared service bounds;
- an honest leader can construct a deterministically valid proposal from
  available authenticated external, state-derived clock, autonomous, or other
  internal work;
- correct nodes eventually recover with intact WAL state;
- an honest leader recurs within one roster rotation;
- honest Prepare signers continue serving their durable bodies; and
- enough honest members of every unfinished predecessor lane committee remain
  running and responsive to satisfy that descriptor's frozen threshold until
  its exact lane evidence is durable, even when those members are absent from
  the successor global roster.

Under those assumptions, the paper argument derives that failed views lead to a TC, rotation
reaches an honest leader, and a safe round forms PrepareQC and CommitQC. Every responsive correct
node independently persists the exact decision, fetches and validates the certified body, applies
it, and advances its local certified prefix; no global all-node application barrier is required.
FLP is the reason these post-GST premises are explicit. This targets consensus-height progress when
a valid proposal with semantic work is available, not idle-height production,
transaction-inclusion fairness, or censorship resistance.

Ten arbitrary-context Core safety wrappers are TLAPS-proved. This includes historical TC-lock
Commit authorization, the dependent direct-or-installed-authorization timeout wrapper, and the
narrower grouped-timeout kernel for Commit intents already present at timeout. The archived
revision-3-rooted ledger has its final 44/3/6/1 status vector and
`machine_checked_completion: true`, but that flag is not revision-4 proof evidence. The liveness
claim remains conditional until fresh strict ledger evidence and the separate mandatory revision-4
TLC/mutation corridor pass against the same signed source.

The executable reducer and persistence-effect ordering are the source-verification boundary.
Cryptographic implementations, canonical Norito encoding, deterministic execution, OS fsync
semantics, clocks, NPoS election economics, and post-GST delivery are the documented trusted
computing base. TLC finite runs search for counterexamples and generate replay traces; only
discharged TLAPS and source-verifier obligations count as deductive proof.

The review proof is recorded in `formal/sumeragi_v2/PROOF.md`; the adjacent TLAPS ledger and
generated source-bound evidence state exactly which obligations were mechanically discharged. The
release checker rejects stale counts, unproved obligations, proof escapes, and the retired
favourable-network liveness corridor.

## Multilane release evidence

The production paths described above are implemented behavior, not a release
attestation. The multilane gates remain open until fresh artifacts pass for
focused/adversarial unit coverage; source-bound TLC and Apalache positives plus
every expected mutation; unskipped four-peer DA/RBC lifecycle suites; 10/10
13-peer global corridor seeds (twelve lane validators) and the two-hour fault
soak; cross-SDK parity;
five paired pinned-hardware one-versus-four-lane scaling runs; and the
prescribed locked/offline full-workspace build, test, strict Clippy,
formatting, and legacy-codec checks. This document makes no claim that those
four-peer, 13-peer global, soak, scaling, or full-workspace runs have passed.

## Taira profile

The Sumeragi-v2 Taira chain starts from a new chain ID, targets one-second blocks, and uses a
ten-second round deadline. Cutover requires all four labeled validators to report the same build,
protocol/config fingerprint, height context, and committed hash across repeated advancing samples.
The shared public edge is checked only after those direct validator checks and a signed runtime-only
canary succeed.
