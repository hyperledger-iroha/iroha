---- MODULE SumeragiCertifiedBlockFetchGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for Sumeragi direct certified-block fetch recovery.

This slice models the protocol contracts around
`request_certified_block_for_qc(...)`,
`dispatch_certified_block_fetch_response(...)`, and
`handle_certified_block_fetch(...)`. The model abstracts concrete blocks,
hashes, signatures, validator snapshots, and network frames into
representative boundary cases while preserving the key safety obligations:
only commit QCs can request exact certified blocks, remote targets are derived
from QC signers with deterministic fallback and no local sends, served
responses must match the local block and cached commit QC, oversized responses
split into proof/body companions without sending an oversized full response,
all proof/response checkpoint fields self-validate before a QC is cached,
bodies materialize only after a matching accepted proof, invalid pending
owners are not revived by recovery traffic, and successful materialization
clears recovery deferrals before waking the commit pipeline.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

NonCommitQcNoRequest == 1
SignerTargetsPreferred == 2
OutOfRangeSignersFallbackTopology == 3
LocalTargetsRemoved == 4
EmptyRemoteTargetsNoRequest == 5
RequestUsesHighPriority == 6
RequestAvoidsGenericMissingFetch == 7
MismatchedRequesterDropped == 8
MissingLocalBlockNoResponse == 9
LocalSubjectMismatchDropped == 10
MissingLocalCommitQcNoResponse == 11
MismatchedCommitQcNoResponse == 12
NposResponseCarriesMatchingStakeSnapshot == 13
FullResponseUnderCapDispatchesFull == 14
OversizedFullSplitsProofBody == 15
OversizedProofDropsAll == 16
OversizedBodyUsesBodyResponseFallback == 17
OversizedBodyResponseUsesBlockCreatedFallback == 18
OversizedAllDropsBodyKeepsProof == 19
ResponseHeightMismatchRejected == 20
ResponseViewMismatchRejected == 21
ResponseBlockHashMismatchRejected == 22
ResponseQcHeightMismatchRejected == 23
ResponseQcViewMismatchRejected == 24
ResponseUncertifiedRejected == 25
ResponseCheckpointMismatchRejected == 26
ProofAcceptedCachesQc == 27
MalformedProofRejected == 28
BodyWithoutProofRejected == 29
BodyMismatchedProofRejected == 30
ProofThenBodyMaterializes == 31
FullResponseMaterializes == 32
InvalidInflightRejected == 33
InvalidPendingRejected == 34
RetryAbortedRevived == 35
MaterializationClearsDeferrals == 36

Candidates == 1..36

NoBug == 0
RequestNonCommitQcBug == 1
SkipSignerTargetsBug == 2
NoTopologyFallbackBug == 3
KeepLocalTargetBug == 4
RequestWithoutRemoteTargetsBug == 5
LowPriorityRequestBug == 6
UseGenericMissingFetchBug == 7
AcceptForgedRequesterBug == 8
ServeMissingLocalBlockBug == 9
ServeMismatchedLocalSubjectBug == 10
ServeWithoutCommitQcBug == 11
ServeMismatchedCommitQcBug == 12
DropNposStakeSnapshotBug == 13
SplitSmallFullResponseBug == 14
SendOversizedFullBug == 15
SendOversizedProofBug == 16
DropInsteadOfBodyResponseFallbackBug == 17
DropInsteadOfBlockCreatedFallbackBug == 18
DropProofWhenBodyTooLargeBug == 19
AcceptResponseHeightMismatchBug == 20
AcceptResponseViewMismatchBug == 21
AcceptResponseBlockHashMismatchBug == 22
AcceptResponseQcHeightMismatchBug == 23
AcceptResponseQcViewMismatchBug == 24
AcceptUncertifiedResponseBug == 25
AcceptCheckpointMismatchBug == 26
ProofDoesNotCacheQcBug == 27
MalformedProofCachesQcBug == 28
BodyWithoutProofMaterializesBug == 29
MismatchedBodyMaterializesBug == 30
ProofOnlyMaterializesBug == 31
FullResponseSkipsProofAdmissionBug == 32
InvalidInflightMaterializesBug == 33
InvalidPendingMaterializesBug == 34
RetryAbortedDroppedBug == 35
MaterializationLeavesDeferralsBug == 36

