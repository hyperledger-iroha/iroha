---- MODULE SumeragiV2HistoricalLockedRecoveryMutation ----
EXTENDS Integers

(***************************************************************************
Bounded regression kernel for the ordinary LockAndCommit -> no-high-TC
recovery seam.  The fixed constructor retains Fetch/Validate ownership from
either an incoming selected high or the exact durable Commit intent which
created the lock.  The installed-only mutation forgets the latter source.

Neither arm grants fresh Commit authority: both reconstruct only body recovery
ownership. An incoming selected high can justify unchanged later reproposal;
only an already durable old same-round Commit may be retransmitted.
***************************************************************************)

CONSTANTS UseFixedRecoveryGuard, AllowFreshHistoricalCommitBug

VARIABLES phase, lockRank, lockSubject, exactLockedCommitIntent,
          incomingHighSelectsLock, fetchOwned, freshCommitAuthorized

vars ==
  <<phase, lockRank, lockSubject, exactLockedCommitIntent,
    incomingHighSelectsLock, fetchOwned, freshCommitAuthorized>>

NoRank == -1
LockedRank == 1
LockedSubject == "A"

LockMatchesHistoricalPrepare ==
  /\ lockRank = LockedRank
  /\ lockSubject = LockedSubject

FixedRecoverySource ==
  /\ LockMatchesHistoricalPrepare
  /\ (incomingHighSelectsLock \/ exactLockedCommitIntent)

InstalledOnlyRecoverySource ==
  /\ LockMatchesHistoricalPrepare
  /\ incomingHighSelectsLock

SelectedRecoverySource ==
  IF UseFixedRecoveryGuard
  THEN FixedRecoverySource
  ELSE InstalledOnlyRecoverySource

FreshHistoricalCommitSource ==
  /\ AllowFreshHistoricalCommitBug
  /\ LockMatchesHistoricalPrepare

Init ==
  /\ phase = 0
  /\ lockRank = NoRank
  /\ lockSubject = "None"
  /\ exactLockedCommitIntent = FALSE
  /\ incomingHighSelectsLock = FALSE
  /\ fetchOwned = FALSE
  /\ freshCommitAuthorized = FALSE

PersistOrdinaryLockAndCommit ==
  /\ phase = 0
  /\ phase' = 1
  /\ lockRank' = LockedRank
  /\ lockSubject' = LockedSubject
  /\ exactLockedCommitIntent' = TRUE
  /\ incomingHighSelectsLock' = FALSE
  /\ fetchOwned' = FALSE
  /\ freshCommitAuthorized' = FALSE

InstallNoHighTcAndConstructRecovery ==
  /\ phase = 1
  /\ ~incomingHighSelectsLock
  /\ phase' = 2
  /\ fetchOwned' = SelectedRecoverySource
  /\ freshCommitAuthorized' = FreshHistoricalCommitSource
  /\ UNCHANGED <<lockRank, lockSubject, exactLockedCommitIntent,
                  incomingHighSelectsLock>>

Next ==
  \/ PersistOrdinaryLockAndCommit
  \/ InstallNoHighTcAndConstructRecovery
  \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeInvariant ==
  /\ phase \in 0..2
  /\ lockRank \in {NoRank, LockedRank}
  /\ lockSubject \in {"None", LockedSubject}
  /\ exactLockedCommitIntent \in BOOLEAN
  /\ incomingHighSelectsLock \in BOOLEAN
  /\ fetchOwned \in BOOLEAN
  /\ freshCommitAuthorized \in BOOLEAN

RecoveryOwnedAfterNoHighCarry ==
  phase < 2 \/ fetchOwned

ExactIntentDoesNotAuthorizeFreshCommit ==
  exactLockedCommitIntent => ~freshCommitAuthorized

NoFreshHistoricalCommit ==
  ~freshCommitAuthorized

=============================================================================
