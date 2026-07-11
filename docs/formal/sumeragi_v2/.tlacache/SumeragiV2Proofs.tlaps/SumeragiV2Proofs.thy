(* automatically generated -- do not edit manually *)
theory SumeragiV2Proofs imports Constant Zenon begin
ML_command \<open> writeln ("*** TLAPS PARSED\n"); \<close>
consts
  "isReal" :: c
  "isa_slas_a" :: "[c,c] => c"
  "isa_bksl_diva" :: "[c,c] => c"
  "isa_perc_a" :: "[c,c] => c"
  "isa_peri_peri_a" :: "[c,c] => c"
  "isInfinity" :: c
  "isa_lbrk_rbrk_a" :: "[c] => c"
  "isa_less_more_a" :: "[c] => c"

lemma ob'22:
(* usable definition CONSTANT_IsFiniteSet_ suppressed *)
(* usable definition CONSTANT_Cardinality_ suppressed *)
fixes a_CONSTANTunde_Nunde_a
fixes a_CONSTANTunde_MaxEpochunde_a
fixes a_CONSTANTunde_EpochRostersunde_a
fixes a_CONSTANTunde_EpochPowersunde_a
fixes a_CONSTANTunde_Honestunde_a
(* usable definition CONSTANT_ValidatorIds_ suppressed *)
(* usable definition CONSTANT_Epochs_ suppressed *)
(* usable definition CONSTANT_VotingPower_ suppressed *)
(* usable definition CONSTANT_RosterSequence_ suppressed *)
(* usable definition CONSTANT_VotingRoster_ suppressed *)
(* usable definition CONSTANT_Byzantine_ suppressed *)
(* usable definition CONSTANT_PowerUnits_ suppressed *)
(* usable definition CONSTANT_PowerOf_ suppressed *)
(* usable definition CONSTANT_CountQuorum_ suppressed *)
(* usable definition CONSTANT_PowerQuorum_ suppressed *)
(* usable definition CONSTANT_DualQuorum_ suppressed *)
(* usable definition CONSTANT_QuorumConfiguration_ suppressed *)
(* usable definition CONSTANT_CountQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_PowerQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_DualQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_NoSubject_ suppressed *)
(* usable definition CONSTANT_Subjects_ suppressed *)
(* usable definition CONSTANT_SubjectOrNone_ suppressed *)
(* usable definition CONSTANT_BodyRecord_ suppressed *)
(* usable definition CONSTANT_ValidationRecord_ suppressed *)
(* usable definition CONSTANT_BodyHeldBy_ suppressed *)
(* usable definition CONSTANT_BodyValidatedBy_ suppressed *)
(* usable definition CONSTANT_PrepareSignerAvailability_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyAvailable_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyValid_ suppressed *)
fixes a_CONSTANTunde_MaxHeightunde_a
fixes a_CONSTANTunde_MaxViewunde_a
fixes a_CONSTANTunde_MaxGenerationunde_a
fixes a_CONSTANTunde_EpochLengthunde_a
fixes a_CONSTANTunde_LeaderStartsunde_a
fixes a_CONSTANTunde_LaneHashesunde_a
fixes a_CONSTANTunde_DaHashesunde_a
fixes a_CONSTANTunde_ChainIdValueunde_a
fixes a_CONSTANTunde_ProtocolVersionValueunde_a
fixes a_CONSTANTunde_ValidSubjectsunde_a
fixes a_CONSTANTunde_Responsiveunde_a
(* usable definition CONSTANT_Heights_ suppressed *)
(* usable definition CONSTANT_Views_ suppressed *)
(* usable definition CONSTANT_Generations_ suppressed *)
(* usable definition CONSTANT_Phases_ suppressed *)
(* usable definition CONSTANT_NoRank_ suppressed *)
(* usable definition CONSTANT_Ranks_ suppressed *)
(* usable definition CONSTANT_CountRostersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountRostersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_CountPowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountPowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StakePowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_StakePowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StartsHeightZero_ suppressed *)
(* usable definition CONSTANT_StartsHeightZeroOne_ suppressed *)
(* usable definition CONSTANT_StartsByzantineFirst_ suppressed *)
(* usable definition CONSTANT_LaneHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_LaneHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_DaHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_DaHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_ExpectedEpoch_ suppressed *)
(* usable definition CONSTANT_ContextRecord_ suppressed *)
(* usable definition CONSTANT_ContextRecords_ suppressed *)
(* usable definition CONSTANT_Leader_ suppressed *)
(* usable definition CONSTANT_Proposal_ suppressed *)
(* usable definition CONSTANT_Vote_ suppressed *)
(* usable definition CONSTANT_QC_ suppressed *)
(* usable definition CONSTANT_TimeoutVote_ suppressed *)
(* usable definition CONSTANT_TC_ suppressed *)
(* usable definition CONSTANT_ProposalRecordSet_ suppressed *)
(* usable definition CONSTANT_VoteRecordSet_ suppressed *)
(* usable definition CONSTANT_QcRecordSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteRecordSet_ suppressed *)
(* usable definition CONSTANT_TcRecordSet_ suppressed *)
(* usable definition CONSTANT_TcWellTyped_ suppressed *)
(* usable definition CONSTANT_ProposalAt_ suppressed *)
(* usable definition CONSTANT_VoteAt_ suppressed *)
(* usable definition CONSTANT_QcAt_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteAt_ suppressed *)
(* usable definition CONSTANT_TcAt_ suppressed *)
(* usable definition CONSTANT_ProposalEnvelope_ suppressed *)
(* usable definition CONSTANT_VoteEnvelope_ suppressed *)
(* usable definition CONSTANT_QcEnvelope_ suppressed *)
(* usable definition CONSTANT_TimeoutEnvelope_ suppressed *)
(* usable definition CONSTANT_TcEnvelope_ suppressed *)
(* usable definition CONSTANT_ProposalWal_ suppressed *)
(* usable definition CONSTANT_PrepareWal_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWal_ suppressed *)
(* usable definition CONSTANT_LockCommitWal_ suppressed *)
(* usable definition CONSTANT_TimeoutWal_ suppressed *)
(* usable definition CONSTANT_InstallTcWal_ suppressed *)
(* usable definition CONSTANT_DecisionWal_ suppressed *)
(* usable definition CONSTANT_ProposalSign_ suppressed *)
(* usable definition CONSTANT_VoteSign_ suppressed *)
(* usable definition CONSTANT_TimeoutSign_ suppressed *)
(* usable definition CONSTANT_ProposalWalSet_ suppressed *)
(* usable definition CONSTANT_PrepareWalSet_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWalSet_ suppressed *)
(* usable definition CONSTANT_LockCommitWalSet_ suppressed *)
(* usable definition CONSTANT_TimeoutWalSet_ suppressed *)
(* usable definition CONSTANT_InstallTcWalSet_ suppressed *)
(* usable definition CONSTANT_DecisionWalSet_ suppressed *)
(* usable definition CONSTANT_ProposalSignSet_ suppressed *)
(* usable definition CONSTANT_VoteSignSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignSet_ suppressed *)
fixes a_VARIABLEunde_heightunde_a a_VARIABLEunde_heightunde_a'
fixes a_VARIABLEunde_contextunde_a a_VARIABLEunde_contextunde_a'
fixes a_VARIABLEunde_contextHistoryunde_a a_VARIABLEunde_contextHistoryunde_a'
fixes a_VARIABLEunde_nodeViewunde_a a_VARIABLEunde_nodeViewunde_a'
fixes a_VARIABLEunde_generationunde_a a_VARIABLEunde_generationunde_a'
fixes a_VARIABLEunde_upunde_a a_VARIABLEunde_upunde_a'
fixes a_VARIABLEunde_gstunde_a a_VARIABLEunde_gstunde_a'
fixes a_VARIABLEunde_availableBodiesunde_a a_VARIABLEunde_availableBodiesunde_a'
fixes a_VARIABLEunde_durableBodiesunde_a a_VARIABLEunde_durableBodiesunde_a'
fixes a_VARIABLEunde_validatedBodiesunde_a a_VARIABLEunde_validatedBodiesunde_a'
fixes a_VARIABLEunde_invalidBodiesunde_a a_VARIABLEunde_invalidBodiesunde_a'
fixes a_VARIABLEunde_seenProposalsunde_a a_VARIABLEunde_seenProposalsunde_a'
fixes a_VARIABLEunde_receivedVotesunde_a a_VARIABLEunde_receivedVotesunde_a'
fixes a_VARIABLEunde_receivedQCsunde_a a_VARIABLEunde_receivedQCsunde_a'
fixes a_VARIABLEunde_receivedTimeoutVotesunde_a a_VARIABLEunde_receivedTimeoutVotesunde_a'
fixes a_VARIABLEunde_receivedTCsunde_a a_VARIABLEunde_receivedTCsunde_a'
fixes a_VARIABLEunde_proposalIntentsunde_a a_VARIABLEunde_proposalIntentsunde_a'
fixes a_VARIABLEunde_prepareIntentsunde_a a_VARIABLEunde_prepareIntentsunde_a'
fixes a_VARIABLEunde_commitIntentsunde_a a_VARIABLEunde_commitIntentsunde_a'
fixes a_VARIABLEunde_timeoutIntentsunde_a a_VARIABLEunde_timeoutIntentsunde_a'
fixes a_VARIABLEunde_prepareQCsunde_a a_VARIABLEunde_prepareQCsunde_a'
fixes a_VARIABLEunde_commitQCsunde_a a_VARIABLEunde_commitQCsunde_a'
fixes a_VARIABLEunde_formedTCsunde_a a_VARIABLEunde_formedTCsunde_a'
fixes a_VARIABLEunde_installedTCsunde_a a_VARIABLEunde_installedTCsunde_a'
fixes a_VARIABLEunde_lockRankunde_a a_VARIABLEunde_lockRankunde_a'
fixes a_VARIABLEunde_lockSubjectunde_a a_VARIABLEunde_lockSubjectunde_a'
fixes a_VARIABLEunde_highestRankunde_a a_VARIABLEunde_highestRankunde_a'
fixes a_VARIABLEunde_highestSubjectunde_a a_VARIABLEunde_highestSubjectunde_a'
fixes a_VARIABLEunde_pendingProposalunde_a a_VARIABLEunde_pendingProposalunde_a'
fixes a_VARIABLEunde_pendingPrepareunde_a a_VARIABLEunde_pendingPrepareunde_a'
fixes a_VARIABLEunde_pendingObservePrepareunde_a a_VARIABLEunde_pendingObservePrepareunde_a'
fixes a_VARIABLEunde_pendingLockCommitunde_a a_VARIABLEunde_pendingLockCommitunde_a'
fixes a_VARIABLEunde_pendingTimeoutunde_a a_VARIABLEunde_pendingTimeoutunde_a'
fixes a_VARIABLEunde_pendingInstallTCunde_a a_VARIABLEunde_pendingInstallTCunde_a'
fixes a_VARIABLEunde_pendingDecisionunde_a a_VARIABLEunde_pendingDecisionunde_a'
fixes a_VARIABLEunde_signProposalsunde_a a_VARIABLEunde_signProposalsunde_a'
fixes a_VARIABLEunde_signVotesunde_a a_VARIABLEunde_signVotesunde_a'
fixes a_VARIABLEunde_signTimeoutsunde_a a_VARIABLEunde_signTimeoutsunde_a'
fixes a_VARIABLEunde_proposalNetworkunde_a a_VARIABLEunde_proposalNetworkunde_a'
fixes a_VARIABLEunde_voteNetworkunde_a a_VARIABLEunde_voteNetworkunde_a'
fixes a_VARIABLEunde_qcNetworkunde_a a_VARIABLEunde_qcNetworkunde_a'
fixes a_VARIABLEunde_timeoutNetworkunde_a a_VARIABLEunde_timeoutNetworkunde_a'
fixes a_VARIABLEunde_tcNetworkunde_a a_VARIABLEunde_tcNetworkunde_a'
fixes a_VARIABLEunde_decisionsunde_a a_VARIABLEunde_decisionsunde_a'
fixes a_VARIABLEunde_appliedunde_a a_VARIABLEunde_appliedunde_a'
(* usable definition STATE_vars_ suppressed *)
(* usable definition STATE_CurrentEpoch_ suppressed *)
(* usable definition STATE_CurrentVoters_ suppressed *)
(* usable definition STATE_BroadcastProposals_ suppressed *)
(* usable definition STATE_BroadcastVotes_ suppressed *)
(* usable definition STATE_BroadcastQCs_ suppressed *)
(* usable definition STATE_BroadcastTimeouts_ suppressed *)
(* usable definition STATE_BroadcastTCs_ suppressed *)
(* usable definition STATE_PendingNodes_ suppressed *)
(* usable definition STATE_AllPendingRequests_ suppressed *)
(* usable definition CONSTANT_RequestNodeSet_ suppressed *)
(* usable definition CONSTANT_RequestsUniqueByNode_ suppressed *)
(* usable definition STATE_SigningNodes_ suppressed *)
(* usable definition STATE_SeenProposalValues_ suppressed *)
(* usable definition STATE_ReceivedQcValues_ suppressed *)
(* usable definition STATE_ReceivedTcValues_ suppressed *)
(* usable definition STATE_DecisionQcValues_ suppressed *)
(* usable definition STATE_NodeIdle_ suppressed *)
(* usable definition STATE_NodeTimedOut_ suppressed *)
(* usable definition STATE_NodeInstalledTC_ suppressed *)
(* usable definition STATE_HighRefValid_ suppressed *)
(* usable definition STATE_QcValid_ suppressed *)
(* usable definition CONSTANT_VoteBacksCertificate_ suppressed *)
(* usable definition CONSTANT_CertificateHonestIntentBacked_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteProtectsCommitSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesDisjoint_ suppressed *)
(* usable definition CONSTANT_TimeoutHighsConflictFree_ suppressed *)
(* usable definition CONSTANT_HighestTimeoutVote_ suppressed *)
(* usable definition STATE_TCValid_ suppressed *)
(* usable definition CONSTANT_TcHighRank_ suppressed *)
(* usable definition CONSTANT_TcHighSubject_ suppressed *)
(* usable definition STATE_ProposalJustified_ suppressed *)
(* usable definition STATE_SafeToPrepare_ suppressed *)
(* usable definition STATE_ProposalValidFor_ suppressed *)
(* usable definition STATE_VoteSignersAt_ suppressed *)
(* usable definition STATE_TimeoutVotesAt_ suppressed *)
(* usable definition CONSTANT_ModelConfiguration_ suppressed *)
(* usable definition ACTION_SetGST_ suppressed *)
(* usable definition ACTION_AssembleLocalBody_ suppressed *)
(* usable definition STATE_LocalProposalJustification_ suppressed *)
(* usable definition STATE_LocalProposalFor_ suppressed *)
(* usable definition ACTION_BeginLocalProposal_ suppressed *)
(* usable definition ACTION_PersistProposal_ suppressed *)
(* usable definition ACTION_CompleteProposalSignature_ suppressed *)
(* usable definition ACTION_DeliverProposal_ suppressed *)
(* usable definition ACTION_FetchBody_ suppressed *)
(* usable definition ACTION_StoreBody_ suppressed *)
(* usable definition ACTION_ValidateBody_ suppressed *)
(* usable definition ACTION_RejectBody_ suppressed *)
(* usable definition STATE_PrepareVoteFor_ suppressed *)
(* usable definition STATE_PrepareRequestFor_ suppressed *)
(* usable definition ACTION_BeginPrepare_ suppressed *)
(* usable definition ACTION_PersistPrepare_ suppressed *)
(* usable definition ACTION_CompleteVoteSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastVote_ suppressed *)
(* usable definition ACTION_DeliverVote_ suppressed *)
(* usable definition ACTION_FormPrepareQC_ suppressed *)
(* usable definition ACTION_DeliverQC_ suppressed *)
(* usable definition ACTION_BeginObservePrepare_ suppressed *)
(* usable definition ACTION_PersistObservePrepare_ suppressed *)
(* usable definition ACTION_BeginLockCommit_ suppressed *)
(* usable definition ACTION_PersistLockCommit_ suppressed *)
(* usable definition ACTION_FormCommitQC_ suppressed *)
(* usable definition ACTION_BeginDecision_ suppressed *)
(* usable definition ACTION_PersistDecision_ suppressed *)
(* usable definition STATE_LocalTimeoutVoteFor_ suppressed *)
(* usable definition STATE_TimeoutRequestFor_ suppressed *)
(* usable definition ACTION_BeginTimeout_ suppressed *)
(* usable definition ACTION_PersistTimeout_ suppressed *)
(* usable definition ACTION_CompleteTimeoutSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastTimeout_ suppressed *)
(* usable definition ACTION_DeliverTimeout_ suppressed *)
(* usable definition ACTION_FormTC_ suppressed *)
(* usable definition ACTION_DeliverTC_ suppressed *)
(* usable definition ACTION_BeginInstallTC_ suppressed *)
(* usable definition ACTION_PersistInstallTC_ suppressed *)
(* usable definition ACTION_FetchCertifiedBody_ suppressed *)
(* usable definition ACTION_ApplyDecision_ suppressed *)
(* usable definition ACTION_Crash_ suppressed *)
(* usable definition ACTION_Restart_ suppressed *)
(* usable definition ACTION_ResumeProposal_ suppressed *)
(* usable definition ACTION_ResumeVote_ suppressed *)
(* usable definition ACTION_ResumeTimeout_ suppressed *)
(* usable definition ACTION_DropProposal_ suppressed *)
(* usable definition ACTION_Next_ suppressed *)
(* usable definition ACTION_ReliableBeginTimeout_ suppressed *)
(* usable definition ACTION_ReliableNext_ suppressed *)
(* usable definition STATE_TypeInvariant_ suppressed *)
(* usable definition STATE_OnePendingPersistencePerNode_ suppressed *)
(* usable definition STATE_HonestPrepareUniqueness_ suppressed *)
(* usable definition STATE_HonestCommitUniqueness_ suppressed *)
(* usable definition STATE_HonestTimeoutUniqueness_ suppressed *)
(* usable definition STATE_LockBelowHighest_ suppressed *)
(* usable definition STATE_DecisionAgreement_ suppressed *)
(* usable definition STATE_OldViewCommitQCAccepted_ suppressed *)
(* usable definition STATE_AppliedRequiresDecision_ suppressed *)
(* usable definition STATE_Safety_ suppressed *)
(* usable definition TEMPORAL_CoreSpec_ suppressed *)
(* usable definition STATE_QuorumCheckNext_ suppressed *)
(* usable definition STATE_GenesisDecisionExists_ suppressed *)
(* usable definition TEMPORAL_PostGstEventuallyGenesisDecision_ suppressed *)
(* usable definition STATE_DurableProjection_ suppressed *)
(* usable definition ACTION_DurableProjectionPrime_ suppressed *)
(* usable definition ACTION_CrashPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_RestartPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_PendingWritesAreUnacknowledged_ suppressed *)
(* usable definition ACTION_StaleGenerationRejected_ suppressed *)
(* usable definition CONSTANT_Frame_ suppressed *)
(* usable definition CONSTANT_ContiguousCompletePrefix_ suppressed *)
(* usable definition CONSTANT_AcknowledgedFrames_ suppressed *)
(* usable definition CONSTANT_IncompleteFinalFrameUnacknowledged_ suppressed *)
(* usable definition CONSTANT_HashChainWellFormed_ suppressed *)
(* usable definition STATE_CommonAppliedSubject_ suppressed *)
(* usable definition ACTION_AdvanceContext_ suppressed *)
(* usable definition ACTION_NextV2_ suppressed *)
(* usable definition ACTION_ReliableNextV2_ suppressed *)
(* usable definition TEMPORAL_Spec_ suppressed *)
(* usable definition TEMPORAL_LivenessSpec_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsParent_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsFrozenEpoch_ suppressed *)
(* usable definition STATE_OldContextCertificateRejected_ suppressed *)
(* usable definition STATE_ContextParentWasApplied_ suppressed *)
(* usable definition STATE_EpochBoundarySafety_ suppressed *)
(* usable definition CONSTANT_SameVoteSlot_ suppressed *)
(* usable definition CONSTANT_HonestVoteUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendVote_ suppressed *)
(* usable definition CONSTANT_SameTimeoutSlot_ suppressed *)
(* usable definition CONSTANT_SameTimeoutContent_ suppressed *)
(* usable definition CONSTANT_HonestTimeoutUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendTimeout_ suppressed *)
(* usable definition CONSTANT_CertificateBackedBy_ suppressed *)
(* usable definition CONSTANT_SameCertificateSlot_ suppressed *)
(* usable definition CONSTANT_HonestIntentSound_ suppressed *)
(* usable definition CONSTANT_CertificateValidityAndAvailability_ suppressed *)
(* usable definition CONSTANT_LockValue_ suppressed *)
(* usable definition CONSTANT_CommitLockAllowed_ suppressed *)
(* usable definition CONSTANT_CommitLockResult_ suppressed *)
(* usable definition CONSTANT_InstallHighLock_ suppressed *)
(* usable definition CONSTANT_LockMonotone_ suppressed *)
(* usable definition CONSTANT_CommitSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutIntentProtectsCommits_ suppressed *)
(* usable definition CONSTANT_TCMaximumProtectsReports_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesBindCertificate_ suppressed *)
(* usable definition CONSTANT_TimeoutRanksTyped_ suppressed *)
(* usable definition CONSTANT_TimeoutProtectionKernel_ suppressed *)
(* usable definition CONSTANT_TCProtectsViewSubject_ suppressed *)
(* usable definition STATE_VoteIntentFor_ suppressed *)
(* usable definition STATE_PrepareCarriesHigherSafeQc_ suppressed *)
(* usable definition STATE_PrepareLineageSound_ suppressed *)
(* usable definition STATE_LocksCoverOwnCommits_ suppressed *)
(* usable definition STATE_CurrentIntentViewsBound_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_PendingCertificateWritesAuthorized_ suppressed *)
(* usable definition STATE_HonestVoteTransportBacked_ suppressed *)
(* usable definition STATE_QcTransportBacked_ suppressed *)
(* usable definition STATE_HonestTimeoutTransportBacked_ suppressed *)
(* usable definition STATE_TcTransportBacked_ suppressed *)
(* usable definition CONSTANT_HistoricalQcValid_ suppressed *)
(* usable definition STATE_CertificatesBackedByIntents_ suppressed *)
(* usable definition STATE_HonestDurableIntentsSound_ suppressed *)
(* usable definition STATE_FormedTimeoutCertificatesSound_ suppressed *)
(* usable definition STATE_DurableTimeoutsProtectCommits_ suppressed *)
(* usable definition STATE_HighestAndLockAreCertified_ suppressed *)
(* usable definition STATE_ReducerProvenanceInvariant_ suppressed *)
(* usable definition STATE_LineageInvariant_ suppressed *)
(* usable definition STATE_StrongInductiveInvariant_ suppressed *)
(* usable definition STATE_ProofRelevantVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutDurableVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutPendingProposalVars_ suppressed *)
(* usable definition CONSTANT_MapThenFoldSet_ suppressed *)
(* usable definition CONSTANT_Restrict_ suppressed *)
(* usable definition CONSTANT_RestrictDomain_ suppressed *)
(* usable definition CONSTANT_RestrictValues_ suppressed *)
(* usable definition CONSTANT_IsRestriction_ suppressed *)
(* usable definition CONSTANT_Range_ suppressed *)
(* usable definition CONSTANT_Pointwise_ suppressed *)
(* usable definition CONSTANT_Inverse_ suppressed *)
(* usable definition CONSTANT_AntiFunction_ suppressed *)
(* usable definition CONSTANT_IsInjective_ suppressed *)
(* usable definition CONSTANT_Injection_ suppressed *)
(* usable definition CONSTANT_Surjection_ suppressed *)
(* usable definition CONSTANT_Bijection_ suppressed *)
(* usable definition CONSTANT_ExistsInjection_ suppressed *)
(* usable definition CONSTANT_ExistsSurjection_ suppressed *)
(* usable definition CONSTANT_ExistsBijection_ suppressed *)
(* usable definition CONSTANT_FoldFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_FoldFunction_ suppressed *)
(* usable definition CONSTANT_SumFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_SumFunction_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_IsTransitivelyClosedOn_ suppressed *)
(* usable definition CONSTANT_IsWellFoundedOn_ suppressed *)
(* usable definition CONSTANT_SetLessThan_ suppressed *)
(* usable definition CONSTANT_WFDefOn_ suppressed *)
(* usable definition CONSTANT_OpDefinesFcn_ suppressed *)
(* usable definition CONSTANT_WFInductiveDefines_ suppressed *)
(* usable definition CONSTANT_WFInductiveUnique_ suppressed *)
(* usable definition CONSTANT_TransitiveClosureOn_ suppressed *)
(* usable definition CONSTANT_OpToRel_ suppressed *)
(* usable definition CONSTANT_PreImage_ suppressed *)
(* usable definition CONSTANT_LexPairOrdering_ suppressed *)
(* usable definition CONSTANT_LexProductOrdering_ suppressed *)
(* usable definition CONSTANT_FiniteSubsetsOf_ suppressed *)
(* usable definition CONSTANT_StrictSubsetOrdering_ suppressed *)
(* usable definition CONSTANT_EnabledWrapper_ suppressed *)
(* usable definition CONSTANT_CdotWrapper_ suppressed *)
(* usable definition STATE_InductiveInvariant_ suppressed *)
(* usable definition STATE_InitialStateObligation_ suppressed *)
assumes v'513: "(a_CONSTANTunde_ModelConfigurationunde_a)"
assumes v'514: "(((a_VARIABLEunde_heightunde_a) = ((0))))"
assumes v'515: "(((a_VARIABLEunde_contextunde_a) = ((a_CONSTANTunde_ContextRecordunde_a (((0)), (a_CONSTANTunde_NoSubjectunde_a))))))"
assumes v'516: "(((a_VARIABLEunde_contextHistoryunde_a) = ({(a_VARIABLEunde_contextunde_a)})))"
assumes v'517: "(((a_VARIABLEunde_nodeViewunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'518: "(((a_VARIABLEunde_generationunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'519: "(((a_VARIABLEunde_upunde_a) = (a_CONSTANTunde_ValidatorIdsunde_a)))"
assumes v'520: "(((a_VARIABLEunde_gstunde_a) = (FALSE)))"
assumes v'521: "(((a_VARIABLEunde_availableBodiesunde_a) = ({})))"
assumes v'522: "(((a_VARIABLEunde_durableBodiesunde_a) = ({})))"
assumes v'523: "(((a_VARIABLEunde_validatedBodiesunde_a) = ({})))"
assumes v'524: "(((a_VARIABLEunde_invalidBodiesunde_a) = ({})))"
assumes v'525: "(((a_VARIABLEunde_seenProposalsunde_a) = ({})))"
assumes v'526: "(((a_VARIABLEunde_receivedVotesunde_a) = ({})))"
assumes v'527: "(((a_VARIABLEunde_receivedQCsunde_a) = ({})))"
assumes v'528: "(((a_VARIABLEunde_receivedTimeoutVotesunde_a) = ({})))"
assumes v'529: "(((a_VARIABLEunde_receivedTCsunde_a) = ({})))"
assumes v'530: "(((a_VARIABLEunde_proposalIntentsunde_a) = ({})))"
assumes v'531: "(((a_VARIABLEunde_prepareIntentsunde_a) = ({})))"
assumes v'532: "(((a_VARIABLEunde_commitIntentsunde_a) = ({})))"
assumes v'533: "(((a_VARIABLEunde_timeoutIntentsunde_a) = ({})))"
assumes v'534: "(((a_VARIABLEunde_prepareQCsunde_a) = ({})))"
assumes v'535: "(((a_VARIABLEunde_commitQCsunde_a) = ({})))"
assumes v'536: "(((a_VARIABLEunde_formedTCsunde_a) = ({})))"
assumes v'537: "(((a_VARIABLEunde_installedTCsunde_a) = ({})))"
assumes v'538: "(((a_VARIABLEunde_lockRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'539: "(((a_VARIABLEunde_lockSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'540: "(((a_VARIABLEunde_highestRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'541: "(((a_VARIABLEunde_highestSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'542: "(((a_VARIABLEunde_pendingProposalunde_a) = ({})))"
assumes v'543: "(((a_VARIABLEunde_pendingPrepareunde_a) = ({})))"
assumes v'544: "(((a_VARIABLEunde_pendingObservePrepareunde_a) = ({})))"
assumes v'545: "(((a_VARIABLEunde_pendingLockCommitunde_a) = ({})))"
assumes v'546: "(((a_VARIABLEunde_pendingTimeoutunde_a) = ({})))"
assumes v'547: "(((a_VARIABLEunde_pendingInstallTCunde_a) = ({})))"
assumes v'548: "(((a_VARIABLEunde_pendingDecisionunde_a) = ({})))"
assumes v'549: "(((a_VARIABLEunde_signProposalsunde_a) = ({})))"
assumes v'550: "(((a_VARIABLEunde_signVotesunde_a) = ({})))"
assumes v'551: "(((a_VARIABLEunde_signTimeoutsunde_a) = ({})))"
assumes v'552: "(((a_VARIABLEunde_proposalNetworkunde_a) = ({})))"
assumes v'553: "(((a_VARIABLEunde_voteNetworkunde_a) = ({})))"
assumes v'554: "(((a_VARIABLEunde_qcNetworkunde_a) = ({})))"
assumes v'555: "(((a_VARIABLEunde_timeoutNetworkunde_a) = ({})))"
assumes v'556: "(((a_VARIABLEunde_tcNetworkunde_a) = ({})))"
assumes v'557: "(((a_VARIABLEunde_decisionsunde_a) = ({})))"
assumes v'558: "(((a_VARIABLEunde_appliedunde_a) = ({})))"
shows "((\<forall> a_CONSTANTunde_requestunde_a \<in> (a_VARIABLEunde_signProposalsunde_a) : (((fapply ((a_CONSTANTunde_requestunde_a), (''proposal''))) \<in> (a_VARIABLEunde_proposalIntentsunde_a)))) & (\<forall> a_CONSTANTunde_requestunde_a \<in> (a_VARIABLEunde_signVotesunde_a) : (((((fapply ((fapply ((a_CONSTANTunde_requestunde_a), (''vote''))), (''phase''))) = (''Prepare''))) \<Rightarrow> (((fapply ((a_CONSTANTunde_requestunde_a), (''vote''))) \<in> (a_VARIABLEunde_prepareIntentsunde_a)))))) & (\<forall> a_CONSTANTunde_requestunde_a \<in> (a_VARIABLEunde_signVotesunde_a) : (((((fapply ((fapply ((a_CONSTANTunde_requestunde_a), (''vote''))), (''phase''))) = (''Commit''))) \<Rightarrow> (((fapply ((a_CONSTANTunde_requestunde_a), (''vote''))) \<in> (a_VARIABLEunde_commitIntentsunde_a)))))) & (\<forall> a_CONSTANTunde_requestunde_a \<in> (a_VARIABLEunde_signTimeoutsunde_a) : (((fapply ((a_CONSTANTunde_requestunde_a), (''vote''))) \<in> (a_VARIABLEunde_timeoutIntentsunde_a)))))"(is "PROP ?ob'22")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 22"; \<close>
show "PROP ?ob'22"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 22"; \<close> qed
lemma ob'25:
(* usable definition CONSTANT_IsFiniteSet_ suppressed *)
(* usable definition CONSTANT_Cardinality_ suppressed *)
fixes a_CONSTANTunde_Nunde_a
fixes a_CONSTANTunde_MaxEpochunde_a
fixes a_CONSTANTunde_EpochRostersunde_a
fixes a_CONSTANTunde_EpochPowersunde_a
fixes a_CONSTANTunde_Honestunde_a
(* usable definition CONSTANT_ValidatorIds_ suppressed *)
(* usable definition CONSTANT_Epochs_ suppressed *)
(* usable definition CONSTANT_VotingPower_ suppressed *)
(* usable definition CONSTANT_RosterSequence_ suppressed *)
(* usable definition CONSTANT_VotingRoster_ suppressed *)
(* usable definition CONSTANT_Byzantine_ suppressed *)
(* usable definition CONSTANT_PowerUnits_ suppressed *)
(* usable definition CONSTANT_PowerOf_ suppressed *)
(* usable definition CONSTANT_CountQuorum_ suppressed *)
(* usable definition CONSTANT_PowerQuorum_ suppressed *)
(* usable definition CONSTANT_DualQuorum_ suppressed *)
(* usable definition CONSTANT_QuorumConfiguration_ suppressed *)
(* usable definition CONSTANT_CountQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_PowerQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_DualQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_NoSubject_ suppressed *)
(* usable definition CONSTANT_Subjects_ suppressed *)
(* usable definition CONSTANT_SubjectOrNone_ suppressed *)
(* usable definition CONSTANT_BodyRecord_ suppressed *)
(* usable definition CONSTANT_ValidationRecord_ suppressed *)
(* usable definition CONSTANT_BodyHeldBy_ suppressed *)
(* usable definition CONSTANT_BodyValidatedBy_ suppressed *)
(* usable definition CONSTANT_PrepareSignerAvailability_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyAvailable_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyValid_ suppressed *)
fixes a_CONSTANTunde_MaxHeightunde_a
fixes a_CONSTANTunde_MaxViewunde_a
fixes a_CONSTANTunde_MaxGenerationunde_a
fixes a_CONSTANTunde_EpochLengthunde_a
fixes a_CONSTANTunde_LeaderStartsunde_a
fixes a_CONSTANTunde_LaneHashesunde_a
fixes a_CONSTANTunde_DaHashesunde_a
fixes a_CONSTANTunde_ChainIdValueunde_a
fixes a_CONSTANTunde_ProtocolVersionValueunde_a
fixes a_CONSTANTunde_ValidSubjectsunde_a
fixes a_CONSTANTunde_Responsiveunde_a
(* usable definition CONSTANT_Heights_ suppressed *)
(* usable definition CONSTANT_Views_ suppressed *)
(* usable definition CONSTANT_Generations_ suppressed *)
(* usable definition CONSTANT_Phases_ suppressed *)
(* usable definition CONSTANT_NoRank_ suppressed *)
(* usable definition CONSTANT_Ranks_ suppressed *)
(* usable definition CONSTANT_CountRostersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountRostersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_CountPowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountPowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StakePowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_StakePowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StartsHeightZero_ suppressed *)
(* usable definition CONSTANT_StartsHeightZeroOne_ suppressed *)
(* usable definition CONSTANT_StartsByzantineFirst_ suppressed *)
(* usable definition CONSTANT_LaneHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_LaneHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_DaHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_DaHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_ExpectedEpoch_ suppressed *)
(* usable definition CONSTANT_ContextRecord_ suppressed *)
(* usable definition CONSTANT_ContextRecords_ suppressed *)
(* usable definition CONSTANT_Leader_ suppressed *)
(* usable definition CONSTANT_Proposal_ suppressed *)
(* usable definition CONSTANT_Vote_ suppressed *)
(* usable definition CONSTANT_QC_ suppressed *)
(* usable definition CONSTANT_TimeoutVote_ suppressed *)
(* usable definition CONSTANT_TC_ suppressed *)
(* usable definition CONSTANT_ProposalRecordSet_ suppressed *)
(* usable definition CONSTANT_VoteRecordSet_ suppressed *)
(* usable definition CONSTANT_QcRecordSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteRecordSet_ suppressed *)
(* usable definition CONSTANT_TcRecordSet_ suppressed *)
(* usable definition CONSTANT_TcWellTyped_ suppressed *)
(* usable definition CONSTANT_ProposalAt_ suppressed *)
(* usable definition CONSTANT_VoteAt_ suppressed *)
(* usable definition CONSTANT_QcAt_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteAt_ suppressed *)
(* usable definition CONSTANT_TcAt_ suppressed *)
(* usable definition CONSTANT_ProposalEnvelope_ suppressed *)
(* usable definition CONSTANT_VoteEnvelope_ suppressed *)
(* usable definition CONSTANT_QcEnvelope_ suppressed *)
(* usable definition CONSTANT_TimeoutEnvelope_ suppressed *)
(* usable definition CONSTANT_TcEnvelope_ suppressed *)
(* usable definition CONSTANT_ProposalWal_ suppressed *)
(* usable definition CONSTANT_PrepareWal_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWal_ suppressed *)
(* usable definition CONSTANT_LockCommitWal_ suppressed *)
(* usable definition CONSTANT_TimeoutWal_ suppressed *)
(* usable definition CONSTANT_InstallTcWal_ suppressed *)
(* usable definition CONSTANT_DecisionWal_ suppressed *)
(* usable definition CONSTANT_ProposalSign_ suppressed *)
(* usable definition CONSTANT_VoteSign_ suppressed *)
(* usable definition CONSTANT_TimeoutSign_ suppressed *)
(* usable definition CONSTANT_ProposalWalSet_ suppressed *)
(* usable definition CONSTANT_PrepareWalSet_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWalSet_ suppressed *)
(* usable definition CONSTANT_LockCommitWalSet_ suppressed *)
(* usable definition CONSTANT_TimeoutWalSet_ suppressed *)
(* usable definition CONSTANT_InstallTcWalSet_ suppressed *)
(* usable definition CONSTANT_DecisionWalSet_ suppressed *)
(* usable definition CONSTANT_ProposalSignSet_ suppressed *)
(* usable definition CONSTANT_VoteSignSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignSet_ suppressed *)
fixes a_VARIABLEunde_heightunde_a a_VARIABLEunde_heightunde_a'
fixes a_VARIABLEunde_contextunde_a a_VARIABLEunde_contextunde_a'
fixes a_VARIABLEunde_contextHistoryunde_a a_VARIABLEunde_contextHistoryunde_a'
fixes a_VARIABLEunde_nodeViewunde_a a_VARIABLEunde_nodeViewunde_a'
fixes a_VARIABLEunde_generationunde_a a_VARIABLEunde_generationunde_a'
fixes a_VARIABLEunde_upunde_a a_VARIABLEunde_upunde_a'
fixes a_VARIABLEunde_gstunde_a a_VARIABLEunde_gstunde_a'
fixes a_VARIABLEunde_availableBodiesunde_a a_VARIABLEunde_availableBodiesunde_a'
fixes a_VARIABLEunde_durableBodiesunde_a a_VARIABLEunde_durableBodiesunde_a'
fixes a_VARIABLEunde_validatedBodiesunde_a a_VARIABLEunde_validatedBodiesunde_a'
fixes a_VARIABLEunde_invalidBodiesunde_a a_VARIABLEunde_invalidBodiesunde_a'
fixes a_VARIABLEunde_seenProposalsunde_a a_VARIABLEunde_seenProposalsunde_a'
fixes a_VARIABLEunde_receivedVotesunde_a a_VARIABLEunde_receivedVotesunde_a'
fixes a_VARIABLEunde_receivedQCsunde_a a_VARIABLEunde_receivedQCsunde_a'
fixes a_VARIABLEunde_receivedTimeoutVotesunde_a a_VARIABLEunde_receivedTimeoutVotesunde_a'
fixes a_VARIABLEunde_receivedTCsunde_a a_VARIABLEunde_receivedTCsunde_a'
fixes a_VARIABLEunde_proposalIntentsunde_a a_VARIABLEunde_proposalIntentsunde_a'
fixes a_VARIABLEunde_prepareIntentsunde_a a_VARIABLEunde_prepareIntentsunde_a'
fixes a_VARIABLEunde_commitIntentsunde_a a_VARIABLEunde_commitIntentsunde_a'
fixes a_VARIABLEunde_timeoutIntentsunde_a a_VARIABLEunde_timeoutIntentsunde_a'
fixes a_VARIABLEunde_prepareQCsunde_a a_VARIABLEunde_prepareQCsunde_a'
fixes a_VARIABLEunde_commitQCsunde_a a_VARIABLEunde_commitQCsunde_a'
fixes a_VARIABLEunde_formedTCsunde_a a_VARIABLEunde_formedTCsunde_a'
fixes a_VARIABLEunde_installedTCsunde_a a_VARIABLEunde_installedTCsunde_a'
fixes a_VARIABLEunde_lockRankunde_a a_VARIABLEunde_lockRankunde_a'
fixes a_VARIABLEunde_lockSubjectunde_a a_VARIABLEunde_lockSubjectunde_a'
fixes a_VARIABLEunde_highestRankunde_a a_VARIABLEunde_highestRankunde_a'
fixes a_VARIABLEunde_highestSubjectunde_a a_VARIABLEunde_highestSubjectunde_a'
fixes a_VARIABLEunde_pendingProposalunde_a a_VARIABLEunde_pendingProposalunde_a'
fixes a_VARIABLEunde_pendingPrepareunde_a a_VARIABLEunde_pendingPrepareunde_a'
fixes a_VARIABLEunde_pendingObservePrepareunde_a a_VARIABLEunde_pendingObservePrepareunde_a'
fixes a_VARIABLEunde_pendingLockCommitunde_a a_VARIABLEunde_pendingLockCommitunde_a'
fixes a_VARIABLEunde_pendingTimeoutunde_a a_VARIABLEunde_pendingTimeoutunde_a'
fixes a_VARIABLEunde_pendingInstallTCunde_a a_VARIABLEunde_pendingInstallTCunde_a'
fixes a_VARIABLEunde_pendingDecisionunde_a a_VARIABLEunde_pendingDecisionunde_a'
fixes a_VARIABLEunde_signProposalsunde_a a_VARIABLEunde_signProposalsunde_a'
fixes a_VARIABLEunde_signVotesunde_a a_VARIABLEunde_signVotesunde_a'
fixes a_VARIABLEunde_signTimeoutsunde_a a_VARIABLEunde_signTimeoutsunde_a'
fixes a_VARIABLEunde_proposalNetworkunde_a a_VARIABLEunde_proposalNetworkunde_a'
fixes a_VARIABLEunde_voteNetworkunde_a a_VARIABLEunde_voteNetworkunde_a'
fixes a_VARIABLEunde_qcNetworkunde_a a_VARIABLEunde_qcNetworkunde_a'
fixes a_VARIABLEunde_timeoutNetworkunde_a a_VARIABLEunde_timeoutNetworkunde_a'
fixes a_VARIABLEunde_tcNetworkunde_a a_VARIABLEunde_tcNetworkunde_a'
fixes a_VARIABLEunde_decisionsunde_a a_VARIABLEunde_decisionsunde_a'
fixes a_VARIABLEunde_appliedunde_a a_VARIABLEunde_appliedunde_a'
(* usable definition STATE_vars_ suppressed *)
(* usable definition STATE_CurrentEpoch_ suppressed *)
(* usable definition STATE_CurrentVoters_ suppressed *)
(* usable definition STATE_BroadcastProposals_ suppressed *)
(* usable definition STATE_BroadcastVotes_ suppressed *)
(* usable definition STATE_BroadcastQCs_ suppressed *)
(* usable definition STATE_BroadcastTimeouts_ suppressed *)
(* usable definition STATE_BroadcastTCs_ suppressed *)
(* usable definition STATE_PendingNodes_ suppressed *)
(* usable definition STATE_AllPendingRequests_ suppressed *)
(* usable definition CONSTANT_RequestNodeSet_ suppressed *)
(* usable definition CONSTANT_RequestsUniqueByNode_ suppressed *)
(* usable definition STATE_SigningNodes_ suppressed *)
(* usable definition STATE_SeenProposalValues_ suppressed *)
(* usable definition STATE_ReceivedQcValues_ suppressed *)
(* usable definition STATE_ReceivedTcValues_ suppressed *)
(* usable definition STATE_DecisionQcValues_ suppressed *)
(* usable definition STATE_NodeIdle_ suppressed *)
(* usable definition STATE_NodeTimedOut_ suppressed *)
(* usable definition STATE_NodeInstalledTC_ suppressed *)
(* usable definition STATE_HighRefValid_ suppressed *)
(* usable definition STATE_QcValid_ suppressed *)
(* usable definition CONSTANT_VoteBacksCertificate_ suppressed *)
(* usable definition CONSTANT_CertificateHonestIntentBacked_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteProtectsCommitSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesDisjoint_ suppressed *)
(* usable definition CONSTANT_TimeoutHighsConflictFree_ suppressed *)
(* usable definition CONSTANT_HighestTimeoutVote_ suppressed *)
(* usable definition STATE_TCValid_ suppressed *)
(* usable definition CONSTANT_TcHighRank_ suppressed *)
(* usable definition CONSTANT_TcHighSubject_ suppressed *)
(* usable definition STATE_ProposalJustified_ suppressed *)
(* usable definition STATE_SafeToPrepare_ suppressed *)
(* usable definition STATE_ProposalValidFor_ suppressed *)
(* usable definition STATE_VoteSignersAt_ suppressed *)
(* usable definition STATE_TimeoutVotesAt_ suppressed *)
(* usable definition CONSTANT_ModelConfiguration_ suppressed *)
(* usable definition ACTION_SetGST_ suppressed *)
(* usable definition ACTION_AssembleLocalBody_ suppressed *)
(* usable definition STATE_LocalProposalJustification_ suppressed *)
(* usable definition STATE_LocalProposalFor_ suppressed *)
(* usable definition ACTION_BeginLocalProposal_ suppressed *)
(* usable definition ACTION_PersistProposal_ suppressed *)
(* usable definition ACTION_CompleteProposalSignature_ suppressed *)
(* usable definition ACTION_DeliverProposal_ suppressed *)
(* usable definition ACTION_FetchBody_ suppressed *)
(* usable definition ACTION_StoreBody_ suppressed *)
(* usable definition ACTION_ValidateBody_ suppressed *)
(* usable definition ACTION_RejectBody_ suppressed *)
(* usable definition STATE_PrepareVoteFor_ suppressed *)
(* usable definition STATE_PrepareRequestFor_ suppressed *)
(* usable definition ACTION_BeginPrepare_ suppressed *)
(* usable definition ACTION_PersistPrepare_ suppressed *)
(* usable definition ACTION_CompleteVoteSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastVote_ suppressed *)
(* usable definition ACTION_DeliverVote_ suppressed *)
(* usable definition ACTION_FormPrepareQC_ suppressed *)
(* usable definition ACTION_DeliverQC_ suppressed *)
(* usable definition ACTION_BeginObservePrepare_ suppressed *)
(* usable definition ACTION_PersistObservePrepare_ suppressed *)
(* usable definition ACTION_BeginLockCommit_ suppressed *)
(* usable definition ACTION_PersistLockCommit_ suppressed *)
(* usable definition ACTION_FormCommitQC_ suppressed *)
(* usable definition ACTION_BeginDecision_ suppressed *)
(* usable definition ACTION_PersistDecision_ suppressed *)
(* usable definition STATE_LocalTimeoutVoteFor_ suppressed *)
(* usable definition STATE_TimeoutRequestFor_ suppressed *)
(* usable definition ACTION_BeginTimeout_ suppressed *)
(* usable definition ACTION_PersistTimeout_ suppressed *)
(* usable definition ACTION_CompleteTimeoutSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastTimeout_ suppressed *)
(* usable definition ACTION_DeliverTimeout_ suppressed *)
(* usable definition ACTION_FormTC_ suppressed *)
(* usable definition ACTION_DeliverTC_ suppressed *)
(* usable definition ACTION_BeginInstallTC_ suppressed *)
(* usable definition ACTION_PersistInstallTC_ suppressed *)
(* usable definition ACTION_FetchCertifiedBody_ suppressed *)
(* usable definition ACTION_ApplyDecision_ suppressed *)
(* usable definition ACTION_Crash_ suppressed *)
(* usable definition ACTION_Restart_ suppressed *)
(* usable definition ACTION_ResumeProposal_ suppressed *)
(* usable definition ACTION_ResumeVote_ suppressed *)
(* usable definition ACTION_ResumeTimeout_ suppressed *)
(* usable definition ACTION_DropProposal_ suppressed *)
(* usable definition ACTION_Next_ suppressed *)
(* usable definition ACTION_ReliableBeginTimeout_ suppressed *)
(* usable definition ACTION_ReliableNext_ suppressed *)
(* usable definition STATE_TypeInvariant_ suppressed *)
(* usable definition STATE_OnePendingPersistencePerNode_ suppressed *)
(* usable definition STATE_PrepareSigningRequiresIntent_ suppressed *)
(* usable definition STATE_CommitSigningRequiresIntent_ suppressed *)
(* usable definition STATE_TimeoutSigningRequiresIntent_ suppressed *)
(* usable definition STATE_ProposalSigningRequiresIntent_ suppressed *)
(* usable definition STATE_LockBelowHighest_ suppressed *)
(* usable definition STATE_OldViewCommitQCAccepted_ suppressed *)
(* usable definition STATE_Safety_ suppressed *)
(* usable definition TEMPORAL_CoreSpec_ suppressed *)
(* usable definition STATE_QuorumCheckNext_ suppressed *)
(* usable definition STATE_GenesisDecisionExists_ suppressed *)
(* usable definition TEMPORAL_PostGstEventuallyGenesisDecision_ suppressed *)
(* usable definition STATE_DurableProjection_ suppressed *)
(* usable definition ACTION_DurableProjectionPrime_ suppressed *)
(* usable definition ACTION_CrashPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_RestartPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_PendingWritesAreUnacknowledged_ suppressed *)
(* usable definition ACTION_StaleGenerationRejected_ suppressed *)
(* usable definition CONSTANT_Frame_ suppressed *)
(* usable definition CONSTANT_ContiguousCompletePrefix_ suppressed *)
(* usable definition CONSTANT_AcknowledgedFrames_ suppressed *)
(* usable definition CONSTANT_IncompleteFinalFrameUnacknowledged_ suppressed *)
(* usable definition CONSTANT_HashChainWellFormed_ suppressed *)
(* usable definition STATE_CommonAppliedSubject_ suppressed *)
(* usable definition ACTION_AdvanceContext_ suppressed *)
(* usable definition ACTION_NextV2_ suppressed *)
(* usable definition ACTION_ReliableNextV2_ suppressed *)
(* usable definition TEMPORAL_Spec_ suppressed *)
(* usable definition TEMPORAL_LivenessSpec_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsParent_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsFrozenEpoch_ suppressed *)
(* usable definition STATE_OldContextCertificateRejected_ suppressed *)
(* usable definition STATE_ContextParentWasApplied_ suppressed *)
(* usable definition STATE_EpochBoundarySafety_ suppressed *)
(* usable definition CONSTANT_SameVoteSlot_ suppressed *)
(* usable definition CONSTANT_HonestVoteUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendVote_ suppressed *)
(* usable definition CONSTANT_SameTimeoutSlot_ suppressed *)
(* usable definition CONSTANT_SameTimeoutContent_ suppressed *)
(* usable definition CONSTANT_HonestTimeoutUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendTimeout_ suppressed *)
(* usable definition CONSTANT_CertificateBackedBy_ suppressed *)
(* usable definition CONSTANT_SameCertificateSlot_ suppressed *)
(* usable definition CONSTANT_HonestIntentSound_ suppressed *)
(* usable definition CONSTANT_CertificateValidityAndAvailability_ suppressed *)
(* usable definition CONSTANT_LockValue_ suppressed *)
(* usable definition CONSTANT_CommitLockAllowed_ suppressed *)
(* usable definition CONSTANT_CommitLockResult_ suppressed *)
(* usable definition CONSTANT_InstallHighLock_ suppressed *)
(* usable definition CONSTANT_LockMonotone_ suppressed *)
(* usable definition CONSTANT_CommitSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutIntentProtectsCommits_ suppressed *)
(* usable definition CONSTANT_TCMaximumProtectsReports_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesBindCertificate_ suppressed *)
(* usable definition CONSTANT_TimeoutRanksTyped_ suppressed *)
(* usable definition CONSTANT_TimeoutProtectionKernel_ suppressed *)
(* usable definition CONSTANT_TCProtectsViewSubject_ suppressed *)
(* usable definition STATE_VoteIntentFor_ suppressed *)
(* usable definition STATE_PrepareCarriesHigherSafeQc_ suppressed *)
(* usable definition STATE_PrepareLineageSound_ suppressed *)
(* usable definition STATE_LocksCoverOwnCommits_ suppressed *)
(* usable definition STATE_CurrentIntentViewsBound_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_PendingCertificateWritesAuthorized_ suppressed *)
(* usable definition STATE_HonestVoteTransportBacked_ suppressed *)
(* usable definition STATE_QcTransportBacked_ suppressed *)
(* usable definition STATE_HonestTimeoutTransportBacked_ suppressed *)
(* usable definition STATE_TcTransportBacked_ suppressed *)
(* usable definition CONSTANT_HistoricalQcValid_ suppressed *)
(* usable definition STATE_CertificatesBackedByIntents_ suppressed *)
(* usable definition STATE_HonestDurableIntentsSound_ suppressed *)
(* usable definition STATE_FormedTimeoutCertificatesSound_ suppressed *)
(* usable definition STATE_DurableTimeoutsProtectCommits_ suppressed *)
(* usable definition STATE_HighestAndLockAreCertified_ suppressed *)
(* usable definition STATE_ReducerProvenanceInvariant_ suppressed *)
(* usable definition STATE_LineageInvariant_ suppressed *)
(* usable definition STATE_StrongInductiveInvariant_ suppressed *)
(* usable definition STATE_ProofRelevantVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutDurableVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutPendingProposalVars_ suppressed *)
(* usable definition CONSTANT_MapThenFoldSet_ suppressed *)
(* usable definition CONSTANT_Restrict_ suppressed *)
(* usable definition CONSTANT_RestrictDomain_ suppressed *)
(* usable definition CONSTANT_RestrictValues_ suppressed *)
(* usable definition CONSTANT_IsRestriction_ suppressed *)
(* usable definition CONSTANT_Range_ suppressed *)
(* usable definition CONSTANT_Pointwise_ suppressed *)
(* usable definition CONSTANT_Inverse_ suppressed *)
(* usable definition CONSTANT_AntiFunction_ suppressed *)
(* usable definition CONSTANT_IsInjective_ suppressed *)
(* usable definition CONSTANT_Injection_ suppressed *)
(* usable definition CONSTANT_Surjection_ suppressed *)
(* usable definition CONSTANT_Bijection_ suppressed *)
(* usable definition CONSTANT_ExistsInjection_ suppressed *)
(* usable definition CONSTANT_ExistsSurjection_ suppressed *)
(* usable definition CONSTANT_ExistsBijection_ suppressed *)
(* usable definition CONSTANT_FoldFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_FoldFunction_ suppressed *)
(* usable definition CONSTANT_SumFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_SumFunction_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_IsTransitivelyClosedOn_ suppressed *)
(* usable definition CONSTANT_IsWellFoundedOn_ suppressed *)
(* usable definition CONSTANT_SetLessThan_ suppressed *)
(* usable definition CONSTANT_WFDefOn_ suppressed *)
(* usable definition CONSTANT_OpDefinesFcn_ suppressed *)
(* usable definition CONSTANT_WFInductiveDefines_ suppressed *)
(* usable definition CONSTANT_WFInductiveUnique_ suppressed *)
(* usable definition CONSTANT_TransitiveClosureOn_ suppressed *)
(* usable definition CONSTANT_OpToRel_ suppressed *)
(* usable definition CONSTANT_PreImage_ suppressed *)
(* usable definition CONSTANT_LexPairOrdering_ suppressed *)
(* usable definition CONSTANT_LexProductOrdering_ suppressed *)
(* usable definition CONSTANT_FiniteSubsetsOf_ suppressed *)
(* usable definition CONSTANT_StrictSubsetOrdering_ suppressed *)
(* usable definition CONSTANT_EnabledWrapper_ suppressed *)
(* usable definition CONSTANT_CdotWrapper_ suppressed *)
(* usable definition STATE_InductiveInvariant_ suppressed *)
(* usable definition STATE_InitialStateObligation_ suppressed *)
assumes v'513: "(a_CONSTANTunde_ModelConfigurationunde_a)"
assumes v'514: "(((a_VARIABLEunde_heightunde_a) = ((0))))"
assumes v'515: "(((a_VARIABLEunde_contextunde_a) = ((a_CONSTANTunde_ContextRecordunde_a (((0)), (a_CONSTANTunde_NoSubjectunde_a))))))"
assumes v'516: "(((a_VARIABLEunde_contextHistoryunde_a) = ({(a_VARIABLEunde_contextunde_a)})))"
assumes v'517: "(((a_VARIABLEunde_nodeViewunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'518: "(((a_VARIABLEunde_generationunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'519: "(((a_VARIABLEunde_upunde_a) = (a_CONSTANTunde_ValidatorIdsunde_a)))"
assumes v'520: "(((a_VARIABLEunde_gstunde_a) = (FALSE)))"
assumes v'521: "(((a_VARIABLEunde_availableBodiesunde_a) = ({})))"
assumes v'522: "(((a_VARIABLEunde_durableBodiesunde_a) = ({})))"
assumes v'523: "(((a_VARIABLEunde_validatedBodiesunde_a) = ({})))"
assumes v'524: "(((a_VARIABLEunde_invalidBodiesunde_a) = ({})))"
assumes v'525: "(((a_VARIABLEunde_seenProposalsunde_a) = ({})))"
assumes v'526: "(((a_VARIABLEunde_receivedVotesunde_a) = ({})))"
assumes v'527: "(((a_VARIABLEunde_receivedQCsunde_a) = ({})))"
assumes v'528: "(((a_VARIABLEunde_receivedTimeoutVotesunde_a) = ({})))"
assumes v'529: "(((a_VARIABLEunde_receivedTCsunde_a) = ({})))"
assumes v'530: "(((a_VARIABLEunde_proposalIntentsunde_a) = ({})))"
assumes v'531: "(((a_VARIABLEunde_prepareIntentsunde_a) = ({})))"
assumes v'532: "(((a_VARIABLEunde_commitIntentsunde_a) = ({})))"
assumes v'533: "(((a_VARIABLEunde_timeoutIntentsunde_a) = ({})))"
assumes v'534: "(((a_VARIABLEunde_prepareQCsunde_a) = ({})))"
assumes v'535: "(((a_VARIABLEunde_commitQCsunde_a) = ({})))"
assumes v'536: "(((a_VARIABLEunde_formedTCsunde_a) = ({})))"
assumes v'537: "(((a_VARIABLEunde_installedTCsunde_a) = ({})))"
assumes v'538: "(((a_VARIABLEunde_lockRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'539: "(((a_VARIABLEunde_lockSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'540: "(((a_VARIABLEunde_highestRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'541: "(((a_VARIABLEunde_highestSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'542: "(((a_VARIABLEunde_pendingProposalunde_a) = ({})))"
assumes v'543: "(((a_VARIABLEunde_pendingPrepareunde_a) = ({})))"
assumes v'544: "(((a_VARIABLEunde_pendingObservePrepareunde_a) = ({})))"
assumes v'545: "(((a_VARIABLEunde_pendingLockCommitunde_a) = ({})))"
assumes v'546: "(((a_VARIABLEunde_pendingTimeoutunde_a) = ({})))"
assumes v'547: "(((a_VARIABLEunde_pendingInstallTCunde_a) = ({})))"
assumes v'548: "(((a_VARIABLEunde_pendingDecisionunde_a) = ({})))"
assumes v'549: "(((a_VARIABLEunde_signProposalsunde_a) = ({})))"
assumes v'550: "(((a_VARIABLEunde_signVotesunde_a) = ({})))"
assumes v'551: "(((a_VARIABLEunde_signTimeoutsunde_a) = ({})))"
assumes v'552: "(((a_VARIABLEunde_proposalNetworkunde_a) = ({})))"
assumes v'553: "(((a_VARIABLEunde_voteNetworkunde_a) = ({})))"
assumes v'554: "(((a_VARIABLEunde_qcNetworkunde_a) = ({})))"
assumes v'555: "(((a_VARIABLEunde_timeoutNetworkunde_a) = ({})))"
assumes v'556: "(((a_VARIABLEunde_tcNetworkunde_a) = ({})))"
assumes v'557: "(((a_VARIABLEunde_decisionsunde_a) = ({})))"
assumes v'558: "(((a_VARIABLEunde_appliedunde_a) = ({})))"
shows "((\<forall> a_CONSTANTunde_leftunde_a \<in> (a_VARIABLEunde_prepareIntentsunde_a) : (\<forall> a_CONSTANTunde_rightunde_a \<in> (a_VARIABLEunde_prepareIntentsunde_a) : (((((((((((fapply ((a_CONSTANTunde_leftunde_a), (''signer''))) \<in> (a_CONSTANTunde_Honestunde_a))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''signer''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''signer''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''context''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''context''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''view''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''view''))))))) \<Rightarrow> (((fapply ((a_CONSTANTunde_rightunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''subject''))))))))) & (\<forall> a_CONSTANTunde_leftunde_a \<in> (a_VARIABLEunde_commitIntentsunde_a) : (\<forall> a_CONSTANTunde_rightunde_a \<in> (a_VARIABLEunde_commitIntentsunde_a) : (((((((((((fapply ((a_CONSTANTunde_leftunde_a), (''signer''))) \<in> (a_CONSTANTunde_Honestunde_a))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''signer''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''signer''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''context''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''context''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''view''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''view''))))))) \<Rightarrow> (((fapply ((a_CONSTANTunde_rightunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''subject''))))))))) & (\<forall> a_CONSTANTunde_leftunde_a \<in> (a_VARIABLEunde_timeoutIntentsunde_a) : (\<forall> a_CONSTANTunde_rightunde_a \<in> (a_VARIABLEunde_timeoutIntentsunde_a) : (((((((((((fapply ((a_CONSTANTunde_leftunde_a), (''signer''))) \<in> (a_CONSTANTunde_Honestunde_a))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''signer''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''signer''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''context''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''context''))))))) \<and> (((fapply ((a_CONSTANTunde_rightunde_a), (''view''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''view''))))))) \<Rightarrow> ((((fapply ((a_CONSTANTunde_rightunde_a), (''highRank''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''highRank''))))) & (((fapply ((a_CONSTANTunde_rightunde_a), (''highSubject''))) = (fapply ((a_CONSTANTunde_leftunde_a), (''highSubject'')))))))))) & (\<forall> a_CONSTANTunde_leftunde_a \<in> (a_VARIABLEunde_decisionsunde_a) : (\<forall> a_CONSTANTunde_rightunde_a \<in> (a_VARIABLEunde_decisionsunde_a) : (((((fapply ((fapply ((a_CONSTANTunde_leftunde_a), (''qc''))), (''context''))) = (fapply ((fapply ((a_CONSTANTunde_rightunde_a), (''qc''))), (''context''))))) \<Rightarrow> (((fapply ((fapply ((a_CONSTANTunde_leftunde_a), (''qc''))), (''subject''))) = (fapply ((fapply ((a_CONSTANTunde_rightunde_a), (''qc''))), (''subject''))))))))) & (((a_VARIABLEunde_appliedunde_a) \<subseteq> (a_VARIABLEunde_decisionsunde_a))))"(is "PROP ?ob'25")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 25"; \<close>
show "PROP ?ob'25"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 25"; \<close> qed
lemma ob'38:
(* usable definition CONSTANT_IsFiniteSet_ suppressed *)
(* usable definition CONSTANT_Cardinality_ suppressed *)
fixes a_CONSTANTunde_Nunde_a
fixes a_CONSTANTunde_MaxEpochunde_a
fixes a_CONSTANTunde_EpochRostersunde_a
fixes a_CONSTANTunde_EpochPowersunde_a
fixes a_CONSTANTunde_Honestunde_a
(* usable definition CONSTANT_ValidatorIds_ suppressed *)
(* usable definition CONSTANT_Epochs_ suppressed *)
(* usable definition CONSTANT_VotingPower_ suppressed *)
(* usable definition CONSTANT_RosterSequence_ suppressed *)
(* usable definition CONSTANT_VotingRoster_ suppressed *)
(* usable definition CONSTANT_Byzantine_ suppressed *)
(* usable definition CONSTANT_PowerUnits_ suppressed *)
(* usable definition CONSTANT_PowerOf_ suppressed *)
(* usable definition CONSTANT_CountQuorum_ suppressed *)
(* usable definition CONSTANT_PowerQuorum_ suppressed *)
(* usable definition CONSTANT_DualQuorum_ suppressed *)
(* usable definition CONSTANT_QuorumConfiguration_ suppressed *)
(* usable definition CONSTANT_CountQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_PowerQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_DualQuorumIntersectionHasHonest_ suppressed *)
(* usable definition CONSTANT_NoSubject_ suppressed *)
(* usable definition CONSTANT_Subjects_ suppressed *)
(* usable definition CONSTANT_SubjectOrNone_ suppressed *)
(* usable definition CONSTANT_BodyRecord_ suppressed *)
(* usable definition CONSTANT_ValidationRecord_ suppressed *)
(* usable definition CONSTANT_BodyHeldBy_ suppressed *)
(* usable definition CONSTANT_BodyValidatedBy_ suppressed *)
(* usable definition CONSTANT_PrepareSignerAvailability_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyAvailable_ suppressed *)
(* usable definition CONSTANT_CertifiedBodyValid_ suppressed *)
fixes a_CONSTANTunde_MaxHeightunde_a
fixes a_CONSTANTunde_MaxViewunde_a
fixes a_CONSTANTunde_MaxGenerationunde_a
fixes a_CONSTANTunde_EpochLengthunde_a
fixes a_CONSTANTunde_LeaderStartsunde_a
fixes a_CONSTANTunde_LaneHashesunde_a
fixes a_CONSTANTunde_DaHashesunde_a
fixes a_CONSTANTunde_ChainIdValueunde_a
fixes a_CONSTANTunde_ProtocolVersionValueunde_a
fixes a_CONSTANTunde_ValidSubjectsunde_a
fixes a_CONSTANTunde_Responsiveunde_a
(* usable definition CONSTANT_Heights_ suppressed *)
(* usable definition CONSTANT_Views_ suppressed *)
(* usable definition CONSTANT_Generations_ suppressed *)
(* usable definition CONSTANT_Phases_ suppressed *)
(* usable definition CONSTANT_NoRank_ suppressed *)
(* usable definition CONSTANT_Ranks_ suppressed *)
(* usable definition CONSTANT_CountRostersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountRostersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_CountPowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_CountPowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StakePowersOneEpoch_ suppressed *)
(* usable definition CONSTANT_StakePowersTwoEpochs_ suppressed *)
(* usable definition CONSTANT_StartsHeightZero_ suppressed *)
(* usable definition CONSTANT_StartsHeightZeroOne_ suppressed *)
(* usable definition CONSTANT_StartsByzantineFirst_ suppressed *)
(* usable definition CONSTANT_LaneHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_LaneHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_DaHashesOneHeight_ suppressed *)
(* usable definition CONSTANT_DaHashesTwoHeights_ suppressed *)
(* usable definition CONSTANT_ExpectedEpoch_ suppressed *)
(* usable definition CONSTANT_ContextRecord_ suppressed *)
(* usable definition CONSTANT_ContextRecords_ suppressed *)
(* usable definition CONSTANT_Leader_ suppressed *)
(* usable definition CONSTANT_Proposal_ suppressed *)
(* usable definition CONSTANT_Vote_ suppressed *)
(* usable definition CONSTANT_QC_ suppressed *)
(* usable definition CONSTANT_TimeoutVote_ suppressed *)
(* usable definition CONSTANT_TC_ suppressed *)
(* usable definition CONSTANT_ProposalRecordSet_ suppressed *)
(* usable definition CONSTANT_VoteRecordSet_ suppressed *)
(* usable definition CONSTANT_QcRecordSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteRecordSet_ suppressed *)
(* usable definition CONSTANT_TcRecordSet_ suppressed *)
(* usable definition CONSTANT_TcWellTyped_ suppressed *)
(* usable definition CONSTANT_ProposalAt_ suppressed *)
(* usable definition CONSTANT_VoteAt_ suppressed *)
(* usable definition CONSTANT_QcAt_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteAt_ suppressed *)
(* usable definition CONSTANT_TcAt_ suppressed *)
(* usable definition CONSTANT_ProposalEnvelope_ suppressed *)
(* usable definition CONSTANT_VoteEnvelope_ suppressed *)
(* usable definition CONSTANT_QcEnvelope_ suppressed *)
(* usable definition CONSTANT_TimeoutEnvelope_ suppressed *)
(* usable definition CONSTANT_TcEnvelope_ suppressed *)
(* usable definition CONSTANT_ProposalWal_ suppressed *)
(* usable definition CONSTANT_PrepareWal_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWal_ suppressed *)
(* usable definition CONSTANT_LockCommitWal_ suppressed *)
(* usable definition CONSTANT_TimeoutWal_ suppressed *)
(* usable definition CONSTANT_InstallTcWal_ suppressed *)
(* usable definition CONSTANT_DecisionWal_ suppressed *)
(* usable definition CONSTANT_ProposalSign_ suppressed *)
(* usable definition CONSTANT_VoteSign_ suppressed *)
(* usable definition CONSTANT_TimeoutSign_ suppressed *)
(* usable definition CONSTANT_ProposalWalSet_ suppressed *)
(* usable definition CONSTANT_PrepareWalSet_ suppressed *)
(* usable definition CONSTANT_ObservePrepareWalSet_ suppressed *)
(* usable definition CONSTANT_LockCommitWalSet_ suppressed *)
(* usable definition CONSTANT_TimeoutWalSet_ suppressed *)
(* usable definition CONSTANT_InstallTcWalSet_ suppressed *)
(* usable definition CONSTANT_DecisionWalSet_ suppressed *)
(* usable definition CONSTANT_ProposalSignSet_ suppressed *)
(* usable definition CONSTANT_VoteSignSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignSet_ suppressed *)
fixes a_VARIABLEunde_heightunde_a a_VARIABLEunde_heightunde_a'
fixes a_VARIABLEunde_contextunde_a a_VARIABLEunde_contextunde_a'
fixes a_VARIABLEunde_contextHistoryunde_a a_VARIABLEunde_contextHistoryunde_a'
fixes a_VARIABLEunde_nodeViewunde_a a_VARIABLEunde_nodeViewunde_a'
fixes a_VARIABLEunde_generationunde_a a_VARIABLEunde_generationunde_a'
fixes a_VARIABLEunde_upunde_a a_VARIABLEunde_upunde_a'
fixes a_VARIABLEunde_gstunde_a a_VARIABLEunde_gstunde_a'
fixes a_VARIABLEunde_availableBodiesunde_a a_VARIABLEunde_availableBodiesunde_a'
fixes a_VARIABLEunde_durableBodiesunde_a a_VARIABLEunde_durableBodiesunde_a'
fixes a_VARIABLEunde_validatedBodiesunde_a a_VARIABLEunde_validatedBodiesunde_a'
fixes a_VARIABLEunde_invalidBodiesunde_a a_VARIABLEunde_invalidBodiesunde_a'
fixes a_VARIABLEunde_seenProposalsunde_a a_VARIABLEunde_seenProposalsunde_a'
fixes a_VARIABLEunde_receivedVotesunde_a a_VARIABLEunde_receivedVotesunde_a'
fixes a_VARIABLEunde_receivedQCsunde_a a_VARIABLEunde_receivedQCsunde_a'
fixes a_VARIABLEunde_receivedTimeoutVotesunde_a a_VARIABLEunde_receivedTimeoutVotesunde_a'
fixes a_VARIABLEunde_receivedTCsunde_a a_VARIABLEunde_receivedTCsunde_a'
fixes a_VARIABLEunde_proposalIntentsunde_a a_VARIABLEunde_proposalIntentsunde_a'
fixes a_VARIABLEunde_prepareIntentsunde_a a_VARIABLEunde_prepareIntentsunde_a'
fixes a_VARIABLEunde_commitIntentsunde_a a_VARIABLEunde_commitIntentsunde_a'
fixes a_VARIABLEunde_timeoutIntentsunde_a a_VARIABLEunde_timeoutIntentsunde_a'
fixes a_VARIABLEunde_prepareQCsunde_a a_VARIABLEunde_prepareQCsunde_a'
fixes a_VARIABLEunde_commitQCsunde_a a_VARIABLEunde_commitQCsunde_a'
fixes a_VARIABLEunde_formedTCsunde_a a_VARIABLEunde_formedTCsunde_a'
fixes a_VARIABLEunde_installedTCsunde_a a_VARIABLEunde_installedTCsunde_a'
fixes a_VARIABLEunde_lockRankunde_a a_VARIABLEunde_lockRankunde_a'
fixes a_VARIABLEunde_lockSubjectunde_a a_VARIABLEunde_lockSubjectunde_a'
fixes a_VARIABLEunde_highestRankunde_a a_VARIABLEunde_highestRankunde_a'
fixes a_VARIABLEunde_highestSubjectunde_a a_VARIABLEunde_highestSubjectunde_a'
fixes a_VARIABLEunde_pendingProposalunde_a a_VARIABLEunde_pendingProposalunde_a'
fixes a_VARIABLEunde_pendingPrepareunde_a a_VARIABLEunde_pendingPrepareunde_a'
fixes a_VARIABLEunde_pendingObservePrepareunde_a a_VARIABLEunde_pendingObservePrepareunde_a'
fixes a_VARIABLEunde_pendingLockCommitunde_a a_VARIABLEunde_pendingLockCommitunde_a'
fixes a_VARIABLEunde_pendingTimeoutunde_a a_VARIABLEunde_pendingTimeoutunde_a'
fixes a_VARIABLEunde_pendingInstallTCunde_a a_VARIABLEunde_pendingInstallTCunde_a'
fixes a_VARIABLEunde_pendingDecisionunde_a a_VARIABLEunde_pendingDecisionunde_a'
fixes a_VARIABLEunde_signProposalsunde_a a_VARIABLEunde_signProposalsunde_a'
fixes a_VARIABLEunde_signVotesunde_a a_VARIABLEunde_signVotesunde_a'
fixes a_VARIABLEunde_signTimeoutsunde_a a_VARIABLEunde_signTimeoutsunde_a'
fixes a_VARIABLEunde_proposalNetworkunde_a a_VARIABLEunde_proposalNetworkunde_a'
fixes a_VARIABLEunde_voteNetworkunde_a a_VARIABLEunde_voteNetworkunde_a'
fixes a_VARIABLEunde_qcNetworkunde_a a_VARIABLEunde_qcNetworkunde_a'
fixes a_VARIABLEunde_timeoutNetworkunde_a a_VARIABLEunde_timeoutNetworkunde_a'
fixes a_VARIABLEunde_tcNetworkunde_a a_VARIABLEunde_tcNetworkunde_a'
fixes a_VARIABLEunde_decisionsunde_a a_VARIABLEunde_decisionsunde_a'
fixes a_VARIABLEunde_appliedunde_a a_VARIABLEunde_appliedunde_a'
(* usable definition STATE_vars_ suppressed *)
(* usable definition STATE_CurrentEpoch_ suppressed *)
(* usable definition STATE_CurrentVoters_ suppressed *)
(* usable definition STATE_BroadcastProposals_ suppressed *)
(* usable definition STATE_BroadcastVotes_ suppressed *)
(* usable definition STATE_BroadcastQCs_ suppressed *)
(* usable definition STATE_BroadcastTimeouts_ suppressed *)
(* usable definition STATE_BroadcastTCs_ suppressed *)
(* usable definition STATE_PendingNodes_ suppressed *)
(* usable definition STATE_AllPendingRequests_ suppressed *)
(* usable definition CONSTANT_RequestNodeSet_ suppressed *)
(* usable definition CONSTANT_RequestsUniqueByNode_ suppressed *)
(* usable definition STATE_SigningNodes_ suppressed *)
(* usable definition STATE_SeenProposalValues_ suppressed *)
(* usable definition STATE_ReceivedQcValues_ suppressed *)
(* usable definition STATE_ReceivedTcValues_ suppressed *)
(* usable definition STATE_DecisionQcValues_ suppressed *)
(* usable definition STATE_NodeIdle_ suppressed *)
(* usable definition STATE_NodeTimedOut_ suppressed *)
(* usable definition STATE_NodeInstalledTC_ suppressed *)
(* usable definition STATE_HighRefValid_ suppressed *)
(* usable definition STATE_QcValid_ suppressed *)
(* usable definition CONSTANT_VoteBacksCertificate_ suppressed *)
(* usable definition CONSTANT_CertificateHonestIntentBacked_ suppressed *)
(* usable definition CONSTANT_TimeoutVoteProtectsCommitSet_ suppressed *)
(* usable definition CONSTANT_TimeoutSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesDisjoint_ suppressed *)
(* usable definition CONSTANT_TimeoutHighsConflictFree_ suppressed *)
(* usable definition CONSTANT_HighestTimeoutVote_ suppressed *)
(* usable definition STATE_TCValid_ suppressed *)
(* usable definition CONSTANT_TcHighRank_ suppressed *)
(* usable definition CONSTANT_TcHighSubject_ suppressed *)
(* usable definition STATE_ProposalJustified_ suppressed *)
(* usable definition STATE_SafeToPrepare_ suppressed *)
(* usable definition STATE_ProposalValidFor_ suppressed *)
(* usable definition STATE_VoteSignersAt_ suppressed *)
(* usable definition STATE_TimeoutVotesAt_ suppressed *)
(* usable definition CONSTANT_ModelConfiguration_ suppressed *)
(* usable definition ACTION_SetGST_ suppressed *)
(* usable definition ACTION_AssembleLocalBody_ suppressed *)
(* usable definition STATE_LocalProposalJustification_ suppressed *)
(* usable definition STATE_LocalProposalFor_ suppressed *)
(* usable definition ACTION_BeginLocalProposal_ suppressed *)
(* usable definition ACTION_PersistProposal_ suppressed *)
(* usable definition ACTION_CompleteProposalSignature_ suppressed *)
(* usable definition ACTION_DeliverProposal_ suppressed *)
(* usable definition ACTION_FetchBody_ suppressed *)
(* usable definition ACTION_StoreBody_ suppressed *)
(* usable definition ACTION_ValidateBody_ suppressed *)
(* usable definition ACTION_RejectBody_ suppressed *)
(* usable definition STATE_PrepareVoteFor_ suppressed *)
(* usable definition STATE_PrepareRequestFor_ suppressed *)
(* usable definition ACTION_BeginPrepare_ suppressed *)
(* usable definition ACTION_PersistPrepare_ suppressed *)
(* usable definition ACTION_CompleteVoteSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastVote_ suppressed *)
(* usable definition ACTION_DeliverVote_ suppressed *)
(* usable definition ACTION_FormPrepareQC_ suppressed *)
(* usable definition ACTION_DeliverQC_ suppressed *)
(* usable definition ACTION_BeginObservePrepare_ suppressed *)
(* usable definition ACTION_PersistObservePrepare_ suppressed *)
(* usable definition ACTION_BeginLockCommit_ suppressed *)
(* usable definition ACTION_PersistLockCommit_ suppressed *)
(* usable definition ACTION_FormCommitQC_ suppressed *)
(* usable definition ACTION_BeginDecision_ suppressed *)
(* usable definition ACTION_PersistDecision_ suppressed *)
(* usable definition STATE_LocalTimeoutVoteFor_ suppressed *)
(* usable definition STATE_TimeoutRequestFor_ suppressed *)
(* usable definition ACTION_BeginTimeout_ suppressed *)
(* usable definition ACTION_PersistTimeout_ suppressed *)
(* usable definition ACTION_CompleteTimeoutSignature_ suppressed *)
(* usable definition ACTION_ByzantineBroadcastTimeout_ suppressed *)
(* usable definition ACTION_DeliverTimeout_ suppressed *)
(* usable definition ACTION_FormTC_ suppressed *)
(* usable definition ACTION_DeliverTC_ suppressed *)
(* usable definition ACTION_BeginInstallTC_ suppressed *)
(* usable definition ACTION_PersistInstallTC_ suppressed *)
(* usable definition ACTION_FetchCertifiedBody_ suppressed *)
(* usable definition ACTION_ApplyDecision_ suppressed *)
(* usable definition ACTION_Crash_ suppressed *)
(* usable definition ACTION_Restart_ suppressed *)
(* usable definition ACTION_ResumeProposal_ suppressed *)
(* usable definition ACTION_ResumeVote_ suppressed *)
(* usable definition ACTION_ResumeTimeout_ suppressed *)
(* usable definition ACTION_DropProposal_ suppressed *)
(* usable definition ACTION_Next_ suppressed *)
(* usable definition ACTION_ReliableBeginTimeout_ suppressed *)
(* usable definition ACTION_ReliableNext_ suppressed *)
(* usable definition STATE_TypeInvariant_ suppressed *)
(* usable definition STATE_OnePendingPersistencePerNode_ suppressed *)
(* usable definition STATE_PrepareSigningRequiresIntent_ suppressed *)
(* usable definition STATE_CommitSigningRequiresIntent_ suppressed *)
(* usable definition STATE_TimeoutSigningRequiresIntent_ suppressed *)
(* usable definition STATE_ProposalSigningRequiresIntent_ suppressed *)
(* usable definition STATE_HonestPrepareUniqueness_ suppressed *)
(* usable definition STATE_HonestCommitUniqueness_ suppressed *)
(* usable definition STATE_HonestTimeoutUniqueness_ suppressed *)
(* usable definition STATE_LockBelowHighest_ suppressed *)
(* usable definition STATE_DecisionAgreement_ suppressed *)
(* usable definition STATE_OldViewCommitQCAccepted_ suppressed *)
(* usable definition STATE_AppliedRequiresDecision_ suppressed *)
(* usable definition STATE_Safety_ suppressed *)
(* usable definition TEMPORAL_CoreSpec_ suppressed *)
(* usable definition STATE_QuorumCheckNext_ suppressed *)
(* usable definition STATE_GenesisDecisionExists_ suppressed *)
(* usable definition TEMPORAL_PostGstEventuallyGenesisDecision_ suppressed *)
(* usable definition STATE_DurableProjection_ suppressed *)
(* usable definition ACTION_DurableProjectionPrime_ suppressed *)
(* usable definition ACTION_CrashPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_RestartPreservesDurableProjection_ suppressed *)
(* usable definition ACTION_PendingWritesAreUnacknowledged_ suppressed *)
(* usable definition ACTION_StaleGenerationRejected_ suppressed *)
(* usable definition CONSTANT_Frame_ suppressed *)
(* usable definition CONSTANT_ContiguousCompletePrefix_ suppressed *)
(* usable definition CONSTANT_AcknowledgedFrames_ suppressed *)
(* usable definition CONSTANT_IncompleteFinalFrameUnacknowledged_ suppressed *)
(* usable definition CONSTANT_HashChainWellFormed_ suppressed *)
(* usable definition STATE_CommonAppliedSubject_ suppressed *)
(* usable definition ACTION_AdvanceContext_ suppressed *)
(* usable definition ACTION_NextV2_ suppressed *)
(* usable definition ACTION_ReliableNextV2_ suppressed *)
(* usable definition TEMPORAL_Spec_ suppressed *)
(* usable definition TEMPORAL_LivenessSpec_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsParent_ suppressed *)
(* usable definition CONSTANT_ContextIdentityBindsFrozenEpoch_ suppressed *)
(* usable definition STATE_ContextParentWasApplied_ suppressed *)
(* usable definition STATE_EpochBoundarySafety_ suppressed *)
(* usable definition CONSTANT_SameVoteSlot_ suppressed *)
(* usable definition CONSTANT_HonestVoteUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendVote_ suppressed *)
(* usable definition CONSTANT_SameTimeoutSlot_ suppressed *)
(* usable definition CONSTANT_SameTimeoutContent_ suppressed *)
(* usable definition CONSTANT_HonestTimeoutUnique_ suppressed *)
(* usable definition CONSTANT_CanAppendTimeout_ suppressed *)
(* usable definition CONSTANT_CertificateBackedBy_ suppressed *)
(* usable definition CONSTANT_SameCertificateSlot_ suppressed *)
(* usable definition CONSTANT_HonestIntentSound_ suppressed *)
(* usable definition CONSTANT_CertificateValidityAndAvailability_ suppressed *)
(* usable definition CONSTANT_LockValue_ suppressed *)
(* usable definition CONSTANT_CommitLockAllowed_ suppressed *)
(* usable definition CONSTANT_CommitLockResult_ suppressed *)
(* usable definition CONSTANT_InstallHighLock_ suppressed *)
(* usable definition CONSTANT_LockMonotone_ suppressed *)
(* usable definition CONSTANT_CommitSignerSet_ suppressed *)
(* usable definition CONSTANT_TimeoutIntentProtectsCommits_ suppressed *)
(* usable definition CONSTANT_TCMaximumProtectsReports_ suppressed *)
(* usable definition CONSTANT_TimeoutVotesBindCertificate_ suppressed *)
(* usable definition CONSTANT_TimeoutRanksTyped_ suppressed *)
(* usable definition CONSTANT_TimeoutProtectionKernel_ suppressed *)
(* usable definition CONSTANT_TCProtectsViewSubject_ suppressed *)
(* usable definition STATE_VoteIntentFor_ suppressed *)
(* usable definition STATE_PrepareCarriesHigherSafeQc_ suppressed *)
(* usable definition STATE_PrepareLineageSound_ suppressed *)
(* usable definition STATE_LocksCoverOwnCommits_ suppressed *)
(* usable definition STATE_CurrentIntentViewsBound_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_PendingCertificateWritesAuthorized_ suppressed *)
(* usable definition STATE_HonestVoteTransportBacked_ suppressed *)
(* usable definition STATE_QcTransportBacked_ suppressed *)
(* usable definition STATE_HonestTimeoutTransportBacked_ suppressed *)
(* usable definition STATE_TcTransportBacked_ suppressed *)
(* usable definition CONSTANT_HistoricalQcValid_ suppressed *)
(* usable definition STATE_CertificatesBackedByIntents_ suppressed *)
(* usable definition STATE_HonestDurableIntentsSound_ suppressed *)
(* usable definition STATE_FormedTimeoutCertificatesSound_ suppressed *)
(* usable definition STATE_DurableTimeoutsProtectCommits_ suppressed *)
(* usable definition STATE_HighestAndLockAreCertified_ suppressed *)
(* usable definition STATE_ReducerProvenanceInvariant_ suppressed *)
(* usable definition STATE_LineageInvariant_ suppressed *)
(* usable definition STATE_StrongInductiveInvariant_ suppressed *)
(* usable definition STATE_ProofRelevantVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutDurableVars_ suppressed *)
(* usable definition STATE_ProofRelevantWithoutPendingProposalVars_ suppressed *)
(* usable definition CONSTANT_MapThenFoldSet_ suppressed *)
(* usable definition CONSTANT_Restrict_ suppressed *)
(* usable definition CONSTANT_RestrictDomain_ suppressed *)
(* usable definition CONSTANT_RestrictValues_ suppressed *)
(* usable definition CONSTANT_IsRestriction_ suppressed *)
(* usable definition CONSTANT_Range_ suppressed *)
(* usable definition CONSTANT_Pointwise_ suppressed *)
(* usable definition CONSTANT_Inverse_ suppressed *)
(* usable definition CONSTANT_AntiFunction_ suppressed *)
(* usable definition CONSTANT_IsInjective_ suppressed *)
(* usable definition CONSTANT_Injection_ suppressed *)
(* usable definition CONSTANT_Surjection_ suppressed *)
(* usable definition CONSTANT_Bijection_ suppressed *)
(* usable definition CONSTANT_ExistsInjection_ suppressed *)
(* usable definition CONSTANT_ExistsSurjection_ suppressed *)
(* usable definition CONSTANT_ExistsBijection_ suppressed *)
(* usable definition CONSTANT_FoldFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_FoldFunction_ suppressed *)
(* usable definition CONSTANT_SumFunctionOnSet_ suppressed *)
(* usable definition CONSTANT_SumFunction_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_NatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefHypothesis_ suppressed *)
(* usable definition CONSTANT_FiniteNatInductiveDefConclusion_ suppressed *)
(* usable definition CONSTANT_IsTransitivelyClosedOn_ suppressed *)
(* usable definition CONSTANT_IsWellFoundedOn_ suppressed *)
(* usable definition CONSTANT_SetLessThan_ suppressed *)
(* usable definition CONSTANT_WFDefOn_ suppressed *)
(* usable definition CONSTANT_OpDefinesFcn_ suppressed *)
(* usable definition CONSTANT_WFInductiveDefines_ suppressed *)
(* usable definition CONSTANT_WFInductiveUnique_ suppressed *)
(* usable definition CONSTANT_TransitiveClosureOn_ suppressed *)
(* usable definition CONSTANT_OpToRel_ suppressed *)
(* usable definition CONSTANT_PreImage_ suppressed *)
(* usable definition CONSTANT_LexPairOrdering_ suppressed *)
(* usable definition CONSTANT_LexProductOrdering_ suppressed *)
(* usable definition CONSTANT_FiniteSubsetsOf_ suppressed *)
(* usable definition CONSTANT_StrictSubsetOrdering_ suppressed *)
(* usable definition CONSTANT_EnabledWrapper_ suppressed *)
(* usable definition CONSTANT_CdotWrapper_ suppressed *)
(* usable definition STATE_InductiveInvariant_ suppressed *)
(* usable definition STATE_InitialStateObligation_ suppressed *)
assumes v'521: "(a_CONSTANTunde_ModelConfigurationunde_a)"
assumes v'522: "(((a_VARIABLEunde_heightunde_a) = ((0))))"
assumes v'523: "(((a_VARIABLEunde_contextunde_a) = ((a_CONSTANTunde_ContextRecordunde_a (((0)), (a_CONSTANTunde_NoSubjectunde_a))))))"
assumes v'524: "(((a_VARIABLEunde_contextHistoryunde_a) = ({(a_VARIABLEunde_contextunde_a)})))"
assumes v'525: "(((a_VARIABLEunde_nodeViewunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'526: "(((a_VARIABLEunde_generationunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> ((0))])))"
assumes v'527: "(((a_VARIABLEunde_upunde_a) = (a_CONSTANTunde_ValidatorIdsunde_a)))"
assumes v'528: "(((a_VARIABLEunde_gstunde_a) = (FALSE)))"
assumes v'529: "(((a_VARIABLEunde_availableBodiesunde_a) = ({})))"
assumes v'530: "(((a_VARIABLEunde_durableBodiesunde_a) = ({})))"
assumes v'531: "(((a_VARIABLEunde_validatedBodiesunde_a) = ({})))"
assumes v'532: "(((a_VARIABLEunde_invalidBodiesunde_a) = ({})))"
assumes v'533: "(((a_VARIABLEunde_seenProposalsunde_a) = ({})))"
assumes v'534: "(((a_VARIABLEunde_receivedVotesunde_a) = ({})))"
assumes v'535: "(((a_VARIABLEunde_receivedQCsunde_a) = ({})))"
assumes v'536: "(((a_VARIABLEunde_receivedTimeoutVotesunde_a) = ({})))"
assumes v'537: "(((a_VARIABLEunde_receivedTCsunde_a) = ({})))"
assumes v'538: "(((a_VARIABLEunde_proposalIntentsunde_a) = ({})))"
assumes v'539: "(((a_VARIABLEunde_prepareIntentsunde_a) = ({})))"
assumes v'540: "(((a_VARIABLEunde_commitIntentsunde_a) = ({})))"
assumes v'541: "(((a_VARIABLEunde_timeoutIntentsunde_a) = ({})))"
assumes v'542: "(((a_VARIABLEunde_prepareQCsunde_a) = ({})))"
assumes v'543: "(((a_VARIABLEunde_commitQCsunde_a) = ({})))"
assumes v'544: "(((a_VARIABLEunde_formedTCsunde_a) = ({})))"
assumes v'545: "(((a_VARIABLEunde_installedTCsunde_a) = ({})))"
assumes v'546: "(((a_VARIABLEunde_lockRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'547: "(((a_VARIABLEunde_lockSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'548: "(((a_VARIABLEunde_highestRankunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoRankunde_a)])))"
assumes v'549: "(((a_VARIABLEunde_highestSubjectunde_a) = ([ a_CONSTANTunde_nodeunde_a \<in> (a_CONSTANTunde_ValidatorIdsunde_a)  \<mapsto> (a_CONSTANTunde_NoSubjectunde_a)])))"
assumes v'550: "(((a_VARIABLEunde_pendingProposalunde_a) = ({})))"
assumes v'551: "(((a_VARIABLEunde_pendingPrepareunde_a) = ({})))"
assumes v'552: "(((a_VARIABLEunde_pendingObservePrepareunde_a) = ({})))"
assumes v'553: "(((a_VARIABLEunde_pendingLockCommitunde_a) = ({})))"
assumes v'554: "(((a_VARIABLEunde_pendingTimeoutunde_a) = ({})))"
assumes v'555: "(((a_VARIABLEunde_pendingInstallTCunde_a) = ({})))"
assumes v'556: "(((a_VARIABLEunde_pendingDecisionunde_a) = ({})))"
assumes v'557: "(((a_VARIABLEunde_signProposalsunde_a) = ({})))"
assumes v'558: "(((a_VARIABLEunde_signVotesunde_a) = ({})))"
assumes v'559: "(((a_VARIABLEunde_signTimeoutsunde_a) = ({})))"
assumes v'560: "(((a_VARIABLEunde_proposalNetworkunde_a) = ({})))"
assumes v'561: "(((a_VARIABLEunde_voteNetworkunde_a) = ({})))"
assumes v'562: "(((a_VARIABLEunde_qcNetworkunde_a) = ({})))"
assumes v'563: "(((a_VARIABLEunde_timeoutNetworkunde_a) = ({})))"
assumes v'564: "(((a_VARIABLEunde_tcNetworkunde_a) = ({})))"
assumes v'565: "(((a_VARIABLEunde_decisionsunde_a) = ({})))"
assumes v'566: "(((a_VARIABLEunde_appliedunde_a) = ({})))"
shows "(\<forall> a_CONSTANTunde_qcunde_a \<in> (((a_VARIABLEunde_prepareQCsunde_a) \<union> (a_VARIABLEunde_commitQCsunde_a))) : (((((fapply ((a_CONSTANTunde_qcunde_a), (''context''))) \<noteq> (a_VARIABLEunde_contextunde_a))) \<Rightarrow> ((~ ((a_STATEunde_QcValidunde_a ((a_CONSTANTunde_qcunde_a)))))))))"(is "PROP ?ob'38")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 38"; \<close>
show "PROP ?ob'38"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 38"; \<close> qed
end
