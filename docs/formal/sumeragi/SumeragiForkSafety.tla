---- MODULE SumeragiForkSafety ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi same-height fork safety.

The commit-path model in Sumeragi.tla has a single candidate block. This model
adds two conflicting branches at the same height so the formal checks can state
the core finality property directly: honest single-vote discipline plus the
locked-QC gate and quorum intersection prevent two conflicting commit
certificates from forming at one height.

The model abstracts signatures and aggregate validation into finite signer
sets. Byzantine validators may sign both branches. Honest validators may not
sign both branches unless BugDisableSingleVote is enabled. The locked-QC gate
prevents a conflicting same-height lock from replacing an existing one unless
BugDisableLockedQcGate is enabled.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  N,
  \* @type: Int;
  F,
  \* @type: Int;
  HonestCount,
  \* @type: Int;
  CommitQuorum,
  \* @type: Bool;
  UseStakeQuorum,
  \* @type: Int;
  StakeQuorum,
  \* @type: Int;
  HonestStakeWeight,
  \* @type: Int;
  ByzantineStakeWeight,
  \* @type: Int;
  MaxView,
  \* @type: Bool;
  BugDisableSingleVote,
  \* @type: Bool;
  BugDisableLockedQcGate

VARIABLES
  \* @type: Int;
  view,
  \* @type: Str;
  lockedBranch,
  \* @type: Int;
  lockView,
  \* @type: Set(Int);
  honestCommitA,
  \* @type: Set(Int);
  honestCommitB,
  \* @type: Set(Int);
  byzCommitA,
  \* @type: Set(Int);
  byzCommitB,
  \* @type: Set(Str);
  commitCerts

vars == <<
  view,
  lockedBranch,
  lockView,
  honestCommitA,
  honestCommitB,
  byzCommitA,
  byzCommitB,
  commitCerts
>>

Branches == {"A", "B"}
LockBranches == Branches \cup {"None"}
Honest == 1..HonestCount
Byzantine == (HonestCount + 1)..N

VotesFor(branch) ==
  IF branch = "A"
  THEN honestCommitA \cup byzCommitA
  ELSE honestCommitB \cup byzCommitB

HonestVotesFor(branch) ==
  IF branch = "A" THEN honestCommitA ELSE honestCommitB

StakeOf(votes) ==
  Cardinality(votes \cap Honest) * HonestStakeWeight
    + Cardinality(votes \cap Byzantine) * ByzantineStakeWeight

CommitCertificateReady(branch) ==
  /\ Cardinality(VotesFor(branch)) >= CommitQuorum
  /\ (~UseStakeQuorum \/ StakeOf(VotesFor(branch)) >= StakeQuorum)

TypeInvariant ==
  /\ N \in Nat
  /\ N > 0
  /\ F \in Nat
  /\ F < N
  /\ HonestCount \in 1..N
  /\ HonestCount + F = N
  /\ CommitQuorum \in 1..N
  /\ 2 * CommitQuorum > N + F
  /\ UseStakeQuorum \in BOOLEAN
  /\ StakeQuorum \in Nat
  /\ HonestStakeWeight \in Nat
  /\ HonestStakeWeight > 0
  /\ ByzantineStakeWeight \in Nat
  /\ MaxView \in Nat
  /\ BugDisableSingleVote \in BOOLEAN
  /\ BugDisableLockedQcGate \in BOOLEAN
  /\ view \in 0..MaxView
  /\ lockedBranch \in LockBranches
  /\ lockView \in 0..MaxView
  /\ honestCommitA \subseteq Honest
  /\ honestCommitB \subseteq Honest
  /\ byzCommitA \subseteq Byzantine
  /\ byzCommitB \subseteq Byzantine
  /\ commitCerts \subseteq Branches
  /\ commitCerts \subseteq {branch \in Branches: CommitCertificateReady(branch)}

Init ==
  /\ view = 0
  /\ lockedBranch = "None"
  /\ lockView = 0
  /\ honestCommitA = {}
  /\ honestCommitB = {}
  /\ byzCommitA = {}
  /\ byzCommitB = {}
  /\ commitCerts = {}

