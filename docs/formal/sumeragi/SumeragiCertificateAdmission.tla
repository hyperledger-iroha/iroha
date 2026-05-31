---- MODULE SumeragiCertificateAdmission ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi certificate admission safety.

This slice covers the fail-closed guard before certificate evidence can mutate
consensus state. Prepare and commit certificates must match the current height,
view, epoch, validator set, and quorum policy. New-view certificates must match
the current height, epoch, validator set, and quorum policy before the
view-change logic can consider them. Certificates for future heights, stale
prepare/commit views, wrong context, or already committed heights must be
ignored.

Other formal slices model what valid evidence does after admission. This model
only checks that invalid evidence cannot change the lock, phase, pending
finality, or committed-height state.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxView,
  \* @type: Bool;
  BugAcceptWrongContext,
  \* @type: Bool;
  BugAcceptStalePrepareCommit,
  \* @type: Bool;
  BugAcceptFutureHeight,
  \* @type: Bool;
  BugAcceptCommittedHeight

VARIABLES
  \* @type: Int;
  currentView,
  \* @type: Str;
  phase,
  \* @type: Bool;
  locked,
  \* @type: Int;
  lockedView,
  \* @type: Bool;
  pendingFinality,
  \* @type: Bool;
  committedHeight,
  \* @type: Bool;
  wrongContextMutated,
  \* @type: Bool;
  stalePrepareCommitMutated,
  \* @type: Bool;
  futureHeightMutated,
  \* @type: Bool;
  committedHeightMutated

vars == <<
  currentView,
  phase,
  locked,
  lockedView,
  pendingFinality,
  committedHeight,
  wrongContextMutated,
  stalePrepareCommitMutated,
  futureHeightMutated,
  committedHeightMutated
>>

Phases == {"Proposal", "Prepare", "Commit", "PendingFinality"}
CertPhases == {"Prepare", "Commit", "NewView"}

MutateForAcceptedCert(certPhase) ==
  /\ IF certPhase = "Prepare"
     THEN
       /\ locked' = TRUE
       /\ lockedView' = currentView
       /\ phase' = "Commit"
       /\ pendingFinality' = pendingFinality
       /\ committedHeight' = committedHeight
     ELSE IF certPhase = "Commit"
     THEN
       /\ locked' = locked
       /\ lockedView' = lockedView
       /\ phase' = "PendingFinality"
       /\ pendingFinality' = TRUE
       /\ committedHeight' = committedHeight
     ELSE
       /\ locked' = locked
       /\ lockedView' = lockedView
       /\ phase' = "Proposal"
       /\ pendingFinality' = pendingFinality
       /\ committedHeight' = committedHeight

TypeInvariant ==
  /\ MaxView \in Nat
  /\ MaxView >= 2
  /\ BugAcceptWrongContext \in BOOLEAN
  /\ BugAcceptStalePrepareCommit \in BOOLEAN
  /\ BugAcceptFutureHeight \in BOOLEAN
  /\ BugAcceptCommittedHeight \in BOOLEAN
  /\ currentView \in 0..MaxView
  /\ phase \in Phases
  /\ locked \in BOOLEAN
  /\ lockedView \in 0..MaxView
  /\ pendingFinality \in BOOLEAN
  /\ committedHeight \in BOOLEAN
  /\ wrongContextMutated \in BOOLEAN
  /\ stalePrepareCommitMutated \in BOOLEAN
  /\ futureHeightMutated \in BOOLEAN
  /\ committedHeightMutated \in BOOLEAN
  /\ ~locked => lockedView = 0
  /\ locked => lockedView <= currentView
  /\ committedHeight => ~pendingFinality

Init ==
  /\ currentView = 0
  /\ phase = "Proposal"
  /\ locked = FALSE
  /\ lockedView = 0
  /\ pendingFinality = FALSE
  /\ committedHeight = FALSE
  /\ wrongContextMutated = FALSE
  /\ stalePrepareCommitMutated = FALSE
  /\ futureHeightMutated = FALSE
  /\ committedHeightMutated = FALSE

AcceptCurrentCertificate(certPhase) ==
  /\ certPhase \in CertPhases
  /\ ~committedHeight
  /\ MutateForAcceptedCert(certPhase)
  /\ UNCHANGED <<
      currentView,
      wrongContextMutated,
      stalePrepareCommitMutated,
      futureHeightMutated,
      committedHeightMutated
     >>

