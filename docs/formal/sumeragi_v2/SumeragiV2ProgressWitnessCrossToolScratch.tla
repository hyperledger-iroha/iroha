---- MODULE SumeragiV2ProgressWitnessCrossToolScratch ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
The authoritative model projection, ledger operator, and cross-tool theorem
now live in SumeragiV2AsyncLivenessProofs.  This scratch module intentionally
declares no duplicate aliases; its uniquely named wrapper only rechecks that
the authoritative bridge remains visible through the release module boundary.
***************************************************************************)
THEOREM ScratchRechecksAuthoritativeProgressWitnessCrossToolRefinement ==
  ProductionProgressWitnessTraceRefinement
    => ProgressWitnessProductionRefinementObligation
PROOF
  BY ProgressWitnessCrossToolRefinement

=============================================================================
