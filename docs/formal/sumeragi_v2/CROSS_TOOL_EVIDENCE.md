# Sumeragi v2 cross-tool refinement evidence

Proof-ledger schema version 2 adds `cross_tool_proved`, a release status for a
production-refinement seam that
cannot be discharged by TLAPS alone. It is deliberately narrower than
`tlaps_proved`: only the three production obligations listed below may use it,
and the proof-ledger checker reconstructs the complete expected evidence from
checked-in code. Evidence cannot choose theorem names, claims, source paths, or
dependencies.

The status is release-eligible only when all of the following hold together:

1. the obligation's named conditional TLA+ refinement theorem exists with its
   exact reviewed statement and an explicit proof body; its consequent is the
   exact ledger theorem symbol, never a repeated premise or an evidence-owned
   surrogate;
2. a fresh strict TLAPS module log is bound to the current canonical formal
   source manifest and the pinned TLAPM commit
   `3ab43c7ff31db4ced850619d4746fa4c841a7681`;
3. every mapped claim has exactly one named public Verus `proof fn` with its
   exact normalized parameters, non-vacuous and non-contradictory `requires`,
   reviewed nontrivial `ensures`, and one exact invocation of its projection
   builder and verified kernel;
4. the linked Verus evidence passes the pinned `--no-cheating` invocation,
   verifier binary identities, exact result counts, transcript markers, and
   current workspace source-manifest checks;
5. each production boolean kernel is a pure shared Rust/Verus predicate with
   the exact reviewed signature and body, and every authoritative production
   call item has an exact token seal with no unfrozen call item remaining;
6. each claim records the current SHA-256 digest of its Verus source and every
   code-owned production source in its exact ordered inventory;
7. every prerequisite in the complete transitive dependency closure has
   either `tlaps_proved` or `cross_tool_proved` status; and
8. each named TLA+ production premise contains its exact ordered `4 + 7 + 6`
   constants, with every mapped constant required to equal `TRUE`; and
9. the cross-tool document exactly matches the source-bound canonical ledger
   and its digest, TLAPS evidence digest, Verus evidence digest, tool
   identities, source manifests, and canonical claim mapping.

Changing a theorem, source, log, tool, dependency status, ledger field, claim
name, claim order, or component-evidence document invalidates the evidence.
The release checker also rejects `cross_tool_proved` on any other obligation
and rejects the three production seams if they are relabeled
`tlaps_proved`.

For each of the three contracts, the named conditional theorem has the form
`Production...Refinement => <exact ledger theorem>`. In particular, the
successor theorem must reach
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`,
whose statement contains both the six production claims and the indexed
model-side invariant. A tautology such as `P => P` is rejected even if a
strict TLAPS transcript is supplied for the substituted source.

The four `nexus::lane_relay::tests` backpressure/ownership tests remain useful
ordinary unit regressions, but they are not production-refinement evidence.
`LaneRelayBroadcaster` currently has no non-test constructor or runtime call
edge and no fairly scheduled production retry owner. Consequently those tests
are excluded from the production liveness inventory; adding them to a release
list cannot discharge any of the trace claims below. They may be reclassified
only after the runtime path and its fairness/ownership mapping are themselves
source-bound and proved.

## Canonical obligations and claims

The checker owns the authoritative inventory in
`scripts/formal/check_sumeragi_v2_proof_ledger.py`. The table below is a review
aid; it does not authorize evidence changes.

### Effective locked-body acquisition (4 claims)

Ledger obligation:
`effective-lock-body-acquisition-production-refinement`.

Named TLA+ theorem:
`SumeragiV2AsyncLivenessProofs!EffectiveLockBodyAcquisitionCrossToolRefinement`.

Verus source:
`crates/iroha_sumeragi_core/src/effective_lock_verus_proofs.rs`.

| TLA+ production constant | Required Verus theorem |
| --- | --- |
| `ProductionEnterViewUsesPostInstallEffectiveLock` | `production_enter_view_uses_post_install_effective_lock` |
| `ProductionBodyOwnershipPreservesEffectiveLock` | `production_body_ownership_preserves_effective_lock` |
| `ProductionBodyCapacityRetirementPreservesEffectiveLock` | `production_body_capacity_retirement_preserves_effective_lock` |
| `ProductionBodyServiceRefinesAsyncFairness` | `production_body_service_refines_async_fairness` |

Its dependency closure includes
`effective-lock-body-acquisition-model`.

### Durable progress witness (7 claims)

Ledger obligation: `progress-witness-production-refinement`.

Named TLA+ theorem:
`SumeragiV2AsyncLivenessProofs!ProgressWitnessCrossToolRefinement`.

Verus source: `crates/iroha_sumeragi_core/src/verus_proofs.rs`.

| TLA+ production constant | Required Verus theorem |
| --- | --- |
| `ProductionDurableIntentTraceRefinesProgressWitness` | `production_durable_intent_trace_refines_progress_witness` |
| `ProductionDecisionTraceRefinesRecoveryWitness` | `production_decision_trace_refines_recovery_witness` |
| `ProductionSchedulerTraceRefinesProtectedOwnership` | `production_scheduler_trace_refines_protected_ownership` |
| `ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership` | `production_ingress_identity_and_class_trace_refines_protected_ownership` |
| `ProductionTwoStageRelayRetryTraceRefinesSourceFairness` | `production_two_stage_relay_retry_trace_refines_source_fairness` |
| `ProductionReliableFlushTraceRefinesOutboundOwnership` | `production_reliable_flush_trace_refines_outbound_ownership` |
| `ProductionApplicationTraceRefinesDecisionCompletion` | `production_application_trace_refines_decision_completion` |

Its dependency closure includes runner scheduler preservation; the async type
and ownership invariants; generation-scoped delivery; post-decision timeout
exclusion; durable decision recovery; and async fair-action refinement.

### Successor activation and exact recovery (6 claims)

Ledger obligation:
`successor-activation-exact-recovery-production-refinement`.

Named TLA+ theorem:
`SumeragiV2ChainEpochRefinement!SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement`.

Verus source: `crates/iroha_sumeragi_core/src/verus_proofs.rs`.

| TLA+ production constant | Required Verus theorem |
| --- | --- |
| `ProductionAppliedSuccessorTraceRefinesIndexedActivation` | `production_applied_successor_trace_refines_indexed_activation` |
| `ProductionRecoveredSuccessorTraceRefinesIndexedActivation` | `production_recovered_successor_trace_refines_indexed_activation` |
| `ProductionStartupFailureAndRestartRefinesIndexedLifecycle` | `production_startup_failure_and_restart_refines_indexed_lifecycle` |
| `ProductionHistoricalCertificateTraceRefinesIndexedAsync` | `production_historical_certificate_trace_refines_indexed_async` |
| `ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync` | `production_historical_body_pipeline_trace_refines_indexed_async` |
| `ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal` | `production_terminal_application_without_successor_activation_refines_indexed_terminal` |

The sixth claim is the authenticated Apply-boundary separation: the exact
receipt, finality artifact, context, block, and durable predecessor agree while
no successor activation is pending. It has no production `MaxHeight` input;
`MaxHeight` remains only a finite-horizon proof projection.

Its dependency closure includes the epoch-boundary proof, durable decision
recovery, and successor-activation starvation freedom.

## Evidence workflow

After the named TLA+ and Verus theorems genuinely exist and both component
proof runs have produced fresh evidence, generate the derived document with:

```sh
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --ledger docs/formal/sumeragi_v2/proof_coverage.json \
  --evidence target/formal/sumeragi_v2/proof_evidence.json \
  --verus-evidence target/formal/sumeragi_v2/verus_evidence.json \
  --write-cross-tool-evidence \
  target/formal/sumeragi_v2/cross_tool_evidence.json
