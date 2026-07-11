(* automatically generated -- do not edit manually *)
theory SumeragiV2AgreementLemmas imports Constant Zenon begin
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

lemma ob'35:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
fixes a_CONSTANTunde_committedunde_a
assumes a_CONSTANTunde_committedunde_a_in : "(a_CONSTANTunde_committedunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
fixes a_CONSTANTunde_laterunde_a
assumes a_CONSTANTunde_laterunde_a_in : "(a_CONSTANTunde_laterunde_a \<in> (a_VARIABLEunde_prepareQCsunde_a))"
fixes a_CONSTANTunde_signerunde_a
assumes a_CONSTANTunde_signerunde_a_in : "(a_CONSTANTunde_signerunde_a \<in> (((((fapply ((a_CONSTANTunde_committedunde_a), (''signers''))) \<inter> (fapply ((a_CONSTANTunde_laterunde_a), (''signers''))))) \<inter> (a_CONSTANTunde_Honestunde_a))))"
fixes a_CONSTANTunde_commitVoteunde_a
assumes a_CONSTANTunde_commitVoteunde_a_in : "(a_CONSTANTunde_commitVoteunde_a \<in> (a_VARIABLEunde_commitIntentsunde_a))"
fixes a_CONSTANTunde_prepareVoteunde_a
assumes a_CONSTANTunde_prepareVoteunde_a_in : "(a_CONSTANTunde_prepareVoteunde_a \<in> (a_VARIABLEunde_prepareIntentsunde_a))"
shows "(((a_CONSTANTunde_signerunde_a) \<in> (a_CONSTANTunde_Honestunde_a)))"(is "PROP ?ob'35")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 35"; \<close>
show "PROP ?ob'35"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 35"; \<close> qed
lemma ob'52:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
fixes a_CONSTANTunde_committedunde_a
assumes a_CONSTANTunde_committedunde_a_in : "(a_CONSTANTunde_committedunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
fixes a_CONSTANTunde_laterunde_a
assumes a_CONSTANTunde_laterunde_a_in : "(a_CONSTANTunde_laterunde_a \<in> (a_VARIABLEunde_prepareQCsunde_a))"
fixes a_CONSTANTunde_signerunde_a
assumes a_CONSTANTunde_signerunde_a_in : "(a_CONSTANTunde_signerunde_a \<in> (((((fapply ((a_CONSTANTunde_committedunde_a), (''signers''))) \<inter> (fapply ((a_CONSTANTunde_laterunde_a), (''signers''))))) \<inter> (a_CONSTANTunde_Honestunde_a))))"
fixes a_CONSTANTunde_commitVoteunde_a
assumes a_CONSTANTunde_commitVoteunde_a_in : "(a_CONSTANTunde_commitVoteunde_a \<in> (a_VARIABLEunde_commitIntentsunde_a))"
fixes a_CONSTANTunde_prepareVoteunde_a
assumes a_CONSTANTunde_prepareVoteunde_a_in : "(a_CONSTANTunde_prepareVoteunde_a \<in> (a_VARIABLEunde_prepareIntentsunde_a))"
fixes a_CONSTANTunde_carriedunde_a
assumes a_CONSTANTunde_carriedunde_a_in : "(a_CONSTANTunde_carriedunde_a \<in> (a_VARIABLEunde_prepareQCsunde_a))"
assumes v'504: "((a_CONSTANTunde_QuorumConfigurationunde_a) & (a_STATEunde_CertificatesBackedByIntentsunde_a) & (a_STATEunde_IntentPhasesCorrectunde_a) & (a_STATEunde_PrepareLineageSoundunde_a) & ((less ((fapply ((a_CONSTANTunde_committedunde_a), (''view''))), (fapply ((a_CONSTANTunde_laterunde_a), (''view'')))))) & (((fapply ((a_CONSTANTunde_committedunde_a), (''subject''))) \<noteq> (fapply ((a_CONSTANTunde_laterunde_a), (''subject''))))))"
assumes v'505: "(((fapply ((a_CONSTANTunde_committedunde_a), (''context''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''context'')))))"
assumes v'506: "(((fapply ((a_CONSTANTunde_commitVoteunde_a), (''context''))) = (fapply ((a_CONSTANTunde_committedunde_a), (''context'')))))"
assumes v'507: "(((fapply ((a_CONSTANTunde_commitVoteunde_a), (''view''))) = (fapply ((a_CONSTANTunde_committedunde_a), (''view'')))))"
assumes v'508: "(((fapply ((a_CONSTANTunde_commitVoteunde_a), (''phase''))) = (fapply ((a_CONSTANTunde_committedunde_a), (''phase'')))))"
assumes v'509: "(((fapply ((a_CONSTANTunde_commitVoteunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_committedunde_a), (''subject'')))))"
assumes v'510: "(((fapply ((a_CONSTANTunde_commitVoteunde_a), (''signer''))) = (a_CONSTANTunde_signerunde_a)))"
assumes v'511: "(((fapply ((a_CONSTANTunde_prepareVoteunde_a), (''context''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''context'')))))"
assumes v'512: "(((fapply ((a_CONSTANTunde_prepareVoteunde_a), (''view''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''view'')))))"
assumes v'513: "(((fapply ((a_CONSTANTunde_prepareVoteunde_a), (''phase''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''phase'')))))"
assumes v'514: "(((fapply ((a_CONSTANTunde_prepareVoteunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''subject'')))))"
assumes v'515: "(((fapply ((a_CONSTANTunde_prepareVoteunde_a), (''signer''))) = (a_CONSTANTunde_signerunde_a)))"
assumes v'516: "(((less ((fapply ((a_CONSTANTunde_commitVoteunde_a), (''view''))), (fapply ((a_CONSTANTunde_carriedunde_a), (''view'')))))) & ((less ((fapply ((a_CONSTANTunde_carriedunde_a), (''view''))), (fapply ((a_CONSTANTunde_prepareVoteunde_a), (''view'')))))))"
assumes v'517: "(((fapply ((a_CONSTANTunde_carriedunde_a), (''context''))) = (fapply ((a_CONSTANTunde_prepareVoteunde_a), (''context'')))))"
assumes v'518: "(((fapply ((a_CONSTANTunde_carriedunde_a), (''phase''))) = (''Prepare'')))"
assumes v'519: "(((fapply ((a_CONSTANTunde_carriedunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_prepareVoteunde_a), (''subject'')))))"
shows "(((fapply ((a_CONSTANTunde_carriedunde_a), (''context''))) = (fapply ((a_CONSTANTunde_committedunde_a), (''context'')))))"(is "PROP ?ob'52")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 52"; \<close>
show "PROP ?ob'52"

