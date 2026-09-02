# Atomic private settlement model-check evidence

This file separates mutable-checkout development evidence from release
evidence. The rows below were run from frozen exact inputs on 2026-09-01, but
the checkout was not a clean settled commit. They are historical development
evidence only. The models now include a globally replicated Prepare
registration, exact Commit binding, terminal lock release, crash/restart
preservation, and a Commit-without-registration negative control; those changes
postdate every result below. The release runner must repeat the complete
ordered matrix from one immutable candidate before any row is treated as
current-source or release evidence.

Local evidence captured on 2026-08-29 against repository base commit
`1bdec3b88c348a84776241839fb0e8ad71738b3e` with uncommitted implementation
changes present.

- TLC: TLA+ tools 1.7.4 / TLC 2.19, pinned JAR SHA-256
  `936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`.
- Java: Homebrew OpenJDK 21.0.12.1 on macOS arm64.
- Model SHA-256 at the run:
  `a7c7f3670052e59e29c9390acdae79cd9babeab04698c4bbf9295a98cf27b667`.
- Runner: `scripts/formal/run_atomic_private_settlement_tlc.sh`.

The then-current count-model bytes and six configurations were repeated on
2026-09-01 with four workers through the current result contract. All three
positive rows reproduced the table below with status 0 and empty stderr; all
three mutations reproduced the required invariant status 12. Frozen-input
development logs are retained under
`/tmp/aps-tlc-count-rest.JIotsT/evidence`. As with the indexed rows, this is a
mutable-checkout partial run and not a complete release report.

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

## Superseded committee-indexed refinement run

The historical committee-indexed module SHA-256 at that run was
`acb9dab11739fd2d5f8f2aaa49aad65bbcdda1124a4984a0161db625c38b68f6`.
It was checked with TLA+ tools 1.7.4 / TLC 2.19, pinned JAR SHA-256
`936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`,
Homebrew OpenJDK 21.0.12.1, four workers, fingerprint index `0`, and seed
`20260829`. SANY accepted both formal modules. Every passing row exited `0`,
left zero queued states, emitted empty stderr, and ended with the exact TLC
success marker.

| Configuration | Generated | Distinct | Depth | Result |
| --- | ---: | ---: | ---: | --- |
| 2 legs, validator faults only | 4,027 | 1,136 | 23 | pass |
| 2 legs, per-committee validator/auditor/channel faults plus committee/global crash | 329,407 | 58,085 | 27 | pass |
| 3 legs, paper-primary full fault and crash mix | 75,562,941 | 8,898,534 | 39 | pass |
| 4 legs, clean indexed path | 1,762 | 598 | 39 | pass |
| 3 legs, abort/expiry and replay rejection | 1,082 | 488 | 31 | pass |
| 2 legs, deliberate staged-record loss on committee crash | 39 | 28 | 10 | required action-property violation, status 13 |

The N=3 run completed in 15 minutes 36 seconds. Its retained development
evidence is `/tmp/aps-tlc-vote-gc-n3.Jymyk7/evidence`; the other current indexed
rows are retained under `/tmp/aps-tlc-indexed-rest.sd6XLd/evidence`. Those
directories contain frozen module/configuration inputs, SANY output, separate
TLC stdout/stderr, statuses, Java version output, and the frozen evidence
tooling. Temporary TLC state queues were removed automatically after success.
These machine-local paths are reproducibility notes, not DOI artifacts.

Those passing liveness configurations checked `Safety`, transition-level
`APSDurabilityTemporal`, transition-level
`APSCertificateQuorumTemporal`, and their terminal liveness property. Vote
collections remain exact committee-member sets until a certificate is created.
The certificate transition checks the pre-state 3-of-4 quorum, then discards
the no-longer-observable vote history; the durable QC remains an opaque marker,
not a signer bitmap. This argument assumes traces begin at the empty `Init`.
A future recovered-state initializer must carry its own QC provenance witness.

Validator identities are exchangeable in this model, so the sole possible
faulty member is represented by validator 1. Once a leg has its Commit QC, its
local volatile fault history is future-inert and is canonicalized. Channel
faults are introduced at the first phase where they can affect delivery; the
model has no wall clock, so an earlier injection is trace-equivalent. Concrete
Hold, Drop, and Delay timing remains a real-process obligation.

Expiry is an adversarial nondeterministic transition representing the point at
which the chain crosses the configured expiry height. The indexed model proves
terminal byte-silence and replay behavior, not the passage or latency of block
height itself. Crash-boundary labels likewise enable a crash once a boundary
has been reached; exact process cuts and persistence acknowledgements remain
real-network evidence obligations. Durability is stronger than a crash-only
check because it is asserted across every modeled transition.

The historical staged-loss mutation configuration exited `13` with exactly
`Error: Action property APSDurabilityTemporal is violated.` and its behavior
trace. Invariant negative controls remain a distinct status-`12` contract. The
formal report producer, runner, shell result contract, and DOI verifier all
enforce that distinction.

The earlier bundle-global-fault, pre-canonicalization, and tabled indexed runs
are superseded. In particular, the table does not qualify the replicated
Prepare-registration lifecycle or its new negative control. Even a new passing
formal run would prove only the bounded abstract state machine, not
cryptographic soundness, transport/storage implementations, real-time latency,
or release readiness. A clean signed candidate, complete ordered formal matrix,
independent review, and DOI-backed archive remain mandatory.
