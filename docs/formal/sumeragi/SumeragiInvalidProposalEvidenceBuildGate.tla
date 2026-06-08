---- MODULE SumeragiInvalidProposalEvidenceBuildGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `invalid_proposal_evidence(...)` and
`build_invalid_proposal_evidence(...)`.

The wrapper must emit `EvidenceKind::InvalidProposal` and preserve both the
proposal and diagnostic reason. The builder must derive the proposal exactly as
the Rust helper does: first block-signature index or zero fallback for the
proposer, block-header view, caller-supplied epoch and payload hash, and the QC
selected by `qc_for_validation_evidence(...)`. The selector guarantees the QC
subject is the proposal parent and that the QC height is below the proposal
height; this gate pins those fields so evidence remains valid downstream.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoKind == 0
InvalidProposalKind == 1
InvalidQcKind == 2

NoProposal == 0
WrapperProposal == 101
BuiltProposal == 201
WrongProposal == 999

NoReason == 0
ValidationErrorReason == 1
LabelReason == 2

NoValue == 0
FirstSignatureProposer == 7
SecondSignatureProposer == 9
MultiFirstSignatureProposer == 4
HeaderView == 6
QcView == 3
ArgEpoch == 5
QcEpoch == 8
PayloadHashArg == 11
BlockHashPayload == 12
QcSubject == 13
WrongSubject == 14
QcHeight == 20
BlockHeight == 21
WrongHeight == 19

WrapPreservesProposalReason == 1
BuildFirstSignature == 2
BuildNoSignatureFallback == 3
BuildMultiSignatureFirst == 4
BuildHeaderView == 5
BuildArgEpoch == 6
BuildPayloadHashArg == 7
BuildQcParentHeight == 8
BuildReasonString == 9

IsBuild(c) ==
  c # WrapPreservesProposalReason

SpecKind(c) ==
  InvalidProposalKind

ActualKind(c) ==
  CASE Bug = "wrapper_wrong_kind"
       /\ c = WrapPreservesProposalReason -> InvalidQcKind
    [] OTHER -> SpecKind(c)

SpecProposal(c) ==
  IF c = WrapPreservesProposalReason THEN WrapperProposal ELSE BuiltProposal

ActualProposal(c) ==
  CASE Bug = "wrapper_rewrites_proposal"
       /\ c = WrapPreservesProposalReason -> WrongProposal
    [] OTHER -> SpecProposal(c)

SpecProposer(c) ==
  CASE c = BuildNoSignatureFallback -> 0
    [] c = BuildMultiSignatureFirst -> MultiFirstSignatureProposer
    [] IsBuild(c) -> FirstSignatureProposer
    [] OTHER -> NoValue

ActualProposer(c) ==
  CASE Bug = "builder_uses_zero_proposer"
       /\ c = BuildFirstSignature -> 0
    [] Bug = "builder_uses_last_signature"
       /\ c = BuildMultiSignatureFirst -> SecondSignatureProposer
    [] Bug = "builder_missing_signature_not_zero"
       /\ c = BuildNoSignatureFallback -> FirstSignatureProposer
    [] OTHER -> SpecProposer(c)

SpecView(c) ==
  IF IsBuild(c) THEN HeaderView ELSE NoValue

ActualView(c) ==
  CASE Bug = "builder_uses_qc_view"
       /\ c = BuildHeaderView -> QcView
    [] Bug = "builder_uses_zero_view"
       /\ c = BuildHeaderView -> 0
    [] OTHER -> SpecView(c)

SpecEpoch(c) ==
  IF IsBuild(c) THEN ArgEpoch ELSE NoValue

ActualEpoch(c) ==
  CASE Bug = "builder_uses_zero_epoch"
       /\ c = BuildArgEpoch -> 0
    [] Bug = "builder_uses_qc_epoch"
       /\ c = BuildArgEpoch -> QcEpoch
    [] OTHER -> SpecEpoch(c)

SpecPayloadHash(c) ==
  IF IsBuild(c) THEN PayloadHashArg ELSE NoValue

ActualPayloadHash(c) ==
  CASE Bug = "builder_uses_block_hash_payload"
       /\ c = BuildPayloadHashArg -> BlockHashPayload
    [] Bug = "builder_drops_payload_hash"
       /\ c = BuildPayloadHashArg -> NoValue
    [] OTHER -> SpecPayloadHash(c)

SpecQcSubject(c) ==
  IF IsBuild(c) THEN QcSubject ELSE NoValue

ActualQcSubject(c) ==
  CASE Bug = "builder_uses_wrong_qc"
       /\ c = BuildQcParentHeight -> WrongSubject
    [] OTHER -> SpecQcSubject(c)

SpecQcHeight(c) ==
  IF IsBuild(c) THEN QcHeight ELSE NoValue

