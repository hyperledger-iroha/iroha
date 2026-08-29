# Atomic private settlement formal model

The formal package has two complementary models.

`AtomicPrivateSettlementV1.tla` is the count-symmetry model for the production
phase barriers and durability edges. It checks the full `2..255` participant
bound without enumerating interchangeable leg subsets:

- every sidecar is certified by an exact 3-of-4 committee;
- a Prepare QC follows durable staging;
- Commit starts only after the complete ordered Prepare barrier has one exact
  digest;
- global application changes either every leg or no leg;
- abort and expiry are byte-silent for private state;
- replay cannot apply a finalized bundle twice; and
- one crash/restart does not discard certified sidecars, staged deltas, QCs,
  the carrier, or the receipt.

`AtomicPrivateSettlementV1CommitteeFaults.tla` is the bounded, committee-indexed
refinement. It does not reduce legs to counts. Instead, it explicitly retains:

- the canonical ordinal and abstract unique route for every one of `2..255`
  ordered legs;
- one independent four-validator committee per leg, a stable `f = 1` fault
  identity, and exact 3-of-4 DA, Prepare, and Commit vote sets;
- unavailable and Byzantine/equivocating validators, including the proof that
  one equivocator cannot certify a conflicting digest;
- one independently available local auditor per leg;
- authenticated DA, Prepare, and Commit channels in Deliver, Hold, Drop, or
  Delay state, with retransmission after healing;
- a coordinator and an explicit 3-of-4 global quorum;
- committee, coordinator, and global crash/restart at sidecar fsync, staged
  delta fsync, Prepare QC, Commit QC, Kura append, WSV application, and receipt
  publication; and
- durable crash floors, atomic visibility, idempotent application, expiry lock
  release, replay rejection, and bounded liveness under eventual recovery.

The possible faulty validator identity is stable within each committee. This
matches the stated static `f = 1` assumption and prevents a mobile adversary
from accumulating old signatures from several identities. Validator, auditor,
and authenticated-channel fault budgets are separate finite counters for every
leg. A per-leg budget of one therefore permits one unavailable or Byzantine
validator, one auditor outage, and one DA/Prepare/Commit channel impairment in
every committee in the same execution. The channel budget is shared by the
three channels of that leg. Crash budgets remain separate bounded scalars.

The primary paper configuration is `AtomicPrivateSettlementV1_3.cfg`.
`AtomicPrivateSettlementV1_255.cfg` exercises the protocol maximum, and
`AtomicPrivateSettlementV1_expiry.cfg` explores invalid/expiry terminal paths.
The model includes deliberate `PartialApply`, `CommitBeforeAllPrepare`, and
`DropStageOnCrash` mutations. Their `*_bug.cfg` configurations are negative
controls and must violate the corresponding safety invariant.

The primary indexed configuration is
`AtomicPrivateSettlementV1CommitteeFaults_3.cfg`. It permits one finite
validator/auditor/channel fault independently in each of three committees and
selects each crash class nondeterministically. The bounded
`AtomicPrivateSettlementV1CommitteeFaults_2.cfg` is the focused per-committee
fault-model check. The tractable
`AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg` isolates one
validator-fault budget in each of two committees while disabling the other
fault and crash classes. `AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg`
extends the clean indexed path to four committees, and
`AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg` checks expiry-to-replay
rejection. The per-committee correction invalidated the earlier two-leg
bounded-fault result; the current N=2 and paper-primary N=3 fault configurations
remain unclaimed until their full TLC runs complete. These bounded
configurations are not a substitute for the 255-leg count-symmetry run.

Run the model with the repository-pinned TLA+ toolchain, once installed:

```text
TLA2TOOLS_JAR=<tla2tools.jar> \
JAVA_BIN=<java> \
scripts/formal/run_atomic_private_settlement_tlc.sh
```

The indexed refinement can also be checked directly with the same authenticated
toolchain:

```text
<java> -cp <tla2tools.jar> tla2sany.SANY \
  formal/private_settlement/AtomicPrivateSettlementV1CommitteeFaults.tla

cd formal/private_settlement
<java> -cp <tla2tools.jar> tlc2.TLC -cleanup -workers auto \
  -config AtomicPrivateSettlementV1CommitteeFaults_2.cfg \
  AtomicPrivateSettlementV1CommitteeFaults.tla
```

Passing this abstraction is necessary evidence, not a substitute for the real
four-validator crash/loss matrix or an independent protocol and cryptography
review.
