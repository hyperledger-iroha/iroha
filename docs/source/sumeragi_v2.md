# Sumeragi v2 consensus

Sumeragi v2 is the only live consensus protocol accepted by this release. It is a breaking
network revision: validators using another protocol version or consensus fingerprint are rejected
during admission, and an existing chain must restart from a fresh genesis rather than mix protocol
versions at one height.

The protocol has one executable decision authority, `iroha_sumeragi_core::Reducer`. Networking,
signature verification, block construction, deterministic validation, payload storage, Kura, and
telemetry are adapters. They may perform work concurrently, but only the serialized reducer may
change consensus state or authorize a signature.

## Frozen height context

Every message is bound to a `HeightContextId`. The hashed context contains:

- chain and protocol identifiers;
- height, epoch, and the previous height's CommitQC;
- the ordered voting roster and every validator's voting power;
- the complete Nexus/AMX consensus context and DA layout;
- the epoch leader seed and quorum policy.

The context for height `h` is derived only from the finalized state at `h - 1`. A reconfiguration
committed at `h` may activate at `h + 1`; certificates from the old context cannot act in the new
one.

The next context and its view-zero proposal bind the parent CommitQC by semantic finality identity:
parent context, height, Commit phase, and block subject. The QC view, aggregate bytes, and signer
subset are excluded because the same safe parent can acquire valid CommitQCs in multiple views or
through independently collected quorum subsets. The successor constructor verifies the locally
carried QC against the exact durable parent context and artifact before opening the height, and the
adapter retains that parent context and its proofs of possession so every alternate QC carried by a
view-zero leader is also cryptographically verified before reducer admission.

Genesis carries this projection as `sumeragi_v2.nexus_amx_context_hash`. The canonical projection
binds the validated lane and dataspace catalogs, routing, staking, fees, AXT, lane fusion and
autoscaling, DA policy, deterministic AMX budgets, and the ordered active public-lane validator
records. Local paths, worker counts, caches, and telemetry are deliberately excluded. An unsigned
deployment template whose final roster is not known carries the config-only projection. `kagami
genesis sign` stages the complete genesis transaction, including validator activation, then
replaces the projection and consensus fingerprint with the exact staged values before it emits a
signed block. A template is therefore not a deployable commitment by itself.

The public Nexus sample remains intentionally non-deployable until an operator supplies the
canonical Nexus XOR asset-definition ID. Generation fails closed without
`--nexus-xor-asset-definition-id`; the Taira XOR identity must never be reused as a substitute.

Permissioned mode assigns power one to every voter. NPoS freezes the finalized stake snapshot for
the epoch. In both modes, a certificate must contain at least `floor(2n/3) + 1` distinct validators
and strictly more than two-thirds of total voting power. Observers are never members of either
quorum.

Leadership rotates through the entire frozen roster:

```text
start        = H(epoch_seed, height) mod roster_len
leader(view) = roster[(start + view) mod roster_len]
```

Stake controls NPoS admission and voting power, not leader frequency.

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

Node role is deliberately outside this shared fingerprint because an observer must be able to use
the same chain configuration without voting. Worker counts, local scheduling budgets, paths,
caches, and telemetry are also local-only. The DA encoding, chunk geometry, maximum chunk count,
and Nexus/AMX commitment are already signed into the `HeightContext`; mutable RBC configuration is
not a second source of those values.

V2 configuration validation fails closed if transaction or body bounds are absent, any adapter
queue is zero, DA is disabled, or a retired mode flip, phase-specific timeout, fast-finality cap,
adaptive resilience path, or consensus fault-injection switch is enabled. Collector routing,
global RBC state, adaptive pacemaker settings, and missing-QC recovery parameters remain in the
legacy structure only while old implementation code is being deleted; the v2 projection neither
hashes nor consumes them. Validators retransmit correctness-critical control messages to the
whole voting roster.

The shipped Taira profile sets `role = "validator"`, a 1,000 ms genesis cadence, a 10,000 ms round
deadline, bounded 96-transaction/16 MiB bodies, and the finalized NPoS stake-snapshot roster. An
observer changes only `role = "observer"`; it must not change the shared fingerprint.

## Round protocol

The global protocol has only Prepare and Commit votes.

1. The expected leader broadcasts a proposal and payload manifest. View zero is justified by the
   parent CommitQC. A later view carries the previous view's TimeoutCertificate and the exact
   highest PrepareQC selected from that certificate. The Proposal signature authenticates the
   current leader. If it re-proposes a locked subject, the canonical block body remains unchanged
   and retains its original creation-view header and block signature.
2. A validator reconstructs the complete canonical body, checks all chunk and payload hashes,
   validates it deterministically against the certified parent, and stores it durably. Only after
   the body and a Prepare sign-once record are durable may it release a Prepare signature.
3. A PrepareQC certifies validity and availability. Its honest signers have the exact body in the
   durable payload store and must serve it after restart.
4. On a valid PrepareQC, a validator with the exact validated body atomically persists its new lock
   and Commit sign-once record. Only the persistence acknowledgement releases the Commit signature.
5. A CommitQC decides the subject. The node persists the decision before applying or publishing it.
   A node missing the body records `PendingApply`, fetches from certified signers, validates the
   exact body, and does not vote at the next height until application completes.

A validator prepares a proposal only when it is unlocked, the subject equals its lock, or the
proposal's selected PrepareQC safely supersedes that lock. Locks never move to a lower certificate.

## Certified view changes