ActualQcHeight(c) ==
  SpecQcHeight(c)

SpecParent(c) ==
  IF IsBuild(c) THEN QcSubject ELSE NoValue

ActualParent(c) ==
  CASE Bug = "builder_wrong_parent"
       /\ c = BuildQcParentHeight -> WrongSubject
    [] OTHER -> SpecParent(c)

SpecHeight(c) ==
  IF IsBuild(c) THEN BlockHeight ELSE NoValue

ActualHeight(c) ==
  CASE Bug = "builder_wrong_height"
       /\ c = BuildQcParentHeight -> WrongHeight
    [] OTHER -> SpecHeight(c)

SpecReason(c) ==
  ValidationErrorReason

ActualReason(c) ==
  CASE Bug = "wrapper_drops_reason"
       /\ c = WrapPreservesProposalReason -> NoReason
    [] Bug = "builder_reason_label_instead_of_error"
       /\ c = BuildReasonString -> LabelReason
    [] OTHER -> SpecReason(c)

\* @type: Int => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecEvidence(c) ==
  <<SpecKind(c), SpecProposal(c), SpecProposer(c), SpecView(c),
    SpecEpoch(c), SpecPayloadHash(c), SpecQcSubject(c), SpecQcHeight(c),
    SpecParent(c), SpecHeight(c), SpecReason(c)>>

\* @type: Int => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualEvidence(c) ==
  <<ActualKind(c), ActualProposal(c), ActualProposer(c), ActualView(c),
    ActualEpoch(c), ActualPayloadHash(c), ActualQcSubject(c), ActualQcHeight(c),
    ActualParent(c), ActualHeight(c), ActualReason(c)>>

Matches(c) ==
  ActualEvidence(c) = SpecEvidence(c)

ValidBuiltEvidenceShape(c) ==
  IF IsBuild(c)
  THEN /\ ActualParent(c) = ActualQcSubject(c)
       /\ ActualQcHeight(c) < ActualHeight(c)
  ELSE TRUE

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "wrapper_wrong_kind",
       "wrapper_drops_reason",
       "wrapper_rewrites_proposal",
       "builder_uses_zero_proposer",
       "builder_uses_last_signature",
       "builder_missing_signature_not_zero",
       "builder_uses_qc_view",
       "builder_uses_zero_view",
       "builder_uses_zero_epoch",
       "builder_uses_qc_epoch",
       "builder_uses_block_hash_payload",
       "builder_drops_payload_hash",
       "builder_uses_wrong_qc",
       "builder_wrong_parent",
       "builder_wrong_height",
       "builder_reason_label_instead_of_error"
     }
  /\ checked = 0

InvalidProposalEvidenceBuildMatchesSpec ==
  /\ Matches(WrapPreservesProposalReason)
  /\ Matches(BuildFirstSignature)
  /\ Matches(BuildNoSignatureFallback)
  /\ Matches(BuildMultiSignatureFirst)
  /\ Matches(BuildHeaderView)
  /\ Matches(BuildArgEpoch)
  /\ Matches(BuildPayloadHashArg)
  /\ Matches(BuildQcParentHeight)
  /\ Matches(BuildReasonString)
  /\ ValidBuiltEvidenceShape(BuildQcParentHeight)

SafetyFast ==
  InvalidProposalEvidenceBuildMatchesSpec

BugWrapperWrongKind ==
  Matches(WrapPreservesProposalReason)

BugWrapperDropsReason ==
  Matches(WrapPreservesProposalReason)

BugWrapperRewritesProposal ==
  Matches(WrapPreservesProposalReason)

BugBuilderUsesZeroProposer ==
  Matches(BuildFirstSignature)

BugBuilderUsesLastSignature ==
  Matches(BuildMultiSignatureFirst)

BugBuilderMissingSignatureNotZero ==
  Matches(BuildNoSignatureFallback)

BugBuilderUsesQcView ==
  Matches(BuildHeaderView)

BugBuilderUsesZeroView ==
  Matches(BuildHeaderView)

BugBuilderUsesZeroEpoch ==
  Matches(BuildArgEpoch)

BugBuilderUsesQcEpoch ==
  Matches(BuildArgEpoch)

BugBuilderUsesBlockHashPayload ==
  Matches(BuildPayloadHashArg)

BugBuilderDropsPayloadHash ==
  Matches(BuildPayloadHashArg)

BugBuilderUsesWrongQc ==
  Matches(BuildQcParentHeight)

BugBuilderWrongParent ==
  Matches(BuildQcParentHeight)

BugBuilderWrongHeight ==
  ValidBuiltEvidenceShape(BuildQcParentHeight)

BugBuilderReasonLabelInsteadOfError ==
  Matches(BuildReasonString)

====
