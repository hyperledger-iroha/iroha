---- MODULE SumeragiV2ProgressWitnessCrossToolScratch ----
EXTENDS SumeragiV2AsyncTemporalClosureProofs

(***************************************************************************
The authoritative closed model theorem, ledger operator, and cross-tool theorem
now live in SumeragiV2AsyncTemporalClosureProofs.  This scratch module intentionally
declares no duplicate aliases; its uniquely named wrapper only rechecks that
the authoritative bridge remains visible through the release module boundary.
***************************************************************************)
THEOREM ScratchRechecksAuthoritativeProgressWitnessCrossToolRefinement ==
  ProductionProgressWitnessTraceRefinement
    => ProgressWitnessProductionRefinementObligation
PROOF
  BY ProgressWitnessCrossToolRefinement

=============================================================================