The round deadline is absolute from view entry and is not reset by partial progress. The default
public-network profile uses a ten-second deadline and retransmits critical control messages every
two seconds.

On expiry, a validator persists one `TimeoutVote(height, view, highest_prepare_qc)`. That durable
record closes the view: the validator can no longer create a Prepare or Commit vote for it. The
validator remains in the view until it receives a valid TimeoutCertificate.

After the TC is durably installed, the reducer discards the closed view's volatile individual-vote
pools and ignores late individual votes for that view. This bounds volatile evidence by the current
view. It does not invalidate a complete earlier-view CommitQC, which remains admissible and
decisive.

A TimeoutCertificate contains individually verifiable votes, optionally aggregated in groups that
reported the same high QC. Groups are canonically sorted, their signer sets are disjoint, and their
union satisfies both quorums. The deterministic maximum valid PrepareQC is selected across every
group. A TC is persisted before entering the next view and any validator may form and rebroadcast
it; there is no correctness-critical collector.

An earlier-view CommitQC remains decisive even when its final shares are assembled or delivered
after a TC. The timeout fence prevents new honest votes in the closed view, while quorum
intersection ensures that every old value which can still reach a Commit quorum is protected by the
TC's selected high QC. The formal proof obligation `TCProtectsPotentialCommit` states that precise
property. Installing a TC never lowers or clears a local lock: only a strictly higher PrepareQC can
release it for another subject. Because timeout votes transport that full QC to every voter, an
omitted lock becomes known to the honest quorum and is selected by a subsequent TC after GST without
depending on a correctness-critical collector.

## Payload availability

PrepareQC replaces global RBC READY and DELIVER as the consensus availability certificate. INIT,
deterministic erasure chunks, repair, the persistent payload store, certified body fetch, and block
sync remain transport mechanisms. Chunk signatures bind epoch, height, view, context, parent,
subject, payload root, encoding, chunk index, and total chunk count, preventing replay or mixed
reconstruction.

Lane-local work is released only after the reducer installs a PrepareQC lock for one exact global
body. Validators reconstruct the lane proposals from that durable body and then form lane
Prepare/Commit certificates asynchronously; a losing or merely advertised global proposal cannot
advance a lane. Global application therefore does not wait for the lane CommitQC. Before the next
lane-local height becomes eligible, Kura must durably hold both the exact lane certificate and an
application receipt bound to the canonical global block and its transaction results. Restart
repairs the narrow certificate-before-receipt crash boundary from those canonical artifacts.
Missing lane or AMX work is fetched or requeued, and an honest leader can still propose an empty
heartbeat when no transaction work is ready.

Merge-committee certificates are likewise not applied out of band. A complete certificate is
stored as a hash-addressed Kura sidecar bound to one global height, parent, and view. The matching
V2 proposal carries a compact certified reference; block validation resolves and revalidates the
exact sidecar, and Kura commits the carrier block and merge-log record in the same global order.
Old-view sidecars and signatures cannot be reused after a view change.

## Safety WAL

The Sumeragi safety WAL is append-only and hash chained. It is bound to the chain, protocol version,
and consensus key. Each successful append includes `flush` and `sync_data`; only that success is a
durability acknowledgement to the reducer. Replay happens before consensus ingress opens.

The WAL records Prepare intent, observed PrepareQC/high QC, atomic lock plus Commit intent, timeout
intent, installed TC, and decision. An incomplete final frame is an unacknowledged crash tail and is
discarded. A checksum failure, broken hash chain, non-monotonic sequence, or identity mismatch
before that tail fails closed. Records are pruned only after the decided block and its certificate
are durable in Kura.

## Correctness claim and trusted boundary

Safety assumes authenticated signatures, collision-resistant hashes, deterministic validation,
fewer than one-third Byzantine validators by count and voting power, and faithful durable-write
acknowledgements. The safety properties are agreement, chain-prefix finality, external validity,
vote uniqueness, crash/restart lock preservation, epoch isolation, and durable availability of a
decided body.

Liveness is conditional because an asynchronous network cannot guarantee termination. After GST:

- more than two-thirds by count and power are correct and responsive;
- critical messages are eventually delivered and retransmitted;
- body transfer, validation, signing, certificate formation, and fsync terminate within the round
  bound;
- correct nodes eventually recover with intact WAL state;
- an honest leader recurs within one roster rotation; and
- honest Prepare signers continue serving their durable bodies.

Under those assumptions, timeouts lead to a TC, validators converge on a view, an honest leader
forms PrepareQC and CommitQC, every correct node decides and applies the body, and the chain advances.

The executable reducer and persistence-effect ordering are the source-verification boundary.
Cryptographic implementations, canonical Norito encoding, deterministic execution, OS fsync
semantics, clocks, NPoS election economics, and post-GST delivery are the documented trusted
computing base. TLC finite runs search for counterexamples and generate replay traces; only
discharged TLAPS and source-verifier obligations count as deductive proof.

The review proof is recorded in `docs/formal/sumeragi_v2/PROOF.md`; the adjacent TLAPS ledger and
`crates/iroha_sumeragi_core/VERIFICATION.md` state exactly which obligations are mechanically
discharged and which still block a production correctness claim.

## Taira profile

The Sumeragi-v2 Taira chain starts from a new chain ID, targets one-second blocks, and uses a
ten-second round deadline. Cutover requires all four labeled validators to report the same build,
protocol/config fingerprint, height context, and committed hash across repeated advancing samples.
The shared public edge is checked only after those direct validator checks and a signed runtime-only
canary succeed.
