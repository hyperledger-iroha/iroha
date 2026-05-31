---- MODULE SumeragiCertifiedRecovery ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi certified block recovery safety.

This model covers the finality path where a commit QC can arrive before the
matching block body is locally available. The implementation must keep the QC
pending, fetch or recover the exact payload named by the commit subject, reject
mismatched payload responses without dropping the pending QC, and commit only
after the subject and payload match.

The model abstracts signatures and full block data into two conflicting
same-height subjects, "A" and "B". Each subject represents the tuple of height,
view, block hash, payload hash, checkpoint, and commit QC identity that the live
certified recovery path verifies before applying state.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugCommitWithoutPayload,
  \* @type: Bool;
  BugAcceptMismatchedPayload,
  \* @type: Bool;
  BugAllowConflictingFinality

VARIABLES
  \* @type: Set(Str);
  qcsObserved,
  \* @type: Str;
  pendingSubject,
  \* @type: Bool;
  fetchRequested,
  \* @type: Set(Str);
  matchingPayloads,
  \* @type: Set(Str);
  mismatchedPayloads,
  \* @type: Set(Str);
  rejectedMismatches,
  \* @type: Set(Str);
  committedSubjects

vars == <<
  qcsObserved,
  pendingSubject,
  fetchRequested,
  matchingPayloads,
  mismatchedPayloads,
  rejectedMismatches,
  committedSubjects
>>

Subjects == {"A", "B"}
MaybeSubject == Subjects \cup {"None"}
AcceptedPayloads == matchingPayloads \cup mismatchedPayloads

TypeInvariant ==
  /\ BugCommitWithoutPayload \in BOOLEAN
  /\ BugAcceptMismatchedPayload \in BOOLEAN
  /\ BugAllowConflictingFinality \in BOOLEAN
  /\ qcsObserved \subseteq Subjects
  /\ pendingSubject \in MaybeSubject
  /\ fetchRequested \in BOOLEAN
  /\ matchingPayloads \subseteq Subjects
  /\ mismatchedPayloads \subseteq Subjects
  /\ rejectedMismatches \subseteq Subjects
  /\ committedSubjects \subseteq Subjects
  /\ fetchRequested => pendingSubject \in Subjects
  /\ pendingSubject = "None" => ~fetchRequested

Init ==
  /\ qcsObserved = {}
  /\ pendingSubject = "None"
  /\ fetchRequested = FALSE
  /\ matchingPayloads = {}
  /\ mismatchedPayloads = {}
  /\ rejectedMismatches = {}
  /\ committedSubjects = {}

ObserveCommitQc(subject) ==
  /\ subject \in Subjects
  /\ subject \notin qcsObserved
  /\ pendingSubject = "None"
  /\ (committedSubjects = {} \/ subject \in committedSubjects
      \/ BugAllowConflictingFinality)
  /\ qcsObserved' = qcsObserved \cup {subject}
  /\ pendingSubject' = subject
  /\ fetchRequested' = TRUE
  /\ UNCHANGED <<
      matchingPayloads,
      mismatchedPayloads,
      rejectedMismatches,
      committedSubjects
     >>

ReceivePayload(response) ==
  /\ pendingSubject \in Subjects
  /\ fetchRequested
  /\ response \in Subjects
  /\ IF response = pendingSubject
     THEN
       /\ matchingPayloads' = matchingPayloads \cup {pendingSubject}
       /\ mismatchedPayloads' = mismatchedPayloads
     ELSE
       /\ BugAcceptMismatchedPayload
       /\ matchingPayloads' = matchingPayloads
       /\ mismatchedPayloads' = mismatchedPayloads \cup {pendingSubject}
  /\ fetchRequested' = FALSE
  /\ UNCHANGED <<
      qcsObserved,
      pendingSubject,
      rejectedMismatches,
      committedSubjects
     >>

RejectMismatchedPayload(response) ==
  /\ pendingSubject \in Subjects
  /\ fetchRequested
  /\ response \in Subjects
  /\ response # pendingSubject
  /\ ~BugAcceptMismatchedPayload
  /\ pendingSubject \notin rejectedMismatches
  /\ rejectedMismatches' = rejectedMismatches \cup {pendingSubject}
  /\ UNCHANGED <<
      qcsObserved,
      pendingSubject,
      fetchRequested,
      matchingPayloads,
      mismatchedPayloads,
      committedSubjects
     >>

CommitFinality(subject) ==
  /\ subject \in Subjects
  /\ subject \notin committedSubjects
  /\ subject \in qcsObserved
  /\ pendingSubject = subject
  /\ (subject \in AcceptedPayloads \/ BugCommitWithoutPayload)
  /\ (committedSubjects = {} \/ subject \in committedSubjects
      \/ BugAllowConflictingFinality)
  /\ committedSubjects' = committedSubjects \cup {subject}
  /\ pendingSubject' = "None"
  /\ fetchRequested' = FALSE
  /\ UNCHANGED <<
      qcsObserved,
      matchingPayloads,
      mismatchedPayloads,
      rejectedMismatches
     >>

StableCommitted ==
  /\ pendingSubject = "None"
  /\ committedSubjects # {}
  /\ UNCHANGED vars

Next ==
  \/ \E subject \in Subjects: ObserveCommitQc(subject)
  \/ \E response \in Subjects: ReceivePayload(response)
  \/ \E response \in Subjects: RejectMismatchedPayload(response)
  \/ \E subject \in Subjects: CommitFinality(subject)
  \/ StableCommitted

PendingFinalityRequiresCommitQc ==
  pendingSubject = "None" \/ pendingSubject \in qcsObserved

CommitRequiresCommitQc ==
  committedSubjects \subseteq qcsObserved

NoCommitWithoutPayload ==
  committedSubjects \subseteq AcceptedPayloads

CommitRequiresMatchingPayload ==
  committedSubjects \subseteq matchingPayloads

NoMismatchedPayloadAccepted ==
  mismatchedPayloads = {}

NoConflictingFinality ==
  Cardinality(committedSubjects) <= 1

====