(* BEGIN ZENON INPUT
;; file=.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_b09f11.znn; PATH='/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/bin:/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/Isabelle/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/System/Cryptexes/App/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/local/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/appleinternal/bin:/pkg/env/global/bin://Applications/Topaz Gigapixel.app/Contents/Resources/bin://Applications/Topaz Photo.app/Contents/Resources/bin:/Library/Apple/usr/bin:/Library/TeX/texbin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0KAG3FU:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override:/Users/mtakemiya/.antigravity-ide/antigravity-ide/bin:/Users/mtakemiya/.yarn/bin:/Users/mtakemiya/.config/yarn/global/node_modules/.bin:/Users/mtakemiya/.antigravity/antigravity/bin:/opt/homebrew/opt/openjdk@21/bin:/Library/Java/JavaVirtualMachines/jdk-25.jdk/Contents/Home/bin:/opt/homebrew/opt/ruby/bin:/usr/local/opt/python/libexec/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0RclyEw:/opt/homebrew/Caskroom/codex/0.144.1/codex-path:/Users/mtakemiya/.cargo/bin:/Applications/iTerm.app/Contents/Resources/utilities:/Users/mtakemiya/.sp1/bin:/Users/mtakemiya/.rvm/bin:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/fallback:/Applications/Codex.app/Contents/Resources'; zenon -p0 -x tla -oisar -max-time 1d "$file" >.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_b09f11.znn.out
;; obligation #52
$hyp "a_CONSTANTunde_committedunde_a_in" (TLA.in a_CONSTANTunde_committedunde_a a_VARIABLEunde_commitQCsunde_a)
$hyp "a_CONSTANTunde_laterunde_a_in" (TLA.in a_CONSTANTunde_laterunde_a a_VARIABLEunde_prepareQCsunde_a)
$hyp "a_CONSTANTunde_signerunde_a_in" (TLA.in a_CONSTANTunde_signerunde_a (TLA.cap (TLA.cap (TLA.fapply a_CONSTANTunde_committedunde_a "signers")
(TLA.fapply a_CONSTANTunde_laterunde_a "signers"))
a_CONSTANTunde_Honestunde_a))
$hyp "a_CONSTANTunde_commitVoteunde_a_in" (TLA.in a_CONSTANTunde_commitVoteunde_a a_VARIABLEunde_commitIntentsunde_a)
$hyp "a_CONSTANTunde_prepareVoteunde_a_in" (TLA.in a_CONSTANTunde_prepareVoteunde_a a_VARIABLEunde_prepareIntentsunde_a)
$hyp "a_CONSTANTunde_carriedunde_a_in" (TLA.in a_CONSTANTunde_carriedunde_a a_VARIABLEunde_prepareQCsunde_a)
$hyp "v'504" (/\ a_CONSTANTunde_QuorumConfigurationunde_a
a_STATEunde_CertificatesBackedByIntentsunde_a
a_STATEunde_IntentPhasesCorrectunde_a a_STATEunde_PrepareLineageSoundunde_a
(arith.lt (TLA.fapply a_CONSTANTunde_committedunde_a "view")
(TLA.fapply a_CONSTANTunde_laterunde_a "view"))
(-. (= (TLA.fapply a_CONSTANTunde_committedunde_a "subject")
(TLA.fapply a_CONSTANTunde_laterunde_a "subject"))))
$hyp "v'505" (= (TLA.fapply a_CONSTANTunde_committedunde_a "context")
(TLA.fapply a_CONSTANTunde_laterunde_a "context"))
$hyp "v'506" (= (TLA.fapply a_CONSTANTunde_commitVoteunde_a "context")
(TLA.fapply a_CONSTANTunde_committedunde_a "context"))
$hyp "v'507" (= (TLA.fapply a_CONSTANTunde_commitVoteunde_a "view")
(TLA.fapply a_CONSTANTunde_committedunde_a "view"))
$hyp "v'508" (= (TLA.fapply a_CONSTANTunde_commitVoteunde_a "phase")
(TLA.fapply a_CONSTANTunde_committedunde_a "phase"))
$hyp "v'509" (= (TLA.fapply a_CONSTANTunde_commitVoteunde_a "subject")
(TLA.fapply a_CONSTANTunde_committedunde_a "subject"))
$hyp "v'510" (= (TLA.fapply a_CONSTANTunde_commitVoteunde_a "signer")
a_CONSTANTunde_signerunde_a)
$hyp "v'511" (= (TLA.fapply a_CONSTANTunde_prepareVoteunde_a "context")
(TLA.fapply a_CONSTANTunde_laterunde_a "context"))
$hyp "v'512" (= (TLA.fapply a_CONSTANTunde_prepareVoteunde_a "view")
(TLA.fapply a_CONSTANTunde_laterunde_a "view"))
$hyp "v'513" (= (TLA.fapply a_CONSTANTunde_prepareVoteunde_a "phase")
(TLA.fapply a_CONSTANTunde_laterunde_a "phase"))
$hyp "v'514" (= (TLA.fapply a_CONSTANTunde_prepareVoteunde_a "subject")
(TLA.fapply a_CONSTANTunde_laterunde_a "subject"))
$hyp "v'515" (= (TLA.fapply a_CONSTANTunde_prepareVoteunde_a "signer")
a_CONSTANTunde_signerunde_a)
$hyp "v'516" (/\ (arith.lt (TLA.fapply a_CONSTANTunde_commitVoteunde_a "view")
(TLA.fapply a_CONSTANTunde_carriedunde_a "view"))
(arith.lt (TLA.fapply a_CONSTANTunde_carriedunde_a "view")
(TLA.fapply a_CONSTANTunde_prepareVoteunde_a "view")))
$hyp "v'517" (= (TLA.fapply a_CONSTANTunde_carriedunde_a "context")
(TLA.fapply a_CONSTANTunde_prepareVoteunde_a "context"))
$hyp "v'518" (= (TLA.fapply a_CONSTANTunde_carriedunde_a "phase")
"Prepare")
$hyp "v'519" (= (TLA.fapply a_CONSTANTunde_carriedunde_a "subject")
(TLA.fapply a_CONSTANTunde_prepareVoteunde_a "subject"))
$goal (= (TLA.fapply a_CONSTANTunde_carriedunde_a "context")
(TLA.fapply a_CONSTANTunde_committedunde_a "context"))
END ZENON  INPUT *)
(* PROOF-FOUND *)
(* BEGIN-PROOF *)
proof (rule zenon_nnpp)
 have z_Hn:"((a_CONSTANTunde_prepareVoteunde_a[''context''])=(a_CONSTANTunde_laterunde_a[''context'']))" (is "?z_hx=?z_hba")
 using v'511 by blast
 have z_Ht:"((a_CONSTANTunde_carriedunde_a[''context''])=?z_hx)" (is "?z_hbc=_")
 using v'517 by blast
 have z_Hh:"((a_CONSTANTunde_committedunde_a[''context''])=?z_hba)" (is "?z_hbe=_")
 using v'505 by blast
 assume z_Hw:"(?z_hbc~=?z_hbe)"
 show FALSE
 proof (rule notE [OF z_Hw])
  have z_Hbg: "(?z_hx=?z_hbc)"
  by (rule sym [OF z_Ht])
  have z_Hbh: "(?z_hba=?z_hbe)"
  by (rule sym [OF z_Hh])
  have z_Hbi: "(?z_hbc=?z_hba)"
  by (rule subst [where P="(\<lambda>zenon_Vp. (zenon_Vp=?z_hba))", OF z_Hbg], fact z_Hn)
  have z_Hbm: "(?z_hbc=?z_hbe)"
  by (rule subst [where P="(\<lambda>zenon_Vi. (?z_hbc=zenon_Vi))", OF z_Hbh], fact z_Hbi)
  thus "(?z_hbc=?z_hbe)" .
 qed
