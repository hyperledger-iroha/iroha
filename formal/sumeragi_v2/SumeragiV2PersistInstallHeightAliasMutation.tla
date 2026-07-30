---- MODULE SumeragiV2PersistInstallHeightAliasMutation ----
EXTENDS Naturals, FiniteSets, TLC

(***************************************************************************
Bounded regression for the redundant-height alias in PersistInstallTC.

VoteRecordSet deliberately represents structurally valid wire records, so its
redundant `height` field can differ from `context.height`.  The retired
field-by-field locked-Commit selector ignored that field.  Two otherwise
identical durable Commit intents therefore produced two distinct VoteSign
owners for one node when InstallTC rebuilt the active locked vote.

The repaired selector compares the complete canonical Vote(...) value.  Its
bounded initial relation enumerates every subset of the canonical vote and
its height-only alias, validating both the empty and singleton outcomes.
***************************************************************************)

BusyNode == "Node"
LockedSubject == "Block"
LockedView == 0
AliasHeight == 1
CurrentContext == [height |-> 0]

Vote(voteContext, roundView, phaseName, subject, signer) ==
  [context |-> voteContext, height |-> voteContext.height,
   view |-> roundView, phase |-> phaseName, subject |-> subject,
   signer |-> signer]

VoteSign(node, vote) ==
  [node |-> node, kind |-> "Vote", vote |-> vote]

CanonicalCommitVote ==
  Vote(CurrentContext, LockedView, "Commit", LockedSubject, BusyNode)

HeightAliasCommitVote ==
  [CanonicalCommitVote EXCEPT !.height = AliasHeight]

VoteUniverse == {CanonicalCommitVote, HeightAliasCommitVote}

InstallOwner == [node |-> BusyNode, kind |-> "InstallTC"]

VARIABLES commitIntents, pendingInstall, signVotes, signOwnerCount, phase

vars ==
  <<commitIntents, pendingInstall, signVotes, signOwnerCount, phase>>

PendingInstallOwners ==
  IF pendingInstall THEN {InstallOwner} ELSE {}

SerializedBusyOwners == PendingInstallOwners \cup signVotes

RequestsUniqueByNode(requests) ==
  \A left, right \in requests:
    left.node = right.node => left = right

SerializedBusyOwnershipInvariant ==
  RequestsUniqueByNode(SerializedBusyOwners)

(***************************************************************************
Retired selector: all semantic Commit fields are compared except the
redundant height.  Both values in VoteUniverse therefore match.
***************************************************************************)
LooseExactLockedCommitIntents ==
  {vote \in commitIntents:
    /\ vote.signer = BusyNode
    /\ vote.context = CurrentContext
    /\ vote.phase = "Commit"
    /\ vote.view = LockedView
    /\ vote.subject = LockedSubject}

LooseActiveSignRequests ==
  {VoteSign(BusyNode, vote):
    vote \in LooseExactLockedCommitIntents}

LooseInit ==
  /\ commitIntents = VoteUniverse
  /\ pendingInstall = TRUE
  /\ signVotes = {}
  /\ signOwnerCount = 0
  /\ phase = "BeforeInstall"

LoosePersistInstallTC ==
  /\ phase = "BeforeInstall"
  /\ pendingInstall
  /\ pendingInstall' = FALSE
  /\ signVotes' = LooseActiveSignRequests
  /\ signOwnerCount' = Cardinality(LooseActiveSignRequests)
  /\ phase' = "AfterInstall"
  /\ UNCHANGED commitIntents

LooseNext == LoosePersistInstallTC

LooseSpec == LooseInit /\ [][LooseNext]_vars

(***************************************************************************
Repaired selector: equality with the full constructor includes
height = context.height.  HeightAliasCommitVote can never match.
***************************************************************************)
CanonicalExactLockedCommitIntents ==
  {vote \in commitIntents:
    vote =
      Vote(CurrentContext, LockedView, "Commit", LockedSubject, BusyNode)}

CanonicalActiveSignRequests ==
  {VoteSign(BusyNode, vote):
    vote \in CanonicalExactLockedCommitIntents}

CanonicalInit ==
  /\ commitIntents \in SUBSET VoteUniverse
  /\ pendingInstall = TRUE
  /\ signVotes = {}
  /\ signOwnerCount = 0
  /\ phase = "BeforeInstall"

CanonicalPersistInstallTC ==
  /\ phase = "BeforeInstall"
  /\ pendingInstall
  /\ pendingInstall' = FALSE
  /\ signVotes' = CanonicalActiveSignRequests
  /\ signOwnerCount' = Cardinality(CanonicalActiveSignRequests)
  /\ phase' = "AfterInstall"
  /\ UNCHANGED commitIntents

CanonicalNext == CanonicalPersistInstallTC

CanonicalSpec == CanonicalInit /\ [][CanonicalNext]_vars

CanonicalFullVoteSelection ==
  /\ CanonicalExactLockedCommitIntents
       \subseteq {CanonicalCommitVote}
  /\ \/ CanonicalExactLockedCommitIntents = {}
     \/ CanonicalExactLockedCommitIntents = {CanonicalCommitVote}

CanonicalSerializedOwnership ==
  /\ SerializedBusyOwnershipInvariant
  /\ signOwnerCount = Cardinality(signVotes)
  /\ signOwnerCount \in 0..1

CanonicalSignReadiness ==
  \A request \in signVotes:
    /\ request.node = BusyNode
    /\ request.kind = "Vote"
    /\ request.vote = CanonicalCommitVote
    /\ request.vote \in commitIntents

CanonicalStateExact ==
  /\ phase \in {"BeforeInstall", "AfterInstall"}
  /\ (phase = "BeforeInstall"
        => /\ pendingInstall
           /\ signVotes = {}
           /\ signOwnerCount = 0)
  /\ (phase = "AfterInstall"
        => /\ ~pendingInstall
           /\ signVotes = CanonicalActiveSignRequests
           /\ signOwnerCount = Cardinality(CanonicalActiveSignRequests))

=============================================================================
