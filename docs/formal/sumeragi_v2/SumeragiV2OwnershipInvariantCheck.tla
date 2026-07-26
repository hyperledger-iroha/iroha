---- MODULE SumeragiV2OwnershipInvariantCheck ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Small exhaustive counterexample search for scheduler ownership.  The state
constraint holds the logical clock at its initial value while retaining every
non-clock AsyncNext branch, so TLC can enumerate all zero-clock ownership
transfers without conflating this finite check with deductive proof evidence.
***************************************************************************)

SingleValidatorRosters == <<<<0>>>>
SingleValidatorPowers == <<<<1>>>>

OwnershipAllVars == <<AsyncAllVars, acquisitionVars>>

OwnershipBoundedInit ==
  /\ AsyncFiniteInit
  /\ AcquisitionInit

(***************************************************************************
The production evidence union has one powerset-valued branch.  The runtime
type invariant already proves every command evidence value structurally
typed, so this bounded TLC instance may select the TC branch without
materializing `TcRecordSet`.  `ownership_n1.cfg` overrides only the equivalent
selector definition; the production transition relation remains otherwise
unchanged.
***************************************************************************)
OwnershipInstallTcFromEvidence(command) ==
  IF AsyncTcRecordTyped(command.evidence)
  THEN command.evidence
  ELSE command.evidence.envelope.tc

