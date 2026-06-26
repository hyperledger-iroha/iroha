---- MODULE SumeragiBlockMessageKindGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `Actor::block_message_kind(...)` and
`Actor::block_message_status_kind(...)`.

The log/future-window kind keeps some distinctions that telemetry deliberately
collapses: certified-fetch subtypes are separate log labels, QC votes and QCs
split by phase so NewView traffic bypasses future-view filtering, and compact
RBC chunks share the `RbcChunk` label. Status telemetry collapses certified
fetch, QC phases, and compact/full RBC chunks, and omits Kura replica adverts.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

\* 1 BlockCreated, 2 BlockSyncUpdate, 3 FetchBlockBody,
\* 4 BlockBodyResponse, 5..8 CertifiedBlockFetch variants,
\* 9 ConsensusParams, 10..12 QcVote phases, 13..15 Qc phases,
\* 16 VrfCommit, 17 VrfReveal, 18 ExecWitness, 19 RbcInitRequest,
\* 20 RbcChunkRequest, 21 RbcInit, 22 RbcChunk, 23 RbcChunkCompact,
\* 24 RbcReady, 25 RbcDeliver, 26 FetchPendingBlock,
\* 27 KuraReplicaAdvert, 28 ProposalHint, 29 Proposal.
Cases == 1..29
CertifiedFetchCases == 5..8
QcVoteCases == {10, 11, 12}
QcCases == {13, 14, 15}

BlockCreatedLabel == 1
BlockSyncUpdateLabel == 2
FetchBlockBodyLabel == 3
BlockBodyResponseLabel == 4
CertifiedBlockFetchRequestLabel == 5
CertifiedBlockFetchResponseLabel == 6
CertifiedBlockFetchProofLabel == 7
CertifiedBlockFetchBodyLabel == 8
ConsensusParamsLabel == 9
PrepareVoteLabel == 10
QcVoteLabel == 11
NewViewVoteLabel == 12
PrepareCertLabel == 13
CommitCertLabel == 14
NewViewCertLabel == 15
VrfCommitLabel == 16
VrfRevealLabel == 17
ExecWitnessLabel == 18
RbcInitRequestLabel == 19
RbcChunkRequestLabel == 20
RbcInitLabel == 21
RbcChunkLabel == 22
RbcChunkCompactDistinctLabel == 23
RbcReadyLabel == 24
RbcDeliverLabel == 25
FetchPendingBlockLabel == 26
KuraReplicaAdvertLabel == 27
ProposalHintLabel == 28
ProposalLabel == 29
CertifiedBlockFetchCollapsedLabel == 30
LogLabels == 1..30

NoStatus == 0
StatusBlockCreated == 1
StatusBlockSyncUpdate == 2
StatusFetchBlockBody == 3
StatusBlockBodyResponse == 4
StatusCertifiedBlockFetch == 5
StatusConsensusParams == 6
StatusProposalHint == 7
StatusProposal == 8
StatusQcVote == 9
StatusQc == 10
StatusVrfCommit == 11
StatusVrfReveal == 12
StatusExecWitness == 13
StatusRbcInitRequest == 14
StatusRbcChunkRequest == 15
StatusRbcInit == 16
StatusRbcChunk == 17
StatusRbcReady == 18
StatusRbcDeliver == 19
StatusFetchPendingBlock == 20
StatusKuraReplicaAdvert == 21
StatusCertifiedBlockFetchResponse == 22
StatusRbcChunkCompact == 23
StatusPrepareVote == 24
StatusNewViewVote == 25
StatusPrepareCert == 26
StatusNewViewCert == 27
StatusLabels == 0..27

SpecLogLabel(c) ==
  CASE c = 1 -> BlockCreatedLabel
    [] c = 2 -> BlockSyncUpdateLabel
    [] c = 3 -> FetchBlockBodyLabel
    [] c = 4 -> BlockBodyResponseLabel
    [] c = 5 -> CertifiedBlockFetchRequestLabel
    [] c = 6 -> CertifiedBlockFetchResponseLabel
    [] c = 7 -> CertifiedBlockFetchProofLabel
    [] c = 8 -> CertifiedBlockFetchBodyLabel
    [] c = 9 -> ConsensusParamsLabel
    [] c = 10 -> PrepareVoteLabel
    [] c = 11 -> QcVoteLabel
    [] c = 12 -> NewViewVoteLabel
    [] c = 13 -> PrepareCertLabel
    [] c = 14 -> CommitCertLabel
    [] c = 15 -> NewViewCertLabel
    [] c = 16 -> VrfCommitLabel
    [] c = 17 -> VrfRevealLabel
    [] c = 18 -> ExecWitnessLabel
    [] c = 19 -> RbcInitRequestLabel
    [] c = 20 -> RbcChunkRequestLabel
    [] c = 21 -> RbcInitLabel
    [] c \in {22, 23} -> RbcChunkLabel
    [] c = 24 -> RbcReadyLabel
    [] c = 25 -> RbcDeliverLabel
    [] c = 26 -> FetchPendingBlockLabel
    [] c = 27 -> KuraReplicaAdvertLabel
    [] c = 28 -> ProposalHintLabel
    [] c = 29 -> ProposalLabel

