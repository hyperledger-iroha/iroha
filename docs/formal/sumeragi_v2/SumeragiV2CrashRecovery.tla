---- MODULE SumeragiV2CrashRecovery ----
EXTENDS SumeragiV2Core

(***************************************************************************
Crash/recovery obligations for the production reducer/WAL boundary.

The core model treats each Persist* action as acknowledgement of one complete,
fsynced frame.  Requests in pending* are unacknowledged and are erased by a
crash. Durable intent, lock, high-QC, TC, and decision state is unchanged.
Restart denotes a complete process replacement: all old callback senders and
queues are gone before generation zero is installed. Resume* may re-sign only
an already acknowledged current-view intent.

The byte-level frame checksum, hash chain, Norito decode, and OS fsync contract
are outside TLA+.  The predicates below state the logical complete-prefix
contract which the production SafetyWal implementation must refine in Verus.
***************************************************************************)

DurableProjection ==
  [proposalIntents |-> proposalIntents,
   prepareIntents |-> prepareIntents,
   commitIntents |-> commitIntents,
   timeoutIntents |-> timeoutIntents,
   prepareQCs |-> prepareQCs,
   commitQCs |-> commitQCs,
   installedTCs |-> installedTCs,
   lockRank |-> lockRank,
   lockSubject |-> lockSubject,
   highestRank |-> highestRank,
   highestSubject |-> highestSubject,
   decisions |-> decisions,
   durableBodies |-> durableBodies]

DurableProjectionPrime ==
  [proposalIntents |-> proposalIntents',
   prepareIntents |-> prepareIntents',
   commitIntents |-> commitIntents',
   timeoutIntents |-> timeoutIntents',
   prepareQCs |-> prepareQCs',
   commitQCs |-> commitQCs',
   installedTCs |-> installedTCs',
   lockRank |-> lockRank',
   lockSubject |-> lockSubject',
   highestRank |-> highestRank',
   highestSubject |-> highestSubject',
   decisions |-> decisions',
   durableBodies |-> durableBodies']

CrashPreservesDurableProjection ==
  \A node \in ValidatorIds:
    Crash(node) => DurableProjectionPrime = DurableProjection

RestartPreservesDurableProjection ==
  \A node \in ValidatorIds:
    Restart(node) => DurableProjectionPrime = DurableProjection

PendingWritesAreUnacknowledged ==
  \A node \in ValidatorIds:
    Crash(node)
      => /\ ~\E request \in pendingProposal': request.node = node
         /\ ~\E request \in pendingPrepare': request.node = node
         /\ ~\E request \in pendingObservePrepare': request.node = node
         /\ ~\E request \in pendingLockCommit': request.node = node
         /\ ~\E request \in pendingTimeout': request.node = node
         /\ ~\E request \in pendingInstallTC': request.node = node
         /\ ~\E request \in pendingDecision': request.node = node

RestartStartsFreshProcessGeneration ==
  \A node \in ValidatorIds:
    Restart(node) => generation'[node] = 0

Frame(sequence, payload, complete, previousHash, frameHash) ==
  [sequence |-> sequence, payload |-> payload, complete |-> complete,
   previousHash |-> previousHash, frameHash |-> frameHash]

ContiguousCompletePrefix(frames) ==
  /\ \A frame \in frames: frame.sequence \in Nat
  /\ \A left, right \in frames:
       left.sequence = right.sequence => left = right
  /\ \A frame \in frames:
       frame.complete
         => \A prior \in 0..(frame.sequence - 1):
              \E previous \in frames:
                /\ previous.sequence = prior
                /\ previous.complete
  /\ \A frame \in frames:
       ~frame.complete
         => ~\E later \in frames: later.sequence > frame.sequence

AcknowledgedFrames(frames) == {frame \in frames: frame.complete}

IncompleteFinalFrameUnacknowledged(frames) ==
  \A frame \in frames:
    ~frame.complete => frame \notin AcknowledgedFrames(frames)

HashChainWellFormed(frames, zeroHash) ==
  \A frame \in AcknowledgedFrames(frames):
    IF frame.sequence = 0
    THEN frame.previousHash = zeroHash
    ELSE \E previous \in AcknowledgedFrames(frames):
           /\ previous.sequence + 1 = frame.sequence
           /\ previous.frameHash = frame.previousHash

=============================================================================
