# Native output metadata ownership

The native G output seal attaches only actual typed execution outputs, their checked
Merkle cache, fragment count, transfer transcripts, AXT envelopes, policy and
transition set. `BlockResult.lane_finality_statements` and its setter parameter are
removed from the first-release wire layout. Current binary and JSON roundtrip and
retired trailing-field refusal controls belong in `iroha_data_model`.

The original output finalizer retains AXT and SCCP World effects. SCCP receives the
already-authenticated native epoch schedule, including genesis, and fails closed
for missing successor authority; staged writes cannot synthesize a genesis quorum.
The retired global V2 context adapter, ordinary lane-frontier writer and QueuePlan
obligation resolver are removed from this native owner.

PipelineGas preserves its exact signed asset/quantity bound, payer scope, sponsor
vault debit and real transfer to the technical account. Its former post-transfer
synthetic XOR quote/receipt is removed. Native execution and configuration reject
`lane_relay_burn`; no receipt can substitute for an actual asset transfer. The
retired receipt producers, pending maps, block accumulator and drain APIs are removed.
Actual fee execution and unsigned fee-plan quoting reject the retired mode before
any fee, sponsor-vault debit or allocation-counter change.

Validation remains candidate-bound. Source syntax and updated unit controls do not
establish full Core/daemon compilation, native network qualification, SDK capture
parity, or cross-dataspace atomic execution (S6). Obsolete grouped/relay tests are
inventoried in the staged review; their active execution and custody obligations
must be discharged by actual native tests before release.