ActualLogLabel(c) ==
  CASE Bug = "collapse_certified_fetch_labels"
       /\ c \in CertifiedFetchCases -> CertifiedBlockFetchCollapsedLabel
    [] Bug = "swap_certified_fetch_response_proof"
       /\ c = 6 -> CertifiedBlockFetchProofLabel
    [] Bug = "swap_certified_fetch_response_proof"
       /\ c = 7 -> CertifiedBlockFetchResponseLabel
    [] Bug = "prepare_vote_uses_commit_label"
       /\ c = 10 -> QcVoteLabel
    [] Bug = "new_view_vote_uses_commit_label"
       /\ c = 12 -> QcVoteLabel
    [] Bug = "prepare_cert_uses_commit_label"
       /\ c = 13 -> CommitCertLabel
    [] Bug = "new_view_cert_uses_commit_label"
       /\ c = 15 -> CommitCertLabel
    [] Bug = "compact_label_distinct"
       /\ c = 23 -> RbcChunkCompactDistinctLabel
    [] Bug = "proposal_hint_uses_proposal_label"
       /\ c = 28 -> ProposalLabel
    [] Bug = "block_body_response_uses_fetch_label"
       /\ c = 4 -> FetchBlockBodyLabel
    [] Bug = "vrf_reveal_uses_commit_label"
       /\ c = 17 -> VrfCommitLabel
    [] Bug = "rbc_ready_uses_deliver_label"
       /\ c = 24 -> RbcDeliverLabel
    [] OTHER -> SpecLogLabel(c)

SpecStatusKind(c) ==
  CASE c = 1 -> StatusBlockCreated
    [] c = 2 -> StatusBlockSyncUpdate
    [] c = 3 -> StatusFetchBlockBody
    [] c = 4 -> StatusBlockBodyResponse
    [] c \in CertifiedFetchCases -> StatusCertifiedBlockFetch
    [] c = 9 -> StatusConsensusParams
    [] c = 28 -> StatusProposalHint
    [] c = 29 -> StatusProposal
    [] c \in QcVoteCases -> StatusQcVote
    [] c \in QcCases -> StatusQc
    [] c = 16 -> StatusVrfCommit
    [] c = 17 -> StatusVrfReveal
    [] c = 18 -> StatusExecWitness
    [] c = 19 -> StatusRbcInitRequest
    [] c = 20 -> StatusRbcChunkRequest
    [] c = 21 -> StatusRbcInit
    [] c \in {22, 23} -> StatusRbcChunk
    [] c = 24 -> StatusRbcReady
    [] c = 25 -> StatusRbcDeliver
    [] c = 26 -> StatusFetchPendingBlock
    [] c = 27 -> NoStatus

ActualStatusKind(c) ==
  CASE Bug = "kura_has_status"
       /\ c = 27 -> StatusKuraReplicaAdvert
    [] Bug = "fetch_pending_missing_status"
       /\ c = 26 -> NoStatus
    [] Bug = "certified_status_split"
       /\ c \in {6, 7, 8} -> StatusCertifiedBlockFetchResponse
    [] Bug = "compact_status_distinct"
       /\ c = 23 -> StatusRbcChunkCompact
    [] Bug = "qcvote_status_phase_split"
       /\ c = 10 -> StatusPrepareVote
    [] Bug = "qcvote_status_phase_split"
       /\ c = 12 -> StatusNewViewVote
    [] Bug = "qc_status_phase_split"
       /\ c = 13 -> StatusPrepareCert
    [] Bug = "qc_status_phase_split"
       /\ c = 15 -> StatusNewViewCert
    [] OTHER -> SpecStatusKind(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 29
     /\ checked' = checked + 1
  \/ /\ checked = 29
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "collapse_certified_fetch_labels",
       "swap_certified_fetch_response_proof",
       "prepare_vote_uses_commit_label",
       "new_view_vote_uses_commit_label",
       "prepare_cert_uses_commit_label",
       "new_view_cert_uses_commit_label",
       "compact_label_distinct",
       "proposal_hint_uses_proposal_label",
       "block_body_response_uses_fetch_label",
       "vrf_reveal_uses_commit_label",
       "rbc_ready_uses_deliver_label",
       "kura_has_status",
       "fetch_pending_missing_status",
       "certified_status_split",
       "compact_status_distinct",
       "qcvote_status_phase_split",
       "qc_status_phase_split"
     }
  /\ checked \in 0..29
  /\ \A c \in Cases:
       /\ SpecLogLabel(c) \in LogLabels
       /\ ActualLogLabel(c) \in LogLabels
       /\ SpecStatusKind(c) \in StatusLabels
       /\ ActualStatusKind(c) \in StatusLabels

