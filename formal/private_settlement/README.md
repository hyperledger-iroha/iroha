# Atomic private settlement formal model

`AtomicPrivateSettlementV1.tla` models the production phase barriers and the
durability edges that protect confidential cross-dataspace settlement:

- every sidecar is certified by an exact 3-of-4 committee;
- a Prepare QC follows durable staging;
- Commit starts only after the complete ordered Prepare barrier has one exact
  digest;
- global application changes either every leg or no leg;
- abort and expiry are byte-silent for private state;
- replay cannot apply a finalized bundle twice; and
- one crash/restart does not discard certified sidecars, staged deltas, QCs,
  the carrier, or the receipt.

Leg identities are symmetry-reduced to certified counts. This preserves the
barrier and atomicity properties while permitting TLC to check `LegCount = 255`
without enumerating every subset of 255 interchangeable legs. Canonical route
ordering, duplicate rejection, and exact cryptographic bindings remain Rust
implementation obligations and are tested beside the Norito types.

The primary paper configuration is `AtomicPrivateSettlementV1_3.cfg`.
`AtomicPrivateSettlementV1_255.cfg` exercises the protocol maximum, and
`AtomicPrivateSettlementV1_expiry.cfg` explores invalid/expiry terminal paths.
The model includes deliberate `PartialApply`, `CommitBeforeAllPrepare`, and
`DropStageOnCrash` mutations. Their `*_bug.cfg` configurations are negative
controls and must violate the corresponding safety invariant.

Run the model with the repository-pinned TLA+ toolchain, once installed:

```text
TLA2TOOLS_JAR=<tla2tools.jar> \
JAVA_BIN=<java> \
scripts/formal/run_atomic_private_settlement_tlc.sh
```

Passing this abstraction is necessary evidence, not a substitute for the real
four-validator crash/loss matrix or an independent protocol and cryptography
review.