PrepareQc(branch) ==
  /\ branch \in Branches
  /\ (lockedBranch = "None" \/ lockedBranch = branch \/ BugDisableLockedQcGate)
  /\ lockedBranch' = branch
  /\ lockView' = view
  /\ UNCHANGED <<
      honestCommitA,
      honestCommitB,
      byzCommitA,
      byzCommitB,
      commitCerts
     >>
  /\ UNCHANGED view

HonestCommit(branch, signer) ==
  /\ branch \in Branches
  /\ signer \in Honest
  /\ (lockedBranch = branch \/ BugDisableLockedQcGate)
  /\ IF branch = "A"
     THEN
       /\ signer \notin honestCommitA
       /\ BugDisableSingleVote \/ signer \notin honestCommitB
       /\ honestCommitA' = honestCommitA \cup {signer}
       /\ honestCommitB' = honestCommitB
     ELSE
       /\ signer \notin honestCommitB
       /\ BugDisableSingleVote \/ signer \notin honestCommitA
       /\ honestCommitB' = honestCommitB \cup {signer}
       /\ honestCommitA' = honestCommitA
  /\ UNCHANGED <<
      view,
      lockedBranch,
      lockView,
      byzCommitA,
      byzCommitB,
      commitCerts
     >>

ByzantineCommit(branch, signer) ==
  /\ branch \in Branches
  /\ signer \in Byzantine
  /\ IF branch = "A"
     THEN
       /\ signer \notin byzCommitA
       /\ byzCommitA' = byzCommitA \cup {signer}
       /\ byzCommitB' = byzCommitB
     ELSE
       /\ signer \notin byzCommitB
       /\ byzCommitB' = byzCommitB \cup {signer}
       /\ byzCommitA' = byzCommitA
  /\ UNCHANGED <<
      view,
      lockedBranch,
      lockView,
      honestCommitA,
      honestCommitB,
      commitCerts
     >>

FormCommitCertificate(branch) ==
  /\ branch \in Branches
  /\ branch \notin commitCerts
  /\ CommitCertificateReady(branch)
  /\ commitCerts' = commitCerts \cup {branch}
  /\ UNCHANGED <<
      view,
      lockedBranch,
      lockView,
      honestCommitA,
      honestCommitB,
      byzCommitA,
      byzCommitB
     >>

AdvanceView ==
  /\ view < MaxView
  /\ view' = view + 1
  /\ UNCHANGED <<
      lockedBranch,
      lockView,
      honestCommitA,
      honestCommitB,
      byzCommitA,
      byzCommitB,
      commitCerts
     >>

Next ==
  \/ \E branch \in Branches: PrepareQc(branch)
  \/ \E branch \in Branches, signer \in Honest: HonestCommit(branch, signer)
  \/ \E branch \in Branches, signer \in Byzantine: ByzantineCommit(branch, signer)
  \/ \E branch \in Branches: FormCommitCertificate(branch)
  \/ AdvanceView

HonestCommitVotesSingleBranch ==
  BugDisableSingleVote \/ honestCommitA \cap honestCommitB = {}

CommitCertificateImpliesCountQuorum ==
  \A branch \in commitCerts:
    Cardinality(VotesFor(branch)) >= CommitQuorum

CommitCertificateImpliesStakeQuorum ==
  \A branch \in commitCerts:
    (~UseStakeQuorum \/ StakeOf(VotesFor(branch)) >= StakeQuorum)

CommitCertificateImpliesHonestSupport ==
  \A branch \in commitCerts:
    Cardinality(HonestVotesFor(branch)) >= CommitQuorum - F

NoConflictingCommitCertificates ==
  ~("A" \in commitCerts /\ "B" \in commitCerts)

ForkSafetyExactness ==
  /\ HonestCommitVotesSingleBranch
  /\ CommitCertificateImpliesCountQuorum
  /\ CommitCertificateImpliesStakeQuorum
  /\ CommitCertificateImpliesHonestSupport
  /\ NoConflictingCommitCertificates

Safety ==
  ForkSafetyExactness

ForkSafetyCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ForkSafetyExactness

====
