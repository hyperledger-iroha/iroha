# Sumeragi revision-4 data availability

This file records the implementation-coupled DA contract for the live
Sumeragi v2 revision-4 protocol. DA/RBC is mandatory and is committed by the
frozen height context; there is no runtime enable/disable switch.

## Frozen layout and bounds

Every `HeightContext` carries one non-empty `DataAvailabilityLayout`. Admission
rejects layouts outside the deterministic first-release limits:

- at most 16 data shards and 16 parity shards;
- at most 32 total shards;
- chunks no larger than 256 KiB;
- a canonical body no larger than 16 MiB;
- at most 1,024 encoded chunks;
- at most 32 MiB for parity expansion or pre-manifest orphan acquisition.

The layout, body length, chunk count, chunk root, and canonical body hash are
part of the proposal manifest. Nodes never infer these values from local
configuration or heuristic codec inspection.

## Routing and fallback

The leader broadcasts the signed proposal manifest to the complete `3f + 1`
committee. The first body/chunk occurrence targets Set A, whose `2f + 1`
members contain the leader and proxy tail. A bounded same-view retransmission
opens fallback and expands body/chunk service to the whole committee. Timeout
and NewView traffic is committee-wide, so a withholding leader or proxy tail
cannot make the recovery path depend on the failed fast-path route.

For a certified missing body, recovery requests authenticated chunks or the
canonical body from certificate signers first and then expands to the frozen
committee. Responses remain bound to the exact height context, proposal round,
manifest, and body subject.

## Full-body voting boundary

A validator may emit neither Prepare nor Commit merely because it holds a
manifest, shard subset, availability count, or hash. The local path is:

1. authenticate every accepted shard against the exact manifest;
2. reconstruct the complete canonical body using the frozen RS16 layout;
3. verify the reconstructed length, chunk root, and body hash;
4. cross one durable canonical-body write boundary;
5. complete deterministic block validation;
6. mark the body `Validated` and only then authorize Prepare;
7. retain the exact body through CommitQC application or certified recovery.

Authenticated partial shards are bounded volatile state. A restart may discard
them and reacquire them from the frozen committee. The complete canonical body
is the only DA object that crosses the mandatory durable boundary, so the fast
path does not pay one fsync per shard while restart safety remains explicit.

The authoritative status surface exposes the local transition through
`SumeragiV2BodyState::{Missing, Reconstructing, Stored, Validated,
PendingApply, Applied}`. A status snapshot is valid only when its body state,
phase, height context, lock, and last committed subject satisfy the revision-4
cross-field invariants.

## Feature-isolated authenticated fault selection

The adversarial test daemon has a feature-only inbound message controller. It
runs after P2P authentication and before ordinary consensus ingress; it is not
a production configuration surface and cannot disable mandatory DA/RBC.

A direct `PayloadChunk` rule binds the semantic sender, the peer identity that
authenticated the received copy, the manifest hash encoded by the chunk, and
the exact chunk index. When a test cannot know the manifest hash before
submission, `payload_chunk_from_proposal` creates a deferred Hold rule for one
sender, authenticated route, proposal height/view, and chunk index. An
unresolved rule may only Hold and can provisionally retain that authenticated
index. The matching authenticated Proposal atomically fills in the manifest
hash before the retained occurrence can be released as exact target evidence.

The daemon retains at most 1,024 route-specific Proposal observations, keyed by
semantic sender, authenticated route, and manifest hash and mapped to the exact
height/view. This bounded evidence prevents a chunk already attributed to an
earlier round from wildcard-matching an unresolved future rule. If that chunk
was provisionally held before its Proposal arrived, the now-mismatched
occurrence is released in ingress order; the target Proposal instead resolves
the rule and keeps its exact manifest/index occurrence held.

## Safety and liveness consequences

- `FullBodyBeforePrepare`: every Prepare signer owns the exact durable,
  validated body.
- `FullBodyBeforeCommit`: every Commit signer retains that exact body.
- Two conflicting CommitQCs still intersect in an honest signer under the
  exact `3f + 1`, at-most-`f` Byzantine assumption.
- A node applies only the certified body it has reconstructed and validated
  locally.
- Conditional liveness requires eventual synchrony, `2f + 1` responsive
  validators, terminating storage/validation, and an eventually honest
  leader/proxy-tail view. Missing partial shards are repairable work, not a
  permanent local veto.

## Functional integration coverage

The grouped integration harness exercises the first two admissible committee
sizes with a 1 MiB canonical body:

```bash
cargo test -p integration_tests --test consensus_and_da \
  sumeragi_da::large_da_payload_commits_with_consistent_v2_subject_four_peers \
  -- --exact --nocapture

cargo test -p integration_tests --test consensus_and_da \
  sumeragi_da::large_da_payload_commits_with_consistent_v2_subject_seven_peers \
  -- --exact --nocapture
```

Each test waits for a canonical commit, validates every returned revision-4
status, and requires quorum peers at or above the target height to report one
identical committed subject. Focused unit and model coverage additionally
checks resource-cap edges, corrupted chunks, withheld evidence, volatile shard
reacquisition, restart hydration, and the one durable canonical-body boundary.

The exact authenticated-loss test starts its controller with an empty
genesis-safe revision, installs Proposal-bound Hold rules for height 2/view 0,
and submits a 10 MiB payload to four validators. It requires at least three
receivers to retain chunk indices 57, 58, and 59 under their resolved exact
manifest selectors, proves the three-of-six RS16 loss prevents premature
commit, heals through the acknowledged drain fence, and then requires all four
peers to converge on one committed subject:

```bash
scripts/cargo_fast.sh --stable-local-metadata -- test \
  -p integration_tests --test consensus_and_da \
  sumeragi_da::authenticated_payload_chunk_hold_heals_and_converges_four_peers \
  -- --nocapture --test-threads=1
```

The test and controller path are present in the current tree. A fresh
current-tree acceptance result is still pending at this checkpoint, so this
document does not claim that integration command passed.

## Performance and fault evidence

`scripts/run_sumeragi_stress.py` runs the revision-4 cadence baseline and bounded
transaction-queue pressure cases. Authenticated consensus-message loss and
healing are exercised by the feature-isolated Sumeragi v2 runner. The
representative soak matrix is restricted to admissible 4-, 7-, and
10-validator committees:

```bash
python3 scripts/run_sumeragi_soak_matrix.py \
  --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
  --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
```

The old 4/6-peer report in `specs/generated/sumeragi_da_report.md` predates
revision 4 and is retained only as historical telemetry. It is not current
qualification evidence; a fresh 4/7/10 matrix must supply that evidence.
