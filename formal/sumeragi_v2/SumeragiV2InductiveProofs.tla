---- MODULE SumeragiV2InductiveProofs ----
EXTENDS SumeragiV2Inductive, SumeragiV2SafetyLemmas,
        SumeragiV2AgreementLemmas, NaturalsInduction, FiniteSetTheorems,
        SequenceTheorems

(***************************************************************************
Action-by-action proof that the executable reducer establishes and preserves
its asynchronous provenance.  This module is intentionally separate from the
TLC-loadable invariant vocabulary.
***************************************************************************)

THEOREM NaturalOrderReflexive ==
  \A value \in Nat: value <= value
BY SMT

THEOREM NaturalBoundBelowSuccessor ==
  \A lower, upper \in Nat: lower <= upper => lower <= upper + 1
BY SMT

THEOREM NaturalStrictUpperIsPositive ==
  \A lower, upper \in Nat: lower < upper => upper > 0
BY SMT

THEOREM NaturalOrderTotal ==
  \A left, right \in Nat: left <= right \/ right < left
BY SMT

THEOREM BoundedNaturalPredecessor ==
  \A upper \in Nat:
    \A value \in 0..upper:
      value > 0 => value - 1 \in 0..upper
BY SMT

THEOREM NaturalQuotientTyped ==
  \A value, divisor \in Nat:
    divisor > 0 => value \div divisor \in Nat
BY SMT

THEOREM NaturalDivisionAlgorithm ==
  \A value, divisor \in Nat:
    divisor > 0
      => /\ value % divisor \in 0..(divisor - 1)
         /\ value
              = divisor * (value \div divisor) + (value % divisor)
BY SMT

THEOREM NaturalStrictMultiplierGap ==
  \A factor, lower, upper \in Nat:
    factor > 0 /\ lower < upper
      => factor * lower + factor <= factor * upper
PROOF
  <1>1. ASSUME NEW lower \in Nat,
              NEW upper \in Nat,
              lower < upper
         PROVE \A factor \in Nat:
                 factor * lower + factor <= factor * upper
    <2>1. 0 * lower + 0 <= 0 * upper
      BY <1>1, SMT
    <2>2. ASSUME NEW factor \in Nat,
                  factor * lower + factor <= factor * upper
           PROVE (factor + 1) * lower + (factor + 1)
                   <= (factor + 1) * upper
      <3>1. /\ (factor + 1) * lower = factor * lower + lower
            /\ (factor + 1) * upper = factor * upper + upper
        BY <1>1, <2>2, SMT
      <3>2. lower + 1 <= upper
        BY <1>1, SMT
      <3> QED BY <1>1, <2>2, <3>1, <3>2, SMT
    <2> QED BY <2>1, <2>2, NatInduction
  <1> QED BY <1>1

THEOREM NaturalDivisionMonotone ==
  \A lower, upper, divisor \in Nat:
    divisor > 0 /\ lower <= upper
      => lower \div divisor <= upper \div divisor
PROOF
  <1>1. ASSUME NEW lower \in Nat,
              NEW upper \in Nat,
              NEW divisor \in Nat,
              divisor > 0,
              lower <= upper
         PROVE lower \div divisor <= upper \div divisor
    <2>1. /\ lower \div divisor \in Nat
          /\ upper \div divisor \in Nat
      BY <1>1, NaturalQuotientTyped
    <2>2. /\ lower % divisor \in 0..(divisor - 1)
          /\ lower
               = divisor * (lower \div divisor) + (lower % divisor)
      BY <1>1, NaturalDivisionAlgorithm
    <2>3. /\ upper % divisor \in 0..(divisor - 1)
          /\ upper
               = divisor * (upper \div divisor) + (upper % divisor)
      BY <1>1, NaturalDivisionAlgorithm
    <2>4. ASSUME ~(lower \div divisor <= upper \div divisor)
           PROVE FALSE
      <3>1. upper \div divisor < lower \div divisor
        BY <2>1, <2>4, NaturalOrderTotal
      <3>2. divisor * (upper \div divisor) + divisor
               <= divisor * (lower \div divisor)
        BY <1>1, <2>1, <3>1, NaturalStrictMultiplierGap
      <3>3. upper
               < divisor * (upper \div divisor) + divisor
        BY <1>1, <2>3, SMT
      <3>4. divisor * (lower \div divisor) <= lower
        BY <2>2, SMT
      <3> QED BY <1>1, <3>2, <3>3, <3>4, SMT
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM BoundedNaturalQuotient ==
  \A lower, upper, divisor, ceiling \in Nat:
    divisor > 0
      /\ lower <= upper
      /\ ceiling >= upper \div divisor
      => lower \div divisor \in 0..ceiling
PROOF
  <1>1. ASSUME NEW lower \in Nat,
              NEW upper \in Nat,
              NEW divisor \in Nat,
              NEW ceiling \in Nat,
              divisor > 0,
              lower <= upper,
              ceiling >= upper \div divisor
         PROVE lower \div divisor \in 0..ceiling
    <2>1. lower \div divisor \in Nat
      BY <1>1, NaturalQuotientTyped
    <2>2. lower \div divisor <= upper \div divisor
      BY <1>1, NaturalDivisionMonotone
    <2> QED BY <1>1, <2>1, <2>2, SMT
  <1> QED BY <1>1

THEOREM IntegerOrderChain ==
  \A lower, middle, upper \in Int:
    lower >= 0 /\ middle >= lower /\ upper > middle
      => /\ middle >= 0
         /\ upper >= 1
         /\ lower < upper
BY SMT

THEOREM IntegerWeakStrongOrderChain ==
  \A lower, middle, upper \in Int:
    lower <= middle /\ middle < upper => lower < upper
BY SMT

THEOREM IntegerStrictImpliesWeak ==
  \A lower, upper \in Int: lower < upper => lower <= upper
BY SMT

THEOREM IntegerWeakOrderTransitive ==
  \A lower, middle, upper \in Int:
    lower >= middle /\ middle >= upper => lower >= upper
BY SMT

THEOREM IntegerWeakBoundsCollapse ==
  \A lower, middle, upper \in Int:
    lower >= middle /\ middle >= upper /\ lower = upper
      => /\ middle = upper
         /\ lower = middle
BY SMT

THEOREM ModelRanksAreIntegers ==
  ModelConfiguration => Ranks \subseteq Int
BY SMT DEF ModelConfiguration, Ranks, Views, NoRank

THEOREM ViewWeakOrderTransitive ==
  ModelConfiguration
    => \A lower, middle, upper \in Views:
         lower <= middle /\ middle <= upper => lower <= upper
BY SMT DEF ModelConfiguration, Views

THEOREM ViewIsNotNoRank ==
  ModelConfiguration => \A roundView \in Views: roundView # NoRank
BY SMT DEF ModelConfiguration, Views, NoRank

THEOREM ViewsAreRanks == Views \subseteq Ranks
BY SMT DEF Views, Ranks, NoRank

THEOREM SubjectsAreSubjectOrNone == Subjects \subseteq SubjectOrNone
BY DEF SubjectOrNone

THEOREM FunctionValueHasCodomain ==
  \A domain, codomain, mapping, key:
    mapping \in [domain -> codomain]
      /\ key \in domain
      => mapping[key] \in codomain
BY Isa

THEOREM FunctionalUpdatePreservesType ==
  \A domain, codomain, mapping, key, value:
    mapping \in [domain -> codomain]
      /\ key \in domain
      /\ value \in codomain
      => [mapping EXCEPT ![key] = value] \in [domain -> codomain]
BY Isa

THEOREM IntervalFunctionIsSequence ==
  \A length \in Nat:
    \A elements:
      \A sequence \in [1..length -> elements]:
        sequence \in Seq(elements)
BY Isa, SeqDef

THEOREM FrozenContextRecordShape ==
  \A initialContext:
    FrozenContextAdmissible(initialContext)
      => \E blockHeight \in Heights:
           \E lineage \in LineagesAt(blockHeight):
             initialContext = ContextRecord(blockHeight, lineage)
BY Isa DEF FrozenContextAdmissible, ContextRecords

THEOREM ContextRecordFieldsTyped ==
  \A contextValue \in ContextRecords:
    /\ contextValue.height \in Heights
    /\ contextValue.lineage \in LineagesAt(contextValue.height)
PROOF
  <1>1. ASSUME NEW contextValue \in ContextRecords
         PROVE /\ contextValue.height \in Heights
               /\ contextValue.lineage
                    \in LineagesAt(contextValue.height)
    <2>1. PICK blockHeight \in Heights:
             \E lineage \in LineagesAt(blockHeight):
               contextValue = ContextRecord(blockHeight, lineage)
      BY <1>1, Isa DEF ContextRecords
    <2>2. PICK lineage \in LineagesAt(blockHeight):
             contextValue = ContextRecord(blockHeight, lineage)
      BY <2>1
    <2>3. /\ contextValue.height = blockHeight
          /\ contextValue.lineage = lineage
      BY <2>2 DEF ContextRecord
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM BootstrapParentPrefixTyped ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      blockHeight > 0
        => [index \in 1..(blockHeight - 1) |-> lineage[index]]
             \in LineagesAt(blockHeight - 1)
PROOF
  <1>1. ASSUME NEW blockHeight \in Heights,
              NEW lineage \in LineagesAt(blockHeight),
              blockHeight > 0
         PROVE [index \in 1..(blockHeight - 1) |-> lineage[index]]
                 \in LineagesAt(blockHeight - 1)
    <2>1. /\ blockHeight \in Nat
          /\ blockHeight - 1 \in Nat
          /\ 1..(blockHeight - 1) \subseteq 1..blockHeight
      BY <1>1, SMT DEF Heights
    <2>2. \A index \in 1..(blockHeight - 1):
             lineage[index] \in Subjects
      BY <1>1, <2>1, FunctionValueHasCodomain
         DEF LineagesAt
    <2> QED BY <2>2, Isa DEF LineagesAt
  <1> QED BY <1>1

THEOREM FrozenContextFieldsTyped ==
  \A initialContext:
    FrozenContextAdmissible(initialContext)
      => /\ initialContext.height \in Heights
         /\ initialContext.lineage \in LineagesAt(initialContext.height)
         /\ (initialContext.height = 0
               => initialContext.parent = NoSubject)
         /\ (initialContext.height > 0
               => initialContext.parent \in ValidSubjects)
PROOF
  <1>1. ASSUME NEW initialContext,
              FrozenContextAdmissible(initialContext)
         PROVE /\ initialContext.height \in Heights
               /\ initialContext.lineage
                    \in LineagesAt(initialContext.height)
               /\ (initialContext.height = 0
                     => initialContext.parent = NoSubject)
               /\ (initialContext.height > 0
                     => initialContext.parent \in ValidSubjects)
    <2>1. PICK blockHeight \in Heights:
             \E lineage \in LineagesAt(blockHeight):
               initialContext = ContextRecord(blockHeight, lineage)
      BY <1>1, FrozenContextRecordShape
    <2>2. PICK lineage \in LineagesAt(blockHeight):
             initialContext = ContextRecord(blockHeight, lineage)
      BY <2>1
    <2>3. /\ initialContext.height = blockHeight
          /\ initialContext.lineage = lineage
          /\ (blockHeight = 0
                => initialContext.parent = NoSubject)
          /\ (blockHeight > 0
                => initialContext.parent = lineage[blockHeight])
      BY <2>2 DEF ContextRecord
    <2>4. blockHeight > 0 => blockHeight \in DOMAIN lineage
      BY <2>1, <2>2, SMT DEF Heights, LineagesAt
    <2>5. blockHeight > 0
            => initialContext.parent \in ValidSubjects
      BY <1>1, <2>3, <2>4
         DEF FrozenContextAdmissible
    <2> QED BY <2>1, <2>2, <2>3, <2>5
  <1> QED BY <1>1

THEOREM BootstrapParentContextTyped ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ BootstrapParentContext(initialContext) \in ContextRecords
         /\ BootstrapParentContext(initialContext).height \in Heights
         /\ BootstrapParentContext(initialContext).height
              = initialContext.height - 1
         /\ BootstrapParentContext(initialContext).epoch \in Epochs
         /\ initialContext.parent \in ValidSubjects
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ BootstrapParentContext(initialContext)
                    \in ContextRecords
               /\ BootstrapParentContext(initialContext).height \in Heights
               /\ BootstrapParentContext(initialContext).height
                    = initialContext.height - 1
               /\ BootstrapParentContext(initialContext).epoch \in Epochs
               /\ initialContext.parent \in ValidSubjects
    <2>1. PICK blockHeight \in Heights:
             \E lineage \in LineagesAt(blockHeight):
               initialContext = ContextRecord(blockHeight, lineage)
      BY <1>1, FrozenContextRecordShape
    <2>2. PICK lineage \in LineagesAt(blockHeight):
             initialContext = ContextRecord(blockHeight, lineage)
      BY <2>1
    <2>3. initialContext.height = blockHeight
      BY <2>2 DEF ContextRecord
    <2>4. /\ blockHeight > 0
          /\ MaxHeight \in Nat
      BY <1>1, <2>1, <2>3, SMT
         DEF ModelConfiguration, Heights
    <2>5. blockHeight - 1 \in Heights
      BY <2>1, <2>4, BoundedNaturalPredecessor DEF Heights
    <2>6. [index \in 1..(blockHeight - 1) |-> lineage[index]]
             \in LineagesAt(blockHeight - 1)
      BY <2>1, <2>2, <2>4, BootstrapParentPrefixTyped
    <2>7. /\ BootstrapParentContext(initialContext)
                 = ContextRecord(
                     blockHeight - 1,
                     [index \in 1..(blockHeight - 1) |-> lineage[index]])
          /\ BootstrapParentContext(initialContext) \in ContextRecords
          /\ BootstrapParentContext(initialContext).height
               = blockHeight - 1
      BY <2>2, <2>3, <2>5, <2>6
         DEF BootstrapParentContext, ContextRecords, ContextRecord
    <2>8. BootstrapParentContext(initialContext).epoch
             = ExpectedEpoch(blockHeight - 1)
      BY <2>7 DEF ContextRecord
    <2>9. /\ blockHeight - 1 \in Nat
          /\ MaxHeight \in Nat
          /\ EpochLength \in Nat
          /\ EpochLength > 0
          /\ MaxEpoch \in Nat
          /\ blockHeight - 1 <= MaxHeight
      BY <1>1, <2>5, SMT
         DEF ModelConfiguration, QuorumConfiguration, Heights
    <2>10. MaxEpoch >= MaxHeight \div EpochLength
      BY <1>1 DEF ModelConfiguration, ExpectedEpoch
    <2>11. ExpectedEpoch(blockHeight - 1) \in 0..MaxEpoch
      BY <2>9, <2>10, BoundedNaturalQuotient DEF ExpectedEpoch
    <2>12. BootstrapParentContext(initialContext).epoch \in Epochs
      BY <2>8, <2>11 DEF Epochs
    <2>13. initialContext.parent \in ValidSubjects
      BY <1>1, FrozenContextFieldsTyped
    <2> QED BY <2>3, <2>5, <2>7, <2>12, <2>13
  <1> QED BY <1>1

THEOREM BootstrapParentQuorumTyped ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ BootstrapParentSigners(initialContext)
                 \subseteq ValidatorIds
         /\ ExactCertificateQuorum(
              BootstrapParentContext(initialContext).epoch,
              BootstrapParentSigners(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ BootstrapParentSigners(initialContext)
                    \subseteq ValidatorIds
               /\ ExactCertificateQuorum(
                    BootstrapParentContext(initialContext).epoch,
                    BootstrapParentSigners(initialContext))
    <2>1. BootstrapParentContext(initialContext).epoch \in Epochs
      BY <1>1, BootstrapParentContextTyped
    <2>2. VotingRoster(BootstrapParentContext(initialContext).epoch)
             \subseteq ValidatorIds
      BY <1>1, <2>1, Isa
         DEF ModelConfiguration, QuorumConfiguration, VotingRoster
    <2>3. ExactCertificateQuorum(
             BootstrapParentContext(initialContext).epoch,
             CanonicalCertificateSigners(
               BootstrapParentContext(initialContext).epoch,
               Responsive
                 \cap VotingRoster(
                        BootstrapParentContext(initialContext).epoch)))
      BY <1>1, <2>1 DEF ModelConfiguration
    <2> QED BY <2>2, <2>3 DEF BootstrapParentSigners
  <1> QED BY <1>1

THEOREM ContextRecordHeightTyped ==
  \A contextValue \in ContextRecords: contextValue.height \in Heights
BY Isa DEF ContextRecords, ContextRecord

THEOREM VoteConstructorTyped ==
  \A contextValue \in ContextRecords,
     roundView \in Views,
     phase \in Phases,
     subject \in Subjects,
     signer \in ValidatorIds:
    Vote(contextValue, roundView, phase, subject, signer)
      \in VoteRecordSet
PROOF
  <1>1. ASSUME NEW contextValue \in ContextRecords,
              NEW roundView \in Views,
              NEW phase \in Phases,
              NEW subject \in Subjects,
              NEW signer \in ValidatorIds
         PROVE Vote(contextValue, roundView, phase, subject, signer)
                 \in VoteRecordSet
    <2>1. contextValue.height \in Heights
      BY <1>1, ContextRecordHeightTyped
    <2> QED BY <1>1, <2>1, Isa DEF Vote, VoteRecordSet
  <1> QED BY <1>1

THEOREM QcConstructorTyped ==
  \A contextValue \in ContextRecords,
     roundView \in Views,
     phase \in Phases,
     subject \in Subjects,
     signers \in SUBSET ValidatorIds:
    QC(contextValue, roundView, phase, subject, signers)
      \in QcRecordSet
PROOF
  <1>1. ASSUME NEW contextValue \in ContextRecords,
              NEW roundView \in Views,
              NEW phase \in Phases,
              NEW subject \in Subjects,
              NEW signers \in SUBSET ValidatorIds
         PROVE QC(contextValue, roundView, phase, subject, signers)
                 \in QcRecordSet
    <2>1. contextValue.height \in Heights
      BY <1>1, ContextRecordHeightTyped
    <2> QED BY <1>1, <2>1, Isa DEF QC, QcRecordSet
  <1> QED BY <1>1

THEOREM BodyRecordConstructorTyped ==
  \A node \in ValidatorIds,
     contextValue \in ContextRecords,
     roundView \in Views,
     subject \in Subjects:
    BodyRecord(node, contextValue, roundView, subject) \in BodyRecordSet
BY Isa DEF BodyRecord, BodyRecordSet

THEOREM RetainedLockedBodyRecordConstructorTyped ==
  \A node \in ValidatorIds,
     contextValue \in ContextRecords,
     subject \in Subjects:
    RetainedLockedBodyRecord(node, contextValue, subject)
      \in RetainedLockedBodyRecordSet
BY Isa DEF RetainedLockedBodyRecord, RetainedLockedBodyRecordSet

THEOREM ValidationRecordConstructorTyped ==
  \A node \in ValidatorIds,
     contextValue \in ContextRecords,
     roundView \in Views,
     requestGeneration \in Generations,
     subject \in Subjects:
    ValidationRecord(node, contextValue, roundView, requestGeneration,
                     subject)
      \in ValidationRecordSet
BY Isa DEF ValidationRecord, ValidationRecordSet

THEOREM BootstrapParentEvidenceTyped ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ BootstrapParentPrepareIntents(initialContext)
                 \subseteq VoteRecordSet
         /\ BootstrapParentCommitIntents(initialContext)
                 \subseteq VoteRecordSet
         /\ BootstrapParentBodies(initialContext) \subseteq BodyRecordSet
         /\ BootstrapParentPrepareQC(initialContext) \in QcRecordSet
         /\ BootstrapParentCommitQC(initialContext) \in QcRecordSet
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ BootstrapParentPrepareIntents(initialContext)
                    \subseteq VoteRecordSet
               /\ BootstrapParentCommitIntents(initialContext)
                    \subseteq VoteRecordSet
               /\ BootstrapParentBodies(initialContext)
                    \subseteq BodyRecordSet
               /\ BootstrapParentPrepareQC(initialContext) \in QcRecordSet
               /\ BootstrapParentCommitQC(initialContext) \in QcRecordSet
    <2>1. /\ BootstrapParentContext(initialContext) \in ContextRecords
          /\ BootstrapParentContext(initialContext).height \in Heights
          /\ initialContext.parent \in ValidSubjects
      BY <1>1, BootstrapParentContextTyped
    <2>2. BootstrapParentSigners(initialContext)
             \subseteq ValidatorIds
      BY <1>1, BootstrapParentQuorumTyped
    <2>3. /\ 0 \in Views
          /\ {"Prepare", "Commit"} \subseteq Phases
          /\ ValidSubjects \subseteq Subjects
      BY <1>1, SMT DEF ModelConfiguration, Views, Phases
    <2>4. \A signer \in BootstrapParentSigners(initialContext):
             /\ Vote(BootstrapParentContext(initialContext), 0,
                     "Prepare", initialContext.parent, signer)
                    \in VoteRecordSet
             /\ Vote(BootstrapParentContext(initialContext), 0,
                     "Commit", initialContext.parent, signer)
                    \in VoteRecordSet
      BY <2>1, <2>2, <2>3, VoteConstructorTyped
    <2>5. /\ BootstrapParentPrepareIntents(initialContext)
                    \subseteq VoteRecordSet
          /\ BootstrapParentCommitIntents(initialContext)
                    \subseteq VoteRecordSet
      BY <2>4, Isa
         DEF BootstrapParentPrepareIntents,
             BootstrapParentCommitIntents
    <2>6. /\ BootstrapParentPrepareQC(initialContext) \in QcRecordSet
          /\ BootstrapParentCommitQC(initialContext) \in QcRecordSet
      BY <2>1, <2>2, <2>3, QcConstructorTyped
         DEF BootstrapParentPrepareQC, BootstrapParentCommitQC,
             QC
    <2>7. BootstrapParentBodies(initialContext) \subseteq BodyRecordSet
      <3>1. \A signer \in BootstrapParentSigners(initialContext):
               BodyRecord(signer,
                          BootstrapParentContext(initialContext), 0,
                          initialContext.parent)
                 \in BodyRecordSet
        BY <2>1, <2>2, <2>3, BodyRecordConstructorTyped
      <3> QED BY <3>1 DEF BootstrapParentBodies
    <2> QED BY <2>5, <2>6, <2>7
  <1> QED BY <1>1

THEOREM BootstrapParentIntentSubjectsUniform ==
  \A initialContext:
    /\ \A left, right \in BootstrapParentPrepareIntents(initialContext):
         right.subject = left.subject
    /\ \A left, right \in BootstrapParentCommitIntents(initialContext):
         right.subject = left.subject
BY Isa
   DEF BootstrapParentPrepareIntents,
       BootstrapParentCommitIntents, Vote

THEOREM BootstrapParentIntentPhases ==
  \A initialContext:
    /\ \A vote \in BootstrapParentPrepareIntents(initialContext):
         vote.phase = "Prepare"
    /\ \A vote \in BootstrapParentCommitIntents(initialContext):
         vote.phase = "Commit"
BY Isa
   DEF BootstrapParentPrepareIntents,
       BootstrapParentCommitIntents, Vote

THEOREM BootstrapParentIntentContextHeights ==
  \A initialContext:
    /\ \A vote \in BootstrapParentPrepareIntents(initialContext):
         vote.context.height
           = BootstrapParentContext(initialContext).height
    /\ \A vote \in BootstrapParentCommitIntents(initialContext):
         vote.context.height
           = BootstrapParentContext(initialContext).height
BY Isa
   DEF BootstrapParentPrepareIntents,
       BootstrapParentCommitIntents, Vote

THEOREM BootstrapParentIntentsStructural ==
  \A initialContext:
    /\ HonestVoteUnique(BootstrapParentPrepareIntents(initialContext))
    /\ HonestVoteUnique(BootstrapParentCommitIntents(initialContext))
    /\ \A vote \in BootstrapParentPrepareIntents(initialContext):
         vote.phase = "Prepare"
    /\ \A vote \in BootstrapParentCommitIntents(initialContext):
         vote.phase = "Commit"
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE /\ HonestVoteUnique(
                       BootstrapParentPrepareIntents(initialContext))
               /\ HonestVoteUnique(
                       BootstrapParentCommitIntents(initialContext))
               /\ \A vote
                        \in BootstrapParentPrepareIntents(initialContext):
                    vote.phase = "Prepare"
               /\ \A vote
                        \in BootstrapParentCommitIntents(initialContext):
                    vote.phase = "Commit"
    <2>1. /\ \A left, right
                       \in BootstrapParentPrepareIntents(initialContext):
                    right.subject = left.subject
          /\ \A left, right
                       \in BootstrapParentCommitIntents(initialContext):
                    right.subject = left.subject
      BY BootstrapParentIntentSubjectsUniform
    <2>2. /\ \A vote
                       \in BootstrapParentPrepareIntents(initialContext):
                    vote.phase = "Prepare"
          /\ \A vote
                       \in BootstrapParentCommitIntents(initialContext):
                    vote.phase = "Commit"
      BY BootstrapParentIntentPhases
    <2>3. HonestVoteUnique(
             BootstrapParentPrepareIntents(initialContext))
      BY <2>1
         DEF HonestVoteUnique, SameVoteSlot
    <2>4. HonestVoteUnique(
             BootstrapParentCommitIntents(initialContext))
      BY <2>1
         DEF HonestVoteUnique, SameVoteSlot
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalQcConstructorValid ==
  \A contextValue, roundView, phase, subject, signers:
    (/\ contextValue \in ContextRecords
     /\ contextValue.height \in Heights
     /\ contextValue.epoch \in Epochs
     /\ roundView \in Views
     /\ phase \in Phases
     /\ subject \in ValidSubjects
     /\ ExactCertificateQuorum(contextValue.epoch, signers))
      => HistoricalQcValid(
           QC(contextValue, roundView, phase, subject, signers))
PROOF
  <1>1. ASSUME NEW contextValue,
              NEW roundView,
              NEW phase,
              NEW subject,
              NEW signers,
              contextValue \in ContextRecords,
              contextValue.height \in Heights,
              contextValue.epoch \in Epochs,
              roundView \in Views,
              phase \in Phases,
              subject \in ValidSubjects,
              ExactCertificateQuorum(contextValue.epoch, signers)
         PROVE HistoricalQcValid(
                 QC(contextValue, roundView, phase, subject, signers))
    <2>1. /\ QC(contextValue, roundView, phase, subject, signers).context
                  = contextValue
          /\ QC(contextValue, roundView, phase, subject, signers).height
                  = contextValue.height
          /\ QC(contextValue, roundView, phase, subject, signers).view
                  = roundView
          /\ QC(contextValue, roundView, phase, subject, signers).phase
                  = phase
          /\ QC(contextValue, roundView, phase, subject, signers).subject
                  = subject
          /\ QC(contextValue, roundView, phase, subject, signers).signers
                  = signers
      BY DEF QC
    <2> QED BY <1>1, <2>1 DEF HistoricalQcValid
  <1> QED BY <1>1

THEOREM ExactVotesBackCertificate ==
  \A epoch, contextValue, roundView, phase, subject, signers:
    DualQuorum(epoch, signers)
      => CertificateBackedBy(
           epoch,
           QC(contextValue, roundView, phase, subject, signers),
           {Vote(contextValue, roundView, phase, subject, signer):
              signer \in signers})
PROOF
  <1>1. ASSUME NEW epoch,
              NEW contextValue,
              NEW roundView,
              NEW phase,
              NEW subject,
              NEW signers,
              DualQuorum(epoch, signers)
         PROVE CertificateBackedBy(
                 epoch,
                 QC(contextValue, roundView, phase, subject, signers),
                 {Vote(contextValue, roundView, phase, subject, signer):
                    signer \in signers})
    <2>1. \A signer \in signers \cap Honest:
             \E vote
                 \in {Vote(contextValue, roundView, phase, subject,
                           voteSigner): voteSigner \in signers}:
               VoteBacksCertificate(
                 vote,
                 QC(contextValue, roundView, phase, subject, signers),
                 signer)
      BY Isa DEF VoteBacksCertificate, Vote, QC
    <2> QED BY <1>1, <2>1 DEF CertificateBackedBy, QC
  <1> QED BY <1>1

THEOREM ExactHonestVotesHaveBodies ==
  \A contextValue, roundView, phase, subject, signers, validSubjects:
    subject \in validSubjects
      => HonestIntentSound(
           {Vote(contextValue, roundView, phase, subject, signer):
              signer \in signers},
           {BodyRecord(signer, contextValue, roundView, subject):
              signer \in signers \cap Honest},
           validSubjects)
BY Isa
   DEF HonestIntentSound, BodyHeldBy, Vote, BodyRecord

THEOREM BootstrapParentCertificatesSound ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ HistoricalQcValid(BootstrapParentPrepareQC(initialContext))
         /\ HistoricalQcValid(BootstrapParentCommitQC(initialContext))
         /\ CertificateBackedBy(
                BootstrapParentContext(initialContext).epoch,
                BootstrapParentPrepareQC(initialContext),
                BootstrapParentPrepareIntents(initialContext))
         /\ CertificateBackedBy(
                BootstrapParentContext(initialContext).epoch,
                BootstrapParentCommitQC(initialContext),
                BootstrapParentCommitIntents(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ HistoricalQcValid(
                       BootstrapParentPrepareQC(initialContext))
               /\ HistoricalQcValid(
                       BootstrapParentCommitQC(initialContext))
               /\ CertificateBackedBy(
                      BootstrapParentContext(initialContext).epoch,
                      BootstrapParentPrepareQC(initialContext),
                      BootstrapParentPrepareIntents(initialContext))
               /\ CertificateBackedBy(
                      BootstrapParentContext(initialContext).epoch,
                      BootstrapParentCommitQC(initialContext),
                      BootstrapParentCommitIntents(initialContext))
    <2>1. /\ BootstrapParentContext(initialContext) \in ContextRecords
          /\ BootstrapParentContext(initialContext).height \in Heights
          /\ BootstrapParentContext(initialContext).epoch \in Epochs
          /\ initialContext.parent \in ValidSubjects
      BY <1>1, BootstrapParentContextTyped
    <2>2. /\ BootstrapParentSigners(initialContext)
                    \subseteq ValidatorIds
          /\ ExactCertificateQuorum(
               BootstrapParentContext(initialContext).epoch,
               BootstrapParentSigners(initialContext))
          /\ DualQuorum(BootstrapParentContext(initialContext).epoch,
                        BootstrapParentSigners(initialContext))
      BY <1>1, BootstrapParentQuorumTyped
         DEF ExactCertificateQuorum
    <2>3. /\ 0 \in Views
          /\ {"Prepare", "Commit"} \subseteq Phases
      BY <1>1, SMT DEF ModelConfiguration, Views, Phases
    <2>4. /\ HistoricalQcValid(
                    BootstrapParentPrepareQC(initialContext))
          /\ HistoricalQcValid(
                    BootstrapParentCommitQC(initialContext))
      BY <2>1, <2>2, <2>3, HistoricalQcConstructorValid
         DEF BootstrapParentPrepareQC, BootstrapParentCommitQC
    <2>5. /\ CertificateBackedBy(
                   BootstrapParentContext(initialContext).epoch,
                   BootstrapParentPrepareQC(initialContext),
                   BootstrapParentPrepareIntents(initialContext))
          /\ CertificateBackedBy(
                   BootstrapParentContext(initialContext).epoch,
                   BootstrapParentCommitQC(initialContext),
                   BootstrapParentCommitIntents(initialContext))
      BY <2>2, ExactVotesBackCertificate
         DEF BootstrapParentPrepareQC, BootstrapParentCommitQC,
             BootstrapParentPrepareIntents,
             BootstrapParentCommitIntents
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM BootstrapParentHonestIntentsSound ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ HonestIntentSound(
                BootstrapParentPrepareIntents(initialContext),
                BootstrapParentBodies(initialContext), ValidSubjects)
         /\ HonestIntentSound(
                BootstrapParentCommitIntents(initialContext),
                BootstrapParentBodies(initialContext), ValidSubjects)
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ HonestIntentSound(
                       BootstrapParentPrepareIntents(initialContext),
                       BootstrapParentBodies(initialContext), ValidSubjects)
               /\ HonestIntentSound(
                       BootstrapParentCommitIntents(initialContext),
                       BootstrapParentBodies(initialContext), ValidSubjects)
    <2>1. initialContext.parent \in ValidSubjects
      BY <1>1, BootstrapParentContextTyped
    <2> QED BY <2>1, ExactHonestVotesHaveBodies
       DEF BootstrapParentPrepareIntents,
           BootstrapParentCommitIntents, BootstrapParentBodies
  <1> QED BY <1>1

THEOREM InitAtEstablishesTypeInvariant ==
  \A initialContext: InitAt(initialContext) => TypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
              InitAt(initialContext)
         PROVE TypeInvariant
    <2>1. /\ ModelConfiguration
          /\ FrozenContextAdmissible(initialContext)
          /\ initialContext \in ContextRecords
          /\ initialContext.height \in Heights
      BY <1>1, FrozenContextFieldsTyped
         DEF InitAt, FrozenContextAdmissible
    <2>2. /\ 0 \in Views
          /\ 0 \in Generations
          /\ NoRank \in Ranks
          /\ NoSubject \in SubjectOrNone
      BY <2>1, SMT
         DEF ModelConfiguration, Views, Generations, Ranks, NoRank,
             SubjectOrNone, Subjects, NoSubject
    <2>3. /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Views]
          /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Generations]
          /\ [node \in ValidatorIds |-> NoRank]
                    \in [ValidatorIds -> Ranks]
          /\ [node \in ValidatorIds |-> NoSubject]
                    \in [ValidatorIds -> SubjectOrNone]
      BY <2>2, Isa
    <2>4. /\ height = initialContext.height
          /\ context = initialContext
          /\ context.height = height
          /\ contextHistory = {context}
          /\ context \in contextHistory
          /\ contextHistory \subseteq ContextRecords
      BY <1>1, <2>1, Isa DEF InitAt
    <2>5. initialContext.height = 0
            => /\ durableBodies \subseteq BodyRecordSet
               /\ prepareIntents \subseteq VoteRecordSet
               /\ commitIntents \subseteq VoteRecordSet
               /\ prepareQCs \subseteq QcRecordSet
               /\ commitQCs \subseteq QcRecordSet
      BY <1>1, Isa DEF InitAt
    <2>6. initialContext.height > 0
            => /\ durableBodies \subseteq BodyRecordSet
               /\ prepareIntents \subseteq VoteRecordSet
               /\ commitIntents \subseteq VoteRecordSet
               /\ prepareQCs \subseteq QcRecordSet
               /\ commitQCs \subseteq QcRecordSet
      BY <1>1, BootstrapParentEvidenceTyped, Isa DEF InitAt
    <2>7. initialContext.height = 0 \/ initialContext.height > 0
      BY <2>1, SMT DEF Heights
    <2>8. /\ durableBodies \subseteq BodyRecordSet
          /\ prepareIntents \subseteq VoteRecordSet
          /\ commitIntents \subseteq VoteRecordSet
          /\ prepareQCs \subseteq QcRecordSet
          /\ commitQCs \subseteq QcRecordSet
      BY <2>5, <2>6, <2>7
    <2>9. /\ proposalIntents \subseteq ProposalRecordSet
          /\ timeoutIntents \subseteq TimeoutVoteRecordSet
          /\ \A tc \in formedTCs: TcWellTyped(tc)
          /\ \A entry \in receivedTCs:
               /\ entry.node \in ValidatorIds
               /\ TcWellTyped(entry.tc)
          /\ \A entry \in installedTCs:
               /\ entry.node \in ValidatorIds
               /\ TcWellTyped(entry.tc)
      BY <1>1, Isa DEF InitAt
    <2>10. /\ pendingProposal \subseteq ProposalWalSet
           /\ pendingPrepare \subseteq PrepareWalSet
           /\ pendingObservePrepare \subseteq ObservePrepareWalSet
           /\ pendingLockCommit \subseteq LockCommitWalSet
           /\ pendingTimeout \subseteq TimeoutWalSet
           /\ pendingInstallTC \subseteq InstallTcWalSet
           /\ pendingDecision \subseteq DecisionWalSet
           /\ signProposals \subseteq ProposalSignSet
           /\ signVotes \subseteq VoteSignSet
           /\ signTimeouts \subseteq TimeoutSignSet
      BY <1>1, Isa DEF InitAt
    <2>11. /\ height \in Heights
           /\ nodeView \in [ValidatorIds -> Views]
           /\ generation \in [ValidatorIds -> Generations]
           /\ up \subseteq ValidatorIds
           /\ gst \in BOOLEAN
           /\ lockRank \in [ValidatorIds -> Ranks]
           /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, <2>1, <2>3, Isa DEF InitAt
    <2>12. /\ availableBodies \subseteq BodyRecordSet
           /\ retainedLockedBodies \subseteq RetainedLockedBodyRecordSet
           /\ validatedBodies \subseteq ValidationRecordSet
           /\ invalidBodies \subseteq BodyRecordSet
           /\ ValidatedBodiesSound(validatedBodies, ValidSubjects)
           /\ RetainedLockedBodiesSound(retainedLockedBodies,
                                         durableBodies)
      BY <1>1, Isa
         DEF InitAt, ValidatedBodiesSound, RetainedLockedBodiesSound
    <2> QED BY <1>1, <2>1, <2>4, <2>8, <2>9, <2>10,
                  <2>11, <2>12
       DEF TypeInvariant, InitAt
  <1> QED BY <1>1

THEOREM InitEstablishesTypeInvariant == Init => TypeInvariant
BY InitAtEstablishesTypeInvariant DEF Init

THEOREM InitAtEstablishesReleaseSafety ==
  \A initialContext: InitAt(initialContext) => Safety
PROOF
  <1>1. ASSUME NEW initialContext,
              InitAt(initialContext)
         PROVE Safety
    <2>1. TypeInvariant
      BY <1>1, InitAtEstablishesTypeInvariant
    <2>2. OnePendingPersistencePerNode
      BY <1>1, Isa
         DEF InitAt, OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests
    <2>3. /\ ProposalSigningRequiresIntent
          /\ PrepareSigningRequiresIntent
          /\ CommitSigningRequiresIntent
          /\ TimeoutSigningRequiresIntent
          /\ HonestTimeoutUniqueness
          /\ AppliedRequiresDecision
      BY <1>1, Isa
         DEF InitAt, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestTimeoutUniqueness,
             AppliedRequiresDecision
    <2>4. /\ HonestPrepareUniqueness
          /\ HonestCommitUniqueness
      PROOF
        <3>1. CASE initialContext.height = 0
          <4>1. /\ prepareIntents = {}
                /\ commitIntents = {}
            BY <1>1, <3>1 DEF InitAt
          <4> QED BY <4>1, Isa
             DEF HonestPrepareUniqueness, HonestCommitUniqueness
        <3>2. CASE initialContext.height # 0
          <4>1. /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
            BY <1>1, <3>2 DEF InitAt
          <4>2. /\ \A left, right
                          \in BootstrapParentPrepareIntents(initialContext):
                       right.subject = left.subject
                /\ \A left, right
                          \in BootstrapParentCommitIntents(initialContext):
                       right.subject = left.subject
            BY BootstrapParentIntentSubjectsUniform
          <4> QED BY <4>1, <4>2
             DEF HonestPrepareUniqueness, HonestCommitUniqueness
        <3>3. initialContext.height = 0 \/ initialContext.height # 0
          BY SMT
        <3> QED BY <3>1, <3>2, <3>3
    <2>5. DecisionAgreement
      BY <1>1, Isa
         DEF InitAt, DecisionAgreement, BootstrapParentDecision,
             BootstrapParentCommitQC
    <2>6. LockBelowHighest
      BY <1>1 DEF InitAt, LockBelowHighest, NoRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6 DEF Safety
  <1> QED BY <1>1

THEOREM InitAtEstablishesReducerProvenance ==
  \A initialContext:
    InitAt(initialContext) => ReducerProvenanceInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
              InitAt(initialContext)
         PROVE ReducerProvenanceInvariant
    <2>1. /\ ModelConfiguration
          /\ FrozenContextAdmissible(initialContext)
          /\ initialContext.height \in Heights
      BY <1>1, FrozenContextFieldsTyped DEF InitAt
    <2>2. initialContext.height = 0 \/ initialContext.height > 0
      BY <2>1, SMT DEF Heights
    <2>3. /\ HonestVoteUnique(prepareIntents)
          /\ HonestVoteUnique(commitIntents)
          /\ IntentPhasesCorrect
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, HonestVoteUnique, IntentPhasesCorrect
        <3>2. CASE initialContext.height > 0
          <4>1. /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
            BY <1>1, <3>2 DEF InitAt
          <4>2. /\ HonestVoteUnique(
                          BootstrapParentPrepareIntents(initialContext))
                /\ HonestVoteUnique(
                          BootstrapParentCommitIntents(initialContext))
                /\ \A vote
                         \in BootstrapParentPrepareIntents(initialContext):
                     vote.phase = "Prepare"
                /\ \A vote
                         \in BootstrapParentCommitIntents(initialContext):
                     vote.phase = "Commit"
            BY BootstrapParentIntentsStructural
          <4> QED BY <4>1, <4>2 DEF IntentPhasesCorrect
        <3> QED BY <2>2, <3>1, <3>2
    <2>4. CertificatesBackedByIntents
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, CertificatesBackedByIntents
        <3>2. CASE initialContext.height > 0
          <4>1. /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
                /\ prepareQCs
                         = {BootstrapParentPrepareQC(initialContext)}
                /\ commitQCs
                         = {BootstrapParentCommitQC(initialContext)}
            BY <1>1, <3>2 DEF InitAt
          <4>2. /\ HistoricalQcValid(
                          BootstrapParentPrepareQC(initialContext))
                /\ HistoricalQcValid(
                          BootstrapParentCommitQC(initialContext))
                /\ CertificateBackedBy(
                       BootstrapParentContext(initialContext).epoch,
                       BootstrapParentPrepareQC(initialContext),
                       BootstrapParentPrepareIntents(initialContext))
                /\ CertificateBackedBy(
                       BootstrapParentContext(initialContext).epoch,
                       BootstrapParentCommitQC(initialContext),
                       BootstrapParentCommitIntents(initialContext))
            BY <2>1, <3>2, BootstrapParentCertificatesSound
          <4> QED BY <4>1, <4>2, Isa
             DEF CertificatesBackedByIntents,
                 BootstrapParentPrepareQC, BootstrapParentCommitQC,
                 QC
        <3> QED BY <2>2, <3>1, <3>2
    <2>5. HonestDurableIntentsSound
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, HonestDurableIntentsSound, HonestIntentSound
        <3>2. CASE initialContext.height > 0
          <4>1. /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
                /\ durableBodies = BootstrapParentBodies(initialContext)
            BY <1>1, <3>2 DEF InitAt
          <4>2. /\ HonestIntentSound(
                         BootstrapParentPrepareIntents(initialContext),
                         BootstrapParentBodies(initialContext),
                         ValidSubjects)
                /\ HonestIntentSound(
                         BootstrapParentCommitIntents(initialContext),
                         BootstrapParentBodies(initialContext),
                         ValidSubjects)
            BY <2>1, <3>2, BootstrapParentHonestIntentsSound
          <4> QED BY <4>1, <4>2 DEF HonestDurableIntentsSound
        <3> QED BY <2>2, <3>1, <3>2
    <2>6. /\ HonestTimeoutUnique(timeoutIntents)
          /\ PendingVoteWritesAuthorized
          /\ PendingCertificateWritesAuthorized
      BY <1>1, Isa
         DEF InitAt, HonestTimeoutUnique,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized
    <2>7. /\ HonestVoteTransportBacked
          /\ QcTransportBacked
          /\ HonestTimeoutTransportBacked
          /\ TcTransportBacked
          /\ FormedTimeoutCertificatesSound
          /\ DurableTimeoutsProtectCommits
      BY <1>1, Isa
         DEF InitAt, HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits,
             TimeoutIntentProtectsCommits
    <2>8. HighestAndLockAreCertified
      BY <1>1, Isa
         DEF InitAt, HighestAndLockAreCertified, NoRank, NoSubject
    <2>9. DurableLockRecoveryProvenanceInvariant
      BY <1>1, Isa
         DEF InitAt, DurableLockRecoveryProvenanceInvariant, NoRank
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9
       DEF ReducerProvenanceInvariant
  <1> QED BY <1>1

THEOREM InitAtEstablishesDurableLockRecoveryProvenance ==
  \A initialContext:
    InitAt(initialContext) => DurableLockRecoveryProvenanceInvariant
BY Isa DEF InitAt, DurableLockRecoveryProvenanceInvariant, NoRank

THEOREM BootstrapParentContextPrecedes ==
  \A initialContext:
    ModelConfiguration
      /\ FrozenContextAdmissible(initialContext)
      /\ initialContext.height > 0
      => /\ BootstrapParentContext(initialContext).height + 1
                  = initialContext.height
         /\ BootstrapParentContext(initialContext).height
                  # initialContext.height
         /\ BootstrapParentContext(initialContext) # initialContext
PROOF
  <1>1. ASSUME NEW initialContext,
              ModelConfiguration,
              FrozenContextAdmissible(initialContext),
              initialContext.height > 0
         PROVE /\ BootstrapParentContext(initialContext).height + 1
                       = initialContext.height
               /\ BootstrapParentContext(initialContext).height
                       # initialContext.height
               /\ BootstrapParentContext(initialContext) # initialContext
    <2>1. initialContext.height \in Nat
      BY <1>1, FrozenContextFieldsTyped DEF Heights
    <2>2. BootstrapParentContext(initialContext).height
             = initialContext.height - 1
      BY DEF BootstrapParentContext, ContextRecord
    <2>3. /\ BootstrapParentContext(initialContext).height + 1
                    = initialContext.height
          /\ BootstrapParentContext(initialContext).height
                    # initialContext.height
      BY <1>1, <2>1, <2>2, SMT
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM InitAtEstablishesContextSafety ==
  \A initialContext:
    InitAt(initialContext)
      => /\ ContextIdentityBindsFrozenEpoch
         /\ OldContextCertificateRejected
         /\ ContextParentWasApplied
PROOF
  <1>1. ASSUME NEW initialContext,
              InitAt(initialContext)
         PROVE /\ ContextIdentityBindsFrozenEpoch
               /\ OldContextCertificateRejected
               /\ ContextParentWasApplied
    <2>1. /\ ModelConfiguration
          /\ FrozenContextAdmissible(initialContext)
          /\ initialContext.height \in Heights
      BY <1>1, FrozenContextFieldsTyped DEF InitAt
    <2>2. initialContext.height = 0 \/ initialContext.height > 0
      BY <2>1, SMT DEF Heights
    <2>3. ContextIdentityBindsFrozenEpoch
      BY DEF ContextIdentityBindsFrozenEpoch, ContextRecords,
             ContextRecord, ExpectedEpoch
    <2>4. OldContextCertificateRejected
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, OldContextCertificateRejected
        <3>2. CASE initialContext.height > 0
          <4>1. /\ context = initialContext
                /\ prepareQCs
                         = {BootstrapParentPrepareQC(initialContext)}
                /\ commitQCs
                         = {BootstrapParentCommitQC(initialContext)}
            BY <1>1, <3>2 DEF InitAt
          <4>2. BootstrapParentContext(initialContext) # initialContext
            BY <2>1, <3>2, BootstrapParentContextPrecedes
          <4> QED BY <4>1, <4>2, Isa
             DEF OldContextCertificateRejected, QcWireValid,
                 BootstrapParentPrepareQC, BootstrapParentCommitQC,
                 QC
        <3> QED BY <2>2, <3>1, <3>2
    <2>5. ContextParentWasApplied
      <3>1. ASSUME NEW contextValue \in contextHistory,
                    contextValue.height > 0
             PROVE \E decision \in decisions:
                     /\ decision.qc.context.height + 1
                          = contextValue.height
                     /\ decision.qc.subject = contextValue.parent
                     /\ [node |-> decision.node, qc |-> decision.qc]
                          \in applied
        <4>1. /\ contextValue = initialContext
              /\ initialContext.height > 0
              /\ decisions = {BootstrapParentDecision(initialContext)}
              /\ applied = {BootstrapParentDecision(initialContext)}
          BY <1>1, <3>1, Isa DEF InitAt
        <4>2. BootstrapParentContext(initialContext).height + 1
                 = initialContext.height
          BY <2>1, <4>1, BootstrapParentContextPrecedes
        <4> QED BY <4>1, <4>2, Isa
           DEF BootstrapParentDecision, BootstrapParentCommitQC, QC
      <3> QED BY <3>1 DEF ContextParentWasApplied
    <2> QED BY <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM InitAtEstablishesLineageInvariant ==
  \A initialContext: InitAt(initialContext) => LineageInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
              InitAt(initialContext)
         PROVE LineageInvariant
    <2>1. /\ ModelConfiguration
          /\ FrozenContextAdmissible(initialContext)
          /\ initialContext.height \in Heights
      BY <1>1, FrozenContextFieldsTyped DEF InitAt
    <2>2. initialContext.height = 0 \/ initialContext.height > 0
      BY <2>1, SMT DEF Heights
    <2>3. PrepareLineageSound
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, PrepareLineageSound,
                 PrepareCarriesHigherSafeQc
        <3>2. CASE initialContext.height > 0
          <4>1. /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
            BY <1>1, <3>2 DEF InitAt
          <4> QED BY <4>1, Isa
             DEF PrepareLineageSound, PrepareCarriesHigherSafeQc,
                 BootstrapParentPrepareIntents,
                 BootstrapParentCommitIntents, Vote
        <3> QED BY <2>2, <3>1, <3>2
    <2>4. /\ LocksCoverOwnCommits
          /\ CurrentIntentViewsBound
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, LocksCoverOwnCommits, CurrentIntentViewsBound
        <3>2. CASE initialContext.height > 0
          <4>1. /\ context = initialContext
                /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
                /\ timeoutIntents = {}
            BY <1>1, <3>2 DEF InitAt
          <4>2. BootstrapParentContext(initialContext) # initialContext
            BY <2>1, <3>2, BootstrapParentContextPrecedes
          <4> QED BY <4>1, <4>2, Isa
             DEF LocksCoverOwnCommits, CurrentIntentViewsBound,
                 BootstrapParentPrepareIntents,
                 BootstrapParentCommitIntents, Vote
        <3> QED BY <2>2, <3>1, <3>2
    <2>5. HonestCommitIntentPrepared
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, HonestCommitIntentPrepared,
                 CommitIntentsPreparedBy
        <3>2. CASE initialContext.height > 0
          <4>1. /\ context = initialContext
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
                /\ prepareQCs
                         = {BootstrapParentPrepareQC(initialContext)}
            BY <1>1, <3>2 DEF InitAt
          <4>2. BootstrapParentContext(initialContext) # initialContext
            BY <2>1, <3>2, BootstrapParentContextPrecedes
          <4> QED BY <4>1, <4>2, Isa
             DEF HonestCommitIntentPrepared, CommitIntentsPreparedBy,
                 BootstrapParentCommitIntents,
                 BootstrapParentPrepareQC, Vote, QC
        <3> QED BY <2>2, <3>1, <3>2
    <2>6. CertificatePhasesCorrect
      BY <1>1, Isa
         DEF InitAt, CertificatePhasesCorrect,
             BootstrapParentPrepareQC, BootstrapParentCommitQC, QC
    <2>7. DurableIntentsDoNotAnticipateHeight
      PROOF
        <3>1. CASE initialContext.height = 0
          BY <1>1, <3>1, Isa
             DEF InitAt, DurableIntentsDoNotAnticipateHeight
        <3>2. CASE initialContext.height > 0
          <4>1. /\ height = initialContext.height
                /\ prepareIntents
                         = BootstrapParentPrepareIntents(initialContext)
                /\ commitIntents
                         = BootstrapParentCommitIntents(initialContext)
                /\ timeoutIntents = {}
            BY <1>1, <3>2 DEF InitAt
          <4>2. /\ BootstrapParentContext(initialContext).height \in Heights
                /\ BootstrapParentContext(initialContext).height + 1
                     = initialContext.height
            BY <2>1, <3>2, BootstrapParentContextTyped,
               BootstrapParentContextPrecedes
          <4>3. /\ \A vote
                         \in BootstrapParentPrepareIntents(initialContext):
                       vote.context.height
                         = BootstrapParentContext(initialContext).height
                /\ \A vote
                         \in BootstrapParentCommitIntents(initialContext):
                       vote.context.height
                         = BootstrapParentContext(initialContext).height
            BY BootstrapParentIntentContextHeights
          <4>4. BootstrapParentContext(initialContext).height <= height
            BY <2>1, <4>1, <4>2, SMT DEF Heights
          <4>5. \A vote \in prepareIntents:
                   vote.context.height <= height
            BY <4>1, <4>3, <4>4
          <4>6. \A vote \in commitIntents:
                   vote.context.height <= height
            BY <4>1, <4>3, <4>4
          <4>7. \A vote \in timeoutIntents:
                   vote.context.height <= height
            BY <4>1, Isa
          <4> QED BY <4>5, <4>6, <4>7
             DEF DurableIntentsDoNotAnticipateHeight
        <3> QED BY <2>2, <3>1, <3>2
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7
       DEF LineageInvariant
  <1> QED BY <1>1

THEOREM InitAtEstablishesStrongInductiveInvariant ==
  \A initialContext:
    InitAt(initialContext) => StrongInductiveInvariant
BY InitAtEstablishesReleaseSafety,
   InitAtEstablishesReducerProvenance,
   InitAtEstablishesContextSafety,
   InitAtEstablishesLineageInvariant
   DEF StrongInductiveInvariant

THEOREM InitEstablishesReleaseSafety == Init => Safety
BY InitAtEstablishesReleaseSafety DEF Init

THEOREM InitEstablishesReducerProvenance ==
  Init => ReducerProvenanceInvariant
BY InitAtEstablishesReducerProvenance DEF Init

THEOREM InitEstablishesContextSafety ==
  Init
    => /\ ContextIdentityBindsFrozenEpoch
       /\ OldContextCertificateRejected
       /\ ContextParentWasApplied
BY InitAtEstablishesContextSafety DEF Init

THEOREM InitEstablishesLineageInvariant == Init => LineageInvariant
BY InitAtEstablishesLineageInvariant DEF Init

THEOREM InitEstablishesStrongInductiveInvariant ==
  Init => StrongInductiveInvariant
BY InitAtEstablishesStrongInductiveInvariant DEF Init

(***************************************************************************
Exact same-round LockAndCommit admission is forced by the pending-write and
timeout-protection invariants. Installed TC provenance has no Commit-creation
branch; its high is consumed only by later proposal justification.
***************************************************************************)
THEOREM PendingLockCommitUsesExactCurrentRound ==
  PendingVoteWritesAuthorized
    => \A request \in pendingLockCommit:
         /\ request.vote.view = nodeView[request.node]
         /\ CurrentOpenPrepareForCommit(request.node, request.qc)
BY SMT DEF PendingVoteWritesAuthorized, CurrentOpenPrepareForCommit

THEOREM DurableTimeoutProtectionIsDirect ==
  DurableTimeoutsProtectCommits
    => \A timeoutVote \in timeoutIntents,
          commitVote \in commitIntents:
         (/\ timeoutVote.signer \in Honest
          /\ commitVote.signer = timeoutVote.signer
          /\ commitVote.context = timeoutVote.context
          /\ commitVote.phase = "Commit"
          /\ commitVote.view <= timeoutVote.view)
           => TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)
BY DEF DurableTimeoutsProtectCommits, TimeoutIntentProtectsCommits,
       TimeoutVoteProtectsCommitSet

THEOREM ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization ==
  ReducerProvenanceInvariant
    => SameRoundLockAndCommitAuthorizationInvariant
BY PendingLockCommitUsesExactCurrentRound,
   DurableTimeoutProtectionIsDirect
   DEF ReducerProvenanceInvariant,
       SameRoundLockAndCommitAuthorizationInvariant

\* Compatibility theorems for downstream proof steps. Their conclusions are
\* aliases of the exact same-round invariant and contain no historical branch.
THEOREM PendingLowerLockCommitRequiresHistoricalTcAuthorization ==
  PendingVoteWritesAuthorized
    => \A request \in pendingLockCommit:
         request.vote.view < nodeView[request.node]
           => HistoricalLockedPrepareForCommit(request.node, request.qc)
BY PendingLockCommitUsesExactCurrentRound
   DEF HistoricalLockedPrepareForCommit

THEOREM DurableTimeoutProtectionSuppliesInstalledTcAuthorization ==
  DurableTimeoutsProtectCommits
    => \A timeoutVote \in timeoutIntents,
          commitVote \in commitIntents:
         (/\ timeoutVote.signer \in Honest
          /\ commitVote.signer = timeoutVote.signer
          /\ commitVote.context = timeoutVote.context
          /\ commitVote.phase = "Commit"
          /\ commitVote.view <= timeoutVote.view
          /\ ~TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote))
           => InstalledTcAuthorizesCommitVote(commitVote)
BY DurableTimeoutProtectionIsDirect
   DEF InstalledTcAuthorizesCommitVote

THEOREM ReducerProvenanceImpliesHistoricalLockedCommitAuthorization ==
  ReducerProvenanceInvariant
    => HistoricalLockedCommitAuthorizationInvariant
BY ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization
   DEF HistoricalLockedCommitAuthorizationInvariant

THEOREM ReducerProvenanceImpliesHistoricalTcLockedCommitAuthorization ==
  ReducerProvenanceInvariant
    => HistoricalTcLockedCommitAuthorizationInvariant
BY ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization
   DEF HistoricalTcLockedCommitAuthorizationInvariant

THEOREM UnchangedPendingVoteWriteVarsPreservesAuthorization ==
  PendingVoteWritesAuthorized
    /\ UNCHANGED <<context, nodeView, durableBodies, receivedQCs,
                   prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                   installedTCs, lockRank, lockSubject, highestRank,
                   highestSubject, pendingPrepare, pendingLockCommit,
                   pendingTimeout>>
    => PendingVoteWritesAuthorized'
PROOF
  <1>1. ASSUME PendingVoteWritesAuthorized,
              UNCHANGED <<context, nodeView, durableBodies, receivedQCs,
                          prepareIntents, commitIntents, timeoutIntents,
                          prepareQCs, installedTCs, lockRank, lockSubject,
                          highestRank, highestSubject, pendingPrepare,
                          pendingLockCommit, pendingTimeout>>
         PROVE PendingVoteWritesAuthorized'
    <2>1. /\ context' = context
          /\ nodeView' = nodeView
          /\ durableBodies' = durableBodies
          /\ receivedQCs' = receivedQCs
          /\ prepareIntents' = prepareIntents
          /\ commitIntents' = commitIntents
          /\ timeoutIntents' = timeoutIntents
          /\ prepareQCs' = prepareQCs
          /\ installedTCs' = installedTCs
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
          /\ highestRank' = highestRank
          /\ highestSubject' = highestSubject
          /\ pendingPrepare' = pendingPrepare
          /\ pendingLockCommit' = pendingLockCommit
          /\ pendingTimeout' = pendingTimeout
      BY <1>1, Isa
    <2>2. (\A request \in pendingPrepare:
             /\ request.node \in Honest
             /\ request.vote.phase = "Prepare"
             /\ request.vote.signer = request.node
             /\ request.vote.context = context
             /\ request.vote.view = nodeView[request.node]
             /\ request.vote.subject \in ValidSubjects
             /\ BodyHeldBy(durableBodies, request.node,
                           request.vote.context, request.vote.view,
                           request.vote.subject)
             /\ CanAppendVote(prepareIntents, request.vote)
             /\ PrepareCarriesHigherSafeQc(request.vote))'
      BY <1>1, <2>1, Isa
         DEF PendingVoteWritesAuthorized, PrepareCarriesHigherSafeQc
    <2>3. (\A request \in pendingLockCommit:
             /\ request.node \in Honest
             /\ request.vote =
                  Vote(context, request.qc.view, "Commit",
                       request.qc.subject, request.node)
             /\ request.vote.phase = "Commit"
             /\ request.vote.signer = request.node
             /\ request.vote.context = context
             /\ request.vote.context = request.qc.context
             /\ request.vote.view = request.qc.view
             /\ request.vote.subject = request.qc.subject
             /\ request.qc.phase = "Prepare"
             /\ request.qc \in prepareQCs
             /\ \/ CurrentOpenPrepareForCommit(request.node, request.qc)
                \/ HistoricalLockedPrepareForCommit(request.node, request.qc)
             /\ request.vote.subject \in ValidSubjects
             /\ BodyHeldBy(durableBodies, request.node,
                           request.vote.context, request.vote.view,
                           request.vote.subject)
             /\ request.qc.view >= lockRank[request.node]
             /\ (request.qc.view = lockRank[request.node]
                   => request.qc.subject = lockSubject[request.node])
             /\ CanAppendVote(commitIntents, request.vote))'
      BY <1>1, <2>1, Isa
         DEF PendingVoteWritesAuthorized, CurrentOpenPrepareForCommit,
             HistoricalLockedPrepareForCommit,
             InstalledTcSelectsPrepareFor,
             NoHigherPrepareOriginKnown, NodeTimedOut
    <2>4. (\A request \in pendingTimeout:
             /\ request.node \in Honest
             /\ request.vote.signer = request.node
             /\ request.vote.context = context
             /\ request.vote.view = nodeView[request.node]
             /\ CanAppendTimeout(timeoutIntents, request.vote)
             /\ TimeoutVoteProtectsCommitSet(
                  request.vote, commitIntents))'
      BY <1>1, <2>1, Isa
         DEF PendingVoteWritesAuthorized, TimeoutVoteProtectsCommitSet,
             TimeoutVoteStrictlyProtectsCommit,
             InstalledTcAuthorizesCommitVote
    <2> QED BY <2>2, <2>3, <2>4 DEF PendingVoteWritesAuthorized
  <1> QED BY <1>1

THEOREM UnchangedLineageVarsPreservesLineageInvariant ==
  LineageInvariant /\ UNCHANGED LineageVars => LineageInvariant'
BY Isa
   DEF LineageInvariant, LineageVars, PrepareLineageSound,
       PrepareCarriesHigherSafeQc, LocksCoverOwnCommits,
       CurrentIntentViewsBound, HonestCommitIntentPrepared,
       CertificatePhasesCorrect, DurableIntentsDoNotAnticipateHeight

THEOREM UnchangedDurableLockRecoveryProvenanceVarsPreserves ==
  DurableLockRecoveryProvenanceInvariant
    /\ UNCHANGED
         <<context, commitIntents, installedTCs, lockRank, lockSubject>>
    => DurableLockRecoveryProvenanceInvariant'
BY Isa
   DEF DurableLockRecoveryProvenanceInvariant,
       ExactLockedCommitIntents, NoRank

THEOREM UnchangedProvenanceVarsPreservesReducerProvenance ==
  ReducerProvenanceInvariant /\ UNCHANGED ProvenanceVars
    => ReducerProvenanceInvariant'
PROOF
  <1>1. ASSUME ReducerProvenanceInvariant,
              UNCHANGED ProvenanceVars
         PROVE ReducerProvenanceInvariant'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
      BY <1>1, Isa
         DEF ProvenanceVars, ReducerProvenanceInvariant,
             HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect,
             SameVoteSlot, SameTimeoutSlot, SameTimeoutContent
    <2>2. PendingVoteWritesAuthorized'
      <3>1. UNCHANGED <<context, nodeView, durableBodies, receivedQCs,
                        prepareIntents, commitIntents, timeoutIntents,
                        prepareQCs, installedTCs, lockRank, lockSubject,
                        highestRank, highestSubject, pendingPrepare,
                        pendingLockCommit, pendingTimeout>>
        BY <1>1, Isa DEF ProvenanceVars
      <3> QED BY <1>1, <3>1,
                   UnchangedPendingVoteWriteVarsPreservesAuthorization
         DEF ReducerProvenanceInvariant
    <2>3. PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF ProvenanceVars, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>4. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. HonestVoteTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>2. QcTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>3. HonestTimeoutTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2>5. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. CertificatesBackedByIntents'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>2. HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound
      <3>4. DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               DurableTimeoutsProtectCommits, TimeoutIntentProtectsCommits,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3>5. HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>6. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF ReducerProvenanceInvariant, ProvenanceVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
       DEF ReducerProvenanceInvariant
  <1> QED BY <1>1

THEOREM UnchangedVoteIndependentProvenancePreserves ==
  ReducerProvenanceWithoutVoteTransport
    /\ UNCHANGED ProvenanceWithoutVoteTransportVars
    => ReducerProvenanceWithoutVoteTransport'
PROOF
  <1>1. ASSUME ReducerProvenanceWithoutVoteTransport,
              UNCHANGED ProvenanceWithoutVoteTransportVars
         PROVE ReducerProvenanceWithoutVoteTransport'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      <3>1. UNCHANGED <<context, nodeView, durableBodies, receivedQCs,
                        prepareIntents, commitIntents, timeoutIntents,
                        prepareQCs, installedTCs, lockRank, lockSubject,
                        highestRank, highestSubject, pendingPrepare,
                        pendingLockCommit, pendingTimeout>>
        BY <1>1, Isa DEF ProvenanceWithoutVoteTransportVars
      <3>2. PendingVoteWritesAuthorized'
        BY <1>1, <3>1,
           UnchangedPendingVoteWriteVarsPreservesAuthorization
           DEF ReducerProvenanceWithoutVoteTransport
      <3>3. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingCertificateWritesAuthorized'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect,
               PendingCertificateWritesAuthorized,
               SameVoteSlot, SameTimeoutSlot, SameTimeoutContent,
               TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3
    <2>2. /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutVoteTransport,
             ProvenanceWithoutVoteTransportVars, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
    <2>3. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. CertificatesBackedByIntents'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               CertificatesBackedByIntents
      <3>2. HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               FormedTimeoutCertificatesSound
      <3>4. DurableTimeoutsProtectCommits'
        <4>1. /\ timeoutIntents' = timeoutIntents
              /\ commitIntents' = commitIntents
              /\ installedTCs' = installedTCs
          BY <1>1 DEF ProvenanceWithoutVoteTransportVars
        <4>2. DurableTimeoutsProtectCommits
          BY <1>1 DEF ReducerProvenanceWithoutVoteTransport
        <4> QED BY <4>1, <4>2, Isa
           DEF DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3>5. HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>4. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF ReducerProvenanceWithoutVoteTransport,
             ProvenanceWithoutVoteTransportVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4
       DEF ReducerProvenanceWithoutVoteTransport
  <1> QED BY <1>1

THEOREM PersistPreparePreservesLineageInvariant ==
  \A request:
    TypeInvariant /\ LineageInvariant /\ PendingVoteWritesAuthorized
      /\ PersistPrepare(request)
      => LineageInvariant'
PROOF
  <1>1. ASSUME NEW request,
              TypeInvariant,
              LineageInvariant,
              PendingVoteWritesAuthorized,
              PersistPrepare(request)
         PROVE LineageInvariant'
    <2>1. request \in pendingPrepare
      BY <1>1 DEF PersistPrepare
    <2>2. /\ request.vote.signer \in Honest
          /\ PrepareCarriesHigherSafeQc(request.vote)
          /\ request.vote.context = context
          /\ request.vote.view = nodeView[request.vote.signer]
      BY <1>1, <2>1
         DEF PendingVoteWritesAuthorized
    <2>3. /\ prepareIntents' = prepareIntents \cup {request.vote}
          /\ context' = context
          /\ nodeView' = nodeView
          /\ commitIntents' = commitIntents
          /\ timeoutIntents' = timeoutIntents
          /\ prepareQCs' = prepareQCs
          /\ commitQCs' = commitQCs
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
      BY <1>1 DEF PersistPrepare
    <2>4. PrepareLineageSound'
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest
             PROVE PrepareCarriesHigherSafeQc(vote)'
        <4>1. vote \in prepareIntents \/ vote = request.vote
          BY <2>3, <3>1
        <4>2. CASE vote \in prepareIntents
          BY <1>1, <2>3, <3>1, <4>2
             DEF LineageInvariant, PrepareLineageSound,
                 PrepareCarriesHigherSafeQc
        <4>3. CASE vote = request.vote
          BY <2>2, <2>3, <3>1, <4>3
             DEF PrepareCarriesHigherSafeQc
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1 DEF PrepareLineageSound
    <2>5. LocksCoverOwnCommits'
      BY <1>1, <2>3, IsaM("blast")
         DEF LineageInvariant, LocksCoverOwnCommits
    <2>6. \A vote \in prepareIntents':
              (vote.signer \in Honest /\ vote.context = context')
                => vote.view <= nodeView'[vote.signer]
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest,
                    vote.context = context'
             PROVE vote.view <= nodeView'[vote.signer]
        <4>1. vote \in prepareIntents \/ vote = request.vote
          BY <2>3, <3>1
        <4>2. CASE vote \in prepareIntents
          BY <1>1, <2>3, <3>1, <4>2
             DEF LineageInvariant, CurrentIntentViewsBound
        <4>3. CASE vote = request.vote
          <5>1. /\ vote.view = request.vote.view
                /\ vote.signer = request.vote.signer
            BY <4>3
          <5>2. request.vote.view =
                   nodeView[request.vote.signer]
            BY <2>2
          <5>3. nodeView'[vote.signer] =
                   nodeView[request.vote.signer]
            BY <2>3, <5>1
          <5>4. vote.view = nodeView'[vote.signer]
            BY <5>1, <5>2, <5>3
          <5>5. vote.signer \in ValidatorIds
            BY <1>1, <3>1, SMT
               DEF TypeInvariant, ModelConfiguration,
                   QuorumConfiguration
          <5>6. nodeView[vote.signer] \in Views
            BY <1>1, <5>5 DEF TypeInvariant
          <5>7. nodeView'[vote.signer] \in Nat
            <6>1. ViewDomain \subseteq Nat
              BY <1>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     ModelConfiguration
            <6>2. nodeView[vote.signer] \in Nat
              BY <5>6, <6>1 DEF Views
            <6> QED BY <5>1, <5>3, <6>2
          <5> QED BY <5>4, <5>7, NaturalOrderReflexive
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
    <2>7. \A vote \in commitIntents':
              (vote.signer \in Honest /\ vote.context = context')
                => vote.view <= nodeView'[vote.signer]
      <3>1. ASSUME NEW vote \in commitIntents',
                    vote.signer \in Honest,
                    vote.context = context'
             PROVE vote.view <= nodeView'[vote.signer]
        <4>1. vote \in commitIntents
          BY <2>3, <3>1
        <4>2. vote.context = context
          BY <2>3, <3>1
        <4>3. vote.view <= nodeView[vote.signer]
          BY <1>1, <3>1, <4>1, <4>2
             DEF LineageInvariant, HonestCommitIntentPrepared
        <4>4. nodeView'[vote.signer] = nodeView[vote.signer]
          BY <2>3
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2>8. CurrentIntentViewsBound'
      BY <1>1, <2>3, <2>6, Isa
         DEF LineageInvariant, CurrentIntentViewsBound
    <2>9. HonestCommitIntentPrepared'
      BY <1>1, <2>3, Isa
         DEF LineageInvariant, HonestCommitIntentPrepared
    <2>10. CertificatePhasesCorrect'
      BY <1>1, <2>3, Isa
         DEF LineageInvariant, CertificatePhasesCorrect
    <2>11. DurableIntentsDoNotAnticipateHeight'
      <3>1. DurableIntentsDoNotAnticipateHeight
        BY <1>1 DEF LineageInvariant
      <3>2. request.vote.context.height <= height
        BY <1>1, <2>2, SMT
           DEF TypeInvariant, Heights
      <3> QED BY <1>1, <2>3, <3>1, <3>2, Isa
         DEF DurableIntentsDoNotAnticipateHeight, PersistPrepare
    <2> QED BY <2>4, <2>5, <2>8, <2>9, <2>10, <2>11
       DEF LineageInvariant
  <1> QED BY <1>1

THEOREM CommitPreparationIsMonotone ==
  \A commits, before, after:
    CommitIntentsPreparedBy(commits, before)
      /\ before \subseteq after
      => CommitIntentsPreparedBy(commits, after)
BY DEF CommitIntentsPreparedBy

THEOREM FormPrepareQCPreservesLineageInvariant ==
  \A node, roundView, subject:
    LineageInvariant /\ FormPrepareQC(node, roundView, subject)
      => LineageInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              LineageInvariant,
              FormPrepareQC(node, roundView, subject)
         PROVE LineageInvariant'
    <2>1. /\ prepareQCs \subseteq prepareQCs'
          /\ prepareIntents' = prepareIntents
          /\ commitIntents' = commitIntents
          /\ timeoutIntents' = timeoutIntents
          /\ commitQCs' = commitQCs
          /\ context' = context
          /\ nodeView' = nodeView
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
          /\ \A qc \in prepareQCs' \ prepareQCs:
               qc.phase = "Prepare"
      BY <1>1, SMT DEF FormPrepareQC, QC
    <2>2. PrepareLineageSound'
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest,
                    NEW commitVote \in commitIntents',
                    /\ vote.signer \in Honest
                    /\ commitVote.signer = vote.signer
                    /\ commitVote.context = vote.context
                    /\ commitVote.phase = "Commit"
                    /\ commitVote.view < vote.view
                    /\ commitVote.subject # vote.subject
             PROVE \E qc \in prepareQCs':
                     /\ qc.context = vote.context
                     /\ qc.phase = "Prepare"
                     /\ commitVote.view < qc.view
                     /\ qc.view < vote.view
                     /\ qc.subject = vote.subject
        <4>1. PrepareCarriesHigherSafeQc(vote)
          BY <1>1, <2>1, <3>1
             DEF LineageInvariant, PrepareLineageSound
        <4>2. PICK qc \in prepareQCs:
                 /\ qc.context = vote.context
                 /\ qc.phase = "Prepare"
                 /\ commitVote.view < qc.view
                 /\ qc.view < vote.view
                 /\ qc.subject = vote.subject
          BY <2>1, <3>1, <4>1 DEF PrepareCarriesHigherSafeQc
        <4> QED BY <2>1, <4>2
      <3> QED BY <3>1
         DEF PrepareLineageSound, PrepareCarriesHigherSafeQc
    <2>3. /\ LocksCoverOwnCommits'
          /\ CurrentIntentViewsBound'
      BY <1>1, <2>1, Isa
         DEF LineageInvariant, LocksCoverOwnCommits,
             CurrentIntentViewsBound
    <2>4. HonestCommitIntentPrepared'
      BY <1>1, <2>1, CommitPreparationIsMonotone
         DEF LineageInvariant, HonestCommitIntentPrepared
    <2>5. CertificatePhasesCorrect'
      BY <1>1, <2>1, SMT
         DEF LineageInvariant, CertificatePhasesCorrect
    <2>6. DurableIntentsDoNotAnticipateHeight'
      BY <1>1, <2>1, Isa
         DEF LineageInvariant, DurableIntentsDoNotAnticipateHeight,
             FormPrepareQC
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
       DEF LineageInvariant
  <1> QED BY <1>1

THEOREM SetGSTPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ SetGST
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              SetGST
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant, SetGST
    <2>2. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, SetGST,
             OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>3. Safety'
      BY <2>1, <2>2 DEF Safety
    <2>4. ContextIdentityBindsFrozenEpoch'
      BY <1>1
         DEF StrongInductiveInvariant, ContextIdentityBindsFrozenEpoch
    <2>5. OldContextCertificateRejected'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, SetGST,
             OldContextCertificateRejected, QcValid, QcWireValid, CurrentEpoch
    <2>6. ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, SetGST, ContextParentWasApplied
    <2>7. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, SetGST, ProvenanceVars
    <2>8. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, SetGST, LineageVars
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ProofRelevantStutterPreservesStrongInvariant ==
  StrongInductiveInvariant
    /\ availableBodies' \subseteq BodyRecordSet
    /\ validatedBodies' \subseteq ValidationRecordSet
    /\ invalidBodies' \subseteq BodyRecordSet
    /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
    /\ UNCHANGED ProofRelevantVars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              availableBodies' \subseteq BodyRecordSet,
              validatedBodies' \subseteq ValidationRecordSet,
              invalidBodies' \subseteq BodyRecordSet,
              ValidatedBodiesSound(validatedBodies', ValidSubjects),
              UNCHANGED ProofRelevantVars
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ProofRelevantVars, Safety,
             TypeInvariant, ValidatedBodiesSound,
             RetainedLockedBodiesSound,
             OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, IsaM("blast")
         DEF StrongInductiveInvariant, ProofRelevantVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>3. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ProofRelevantVars, ProvenanceVars
    <2>4. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, ProofRelevantVars, LineageVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverProposalPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverProposal(envelope)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF DeliverProposal, ProofRelevantVars, ValidatedBodiesSound,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM ByzantineBroadcastProposalPreservesStrongInvariant ==
  \A signer, roundView, subject, justifyRank, justifySubject:
    StrongInductiveInvariant
      /\ ByzantineBroadcastProposal(signer, roundView, subject,
                                    justifyRank, justifySubject)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF ByzantineBroadcastProposal, ProofRelevantVars,
       ValidatedBodiesSound, StrongInductiveInvariant, Safety, TypeInvariant

THEOREM FetchBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ FetchBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF FetchBody, ProofRelevantVars, ValidatedBodiesSound,
       BodyRecordSet, ValidationRecordSet,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM RebindRetainedBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ RebindRetainedBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF RebindRetainedBody, ProofRelevantVars, ValidatedBodiesSound,
       BodyRecordSet, ValidationRecordSet,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM ValidateBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ ValidateBody(node, proposal)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW proposal,
              StrongInductiveInvariant,
              ValidateBody(node, proposal)
         PROVE StrongInductiveInvariant'
    <2>1. ValidatedBodiesSound(validatedBodies', ValidSubjects)
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ValidatedBodiesSound, ValidateBody, ValidationRecord
    <2>2. /\ availableBodies' \subseteq BodyRecordSet
          /\ validatedBodies' \subseteq ValidationRecordSet
          /\ invalidBodies' \subseteq BodyRecordSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant, ValidateBody
    <2>3. UNCHANGED ProofRelevantVars
      BY <1>1 DEF ValidateBody, ProofRelevantVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                  ProofRelevantStutterPreservesStrongInvariant
  <1> QED BY <1>1

THEOREM ValidateDecidedBodyPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ ValidateDecidedBody(node, qc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW qc,
              StrongInductiveInvariant,
              ValidateDecidedBody(node, qc)
         PROVE StrongInductiveInvariant'
    <2>1. ValidatedBodiesSound(validatedBodies', ValidSubjects)
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ValidatedBodiesSound, ValidateDecidedBody, ValidationRecord
    <2>2. /\ availableBodies' \subseteq BodyRecordSet
          /\ validatedBodies' \subseteq ValidationRecordSet
          /\ invalidBodies' \subseteq BodyRecordSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ValidateDecidedBody
    <2>3. UNCHANGED ProofRelevantVars
      BY <1>1 DEF ValidateDecidedBody, ProofRelevantVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                  ProofRelevantStutterPreservesStrongInvariant
  <1> QED BY <1>1

THEOREM ValidateLockedBodyPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ ValidateLockedBody(node, qc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW qc,
              StrongInductiveInvariant,
              ValidateLockedBody(node, qc)
         PROVE StrongInductiveInvariant'
    <2>1. ValidatedBodiesSound(validatedBodies', ValidSubjects)
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ValidatedBodiesSound, ValidateLockedBody, ValidationRecord
    <2>2. /\ availableBodies' \subseteq BodyRecordSet
          /\ validatedBodies' \subseteq ValidationRecordSet
          /\ invalidBodies' \subseteq BodyRecordSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ValidateLockedBody
    <2>3. UNCHANGED ProofRelevantVars
      BY <1>1 DEF ValidateLockedBody, ProofRelevantVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                  ProofRelevantStutterPreservesStrongInvariant
  <1> QED BY <1>1

THEOREM RejectBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ RejectBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF RejectBody, ProofRelevantVars, ValidatedBodiesSound,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM FetchCertifiedBodyPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ FetchCertifiedBody(node, qc)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF FetchCertifiedBody, InstallCertifiedBodyEffect,
       ProofRelevantVars, ValidatedBodiesSound,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM AcceptCertifiedResponseCapabilityPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant
      /\ AcceptCertifiedResponseCapability(node, roundView, subject)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF AcceptCertifiedResponseCapability, InstallCertifiedBodyEffect,
       ProofRelevantVars, ValidatedBodiesSound,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM DropProposalPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DropProposal(envelope)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF DropProposal, ProofRelevantVars, ValidatedBodiesSound,
       StrongInductiveInvariant, Safety, TypeInvariant

THEOREM HonestIntentSoundIsMonotoneInDurableBodies ==
  \A intents, before, after, validSubjects:
    HonestIntentSound(intents, before, validSubjects)
      /\ before \subseteq after
      => HonestIntentSound(intents, after, validSubjects)
BY DEF HonestIntentSound, BodyHeldBy

THEOREM BodyHeldIsMonotone ==
  \A before, after, node, bodyContext, roundView, subject:
    before \subseteq after
      /\ BodyHeldBy(before, node, bodyContext, roundView, subject)
      => BodyHeldBy(after, node, bodyContext, roundView, subject)
BY DEF BodyHeldBy

THEOREM RetainedLockedBodiesSoundIsMonotoneInDurableBodies ==
  \A retained, before, after:
    RetainedLockedBodiesSound(retained, before)
      /\ before \subseteq after
      => RetainedLockedBodiesSound(retained, after)
BY DEF RetainedLockedBodiesSound, BodyHeldBy

THEOREM UnchangedDurableTimeoutProtectionVarsPreserves ==
  DurableTimeoutsProtectCommits
    /\ UNCHANGED <<timeoutIntents, commitIntents, installedTCs>>
    => DurableTimeoutsProtectCommits'
BY Isa
   DEF DurableTimeoutsProtectCommits,
       TimeoutIntentProtectsCommits,
       TimeoutVoteProtectsCommitSet,
       TimeoutVoteStrictlyProtectsCommit,
       InstalledTcAuthorizesCommitVote

THEOREM InstalledTcGrowthPreservesDurableTimeoutProtection ==
  DurableTimeoutsProtectCommits
    /\ installedTCs \subseteq installedTCs'
    /\ UNCHANGED <<timeoutIntents, commitIntents>>
    => DurableTimeoutsProtectCommits'
BY Isa
   DEF DurableTimeoutsProtectCommits,
       TimeoutIntentProtectsCommits,
       TimeoutVoteProtectsCommitSet,
       TimeoutVoteStrictlyProtectsCommit,
       InstalledTcAuthorizesCommitVote

THEOREM UnchangedHighestAndLockCertificationVarsPreserves ==
  HighestAndLockAreCertified
    /\ UNCHANGED <<context, prepareQCs, lockRank, lockSubject,
                   highestRank, highestSubject>>
    => HighestAndLockAreCertified'
BY Isa DEF HighestAndLockAreCertified

THEOREM PersistLockCommitPreservesDurableLockRecoveryProvenance ==
  \A request:
    /\ StrongInductiveInvariant
    /\ PersistLockCommit(request)
    => DurableLockRecoveryProvenanceInvariant'
BY SMTT(90), Isa
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized,
       DurableLockRecoveryProvenanceInvariant,
       ExactLockedCommitIntents, PersistLockCommit

THEOREM PersistInstallTCPreservesDurableLockRecoveryProvenance ==
  \A request:
    /\ StrongInductiveInvariant
    /\ PersistInstallTC(request)
    => DurableLockRecoveryProvenanceInvariant'
BY SMTT(120), Isa
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       DurableLockRecoveryProvenanceInvariant,
       ExactLockedCommitIntents, PersistInstallTC,
       ResultingInstallLockRank, ResultingInstallLockSubject

(***************************************************************************
InstallTimeout is a generation boundary in the production reducer:
`Continuation::InstallTimeout` clears `body_work` before publishing EnterView
or recovery Fetch effects.  These leaves pin the corresponding source
semantics: the installing node has no old volatile validation receipt, while
other reducers' receipts and the validation type/soundness invariants remain
intact.
***************************************************************************)
THEOREM PersistInstallTCClearsInstallingNodeValidationReceipts ==
  \A request:
    PersistInstallTC(request)
      => /\ validatedBodies' =
               {validation \in validatedBodies:
                  validation.node # request.node}
         /\ \A validation \in validatedBodies':
              validation.node # request.node
BY Isa DEF PersistInstallTC

THEOREM PersistInstallTCPreservesOtherNodeValidationReceipts ==
  \A request, validation:
    /\ PersistInstallTC(request)
    /\ validation.node # request.node
    => (validation \in validatedBodies'
          <=> validation \in validatedBodies)
BY Isa DEF PersistInstallTC

THEOREM PersistInstallTCPreservesValidationReceiptTypeAndSoundness ==
  \A request:
    /\ TypeInvariant
    /\ PersistInstallTC(request)
    => /\ validatedBodies' \subseteq ValidationRecordSet
       /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
BY Isa
   DEF TypeInvariant, PersistInstallTC, ValidatedBodiesSound

THEOREM AdvanceContextPreservesDurableLockRecoveryProvenance ==
  \A subject:
    AdvanceContext(subject)
      => DurableLockRecoveryProvenanceInvariant'
BY Isa
   DEF AdvanceContext, DurableLockRecoveryProvenanceInvariant, NoRank

THEOREM DurableGrowthPreservesStrongInvariant ==
  StrongInductiveInvariant
    /\ durableBodies \subseteq durableBodies'
    /\ availableBodies' \subseteq BodyRecordSet
    /\ durableBodies' \subseteq BodyRecordSet
    /\ validatedBodies' \subseteq ValidationRecordSet
    /\ invalidBodies' \subseteq BodyRecordSet
    /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
    /\ UNCHANGED ProofRelevantWithoutDurableVars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              durableBodies \subseteq durableBodies',
              availableBodies' \subseteq BodyRecordSet,
              durableBodies' \subseteq BodyRecordSet,
              validatedBodies' \subseteq ValidationRecordSet,
              invalidBodies' \subseteq BodyRecordSet,
              ValidatedBodiesSound(validatedBodies', ValidSubjects),
              UNCHANGED ProofRelevantWithoutDurableVars
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      <3>1. RetainedLockedBodiesSound(retainedLockedBodies,
                                      durableBodies)
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. retainedLockedBodies' = retainedLockedBodies
        BY <1>1 DEF ProofRelevantWithoutDurableVars
      <3>3. RetainedLockedBodiesSound(retainedLockedBodies',
                                      durableBodies')
        BY <1>1, <3>1, <3>2,
           RetainedLockedBodiesSoundIsMonotoneInDurableBodies
      <3>4. TypeInvariant'
        BY <1>1, <3>3, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ProofRelevantWithoutDurableVars
      <3>5. UNCHANGED
                 <<pendingProposal, pendingPrepare,
                   pendingObservePrepare, pendingLockCommit,
                   pendingTimeout, pendingInstallTC, pendingDecision,
                   proposalIntents, prepareIntents, commitIntents,
                   timeoutIntents, signProposals, signVotes, signTimeouts,
                   lockRank, highestRank, commitQCs, decisions, applied>>
        BY <1>1, Isa DEF ProofRelevantWithoutDurableVars
      <3>6. OnePendingPersistencePerNode'
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode,
               RequestsUniqueByNode, AllPendingRequests
      <3>7. /\ ProposalSigningRequiresIntent'
            /\ PrepareSigningRequiresIntent'
            /\ CommitSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety,
               ProposalSigningRequiresIntent,
               PrepareSigningRequiresIntent,
               CommitSigningRequiresIntent,
               TimeoutSigningRequiresIntent
      <3>8. /\ HonestPrepareUniqueness'
            /\ HonestCommitUniqueness'
            /\ HonestTimeoutUniqueness'
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety,
               HonestPrepareUniqueness, HonestCommitUniqueness,
               HonestTimeoutUniqueness
      <3>9. LockBelowHighest'
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety, LockBelowHighest
      <3>10. /\ DecisionAgreement'
             /\ AppliedRequiresDecision'
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety,
               DecisionAgreement, AppliedRequiresDecision
      <3> QED BY <3>4, <3>6, <3>7, <3>8, <3>9, <3>10 DEF Safety
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>3. HonestDurableIntentsSound'
      BY <1>1, HonestIntentSoundIsMonotoneInDurableBodies
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound,
             ProofRelevantWithoutDurableVars
    <2>4. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. \A request \in pendingPrepare':
               /\ request.node \in Honest
               /\ request.vote.phase = "Prepare"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.view, request.vote.subject)
               /\ CanAppendVote(prepareIntents', request.vote)
               /\ PrepareCarriesHigherSafeQc(request.vote)'
        <4>1. ASSUME NEW request \in pendingPrepare'
               PROVE /\ request.node \in Honest
                     /\ request.vote.phase = "Prepare"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context'
                     /\ request.vote.view = nodeView'[request.node]
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', request.node,
                                   request.vote.context, request.vote.view, request.vote.subject)
                     /\ CanAppendVote(prepareIntents', request.vote)
                     /\ PrepareCarriesHigherSafeQc(request.vote)'
          <5>1. BodyHeldBy(durableBodies, request.node,
                          request.vote.context, request.vote.view, request.vote.subject)
            BY <1>1, <3>1, <4>1
               DEF ProofRelevantWithoutDurableVars,
                   PendingVoteWritesAuthorized
          <5>2. BodyHeldBy(durableBodies', request.node,
                          request.vote.context, request.vote.view, request.vote.subject)
            BY <1>1, <5>1, BodyHeldIsMonotone
          <5> QED BY <1>1, <3>1, <4>1, <5>2, Isa
             DEF ProofRelevantWithoutDurableVars,
                 PendingVoteWritesAuthorized,
                 PrepareCarriesHigherSafeQc
        <4> QED BY <4>1
      <3>3. \A request \in pendingLockCommit':
               /\ request.node \in Honest
               /\ request.vote =
                    Vote(context', request.qc.view, "Commit",
                         request.qc.subject, request.node)
               /\ request.vote.phase = "Commit"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.context = request.qc.context
               /\ request.vote.view = request.qc.view
               /\ request.vote.subject = request.qc.subject
               /\ request.qc.phase = "Prepare"
               /\ request.qc \in prepareQCs'
               /\ \/ CurrentOpenPrepareForCommit(
                        request.node, request.qc)'
                  \/ HistoricalLockedPrepareForCommit(
                        request.node, request.qc)'
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.view, request.vote.subject)
               /\ request.qc.view >= lockRank'[request.node]
               /\ (request.qc.view = lockRank'[request.node]
                     => request.qc.subject = lockSubject'[request.node])
               /\ CanAppendVote(commitIntents', request.vote)
        <4>1. ASSUME NEW request \in pendingLockCommit'
               PROVE /\ request.node \in Honest
                     /\ request.vote =
                          Vote(context', request.qc.view, "Commit",
                               request.qc.subject, request.node)
                     /\ request.vote.phase = "Commit"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context'
                     /\ request.vote.context = request.qc.context
                     /\ request.vote.view = request.qc.view
                     /\ request.vote.subject = request.qc.subject
                     /\ request.qc.phase = "Prepare"
                     /\ request.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              request.node, request.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              request.node, request.qc)'
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', request.node,
                                   request.vote.context, request.vote.view, request.vote.subject)
                     /\ request.qc.view >= lockRank'[request.node]
                     /\ (request.qc.view = lockRank'[request.node]
                           => request.qc.subject = lockSubject'[request.node])
                     /\ CanAppendVote(commitIntents', request.vote)
          <5>1. BodyHeldBy(durableBodies, request.node,
                          request.vote.context, request.vote.view, request.vote.subject)
            BY <1>1, <3>1, <4>1
               DEF ProofRelevantWithoutDurableVars,
                   PendingVoteWritesAuthorized
          <5>2. BodyHeldBy(durableBodies', request.node,
                          request.vote.context, request.vote.view, request.vote.subject)
            BY <1>1, <5>1, BodyHeldIsMonotone
          <5>3. request \in pendingLockCommit
            BY <1>1, <4>1 DEF ProofRelevantWithoutDurableVars
          <5>4. /\ request.node \in Honest
                 /\ request.vote =
                      Vote(context, request.qc.view, "Commit",
                           request.qc.subject, request.node)
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs
                 /\ \/ CurrentOpenPrepareForCommit(
                          request.node, request.qc)
                    \/ HistoricalLockedPrepareForCommit(
                          request.node, request.qc)
                 /\ request.vote.subject \in ValidSubjects
                 /\ request.qc.view >= lockRank[request.node]
                 /\ (request.qc.view = lockRank[request.node]
                       => request.qc.subject = lockSubject[request.node])
                 /\ CanAppendVote(commitIntents, request.vote)
            BY <3>1, <5>3
               DEF PendingVoteWritesAuthorized,
                   CurrentOpenPrepareForCommit
          <5>5. /\ context' = context
                 /\ nodeView' = nodeView
                 /\ timeoutIntents' = timeoutIntents
                 /\ prepareQCs' = prepareQCs
                 /\ lockRank' = lockRank
                 /\ lockSubject' = lockSubject
                 /\ commitIntents' = commitIntents
            BY <1>1 DEF ProofRelevantWithoutDurableVars
          <5>6. /\ request.node \in Honest
                 /\ request.vote =
                      Vote(context', request.qc.view, "Commit",
                           request.qc.subject, request.node)
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context'
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs'
                 /\ \/ CurrentOpenPrepareForCommit(
                          request.node, request.qc)'
                    \/ HistoricalLockedPrepareForCommit(
                          request.node, request.qc)'
                 /\ request.vote.subject \in ValidSubjects
                 /\ request.qc.view >= lockRank'[request.node]
                 /\ (request.qc.view = lockRank'[request.node]
                       => request.qc.subject = lockSubject'[request.node])
                 /\ CanAppendVote(commitIntents', request.vote)
            BY <1>1, <5>4, <5>5, Isa
               DEF ProofRelevantWithoutDurableVars,
                   CurrentOpenPrepareForCommit,
                   HistoricalLockedPrepareForCommit,
                   InstalledTcSelectsPrepareFor,
                   NoHigherPrepareOriginKnown, NodeTimedOut
          <5> QED BY <5>2, <5>6
        <4> QED BY <4>1
      <3>4. \A request \in pendingTimeout':
               /\ request.node \in Honest
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ CanAppendTimeout(timeoutIntents', request.vote)
               /\ TimeoutVoteProtectsCommitSet(request.vote,
                                               commitIntents)'
        BY <1>1, <3>1, Isa
           DEF ProofRelevantWithoutDurableVars,
               PendingVoteWritesAuthorized,
               NodeTimedOut,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3> QED BY <3>2, <3>3, <3>4
         DEF PendingVoteWritesAuthorized
    <2>5. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingCertificateWritesAuthorized, TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>6. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>7. CertificatesBackedByIntents'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             CertificatesBackedByIntents
    <2>8. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             FormedTimeoutCertificatesSound
    <2>9. /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. UNCHANGED <<timeoutIntents, commitIntents, installedTCs>>
        BY <1>1, Isa DEF ProofRelevantWithoutDurableVars
      <3>2. DurableTimeoutsProtectCommits
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>3. DurableTimeoutsProtectCommits'
        BY <3>1, <3>2,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>4. UNCHANGED <<context, prepareQCs, lockRank, lockSubject,
                        highestRank, highestSubject>>
        BY <1>1, Isa DEF ProofRelevantWithoutDurableVars
      <3>5. HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>6. HighestAndLockAreCertified'
        BY <3>4, <3>5,
           UnchangedHighestAndLockCertificationVarsPreserves
      <3> QED BY <3>3, <3>6
    <2>9a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars
    <2>10. ReducerProvenanceInvariant'
      BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9, <2>9a
         DEF ReducerProvenanceInvariant
    <2>11. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             LineageVars
    <2> QED BY <2>1, <2>2, <2>10, <2>11
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM AssembleLocalBodyPreservesStrongInvariant ==
  \A node, subject:
    StrongInductiveInvariant /\ AssembleLocalBody(node, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW subject,
              StrongInductiveInvariant,
              AssembleLocalBody(node, subject)
         PROVE StrongInductiveInvariant'
    <2>1. durableBodies \subseteq durableBodies'
      BY <1>1 DEF AssembleLocalBody
    <2>2. ValidatedBodiesSound(validatedBodies, ValidSubjects)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>3. ValidatedBodiesSound(validatedBodies', ValidSubjects)
      BY <1>1, <2>2, Isa
         DEF AssembleLocalBody, ValidatedBodiesSound, ValidationRecord
    <2>4. /\ availableBodies' \subseteq BodyRecordSet
          /\ durableBodies' \subseteq BodyRecordSet
          /\ validatedBodies' \subseteq ValidationRecordSet
          /\ invalidBodies' \subseteq BodyRecordSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             AssembleLocalBody
    <2>5. UNCHANGED ProofRelevantWithoutDurableVars
      BY <1>1 DEF AssembleLocalBody, ProofRelevantWithoutDurableVars
    <2> QED BY <1>1, <2>1, <2>3, <2>4, <2>5,
                  DurableGrowthPreservesStrongInvariant
  <1> QED BY <1>1

THEOREM StoreBodyPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ StoreBody(node, roundView, subject)
      => StrongInductiveInvariant'
BY DurableGrowthPreservesStrongInvariant
   DEF StoreBody, ProofRelevantWithoutDurableVars,
       ValidatedBodiesSound, StrongInductiveInvariant, Safety, TypeInvariant

THEOREM PendingNodesAreAllRequestNodes ==
  PendingNodes = RequestNodeSet(AllPendingRequests)
BY Isa DEF PendingNodes, RequestNodeSet, AllPendingRequests

THEOREM NonInstallWalRequestsHaveValidatorNodes ==
  \A request \in ProposalWalSet \cup PrepareWalSet
                  \cup ObservePrepareWalSet \cup LockCommitWalSet
                  \cup TimeoutWalSet \cup DecisionWalSet:
    request.node \in ValidatorIds
BY Isa DEF ProposalWalSet, PrepareWalSet, ObservePrepareWalSet,
           LockCommitWalSet, TimeoutWalSet, DecisionWalSet

THEOREM TypeInvariantTypesAllPendingNodes ==
  TypeInvariant
    => \A request \in AllPendingRequests:
         request.node \in ValidatorIds
PROOF
  <1>1. ASSUME TypeInvariant,
              NEW request \in AllPendingRequests
         PROVE request.node \in ValidatorIds
    <2>1. CASE request \in pendingInstallTC
      <3>1. request \in InstallTcWalSet
        BY <1>1, <2>1 DEF TypeInvariant
      <3> QED BY <3>1 DEF InstallTcWalSet
    <2>2. CASE request \notin pendingInstallTC
      <3>1. request \in ProposalWalSet \cup PrepareWalSet
                        \cup ObservePrepareWalSet \cup LockCommitWalSet
                        \cup TimeoutWalSet \cup DecisionWalSet
        BY <1>1, <2>2, Isa DEF TypeInvariant, AllPendingRequests
      <3> QED BY <3>1, NonInstallWalRequestsHaveValidatorNodes
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM TypeInvariantTypesAllIntentVotes ==
  TypeInvariant
    => \A vote \in prepareIntents \cup commitIntents \cup timeoutIntents:
         /\ vote.signer \in ValidatorIds
         /\ vote.view \in Views
PROOF
  <1>1. ASSUME TypeInvariant,
              NEW vote \in prepareIntents \cup commitIntents
                              \cup timeoutIntents
         PROVE /\ vote.signer \in ValidatorIds
               /\ vote.view \in Views
    <2>1. CASE vote \in prepareIntents
      <3>1. vote \in VoteRecordSet
        BY <1>1, <2>1 DEF TypeInvariant
      <3> QED BY <3>1 DEF VoteRecordSet
    <2>2. CASE vote \in commitIntents
      <3>1. vote \in VoteRecordSet
        BY <1>1, <2>2 DEF TypeInvariant
      <3> QED BY <3>1 DEF VoteRecordSet
    <2>3. CASE vote \in timeoutIntents
      <3>1. vote \in TimeoutVoteRecordSet
        BY <1>1, <2>3 DEF TypeInvariant
      <3> QED BY <3>1 DEF TimeoutVoteRecordSet
    <2> QED BY <1>1, <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM InstallKindExcludesOtherWalSets ==
  \A request:
    request.kind = "InstallTC"
      => request \notin ProposalWalSet \cup PrepareWalSet
                         \cup ObservePrepareWalSet \cup LockCommitWalSet
                         \cup TimeoutWalSet \cup DecisionWalSet
BY Isa DEF ProposalWalSet, PrepareWalSet, ObservePrepareWalSet,
           LockCommitWalSet, TimeoutWalSet, DecisionWalSet

THEOREM NewRequestPreservesNodeUniqueness ==
  \A requests, request:
    RequestsUniqueByNode(requests)
      /\ request.node \notin RequestNodeSet(requests)
      => RequestsUniqueByNode(requests \cup {request})
BY SMT DEF RequestsUniqueByNode, RequestNodeSet

THEOREM RemovingRequestsPreservesNodeUniqueness ==
  \A before, after:
    RequestsUniqueByNode(before) /\ after \subseteq before
      => RequestsUniqueByNode(after)
BY DEF RequestsUniqueByNode

THEOREM DistinctUniqueRequestsHaveDistinctNodes ==
  \A requests, left, right:
    RequestsUniqueByNode(requests)
      /\ left \in requests
      /\ right \in requests
      /\ left # right
      => left.node # right.node
BY SMT DEF RequestsUniqueByNode

THEOREM ValidatedBodySubjectIsExternallyValid ==
  \A node, bodyContext, roundView, bodyGeneration, subject:
    TypeInvariant
      /\ BodyValidatedBy(validatedBodies, node, bodyContext, roundView,
                         bodyGeneration, subject)
      => subject \in ValidSubjects
PROOF
  <1>1. ASSUME NEW node, NEW bodyContext, NEW roundView,
              NEW bodyGeneration, NEW subject,
              TypeInvariant,
              BodyValidatedBy(validatedBodies, node, bodyContext,
                              roundView, bodyGeneration, subject)
         PROVE subject \in ValidSubjects
    <2>1. ValidationRecord(node, bodyContext, roundView,
                           bodyGeneration, subject) \in validatedBodies
      BY <1>1 DEF BodyValidatedBy
    <2>2. ValidatedBodiesSound(validatedBodies, ValidSubjects)
      BY <1>1 DEF TypeInvariant
    <2> QED BY <2>1, <2>2
       DEF ValidatedBodiesSound, ValidationRecord
  <1> QED BY <1>1

THEOREM BeginLocalProposalValidityIsDerived ==
  \A node, subject:
    StrongInductiveInvariant /\ BeginLocalProposal(node, subject)
      => ProposalValidFor(node, LocalProposalFor(node, subject))
PROOF
  <1>1. ASSUME NEW node, NEW subject,
              StrongInductiveInvariant,
              BeginLocalProposal(node, subject)
         PROVE ProposalValidFor(node, LocalProposalFor(node, subject))
    <2>1. /\ TypeInvariant
          /\ ProposalWireValidFor(node, LocalProposalFor(node, subject))
          /\ BodyValidatedBy(validatedBodies, node, context,
                             nodeView[node], generation[node], subject)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, BeginLocalProposal,
             LocalProposalFor, PrepareSignerAvailability
    <2>2. subject \in ValidSubjects
      BY <2>1, ValidatedBodySubjectIsExternallyValid
    <2> QED BY <2>1, <2>2
       DEF ProposalValidFor, LocalProposalFor, Proposal
  <1> QED BY <1>1

THEOREM BeginLocalProposalReproposesExactJustifiedHigh ==
  \A node, subject:
    BeginLocalProposal(node, subject)
      => LET justification == LocalProposalJustification(node)
         IN \/ justification.rank = NoRank
            \/ subject = justification.subject
BY SMT
   DEF BeginLocalProposal, LocalProposalReproposesJustifiedHigh,
       LocalProposalFor, LocalProposalJustification, Proposal

THEOREM BeginPrepareProposalValidityIsDerived ==
  \A node, proposal:
    StrongInductiveInvariant /\ BeginPrepare(node, proposal)
      => ProposalValidFor(node, proposal)
PROOF
  <1>1. ASSUME NEW node, NEW proposal,
              StrongInductiveInvariant,
              BeginPrepare(node, proposal)
         PROVE ProposalValidFor(node, proposal)
    <2>1. /\ TypeInvariant
          /\ ProposalWireValidFor(node, proposal)
          /\ BodyValidatedBy(validatedBodies, node, context,
                             proposal.view, generation[node],
                             proposal.subject)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, BeginPrepare,
             PrepareSignerAvailability
    <2>2. proposal.subject \in ValidSubjects
      BY <2>1, ValidatedBodySubjectIsExternallyValid
    <2> QED BY <2>1, <2>2 DEF ProposalValidFor
  <1> QED BY <1>1

THEOREM SafeProposalCarriesCommitLineage ==
  \A node, proposal:
    node \in ValidatorIds
      /\ TypeInvariant
      /\ LineageInvariant
      /\ HighestAndLockAreCertified
      /\ ProposalValidFor(node, proposal)
      /\ lockRank[node] < proposal.view
      => PrepareCarriesHigherSafeQc(PrepareVoteFor(node, proposal))
PROOF
  <1>1. ASSUME NEW node, NEW proposal,
              node \in ValidatorIds,
              TypeInvariant,
              LineageInvariant,
              HighestAndLockAreCertified,
              ProposalValidFor(node, proposal),
              lockRank[node] < proposal.view
         PROVE PrepareCarriesHigherSafeQc(
                 PrepareVoteFor(node, proposal))
    <2>1. ASSUME NEW commitVote \in commitIntents,
                  /\ PrepareVoteFor(node, proposal).signer \in Honest
                  /\ commitVote.signer =
                       PrepareVoteFor(node, proposal).signer
                  /\ commitVote.context =
                       PrepareVoteFor(node, proposal).context
                  /\ commitVote.phase = "Commit"
                  /\ commitVote.view <
                       PrepareVoteFor(node, proposal).view
                  /\ commitVote.subject #
                       PrepareVoteFor(node, proposal).subject
           PROVE \E qc \in prepareQCs:
                   /\ qc.context = PrepareVoteFor(node, proposal).context
                   /\ qc.phase = "Prepare"
                   /\ commitVote.view < qc.view
                   /\ qc.view < PrepareVoteFor(node, proposal).view
                   /\ qc.subject =
                        PrepareVoteFor(node, proposal).subject
      <3>1. /\ commitVote.signer = node
            /\ commitVote.context = context
            /\ commitVote.subject # proposal.subject
            /\ PrepareVoteFor(node, proposal).context = context
            /\ PrepareVoteFor(node, proposal).view = proposal.view
            /\ PrepareVoteFor(node, proposal).subject = proposal.subject
        BY <1>1, <2>1
           DEF PrepareVoteFor, Vote, ProposalValidFor
      <3>2. /\ lockRank[node] >= commitVote.view
            /\ (lockRank[node] = commitVote.view
                  => lockSubject[node] = commitVote.subject)
        BY <1>1, <2>1, <3>1 DEF LineageInvariant, LocksCoverOwnCommits
      <3>3. /\ commitVote.view \in Views
            /\ commitVote.view \in Nat
        BY <1>1, <2>1, SMT
           DEF TypeInvariant, VoteRecordSet, Views,
               ModelConfiguration
      <3>4. lockRank[node] # NoRank
        BY <3>2, <3>3, SMT DEF Views, NoRank
      <3>5. CASE lockSubject[node] = proposal.subject
        <4>1. lockRank[node] > commitVote.view
          BY <3>1, <3>2, <3>5, SMT
        <4>2. \E qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = lockRank[node]
                 /\ qc.subject = lockSubject[node]
          BY <1>1, <3>4 DEF HighestAndLockAreCertified
        <4>3. PICK qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = lockRank[node]
                 /\ qc.subject = lockSubject[node]
          BY <4>2
        <4>4. qc.phase = "Prepare"
          BY <1>1, <4>3
             DEF LineageInvariant, CertificatePhasesCorrect
        <4>5. /\ qc.context = PrepareVoteFor(node, proposal).context
              /\ qc.phase = "Prepare"
              /\ commitVote.view < qc.view
              /\ qc.view < PrepareVoteFor(node, proposal).view
              /\ qc.subject = PrepareVoteFor(node, proposal).subject
          BY <1>1, <3>1, <3>5, <4>1, <4>3, <4>4
        <4> QED BY <4>3, <4>5
      <3>6. CASE lockSubject[node] # proposal.subject
        <4>1. /\ proposal.justifyRank > lockRank[node]
              /\ proposal.justifySubject = proposal.subject
          <5>1. SafeToPrepare(node, proposal)
            BY <1>1 DEF ProposalValidFor, ProposalWireValidFor
          <5> QED BY <3>4, <3>6, <5>1 DEF SafeToPrepare
        <4>2. proposal.view > 0
          <5>1. commitVote.view < proposal.view
            BY <2>1, <3>1
          <5>2. /\ proposal.view \in Views
                /\ proposal.view \in Nat
            <6>1. /\ node \in ValidatorIds
                  /\ nodeView \in [ValidatorIds -> Views]
                  /\ proposal.view = nodeView[node]
                  /\ ViewDomain \subseteq Nat
              BY <1>1
                 DEF TypeInvariant, ModelConfiguration, ProposalValidFor,
                     ProposalWireValidFor
            <6> QED BY <6>1, FunctionValueHasCodomain DEF Views
          <5>3. proposal.view > 0
            BY <3>3, <5>1, <5>2, NaturalStrictUpperIsPositive
          <5> QED BY <5>3
        <4>3. /\ proposal.justifyRank < proposal.view
              /\ \E qc \in prepareQCs:
                   /\ qc.context = context
                   /\ qc.view = proposal.justifyRank
                   /\ qc.subject = proposal.justifySubject
          <5>1. ProposalJustified(node, proposal)
            BY <1>1 DEF ProposalValidFor, ProposalWireValidFor
          <5>2. proposal.justifyRank # NoRank
            <6>1. /\ commitVote.view >= 0
                  /\ lockRank[node] >= commitVote.view
                  /\ proposal.justifyRank > lockRank[node]
              BY <3>2, <3>3, <4>1, SMT
            <6>2. /\ commitVote.view \in Int
                  /\ lockRank[node] \in Int
                  /\ proposal.justifyRank \in Int
              BY <1>1, <3>3, <4>2, <5>1, SMT
                 DEF TypeInvariant, ModelConfiguration, Ranks, Views,
                     ProposalJustified, AuthenticatedHighRef, HighRefValid, NoRank
            <6>3. /\ lockRank[node] >= 0
                  /\ proposal.justifyRank >= 1
                  /\ commitVote.view < proposal.justifyRank
              BY <6>1, <6>2, IntegerOrderChain
            <6> QED BY <6>3, SMT DEF NoRank
          <5>3. /\ proposal.justifyRank < proposal.view
                /\ HighRefValid(proposal.justifyRank,
                                proposal.justifySubject)
            BY <4>2, <5>1, Isa
               DEF ProposalJustified, AuthenticatedHighRef
          <5> QED BY <5>2, <5>3 DEF HighRefValid
        <4>4. PICK qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = proposal.justifyRank
                 /\ qc.subject = proposal.justifySubject
          BY <4>3
        <4>5. qc.phase = "Prepare"
          BY <1>1, <4>4
             DEF LineageInvariant, CertificatePhasesCorrect
        <4>6. /\ qc.context = PrepareVoteFor(node, proposal).context
              /\ qc.phase = "Prepare"
              /\ commitVote.view < qc.view
              /\ qc.view < PrepareVoteFor(node, proposal).view
              /\ qc.subject = PrepareVoteFor(node, proposal).subject
          <5>1. qc.context = PrepareVoteFor(node, proposal).context
            BY <3>1, <4>4
          <5>2. qc.phase = "Prepare"
            BY <4>5
          <5>3. /\ commitVote.view \in Int
                /\ lockRank[node] \in Int
                /\ qc.view \in Int
            BY <1>1, <3>3, <4>4, SMT
               DEF TypeInvariant, ModelConfiguration, Ranks, Views,
                   QcRecordSet, NoRank
          <5>4. commitVote.view < qc.view
            BY <3>2, <4>1, <4>4, <5>3, IntegerOrderChain
          <5>5. qc.view < PrepareVoteFor(node, proposal).view
            BY <3>1, <4>3, <4>4
          <5>6. qc.subject = PrepareVoteFor(node, proposal).subject
            BY <3>1, <4>1, <4>4
          <5> QED BY <5>1, <5>2, <5>4, <5>5, <5>6
        <4> QED BY <4>4, <4>6
      <3> QED BY <3>5, <3>6
    <2> QED BY <2>1 DEF PrepareCarriesHigherSafeQc
  <1> QED BY <1>1

THEOREM BeginLocalProposalPreservesStrongInvariant ==
  \A node, subject:
    StrongInductiveInvariant /\ BeginLocalProposal(node, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW subject,
              StrongInductiveInvariant,
              BeginLocalProposal(node, subject)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginLocalProposal
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginLocalProposal, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests
                   \cup {ProposalWal(node,
                                     LocalProposalFor(node, subject))}
        BY <1>1, Isa DEF BeginLocalProposal, AllPendingRequests
      <3>4. ProposalWal(node, LocalProposalFor(node, subject)).node = node
        BY DEF ProposalWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests
                 \cup {ProposalWal(node,
                                   LocalProposalFor(node, subject))})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, BeginLocalProposal,
             ProofRelevantWithoutPendingProposalVars,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginLocalProposal,
             ProofRelevantWithoutPendingProposalVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>6. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, BeginLocalProposal, ProvenanceVars
    <2> QED BY <1>1, <2>4, <2>5, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginLocalProposal, LineageVars
  <1> QED BY <1>1

THEOREM PersistProposalPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistProposal(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistProposal(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      <3>1. /\ request.node \in ValidatorIds
            /\ request.proposal \in ProposalRecordSet
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal, ProposalWalSet
      <3>2. proposalIntents \subseteq ProposalRecordSet
        BY <1>1 DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>3. proposalIntents' \subseteq ProposalRecordSet
        BY <1>1, <3>1, <3>2, Isa DEF PersistProposal
      <3>4. pendingProposal' \subseteq ProposalWalSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal
      <3>5. ProposalSign(request.node, request.proposal)
                 \in ProposalSignSet
        BY <3>1 DEF ProposalSign, ProposalSignSet
      <3>6. signProposals' \subseteq ProposalSignSet
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal
      <3> QED BY <1>1, <3>3, <3>4, <3>6, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistProposal
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistProposal, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>3. ProposalSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistProposal,
             ProposalSigningRequiresIntent, ProposalSign
    <2>4. /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistProposal,
             PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>5. Safety'
      BY <2>1, <2>2, <2>3, <2>4 DEF Safety
    <2>6. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistProposal,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>7. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, PersistProposal, ProvenanceVars
    <2> QED BY <1>1, <2>5, <2>6, <2>7,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, PersistProposal, LineageVars
  <1> QED BY <1>1

THEOREM CompleteProposalSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteProposalSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteProposalSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteProposalSignature
    <2>2. Safety'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety,
             CompleteProposalSignature,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             TypeInvariant
    <2>3. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteProposalSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>4. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, CompleteProposalSignature,
             ProvenanceVars
    <2> QED BY <1>1, <2>2, <2>3, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteProposalSignature, LineageVars
  <1> QED BY <1>1

THEOREM ResumeProposalPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ ResumeProposal(node, proposal)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW proposal,
              StrongInductiveInvariant,
              ResumeProposal(node, proposal)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeProposal, ProposalSign, ProposalSignSet
    <2>2. ProposalSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeProposal,
             ProposalSigningRequiresIntent, ProposalSign
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeProposal,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance, Isa
         DEF StrongInductiveInvariant, ResumeProposal,
             ProvenanceVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <1>1, <2>3, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeProposal, LineageVars
  <1> QED BY <1>1

THEOREM BeginPreparePreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ BeginPrepare(node, proposal)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW proposal,
              StrongInductiveInvariant,
              BeginPrepare(node, proposal)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginPrepare
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginPrepare, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests
                   \cup {PrepareRequestFor(node, proposal)}
        BY <1>1, Isa DEF BeginPrepare, AllPendingRequests
      <3>4. PrepareRequestFor(node, proposal).node = node
        BY DEF PrepareRequestFor, PrepareWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests
                 \cup {PrepareRequestFor(node, proposal)})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, BeginPrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginPrepare,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>6. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. PrepareCarriesHigherSafeQc(PrepareVoteFor(node, proposal))
        <4>1. /\ node \in ValidatorIds
              /\ TypeInvariant
              /\ LineageInvariant
              /\ HighestAndLockAreCertified
              /\ ProposalValidFor(node, proposal)
              /\ lockRank[node] < proposal.view
          <5>1. /\ node \in Honest
                /\ Honest \subseteq ValidatorIds
            BY <1>1
               DEF BeginPrepare, StrongInductiveInvariant, Safety,
                   TypeInvariant, ModelConfiguration,
                   QuorumConfiguration
          <5>2. /\ TypeInvariant
                /\ LineageInvariant
                /\ HighestAndLockAreCertified
            BY <1>1
               DEF StrongInductiveInvariant, Safety,
                   ReducerProvenanceInvariant
          <5>3. /\ ProposalValidFor(node, proposal)
                /\ lockRank[node] < proposal.view
            BY <1>1, BeginPrepareProposalValidityIsDerived
               DEF BeginPrepare
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1, SafeProposalCarriesCommitLineage
      <3>3. /\ PrepareRequestFor(node, proposal).node \in Honest
            /\ PrepareRequestFor(node, proposal).vote.phase = "Prepare"
            /\ PrepareRequestFor(node, proposal).vote.signer =
                 PrepareRequestFor(node, proposal).node
            /\ PrepareRequestFor(node, proposal).vote.context = context
            /\ PrepareRequestFor(node, proposal).vote.view = nodeView[node]
            /\ PrepareRequestFor(node, proposal).vote.subject
                 \in ValidSubjects
            /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                           proposal.subject)
            /\ CanAppendVote(prepareIntents,
                             PrepareRequestFor(node, proposal).vote)
        <4>1. ProposalValidFor(node, proposal)
          BY <1>1, BeginPrepareProposalValidityIsDerived
        <4>2. /\ node \in Honest
              /\ proposal.view = nodeView[node]
              /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                             proposal.subject)
              /\ ~(\E prior \in prepareIntents:
                       /\ prior.signer = node
                       /\ prior.context = context
                       /\ prior.view = proposal.view)
          BY <1>1, <4>1
             DEF BeginPrepare, ProposalValidFor, ProposalWireValidFor,
                 PrepareSignerAvailability
        <4>3. /\ PrepareRequestFor(node, proposal).node = node
              /\ PrepareRequestFor(node, proposal).vote.phase = "Prepare"
              /\ PrepareRequestFor(node, proposal).vote.signer = node
              /\ PrepareRequestFor(node, proposal).vote.context = context
              /\ PrepareRequestFor(node, proposal).vote.view = proposal.view
              /\ PrepareRequestFor(node, proposal).vote.subject =
                   proposal.subject
          BY DEF PrepareRequestFor, PrepareVoteFor, PrepareWal, Vote
        <4>4. CanAppendVote(prepareIntents,
                            PrepareRequestFor(node, proposal).vote)
          <5>1. ASSUME NEW prior \in prepareIntents,
                        SameVoteSlot(
                          prior, PrepareRequestFor(node, proposal).vote)
                 PROVE prior.subject =
                         PrepareRequestFor(node, proposal).vote.subject
            <6>1. /\ prior.signer = node
                  /\ prior.context = context
                  /\ prior.view = proposal.view
              BY <4>3, <5>1 DEF SameVoteSlot
            <6> QED BY <4>2, <6>1
          <5> QED BY <4>2, <5>1 DEF CanAppendVote
        <4> QED BY <4>1, <4>2, <4>3, <4>4 DEF ProposalValidFor
      <3>4. /\ pendingPrepare' =
                     pendingPrepare \cup {PrepareRequestFor(node, proposal)}
            /\ pendingLockCommit' = pendingLockCommit
            /\ pendingTimeout' = pendingTimeout
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ context' = context
            /\ nodeView' = nodeView
            /\ durableBodies' = durableBodies
            /\ receivedQCs' = receivedQCs
            /\ prepareQCs' = prepareQCs
            /\ installedTCs' = installedTCs
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
            /\ highestRank' = highestRank
            /\ highestSubject' = highestSubject
        BY <1>1 DEF BeginPrepare
      <3>5. \A pending \in pendingPrepare':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Prepare"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
               /\ CanAppendVote(prepareIntents', pending.vote)
               /\ PrepareCarriesHigherSafeQc(pending.vote)'
        <4>1. ASSUME NEW pending \in pendingPrepare'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.phase = "Prepare"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ CanAppendVote(prepareIntents', pending.vote)
                     /\ PrepareCarriesHigherSafeQc(pending.vote)'
          <5>1. pending \in pendingPrepare
                  \/ pending = PrepareRequestFor(node, proposal)
            BY <3>4, <4>1
          <5>2. CASE pending \in pendingPrepare
            BY <3>1, <3>4, <4>1, <5>2, IsaM("blast")
               DEF PendingVoteWritesAuthorized,
                   PrepareCarriesHigherSafeQc
          <5>3. CASE pending = PrepareRequestFor(node, proposal)
            <6>1. PrepareCarriesHigherSafeQc(
                     PrepareRequestFor(node, proposal).vote)
              BY <3>2 DEF PrepareRequestFor, PrepareWal
            <6>2. (PrepareRequestFor(node, proposal).vote)' =
                     PrepareRequestFor(node, proposal).vote
              BY <3>4
                 DEF PrepareRequestFor, PrepareVoteFor, PrepareWal, Vote
            <6>3. PrepareCarriesHigherSafeQc(
                     PrepareRequestFor(node, proposal).vote)'
              BY <3>4, <6>1, <6>2 DEF PrepareCarriesHigherSafeQc
            <6>4. /\ pending.node =
                        PrepareRequestFor(node, proposal).node
                  /\ pending.vote =
                        PrepareRequestFor(node, proposal).vote
              BY <5>3
            <6>5. /\ pending.node = node
                  /\ pending.vote = PrepareVoteFor(node, proposal)
              BY <6>4 DEF PrepareRequestFor, PrepareWal
            <6>6. pending.node \in Honest
              BY <3>3, <6>4
            <6>7. pending.vote.phase = "Prepare"
              BY <3>3, <6>4
            <6>8. pending.vote.signer = pending.node
              BY <3>3, <6>4
            <6>9. pending.vote.context = context'
              BY <3>3, <3>4, <6>4
            <6>10. pending.vote.view = nodeView[node]
              BY <3>3, <6>4
            <6>11. nodeView'[pending.node] = nodeView[node]
              BY <3>4, <6>5
            <6>12. pending.vote.view = nodeView'[pending.node]
              BY <6>10, <6>11
            <6>13. pending.vote.subject \in ValidSubjects
              BY <3>3, <6>4
            <6>14. BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
              BY <3>3, <3>4, <6>5
                 DEF PrepareVoteFor, Vote
            <6>15. CanAppendVote(prepareIntents', pending.vote)
              BY <3>3, <3>4, <6>5
                 DEF PrepareRequestFor, PrepareWal
            <6>16. PrepareCarriesHigherSafeQc(pending.vote)'
              <7>1. ASSUME NEW commitVote \in commitIntents',
                            /\ pending.vote.signer \in Honest
                            /\ commitVote.signer = pending.vote.signer
                            /\ commitVote.context = pending.vote.context
                            /\ commitVote.phase = "Commit"
                            /\ commitVote.view < pending.vote.view
                            /\ commitVote.subject # pending.vote.subject
                     PROVE \E qc \in prepareQCs':
                             /\ qc.context = pending.vote.context
                             /\ qc.phase = "Prepare"
                             /\ commitVote.view < qc.view
                             /\ qc.view < pending.vote.view
                             /\ qc.subject = pending.vote.subject
                <8>1. commitVote \in commitIntents
                  BY <3>4, <7>1
                <8>2. /\ PrepareRequestFor(node, proposal).vote.signer
                              \in Honest
                      /\ commitVote.signer =
                           PrepareRequestFor(node, proposal).vote.signer
                      /\ commitVote.context =
                           PrepareRequestFor(node, proposal).vote.context
                      /\ commitVote.phase = "Commit"
                      /\ commitVote.view <
                           PrepareRequestFor(node, proposal).vote.view
                      /\ commitVote.subject #
                           PrepareRequestFor(node, proposal).vote.subject
                  BY <6>4, <7>1
                <8>3. \E qc \in prepareQCs:
                         /\ qc.context =
                              PrepareRequestFor(node, proposal).vote.context
                         /\ qc.phase = "Prepare"
                         /\ commitVote.view < qc.view
                         /\ qc.view <
                              PrepareRequestFor(node, proposal).vote.view
                         /\ qc.subject =
                              PrepareRequestFor(node, proposal).vote.subject
                  BY <6>1, <8>1, <8>2 DEF PrepareCarriesHigherSafeQc
                <8>4. PICK qc \in prepareQCs:
                         /\ qc.context =
                              PrepareRequestFor(node, proposal).vote.context
                         /\ qc.phase = "Prepare"
                         /\ commitVote.view < qc.view
                         /\ qc.view <
                              PrepareRequestFor(node, proposal).vote.view
                         /\ qc.subject =
                              PrepareRequestFor(node, proposal).vote.subject
                  BY <8>3
                <8>5. qc \in prepareQCs'
                  BY <3>4, <8>4
                <8>6. /\ qc.context = pending.vote.context
                      /\ qc.phase = "Prepare"
                      /\ commitVote.view < qc.view
                      /\ qc.view < pending.vote.view
                      /\ qc.subject = pending.vote.subject
                  BY <6>4, <8>4
                <8> QED BY <8>5, <8>6
              <7> QED BY <7>1 DEF PrepareCarriesHigherSafeQc
            <6> QED BY <6>6, <6>7, <6>8, <6>9, <6>12,
                         <6>13, <6>14, <6>15, <6>16
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>6. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote =
                    Vote(context', pending.qc.view, "Commit",
                         pending.qc.subject, pending.node)
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ \/ CurrentOpenPrepareForCommit(
                        pending.node, pending.qc)'
                  \/ HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        BY <3>1, <3>4, IsaM("blast")
           DEF PendingVoteWritesAuthorized,
               CurrentOpenPrepareForCommit,
               HistoricalLockedPrepareForCommit,
               InstalledTcSelectsPrepareFor,
               NoHigherPrepareOriginKnown, NodeTimedOut
      <3>7. \A pending \in pendingTimeout':
               /\ pending.node \in Honest
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ CanAppendTimeout(timeoutIntents', pending.vote)
               /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                               commitIntents)'
        <4>1. \A pending \in pendingTimeout:
                 /\ pending.node \in Honest
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context
                 /\ pending.vote.view = nodeView[pending.node]
                 /\ CanAppendTimeout(timeoutIntents, pending.vote)
                 /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                 commitIntents)
          BY <3>1 DEF PendingVoteWritesAuthorized
        <4>2. /\ pendingTimeout' = pendingTimeout
              /\ timeoutIntents' = timeoutIntents
              /\ commitIntents' = commitIntents
              /\ installedTCs' = installedTCs
              /\ context' = context
              /\ nodeView' = nodeView
          BY <1>1 DEF BeginPrepare
        <4> QED BY <4>1, <4>2
           DEF CanAppendTimeout, TimeoutVoteProtectsCommitSet,
               InstalledTcAuthorizesCommitVote
      <3> QED BY <3>5, <3>6, <3>7
         DEF PendingVoteWritesAuthorized
      <2>7. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
          /\ DurableLockRecoveryProvenanceInvariant'
      <3>1. DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginPrepare, DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingCertificateWritesAuthorized'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
            /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginPrepare, HonestVoteUnique, HonestTimeoutUnique,
               IntentPhasesCorrect,
               PendingCertificateWritesAuthorized, TCValid,
               AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters,
               HonestVoteTransportBacked, QcTransportBacked,
               HonestTimeoutTransportBacked, TcTransportBacked,
               CertificatesBackedByIntents, HonestDurableIntentsSound,
               FormedTimeoutCertificatesSound,
               HighestAndLockAreCertified, VoteIntentFor
      <3>3. DurableLockRecoveryProvenanceInvariant'
        BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginPrepare
      <3> QED BY <3>1, <3>2, <3>3
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginPrepare, LineageVars
  <1> QED BY <1>1

THEOREM HonestIntentSoundAppend ==
  \A intents, vote, durable, validSubjects:
    HonestIntentSound(intents, durable, validSubjects)
      /\ (vote.signer \in Honest
            => /\ vote.subject \in validSubjects
               /\ BodyHeldBy(durable, vote.signer,
                             vote.context, vote.view, vote.subject))
      => HonestIntentSound(intents \cup {vote}, durable, validSubjects)
BY SMT DEF HonestIntentSound

THEOREM DistinctSignerAppendPreservesCanAppendVote ==
  \A votes, appended, candidate:
    CanAppendVote(votes, candidate)
      /\ appended.signer # candidate.signer
      => CanAppendVote(votes \cup {appended}, candidate)
BY SMT DEF CanAppendVote, SameVoteSlot

THEOREM DistinctSignerAppendPreservesCanAppendTimeout ==
  \A votes, appended, candidate:
    CanAppendTimeout(votes, candidate)
      /\ appended.signer # candidate.signer
      => CanAppendTimeout(votes \cup {appended}, candidate)
BY SMT DEF CanAppendTimeout, SameTimeoutSlot

THEOREM CertificateBackingIsMonotone ==
  \A epoch, qc, before, after:
    CertificateBackedBy(epoch, qc, before)
      /\ before \subseteq after
      => CertificateBackedBy(epoch, qc, after)
BY DEF CertificateBackedBy

THEOREM PhasedVoteUniquenessImpliesSlotUniqueness ==
  \A intents, phase:
    HonestVoteUnique(intents)
      /\ (\A vote \in intents: vote.phase = phase)
      => \A left, right \in intents:
           (left.signer \in Honest
             /\ right.signer = left.signer
             /\ right.context = left.context
             /\ right.view = left.view)
           => right.subject = left.subject
BY SMT DEF HonestVoteUnique, SameVoteSlot

THEOREM PersistPreparePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistPrepare(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistPrepare(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.vote \in VoteRecordSet
          /\ request.vote.phase = "Prepare"
          /\ request.vote.signer = request.node
          /\ request.node \in Honest
          /\ request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, request.node,
                        request.vote.context, request.vote.view, request.vote.subject)
          /\ CanAppendVote(prepareIntents, request.vote)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistPrepare, PrepareWalSet
    <2>2. TypeInvariant'
      <3>1. /\ prepareIntents' \subseteq VoteRecordSet
            /\ pendingPrepare' \subseteq PrepareWalSet
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistPrepare
      <3>2. VoteSign(request.node, request.vote) \in VoteSignSet
        BY <2>1 DEF VoteSign, VoteSignSet
      <3>3. signVotes' \subseteq VoteSignSet
        BY <1>1, <3>2, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistPrepare
      <3> QED BY <1>1, <3>1, <3>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistPrepare
    <2>3. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistPrepare, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>4. HonestVoteUnique(prepareIntents)'
      BY <1>1, <2>1, DurableVoteAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare
    <2>5. IntentPhasesCorrect'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             IntentPhasesCorrect, PersistPrepare
    <2>6. HonestDurableIntentsSound'
      BY <1>1, <2>1, HonestIntentSoundAppend
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, PersistPrepare
    <2>7. PrepareSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             PrepareSigningRequiresIntent, VoteSign
    <2>8. HonestPrepareUniqueness'
      BY <2>4, <2>5, PhasedVoteUniquenessImpliesSlotUniqueness
         DEF HonestPrepareUniqueness, IntentPhasesCorrect
    <2>9. CommitSigningRequiresIntent'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             CommitSigningRequiresIntent, VoteSign
    <2>10. /\ ProposalSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             ProposalSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>11. Safety'
      BY <2>2, <2>3, <2>7, <2>8, <2>9, <2>10 DEF Safety
    <2>12. /\ ContextIdentityBindsFrozenEpoch'
           /\ OldContextCertificateRejected'
           /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistPrepare,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>13. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. ASSUME NEW pending \in pendingPrepare'
             PROVE /\ pending.node \in Honest
                   /\ pending.vote.phase = "Prepare"
                   /\ pending.vote.signer = pending.node
                   /\ pending.vote.context = context'
                   /\ pending.vote.view = nodeView'[pending.node]
                   /\ pending.vote.subject \in ValidSubjects
                   /\ BodyHeldBy(durableBodies', pending.node,
                                 pending.vote.context, pending.vote.view, pending.vote.subject)
                   /\ CanAppendVote(prepareIntents', pending.vote)
                   /\ PrepareCarriesHigherSafeQc(pending.vote)'
        <4>1. /\ pending \in pendingPrepare
              /\ pending # request
          BY <1>1, <3>2 DEF PersistPrepare
        <4>2. /\ pending \in AllPendingRequests
              /\ request \in AllPendingRequests
              /\ RequestsUniqueByNode(AllPendingRequests)
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode, AllPendingRequests,
                 PersistPrepare
        <4>3. pending.node # request.node
          BY <4>1, <4>2 DEF RequestsUniqueByNode
        <4>4. /\ CanAppendVote(prepareIntents, pending.vote)
              /\ request.vote.signer # pending.vote.signer
          BY <2>1, <3>1, <4>1, <4>3
             DEF PendingVoteWritesAuthorized
        <4>5. CanAppendVote(prepareIntents \cup {request.vote},
                            pending.vote)
          BY <4>4, DistinctSignerAppendPreservesCanAppendVote
        <4> QED BY <1>1, <3>1, <4>1, <4>5, Isa
           DEF PendingVoteWritesAuthorized, PersistPrepare,
               PrepareCarriesHigherSafeQc
      <3>3. \A pending \in pendingLockCommit':
                     /\ pending.node \in Honest
                     /\ pending.vote =
                          Vote(context', pending.qc.view, "Commit",
                               pending.qc.subject, pending.node)
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              pending.node, pending.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
        <4>1. ASSUME NEW pending \in pendingLockCommit'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote =
                          Vote(context', pending.qc.view, "Commit",
                               pending.qc.subject, pending.node)
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              pending.node, pending.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view,
                                   pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. /\ pending \in pendingLockCommit
                /\ request \in pendingPrepare
                /\ pending \in AllPendingRequests
                /\ request \in AllPendingRequests
                /\ RequestsUniqueByNode(AllPendingRequests)
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety,
                   OnePendingPersistencePerNode, AllPendingRequests,
                   PersistPrepare
          <5>2. pending # request
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   PersistPrepare, LockCommitWalSet, PrepareWalSet
          <5>3. pending.node # request.node
            BY <5>1, <5>2, DistinctUniqueRequestsHaveDistinctNodes
          <5>4. /\ pending.node \in Honest
                /\ pending.vote =
                     Vote(context, pending.qc.view, "Commit",
                          pending.qc.subject, pending.node)
                /\ pending.vote.phase = "Commit"
                /\ pending.vote.signer = pending.node
                /\ pending.vote.context = context
                /\ pending.vote.context = pending.qc.context
                /\ pending.vote.view = pending.qc.view
                /\ pending.vote.subject = pending.qc.subject
                /\ pending.qc.phase = "Prepare"
                /\ pending.qc \in prepareQCs
                /\ \/ CurrentOpenPrepareForCommit(
                         pending.node, pending.qc)
                   \/ HistoricalLockedPrepareForCommit(
                         pending.node, pending.qc)
                /\ pending.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies, pending.node,
                              pending.vote.context, pending.vote.view,
                              pending.vote.subject)
                /\ pending.qc.view >= lockRank[pending.node]
                /\ (pending.qc.view = lockRank[pending.node]
                      => pending.qc.subject = lockSubject[pending.node])
                /\ CanAppendVote(commitIntents, pending.vote)
            BY <3>1, <5>1 DEF PendingVoteWritesAuthorized
          <5>5. /\ context' = context
                /\ nodeView' = nodeView
                /\ durableBodies' = durableBodies
                /\ receivedQCs' = receivedQCs
                /\ prepareQCs' = prepareQCs
                /\ timeoutIntents' = timeoutIntents
                /\ installedTCs' = installedTCs
                /\ lockRank' = lockRank
                /\ lockSubject' = lockSubject
                /\ highestRank' = highestRank
                /\ highestSubject' = highestSubject
                /\ commitIntents' = commitIntents
                /\ prepareIntents' = prepareIntents \cup {request.vote}
            BY <1>1 DEF PersistPrepare
          <5>6. CurrentOpenPrepareForCommit(pending.node, pending.qc)
                   => CurrentOpenPrepareForCommit(pending.node, pending.qc)'
            BY <5>5 DEF CurrentOpenPrepareForCommit, NodeTimedOut
          <5>7. HistoricalLockedPrepareForCommit(
                   pending.node, pending.qc)
                   => HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
            <6>1. request.vote.signer = request.node
              BY <3>1, <5>1 DEF PendingVoteWritesAuthorized
            <6>2. NoHigherPrepareOriginKnown(
                     pending.node, pending.qc)
                     => NoHigherPrepareOriginKnown(
                          pending.node, pending.qc)'
              BY <5>3, <5>5, <6>1, Isa
                 DEF NoHigherPrepareOriginKnown
            <6> QED BY <5>5, <6>2, Isa
               DEF HistoricalLockedPrepareForCommit,
                   InstalledTcSelectsPrepareFor
          <5>8. \/ CurrentOpenPrepareForCommit(
                       pending.node, pending.qc)'
                 \/ HistoricalLockedPrepareForCommit(
                       pending.node, pending.qc)'
            BY <5>4, <5>6, <5>7
          <5> QED BY <5>4, <5>5, <5>8, Isa
        <4> QED BY <4>1
      <3>4. \A pending \in pendingTimeout':
                     /\ pending.node \in Honest
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ CanAppendTimeout(timeoutIntents', pending.vote)
                     /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                     commitIntents)'
        BY <1>1, <3>1, Isa
           DEF PersistPrepare, PendingVoteWritesAuthorized,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3> QED BY <3>2, <3>3, <3>4
         DEF PendingVoteWritesAuthorized
    <2>14. /\ HonestVoteUnique(commitIntents)'
           /\ HonestTimeoutUnique(timeoutIntents)'
           /\ PendingCertificateWritesAuthorized'
           /\ QcTransportBacked'
           /\ HonestTimeoutTransportBacked'
           /\ TcTransportBacked'
           /\ FormedTimeoutCertificatesSound'
           /\ DurableTimeoutsProtectCommits'
           /\ HighestAndLockAreCertified'
      <3>1. DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistPrepare, DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3>2. /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ PendingCertificateWritesAuthorized'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
            /\ FormedTimeoutCertificatesSound'
            /\ HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistPrepare, HonestVoteUnique, HonestTimeoutUnique,
               PendingCertificateWritesAuthorized, TCValid,
               AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters, QcTransportBacked,
               HonestTimeoutTransportBacked, TcTransportBacked,
               FormedTimeoutCertificatesSound,
               HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2
    <2>15. HonestVoteTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare, HonestVoteTransportBacked, VoteIntentFor,
             IntentPhasesCorrect
    <2>16. CertificatesBackedByIntents'
      <3>1. prepareIntents \subseteq prepareIntents'
        BY <1>1, Isa DEF PersistPrepare
      <3>2. \A qc \in prepareQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents')
        BY <1>1, <3>1, CertificateBackingIsMonotone
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, PersistPrepare
      <3>3. \A qc \in commitQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      commitIntents')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, PersistPrepare
      <3>4. /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
        BY <1>1 DEF PersistPrepare
      <3> QED BY <3>2, <3>3, <3>4
         DEF CertificatesBackedByIntents
    <2>16a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare
    <2>17. ReducerProvenanceInvariant'
      BY <2>4, <2>5, <2>6, <2>13, <2>14, <2>15, <2>16, <2>16a
         DEF ReducerProvenanceInvariant
    <2>18. LineageInvariant'
      <3>1. /\ TypeInvariant
            /\ LineageInvariant
            /\ PendingVoteWritesAuthorized
            /\ PersistPrepare(request)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant
      <3> QED BY <3>1, PersistPreparePreservesLineageInvariant
    <2> QED BY <2>11, <2>12, <2>17, <2>18
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM CompleteVoteSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteVoteSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteVoteSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteVoteSignature
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteVoteSignature,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent
    <2>3. /\ OnePendingPersistencePerNode'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteVoteSignature,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteVoteSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>6. HonestVoteTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CompleteVoteSignature, HonestVoteTransportBacked,
             VoteIntentFor, BroadcastVotes, VoteEnvelope, VoteAt,
             IntentPhasesCorrect
    <2>7. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport,
             CompleteVoteSignature, ProvenanceWithoutVoteTransportVars
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteVoteSignature, LineageVars
  <1> QED BY <1>1

THEOREM ByzantineBroadcastVotePreservesStrongInvariant ==
  \A signer, roundView, phase, subject:
    StrongInductiveInvariant
      /\ ByzantineBroadcastVote(signer, roundView, phase, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW signer,
              NEW roundView,
              NEW phase,
              NEW subject,
              StrongInductiveInvariant,
              ByzantineBroadcastVote(signer, roundView, phase, subject)
         PROVE StrongInductiveInvariant'
    <2>1. signer \notin Honest
      BY <1>1 DEF ByzantineBroadcastVote, Byzantine
    <2>2. HonestVoteTransportBacked'
      <3>1. \A envelope \in voteNetwork:
               envelope.vote.signer \in Honest
                 => VoteIntentFor(envelope.vote)
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked
      <3>2. \A envelope \in
                    BroadcastVotes(
                      Vote(context, roundView, phase, subject, signer)):
               envelope.vote.signer \notin Honest
        BY <2>1 DEF BroadcastVotes, VoteEnvelope, Vote
      <3>3. \A envelope \in voteNetwork':
               envelope.vote.signer \in Honest
                 => VoteIntentFor(envelope.vote)'
        <4>1. ASSUME NEW envelope \in voteNetwork',
                      envelope.vote.signer \in Honest
               PROVE VoteIntentFor(envelope.vote)'
          <5>1. \/ envelope \in voteNetwork
                \/ envelope \in BroadcastVotes(
                     Vote(context, roundView, phase, subject, signer))
            BY <1>1, <4>1 DEF ByzantineBroadcastVote
          <5>2. CASE envelope \in voteNetwork
            BY <1>1, <3>1, <4>1, <5>2
               DEF ByzantineBroadcastVote, VoteIntentFor
          <5>3. CASE envelope \in BroadcastVotes(
                            Vote(context, roundView, phase, subject, signer))
            BY <3>2, <4>1, <5>3
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>4. \A received \in receivedVotes':
               received.vote.signer \in Honest
                 => VoteIntentFor(received.vote)'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               ByzantineBroadcastVote, HonestVoteTransportBacked,
               VoteIntentFor
      <3> QED BY <3>3, <3>4 DEF HonestVoteTransportBacked
    <2>3. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, ByzantineBroadcastVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>4. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport,
             ByzantineBroadcastVote, ProvenanceWithoutVoteTransportVars
    <2>5. ReducerProvenanceInvariant'
      BY <2>2, <2>4
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>3, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ByzantineBroadcastVote, LineageVars
  <1> QED BY <1>1

THEOREM DeliverVotePreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverVote(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverVote(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. HonestVoteTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverVote, HonestVoteTransportBacked,
             VoteIntentFor, VoteAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, DeliverVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>3. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport, DeliverVote,
             ProvenanceWithoutVoteTransportVars
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>2, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverVote, LineageVars
  <1> QED BY <1>1

THEOREM ResumeVotePreservesStrongInvariant ==
  \A node, vote:
    StrongInductiveInvariant /\ ResumeVote(node, vote)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW vote,
              StrongInductiveInvariant,
              ResumeVote(node, vote)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeVote, VoteResumeAuthorized, VoteSign, VoteSignSet
    <2>2. /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeVote,
             VoteResumeAuthorized,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             VoteSign, IntentPhasesCorrect, ReducerProvenanceInvariant
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeVote,
             VoteResumeAuthorized,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ResumeVote,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>5. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ResumeVote, ProvenanceVars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeVote, LineageVars
  <1> QED BY <1>1

THEOREM CurrentQcValidityIsHistorical ==
  \A qc:
    TypeInvariant /\ QcValid(qc) => HistoricalQcValid(qc)
PROOF
  <1>1. ASSUME NEW qc,
              TypeInvariant,
              QcValid(qc)
         PROVE HistoricalQcValid(qc)
    <2>1. /\ qc.context \in ContextRecords
          /\ qc.height = qc.context.height
          /\ qc.view \in Views
          /\ qc.phase \in Phases
          /\ qc.subject \in ValidSubjects
          /\ ExactCertificateQuorum(qc.context.epoch, qc.signers)
      BY <1>1 DEF TypeInvariant, QcValid, QcWireValid, CurrentEpoch
    <2>2. qc.context.epoch \in Epochs
      BY <1>1
         DEF QcValid, QcWireValid, CurrentEpoch,
             ExactCertificateQuorum, DualQuorum, CountQuorum
    <2> QED BY <2>1, <2>2 DEF HistoricalQcValid
  <1> QED BY <1>1

THEOREM CurrentQcBackingIsCertificateBacking ==
  \A qc, intents:
    QcWireValid(qc) /\ CertificateHonestIntentBacked(qc, intents)
      => CertificateBackedBy(CurrentEpoch, qc, intents)
BY DEF QcWireValid, ExactCertificateQuorum,
       CertificateHonestIntentBacked, CertificateBackedBy

THEOREM WireValidBackedCertificateIsSemanticallyValid ==
  \A qc, intents, durable:
    (/\ TypeInvariant
     /\ QcWireValid(qc)
     /\ CertificateBackedBy(CurrentEpoch, qc, intents)
     /\ HonestIntentSound(intents, durable, ValidSubjects))
      => QcValid(qc)
PROOF
  <1>1. ASSUME NEW qc, NEW intents, NEW durable,
              TypeInvariant,
              QcWireValid(qc),
              CertificateBackedBy(CurrentEpoch, qc, intents),
              HonestIntentSound(intents, durable, ValidSubjects)
         PROVE QcValid(qc)
    <2>1. /\ QuorumConfiguration
          /\ CurrentEpoch \in Epochs
      BY <1>1
         DEF TypeInvariant, ModelConfiguration, QcWireValid,
             CurrentEpoch, ExactCertificateQuorum,
             DualQuorum, CountQuorum
    <2>2. CertificateValidityAndAvailability(
             qc, durable, ValidSubjects)
      BY <1>1, <2>1, BackedCertificateIsValidAndAvailable
    <2>3. qc.subject \in ValidSubjects
      BY <2>2 DEF CertificateValidityAndAvailability
    <2> QED BY <1>1, <2>3 DEF QcValid
  <1> QED BY <1>1

THEOREM FetchCertifiedBodySourceAvailabilityIsDerived ==
  \A node, qc:
    StrongInductiveInvariant /\ FetchCertifiedBody(node, qc)
      => CertifiedBodyAvailable(CurrentEpoch, qc.signers, durableBodies,
                                context, qc.view, qc.subject)
PROOF
  <1>1. ASSUME NEW node, NEW qc,
              StrongInductiveInvariant,
              FetchCertifiedBody(node, qc)
         PROVE CertifiedBodyAvailable(
                 CurrentEpoch, qc.signers, durableBodies,
                 context, qc.view, qc.subject)
    <2>1. CertifiedBodyRecoveryAuthority(node, qc)
      BY <1>1 DEF FetchCertifiedBody
    <2>2. CASE DecisionCertifiedBodyRecoveryAuthority(node, qc)
      <3>1. /\ qc \in commitQCs
            /\ qc.context = context
        BY <1>1, <2>2
           DEF StrongInductiveInvariant, Safety, DecisionAgreement,
               DecisionCertifiedBodyRecoveryAuthority
      <3>2. /\ HistoricalQcValid(qc)
            /\ CertificateBackedBy(CurrentEpoch, qc, commitIntents)
            /\ HonestIntentSound(commitIntents, durableBodies,
                                 ValidSubjects)
            /\ QuorumConfiguration
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HonestDurableIntentsSound, CurrentEpoch, ModelConfiguration
      <3>3. CurrentEpoch \in Epochs
        BY <3>1, <3>2 DEF HistoricalQcValid, CurrentEpoch
      <3>4. CertificateValidityAndAvailability(
               qc, durableBodies, ValidSubjects)
        BY <3>2, <3>3, BackedCertificateIsValidAndAvailable
      <3> QED BY <3>1, <3>4
         DEF CertificateValidityAndAvailability, CertifiedBodyAvailable
    <2>3. CASE HistoricalLockedPrepareSource(node, qc)
      <3>1. /\ qc \in prepareQCs
            /\ qc.context = context
        BY <2>3 DEF HistoricalLockedPrepareSource
      <3>2. /\ HistoricalQcValid(qc)
            /\ CertificateBackedBy(CurrentEpoch, qc, prepareIntents)
            /\ HonestIntentSound(prepareIntents, durableBodies,
                                 ValidSubjects)
            /\ QuorumConfiguration
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HonestDurableIntentsSound, CurrentEpoch, ModelConfiguration
      <3>3. CurrentEpoch \in Epochs
        BY <3>1, <3>2 DEF HistoricalQcValid, CurrentEpoch
      <3>4. CertificateValidityAndAvailability(
               qc, durableBodies, ValidSubjects)
        BY <3>2, <3>3, BackedCertificateIsValidAndAvailable
      <3> QED BY <3>1, <3>4
         DEF CertificateValidityAndAvailability, CertifiedBodyAvailable
    <2> QED BY <2>1, <2>2, <2>3 DEF CertifiedBodyRecoveryAuthority
  <1> QED BY <1>1

(***************************************************************************
The executable reducer forms a QC only from its authenticated local vote
pool.  HonestVoteTransportBacked connects every honest vote in that pool to
the signer's durable intent, so inspecting the global intent history is not
an executable guard.  These two lemmas recover the proof fact from the local
pool after the oracle-like guards have been removed from FormPrepareQC and
FormCommitQC.
***************************************************************************)

THEOREM PrepareVotePoolCertificateIsIntentBacked ==
  \A node, roundView, subject:
    HonestVoteTransportBacked
      => CertificateHonestIntentBacked(
           QC(context, roundView, "Prepare", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Prepare", subject)),
           prepareIntents)
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              HonestVoteTransportBacked
         PROVE CertificateHonestIntentBacked(
                 QC(context, roundView, "Prepare", subject,
                    ProjectedVoteSignersAt(
                      node, roundView, "Prepare", subject)),
                 prepareIntents)
    <2>1. ASSUME NEW signer \in
                    QC(context, roundView, "Prepare", subject,
                       ProjectedVoteSignersAt(
                         node, roundView, "Prepare", subject)).signers
                      \cap Honest
           PROVE \E vote \in prepareIntents:
                   VoteBacksCertificate(
                     vote,
                     QC(context, roundView, "Prepare", subject,
                        ProjectedVoteSignersAt(
                          node, roundView, "Prepare", subject)),
                     signer)
      <3>1. signer \in
               ProjectedVoteSignersAt(
                 node, roundView, "Prepare", subject)
        BY <2>1 DEF QC
      <3>2. PICK received \in receivedVotes:
               /\ received.node = node
               /\ received.vote.context = context
               /\ received.vote.view = roundView
               /\ received.vote.phase = "Prepare"
               /\ received.vote.subject = subject
               /\ received.vote.signer = signer
        BY <3>1 DEF ProjectedVoteSignersAt,
                       CanonicalCertificateSigners, VoteSignersAt
      <3>3. received.vote \in prepareIntents
        BY <1>1, <2>1, <3>2
           DEF HonestVoteTransportBacked, VoteIntentFor
      <3>4. VoteBacksCertificate(
               received.vote,
               QC(context, roundView, "Prepare", subject,
                  ProjectedVoteSignersAt(
                    node, roundView, "Prepare", subject)),
               signer)
        BY <3>2 DEF VoteBacksCertificate, QC
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1 DEF CertificateHonestIntentBacked
  <1> QED BY <1>1

THEOREM CommitVotePoolCertificateIsIntentBacked ==
  \A node, roundView, subject:
    HonestVoteTransportBacked
      => CertificateHonestIntentBacked(
           QC(context, roundView, "Commit", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Commit", subject)),
           commitIntents)
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              HonestVoteTransportBacked
         PROVE CertificateHonestIntentBacked(
                 QC(context, roundView, "Commit", subject,
                    ProjectedVoteSignersAt(
                      node, roundView, "Commit", subject)),
                 commitIntents)
    <2>1. ASSUME NEW signer \in
                    QC(context, roundView, "Commit", subject,
                       ProjectedVoteSignersAt(
                         node, roundView, "Commit", subject)).signers
                      \cap Honest
           PROVE \E vote \in commitIntents:
                   VoteBacksCertificate(
                     vote,
                     QC(context, roundView, "Commit", subject,
                        ProjectedVoteSignersAt(
                          node, roundView, "Commit", subject)),
                     signer)
      <3>1. signer \in ProjectedVoteSignersAt(
                          node, roundView, "Commit", subject)
        BY <2>1 DEF QC
      <3>2. PICK received \in receivedVotes:
               /\ received.node = node
               /\ received.vote.context = context
               /\ received.vote.view = roundView
               /\ received.vote.phase = "Commit"
               /\ received.vote.subject = subject
               /\ received.vote.signer = signer
        BY <3>1 DEF ProjectedVoteSignersAt,
                       CanonicalCertificateSigners, VoteSignersAt
      <3>3. received.vote \in commitIntents
        BY <1>1, <2>1, <3>2
           DEF HonestVoteTransportBacked, VoteIntentFor
      <3>4. VoteBacksCertificate(
               received.vote,
               QC(context, roundView, "Commit", subject,
                  ProjectedVoteSignersAt(
                    node, roundView, "Commit", subject)),
               signer)
        BY <3>2 DEF VoteBacksCertificate, QC
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1 DEF CertificateHonestIntentBacked
  <1> QED BY <1>1

THEOREM FormPrepareQCSemanticValidityIsDerived ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormPrepareQC(node, roundView, subject)
      => QcValid(
           QC(context, roundView, "Prepare", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Prepare", subject)))
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              StrongInductiveInvariant,
              FormPrepareQC(node, roundView, subject)
         PROVE QcValid(
                 QC(context, roundView, "Prepare", subject,
                    ProjectedVoteSignersAt(
                      node, roundView, "Prepare", subject)))
    <2> DEFINE Certificate ==
           QC(context, roundView, "Prepare", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Prepare", subject))
    <2>1. /\ TypeInvariant
          /\ QcWireValid(Certificate)
          /\ HonestVoteTransportBacked
          /\ HonestIntentSound(prepareIntents, durableBodies,
                               ValidSubjects)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, FormPrepareQC, Certificate
    <2>2. CertificateHonestIntentBacked(Certificate, prepareIntents)
      BY <2>1, PrepareVotePoolCertificateIsIntentBacked
         DEF Certificate
    <2>3. CertificateBackedBy(CurrentEpoch, Certificate, prepareIntents)
      BY <2>1, <2>2, CurrentQcBackingIsCertificateBacking
    <2> QED BY <2>1, <2>3,
                  WireValidBackedCertificateIsSemanticallyValid
  <1> QED BY <1>1

THEOREM FormCommitQCSemanticValidityIsDerived ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormCommitQC(node, roundView, subject)
      => QcValid(
           QC(context, roundView, "Commit", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Commit", subject)))
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              StrongInductiveInvariant,
              FormCommitQC(node, roundView, subject)
         PROVE QcValid(
                 QC(context, roundView, "Commit", subject,
                    ProjectedVoteSignersAt(
                      node, roundView, "Commit", subject)))
    <2> DEFINE Certificate ==
           QC(context, roundView, "Commit", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Commit", subject))
    <2>1. /\ TypeInvariant
          /\ QcWireValid(Certificate)
          /\ HonestVoteTransportBacked
          /\ HonestIntentSound(commitIntents, durableBodies,
                               ValidSubjects)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, FormCommitQC, Certificate
    <2>2. CertificateHonestIntentBacked(Certificate, commitIntents)
      BY <2>1, CommitVotePoolCertificateIsIntentBacked
         DEF Certificate
    <2>3. CertificateBackedBy(CurrentEpoch, Certificate, commitIntents)
      BY <2>1, <2>2, CurrentQcBackingIsCertificateBacking
    <2> QED BY <2>1, <2>3,
                  WireValidBackedCertificateIsSemanticallyValid
  <1> QED BY <1>1

THEOREM DeliverQCSemanticValidityIsDerived ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverQC(envelope)
      => QcValid(envelope.qc)
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverQC(envelope)
         PROVE QcValid(envelope.qc)
    <2>1. /\ QcWireValid(envelope.qc)
          /\ envelope.qc \in prepareQCs \cup commitQCs
      BY <1>1
         DEF DeliverQC, StrongInductiveInvariant,
             ReducerProvenanceInvariant, QcTransportBacked
    <2>2. /\ (\A qc \in prepareQCs: HistoricalQcValid(qc))
          /\ (\A qc \in commitQCs: HistoricalQcValid(qc))
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CertificatesBackedByIntents
    <2>3. envelope.qc.subject \in ValidSubjects
      BY <2>1, <2>2 DEF HistoricalQcValid
    <2> QED BY <2>1, <2>3 DEF QcValid
  <1> QED BY <1>1

THEOREM FormPrepareQCPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormPrepareQC(node, roundView, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW roundView,
              NEW subject,
              StrongInductiveInvariant,
              FormPrepareQC(node, roundView, subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE NewQc ==
           QC(context, roundView, "Prepare", subject,
              ProjectedVoteSignersAt(
                node, roundView, "Prepare", subject))
    <2>1. /\ QcValid(NewQc)
          /\ NewQc \in QcRecordSet
          /\ CertificateHonestIntentBacked(NewQc, prepareIntents)
      <3>1. /\ QcWireValid(NewQc)
            /\ NewQc \in QcRecordSet
        BY <1>1 DEF FormPrepareQC, NewQc
      <3>2. HonestVoteTransportBacked
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>3. CertificateHonestIntentBacked(NewQc, prepareIntents)
        BY <3>2, PrepareVotePoolCertificateIsIntentBacked DEF NewQc
      <3>4. /\ TypeInvariant
            /\ CertificateBackedBy(CurrentEpoch, NewQc, prepareIntents)
            /\ HonestIntentSound(prepareIntents, durableBodies,
                                 ValidSubjects)
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. CertificateBackedBy(
                 CurrentEpoch, NewQc, prepareIntents)
          BY <3>1, <3>3, CurrentQcBackingIsCertificateBacking
        <4>3. HonestIntentSound(
                 prepareIntents, durableBodies, ValidSubjects)
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestDurableIntentsSound
        <4> QED BY <4>1, <4>2, <4>3
      <3>5. QcValid(NewQc)
        BY <3>1, <3>4,
           WireValidBackedCertificateIsSemanticallyValid
      <3> QED BY <3>1, <3>3, <3>5
    <2>2. /\ HistoricalQcValid(NewQc)
          /\ CertificateBackedBy(CurrentEpoch, NewQc, prepareIntents)
      <3>1. TypeInvariant
        BY <1>1 DEF StrongInductiveInvariant, Safety
      <3>2. QcWireValid(NewQc)
        BY <2>1 DEF QcValid
      <3>3. HistoricalQcValid(NewQc)
        BY <2>1, <3>1, CurrentQcValidityIsHistorical
      <3>4. CertificateBackedBy(CurrentEpoch, NewQc, prepareIntents)
        BY <2>1, <3>2, CurrentQcBackingIsCertificateBacking
      <3> QED BY <3>3, <3>4
    <2>3. TypeInvariant'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             FormPrepareQC, NewQc
    <2>4. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, FormPrepareQC,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>5. Safety'
      BY <2>3, <2>4 DEF Safety
    <2>6. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, FormPrepareQC, NewQc,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>7. CertificatesBackedByIntents'
      <3>1. /\ prepareQCs' = prepareQCs \cup {NewQc}
            /\ prepareIntents' = prepareIntents
        BY <1>1 DEF FormPrepareQC, NewQc
      <3>2. \A qc \in prepareQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents)
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>3. \A qc \in prepareQCs':
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents')
        <4>1. ASSUME NEW qc \in prepareQCs'
               PROVE /\ HistoricalQcValid(qc)
                     /\ CertificateBackedBy(qc.context.epoch, qc,
                                            prepareIntents')
          <5>1. qc \in prepareQCs \/ qc = NewQc
            BY <3>1, <4>1
          <5>2. CASE qc \in prepareQCs
            BY <3>1, <3>2, <4>1, <5>2
          <5>3. CASE qc = NewQc
            BY <1>1, <2>2, <3>1, <5>3
               DEF CurrentEpoch, QC
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>4. \A qc \in commitQCs':
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      commitIntents')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, FormPrepareQC
      <3> QED BY <3>3, <3>4 DEF CertificatesBackedByIntents
    <2>8. QcTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, QcTransportBacked,
             BroadcastQCs, QcEnvelope
    <2>9. PendingCertificateWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, PendingCertificateWritesAuthorized,
             TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
    <2>10. HighestAndLockAreCertified'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, HighestAndLockAreCertified
    <2>11. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs \cup {NewQc}
            /\ durableBodies' = durableBodies
            /\ receivedVotes' = receivedVotes
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ voteNetwork' = voteNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF FormPrepareQC
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
      <3>3. /\ HonestVoteTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>4. /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
        <4>1. UNCHANGED <<prepareIntents, commitIntents,
                          durableBodies>>
          BY <3>1
        <4>2. HonestDurableIntentsSound'
          BY <1>1, <4>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestDurableIntentsSound
        <4>3. UNCHANGED <<formedTCs, timeoutIntents>>
          BY <3>1
        <4>4. FormedTimeoutCertificatesSound'
          BY <1>1, <4>3, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 FormedTimeoutCertificatesSound
        <4>5. UNCHANGED
                   <<timeoutIntents, commitIntents, installedTCs>>
          BY <3>1
        <4>6. DurableTimeoutsProtectCommits
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>7. DurableTimeoutsProtectCommits'
          BY <4>5, <4>6,
             UnchangedDurableTimeoutProtectionVarsPreserves
        <4> QED BY <4>2, <4>4, <4>7
      <3> QED BY <3>2, <3>3, <3>4
    <2>12. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. /\ prepareQCs' = prepareQCs \cup {NewQc}
            /\ UNCHANGED
                 <<context, nodeView, durableBodies, receivedQCs,
                   prepareIntents, commitIntents, timeoutIntents,
                   installedTCs, lockRank, lockSubject, highestRank,
                   highestSubject, pendingPrepare, pendingLockCommit,
                   pendingTimeout>>
        BY <1>1 DEF FormPrepareQC, NewQc
      <3>3. \A request \in pendingPrepare':
               /\ request.node \in Honest
               /\ request.vote.phase = "Prepare"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.view,
                             request.vote.subject)
               /\ CanAppendVote(prepareIntents', request.vote)
               /\ PrepareCarriesHigherSafeQc(request.vote)'
        BY <3>1, <3>2, Isa
           DEF PendingVoteWritesAuthorized,
               PrepareCarriesHigherSafeQc
      <3>4. \A request \in pendingLockCommit':
               /\ request.node \in Honest
               /\ request.vote =
                    Vote(context', request.qc.view, "Commit",
                         request.qc.subject, request.node)
               /\ request.vote.phase = "Commit"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.context = request.qc.context
               /\ request.vote.view = request.qc.view
               /\ request.vote.subject = request.qc.subject
               /\ request.qc.phase = "Prepare"
               /\ request.qc \in prepareQCs'
               /\ \/ CurrentOpenPrepareForCommit(
                        request.node, request.qc)'
                  \/ HistoricalLockedPrepareForCommit(
                        request.node, request.qc)'
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.view,
                             request.vote.subject)
               /\ request.qc.view >= lockRank'[request.node]
               /\ (request.qc.view = lockRank'[request.node]
                     => request.qc.subject = lockSubject'[request.node])
               /\ CanAppendVote(commitIntents', request.vote)
        <4>1. ASSUME NEW request \in pendingLockCommit'
               PROVE /\ request.node \in Honest
                     /\ request.vote =
                          Vote(context', request.qc.view, "Commit",
                               request.qc.subject, request.node)
                     /\ request.vote.phase = "Commit"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context'
                     /\ request.vote.context = request.qc.context
                     /\ request.vote.view = request.qc.view
                     /\ request.vote.subject = request.qc.subject
                     /\ request.qc.phase = "Prepare"
                     /\ request.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              request.node, request.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              request.node, request.qc)'
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', request.node,
                                   request.vote.context,
                                   request.vote.view,
                                   request.vote.subject)
                     /\ request.qc.view >= lockRank'[request.node]
                     /\ (request.qc.view = lockRank'[request.node]
                           => request.qc.subject =
                                lockSubject'[request.node])
                     /\ CanAppendVote(commitIntents', request.vote)
          <5>1. request \in pendingLockCommit
            BY <3>2, <4>1
          <5>2. /\ request.node \in Honest
                 /\ request.vote =
                      Vote(context, request.qc.view, "Commit",
                           request.qc.subject, request.node)
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs
                 /\ \/ CurrentOpenPrepareForCommit(
                          request.node, request.qc)
                    \/ HistoricalLockedPrepareForCommit(
                          request.node, request.qc)
                 /\ request.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies, request.node,
                               request.vote.context, request.vote.view,
                               request.vote.subject)
                 /\ request.qc.view >= lockRank[request.node]
                 /\ (request.qc.view = lockRank[request.node]
                       => request.qc.subject = lockSubject[request.node])
                 /\ CanAppendVote(commitIntents, request.vote)
            BY <3>1, <5>1 DEF PendingVoteWritesAuthorized
          <5>3. \/ CurrentOpenPrepareForCommit(
                    request.node, request.qc)'
                 \/ HistoricalLockedPrepareForCommit(
                    request.node, request.qc)'
            <6>1. CASE CurrentOpenPrepareForCommit(
                         request.node, request.qc)
              BY <3>2, <6>1, Isa
                 DEF CurrentOpenPrepareForCommit, NodeTimedOut
            <6>2. CASE HistoricalLockedPrepareForCommit(
                         request.node, request.qc)
              BY <3>2, <6>2, Isa
                 DEF HistoricalLockedPrepareForCommit,
                     InstalledTcSelectsPrepareFor,
                     NoHigherPrepareOriginKnown
            <6> QED BY <5>2, <6>1, <6>2
          <5> QED BY <3>2, <5>2, <5>3, Isa
        <4> QED BY <4>1
      <3>5. \A request \in pendingTimeout':
               /\ request.node \in Honest
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ CanAppendTimeout(timeoutIntents', request.vote)
               /\ TimeoutVoteProtectsCommitSet(
                    request.vote, commitIntents)'
        BY <3>1, <3>2, Isa
           DEF PendingVoteWritesAuthorized,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3> QED BY <3>3, <3>4, <3>5
         DEF PendingVoteWritesAuthorized
    <2>12a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC
    <2>13. ReducerProvenanceInvariant'
      BY <2>7, <2>8, <2>9, <2>10, <2>11, <2>12, <2>12a
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>5, <2>6, <2>13,
                  FormPrepareQCPreservesLineageInvariant
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverQCPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverQC(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverQC(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. QcTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverQC, QcTransportBacked, QcAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      <3>1. TypeInvariant'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, DeliverQC
      <3>2. OnePendingPersistencePerNode'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode, RequestsUniqueByNode,
               AllPendingRequests, DeliverQC
      <3>3. /\ ProposalSigningRequiresIntent'
            /\ PrepareSigningRequiresIntent'
            /\ CommitSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
            /\ HonestPrepareUniqueness'
            /\ HonestCommitUniqueness'
            /\ HonestTimeoutUniqueness'
            /\ LockBelowHighest'
            /\ DecisionAgreement'
            /\ AppliedRequiresDecision'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, DeliverQC,
               ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
               CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
               HonestPrepareUniqueness, HonestCommitUniqueness,
               HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
               AppliedRequiresDecision
      <3>4. Safety'
        BY <3>1, <3>2, <3>3 DEF Safety
      <3>5. /\ ContextIdentityBindsFrozenEpoch'
            /\ OldContextCertificateRejected'
            /\ ContextParentWasApplied'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, DeliverQC,
               ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
               ContextParentWasApplied, QcValid, QcWireValid, CurrentEpoch
      <3> QED BY <3>4, <3>5
    <2>3. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      <3>1. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs
            /\ pendingPrepare' = pendingPrepare
            /\ pendingLockCommit' = pendingLockCommit
            /\ pendingTimeout' = pendingTimeout
            /\ pendingObservePrepare' = pendingObservePrepare
            /\ pendingInstallTC' = pendingInstallTC
            /\ pendingDecision' = pendingDecision
            /\ context' = context
            /\ nodeView' = nodeView
            /\ durableBodies' = durableBodies
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ receivedQCs' =
                 receivedQCs
                   \cup {QcAt(envelope.recipient, envelope.qc)}
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
            /\ highestRank' = highestRank
            /\ highestSubject' = highestSubject
        BY <1>1 DEF DeliverQC
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
      <3>3. PendingVoteWritesAuthorized'
        <4>1. PendingVoteWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>2. \A request \in pendingPrepare':
                 /\ request.node \in Honest
                 /\ request.vote.phase = "Prepare"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context'
                 /\ request.vote.view = nodeView'[request.node]
                 /\ request.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies', request.node,
                               request.vote.context, request.vote.view,
                               request.vote.subject)
                 /\ CanAppendVote(prepareIntents', request.vote)
                 /\ PrepareCarriesHigherSafeQc(request.vote)'
          BY <3>1, <4>1, Isa
             DEF PendingVoteWritesAuthorized,
                 PrepareCarriesHigherSafeQc
        <4>3. \A request \in pendingLockCommit':
                 /\ request.node \in Honest
                 /\ request.vote =
                      Vote(context', request.qc.view, "Commit",
                           request.qc.subject, request.node)
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context'
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs'
                 /\ \/ CurrentOpenPrepareForCommit(
                          request.node, request.qc)'
                    \/ HistoricalLockedPrepareForCommit(
                          request.node, request.qc)'
                 /\ request.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies', request.node,
                               request.vote.context, request.vote.view,
                               request.vote.subject)
                 /\ request.qc.view >= lockRank'[request.node]
                 /\ (request.qc.view = lockRank'[request.node]
                       => request.qc.subject = lockSubject'[request.node])
                 /\ CanAppendVote(commitIntents', request.vote)
          <5>1. ASSUME NEW request \in pendingLockCommit'
                 PROVE /\ request.node \in Honest
                       /\ request.vote =
                            Vote(context', request.qc.view, "Commit",
                                 request.qc.subject, request.node)
                       /\ request.vote.phase = "Commit"
                       /\ request.vote.signer = request.node
                       /\ request.vote.context = context'
                       /\ request.vote.context = request.qc.context
                       /\ request.vote.view = request.qc.view
                       /\ request.vote.subject = request.qc.subject
                       /\ request.qc.phase = "Prepare"
                       /\ request.qc \in prepareQCs'
                       /\ \/ CurrentOpenPrepareForCommit(
                                request.node, request.qc)'
                          \/ HistoricalLockedPrepareForCommit(
                                request.node, request.qc)'
                       /\ request.vote.subject \in ValidSubjects
                       /\ BodyHeldBy(durableBodies', request.node,
                                     request.vote.context,
                                     request.vote.view,
                                     request.vote.subject)
                       /\ request.qc.view >= lockRank'[request.node]
                       /\ (request.qc.view = lockRank'[request.node]
                             => request.qc.subject =
                                  lockSubject'[request.node])
                       /\ CanAppendVote(commitIntents', request.vote)
            <6>1. request \in pendingLockCommit
              BY <3>1, <5>1
            <6>2. /\ request.node \in Honest
                   /\ request.vote =
                        Vote(context, request.qc.view, "Commit",
                             request.qc.subject, request.node)
                   /\ request.vote.phase = "Commit"
                   /\ request.vote.signer = request.node
                   /\ request.vote.context = context
                   /\ request.vote.context = request.qc.context
                   /\ request.vote.view = request.qc.view
                   /\ request.vote.subject = request.qc.subject
                   /\ request.qc.phase = "Prepare"
                   /\ request.qc \in prepareQCs
                   /\ \/ CurrentOpenPrepareForCommit(
                            request.node, request.qc)
                      \/ HistoricalLockedPrepareForCommit(
                            request.node, request.qc)
                   /\ request.vote.subject \in ValidSubjects
                   /\ BodyHeldBy(durableBodies, request.node,
                                 request.vote.context,
                                 request.vote.view,
                                 request.vote.subject)
                   /\ request.qc.view >= lockRank[request.node]
                   /\ (request.qc.view = lockRank[request.node]
                         => request.qc.subject = lockSubject[request.node])
                   /\ CanAppendVote(commitIntents, request.vote)
              BY <4>1, <6>1 DEF PendingVoteWritesAuthorized
            <6>3. \/ CurrentOpenPrepareForCommit(
                      request.node, request.qc)'
                   \/ HistoricalLockedPrepareForCommit(
                      request.node, request.qc)'
              <7>1. CASE CurrentOpenPrepareForCommit(
                           request.node, request.qc)
                BY <3>1, <7>1, Isa
                   DEF CurrentOpenPrepareForCommit, NodeTimedOut
              <7>2. CASE HistoricalLockedPrepareForCommit(
                           request.node, request.qc)
                BY <3>1, <7>2, Isa
                   DEF HistoricalLockedPrepareForCommit,
                       InstalledTcSelectsPrepareFor,
                       NoHigherPrepareOriginKnown
              <7> QED BY <6>2, <7>1, <7>2
            <6> QED BY <3>1, <6>2, <6>3, Isa
          <5> QED BY <5>1
        <4>4. \A request \in pendingTimeout':
                 /\ request.node \in Honest
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context'
                 /\ request.vote.view = nodeView'[request.node]
                 /\ CanAppendTimeout(timeoutIntents', request.vote)
                 /\ TimeoutVoteProtectsCommitSet(
                      request.vote, commitIntents)'
          BY <3>1, <4>1, Isa
             DEF PendingVoteWritesAuthorized,
                 TimeoutVoteProtectsCommitSet,
                 TimeoutVoteStrictlyProtectsCommit,
                 InstalledTcAuthorizesCommitVote
        <4> QED BY <4>2, <4>3, <4>4
           DEF PendingVoteWritesAuthorized
      <3>4. PendingCertificateWritesAuthorized'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>4. /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs
            /\ receivedVotes' = receivedVotes
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ installedTCs' = installedTCs
            /\ formedTCs' = formedTCs
            /\ voteNetwork' = voteNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF DeliverQC
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. HonestTimeoutTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>5. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. UNCHANGED
                 <<context, durableBodies, prepareIntents, commitIntents,
                   timeoutIntents, prepareQCs, commitQCs, formedTCs,
                   installedTCs, lockRank, lockSubject, highestRank,
                   highestSubject>>
        BY <1>1, Isa DEF DeliverQC
      <3>2. CertificatesBackedByIntents'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>3. HonestDurableIntentsSound'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestDurableIntentsSound
      <3>4. FormedTimeoutCertificatesSound'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound
      <3>5. DurableTimeoutsProtectCommits
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>6. DurableTimeoutsProtectCommits'
        BY <3>1, <3>5,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>7. HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>8. HighestAndLockAreCertified'
        BY <3>1, <3>7,
           UnchangedHighestAndLockCertificationVarsPreserves
      <3> QED BY <3>2, <3>3, <3>4, <3>6, <3>8
    <2>5a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverQC
    <2>6. ReducerProvenanceInvariant'
      BY <2>1, <2>3, <2>4, <2>5, <2>5a
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>2, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverQC, LineageVars
  <1> QED BY <1>1

THEOREM RestartPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ Restart(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              Restart(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      <3>1. /\ generation \in [ValidatorIds -> Generations]
            /\ node \in ValidatorIds
            /\ 0 \in Generations
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               Restart, ModelConfiguration, Generations
      <3>3. generation' \in [ValidatorIds -> Generations]
        BY <1>1, <3>1, Isa DEF Restart
      <3> QED BY <1>1, <3>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant, Restart
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Restart,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent
    <2>3. /\ OnePendingPersistencePerNode'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Restart,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance, Isa
         DEF StrongInductiveInvariant, Restart,
             ProvenanceVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <1>1, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, Restart, LineageVars
  <1> QED BY <1>1

THEOREM CrashPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ Crash(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              Crash(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      <3>1. up' \subseteq ValidatorIds
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash
      <3>2. ValidatedBodiesSound(validatedBodies', ValidSubjects)
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash,
               ValidatedBodiesSound
      <3>3. /\ pendingProposal' \subseteq ProposalWalSet
            /\ pendingPrepare' \subseteq PrepareWalSet
            /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
            /\ pendingLockCommit' \subseteq LockCommitWalSet
            /\ pendingTimeout' \subseteq TimeoutWalSet
            /\ pendingDecision' \subseteq DecisionWalSet
            /\ signProposals' \subseteq ProposalSignSet
            /\ signVotes' \subseteq VoteSignSet
            /\ signTimeouts' \subseteq TimeoutSignSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash
      <3>4. pendingInstallTC' \subseteq InstallTcWalSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash
      <3> QED BY <1>1, <3>1, <3>2, <3>3, <3>4, IsaT(60)
         DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash
    <2>2. OnePendingPersistencePerNode'
      <3>1. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF Crash, AllPendingRequests
      <3>2. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Crash,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Crash,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>6. ReducerProvenanceInvariant'
      <3>1. /\ pendingPrepare' \subseteq pendingPrepare
            /\ pendingLockCommit' \subseteq pendingLockCommit
            /\ pendingTimeout' \subseteq pendingTimeout
            /\ pendingObservePrepare' \subseteq pendingObservePrepare
            /\ pendingInstallTC' \subseteq pendingInstallTC
            /\ pendingDecision' \subseteq pendingDecision
        BY <1>1, Isa DEF Crash
      <3>2. /\ PendingVoteWritesAuthorized'
            /\ PendingCertificateWritesAuthorized'
        <4>1. /\ height' = height
              /\ context' = context
              /\ nodeView' = nodeView
              /\ durableBodies' = durableBodies
              /\ prepareIntents' = prepareIntents
              /\ commitIntents' = commitIntents
              /\ timeoutIntents' = timeoutIntents
              /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs
              /\ formedTCs' = formedTCs
              /\ installedTCs' = installedTCs
              /\ receivedQCs' = receivedQCs
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
              /\ highestRank' = highestRank
              /\ highestSubject' = highestSubject
          BY <1>1 DEF Crash
        <4>2. PendingVoteWritesAuthorized'
          <5>1. PendingVoteWritesAuthorized
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant
          <5>2. \A request \in pendingPrepare':
                   /\ request.node \in Honest
                   /\ request.vote.phase = "Prepare"
                   /\ request.vote.signer = request.node
                   /\ request.vote.context = context'
                   /\ request.vote.view = nodeView'[request.node]
                   /\ request.vote.subject \in ValidSubjects
                   /\ BodyHeldBy(durableBodies', request.node,
                                 request.vote.context, request.vote.view,
                                 request.vote.subject)
                   /\ CanAppendVote(prepareIntents', request.vote)
                   /\ PrepareCarriesHigherSafeQc(request.vote)'
            BY <3>1, <4>1, <5>1, Isa
               DEF PendingVoteWritesAuthorized,
                   PrepareCarriesHigherSafeQc
          <5>3. \A request \in pendingLockCommit':
                   /\ request.node \in Honest
                   /\ request.vote =
                        Vote(context', request.qc.view, "Commit",
                             request.qc.subject, request.node)
                   /\ request.vote.phase = "Commit"
                   /\ request.vote.signer = request.node
                   /\ request.vote.context = context'
                   /\ request.vote.context = request.qc.context
                   /\ request.vote.view = request.qc.view
                   /\ request.vote.subject = request.qc.subject
                   /\ request.qc.phase = "Prepare"
                   /\ request.qc \in prepareQCs'
                   /\ \/ CurrentOpenPrepareForCommit(
                            request.node, request.qc)'
                      \/ HistoricalLockedPrepareForCommit(
                            request.node, request.qc)'
                   /\ request.vote.subject \in ValidSubjects
                   /\ BodyHeldBy(durableBodies', request.node,
                                 request.vote.context, request.vote.view,
                                 request.vote.subject)
                   /\ request.qc.view >= lockRank'[request.node]
                   /\ (request.qc.view = lockRank'[request.node]
                         => request.qc.subject = lockSubject'[request.node])
                   /\ CanAppendVote(commitIntents', request.vote)
            <6>1. ASSUME NEW request \in pendingLockCommit'
                   PROVE /\ request.node \in Honest
                         /\ request.vote =
                              Vote(context', request.qc.view, "Commit",
                                   request.qc.subject, request.node)
                         /\ request.vote.phase = "Commit"
                         /\ request.vote.signer = request.node
                         /\ request.vote.context = context'
                         /\ request.vote.context = request.qc.context
                         /\ request.vote.view = request.qc.view
                         /\ request.vote.subject = request.qc.subject
                         /\ request.qc.phase = "Prepare"
                         /\ request.qc \in prepareQCs'
                         /\ \/ CurrentOpenPrepareForCommit(
                                  request.node, request.qc)'
                            \/ HistoricalLockedPrepareForCommit(
                                  request.node, request.qc)'
                         /\ request.vote.subject \in ValidSubjects
                         /\ BodyHeldBy(durableBodies', request.node,
                                       request.vote.context,
                                       request.vote.view,
                                       request.vote.subject)
                         /\ request.qc.view >= lockRank'[request.node]
                         /\ (request.qc.view = lockRank'[request.node]
                               => request.qc.subject =
                                    lockSubject'[request.node])
                         /\ CanAppendVote(commitIntents', request.vote)
              <7>1. request \in pendingLockCommit
                BY <3>1, <6>1
              <7>2. /\ request.node \in Honest
                     /\ request.vote =
                          Vote(context, request.qc.view, "Commit",
                               request.qc.subject, request.node)
                     /\ request.vote.phase = "Commit"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context
                     /\ request.vote.context = request.qc.context
                     /\ request.vote.view = request.qc.view
                     /\ request.vote.subject = request.qc.subject
                     /\ request.qc.phase = "Prepare"
                     /\ request.qc \in prepareQCs
                     /\ \/ CurrentOpenPrepareForCommit(
                              request.node, request.qc)
                        \/ HistoricalLockedPrepareForCommit(
                              request.node, request.qc)
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies, request.node,
                                   request.vote.context,
                                   request.vote.view,
                                   request.vote.subject)
                     /\ request.qc.view >= lockRank[request.node]
                     /\ (request.qc.view = lockRank[request.node]
                           => request.qc.subject = lockSubject[request.node])
                     /\ CanAppendVote(commitIntents, request.vote)
                BY <5>1, <7>1 DEF PendingVoteWritesAuthorized
              <7>3. \/ CurrentOpenPrepareForCommit(
                        request.node, request.qc)'
                     \/ HistoricalLockedPrepareForCommit(
                        request.node, request.qc)'
                <8>1. CASE CurrentOpenPrepareForCommit(
                             request.node, request.qc)
                  BY <4>1, <8>1, Isa
                     DEF CurrentOpenPrepareForCommit, NodeTimedOut
                <8>2. CASE HistoricalLockedPrepareForCommit(
                             request.node, request.qc)
                  BY <4>1, <8>2, Isa
                     DEF HistoricalLockedPrepareForCommit,
                         InstalledTcSelectsPrepareFor,
                         NoHigherPrepareOriginKnown
                <8> QED BY <7>2, <8>1, <8>2
              <7> QED BY <4>1, <7>2, <7>3, Isa
            <6> QED BY <6>1
          <5>4. \A request \in pendingTimeout':
                   /\ request.node \in Honest
                   /\ request.vote.signer = request.node
                   /\ request.vote.context = context'
                   /\ request.vote.view = nodeView'[request.node]
                   /\ CanAppendTimeout(timeoutIntents', request.vote)
                   /\ TimeoutVoteProtectsCommitSet(
                        request.vote, commitIntents)'
            BY <3>1, <4>1, <5>1, Isa
               DEF PendingVoteWritesAuthorized,
                   TimeoutVoteProtectsCommitSet,
                   TimeoutVoteStrictlyProtectsCommit,
                   InstalledTcAuthorizesCommitVote
          <5> QED BY <5>2, <5>3, <5>4
             DEF PendingVoteWritesAuthorized
        <4>3. PendingCertificateWritesAuthorized'
          BY <1>1, <3>1, <4>1, SMT
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PendingCertificateWritesAuthorized, TCValid, AuthenticatedHighRef, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4> QED BY <4>2, <4>3
      <3>3. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
            /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        <4>1. UNCHANGED
                   <<height, context, nodeView, durableBodies,
                     receivedVotes, receivedQCs, receivedTimeoutVotes,
                     receivedTCs, prepareIntents, commitIntents,
                     timeoutIntents, prepareQCs, commitQCs, formedTCs,
                     installedTCs, lockRank, lockSubject, highestRank,
                     highestSubject, voteNetwork, qcNetwork,
                     timeoutNetwork, tcNetwork>>
          BY <1>1, Isa DEF Crash
        <4>2. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, <4>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestVoteUnique, HonestTimeoutUnique,
                 IntentPhasesCorrect
        <4>3. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          BY <1>1, <4>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestVoteTransportBacked, QcTransportBacked,
                 HonestTimeoutTransportBacked, TcTransportBacked,
                 VoteIntentFor, TCValid, AuthenticatedHighRef,
                 HighRefValid, CurrentEpoch, CurrentVoters
        <4>4. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
          BY <1>1, <4>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents,
                 HonestDurableIntentsSound,
                 FormedTimeoutCertificatesSound
        <4>5. DurableTimeoutsProtectCommits
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>6. DurableTimeoutsProtectCommits'
          BY <4>1, <4>5,
             UnchangedDurableTimeoutProtectionVarsPreserves
        <4>7. HighestAndLockAreCertified
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>8. HighestAndLockAreCertified'
          BY <4>1, <4>7,
             UnchangedHighestAndLockCertificationVarsPreserves
        <4>9. DurableLockRecoveryProvenanceInvariant'
          BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 Crash
        <4> QED BY <4>2, <4>3, <4>4, <4>6, <4>8, <4>9
      <3> QED BY <3>2, <3>3 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>4, <2>5, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, Crash, LineageVars
  <1> QED BY <1>1

(***************************************************************************
Core safety admits a crash of every active node, including a responsive node
after GST.  Stable-uptime restrictions belong only to the conditional
liveness wrapper, so the safety induction is not narrowed by that premise.
***************************************************************************)

THEOREM UnrestrictedCoreCrashPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ Crash(node)
      => StrongInductiveInvariant'
BY CrashPreservesStrongInvariant

(***************************************************************************
Timeout creation reads only the local durable highest PrepareQC and lock.
The following lemmas derive the former global-history guards from those local
values plus the inductive provenance.  BeginTimeout therefore need not query
prepareQCs or every validator's durable Commit intents.
***************************************************************************)

THEOREM AuthenticatedHighRefGhostEquivalence ==
  \A highRank, highSubject:
    AuthenticatedHighRef(highRank, highSubject)
      <=> HighRefValid(highRank, highSubject)
BY DEF AuthenticatedHighRef

THEOREM DeliverTimeoutAuthenticationCarriesGhostCertificate ==
  \A envelope:
    DeliverTimeout(envelope)
      => HighRefValid(envelope.vote.highRank,
                      envelope.vote.highSubject)
BY AuthenticatedHighRefGhostEquivalence DEF DeliverTimeout

THEOREM LocalTimeoutHighRefIsValid ==
  \A node \in ValidatorIds:
    TypeInvariant /\ HighestAndLockAreCertified
      => HighRefValid(LocalTimeoutVoteFor(node).highRank,
                      LocalTimeoutVoteFor(node).highSubject)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              TypeInvariant,
              HighestAndLockAreCertified
         PROVE HighRefValid(LocalTimeoutVoteFor(node).highRank,
                            LocalTimeoutVoteFor(node).highSubject)
    <2>1. /\ LocalTimeoutVoteFor(node).highRank = highestRank[node]
          /\ LocalTimeoutVoteFor(node).highSubject = highestSubject[node]
          /\ highestRank[node] \in Ranks
          /\ highestSubject[node] \in SubjectOrNone
      BY <1>1 DEF TypeInvariant, LocalTimeoutVoteFor, TimeoutVote
    <2>2. CASE highestRank[node] = NoRank
      <3>1. highestSubject[node] = NoSubject
        BY <1>1, <2>2 DEF HighestAndLockAreCertified
      <3> QED BY <2>1, <2>2, <3>1 DEF HighRefValid
    <2>3. CASE highestRank[node] # NoRank
      <3>1. \E qc \in prepareQCs:
               /\ qc.context = context
               /\ qc.view = highestRank[node]
               /\ qc.subject = highestSubject[node]
        BY <1>1, <2>3 DEF HighestAndLockAreCertified
      <3>2. PICK qc \in prepareQCs:
               /\ qc.context = context
               /\ qc.view = highestRank[node]
               /\ qc.subject = highestSubject[node]
        BY <3>1
      <3>3. /\ highestRank[node] \in Views
            /\ highestSubject[node] \in Subjects
        BY <1>1, <2>1, <2>3, <3>2, SMT
           DEF TypeInvariant, QcRecordSet, Ranks, SubjectOrNone,
               ModelConfiguration
      <3> QED BY <2>1, <3>2, <3>3 DEF HighRefValid
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM LocalTimeoutVoteProtectsDurableCommits ==
  \A node \in ValidatorIds:
    StrongInductiveInvariant /\ node \in Honest
      => TimeoutVoteProtectsCommitSet(
           LocalTimeoutVoteFor(node), commitIntents)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              StrongInductiveInvariant,
              node \in Honest
         PROVE TimeoutVoteProtectsCommitSet(
                 LocalTimeoutVoteFor(node), commitIntents)
    <2>1. ASSUME NEW commitVote \in commitIntents,
                  /\ LocalTimeoutVoteFor(node).signer \in Honest
                  /\ commitVote.signer =
                       LocalTimeoutVoteFor(node).signer
                  /\ commitVote.context =
                       LocalTimeoutVoteFor(node).context
                  /\ commitVote.phase = "Commit"
                  /\ commitVote.view <= LocalTimeoutVoteFor(node).view
           PROVE /\ LocalTimeoutVoteFor(node).highRank >= commitVote.view
                 /\ (LocalTimeoutVoteFor(node).highRank = commitVote.view
                       => LocalTimeoutVoteFor(node).highSubject =
                            commitVote.subject)
      <3>1. /\ LocalTimeoutVoteFor(node).signer = node
            /\ LocalTimeoutVoteFor(node).context = context
            /\ LocalTimeoutVoteFor(node).highRank = highestRank[node]
            /\ LocalTimeoutVoteFor(node).highSubject = highestSubject[node]
        BY DEF LocalTimeoutVoteFor, TimeoutVote
      <3>2. /\ commitVote.signer = node
            /\ commitVote.context = context
        BY <2>1, <3>1
      <3>3. /\ lockRank[node] >= commitVote.view
            /\ (lockRank[node] = commitVote.view
                  => lockSubject[node] = commitVote.subject)
        BY <1>1, <2>1, <3>2
           DEF StrongInductiveInvariant, LineageInvariant,
               LocksCoverOwnCommits
      <3>4. lockRank[node] <= highestRank[node]
        BY <1>1
           DEF StrongInductiveInvariant, Safety, LockBelowHighest
      <3>5. /\ commitVote.view \in Views
            /\ lockRank[node] \in Ranks
            /\ highestRank[node] \in Ranks
            /\ commitVote.view \in Int
            /\ lockRank[node] \in Int
            /\ highestRank[node] \in Int
        <4>1. /\ commitVote.view \in Views
              /\ lockRank[node] \in Ranks
              /\ highestRank[node] \in Ranks
          BY <1>1, <2>1, Isa
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 VoteRecordSet
        <4>2. ViewDomain \subseteq Nat
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 ModelConfiguration
        <4>3. /\ Views \subseteq Int
              /\ Ranks \subseteq Int
          BY <4>2, SMT DEF Views, Ranks, NoRank
        <4> QED BY <4>1, <4>3, Isa
      <3>6. LocalTimeoutVoteFor(node).highRank >= commitVote.view
        BY <3>1, <3>3, <3>4, <3>5, SMT
      <3>7. ASSUME LocalTimeoutVoteFor(node).highRank = commitVote.view
             PROVE LocalTimeoutVoteFor(node).highSubject =
                     commitVote.subject
        <4>1. /\ highestRank[node] = commitVote.view
              /\ lockRank[node] = commitVote.view
              /\ lockSubject[node] = commitVote.subject
          BY <3>1, <3>3, <3>4, <3>5, <3>7, SMT
        <4>2. /\ highestRank[node] # NoRank
              /\ lockRank[node] # NoRank
          <5>1. ModelConfiguration
            BY <1>1 DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. commitVote.view # NoRank
            BY <3>5, <5>1, ViewIsNotNoRank
          <5> QED BY <4>1, <5>2
        <4>3. PICK highestQc \in prepareQCs:
                 /\ highestQc.context = context
                 /\ highestQc.view = highestRank[node]
                 /\ highestQc.subject = highestSubject[node]
          BY <1>1, <4>2
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HighestAndLockAreCertified
        <4>4. PICK lockQc \in prepareQCs:
                 /\ lockQc.context = context
                 /\ lockQc.view = lockRank[node]
                 /\ lockQc.subject = lockSubject[node]
          BY <1>1, <4>2
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HighestAndLockAreCertified
        <4>5. /\ highestQc.phase = "Prepare"
              /\ lockQc.phase = "Prepare"
              /\ HistoricalQcValid(highestQc)
              /\ HistoricalQcValid(lockQc)
              /\ CertificateBackedBy(highestQc.context.epoch,
                                     highestQc, prepareIntents)
              /\ CertificateBackedBy(lockQc.context.epoch,
                                     lockQc, prepareIntents)
              /\ HonestVoteUnique(prepareIntents)
              /\ QuorumConfiguration
          BY <1>1, <4>3, <4>4
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 LineageInvariant, CertificatePhasesCorrect,
                 CertificatesBackedByIntents, ModelConfiguration,
                 TypeInvariant, Safety
        <4>6. highestQc.context = lockQc.context
          BY <4>3, <4>4
        <4>7. highestQc.context.epoch = lockQc.context.epoch
          BY <4>6
        <4>8. highestQc.context.epoch \in Epochs
          BY <4>5 DEF HistoricalQcValid
        <4>9. SameCertificateSlot(highestQc, lockQc)
          BY <4>1, <4>3, <4>4, <4>5, <4>6
             DEF SameCertificateSlot
        <4>10. highestQc.subject = lockQc.subject
          BY <4>5, <4>7, <4>8, <4>9,
             SameViewCertificateUniqueness
        <4> QED BY <3>1, <4>1, <4>3, <4>4, <4>10
      <3> QED BY <3>6, <3>7
    <2> QED BY <2>1
       DEF TimeoutVoteProtectsCommitSet,
           TimeoutVoteStrictlyProtectsCommit
  <1> QED BY <1>1

THEOREM BeginTimeoutHistoryGuardsAreDerived ==
  \A node:
    StrongInductiveInvariant /\ BeginTimeout(node)
      => /\ HighRefValid(LocalTimeoutVoteFor(node).highRank,
                         LocalTimeoutVoteFor(node).highSubject)
         /\ TimeoutVoteProtectsCommitSet(
              LocalTimeoutVoteFor(node), commitIntents)
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              BeginTimeout(node)
         PROVE /\ HighRefValid(LocalTimeoutVoteFor(node).highRank,
                               LocalTimeoutVoteFor(node).highSubject)
               /\ TimeoutVoteProtectsCommitSet(
                    LocalTimeoutVoteFor(node), commitIntents)
    <2>1. /\ node \in ValidatorIds
          /\ node \in Honest
          /\ TypeInvariant
          /\ HighestAndLockAreCertified
      <3>1. node \in Honest
        BY <1>1 DEF BeginTimeout
      <3>2. /\ TypeInvariant
            /\ HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant
      <3>3. /\ TimeoutRequestFor(node) \in TimeoutWalSet
            /\ TimeoutRequestFor(node).node = node
        BY <1>1
           DEF BeginTimeout, TimeoutRequestFor, TimeoutWal
      <3>4. node \in ValidatorIds
        BY <3>3 DEF TimeoutWalSet
      <3> QED BY <3>1, <3>2, <3>4
    <2>2. HighRefValid(LocalTimeoutVoteFor(node).highRank,
                       LocalTimeoutVoteFor(node).highSubject)
      BY <2>1, LocalTimeoutHighRefIsValid
    <2>3. TimeoutVoteProtectsCommitSet(
             LocalTimeoutVoteFor(node), commitIntents)
      BY <1>1, <2>1, LocalTimeoutVoteProtectsDurableCommits
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM BeginTimeoutPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ BeginTimeout(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              BeginTimeout(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginTimeout
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginTimeout, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests \cup {TimeoutRequestFor(node)}
        BY <1>1, Isa DEF BeginTimeout, AllPendingRequests
      <3>4. TimeoutRequestFor(node).node = node
        BY DEF TimeoutRequestFor, TimeoutWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests \cup {TimeoutRequestFor(node)})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, BeginTimeout,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginTimeout,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>5. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. /\ TimeoutRequestFor(node).node \in Honest
            /\ TimeoutRequestFor(node).vote.signer =
                 TimeoutRequestFor(node).node
            /\ TimeoutRequestFor(node).vote.context = context
            /\ TimeoutRequestFor(node).vote.view = nodeView[node]
            /\ CanAppendTimeout(timeoutIntents,
                                TimeoutRequestFor(node).vote)
            /\ TimeoutVoteProtectsCommitSet(
                 TimeoutRequestFor(node).vote, commitIntents)
        <4>1. node \in Honest
          BY <1>1
             DEF BeginTimeout, TimeoutRequestFor,
                 LocalTimeoutVoteFor, TimeoutWal
        <4>2. /\ TimeoutRequestFor(node).node = node
              /\ TimeoutRequestFor(node).vote = LocalTimeoutVoteFor(node)
              /\ LocalTimeoutVoteFor(node).signer = node
              /\ LocalTimeoutVoteFor(node).context = context
              /\ LocalTimeoutVoteFor(node).view = nodeView[node]
          BY DEF TimeoutRequestFor, TimeoutWal,
                 LocalTimeoutVoteFor, TimeoutVote
        <4>3. TimeoutVoteProtectsCommitSet(
                 LocalTimeoutVoteFor(node), commitIntents)
          BY <1>1, BeginTimeoutHistoryGuardsAreDerived
        <4>4. \A prior \in timeoutIntents:
                 ~SameTimeoutSlot(prior, LocalTimeoutVoteFor(node))
          <5>1. ASSUME NEW prior \in timeoutIntents
                 PROVE ~SameTimeoutSlot(
                           prior, LocalTimeoutVoteFor(node))
            <6>1. ASSUME SameTimeoutSlot(
                           prior, LocalTimeoutVoteFor(node))
                   PROVE FALSE
              <7>1. NodeTimedOut(node, nodeView[node])
                BY <5>1, <6>1
                   DEF NodeTimedOut, SameTimeoutSlot,
                       LocalTimeoutVoteFor, TimeoutVote
              <7>2. ~NodeTimedOut(node, nodeView[node])
                BY <1>1 DEF BeginTimeout
              <7> QED BY <7>1, <7>2
            <6> QED BY <6>1
          <5> QED BY <5>1
        <4>5. CanAppendTimeout(timeoutIntents,
                              LocalTimeoutVoteFor(node))
          BY <4>1, <4>4, SMT
             DEF CanAppendTimeout, LocalTimeoutVoteFor, TimeoutVote
        <4> QED BY <4>1, <4>2, <4>3, <4>5
      <3>3. /\ pendingTimeout' =
                     pendingTimeout \cup {TimeoutRequestFor(node)}
            /\ pendingPrepare' = pendingPrepare
            /\ pendingLockCommit' = pendingLockCommit
            /\ context' = context
            /\ nodeView' = nodeView
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ durableBodies' = durableBodies
            /\ prepareQCs' = prepareQCs
            /\ installedTCs' = installedTCs
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
        BY <1>1 DEF BeginTimeout
      <3>4. \A pending \in pendingPrepare':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Prepare"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
               /\ CanAppendVote(prepareIntents', pending.vote)
               /\ PrepareCarriesHigherSafeQc(pending.vote)'
        BY <3>1, <3>3, SMT DEF PendingVoteWritesAuthorized,
                                    PrepareCarriesHigherSafeQc
      <3>5. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote =
                    Vote(context', pending.qc.view, "Commit",
                         pending.qc.subject, pending.node)
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ \/ CurrentOpenPrepareForCommit(
                        pending.node, pending.qc)'
                  \/ HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        BY <1>1, <3>1, <3>3, Isa
           DEF BeginTimeout, PendingVoteWritesAuthorized,
               CurrentOpenPrepareForCommit,
               HistoricalLockedPrepareForCommit,
               InstalledTcSelectsPrepareFor,
               NoHigherPrepareOriginKnown, NodeTimedOut
      <3>6. \A pending \in pendingTimeout':
               /\ pending.node \in Honest
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ CanAppendTimeout(timeoutIntents', pending.vote)
               /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                               commitIntents)'
        <4>1. ASSUME NEW pending \in pendingTimeout'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ CanAppendTimeout(timeoutIntents', pending.vote)
                     /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                     commitIntents)'
          <5>1. pending \in pendingTimeout
                  \/ pending = TimeoutRequestFor(node)
            BY <3>3, <4>1
          <5>2. CASE pending \in pendingTimeout
            BY <3>1, <3>3, <4>1, <5>2, Isa
               DEF PendingVoteWritesAuthorized,
                   TimeoutVoteProtectsCommitSet,
                   InstalledTcAuthorizesCommitVote
          <5>3. CASE pending = TimeoutRequestFor(node)
            <6>1. /\ pending.node = node
                  /\ pending.vote = LocalTimeoutVoteFor(node)
              BY <5>3 DEF TimeoutRequestFor, TimeoutWal
            <6>2. /\ node \in Honest
                  /\ LocalTimeoutVoteFor(node).signer = node
                  /\ LocalTimeoutVoteFor(node).context = context
                  /\ LocalTimeoutVoteFor(node).view = nodeView[node]
                  /\ CanAppendTimeout(timeoutIntents,
                                      LocalTimeoutVoteFor(node))
                  /\ TimeoutVoteProtectsCommitSet(
                       LocalTimeoutVoteFor(node), commitIntents)
              BY <3>2
                 DEF TimeoutRequestFor, TimeoutWal,
                     LocalTimeoutVoteFor, TimeoutVote
            <6>3. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ timeoutIntents' = timeoutIntents
                  /\ commitIntents' = commitIntents
                  /\ installedTCs' = installedTCs
              BY <3>3
            <6>4. /\ pending.node \in Honest
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.view = nodeView[pending.node]
                  /\ CanAppendTimeout(timeoutIntents, pending.vote)
                  /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                  commitIntents)
              BY <6>1, <6>2, Isa
            <6>5. /\ pending.vote.context = context'
                  /\ pending.vote.view = nodeView'[pending.node]
                  /\ CanAppendTimeout(timeoutIntents', pending.vote)
                  /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                  commitIntents)'
              BY <6>3, <6>4, Isa
                 DEF CanAppendTimeout, TimeoutVoteProtectsCommitSet,
                     InstalledTcAuthorizesCommitVote
            <6> QED BY <6>4, <6>5
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>4, <3>5, <3>6
         DEF PendingVoteWritesAuthorized
    <2>6. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingCertificateWritesAuthorized,
             TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
    <2>7. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ receivedVotes' = receivedVotes
            /\ receivedQCs' = receivedQCs
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ voteNetwork' = voteNetwork
            /\ qcNetwork' = qcNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF BeginTimeout
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. QcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>4. HonestTimeoutTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>5. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2>8. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. UNCHANGED
                 <<context, durableBodies, prepareIntents, commitIntents,
                   timeoutIntents, prepareQCs, commitQCs, formedTCs,
                   installedTCs, lockRank, lockSubject, highestRank,
                   highestSubject>>
        BY <1>1, Isa DEF BeginTimeout
      <3>2. CertificatesBackedByIntents'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>3. HonestDurableIntentsSound'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestDurableIntentsSound
      <3>4. FormedTimeoutCertificatesSound'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound
      <3>5. DurableTimeoutsProtectCommits
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>6. DurableTimeoutsProtectCommits'
        BY <3>1, <3>5,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>7. HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>8. HighestAndLockAreCertified'
        BY <3>1, <3>7,
           UnchangedHighestAndLockCertificationVarsPreserves
      <3> QED BY <3>2, <3>3, <3>4, <3>6, <3>8
    <2>8a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout
    <2>9. ReducerProvenanceInvariant'
      BY <2>5, <2>6, <2>7, <2>8, <2>8a
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>3, <2>4, <2>9,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginTimeout, LineageVars
  <1> QED BY <1>1

THEOREM TimeoutProtectionAppend ==
  \A timeoutVotes, timeoutVote, commits:
    TimeoutIntentProtectsCommits(timeoutVotes, commits)
      /\ TimeoutVoteProtectsCommitSet(timeoutVote, commits)
      => TimeoutIntentProtectsCommits(
           timeoutVotes \cup {timeoutVote}, commits)
BY DEF TimeoutIntentProtectsCommits

THEOREM PersistTimeoutPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistTimeout(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistTimeout(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.vote \in TimeoutVoteRecordSet
          /\ request.node \in Honest
          /\ request.vote.signer = request.node
          /\ request.vote.context = context
          /\ request.vote.view = nodeView[request.node]
          /\ CanAppendTimeout(timeoutIntents, request.vote)
          /\ TimeoutVoteProtectsCommitSet(request.vote, commitIntents)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistTimeout, TimeoutWalSet
    <2>2. TypeInvariant'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistTimeout, TimeoutSign, TimeoutSignSet
    <2>3. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistTimeout, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>4. HonestTimeoutUnique(timeoutIntents)'
      BY <1>1, <2>1, DurableTimeoutAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout
    <2>5. DurableTimeoutsProtectCommits'
      <3>1. TimeoutIntentProtectsCommits(
               timeoutIntents \cup {request.vote}, commitIntents)
        BY <1>1, <2>1, TimeoutProtectionAppend
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               DurableTimeoutsProtectCommits
      <3>2. /\ timeoutIntents' =
                    timeoutIntents \cup {request.vote}
            /\ commitIntents' = commitIntents
            /\ installedTCs' = installedTCs
        BY <1>1 DEF PersistTimeout
      <3> QED BY <3>1, <3>2, Isa
         DEF DurableTimeoutsProtectCommits,
             TimeoutIntentProtectsCommits,
             TimeoutVoteProtectsCommitSet,
             TimeoutVoteStrictlyProtectsCommit,
             InstalledTcAuthorizesCommitVote
    <2>6. TimeoutSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistTimeout,
             TimeoutSigningRequiresIntent, TimeoutSign
    <2>7. HonestTimeoutUniqueness'
      BY <2>4
         DEF HonestTimeoutUnique, HonestTimeoutUniqueness,
             SameTimeoutSlot, SameTimeoutContent
    <2>8. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistTimeout,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>9. Safety'
      BY <2>2, <2>3, <2>6, <2>7, <2>8 DEF Safety
    <2>10. /\ ContextIdentityBindsFrozenEpoch'
           /\ OldContextCertificateRejected'
           /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistTimeout,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>11. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. ASSUME NEW pending \in pendingTimeout'
             PROVE /\ pending.node \in Honest
                   /\ pending.vote.signer = pending.node
                   /\ pending.vote.context = context'
                   /\ pending.vote.view = nodeView'[pending.node]
                   /\ CanAppendTimeout(timeoutIntents', pending.vote)
                   /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                   commitIntents)'
        <4>1. /\ pending \in pendingTimeout
              /\ pending # request
          BY <1>1, <3>2 DEF PersistTimeout
        <4>2. /\ pending \in AllPendingRequests
              /\ request \in AllPendingRequests
              /\ RequestsUniqueByNode(AllPendingRequests)
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode, AllPendingRequests,
                 PersistTimeout
        <4>3. pending.node # request.node
          BY <4>1, <4>2 DEF RequestsUniqueByNode
        <4>4. /\ CanAppendTimeout(timeoutIntents, pending.vote)
              /\ request.vote.signer # pending.vote.signer
          BY <2>1, <3>1, <4>1, <4>3
             DEF PendingVoteWritesAuthorized
        <4>5. CanAppendTimeout(timeoutIntents \cup {request.vote},
                               pending.vote)
          BY <4>4, DistinctSignerAppendPreservesCanAppendTimeout
        <4> QED BY <1>1, <3>1, <4>1, <4>5, Isa
           DEF PendingVoteWritesAuthorized, PersistTimeout,
               TimeoutVoteProtectsCommitSet,
               InstalledTcAuthorizesCommitVote
      <3>3. \A pending \in pendingPrepare':
                     /\ pending.node \in Honest
                     /\ pending.vote.phase = "Prepare"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ CanAppendVote(prepareIntents', pending.vote)
                     /\ PrepareCarriesHigherSafeQc(pending.vote)'
        BY <1>1, <3>1, SMT
           DEF PersistTimeout, PendingVoteWritesAuthorized,
               PrepareCarriesHigherSafeQc
      <3>4. \A pending \in pendingLockCommit':
                     /\ pending.node \in Honest
                     /\ pending.vote =
                          Vote(context', pending.qc.view, "Commit",
                               pending.qc.subject, pending.node)
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              pending.node, pending.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
        <4>1. ASSUME NEW pending \in pendingLockCommit'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote =
                          Vote(context', pending.qc.view, "Commit",
                               pending.qc.subject, pending.node)
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              pending.node, pending.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. /\ pending \in pendingLockCommit
                /\ request \in pendingTimeout
                /\ pending \in AllPendingRequests
                /\ request \in AllPendingRequests
                /\ RequestsUniqueByNode(AllPendingRequests)
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety,
                   OnePendingPersistencePerNode, AllPendingRequests,
                   PersistTimeout
          <5>2. pending # request
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   PersistTimeout, LockCommitWalSet, TimeoutWalSet
          <5>3. pending.node # request.node
            BY <5>1, <5>2, DistinctUniqueRequestsHaveDistinctNodes
          <5>4. /\ pending.node \in Honest
                /\ pending.vote =
                     Vote(context, pending.qc.view, "Commit",
                          pending.qc.subject, pending.node)
                /\ pending.vote.phase = "Commit"
                /\ pending.vote.signer = pending.node
                /\ pending.vote.context = context
                /\ pending.vote.context = pending.qc.context
                /\ pending.vote.view = pending.qc.view
                /\ pending.vote.subject = pending.qc.subject
                /\ pending.qc.phase = "Prepare"
                /\ pending.qc \in prepareQCs
                /\ \/ CurrentOpenPrepareForCommit(
                         pending.node, pending.qc)
                   \/ HistoricalLockedPrepareForCommit(
                         pending.node, pending.qc)
                /\ pending.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies, pending.node,
                              pending.vote.context, pending.vote.view, pending.vote.subject)
                /\ pending.qc.view >= lockRank[pending.node]
                /\ (pending.qc.view = lockRank[pending.node]
                      => pending.qc.subject = lockSubject[pending.node])
                /\ CanAppendVote(commitIntents, pending.vote)
            BY <3>1, <5>1 DEF PendingVoteWritesAuthorized
          <5>5. /\ request.vote.signer = request.node
                /\ timeoutIntents' = timeoutIntents \cup {request.vote}
                /\ context' = context
                /\ nodeView' = nodeView
                /\ receivedQCs' = receivedQCs
                /\ prepareQCs' = prepareQCs
                /\ installedTCs' = installedTCs
                /\ prepareIntents' = prepareIntents
                /\ durableBodies' = durableBodies
                /\ lockRank' = lockRank
                /\ lockSubject' = lockSubject
                /\ highestRank' = highestRank
                /\ highestSubject' = highestSubject
                /\ commitIntents' = commitIntents
            BY <1>1, <2>1 DEF PersistTimeout
          <5>6. /\ (CurrentOpenPrepareForCommit(
                          pending.node, pending.qc)'
                        <=> CurrentOpenPrepareForCommit(
                              pending.node, pending.qc))
                 /\ (HistoricalLockedPrepareForCommit(
                          pending.node, pending.qc)'
                        <=> HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc))
            BY <5>3, <5>5, Isa
               DEF CurrentOpenPrepareForCommit,
                   HistoricalLockedPrepareForCommit,
                   InstalledTcSelectsPrepareFor,
                   NoHigherPrepareOriginKnown, NodeTimedOut
          <5> QED BY <5>4, <5>5, <5>6, Isa
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3, <3>4
         DEF PendingVoteWritesAuthorized
    <2>12. /\ HonestVoteUnique(prepareIntents)'
           /\ HonestVoteUnique(commitIntents)'
           /\ IntentPhasesCorrect'
           /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, HonestVoteUnique, IntentPhasesCorrect,
             PendingCertificateWritesAuthorized, TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>13. /\ HonestVoteTransportBacked'
           /\ QcTransportBacked'
           /\ TcTransportBacked'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ receivedVotes' = receivedVotes
            /\ receivedQCs' = receivedQCs
            /\ receivedTCs' = receivedTCs
            /\ voteNetwork' = voteNetwork
            /\ qcNetwork' = qcNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF PersistTimeout
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. QcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>14. /\ CertificatesBackedByIntents'
           /\ HonestDurableIntentsSound'
           /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, CertificatesBackedByIntents,
             HonestDurableIntentsSound, HighestAndLockAreCertified
    <2>15. HonestTimeoutTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, HonestTimeoutTransportBacked
    <2>16. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, FormedTimeoutCertificatesSound
    <2>16a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout
    <2>17. ReducerProvenanceInvariant'
      BY <2>4, <2>5, <2>11, <2>12, <2>13, <2>14, <2>15, <2>16,
         <2>16a
         DEF ReducerProvenanceInvariant
    <2>18. CurrentIntentViewsBound'
      <3>1. /\ context' = context
            /\ nodeView' = nodeView
            /\ prepareIntents' = prepareIntents
            /\ timeoutIntents' = timeoutIntents \cup {request.vote}
        BY <1>1 DEF PersistTimeout
      <3>2. \A vote \in prepareIntents':
               (vote.signer \in Honest /\ vote.context = context')
                 => vote.view <= nodeView'[vote.signer]
        BY <1>1, <3>1, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             CurrentIntentViewsBound
      <3>3. \A vote \in timeoutIntents':
               (vote.signer \in Honest /\ vote.context = context')
                 => vote.view <= nodeView'[vote.signer]
        <4>1. ASSUME NEW vote \in timeoutIntents',
                      vote.signer \in Honest,
                      vote.context = context'
               PROVE vote.view <= nodeView'[vote.signer]
          <5>1. vote \in timeoutIntents \/ vote = request.vote
            BY <3>1, <4>1
          <5>2. CASE vote \in timeoutIntents
            BY <1>1, <3>1, <4>1, <5>2
               DEF StrongInductiveInvariant, LineageInvariant,
                   CurrentIntentViewsBound
          <5>3. CASE vote = request.vote
            <6>1. /\ vote.view = request.vote.view
                  /\ vote.signer = request.vote.signer
              BY <5>3
            <6>2. /\ request.vote.view = nodeView[request.node]
                  /\ request.vote.signer = request.node
              BY <2>1
            <6>3. vote.view = nodeView[vote.signer]
              BY <6>1, <6>2
            <6>4. vote.signer \in ValidatorIds
              BY <2>1, <5>3 DEF TimeoutVoteRecordSet
            <6>5. nodeView'[vote.signer] = nodeView[vote.signer]
              BY <3>1, <6>4, Isa
            <6>6. vote.view = nodeView'[vote.signer]
              BY <6>3, <6>5, Isa
            <6>7. nodeView'[vote.signer] \in Nat
              <7>1. /\ nodeView' \in [ValidatorIds -> Views]
                    /\ ModelConfiguration
                BY <2>2 DEF TypeInvariant
              <7>2. nodeView'[vote.signer] \in Views
                BY <6>4, <7>1, FunctionValueHasCodomain
              <7>3. ViewDomain \subseteq Nat
                BY <7>1 DEF ModelConfiguration
              <7> QED BY <7>2, <7>3 DEF Views
            <6> QED BY <6>6, <6>7, NaturalOrderReflexive
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3 DEF CurrentIntentViewsBound
    <2>19. UNCHANGED
              <<context, nodeView, prepareIntents, commitIntents,
                prepareQCs, commitQCs, lockRank, lockSubject>>
      BY <1>1 DEF PersistTimeout
    <2>20. PrepareLineageSound'
      BY <1>1, <2>19, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             PrepareLineageSound, PrepareCarriesHigherSafeQc
    <2>21. /\ LocksCoverOwnCommits'
           /\ HonestCommitIntentPrepared'
           /\ CertificatePhasesCorrect'
      BY <1>1, <2>19, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             LocksCoverOwnCommits, HonestCommitIntentPrepared,
             CommitIntentsPreparedBy, CertificatePhasesCorrect
    <2>22. DurableIntentsDoNotAnticipateHeight'
      <3>1. DurableIntentsDoNotAnticipateHeight
        BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
      <3>2. request.vote.context.height <= height
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Heights
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF DurableIntentsDoNotAnticipateHeight, PersistTimeout
    <2>23. LineageInvariant'
      BY <2>18, <2>20, <2>21, <2>22 DEF LineageInvariant
    <2> QED BY <2>9, <2>10, <2>17, <2>23
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM UnchangedTimeoutIndependentProvenancePreserves ==
  ReducerProvenanceWithoutTimeoutTransport
    /\ UNCHANGED ProvenanceWithoutTimeoutTransportVars
    => ReducerProvenanceWithoutTimeoutTransport'
PROOF
  <1>1. ASSUME ReducerProvenanceWithoutTimeoutTransport,
              UNCHANGED ProvenanceWithoutTimeoutTransportVars
         PROVE ReducerProvenanceWithoutTimeoutTransport'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      <3>1. UNCHANGED <<context, nodeView, durableBodies, receivedQCs,
                        prepareIntents, commitIntents, timeoutIntents,
                        prepareQCs, installedTCs, lockRank, lockSubject,
                        highestRank, highestSubject, pendingPrepare,
                        pendingLockCommit, pendingTimeout>>
        BY <1>1, Isa DEF ProvenanceWithoutTimeoutTransportVars
      <3>2. PendingVoteWritesAuthorized'
        BY <1>1, <3>1,
           UnchangedPendingVoteWriteVarsPreservesAuthorization
           DEF ReducerProvenanceWithoutTimeoutTransport
      <3>3. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingCertificateWritesAuthorized'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutTimeoutTransport,
               ProvenanceWithoutTimeoutTransportVars, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect,
               PendingCertificateWritesAuthorized,
               SameVoteSlot, SameTimeoutSlot, SameTimeoutContent,
               TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3
    <2>2. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             HonestVoteTransportBacked, QcTransportBacked,
             TcTransportBacked, VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>3. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             CertificatesBackedByIntents, HonestDurableIntentsSound
    <2>4. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             FormedTimeoutCertificatesSound
    <2>5. /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. /\ timeoutIntents' = timeoutIntents
            /\ commitIntents' = commitIntents
            /\ installedTCs' = installedTCs
        BY <1>1 DEF ProvenanceWithoutTimeoutTransportVars
      <3>2. DurableTimeoutsProtectCommits
        BY <1>1 DEF ReducerProvenanceWithoutTimeoutTransport
      <3>3. DurableTimeoutsProtectCommits'
        BY <3>1, <3>2, Isa
           DEF DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits,
               TimeoutVoteProtectsCommitSet,
               TimeoutVoteStrictlyProtectsCommit,
               InstalledTcAuthorizesCommitVote
      <3>4. UNCHANGED <<context, prepareQCs, lockRank, lockSubject,
                        highestRank, highestSubject>>
        BY <1>1, Isa DEF ProvenanceWithoutTimeoutTransportVars
      <3>5. HighestAndLockAreCertified'
        BY <1>1, <3>4, Isa
           DEF ReducerProvenanceWithoutTimeoutTransport,
               HighestAndLockAreCertified
      <3> QED BY <3>3, <3>5
    <2>6. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
       DEF ReducerProvenanceWithoutTimeoutTransport
  <1> QED BY <1>1

THEOREM CompleteTimeoutSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteTimeoutSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteTimeoutSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteTimeoutSignature
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteTimeoutSignature,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent
    <2>3. /\ OnePendingPersistencePerNode'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteTimeoutSignature,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteTimeoutSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>6. HonestTimeoutTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CompleteTimeoutSignature, HonestTimeoutTransportBacked,
             BroadcastTimeouts, TimeoutEnvelope
    <2>7. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport,
             CompleteTimeoutSignature,
             ProvenanceWithoutTimeoutTransportVars
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteTimeoutSignature, LineageVars
  <1> QED BY <1>1

THEOREM ByzantineBroadcastTimeoutPreservesStrongInvariant ==
  \A signer, roundView, highestPrepare:
    StrongInductiveInvariant
      /\ ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW signer,
              NEW roundView,
              NEW highestPrepare,
              StrongInductiveInvariant,
              ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
         PROVE StrongInductiveInvariant'
    <2>1. signer \notin Honest
      BY <1>1 DEF ByzantineBroadcastTimeout, Byzantine
    <2>2. HonestTimeoutTransportBacked'
      <3>1. \A envelope \in timeoutNetwork:
               envelope.vote.signer \in Honest
                 => envelope.vote \in timeoutIntents
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>2. \A envelope \in BroadcastTimeouts(
                    TimeoutVote(context, roundView, signer, highestPrepare)):
               envelope.vote.signer \notin Honest
        BY <2>1 DEF BroadcastTimeouts, TimeoutEnvelope, TimeoutVote
      <3>3. \A envelope \in timeoutNetwork':
               envelope.vote.signer \in Honest
                 => envelope.vote \in timeoutIntents'
        BY <1>1, <3>1, <3>2, SMT
           DEF ByzantineBroadcastTimeout
      <3>4. \A received \in receivedTimeoutVotes':
               received.vote.signer \in Honest
                 => received.vote \in timeoutIntents'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               ByzantineBroadcastTimeout, HonestTimeoutTransportBacked
      <3> QED BY <3>3, <3>4 DEF HonestTimeoutTransportBacked
    <2>3. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             ByzantineBroadcastTimeout, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>4. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport,
             ByzantineBroadcastTimeout,
             ProvenanceWithoutTimeoutTransportVars
    <2>5. ReducerProvenanceInvariant'
      BY <2>2, <2>4
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>3, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ByzantineBroadcastTimeout, LineageVars
  <1> QED BY <1>1

THEOREM DeliverTimeoutPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverTimeout(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverTimeout(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. HonestTimeoutTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverTimeout, HonestTimeoutTransportBacked,
             TimeoutVoteAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             DeliverTimeout, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>3. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport, DeliverTimeout,
             ProvenanceWithoutTimeoutTransportVars
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>2, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverTimeout, LineageVars
  <1> QED BY <1>1

THEOREM ResumeTimeoutPreservesStrongInvariant ==
  \A node, vote:
    StrongInductiveInvariant /\ ResumeTimeout(node, vote)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW vote,
              StrongInductiveInvariant,
              ResumeTimeout(node, vote)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeTimeout, TimeoutSign, TimeoutSignSet
    <2>2. TimeoutSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeTimeout,
             TimeoutSigningRequiresIntent, TimeoutSign
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeTimeout,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ResumeTimeout,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, QcWireValid, CurrentEpoch
    <2>5. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ResumeTimeout, ProvenanceVars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeTimeout, LineageVars
  <1> QED BY <1>1

THEOREM BeginObservePreparePreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ BeginObservePrepare(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              BeginObservePrepare(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == ObservePrepareWal(node, qc)
    <2>1. qc \in prepareQCs
      <3>1. qc \in prepareQCs \cup commitQCs
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked, BeginObservePrepare, QcAt
      <3>2. qc.phase = "Prepare"
        BY <1>1 DEF BeginObservePrepare
      <3>3. \A committed \in commitQCs: committed.phase = "Commit"
        BY <1>1
           DEF StrongInductiveInvariant, LineageInvariant,
               CertificatePhasesCorrect
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. qc \in QcRecordSet
      BY <1>1, <2>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>3. Request \in ObservePrepareWalSet
      BY <1>1, <2>2, Isa
         DEF Request, ObservePrepareWal, ObservePrepareWalSet
    <2>4. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginObservePrepare, NodeIdle
      <3>3. /\ AllPendingRequests' = AllPendingRequests \cup {Request}
            /\ Request.node = node
        BY <1>1 DEF BeginObservePrepare, AllPendingRequests,
                       Request, ObservePrepareWal
      <3> QED BY <3>1, <3>2, <3>3,
                   NewRequestPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>5. TypeInvariant'
      BY <1>1, <2>3, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginObservePrepare
    <2>6. PendingCertificateWritesAuthorized'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             BeginObservePrepare, Request, ObservePrepareWal,
             TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
    <2>7. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, BeginObservePrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, QcWireValid, CurrentEpoch
    <2>8. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingVoteWritesAuthorized'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        <4>1. UNCHANGED
                 <<context, nodeView, durableBodies, receivedQCs,
                   prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                   installedTCs, lockRank, lockSubject, highestRank,
                   highestSubject, pendingPrepare, pendingLockCommit,
                   pendingTimeout>>
          BY <1>1 DEF BeginObservePrepare
        <4>2. PendingVoteWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>3. PendingVoteWritesAuthorized'
          BY <4>1, <4>2,
             UnchangedPendingVoteWriteVarsPreservesAuthorization
        <4>4. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginObservePrepare, HonestVoteUnique,
                 HonestTimeoutUnique, IntentPhasesCorrect
        <4>5. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginObservePrepare, HonestVoteTransportBacked,
                 QcTransportBacked, HonestTimeoutTransportBacked,
                 TcTransportBacked, VoteIntentFor, TCValid,
                 AuthenticatedHighRef, HighRefValid, CurrentEpoch,
                 CurrentVoters
        <4> QED BY <4>3, <4>4, <4>5
      <3>2. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, CertificatesBackedByIntents,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, FormedTimeoutCertificatesSound
      <3>4. /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        <4>1. UNCHANGED
                 <<timeoutIntents, commitIntents, installedTCs>>
          BY <1>1 DEF BeginObservePrepare
        <4>2. DurableTimeoutsProtectCommits
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>3. DurableTimeoutsProtectCommits'
          BY <4>1, <4>2,
             UnchangedDurableTimeoutProtectionVarsPreserves
        <4>4. UNCHANGED
                 <<context, prepareQCs, lockRank, lockSubject,
                   highestRank, highestSubject>>
          BY <1>1 DEF BeginObservePrepare
        <4>5. HighestAndLockAreCertified
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>6. HighestAndLockAreCertified'
          BY <4>4, <4>5,
             UnchangedHighestAndLockCertificationVarsPreserves
        <4> QED BY <4>3, <4>6
      <3>5. DurableLockRecoveryProvenanceInvariant'
        BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare
      <3> QED BY <2>6, <3>1, <3>2, <3>3, <3>4, <3>5
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>7, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginObservePrepare, LineageVars
  <1> QED BY <1>1

THEOREM PersistObservePreparePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistObservePrepare(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistObservePrepare(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.qc \in prepareQCs
          /\ request.qc.context = context
          /\ request.qc.view > highestRank[request.node]
          /\ request.qc.view \in Views
          /\ request.qc.subject \in SubjectOrNone
      <3>1. request \in pendingObservePrepare
        BY <1>1 DEF PersistObservePrepare
      <3>2. request \in ObservePrepareWalSet
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>3. /\ request.node \in ValidatorIds
            /\ request.qc \in QcRecordSet
            /\ request.qc.view \in Views
            /\ request.qc.subject \in SubjectOrNone
        BY <3>2, IsaT(120)
           DEF ObservePrepareWalSet, QcRecordSet, Subjects,
               SubjectOrNone
      <3>4. /\ request.qc \in prepareQCs
            /\ request.qc.context = context
            /\ request.qc.view > highestRank[request.node]
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized
      <3> QED BY <3>3, <3>4
    <2>2. /\ highestRank' \in [ValidatorIds -> Ranks]
          /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
      <3>1. /\ highestRank \in [ValidatorIds -> Ranks]
            /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. /\ request.qc.view \in Ranks
            /\ request.qc.subject \in SubjectOrNone
        BY <2>1, ViewsAreRanks
      <3>3. /\ highestRank' \in [ValidatorIds -> Ranks]
            /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
        BY <1>1, <2>1, <3>1, <3>2, Isa
           DEF PersistObservePrepare
      <3> QED BY <3>3
    <2>3. TypeInvariant'
      <3>1. /\ ModelConfiguration
            /\ height' \in Heights
            /\ context' \in ContextRecords
            /\ context'.height = height'
            /\ contextHistory' \subseteq ContextRecords
            /\ context' \in contextHistory'
            /\ nodeView' \in [ValidatorIds -> Views]
            /\ generation' \in [ValidatorIds -> Generations]
            /\ up' \subseteq ValidatorIds
            /\ gst' \in BOOLEAN
            /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
            /\ proposalIntents' \subseteq ProposalRecordSet
            /\ prepareIntents' \subseteq VoteRecordSet
            /\ commitIntents' \subseteq VoteRecordSet
            /\ timeoutIntents' \subseteq TimeoutVoteRecordSet
            /\ prepareQCs' \subseteq QcRecordSet
            /\ commitQCs' \subseteq QcRecordSet
            /\ \A tc \in formedTCs': TcWellTyped(tc)
            /\ \A entry \in receivedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ \A entry \in installedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ lockRank' \in [ValidatorIds -> Ranks]
            /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
        BY <1>1, IsaT(60)
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3>2. /\ pendingProposal' \subseteq ProposalWalSet
            /\ pendingPrepare' \subseteq PrepareWalSet
            /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
            /\ pendingLockCommit' \subseteq LockCommitWalSet
            /\ pendingTimeout' \subseteq TimeoutWalSet
            /\ pendingInstallTC' \subseteq InstallTcWalSet
            /\ pendingDecision' \subseteq DecisionWalSet
        BY <1>1, IsaT(60)
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3>3. /\ signProposals' \subseteq ProposalSignSet
            /\ signVotes' \subseteq VoteSignSet
            /\ signTimeouts' \subseteq TimeoutSignSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3>4. /\ availableBodies' \subseteq BodyRecordSet
            /\ durableBodies' \subseteq BodyRecordSet
            /\ retainedLockedBodies' \subseteq RetainedLockedBodyRecordSet
            /\ validatedBodies' \subseteq ValidationRecordSet
            /\ invalidBodies' \subseteq BodyRecordSet
            /\ RetainedLockedBodiesSound(retainedLockedBodies',
                                          durableBodies')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4 DEF TypeInvariant
    <2>4. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistObservePrepare
    <2>5. LockBelowHighest'
      <3>1. /\ LockBelowHighest
            /\ request.node \in ValidatorIds
            /\ request.qc.view > highestRank[request.node]
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety
      <3>2. ASSUME NEW node \in ValidatorIds
             PROVE lockRank'[node] <= highestRank'[node]
        <4>1. CASE node = request.node
          <5>1. /\ lockRank' = lockRank
                /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockRank' \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestRank' \in [ValidatorIds -> Ranks]
            BY <1>1, <2>3
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. /\ lockRank'[node] = lockRank[node]
                /\ highestRank'[node] = request.qc.view
            BY <2>1, <3>2, <4>1, <5>1, <5>2, Isa
          <5>4. lockRank[node] <= highestRank[node]
            BY <3>1, <3>2 DEF LockBelowHighest
          <5>5. /\ lockRank[node] \in Int
                /\ highestRank[node] \in Int
                /\ request.qc.view \in Int
            <6>1. ModelConfiguration
              BY <1>1 DEF StrongInductiveInvariant, Safety, TypeInvariant
            <6>2. ViewDomain \subseteq Nat
              BY <6>1 DEF ModelConfiguration
            <6>3. /\ Views \subseteq Int
                  /\ Ranks \subseteq Int
              BY <6>2, SMT DEF Views, Ranks, NoRank
            <6>4. /\ lockRank[node] \in Ranks
                  /\ highestRank[node] \in Ranks
                  /\ request.qc.view \in Views
              BY <2>1, <3>2, <5>2, FunctionValueHasCodomain
            <6> QED BY <6>3, <6>4, Isa
          <5>6. lockRank[node] < request.qc.view
            BY <3>1, <4>1, <5>4, <5>5,
               IntegerWeakStrongOrderChain
          <5>7. lockRank'[node] < highestRank'[node]
            BY <5>3, <5>6, Isa
          <5> QED BY <5>5, <5>7, IntegerStrictImpliesWeak
        <4>2. CASE node # request.node
          <5>1. /\ lockRank' = lockRank
                /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockRank' \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestRank' \in [ValidatorIds -> Ranks]
            BY <1>1, <2>3
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. /\ lockRank'[node] = lockRank[node]
                /\ highestRank'[node] = highestRank[node]
            BY <3>2, <4>2, <5>1, <5>2, Isa
          <5> QED BY <3>1, <3>2, <5>3
             DEF LockBelowHighest
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>2 DEF LockBelowHighest
    <2>6. PendingCertificateWritesAuthorized'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. \A pending \in pendingObservePrepare':
               /\ pending.qc \in prepareQCs'
               /\ pending.qc.context = context'
               /\ pending.qc.view > highestRank'[pending.node]
        <4>1. ASSUME NEW pending \in pendingObservePrepare'
               PROVE /\ pending.qc \in prepareQCs'
                     /\ pending.qc.context = context'
                     /\ pending.qc.view > highestRank'[pending.node]
          <5>1. /\ pending \in pendingObservePrepare
                /\ pending # request
            BY <1>1, <4>1 DEF PersistObservePrepare
          <5>2. pending.node # request.node
            <6>1. /\ pending \in AllPendingRequests
                  /\ request \in AllPendingRequests
              BY <1>1, <5>1
                 DEF PersistObservePrepare, AllPendingRequests
            <6> QED BY <3>1, <5>1, <6>1,
                         DistinctUniqueRequestsHaveDistinctNodes
          <5>3. /\ prepareQCs' = prepareQCs
                /\ context' = context
                /\ highestRank'[pending.node] = highestRank[pending.node]
            <6>1. /\ prepareQCs' = prepareQCs
                  /\ context' = context
                  /\ highestRank' =
                       [highestRank EXCEPT
                          ![request.node] = request.qc.view]
              BY <1>1 DEF PersistObservePrepare
            <6>2. /\ pending.node \in ValidatorIds
                  /\ request.node \in ValidatorIds
                  /\ highestRank \in [ValidatorIds -> Ranks]
              BY <1>1, <2>1, <5>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     ObservePrepareWalSet
            <6> QED BY <5>2, <6>1, <6>2, Isa
          <5>4. /\ pending.qc \in prepareQCs
                /\ pending.qc.context = context
                /\ pending.qc.view > highestRank[pending.node]
            BY <1>1, <5>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   PendingCertificateWritesAuthorized
          <5> QED BY <5>3, <5>4
        <4> QED BY <4>1
      <3>3. /\ \A pending \in pendingInstallTC':
                     /\ pending.tc \in formedTCs'
                     /\ pending.tc.context = context'
                     /\ TCValid(pending.tc)'
                     /\ pending.tc.votes # {}
                     /\ pending.tc.view + 1 \in Views
                     /\ pending.tc.view + 1 >= nodeView'[pending.node]
            /\ \A pending \in pendingDecision':
                     /\ pending.qc \in commitQCs'
                     /\ pending.qc.context = context'
                     /\ pending.qc.phase = "Commit"
                     /\ pending.qc.height = height'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized, PersistObservePrepare,
               TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3 DEF PendingCertificateWritesAuthorized
    <2>7. HighestAndLockAreCertified'
      <3>1. /\ highestRank'[request.node] = request.qc.view
            /\ highestSubject'[request.node] = request.qc.subject
            /\ lockRank'[request.node] = lockRank[request.node]
            /\ lockSubject'[request.node] = lockSubject[request.node]
        <4>1. /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ highestSubject' =
                     [highestSubject EXCEPT
                        ![request.node] = request.qc.subject]
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
          BY <1>1 DEF PersistObservePrepare
        <4>2. /\ request.node \in ValidatorIds
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1, <2>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4> QED BY <2>1, <4>1, <4>2, Isa
      <3>2. \A node \in ValidatorIds:
               node # request.node
                 => /\ highestRank'[node] = highestRank[node]
                    /\ highestSubject'[node] = highestSubject[node]
                    /\ lockRank'[node] = lockRank[node]
                    /\ lockSubject'[node] = lockSubject[node]
        <4>1. /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ highestSubject' =
                     [highestSubject EXCEPT
                        ![request.node] = request.qc.subject]
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
          BY <1>1 DEF PersistObservePrepare
        <4>2. ASSUME NEW node \in ValidatorIds,
                       node # request.node
               PROVE /\ highestRank'[node] = highestRank[node]
                     /\ highestSubject'[node] = highestSubject[node]
                     /\ lockRank'[node] = lockRank[node]
                     /\ lockSubject'[node] = lockSubject[node]
          <5>1. /\ request.node \in ValidatorIds
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
            BY <1>1, <2>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5> QED BY <4>1, <4>2, <5>1, Isa
        <4> QED BY <4>2
      <3>3. ASSUME NEW node \in ValidatorIds
             PROVE /\ (highestRank'[node] = NoRank
                          => highestSubject'[node] = NoSubject)
                   /\ (highestRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = highestRank'[node]
                               /\ qc.subject = highestSubject'[node])
                   /\ (lockRank'[node] = NoRank
                          => lockSubject'[node] = NoSubject)
                   /\ (lockRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = lockRank'[node]
                               /\ qc.subject = lockSubject'[node])
        <4>1. CASE node = request.node
          <5>1. request.qc.view # NoRank
            BY <1>1, <2>1, ViewIsNotNoRank
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. /\ prepareQCs' = prepareQCs
                /\ context' = context
            BY <1>1 DEF PersistObservePrepare
          <5>3. /\ highestRank'[node] = request.qc.view
                /\ highestSubject'[node] = request.qc.subject
                /\ lockRank'[node] = lockRank[node]
                /\ lockSubject'[node] = lockSubject[node]
            BY <3>1, <4>1
          <5>4. /\ (highestRank'[node] = NoRank
                           => highestSubject'[node] = NoSubject)
                /\ (highestRank'[node] # NoRank
                           => \E qc \in prepareQCs':
                                /\ qc.context = context'
                                /\ qc.view = highestRank'[node]
                                /\ qc.subject = highestSubject'[node])
            <6>1. highestRank'[node] # NoRank
              BY <5>1, <5>3
            <6>2. /\ request.qc \in prepareQCs'
                  /\ request.qc.context = context'
                  /\ request.qc.view = highestRank'[node]
                  /\ request.qc.subject = highestSubject'[node]
              BY <2>1, <5>2, <5>3
            <6>3. \E qc \in prepareQCs':
                     /\ qc.context = context'
                     /\ qc.view = highestRank'[node]
                     /\ qc.subject = highestSubject'[node]
              BY <6>2
            <6> QED BY <6>1, <6>3
          <5>5. /\ (lockRank[node] = NoRank
                           => lockSubject[node] = NoSubject)
                /\ (lockRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = lockRank[node]
                                /\ qc.subject = lockSubject[node])
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   HighestAndLockAreCertified
          <5> QED BY <5>2, <5>3, <5>4, <5>5
        <4>2. CASE node # request.node
          <5>1. /\ prepareQCs' = prepareQCs
                /\ context' = context
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ (highestRank[node] = NoRank
                           => highestSubject[node] = NoSubject)
                /\ (highestRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = highestRank[node]
                                /\ qc.subject = highestSubject[node])
                /\ (lockRank[node] = NoRank
                           => lockSubject[node] = NoSubject)
                /\ (lockRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = lockRank[node]
                                /\ qc.subject = lockSubject[node])
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   HighestAndLockAreCertified
          <5> QED BY <3>2, <4>2, <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>3 DEF HighestAndLockAreCertified
    <2>8. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>3, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, PersistObservePrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2>9. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect
      <3>2. PendingVoteWritesAuthorized'
        <4>1. PendingVoteWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>2. RequestsUniqueByNode(AllPendingRequests)
          BY <1>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode
        <4>3. \A pending \in pendingLockCommit:
                 pending.node # request.node
          <5>1. ASSUME NEW pending \in pendingLockCommit
                 PROVE pending.node # request.node
            <6>1. /\ pending \in LockCommitWalSet
                  /\ request \in ObservePrepareWalSet
              BY <1>1, <5>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     PersistObservePrepare
            <6>2. pending # request
              BY <6>1, Isa
                 DEF LockCommitWalSet, ObservePrepareWalSet
            <6>3. /\ pending \in AllPendingRequests
                  /\ request \in AllPendingRequests
              BY <1>1, <5>1
                 DEF PersistObservePrepare, AllPendingRequests
            <6> QED BY <4>2, <6>2, <6>3,
                         DistinctUniqueRequestsHaveDistinctNodes
          <5> QED BY <5>1
        <4>4. \A pending \in pendingLockCommit:
                 /\ highestRank'[pending.node] = highestRank[pending.node]
                 /\ highestSubject'[pending.node] = highestSubject[pending.node]
          <5>1. ASSUME NEW pending \in pendingLockCommit
                 PROVE /\ highestRank'[pending.node] =
                              highestRank[pending.node]
                       /\ highestSubject'[pending.node] =
                              highestSubject[pending.node]
            <6>1. /\ pending.node # request.node
                  /\ pending.node \in ValidatorIds
                  /\ request.node \in ValidatorIds
                  /\ highestRank \in [ValidatorIds -> Ranks]
                  /\ highestSubject \in
                       [ValidatorIds -> SubjectOrNone]
              BY <1>1, <2>1, <4>3, <5>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     LockCommitWalSet
            <6>2. /\ highestRank' =
                         [highestRank EXCEPT
                            ![request.node] = request.qc.view]
                  /\ highestSubject' =
                         [highestSubject EXCEPT
                            ![request.node] = request.qc.subject]
              BY <1>1 DEF PersistObservePrepare
            <6> QED BY <6>1, <6>2, Isa
          <5> QED BY <5>1
        <4>5. (\A pending \in pendingPrepare:
                 /\ pending.node \in Honest
                 /\ pending.vote.phase = "Prepare"
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context
                 /\ pending.vote.view = nodeView[pending.node]
                 /\ pending.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies, pending.node,
                               pending.vote.context, pending.vote.view,
                               pending.vote.subject)
                 /\ CanAppendVote(prepareIntents, pending.vote)
                 /\ PrepareCarriesHigherSafeQc(pending.vote))'
          BY <1>1, <4>1, Isa
             DEF PersistObservePrepare, PendingVoteWritesAuthorized,
                 PrepareCarriesHigherSafeQc
        <4>6. \A pending \in pendingLockCommit:
                 CurrentOpenPrepareForCommit(pending.node, pending.qc)
                   => CurrentOpenPrepareForCommit(pending.node, pending.qc)'
          BY <1>1, Isa
             DEF PersistObservePrepare, CurrentOpenPrepareForCommit,
                 NodeTimedOut
        <4>7. \A pending \in pendingLockCommit:
                 HistoricalLockedPrepareForCommit(
                   pending.node, pending.qc)
                   => HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
          BY <1>1, <4>4, Isa
             DEF PersistObservePrepare,
                 HistoricalLockedPrepareForCommit,
                 InstalledTcSelectsPrepareFor,
                 NoHigherPrepareOriginKnown
        <4>8. (\A pending \in pendingLockCommit:
                 /\ pending.node \in Honest
                 /\ pending.vote.phase = "Commit"
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context
                 /\ pending.vote.context = pending.qc.context
                 /\ pending.vote.view = pending.qc.view
                 /\ pending.vote.subject = pending.qc.subject
                 /\ pending.vote =
                      Vote(context, pending.qc.view, "Commit",
                           pending.qc.subject, pending.node)
                 /\ pending.qc.phase = "Prepare"
                 /\ pending.qc \in prepareQCs
                 /\ pending.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies, pending.node,
                               pending.vote.context, pending.vote.view,
                               pending.vote.subject)
                 /\ pending.qc.view >= lockRank[pending.node]
                 /\ (pending.qc.view = lockRank[pending.node]
                       => pending.qc.subject = lockSubject[pending.node])
                 /\ CanAppendVote(commitIntents, pending.vote))'
          BY <1>1, <4>1, Isa
             DEF PersistObservePrepare, PendingVoteWritesAuthorized
        <4>9. (\A pending \in pendingLockCommit:
                 \/ CurrentOpenPrepareForCommit(
                      pending.node, pending.qc)
                 \/ HistoricalLockedPrepareForCommit(
                      pending.node, pending.qc))'
          BY <1>1, <4>1, <4>6, <4>7, Isa
             DEF PersistObservePrepare, PendingVoteWritesAuthorized
        <4>10. (\A pending \in pendingLockCommit:
                  /\ pending.node \in Honest
                  /\ pending.vote.phase = "Commit"
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.context = pending.qc.context
                  /\ pending.vote.view = pending.qc.view
                  /\ pending.vote.subject = pending.qc.subject
                  /\ pending.vote =
                       Vote(context, pending.qc.view, "Commit",
                            pending.qc.subject, pending.node)
                  /\ pending.qc.phase = "Prepare"
                  /\ pending.qc \in prepareQCs
                  /\ \/ CurrentOpenPrepareForCommit(
                           pending.node, pending.qc)
                     \/ HistoricalLockedPrepareForCommit(
                           pending.node, pending.qc)
                  /\ pending.vote.subject \in ValidSubjects
                  /\ BodyHeldBy(durableBodies, pending.node,
                                pending.vote.context, pending.vote.view,
                                pending.vote.subject)
                  /\ pending.qc.view >= lockRank[pending.node]
                  /\ (pending.qc.view = lockRank[pending.node]
                        => pending.qc.subject = lockSubject[pending.node])
                  /\ CanAppendVote(commitIntents, pending.vote))'
          BY <4>8, <4>9, Isa
        <4>11. (\A pending \in pendingTimeout:
                 /\ pending.node \in Honest
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context
                 /\ pending.vote.view = nodeView[pending.node]
                 /\ CanAppendTimeout(timeoutIntents, pending.vote)
                 /\ TimeoutVoteProtectsCommitSet(
                      pending.vote, commitIntents))'
          BY <1>1, <4>1, Isa
             DEF PersistObservePrepare, PendingVoteWritesAuthorized,
                 TimeoutVoteProtectsCommitSet,
                 TimeoutVoteStrictlyProtectsCommit,
                 InstalledTcAuthorizesCommitVote
        <4> QED BY <4>5, <4>10, <4>11
           DEF PendingVoteWritesAuthorized
      <3>3. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, HonestVoteTransportBacked,
               QcTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>4. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
        <4>1. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PersistObservePrepare, CertificatesBackedByIntents,
                 HonestDurableIntentsSound,
                 FormedTimeoutCertificatesSound
        <4>2. UNCHANGED
                 <<timeoutIntents, commitIntents, installedTCs>>
          BY <1>1 DEF PersistObservePrepare
        <4>3. DurableTimeoutsProtectCommits
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>4. DurableTimeoutsProtectCommits'
          BY <4>2, <4>3,
             UnchangedDurableTimeoutProtectionVarsPreserves
        <4> QED BY <4>1, <4>4
      <3>5. DurableLockRecoveryProvenanceInvariant'
        BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare
      <3> QED BY <2>6, <2>7, <3>1, <3>2, <3>3, <3>4, <3>5
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>8, <2>9,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, PersistObservePrepare, LineageVars
  <1> QED BY <1>1

(***************************************************************************
Pending LockCommit writes retain one of two authorizations across unrelated
asynchronous transitions.  The ordinary path is exactly at the installed
view; the TC-recovery path is strictly below it.  Both therefore install a
lock no higher than the node's view, but only the historical path supplies
installed-TC authorization for a Commit created after the timeout.
***************************************************************************)
THEOREM PendingLockCommitAuthorizationFacts ==
  \A request:
    /\ TypeInvariant
    /\ PendingVoteWritesAuthorized
    /\ request \in pendingLockCommit
    => /\ \/ CurrentOpenPrepareForCommit(request.node, request.qc)
           \/ HistoricalLockedPrepareForCommit(request.node, request.qc)
       /\ request.qc.view <= nodeView[request.node]
       /\ (request.qc.view < nodeView[request.node]
             => HistoricalLockedPrepareForCommit(
                  request.node, request.qc))
PROOF
  <1>1. ASSUME NEW request,
                TypeInvariant,
                PendingVoteWritesAuthorized,
                request \in pendingLockCommit
         PROVE /\ \/ CurrentOpenPrepareForCommit(request.node, request.qc)
                    \/ HistoricalLockedPrepareForCommit(
                         request.node, request.qc)
               /\ request.qc.view <= nodeView[request.node]
               /\ (request.qc.view < nodeView[request.node]
                     => HistoricalLockedPrepareForCommit(
                          request.node, request.qc))
    <2>1. \/ CurrentOpenPrepareForCommit(request.node, request.qc)
           \/ HistoricalLockedPrepareForCommit(request.node, request.qc)
      BY <1>1 DEF PendingVoteWritesAuthorized
    <2>2. /\ request.qc.view \in Nat
           /\ nodeView[request.node] \in Nat
      <3>1. pendingLockCommit \subseteq LockCommitWalSet
        BY <1>1 DEF TypeInvariant
      <3>2. /\ request.node \in ValidatorIds
            /\ request.qc \in QcRecordSet
        BY <1>1, <3>1, Isa DEF LockCommitWalSet
      <3>3. /\ request.qc.view \in Views
            /\ nodeView[request.node] \in Views
        BY <1>1, <3>2, Isa DEF TypeInvariant, QcRecordSet
      <3>4. Views \subseteq Nat
        BY <1>1 DEF TypeInvariant, ModelConfiguration, Views
      <3> QED BY <3>3, <3>4
    <2>3. request.qc.view <= nodeView[request.node]
      BY <2>1, <2>2, SMT
         DEF CurrentOpenPrepareForCommit,
             HistoricalLockedPrepareForCommit
    <2>4. request.qc.view < nodeView[request.node]
             => HistoricalLockedPrepareForCommit(
                  request.node, request.qc)
      BY <2>1, SMT DEF CurrentOpenPrepareForCommit
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalPendingLockCommitHasInstalledTcAuthorization ==
  \A request:
    /\ PendingVoteWritesAuthorized
    /\ request \in pendingLockCommit
    /\ HistoricalLockedPrepareForCommit(request.node, request.qc)
    => InstalledTcAuthorizesCommitVote(request.vote)
BY SMT
   DEF PendingVoteWritesAuthorized,
       HistoricalLockedPrepareForCommit,
       InstalledTcSelectsPrepareFor,
       InstalledTcAuthorizesCommitVote

THEOREM PersistLockCommitOnlyPrunesVoteReceipts ==
  \A request:
    PersistLockCommit(request) => receivedVotes' \subseteq receivedVotes
BY SMT DEF PersistLockCommit

THEOREM BeginLockCommitPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ BeginLockCommit(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              BeginLockCommit(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE CommitVote ==
           Vote(context, qc.view, "Commit", qc.subject, node)
    <2> DEFINE Request == LockCommitWal(node, qc, CommitVote)
    <2>1. qc \in prepareQCs
      <3>1. qc \in prepareQCs \cup commitQCs
        <4>1. \/ CurrentOpenPrepareForCommit(node, qc)
               \/ HistoricalLockedPrepareForCommit(node, qc)
          BY <1>1 DEF BeginLockCommit
        <4>2. QcTransportBacked
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>3. CASE CurrentOpenPrepareForCommit(node, qc)
          BY <4>2, <4>3
             DEF QcTransportBacked, CurrentOpenPrepareForCommit, QcAt
        <4>4. CASE HistoricalLockedPrepareForCommit(node, qc)
          BY <4>4 DEF HistoricalLockedPrepareForCommit
        <4> QED BY <4>1, <4>3, <4>4
      <3>2. qc.phase = "Prepare"
        BY <1>1 DEF BeginLockCommit
      <3>3. \A committed \in commitQCs: committed.phase = "Commit"
        BY <1>1
           DEF StrongInductiveInvariant, LineageInvariant,
               CertificatePhasesCorrect
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. CanAppendVote(commitIntents, CommitVote)
      <3>1. node \in Honest
        BY <1>1 DEF BeginLockCommit
      <3>2. ASSUME NEW prior \in commitIntents,
                    SameVoteSlot(prior, CommitVote)
             PROVE prior.subject = CommitVote.subject
        <4>1. /\ prior.signer = node
              /\ prior.context = context
              /\ prior.view = qc.view
              /\ CommitVote.subject = qc.subject
          BY <3>2 DEF CommitVote, Vote, SameVoteSlot
        <4>2. /\ lockRank[node] >= prior.view
              /\ (lockRank[node] = prior.view
                    => lockSubject[node] = prior.subject)
          BY <1>1, <3>1, <3>2, <4>1
             DEF StrongInductiveInvariant, LineageInvariant,
                 LocksCoverOwnCommits
        <4>3. /\ qc.view >= lockRank[node]
              /\ (qc.view = lockRank[node]
                    => qc.subject = lockSubject[node])
          BY <1>1 DEF BeginLockCommit
        <4>4. /\ qc.view \in Int
              /\ lockRank[node] \in Int
          <5>1. qc.view \in Views
            BY <1>1, <2>1, Isa
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   QcRecordSet
          <5>2. lockRank[node] \in Ranks
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. ViewDomain \subseteq Nat
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   ModelConfiguration
          <5> QED BY <5>1, <5>2, <5>3, SMT
             DEF Views, Ranks, NoRank
        <4>5. qc.view = lockRank[node]
          BY <4>1, <4>2, <4>3, <4>4, SMT
        <4>6. /\ qc.subject = lockSubject[node]
              /\ prior.subject = lockSubject[node]
          BY <4>1, <4>2, <4>3, <4>5
        <4> QED BY <4>1, <4>6
      <3> QED BY <3>1, <3>2 DEF CanAppendVote, CommitVote, Vote
    <2>3. /\ Request \in LockCommitWalSet
          /\ Request.node \in Honest
          /\ Request.vote =
               Vote(context, Request.qc.view, "Commit",
                    Request.qc.subject, Request.node)
          /\ Request.vote.phase = "Commit"
          /\ Request.vote.signer = Request.node
          /\ Request.vote.context = context
          /\ Request.vote.context = Request.qc.context
          /\ Request.vote.view = Request.qc.view
          /\ Request.vote.subject = Request.qc.subject
          /\ Request.qc.phase = "Prepare"
          /\ Request.qc \in prepareQCs
          /\ \/ CurrentOpenPrepareForCommit(Request.node, Request.qc)
             \/ HistoricalLockedPrepareForCommit(
                  Request.node, Request.qc)
          /\ Request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, Request.node,
                        Request.vote.context, Request.vote.view, Request.vote.subject)
          /\ Request.qc.view >= lockRank[Request.node]
          /\ (Request.qc.view = lockRank[Request.node]
                => Request.qc.subject = lockSubject[Request.node])
          /\ CanAppendVote(commitIntents, Request.vote)
      <3>1. /\ node \in ValidatorIds
            /\ node \in Honest
            /\ qc \in QcRecordSet
            /\ qc.context = context
            /\ qc.view \in Views
            /\ qc.subject \in ValidSubjects
            /\ qc.phase = "Prepare"
            /\ \/ CurrentOpenPrepareForCommit(node, qc)
               \/ HistoricalLockedPrepareForCommit(node, qc)
            /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
            /\ qc.view >= lockRank[node]
            /\ (qc.view = lockRank[node]
                  => qc.subject = lockSubject[node])
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HistoricalQcValid, BeginLockCommit
      <3>2. CommitVote \in VoteRecordSet
        <4>1. /\ context \in ContextRecords
              /\ context.height \in Heights
              /\ qc.view \in Views
              /\ qc.subject \in Subjects
              /\ node \in ValidatorIds
              /\ "Commit" \in Phases
          BY <1>1, <3>1, Isa
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 ModelConfiguration, Phases
        <4> QED BY <4>1, IsaT(120)
           DEF CommitVote, Vote, VoteRecordSet
      <3>3. /\ Request \in LockCommitWalSet
            /\ Request.node = node
            /\ Request.vote = CommitVote
            /\ Request.qc = qc
            /\ Request.vote.phase = "Commit"
            /\ Request.vote.signer = Request.node
            /\ Request.vote.context = context
            /\ Request.vote.context = Request.qc.context
            /\ Request.vote.view = Request.qc.view
            /\ Request.vote.subject = Request.qc.subject
        <4>1. Request \in LockCommitWalSet
          BY <3>1, <3>2, Isa
             DEF Request, LockCommitWal, LockCommitWalSet
        <4>2. /\ Request.node = node
              /\ Request.vote = CommitVote
              /\ Request.qc = qc
              /\ Request.vote.phase = "Commit"
              /\ Request.vote.signer = Request.node
              /\ Request.vote.context = context
              /\ Request.vote.context = Request.qc.context
              /\ Request.vote.view = Request.qc.view
              /\ Request.vote.subject = Request.qc.subject
          BY <3>1, Isa
             DEF Request, CommitVote, LockCommitWal, Vote
        <4> QED BY <4>1, <4>2
      <3> QED BY <1>1, <2>1, <2>2, <3>1, <3>3
         DEF BeginLockCommit, Request, LockCommitWal
    <2>4. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginLockCommit, NodeIdle
      <3>3. /\ AllPendingRequests' = AllPendingRequests \cup {Request}
            /\ Request.node = node
        BY <1>1 DEF BeginLockCommit, AllPendingRequests,
                       Request, LockCommitWal
      <3> QED BY <3>1, <3>2, <3>3,
                   NewRequestPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>5. TypeInvariant'
      BY <1>1, <2>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginLockCommit, Request
    <2>6. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. pendingLockCommit' = pendingLockCommit \cup {Request}
        BY <1>1 DEF BeginLockCommit, Request
      <3>3. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote =
                    Vote(context', pending.qc.view, "Commit",
                         pending.qc.subject, pending.node)
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ \/ CurrentOpenPrepareForCommit(
                        pending.node, pending.qc)'
                  \/ HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        <4>1. ASSUME NEW pending \in pendingLockCommit'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote =
                          Vote(context', pending.qc.view, "Commit",
                               pending.qc.subject, pending.node)
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              pending.node, pending.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              pending.node, pending.qc)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context, pending.vote.view, pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. pending \in pendingLockCommit \/ pending = Request
            BY <3>2, <4>1
          <5>2. CASE pending \in pendingLockCommit
            <6>1. /\ pending.node \in Honest
                  /\ pending.vote =
                       Vote(context, pending.qc.view, "Commit",
                            pending.qc.subject, pending.node)
                  /\ pending.vote.phase = "Commit"
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.context = pending.qc.context
                  /\ pending.vote.view = pending.qc.view
                  /\ pending.vote.subject = pending.qc.subject
                  /\ pending.qc.phase = "Prepare"
                  /\ pending.qc \in prepareQCs
                  /\ \/ CurrentOpenPrepareForCommit(
                           pending.node, pending.qc)
                     \/ HistoricalLockedPrepareForCommit(
                           pending.node, pending.qc)
                  /\ pending.vote.subject \in ValidSubjects
                  /\ BodyHeldBy(durableBodies, pending.node,
                                pending.vote.context, pending.vote.view, pending.vote.subject)
                  /\ pending.qc.view >= lockRank[pending.node]
                  /\ (pending.qc.view = lockRank[pending.node]
                        => pending.qc.subject = lockSubject[pending.node])
                  /\ CanAppendVote(commitIntents, pending.vote)
              BY <3>1, <5>2 DEF PendingVoteWritesAuthorized
            <6>2. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ durableBodies' = durableBodies
                  /\ receivedQCs' = receivedQCs
                  /\ prepareQCs' = prepareQCs
                  /\ timeoutIntents' = timeoutIntents
                  /\ installedTCs' = installedTCs
                  /\ prepareIntents' = prepareIntents
                  /\ lockRank' = lockRank
                  /\ lockSubject' = lockSubject
                  /\ highestRank' = highestRank
                  /\ highestSubject' = highestSubject
                  /\ commitIntents' = commitIntents
              BY <1>1 DEF BeginLockCommit
            <6>3. /\ (CurrentOpenPrepareForCommit(
                            pending.node, pending.qc)'
                          <=> CurrentOpenPrepareForCommit(
                                pending.node, pending.qc))
                   /\ (HistoricalLockedPrepareForCommit(
                            pending.node, pending.qc)'
                          <=> HistoricalLockedPrepareForCommit(
                                pending.node, pending.qc))
              BY <6>2
                 DEF CurrentOpenPrepareForCommit,
                     HistoricalLockedPrepareForCommit,
                     InstalledTcSelectsPrepareFor,
                     NoHigherPrepareOriginKnown, NodeTimedOut
            <6> QED BY <6>1, <6>2, <6>3, Isa
          <5>3. CASE pending = Request
            <6>1. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ durableBodies' = durableBodies
                  /\ receivedQCs' = receivedQCs
                  /\ prepareQCs' = prepareQCs
                  /\ timeoutIntents' = timeoutIntents
                  /\ installedTCs' = installedTCs
                  /\ prepareIntents' = prepareIntents
                  /\ lockRank' = lockRank
                  /\ lockSubject' = lockSubject
                  /\ highestRank' = highestRank
                  /\ highestSubject' = highestSubject
                  /\ commitIntents' = commitIntents
              BY <1>1 DEF BeginLockCommit
            <6> QED BY <2>3, <5>3, <6>1, Isa
               DEF CurrentOpenPrepareForCommit,
                   HistoricalLockedPrepareForCommit,
                   InstalledTcSelectsPrepareFor,
                   NoHigherPrepareOriginKnown, NodeTimedOut
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>4. UNCHANGED
               <<context, nodeView, durableBodies, receivedQCs,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 installedTCs, lockRank, lockSubject, highestRank,
                 highestSubject, pendingPrepare, pendingTimeout>>
        BY <1>1 DEF BeginLockCommit
      <3>5. \A pending \in pendingPrepare':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Prepare"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.view,
                             pending.vote.subject)
               /\ CanAppendVote(prepareIntents', pending.vote)
               /\ PrepareCarriesHigherSafeQc(pending.vote)'
        BY <3>1, <3>4, Isa
           DEF PendingVoteWritesAuthorized, PrepareCarriesHigherSafeQc
      <3>6. \A pending \in pendingTimeout':
               /\ pending.node \in Honest
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ CanAppendTimeout(timeoutIntents', pending.vote)
               /\ TimeoutVoteProtectsCommitSet(
                    pending.vote, commitIntents)'
        BY <3>1, <3>4, Isa
           DEF PendingVoteWritesAuthorized, TimeoutVoteProtectsCommitSet,
               InstalledTcAuthorizesCommitVote
      <3> QED BY <3>3, <3>5, <3>6
         DEF PendingVoteWritesAuthorized
    <2>7. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, BeginLockCommit,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, QcWireValid, CurrentEpoch
    <2>8. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingCertificateWritesAuthorized'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
            /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        <4>1. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, HonestVoteUnique,
                 HonestTimeoutUnique, IntentPhasesCorrect
        <4>2. PendingCertificateWritesAuthorized'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, PendingCertificateWritesAuthorized,
                 TCValid, AuthenticatedHighRef, HighRefValid, CurrentEpoch, CurrentVoters
        <4>3. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, HonestVoteTransportBacked,
                 QcTransportBacked, HonestTimeoutTransportBacked,
                 TcTransportBacked, VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4>4. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
              /\ DurableTimeoutsProtectCommits'
              /\ HighestAndLockAreCertified'
          <5>1. /\ CertificatesBackedByIntents'
                /\ HonestDurableIntentsSound'
                /\ FormedTimeoutCertificatesSound'
            BY <1>1, Isa
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   BeginLockCommit, CertificatesBackedByIntents,
                   HonestDurableIntentsSound,
                   FormedTimeoutCertificatesSound
          <5>2. UNCHANGED
                   <<timeoutIntents, commitIntents, installedTCs>>
            BY <1>1 DEF BeginLockCommit
          <5>3. DurableTimeoutsProtectCommits
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant
          <5>4. DurableTimeoutsProtectCommits'
            BY <5>2, <5>3,
               UnchangedDurableTimeoutProtectionVarsPreserves
          <5>5. UNCHANGED
                   <<context, prepareQCs, lockRank, lockSubject,
                     highestRank, highestSubject>>
            BY <1>1 DEF BeginLockCommit
          <5>6. HighestAndLockAreCertified
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant
          <5>7. HighestAndLockAreCertified'
            BY <5>5, <5>6,
               UnchangedHighestAndLockCertificationVarsPreserves
          <5> QED BY <5>1, <5>4, <5>7
        <4>5. DurableLockRecoveryProvenanceInvariant'
          BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5
      <3> QED BY <2>6, <3>1 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>7, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginLockCommit, LineageVars
  <1> QED BY <1>1

(***************************************************************************
The remaining certificate and WAL acknowledgements are proved here instead
of being hidden behind the top-level Next disjunction.  In particular, the
pending-request invariant retains every admission guard needed after an
arbitrary number of unrelated asynchronous transitions.
***************************************************************************)

THEOREM PersistLockCommitPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistLockCommit(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistLockCommit(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request \in pendingLockCommit
          /\ request.node \in Honest
          /\ request.qc \in prepareQCs
          /\ request.qc.phase = "Prepare"
          /\ request.vote =
               Vote(context, request.qc.view, "Commit",
                    request.qc.subject, request.node)
          /\ request.vote.phase = "Commit"
          /\ request.vote.signer = request.node
          /\ request.vote.context = context
          /\ request.vote.context = request.qc.context
          /\ request.vote.view = request.qc.view
          /\ request.vote.subject = request.qc.subject
          /\ \/ CurrentOpenPrepareForCommit(request.node, request.qc)
             \/ HistoricalLockedPrepareForCommit(
                  request.node, request.qc)
          /\ request.vote.view <= nodeView[request.node]
          /\ request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, request.node,
                        request.vote.context, request.vote.view, request.vote.subject)
          /\ request.qc.view >= lockRank[request.node]
          /\ (request.qc.view = lockRank[request.node]
                => request.qc.subject = lockSubject[request.node])
          /\ CanAppendVote(commitIntents, request.vote)
      <3>1. /\ TypeInvariant
            /\ PendingVoteWritesAuthorized
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant
      <3>2. request \in pendingLockCommit
        BY <1>1 DEF PersistLockCommit
      <3>3. /\ request.node \in Honest
            /\ request.qc \in prepareQCs
            /\ request.qc.phase = "Prepare"
            /\ request.vote =
                 Vote(context, request.qc.view, "Commit",
                      request.qc.subject, request.node)
            /\ request.vote.phase = "Commit"
            /\ request.vote.signer = request.node
        BY <3>1, <3>2 DEF PendingVoteWritesAuthorized
      <3>4. /\ request.vote.context = context
            /\ request.vote.context = request.qc.context
            /\ request.vote.view = request.qc.view
            /\ request.vote.subject = request.qc.subject
            /\ request.vote.subject \in ValidSubjects
            /\ BodyHeldBy(durableBodies, request.node,
                          request.vote.context, request.vote.view,
                          request.vote.subject)
            /\ request.qc.view >= lockRank[request.node]
            /\ (request.qc.view = lockRank[request.node]
                  => request.qc.subject = lockSubject[request.node])
            /\ CanAppendVote(commitIntents, request.vote)
        BY <3>1, <3>2 DEF PendingVoteWritesAuthorized
      <3>5. /\ \/ CurrentOpenPrepareForCommit(
                         request.node, request.qc)
                   \/ HistoricalLockedPrepareForCommit(
                         request.node, request.qc)
            /\ request.qc.view <= nodeView[request.node]
        BY <3>1, <3>2, PendingLockCommitAuthorizationFacts
      <3>6. request.vote.view <= nodeView[request.node]
        BY <3>4, <3>5
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6
    <2>2. HonestVoteUnique(commitIntents')
      BY <1>1, <2>1, DurableVoteAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistLockCommit
    <2>3. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistLockCommit
    <2>4. TypeInvariant'
      <3>1. /\ request.node \in ValidatorIds
            /\ request.vote \in VoteRecordSet
            /\ request.qc.view \in Views
            /\ request.qc.subject \in Subjects
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. /\ QuorumConfiguration
              /\ ValidSubjects \subseteq Subjects
          BY <4>1 DEF TypeInvariant, ModelConfiguration
        <4>3. request.node \in ValidatorIds
          BY <2>1, <4>2 DEF QuorumConfiguration
        <4>4. request.vote \in VoteRecordSet
          <5>1. pendingLockCommit \subseteq LockCommitWalSet
            BY <4>1 DEF TypeInvariant
          <5>2. request \in pendingLockCommit
            BY <1>1 DEF PersistLockCommit
          <5>3. request \in LockCommitWalSet
            BY <5>1, <5>2
          <5> QED BY <5>3 DEF LockCommitWalSet
        <4>5. request.qc \in QcRecordSet
          BY <2>1, <4>1 DEF TypeInvariant
        <4>6. request.qc.view \in Views
          BY <4>5 DEF QcRecordSet
        <4>7. request.qc.subject \in Subjects
          BY <2>1, <4>2 DEF QcRecordSet
        <4> QED BY <4>3, <4>4, <4>6, <4>7
      <3>2. /\ ModelConfiguration'
            /\ height' \in Heights
            /\ context' \in ContextRecords
            /\ context'.height = height'
            /\ contextHistory' \subseteq ContextRecords
            /\ context' \in contextHistory'
            /\ nodeView' \in [ValidatorIds -> Views]
            /\ generation' \in [ValidatorIds -> Generations]
            /\ up' \subseteq ValidatorIds
            /\ gst' \in BOOLEAN
            /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
            /\ proposalIntents' \subseteq ProposalRecordSet
            /\ prepareIntents' \subseteq VoteRecordSet
            /\ timeoutIntents' \subseteq TimeoutVoteRecordSet
            /\ prepareQCs' \subseteq QcRecordSet
            /\ commitQCs' \subseteq QcRecordSet
            /\ \A tc \in formedTCs': TcWellTyped(tc)
            /\ \A entry \in receivedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ \A entry \in installedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ pendingProposal' \subseteq ProposalWalSet
            /\ pendingPrepare' \subseteq PrepareWalSet
            /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
            /\ pendingTimeout' \subseteq TimeoutWalSet
            /\ pendingInstallTC' \subseteq InstallTcWalSet
            /\ pendingDecision' \subseteq DecisionWalSet
            /\ signProposals' \subseteq ProposalSignSet
            /\ signTimeouts' \subseteq TimeoutSignSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>3. commitIntents' \subseteq VoteRecordSet
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>4. /\ lockRank' \in [ValidatorIds -> Ranks]
            /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
            /\ highestRank' \in [ValidatorIds -> Ranks]
            /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
        <4>1. /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. /\ request.qc.view \in Ranks
              /\ request.qc.subject \in SubjectOrNone
              /\ highestRank[request.node] \in Ranks
              /\ highestSubject[request.node] \in SubjectOrNone
          <5>1. request.qc.view \in Ranks
            BY <3>1, ViewsAreRanks
          <5>2. request.qc.subject \in SubjectOrNone
            BY <3>1, SubjectsAreSubjectOrNone
          <5>3. highestRank[request.node] \in Ranks
            BY <3>1, <4>1, FunctionValueHasCodomain
          <5>4. highestSubject[request.node] \in SubjectOrNone
            BY <3>1, <4>1, FunctionValueHasCodomain
          <5> QED BY <5>1, <5>2, <5>3, <5>4
        <4> DEFINE NextHighestRank ==
             IF request.qc.view > highestRank[request.node]
             THEN request.qc.view ELSE highestRank[request.node]
        <4> DEFINE NextHighestSubject ==
             IF request.qc.view > highestRank[request.node]
             THEN request.qc.subject ELSE highestSubject[request.node]
        <4>3. /\ NextHighestRank \in Ranks
              /\ NextHighestSubject \in SubjectOrNone
          BY <4>2 DEF NextHighestRank, NextHighestSubject
        <4>4. /\ lockRank'
                    = [lockRank EXCEPT
                         ![request.node] = request.qc.view]
              /\ lockSubject'
                    = [lockSubject EXCEPT
                         ![request.node] = request.qc.subject]
              /\ highestRank'
                    = [highestRank EXCEPT
                         ![request.node] = NextHighestRank]
              /\ highestSubject'
                    = [highestSubject EXCEPT
                         ![request.node] = NextHighestSubject]
          BY <1>1 DEF PersistLockCommit,
                         NextHighestRank, NextHighestSubject
        <4>5. lockRank' \in [ValidatorIds -> Ranks]
          BY <3>1, <4>1, <4>2, <4>4,
             FunctionalUpdatePreservesType
        <4>6. lockSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>1, <4>1, <4>2, <4>4,
             FunctionalUpdatePreservesType
        <4>7. highestRank' \in [ValidatorIds -> Ranks]
          BY <3>1, <4>1, <4>3, <4>4,
             FunctionalUpdatePreservesType
        <4>8. highestSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>1, <4>1, <4>3, <4>4,
             FunctionalUpdatePreservesType
        <4> QED BY <4>5, <4>6, <4>7, <4>8
      <3>5. pendingLockCommit' \subseteq LockCommitWalSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>6. signVotes' \subseteq VoteSignSet
        <4>1. signVotes \subseteq VoteSignSet
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. VoteSign(request.node, request.vote) \in VoteSignSet
          BY <3>1 DEF VoteSign, VoteSignSet
        <4>3. signVotes'
              = signVotes \cup {VoteSign(request.node, request.vote)}
          BY <1>1 DEF PersistLockCommit
        <4> QED BY <4>1, <4>2, <4>3
      <3>7. /\ availableBodies' \subseteq BodyRecordSet
            /\ durableBodies' \subseteq BodyRecordSet
            /\ retainedLockedBodies'
                   \subseteq RetainedLockedBodyRecordSet
            /\ validatedBodies' \subseteq ValidationRecordSet
            /\ invalidBodies' \subseteq BodyRecordSet
            /\ RetainedLockedBodiesSound(retainedLockedBodies',
                                          durableBodies')
        <4> DEFINE Retained ==
             RetainedLockedBodyRecord(
               request.node, request.qc.context, request.qc.subject)
        <4>1. /\ availableBodies \subseteq BodyRecordSet
              /\ durableBodies \subseteq BodyRecordSet
              /\ retainedLockedBodies
                     \subseteq RetainedLockedBodyRecordSet
              /\ validatedBodies \subseteq ValidationRecordSet
              /\ invalidBodies \subseteq BodyRecordSet
              /\ prepareQCs \subseteq QcRecordSet
              /\ RetainedLockedBodiesSound(retainedLockedBodies,
                                            durableBodies)
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. /\ availableBodies' = availableBodies
              /\ durableBodies' = durableBodies
              /\ validatedBodies' = validatedBodies
              /\ invalidBodies' = invalidBodies
              /\ retainedLockedBodies'
                   = retainedLockedBodies \cup {Retained}
              /\ Retained \in RetainedLockedBodyRecordSet
              /\ BodyHeldBy(durableBodies, request.node,
                            request.qc.context, request.qc.view,
                            request.qc.subject)
          BY <1>1 DEF PersistLockCommit, Retained
        <4>3. request.qc.view \in Views
          BY <2>1, <4>1 DEF QcRecordSet
        <4>4. /\ availableBodies' \subseteq BodyRecordSet
              /\ durableBodies' \subseteq BodyRecordSet
              /\ validatedBodies' \subseteq ValidationRecordSet
              /\ invalidBodies' \subseteq BodyRecordSet
          BY <4>1, <4>2
        <4>5. retainedLockedBodies'
                   \subseteq RetainedLockedBodyRecordSet
          BY <4>1, <4>2, Isa
        <4>6. RetainedLockedBodiesSound(retainedLockedBodies',
                                        durableBodies')
          <5>1. ASSUME NEW retained \in retainedLockedBodies'
                 PROVE \E sourceView \in Views:
                         BodyHeldBy(durableBodies', retained.node,
                                    retained.context, sourceView,
                                    retained.subject)
            <6>1. retained \in retainedLockedBodies
                    \/ retained = Retained
              BY <4>2, <5>1, Isa
            <6>2. CASE retained \in retainedLockedBodies
              <7>1. \E sourceView \in Views:
                       BodyHeldBy(durableBodies, retained.node,
                                  retained.context, sourceView,
                                  retained.subject)
                BY <4>1, <6>2 DEF RetainedLockedBodiesSound
              <7> QED BY <4>2, <7>1
            <6>3. CASE retained = Retained
              <7>1. BodyHeldBy(durableBodies', retained.node,
                               retained.context, request.qc.view,
                               retained.subject)
                BY <4>2, <6>3, Isa
                   DEF Retained, RetainedLockedBodyRecord
              <7> QED BY <4>3, <7>1
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1 DEF RetainedLockedBodiesSound
        <4> QED BY <4>4, <4>5, <4>6
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6, <3>7
         DEF TypeInvariant
    <2>5. LockBelowHighest'
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE lockRank'[node] <= highestRank'[node]
        <4>1. CASE node = request.node
          <5>1. /\ ModelConfiguration
                /\ request.qc.view \in Views
                /\ lockRank \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestRank[node] \in Ranks
            <6>1. /\ ModelConfiguration
                  /\ prepareQCs \subseteq QcRecordSet
                  /\ lockRank \in [ValidatorIds -> Ranks]
                  /\ highestRank \in [ValidatorIds -> Ranks]
              BY <1>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant
            <6>2. request.qc \in QcRecordSet
              BY <2>1, <6>1
            <6>3. request.qc.view \in Views
              BY <6>2 DEF QcRecordSet
            <6>4. highestRank[node] \in Ranks
              BY <3>1, <6>1, FunctionValueHasCodomain
            <6> QED BY <6>1, <6>3, <6>4
          <5>2. /\ request.qc.view \in Int
                /\ highestRank[node] \in Int
            <6>1. Ranks \subseteq Int
              BY <5>1, ModelRanksAreIntegers
            <6>2. request.qc.view \in Ranks
              BY <5>1, ViewsAreRanks
            <6> QED BY <5>1, <6>1, <6>2
          <5>3. /\ lockRank'
                       = [lockRank EXCEPT
                            ![request.node] = request.qc.view]
                /\ highestRank'
                       = [highestRank EXCEPT
                            ![request.node] =
                              IF request.qc.view
                                   > highestRank[request.node]
                              THEN request.qc.view
                              ELSE highestRank[request.node]]
            BY <1>1 DEF PersistLockCommit
          <5>4. /\ lockRank'[node] = request.qc.view
                /\ highestRank'[node]
                     = IF request.qc.view > highestRank[node]
                       THEN request.qc.view ELSE highestRank[node]
            BY <4>1, <5>1, <5>3, Isa
          <5> QED BY <5>2, <5>4, SMT
        <4>2. CASE node # request.node
          <5>1. /\ lockRank'
                       = [lockRank EXCEPT
                            ![request.node] = request.qc.view]
                /\ highestRank'
                       = [highestRank EXCEPT
                            ![request.node] =
                              IF request.qc.view
                                   > highestRank[request.node]
                              THEN request.qc.view
                              ELSE highestRank[request.node]]
            BY <1>1 DEF PersistLockCommit
          <5>2. /\ lockRank \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. /\ lockRank'[node] = lockRank[node]
                /\ highestRank'[node] = highestRank[node]
            BY <4>2, <5>1, <5>2, Isa
          <5>4. lockRank[node] <= highestRank[node]
            BY <1>1, <3>1
               DEF StrongInductiveInvariant, Safety,
                   LockBelowHighest
          <5> QED BY <5>3, <5>4
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF LockBelowHighest
    <2>6. Safety'
      <3>1. IntentPhasesCorrect'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               IntentPhasesCorrect, PersistLockCommit
      <3>2. HonestCommitUniqueness'
        BY <2>2, <3>1, PhasedVoteUniquenessImpliesSlotUniqueness
           DEF HonestCommitUniqueness, IntentPhasesCorrect
      <3>3. PrepareSigningRequiresIntent'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               PrepareSigningRequiresIntent, VoteSign
      <3>4. CommitSigningRequiresIntent'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               CommitSigningRequiresIntent, VoteSign
      <3>5. /\ ProposalSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
            /\ HonestPrepareUniqueness'
            /\ HonestTimeoutUniqueness'
            /\ DecisionAgreement'
            /\ AppliedRequiresDecision'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               ProposalSigningRequiresIntent,
               TimeoutSigningRequiresIntent,
               HonestPrepareUniqueness, HonestTimeoutUniqueness,
               DecisionAgreement, AppliedRequiresDecision
      <3> QED BY <2>3, <2>4, <2>5, <3>2, <3>3, <3>4, <3>5
         DEF Safety
    <2>7. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <2>1, <2>2, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistLockCommit, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect
      <3>2. PendingVoteWritesAuthorized'
        <4>1. PendingVoteWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>2. RequestsUniqueByNode(AllPendingRequests)
          BY <1>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode
        <4>3. \A pending \in pendingPrepare':
                 /\ pending.node \in Honest
                 /\ pending.vote.phase = "Prepare"
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context'
                 /\ pending.vote.view = nodeView'[pending.node]
                 /\ pending.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies', pending.node,
                               pending.vote.context, pending.vote.view, pending.vote.subject)
                 /\ CanAppendVote(prepareIntents', pending.vote)
                 /\ PrepareCarriesHigherSafeQc(pending.vote)'
          <5>1. ASSUME NEW pending \in pendingPrepare'
                 PROVE /\ pending.node \in Honest
                       /\ pending.vote.phase = "Prepare"
                       /\ pending.vote.signer = pending.node
                       /\ pending.vote.context = context'
                       /\ pending.vote.view = nodeView'[pending.node]
                       /\ pending.vote.subject \in ValidSubjects
                       /\ BodyHeldBy(durableBodies', pending.node,
                                     pending.vote.context, pending.vote.view, pending.vote.subject)
                       /\ CanAppendVote(prepareIntents', pending.vote)
                       /\ PrepareCarriesHigherSafeQc(pending.vote)'
            <6>1. pending \in pendingPrepare
              BY <1>1, <5>1 DEF PersistLockCommit
            <6>2. pending # request
              <7>1. /\ pending \in PrepareWalSet
                    /\ request \in LockCommitWalSet
                BY <1>1, <2>1, <6>1
                   DEF StrongInductiveInvariant, Safety, TypeInvariant
              <7> QED BY <7>1, Isa
                 DEF PrepareWalSet, LockCommitWalSet
            <6>3. pending.node # request.node
              <7>1. /\ pending \in AllPendingRequests
                    /\ request \in AllPendingRequests
                BY <1>1, <6>1
                   DEF PersistLockCommit, AllPendingRequests
              <7> QED BY <4>2, <6>2, <7>1,
                           DistinctUniqueRequestsHaveDistinctNodes
            <6>4. /\ pending.node \in Honest
                  /\ pending.vote.phase = "Prepare"
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.view = nodeView[pending.node]
                  /\ pending.vote.subject \in ValidSubjects
                  /\ BodyHeldBy(durableBodies, pending.node,
                                pending.vote.context, pending.vote.view, pending.vote.subject)
                  /\ CanAppendVote(prepareIntents, pending.vote)
                  /\ PrepareCarriesHigherSafeQc(pending.vote)
              BY <4>1, <6>1 DEF PendingVoteWritesAuthorized
            <6>5. request.vote.signer # pending.vote.signer
              BY <2>1, <6>3, <6>4
            <6>6. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ durableBodies' = durableBodies
                  /\ prepareIntents' = prepareIntents
                  /\ prepareQCs' = prepareQCs
                  /\ commitIntents'
                       = commitIntents \cup {request.vote}
              BY <1>1 DEF PersistLockCommit
            <6>7. PrepareCarriesHigherSafeQc(pending.vote)'
              BY <6>4, <6>5, <6>6, Isa
                 DEF PrepareCarriesHigherSafeQc
            <6> QED BY <6>4, <6>6, <6>7
          <5> QED BY <5>1
        <4>4. \A pending \in pendingLockCommit':
                 /\ pending.node \in Honest
                 /\ pending.vote.phase = "Commit"
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context'
                 /\ pending.vote.context = pending.qc.context
                 /\ pending.vote.view = pending.qc.view
                 /\ pending.vote.subject = pending.qc.subject
                 /\ pending.vote =
                      Vote(context', pending.qc.view, "Commit",
                           pending.qc.subject, pending.node)
                 /\ pending.qc.phase = "Prepare"
                 /\ pending.qc \in prepareQCs'
                 /\ \/ CurrentOpenPrepareForCommit(
                          pending.node, pending.qc)'
                    \/ HistoricalLockedPrepareForCommit(
                          pending.node, pending.qc)'
                 /\ pending.vote.subject \in ValidSubjects
                 /\ BodyHeldBy(durableBodies', pending.node,
                               pending.vote.context, pending.vote.view, pending.vote.subject)
                 /\ pending.qc.view >= lockRank'[pending.node]
                 /\ (pending.qc.view = lockRank'[pending.node]
                       => pending.qc.subject =
                            lockSubject'[pending.node])
                 /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. ASSUME NEW pending \in pendingLockCommit'
                 PROVE /\ pending.node \in Honest
                       /\ pending.vote.phase = "Commit"
                       /\ pending.vote.signer = pending.node
                       /\ pending.vote.context = context'
                       /\ pending.vote.context = pending.qc.context
                       /\ pending.vote.view = pending.qc.view
                       /\ pending.vote.subject = pending.qc.subject
                       /\ pending.vote =
                            Vote(context', pending.qc.view, "Commit",
                                 pending.qc.subject, pending.node)
                       /\ pending.qc.phase = "Prepare"
                       /\ pending.qc \in prepareQCs'
                       /\ \/ CurrentOpenPrepareForCommit(
                                pending.node, pending.qc)'
                          \/ HistoricalLockedPrepareForCommit(
                                pending.node, pending.qc)'
                       /\ pending.vote.subject \in ValidSubjects
                       /\ BodyHeldBy(durableBodies', pending.node,
                                     pending.vote.context, pending.vote.view, pending.vote.subject)
                       /\ pending.qc.view >= lockRank'[pending.node]
                       /\ (pending.qc.view = lockRank'[pending.node]
                             => pending.qc.subject =
                                  lockSubject'[pending.node])
                       /\ CanAppendVote(commitIntents', pending.vote)
            <6>1. /\ pending \in pendingLockCommit
                  /\ pending # request
              BY <1>1, <5>1 DEF PersistLockCommit
            <6>2. pending.node # request.node
              <7>1. /\ pending \in AllPendingRequests
                    /\ request \in AllPendingRequests
                BY <1>1, <6>1
                   DEF PersistLockCommit, AllPendingRequests
              <7> QED BY <4>2, <6>1, <7>1,
                           DistinctUniqueRequestsHaveDistinctNodes
            <6>3. /\ pending.node \in Honest
                  /\ pending.vote.phase = "Commit"
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.context = pending.qc.context
                  /\ pending.vote.view = pending.qc.view
                  /\ pending.vote.subject = pending.qc.subject
                  /\ pending.vote =
                       Vote(context, pending.qc.view, "Commit",
                            pending.qc.subject, pending.node)
                  /\ pending.qc.phase = "Prepare"
                  /\ pending.qc \in prepareQCs
                  /\ \/ CurrentOpenPrepareForCommit(
                           pending.node, pending.qc)
                     \/ HistoricalLockedPrepareForCommit(
                           pending.node, pending.qc)
                  /\ pending.vote.subject \in ValidSubjects
                  /\ BodyHeldBy(durableBodies, pending.node,
                                pending.vote.context, pending.vote.view, pending.vote.subject)
                  /\ pending.qc.view >= lockRank[pending.node]
                  /\ (pending.qc.view = lockRank[pending.node]
                        => pending.qc.subject =
                             lockSubject[pending.node])
                  /\ CanAppendVote(commitIntents, pending.vote)
              BY <4>1, <6>1 DEF PendingVoteWritesAuthorized
            <6>4. /\ pending.node \in ValidatorIds
                  /\ request.node \in ValidatorIds
                  /\ lockRank \in [ValidatorIds -> Ranks]
                  /\ lockSubject \in
                       [ValidatorIds -> SubjectOrNone]
                  /\ highestRank \in [ValidatorIds -> Ranks]
                  /\ highestSubject \in
                       [ValidatorIds -> SubjectOrNone]
              BY <1>1, <2>1, <6>1, <6>3
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     ModelConfiguration, QuorumConfiguration
            <6>5. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ durableBodies' = durableBodies
                  /\ receivedQCs' = receivedQCs
                  /\ prepareQCs' = prepareQCs
                  /\ timeoutIntents' = timeoutIntents
                  /\ installedTCs' = installedTCs
                  /\ prepareIntents' = prepareIntents
                  /\ commitIntents'
                       = commitIntents \cup {request.vote}
                  /\ lockRank'[pending.node] = lockRank[pending.node]
                  /\ lockSubject'[pending.node]
                       = lockSubject[pending.node]
                  /\ highestRank'[pending.node]
                       = highestRank[pending.node]
                  /\ highestSubject'[pending.node]
                       = highestSubject[pending.node]
              <7>1. /\ context' = context
                    /\ nodeView' = nodeView
                    /\ durableBodies' = durableBodies
                    /\ receivedQCs' = receivedQCs
                    /\ prepareQCs' = prepareQCs
                    /\ timeoutIntents' = timeoutIntents
                    /\ installedTCs' = installedTCs
                    /\ prepareIntents' = prepareIntents
                    /\ commitIntents'
                         = commitIntents \cup {request.vote}
                    /\ lockRank'
                         = [lockRank EXCEPT
                              ![request.node] = request.qc.view]
                    /\ lockSubject'
                         = [lockSubject EXCEPT
                              ![request.node] = request.qc.subject]
                    /\ highestRank'
                         = [highestRank EXCEPT
                              ![request.node] =
                                IF request.qc.view
                                     > highestRank[request.node]
                                THEN request.qc.view
                                ELSE highestRank[request.node]]
                    /\ highestSubject'
                         = [highestSubject EXCEPT
                              ![request.node] =
                                IF request.qc.view
                                     > highestRank[request.node]
                                THEN request.qc.subject
                                ELSE highestSubject[request.node]]
                BY <1>1 DEF PersistLockCommit
              <7>2. lockRank'[pending.node]
                       = lockRank[pending.node]
                BY <6>2, <6>4, <7>1, Isa
              <7>3. lockSubject'[pending.node]
                       = lockSubject[pending.node]
                BY <6>2, <6>4, <7>1, Isa
              <7>4. highestRank'[pending.node]
                       = highestRank[pending.node]
                BY <6>2, <6>4, <7>1, Isa
              <7>5. highestSubject'[pending.node]
                       = highestSubject[pending.node]
                BY <6>2, <6>4, <7>1, Isa
              <7> QED BY <7>1, <7>2, <7>3, <7>4, <7>5
            <6>6. request.vote.signer # pending.vote.signer
              BY <2>1, <6>2, <6>3
            <6>7. CanAppendVote(commitIntents', pending.vote)
              BY <6>3, <6>5, <6>6,
                 DistinctSignerAppendPreservesCanAppendVote
            <6>8. /\ (CurrentOpenPrepareForCommit(
                            pending.node, pending.qc)'
                          <=> CurrentOpenPrepareForCommit(
                                pending.node, pending.qc))
                   /\ (HistoricalLockedPrepareForCommit(
                            pending.node, pending.qc)'
                          <=> HistoricalLockedPrepareForCommit(
                                pending.node, pending.qc))
              BY <6>5
                 DEF CurrentOpenPrepareForCommit,
                     HistoricalLockedPrepareForCommit,
                     InstalledTcSelectsPrepareFor,
                     NoHigherPrepareOriginKnown, NodeTimedOut
            <6> QED BY <6>3, <6>5, <6>7, <6>8
          <5> QED BY <5>1
        <4>5. \A pending \in pendingTimeout':
                 /\ pending.node \in Honest
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context'
                 /\ pending.vote.view = nodeView'[pending.node]
                 /\ CanAppendTimeout(timeoutIntents', pending.vote)
                 /\ TimeoutVoteProtectsCommitSet(
                      pending.vote, commitIntents)'
          <5>1. ASSUME NEW pending \in pendingTimeout'
                 PROVE /\ pending.node \in Honest
                       /\ pending.vote.signer = pending.node
                       /\ pending.vote.context = context'
                       /\ pending.vote.view = nodeView'[pending.node]
                       /\ CanAppendTimeout(
                            timeoutIntents', pending.vote)
                       /\ TimeoutVoteProtectsCommitSet(
                            pending.vote, commitIntents)'
            <6>1. pending \in pendingTimeout
              BY <1>1, <5>1 DEF PersistLockCommit
            <6>2. pending # request
              <7>1. /\ pending \in TimeoutWalSet
                    /\ request \in LockCommitWalSet
                BY <1>1, <2>1, <6>1
                   DEF StrongInductiveInvariant, Safety, TypeInvariant
              <7> QED BY <7>1, Isa
                 DEF TimeoutWalSet, LockCommitWalSet
            <6>3. pending.node # request.node
              <7>1. /\ pending \in AllPendingRequests
                    /\ request \in AllPendingRequests
                BY <1>1, <6>1
                   DEF PersistLockCommit, AllPendingRequests
              <7> QED BY <4>2, <6>2, <7>1,
                           DistinctUniqueRequestsHaveDistinctNodes
            <6>4. /\ pending.node \in Honest
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.view = nodeView[pending.node]
                  /\ CanAppendTimeout(timeoutIntents, pending.vote)
                  /\ TimeoutVoteProtectsCommitSet(
                       pending.vote, commitIntents)
              BY <4>1, <6>1 DEF PendingVoteWritesAuthorized
            <6>5. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ timeoutIntents' = timeoutIntents
                  /\ installedTCs' = installedTCs
                  /\ commitIntents'
                       = commitIntents \cup {request.vote}
              BY <1>1 DEF PersistLockCommit
            <6>6. request.vote.signer # pending.vote.signer
              BY <2>1, <6>3, <6>4
            <6>7. TimeoutVoteProtectsCommitSet(
                     pending.vote, commitIntents)'
              BY <6>4, <6>5, <6>6, Isa
                 DEF TimeoutVoteProtectsCommitSet,
                     InstalledTcAuthorizesCommitVote
            <6> QED BY <6>4, <6>5, <6>7
          <5> QED BY <5>1
        <4> QED BY <4>3, <4>4, <4>5
           DEF PendingVoteWritesAuthorized, NodeTimedOut
      <3>3. PendingCertificateWritesAuthorized'
        <4>1. RequestsUniqueByNode(AllPendingRequests)
          BY <1>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode
        <4>2. \A pending \in pendingObservePrepare':
                 /\ pending.qc \in prepareQCs'
                 /\ pending.qc.context = context'
                 /\ pending.qc.view > highestRank'[pending.node]
          <5>1. ASSUME NEW pending \in pendingObservePrepare'
                 PROVE /\ pending.qc \in prepareQCs'
                       /\ pending.qc.context = context'
                       /\ pending.qc.view >
                            highestRank'[pending.node]
            <6>1. pending \in pendingObservePrepare
              BY <1>1, <5>1 DEF PersistLockCommit
            <6>2. pending # request
              <7>1. /\ pending \in ObservePrepareWalSet
                    /\ request \in LockCommitWalSet
                BY <1>1, <2>1, <6>1
                   DEF StrongInductiveInvariant, Safety, TypeInvariant
              <7> QED BY <7>1, Isa
                 DEF ObservePrepareWalSet, LockCommitWalSet
            <6>3. pending.node # request.node
              <7>1. /\ pending \in AllPendingRequests
                    /\ request \in AllPendingRequests
                BY <1>1, <6>1
                   DEF PersistLockCommit, AllPendingRequests
              <7> QED BY <4>1, <6>2, <7>1,
                           DistinctUniqueRequestsHaveDistinctNodes
            <6>4. /\ pending.node \in ValidatorIds
                  /\ request.node \in ValidatorIds
                  /\ highestRank \in [ValidatorIds -> Ranks]
              BY <1>1, <2>1, <6>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     ModelConfiguration, QuorumConfiguration,
                     ObservePrepareWalSet
            <6>5. /\ prepareQCs' = prepareQCs
                  /\ context' = context
                  /\ highestRank'[pending.node]
                       = highestRank[pending.node]
              <7>1. /\ prepareQCs' = prepareQCs
                    /\ context' = context
                    /\ highestRank'
                         = [highestRank EXCEPT
                              ![request.node] =
                                IF request.qc.view
                                     > highestRank[request.node]
                                THEN request.qc.view
                                ELSE highestRank[request.node]]
                BY <1>1 DEF PersistLockCommit
              <7>2. highestRank'[pending.node]
                       = highestRank[pending.node]
                BY <6>3, <6>4, <7>1, Isa
              <7> QED BY <7>1, <7>2
            <6>6. /\ pending.qc \in prepareQCs
                  /\ pending.qc.context = context
                  /\ pending.qc.view > highestRank[pending.node]
              BY <1>1, <6>1
                 DEF StrongInductiveInvariant,
                     ReducerProvenanceInvariant,
                     PendingCertificateWritesAuthorized
            <6> QED BY <6>5, <6>6
          <5> QED BY <5>1
        <4>3. /\ \A pending \in pendingInstallTC':
                       /\ pending.tc \in formedTCs'
                       /\ pending.tc.context = context'
                       /\ TCValid(pending.tc)'
                       /\ pending.tc.votes # {}
                       /\ pending.tc.view + 1 \in Views
                       /\ pending.tc.view + 1 >= nodeView'[pending.node]
              /\ \A pending \in pendingDecision':
                       /\ pending.qc \in commitQCs'
                       /\ pending.qc.context = context'
                       /\ pending.qc.phase = "Commit"
                       /\ pending.qc.height = height'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PersistLockCommit,
                 PendingCertificateWritesAuthorized,
                 TCValid, AuthenticatedHighRef, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4> QED BY <4>2, <4>3
           DEF PendingCertificateWritesAuthorized
      <3>4. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, PersistLockCommitOnlyPrunesVoteReceipts, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistLockCommit, HonestVoteTransportBacked,
               QcTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid,
               AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>5. CertificatesBackedByIntents'
        <4>1. commitIntents \subseteq commitIntents'
          BY <1>1 DEF PersistLockCommit
        <4>2. /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs
              /\ prepareIntents' = prepareIntents
          BY <1>1 DEF PersistLockCommit
        <4>3. \A qc \in prepareQCs':
                 /\ HistoricalQcValid(qc)
                 /\ CertificateBackedBy(qc.context.epoch, qc,
                                        prepareIntents')
          BY <1>1, <4>2
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents
        <4>4. \A qc \in commitQCs':
                 /\ HistoricalQcValid(qc)
                 /\ CertificateBackedBy(qc.context.epoch, qc,
                                        commitIntents')
          BY <1>1, <4>1, <4>2, CertificateBackingIsMonotone
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents
        <4> QED BY <4>3, <4>4 DEF CertificatesBackedByIntents
      <3>6. HonestDurableIntentsSound'
        BY <1>1, <2>1, HonestIntentSoundAppend
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestDurableIntentsSound, PersistLockCommit
      <3>7. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistLockCommit, FormedTimeoutCertificatesSound
      <3>8. DurableTimeoutsProtectCommits'
        <4>1. TimeoutIntentProtectsCommits(
                 timeoutIntents, commitIntents)
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 DurableTimeoutsProtectCommits
        <4>2. /\ timeoutIntents' = timeoutIntents
              /\ installedTCs' = installedTCs
              /\ commitIntents'
                   = commitIntents \cup {request.vote}
          BY <1>1 DEF PersistLockCommit
        <4>3. ASSUME NEW timeoutVote \in timeoutIntents'
               PROVE TimeoutVoteProtectsCommitSet(
                       timeoutVote, commitIntents)'
          <5>1. /\ timeoutVote \in timeoutIntents
                /\ TimeoutVoteProtectsCommitSet(
                     timeoutVote, commitIntents)
            BY <4>1, <4>2, <4>3
               DEF TimeoutIntentProtectsCommits
          <5>2. TimeoutVoteProtectsCommitSet(
                   timeoutVote, {request.vote})
            <6>1. ASSUME NEW commitVote \in {request.vote},
                          /\ timeoutVote.signer \in Honest
                          /\ commitVote.signer = timeoutVote.signer
                          /\ commitVote.context = timeoutVote.context
                          /\ commitVote.phase = "Commit"
                          /\ commitVote.view <= timeoutVote.view
                   PROVE \/ TimeoutVoteStrictlyProtectsCommit(
                                timeoutVote, commitVote)
                         \/ InstalledTcAuthorizesCommitVote(commitVote)
              <7>1. /\ commitVote = request.vote
                    /\ timeoutVote.signer = request.node
                    /\ timeoutVote.context = context
                    /\ request.vote.view <= timeoutVote.view
                BY <2>1, <6>1
              <7>2. \/ CurrentOpenPrepareForCommit(
                           request.node, request.qc)
                     \/ HistoricalLockedPrepareForCommit(
                           request.node, request.qc)
                BY <2>1
              <7>3. CASE HistoricalLockedPrepareForCommit(
                            request.node, request.qc)
                <8>1. InstalledTcAuthorizesCommitVote(request.vote)
                  BY <1>1, <2>1, <7>3,
                     HistoricalPendingLockCommitHasInstalledTcAuthorization
                     DEF StrongInductiveInvariant,
                         ReducerProvenanceInvariant
                <8>2. InstalledTcAuthorizesCommitVote(commitVote)
                  BY <7>1, <8>1
                <8> QED BY <8>2
              <7>4. CASE CurrentOpenPrepareForCommit(
                            request.node, request.qc)
                <8>1. timeoutVote.view <=
                         nodeView[timeoutVote.signer]
                  BY <1>1, <5>1, <6>1, <7>1
                     DEF StrongInductiveInvariant, LineageInvariant,
                         CurrentIntentViewsBound
                <8>2. /\ ModelConfiguration
                      /\ timeoutVote.view \in Views
                      /\ request.vote.view \in Views
                  <9>1. /\ TypeInvariant
                        /\ ModelConfiguration
                        /\ timeoutIntents
                             \subseteq TimeoutVoteRecordSet
                        /\ prepareQCs \subseteq QcRecordSet
                    BY <1>1
                       DEF StrongInductiveInvariant, Safety,
                           TypeInvariant
                  <9>2. timeoutVote \in TimeoutVoteRecordSet
                    BY <5>1, <9>1
                  <9>3. timeoutVote.view \in Views
                    BY <9>2 DEF TimeoutVoteRecordSet
                  <9>4. request.qc \in QcRecordSet
                    BY <2>1, <9>1
                  <9>5. request.vote.view \in Views
                    BY <2>1, <9>4 DEF QcRecordSet
                  <9> QED BY <9>1, <9>3, <9>5
                <8>3. /\ timeoutVote.view \in Int
                      /\ request.vote.view \in Int
                  <9>1. Ranks \subseteq Int
                    BY <8>2, ModelRanksAreIntegers
                  <9>2. Views \subseteq Ranks
                    BY ViewsAreRanks
                  <9> QED BY <8>2, <9>1, <9>2
                <8>4. timeoutVote.view = request.vote.view
                  BY <2>1, <7>1, <7>4, <8>1, <8>3, SMT
                     DEF CurrentOpenPrepareForCommit
                <8>5. NodeTimedOut(
                         request.node, request.vote.view)
                  BY <5>1, <7>1, <8>4 DEF NodeTimedOut
                <8>6. request.qc.view = request.vote.view
                  BY <2>1
                <8>7. NodeTimedOut(
                         request.node, request.qc.view)
                  BY <8>5, <8>6
                <8>8. FALSE
                  BY <7>4, <8>7 DEF CurrentOpenPrepareForCommit
                <8> QED BY <8>8
              <7> QED BY <7>2, <7>3, <7>4
            <6> QED BY <6>1 DEF TimeoutVoteProtectsCommitSet
          <5>3. TimeoutVoteProtectsCommitSet(
                   timeoutVote,
                   commitIntents \cup {request.vote})
            BY <5>1, <5>2, Isa DEF TimeoutVoteProtectsCommitSet
          <5>4. TimeoutVoteProtectsCommitSet(
                   timeoutVote, commitIntents)'
            BY <4>2, <5>3, Isa
               DEF TimeoutVoteProtectsCommitSet,
                   InstalledTcAuthorizesCommitVote
          <5> QED BY <5>4
        <4> QED BY <4>3
           DEF DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits
      <3>9. HistoricalTcLockedCommitAuthorizationInvariant'
        <4>1. \A pending \in pendingLockCommit':
                 pending.vote.view < nodeView'[pending.node]
                   => HistoricalLockedPrepareForCommit(
                        pending.node, pending.qc)'
          BY <3>2, SMT
             DEF PendingVoteWritesAuthorized,
                 CurrentOpenPrepareForCommit
        <4>2. \A timeoutVote \in timeoutIntents',
                    commitVote \in commitIntents':
                 (/\ timeoutVote.signer \in Honest
                  /\ commitVote.signer = timeoutVote.signer
                  /\ commitVote.context = timeoutVote.context
                  /\ commitVote.phase = "Commit"
                  /\ commitVote.view <= timeoutVote.view
                  /\ ~TimeoutVoteStrictlyProtectsCommit(
                         timeoutVote, commitVote))
                   => InstalledTcAuthorizesCommitVote(commitVote)'
          BY <3>8, SMT
             DEF DurableTimeoutsProtectCommits,
                 TimeoutIntentProtectsCommits,
                 TimeoutVoteProtectsCommitSet
        <4> QED BY <4>1, <4>2
           DEF HistoricalTcLockedCommitAuthorizationInvariant
      <3>10. HighestAndLockAreCertified'
        <4>1. /\ context' = context
              /\ prepareQCs' = prepareQCs
              /\ lockRank'
                   = [lockRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ lockSubject'
                   = [lockSubject EXCEPT
                        ![request.node] = request.qc.subject]
              /\ highestRank'
                   = [highestRank EXCEPT
                        ![request.node] =
                          IF request.qc.view
                               > highestRank[request.node]
                          THEN request.qc.view
                          ELSE highestRank[request.node]]
              /\ highestSubject'
                   = [highestSubject EXCEPT
                        ![request.node] =
                          IF request.qc.view
                               > highestRank[request.node]
                          THEN request.qc.subject
                          ELSE highestSubject[request.node]]
          BY <1>1 DEF PersistLockCommit
        <4>2. /\ request.node \in ValidatorIds
              /\ request.qc.view # NoRank
              /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in
                   [ValidatorIds -> SubjectOrNone]
          <5>1. /\ TypeInvariant
                /\ ModelConfiguration
                /\ prepareQCs \subseteq QcRecordSet
                /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockSubject \in
                     [ValidatorIds -> SubjectOrNone]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestSubject \in
                     [ValidatorIds -> SubjectOrNone]
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. request.node \in ValidatorIds
            BY <2>1, <5>1
               DEF TypeInvariant, ModelConfiguration,
                   QuorumConfiguration
          <5>3. request.qc.view \in Views
            BY <2>1, <5>1 DEF QcRecordSet
          <5>4. request.qc.view # NoRank
            BY <5>1, <5>3, ViewIsNotNoRank
          <5> QED BY <5>1, <5>2, <5>4
        <4>3. HighestAndLockAreCertified
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>4. ASSUME NEW node \in ValidatorIds
               PROVE /\ (highestRank'[node] = NoRank
                            => highestSubject'[node] = NoSubject)
                     /\ (highestRank'[node] # NoRank
                            => \E qc \in prepareQCs':
                                 /\ qc.context = context'
                                 /\ qc.view = highestRank'[node]
                                 /\ qc.subject =
                                      highestSubject'[node])
                     /\ (lockRank'[node] = NoRank
                            => lockSubject'[node] = NoSubject)
                     /\ (lockRank'[node] # NoRank
                            => \E qc \in prepareQCs':
                                 /\ qc.context = context'
                                 /\ qc.view = lockRank'[node]
                                 /\ qc.subject = lockSubject'[node])
          <5>1. CASE node = request.node
            <6>1. /\ lockRank'[node] = request.qc.view
                  /\ lockSubject'[node] = request.qc.subject
                  /\ highestRank'[node]
                       = IF request.qc.view > highestRank[node]
                         THEN request.qc.view ELSE highestRank[node]
                  /\ highestSubject'[node]
                       = IF request.qc.view > highestRank[node]
                         THEN request.qc.subject
                         ELSE highestSubject[node]
              BY <4>1, <4>2, <5>1, Isa
            <6>2. /\ request.qc \in prepareQCs'
                  /\ request.qc.context = context'
                  /\ request.qc.view = lockRank'[node]
                  /\ request.qc.subject = lockSubject'[node]
              BY <2>1, <4>1, <6>1
            <6>3. /\ (highestRank[node] = NoRank
                          => highestSubject[node] = NoSubject)
                  /\ (highestRank[node] # NoRank
                          => \E qc \in prepareQCs:
                               /\ qc.context = context
                               /\ qc.view = highestRank[node]
                               /\ qc.subject = highestSubject[node])
              BY <4>3, <4>4
                 DEF HighestAndLockAreCertified
            <6>4. /\ (lockRank'[node] = NoRank
                          => lockSubject'[node] = NoSubject)
                  /\ (lockRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = lockRank'[node]
                               /\ qc.subject = lockSubject'[node])
              <7>1. lockRank'[node] # NoRank
                BY <4>2, <6>1
              <7> QED BY <6>2, <7>1
            <6>5. /\ (highestRank'[node] = NoRank
                          => highestSubject'[node] = NoSubject)
                  /\ (highestRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = highestRank'[node]
                               /\ qc.subject = highestSubject'[node])
              <7>1. CASE request.qc.view > highestRank[node]
                <8>1. /\ highestRank'[node] = request.qc.view
                      /\ highestSubject'[node] = request.qc.subject
                  BY <6>1, <7>1
                <8>2. highestRank'[node] # NoRank
                  BY <4>2, <8>1
                <8> QED BY <6>2, <8>1, <8>2
              <7>2. CASE ~(request.qc.view > highestRank[node])
                <8>1. /\ highestRank'[node] = highestRank[node]
                      /\ highestSubject'[node] = highestSubject[node]
                  BY <6>1, <7>2
                <8> QED BY <4>1, <6>3, <8>1
              <7> QED BY <7>1, <7>2
            <6> QED BY <6>4, <6>5
          <5>2. CASE node # request.node
            <6>1. /\ highestRank'[node] = highestRank[node]
                  /\ highestSubject'[node] = highestSubject[node]
                  /\ lockRank'[node] = lockRank[node]
                  /\ lockSubject'[node] = lockSubject[node]
              BY <4>1, <4>2, <4>4, <5>2, Isa
            <6>2. /\ (highestRank[node] = NoRank
                          => highestSubject[node] = NoSubject)
                  /\ (highestRank[node] # NoRank
                          => \E qc \in prepareQCs:
                               /\ qc.context = context
                               /\ qc.view = highestRank[node]
                               /\ qc.subject = highestSubject[node])
                  /\ (lockRank[node] = NoRank
                          => lockSubject[node] = NoSubject)
                  /\ (lockRank[node] # NoRank
                          => \E qc \in prepareQCs:
                               /\ qc.context = context
                               /\ qc.view = lockRank[node]
                               /\ qc.subject = lockSubject[node])
              BY <4>3, <4>4 DEF HighestAndLockAreCertified
            <6> QED BY <4>1, <6>1, <6>2
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>4 DEF HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5,
                  <3>6, <3>7, <3>8, <3>9, <3>10,
                  <1>1,
                  PersistLockCommitPreservesDurableLockRecoveryProvenance
         DEF ReducerProvenanceInvariant
    <2>8. LineageInvariant'
      <3>1. PrepareLineageSound'
        <4>1. /\ PrepareLineageSound
              /\ CurrentIntentViewsBound
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ prepareIntents' = prepareIntents
              /\ commitIntents'
                   = commitIntents \cup {request.vote}
              /\ prepareQCs' = prepareQCs
          BY <1>1 DEF PersistLockCommit
        <4>3. /\ ModelConfiguration
              /\ prepareIntents \subseteq VoteRecordSet
              /\ prepareQCs \subseteq QcRecordSet
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>4. ASSUME NEW vote \in prepareIntents',
                      vote.signer \in Honest
               PROVE PrepareCarriesHigherSafeQc(vote)'
          <5>1. /\ vote \in prepareIntents
                /\ PrepareCarriesHigherSafeQc(vote)
            BY <4>1, <4>2, <4>4 DEF PrepareLineageSound
          <5>2. ASSUME NEW commitVote \in commitIntents',
                        /\ vote.signer \in Honest
                        /\ commitVote.signer = vote.signer
                        /\ commitVote.context = vote.context
                        /\ commitVote.phase = "Commit"
                        /\ commitVote.view < vote.view
                        /\ commitVote.subject # vote.subject
                 PROVE \E qc \in prepareQCs':
                         /\ qc.context = vote.context
                         /\ qc.phase = "Prepare"
                         /\ commitVote.view < qc.view
                         /\ qc.view < vote.view
                         /\ qc.subject = vote.subject
            <6>1. commitVote \in commitIntents
                     \/ commitVote = request.vote
              BY <4>2, <5>2
            <6>2. CASE commitVote \in commitIntents
              <7>1. \E qc \in prepareQCs:
                       /\ qc.context = vote.context
                       /\ qc.phase = "Prepare"
                       /\ commitVote.view < qc.view
                       /\ qc.view < vote.view
                       /\ qc.subject = vote.subject
                BY <5>1, <5>2, <6>2
                   DEF PrepareCarriesHigherSafeQc
              <7> QED BY <4>2, <7>1
            <6>3. CASE commitVote = request.vote
              <7>1. /\ vote.signer = request.node
                    /\ vote.context = context
                    /\ request.vote.view < vote.view
                BY <2>1, <5>2, <6>3
              <7>2. vote.view <= nodeView[vote.signer]
                <8>1. \A prepareVote \in prepareIntents:
                         (prepareVote.signer \in Honest
                            /\ prepareVote.context = context)
                           => prepareVote.view
                                <= nodeView[prepareVote.signer]
                  BY <4>1 DEF CurrentIntentViewsBound
                <8> QED BY <5>1, <5>2, <7>1, <8>1
              <7>3. \/ CurrentOpenPrepareForCommit(
                           request.node, request.qc)
                     \/ HistoricalLockedPrepareForCommit(
                           request.node, request.qc)
                BY <2>1
              <7>4. /\ vote.view \in Views
                    /\ request.vote.view \in Views
                <8>1. vote \in VoteRecordSet
                  BY <4>3, <5>1
                <8>2. vote.view \in Views
                  BY <8>1 DEF VoteRecordSet
                <8>3. request.qc \in QcRecordSet
                  BY <2>1, <4>3
                <8>4. request.vote.view \in Views
                  BY <2>1, <8>3 DEF QcRecordSet
                <8> QED BY <8>2, <8>4
              <7>5. /\ vote.view \in Int
                    /\ request.vote.view \in Int
                <8>1. Ranks \subseteq Int
                  BY <4>3, ModelRanksAreIntegers
                <8> QED BY <7>4, <8>1, ViewsAreRanks
              <7>6. CASE CurrentOpenPrepareForCommit(
                            request.node, request.qc)
                <8>1. request.vote.view = nodeView[vote.signer]
                  BY <2>1, <7>1, <7>6
                     DEF CurrentOpenPrepareForCommit
                <8>2. FALSE
                  BY <7>1, <7>2, <7>5, <8>1, SMT
                <8> QED BY <8>2
              <7>7. CASE HistoricalLockedPrepareForCommit(
                            request.node, request.qc)
                <8>1. vote.phase = "Prepare"
                  BY <1>1, <5>1
                     DEF StrongInductiveInvariant,
                         ReducerProvenanceInvariant,
                         IntentPhasesCorrect
                <8>2. /\ vote \in prepareIntents
                      /\ vote.signer = request.node
                      /\ vote.context = request.qc.context
                      /\ vote.phase = "Prepare"
                      /\ vote.view > request.qc.view
                      /\ vote.subject # request.qc.subject
                  <9>1. vote.context = request.qc.context
                    BY <2>1, <7>1
                  <9>2. vote.view > request.qc.view
                    BY <2>1, <7>1
                  <9>3. vote.subject # request.qc.subject
                    BY <2>1, <5>2, <6>3, SMT
                  <9> QED BY <5>1, <7>1, <8>1,
                              <9>1, <9>2, <9>3
                <8>3. ~\E prepareVote \in prepareIntents:
                         /\ prepareVote.signer = request.node
                         /\ prepareVote.context = request.qc.context
                         /\ prepareVote.phase = "Prepare"
                         /\ prepareVote.view > request.qc.view
                         /\ prepareVote.subject # request.qc.subject
                  BY <7>7
                     DEF HistoricalLockedPrepareForCommit,
                         NoHigherPrepareOriginKnown
                <8>4. \E prepareVote \in prepareIntents:
                         /\ prepareVote.signer = request.node
                         /\ prepareVote.context = request.qc.context
                         /\ prepareVote.phase = "Prepare"
                         /\ prepareVote.view > request.qc.view
                         /\ prepareVote.subject # request.qc.subject
                  BY <8>2
                <8>5. FALSE
                  BY <8>3, <8>4
                <8> QED BY <8>5
              <7> QED BY <7>3, <7>6, <7>7
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>2 DEF PrepareCarriesHigherSafeQc
        <4> QED BY <4>4 DEF PrepareLineageSound
      <3>2. LocksCoverOwnCommits'
        <4>1. LocksCoverOwnCommits
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ context' = context
              /\ commitIntents'
                   = commitIntents \cup {request.vote}
              /\ lockRank'
                   = [lockRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ lockSubject'
                   = [lockSubject EXCEPT
                        ![request.node] = request.qc.subject]
          BY <1>1 DEF PersistLockCommit
        <4>3. /\ ModelConfiguration
              /\ request.node \in ValidatorIds
              /\ request.qc.view \in Views
              /\ commitIntents \subseteq VoteRecordSet
              /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          <5>1. /\ TypeInvariant
                /\ ModelConfiguration
                /\ prepareQCs \subseteq QcRecordSet
                /\ commitIntents \subseteq VoteRecordSet
                /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockSubject \in
                     [ValidatorIds -> SubjectOrNone]
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. request.node \in ValidatorIds
            BY <2>1, <5>1
               DEF ModelConfiguration, QuorumConfiguration
          <5>3. request.qc \in QcRecordSet
            BY <2>1, <5>1
          <5>4. request.qc.view \in Views
            BY <5>3 DEF QcRecordSet
          <5> QED BY <5>1, <5>2, <5>4
        <4>4. ASSUME NEW vote \in commitIntents',
                      vote.signer \in Honest,
                      vote.context = context'
               PROVE /\ lockRank'[vote.signer] >= vote.view
                     /\ (lockRank'[vote.signer] = vote.view
                           => lockSubject'[vote.signer] = vote.subject)
          <5>1. vote \in commitIntents \/ vote = request.vote
            BY <4>2, <4>4
          <5>2. CASE vote = request.vote
            <6>1. /\ vote.signer = request.node
                  /\ vote.view = request.qc.view
                  /\ vote.subject = request.qc.subject
              BY <2>1, <5>2
            <6>2. /\ lockRank'[vote.signer] = vote.view
                  /\ lockSubject'[vote.signer] = vote.subject
              BY <4>2, <4>3, <6>1, Isa
            <6>3. vote.view \in Int
              <7>1. Ranks \subseteq Int
                BY <4>3, ModelRanksAreIntegers
              <7> QED BY <4>3, <6>1, <7>1, ViewsAreRanks
            <6> QED BY <6>2, <6>3, SMT
          <5>3. CASE vote \in commitIntents
            <6>1. /\ lockRank[vote.signer] >= vote.view
                  /\ (lockRank[vote.signer] = vote.view
                        => lockSubject[vote.signer] = vote.subject)
              BY <4>1, <4>2, <4>4, <5>3
                 DEF LocksCoverOwnCommits
            <6>2. vote \in VoteRecordSet
              BY <4>3, <5>3
            <6>3. CASE vote.signer = request.node
              <7>1. /\ lockRank'[vote.signer] = request.qc.view
                    /\ lockSubject'[vote.signer] = request.qc.subject
                BY <4>2, <4>3, <6>3, Isa
              <7>2. /\ request.qc.view \in Int
                    /\ lockRank[vote.signer] \in Int
                    /\ vote.view \in Int
                <8>1. request.qc.view \in Ranks
                  BY <4>3, ViewsAreRanks
                <8>2. vote.signer \in ValidatorIds
                  BY <6>2 DEF VoteRecordSet
                <8>3. lockRank[vote.signer] \in Ranks
                  BY <4>3, <8>2, FunctionValueHasCodomain
                <8>4. vote.view \in Ranks
                  BY <6>2, ViewsAreRanks DEF VoteRecordSet
                <8>5. Ranks \subseteq Int
                  BY <4>3, ModelRanksAreIntegers
                <8> QED BY <8>1, <8>3, <8>4, <8>5
              <7>3. lockRank'[vote.signer] >= vote.view
                <8>1. request.qc.view
                          >= lockRank[vote.signer]
                  BY <2>1, <6>3
                <8>2. request.qc.view >= vote.view
                  BY <6>1, <7>2, <8>1,
                     IntegerWeakOrderTransitive
                <8> QED BY <7>1, <8>2
              <7>4. ASSUME lockRank'[vote.signer] = vote.view
                     PROVE lockSubject'[vote.signer] = vote.subject
                <8>1. /\ request.qc.view = lockRank[vote.signer]
                      /\ lockRank[vote.signer] = vote.view
                  <9>1. request.qc.view
                            >= lockRank[vote.signer]
                    BY <2>1, <6>3
                  <9>2. request.qc.view = vote.view
                    BY <7>1, <7>4
                  <9> QED BY <6>1, <7>2, <9>1, <9>2,
                              IntegerWeakBoundsCollapse
                <8>2. lockSubject[vote.signer] = vote.subject
                  BY <6>1, <8>1
                <8>3. request.qc.subject =
                         lockSubject[vote.signer]
                  BY <2>1, <6>3, <8>1
                <8> QED BY <7>1, <8>2, <8>3
              <7> QED BY <7>3, <7>4
            <6>4. CASE vote.signer # request.node
              <7>1. /\ vote.signer \in ValidatorIds
                    /\ lockRank'[vote.signer]
                         = lockRank[vote.signer]
                    /\ lockSubject'[vote.signer]
                         = lockSubject[vote.signer]
                <8>1. vote.signer \in ValidatorIds
                  BY <6>2 DEF VoteRecordSet
                <8>2. /\ lockRank'[vote.signer]
                               = lockRank[vote.signer]
                      /\ lockSubject'[vote.signer]
                               = lockSubject[vote.signer]
                  BY <4>2, <4>3, <6>4, <8>1, Isa
                <8> QED BY <8>1, <8>2
              <7> QED BY <6>1, <7>1
            <6> QED BY <6>3, <6>4
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>4 DEF LocksCoverOwnCommits
      <3>3. CurrentIntentViewsBound'
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, LineageInvariant,
               PersistLockCommit, CurrentIntentViewsBound
      <3>4. HonestCommitIntentPrepared'
        <4>1. HonestCommitIntentPrepared
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ commitIntents'
                   = commitIntents \cup {request.vote}
              /\ prepareQCs' = prepareQCs
              /\ context' = context
              /\ nodeView' = nodeView
          BY <1>1 DEF PersistLockCommit
        <4>3. CommitIntentsPreparedBy(
                 commitIntents', prepareQCs')
          <5>1. ASSUME NEW vote \in commitIntents',
                        vote.signer \in Honest
                 PROVE \E qc \in prepareQCs':
                         /\ qc.context = vote.context
                         /\ qc.view = vote.view
                         /\ qc.phase = "Prepare"
                         /\ qc.subject = vote.subject
            <6>1. vote \in commitIntents
                     \/ vote = request.vote
              BY <4>2, <5>1
            <6>2. CASE vote \in commitIntents
              <7>1. \E qc \in prepareQCs:
                       /\ qc.context = vote.context
                       /\ qc.view = vote.view
                       /\ qc.phase = "Prepare"
                       /\ qc.subject = vote.subject
                BY <4>1, <5>1, <6>2
                   DEF HonestCommitIntentPrepared,
                       CommitIntentsPreparedBy
              <7> QED BY <4>2, <7>1
            <6>3. CASE vote = request.vote
              <7>1. /\ request.qc \in prepareQCs'
                    /\ request.qc.context = vote.context
                    /\ request.qc.view = vote.view
                    /\ request.qc.phase = "Prepare"
                    /\ request.qc.subject = vote.subject
                BY <2>1, <4>2, <6>3
              <7> QED BY <7>1
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1 DEF CommitIntentsPreparedBy
        <4>4. \A vote \in commitIntents':
                 (vote.signer \in Honest /\ vote.context = context')
                   => vote.view <= nodeView'[vote.signer]
          <5>1. ASSUME NEW vote \in commitIntents',
                        vote.signer \in Honest,
                        vote.context = context'
                 PROVE vote.view <= nodeView'[vote.signer]
            <6>1. vote \in commitIntents
                     \/ vote = request.vote
              BY <4>2, <5>1
            <6>2. CASE vote \in commitIntents
              BY <4>1, <4>2, <5>1, <6>2
                 DEF HonestCommitIntentPrepared
            <6>3. CASE vote = request.vote
              <7>1. vote.view <= nodeView'[vote.signer]
                BY <2>1, <4>2, <6>3
              <7> QED BY <7>1
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1
        <4> QED BY <4>3, <4>4
           DEF HonestCommitIntentPrepared
      <3>5. CertificatePhasesCorrect'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, LineageInvariant,
               PersistLockCommit, CertificatePhasesCorrect
      <3>6. DurableIntentsDoNotAnticipateHeight'
        <4>1. DurableIntentsDoNotAnticipateHeight
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ height' = height
              /\ prepareIntents' = prepareIntents
              /\ commitIntents'
                   = commitIntents \cup {request.vote}
              /\ timeoutIntents' = timeoutIntents
          BY <1>1 DEF PersistLockCommit
        <4>3. request.vote.context.height <= height
          <5>1. /\ request.vote.context = context
                /\ context.height = height
            BY <1>1, <2>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. height \in Nat
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   Heights
          <5> QED BY <5>1, <5>2, NaturalOrderReflexive
        <4> QED BY <4>1, <4>2, <4>3, Isa
           DEF DurableIntentsDoNotAnticipateHeight
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
         DEF LineageInvariant
    <2>9. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistLockCommit,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <2>6, <2>7, <2>8, <2>9
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM FormCommitQCPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormCommitQC(node, roundView, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              StrongInductiveInvariant,
              FormCommitQC(node, roundView, subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Signers ==
           ProjectedVoteSignersAt(
             node, roundView, "Commit", subject)
    <2> DEFINE Certificate ==
           QC(context, roundView, "Commit", subject, Signers)
    <2> DEFINE Request == DecisionWal(node, Certificate, TRUE)
    <2>1. /\ Certificate \in QcRecordSet
          /\ QcValid(Certificate)
          /\ CertificateHonestIntentBacked(Certificate, commitIntents)
          /\ Certificate.phase = "Commit"
          /\ commitQCs' = commitQCs \cup {Certificate}
          /\ pendingDecision' = pendingDecision \cup {Request}
      <3>1. /\ Certificate \in QcRecordSet
            /\ QcWireValid(Certificate)
            /\ Certificate.phase = "Commit"
            /\ commitQCs' = commitQCs \cup {Certificate}
            /\ pendingDecision' = pendingDecision \cup {Request}
        BY <1>1
           DEF FormCommitQC, Certificate, Signers, Request, QC
      <3>2. HonestVoteTransportBacked
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>3. CertificateHonestIntentBacked(Certificate, commitIntents)
        BY <3>2, CommitVotePoolCertificateIsIntentBacked
           DEF Certificate, Signers
      <3>4. /\ TypeInvariant
            /\ CertificateBackedBy(CurrentEpoch, Certificate,
                                   commitIntents)
            /\ HonestIntentSound(commitIntents, durableBodies,
                                 ValidSubjects)
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. CertificateBackedBy(
                 CurrentEpoch, Certificate, commitIntents)
          BY <3>1, <3>3, CurrentQcBackingIsCertificateBacking
        <4>3. HonestIntentSound(
                 commitIntents, durableBodies, ValidSubjects)
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestDurableIntentsSound
        <4> QED BY <4>1, <4>2, <4>3
      <3>5. QcValid(Certificate)
        BY <3>1, <3>4,
           WireValidBackedCertificateIsSemanticallyValid
      <3> QED BY <3>1, <3>3, <3>5
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, FormCommitQC,
             AllPendingRequests, NodeIdle, Request, DecisionWal
    <2>3. /\ TypeInvariant'
          /\ PendingCertificateWritesAuthorized'
          /\ CertificatesBackedByIntents'
      <3>1. /\ HistoricalQcValid(Certificate)
            /\ CertificateBackedBy(CurrentEpoch, Certificate,
                                   commitIntents)
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. QcWireValid(Certificate)
          BY <2>1 DEF QcValid
        <4>3. HistoricalQcValid(Certificate)
          BY <2>1, <4>1, CurrentQcValidityIsHistorical
        <4>4. CertificateBackedBy(
                 CurrentEpoch, Certificate, commitIntents)
          BY <2>1, <4>2, CurrentQcBackingIsCertificateBacking
        <4> QED BY <4>3, <4>4
      <3>2. TypeInvariant'
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. node \in ValidatorIds
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 FormCommitQC
        <4>3. Request \in DecisionWalSet
          BY <2>1, <4>2, SMT
             DEF Request, DecisionWal, DecisionWalSet
        <4> QED BY <1>1, <2>1, <4>1, <4>3, Isa
           DEF TypeInvariant, FormCommitQC, Certificate, Request
      <3>3. PendingCertificateWritesAuthorized'
        <4>1. PendingCertificateWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>2. /\ context' = context
              /\ height' = height
              /\ nodeView' = nodeView
              /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs \cup {Certificate}
              /\ formedTCs' = formedTCs
              /\ highestRank' = highestRank
              /\ pendingObservePrepare' = pendingObservePrepare
              /\ pendingInstallTC' = pendingInstallTC
              /\ pendingDecision' = pendingDecision \cup {Request}
          BY <1>1, <2>1
             DEF FormCommitQC, Certificate, Request
        <4>3. \A pending \in pendingObservePrepare':
                 /\ pending.qc \in prepareQCs'
                 /\ pending.qc.context = context'
                 /\ pending.qc.view > highestRank'[pending.node]
          BY <4>1, <4>2 DEF PendingCertificateWritesAuthorized
        <4>4. \A pending \in pendingInstallTC':
                 /\ pending.tc \in formedTCs'
                 /\ pending.tc.context = context'
                 /\ TCValid(pending.tc)'
                 /\ pending.tc.votes # {}
                 /\ pending.tc.view + 1 \in Views
                 /\ pending.tc.view + 1 >= nodeView'[pending.node]
          BY <1>1, <4>1, <4>2, Isa
             DEF PendingCertificateWritesAuthorized, FormCommitQC,
                 TCValid, AuthenticatedHighRef, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4>5. \A pending \in pendingDecision':
                 /\ pending.qc \in commitQCs'
                 /\ pending.qc.context = context'
                 /\ pending.qc.phase = "Commit"
                 /\ pending.qc.height = height'
          <5>1. ASSUME NEW pending \in pendingDecision'
                 PROVE /\ pending.qc \in commitQCs'
                       /\ pending.qc.context = context'
                       /\ pending.qc.phase = "Commit"
                       /\ pending.qc.height = height'
            <6>1. pending \in pendingDecision \/ pending = Request
              BY <4>2, <5>1
            <6>2. CASE pending \in pendingDecision
              BY <4>1, <4>2, <6>2
                 DEF PendingCertificateWritesAuthorized
            <6>3. CASE pending = Request
              <7>1. /\ pending.qc = Certificate
                    /\ context' = context
                    /\ height' = height
                    /\ Certificate \in commitQCs'
                BY <2>1, <4>2, <6>3
                   DEF Request, DecisionWal
              <7>2. /\ Certificate.context = context
                    /\ Certificate.phase = "Commit"
                    /\ Certificate.height = context.height
                BY DEF Certificate, QC
              <7>3. context.height = height
                BY <1>1
                   DEF StrongInductiveInvariant, Safety, TypeInvariant
              <7> QED BY <7>1, <7>2, <7>3
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1
        <4> QED BY <4>3, <4>4, <4>5
           DEF PendingCertificateWritesAuthorized
      <3>4. CertificatesBackedByIntents'
        <4>1. /\ commitQCs' = commitQCs \cup {Certificate}
              /\ commitIntents' = commitIntents
              /\ prepareQCs' = prepareQCs
              /\ prepareIntents' = prepareIntents
          BY <1>1 DEF FormCommitQC, Certificate
        <4>2. \A qc \in commitQCs:
                 /\ HistoricalQcValid(qc)
                 /\ CertificateBackedBy(qc.context.epoch, qc,
                                        commitIntents)
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents
        <4>3. \A qc \in commitQCs':
                 /\ HistoricalQcValid(qc)
                 /\ CertificateBackedBy(qc.context.epoch, qc,
                                        commitIntents')
          <5>1. ASSUME NEW qc \in commitQCs'
                 PROVE /\ HistoricalQcValid(qc)
                       /\ CertificateBackedBy(qc.context.epoch, qc,
                                              commitIntents')
            <6>1. qc \in commitQCs \/ qc = Certificate
              BY <4>1, <5>1
            <6>2. CASE qc \in commitQCs
              BY <4>1, <4>2, <6>2
            <6>3. CASE qc = Certificate
              BY <1>1, <3>1, <4>1, <6>3
                 DEF CurrentEpoch, Certificate, QC
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1
        <4>4. \A qc \in prepareQCs':
                 /\ HistoricalQcValid(qc)
                 /\ CertificateBackedBy(qc.context.epoch, qc,
                                        prepareIntents')
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents
        <4> QED BY <4>3, <4>4 DEF CertificatesBackedByIntents
      <3> QED BY <3>2, <3>3, <3>4
    <2>4. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
      <3>1. Safety'
        BY <1>1, <2>2, <2>3, Isa
           DEF StrongInductiveInvariant, Safety, FormCommitQC,
               ProposalSigningRequiresIntent,
               PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
               TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
               HonestCommitUniqueness, HonestTimeoutUniqueness,
               LockBelowHighest, DecisionAgreement,
               AppliedRequiresDecision
      <3>2. ReducerProvenanceInvariant'
        <4>1. ReducerProvenanceInvariant
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs \cup {Certificate}
              /\ qcNetwork' = qcNetwork
              /\ receivedQCs' = receivedQCs
          BY <1>1 DEF FormCommitQC, Certificate
        <4>3. QcTransportBacked'
          BY <4>1, <4>2, Isa
             DEF ReducerProvenanceInvariant, QcTransportBacked
        <4>4. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
              /\ PendingVoteWritesAuthorized'
              /\ HonestVoteTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
              /\ DurableTimeoutsProtectCommits'
              /\ HighestAndLockAreCertified'
          <5>1. /\ HonestVoteUnique(prepareIntents)'
                /\ HonestVoteUnique(commitIntents)'
                /\ HonestTimeoutUnique(timeoutIntents)'
                /\ IntentPhasesCorrect'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   HonestVoteUnique, HonestTimeoutUnique,
                   IntentPhasesCorrect
          <5>2. PendingVoteWritesAuthorized'
            <6>1. PendingVoteWritesAuthorized
              BY <4>1 DEF ReducerProvenanceInvariant
            <6>2. UNCHANGED
                     <<context, nodeView, durableBodies, receivedQCs,
                       prepareIntents, commitIntents, timeoutIntents,
                       prepareQCs, installedTCs, lockRank, lockSubject,
                       highestRank, highestSubject, pendingPrepare,
                       pendingLockCommit, pendingTimeout>>
              BY <1>1 DEF FormCommitQC
            <6> QED BY <6>1, <6>2,
                         UnchangedPendingVoteWriteVarsPreservesAuthorization
          <5>3. HonestVoteTransportBacked'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   HonestVoteTransportBacked, VoteIntentFor
          <5>4. HonestTimeoutTransportBacked'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   HonestTimeoutTransportBacked
          <5>5. TcTransportBacked'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   TcTransportBacked, TCValid, AuthenticatedHighRef,
                   HighRefValid, CurrentEpoch, CurrentVoters
          <5>6. HonestDurableIntentsSound'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   HonestDurableIntentsSound
          <5>7. FormedTimeoutCertificatesSound'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, ReducerProvenanceInvariant,
                   FormedTimeoutCertificatesSound
          <5>8. DurableTimeoutsProtectCommits'
            <6>1. UNCHANGED
                     <<timeoutIntents, commitIntents, installedTCs>>
              BY <1>1 DEF FormCommitQC
            <6>2. DurableTimeoutsProtectCommits
              BY <4>1 DEF ReducerProvenanceInvariant
            <6> QED BY <6>1, <6>2,
                         UnchangedDurableTimeoutProtectionVarsPreserves
          <5>9. HighestAndLockAreCertified'
            <6>1. UNCHANGED
                     <<context, prepareQCs, lockRank, lockSubject,
                       highestRank, highestSubject>>
              BY <1>1 DEF FormCommitQC
            <6>2. HighestAndLockAreCertified
              BY <4>1 DEF ReducerProvenanceInvariant
            <6> QED BY <6>1, <6>2,
                         UnchangedHighestAndLockCertificationVarsPreserves
          <5> QED BY <5>1, <5>2, <5>3, <5>4, <5>5,
                       <5>6, <5>7, <5>8, <5>9
        <4>5. DurableLockRecoveryProvenanceInvariant'
          BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 FormCommitQC
        <4> QED BY <2>3, <4>3, <4>4, <4>5
           DEF ReducerProvenanceInvariant
      <3>3. LineageInvariant'
        <4>1. LineageInvariant
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs \cup {Certificate}
          BY <1>1 DEF FormCommitQC, Certificate
        <4>3. CertificatePhasesCorrect'
          BY <2>1, <4>1, <4>2, Isa
             DEF LineageInvariant, CertificatePhasesCorrect
        <4>4. /\ PrepareLineageSound'
              /\ LocksCoverOwnCommits'
              /\ CurrentIntentViewsBound'
              /\ HonestCommitIntentPrepared'
              /\ DurableIntentsDoNotAnticipateHeight'
          <5>1. PrepareLineageSound'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, LineageInvariant,
                   PrepareLineageSound, PrepareCarriesHigherSafeQc
          <5>2. LocksCoverOwnCommits'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, LineageInvariant, LocksCoverOwnCommits
          <5>3. CurrentIntentViewsBound'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, LineageInvariant, CurrentIntentViewsBound
          <5>4. HonestCommitIntentPrepared'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, LineageInvariant,
                   HonestCommitIntentPrepared, CommitIntentsPreparedBy
          <5>5. DurableIntentsDoNotAnticipateHeight'
            BY <1>1, <4>1, Isa
               DEF FormCommitQC, LineageInvariant,
                   DurableIntentsDoNotAnticipateHeight
          <5> QED BY <5>1, <5>2, <5>3, <5>4, <5>5
        <4> QED BY <4>3, <4>4 DEF LineageInvariant
      <3> QED BY <3>1, <3>2, <3>3
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, FormCommitQC,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <2>4, <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginDecisionPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ BeginDecision(node, qc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW qc,
              StrongInductiveInvariant,
              BeginDecision(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == DecisionWal(node, qc, FALSE)
    <2>1. /\ qc \in commitQCs
          /\ qc.phase = "Commit"
          /\ qc.context = context
          /\ Request \in DecisionWalSet
      <3>1. /\ node \in ValidatorIds
            /\ QcAt(node, qc) \in receivedQCs
            /\ qc.phase = "Commit"
            /\ qc.context = context
        BY <1>1 DEF BeginDecision
      <3>2. qc \in prepareQCs \cup commitQCs
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked, QcAt
      <3>3. /\ \A prepared \in prepareQCs:
                    prepared.phase = "Prepare"
            /\ \A committed \in commitQCs:
                    committed.phase = "Commit"
        BY <1>1
           DEF StrongInductiveInvariant, LineageInvariant,
               CertificatePhasesCorrect
      <3>4. qc \in commitQCs
        BY <3>1, <3>2, <3>3
      <3>5. qc \in QcRecordSet
        BY <1>1, <3>4
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>6. Request \in DecisionWalSet
        BY <3>1, <3>5, SMT
           DEF Request, DecisionWal, DecisionWalSet
      <3> QED BY <3>1, <3>4, <3>6
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, BeginDecision,
             AllPendingRequests, NodeIdle, Request, DecisionWal
    <2>3. /\ TypeInvariant'
          /\ PendingCertificateWritesAuthorized'
      <3>1. TypeInvariant'
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               BeginDecision, Request
      <3>2. qc.height = height
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HistoricalQcValid
      <3>3. PendingCertificateWritesAuthorized'
        <4>0. /\ context' = context
              /\ height' = height
              /\ nodeView' = nodeView
              /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs
              /\ formedTCs' = formedTCs
              /\ highestRank' = highestRank
              /\ pendingObservePrepare' = pendingObservePrepare
              /\ pendingInstallTC' = pendingInstallTC
          BY <1>1 DEF BeginDecision
        <4>1. \A pending \in pendingObservePrepare':
                 /\ pending.qc \in prepareQCs'
                 /\ pending.qc.context = context'
                 /\ pending.qc.view > highestRank'[pending.node]
          BY <1>1, <4>0
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PendingCertificateWritesAuthorized, BeginDecision
        <4>2. \A pending \in pendingInstallTC':
                 /\ pending.tc \in formedTCs'
                 /\ pending.tc.context = context'
                 /\ TCValid(pending.tc)'
                 /\ pending.tc.votes # {}
                 /\ pending.tc.view + 1 \in Views
                 /\ pending.tc.view + 1 >= nodeView'[pending.node]
          <5>1. ASSUME NEW pending \in pendingInstallTC'
                 PROVE /\ pending.tc \in formedTCs'
                       /\ pending.tc.context = context'
                       /\ TCValid(pending.tc)'
                       /\ pending.tc.votes # {}
                       /\ pending.tc.view + 1 \in Views
                       /\ pending.tc.view + 1 >= nodeView'[pending.node]
            <6>1. /\ pending.tc \in formedTCs
                  /\ pending.tc.context = context
                  /\ TCValid(pending.tc)
                  /\ pending.tc.votes # {}
                  /\ pending.tc.view + 1 \in Views
                  /\ pending.tc.view + 1 >= nodeView[pending.node]
              BY <1>1, <4>0, <5>1
                 DEF StrongInductiveInvariant,
                     ReducerProvenanceInvariant,
                     PendingCertificateWritesAuthorized
            <6>2. TCValid(pending.tc)' <=> TCValid(pending.tc)
              BY <4>0
                 DEF TCValid, AuthenticatedHighRef, HighRefValid, CurrentVoters, CurrentEpoch
            <6> QED BY <4>0, <6>1, <6>2
          <5> QED BY <5>1
        <4>3. \A pending \in pendingDecision':
                 /\ pending.qc \in commitQCs'
                 /\ pending.qc.context = context'
                 /\ pending.qc.phase = "Commit"
                 /\ pending.qc.height = height'
          <5>1. ASSUME NEW pending \in pendingDecision'
                 PROVE /\ pending.qc \in commitQCs'
                       /\ pending.qc.context = context'
                       /\ pending.qc.phase = "Commit"
                       /\ pending.qc.height = height'
            <6>1. pending \in pendingDecision \/ pending = Request
              BY <1>1, <5>1 DEF BeginDecision, Request
            <6>2. CASE pending \in pendingDecision
              BY <1>1, <6>2
                 DEF StrongInductiveInvariant,
                     ReducerProvenanceInvariant,
                     PendingCertificateWritesAuthorized,
                     BeginDecision
            <6>3. CASE pending = Request
              BY <1>1, <2>1, <3>2, <6>3
                 DEF BeginDecision, Request, DecisionWal
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2, <4>3
           DEF PendingCertificateWritesAuthorized
      <3> QED BY <3>1, <3>3
    <2>4. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      <3>1. Safety'
        BY <1>1, <2>2, <2>3, Isa
           DEF StrongInductiveInvariant, Safety, BeginDecision,
               ProposalSigningRequiresIntent,
               PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
               TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
               HonestCommitUniqueness, HonestTimeoutUniqueness,
               LockBelowHighest, DecisionAgreement,
               AppliedRequiresDecision
      <3>2. ReducerProvenanceInvariant'
        <4>1. ReducerProvenanceInvariant
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, <4>1, Isa
             DEF BeginDecision, ReducerProvenanceInvariant,
                 HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
        <4>3. PendingVoteWritesAuthorized'
          <5>1. UNCHANGED
                   <<context, nodeView, durableBodies, receivedQCs,
                     prepareIntents, commitIntents, timeoutIntents,
                     prepareQCs, installedTCs, lockRank, lockSubject,
                     highestRank, highestSubject, pendingPrepare,
                     pendingLockCommit, pendingTimeout>>
            BY <1>1 DEF BeginDecision
          <5>2. PendingVoteWritesAuthorized
            BY <4>1 DEF ReducerProvenanceInvariant
          <5> QED BY <5>1, <5>2,
                       UnchangedPendingVoteWriteVarsPreservesAuthorization
        <4>4. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          BY <1>1, <4>1, Isa
             DEF BeginDecision, ReducerProvenanceInvariant,
                 HonestVoteTransportBacked, VoteIntentFor,
                 QcTransportBacked, HonestTimeoutTransportBacked,
                 TcTransportBacked, TCValid, AuthenticatedHighRef,
                 HighRefValid, CurrentEpoch, CurrentVoters
        <4>5. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
              /\ DurableTimeoutsProtectCommits'
              /\ HighestAndLockAreCertified'
          <5>1. /\ CertificatesBackedByIntents'
                /\ HonestDurableIntentsSound'
                /\ FormedTimeoutCertificatesSound'
            BY <1>1, <4>1, Isa
               DEF BeginDecision, ReducerProvenanceInvariant,
                   CertificatesBackedByIntents,
                   HonestDurableIntentsSound,
                   FormedTimeoutCertificatesSound
          <5>2. UNCHANGED
                   <<timeoutIntents, commitIntents, installedTCs>>
            BY <1>1 DEF BeginDecision
          <5>3. DurableTimeoutsProtectCommits
            BY <4>1 DEF ReducerProvenanceInvariant
          <5>4. DurableTimeoutsProtectCommits'
            BY <5>2, <5>3,
               UnchangedDurableTimeoutProtectionVarsPreserves
          <5>5. UNCHANGED
                   <<context, prepareQCs, lockRank, lockSubject,
                     highestRank, highestSubject>>
            BY <1>1 DEF BeginDecision
          <5>6. HighestAndLockAreCertified
            BY <4>1 DEF ReducerProvenanceInvariant
          <5>7. HighestAndLockAreCertified'
            BY <5>5, <5>6,
               UnchangedHighestAndLockCertificationVarsPreserves
          <5> QED BY <5>1, <5>4, <5>7
        <4>6. DurableLockRecoveryProvenanceInvariant'
          BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginDecision
        <4> QED BY <2>3, <4>2, <4>3, <4>4, <4>5, <4>6
           DEF ReducerProvenanceInvariant
      <3>3. LineageInvariant'
        BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
           DEF StrongInductiveInvariant, BeginDecision, LineageVars
      <3>4. /\ ContextIdentityBindsFrozenEpoch'
              /\ OldContextCertificateRejected'
              /\ ContextParentWasApplied'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, BeginDecision,
               ContextIdentityBindsFrozenEpoch,
               OldContextCertificateRejected, ContextParentWasApplied,
               QcValid, QcWireValid, CurrentEpoch
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM PersistDecisionPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistDecision(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistDecision(request)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Decision == [node |-> request.node, qc |-> request.qc]
    <2>1. /\ request \in pendingDecision
          /\ request.qc \in commitQCs
          /\ request.qc.phase = "Commit"
          /\ decisions' = decisions \cup {Decision}
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             PersistDecision, Decision
    <2>2. \A left, right \in commitQCs:
             left.context = right.context
               => left.subject = right.subject
      <3>1. /\ QuorumConfiguration
            /\ CertificatesBackedByIntents
            /\ IntentPhasesCorrect
            /\ CertificatePhasesCorrect
            /\ HonestVoteUnique(commitIntents)
            /\ PrepareLineageSound
            /\ HonestCommitIntentPrepared
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration, ReducerProvenanceInvariant,
               LineageInvariant
      <3>2. (/\ QuorumConfiguration
             /\ CertificatesBackedByIntents
             /\ IntentPhasesCorrect
             /\ CertificatePhasesCorrect
             /\ HonestVoteUnique(commitIntents)
             /\ PrepareLineageSound
             /\ HonestCommitIntentPrepared)
              => \A left, right \in commitQCs:
                   left.context = right.context
                     => left.subject = right.subject
        BY CommitCertificateAgreement
      <3> QED BY <3>1, <3>2
    <2>3. DecisionAgreement'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, DecisionAgreement,
             PersistDecision, Decision
    <2>4. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistDecision
    <2>5. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      <3>1. Safety'
        <4>1. Safety
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. TypeInvariant'
          BY <1>1, <2>1, Isa
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 PersistDecision
        <4>3. /\ proposalIntents' = proposalIntents
              /\ prepareIntents' = prepareIntents
              /\ commitIntents' = commitIntents
              /\ timeoutIntents' = timeoutIntents
              /\ signProposals' = signProposals
              /\ signVotes' = signVotes
              /\ signTimeouts' = signTimeouts
              /\ lockRank' = lockRank
              /\ highestRank' = highestRank
              /\ applied' = applied
          BY <1>1 DEF PersistDecision
        <4>4. /\ ProposalSigningRequiresIntent'
              /\ PrepareSigningRequiresIntent'
              /\ CommitSigningRequiresIntent'
              /\ TimeoutSigningRequiresIntent'
          BY <4>1, <4>3
             DEF Safety, ProposalSigningRequiresIntent,
                 PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
                 TimeoutSigningRequiresIntent
        <4>5. /\ HonestPrepareUniqueness'
              /\ HonestCommitUniqueness'
              /\ HonestTimeoutUniqueness'
          BY <4>1, <4>3
             DEF Safety, HonestPrepareUniqueness,
                 HonestCommitUniqueness, HonestTimeoutUniqueness
        <4>6. LockBelowHighest'
          BY <4>1, <4>3 DEF Safety, LockBelowHighest
        <4>7. AppliedRequiresDecision'
          BY <2>1, <4>1, <4>3, Isa
             DEF Safety, AppliedRequiresDecision, Decision
        <4> QED BY <2>3, <2>4, <4>2, <4>4, <4>5, <4>6, <4>7
           DEF Safety
      <3>2. ReducerProvenanceInvariant'
        <4>1. ReducerProvenanceInvariant
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. PendingCertificateWritesAuthorized'
          BY <1>1, <2>1, <4>1, Isa
             DEF PersistDecision, ReducerProvenanceInvariant,
                 PendingCertificateWritesAuthorized,
                 TCValid, AuthenticatedHighRef, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4>3. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, <4>1, Isa
             DEF PersistDecision, ReducerProvenanceInvariant,
                 HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
        <4>4. PendingVoteWritesAuthorized'
          <5>1. UNCHANGED
                   <<context, nodeView, durableBodies, receivedQCs,
                     prepareIntents, commitIntents, timeoutIntents,
                     prepareQCs, installedTCs, lockRank, lockSubject,
                     highestRank, highestSubject, pendingPrepare,
                     pendingLockCommit, pendingTimeout>>
            BY <1>1 DEF PersistDecision
          <5>2. PendingVoteWritesAuthorized
            BY <4>1 DEF ReducerProvenanceInvariant
          <5> QED BY <5>1, <5>2,
                       UnchangedPendingVoteWriteVarsPreservesAuthorization
        <4>5. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          <5>1. HonestVoteTransportBacked'
            BY <1>1, <4>1, Isa
               DEF PersistDecision, ReducerProvenanceInvariant,
                   HonestVoteTransportBacked, VoteIntentFor
          <5>2. QcTransportBacked'
            <6>1. QcTransportBacked
              BY <4>1 DEF ReducerProvenanceInvariant
            <6>2. /\ prepareQCs' = prepareQCs
                  /\ commitQCs' = commitQCs
                  /\ receivedQCs' = receivedQCs
                  /\ qcNetwork' =
                       IF request.rebroadcast
                       THEN qcNetwork \cup BroadcastQCs(request.qc)
                       ELSE qcNetwork
              BY <1>1 DEF PersistDecision
            <6>3. \A envelope \in qcNetwork':
                     envelope.qc \in prepareQCs' \cup commitQCs'
              <7>1. ASSUME NEW envelope \in qcNetwork'
                     PROVE envelope.qc \in prepareQCs' \cup commitQCs'
                <8>1. CASE envelope \in qcNetwork
                  BY <6>1, <6>2, <8>1 DEF QcTransportBacked
                <8>2. CASE envelope \notin qcNetwork
                  <9>1. envelope \in BroadcastQCs(request.qc)
                    BY <6>2, <7>1, <8>2, Isa
                  <9>2. envelope.qc = request.qc
                    BY <9>1, Isa DEF BroadcastQCs, QcEnvelope
                  <9> QED BY <2>1, <6>2, <9>2
                <8> QED BY <8>1, <8>2
              <7> QED BY <7>1
            <6>4. \A received \in receivedQCs':
                     received.qc \in prepareQCs' \cup commitQCs'
              BY <6>1, <6>2 DEF QcTransportBacked
            <6> QED BY <6>3, <6>4 DEF QcTransportBacked
          <5>3. HonestTimeoutTransportBacked'
            BY <1>1, <4>1, Isa
               DEF PersistDecision, ReducerProvenanceInvariant,
                   HonestTimeoutTransportBacked
          <5>4. TcTransportBacked'
            BY <1>1, <4>1, Isa
               DEF PersistDecision, ReducerProvenanceInvariant,
                   TcTransportBacked, TCValid, AuthenticatedHighRef,
                   HighRefValid, CurrentEpoch, CurrentVoters
          <5> QED BY <5>1, <5>2, <5>3, <5>4
        <4>6. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
              /\ DurableTimeoutsProtectCommits'
              /\ HighestAndLockAreCertified'
          <5>1. /\ CertificatesBackedByIntents'
                /\ HonestDurableIntentsSound'
                /\ FormedTimeoutCertificatesSound'
            BY <1>1, <4>1, Isa
               DEF PersistDecision, ReducerProvenanceInvariant,
                   CertificatesBackedByIntents,
                   HonestDurableIntentsSound,
                   FormedTimeoutCertificatesSound
          <5>2. UNCHANGED
                   <<timeoutIntents, commitIntents, installedTCs>>
            BY <1>1 DEF PersistDecision
          <5>3. DurableTimeoutsProtectCommits
            BY <4>1 DEF ReducerProvenanceInvariant
          <5>4. DurableTimeoutsProtectCommits'
            BY <5>2, <5>3,
               UnchangedDurableTimeoutProtectionVarsPreserves
          <5>5. UNCHANGED
                   <<context, prepareQCs, lockRank, lockSubject,
                     highestRank, highestSubject>>
            BY <1>1 DEF PersistDecision
          <5>6. HighestAndLockAreCertified
            BY <4>1 DEF ReducerProvenanceInvariant
          <5>7. HighestAndLockAreCertified'
            BY <5>5, <5>6,
               UnchangedHighestAndLockCertificationVarsPreserves
          <5> QED BY <5>1, <5>4, <5>7
        <4>7. DurableLockRecoveryProvenanceInvariant'
          BY <1>1, UnchangedDurableLockRecoveryProvenanceVarsPreserves
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PersistDecision
        <4> QED BY <4>2, <4>3, <4>4, <4>5, <4>6, <4>7
           DEF ReducerProvenanceInvariant
      <3>3. LineageInvariant'
        BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
           DEF StrongInductiveInvariant, PersistDecision, LineageVars
      <3>4. /\ ContextIdentityBindsFrozenEpoch'
              /\ OldContextCertificateRejected'
              /\ ContextParentWasApplied'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, PersistDecision,
               ContextIdentityBindsFrozenEpoch,
               OldContextCertificateRejected, ContextParentWasApplied,
               QcValid, QcWireValid, CurrentEpoch
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM FiniteIntegerRanksHaveMaximum ==
  \A votes:
    (/\ IsFiniteSet(votes)
     /\ votes # {}
     /\ \A vote \in votes: vote.highRank \in Int)
      => \E candidate \in votes:
           \A other \in votes:
             candidate.highRank >= other.highRank
PROOF
  <1>1. ASSUME NEW votes,
              IsFiniteSet(votes),
              votes # {},
              \A vote \in votes: vote.highRank \in Int
         PROVE \E candidate \in votes:
                 \A other \in votes:
                   candidate.highRank >= other.highRank
    <2> DEFINE HasMaximum(set) ==
           \/ set = {}
           \/ \E candidate \in set:
                \A other \in set:
                  candidate.highRank >= other.highRank
    <2>1. HasMaximum({})
      BY DEF HasMaximum
    <2>2. ASSUME NEW subset \in SUBSET votes,
                  IsFiniteSet(subset),
                  HasMaximum(subset),
                  NEW added \in votes \ subset
           PROVE HasMaximum(subset \cup {added})
      <3>1. added.highRank \in Int
        BY <1>1, <2>2
      <3>2. CASE subset = {}
        <4>1. \A other \in subset \cup {added}:
                 added.highRank >= other.highRank
          BY <3>1, <3>2, SMT
        <4> QED BY <2>2, <4>1 DEF HasMaximum
      <3>3. CASE subset # {}
        <4>1. PICK candidate \in subset:
                 \A other \in subset:
                   candidate.highRank >= other.highRank
          BY <2>2, <3>3 DEF HasMaximum
        <4>2. candidate.highRank \in Int
          BY <1>1, <2>2, <4>1
        <4>3. CASE added.highRank >= candidate.highRank
          <5>1. \A other \in subset:
                   added.highRank >= other.highRank
            <6>1. ASSUME NEW other \in subset
                   PROVE added.highRank >= other.highRank
              <7>1. other.highRank \in Int
                BY <1>1, <2>2, <6>1
              <7>2. candidate.highRank >= other.highRank
                BY <4>1, <6>1
              <7> QED BY <3>1, <4>2, <4>3, <7>1, <7>2, SMT
            <6> QED BY <6>1
          <5>2. added.highRank >= added.highRank
            BY <3>1, SMT
          <5>3. \A other \in subset \cup {added}:
                   added.highRank >= other.highRank
            BY <5>1, <5>2, Isa
          <5> QED BY <2>2, <5>3 DEF HasMaximum
        <4>4. CASE added.highRank < candidate.highRank
          <5>1. candidate.highRank >= added.highRank
            BY <3>1, <4>2, <4>4, SMT
          <5>2. \A other \in subset \cup {added}:
                   candidate.highRank >= other.highRank
            BY <4>1, <5>1, Isa
          <5> QED BY <4>1, <5>2 DEF HasMaximum
        <4>5. added.highRank >= candidate.highRank
                 \/ added.highRank < candidate.highRank
          BY <3>1, <4>2, SMT
        <4> QED BY <4>3, <4>4, <4>5
      <3>4. subset = {} \/ subset # {}
        BY Isa
      <3> QED BY <3>2, <3>3, <3>4
    <2> DEFINE Q(n) ==
           \A subset \in SUBSET votes:
             Cardinality(subset) = n => HasMaximum(subset)
    <2>3. Q(0)
      BY <1>1, FS_EmptySet, FS_Subset DEF Q
    <2>4. ASSUME NEW n \in Nat,
                  Q(n),
                  NEW subset \in SUBSET votes,
                  Cardinality(subset) = n + 1
           PROVE HasMaximum(subset)
      <3>1. PICK added \in subset: TRUE
        BY <2>4, FS_EmptySet, FS_Subset
      <3>2. IsFiniteSet(subset)
        BY <1>1, <2>4, FS_Subset
      <3>3. /\ subset \ {added} \in SUBSET votes
             /\ IsFiniteSet(subset \ {added})
             /\ Cardinality(subset \ {added}) = n
        BY <2>4, <3>1, <3>2, FS_RemoveElement, FS_Subset, Isa
      <3>4. HasMaximum(subset \ {added})
        BY <2>4, <3>3 DEF Q
      <3>5. added \in votes \ (subset \ {added})
        BY <2>4, <3>1, Isa
      <3> QED BY <2>2, <3>3, <3>4, <3>5
    <2>5. \A n \in Nat: Q(n)
      BY <2>3, <2>4, NatInduction
    <2>6. Cardinality(votes) \in Nat
      BY <1>1, FS_CardinalityType
    <2>7. HasMaximum(votes)
      BY <2>5, <2>6 DEF Q
    <2> QED BY <1>1, <2>7 DEF HasMaximum
  <1> QED BY <1>1

THEOREM MaximumWitnessMakesSelectorMaximal ==
  \A votes:
    (\E candidate \in votes:
       \A other \in votes:
         candidate.highRank >= other.highRank)
      => HighestTimeoutVote(votes) \in MaximalTimeoutVotes(votes)
PROOF
  <1>1. ASSUME NEW votes,
              \E candidate \in votes:
                \A other \in votes:
                  candidate.highRank >= other.highRank
         PROVE HighestTimeoutVote(votes) \in MaximalTimeoutVotes(votes)
    <2>1. MaximalTimeoutVotes(votes) # {}
      BY <1>1, Isa DEF MaximalTimeoutVotes
    <2> QED BY <2>1, Zenon DEF HighestTimeoutVote
  <1> QED BY <1>1

THEOREM ValidTimeoutCertificateSelectsMaximal ==
  \A tc:
    ModelConfiguration /\ TCValid(tc)
      => HighestTimeoutVote(tc.votes) \in MaximalTimeoutVotes(tc.votes)
PROOF
  <1>1. ASSUME NEW tc, ModelConfiguration, TCValid(tc)
         PROVE HighestTimeoutVote(tc.votes)
                 \in MaximalTimeoutVotes(tc.votes)
    <2>1. /\ IsFiniteSet(tc.votes)
          /\ tc.votes # {}
          /\ \A vote \in tc.votes:
               vote.highRank \in Int
      BY <1>1, SMT
         DEF ModelConfiguration, TCValid, AuthenticatedHighRef, HighRefValid,
             Views, Ranks, NoRank
    <2>2. \E candidate \in tc.votes:
             \A other \in tc.votes:
               candidate.highRank >= other.highRank
      BY <2>1, FiniteIntegerRanksHaveMaximum
    <2>3. HighestTimeoutVote(tc.votes)
             \in MaximalTimeoutVotes(tc.votes)
      BY <2>2, MaximumWitnessMakesSelectorMaximal
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ValidTimeoutCertificateSelectsMember ==
  \A tc:
    ModelConfiguration /\ TCValid(tc)
      => HighestTimeoutVote(tc.votes) \in tc.votes
BY ValidTimeoutCertificateSelectsMaximal
   DEF MaximalTimeoutVotes

THEOREM ValidTimeoutCertificateSelectsReportedMaximum ==
  \A tc:
    ModelConfiguration /\ TCValid(tc)
      => TCMaximumProtectsReports(tc)
PROOF
  <1>1. ASSUME NEW tc, ModelConfiguration, TCValid(tc)
         PROVE TCMaximumProtectsReports(tc)
    <2>1. HighestTimeoutVote(tc.votes) \in tc.votes
      BY <1>1, ValidTimeoutCertificateSelectsMember
    <2>2. HighestTimeoutVote(tc.votes)
             \in MaximalTimeoutVotes(tc.votes)
      BY <1>1, ValidTimeoutCertificateSelectsMaximal
    <2>3. \A other \in tc.votes:
             HighestTimeoutVote(tc.votes).highRank >= other.highRank
      BY <2>2 DEF MaximalTimeoutVotes
    <2>4. \A other \in tc.votes:
             HighestTimeoutVote(tc.votes).highRank = other.highRank
               => HighestTimeoutVote(tc.votes).highSubject
                    = other.highSubject
      <3>1. ASSUME NEW other \in tc.votes,
                    HighestTimeoutVote(tc.votes).highRank = other.highRank
             PROVE HighestTimeoutVote(tc.votes).highSubject
                     = other.highSubject
        <4>1. CASE other.highRank = NoRank
          BY <1>1, <2>1, <3>1, <4>1, SMT
             DEF TCValid, AuthenticatedHighRef, HighRefValid, ModelConfiguration,
                 Views, NoRank
        <4>2. CASE other.highRank # NoRank
          BY <1>1, <2>1, <3>1, <4>2
             DEF TCValid, TimeoutHighsConflictFree
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4
       DEF TCMaximumProtectsReports, TcHighRank, TcHighSubject,
           TCValid
  <1> QED BY <1>1

THEOREM StrongInvariantImpliesTimeoutCertificateSelectorsSound ==
  StrongInductiveInvariant => TimeoutCertificateSelectorsSound
PROOF
  <1>1. ASSUME StrongInductiveInvariant, NEW tc \in formedTCs
         PROVE HighestTimeoutVote(tc.votes) \in tc.votes
    <2>1. /\ IsFiniteSet(tc.votes)
          /\ tc.votes # {}
          /\ \A vote \in tc.votes:
               vote.highRank \in Int
      <3>1. /\ IsFiniteSet(tc.votes)
            /\ tc.votes # {}
            /\ \A vote \in tc.votes: vote.highRank \in Ranks
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound
      <3>2. /\ ViewDomain \subseteq Nat
            /\ Ranks = {NoRank} \cup ViewDomain
            /\ NoRank = -1
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration, Views, Ranks, NoRank
      <3>3. \A vote \in tc.votes: vote.highRank \in Int
        BY <3>1, <3>2, SMT
      <3> QED BY <3>1, <3>3
    <2>2. \E candidate \in tc.votes:
             \A other \in tc.votes:
               candidate.highRank >= other.highRank
      BY <2>1, FiniteIntegerRanksHaveMaximum
    <2>3. HighestTimeoutVote(tc.votes)
             \in MaximalTimeoutVotes(tc.votes)
      BY <2>2, MaximumWitnessMakesSelectorMaximal
    <2> QED BY <2>3 DEF MaximalTimeoutVotes
  <1> QED BY <1>1 DEF TimeoutCertificateSelectorsSound

THEOREM ValidTimeoutCertificateIsWellTyped ==
  \A tc:
    ModelConfiguration
      /\ context \in ContextRecords
      /\ height \in Heights
      /\ TCValid(tc)
        => TcWellTyped(tc)
PROOF
  <1>1. ASSUME NEW tc,
              ModelConfiguration,
              context \in ContextRecords,
              height \in Heights,
              TCValid(tc)
         PROVE TcWellTyped(tc)
    <2>1. /\ tc \in TcRecordSet
          /\ DOMAIN tc = {"context", "height", "view", "votes"}
          /\ tc.context \in ContextRecords
          /\ tc.height \in Heights
          /\ tc.view \in Views
      BY <1>1 DEF TCValid
    <2>2. CurrentEpoch \in Epochs
      BY <1>1 DEF TCValid, DualQuorum, CountQuorum
    <2>3. VotingRoster(CurrentEpoch) \subseteq ValidatorIds
      BY <1>1, <2>2
         DEF ModelConfiguration, QuorumConfiguration, VotingRoster
    <2>4. \A vote \in tc.votes: vote \in TimeoutVoteRecordSet
      BY <1>1 DEF TCValid
    <2>5. tc.votes \subseteq TimeoutVoteRecordSet
      BY <2>4
    <2> QED BY <2>1, <2>5 DEF TcWellTyped
  <1> QED BY <1>1

THEOREM TimeoutCertificateRecordTyping ==
  \A tc: TcWellTyped(tc) => tc \in TcRecordSet
BY DEF TcWellTyped

THEOREM InstallTcWalRecordTyping ==
  \A tc:
    \A node \in ValidatorIds, rebroadcast \in BOOLEAN:
      TcWellTyped(tc)
        => InstallTcWal(node, tc, rebroadcast) \in InstallTcWalSet
PROOF
  <1>1. ASSUME NEW tc,
              NEW node \in ValidatorIds,
              NEW rebroadcast \in BOOLEAN,
              TcWellTyped(tc)
         PROVE InstallTcWal(node, tc, rebroadcast) \in InstallTcWalSet
    <2>1. tc \in TcRecordSet
      BY <1>1, TimeoutCertificateRecordTyping
    <2> QED BY <1>1, <2>1, Isa
       DEF InstallTcWal, InstallTcWalSet
  <1> QED BY <1>1

THEOREM DeliverTCPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverTC(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverTC(envelope)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Received == TcAt(envelope.recipient, envelope.tc)
    <2> DEFINE StableVars ==
          <<height, context, contextHistory, nodeView, generation, up, gst,
            availableBodies, durableBodies, retainedLockedBodies,
            validatedBodies, invalidBodies,
            seenProposals, receivedVotes, receivedQCs, receivedTimeoutVotes,
            proposalIntents, prepareIntents, commitIntents, timeoutIntents,
            prepareQCs, commitQCs, formedTCs, installedTCs, lockRank,
            lockSubject, highestRank, highestSubject, pendingProposal,
            pendingPrepare, pendingObservePrepare, pendingLockCommit,
            pendingTimeout, pendingInstallTC, pendingDecision,
            signProposals, signVotes, signTimeouts, proposalNetwork,
            voteNetwork, qcNetwork, timeoutNetwork, decisions, applied>>
    <2>1. /\ envelope.tc \in formedTCs
          /\ TCValid(envelope.tc)
          /\ TcWellTyped(envelope.tc)
          /\ envelope.recipient \in ValidatorIds
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, TcTransportBacked,
             DeliverTC
    <2>2. UNCHANGED StableVars
      BY <1>1 DEF DeliverTC, StableVars
    <2>3. /\ receivedTCs' = receivedTCs \cup {Received}
          /\ tcNetwork' = tcNetwork \ {envelope}
      BY <1>1 DEF DeliverTC, Received
    <2>4. /\ Received.node \in ValidatorIds
          /\ TcWellTyped(Received.tc)
      BY <2>1 DEF Received, TcAt
    <2>5. TypeInvariant'
      BY <1>1, <2>2, <2>3, <2>4, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             StableVars
    <2>6. TcTransportBacked'
      <3>1. \A pending \in tcNetwork':
               /\ pending.tc \in formedTCs'
               /\ TCValid(pending.tc)'
        <4>1. ASSUME NEW pending \in tcNetwork'
               PROVE /\ pending.tc \in formedTCs'
                     /\ TCValid(pending.tc)'
          <5>1. /\ pending \in tcNetwork
                /\ pending.tc \in formedTCs
                /\ TCValid(pending.tc)
            BY <1>1, <2>2, <2>3, <4>1, Isa
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   TcTransportBacked, StableVars
          <5>2. TCValid(pending.tc)'
            BY <2>2, <5>1, Isa
               DEF StableVars, TCValid, AuthenticatedHighRef, HighRefValid,
                   CurrentEpoch, CurrentVoters
          <5> QED BY <2>2, <5>1, <5>2, Isa DEF StableVars
        <4> QED BY <4>1
      <3>2. \A pending \in receivedTCs':
               /\ pending.tc \in formedTCs'
               /\ TCValid(pending.tc)'
        <4>1. ASSUME NEW pending \in receivedTCs'
               PROVE /\ pending.tc \in formedTCs'
                     /\ TCValid(pending.tc)'
          <5>1. pending \in receivedTCs \/ pending = Received
            BY <2>3, <4>1, Isa
          <5>2. CASE pending \in receivedTCs
            <6>1. /\ pending.tc \in formedTCs
                  /\ TCValid(pending.tc)
              BY <1>1, <5>2
                 DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                     TcTransportBacked
            <6>2. TCValid(pending.tc)'
              BY <2>2, <6>1, Isa
                 DEF StableVars, TCValid, AuthenticatedHighRef, HighRefValid,
                     CurrentEpoch, CurrentVoters
            <6> QED BY <2>2, <6>1, <6>2, Isa DEF StableVars
          <5>3. CASE pending = Received
            BY <2>1, <2>2, <5>3, Isa
               DEF StableVars, Received, TcAt,
                   TCValid, AuthenticatedHighRef, HighRefValid,
                   CurrentEpoch, CurrentVoters
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>3. \A installed \in installedTCs':
               installed.tc \in formedTCs'
        BY <1>1, <2>2, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, StableVars
      <3> QED BY <3>1, <3>2, <3>3 DEF TcTransportBacked
    <2>7. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      <3>1. OnePendingPersistencePerNode'
        BY <1>1, <2>2, Isa
           DEF StrongInductiveInvariant, Safety, StableVars,
               OnePendingPersistencePerNode, AllPendingRequests,
               RequestsUniqueByNode
      <3>2. /\ ProposalSigningRequiresIntent'
            /\ PrepareSigningRequiresIntent'
            /\ CommitSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
        BY <1>1, <2>2, Isa
           DEF StrongInductiveInvariant, Safety, StableVars,
               ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
               CommitSigningRequiresIntent, TimeoutSigningRequiresIntent
      <3> QED BY <3>1, <3>2
    <2>8. /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, StableVars,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness
    <2>9. /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, StableVars,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>10. Safety'
      BY <2>5, <2>7, <2>8, <2>9 DEF Safety
    <2>11. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             StableVars, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect
    <2>12. PendingVoteWritesAuthorized'
      <3>1. UNCHANGED
               <<context, nodeView, durableBodies, receivedQCs,
                 prepareIntents, commitIntents, timeoutIntents,
                 prepareQCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingPrepare,
                 pendingLockCommit, pendingTimeout>>
        BY <2>2, Isa DEF StableVars
      <3>2. PendingVoteWritesAuthorized
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3> QED BY <3>1, <3>2,
                   UnchangedPendingVoteWriteVarsPreservesAuthorization
    <2>13. PendingCertificateWritesAuthorized'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             StableVars, PendingCertificateWritesAuthorized,
             TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>14. /\ HonestVoteTransportBacked'
           /\ QcTransportBacked'
           /\ HonestTimeoutTransportBacked'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             StableVars, HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, VoteIntentFor
    <2>15. /\ CertificatesBackedByIntents'
           /\ HonestDurableIntentsSound'
           /\ FormedTimeoutCertificatesSound'
           /\ DurableTimeoutsProtectCommits'
           /\ HighestAndLockAreCertified'
      <3>1. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
        BY <1>1, <2>2, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               StableVars, CertificatesBackedByIntents,
               HonestDurableIntentsSound,
               FormedTimeoutCertificatesSound
      <3>2. UNCHANGED
               <<timeoutIntents, commitIntents, installedTCs>>
        BY <2>2, Isa DEF StableVars
      <3>3. DurableTimeoutsProtectCommits
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>4. DurableTimeoutsProtectCommits'
        BY <3>2, <3>3,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>5. UNCHANGED
               <<context, prepareQCs, lockRank, lockSubject,
                 highestRank, highestSubject>>
        BY <2>2, Isa DEF StableVars
      <3>6. HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>7. HighestAndLockAreCertified'
        BY <3>5, <3>6,
           UnchangedHighestAndLockCertificationVarsPreserves
      <3> QED BY <3>1, <3>4, <3>7
    <2>15a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, <2>2,
         UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             StableVars, DeliverTC
    <2>16. ReducerProvenanceInvariant'
      BY <2>6, <2>11, <2>12, <2>13, <2>14, <2>15, <2>15a
         DEF ReducerProvenanceInvariant
    <2>17. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, DeliverTC, LineageVars
    <2>18. /\ ContextIdentityBindsFrozenEpoch'
            /\ OldContextCertificateRejected'
            /\ ContextParentWasApplied'
      BY <1>1, <2>2, Isa
         DEF StrongInductiveInvariant, StableVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <2>10, <2>16, <2>17, <2>18
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginInstallTCPreservesStrongInvariant ==
  \A node, tc:
    StrongInductiveInvariant /\ BeginInstallTC(node, tc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW tc,
              StrongInductiveInvariant,
              BeginInstallTC(node, tc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == InstallTcWal(node, tc, FALSE)
    <2> DEFINE StableVars ==
          <<height, context, contextHistory, nodeView, generation, up, gst,
            availableBodies, durableBodies, retainedLockedBodies,
            validatedBodies, invalidBodies,
            seenProposals, receivedVotes, receivedQCs, receivedTimeoutVotes,
            receivedTCs, proposalIntents, prepareIntents, commitIntents,
            timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
            lockRank, lockSubject, highestRank, highestSubject,
            pendingProposal, pendingPrepare, pendingObservePrepare,
            pendingLockCommit, pendingTimeout, pendingDecision,
            signProposals, signVotes, signTimeouts, proposalNetwork,
            voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
            decisions, applied>>
    <2>1. /\ tc \in formedTCs
          /\ TCValid(tc)
          /\ tc.votes # {}
          /\ tc.view + 1 \in Views
          /\ tc.view + 1 >= nodeView[node]
          /\ node \in ValidatorIds
          /\ TcWellTyped(tc)
      <3>1. TcAt(node, tc) \in receivedTCs
        BY <1>1 DEF BeginInstallTC
      <3>2. /\ tc \in formedTCs
            /\ TCValid(tc)
        <4>1. /\ TcAt(node, tc).tc \in formedTCs
              /\ TCValid(TcAt(node, tc).tc)
          BY <1>1, <3>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 TcTransportBacked
        <4>2. TcAt(node, tc).tc = tc
          BY DEF TcAt
        <4> QED BY <4>1, <4>2
      <3>3. /\ node \in ValidatorIds
            /\ TcWellTyped(tc)
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant, TcAt
      <3>4. /\ tc.votes # {}
            /\ tc.view + 1 \in Views
            /\ tc.view + 1 >= nodeView[node]
        BY <1>1, <3>2 DEF BeginInstallTC, TCValid
      <3> QED BY <3>2, <3>3, <3>4
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, BeginInstallTC,
             AllPendingRequests, NodeIdle, Request, InstallTcWal
    <2>3. UNCHANGED StableVars
      BY <1>1 DEF BeginInstallTC, StableVars
    <2>4. /\ pendingInstallTC' = pendingInstallTC \cup {Request}
          /\ Request \in InstallTcWalSet
      <3>1. pendingInstallTC' = pendingInstallTC \cup {Request}
        BY <1>1 DEF BeginInstallTC, Request
      <3>3. Request \in InstallTcWalSet
        BY <2>1, InstallTcWalRecordTyping DEF Request
      <3> QED BY <3>1, <3>3
    <2>5. TypeInvariant'
      BY <1>1, <2>3, <2>4, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             StableVars
    <2>6. /\ Request.tc \in formedTCs'
          /\ Request.tc.context = context'
          /\ TCValid(Request.tc)'
          /\ Request.tc.votes # {}
          /\ Request.tc.view + 1 \in Views
          /\ Request.tc.view + 1 >= nodeView'[Request.node]
      BY <2>1, <2>3, Isa
         DEF StableVars, Request, InstallTcWal,
             TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>7. PendingCertificateWritesAuthorized'
      BY <1>1, <2>3, <2>4, <2>6, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, StableVars,
             TCValid, AuthenticatedHighRef, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>8. Safety'
      BY <1>1, <2>2, <2>3, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, StableVars,
             OnePendingPersistencePerNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>9. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <2>3, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               StableVars, HonestVoteUnique, HonestTimeoutUnique,
               IntentPhasesCorrect
      <3>2. PendingVoteWritesAuthorized'
        <4>1. UNCHANGED
                 <<context, nodeView, durableBodies, receivedQCs,
                   prepareIntents, commitIntents, timeoutIntents,
                   prepareQCs, installedTCs, lockRank, lockSubject,
                   highestRank, highestSubject, pendingPrepare,
                   pendingLockCommit, pendingTimeout>>
          BY <2>3, Isa DEF StableVars
        <4>2. PendingVoteWritesAuthorized
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4> QED BY <4>1, <4>2,
                     UnchangedPendingVoteWriteVarsPreservesAuthorization
      <3>3. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, <2>3, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               StableVars, HonestVoteTransportBacked, QcTransportBacked,
               HonestTimeoutTransportBacked, TcTransportBacked,
               VoteIntentFor, TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>1, <3>2, <3>3
    <2>10. /\ CertificatesBackedByIntents'
           /\ HonestDurableIntentsSound'
           /\ FormedTimeoutCertificatesSound'
           /\ DurableTimeoutsProtectCommits'
           /\ HighestAndLockAreCertified'
      <3>1. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
        BY <1>1, <2>3, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               StableVars, CertificatesBackedByIntents,
               HonestDurableIntentsSound,
               FormedTimeoutCertificatesSound
      <3>2. UNCHANGED
               <<timeoutIntents, commitIntents, installedTCs>>
        BY <2>3, Isa DEF StableVars
      <3>3. DurableTimeoutsProtectCommits
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>4. DurableTimeoutsProtectCommits'
        BY <3>2, <3>3,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>5. UNCHANGED
               <<context, prepareQCs, lockRank, lockSubject,
                 highestRank, highestSubject>>
        BY <2>3, Isa DEF StableVars
      <3>6. HighestAndLockAreCertified
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>7. HighestAndLockAreCertified'
        BY <3>5, <3>6,
           UnchangedHighestAndLockCertificationVarsPreserves
      <3> QED BY <3>1, <3>4, <3>7
    <2>10a. DurableLockRecoveryProvenanceInvariant'
      BY <1>1, <2>3,
         UnchangedDurableLockRecoveryProvenanceVarsPreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             StableVars, BeginInstallTC
    <2>11. ReducerProvenanceInvariant'
      BY <2>7, <2>9, <2>10, <2>10a DEF ReducerProvenanceInvariant
    <2>12. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, BeginInstallTC, LineageVars
    <2>13. /\ ContextIdentityBindsFrozenEpoch'
           /\ OldContextCertificateRejected'
           /\ ContextParentWasApplied'
      BY <1>1, <2>3, Isa
         DEF StrongInductiveInvariant, StableVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, QcWireValid, CurrentEpoch
    <2> QED BY <2>8, <2>11, <2>12, <2>13
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM PersistInstallTCPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistInstallTC(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistInstallTC(request)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE SelectedRank == TcHighRank(Certificate)
    <2> DEFINE SelectedSubject == TcHighSubject(Certificate)
    <2> DEFINE SameRoundUpgrade ==
          StrictSameRoundTcUpgrade(Node, Certificate)
    <2> DEFINE StableVars ==
          <<height, context, contextHistory, up, gst, availableBodies,
            durableBodies, retainedLockedBodies, invalidBodies, seenProposals,
            receivedQCs, receivedTimeoutVotes, receivedTCs,
            proposalIntents, prepareIntents, commitIntents, timeoutIntents,
            prepareQCs, commitQCs, formedTCs, pendingProposal,
            pendingPrepare, pendingObservePrepare, pendingLockCommit,
            pendingTimeout, pendingDecision, signProposals,
            signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
            timeoutNetwork, decisions, applied>>
    <2>1. /\ request \in pendingInstallTC
          /\ request \in InstallTcWalSet
          /\ Certificate \in formedTCs
          /\ TCValid(Certificate)
          /\ Certificate.votes # {}
          /\ Certificate.view + 1 \in Views
          /\ \/ Certificate.view >= nodeView[Node]
             \/ SameRoundUpgrade
          /\ Node \in ValidatorIds
          /\ TcWellTyped(Certificate)
          /\ request.rebroadcast \in BOOLEAN
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, PersistInstallTC,
             InstallTcWalSet, TcRecordSet, TcWellTyped,
             Certificate, Node, SameRoundUpgrade
    <2>2. /\ SelectedRank \in Ranks
          /\ (SelectedRank = NoRank => SelectedSubject = NoSubject)
          /\ (SelectedRank # NoRank
                => \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = SelectedRank
                     /\ qc.subject = SelectedSubject)
          /\ SelectedSubject \in SubjectOrNone
      <3>1. ModelConfiguration
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. HighestTimeoutVote(Certificate.votes)
               \in Certificate.votes
        BY <2>1, <3>1, ValidTimeoutCertificateSelectsMember
      <3>3. HighRefValid(
               HighestTimeoutVote(Certificate.votes).highRank,
               HighestTimeoutVote(Certificate.votes).highSubject)
        BY <2>1, <3>2 DEF TCValid, AuthenticatedHighRef
      <3>4. \/ /\ SelectedRank = NoRank
                  /\ SelectedSubject = NoSubject
             \/ /\ SelectedRank \in Views
                  /\ SelectedSubject \in Subjects
                  /\ \E qc \in prepareQCs:
                       /\ qc.context = context
                       /\ qc.view = SelectedRank
                       /\ qc.subject = SelectedSubject
        BY <3>3
           DEF HighRefValid, TcHighRank, TcHighSubject,
               SelectedRank, SelectedSubject, Certificate
      <3>5. SelectedRank \in Ranks
        BY <3>4, SMT DEF Views, Ranks, NoRank
      <3>6. SelectedRank = NoRank => SelectedSubject = NoSubject
        BY <3>1, <3>4, SMT DEF ModelConfiguration, Views, NoRank
      <3>7. SelectedRank # NoRank
               => \E qc \in prepareQCs:
                    /\ qc.context = context
                    /\ qc.view = SelectedRank
                    /\ qc.subject = SelectedSubject
        BY <3>4
      <3>8. SelectedSubject \in SubjectOrNone
        BY <3>4, Isa DEF SubjectOrNone
      <3> QED BY <3>5, <3>6, <3>7, <3>8
    <2>3. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistInstallTC
    <2>4. TypeInvariant'
      <3>1. TypeInvariant
        BY <1>1 DEF StrongInductiveInvariant, Safety
      <3>2. UNCHANGED StableVars
        BY <1>1 DEF PersistInstallTC, StableVars
      <3>2a. /\ validatedBodies' \subseteq ValidationRecordSet
              /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
        BY <1>1, <3>1,
           PersistInstallTCPreservesValidationReceiptTypeAndSoundness
      <3>3. /\ nodeView' =
                   [nodeView EXCEPT ![Node] =
                      IF SameRoundUpgrade THEN @
                      ELSE Certificate.view + 1]
            /\ generation' =
                   [generation EXCEPT ![Node] =
                      IF SameRoundUpgrade THEN @ + 1 ELSE 0]
            /\ highestRank' =
                   [highestRank EXCEPT ![Node] =
                      IF SelectedRank > highestRank[Node]
                      THEN SelectedRank ELSE @]
            /\ highestSubject' =
                   [highestSubject EXCEPT ![Node] =
                      IF SelectedRank > highestRank[Node]
                      THEN SelectedSubject ELSE @]
            /\ lockRank' =
                   [lockRank EXCEPT ![Node] =
                      IF SelectedRank > lockRank[Node]
                      THEN SelectedRank ELSE @]
            /\ lockSubject' =
                   [lockSubject EXCEPT ![Node] =
                      IF SelectedRank > lockRank[Node]
                      THEN SelectedSubject ELSE @]
        BY <1>1
           DEF PersistInstallTC, Node, Certificate,
               SelectedRank, SelectedSubject, SameRoundUpgrade
      <3>4. nodeView' \in [ValidatorIds -> Views]
        BY <2>1, <3>1, <3>3, Isa
           DEF TypeInvariant
      <3>5. generation' \in [ValidatorIds -> Generations]
        <4>1. generation \in [ValidatorIds -> Generations]
          BY <3>1 DEF TypeInvariant
        <4>2. Node \in ValidatorIds
          BY <2>1
        <4>3. (IF SameRoundUpgrade
               THEN generation[Node] + 1 ELSE 0) \in Generations
          BY <1>1, <3>1, SMT
             DEF PersistInstallTC, TypeInvariant, ModelConfiguration,
                 Generations, GenerationCanIncrement, SameRoundUpgrade,
                 Node, Certificate
        <4> QED BY <3>3, <4>1, <4>2, <4>3, Isa
      <3>6. /\ highestRank' \in [ValidatorIds -> Ranks]
            /\ lockRank' \in [ValidatorIds -> Ranks]
            /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
            /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
        <4>1. /\ highestRank \in [ValidatorIds -> Ranks]
              /\ lockRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
              /\ Node \in ValidatorIds
          BY <2>1, <3>1 DEF TypeInvariant
        <4>2. /\ (IF SelectedRank > highestRank[Node]
                       THEN SelectedRank ELSE highestRank[Node]) \in Ranks
              /\ (IF SelectedRank > lockRank[Node]
                       THEN SelectedRank ELSE lockRank[Node]) \in Ranks
              /\ (IF SelectedRank > highestRank[Node]
                       THEN SelectedSubject
                       ELSE highestSubject[Node]) \in SubjectOrNone
              /\ (IF SelectedRank > lockRank[Node]
                       THEN SelectedSubject
                       ELSE lockSubject[Node]) \in SubjectOrNone
          BY <2>2, <4>1, Isa
        <4>3. highestRank' \in [ValidatorIds -> Ranks]
          BY <3>3, <4>1, <4>2, Isa
        <4>4. lockRank' \in [ValidatorIds -> Ranks]
          BY <3>3, <4>1, <4>2, Isa
        <4>5. highestSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>3, <4>1, <4>2, Isa
        <4>6. lockSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>3, <4>1, <4>2, Isa
        <4> QED BY <4>3, <4>4, <4>5, <4>6
      <3>6a. \A other \in ValidatorIds:
                 generation'[other] <= highestRank'[other] + 1
        BY <1>1, <2>1, <2>2, <3>1, <3>3, SMT
           DEF TypeInvariant, ModelConfiguration, Ranks, Views, NoRank,
               SameRoundUpgrade, StrictSameRoundTcUpgrade,
               SelectedRank, Certificate, Node
      <3>7. installedTCs' =
               installedTCs
                 \cup {[node |-> Node, tc |-> Certificate]}
        BY <1>1
           DEF PersistInstallTC, Node, Certificate
      <3>8. \A installed \in installedTCs':
               /\ installed.node \in ValidatorIds
               /\ TcWellTyped(installed.tc)
        BY <2>1, <3>1, <3>7, Isa DEF TypeInvariant
      <3>9. pendingInstallTC' = pendingInstallTC \ {request}
        BY <1>1 DEF PersistInstallTC
      <3>10. pendingInstallTC' \subseteq InstallTcWalSet
        BY <3>1, <3>9, Isa DEF TypeInvariant
      <3>11. signVotes' \subseteq VoteSignSet
        <4>1. /\ Node \in ValidatorIds
              /\ commitIntents \subseteq VoteRecordSet
              /\ signVotes \subseteq VoteSignSet
          BY <2>1, <3>1 DEF TypeInvariant
        <4>2. \A vote \in ExactLockedCommitIntents(
                         Node,
                         ResultingInstallLockRank(Node, Certificate),
                         ResultingInstallLockSubject(Node, Certificate)):
                 VoteSign(Node, vote) \in VoteSignSet
          <5>1. ASSUME NEW vote \in ExactLockedCommitIntents(
                                 Node,
                                 ResultingInstallLockRank(
                                   Node, Certificate),
                                 ResultingInstallLockSubject(
                                   Node, Certificate))
                 PROVE VoteSign(Node, vote) \in VoteSignSet
            <6>1. vote \in VoteRecordSet
              BY <4>1, <5>1 DEF ExactLockedCommitIntents
            <6> QED BY <4>1, <6>1 DEF VoteSign, VoteSignSet
          <5> QED BY <5>1
        <4>3. ActiveLockedCommitSignRequestsAfterInstall(
                   Node, Certificate) \subseteq VoteSignSet
          BY <4>2 DEF ActiveLockedCommitSignRequestsAfterInstall
        <4>4. signVotes' =
                 signVotes
                   \cup ActiveLockedCommitSignRequestsAfterInstall(
                          Node, Certificate)
          BY <1>1
             DEF PersistInstallTC, Node, Certificate
        <4> QED BY <4>1, <4>3, <4>4, Isa
      <3> QED BY <3>1, <3>2, <3>2a, <3>4, <3>5, <3>6, <3>6a, <3>8,
                   <3>10, <3>11, Isa
         DEF TypeInvariant, StableVars
    <2>5. LockBelowHighest'
      <3>1. /\ lockRank' =
                       [lockRank EXCEPT ![Node] =
                          IF SelectedRank > lockRank[Node]
                          THEN SelectedRank ELSE @]
            /\ highestRank' =
                       [highestRank EXCEPT ![Node] =
                          IF SelectedRank > highestRank[Node]
                          THEN SelectedRank ELSE @]
        BY <1>1
           DEF PersistInstallTC, Node, Certificate,
               SelectedRank, SelectedSubject
      <3>2. ASSUME NEW other \in ValidatorIds
             PROVE lockRank'[other] <= highestRank'[other]
        <4>1. /\ lockRank \in [ValidatorIds -> Ranks]
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ Node \in ValidatorIds
              /\ lockRank[other] \in Ranks
              /\ highestRank[other] \in Ranks
              /\ SelectedRank \in Ranks
              /\ lockRank[other] <= highestRank[other]
          BY <1>1, <2>1, <2>2, <3>2, Isa
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 LockBelowHighest
        <4>2. /\ lockRank[other] \in Int
              /\ highestRank[other] \in Int
              /\ SelectedRank \in Int
          BY <1>1, <4>1, SMT
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 ModelConfiguration, Ranks, Views, NoRank
        <4>3. CASE other = Node
          <5>1. /\ lockRank'[other] =
                       IF SelectedRank > lockRank[other]
                       THEN SelectedRank ELSE lockRank[other]
                /\ highestRank'[other] =
                       IF SelectedRank > highestRank[other]
                       THEN SelectedRank ELSE highestRank[other]
            BY <3>1, <4>1, <4>3, Isa
          <5> QED BY <4>1, <4>2, <5>1, SMT
        <4>4. CASE other # Node
          <5>1. /\ lockRank'[other] = lockRank[other]
                /\ highestRank'[other] = highestRank[other]
            BY <3>1, <4>1, <4>4, Isa
          <5> QED BY <4>1, <5>1
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>2 DEF LockBelowHighest
    <2>6. HighestAndLockAreCertified'
      <3> DEFINE CertifiedRefAt(
                   certificateContext, certificates,
                   rankValue, subjectValue) ==
          /\ (rankValue = NoRank => subjectValue = NoSubject)
          /\ (rankValue # NoRank
                => \E qc \in certificates:
                     /\ qc.context = certificateContext
                     /\ qc.view = rankValue
                     /\ qc.subject = subjectValue)
      <3>1. \A other \in ValidatorIds:
               /\ CertifiedRefAt(
                    context, prepareQCs,
                    highestRank[other], highestSubject[other])
               /\ CertifiedRefAt(
                    context, prepareQCs,
                    lockRank[other], lockSubject[other])
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HighestAndLockAreCertified, CertifiedRefAt
      <3>2. CertifiedRefAt(
               context, prepareQCs, SelectedRank, SelectedSubject)
        BY <2>2 DEF CertifiedRefAt
      <3>3. /\ context' = context
            /\ prepareQCs' = prepareQCs
            /\ highestRank' =
                 [highestRank EXCEPT ![Node] =
                    IF SelectedRank > highestRank[Node]
                    THEN SelectedRank ELSE @]
            /\ highestSubject' =
                 [highestSubject EXCEPT ![Node] =
                    IF SelectedRank > highestRank[Node]
                    THEN SelectedSubject ELSE @]
            /\ lockRank' =
                 [lockRank EXCEPT ![Node] =
                    IF SelectedRank > lockRank[Node]
                    THEN SelectedRank ELSE @]
            /\ lockSubject' =
                 [lockSubject EXCEPT ![Node] =
                    IF SelectedRank > lockRank[Node]
                    THEN SelectedSubject ELSE @]
        BY <1>1
           DEF PersistInstallTC, Node, Certificate,
               SelectedRank, SelectedSubject
      <3>4. ASSUME NEW other \in ValidatorIds
             PROVE /\ CertifiedRefAt(
                         context', prepareQCs',
                         highestRank'[other], highestSubject'[other])
                   /\ CertifiedRefAt(
                         context', prepareQCs',
                         lockRank'[other], lockSubject'[other])
        <4>1. /\ CertifiedRefAt(
                     context, prepareQCs,
                     highestRank[other], highestSubject[other])
               /\ CertifiedRefAt(
                     context, prepareQCs,
                     lockRank[other], lockSubject[other])
          BY <3>1, <3>4
        <4>2. /\ Node \in ValidatorIds
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
              /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1, <2>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>3. CASE other = Node
          <5>1. /\ highestRank'[other] =
                       IF SelectedRank > highestRank[other]
                       THEN SelectedRank ELSE highestRank[other]
                /\ highestSubject'[other] =
                       IF SelectedRank > highestRank[other]
                       THEN SelectedSubject ELSE highestSubject[other]
                /\ lockRank'[other] =
                       IF SelectedRank > lockRank[other]
                       THEN SelectedRank ELSE lockRank[other]
                /\ lockSubject'[other] =
                       IF SelectedRank > lockRank[other]
                       THEN SelectedSubject ELSE lockSubject[other]
            BY <3>3, <4>2, <4>3, Isa
          <5>2. CertifiedRefAt(
                   context', prepareQCs',
                   highestRank'[other], highestSubject'[other])
            <6>1. CASE SelectedRank > highestRank[other]
              BY <3>2, <3>3, <5>1, <6>1, Isa
            <6>2. CASE ~(SelectedRank > highestRank[other])
              BY <3>3, <4>1, <5>1, <6>2, Isa
            <6> QED BY <6>1, <6>2
          <5>3. CertifiedRefAt(
                   context', prepareQCs',
                   lockRank'[other], lockSubject'[other])
            <6>1. CASE SelectedRank > lockRank[other]
              BY <3>2, <3>3, <5>1, <6>1, Isa
            <6>2. CASE ~(SelectedRank > lockRank[other])
              BY <3>3, <4>1, <5>1, <6>2, Isa
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>2, <5>3
        <4>4. CASE other # Node
          <5>1. /\ highestRank'[other] = highestRank[other]
                /\ highestSubject'[other] = highestSubject[other]
                /\ lockRank'[other] = lockRank[other]
                /\ lockSubject'[other] = lockSubject[other]
            BY <3>3, <4>2, <4>4, Isa
          <5> QED BY <3>3, <4>1, <5>1, Isa
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>4
         DEF HighestAndLockAreCertified, CertifiedRefAt
    <2>7. /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      <3>1. /\ PendingVoteWritesAuthorized
            /\ PendingCertificateWritesAuthorized
            /\ RequestsUniqueByNode(AllPendingRequests)
            /\ request \in AllPendingRequests
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode,
               ReducerProvenanceInvariant, AllPendingRequests
      <3>2. \A other \in AllPendingRequests:
               other.node \in ValidatorIds
        BY <1>1, TypeInvariantTypesAllPendingNodes
           DEF StrongInductiveInvariant, Safety
      <3>3. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF AllPendingRequests, PersistInstallTC
      <3>4. request \notin AllPendingRequests'
        <4>1. /\ pendingProposal' \subseteq ProposalWalSet
              /\ pendingPrepare' \subseteq PrepareWalSet
              /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
              /\ pendingLockCommit' \subseteq LockCommitWalSet
              /\ pendingTimeout' \subseteq TimeoutWalSet
              /\ (\A pending \in pendingInstallTC':
                    pending.kind = "InstallTC")
              /\ pendingDecision' \subseteq DecisionWalSet
          BY <2>4 DEF TypeInvariant, InstallTcWalSet
        <4>2. request.kind = "InstallTC"
          BY <1>1, <2>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 InstallTcWalSet
        <4>3. pendingInstallTC' = pendingInstallTC \ {request}
          BY <1>1 DEF PersistInstallTC
        <4>4. request \notin ProposalWalSet \cup PrepareWalSet
                              \cup ObservePrepareWalSet
                              \cup LockCommitWalSet \cup TimeoutWalSet
                              \cup DecisionWalSet
          BY <4>2, InstallKindExcludesOtherWalSets
        <4>5. request \notin pendingProposal' \cup pendingPrepare'
                              \cup pendingObservePrepare'
                              \cup pendingLockCommit' \cup pendingTimeout'
                              \cup pendingDecision'
          BY <4>1, <4>4, Isa
        <4>6. request \notin pendingInstallTC'
          BY <4>3
        <4> QED BY <4>5, <4>6 DEF AllPendingRequests
      <3>5. \A other \in AllPendingRequests': other.node # Node
        <4>1. ASSUME NEW other \in AllPendingRequests'
               PROVE other.node # Node
          <5>1. /\ other \in AllPendingRequests
                /\ other # request
            BY <3>3, <3>4, <4>1
          <5> QED BY <3>1, <5>1,
                       DistinctUniqueRequestsHaveDistinctNodes
             DEF Node
        <4> QED BY <4>1
      <3>6. \A other \in AllPendingRequests':
               /\ other.node \in ValidatorIds
               /\ nodeView'[other.node] = nodeView[other.node]
               /\ highestRank'[other.node] = highestRank[other.node]
               /\ highestSubject'[other.node] = highestSubject[other.node]
               /\ lockRank'[other.node] = lockRank[other.node]
               /\ lockSubject'[other.node] = lockSubject[other.node]
        <4>1. /\ nodeView \in [ValidatorIds -> Views]
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
              /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. /\ nodeView' =
                       [nodeView EXCEPT ![Node] =
                          IF SameRoundUpgrade THEN @
                          ELSE Certificate.view + 1]
              /\ highestRank' =
                       [highestRank EXCEPT ![Node] =
                          IF SelectedRank > highestRank[Node]
                          THEN SelectedRank ELSE @]
              /\ highestSubject' =
                       [highestSubject EXCEPT ![Node] =
                          IF SelectedRank > highestRank[Node]
                          THEN SelectedSubject ELSE @]
              /\ lockRank' =
                       [lockRank EXCEPT ![Node] =
                          IF SelectedRank > lockRank[Node]
                          THEN SelectedRank ELSE @]
              /\ lockSubject' =
                       [lockSubject EXCEPT ![Node] =
                          IF SelectedRank > lockRank[Node]
                          THEN SelectedSubject ELSE @]
          BY <1>1
             DEF PersistInstallTC, Node, Certificate,
                 SelectedRank, SelectedSubject, SameRoundUpgrade
        <4>3. ASSUME NEW other \in AllPendingRequests'
               PROVE /\ other.node \in ValidatorIds
                     /\ nodeView'[other.node] = nodeView[other.node]
                     /\ highestRank'[other.node] = highestRank[other.node]
                     /\ highestSubject'[other.node]
                            = highestSubject[other.node]
                     /\ lockRank'[other.node] = lockRank[other.node]
                     /\ lockSubject'[other.node] = lockSubject[other.node]
          <5>1. other.node \in ValidatorIds
            BY <3>2, <3>3, <4>3
          <5>2. other.node # Node
            BY <3>5, <4>3
          <5> QED BY <4>1, <4>2, <5>1, <5>2, Isa
        <4> QED BY <4>3
      <3>7. UNCHANGED
               <<context, height, durableBodies,
                 prepareIntents, commitIntents, timeoutIntents,
                 prepareQCs, commitQCs, formedTCs>>
        BY <1>1 DEF PersistInstallTC
      <3>8. /\ pendingPrepare' = pendingPrepare
            /\ pendingObservePrepare' = pendingObservePrepare
            /\ pendingLockCommit' = pendingLockCommit
            /\ pendingTimeout' = pendingTimeout
            /\ pendingInstallTC' = pendingInstallTC \ {request}
            /\ pendingDecision' = pendingDecision
        BY <1>1 DEF PersistInstallTC
      <3>9. \A other \in pendingPrepare':
               /\ other.node \in Honest
               /\ other.vote.phase = "Prepare"
               /\ other.vote.signer = other.node
               /\ other.vote.context = context'
               /\ other.vote.view = nodeView'[other.node]
               /\ other.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', other.node,
                             other.vote.context, other.vote.view, other.vote.subject)
               /\ CanAppendVote(prepareIntents', other.vote)
               /\ PrepareCarriesHigherSafeQc(other.vote)'
        <4>1. ASSUME NEW other \in pendingPrepare'
               PROVE /\ other.node \in Honest
                     /\ other.vote.phase = "Prepare"
                     /\ other.vote.signer = other.node
                     /\ other.vote.context = context'
                     /\ other.vote.view = nodeView'[other.node]
                     /\ other.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', other.node,
                                   other.vote.context, other.vote.view, other.vote.subject)
                     /\ CanAppendVote(prepareIntents', other.vote)
                     /\ PrepareCarriesHigherSafeQc(other.vote)'
          <5>1. other \in AllPendingRequests'
            BY <4>1 DEF AllPendingRequests
          <5>2. /\ other.node \in Honest
                /\ other.vote.phase = "Prepare"
                /\ other.vote.signer = other.node
                /\ other.vote.context = context
                /\ other.vote.view = nodeView[other.node]
                /\ other.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies, other.node,
                              other.vote.context, other.vote.view, other.vote.subject)
                /\ CanAppendVote(prepareIntents, other.vote)
                /\ PrepareCarriesHigherSafeQc(other.vote)
            BY <3>1, <3>8, <4>1
               DEF PendingVoteWritesAuthorized
          <5>3. /\ context' = context
                /\ durableBodies' = durableBodies
                /\ prepareIntents' = prepareIntents
                /\ commitIntents' = commitIntents
                /\ prepareQCs' = prepareQCs
                /\ nodeView'[other.node] = nodeView[other.node]
            BY <1>1, <3>6, <3>7, <5>1 DEF PersistInstallTC
          <5>4. PrepareCarriesHigherSafeQc(other.vote)'
                    <=> PrepareCarriesHigherSafeQc(other.vote)
            BY <3>7 DEF PrepareCarriesHigherSafeQc
          <5> QED BY <5>2, <5>3, <5>4, Isa
        <4> QED BY <4>1
      <3>10. \A other \in pendingLockCommit':
                /\ other.node \in Honest
                /\ other.vote =
                     Vote(context', other.qc.view, "Commit",
                          other.qc.subject, other.node)
                /\ other.vote.phase = "Commit"
                /\ other.vote.signer = other.node
                /\ other.vote.context = context'
                /\ other.vote.context = other.qc.context
                /\ other.vote.view = other.qc.view
                /\ other.vote.subject = other.qc.subject
                /\ other.qc.phase = "Prepare"
                /\ other.qc \in prepareQCs'
                /\ \/ CurrentOpenPrepareForCommit(
                         other.node, other.qc)'
                   \/ HistoricalLockedPrepareForCommit(
                         other.node, other.qc)'
                /\ other.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies', other.node,
                              other.vote.context, other.vote.view, other.vote.subject)
                /\ other.qc.view >= lockRank'[other.node]
                /\ (other.qc.view = lockRank'[other.node]
                      => other.qc.subject = lockSubject'[other.node])
                /\ CanAppendVote(commitIntents', other.vote)
        <4>1. ASSUME NEW other \in pendingLockCommit'
               PROVE /\ other.node \in Honest
                     /\ other.vote =
                          Vote(context', other.qc.view, "Commit",
                               other.qc.subject, other.node)
                     /\ other.vote.phase = "Commit"
                     /\ other.vote.signer = other.node
                     /\ other.vote.context = context'
                     /\ other.vote.context = other.qc.context
                     /\ other.vote.view = other.qc.view
                     /\ other.vote.subject = other.qc.subject
                     /\ other.qc.phase = "Prepare"
                     /\ other.qc \in prepareQCs'
                     /\ \/ CurrentOpenPrepareForCommit(
                              other.node, other.qc)'
                        \/ HistoricalLockedPrepareForCommit(
                              other.node, other.qc)'
                     /\ other.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', other.node,
                                   other.vote.context, other.vote.view, other.vote.subject)
                     /\ other.qc.view >= lockRank'[other.node]
                     /\ (other.qc.view = lockRank'[other.node]
                           => other.qc.subject = lockSubject'[other.node])
                     /\ CanAppendVote(commitIntents', other.vote)
          <5>1. other \in AllPendingRequests'
            BY <4>1 DEF AllPendingRequests
          <5>2. /\ other.node \in Honest
                /\ other.vote =
                     Vote(context, other.qc.view, "Commit",
                          other.qc.subject, other.node)
                /\ other.vote.phase = "Commit"
                /\ other.vote.signer = other.node
                /\ other.vote.context = context
                /\ other.vote.context = other.qc.context
                /\ other.vote.view = other.qc.view
                /\ other.vote.subject = other.qc.subject
                /\ other.qc.phase = "Prepare"
                /\ other.qc \in prepareQCs
                /\ \/ CurrentOpenPrepareForCommit(
                         other.node, other.qc)
                   \/ HistoricalLockedPrepareForCommit(
                         other.node, other.qc)
                /\ other.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies, other.node,
                              other.vote.context, other.vote.view, other.vote.subject)
                /\ other.qc.view >= lockRank[other.node]
                /\ (other.qc.view = lockRank[other.node]
                      => other.qc.subject = lockSubject[other.node])
                /\ CanAppendVote(commitIntents, other.vote)
            BY <3>1, <3>8, <4>1
               DEF PendingVoteWritesAuthorized
          <5>3. /\ context' = context
                /\ durableBodies' = durableBodies
                /\ timeoutIntents' = timeoutIntents
                /\ receivedQCs' = receivedQCs
                /\ prepareQCs' = prepareQCs
                /\ commitIntents' = commitIntents
                /\ nodeView'[other.node] = nodeView[other.node]
                /\ lockRank'[other.node] = lockRank[other.node]
                /\ lockSubject'[other.node] = lockSubject[other.node]
            BY <1>1, <3>6, <3>7, <5>1 DEF PersistInstallTC
          <5>4. installedTCs \subseteq installedTCs'
            BY <1>1, Isa DEF PersistInstallTC
          <5>5. CurrentOpenPrepareForCommit(other.node, other.qc)
                   => CurrentOpenPrepareForCommit(other.node, other.qc)'
            BY <1>1, <3>6, <3>7, <5>1, <5>3, Isa
               DEF PersistInstallTC, CurrentOpenPrepareForCommit,
                   NodeTimedOut
          <5>6. InstalledTcSelectsPrepareFor(other.node, other.qc)
                   => InstalledTcSelectsPrepareFor(other.node, other.qc)'
            BY <5>4, Isa DEF InstalledTcSelectsPrepareFor
          <5>7. NoHigherPrepareOriginKnown(other.node, other.qc)
                   <=> NoHigherPrepareOriginKnown(
                        other.node, other.qc)'
            BY <3>6, <3>7, <5>1, Isa
               DEF NoHigherPrepareOriginKnown
          <5>8. HistoricalLockedPrepareForCommit(
                   other.node, other.qc)
                   => HistoricalLockedPrepareForCommit(
                        other.node, other.qc)'
            BY <5>3, <5>6, <5>7, Isa
               DEF HistoricalLockedPrepareForCommit
          <5> QED BY <5>2, <5>3, <5>5, <5>8
        <4> QED BY <4>1
      <3>11. \A other \in pendingTimeout':
                /\ other.node \in Honest
                /\ other.vote.signer = other.node
                /\ other.vote.context = context'
                /\ other.vote.view = nodeView'[other.node]
                /\ CanAppendTimeout(timeoutIntents', other.vote)
                /\ TimeoutVoteProtectsCommitSet(
                     other.vote, commitIntents)'
        <4>1. ASSUME NEW other \in pendingTimeout'
               PROVE /\ other.node \in Honest
                     /\ other.vote.signer = other.node
                     /\ other.vote.context = context'
                     /\ other.vote.view = nodeView'[other.node]
                     /\ CanAppendTimeout(timeoutIntents', other.vote)
                     /\ TimeoutVoteProtectsCommitSet(
                          other.vote, commitIntents)'
          <5>1. other \in AllPendingRequests'
            BY <4>1 DEF AllPendingRequests
          <5>2. /\ other.node \in Honest
                /\ other.vote.signer = other.node
                /\ other.vote.context = context
                /\ other.vote.view = nodeView[other.node]
                /\ CanAppendTimeout(timeoutIntents, other.vote)
                /\ TimeoutVoteProtectsCommitSet(other.vote,
                                                commitIntents)
            BY <3>1, <3>8, <4>1
               DEF PendingVoteWritesAuthorized
          <5>3. /\ context' = context
                /\ timeoutIntents' = timeoutIntents
                /\ commitIntents' = commitIntents
                /\ nodeView'[other.node] = nodeView[other.node]
            BY <3>6, <3>7, <5>1
          <5>4. installedTCs \subseteq installedTCs'
            BY <1>1, Isa DEF PersistInstallTC
          <5>5. \A commitVote:
                   InstalledTcAuthorizesCommitVote(commitVote)
                     => InstalledTcAuthorizesCommitVote(commitVote)'
            BY <5>4, Isa DEF InstalledTcAuthorizesCommitVote
          <5>6. TimeoutVoteProtectsCommitSet(
                   other.vote, commitIntents)'
            BY <5>2, <5>3, <5>5, Isa
               DEF TimeoutVoteProtectsCommitSet
          <5> QED BY <5>2, <5>3, <5>6
        <4> QED BY <4>1
      <3>12. PendingVoteWritesAuthorized'
        BY <3>9, <3>10, <3>11 DEF PendingVoteWritesAuthorized
      <3>13. \A other \in pendingObservePrepare':
                /\ other.qc \in prepareQCs'
                /\ other.qc.context = context'
                /\ other.qc.view > highestRank'[other.node]
        <4>1. ASSUME NEW other \in pendingObservePrepare'
               PROVE /\ other.qc \in prepareQCs'
                     /\ other.qc.context = context'
                     /\ other.qc.view > highestRank'[other.node]
          <5>1. other \in AllPendingRequests'
            BY <4>1 DEF AllPendingRequests
          <5>2. /\ other.qc \in prepareQCs
                /\ other.qc.context = context
                /\ other.qc.view > highestRank[other.node]
            BY <3>1, <3>8, <4>1
               DEF PendingCertificateWritesAuthorized
          <5>3. /\ prepareQCs' = prepareQCs
                /\ context' = context
                /\ highestRank'[other.node] = highestRank[other.node]
            BY <3>6, <3>7, <5>1
          <5> QED BY <5>2, <5>3, Isa
        <4> QED BY <4>1
      <3>14. \A other \in pendingInstallTC':
                /\ other.tc \in formedTCs'
                /\ other.tc.context = context'
                /\ TCValid(other.tc)'
                /\ other.tc.votes # {}
                /\ other.tc.view + 1 \in Views
                /\ other.tc.view + 1 >= nodeView'[other.node]
        <4>1. ASSUME NEW other \in pendingInstallTC'
               PROVE /\ other.tc \in formedTCs'
                     /\ other.tc.context = context'
                     /\ TCValid(other.tc)'
                     /\ other.tc.votes # {}
                     /\ other.tc.view + 1 \in Views
                     /\ other.tc.view + 1 >= nodeView'[other.node]
          <5>1. other \in AllPendingRequests'
            BY <4>1 DEF AllPendingRequests
          <5>2. /\ other.tc \in formedTCs
                /\ other.tc.context = context
                /\ TCValid(other.tc)
                /\ other.tc.votes # {}
                /\ other.tc.view + 1 \in Views
                /\ other.tc.view + 1 >= nodeView[other.node]
            BY <3>1, <3>8, <4>1
               DEF PendingCertificateWritesAuthorized
          <5>3. /\ formedTCs' = formedTCs
                /\ context' = context
                /\ height' = height
                /\ prepareQCs' = prepareQCs
                /\ nodeView'[other.node] = nodeView[other.node]
            BY <3>6, <3>7, <5>1
          <5>4. TCValid(other.tc)' <=> TCValid(other.tc)
            BY <5>3, Isa
               DEF TCValid, AuthenticatedHighRef, HighRefValid,
                   CurrentEpoch, CurrentVoters
          <5> QED BY <5>2, <5>3, <5>4, Isa
        <4> QED BY <4>1
      <3>15. \A other \in pendingDecision':
                /\ other.qc \in commitQCs'
                /\ other.qc.context = context'
                /\ other.qc.phase = "Commit"
                /\ other.qc.height = height'
        BY <3>1, <3>7, <3>8, Isa
           DEF PendingCertificateWritesAuthorized
      <3>16. PendingCertificateWritesAuthorized'
        BY <3>13, <3>14, <3>15
           DEF PendingCertificateWritesAuthorized
      <3> QED BY <3>12, <3>16
    <2>8. /\ TcTransportBacked'
          /\ FormedTimeoutCertificatesSound'
      <3>1. /\ TcTransportBacked
            /\ FormedTimeoutCertificatesSound
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. /\ context' = context
            /\ height' = height
            /\ prepareQCs' = prepareQCs
            /\ formedTCs' = formedTCs
            /\ timeoutIntents' = timeoutIntents
            /\ receivedTCs' = receivedTCs
        BY <1>1 DEF PersistInstallTC
      <3>3. \A tc: TCValid(tc)' <=> TCValid(tc)
        BY <3>2, Isa
           DEF TCValid, AuthenticatedHighRef, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>4. FormedTimeoutCertificatesSound'
        BY <3>1, <3>2, Isa DEF FormedTimeoutCertificatesSound
      <3>5. /\ tcNetwork' =
                 IF request.rebroadcast
                 THEN tcNetwork \cup BroadcastTCs(Certificate)
                 ELSE tcNetwork
            /\ installedTCs' =
                 installedTCs \cup {[node |-> Node, tc |-> Certificate]}
        BY <1>1 DEF PersistInstallTC, Node, Certificate
      <3>6. \A envelope \in tcNetwork':
               /\ envelope.tc \in formedTCs'
               /\ TCValid(envelope.tc)'
        <4>1. ASSUME NEW envelope \in tcNetwork'
               PROVE /\ envelope.tc \in formedTCs'
                     /\ TCValid(envelope.tc)'
          <5>1. CASE envelope \in tcNetwork
            <6>1. /\ envelope.tc \in formedTCs
                  /\ TCValid(envelope.tc)
              BY <3>1, <5>1 DEF TcTransportBacked
            <6> QED BY <3>2, <3>3, <6>1
          <5>2. CASE envelope \notin tcNetwork
            <6>1. envelope \in BroadcastTCs(Certificate)
              BY <2>1, <3>5, <4>1, <5>2, Isa
            <6>2. envelope.tc = Certificate
              BY <6>1, Isa DEF BroadcastTCs, TcEnvelope
            <6> QED BY <2>1, <3>2, <3>3, <6>2
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>7. \A received \in receivedTCs':
               /\ received.tc \in formedTCs'
               /\ TCValid(received.tc)'
        BY <3>1, <3>2, <3>3, Isa DEF TcTransportBacked
      <3>8. \A installed \in installedTCs':
               installed.tc \in formedTCs'
        BY <2>1, <3>1, <3>2, <3>5, Isa
           DEF TcTransportBacked
      <3>9. TcTransportBacked'
        BY <3>6, <3>7, <3>8 DEF TcTransportBacked
      <3> QED BY <3>4, <3>9
    <2>9. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      <3>1. /\ ProposalSigningRequiresIntent'
            /\ PrepareSigningRequiresIntent'
            /\ CommitSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
            /\ HonestPrepareUniqueness'
            /\ HonestCommitUniqueness'
            /\ HonestTimeoutUniqueness'
            /\ DecisionAgreement'
            /\ AppliedRequiresDecision'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, PersistInstallTC,
               ActiveLockedCommitSignRequestsAfterInstall,
               ExactLockedCommitIntents, VoteSign,
               ProposalSigningRequiresIntent,
               PrepareSigningRequiresIntent,
               CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
               HonestPrepareUniqueness, HonestCommitUniqueness,
               HonestTimeoutUniqueness, DecisionAgreement,
               AppliedRequiresDecision
      <3>2. Safety'
        BY <2>3, <2>4, <2>5, <3>1 DEF Safety
      <3>3. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ DurableTimeoutsProtectCommits'
        <4>1. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PersistInstallTC, HonestVoteUnique,
                 HonestTimeoutUnique, IntentPhasesCorrect,
                 QcTransportBacked, HonestTimeoutTransportBacked,
                 CertificatesBackedByIntents,
                 HonestDurableIntentsSound
        <4>2. /\ receivedVotes' \subseteq receivedVotes
              /\ voteNetwork' = voteNetwork
              /\ prepareIntents' = prepareIntents
              /\ commitIntents' = commitIntents
          BY <1>1, Isa DEF PersistInstallTC
        <4>3. HonestVoteTransportBacked'
          BY <1>1, <4>2, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 HonestVoteTransportBacked, VoteIntentFor
        <4>4. /\ installedTCs \subseteq installedTCs'
              /\ UNCHANGED <<timeoutIntents, commitIntents>>
          BY <1>1, Isa DEF PersistInstallTC
        <4>5. DurableTimeoutsProtectCommits
          BY <1>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant
        <4>6. DurableTimeoutsProtectCommits'
          BY <4>4, <4>5,
             InstalledTcGrowthPreservesDurableTimeoutProtection
        <4> QED BY <4>1, <4>3, <4>6
      <3>4. ReducerProvenanceInvariant'
        BY <1>1, <2>6, <2>7, <2>8, <3>3,
           PersistInstallTCPreservesDurableLockRecoveryProvenance
           DEF ReducerProvenanceInvariant
      <3>5. \A other \in ValidatorIds:
               /\ nodeView'[other] >= nodeView[other]
               /\ lockRank'[other] >= lockRank[other]
               /\ (lockRank'[other] = lockRank[other]
                     => lockSubject'[other] = lockSubject[other])
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ nodeView'[other] >= nodeView[other]
                     /\ lockRank'[other] >= lockRank[other]
                     /\ (lockRank'[other] = lockRank[other]
                           => lockSubject'[other] = lockSubject[other])
          <5>1. /\ nodeView \in [ValidatorIds -> Views]
                /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
                /\ Node \in ValidatorIds
            BY <1>1, <2>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>2. /\ Certificate.view \in Int
                /\ nodeView[other] \in Int
                /\ nodeView[Node] \in Int
                /\ lockRank[other] \in Int
                /\ lockRank[Node] \in Int
                /\ SelectedRank \in Int
                /\ \/ Certificate.view >= nodeView[Node]
                   \/ SameRoundUpgrade
                /\ (SameRoundUpgrade
                      => Certificate.view + 1 = nodeView[Node])
            BY <1>1, <2>1, <2>2, <4>1, <5>1, SMT
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   ModelConfiguration, TCValid, Views, Ranks, NoRank,
                   SameRoundUpgrade, StrictSameRoundTcUpgrade,
                   Node, Certificate
          <5>3. /\ nodeView' =
                         [nodeView EXCEPT ![Node] =
                            IF SameRoundUpgrade THEN @
                            ELSE Certificate.view + 1]
                /\ lockRank' =
                         [lockRank EXCEPT ![Node] =
                            IF SelectedRank > lockRank[Node]
                            THEN SelectedRank ELSE @]
                /\ lockSubject' =
                         [lockSubject EXCEPT ![Node] =
                            IF SelectedRank > lockRank[Node]
                            THEN SelectedSubject ELSE @]
            BY <1>1
               DEF PersistInstallTC, Node, Certificate,
                   SelectedRank, SelectedSubject, SameRoundUpgrade
          <5>4. CASE other = Node
            <6>1. /\ lockRank'[other] =
                       IF SelectedRank > lockRank[other]
                       THEN SelectedRank ELSE lockRank[other]
                  /\ lockSubject'[other] =
                       IF SelectedRank > lockRank[other]
                       THEN SelectedSubject ELSE lockSubject[other]
              BY <5>1, <5>3, <5>4, Isa
            <6>2. nodeView'[other] >= nodeView[other]
              BY <5>2, <5>3, <5>4, SMT
            <6>5. lockRank'[other] >= lockRank[other]
              BY <5>2, <6>1, SMT
            <6>6. lockRank'[other] = lockRank[other]
                     => lockSubject'[other] = lockSubject[other]
              BY <5>2, <6>1, SMT
            <6> QED BY <6>2, <6>5, <6>6
          <5>5. CASE other # Node
            <6>1. /\ nodeView'[other] = nodeView[other]
                  /\ lockRank'[other] = lockRank[other]
                  /\ lockSubject'[other] = lockSubject[other]
              BY <5>1, <5>3, <5>5, Isa
            <6>2. nodeView'[other] >= nodeView[other]
              BY <5>2, <6>1, SMT
            <6>3. lockRank'[other] >= lockRank[other]
              BY <5>2, <6>1, SMT
            <6>4. lockRank'[other] = lockRank[other]
                     => lockSubject'[other] = lockSubject[other]
              BY <6>1
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>4, <5>5
        <4> QED BY <4>1
      <3>6. /\ PrepareLineageSound'
            /\ CertificatePhasesCorrect'
            /\ DurableIntentsDoNotAnticipateHeight'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, LineageInvariant,
               PersistInstallTC, PrepareLineageSound,
               PrepareCarriesHigherSafeQc, CertificatePhasesCorrect,
               DurableIntentsDoNotAnticipateHeight
      <3>7. LocksCoverOwnCommits'
        <4>1. ASSUME NEW vote \in commitIntents',
                    vote.signer \in Honest,
                    vote.context = context'
               PROVE /\ lockRank'[vote.signer] >= vote.view
                     /\ (lockRank'[vote.signer] = vote.view
                           => lockSubject'[vote.signer] = vote.subject)
          <5>1. /\ commitIntents' = commitIntents
                /\ context' = context
            BY <1>1 DEF PersistInstallTC
          <5>2. vote \in commitIntents
            BY <4>1, <5>1
          <5>3. vote.signer \in ValidatorIds
            BY <1>1, <5>2, Isa
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   VoteRecordSet
          <5>4. /\ lockRank[vote.signer] >= vote.view
                /\ (lockRank[vote.signer] = vote.view
                      => lockSubject[vote.signer] = vote.subject)
            BY <1>1, <4>1, <5>1, <5>2
               DEF StrongInductiveInvariant, LineageInvariant,
                   LocksCoverOwnCommits
          <5>5. /\ lockRank'[vote.signer] >= lockRank[vote.signer]
                /\ (lockRank'[vote.signer] = lockRank[vote.signer]
                      => lockSubject'[vote.signer]
                           = lockSubject[vote.signer])
            BY <3>5, <5>3
          <5>6. /\ lockRank'[vote.signer] \in Int
                /\ lockRank[vote.signer] \in Int
                /\ vote.view \in Int
            <6>1. ModelConfiguration
              BY <1>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant
            <6>2. lockRank'[vote.signer] \in Ranks
              BY <2>4, <5>3 DEF TypeInvariant
            <6>3. lockRank[vote.signer] \in Ranks
              BY <1>1, <5>3
                 DEF StrongInductiveInvariant, Safety, TypeInvariant
            <6>4. vote \in VoteRecordSet
              BY <1>1, <5>2
                 DEF StrongInductiveInvariant, Safety, TypeInvariant
            <6>5. vote.view \in Views
              BY <6>4 DEF VoteRecordSet
            <6>6. vote.view \in Ranks
              BY <6>5, ViewsAreRanks
            <6>7. Ranks \subseteq Int
              BY <6>1, ModelRanksAreIntegers
            <6> QED BY <6>2, <6>3, <6>6, <6>7
          <5>7. lockRank'[vote.signer] >= vote.view
            BY <5>4, <5>5, <5>6, IntegerWeakOrderTransitive
          <5>8. lockRank'[vote.signer] = vote.view
                   => lockSubject'[vote.signer] = vote.subject
            <6>1. ASSUME lockRank'[vote.signer] = vote.view
                   PROVE lockSubject'[vote.signer] = vote.subject
              <7>1. /\ lockRank[vote.signer] = vote.view
                    /\ lockRank'[vote.signer] = lockRank[vote.signer]
                BY <5>4, <5>5, <5>6, <6>1,
                   IntegerWeakBoundsCollapse
              <7>2. lockSubject'[vote.signer]
                       = lockSubject[vote.signer]
                BY <5>5, <7>1
              <7>3. lockSubject[vote.signer] = vote.subject
                BY <5>4, <7>1
              <7> QED BY <7>2, <7>3
            <6> QED BY <6>1
          <5> QED BY <5>7, <5>8
        <4> QED BY <4>1 DEF LocksCoverOwnCommits
      <3>8. /\ ModelConfiguration
            /\ \A vote \in prepareIntents \cup commitIntents
                              \cup timeoutIntents:
                 /\ vote.signer \in ValidatorIds
                 /\ vote.view \in Views
                 /\ nodeView[vote.signer] \in Views
                 /\ nodeView'[vote.signer] \in Views
                 /\ nodeView'[vote.signer] >= nodeView[vote.signer]
        <4>1. ModelConfiguration
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. ASSUME NEW vote \in prepareIntents \cup commitIntents
                                      \cup timeoutIntents
               PROVE /\ vote.signer \in ValidatorIds
                     /\ vote.view \in Views
                     /\ nodeView[vote.signer] \in Views
                     /\ nodeView'[vote.signer] \in Views
                     /\ nodeView'[vote.signer] >= nodeView[vote.signer]
          <5>1. TypeInvariant
            BY <1>1 DEF StrongInductiveInvariant, Safety
          <5>2. TypeInvariant'
            BY <2>4
          <5>3. /\ vote.signer \in ValidatorIds
                /\ vote.view \in Views
            BY <4>2, <5>1, TypeInvariantTypesAllIntentVotes
          <5>4. nodeView[vote.signer] \in Views
            BY <5>1, <5>3 DEF TypeInvariant
          <5>5. nodeView'[vote.signer] \in Views
            BY <5>2, <5>3 DEF TypeInvariant
          <5>6. nodeView'[vote.signer] >= nodeView[vote.signer]
            BY <3>5, <5>3
          <5> QED BY <5>3, <5>4, <5>5, <5>6
        <4> QED BY <4>1, <4>2
      <3>9. CurrentIntentViewsBound'
        <4>1. CurrentIntentViewsBound
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ context' = context
              /\ prepareIntents' = prepareIntents
              /\ timeoutIntents' = timeoutIntents
          BY <1>1 DEF PersistInstallTC
        <4>3. \A vote \in prepareIntents':
                 (vote.signer \in Honest /\ vote.context = context')
                   => vote.view <= nodeView'[vote.signer]
          <5>1. ASSUME NEW vote \in prepareIntents',
                      vote.signer \in Honest,
                      vote.context = context'
                 PROVE vote.view <= nodeView'[vote.signer]
            <6>1. /\ vote \in prepareIntents
                  /\ vote.context = context
              BY <4>2, <5>1
            <6>2. vote.view <= nodeView[vote.signer]
              BY <4>1, <5>1, <6>1 DEF CurrentIntentViewsBound
            <6>3. nodeView'[vote.signer] >= nodeView[vote.signer]
              BY <3>8, <6>1
            <6> QED BY <3>8, <6>1, <6>2, <6>3,
                       ViewWeakOrderTransitive
          <5> QED BY <5>1
        <4>4. \A vote \in timeoutIntents':
                 (vote.signer \in Honest /\ vote.context = context')
                   => vote.view <= nodeView'[vote.signer]
          <5>1. ASSUME NEW vote \in timeoutIntents',
                      vote.signer \in Honest,
                      vote.context = context'
                 PROVE vote.view <= nodeView'[vote.signer]
            <6>1. /\ vote \in timeoutIntents
                  /\ vote.context = context
              BY <4>2, <5>1
            <6>2. vote.view <= nodeView[vote.signer]
              BY <4>1, <5>1, <6>1 DEF CurrentIntentViewsBound
            <6>3. nodeView'[vote.signer] >= nodeView[vote.signer]
              BY <3>8, <6>1
            <6> QED BY <3>8, <6>1, <6>2, <6>3,
                       ViewWeakOrderTransitive
          <5> QED BY <5>1
        <4> QED BY <4>3, <4>4 DEF CurrentIntentViewsBound
      <3>10. HonestCommitIntentPrepared'
        <4>1. HonestCommitIntentPrepared
          BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
        <4>2. /\ context' = context
              /\ commitIntents' = commitIntents
              /\ prepareQCs' = prepareQCs
          BY <1>1 DEF PersistInstallTC
        <4>3. CommitIntentsPreparedBy(commitIntents', prepareQCs')
          BY <4>1, <4>2 DEF HonestCommitIntentPrepared
        <4>4. \A vote \in commitIntents':
                 (vote.signer \in Honest /\ vote.context = context')
                   => vote.view <= nodeView'[vote.signer]
          <5>1. ASSUME NEW vote \in commitIntents',
                      vote.signer \in Honest,
                      vote.context = context'
                 PROVE vote.view <= nodeView'[vote.signer]
            <6>1. /\ vote \in commitIntents
                  /\ vote.context = context
              BY <4>2, <5>1
            <6>2. vote.view <= nodeView[vote.signer]
              BY <4>1, <5>1, <6>1 DEF HonestCommitIntentPrepared
            <6>3. nodeView'[vote.signer] >= nodeView[vote.signer]
              BY <3>8, <6>1
            <6> QED BY <3>8, <6>1, <6>2, <6>3,
                       ViewWeakOrderTransitive
          <5> QED BY <5>1
        <4> QED BY <4>3, <4>4 DEF HonestCommitIntentPrepared
      <3>11. LineageInvariant'
        BY <3>6, <3>7, <3>9, <3>10 DEF LineageInvariant
      <3>12. /\ ContextIdentityBindsFrozenEpoch'
             /\ OldContextCertificateRejected'
             /\ ContextParentWasApplied'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, PersistInstallTC,
               ContextIdentityBindsFrozenEpoch,
               OldContextCertificateRejected, ContextParentWasApplied,
               QcValid, QcWireValid, CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>4, <3>11, <3>12
    <2> QED BY <2>9 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM AdvanceContextPreservesStrongInvariant ==
  \A subject:
    StrongInductiveInvariant /\ AdvanceContext(subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW subject,
              StrongInductiveInvariant,
              AdvanceContext(subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE NextHeight == height + 1
    <2> DEFINE NextLineage == Append(context.lineage, subject)
    <2> DEFINE NextContext == ContextRecord(NextHeight, NextLineage)
    <2>1. /\ height \in Heights
          /\ height < MaxHeight
          /\ subject \in Subjects
          /\ context \in ContextRecords
          /\ context.height = height
          /\ NextHeight \in Heights
          /\ NextLineage \in LineagesAt(NextHeight)
          /\ NextContext \in ContextRecords
          /\ NextContext.height = NextHeight
          /\ height' = NextHeight
          /\ context' = NextContext
      <3>1. TypeInvariant
        BY <1>1 DEF StrongInductiveInvariant, Safety
      <3>2. /\ height \in Heights
            /\ context \in ContextRecords
            /\ context.height = height
            /\ MaxHeight \in Nat
        BY <3>1 DEF TypeInvariant, ModelConfiguration
      <3>3. /\ height < MaxHeight
            /\ CommonAppliedSubject(subject)
            /\ height' = height + 1
            /\ context' =
                 ContextRecord(height + 1,
                               Append(context.lineage, subject))
        BY <1>1 DEF AdvanceContext
      <3>4. subject \in Subjects
        BY <3>3 DEF CommonAppliedSubject
      <3>5. /\ height' = NextHeight
            /\ context' = NextContext
        BY <3>3 DEF NextHeight, NextLineage, NextContext
      <3>6. /\ height \in Nat
            /\ NextHeight \in Heights
        BY <3>2, <3>3, SMT DEF Heights, NextHeight
      <3>7. context.lineage \in LineagesAt(height)
        BY <3>2, ContextRecordFieldsTyped
      <3>8. /\ context.lineage \in Seq(Subjects)
            /\ Len(context.lineage) = height
        <4>1. context.lineage \in Seq(Subjects)
          BY <3>6, <3>7, IntervalFunctionIsSequence DEF LineagesAt
        <4>2. DOMAIN context.lineage = 1..height
          BY <3>7 DEF LineagesAt
        <4>3. /\ Len(context.lineage) \in Nat
              /\ DOMAIN context.lineage = 1..Len(context.lineage)
          BY <4>1, LenProperties
        <4>4. Len(context.lineage) = height
          BY <3>6, <4>2, <4>3, Isa
        <4> QED BY <4>1, <4>4
      <3>9. /\ NextLineage \in Seq(Subjects)
            /\ Len(NextLineage) = NextHeight
        BY <3>4, <3>8, AppendProperties
           DEF NextHeight, NextLineage
      <3>10. NextLineage \in LineagesAt(NextHeight)
        BY <3>9, LenProperties DEF LineagesAt
      <3>11. NextContext \in ContextRecords
        BY <3>6, <3>10, Isa DEF NextContext, ContextRecords
      <3>12. NextContext.height = NextHeight
        BY DEF NextContext, ContextRecord
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6, <3>10,
                   <3>11, <3>12
    <2>2. /\ \A vote \in prepareIntents:
               vote.context # NextContext
          /\ \A vote \in commitIntents:
               vote.context # NextContext
          /\ \A vote \in timeoutIntents:
               vote.context # NextContext
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, LineageInvariant,
             DurableIntentsDoNotAnticipateHeight,
             NextContext, NextHeight, Heights, ContextRecord
    <2>3. (Responsive \cap CurrentVoters) # {}
      <3>1. /\ height \in Heights
            /\ context \in ContextRecords
            /\ context.epoch = ExpectedEpoch(context.height)
            /\ context.height = height
            /\ MaxHeight \in Nat
            /\ EpochLength \in Nat \ {0}
            /\ MaxEpoch \in Nat
            /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
        <4>1. /\ Safety
              /\ ContextIdentityBindsFrozenEpoch
          BY <1>1 DEF StrongInductiveInvariant
        <4>2. TypeInvariant
          BY <4>1 DEF Safety
        <4>3. /\ height \in Heights
              /\ context \in ContextRecords
              /\ context.height = height
          BY <4>2 DEF TypeInvariant
        <4>4. context.epoch = ExpectedEpoch(context.height)
          BY <4>1, <4>3 DEF ContextIdentityBindsFrozenEpoch
        <4>5. /\ MaxHeight \in Nat
              /\ EpochLength \in Nat \ {0}
              /\ MaxEpoch \in Nat
              /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
          BY <4>2
             DEF TypeInvariant, ModelConfiguration, QuorumConfiguration
        <4> QED BY <4>3, <4>4, <4>5
      <3>2. CurrentEpoch \in Epochs
        <4>1. /\ height \in Nat
              /\ height <= MaxHeight
              /\ EpochLength > 0
          BY <3>1, SMT DEF Heights
        <4>2. ExpectedEpoch(height) \in 0..MaxEpoch
          BY <3>1, <4>1, BoundedNaturalQuotient DEF ExpectedEpoch
        <4> QED BY <3>1, <4>2 DEF CurrentEpoch, Epochs
      <3>3. DualQuorum(CurrentEpoch,
                       Responsive \cap VotingRoster(CurrentEpoch))
        BY <1>1, <3>2
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration
      <3>4. CountQuorum(CurrentEpoch,
                        Responsive \cap VotingRoster(CurrentEpoch))
        BY <3>3 DEF DualQuorum
      <3>5. IsFiniteSet(VotingRoster(CurrentEpoch))
        BY <1>1, <3>2
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration, QuorumConfiguration
      <3>6. Cardinality(VotingRoster(CurrentEpoch)) \in Nat
        BY <3>5, FS_CardinalityType
      <3>7. ASSUME Responsive \cap VotingRoster(CurrentEpoch) = {}
             PROVE FALSE
        BY <3>4, <3>6, <3>7, FS_EmptySet, SMT
           DEF CountQuorum
      <3> QED BY <3>7 DEF CurrentVoters
    <2>4. PICK witness \in Responsive \cap CurrentVoters: TRUE
      BY <2>3
    <2>5. PICK parentDecision \in decisions:
             /\ parentDecision.node = witness
             /\ parentDecision.qc.context = context
             /\ parentDecision.qc.subject = subject
             /\ [node |-> witness, qc |-> parentDecision.qc] \in applied
      BY <1>1, <2>4
         DEF AdvanceContext, CommonAppliedSubject
    <2>6. TypeInvariant'
      <3>1. TypeInvariant
        BY <1>1 DEF StrongInductiveInvariant, Safety
      <3>2. ModelConfiguration'
        BY <3>1
           DEF TypeInvariant, ModelConfiguration, QuorumConfiguration
      <3>3. /\ height' \in Heights
            /\ context' \in ContextRecords
            /\ context'.height = height'
        BY <2>1
      <3>4. /\ contextHistory \subseteq ContextRecords
            /\ contextHistory' = contextHistory \cup {NextContext}
        BY <1>1, <3>1 DEF TypeInvariant, AdvanceContext,
                              NextHeight, NextLineage, NextContext
      <3>5. /\ contextHistory' \subseteq ContextRecords
            /\ context' \in contextHistory'
        BY <2>1, <3>4
      <3>6. 0 \in Views
        BY <3>1 DEF TypeInvariant, ModelConfiguration, Views
      <3>7. nodeView' = [node \in ValidatorIds |-> 0]
        BY <1>1 DEF AdvanceContext
      <3>8. nodeView' \in [ValidatorIds -> Views]
        BY <3>6, <3>7, Isa
      <3>9. 0 \in Generations
        BY <3>1
           DEF TypeInvariant, ModelConfiguration, Generations
      <3>10. generation' = [node \in ValidatorIds |-> 0]
        BY <1>1 DEF AdvanceContext
      <3>11. generation' \in [ValidatorIds -> Generations]
        BY <3>9, <3>10, Isa
      <3>12. /\ up' \subseteq ValidatorIds
             /\ gst' \in BOOLEAN
        BY <1>1, <3>1, Isa DEF AdvanceContext, TypeInvariant
      <3>13. /\ availableBodies' \subseteq BodyRecordSet
             /\ durableBodies' \subseteq BodyRecordSet
             /\ retainedLockedBodies'
                    \subseteq RetainedLockedBodyRecordSet
             /\ validatedBodies' \subseteq ValidationRecordSet
             /\ invalidBodies' \subseteq BodyRecordSet
             /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
             /\ RetainedLockedBodiesSound(retainedLockedBodies',
                                           durableBodies')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               AdvanceContext, ValidatedBodiesSound,
               RetainedLockedBodiesSound
      <3>14. /\ proposalIntents' = proposalIntents
             /\ prepareIntents' = prepareIntents
             /\ commitIntents' = commitIntents
             /\ timeoutIntents' = timeoutIntents
             /\ prepareQCs' = prepareQCs
             /\ commitQCs' = commitQCs
             /\ formedTCs' = formedTCs
             /\ installedTCs' = installedTCs
        BY <1>1 DEF AdvanceContext
      <3>15. /\ proposalIntents' \subseteq ProposalRecordSet
             /\ prepareIntents' \subseteq VoteRecordSet
             /\ commitIntents' \subseteq VoteRecordSet
             /\ timeoutIntents' \subseteq TimeoutVoteRecordSet
             /\ prepareQCs' \subseteq QcRecordSet
             /\ commitQCs' \subseteq QcRecordSet
             /\ \A tc \in formedTCs': TcWellTyped(tc)
             /\ \A entry \in installedTCs':
                  /\ entry.node \in ValidatorIds
                  /\ TcWellTyped(entry.tc)
        BY <3>1, <3>14 DEF TypeInvariant
      <3>16. /\ receivedTCs' = {}
             /\ pendingProposal' = {}
             /\ pendingPrepare' = {}
             /\ pendingObservePrepare' = {}
             /\ pendingLockCommit' = {}
             /\ pendingTimeout' = {}
             /\ pendingInstallTC' = {}
             /\ pendingDecision' = {}
             /\ signProposals' = {}
             /\ signVotes' = {}
             /\ signTimeouts' = {}
        BY <1>1 DEF AdvanceContext
      <3>17. /\ \A entry \in receivedTCs':
                  /\ entry.node \in ValidatorIds
                  /\ TcWellTyped(entry.tc)
             /\ pendingProposal' \subseteq ProposalWalSet
             /\ pendingPrepare' \subseteq PrepareWalSet
             /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
             /\ pendingLockCommit' \subseteq LockCommitWalSet
             /\ pendingTimeout' \subseteq TimeoutWalSet
             /\ pendingInstallTC' \subseteq InstallTcWalSet
             /\ pendingDecision' \subseteq DecisionWalSet
             /\ signProposals' \subseteq ProposalSignSet
             /\ signVotes' \subseteq VoteSignSet
             /\ signTimeouts' \subseteq TimeoutSignSet
        BY <3>16
      <3>18. /\ NoRank \in Ranks
             /\ NoSubject \in SubjectOrNone
        BY DEF Ranks, SubjectOrNone
      <3>19. /\ lockRank' = [node \in ValidatorIds |-> NoRank]
             /\ lockSubject' = [node \in ValidatorIds |-> NoSubject]
             /\ highestRank' = [node \in ValidatorIds |-> NoRank]
             /\ highestSubject' =
                  [node \in ValidatorIds |-> NoSubject]
        BY <1>1 DEF AdvanceContext
      <3>20. /\ lockRank' \in [ValidatorIds -> Ranks]
             /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
             /\ highestRank' \in [ValidatorIds -> Ranks]
             /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
        BY <3>18, <3>19, Isa
      <3> QED BY <3>2, <3>3, <3>5, <3>8, <3>11, <3>12,
                   <3>13, <3>15, <3>17, <3>20
         DEF TypeInvariant
    <2>7. Safety'
      <3>1. Safety
        BY <1>1 DEF StrongInductiveInvariant
      <3>2. /\ pendingProposal' = {}
            /\ pendingPrepare' = {}
            /\ pendingObservePrepare' = {}
            /\ pendingLockCommit' = {}
            /\ pendingTimeout' = {}
            /\ pendingInstallTC' = {}
            /\ pendingDecision' = {}
            /\ signProposals' = {}
            /\ signVotes' = {}
            /\ signTimeouts' = {}
        BY <1>1 DEF AdvanceContext
      <3>3. /\ OnePendingPersistencePerNode'
            /\ ProposalSigningRequiresIntent'
            /\ PrepareSigningRequiresIntent'
            /\ CommitSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
        BY <3>2
           DEF OnePendingPersistencePerNode, RequestsUniqueByNode,
               AllPendingRequests, ProposalSigningRequiresIntent,
               PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
               TimeoutSigningRequiresIntent
      <3>4. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
        BY <1>1 DEF AdvanceContext
      <3>5. /\ HonestPrepareUniqueness'
            /\ HonestCommitUniqueness'
            /\ HonestTimeoutUniqueness'
        BY <3>1, <3>4
           DEF Safety, HonestPrepareUniqueness,
               HonestCommitUniqueness, HonestTimeoutUniqueness
      <3>6. /\ lockRank' = [node \in ValidatorIds |-> NoRank]
            /\ highestRank' = [node \in ValidatorIds |-> NoRank]
        BY <1>1 DEF AdvanceContext
      <3>7. LockBelowHighest'
        <4>1. \A node \in ValidatorIds:
                 lockRank'[node] <= highestRank'[node]
          <5>1. ASSUME NEW node \in ValidatorIds
                 PROVE lockRank'[node] <= highestRank'[node]
            <6>1. /\ lockRank'[node] = NoRank
                  /\ highestRank'[node] = NoRank
              BY <3>6, <5>1, Isa
            <6> QED BY <6>1, SMT DEF NoRank
          <5> QED BY <5>1
        <4> QED BY <4>1 DEF LockBelowHighest
      <3>8. /\ decisions' = decisions
            /\ applied' = applied
            /\ commitQCs' = commitQCs
        BY <1>1 DEF AdvanceContext
      <3>9. /\ DecisionAgreement'
            /\ AppliedRequiresDecision'
        BY <3>1, <3>8
           DEF Safety, DecisionAgreement, AppliedRequiresDecision
      <3> QED BY <2>6, <3>3, <3>5, <3>7, <3>9 DEF Safety
    <2>8. ContextIdentityBindsFrozenEpoch'
      <3>1. ContextIdentityBindsFrozenEpoch
        BY <1>1 DEF StrongInductiveInvariant
      <3> QED BY <3>1 DEF ContextIdentityBindsFrozenEpoch
    <2>9. OldContextCertificateRejected'
      BY <1>1, Isa
         DEF AdvanceContext, OldContextCertificateRejected,
             QcValid, QcWireValid, CurrentEpoch
    <2>10. ContextParentWasApplied'
      <3>1. ContextParentWasApplied
        BY <1>1 DEF StrongInductiveInvariant
      <3>2. /\ contextHistory' = contextHistory \cup {NextContext}
            /\ decisions' = decisions
            /\ applied' = applied
        BY <1>1
           DEF AdvanceContext, NextContext, NextHeight, NextLineage
      <3>3. /\ NextContext.height = NextHeight
            /\ NextContext.parent = subject
        <4>1. /\ NextHeight \in Nat
              /\ NextHeight > 0
          BY <2>1, SMT DEF Heights
        <4>2. context.lineage \in LineagesAt(height)
          BY <2>1, ContextRecordFieldsTyped
        <4>3. height \in Nat
          BY <2>1 DEF Heights
        <4>4. context.lineage \in Seq(Subjects)
          BY <4>2, <4>3, IntervalFunctionIsSequence DEF LineagesAt
        <4>5. Len(context.lineage) = height
          <5>1. DOMAIN context.lineage = 1..height
            BY <4>2 DEF LineagesAt
          <5>2. /\ Len(context.lineage) \in Nat
                /\ DOMAIN context.lineage = 1..Len(context.lineage)
            BY <4>4, LenProperties
          <5> QED BY <4>3, <5>1, <5>2, Isa
        <4>6. NextLineage[NextHeight] = subject
          BY <2>1, <4>4, <4>5, AppendProperties
             DEF NextHeight, NextLineage
        <4>7. /\ NextContext.height = NextHeight
              /\ NextContext.parent = NextLineage[NextHeight]
          BY <4>1 DEF NextContext, ContextRecord
        <4> QED BY <4>6, <4>7
      <3>4. \A contextValue \in contextHistory':
               contextValue.height > 0
                 => \E decision \in decisions':
                      /\ decision.qc.context.height + 1
                           = contextValue.height
                      /\ decision.qc.subject = contextValue.parent
                      /\ [node |-> decision.node, qc |-> decision.qc]
                           \in applied'
        <4>1. ASSUME NEW contextValue \in contextHistory',
                    contextValue.height > 0
               PROVE \E decision \in decisions':
                       /\ decision.qc.context.height + 1
                            = contextValue.height
                       /\ decision.qc.subject = contextValue.parent
                       /\ [node |-> decision.node, qc |-> decision.qc]
                            \in applied'
          <5>1. CASE contextValue \in contextHistory
            BY <3>1, <3>2, <4>1, <5>1
               DEF ContextParentWasApplied
          <5>2. CASE contextValue \notin contextHistory
            <6>1. contextValue = NextContext
              BY <3>2, <4>1, <5>2
            <6>2. /\ parentDecision \in decisions'
                  /\ parentDecision.qc.context.height + 1
                       = contextValue.height
                  /\ parentDecision.qc.subject = contextValue.parent
                  /\ [node |-> parentDecision.node,
                       qc |-> parentDecision.qc] \in applied'
              BY <2>1, <2>5, <3>2, <3>3, <6>1, SMT
            <6> QED BY <6>2
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>4 DEF ContextParentWasApplied
    <2>11. ReducerProvenanceInvariant'
      <3>1. ReducerProvenanceInvariant
        BY <1>1 DEF StrongInductiveInvariant
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <3>1, Isa
           DEF AdvanceContext, ReducerProvenanceInvariant,
               HonestVoteUnique, HonestTimeoutUnique,
               IntentPhasesCorrect
      <3>3. /\ PendingVoteWritesAuthorized'
            /\ PendingCertificateWritesAuthorized'
        BY <1>1, Isa
           DEF AdvanceContext, PendingVoteWritesAuthorized,
               PendingCertificateWritesAuthorized
      <3>4. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
        BY <1>1, Isa
           DEF AdvanceContext, HonestVoteTransportBacked,
               QcTransportBacked, HonestTimeoutTransportBacked
      <3>5. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF AdvanceContext, ReducerProvenanceInvariant,
               TcTransportBacked
      <3>6. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
        BY <1>1, <3>1, Isa
           DEF AdvanceContext, ReducerProvenanceInvariant,
               CertificatesBackedByIntents,
               HonestDurableIntentsSound,
               FormedTimeoutCertificatesSound
      <3>7. UNCHANGED
               <<timeoutIntents, commitIntents, installedTCs>>
        BY <1>1 DEF AdvanceContext
      <3>8. DurableTimeoutsProtectCommits
        BY <3>1 DEF ReducerProvenanceInvariant
      <3>9. DurableTimeoutsProtectCommits'
        BY <3>7, <3>8,
           UnchangedDurableTimeoutProtectionVarsPreserves
      <3>10. HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF AdvanceContext, HighestAndLockAreCertified
      <3> QED BY <1>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                  <3>9, <3>10,
                  AdvanceContextPreservesDurableLockRecoveryProvenance
         DEF ReducerProvenanceInvariant
    <2>12. LineageInvariant'
      <3>1. LineageInvariant
        BY <1>1 DEF StrongInductiveInvariant
      <3>2. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ height' = NextHeight
            /\ context' = NextContext
        BY <1>1, <2>1 DEF AdvanceContext
      <3>3. PrepareLineageSound'
        BY <3>1, <3>2, Isa
           DEF LineageInvariant, PrepareLineageSound,
               PrepareCarriesHigherSafeQc
      <3>4. /\ \A vote \in prepareIntents':
                  vote.context # context'
            /\ \A vote \in commitIntents':
                  vote.context # context'
            /\ \A vote \in timeoutIntents':
                  vote.context # context'
        BY <2>2, <3>2
      <3>5. LocksCoverOwnCommits'
        BY <3>4 DEF LocksCoverOwnCommits
      <3>6. CurrentIntentViewsBound'
        BY <3>4 DEF CurrentIntentViewsBound
      <3>7. CommitIntentsPreparedBy(commitIntents', prepareQCs')
        BY <3>1, <3>2
           DEF LineageInvariant, HonestCommitIntentPrepared
      <3>8. HonestCommitIntentPrepared'
        BY <3>4, <3>7 DEF HonestCommitIntentPrepared
      <3>9. CertificatePhasesCorrect'
        BY <3>1, <3>2 DEF LineageInvariant, CertificatePhasesCorrect
      <3>10. /\ TypeInvariant
             /\ DurableIntentsDoNotAnticipateHeight
             /\ height \in Nat
             /\ height' = height + 1
        BY <1>1, <2>1, <3>1
           DEF StrongInductiveInvariant, Safety, LineageInvariant,
               NextHeight, Heights
      <3>11. /\ \A vote \in prepareIntents':
                   vote.context.height <= height'
             /\ \A vote \in commitIntents':
                   vote.context.height <= height'
             /\ \A vote \in timeoutIntents':
                   vote.context.height <= height'
        <4>1. \A vote \in prepareIntents':
                 vote.context.height <= height'
          <5>1. ASSUME NEW vote \in prepareIntents'
                 PROVE vote.context.height <= height'
            <6>1. vote.context.height <= height
              BY <3>2, <3>10, <5>1
                 DEF DurableIntentsDoNotAnticipateHeight
            <6>2. vote \in VoteRecordSet
              BY <3>2, <3>10, <5>1 DEF TypeInvariant
            <6>3. vote.context \in ContextRecords
              BY <6>2 DEF VoteRecordSet
            <6>4. vote.context.height \in Nat
              BY <6>3, ContextRecordFieldsTyped DEF Heights
            <6>5. vote.context.height <= height + 1
              BY <3>10, <6>1, <6>4, NaturalBoundBelowSuccessor
            <6> QED BY <3>10, <6>5
          <5> QED BY <5>1
        <4>2. \A vote \in commitIntents':
                 vote.context.height <= height'
          <5>1. ASSUME NEW vote \in commitIntents'
                 PROVE vote.context.height <= height'
            <6>1. vote.context.height <= height
              BY <3>2, <3>10, <5>1
                 DEF DurableIntentsDoNotAnticipateHeight
            <6>2. vote \in VoteRecordSet
              BY <3>2, <3>10, <5>1 DEF TypeInvariant
            <6>3. vote.context \in ContextRecords
              BY <6>2 DEF VoteRecordSet
            <6>4. vote.context.height \in Nat
              BY <6>3, ContextRecordFieldsTyped DEF Heights
            <6>5. vote.context.height <= height + 1
              BY <3>10, <6>1, <6>4, NaturalBoundBelowSuccessor
            <6> QED BY <3>10, <6>5
          <5> QED BY <5>1
        <4>3. \A vote \in timeoutIntents':
                 vote.context.height <= height'
          <5>1. ASSUME NEW vote \in timeoutIntents'
                 PROVE vote.context.height <= height'
            <6>1. vote.context.height <= height
              BY <3>2, <3>10, <5>1
                 DEF DurableIntentsDoNotAnticipateHeight
            <6>2. vote \in TimeoutVoteRecordSet
              BY <3>2, <3>10, <5>1 DEF TypeInvariant
            <6>3. vote.context \in ContextRecords
              BY <6>2 DEF TimeoutVoteRecordSet
            <6>4. vote.context.height \in Nat
              BY <6>3, ContextRecordFieldsTyped DEF Heights
            <6>5. vote.context.height <= height + 1
              BY <3>10, <6>1, <6>4, NaturalBoundBelowSuccessor
            <6> QED BY <3>10, <6>5
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2, <4>3
      <3>12. DurableIntentsDoNotAnticipateHeight'
        BY <3>11 DEF DurableIntentsDoNotAnticipateHeight
      <3> QED BY <3>3, <3>5, <3>6, <3>8, <3>9, <3>12
         DEF LineageInvariant
    <2> QED BY <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ApplyDecisionPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ ApplyDecision(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              ApplyDecision(node, qc)
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      BY <1>1, IsaT(120)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             ApplyDecision
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ApplyDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, QcWireValid, CurrentEpoch
    <2>3. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ApplyDecision, ProvenanceVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ApplyDecision, LineageVars
  <1> QED BY <1>1

THEOREM ValidateOrRejectBodyPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A proposal \in SeenProposalValues:
      StrongInductiveInvariant /\
        (ValidateBody(node, proposal) \/ RejectBody(node, proposal))
        => StrongInductiveInvariant'
BY IsaM("blast"), ValidateBodyPreservesStrongInvariant,
   RejectBodyPreservesStrongInvariant

THEOREM CrashOrRestartPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    StrongInductiveInvariant /\ (Crash(node) \/ Restart(node))
      => StrongInductiveInvariant'
BY IsaM("blast"), CrashPreservesStrongInvariant,
   RestartPreservesStrongInvariant

THEOREM NextPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ Next
    => StrongInductiveInvariant'
BY SMT,
   SetGSTPreservesStrongInductiveInvariant,
   AssembleLocalBodyPreservesStrongInvariant,
   BeginLocalProposalPreservesStrongInvariant,
   PersistProposalPreservesStrongInvariant,
   CompleteProposalSignaturePreservesStrongInvariant,
   ByzantineBroadcastProposalPreservesStrongInvariant,
   DeliverProposalPreservesStrongInvariant,
   FetchBodyPreservesStrongInvariant,
   RebindRetainedBodyPreservesStrongInvariant,
   StoreBodyPreservesStrongInvariant,
   ValidateOrRejectBodyPreservesStrongInvariant,
   ValidateDecidedBodyPreservesStrongInvariant,
   ValidateLockedBodyPreservesStrongInvariant,
   BeginPreparePreservesStrongInvariant,
   PersistPreparePreservesStrongInvariant,
   CompleteVoteSignaturePreservesStrongInvariant,
   ByzantineBroadcastVotePreservesStrongInvariant,
   DeliverVotePreservesStrongInvariant,
   FormPrepareQCPreservesStrongInvariant,
   DeliverQCPreservesStrongInvariant,
   BeginObservePreparePreservesStrongInvariant,
   PersistObservePreparePreservesStrongInvariant,
   BeginLockCommitPreservesStrongInvariant,
   PersistLockCommitPreservesStrongInvariant,
   FormCommitQCPreservesStrongInvariant,
   BeginDecisionPreservesStrongInvariant,
   PersistDecisionPreservesStrongInvariant,
   BeginTimeoutPreservesStrongInvariant,
   PersistTimeoutPreservesStrongInvariant,
   CompleteTimeoutSignaturePreservesStrongInvariant,
   ByzantineBroadcastTimeoutPreservesStrongInvariant,
   DeliverTimeoutPreservesStrongInvariant,
   DeliverTCPreservesStrongInvariant,
   BeginInstallTCPreservesStrongInvariant,
   PersistInstallTCPreservesStrongInvariant,
   FetchCertifiedBodyPreservesStrongInvariant,
   AcceptCertifiedResponseCapabilityPreservesStrongInvariant,
   ApplyDecisionPreservesStrongInvariant,
   CrashOrRestartPreservesStrongInvariant,
   ResumeProposalPreservesStrongInvariant,
   ResumeVotePreservesStrongInvariant,
   ResumeTimeoutPreservesStrongInvariant,
   DropProposalPreservesStrongInvariant
   DEF Next

THEOREM NextV2PreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ NextV2
    => StrongInductiveInvariant'
BY IsaM("blast"), NextPreservesStrongInductiveInvariant,
   AdvanceContextPreservesStrongInvariant
   DEF NextV2

THEOREM StrongInductiveActionPreservation ==
  StrongInductiveInvariant /\ [NextV2]_vars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              [NextV2]_vars
         PROVE StrongInductiveInvariant'
    <2>1. CASE NextV2
      BY <1>1, <2>1, NextV2PreservesStrongInductiveInvariant
    <2>2. CASE UNCHANGED vars
      <3>1. /\ availableBodies' \subseteq BodyRecordSet
            /\ validatedBodies' \subseteq ValidationRecordSet
            /\ invalidBodies' \subseteq BodyRecordSet
            /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
        BY <1>1, <2>2, Isa
           DEF vars, StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. UNCHANGED ProofRelevantVars
        BY <2>2 DEF vars, ProofRelevantVars
      <3> QED BY <1>1, <3>1, <3>2,
                    ProofRelevantStutterPreservesStrongInvariant
    <2>3. NextV2 \/ UNCHANGED vars
      BY <1>1
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM CoreStrongInductiveActionPreservation ==
  StrongInductiveInvariant /\ [Next]_vars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              [Next]_vars
         PROVE StrongInductiveInvariant'
    <2>1. CASE Next
      BY <1>1, <2>1, NextPreservesStrongInductiveInvariant
    <2>2. CASE UNCHANGED vars
      <3>1. /\ availableBodies' \subseteq BodyRecordSet
            /\ validatedBodies' \subseteq ValidationRecordSet
            /\ invalidBodies' \subseteq BodyRecordSet
            /\ ValidatedBodiesSound(validatedBodies', ValidSubjects)
        BY <1>1, <2>2, Isa
           DEF vars, StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. UNCHANGED ProofRelevantVars
        BY <2>2 DEF vars, ProofRelevantVars
      <3> QED BY <1>1, <3>1, <3>2,
                    ProofRelevantStutterPreservesStrongInvariant
    <2>3. Next \/ UNCHANGED vars
      BY <1>1
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

=============================================================================
