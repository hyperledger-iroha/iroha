# PoTR checkpoint acknowledgement identity — 2026-09-23

On `optimizations`, the SoraFS node now rederives each persisted terminal
proof-outcome operation ID from the signed PoTR receipt and retained admission
envelope digest, and each repair task ID from the signed receipt digest. A
checkpoint with a substituted nonempty acknowledgement is rejected at restart;
an absent acknowledgement remains pending for replay. No wire layout or
compatibility path changed.

The focused canonical-checkpoint tamper/restart regression passed 1/1. The
fresh `sorafs_node` binary passed all `potr::tests::` cases, 15/15, including
proof-outcome and repair outage/replay tests. These are local component tests;
distributed handoff completion, finalized producer binding, crash/fork/load
qualification and the SoraFS release gate remain open.
The deterministic acknowledgement ID proves which handoff was claimed, not
that an independent outbox or repair consumer executed it. Canonical checkpoint
bytes also do not themselves authenticate the storage origin; those existing
production trust boundaries still require the full finalized and recovery
qualification.
