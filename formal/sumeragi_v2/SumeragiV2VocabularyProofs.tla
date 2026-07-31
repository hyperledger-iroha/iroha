---- MODULE SumeragiV2VocabularyProofs ----
EXTENDS SumeragiV2Reconfiguration

(***************************************************************************
Small deductive facts about the executable vocabulary.

Keeping theorem declarations outside the vocabulary import graph is
semantically inert, but important for indexed refinement: the pinned TLAPM
elaborator cannot anonymize theorem-bearing modules under an operator-
parameterized INSTANCE.  These statements and proofs are the exact facts
formerly declared inline by Availability, CrashRecovery, and Reconfiguration.
The release proof runner checks this module directly.
***************************************************************************)

THEOREM PrepareSignerAvailabilityIncludesDurability ==
  \A bodyStore, validationStore, bodyContext, view, generations,
     subject, signer:
    PrepareSignerAvailability(bodyStore, validationStore, bodyContext,
                              view, generations, subject, signer)
      => BodyHeldBy(bodyStore, signer, bodyContext, view, subject)
BY DEF PrepareSignerAvailability

THEOREM CrashDoesNotErasePrepareIntents ==
  \A node \in ValidatorIds:
    Crash(node) => prepareIntents' = prepareIntents
BY DEF Crash

THEOREM CrashDoesNotEraseDecisions ==
  \A node \in ValidatorIds:
    Crash(node) => decisions' = decisions
BY DEF Crash

THEOREM IncompleteFrameIsNotAcknowledged ==
  \A frames:
    IncompleteFinalFrameUnacknowledged(frames)
BY DEF IncompleteFinalFrameUnacknowledged, AcknowledgedFrames

THEOREM ContextRecordCarriesFrozenEpoch ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      ContextRecord(blockHeight, lineage).epoch = ExpectedEpoch(blockHeight)
BY DEF ContextRecord

THEOREM ContextRecordCarriesParent ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      /\ (blockHeight = 0
            => ContextRecord(blockHeight, lineage).parent = NoSubject)
      /\ (blockHeight > 0
            => ContextRecord(blockHeight, lineage).parent = lineage[blockHeight])
BY DEF ContextRecord

THEOREM ContextRecordCarriesParentContext ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      ContextRecord(blockHeight, lineage).parentContextKey
        = ParentContextKey(blockHeight, lineage)
BY DEF ContextRecord

THEOREM EquivalentParentCommitQcsConverge ==
  \A parentContextKey,
     parentHeight,
     parentSubject,
     leftView,
     rightView,
     leftSigners,
     rightSigners:
    SemanticParentFinality(
      CarriedParentCommit(parentContextKey, parentHeight, parentSubject,
                          leftView, leftSigners))
      = SemanticParentFinality(
          CarriedParentCommit(parentContextKey, parentHeight, parentSubject,
                              rightView, rightSigners))
BY DEF SemanticParentFinality, CarriedParentCommit

THEOREM ForeignParentLineageHasDifferentIdentity ==
  \A leftContextKey,
     rightContextKey,
     parentHeight,
     parentSubject,
     leftView,
     rightView,
     leftSigners,
     rightSigners:
    leftContextKey # rightContextKey
      => SemanticParentFinality(
           CarriedParentCommit(leftContextKey, parentHeight, parentSubject,
                               leftView, leftSigners))
           # SemanticParentFinality(
               CarriedParentCommit(rightContextKey, parentHeight,
                                   parentSubject, rightView, rightSigners))
BY DEF SemanticParentFinality, CarriedParentCommit

=============================================================================
