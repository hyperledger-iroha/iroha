---- MODULE SumeragiV2AsyncRankAndInitContinuationProofs ----
EXTENDS SumeragiV2AsyncRankAndInitProofs

THEOREM ModelResponsiveValidators ==
  ModelConfiguration => Responsive \subseteq ValidatorIds
BY SMT DEF ModelConfiguration, QuorumConfiguration

AsyncHistoricalRecoveryFrameVars ==
  <<context, up, gst, applied, asyncHistoricalRecoveryTargets>>

THEOREM HistoricalRecoveryFramePreservesType ==
  /\ AsyncHistoricalRecoveryTypeInvariant
  /\ UNCHANGED AsyncHistoricalRecoveryFrameVars
  => AsyncHistoricalRecoveryTypeInvariant'
BY SMT
   DEF AsyncHistoricalRecoveryTypeInvariant,
       AsyncHistoricalRecoveryFrameVars, NodeHasApplication

THEOREM AsyncInitEstablishesHistoricalRecoveryType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncHistoricalRecoveryTypeInvariant
BY SMT
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncHistoricalRecoveryTypeInvariant, NodeHasApplication

THEOREM HistoricalRecoveryTargetsAreValidators ==
  AsyncTypeInvariant
    => asyncHistoricalRecoveryTargets \subseteq ValidatorIds
BY ModelResponsiveValidators, SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant, TypeInvariant

THEOREM HistoricalRecoveryOnlyChangePreservesSchedulerType ==
  /\ AsyncSchedulerTypeInvariant
  /\ UNCHANGED <<context, AsyncSchedulerExceptHistoricalRecoveryTargets>>
  /\ AsyncHistoricalRecoveryTypeInvariant'
  => AsyncSchedulerTypeInvariant'
BY Isa
   DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncSchedulerExceptHistoricalRecoveryTargets,
       AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars

THEOREM AsyncInitEstablishesProducerTypeInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncProducerTypeInvariant
BY FS_EmptySet, Zenon
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncProducerInit,
       AsyncProducerTypeInvariant, AsyncProducerTypeInvariantAt,
       AsyncProducerJournalClosed, AsyncProducerJournalClosedAt

THEOREM AsyncInitEstablishesServeProducerEpisodeTypeInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncServeProducerEpisodeTypeInvariant
BY Zenon
   DEF AsyncInitAt, AsyncBaseInitAt,
       AsyncServeProducerEpisodeInit,
       AsyncServeProducerEpisodeTypeInvariant

THEOREM AsyncInitEstablishesSchedulerType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncSchedulerTypeInvariant
BY AsyncInitEstablishesRuntimeType, AsyncInitEstablishesIoType,
   AsyncInitEstablishesDeferredType, AsyncInitEstablishesTransportType,
   AsyncInitEstablishesIngressType,
   AsyncInitEstablishesHistoricalRecoveryType
   DEF AsyncSchedulerTypeInvariant

=============================================================================
