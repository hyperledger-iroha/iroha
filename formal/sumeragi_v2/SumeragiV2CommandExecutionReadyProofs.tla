---- MODULE SumeragiV2CommandExecutionReadyProofs ----
EXTENDS SumeragiV2RegularCommandExecutionReadyProofs,
        SumeragiV2NonRegularCommandExecutionReadyProofs,
        FiniteSetTheorems,
        TLAPS

(***************************************************************************
Exact dispatch-readiness boundary.

The production model selects one of thirteen executor arms.  Each theorem
below proves that the corresponding pure readiness predicate is equivalent
to ENABLED on that fully framed action.  The aggregate theorem then proves
both directions for the exact ExecuteCommand union without leaving nested
ENABLED expressions in the scheduler imported by downstream proofs.
***************************************************************************)

CommandExecutionReadyArmNames ==
  {"Regular", "DecisionFetch", "SignProposal", "SignVote",
   "FormPrepareQC", "SignTimeout", "PersistInstall", "PersistDecision",
   "RequestCertifiedBody", "Apply", "CoreDelivery", "ChunkDelivery",
   "RejectAuthenticatedJunk"}

THEOREM CommandExecutionReadyArmDomainHasExactlyThirteenMembers ==
  Cardinality(CommandExecutionReadyArmNames) = 13
