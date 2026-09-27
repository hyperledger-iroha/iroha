# F10 Parliament private-ballot restart source audit — 2026-09-24

This is a source audit of the current `optimizations` checkout, not a release
qualification run. It changes no Parliament protocol or production gate.

## Existing fail-closed ownership

- The [private-ballot reducer](../../../crates/iroha_core/src/governance/parliament/reducer_ballot.rs)
  derives the exact TLE session from the ballot, key session, logical beacon,
  and release height; a retry requires a fresh session and the next sequence.
  Its opening transition requires the complete sorted set of awaiting ballots
  for one session-height slot, checks the accepted anonymity floor and opening
  deadline, and consumes the pulse ID and slot once. `NoResult` derives its
  reason from the frozen phase and height, rather than accepting a caller's
  reason.
- The [Core instruction path](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)
  checks the canonical network-wide logical beacon, looks up a stored pulse by
  its exact ID and height, and re-verifies the persisted pulse's public DKG and
  final signature before `BeginBallotOpeningBatch`. `FailBallotNoResult` obtains
  pulse availability from the verified canonical slot, rather than a submitted
  availability bit. Its deterministic failure checkpoint is permissionless but
  still bound to the authoritative attempt and phase.
- The [restore validators](../../../crates/iroha_core/src/state/deserialize_world.rs)
  rebuild and check the pulse-slot and Parliament indexes, reject finalized
  pulse history that contradicts a slot terminally classified as unavailable,
  and cross-check ballot status against persisted timed-OVN lifecycle evidence.
  The [daemon startup path](../../../crates/irohad/src/main/runtime_deps.rs)
  checks exact local beacon and TLE signer custody for current and
  deadline-retained sessions without signing during readiness checks.

## Evidence boundary

The [four-validator positive corridor](../../../integration_tests/tests/sora_parliament_lifecycle_smoke.rs)
contains proof-valid timed-OVN aggregate opening, a finalized beacon release,
revision-4 finality checks, and one validator restart after enactment. The
[private retry corridor](../../../integration_tests/tests/sora_parliament_private_ballot_retry.rs)
uses actual signed lifecycle transitions, two TLE-distinct ballot attempts,
registration-deadline failures, four-validator finality, and a validator
restart after each failed attempt. Its own source explicitly excludes positive
aggregate opening. The [integration test instructions](../../../integration_tests/README.md)
explicitly leave later-phase private deadline retries and partial-write
rollback for separate four-validator coverage. No run of either network target
against a frozen release candidate was performed in this audit. The
`parliament-test-signers` provider is feature-isolated test custody, not
deployment signer qualification.

The next bounded test cut is to extend the private retry corridor through a
proof-valid registration and frozen survivor/corpus phase, trigger one later
objective deadline, and restart a validator on each side of that transition.
Compare all four peers' exact Parliament state bytes, block hash and 3-of-4
finality before and after restart; submit a rejected transition after a staged
partial effect and assert no state write escapes. Run the positive and negative
corridors with matching same-source binaries and retain the command, artifact
hashes, logs and authenticated finality receipts. Independent timed-OVN and
threshold-BLS review and deployment software-custody qualification remain
separate release gates. F10 remains open.
