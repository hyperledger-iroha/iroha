---- MODULE SumeragiV2ProposalPipelineBudgetMutation ----
EXTENDS Integers, Naturals

(***************************************************************************
Bounded mutation for the proposal-pipeline product.

The four pipeline phases each visit every frozen semantic origin and every
origin-local non-chunk/chunk slot.  Every slot owns a complete physical
episode, including its departure step.  The repaired budget multiplies those
dimensions.  The mutation adds the slot capacity to one physical episode, so
cross-origin or cross-phase service silently recharges work after the
configured credit is exhausted.

The production operator is source-pinned separately by the aggregate checker;
this finite pair is mutation evidence, not deductive proof.
***************************************************************************)

CONSTANTS MutationMode, ValidatorCount, ChunkCount, PhysicalEpisodeBudget

VARIABLES pipelinePhase, semanticOrigin, originSlot, physicalEpisode, done

SlotCapacity == ChunkCount + 8

PerSlotEpisode == PhysicalEpisodeBudget + 1

ExactPipelineBudget ==
  4 * ValidatorCount * SlotCapacity * PerSlotEpisode

AdditivePipelineBudget ==
  4 * ValidatorCount * (PhysicalEpisodeBudget + SlotCapacity)

ConfiguredPipelineBudget ==
  IF MutationMode = "Product"
  THEN ExactPipelineBudget
  ELSE AdditivePipelineBudget

ChargedEpisodes ==
  IF done
  THEN ExactPipelineBudget
  ELSE (((pipelinePhase * ValidatorCount + semanticOrigin)
           * SlotCapacity + originSlot)
          * PerSlotEpisode + physicalEpisode)

vars ==
  <<pipelinePhase, semanticOrigin, originSlot, physicalEpisode, done>>

TypeInvariant ==
  /\ MutationMode \in {"Product", "Additive"}
  /\ ValidatorCount \in Nat \ {0}
  /\ ChunkCount \in Nat \ {0}
  /\ PhysicalEpisodeBudget \in Nat \ {0}
  /\ pipelinePhase \in 0..3
  /\ semanticOrigin \in 0..(ValidatorCount - 1)
  /\ originSlot \in 0..(SlotCapacity - 1)
  /\ physicalEpisode \in 0..(PerSlotEpisode - 1)
  /\ done \in BOOLEAN

Init ==
  /\ pipelinePhase = 0
  /\ semanticOrigin = 0
  /\ originSlot = 0
  /\ physicalEpisode = 0
  /\ ~done

AdvancePhysicalEpisode ==
  /\ ~done
  /\ physicalEpisode + 1 < PerSlotEpisode
  /\ physicalEpisode' = physicalEpisode + 1
  /\ UNCHANGED <<pipelinePhase, semanticOrigin, originSlot, done>>

AdvanceOriginSlot ==
  /\ ~done
  /\ physicalEpisode = PerSlotEpisode - 1
  /\ originSlot + 1 < SlotCapacity
  /\ originSlot' = originSlot + 1
  /\ physicalEpisode' = 0
  /\ UNCHANGED <<pipelinePhase, semanticOrigin, done>>

AdvanceSemanticOrigin ==
  /\ ~done
  /\ physicalEpisode = PerSlotEpisode - 1
  /\ originSlot = SlotCapacity - 1
  /\ semanticOrigin + 1 < ValidatorCount
  /\ semanticOrigin' = semanticOrigin + 1
  /\ originSlot' = 0
  /\ physicalEpisode' = 0
  /\ UNCHANGED <<pipelinePhase, done>>

AdvancePipelinePhase ==
  /\ ~done
  /\ physicalEpisode = PerSlotEpisode - 1
  /\ originSlot = SlotCapacity - 1
  /\ semanticOrigin = ValidatorCount - 1
  /\ pipelinePhase < 3
  /\ pipelinePhase' = pipelinePhase + 1
  /\ semanticOrigin' = 0
  /\ originSlot' = 0
  /\ physicalEpisode' = 0
  /\ UNCHANGED done

FinishPipeline ==
  /\ ~done
  /\ physicalEpisode = PerSlotEpisode - 1
  /\ originSlot = SlotCapacity - 1
  /\ semanticOrigin = ValidatorCount - 1
  /\ pipelinePhase = 3
  /\ done'
  /\ UNCHANGED <<pipelinePhase, semanticOrigin, originSlot, physicalEpisode>>

RemainComplete ==
  /\ done
  /\ UNCHANGED vars

Next ==
  AdvancePhysicalEpisode
    \/ AdvanceOriginSlot
    \/ AdvanceSemanticOrigin
    \/ AdvancePipelinePhase
    \/ FinishPipeline
    \/ RemainComplete

Spec ==
  Init /\ [][Next]_vars

PipelineBudgetCoversEveryCrossSlotEpisode ==
  ChargedEpisodes <= ConfiguredPipelineBudget

=============================================================================