LogLabelsExact ==
  \A c \in Cases:
    ActualLogLabel(c) = SpecLogLabel(c)

StatusProjectionExact ==
  \A c \in Cases:
    ActualStatusKind(c) = SpecStatusKind(c)

NewViewBypassLabelsPreserved ==
  /\ ActualLogLabel(12) = NewViewVoteLabel
  /\ ActualLogLabel(15) = NewViewCertLabel

CertifiedFetchLogSubtypesPreserved ==
  /\ ActualLogLabel(5) = CertifiedBlockFetchRequestLabel
  /\ ActualLogLabel(6) = CertifiedBlockFetchResponseLabel
  /\ ActualLogLabel(7) = CertifiedBlockFetchProofLabel
  /\ ActualLogLabel(8) = CertifiedBlockFetchBodyLabel

CompactKindCollapsesToRbcChunk ==
  /\ ActualLogLabel(23) = RbcChunkLabel
  /\ ActualStatusKind(23) = StatusRbcChunk

KuraReplicaAdvertStatusOmitted ==
  ActualStatusKind(27) = NoStatus

LogKindAnchors ==
  /\ LogLabelsExact
  /\ CertifiedFetchLogSubtypesPreserved
  /\ NewViewBypassLabelsPreserved
  /\ ActualLogLabel(10) = PrepareVoteLabel
  /\ ActualLogLabel(13) = PrepareCertLabel
  /\ ActualLogLabel(22) = RbcChunkLabel
  /\ ActualLogLabel(23) = RbcChunkLabel
  /\ ActualLogLabel(28) = ProposalHintLabel

StatusKindAnchors ==
  /\ StatusProjectionExact
  /\ KuraReplicaAdvertStatusOmitted
  /\ ActualStatusKind(5) = StatusCertifiedBlockFetch
  /\ ActualStatusKind(6) = StatusCertifiedBlockFetch
  /\ ActualStatusKind(10) = StatusQcVote
  /\ ActualStatusKind(12) = StatusQcVote
  /\ ActualStatusKind(13) = StatusQc
  /\ ActualStatusKind(15) = StatusQc
  /\ ActualStatusKind(22) = StatusRbcChunk
  /\ ActualStatusKind(23) = StatusRbcChunk
  /\ ActualStatusKind(26) = StatusFetchPendingBlock

BlockMessageKindSafetyAnchors ==
  /\ LogKindAnchors
  /\ StatusKindAnchors
  /\ CompactKindCollapsesToRbcChunk

SafetyFast ==
  /\ LogLabelsExact
  /\ StatusProjectionExact
  /\ NewViewBypassLabelsPreserved
  /\ CertifiedFetchLogSubtypesPreserved
  /\ CompactKindCollapsesToRbcChunk
  /\ KuraReplicaAdvertStatusOmitted

BlockMessageLogProjectionExactness ==
  /\ LogLabelsExact
  /\ CertifiedFetchLogSubtypesPreserved
  /\ NewViewBypassLabelsPreserved
  /\ LogKindAnchors

BlockMessageStatusProjectionExactness ==
  /\ StatusProjectionExact
  /\ KuraReplicaAdvertStatusOmitted
  /\ StatusKindAnchors

BlockMessageCompactRbcExactness ==
  /\ CompactKindCollapsesToRbcChunk

BlockMessageKindExactness ==
  /\ SafetyFast
  /\ BlockMessageLogProjectionExactness
  /\ BlockMessageStatusProjectionExactness
  /\ BlockMessageCompactRbcExactness
  /\ BlockMessageKindSafetyAnchors

Safety == BlockMessageKindExactness

BlockMessageKindCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ BlockMessageKindSafetyAnchors

====