Bugs == 0..36

NoRequest == 1
SendCertifiedRequest == 2
RequireCommitPhase == 3
SelectSignerTargets == 4
SelectTopologyTargets == 5
RemoveLocalTarget == 6
SortTargets == 7
DedupTargets == 8
SuppressEmptyTargets == 9
UseHighPriority == 10
AvoidGenericFetch == 11
DropRequest == 12
ValidateRequester == 13
RequireLocalBlock == 14
ValidateLocalSubject == 15
RequireLocalCommitQc == 16
ValidateLocalCommitQc == 17
BuildResponse == 18
AttachStakeSnapshot == 19
ValidateStakeSnapshot == 20
DispatchFull == 21
DispatchProof == 22
DispatchBody == 23
DispatchBodyResponse == 24
DispatchBlockCreated == 25
DropOversized == 26
BypassQueue == 27
NoDispatchFull == 28
ValidateResponse == 29
ValidateProof == 30
ValidateBody == 31
DropResponse == 32
DropProof == 33
DropBody == 34
CacheCommitQc == 35
RecordRosterSnapshot == 36
RecordCommitQcStatus == 37
ClearMissingCommitQc == 38
RequireCachedProof == 39
MaterializePending == 40
LinkCommitQc == 41
ReviveRetryAborted == 42
PreserveInvalidInflight == 43
PreserveInvalidPending == 44
ClearDeferredPayloadQc == 45
ClearDeferredBlockSync == 46
ClearMissingPayload == 47
ClearViewChange == 48
FlushBodyRequests == 49
WakeCommitPipeline == 50
CacheKuraBody == 51

Actions == 1..51