```

Then require all three documents at release validation:

```sh
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json \
  --verus-evidence target/formal/sumeragi_v2/verus_evidence.json \
  --cross-tool-evidence target/formal/sumeragi_v2/cross_tool_evidence.json
```

An archived release passes its copied transcript with `--verus-log`; the
evidence still declares and hashes the canonical transcript name, while the
override lets validation read the immutable archived copy instead of mutable
`target/` state.

## Current status

The evidence architecture does not itself prove a production refinement. The
three ledger obligations above remain `specified_unproved`, but the code-owned
4 + 7 + 6 inventory now has exact named Verus signatures, non-vacuous
postconditions, shared Rust/Verus kernels, sealed projection builders and
identity extractors, and fail-closed production call-site expressions. The
checker also seals the complete token stream of each authoritative production
call item, so retaining the reviewed kernel snippet behind a newly inserted
bypass is not accepted. Fifteen of the 22 call items are sealed. Seven remain
explicitly unfrozen while the reply-route ownership patch changes
`v2_runtime.rs::{pop_next_with_ownership, enqueue_classified_command,
enqueue_completion_batch, commit_canonical_body_available}`,
`v2_worker.rs::poll_sidecar_flushes`, and
`v2_runner.rs::{recovered, run_inner}`. Each unfrozen item is a promotion error,
not evidence. None of the three obligations is promoted until those items are
sealed and strict TLAPS and pinned Verus results are regenerated as one
immutable, source-manifest-bound release evidence set. The current boundary is:

- effective-lock verification now covers the serialized post-install lock,
  immutable body-owner, exact retirement-accounting, and bounded class-selector
  seams through live production invocations. Repeated host invocation and
  terminating local work remain explicit runtime assumptions of the liveness
  theorem rather than facts manufactured by this refinement;
- progress-witness verification reaches seven pure reducer/WAL, timer/FIFO,
  ingress, two-stage relay retry, writer-flush, and application kernels, but
  not those collection-heavy runtime paths as one verified transition system.
  The writer-flush and lane-application kernels are linked by typed,
  process-local identities for the opaque authenticated source key, the exact
  admitted delivery route, and the clone-shared one-shot writer claim. Equal
  peer, ordinal, ticket, and payload fields under another actor, route, budget,
  or independently rebuilt completion therefore cannot satisfy the link. These
  identities are never wire, persistent, or consensus fields. Promotion remains
  blocked until the remaining production paths are sealed and one fresh pinned
  source-bound evidence set verifies the complete contract; and
- successor verification reaches six pure status, runner, startup, historical
  block-sync, and terminal Apply-boundary kernels. Crash/restart scheduling and
  the full epoch-boundary action map remain outside those safety refinements.

Earlier pinned Verus result counts predate this 17-claim source closure and
remain diagnostic only. In addition, the progress-witness target still depends
on the unproved async progress-ownership invariant, and the successor target
still depends on unproved successor starvation freedom.
Because no ledger entry is yet `cross_tool_proved`, the dormant release
workflow neither generates nor accepts a cross-tool evidence document. The
checker reports the active set with `--print-cross-tool-obligations`; empty
output means the document must be absent.