qed
(* END-PROOF *)
ML_command \<open> writeln "*** TLAPS EXIT 52"; \<close> qed
lemma ob'81:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
(* usable definition STATE_ConflictingPrepareAt_ suppressed *)
fixes a_CONSTANTunde_committedunde_a
assumes a_CONSTANTunde_committedunde_a_in : "(a_CONSTANTunde_committedunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
fixes a_CONSTANTunde_laterunde_a
assumes a_CONSTANTunde_laterunde_a_in : "(a_CONSTANTunde_laterunde_a \<in> (a_VARIABLEunde_prepareQCsunde_a))"
assumes v'474: "(((((fapply ((a_CONSTANTunde_committedunde_a), (''subject''))) \<noteq> (fapply ((a_CONSTANTunde_laterunde_a), (''subject''))))) \<Longrightarrow> (FALSE)))"
assumes v'475: "((a_CONSTANTunde_QuorumConfigurationunde_a) & (a_STATEunde_CertificatesBackedByIntentsunde_a) & (a_STATEunde_IntentPhasesCorrectunde_a) & (a_STATEunde_PrepareLineageSoundunde_a) & ((less ((fapply ((a_CONSTANTunde_committedunde_a), (''view''))), (fapply ((a_CONSTANTunde_laterunde_a), (''view'')))))))"
assumes v'476: "(((fapply ((a_CONSTANTunde_committedunde_a), (''context''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''context'')))))"
shows "(((fapply ((a_CONSTANTunde_committedunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_laterunde_a), (''subject'')))))"(is "PROP ?ob'81")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 81"; \<close>
show "PROP ?ob'81"