SpecActions(candidate) ==
  CASE candidate = NonCommitQcNoRequest ->
      {NoRequest, RequireCommitPhase}
    [] candidate = SignerTargetsPreferred ->
      {SendCertifiedRequest, SelectSignerTargets, SortTargets, DedupTargets}
    [] candidate = OutOfRangeSignersFallbackTopology ->
      {SendCertifiedRequest, SelectTopologyTargets}
    [] candidate = LocalTargetsRemoved ->
      {RemoveLocalTarget}
    [] candidate = EmptyRemoteTargetsNoRequest ->
      {NoRequest, RemoveLocalTarget, SuppressEmptyTargets}
    [] candidate = RequestUsesHighPriority ->
      {SendCertifiedRequest, UseHighPriority}
    [] candidate = RequestAvoidsGenericMissingFetch ->
      {SendCertifiedRequest, AvoidGenericFetch}
    [] candidate = MismatchedRequesterDropped ->
      {DropRequest, ValidateRequester}
    [] candidate = MissingLocalBlockNoResponse ->
      {DropRequest, RequireLocalBlock}
    [] candidate = LocalSubjectMismatchDropped ->
      {DropRequest, ValidateLocalSubject}
    [] candidate = MissingLocalCommitQcNoResponse ->
      {DropRequest, RequireLocalCommitQc}
    [] candidate = MismatchedCommitQcNoResponse ->
      {DropRequest, ValidateLocalCommitQc}
    [] candidate = NposResponseCarriesMatchingStakeSnapshot ->
      {BuildResponse, AttachStakeSnapshot, ValidateStakeSnapshot}
    [] candidate = FullResponseUnderCapDispatchesFull ->
      {DispatchFull, BypassQueue}
    [] candidate = OversizedFullSplitsProofBody ->
      {DispatchProof, DispatchBody, NoDispatchFull, BypassQueue}
    [] candidate = OversizedProofDropsAll ->
      {DropOversized, NoDispatchFull}
    [] candidate = OversizedBodyUsesBodyResponseFallback ->
      {DispatchProof, DispatchBodyResponse, NoDispatchFull}
    [] candidate = OversizedBodyResponseUsesBlockCreatedFallback ->
      {DispatchProof, DispatchBlockCreated, NoDispatchFull}
    [] candidate = OversizedAllDropsBodyKeepsProof ->
      {DispatchProof, DropOversized, NoDispatchFull}
    [] candidate = ResponseHeightMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseViewMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseBlockHashMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseQcHeightMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseQcViewMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseUncertifiedRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ResponseCheckpointMismatchRejected ->
      {DropResponse, ValidateResponse}
    [] candidate = ProofAcceptedCachesQc ->
      {ValidateProof, CacheCommitQc, RecordRosterSnapshot, RecordCommitQcStatus,
       ClearMissingCommitQc}
    [] candidate = MalformedProofRejected ->
      {DropProof, ValidateProof}
    [] candidate = BodyWithoutProofRejected ->
      {DropBody, ValidateBody, RequireCachedProof}
    [] candidate = BodyMismatchedProofRejected ->
      {DropBody, ValidateBody, RequireCachedProof}
    [] candidate = ProofThenBodyMaterializes ->
      {ValidateProof, CacheCommitQc, ValidateBody, RequireCachedProof,
       MaterializePending, LinkCommitQc}
    [] candidate = FullResponseMaterializes ->
      {ValidateResponse, ValidateProof, CacheCommitQc, MaterializePending,
       LinkCommitQc}
    [] candidate = InvalidInflightRejected ->
      {DropResponse, PreserveInvalidInflight}
    [] candidate = InvalidPendingRejected ->
      {DropResponse, PreserveInvalidPending}
    [] candidate = RetryAbortedRevived ->
      {MaterializePending, ReviveRetryAborted, LinkCommitQc}
    [] candidate = MaterializationClearsDeferrals ->
      {MaterializePending, CacheKuraBody, ClearDeferredPayloadQc,
       ClearDeferredBlockSync, ClearMissingPayload, ClearViewChange,
       FlushBodyRequests, WakeCommitPipeline}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NonCommitQcNoRequest /\ Bug = RequestNonCommitQcBug ->
      (spec \ {NoRequest}) \cup {SendCertifiedRequest}
    [] candidate = SignerTargetsPreferred /\ Bug = SkipSignerTargetsBug ->
      (spec \ {SelectSignerTargets}) \cup {SelectTopologyTargets}
    [] candidate = OutOfRangeSignersFallbackTopology /\
          Bug = NoTopologyFallbackBug ->
      spec \ {SelectTopologyTargets}
    [] candidate = LocalTargetsRemoved /\ Bug = KeepLocalTargetBug ->
      spec \ {RemoveLocalTarget}
    [] candidate = EmptyRemoteTargetsNoRequest /\
          Bug = RequestWithoutRemoteTargetsBug ->
      (spec \ {NoRequest, SuppressEmptyTargets}) \cup {SendCertifiedRequest}
    [] candidate = RequestUsesHighPriority /\ Bug = LowPriorityRequestBug ->
      spec \ {UseHighPriority}
    [] candidate = RequestAvoidsGenericMissingFetch /\
          Bug = UseGenericMissingFetchBug ->
      spec \ {AvoidGenericFetch}
    [] candidate = MismatchedRequesterDropped /\
          Bug = AcceptForgedRequesterBug ->
      (spec \ {DropRequest, ValidateRequester}) \cup {BuildResponse}
    [] candidate = MissingLocalBlockNoResponse /\ Bug = ServeMissingLocalBlockBug ->
      (spec \ {DropRequest, RequireLocalBlock}) \cup {BuildResponse}
    [] candidate = LocalSubjectMismatchDropped /\
          Bug = ServeMismatchedLocalSubjectBug ->
      (spec \ {DropRequest, ValidateLocalSubject}) \cup {BuildResponse}
    [] candidate = MissingLocalCommitQcNoResponse /\
          Bug = ServeWithoutCommitQcBug ->
      (spec \ {DropRequest, RequireLocalCommitQc}) \cup {BuildResponse}
    [] candidate = MismatchedCommitQcNoResponse /\
          Bug = ServeMismatchedCommitQcBug ->
      (spec \ {DropRequest, ValidateLocalCommitQc}) \cup {BuildResponse}
    [] candidate = NposResponseCarriesMatchingStakeSnapshot /\
          Bug = DropNposStakeSnapshotBug ->
      spec \ {AttachStakeSnapshot, ValidateStakeSnapshot}
    [] candidate = FullResponseUnderCapDispatchesFull /\
          Bug = SplitSmallFullResponseBug ->
      (spec \ {DispatchFull}) \cup {DispatchProof, DispatchBody}
    [] candidate = OversizedFullSplitsProofBody /\ Bug = SendOversizedFullBug ->
      (spec \ {NoDispatchFull}) \cup {DispatchFull}
    [] candidate = OversizedProofDropsAll /\ Bug = SendOversizedProofBug ->
      (spec \ {DropOversized}) \cup {DispatchProof}
    [] candidate = OversizedBodyUsesBodyResponseFallback /\
          Bug = DropInsteadOfBodyResponseFallbackBug ->
      spec \ {DispatchBodyResponse}
    [] candidate = OversizedBodyResponseUsesBlockCreatedFallback /\
          Bug = DropInsteadOfBlockCreatedFallbackBug ->
      spec \ {DispatchBlockCreated}
    [] candidate = OversizedAllDropsBodyKeepsProof /\
          Bug = DropProofWhenBodyTooLargeBug ->
      spec \ {DispatchProof}
    [] candidate = ResponseHeightMismatchRejected /\
          Bug = AcceptResponseHeightMismatchBug ->
      (spec \ {DropResponse}) \cup {MaterializePending}
    [] candidate = ResponseViewMismatchRejected /\
          Bug = AcceptResponseViewMismatchBug ->
      (spec \ {DropResponse}) \cup {MaterializePending}
    [] candidate = ResponseBlockHashMismatchRejected /\
          Bug = AcceptResponseBlockHashMismatchBug ->
      (spec \ {DropResponse}) \cup {MaterializePending}
    [] candidate = ResponseQcHeightMismatchRejected /\
          Bug = AcceptResponseQcHeightMismatchBug ->
      (spec \ {DropResponse}) \cup {CacheCommitQc}
    [] candidate = ResponseQcViewMismatchRejected /\
          Bug = AcceptResponseQcViewMismatchBug ->
      (spec \ {DropResponse}) \cup {CacheCommitQc}
    [] candidate = ResponseUncertifiedRejected /\
          Bug = AcceptUncertifiedResponseBug ->
      (spec \ {DropResponse}) \cup {CacheCommitQc, MaterializePending}
    [] candidate = ResponseCheckpointMismatchRejected /\
          Bug = AcceptCheckpointMismatchBug ->
      (spec \ {DropResponse}) \cup {CacheCommitQc}
    [] candidate = ProofAcceptedCachesQc /\ Bug = ProofDoesNotCacheQcBug ->
      spec \ {CacheCommitQc}
    [] candidate = MalformedProofRejected /\ Bug = MalformedProofCachesQcBug ->
      (spec \ {DropProof}) \cup {CacheCommitQc}
    [] candidate = BodyWithoutProofRejected /\
          Bug = BodyWithoutProofMaterializesBug ->
      (spec \ {DropBody, RequireCachedProof}) \cup {MaterializePending}
    [] candidate = BodyMismatchedProofRejected /\
          Bug = MismatchedBodyMaterializesBug ->
      (spec \ {DropBody}) \cup {MaterializePending}
    [] candidate = ProofThenBodyMaterializes /\ Bug = ProofOnlyMaterializesBug ->
      (spec \ {ValidateBody, RequireCachedProof}) \cup {MaterializePending}
    [] candidate = FullResponseMaterializes /\
          Bug = FullResponseSkipsProofAdmissionBug ->
      spec \ {ValidateProof, CacheCommitQc}
    [] candidate = InvalidInflightRejected /\
          Bug = InvalidInflightMaterializesBug ->
      (spec \ {DropResponse, PreserveInvalidInflight}) \cup {MaterializePending}
    [] candidate = InvalidPendingRejected /\
          Bug = InvalidPendingMaterializesBug ->
      (spec \ {DropResponse, PreserveInvalidPending}) \cup {MaterializePending}
    [] candidate = RetryAbortedRevived /\ Bug = RetryAbortedDroppedBug ->
      spec \ {ReviveRetryAborted, MaterializePending}
    [] candidate = MaterializationClearsDeferrals /\
          Bug = MaterializationLeavesDeferralsBug ->
      spec \ {ClearDeferredPayloadQc, ClearDeferredBlockSync,
              ClearMissingPayload, ClearViewChange, FlushBodyRequests,
              WakeCommitPipeline}
    [] OTHER -> spec