FinalizePendingCurrentHeight ==
  /\ pendingFinality
  /\ committedHeight' = TRUE
  /\ pendingFinality' = FALSE
  /\ phase' = "Proposal"
  /\ UNCHANGED <<
      currentView,
      locked,
      lockedView,
      wrongContextMutated,
      stalePrepareCommitMutated,
      futureHeightMutated,
      committedHeightMutated
     >>

TimeoutAdvanceView ==
  /\ currentView < MaxView
  /\ currentView' = currentView + 1
  /\ phase' = "Proposal"
  /\ UNCHANGED <<
      locked,
      lockedView,
      pendingFinality,
      committedHeight,
      wrongContextMutated,
      stalePrepareCommitMutated,
      futureHeightMutated,
      committedHeightMutated
     >>

WrongContextCertificate(certPhase) ==
  /\ certPhase \in CertPhases
  /\ IF BugAcceptWrongContext
     THEN
       /\ MutateForAcceptedCert(certPhase)
       /\ wrongContextMutated' = TRUE
     ELSE
       /\ UNCHANGED <<
           phase,
           locked,
           lockedView,
           pendingFinality,
           committedHeight,
           wrongContextMutated
          >>
  /\ UNCHANGED <<
      currentView,
      stalePrepareCommitMutated,
      futureHeightMutated,
      committedHeightMutated
     >>

StalePrepareCommitCertificate(certPhase) ==
  /\ certPhase \in {"Prepare", "Commit"}
  /\ currentView > 0
  /\ IF BugAcceptStalePrepareCommit
     THEN
       /\ MutateForAcceptedCert(certPhase)
       /\ stalePrepareCommitMutated' = TRUE
     ELSE
       /\ UNCHANGED <<
           phase,
           locked,
           lockedView,
           pendingFinality,
           committedHeight,
           stalePrepareCommitMutated
          >>
  /\ UNCHANGED <<
      currentView,
      wrongContextMutated,
      futureHeightMutated,
      committedHeightMutated
     >>

FutureHeightCertificate(certPhase) ==
  /\ certPhase \in CertPhases
  /\ IF BugAcceptFutureHeight
     THEN
       /\ MutateForAcceptedCert(certPhase)
       /\ futureHeightMutated' = TRUE
     ELSE
       /\ UNCHANGED <<
           phase,
           locked,
           lockedView,
           pendingFinality,
           committedHeight,
           futureHeightMutated
          >>
  /\ UNCHANGED <<
      currentView,
      wrongContextMutated,
      stalePrepareCommitMutated,
      committedHeightMutated
     >>

CommittedHeightCertificate(certPhase) ==
  /\ certPhase \in CertPhases
  /\ committedHeight
  /\ IF BugAcceptCommittedHeight
     THEN
       /\ MutateForAcceptedCert(certPhase)
       /\ committedHeightMutated' = TRUE
     ELSE
       /\ UNCHANGED <<
           phase,
           locked,
           lockedView,
           pendingFinality,
           committedHeight,
           committedHeightMutated
          >>
  /\ UNCHANGED <<
      currentView,
      wrongContextMutated,
      stalePrepareCommitMutated,
      futureHeightMutated
     >>

Stable ==
  UNCHANGED vars

Next ==
  \/ \E certPhase \in CertPhases: AcceptCurrentCertificate(certPhase)
  \/ FinalizePendingCurrentHeight
  \/ TimeoutAdvanceView
  \/ \E certPhase \in CertPhases: WrongContextCertificate(certPhase)
  \/ \E certPhase \in {"Prepare", "Commit"}:
       StalePrepareCommitCertificate(certPhase)
  \/ \E certPhase \in CertPhases: FutureHeightCertificate(certPhase)
  \/ \E certPhase \in CertPhases: CommittedHeightCertificate(certPhase)
  \/ Stable

WrongContextCertificatesIgnored ==
  ~wrongContextMutated

StalePrepareCommitCertificatesIgnored ==
  ~stalePrepareCommitMutated

FutureHeightCertificatesIgnored ==
  ~futureHeightMutated

CommittedHeightCertificatesIgnored ==
  ~committedHeightMutated

LockedCertificateMatchesCurrentView ==
  locked => lockedView <= currentView

CommittedHeightHasNoPendingFinality ==
  committedHeight => ~pendingFinality

====