(***************************************************************************
The generic definition quantifies over `AsyncCandidateSet` before constraining
all candidate fields to one exact Fetch. That Cartesian record carrier is too
large for TLC once a historical lock appears. Constructing the unique
candidate and checking its structural type is logically equivalent and keeps
the bounded ownership search executable.
***************************************************************************)
OwnershipHistoricalLockRestartExactCurrentFetchOwnerAfter(authority) ==
  \E qc \in prepareQCs':
    LET candidate ==
          AsyncCandidateWithIdentity(
            "Completion", "FetchBody", authority.node,
            authority.context.height, authority.view, authority.subject,
            NoAsyncItem, authority.context, nodeView'[authority.node],
            generation'[authority.node], qc, authority.subject,
            authority.subject, authority.subject)
    IN /\ AsyncCandidateTyped(candidate)
       /\ HistoricalLockRestartAuthoritySourceKernel(
            authority, qc, context', nodeView', lockRank', lockSubject',
            installedTCs', commitIntents', decisions')
       /\ HistoricalLockRestartExactCurrentFetchKernel(
            authority, qc, candidate, context', nodeView', generation',
            asyncCommandQueues', asyncDeferredCompletionQueues',
            asyncDeferredProgressQueues', asyncDeferredNormalQueues',
            asyncCausalQueues', asyncOutstandingWork')

OwnershipHistoricalLockRestartAuthorityTransition ==
  \/ \E node \in ValidatorIds:
       /\ ResponsiveCrashRecoveryRegistration(node)
       /\ asyncHistoricalLockRestartAuthorities' =
            asyncHistoricalLockRestartAuthorities
              \cup ResponsiveCrashHistoricalLockRestartAuthorities(node)
  \/ /\ ~\E node \in ValidatorIds:
             ResponsiveCrashRecoveryRegistration(node)
     /\ asyncHistoricalLockRestartAuthorities' =
          {authority \in asyncHistoricalLockRestartAuthorities:
             /\ HistoricalLockRestartAuthoritySourceAfter(authority)
             /\ ~OwnershipHistoricalLockRestartExactCurrentFetchOwnerAfter(
                   authority)}

(***************************************************************************
The production relation keeps Byzantine signer membership inside each Core
action. TLC enumerates the much larger proposal/QC domains before reaching
that guard, even when the N=1 ownership model has no Byzantine validator.
This equivalent finite-search projection moves the same signer guard to the
outer quantifier. It retains every fault branch while avoiding enumeration of
impossible Byzantine payloads in the all-honest boundary configuration.
***************************************************************************)
OwnershipFaultStep ==
  \/ \E packet \in asyncTransport: PreGstLosePacket(packet)
  \/ \E node \in ValidatorIds: PreGstCrash(node)
  \/ \E source \in AsyncIngressSources, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineNoise(source, recipient, nonce)
  \/ \E kind \in IngressTransportCompletionKinds,
       recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectUntrustedTransportCompletion(kind, recipient, nonce)
  \/ \E kind \in {"NormalJunk", "ProgressJunk"},
       source \in ValidatorIds, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectAuthenticatedJunk(kind, source, recipient, nonce)
  \/ \E source \in ValidatorIds, recipient \in ValidatorIds,
       qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineProposal(signer, roundView, subject,
                              timeoutCertificate, highestPrepare)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, phase \in Phases, subject \in Subjects:
       AsyncByzantineVote(signer, roundView, phase, subject)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineTimeout(signer, roundView, highestPrepare)

OwnershipNonRunnerStep ==
  /\ \/ AsyncSetGST
     \/ AsyncTick
     \/ (\E node \in ValidatorIds: OpenHistoricalRecovery(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           DirectCommitCertificateDiscoveryStep(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           DirectHistoricalCommitCertificateDiscoveryStep(node))
     \/ (\E node \in AsyncArchiveIoServiceNodes:
           ServiceIoWorker(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           ServiceHistoricalRecoveryIoWorker(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           EnqueueIoLocalControl(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           EnqueueHistoricalRecoveryIoLocalControl(node))
     \/ AsyncNetworkStep
     \/ OwnershipFaultStep
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipNonRunnerNoFaultStep ==
  /\ \/ AsyncSetGST
     \/ AsyncTick
     \/ (\E node \in ValidatorIds: OpenHistoricalRecovery(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           DirectCommitCertificateDiscoveryStep(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           DirectHistoricalCommitCertificateDiscoveryStep(node))
     \/ (\E node \in AsyncArchiveIoServiceNodes:
           ServiceIoWorker(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           ServiceHistoricalRecoveryIoWorker(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           EnqueueIoLocalControl(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           EnqueueHistoricalRecoveryIoLocalControl(node))
     \/ AsyncNetworkStep
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipNonCrashStep ==
  \/ /\ (AsyncRunnerStep \/ OwnershipNonRunnerStep)
     /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
     /\ OwnershipHistoricalLockRestartAuthorityTransition
  \/ /\ (DriveResponsiveReplayHead \/ FinishResponsiveReplay)
     /\ UNCHANGED up
  \/ /\ RearmResponsiveRecovery
     /\ UNCHANGED up

OwnershipAsyncNext ==
  /\ (OwnershipNonCrashStep
        \/ (\E node \in ValidatorIds: PreGstCrash(node))
        \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay)
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context>>

OwnershipBoundedNext ==
  /\ OwnershipAsyncNext
  /\ UNCHANGED acquisitionVars

OwnershipDebugStutter ==
  UNCHANGED OwnershipAllVars

OwnershipDebugAsyncOnlyNext ==
  OwnershipBoundedNext

OwnershipDebugNonRunnerOnlyNext ==
  /\ OwnershipNonRunnerStep
  /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context, acquisitionVars>>

OwnershipDebugRunnerOnlyNext ==
  /\ AsyncRunnerStep
  /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context, acquisitionVars>>

OwnershipDebugRunnerAndNonFaultNext ==
  /\ (\/ /\ (AsyncRunnerStep \/ OwnershipNonRunnerNoFaultStep)
           /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
           /\ OwnershipHistoricalLockRestartAuthorityTransition
        \/ /\ (DriveResponsiveReplayHead \/ FinishResponsiveReplay)
           /\ UNCHANGED up
        \/ /\ RearmResponsiveRecovery
           /\ UNCHANGED up
        \/ (\E node \in ValidatorIds: PreGstCrash(node))
        \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay)
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context, acquisitionVars>>

OwnershipDebugStableRunnerAndNonFaultNext ==
  /\ (AsyncRunnerStep \/ OwnershipNonRunnerNoFaultStep)
  /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context, acquisitionVars>>

OwnershipDebugClockStep ==
  /\ (AsyncSetGST \/ AsyncTick)
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugRecoveryStep ==
  /\ (\/ (\E node \in ValidatorIds: OpenHistoricalRecovery(node))
      \/ (\E node \in AsyncCurrentResponsiveVoters:
            DirectCommitCertificateDiscoveryStep(node))
      \/ (\E node \in asyncHistoricalRecoveryTargets:
            DirectHistoricalCommitCertificateDiscoveryStep(node)))
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugIoStep ==
  /\ (\/ (\E node \in AsyncArchiveIoServiceNodes:
            ServiceIoWorker(node))
      \/ (\E node \in asyncHistoricalRecoveryTargets:
            ServiceHistoricalRecoveryIoWorker(node))
      \/ (\E node \in AsyncCurrentResponsiveVoters:
            EnqueueIoLocalControl(node))
      \/ (\E node \in asyncHistoricalRecoveryTargets:
            EnqueueHistoricalRecoveryIoLocalControl(node)))
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugIoServiceStep ==
  /\ (\/ (\E node \in AsyncArchiveIoServiceNodes:
            ServiceIoWorker(node))
      \/ (\E node \in asyncHistoricalRecoveryTargets:
            ServiceHistoricalRecoveryIoWorker(node)))
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugIoEnqueueStep ==
  /\ (\/ (\E node \in AsyncCurrentResponsiveVoters:
            EnqueueIoLocalControl(node))
      \/ (\E node \in asyncHistoricalRecoveryTargets:
            EnqueueHistoricalRecoveryIoLocalControl(node)))
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugNetworkStep ==
  /\ AsyncNetworkStep
  /\ UNCHANGED asyncNodeServiceDeadlines

OwnershipDebugStableRunnerWith(nonRunnerStep) ==
  /\ (AsyncRunnerStep \/ nonRunnerStep)
  /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
  /\ OwnershipHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context, acquisitionVars>>

OwnershipDebugStableRunnerAndClockNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugClockStep)

OwnershipDebugStableRunnerAndRecoveryNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugRecoveryStep)

OwnershipDebugStableRunnerAndIoNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugIoStep)

OwnershipDebugStableRunnerAndIoServiceNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugIoServiceStep)

OwnershipDebugStableRunnerAndIoEnqueueNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugIoEnqueueStep)

OwnershipDebugStableRunnerAndNetworkNext ==
  OwnershipDebugStableRunnerWith(OwnershipDebugNetworkStep)

OwnershipBoundedSpec ==
  OwnershipBoundedInit /\ [][OwnershipBoundedNext]_OwnershipAllVars

OwnershipInitialClock == asyncNow = 0

=============================================================================