Init ==
  tried = {}

Next ==
  \E candidate \in Candidates \ tried:
    tried' = tried \cup {candidate}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried: ImplementationActions(candidate) \subseteq Actions

CertifiedFetchRequestCases == {
  NonCommitQcNoRequest, SignerTargetsPreferred,
  OutOfRangeSignersFallbackTopology, LocalTargetsRemoved,
  EmptyRemoteTargetsNoRequest, RequestUsesHighPriority,
  RequestAvoidsGenericMissingFetch
}

CertifiedFetchServeCases == {
  MismatchedRequesterDropped, MissingLocalBlockNoResponse,
  LocalSubjectMismatchDropped, MissingLocalCommitQcNoResponse,
  MismatchedCommitQcNoResponse, NposResponseCarriesMatchingStakeSnapshot
}

CertifiedFetchDispatchCases == {
  FullResponseUnderCapDispatchesFull, OversizedFullSplitsProofBody,
  OversizedProofDropsAll, OversizedBodyUsesBodyResponseFallback,
  OversizedBodyResponseUsesBlockCreatedFallback, OversizedAllDropsBodyKeepsProof
}

CertifiedFetchAdmissionCases == {
  ResponseHeightMismatchRejected, ResponseViewMismatchRejected,
  ResponseBlockHashMismatchRejected, ResponseQcHeightMismatchRejected,
  ResponseQcViewMismatchRejected, ResponseUncertifiedRejected,
  ResponseCheckpointMismatchRejected, ProofAcceptedCachesQc,
  MalformedProofRejected, BodyWithoutProofRejected, BodyMismatchedProofRejected
}

