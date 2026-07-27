---- MODULE SumeragiV2TimeoutViewProgressProofs ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs

(***************************************************************************
Direct timeout/view temporal decomposition.

This leaf is below locked-body reproposal and rotating-leader progress.  It
therefore cannot use either rotating-leader clause, aggregate responsive
Decision convergence, or a terminal Decision reached through adequate-leader
service to discharge the timeout property.

For one frozen `(target, roundView)` the direct corridor is:

  1. source ownership exposes a higher-view TC/Decision frontier or a finite
     catch-up rank;
  2. catch-up consumes every missing `(source, intermediate view)` slot;
  3. once every responsive voter is at `roundView`, receipt service consumes
     the finite set of missing `(target, signer, roundView)` receipts;
  4. the exact last receipt exposes the target's TC formation/install
     frontier; and
  5. PersistInstallTC strictly advances the target view.

The catch-up rank is deliberately not the number of lagging validators.  One
validator may advance several views while remaining below `roundView`, which
would leave that count unchanged.  `TimeoutCatchupDebtSlots` instead contains
one slot for every missing intermediate view, so every certified view advance
removes concrete debt.

Six proof seams remain explicit below.  They are property declarations and
premises of the final reduction, not asserted theorems:

  * timeout ownership initialization and action preservation;
  * pre-deadline clock convergence while Tick may be blocked;
  * exact local timeout-owner/candidate exit handoff;
  * source-isolated vote delivery convergence;
  * finite catch-up/receipt rank coordination; and
  * exact TC-install and Decision-certificate convergence.

No scheduler action, fairness domain, or network transition is redefined.
***************************************************************************)

TimeoutDirectReleaseSource(target, roundView) ==
  /\ gst
  /\ target \in AsyncCurrentResponsiveVoters
  /\ nodeView[target] = roundView
  /\ ~NodeHasDecision(target)

TimeoutDirectGoal(target, roundView) ==
  TimeoutViewGoal(target, roundView)

(***************************************************************************
Finite two-stage rank.

Stage 2 is voter catch-up and stage 1 is timeout-receipt collection.  The
ordinary lexicographic natural ordering therefore permits an arbitrary finite
receipt rank after catch-up reaches zero while still making the stage change
strictly descend.
***************************************************************************)

TimeoutCatchupDebtSlots(roundView) ==
  {slot \in AsyncCurrentResponsiveVoters \X (0..roundView):
     /\ nodeView[slot[1]] <= slot[2]
     /\ slot[2] < roundView}

TimeoutCatchupDebtRank(roundView) ==
  Cardinality(TimeoutCatchupDebtSlots(roundView))

TimeoutCatchupDebtAtRank(target, roundView, rank) ==
  /\ TimeoutRoundStable(target, roundView)
  /\ ~ResponsiveDecisionExists
  /\ \A source \in AsyncCurrentResponsiveVoters:
       nodeView[source] <= roundView
  /\ TimeoutCatchupDebtRank(roundView) = rank

TimeoutProgressRankCarrier == (1..2) \X Nat

TimeoutProgressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 1..2, Nat)

TimeoutProgressRankFrontier(target, roundView, rank) ==
  /\ rank \in TimeoutProgressRankCarrier
  /\ IF rank[1] = 2
     THEN TimeoutCatchupDebtAtRank(target, roundView, rank[2])
     ELSE TimeoutReceiptAtRank(target, roundView, rank[2])

TimeoutProgressRankedFrontier(target, roundView) ==
  \E rank \in TimeoutProgressRankCarrier:
    TimeoutProgressRankFrontier(target, roundView, rank)

(***************************************************************************
Exact non-rank owners.

Every disjunct retains the target identity.  In particular, a global formed
TC is not accepted: the certificate must be in this target's exact retained,
transport, ingress, reducer, or install frontier.
***************************************************************************)

