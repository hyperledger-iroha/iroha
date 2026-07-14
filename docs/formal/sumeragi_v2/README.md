# Sumeragi v2 formal verification

This directory is the first-release formal corridor for the production
Sumeragi v2 consensus protocol. There is no legacy Sumeragi proof corridor.
The model fixes protocol revision 3 and proves over arbitrary finite frozen
rosters; production separately enforces the release limit of 128 validators.

## Modules

- `SumeragiV2Quorums.tla` and `SumeragiV2QuorumProofs.tla` define and prove
  strict count-and-power quorum intersection.
- `SumeragiV2Availability.tla`, `SumeragiV2CrashRecovery.tla`, and
  `SumeragiV2Reconfiguration.tla` define durable-body, WAL, restart, and frozen
  height-context boundaries.
- `SumeragiV2Core.tla` models addressed asynchronous delivery, durable intents,
  locks and highest PrepareQCs, grouped timeout certificates, future-view
  catch-up, old-view CommitQCs, body recovery, decisions, and application.
- `SumeragiV2SafetyLemmas.tla`, `SumeragiV2AgreementLemmas.tla`,
  `SumeragiV2Inductive.tla`, `SumeragiV2InductiveProofs.tla`, and
  `SumeragiV2Proofs.tla` contain the action-by-action safety induction and its
  end-to-end theorems.
- `SumeragiV2ChainEpoch.tla`, `SumeragiV2ChainEpochProofs.tla`, and
  `SumeragiV2ChainEpochRefinement.tla` prove prefix-comparable per-validator
  histories and frozen epoch routing from exact durable CommitQC decisions and
  exact local application receipts. Certification and local application do not
  use a global all-node barrier.
- `SumeragiV2AsyncNetwork.tla`, `SumeragiV2LivenessProofs.tla`, and
  `SumeragiV2AsyncLivenessProofs.tla` model the production scheduler and
  transport and prove conditional progress after GST. Logical views are
  unbounded in the deductive liveness abstraction; finite TLC configurations
  remain counterexample searches only.
- `proof_coverage.json` is the authoritative theorem/trust-boundary ledger.
  Tool output and obligation counts belong only in generated evidence under
  `target/formal/sumeragi_v2/`.

## Exact protocol abstractions

`ContextRecord` binds the chain and protocol identities, semantic parent
finality, height, epoch, canonical roster and powers, lane/DA commitments, and
the already-computed production leader start. Certificates must satisfy both
`3 * signer_count > 2 * voter_count` and
`3 * signer_power > 2 * total_power`; observers never pad either threshold.

Honest Proposal, Prepare, Commit, and Timeout signatures require their matching
acknowledged WAL intent. A timeout vote carries the full highest durable
PrepareQC, and a TC contains disjoint signer groups whose union independently
satisfies both quorum thresholds. Installing a TC may move one validator to
`tc.view + 1`; it does not require other validators to install first. An
old-view CommitQC remains decisive after a view change.

A certified chain slot is created only from the exact valid CommitQC in a
durable decision receipt for the canonical parent context. Each validator
fetches, reconstructs, validates, durably applies, and advances independently
from its own exact receipt. Lagging validators remain on their certified
prefix and cannot be advanced by another node's receipt.

The asynchronous model includes pre-GST loss, duplication, reordering, crash,
and replay. After GST it models bounded authenticated per-source transport,
normal/progress/completion ingress reserves, absolute timeout priority,
periodic retransmission, FIFO debt, stale-completion rejection, manifest and
chunk recovery, validation, and independent application. The scheduler choice
matches the source-linked production kernel:

1. absolute timeout;
2. an owed FIFO command;
3. one periodic retransmission;
4. the oldest FIFO command; or
5. idle service.

Textual TLA+ disjunction order is never treated as priority; the selected-work
operator makes the branches mutually exclusive.

## Theorem scope and FLP boundary

Safety is asynchronous: it permits arbitrary delay, loss, duplication,
reordering, Byzantine messages within authenticated identities, and crashes at
effect boundaries. The proved properties cover durable sign-once behavior,
external validity, certified-body availability, lock and timeout protection,
agreement, absence of conflicting CommitQCs, crash/restart preservation,
chain-prefix safety, and epoch-context isolation.

Liveness is necessarily conditional. FLP rules out unconditional deterministic
consensus termination in a fully asynchronous network. The post-GST theorem
therefore has explicit premises: a non-crashing honest set independently meets
both quorum thresholds; authenticated retransmissions and serialized service
have declared finite bounds; the monotonic clock and run loop continue; and
admitted fsync, signature, reconstruction, deterministic validation, and local
application work terminates within the configured successful-round bound.
Under those premises, failed views form and install TCs, rotation reaches a
responsive honest leader, a safe round decides, every responsive validator
eventually applies the certified body, and each local chain advances.

The theorem is consensus-height progress, not transaction fairness. A valid
empty heartbeat can satisfy progress. Transaction inclusion, mempool fairness,
and censorship resistance are explicitly out of scope in the proof ledger.

## Evidence and release gate

The release gate uses TLAPM commit
`763bf3c1826d77a4cf206f43d5aa16775da1da33`, TLA2Tools 1.8.0, and Verus
`0.2026.05.31.5dd6d83`. The strict TLAPS runner checks every deductive module,
then generates evidence bound to the exact ordered module list, every proof
log, the pinned tool identity, and a SHA-256 manifest of every TLA+ source.
The checked-in ledger cannot contain stale tool-run counts.

The structural checker rejects top-level TLA+ assumptions/axioms, omitted
proofs, Verus assume/admit/trusted-body escapes, non-theorem ledger targets,
retired Sumeragi paths, and the former favourable-network liveness corridor.
`machine_checked_completion=true` is accepted for release only when no
`specified_unproved` entry remains and fresh source-bound evidence validates.

Run the full gate from the repository root:

```bash
bash scripts/formal/install_sumeragi_v2_tlapm.sh
bash scripts/formal/install_sumeragi_v2_tla2tools.sh
bash scripts/formal/install_sumeragi_v2_verus.sh
bash ci/check_sumeragi_formal.sh
```

TLC searches the six bounded configurations for counterexamples and never
changes proof status. The model-trace replayer drives the exact production
reducer API. The source-linked Verus harness proves the reducer/WAL and
scheduler kernels, runs the required adversarial simulations, and retains its
log under `target/formal/sumeragi_v2/`. The ignored 100,000-height chaos test is
an additional implementation stress gate, not deductive evidence.

## Trusted computing boundary

The proof ledger keeps the remaining premises explicit:

- signature authenticity and collision resistance;
- faithful complete-frame `fsync` acknowledgement;
- deterministic reconstruction, validation, and execution;
- a responsive honest dual quorum after GST;
- bounded post-GST authenticated transport; and
- continuing clock/run-loop service plus termination of admitted local work.

TLAPM, its backends, Verus, vstd, the SMT solver, the Rust compiler, the
production-to-proof extraction code, the operating system, and the hardware
remain part of the implementation proof TCB. Generated evidence attests that
the configured tools accepted the exact sources; it is not a cryptographic
attestation of the host that ran them.
