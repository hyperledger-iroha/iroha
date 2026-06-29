---- MODULE SumeragiSlotProposalEvidenceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `slot_has_proposal_evidence(height, view)`.

The helper is the proposal duplicate-suppression predicate used by the
pacemaker path. Proposal evidence exists when the exact queried slot has an
authoritative payload, an entry in `proposals_seen`, a cached proposal, an
authoritative frontier owner with matching frontier metadata, or active
frontier owner state. Wrong-slot or incomplete earlier sources must be ignored
and may fall through to later valid evidence sources.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

AuthoritativePayload == "authoritative_payload"
AuthoritativeWrongHeight == "authoritative_wrong_height"
AuthoritativeWrongView == "authoritative_wrong_view"
AuthoritativeWrongHeightWithSeen == "authoritative_wrong_height_with_seen"

ProposalSeen == "proposal_seen"
ProposalSeenWrongHeight == "proposal_seen_wrong_height"
ProposalSeenWrongView == "proposal_seen_wrong_view"
ProposalSeenWrongViewWithCache == "proposal_seen_wrong_view_with_cache"

CachedProposal == "cached_proposal"
CachedProposalWrongHeight == "cached_proposal_wrong_height"
CachedProposalWrongView == "cached_proposal_wrong_view"
CachedProposalWrongViewWithOwner == "cached_proposal_wrong_view_with_owner"

AuthoritativeOwnerInfo == "authoritative_owner_info"
AuthoritativeOwnerNoInfo == "authoritative_owner_no_info"
AuthoritativeOwnerWrongHeight == "authoritative_owner_wrong_height"
AuthoritativeOwnerWrongView == "authoritative_owner_wrong_view"
AuthoritativeOwnerNoInfoWithActive == "authoritative_owner_no_info_with_active"

ActiveFrontierOwner == "active_frontier_owner"
ActiveOwnerWrongHeight == "active_owner_wrong_height"
ActiveOwnerWrongView == "active_owner_wrong_view"

NoEvidence == "no_evidence"

Cases == {
  AuthoritativePayload,
  AuthoritativeWrongHeight,
  AuthoritativeWrongView,
  AuthoritativeWrongHeightWithSeen,
  ProposalSeen,
  ProposalSeenWrongHeight,
  ProposalSeenWrongView,
  ProposalSeenWrongViewWithCache,
  CachedProposal,
  CachedProposalWrongHeight,
  CachedProposalWrongView,
  CachedProposalWrongViewWithOwner,
  AuthoritativeOwnerInfo,
  AuthoritativeOwnerNoInfo,
  AuthoritativeOwnerWrongHeight,
  AuthoritativeOwnerWrongView,
  AuthoritativeOwnerNoInfoWithActive,
  ActiveFrontierOwner,
  ActiveOwnerWrongHeight,
  ActiveOwnerWrongView,
  NoEvidence
}

AuthoritativeAcceptedCases == {AuthoritativePayload}
AuthoritativeWrongSlotCases == {
  AuthoritativeWrongHeight,
  AuthoritativeWrongView,
  AuthoritativeWrongHeightWithSeen
}

SeenAcceptedCases == {
  ProposalSeen,
  AuthoritativeWrongHeightWithSeen
}
SeenWrongSlotCases == {
  ProposalSeenWrongHeight,
  ProposalSeenWrongView,
  ProposalSeenWrongViewWithCache
}

CacheAcceptedCases == {
  CachedProposal,
  ProposalSeenWrongViewWithCache
}
CacheWrongSlotCases == {
  CachedProposalWrongHeight,
  CachedProposalWrongView,
  CachedProposalWrongViewWithOwner
}

OwnerInfoAcceptedCases == {
  AuthoritativeOwnerInfo,
  CachedProposalWrongViewWithOwner
}
OwnerNoInfoCases == {
  AuthoritativeOwnerNoInfo,
  AuthoritativeOwnerNoInfoWithActive
}
OwnerWrongSlotCases == {
  AuthoritativeOwnerWrongHeight,
  AuthoritativeOwnerWrongView
}

ActiveOwnerAcceptedCases == {
  ActiveFrontierOwner,
  AuthoritativeOwnerNoInfoWithActive
}
ActiveOwnerWrongSlotCases == {
  ActiveOwnerWrongHeight,
  ActiveOwnerWrongView
}

