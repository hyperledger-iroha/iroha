---- MODULE SumeragiBlockMessagePriorityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `BlockMessage::priority()`.

All consensus block-message variants must stay at high network priority. Votes,
QCs, proposals, RBC chunks, block-sync repair traffic, VRF material, consensus
parameter adverts, execution witnesses, and body-fetch messages all sit on the
consensus critical path. Downgrading any variant can starve consensus progress
behind lower-priority gossip.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == 0
DowngradeBlockCreated == 1
DowngradeBlockSyncUpdate == 2
DowngradeFetchBlockBody == 3
DowngradeBlockBodyResponse == 4
DowngradeCertifiedBlockFetch == 5
DowngradeConsensusParams == 6
DowngradeVrfCommit == 7
DowngradeVrfReveal == 8
DowngradeExecWitness == 9
DowngradeRbcInitRequest == 10
DowngradeRbcChunkRequest == 11
DowngradeRbcInit == 12
DowngradeRbcChunk == 13
DowngradeRbcChunkCompact == 14
DowngradeRbcReady == 15
DowngradeRbcDeliver == 16
DowngradeFetchPendingBlock == 17
DowngradeKuraReplicaAdvert == 18
DowngradeProposalHint == 19
DowngradeProposal == 20
DowngradeQcVote == 21
DowngradeQc == 22

Bugs == 0..22

BlockCreatedHigh == Bug # DowngradeBlockCreated
BlockSyncUpdateHigh == Bug # DowngradeBlockSyncUpdate
FetchBlockBodyHigh == Bug # DowngradeFetchBlockBody
BlockBodyResponseHigh == Bug # DowngradeBlockBodyResponse
CertifiedBlockFetchHigh == Bug # DowngradeCertifiedBlockFetch
ConsensusParamsHigh == Bug # DowngradeConsensusParams
VrfCommitHigh == Bug # DowngradeVrfCommit
VrfRevealHigh == Bug # DowngradeVrfReveal
ExecWitnessHigh == Bug # DowngradeExecWitness
RbcInitRequestHigh == Bug # DowngradeRbcInitRequest
RbcChunkRequestHigh == Bug # DowngradeRbcChunkRequest
RbcInitHigh == Bug # DowngradeRbcInit
RbcChunkHigh == Bug # DowngradeRbcChunk
RbcChunkCompactHigh == Bug # DowngradeRbcChunkCompact
RbcReadyHigh == Bug # DowngradeRbcReady
RbcDeliverHigh == Bug # DowngradeRbcDeliver
FetchPendingBlockHigh == Bug # DowngradeFetchPendingBlock
KuraReplicaAdvertHigh == Bug # DowngradeKuraReplicaAdvert
ProposalHintHigh == Bug # DowngradeProposalHint
ProposalHigh == Bug # DowngradeProposal
QcVoteHigh == Bug # DowngradeQcVote
QcHigh == Bug # DowngradeQc

Init ==
  checked = 0

Next ==
  \/ /\ checked < 22
     /\ checked' = checked + 1
  \/ /\ checked = 22
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..22

BlockSyncPrioritySafety ==
  /\ BlockCreatedHigh
  /\ BlockSyncUpdateHigh
  /\ FetchBlockBodyHigh
  /\ BlockBodyResponseHigh
  /\ CertifiedBlockFetchHigh
  /\ FetchPendingBlockHigh
  /\ KuraReplicaAdvertHigh

VrfAndExecPrioritySafety ==
  /\ ConsensusParamsHigh
  /\ VrfCommitHigh
  /\ VrfRevealHigh
  /\ ExecWitnessHigh

RbcPrioritySafety ==
  /\ RbcInitRequestHigh
  /\ RbcChunkRequestHigh
  /\ RbcInitHigh
  /\ RbcChunkHigh
  /\ RbcChunkCompactHigh
  /\ RbcReadyHigh
  /\ RbcDeliverHigh

ProposalAndCertificatePrioritySafety ==
  /\ ProposalHintHigh
  /\ ProposalHigh
  /\ QcVoteHigh
  /\ QcHigh

BlockMessagePrioritySafetyAnchors ==
  /\ BlockSyncPrioritySafety
  /\ VrfAndExecPrioritySafety
  /\ RbcPrioritySafety
  /\ ProposalAndCertificatePrioritySafety

BlockMessageSyncPriorityExactness ==
  /\ BlockSyncPrioritySafety

BlockMessageVrfAndExecPriorityExactness ==
  /\ VrfAndExecPrioritySafety

BlockMessageRbcPriorityExactness ==
  /\ RbcPrioritySafety

BlockMessageProposalAndCertificatePriorityExactness ==
  /\ ProposalAndCertificatePrioritySafety

BlockMessagePriorityExactness ==
  /\ BlockSyncPrioritySafety
  /\ VrfAndExecPrioritySafety
  /\ RbcPrioritySafety
  /\ ProposalAndCertificatePrioritySafety
  /\ BlockMessagePrioritySafetyAnchors

Safety == BlockMessagePriorityExactness

SafetyFast == Safety

BlockMessagePriorityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockMessagePriorityExactness

====