TimeoutDirectOwnerFrontier(target, roundView) ==
  \/ DecisionPropagationFrontier(target)
  \/ TcFrontier(target, roundView)
  \/ TimeoutCertificateFormationFrontier(target, roundView)

TimeoutDeadlineArmedOwner(source, sourceView) ==
  /\ TimeoutRoundStable(source, sourceView)
  /\ ~NodeTimedOut(source, sourceView)
  /\ \/ asyncNodeDeadlines[source] <= asyncNow
     \/ "TimeoutElapsed" \in asyncOutstandingTags[source]

(***************************************************************************
Static rank and source-exposure facts.
***************************************************************************)

THEOREM TimeoutCatchupDebtRankIsNatural ==
  \A roundView \in Views:
    AsyncTypeInvariant
      => TimeoutCatchupDebtRank(roundView) \in Nat
PROOF
  <1>1. ASSUME NEW roundView \in Views,
                AsyncTypeInvariant
         PROVE TimeoutCatchupDebtRank(roundView) \in Nat
    <2>1. /\ roundView \in Nat
           /\ IsFiniteSet(AsyncCurrentResponsiveVoters)
      BY <1>1, RuntimeValidatorIdsAreFinite, FS_Subset, Isa
         DEF AsyncTypeInvariant, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch, Views
    <2>2. IsFiniteSet(
             AsyncCurrentResponsiveVoters \X (0..roundView))
      BY <2>1, FS_Interval, FS_Product
    <2>3. TimeoutCatchupDebtSlots(roundView)
             \subseteq
               AsyncCurrentResponsiveVoters \X (0..roundView)
      BY DEF TimeoutCatchupDebtSlots
    <2>4. IsFiniteSet(TimeoutCatchupDebtSlots(roundView))
      BY <2>2, <2>3, FS_Subset
    <2> QED BY <2>4, FS_CardinalityType
         DEF TimeoutCatchupDebtRank
  <1> QED BY <1>1

THEOREM TimeoutDirectReleaseSourceIsRoundStable ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutDirectReleaseSource(target, roundView)
    => TimeoutRoundStable(target, roundView)
BY GstResponsiveNodesAreUp, Isa
   DEF TimeoutDirectReleaseSource, TimeoutRoundStable,
       AsyncStrongTypeInvariant

THEOREM TimeoutRoundStableExposesRankOrExactOwner ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    => \/ TimeoutDirectGoal(target, roundView)
       \/ TimeoutDirectOwnerFrontier(target, roundView)
       \/ TimeoutProgressRankedFrontier(target, roundView)
BY TimeoutCatchupDebtRankIsNatural,
   ResponsiveAuthoritySuppliesEveryTcFrontier,
   IsaT(240)
   DEF TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutProgressRankedFrontier,
       TimeoutProgressRankFrontier,
       TimeoutCatchupDebtAtRank, TimeoutCatchupDebtRank,
       TimeoutRoundStable, ResponsiveDecisionExists,
       TimeoutViewOwnershipInvariant,
       ResponsiveViewCertificateAuthority,
       DecisionPropagationFrontier, DecisionSourceAt,
       NodeHasDecision, TcFrontier

(***************************************************************************
Six honest residual properties.
***************************************************************************)

TimeoutViewOwnershipPreservationProperty(specification) ==
  specification => []TimeoutViewOwnershipInvariant

\* TODO: prove initialization and preservation of each source-owned timeout
\* vote, exact current TC outbox, and Decision propagation frontier across
\* every async step, crash/restart, replay, and retained-control replacement.

TimeoutDeadlineClockConvergenceProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutRoundTrigger(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutDeadlineArmedOwner(source, sourceView)
                \/ \E vote \in TimeoutVoteRecordSet:
                     TimeoutOrigin(source, sourceView, vote))

\* TODO: prove the pre-deadline clock prefix.  Weak fairness of AsyncTick is
\* insufficient while overdue packet, node-service, or I/O-service owners
\* disable it; those exact finite blockers must be discharged first.