AuthoritativeAccepted(c) == c \in AuthoritativeAcceptedCases
SeenAccepted(c) == c \in SeenAcceptedCases
CacheAccepted(c) == c \in CacheAcceptedCases
OwnerInfoAccepted(c) == c \in OwnerInfoAcceptedCases
ActiveOwnerAccepted(c) == c \in ActiveOwnerAcceptedCases

AfterAuthoritative(c) == ~AuthoritativeAccepted(c)
AfterSeen(c) == AfterAuthoritative(c) /\ ~SeenAccepted(c)
AfterCache(c) == AfterSeen(c) /\ ~CacheAccepted(c)
AfterOwner(c) == AfterCache(c) /\ ~OwnerInfoAccepted(c)

SpecResult(c) ==
  AuthoritativeAccepted(c)
    \/ SeenAccepted(c)
    \/ CacheAccepted(c)
    \/ OwnerInfoAccepted(c)
    \/ ActiveOwnerAccepted(c)

ReturnTrue == 1
ReturnFalse == 2
CheckAuthoritativePayload == 3
CheckProposalSeen == 4
CheckProposalCache == 5
CheckAuthoritativeOwner == 6
CheckActiveOwner == 7
AuthoritativePayloadAccepted == 8
AuthoritativeSlotMismatchIgnored == 9
ProposalSeenAccepted == 10
ProposalSeenSlotMismatchIgnored == 11
CachedProposalAccepted == 12
CachedProposalSlotMismatchIgnored == 13
AuthoritativeOwnerInfoAccepted == 14
AuthoritativeOwnerNoInfoIgnored == 15
AuthoritativeOwnerSlotMismatchIgnored == 16
ActiveOwnerAcceptedAction == 17
ActiveOwnerSlotMismatchIgnored == 18

ActionUniverse == 1..18

AuthoritativeAction(c) ==
  CASE AuthoritativeAccepted(c) -> {AuthoritativePayloadAccepted}
    [] c \in AuthoritativeWrongSlotCases -> {AuthoritativeSlotMismatchIgnored}
    [] OTHER -> {}

SeenAction(c) ==
  CASE SeenAccepted(c) -> {ProposalSeenAccepted}
    [] c \in SeenWrongSlotCases -> {ProposalSeenSlotMismatchIgnored}
    [] OTHER -> {}

CacheAction(c) ==
  CASE CacheAccepted(c) -> {CachedProposalAccepted}
    [] c \in CacheWrongSlotCases -> {CachedProposalSlotMismatchIgnored}
    [] OTHER -> {}

OwnerAction(c) ==
  CASE OwnerInfoAccepted(c) -> {AuthoritativeOwnerInfoAccepted}
    [] c \in OwnerNoInfoCases -> {AuthoritativeOwnerNoInfoIgnored}
    [] c \in OwnerWrongSlotCases -> {AuthoritativeOwnerSlotMismatchIgnored}
    [] OTHER -> {}

ActiveAction(c) ==
  CASE ActiveOwnerAccepted(c) -> {ActiveOwnerAcceptedAction}
    [] c \in ActiveOwnerWrongSlotCases -> {ActiveOwnerSlotMismatchIgnored}
    [] OTHER -> {}

SpecActions(c) ==
  {CheckAuthoritativePayload}
    \cup (IF SpecResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})
    \cup AuthoritativeAction(c)
    \cup (IF AfterAuthoritative(c) THEN {CheckProposalSeen} ELSE {})
    \cup (IF AfterAuthoritative(c) THEN SeenAction(c) ELSE {})
    \cup (IF AfterSeen(c) THEN {CheckProposalCache} ELSE {})
    \cup (IF AfterSeen(c) THEN CacheAction(c) ELSE {})
    \cup (IF AfterCache(c) THEN {CheckAuthoritativeOwner} ELSE {})
    \cup (IF AfterCache(c) THEN OwnerAction(c) ELSE {})
    \cup (IF AfterOwner(c) THEN {CheckActiveOwner} ELSE {})
    \cup (IF AfterOwner(c) THEN ActiveAction(c) ELSE {})

RejectAtCurrentStage(spec, acceptedAction) ==
  (spec \ {ReturnTrue, acceptedAction}) \cup {ReturnFalse}

AcceptAtAuthoritative(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckProposalSeen, CheckProposalCache,
           CheckAuthoritativeOwner, CheckActiveOwner}) \cup
    {ReturnTrue, AuthoritativePayloadAccepted}

AcceptAtSeen(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckProposalCache,
           CheckAuthoritativeOwner, CheckActiveOwner}) \cup
    {ReturnTrue, ProposalSeenAccepted}