(* BEGIN ZENON INPUT
;; file=.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_bc6f9b.znn; PATH='/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/bin:/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/Isabelle/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/System/Cryptexes/App/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/local/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/appleinternal/bin:/pkg/env/global/bin://Applications/Topaz Gigapixel.app/Contents/Resources/bin://Applications/Topaz Photo.app/Contents/Resources/bin:/Library/Apple/usr/bin:/Library/TeX/texbin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0KAG3FU:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override:/Users/mtakemiya/.antigravity-ide/antigravity-ide/bin:/Users/mtakemiya/.yarn/bin:/Users/mtakemiya/.config/yarn/global/node_modules/.bin:/Users/mtakemiya/.antigravity/antigravity/bin:/opt/homebrew/opt/openjdk@21/bin:/Library/Java/JavaVirtualMachines/jdk-25.jdk/Contents/Home/bin:/opt/homebrew/opt/ruby/bin:/usr/local/opt/python/libexec/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0RclyEw:/opt/homebrew/Caskroom/codex/0.144.1/codex-path:/Users/mtakemiya/.cargo/bin:/Applications/iTerm.app/Contents/Resources/utilities:/Users/mtakemiya/.sp1/bin:/Users/mtakemiya/.rvm/bin:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/fallback:/Applications/Codex.app/Contents/Resources'; zenon -p0 -x tla -oisar -max-time 1d "$file" >.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_bc6f9b.znn.out
;; obligation #81
$hyp "a_CONSTANTunde_committedunde_a_in" (TLA.in a_CONSTANTunde_committedunde_a a_VARIABLEunde_commitQCsunde_a)
$hyp "a_CONSTANTunde_laterunde_a_in" (TLA.in a_CONSTANTunde_laterunde_a a_VARIABLEunde_prepareQCsunde_a)
$hyp "v'474" (=> (-. (= (TLA.fapply a_CONSTANTunde_committedunde_a "subject")
(TLA.fapply a_CONSTANTunde_laterunde_a "subject"))) F.)
$hyp "v'475" (/\ a_CONSTANTunde_QuorumConfigurationunde_a
a_STATEunde_CertificatesBackedByIntentsunde_a
a_STATEunde_IntentPhasesCorrectunde_a a_STATEunde_PrepareLineageSoundunde_a
(arith.lt (TLA.fapply a_CONSTANTunde_committedunde_a "view")
(TLA.fapply a_CONSTANTunde_laterunde_a "view")))
$hyp "v'476" (= (TLA.fapply a_CONSTANTunde_committedunde_a "context")
(TLA.fapply a_CONSTANTunde_laterunde_a "context"))
$goal (= (TLA.fapply a_CONSTANTunde_committedunde_a "subject")
(TLA.fapply a_CONSTANTunde_laterunde_a "subject"))
END ZENON  INPUT *)
(* PROOF-FOUND *)
(* BEGIN-PROOF *)
proof (rule zenon_nnpp)
 have z_Hc:"(((a_CONSTANTunde_committedunde_a[''subject''])~=(a_CONSTANTunde_laterunde_a[''subject'']))=>FALSE)" (is "?z_hf=>?z_hl")
 using v'474 by blast
 assume z_Hf:"?z_hf" (is "?z_hg~=?z_hj")
 show FALSE
 proof (rule zenon_imply [OF z_Hc])
  assume z_Hm:"(~?z_hf)" (is "~~?z_hn")
  show FALSE
  by (rule notE [OF z_Hm z_Hf])
 next
  assume z_Hl:"?z_hl"
  show FALSE
  by (rule z_Hl)
 qed
