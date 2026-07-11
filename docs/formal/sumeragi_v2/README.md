# Sumeragi v2 formal model

This directory contains the compact formal model for the executable Sumeragi
v2 reducer. It is intentionally split by proof concern:

- `SumeragiV2Quorums.tla` defines epoch-frozen ordered rosters, count and
  voting-power thresholds, and a finite one-token-per-power-unit encoding.
- `SumeragiV2QuorumProofs.tla` proves count, power, and dual-quorum honest
  intersection using finite-set inclusion/exclusion.
- `SumeragiV2Availability.tla` defines durable-body and deterministic-validation
  boundaries.
- `SumeragiV2Core.tla` models per-validator views, locks, highest PrepareQCs,
  persistence and signing stages, asynchronous addressed delivery, grouped
  timeout certificates, future-view TC catch-up, old-view CommitQCs, certified
  body fetch, Byzantine control messages, and application.
- `SumeragiV2CrashRecovery.tla` states the acknowledged-complete-prefix WAL and
  restart-generation obligations.
- `SumeragiV2Reconfiguration.tla` binds a new height context to the applied
  parent decision and frozen epoch inputs.
- `SumeragiV2SafetyLemmas.tla` proves the compositional durable-append,
  same-view certificate, validity/availability, lock, and grouped-timeout
  kernels.
- `SumeragiV2Proofs.tla` is the top-level ledger: checked lemmas are theorems;
  end-to-end reducer-network induction and liveness remain named predicates.
- `SumeragiV2.tla` is a thin compatibility entry point for TLC.
- `proof_coverage.json` is the authoritative machine-readable proof ledger.

## Exact abstractions

`LeaderStarts[h + 1]` is the already-computed production value
`H(epoch_seed, h) mod roster_len`; the model does not replace that hash with
height arithmetic. `ContextRecord` binds chain/protocol identity, parent,
epoch roster and powers, lane hash, DA layout, and leader start.

Timeout votes report the validator's highest durable PrepareQC, not merely its
lock. A TC contains distinct signer votes, requires a dual-quorum union, rejects
different subjects at the same highest QC view, and selects the unique maximum.
TC installation may jump directly from a lower local view to `tc.view + 1`, as
the executable WAL does. Old-view CommitQCs remain admissible.

Persistence requests and their fsync acknowledgements are separate actions.
Proposal, Prepare, Commit, and Timeout signatures are enabled only after the
matching acknowledged intent. Installing a TC changes view only in the
acknowledgement action. Applying a block requires a durable decision and a
durable validated body.

The ordinary model relation includes Byzantine noise, loss before GST,
crash/restart, and replay. `ReliableNextV2`, used only by `liveness.cfg`, is the
trusted-contract corridor after GST: it excludes loss, Byzantine scheduling
noise, crash/replay, and a timeout beating a responsive leader's bounded
successful round. It reuses the exact same protocol actions; weak fairness is
applied to this finite, acyclic corridor. This is a conditional trace check,
not a proof of the trusted timing/network contracts.

## Proof status

The official arm64 TLAPM 1.6.0-pre build at commit `763bf3c` discharged:

- all 218 obligations in `SumeragiV2QuorumProofs.tla`;
- all 144 obligations in `SumeragiV2SafetyLemmas.tla`; and
- all 8 theorem obligations in `SumeragiV2Proofs.tla`.

This proves count/power/dual honest intersection and the compositional kernels
for authorized durable appends, intent-backed same-view certificate uniqueness,
certificate validity/body availability, monotone locks, and grouped timeout
protection. It does **not** yet prove that every branch of `NextV2` preserves
all kernel antecedents. The end-to-end `Spec => []...` obligations for durable
history provenance, agreement, chain prefix, crash recovery, reconfiguration,
and temporal liveness remain `specified_unproved` in `proof_coverage.json`.

The handwritten argument in `PROOF.md` is review material. It does not change
the machine-readable status. TLC is a bounded counterexample search and never
upgrades an obligation to proved.

## Bounded checks

The quorum configurations are exhaustive one-state checks of two frozen
contexts. The safety configurations are finite randomized counterexample
searches over deep asynchronous traces; they are not deductive proofs. Keep
TLC metadata outside the repository:

```bash
java -cp "$TLA2TOOLS_JAR" tlc2.TLC -metadir /tmp/sumeragi-qc-count -config quorum_count.cfg SumeragiV2
java -cp "$TLA2TOOLS_JAR" tlc2.TLC -metadir /tmp/sumeragi-qc-stake -config quorum_stake.cfg SumeragiV2
java -cp "$TLA2TOOLS_JAR" tlc2.TLC -noGenerateSpecTE -metadir /tmp/sumeragi-safety-count -workers 4 -depth 100 -simulate num=1000 -config safety_count.cfg SumeragiV2
java -cp "$TLA2TOOLS_JAR" tlc2.TLC -noGenerateSpecTE -metadir /tmp/sumeragi-safety-stake -workers 4 -depth 100 -simulate num=1000 -config safety_stake.cfg SumeragiV2
java -cp "$TLA2TOOLS_JAR" tlc2.TLC -noGenerateSpecTE -metadir /tmp/sumeragi-liveness -workers 2 -depth 150 -simulate num=100 -config liveness.cfg SumeragiV2
```

Run the deductive ledger with:

```bash
tlapm SumeragiV2QuorumProofs.tla
tlapm SumeragiV2SafetyLemmas.tla
tlapm SumeragiV2Proofs.tla
```

Do not turn an end-to-end obligation predicate into `THEOREM` merely because a
compositional kernel is proved. Upgrade it only after TLAPS discharges the full
action induction or temporal proof without an unchecked axiom, omitted proof,
or trusted shortcut, and update the coverage manifest in the same change.
