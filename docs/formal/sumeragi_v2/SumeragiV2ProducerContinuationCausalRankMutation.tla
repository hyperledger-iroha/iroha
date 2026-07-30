---- MODULE SumeragiV2ProducerContinuationCausalRankMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation kernel for the frozen producer-continuation causal rank.

An adapter service stage is not a causal-work rank.  A replacement can retain
the same immutable lifecycle ordinal, causal origin, and adapter stage while
installing work with a different remaining command weight.  A stage-only
prefix therefore observes equality even when the finite producer episode has
been replenished.  The repaired transition admits only a strict decrease of
the frozen causal weight; the mutation preserves the stage projection while
increasing that weight.
***************************************************************************)

CONSTANT EnforceCausalWeightDescent

ASSUME EnforceCausalWeightDescent \in BOOLEAN

FrozenOrigin == "leader-1/context-0/view-4/subject-A"
LifecycleOrdinal == 11
AdapterStage == "TimeoutCertificateReceived"
ParentIdentity == "DeliverTC/leader-1/context-0/view-4/subject-A"
ReplacementIdentity == "PersistInstallTC/leader-1/context-0/view-4/subject-A"

ParentCausalWeight == 8
StrictSuccessorWeight == 3
ReplenishedWeight == 10

VARIABLES
  phase,
  identity,
  origin,
  ordinal,
  stage,
  causalWeight

vars == <<phase, identity, origin, ordinal, stage, causalWeight>>

StageProjectionRank ==
  IF stage = AdapterStage THEN 6 ELSE 0

FrozenEpisodeRank == causalWeight

TypeInvariant ==
  /\ phase \in {"Parent", "Replaced"}
  /\ identity \in {ParentIdentity, ReplacementIdentity}
  /\ origin = FrozenOrigin
  /\ ordinal = LifecycleOrdinal
  /\ stage = AdapterStage
  /\ causalWeight \in
       {ParentCausalWeight, StrictSuccessorWeight, ReplenishedWeight}

ReplacementRetainsFrozenIdentity ==
  phase = "Replaced"
    => /\ identity = ReplacementIdentity
       /\ origin = FrozenOrigin
       /\ ordinal = LifecycleOrdinal

StageOnlyProjectionCannotDetectReplacement ==
  phase = "Replaced" => StageProjectionRank = 6

FrozenCausalEpisodeCannotReplenish ==
  phase = "Replaced"
    => FrozenEpisodeRank < ParentCausalWeight

Init ==
  /\ phase = "Parent"
  /\ identity = ParentIdentity
  /\ origin = FrozenOrigin
  /\ ordinal = LifecycleOrdinal
  /\ stage = AdapterStage
  /\ causalWeight = ParentCausalWeight

ReplaceAtSameAdapterStage ==
  /\ phase = "Parent"
  /\ phase' = "Replaced"
  /\ identity' = ReplacementIdentity
  /\ origin' = origin
  /\ ordinal' = ordinal
  /\ stage' = stage
  /\ causalWeight' =
       IF EnforceCausalWeightDescent
       THEN StrictSuccessorWeight
       ELSE ReplenishedWeight

Next == ReplaceAtSameAdapterStage

=============================================================================
