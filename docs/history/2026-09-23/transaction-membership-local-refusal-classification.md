# Carrier membership local-refusal classification

The ordinary and Native output finalizers now pass errors from
`StateBlock::stage_canonical_carrier_membership` through the existing typed
`BlockValidationError::from_certified_merge_stage_error` classifier. A local
`MembershipAdmissionError` stays `BlockValidationError::MembershipAdmission`,
which the v2 Apply owner treats as local validation refusal and the pipeline
event projection excludes from block rejection. Deterministic
`ExecutionBatchInvalid` remains an invalid execution context.

The focused `local_storage_recovery_emits_no_block_rejection` regression now
includes a membership-capacity refusal alongside existing local-storage cases
and keeps a deterministic invalid-batch control. This change repairs the error
route at both finalizer call sites; it does not claim a funded hot-tip `HashSet`,
snapshot scratch, or complete Native Validate-to-Apply ownership. The
[separate admission audit](transaction-hot-tip-snapshot-admission-gap.md)
describes those remaining allocations and the original-owner retry test.