qed
(* END-PROOF *)
ML_command \<open> writeln "*** TLAPS EXIT 81"; \<close> qed
lemma ob'150:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
(* usable definition STATE_ConflictingPrepareAt_ suppressed *)
fixes a_CONSTANTunde_leftunde_a
assumes a_CONSTANTunde_leftunde_a_in : "(a_CONSTANTunde_leftunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
fixes a_CONSTANTunde_rightunde_a
assumes a_CONSTANTunde_rightunde_a_in : "(a_CONSTANTunde_rightunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
assumes v'482: "(((((fapply ((a_CONSTANTunde_leftunde_a), (''view''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''view''))))) \<Longrightarrow> (((fapply ((a_CONSTANTunde_leftunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''subject'')))))))"
assumes v'483: "((((less ((fapply ((a_CONSTANTunde_leftunde_a), (''view''))), (fapply ((a_CONSTANTunde_rightunde_a), (''view'')))))) \<Longrightarrow> (((fapply ((a_CONSTANTunde_leftunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''subject'')))))))"
assumes v'484: "((((less ((fapply ((a_CONSTANTunde_rightunde_a), (''view''))), (fapply ((a_CONSTANTunde_leftunde_a), (''view'')))))) \<Longrightarrow> (((fapply ((a_CONSTANTunde_leftunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''subject'')))))))"
assumes v'485: "((((fapply ((a_CONSTANTunde_leftunde_a), (''view''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''view''))))) | ((less ((fapply ((a_CONSTANTunde_leftunde_a), (''view''))), (fapply ((a_CONSTANTunde_rightunde_a), (''view'')))))) | ((less ((fapply ((a_CONSTANTunde_rightunde_a), (''view''))), (fapply ((a_CONSTANTunde_leftunde_a), (''view'')))))))"
shows "(((fapply ((a_CONSTANTunde_leftunde_a), (''subject''))) = (fapply ((a_CONSTANTunde_rightunde_a), (''subject'')))))"(is "PROP ?ob'150")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 150"; \<close>
show "PROP ?ob'150"

