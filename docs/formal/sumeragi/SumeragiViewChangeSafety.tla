---- MODULE SumeragiViewChangeSafety ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi view-change and lock safety.

This slice covers the rules that connect pacemaker/new-view certificates to
locked proposal acceptance:
- accepted new-view certificates move the local view forward only,
- highest-QC tracking is monotonic over accepted certificate evidence,
- a locked validator accepts a conflicting proposal only when it carries a QC
  strictly higher than the lock, and
- conflicting prepare evidence cannot overwrite an existing lock at the same
  or lower QC rank.

The model abstracts certificate ordering into a finite integer rank. Rank 0 is
the absence of a QC. Higher ranks represent later or stronger QCs under the
same ordering used by the pure engine: height, view, phase, then hash tie-break.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxView,
  \* @type: Int;
  MaxRank,
  \* @type: Bool;
  BugAcceptStaleNewView,
  \* @type: Bool;
  BugAcceptUnsafeProposal,
  \* @type: Bool;
  BugAllowLockOverwrite,
  \* @type: Bool;
  BugAllowHighestRegression

VARIABLES
  \* @type: Int;
  currentView,
  \* @type: Int;
  maxAcceptedView,
  \* @type: Str;
  phase,
  \* @type: Str;
  lockedBranch,
  \* @type: Int;
  lockRank,
  \* @type: Int;
  highestRank,
  \* @type: Set(Int);
  acceptedQcRanks,
  \* @type: Bool;
  staleNewViewAccepted,
  \* @type: Bool;
  unsafeProposalAccepted,
  \* @type: Bool;
  unsafeLockOverwrite,
  \* @type: Bool;
  highestRegressed

vars == <<
  currentView,
  maxAcceptedView,
  phase,
  lockedBranch,
  lockRank,
  highestRank,
  acceptedQcRanks,
  staleNewViewAccepted,
  unsafeProposalAccepted,
  unsafeLockOverwrite,
  highestRegressed
>>

Branches == {"A", "B"}
MaybeBranch == Branches \cup {"None"}
Phases == {"Proposal", "Prepare", "Commit"}
Ranks == 0..MaxRank
QcRanks == 1..MaxRank

Max(a, b) == IF a >= b THEN a ELSE b

ProposalSafe(branch, carriedRank) ==
  \/ lockedBranch = "None"
  \/ branch = lockedBranch
  \/ carriedRank > lockRank

HighestWouldRegress(candidate) ==
  /\ BugAllowHighestRegression
  /\ candidate < highestRank

NextHighest(candidate) ==
  IF candidate > highestRank
  THEN candidate
  ELSE IF HighestWouldRegress(candidate)
       THEN candidate
       ELSE highestRank

TypeInvariant ==
  /\ MaxView \in Nat
  /\ MaxView >= 2
  /\ MaxRank \in Nat
  /\ MaxRank >= 2
  /\ BugAcceptStaleNewView \in BOOLEAN
  /\ BugAcceptUnsafeProposal \in BOOLEAN
  /\ BugAllowLockOverwrite \in BOOLEAN
  /\ BugAllowHighestRegression \in BOOLEAN
  /\ currentView \in 0..MaxView
  /\ maxAcceptedView \in 0..MaxView
  /\ phase \in Phases
  /\ lockedBranch \in MaybeBranch
  /\ lockRank \in Ranks
  /\ highestRank \in Ranks
  /\ acceptedQcRanks \subseteq Ranks
  /\ 0 \in acceptedQcRanks
  /\ staleNewViewAccepted \in BOOLEAN
  /\ unsafeProposalAccepted \in BOOLEAN
  /\ unsafeLockOverwrite \in BOOLEAN
  /\ highestRegressed \in BOOLEAN
  /\ lockedBranch = "None" <=> lockRank = 0
  /\ lockRank <= highestRank

