# Current-source ZK network qualification

Status: normal production build failed; runtime controls unexecuted. This record
replaces the retired replay-binding network plan for the current APIs. It does
not qualify a complete IVM execution proof or current production release.

## Fixture migration

The shared proof fixture uses `ConfidentialProver` to create a real one-note full
unshield, with the canonical key, schema, circuit identity and proof-byte cap.
Its synthetic network and commitment root are self-consistent public statements.
Generic `VerifyProof` records verification outcomes; these synthetic statements
do not authorize a ledger spend or change the confidential tree.

The native negative clones the valid proof and flips exactly one `PROF` byte.
The test requires its `I10P` bytes and all envelope identity fields to remain
unchanged. The network-binding control checks all eight actual public columns:
the network tag and active nullifier change, while other columns stay fixed.
Fresh proof randomness alone cannot satisfy that assertion. The full-capacity
wallet's wrong-role negative uses the retained confidential-transfer relation.

The proof-record scenario quotes the exact prepared transaction, checks the
payer and gas bound, installs the quote and signs once before submission to all
four peers. Fee failures cannot serve as proof-verification evidence.

The current `IvmProved` case first executes an ordinary contract call and checks
the resulting counter. It then requires the exact unavailable-execution-relation
rejection at every ingress, both with an empty overlay and changed nonempty
overlay. Early typed admission rejection and durable committed rejection have
separate evidence checks; an early rejection need not fabricate a committed
record. Independent Applied barriers and all-peer counter reads establish
progress and unchanged state. Transport, timeout, expiry and fee errors cannot
satisfy the rejection control.

## Captured candidate and plan

The managed `zk-current-network` checkout starts at
`24789950586efd7fa3a6cc73a96f4acd5d760bf9` and contains 32 exact local amendments
across a 20,543-file source census. Capture completed without source drift.
The source manifest SHA-256 is
`bec3119129d80d4587a665df0e93a16f52ab5191f3f8eb17bb65e7d6cc893b92`.
The candidate's inherited Git root was corrected only in its worktree-specific
configuration; the shared repository configuration stayed byte-identical.

The reviewed normal-Cargo runner builds the daemon, CLI and attachment sanitizer,
plus the `queries_and_proofs`, `events_and_triggers` and `core_api` harnesses with
the actual Halo2/STARK features. It retains all six compiler-reported binaries,
checks their hashes before every invocation, checks exact source manifests, and
retains the executed runner. There are eight exact native controls followed by
six sequential four-validator scenarios: proof records, events, queries, ordinary
transfer simulation, the full-capacity wallet and unavailable IVM execution.
Mandatory-network mode rejects skipped execution. Historical successful replay
tests cannot substitute for this current candidate.

Independent source review accepted the relation/key/schema migration and the
fee, public-column and single-byte-corruption corrections. Scoped Rust formatting,
diff checks and the retired-codec guard pass.

## Normal build failure

The ordinary debug production build, with two Cargo jobs and no profile overrides,
exited 101 after 316.19 seconds. All 20,543 candidate files remained unchanged.
Core reports 23 errors: 22 unresolved imports from the unfinished incoming
Sumeragi migration and an incomplete lane `Executor` implementation missing
`build_control_witness`, `drive_control` and `receive_application_control`.
The same migration failure is documented in the [merge validation](merge-validation.md).

No daemon/CLI/sanitizer bundle was captured, and neither the eight native controls
nor six network scenarios ran. Their ZK fixture migration remains uncompiled.
Qualification requires the migration to be completed on a new captured source;
restoring retired consensus modules or adding no-op trait methods would not
satisfy it. The frozen candidate stays intact for standalone FASTPQ qualification.
The failed command, log, post-failure source census and receipt are retained under
`dist/zk-remediation/2026-09-29/current-network-20260929T0540Z/`.

Local retained evidence is under
`dist/zk-remediation/2026-09-29/current-network-candidate/`, with the selector plan,
review receipts and `run-current-network.py` in its parent directory. These are
local reproducibility records, not signed release artifacts.
