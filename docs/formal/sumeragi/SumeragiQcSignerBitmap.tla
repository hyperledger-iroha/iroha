---- MODULE SumeragiQcSignerBitmap ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi QC signer bitmap admission.

QC aggregates encode signer indices in a bitmap. The parser must reject bitmap
length mismatches and bits outside the topology. Quorum accounting must count
only indices inside the voting validator set; observer/padding indices may be
present in the topology but cannot satisfy quorum.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxVoting,
  \* @type: Int;
  MaxObservers,
  \* @type: Int;
  MaxBitmapLen,
  \* @type: Bool;
  BugCountObservers,
  \* @type: Bool;
  BugIgnoreBitmapLength,
  \* @type: Bool;
  BugIgnoreOutOfBounds,
  \* @type: Bool;
  BugUnderQuorumAccept

VARIABLES
  \* @type: Int;
  votingLen,
  \* @type: Int;
  observerCount,
  \* @type: Int;
  topologyLen,
  \* @type: Int;
  bitmapLen,
  \* @type: Int;
  expectedBitmapLen,
  \* @type: Int;
  votingSigners,
  \* @type: Int;
  observerSigners,
  \* @type: Int;
  outOfBoundsSigners,
  \* @type: Int;
  presentSigners,
  \* @type: Int;
  countedVotingSigners,
  \* @type: Bool;
  parsedOk,
  \* @type: Bool;
  accepted

vars == <<
  votingLen,
  observerCount,
  topologyLen,
  bitmapLen,
  expectedBitmapLen,
  votingSigners,
  observerSigners,
  outOfBoundsSigners,
  presentSigners,
  countedVotingSigners,
  parsedOk,
  accepted
>>

CommitQuorum(n) ==
  IF n <= 3 THEN n ELSE (n * 2) \div 3 + 1

ExpectedBitmapLen(n) ==
  (n + 7) \div 8

TopologyMax ==
  MaxVoting + MaxObservers

BitmapLenValues ==
  0..MaxBitmapLen

ParsedSpec(voters, observers, blen, outOfBounds) ==
  /\ blen = ExpectedBitmapLen(voters + observers)
  /\ outOfBounds = 0

AcceptedSpec(voters, observers, blen, inVoting, inObservers, outOfBounds) ==
  /\ ParsedSpec(voters, observers, blen, outOfBounds)
  /\ inVoting >= CommitQuorum(voters)

CountedVotingPolicy(inVoting, inObservers) ==
  IF BugCountObservers THEN inVoting + inObservers ELSE inVoting

ParsedPolicy(voters, observers, blen, outOfBounds) ==
  /\ BugIgnoreBitmapLength \/ blen = ExpectedBitmapLen(voters + observers)
  /\ BugIgnoreOutOfBounds \/ outOfBounds = 0

AcceptedPolicy(voters, observers, blen, inVoting, inObservers, outOfBounds) ==
  LET counted == CountedVotingPolicy(inVoting, inObservers)
  IN
    /\ ParsedPolicy(voters, observers, blen, outOfBounds)
    /\ IF BugUnderQuorumAccept
       THEN counted + 1 >= CommitQuorum(voters)
       ELSE counted >= CommitQuorum(voters)

TypeInvariant ==
  /\ MaxVoting \in Nat
  /\ MaxVoting >= 4
  /\ MaxObservers \in Nat
  /\ MaxObservers >= 1
  /\ MaxBitmapLen \in Nat
  /\ MaxBitmapLen >= ExpectedBitmapLen(TopologyMax) + 1
  /\ BugCountObservers \in BOOLEAN
  /\ BugIgnoreBitmapLength \in BOOLEAN
  /\ BugIgnoreOutOfBounds \in BOOLEAN
  /\ BugUnderQuorumAccept \in BOOLEAN
  /\ votingLen \in 1..MaxVoting
  /\ observerCount \in 0..MaxObservers
  /\ topologyLen = votingLen + observerCount
  /\ bitmapLen \in BitmapLenValues
  /\ expectedBitmapLen = ExpectedBitmapLen(topologyLen)
  /\ votingSigners \in 0..votingLen
  /\ observerSigners \in 0..observerCount
  /\ outOfBoundsSigners \in 0..1
  /\ presentSigners = votingSigners + observerSigners
  /\ countedVotingSigners \in 0..topologyLen
  /\ parsedOk \in BOOLEAN
  /\ accepted \in BOOLEAN

Init ==
  /\ votingLen = 1
  /\ observerCount = 0
  /\ topologyLen = 1
  /\ bitmapLen = 1
  /\ expectedBitmapLen = 1
  /\ votingSigners = 0
  /\ observerSigners = 0
  /\ outOfBoundsSigners = 0
  /\ presentSigners = 0
  /\ countedVotingSigners = 0
  /\ parsedOk = TRUE
  /\ accepted = FALSE

Evaluate(voters, observers, blen, inVoting, inObservers, outOfBounds) ==
  /\ votingLen' = voters
  /\ observerCount' = observers
  /\ topologyLen' = voters + observers
  /\ bitmapLen' = blen
  /\ expectedBitmapLen' = ExpectedBitmapLen(voters + observers)
  /\ votingSigners' = inVoting
  /\ observerSigners' = inObservers
  /\ outOfBoundsSigners' = outOfBounds
  /\ presentSigners' = inVoting + inObservers
  /\ countedVotingSigners' = CountedVotingPolicy(inVoting, inObservers)
  /\ parsedOk' = ParsedPolicy(voters, observers, blen, outOfBounds)
  /\ accepted' = AcceptedPolicy(voters, observers, blen, inVoting, inObservers, outOfBounds)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E voters \in 1..MaxVoting:
       \E observers \in 0..MaxObservers:
         \E blen \in BitmapLenValues:
           \E inVoting \in 0..voters:
             \E inObservers \in 0..observers:
               \E outOfBounds \in 0..1:
                 Evaluate(voters, observers, blen, inVoting, inObservers, outOfBounds)
  \/ Stable

ExpectedLengthMatchesTopology ==
  expectedBitmapLen = ExpectedBitmapLen(topologyLen)

ParsedMatchesSpec ==
  parsedOk = ParsedSpec(votingLen, observerCount, bitmapLen, outOfBoundsSigners)

VotingCountIgnoresObservers ==
  countedVotingSigners = votingSigners

ObserverPaddingCannotSatisfyQuorum ==
  /\ votingSigners < CommitQuorum(votingLen)
  /\ votingSigners + observerSigners >= CommitQuorum(votingLen)
  => ~accepted

OutOfBoundsRejected ==
  outOfBoundsSigners > 0 => ~parsedOk /\ ~accepted

LengthMismatchRejected ==
  bitmapLen # expectedBitmapLen => ~parsedOk /\ ~accepted

AcceptedMatchesSpec ==
  accepted =
    AcceptedSpec(
      votingLen,
      observerCount,
      bitmapLen,
      votingSigners,
      observerSigners,
      outOfBoundsSigners
    )

QcSignerBitmapAdmissionExactness ==
  /\ ExpectedLengthMatchesTopology
  /\ ParsedMatchesSpec
  /\ VotingCountIgnoresObservers
  /\ ObserverPaddingCannotSatisfyQuorum
  /\ OutOfBoundsRejected
  /\ LengthMismatchRejected
  /\ AcceptedMatchesSpec

====