PROOF
  <1>1. /\ IsFiniteSet({"RejectAuthenticatedJunk"})
         /\ Cardinality({"RejectAuthenticatedJunk"}) = 1
    BY FS_Singleton
  <1>2. "ChunkDelivery" \notin {"RejectAuthenticatedJunk"}
    BY SMT
  <1>3. /\ IsFiniteSet({"ChunkDelivery", "RejectAuthenticatedJunk"})
         /\ Cardinality({"ChunkDelivery", "RejectAuthenticatedJunk"}) = 2
    BY <1>1, <1>2, FS_AddElement, Isa
  <1>4. "CoreDelivery"
           \notin {"ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>5. /\ IsFiniteSet(
             {"CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"})
         /\ Cardinality(
             {"CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"}) = 3
    BY <1>3, <1>4, FS_AddElement, Isa
  <1>6. "Apply"
           \notin {"CoreDelivery", "ChunkDelivery",
                   "RejectAuthenticatedJunk"}
    BY SMT
  <1>7. /\ IsFiniteSet(
             {"Apply", "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"})
         /\ Cardinality(
             {"Apply", "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"}) = 4
    PROOF
      <2>1. {"Apply", "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} =
             {"CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} \cup {"Apply"}
        BY Isa
      <2> QED BY <1>5, <1>6, <2>1, FS_AddElement, SMT
  <1>8. "RequestCertifiedBody"
           \notin {"Apply", "CoreDelivery", "ChunkDelivery",
                   "RejectAuthenticatedJunk"}
    BY SMT
  <1>9. /\ IsFiniteSet(
             {"RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"})
         /\ Cardinality(
             {"RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}) = 5
    PROOF
      <2>1. {"RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"} =
             {"Apply", "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} \cup {"RequestCertifiedBody"}
        BY Isa
      <2> QED BY <1>7, <1>8, <2>1, FS_AddElement, SMT
  <1>10. "PersistDecision"
            \notin {"RequestCertifiedBody", "Apply", "CoreDelivery",
                    "ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>11. /\ IsFiniteSet(
              {"PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"}) = 6
    PROOF
      <2>1. {"PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} =
             {"RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}
               \cup {"PersistDecision"}
        BY Isa
      <2> QED BY <1>9, <1>10, <2>1, FS_AddElement, SMT
  <1>12. "PersistInstall"
            \notin {"PersistDecision", "RequestCertifiedBody", "Apply",
                    "CoreDelivery", "ChunkDelivery",
                    "RejectAuthenticatedJunk"}
    BY SMT
  <1>13. /\ IsFiniteSet(
              {"PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"}) = 7
    PROOF
      <2>1. {"PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"} =
             {"PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} \cup {"PersistInstall"}
        BY Isa
      <2> QED BY <1>11, <1>12, <2>1, FS_AddElement, SMT
  <1>14. "SignTimeout"
            \notin {"PersistInstall", "PersistDecision",
                    "RequestCertifiedBody", "Apply", "CoreDelivery",
                    "ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>15. /\ IsFiniteSet(
              {"SignTimeout", "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"SignTimeout", "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"}) = 8
    PROOF
      <2>1. {"SignTimeout", "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"} =
             {"PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}
               \cup {"SignTimeout"}
        BY Isa
      <2> QED BY <1>13, <1>14, <2>1, FS_AddElement, SMT
  <1>16. "FormPrepareQC"
            \notin {"SignTimeout", "PersistInstall", "PersistDecision",
                    "RequestCertifiedBody", "Apply", "CoreDelivery",
                    "ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>17. /\ IsFiniteSet(
              {"FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"}) = 9
    PROOF
      <2>1. {"FormPrepareQC", "SignTimeout", "PersistInstall",
              "PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} =
             {"SignTimeout", "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}
               \cup {"FormPrepareQC"}
        BY Isa
      <2> QED BY <1>15, <1>16, <2>1, FS_AddElement, SMT
  <1>18. "SignVote"
            \notin {"FormPrepareQC", "SignTimeout", "PersistInstall",
                    "PersistDecision", "RequestCertifiedBody", "Apply",
                    "CoreDelivery", "ChunkDelivery",
                    "RejectAuthenticatedJunk"}
    BY SMT
  <1>19. /\ IsFiniteSet(
              {"SignVote", "FormPrepareQC", "SignTimeout",
               "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"SignVote", "FormPrepareQC", "SignTimeout",
               "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"}) = 10
    PROOF
      <2>1. {"SignVote", "FormPrepareQC", "SignTimeout",
              "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"} =
             {"FormPrepareQC", "SignTimeout", "PersistInstall",
              "PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} \cup {"SignVote"}
        BY Isa
      <2> QED BY <1>17, <1>18, <2>1, FS_AddElement, SMT
  <1>20. "SignProposal"
            \notin {"SignVote", "FormPrepareQC", "SignTimeout",
                    "PersistInstall", "PersistDecision",
                    "RequestCertifiedBody", "Apply", "CoreDelivery",
                    "ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>21. /\ IsFiniteSet(
              {"SignProposal", "SignVote", "FormPrepareQC",
               "SignTimeout", "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"SignProposal", "SignVote", "FormPrepareQC",
               "SignTimeout", "PersistInstall", "PersistDecision",
               "RequestCertifiedBody", "Apply", "CoreDelivery",
               "ChunkDelivery", "RejectAuthenticatedJunk"}) = 11
    PROOF
      <2>1. {"SignProposal", "SignVote", "FormPrepareQC",
              "SignTimeout", "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"} =
             {"SignVote", "FormPrepareQC", "SignTimeout",
              "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}
               \cup {"SignProposal"}
        BY Isa
      <2> QED BY <1>19, <1>20, <2>1, FS_AddElement, SMT
  <1>22. "DecisionFetch"
            \notin {"SignProposal", "SignVote", "FormPrepareQC",
                    "SignTimeout", "PersistInstall", "PersistDecision",
                    "RequestCertifiedBody", "Apply", "CoreDelivery",
                    "ChunkDelivery", "RejectAuthenticatedJunk"}
    BY SMT
  <1>23. /\ IsFiniteSet(
              {"DecisionFetch", "SignProposal", "SignVote",
               "FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"DecisionFetch", "SignProposal", "SignVote",
               "FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"}) = 12
    PROOF
      <2>1. {"DecisionFetch", "SignProposal", "SignVote",
              "FormPrepareQC", "SignTimeout", "PersistInstall",
              "PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} =
             {"SignProposal", "SignVote", "FormPrepareQC",
              "SignTimeout", "PersistInstall", "PersistDecision",
              "RequestCertifiedBody", "Apply", "CoreDelivery",
              "ChunkDelivery", "RejectAuthenticatedJunk"}
               \cup {"DecisionFetch"}
        BY Isa
      <2> QED BY <1>21, <1>22, <2>1, FS_AddElement, SMT
  <1>24. "Regular"
            \notin {"DecisionFetch", "SignProposal", "SignVote",
                    "FormPrepareQC", "SignTimeout", "PersistInstall",
                    "PersistDecision", "RequestCertifiedBody", "Apply",
                    "CoreDelivery", "ChunkDelivery",
                    "RejectAuthenticatedJunk"}
    BY SMT
  <1>25. /\ IsFiniteSet(
              {"Regular", "DecisionFetch", "SignProposal", "SignVote",
               "FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"})
          /\ Cardinality(
              {"Regular", "DecisionFetch", "SignProposal", "SignVote",
               "FormPrepareQC", "SignTimeout", "PersistInstall",
               "PersistDecision", "RequestCertifiedBody", "Apply",
               "CoreDelivery", "ChunkDelivery",
               "RejectAuthenticatedJunk"}) = 13
    PROOF
      <2>1. {"Regular", "DecisionFetch", "SignProposal", "SignVote",
              "FormPrepareQC", "SignTimeout", "PersistInstall",
              "PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} =
             {"DecisionFetch", "SignProposal", "SignVote",
              "FormPrepareQC", "SignTimeout", "PersistInstall",
              "PersistDecision", "RequestCertifiedBody", "Apply",
              "CoreDelivery", "ChunkDelivery",
              "RejectAuthenticatedJunk"} \cup {"Regular"}
        BY Isa
      <2> QED BY <1>23, <1>24, <2>1, FS_AddElement, SMT
  <1> QED BY <1>25 DEF CommandExecutionReadyArmNames

THEOREM ExecuteRegularCommandReadyIffEnabled ==
  \A command:
    ExecuteRegularCommandReady(command)
      <=> ENABLED ExecuteRegularCommand(command)
BY ExecuteRegularCommandReadyIffEnabledComposed

THEOREM ExecuteDecisionFetchReadyIffEnabled ==
  \A command:
    ExecuteDecisionFetchReady(command)
      <=> ENABLED ExecuteDecisionFetch(command)
BY ExecuteDecisionFetchReadyIffEnabledComposed

THEOREM ExecuteSignProposalReadyIffEnabled ==
  \A command:
    ExecuteSignProposalReady(command)
      <=> ENABLED ExecuteSignProposal(command)
BY ExecuteSignProposalReadyIffEnabledComposed

THEOREM ExecuteSignVoteReadyIffEnabled ==
  \A command:
    ExecuteSignVoteReady(command) <=> ENABLED ExecuteSignVote(command)
BY ExecuteSignVoteReadyIffEnabledComposed

THEOREM ExecuteFormPrepareQCReadyIffEnabled ==
  \A command:
    ExecuteFormPrepareQCReady(command)
      <=> ENABLED ExecuteFormPrepareQC(command)
BY ExpandENABLED, IsaT(300)
   DEF ExecuteFormPrepareQCReady, ExecuteFormPrepareQC,
       FormPrepareQCReady, FormPrepareQC, PublishControlItems,
       AsyncAuxVars, vars

THEOREM ExecuteSignTimeoutReadyIffEnabled ==
  \A command:
    ExecuteSignTimeoutReady(command)
      <=> ENABLED ExecuteSignTimeout(command)
BY ExecuteSignTimeoutReadyIffEnabledComposed

THEOREM ExecutePersistInstallReadyIffEnabled ==
  \A command:
    ExecutePersistInstallReady(command)
      <=> ENABLED ExecutePersistInstall(command)
BY ExecutePersistInstallReadyIffEnabledComposed

THEOREM ExecutePersistDecisionReadyIffEnabled ==
  \A command:
    ExecutePersistDecisionReady(command)
      <=> ENABLED ExecutePersistDecision(command)
BY ExecutePersistDecisionReadyIffEnabledComposed

THEOREM ExecuteRequestCertifiedBodyReadyIffEnabled ==
  \A command:
    ExecuteRequestCertifiedBodyReady(command)
      <=> ENABLED ExecuteRequestCertifiedBody(command)
BY ExecuteRequestCertifiedBodyReadyIffEnabledComposed

THEOREM ExecuteApplyReadyIffEnabled ==
  \A command:
    ExecuteApplyReady(command) <=> ENABLED ExecuteApply(command)
BY ExecuteApplyReadyIffEnabledComposed

THEOREM ExecuteCoreDeliveryReadyIffEnabled ==
  \A command:
    ExecuteCoreDeliveryReady(command)
      <=> ENABLED ExecuteCoreDelivery(command)
BY ExpandENABLED, IsaT(600)
   DEF ExecuteCoreDeliveryReady, ExecuteCoreDelivery,
       DeliverProposalReady, DeliverVoteReady, DeliverQCReady,
       DeliverTimeoutReady, DeliverTCReady,
       DeliverProposal, DeliverVote, DeliverQC, DeliverTimeout, DeliverTC,
       AsyncAuxVars, vars

THEOREM ExecuteChunkDeliveryReadyIffEnabled ==
  \A command:
    ExecuteChunkDeliveryReady(command)
      <=> ENABLED ExecuteChunkDelivery(command)
BY ExpandENABLED, IsaT(300)
   DEF ExecuteChunkDeliveryReady, ExecuteChunkDelivery,
       AsyncAuxVars, vars

THEOREM ExecuteRejectAuthenticatedJunkReadyIffEnabled ==
  \A command:
    ExecuteRejectAuthenticatedJunkReady(command)
      <=> ENABLED ExecuteRejectAuthenticatedJunk(command)
BY ExpandENABLED, IsaT(300)
   DEF ExecuteRejectAuthenticatedJunkReady,
       ExecuteRejectAuthenticatedJunk, AsyncAuxVars, vars

THEOREM ExecuteFormPrepareQCImpliesReady ==
  \A command:
    ExecuteFormPrepareQC(command)
      => ExecuteFormPrepareQCReady(command)
BY Isa
   DEF ExecuteFormPrepareQC, ExecuteFormPrepareQCReady,
       FormPrepareQC, FormPrepareQCReady, PublishControlItems

THEOREM ExecuteCoreDeliveryImpliesEnabled ==
  \A command:
    ExecuteCoreDelivery(command)
      => ENABLED ExecuteCoreDelivery(command)
BY ExpandENABLED, IsaT(300)
   DEF ExecuteCoreDelivery, DeliverProposal, DeliverVote, DeliverQC,
       DeliverTimeout, DeliverTC, AsyncAuxVars, vars

THEOREM ExecuteCoreDeliveryImpliesReady ==
  \A command:
    ExecuteCoreDelivery(command)
      => ExecuteCoreDeliveryReady(command)
BY ExecuteCoreDeliveryImpliesEnabled,
   ExecuteCoreDeliveryReadyIffEnabled,
   Isa

THEOREM ExecuteChunkDeliveryImpliesReady ==
  \A command:
    ExecuteChunkDelivery(command)
      => ExecuteChunkDeliveryReady(command)
BY Isa DEF ExecuteChunkDelivery, ExecuteChunkDeliveryReady

THEOREM ExecuteRejectAuthenticatedJunkImpliesReady ==
  \A command:
    ExecuteRejectAuthenticatedJunk(command)
      => ExecuteRejectAuthenticatedJunkReady(command)
BY Isa
   DEF ExecuteRejectAuthenticatedJunk,
       ExecuteRejectAuthenticatedJunkReady

THEOREM CommandArmEnabledSelectionsEnableAggregate ==
  \A command:
    /\ (ENABLED ExecuteRegularCommand(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteDecisionFetch(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteSignProposal(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteSignVote(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteFormPrepareQC(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteSignTimeout(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecutePersistInstall(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecutePersistDecision(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteRequestCertifiedBody(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteApply(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteCoreDelivery(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteChunkDelivery(command)
          => ENABLED ExecuteCommand(command))
    /\ (ENABLED ExecuteRejectAuthenticatedJunk(command)
          => ENABLED ExecuteCommand(command))
PROOF
  <1>1. ASSUME NEW command
         PROVE
           /\ (ENABLED ExecuteRegularCommand(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteDecisionFetch(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignProposal(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignVote(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteFormPrepareQC(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignTimeout(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecutePersistInstall(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecutePersistDecision(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteRequestCertifiedBody(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteApply(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteCoreDelivery(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteChunkDelivery(command)
                 => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteRejectAuthenticatedJunk(command)
                 => ENABLED ExecuteCommand(command))
    <2>1. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>2. ExecuteRegularCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteRegularCommand
    <2>3. ExecuteRegularCommand(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>4. ENABLED ExecuteRegularCommand(command)
             => ENABLED ExecuteCommand(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2>5. ExecuteDecisionFetch(command) \in BOOLEAN
      BY Isa DEF ExecuteDecisionFetch
    <2>6. ExecuteDecisionFetch(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>7. ENABLED ExecuteDecisionFetch(command)
             => ENABLED ExecuteCommand(command)
      BY <2>1, <2>5, <2>6, ENABLEDaxioms
    <2>8. ExecuteSignProposal(command) \in BOOLEAN
      BY Isa DEF ExecuteSignProposal
    <2>9. ExecuteSignProposal(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>10. ENABLED ExecuteSignProposal(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>8, <2>9, ENABLEDaxioms
    <2>11. ExecuteSignVote(command) \in BOOLEAN
      BY Isa DEF ExecuteSignVote
    <2>12. ExecuteSignVote(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>13. ENABLED ExecuteSignVote(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>11, <2>12, ENABLEDaxioms
    <2>14. ExecuteFormPrepareQC(command) \in BOOLEAN
      BY Isa DEF ExecuteFormPrepareQC
    <2>15. ExecuteFormPrepareQC(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>16. ENABLED ExecuteFormPrepareQC(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>14, <2>15, ENABLEDaxioms
    <2>17. ExecuteSignTimeout(command) \in BOOLEAN
      BY Isa DEF ExecuteSignTimeout
    <2>18. ExecuteSignTimeout(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>19. ENABLED ExecuteSignTimeout(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>17, <2>18, ENABLEDaxioms
    <2>20. ExecutePersistInstall(command) \in BOOLEAN
      BY Isa DEF ExecutePersistInstall
    <2>21. ExecutePersistInstall(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>22. ENABLED ExecutePersistInstall(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>20, <2>21, ENABLEDaxioms
    <2>23. ExecutePersistDecision(command) \in BOOLEAN
      BY Isa DEF ExecutePersistDecision
    <2>24. ExecutePersistDecision(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>25. ENABLED ExecutePersistDecision(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>23, <2>24, ENABLEDaxioms
    <2>26. ExecuteRequestCertifiedBody(command) \in BOOLEAN
      BY Isa DEF ExecuteRequestCertifiedBody
    <2>27. ExecuteRequestCertifiedBody(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>28. ENABLED ExecuteRequestCertifiedBody(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>26, <2>27, ENABLEDaxioms
    <2>29. ExecuteApply(command) \in BOOLEAN
      BY Isa DEF ExecuteApply
    <2>30. ExecuteApply(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>31. ENABLED ExecuteApply(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>29, <2>30, ENABLEDaxioms
    <2>32. ExecuteCoreDelivery(command) \in BOOLEAN
      BY Isa DEF ExecuteCoreDelivery
    <2>33. ExecuteCoreDelivery(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>34. ENABLED ExecuteCoreDelivery(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>32, <2>33, ENABLEDaxioms
    <2>35. ExecuteChunkDelivery(command) \in BOOLEAN
      BY Isa DEF ExecuteChunkDelivery
    <2>36. ExecuteChunkDelivery(command) => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>37. ENABLED ExecuteChunkDelivery(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>35, <2>36, ENABLEDaxioms
    <2>38. ExecuteRejectAuthenticatedJunk(command) \in BOOLEAN
      BY Isa DEF ExecuteRejectAuthenticatedJunk
    <2>39. ExecuteRejectAuthenticatedJunk(command)
              => ExecuteCommand(command)
      BY DEF ExecuteCommand
    <2>40. ENABLED ExecuteRejectAuthenticatedJunk(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>38, <2>39, ENABLEDaxioms
    <2> QED
      BY <2>4, <2>7, <2>10, <2>13, <2>16, <2>19, <2>22,
         <2>25, <2>28, <2>31, <2>34, <2>37, <2>40
  <1> QED BY <1>1

CommandExecutionReadyProjection(command) ==
  /\ CommandExecutionReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteCommandImpliesReadyProjection ==
  \A command:
    ExecuteCommand(command)
      => CommandExecutionReadyProjection(command)
BY ExecuteRegularCommandImpliesReady,
   ExecuteDecisionFetchImpliesReady,
   ExecuteSignProposalImpliesReady,
   ExecuteSignVoteImpliesReady,
   ExecuteFormPrepareQCImpliesReady,
   ExecuteSignTimeoutImpliesReady,
   ExecutePersistInstallImpliesReady,
   ExecutePersistDecisionImpliesReady,
   ExecuteRequestCertifiedBodyImpliesReady,
   ExecuteApplyImpliesReady,
   ExecuteCoreDeliveryImpliesReady,
   ExecuteChunkDeliveryImpliesReady,
   ExecuteRejectAuthenticatedJunkImpliesReady,
   IsaT(300)
   DEF ExecuteCommand, CommandExecutionReady,
       CommandExecutionReadyProjection

THEOREM CommandExecutionReadyProjectionIffReady ==
  \A command:
    ENABLED CommandExecutionReadyProjection(command)
      <=> CommandExecutionReady(command)
BY ExpandENABLED, IsaT(300)
   DEF CommandExecutionReadyProjection, CommandExecutionReady, vars

THEOREM ExecuteCommandEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteCommand(command)
      => CommandExecutionReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteCommand(command)
         PROVE CommandExecutionReady(command)
    <2>1. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>2. CommandExecutionReadyProjection(command) \in BOOLEAN
      BY Isa DEF CommandExecutionReadyProjection
    <2>3. ExecuteCommand(command)
             => CommandExecutionReadyProjection(command)
      BY ExecuteCommandImpliesReadyProjection
    <2>4. ENABLED ExecuteCommand(command)
             => ENABLED CommandExecutionReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, CommandExecutionReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM CommandExecutionReadyExactlyCharacterizesEnabledAction ==
  \A command:
    CommandExecutionReady(command) <=> ENABLED ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW command
         PROVE CommandExecutionReady(command)
                 <=> ENABLED ExecuteCommand(command)
    <2>1. /\ (ENABLED ExecuteRegularCommand(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteDecisionFetch(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignProposal(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignVote(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteFormPrepareQC(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteSignTimeout(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecutePersistInstall(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecutePersistDecision(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteRequestCertifiedBody(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteApply(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteCoreDelivery(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteChunkDelivery(command)
                   => ENABLED ExecuteCommand(command))
           /\ (ENABLED ExecuteRejectAuthenticatedJunk(command)
                   => ENABLED ExecuteCommand(command))
      BY CommandArmEnabledSelectionsEnableAggregate
    <2>2. ENABLED ExecuteCommand(command)
             => CommandExecutionReady(command)
      BY ExecuteCommandEnabledImpliesReady
    <2>3. ExecuteRegularCommandReady(command)
             => ENABLED ExecuteRegularCommand(command)
      BY ExecuteRegularCommandReadyIffEnabled
    <2>4. ExecuteDecisionFetchReady(command)
             => ENABLED ExecuteDecisionFetch(command)
      BY ExecuteDecisionFetchReadyIffEnabled
    <2>5. ExecuteSignProposalReady(command)
             => ENABLED ExecuteSignProposal(command)
      BY ExecuteSignProposalReadyIffEnabled
    <2>6. ExecuteSignVoteReady(command)
             => ENABLED ExecuteSignVote(command)
      BY ExecuteSignVoteReadyIffEnabled
    <2>7. ExecuteFormPrepareQCReady(command)
             => ENABLED ExecuteFormPrepareQC(command)
      BY ExecuteFormPrepareQCReadyIffEnabled
    <2>8. ExecuteSignTimeoutReady(command)
             => ENABLED ExecuteSignTimeout(command)
      BY ExecuteSignTimeoutReadyIffEnabled
    <2>9. ExecutePersistInstallReady(command)
             => ENABLED ExecutePersistInstall(command)
      BY ExecutePersistInstallReadyIffEnabled
    <2>10. ExecutePersistDecisionReady(command)
              => ENABLED ExecutePersistDecision(command)
      BY ExecutePersistDecisionReadyIffEnabled
    <2>11. ExecuteRequestCertifiedBodyReady(command)
              => ENABLED ExecuteRequestCertifiedBody(command)
      BY ExecuteRequestCertifiedBodyReadyIffEnabled
    <2>12. ExecuteApplyReady(command)
              => ENABLED ExecuteApply(command)
      BY ExecuteApplyReadyIffEnabled
    <2>13. ExecuteCoreDeliveryReady(command)
              => ENABLED ExecuteCoreDelivery(command)
      BY ExecuteCoreDeliveryReadyIffEnabled
    <2>14. ExecuteChunkDeliveryReady(command)
              => ENABLED ExecuteChunkDelivery(command)
      BY ExecuteChunkDeliveryReadyIffEnabled
    <2>15. ExecuteRejectAuthenticatedJunkReady(command)
              => ENABLED ExecuteRejectAuthenticatedJunk(command)
      BY ExecuteRejectAuthenticatedJunkReadyIffEnabled
    <2>16. CommandExecutionReady(command)
              => \/ ExecuteRegularCommandReady(command)
                 \/ ExecuteDecisionFetchReady(command)
                 \/ ExecuteSignProposalReady(command)
                 \/ ExecuteSignVoteReady(command)
                 \/ ExecuteFormPrepareQCReady(command)
                 \/ ExecuteSignTimeoutReady(command)
                 \/ ExecutePersistInstallReady(command)
                 \/ ExecutePersistDecisionReady(command)
                 \/ ExecuteRequestCertifiedBodyReady(command)
                 \/ ExecuteApplyReady(command)
                 \/ ExecuteCoreDeliveryReady(command)
                 \/ ExecuteChunkDeliveryReady(command)
                 \/ ExecuteRejectAuthenticatedJunkReady(command)
      BY Isa DEF CommandExecutionReady
    <2>17. CommandExecutionReady(command)
              => ENABLED ExecuteCommand(command)
      BY <2>1, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9,
         <2>10, <2>11, <2>12, <2>13, <2>14, <2>15, <2>16, Isa
    <2> QED BY <2>2, <2>17
  <1> QED BY <1>1

=============================================================================
