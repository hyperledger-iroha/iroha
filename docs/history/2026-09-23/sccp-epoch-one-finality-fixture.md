# SCCP signed epoch-one finality fixture — 2026-09-23

Status: test-only source on `optimizations`; the SCCP unit test and fresh Core
bridge suite passed. No production bridge or release gate closes here.

The earlier exact outbound fixture signs genesis scheduling epoch zero through
height 10. Its height-two successor correctly inherits epoch zero, so the Core
SCCP anchor builder returns `EpochZero`. Changing the expected error alone did
not provide a positive test for the epoch-aware production projection.

The new exact fixture signs a height-one epoch boundary with a valid retained
authority authorization for epoch one, its full next-epoch roster, aligned BLS
proofs of possession and quorum. The height-two successor uses that certified
snapshot and the exact parent `CommitQC`; the test-only signer binds proposal
and executed block wires before aggregating its ordinary commit votes. No
context field or signature is patched after verification. Existing same-epoch
fixtures continue to use the ten-height genesis schedule.

`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_sccp --lib
exact_epoch_one_successor_inherits_certified_boundary_and_parent_commit --
--nocapture` passed 1/1. A fresh Core `verified_finality_derives_epoch_aware_sora_anchor_and_rejects_boundaries`
selector passed 1/1, followed by `bridge::tests::` at 77/77 from that same
binary. The fixture does not qualify production SCCP
messages, resource limits, deployment or independently audited finality.
