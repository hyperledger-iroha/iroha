---- MODULE SumeragiV2AsyncRankAndInitContinuationProofs ----
EXTENDS SumeragiV2AsyncRankAndInitProofs

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