TimeoutSemanticOwnerHandoffProperty(specification) ==
  specification
    => /\ \A source \in AsyncCurrentResponsiveVoters,
              sourceView \in Views:
             TimeoutDeadlineArmedOwner(source, sourceView)
               ~> (TimeoutDirectGoal(source, sourceView)
                    \/ \E vote \in TimeoutVoteRecordSet:
                         TimeoutOrigin(source, sourceView, vote))
       /\ \A source \in AsyncCurrentResponsiveVoters,
              sourceView \in Views,
              vote \in TimeoutVoteRecordSet,
              recipient \in AsyncCurrentResponsiveVoters:
             TimeoutOrigin(source, sourceView, vote)
               ~> TimeoutOriginOutcome(
                    source, sourceView, vote, recipient)

\* TODO: prove exact BeginTimeout/PersistTimeout/SignTimeout candidate exits
\* from the causal, deferred, runtime, I/O, and replay owners.  Generic
\* protected-candidate starvation alone does not identify the successor
\* timeout vote or preserve its semantic vote identity.

TimeoutSourceIsolatedDeliveryConvergenceProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         TimeoutDelivery(vote, recipient)
           ~> TimeoutDeliveryOutcome(vote, recipient)

\* TODO: prove per-(authenticated source, recipient) transport, ingress, and
\* reducer convergence for the exact immutable TimeoutVote item.  Traffic or
\* backpressure from another source must not replace this owner.

TimeoutFiniteRankDescentProperty(specification) ==
  (/\ TimeoutDeadlineClockConvergenceProperty(specification)
   /\ TimeoutSemanticOwnerHandoffProperty(specification)
   /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(specification))
    => (specification
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                rank \in TimeoutProgressRankCarrier:
               TimeoutProgressRankFrontier(target, roundView, rank)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(target, roundView)
                      \/ \E lowerRank \in
                             SetLessThan(
                               rank,
                               TimeoutProgressRankOrdering,
                               TimeoutProgressRankCarrier):
                           TimeoutProgressRankFrontier(
                             target, roundView, lowerRank)))

\* TODO: prove finite coordination.  Select one concrete missing catch-up slot
\* or receipt signer, preserve all already-retired slots/receipts, and show
\* its exact timeout outcome either lowers this rank or exposes a higher TC or
\* Decision owner for the same target.

TimeoutCertificateAndDecisionConvergenceProperty(specification) ==
  specification
    => \A target \in AsyncCurrentResponsiveVoters,
          roundView \in Views:
         /\ DecisionPropagationFrontier(target)
              ~> NodeHasDecision(target)
         /\ TcFrontier(target, roundView)
              ~> TimeoutDirectGoal(target, roundView)
         /\ TimeoutCertificateFormationFrontier(target, roundView)
              ~> TimeoutDirectGoal(target, roundView)

\* TODO: prove exact Commit-certificate propagation and target-local
\* TC delivery/formation/install convergence.  The terminal TC step must be
\* ExecutePersistInstall for this target; a global formed certificate or
\* another validator's view advance is not an accepted outcome.

DirectTimeoutViewClosureResidualProperty(specification) ==
  /\ TimeoutViewOwnershipPreservationProperty(specification)
  /\ TimeoutDeadlineClockConvergenceProperty(specification)
  /\ TimeoutSemanticOwnerHandoffProperty(specification)
  /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(specification)
  /\ TimeoutFiniteRankDescentProperty(specification)
  /\ TimeoutCertificateAndDecisionConvergenceProperty(specification)

(***************************************************************************
Derived source exposure.

This theorem consumes only the ownership-preservation seam plus already
proved type/recovery invariants.  It does not assume any temporal service
claim for the exposed rank or owner.
***************************************************************************)

