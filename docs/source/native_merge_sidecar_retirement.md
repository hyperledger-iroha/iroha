# Native beacon history and retired merge transport

The beacon-history inspector reads the canonical global block's network inputs and typed execution outputs. Merged lane inputs are the trailing execution suffix produced by `SumeragiLaneMerge` expansion. Public pulse candidates come from the bounded native `ExecutionResultCommitment` and must match the exact stored execution identity, height, input/output roots and predecessor. These are structural checks: the report does not certify finality, replay success, absence or provider custody.

The old merge-sidecar transfer/lifecycle/signing guard owner and `--merge-sidecar` input are removed. There is no adapter, alternative decoder or legacy service. Native `LaneRunner`, its physical stores and the original global executor remain the production owners. The [retired assertion inventory](native_merge_sidecar_retirement.json) records exact source digests and test names. Retired protocol tests are not passing native tests.

TODO: Complete native transport fault/replay/restart and original signing-custody qualification on the unchanged release candidate. The inspector's real native pulse, foreign-result rejection, malformed-preimage rejection and removed-option controls are added but remain unrun while Core compilation is incomplete.