CertifiedFetchMaterializationCases == {
  ProofThenBodyMaterializes, FullResponseMaterializes,
  InvalidInflightRejected, InvalidPendingRejected, RetryAbortedRevived,
  MaterializationClearsDeferrals
}

CertifiedFetchGroupedCases ==
  CertifiedFetchRequestCases \cup CertifiedFetchServeCases \cup
  CertifiedFetchDispatchCases \cup CertifiedFetchAdmissionCases \cup
  CertifiedFetchMaterializationCases

CertifiedFetchCaseGroupsComplete ==
  CertifiedFetchGroupedCases = Candidates

CertifiedFetchRequestExact ==
  \A candidate \in tried:
    candidate \in CertifiedFetchRequestCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

CertifiedFetchServeExact ==
  \A candidate \in tried:
    candidate \in CertifiedFetchServeCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

CertifiedFetchDispatchExact ==
  \A candidate \in tried:
    candidate \in CertifiedFetchDispatchCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

CertifiedFetchAdmissionExact ==
  \A candidate \in tried:
    candidate \in CertifiedFetchAdmissionCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

CertifiedFetchMaterializationExact ==
  \A candidate \in tried:
    candidate \in CertifiedFetchMaterializationCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

CertifiedFetchExactness ==
  /\ CertifiedFetchCaseGroupsComplete
  /\ CertifiedFetchRequestExact
  /\ CertifiedFetchServeExact
  /\ CertifiedFetchDispatchExact
  /\ CertifiedFetchAdmissionExact
  /\ CertifiedFetchMaterializationExact

Safety ==
  CertifiedFetchExactness

====
