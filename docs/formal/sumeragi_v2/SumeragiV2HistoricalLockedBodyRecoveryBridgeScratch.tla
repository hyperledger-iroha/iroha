---- MODULE SumeragiV2HistoricalLockedBodyRecoveryBridgeScratch ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Independent deductive check for folding the production historical locked-body
sentinel into the reviewed effective-lock and progress-witness cross-tool
seams.  This scratch theorem does not promote either ledger entry.
***************************************************************************)

THEOREM HistoricalLockedBodyRecoveryProductionRefinementBridgeScratch ==
  /\ EffectiveLockBodyAcquisitionProductionRefinementObligation
  /\ ProgressWitnessProductionRefinementObligation
  => ProductionHistoricalLockedBodyRecoveryRefinement
PROOF
  BY DEF EffectiveLockBodyAcquisitionProductionRefinementObligation,
         ProgressWitnessProductionRefinementObligation,
         ProductionEffectiveLockBodyAcquisitionRefinement,
         ProductionProgressWitnessTraceRefinement,
         ProductionHistoricalLockedBodyRecoveryRefinement

=============================================================================
