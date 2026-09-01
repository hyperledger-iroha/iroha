# Sumeragi consensus

Finite Sumeragi Torii reads are operator-only. For brevity, every
`iroha[_cli] ... ops sumeragi ...` example in this specification assumes the
global `--operator-private-key-file /absolute/runtime/operator.key` option.
The key must be an explicitly supplied allow-listed operator key and is bound
to the exact `network_id` in the client configuration; account keys, tokens,
redirects, and retries are not fallbacks. `/v1/sumeragi/status/sse` remains the
intentional public protocol-handshake stream.

## Authoritative Sumeragi v2 revision 4

Revision 4 is the first-release production consensus contract. Wire and pure
core protocol versions are exactly `4`; nodes do not negotiate or reinterpret
older consensus messages.

### Committee and roles

- Every height uses an equal-vote committee with exact `n = 3f + 1` geometry,
  where `1 <= f <= 10`. Valid committee sizes are 4, 7, 10, …, 31 and quorum is
  exactly `q = 2f + 1`. NPoS stake affects candidate election, not consensus
  vote weight.
- The finalized height seed and height determine a stable roster permutation.
  Views rotate that permutation cyclically without changing validator indices.
- Set A is the first `q` members: the leader is first and the proxy tail is
  last. Set B is the remaining `f` members.

### Proposal, availability, and voting

1. The leader broadcasts the signed proposal manifest to the full committee
   and initially sends its canonical body chunks to Set A.
2. Reed-Solomon-16 is mandatory. A validator may Prepare-vote only after it has
   reconstructed the complete canonical body, verified its manifest and hashes,
   stored it durably, and completed deterministic validation.
3. Every validator that votes sends its Prepare and Commit vote to the full
   committee. Any validator may aggregate `q` equal votes and broadcast the
   corresponding QC. Duplicate votes and QCs are idempotent.
4. When the fast path does not complete, recovery expands body/chunk delivery
   to Set B. Voting is already committee-wide; the quorum remains `q`, and Set
   B never weakens the certificate.
5. Timeout votes are committee-wide and do not depend on the proxy tail. A
   TimeoutCertificate rotates the leader, proxy tail, Set A, and Set B. A
   locked body is re-proposed unchanged; Proposal, Vote, and QC evidence always
   has same-round semantics.

### Finality and progress

- A CommitQC is unique under the standard authenticated `3f + 1`, at-most-`f`
  Byzantine assumption and durable honest sign-once/lock rules.
- A node applies only the exact certified, locally available body. Durable
  application and its typed finality artifact authorize successor construction.
- NPoS epochs advance after exactly 3,600 finalized heights. A resultless or
  wire-empty carrier is admissible only when the shared semantic-work gate
  proves state-derived ledger-clock progress or authenticated external,
  autonomous, or other internal work. Genuinely idle, semantic-work-free bodies
  are rejected before voting/finality; the old committee authenticates the full
  next-epoch transition evidence.
- Retryable finalized-height output, merge/lane sidecars, historical service,
  and cleanup run under detached supervised repair. They remain observable
  debt but cannot revoke finality or block successor activation.
- One crate-internal `LifecycleCoordinator`, projected as
  `LifecycleLedgerV1`, owns lifecycle admission through body/persistence
  recovery and successor rollover. Retired Serve snapshots,
  predecessor-witness/latch state, and producer-episode schedulers are not
  alternate authorities.
- Liveness is conditional on partial synchrony after GST, at least `2f + 1`
  responsive committee members, terminating deterministic validation and
  durable storage, and an eventually honest leader. It is not claimed during
  an unbounded partition or permanent storage failure.

### Fresh-genesis migration

Revision 4 is a fresh-genesis cutover. There is no rolling or in-place
compatibility path from revision 3: old consensus wire values, frozen height
contexts, safety WAL state, and partially completed rounds must be rejected.
Operators must generate and sign a revision-4 genesis and start new revision-4
storage. State transfer, if separately authorized, terminates at an audited
snapshot boundary and does not import revision-3 consensus state.

## Configuration

Consensus mode, cadence, committee/leader selection, quorum, and DA layout are
signed genesis/current-height context. The accepted local `[sumeragi]` surface
contains only:

- `role`, selecting validator or observer participation;
- `block`, bounding candidate transactions, canonical body bytes, and proposal
  queue scanning;
- `queues`, bounding serialized reducer/body/chunk/ready-body ingress;
- `limits`, bounding lane, merge, historical-recovery, and Native AMX services;
- `keys`, defining consensus key rotation and allowed algorithms.

The canonical shared projection fingerprints signed mode/cadence with these
finite limits and key policy. Validators must agree on the fingerprint before
activation. V1 actor, collector, global RBC/DA, adaptive timer, persistence,
recovery, gating, and debug tables are rejected rather than ignored.

## Governed NPoS reconfiguration

Evidence retention and penalty scheduling are governed on-chain rather than
read from local node config. The first-release defaults are:

- `SumeragiNposParameters.reconfig.evidence_horizon_blocks = 7200`;
- `SumeragiNposParameters.reconfig.activation_lag_blocks = 1`;
- `SumeragiNposParameters.reconfig.slashing_delay_blocks = 3600`.

The evidence horizon, slashing delay, and epoch length are immutable after the
initial signed installation. The horizon plus delay may span at most three
epochs, matching the fixed four-roster committed-evidence capacity. Other
admitted fields may be governed through the on-chain parameter path; validators
and executor upgrades must never replace consensus-owned values with local
TOML or executor defaults.

A staged mode transition preserves the joint-consensus rule that the outgoing
set authenticates the boundary: `mode_activation_height requires next_mode to be set in the same block`.

## Deadlines and view change

The view-zero round deadline is ten signed block-cadence intervals. Critical
message retransmission is one fifth of that deadline. Certified view `v` uses a
`min(v + 1, 10)` deadline multiplier, while retransmission stays fixed. Startup
and recovery do not consume the first live deadline; after clock arming, only a
certified `EnterView` transition restarts the clocks. See
`specs/sumeragi_pacemaker.md` for the source-coupled timing contract.

## Diagnostics and test control

Authenticated `/v1/sumeragi/status` is the authoritative reducer/operator
snapshot. It exposes protocol/context fingerprints, height/view/phase/leader,
QC and TimeoutCertificate references, body/persistence state, latest durable
commit, and finite adapter/transaction queues. Retired global-RBC
INIT/READY/DELIVER status and metric fields are absent; signed RS16
`PayloadManifest`/`PayloadChunk` DA diagnostics describe the revision-4
availability path.

Safety tests inject authenticated revision-4 messages at the runner boundary.
Availability and liveness tests use signed genesis/current configuration plus
either real process/network outages or the feature-isolated authenticated
message controller at that same runner boundary. A test must not lower quorum,
disable mandatory DA, select a local protocol version, or synthesize consensus
messages through node-local debug configuration.