AcceptAtCache(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckAuthoritativeOwner,
           CheckActiveOwner}) \cup {ReturnTrue, CachedProposalAccepted}

AcceptAtOwner(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckActiveOwner}) \cup
    {ReturnTrue, AuthoritativeOwnerInfoAccepted}

AcceptAtActive(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction}) \cup
    {ReturnTrue, ActiveOwnerAcceptedAction}

BlockFallbackAfterAuthoritative(spec) ==
  (spec \ {ReturnTrue, ProposalSeenAccepted, CheckProposalSeen,
           CheckProposalCache, CheckAuthoritativeOwner, CheckActiveOwner}) \cup
    {ReturnFalse}

BlockFallbackAfterSeen(spec) ==
  (spec \ {ReturnTrue, CachedProposalAccepted, CheckProposalCache,
           CheckAuthoritativeOwner, CheckActiveOwner}) \cup {ReturnFalse}

BlockFallbackAfterCache(spec) ==
  (spec \ {ReturnTrue, AuthoritativeOwnerInfoAccepted,
           CheckAuthoritativeOwner, CheckActiveOwner}) \cup {ReturnFalse}

BlockFallbackAfterOwner(spec) ==
  (spec \ {ReturnTrue, ActiveOwnerAcceptedAction, CheckActiveOwner}) \cup
    {ReturnFalse}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "reject_authoritative_payload"
       /\ c = AuthoritativePayload ->
      RejectAtCurrentStage(spec, AuthoritativePayloadAccepted)
    [] Bug = "accept_authoritative_wrong_height"
       /\ c = AuthoritativeWrongHeight ->
      AcceptAtAuthoritative(spec, AuthoritativeSlotMismatchIgnored)
    [] Bug = "accept_authoritative_wrong_view"
       /\ c = AuthoritativeWrongView ->
      AcceptAtAuthoritative(spec, AuthoritativeSlotMismatchIgnored)
    [] Bug = "authoritative_mismatch_blocks_seen"
       /\ c = AuthoritativeWrongHeightWithSeen ->
      BlockFallbackAfterAuthoritative(spec)
    [] Bug = "reject_proposal_seen"
       /\ c = ProposalSeen ->
      RejectAtCurrentStage(spec, ProposalSeenAccepted)
    [] Bug = "accept_seen_wrong_height"
       /\ c = ProposalSeenWrongHeight ->
      AcceptAtSeen(spec, ProposalSeenSlotMismatchIgnored)
    [] Bug = "accept_seen_wrong_view"
       /\ c = ProposalSeenWrongView ->
      AcceptAtSeen(spec, ProposalSeenSlotMismatchIgnored)
    [] Bug = "seen_mismatch_blocks_cache"
       /\ c = ProposalSeenWrongViewWithCache ->
      BlockFallbackAfterSeen(spec)
    [] Bug = "reject_cached_proposal"
       /\ c = CachedProposal ->
      RejectAtCurrentStage(spec, CachedProposalAccepted)
    [] Bug = "accept_cache_wrong_height"
       /\ c = CachedProposalWrongHeight ->
      AcceptAtCache(spec, CachedProposalSlotMismatchIgnored)
    [] Bug = "accept_cache_wrong_view"
       /\ c = CachedProposalWrongView ->
      AcceptAtCache(spec, CachedProposalSlotMismatchIgnored)
    [] Bug = "cache_mismatch_blocks_owner"
       /\ c = CachedProposalWrongViewWithOwner ->
      BlockFallbackAfterCache(spec)
    [] Bug = "reject_authoritative_owner_info"
       /\ c = AuthoritativeOwnerInfo ->
      RejectAtCurrentStage(spec, AuthoritativeOwnerInfoAccepted)
    [] Bug = "accept_owner_without_info"
       /\ c = AuthoritativeOwnerNoInfo ->
      AcceptAtOwner(spec, AuthoritativeOwnerNoInfoIgnored)
    [] Bug = "accept_owner_info_wrong_height"
       /\ c = AuthoritativeOwnerWrongHeight ->
      AcceptAtOwner(spec, AuthoritativeOwnerSlotMismatchIgnored)
    [] Bug = "accept_owner_info_wrong_view"
       /\ c = AuthoritativeOwnerWrongView ->
      AcceptAtOwner(spec, AuthoritativeOwnerSlotMismatchIgnored)
    [] Bug = "owner_no_info_blocks_active"
       /\ c = AuthoritativeOwnerNoInfoWithActive ->
      BlockFallbackAfterOwner(spec)
    [] Bug = "reject_active_owner"
       /\ c = ActiveFrontierOwner ->
      RejectAtCurrentStage(spec, ActiveOwnerAcceptedAction)
    [] Bug = "accept_active_owner_wrong_height"
       /\ c = ActiveOwnerWrongHeight ->
      AcceptAtActive(spec, ActiveOwnerSlotMismatchIgnored)
    [] Bug = "accept_active_owner_wrong_view"
       /\ c = ActiveOwnerWrongView ->
      AcceptAtActive(spec, ActiveOwnerSlotMismatchIgnored)
    [] Bug = "accept_no_evidence"
       /\ c = NoEvidence ->
      (spec \ {ReturnFalse}) \cup {ReturnTrue, ActiveOwnerAcceptedAction}
    [] OTHER -> spec

