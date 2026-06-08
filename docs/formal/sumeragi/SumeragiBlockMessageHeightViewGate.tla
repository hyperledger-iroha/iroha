---- MODULE SumeragiBlockMessageHeightViewGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `Actor::block_message_height_view(...)`.

This helper decides which consensus block messages are subject to future-window
admission checks. Messages with an intrinsic block slot must project the exact
height/view pair from their payload; messages without a meaningful slot must
return `None` so they are not filtered by the future-window guard.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSource == 0
HeaderSource == 1
FieldSource == 2
CertifiedFetchSource == 3
CompactFieldSource == 4
Sources == {NoSource, HeaderSource, FieldSource, CertifiedFetchSource, CompactFieldSource}

NoOrder == 0
HeightViewOrder == 1
ViewHeightOrder == 2
Orders == {NoOrder, HeightViewOrder, ViewHeightOrder}

\* 1 BlockCreated, 2 BlockSyncUpdate, 3 FetchBlockBody,
\* 4 BlockBodyResponse, 5..8 CertifiedBlockFetch variants,
\* 9 ConsensusParams, 10 VrfCommit, 11 VrfReveal, 12 ExecWitness,
\* 13 RbcInitRequest, 14 RbcChunkRequest, 15 RbcInit, 16 RbcChunk,
\* 17 RbcChunkCompact, 18 RbcReady, 19 RbcDeliver, 20 ProposalHint,
\* 21 Proposal, 22 QcVote, 23 Qc, 24 FetchPendingBlock,
\* 25 KuraReplicaAdvert.
Cases == 1..25

NoSlotCases == {2, 9, 10, 11, 24, 25}
CertifiedFetchCases == 5..8
RbcCases == 13..19
ProposalCases == {20, 21}
QcCases == {22, 23}

SpecHasSlot(c) ==
  c \notin NoSlotCases

SpecSource(c) ==
  CASE c \in NoSlotCases -> NoSource
    [] c \in {1, 21} -> HeaderSource
    [] c \in CertifiedFetchCases -> CertifiedFetchSource
    [] c = 17 -> CompactFieldSource
    [] OTHER -> FieldSource

SpecOrder(c) ==
  IF SpecHasSlot(c) THEN HeightViewOrder ELSE NoOrder

SpecWidenCompact(c) ==
  c = 17

ActualHasSlot(c) ==
  CASE Bug = "block_sync_update_has_slot"
       /\ c = 2 -> TRUE
    [] Bug = "consensus_params_has_slot"
       /\ c = 9 -> TRUE
    [] Bug = "vrf_has_slot"
       /\ c \in {10, 11} -> TRUE
    [] Bug = "fetch_pending_has_slot"
       /\ c = 24 -> TRUE
    [] Bug = "kura_advert_has_slot"
       /\ c = 25 -> TRUE
    [] Bug = "drop_block_created_slot"
       /\ c = 1 -> FALSE
    [] Bug = "drop_certified_fetch_slot"
       /\ c \in CertifiedFetchCases -> FALSE
    [] Bug = "drop_rbc_slot"
       /\ c \in RbcCases -> FALSE
    [] Bug = "drop_proposal_slot"
       /\ c \in ProposalCases -> FALSE
    [] Bug = "drop_qc_slot"
       /\ c \in QcCases -> FALSE
    [] OTHER -> SpecHasSlot(c)

ActualSource(c) ==
  CASE Bug = "block_created_uses_field_source"
       /\ c = 1 -> FieldSource
    [] Bug = "proposal_uses_field_source"
       /\ c = 21 -> FieldSource
    [] Bug = "certified_fetch_uses_header_source"
       /\ c \in CertifiedFetchCases -> HeaderSource
    [] Bug = "compact_not_widened"
       /\ c = 17 -> FieldSource
    [] ~ActualHasSlot(c) -> NoSource
    [] OTHER -> SpecSource(c)

ActualOrder(c) ==
  CASE Bug = "swap_height_view"
       /\ ActualHasSlot(c) -> ViewHeightOrder
    [] ActualHasSlot(c) -> HeightViewOrder
    [] OTHER -> NoOrder

ActualWidenCompact(c) ==
  CASE Bug = "compact_not_widened"
       /\ c = 17 -> FALSE
    [] OTHER -> SpecWidenCompact(c)

