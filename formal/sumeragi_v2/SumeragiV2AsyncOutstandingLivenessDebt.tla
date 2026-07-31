---- MODULE SumeragiV2AsyncOutstandingLivenessDebt ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

(***************************************************************************
Compatibility boundary for the former liveness-debt shard.

The exact source and outcome operators remain declared in
`SumeragiV2LivenessProofs`, below this ordered shard chain.  Keeping vocabulary
below this ordered shard boundary lets the direct retained-lock proof leaf
import the same predicates without flowing a proofless theorem back into its
own premises.

No theorem is declared here.  Timeout/view progress, rotating-leader
convergence, and locked-body reproposal are discharged by the independent
release cones in SumeragiV2AsyncTemporalClosureProofs.  Keeping this module
as an import-compatible boundary avoids silently redirecting an older shard
name to one of those theorems while downstream source and proof-range tooling
move to the proved release modules.
***************************************************************************)
=============================================================================
