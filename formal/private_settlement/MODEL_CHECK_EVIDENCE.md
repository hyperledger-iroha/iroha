# Atomic private settlement model-check evidence

Local evidence captured on 2026-08-29 against repository base commit
`1bdec3b88c348a84776241839fb0e8ad71738b3e` with uncommitted implementation
changes present.

- TLC: TLA+ tools 1.7.4 / TLC 2.19, pinned JAR SHA-256
  `936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`.
- Java: Homebrew OpenJDK 21.0.12.1 on macOS arm64.
- Model SHA-256 at the run:
  `a7c7f3670052e59e29c9390acdae79cd9babeab04698c4bbf9295a98cf27b667`.
- Runner: `scripts/formal/run_atomic_private_settlement_tlc.sh`.

Positive results:

| Configuration | Generated | Distinct | Depth | Result |
| --- | ---: | ---: | ---: | --- |
| 3 legs, one crash, bounded liveness | 93 | 64 | 21 | pass |
| 255 legs, one crash, bounded liveness | 199,425 | 100,990 | 1,281 | pass |
| 3 legs, invalid/expiry terminals | 233 | 134 | 21 | pass |

All three negative controls failed with `Invariant Safety is violated`, as
required:

- partial global application;
- Commit before the complete Prepare barrier; and
- loss of a durably staged leg across crash.

This count-symmetry abstraction covers the cross-leg barriers, atomic
visibility, idempotency, expiry, crash recovery, and bounded liveness under
3-of-4 committee availability. It does not prove the STARK relation,
cryptographic implementations, network transport, canonical ordering code, or
storage implementation. Those remain separate test and independent-audit
obligations.

## Committee-indexed refinement

The committee-indexed refinement was corrected on 2026-08-29 so validator,
auditor, and authenticated-channel fault budgets are per leg rather than one
scalar budget shared by the complete bundle. With a configured budget of one,
each committee may now have one unavailable or Byzantine validator, one local
auditor outage, and one impaired DA/Prepare/Commit channel in the same
execution. The validator identity remains stable within each committee.

The corrected module was checked with the same pinned TLA+ tools 1.7.4 / TLC
2.19 JAR and Homebrew OpenJDK 21.0.12.1 on macOS. Its SHA-256 was
`e63846d681911a26157e57fad0b72818f5dad712097d621de1720fa041d454f0`.

SANY completed semantic processing successfully:

```text
<java-21> -cp <tla2tools-1.7.4.jar> tla2sany.SANY \
  formal/private_settlement/AtomicPrivateSettlementV1CommitteeFaults.tla
```

Tractable TLC results against that exact corrected module:

| Configuration | Generated | Distinct | Depth | Result |
| --- | ---: | ---: | ---: | --- |
| 2 legs, one validator fault per committee, other fault/crash budgets zero | 712,894 | 253,678 | 31 | pass |
| 4 legs, clean indexed path | 1,762 | 598 | 39 | pass |
| 3 legs, abort/expiry and replay rejection | 1,082 | 488 | 31 | pass |

The exact TLC invocation shape for each completed row was:

```text
cd formal/private_settlement
<java-21> -XX:+UseParallelGC -cp <tla2tools-1.7.4.jar> tlc2.TLC \
  -cleanup -workers auto -fp 0 -seed 20260829 \
  -metadir <temporary-metadir> \
  -config AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg \
  AtomicPrivateSettlementV1CommitteeFaults.tla

<java-21> -XX:+UseParallelGC -cp <tla2tools-1.7.4.jar> tlc2.TLC \
  -cleanup -workers auto -fp 0 -seed 20260829 \
  -metadir <temporary-metadir> \
  -config AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg \
  AtomicPrivateSettlementV1CommitteeFaults.tla

<java-21> -XX:+UseParallelGC -cp <tla2tools-1.7.4.jar> tlc2.TLC \
  -cleanup -workers auto -fp 0 -seed 20260829 \
  -metadir <temporary-metadir> \
  -config AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg \
  AtomicPrivateSettlementV1CommitteeFaults.tla
```

All three completed with `Model checking completed. No error has been found.`
They used TLC fingerprint index 0 and seed `20260829`. The focused validator
run and four-leg clean run checked `Safety` and
`APSEventuallyFinalizedAndPublished`; the expiry run checked `Safety` and
`APSExpiryEventuallyRejectsReplay`. The focused run exhaustively checks the
corrected state space with up to one static validator fault independently in
each committee. It does not include auditor, channel, or crash faults and does
not replace the complete two-leg configuration.

The earlier two-leg bounded-fault result at module SHA-256
`2f4ba4fc69a354f17d156c3d319c9c3c4bdda4deae9318135c6fa5025c5cda8e`
is superseded and is not release evidence: that revision allowed only one
validator, auditor, and channel fault across the entire bundle. The corrected
two-leg bounded-fault and three-leg paper-primary fault configurations are
provided but have not completed TLC and are not claimed here. Until both are
run, the repository has semantic and clean/expiry evidence for the corrected
module, but no completed combined state-space claim spanning simultaneous
per-committee validator, auditor, and channel faults with the crash schedule.

The corrected abstraction represents committee separation, per-leg
authenticated-channel faults, per-leg local-auditor availability, static
`f = 1` equivocation, global quorum, and every named durability boundary. It
still does not prove transport or storage implementations, cryptographic
soundness, or real-time latency bounds.
