# Original execution through retained publication phases

Work location: `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The Sumeragi liveness redesign remains active. This record covers the retained
validation service's publication handoff, not activation of the live runner.

## Missing continuation

`SelectedValidationCarrier::try_consume` restores its original owner type on
local refusal. That owner previously accepted only `PreparedCarrierJournals`.
Decision binding consumes its actual `ValidBlock` into a `CommittedBlock` and
returns `DecisionBoundCarrierJournals`; checkpoint attachment changes that owner
again. A publication refusal therefore could not return the current decided or
checkpointed execution to the existing candidate slot. Reconstructing the old
validation type or executing again would discard the very ownership this
handoff must preserve.

Production still calls the scalar `V2ApplyService::validate_candidate` and later
executes in `validate_and_apply`. The retained service and consuming publisher
must replace that complete path together, after their remaining resource and
retirement obligations are implemented.

## Implemented ownership

`RetainedCarrier<Admission, BindingAdmission>` owns exactly one validated,
decided or checkpointed carrier. Each variant contains its original journals,
sources, effects and capacity owners by move. Candidate identity and execution
commitment are read from those journals in every phase. No copied identity,
second candidate table, reverse block conversion or extra phase-transition
coordinator is introduced.

The existing retained-validation service reserves candidate and marker vectors
before invoking the validator. Its candidate slot holds the complete phase;
selection refusal restores the phase returned by the consuming operation.
The descriptor covers the largest inline variant before execution, so advancing
phase needs no additional descriptor allocation. Physical writers remain
short-lived: refusal and abort return the detached checkpointed owner before
retaining it in the service. The sealed production owner trait accepts this
complete phase owner; the scalar test stand-in remains test-only.

## Validation scope

Build104 compiled the seven-package test target successfully in 378.9 seconds
with unchanged inputs. Its 140 publication, retained-validation, geometry,
lifecycle and Native owner tests and all 32 original review regressions passed
on one unchanged binary, SHA-256
`d91b45e65c4b0e6533221c4991deee65cd8c5ac767891ad66f6a8aac5bad7ed8`.
The new integrated regression uses an actual authority-signed genesis, an exact four-validator height context,
mandatory RS16, a three-of-four BLS decision, real BodyStore files and real Kura
checkpoint persistence. It exercises initial marker file-sync refusal, later
reproposal directory-sync refusal while already decided, original World writer
contention, physical abort and eventual publication. It compares six original
journal/source allocations across refusals and requires exactly one validator
execution. After publication, the bounded consumed-subject entry refuses a
delayed cached occurrence without executing again.

All 37 focused formal controls passed on 9,344 unchanged captured inputs. They
check the three inline phases, delegation to original journals, reservation
before execution, restoration of the progressed owner, consumed-subject
retention and marker installation before sync/confirmation. Fourteen exact
source-binding ledger rows were updated with their checker expectations.
Rust formatting, the codec guard, diff whitespace and the historical archive
checks passed. Build and runtime receipts are under
`dist/sumeragi-main-work/{build104-receipt.json,publication104,review-current104}`;
focused formal receipts are under `dist/sumeragi-main-work/retained-phase104/controls`.
The final canonical gate is captured separately under
`dist/sumeragi-main-work/retained-phase104/canonical`; its full source manifest
must match every compiled input before these checks can qualify one candidate.

The test's counted admission objects verify lifetime and release order. They
are not a production resource policy. Full pre-execution/capture/publication
admission, original Apply-service State/Queue binding, Queue retirement and
participant durability remain prerequisites for the live switch. Cold unfinished
recovery must retain its one executed owner before reinstalling markers;
already-applied recovery must repair durable completion without execution.
Production Native cutover, full workspace tests and unchanged four/seven-validator
fault/restart/final-transaction campaigns remain open.

The next original-target boundary also has a concrete private-path gap:
`try_prepare_physical` authenticates Kura before derived witness/archive writes,
but a different State sharing that Kura is rejected only at component
acquisition. Move the existing geometry State/header identity predicate before
the first Kura probe, while retaining the later component and terminal geometry
checks. Prove wrong-target rejection leaves witness/archive storage unchanged
and returns the same owner for an original-State retry. This is a private
integration prerequisite; no live production exposure is claimed.