THEOREM TimeoutOwnershipPreservationSuppliesExactFrontierExposure ==
  \A initialContext:
    TimeoutViewOwnershipPreservationProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectReleaseSource(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView)
                        \/ TimeoutProgressRankedFrontier(
                             target, roundView)))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target \in AsyncCurrentResponsiveVoters,
                       roundView \in Views:
                      TimeoutDirectReleaseSource(target, roundView)
                        ~> (TimeoutDirectGoal(target, roundView)
                             \/ TimeoutDirectOwnerFrontier(
                                  target, roundView)
                             \/ TimeoutProgressRankedFrontier(
                                  target, roundView))
    <2>1. AsyncSpecAt(initialContext)
              => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncLiveSpecAt(initialContext)
              => []TimeoutViewOwnershipInvariant
      BY <1>1 DEF TimeoutViewOwnershipPreservationProperty
    <2>3. [](\A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
               /\ AsyncStrongTypeInvariant
               /\ TimeoutViewOwnershipInvariant
               /\ TimeoutDirectReleaseSource(target, roundView)
              => \/ TimeoutDirectGoal(target, roundView)
                 \/ TimeoutDirectOwnerFrontier(target, roundView)
                 \/ TimeoutProgressRankedFrontier(
                      target, roundView))
      BY TimeoutDirectReleaseSourceIsRoundStable,
         TimeoutRoundStableExposesRankOrExactOwner,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Well-founded rank closure.
***************************************************************************)

THEOREM TimeoutProgressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    TimeoutProgressRankOrdering,
    TimeoutProgressRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF TimeoutProgressRankOrdering,
       TimeoutProgressRankCarrier

THEOREM TimeoutFiniteRankDescentClosesExactRank ==
  \A initialContext:
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutFiniteRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                rank \in TimeoutProgressRankCarrier:
               TimeoutProgressRankFrontier(target, roundView, rank)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(
                           target, roundView)))
BY TimeoutProgressRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF TimeoutFiniteRankDescentProperty

THEOREM TimeoutFiniteRankDescentClosesRankedFrontier ==
  \A initialContext:
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutFiniteRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views:
               TimeoutProgressRankedFrontier(target, roundView)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(
                           target, roundView)))
BY TimeoutFiniteRankDescentClosesExactRank, PTL
   DEF TimeoutProgressRankedFrontier

THEOREM TimeoutExactOwnerConvergenceClosesOwnerFrontier ==
  \A initialContext:
    TimeoutCertificateAndDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectOwnerFrontier(target, roundView)
                   ~> TimeoutDirectGoal(target, roundView))
BY PTL
   DEF TimeoutCertificateAndDecisionConvergenceProperty,
       TimeoutDirectOwnerFrontier, TimeoutDirectGoal,
       TimeoutCertificateFormationFrontier

(***************************************************************************
Direct release reduction.

Every premise is below timeout/view progress.  No rotating-leader or aggregate
Decision-convergence result appears in this module's dependency path.
***************************************************************************)

THEOREM DirectTimeoutViewDecompositionClosesTimeoutViewProgress ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutViewProgressProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                DirectTimeoutViewClosureResidualProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutViewProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectReleaseSource(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView)
                        \/ TimeoutProgressRankedFrontier(
                             target, roundView))
      BY <1>1,
         TimeoutOwnershipPreservationSuppliesExactFrontierExposure
         DEF DirectTimeoutViewClosureResidualProperty
    <2>2. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutProgressRankedFrontier(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView))
      BY <1>1, TimeoutFiniteRankDescentClosesRankedFrontier
         DEF DirectTimeoutViewClosureResidualProperty
    <2>3. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectOwnerFrontier(target, roundView)
                   ~> TimeoutDirectGoal(target, roundView)
      BY <1>1, TimeoutExactOwnerConvergenceClosesOwnerFrontier
         DEF DirectTimeoutViewClosureResidualProperty
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF TimeoutViewProgressProperty,
             TimeoutDirectReleaseSource,
             TimeoutDirectGoal
  <1> QED BY <1>1

=============================================================================
