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