(* BEGIN ZENON INPUT
;; file=.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_899c1c.znn; PATH='/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/bin:/private/tmp/tlapm-1.6.0-pre-arm64-darwin/tlapm/lib/tlapm/backends/Isabelle/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/System/Cryptexes/App/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/local/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/appleinternal/bin:/pkg/env/global/bin://Applications/Topaz Gigapixel.app/Contents/Resources/bin://Applications/Topaz Photo.app/Contents/Resources/bin:/Library/Apple/usr/bin:/Library/TeX/texbin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0KAG3FU:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override:/Users/mtakemiya/.antigravity-ide/antigravity-ide/bin:/Users/mtakemiya/.yarn/bin:/Users/mtakemiya/.config/yarn/global/node_modules/.bin:/Users/mtakemiya/.antigravity/antigravity/bin:/opt/homebrew/opt/openjdk@21/bin:/Library/Java/JavaVirtualMachines/jdk-25.jdk/Contents/Home/bin:/opt/homebrew/opt/ruby/bin:/usr/local/opt/python/libexec/bin:/Users/mtakemiya/.local/share/solana/install/active_release/bin:/Users/mtakemiya/.codex/tmp/arg0/codex-arg0RclyEw:/opt/homebrew/Caskroom/codex/0.144.1/codex-path:/Users/mtakemiya/.cargo/bin:/Applications/iTerm.app/Contents/Resources/utilities:/Users/mtakemiya/.sp1/bin:/Users/mtakemiya/.rvm/bin:/Users/mtakemiya/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/fallback:/Applications/Codex.app/Contents/Resources'; zenon -p0 -x tla -oisar -max-time 1d "$file" >.tlacache/SumeragiV2AgreementLemmas.tlaps/tlapm_899c1c.znn.out
;; obligation #150
$hyp "a_CONSTANTunde_leftunde_a_in" (TLA.in a_CONSTANTunde_leftunde_a a_VARIABLEunde_commitQCsunde_a)
$hyp "a_CONSTANTunde_rightunde_a_in" (TLA.in a_CONSTANTunde_rightunde_a a_VARIABLEunde_commitQCsunde_a)
$hyp "v'482" (=> (= (TLA.fapply a_CONSTANTunde_leftunde_a "view")
(TLA.fapply a_CONSTANTunde_rightunde_a "view")) (= (TLA.fapply a_CONSTANTunde_leftunde_a "subject")
(TLA.fapply a_CONSTANTunde_rightunde_a "subject")))
$hyp "v'483" (=> (arith.lt (TLA.fapply a_CONSTANTunde_leftunde_a "view")
(TLA.fapply a_CONSTANTunde_rightunde_a "view")) (= (TLA.fapply a_CONSTANTunde_leftunde_a "subject")
(TLA.fapply a_CONSTANTunde_rightunde_a "subject")))
$hyp "v'484" (=> (arith.lt (TLA.fapply a_CONSTANTunde_rightunde_a "view")
(TLA.fapply a_CONSTANTunde_leftunde_a "view")) (= (TLA.fapply a_CONSTANTunde_leftunde_a "subject")
(TLA.fapply a_CONSTANTunde_rightunde_a "subject")))
$hyp "v'485" (\/ (= (TLA.fapply a_CONSTANTunde_leftunde_a "view")
(TLA.fapply a_CONSTANTunde_rightunde_a "view"))
(arith.lt (TLA.fapply a_CONSTANTunde_leftunde_a "view")
(TLA.fapply a_CONSTANTunde_rightunde_a "view"))
(arith.lt (TLA.fapply a_CONSTANTunde_rightunde_a "view")
(TLA.fapply a_CONSTANTunde_leftunde_a "view")))
$goal (= (TLA.fapply a_CONSTANTunde_leftunde_a "subject")
(TLA.fapply a_CONSTANTunde_rightunde_a "subject"))
END ZENON  INPUT *)
(* PROOF-FOUND *)
(* BEGIN-PROOF *)
proof (rule zenon_nnpp)
 have z_Hc:"(((a_CONSTANTunde_leftunde_a[''view''])=(a_CONSTANTunde_rightunde_a[''view'']))=>((a_CONSTANTunde_leftunde_a[''subject''])=(a_CONSTANTunde_rightunde_a[''subject''])))" (is "?z_hh=>?z_hn")
 using v'482 by blast
 have z_He:"(((a_CONSTANTunde_rightunde_a[''view'']) < (a_CONSTANTunde_leftunde_a[''view'']))=>?z_hn)" (is "?z_hr=>_")
 using v'484 by blast
 have z_Hf:"(?z_hh|(((a_CONSTANTunde_leftunde_a[''view'']) < (a_CONSTANTunde_rightunde_a[''view'']))|?z_hr))" (is "_|?z_hs")
 using v'485 by blast
 have z_Hd:"(((a_CONSTANTunde_leftunde_a[''view'']) < (a_CONSTANTunde_rightunde_a[''view'']))=>?z_hn)" (is "?z_ht=>_")
 using v'483 by blast
 assume z_Hg:"((a_CONSTANTunde_leftunde_a[''subject''])~=(a_CONSTANTunde_rightunde_a[''subject'']))" (is "?z_ho~=?z_hq")
 show FALSE
 proof (rule zenon_imply [OF z_Hc])
  assume z_Hu:"((a_CONSTANTunde_leftunde_a[''view''])~=(a_CONSTANTunde_rightunde_a[''view'']))" (is "?z_hi~=?z_hl")
  show FALSE
  proof (rule zenon_imply [OF z_Hd])
   assume z_Hv:"(~?z_ht)"
   show FALSE
   proof (rule zenon_imply [OF z_He])
    assume z_Hw:"(~?z_hr)"
    show FALSE
    proof (rule zenon_or [OF z_Hf])
     assume z_Hh:"?z_hh"
     show FALSE
     by (rule notE [OF z_Hu z_Hh])
    next
     assume z_Hs:"?z_hs"
     show FALSE
     proof (rule zenon_or [OF z_Hs])
      assume z_Ht:"?z_ht"
      show FALSE
      by (rule notE [OF z_Hv z_Ht])
     next
      assume z_Hr:"?z_hr"
      show FALSE
      by (rule notE [OF z_Hw z_Hr])
     qed
    qed
   next
    assume z_Hn:"?z_hn"
    show FALSE
    by (rule notE [OF z_Hg z_Hn])
   qed
  next
   assume z_Hn:"?z_hn"
   show FALSE
   by (rule notE [OF z_Hg z_Hn])
  qed
 next
  assume z_Hn:"?z_hn"
  show FALSE
  by (rule notE [OF z_Hg z_Hn])
 qed