Init ==
  /\ currentView = 0
  /\ maxAcceptedView = 0
  /\ phase = "Proposal"
  /\ lockedBranch = "None"
  /\ lockRank = 0
  /\ highestRank = 0
  /\ acceptedQcRanks = {0}
  /\ staleNewViewAccepted = FALSE
  /\ unsafeProposalAccepted = FALSE
  /\ unsafeLockOverwrite = FALSE
  /\ highestRegressed = FALSE

AcceptProposal(branch, carriedRank) ==
  /\ branch \in Branches
  /\ carriedRank \in Ranks
  /\ phase = "Proposal"
  /\ (ProposalSafe(branch, carriedRank) \/ BugAcceptUnsafeProposal)
  /\ unsafeProposalAccepted' =
      (unsafeProposalAccepted \/ ~ProposalSafe(branch, carriedRank))
  /\ phase' = "Prepare"
  /\ UNCHANGED <<
      currentView,
      maxAcceptedView,
      lockedBranch,
      lockRank,
      highestRank,
      acceptedQcRanks,
      staleNewViewAccepted,
      unsafeLockOverwrite,
      highestRegressed
     >>

PrepareQc(branch, rank) ==
  /\ branch \in Branches
  /\ rank \in QcRanks
  /\ (lockedBranch = "None" \/ branch = lockedBranch \/ rank > lockRank
      \/ BugAllowLockOverwrite)
  /\ unsafeLockOverwrite' =
      (unsafeLockOverwrite
        \/ (lockedBranch # "None" /\ branch # lockedBranch /\ rank <= lockRank))
  /\ lockedBranch' =
      IF lockedBranch = "None" \/ rank > lockRank \/ BugAllowLockOverwrite
      THEN branch
      ELSE lockedBranch
  /\ lockRank' = Max(lockRank, rank)
  /\ highestRank' = NextHighest(rank)
  /\ acceptedQcRanks' = acceptedQcRanks \cup {rank}
  /\ highestRegressed' = (highestRegressed \/ HighestWouldRegress(rank))
  /\ phase' = "Commit"
  /\ UNCHANGED <<
      currentView,
      maxAcceptedView,
      staleNewViewAccepted,
      unsafeProposalAccepted
     >>

NewViewQc(newView, carriedRank) ==
  /\ newView \in 0..MaxView
  /\ carriedRank \in Ranks
  /\ (newView > currentView \/ BugAcceptStaleNewView)
  /\ staleNewViewAccepted' =
      (staleNewViewAccepted \/ (newView <= currentView))
  /\ currentView' = newView
  /\ maxAcceptedView' = Max(maxAcceptedView, newView)
  /\ phase' = "Proposal"
  /\ highestRank' = NextHighest(carriedRank)
  /\ acceptedQcRanks' = acceptedQcRanks \cup {carriedRank}
  /\ highestRegressed' =
      (highestRegressed \/ HighestWouldRegress(carriedRank))
  /\ UNCHANGED <<
      lockedBranch,
      lockRank,
      unsafeProposalAccepted,
      unsafeLockOverwrite
     >>

Stable ==
  UNCHANGED vars

Next ==
  \/ \E branch \in Branches, carriedRank \in Ranks:
       AcceptProposal(branch, carriedRank)
  \/ \E branch \in Branches, rank \in QcRanks:
       PrepareQc(branch, rank)
  \/ \E newView \in 0..MaxView, carriedRank \in Ranks:
       NewViewQc(newView, carriedRank)
  \/ Stable

CurrentViewNeverRewinds ==
  currentView = maxAcceptedView

StaleNewViewCertificatesRejected ==
  ~staleNewViewAccepted

HighestQcDominatesAcceptedEvidence ==
  \A rank \in acceptedQcRanks: highestRank >= rank

HighestQcNeverRegresses ==
  ~highestRegressed

UnsafeProposalsRejected ==
  ~unsafeProposalAccepted

ConflictingLockOverwritesRejected ==
  ~unsafeLockOverwrite

====