Matches(c) ==
  /\ ActualHasSlot(c) = SpecHasSlot(c)
  /\ ActualSource(c) = SpecSource(c)
  /\ ActualOrder(c) = SpecOrder(c)
  /\ ActualWidenCompact(c) = SpecWidenCompact(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "block_sync_update_has_slot",
       "consensus_params_has_slot",
       "vrf_has_slot",
       "fetch_pending_has_slot",
       "kura_advert_has_slot",
       "drop_block_created_slot",
       "drop_certified_fetch_slot",
       "drop_rbc_slot",
       "drop_proposal_slot",
       "drop_qc_slot",
       "block_created_uses_field_source",
       "proposal_uses_field_source",
       "certified_fetch_uses_header_source",
       "compact_not_widened",
       "swap_height_view"
     }
  /\ checked \in 0..25
  /\ \A c \in Cases:
       /\ SpecSource(c) \in Sources
       /\ ActualSource(c) \in Sources
       /\ SpecOrder(c) \in Orders
       /\ ActualOrder(c) \in Orders
       /\ SpecWidenCompact(c) \in BOOLEAN
       /\ ActualWidenCompact(c) \in BOOLEAN

SafetyFast ==
  \A c \in Cases: Matches(c)

NoSlotMessagesStayUnfiltered ==
  \A c \in NoSlotCases:
    ~ActualHasSlot(c)

SlotMessagesRemainFutureWindowEligible ==
  \A c \in Cases \ NoSlotCases:
    ActualHasSlot(c)

HeightViewOrderPreserved ==
  \A c \in Cases:
    ActualOrder(c) = SpecOrder(c)

ProjectionSourcePreserved ==
  \A c \in Cases:
    ActualSource(c) = SpecSource(c)

CompactChunkWidensSlot ==
  ActualWidenCompact(17)

NoSlotProjectionAnchors ==
  /\ NoSlotMessagesStayUnfiltered
  /\ \A c \in NoSlotCases:
       /\ ActualSource(c) = NoSource
       /\ ActualOrder(c) = NoOrder

SlotProjectionAnchors ==
  /\ SlotMessagesRemainFutureWindowEligible
  /\ ProjectionSourcePreserved
  /\ HeightViewOrderPreserved

SourceSelectionAnchors ==
  /\ \A c \in {1, 21}: ActualSource(c) = HeaderSource
  /\ \A c \in CertifiedFetchCases:
       ActualSource(c) = CertifiedFetchSource
  /\ ActualSource(17) = CompactFieldSource
  /\ \A c \in RbcCases \ {17}: ActualSource(c) = FieldSource
  /\ \A c \in ProposalCases \ {21}: ActualSource(c) = FieldSource
  /\ \A c \in QcCases: ActualSource(c) = FieldSource

CompactWideningAnchors ==
  /\ CompactChunkWidensSlot
  /\ ActualHasSlot(17)
  /\ ActualSource(17) = CompactFieldSource
  /\ ActualOrder(17) = HeightViewOrder

BlockMessageHeightViewSafetyAnchors ==
  /\ SafetyFast
  /\ NoSlotProjectionAnchors
  /\ SlotProjectionAnchors
  /\ SourceSelectionAnchors
  /\ CompactWideningAnchors

BlockMessageNoSlotExactness ==
  /\ NoSlotMessagesStayUnfiltered
  /\ NoSlotProjectionAnchors

BlockMessageSlotProjectionExactness ==
  /\ SlotMessagesRemainFutureWindowEligible
  /\ ProjectionSourcePreserved
  /\ HeightViewOrderPreserved
  /\ SlotProjectionAnchors

BlockMessageSourceSelectionExactness ==
  /\ ProjectionSourcePreserved
  /\ SourceSelectionAnchors

BlockMessageCompactWideningExactness ==
  /\ CompactChunkWidensSlot
  /\ CompactWideningAnchors

BlockMessageHeightViewExactness ==
  /\ SafetyFast
  /\ BlockMessageNoSlotExactness
  /\ BlockMessageSlotProjectionExactness
  /\ BlockMessageSourceSelectionExactness
  /\ BlockMessageCompactWideningExactness
  /\ BlockMessageHeightViewSafetyAnchors

Safety == BlockMessageHeightViewExactness

====