ImplementationResult(c) == ReturnTrue \in ImplementationActions(c)

Bugs == {
  "none",
  "reject_authoritative_payload",
  "accept_authoritative_wrong_height",
  "accept_authoritative_wrong_view",
  "authoritative_mismatch_blocks_seen",
  "reject_proposal_seen",
  "accept_seen_wrong_height",
  "accept_seen_wrong_view",
  "seen_mismatch_blocks_cache",
  "reject_cached_proposal",
  "accept_cache_wrong_height",
  "accept_cache_wrong_view",
  "cache_mismatch_blocks_owner",
  "reject_authoritative_owner_info",
  "accept_owner_without_info",
  "accept_owner_info_wrong_height",
  "accept_owner_info_wrong_view",
  "owner_no_info_blocks_active",
  "reject_active_owner",
  "accept_active_owner_wrong_height",
  "accept_active_owner_wrong_view",
  "accept_no_evidence"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultsMatchSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

AcceptedSourcesProduceEvidence ==
  /\ ImplementationResult(AuthoritativePayload)
  /\ ImplementationResult(ProposalSeen)
  /\ ImplementationResult(CachedProposal)
  /\ ImplementationResult(AuthoritativeOwnerInfo)
  /\ ImplementationResult(ActiveFrontierOwner)

WrongSlotAndIncompleteSourcesRejected ==
  /\ ~ImplementationResult(AuthoritativeWrongHeight)
  /\ ~ImplementationResult(AuthoritativeWrongView)
  /\ ~ImplementationResult(ProposalSeenWrongHeight)
  /\ ~ImplementationResult(ProposalSeenWrongView)
  /\ ~ImplementationResult(CachedProposalWrongHeight)
  /\ ~ImplementationResult(CachedProposalWrongView)
  /\ ~ImplementationResult(AuthoritativeOwnerNoInfo)
  /\ ~ImplementationResult(AuthoritativeOwnerWrongHeight)
  /\ ~ImplementationResult(AuthoritativeOwnerWrongView)
  /\ ~ImplementationResult(ActiveOwnerWrongHeight)
  /\ ~ImplementationResult(ActiveOwnerWrongView)
  /\ ~ImplementationResult(NoEvidence)

FallbackAfterEarlierMissesPreserved ==
  /\ ImplementationResult(AuthoritativeWrongHeightWithSeen)
  /\ AuthoritativeSlotMismatchIgnored \in
       ImplementationActions(AuthoritativeWrongHeightWithSeen)
  /\ ProposalSeenAccepted \in
       ImplementationActions(AuthoritativeWrongHeightWithSeen)
  /\ ImplementationResult(ProposalSeenWrongViewWithCache)
  /\ ProposalSeenSlotMismatchIgnored \in
       ImplementationActions(ProposalSeenWrongViewWithCache)
  /\ CachedProposalAccepted \in
       ImplementationActions(ProposalSeenWrongViewWithCache)
  /\ ImplementationResult(CachedProposalWrongViewWithOwner)
  /\ CachedProposalSlotMismatchIgnored \in
       ImplementationActions(CachedProposalWrongViewWithOwner)
  /\ AuthoritativeOwnerInfoAccepted \in
       ImplementationActions(CachedProposalWrongViewWithOwner)
  /\ ImplementationResult(AuthoritativeOwnerNoInfoWithActive)
  /\ AuthoritativeOwnerNoInfoIgnored \in
       ImplementationActions(AuthoritativeOwnerNoInfoWithActive)
  /\ ActiveOwnerAcceptedAction \in
       ImplementationActions(AuthoritativeOwnerNoInfoWithActive)

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckAuthoritativePayload \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterAuthoritative(c) =>
         CheckProposalSeen \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterSeen(c) =>
         CheckProposalCache \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterCache(c) =>
         CheckAuthoritativeOwner \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterOwner(c) =>
         CheckActiveOwner \in ImplementationActions(c)

ReturnActionMatchesResult ==
  \A c \in Cases:
    /\ (ReturnTrue \in ImplementationActions(c)) = ImplementationResult(c)
    /\ (ReturnFalse \in ImplementationActions(c)) = ~ImplementationResult(c)
    /\ ~(
         ReturnTrue \in ImplementationActions(c)
           /\ ReturnFalse \in ImplementationActions(c)
       )

AcceptedSourceActionAnchors ==
  /\ AuthoritativePayloadAccepted \in ImplementationActions(AuthoritativePayload)
  /\ ProposalSeenAccepted \in ImplementationActions(ProposalSeen)
  /\ CachedProposalAccepted \in ImplementationActions(CachedProposal)
  /\ AuthoritativeOwnerInfoAccepted \in
       ImplementationActions(AuthoritativeOwnerInfo)
  /\ ActiveOwnerAcceptedAction \in ImplementationActions(ActiveFrontierOwner)

RejectedSourceReturnAnchors ==
  /\ ReturnFalse \in ImplementationActions(AuthoritativeWrongHeight)
  /\ ReturnFalse \in ImplementationActions(AuthoritativeWrongView)
  /\ ReturnFalse \in ImplementationActions(ProposalSeenWrongHeight)
  /\ ReturnFalse \in ImplementationActions(ProposalSeenWrongView)
  /\ ReturnFalse \in ImplementationActions(CachedProposalWrongHeight)
  /\ ReturnFalse \in ImplementationActions(CachedProposalWrongView)
  /\ ReturnFalse \in ImplementationActions(AuthoritativeOwnerNoInfo)
  /\ ReturnFalse \in ImplementationActions(AuthoritativeOwnerWrongHeight)
  /\ ReturnFalse \in ImplementationActions(AuthoritativeOwnerWrongView)
  /\ ReturnFalse \in ImplementationActions(ActiveOwnerWrongHeight)
  /\ ReturnFalse \in ImplementationActions(ActiveOwnerWrongView)
  /\ ReturnFalse \in ImplementationActions(NoEvidence)

ShortCircuitAndFallbackAnchors ==
  /\ CheckProposalSeen \notin ImplementationActions(AuthoritativePayload)
  /\ CheckProposalCache \notin ImplementationActions(ProposalSeen)
  /\ CheckAuthoritativeOwner \notin ImplementationActions(CachedProposal)
  /\ CheckActiveOwner \notin ImplementationActions(AuthoritativeOwnerInfo)
  /\ CheckProposalSeen \in
       ImplementationActions(AuthoritativeWrongHeightWithSeen)
  /\ CheckProposalCache \in
       ImplementationActions(ProposalSeenWrongViewWithCache)
  /\ CheckAuthoritativeOwner \in
       ImplementationActions(CachedProposalWrongViewWithOwner)
  /\ CheckActiveOwner \in
       ImplementationActions(AuthoritativeOwnerNoInfoWithActive)

SlotProposalEvidenceCoreSafety ==
  /\ ResultsMatchSpec
  /\ ActionsMatchSpec
  /\ AcceptedSourcesProduceEvidence
  /\ WrongSlotAndIncompleteSourcesRejected
  /\ FallbackAfterEarlierMissesPreserved
  /\ LookupShapeMatchesShortCircuit
  /\ ReturnActionMatchesResult
  /\ AcceptedSourceActionAnchors
  /\ RejectedSourceReturnAnchors
  /\ ShortCircuitAndFallbackAnchors

SlotProposalEvidenceExactness ==
  /\ ResultsMatchSpec
  /\ ActionsMatchSpec
  /\ AcceptedSourcesProduceEvidence
  /\ WrongSlotAndIncompleteSourcesRejected
  /\ FallbackAfterEarlierMissesPreserved
  /\ LookupShapeMatchesShortCircuit
  /\ ReturnActionMatchesResult
  /\ AcceptedSourceActionAnchors
  /\ RejectedSourceReturnAnchors
  /\ ShortCircuitAndFallbackAnchors
SlotProposalEvidenceCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SlotProposalEvidenceExactness

NoBugInvariant == SlotProposalEvidenceExactness

SafetyFast == SlotProposalEvidenceExactness

====