qed
(* END-PROOF *)
ML_command \<open> writeln "*** TLAPS EXIT 150"; \<close> qed
lemma ob'129:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
(* usable definition STATE_ConflictingPrepareAt_ suppressed *)
fixes a_CONSTANTunde_committedunde_a
assumes a_CONSTANTunde_committedunde_a_in : "(a_CONSTANTunde_committedunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
assumes v'476: "((((fapply ((a_CONSTANTunde_committedunde_a), (''signers''))) \<in> ((SUBSET ((a_CONSTANTunde_VotingRosterunde_a ((fapply ((fapply ((a_CONSTANTunde_committedunde_a), (''context''))), (''epoch'')))))))))) & (((((((fapply ((a_CONSTANTunde_committedunde_a), (''signers''))) \<inter> (fapply ((a_CONSTANTunde_committedunde_a), (''signers''))))) \<inter> (a_CONSTANTunde_Honestunde_a))) \<noteq> ({}))))"
shows "(\<exists> a_CONSTANTunde_signerunde_a \<in> (((fapply ((a_CONSTANTunde_committedunde_a), (''signers''))) \<inter> (a_CONSTANTunde_Honestunde_a))) : (TRUE))"(is "PROP ?ob'129")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 129"; \<close>
show "PROP ?ob'129"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 129"; \<close> qed
lemma ob'89:
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
(* usable definition STATE_Init_ suppressed *)
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
(* usable definition STATE_HonestCommitIntentPrepared_ suppressed *)
(* usable definition STATE_PendingVoteWritesAuthorized_ suppressed *)
(* usable definition STATE_IntentPhasesCorrect_ suppressed *)
(* usable definition STATE_CertificatePhasesCorrect_ suppressed *)
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
(* usable definition STATE_ConflictingPrepareAt_ suppressed *)
fixes a_CONSTANTunde_committedunde_a
assumes a_CONSTANTunde_committedunde_a_in : "(a_CONSTANTunde_committedunde_a \<in> (a_VARIABLEunde_commitQCsunde_a))"
fixes a_CONSTANTunde_laterunde_a
assumes a_CONSTANTunde_laterunde_a_in : "(a_CONSTANTunde_laterunde_a \<in> (a_VARIABLEunde_prepareQCsunde_a))"
assumes v'481: "((a_STATEunde_ConflictingPrepareAtunde_a ((a_CONSTANTunde_committedunde_a), (fapply ((a_CONSTANTunde_laterunde_a), (''view''))))))"
assumes v'482: "(((fapply ((a_CONSTANTunde_laterunde_a), (''view''))) \<in> (Nat)))"
assumes v'483: "((\<And> a_CONSTANTunde_Punde_a :: c => c. (\<And> a_CONSTANTunde_nunde_a :: c. a_CONSTANTunde_nunde_a \<in> (Nat) \<Longrightarrow> (((a_CONSTANTunde_Punde_a ((a_CONSTANTunde_nunde_a)))) \<Longrightarrow> (\<exists> a_CONSTANTunde_munde_a \<in> (Nat) : (((a_CONSTANTunde_Punde_a ((a_CONSTANTunde_munde_a)))) & (\<forall> a_CONSTANTunde_kunde_a \<in> ((intvl (((0)), ((subint ((a_CONSTANTunde_munde_a), ((succ[0])))))))) : ((~ ((a_CONSTANTunde_Punde_a ((a_CONSTANTunde_kunde_a)))))))))))))"
shows "(\<exists> a_CONSTANTunde_leastunde_a \<in> (Nat) : (((a_STATEunde_ConflictingPrepareAtunde_a ((a_CONSTANTunde_committedunde_a), (a_CONSTANTunde_leastunde_a)))) & (\<forall> a_CONSTANTunde_priorunde_a \<in> ((intvl (((0)), ((subint ((a_CONSTANTunde_leastunde_a), ((succ[0])))))))) : ((~ ((a_STATEunde_ConflictingPrepareAtunde_a ((a_CONSTANTunde_committedunde_a), (a_CONSTANTunde_priorunde_a)))))))))"(is "PROP ?ob'89")
proof -
ML_command \<open> writeln "*** TLAPS ENTER 89"; \<close>
show "PROP ?ob'89"
using assms by auto
ML_command \<open> writeln "*** TLAPS EXIT 89"; \<close> qed
end
