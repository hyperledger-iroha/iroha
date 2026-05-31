---- MODULE SumeragiClassicSigningPreimageGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for classic Sumeragi signing preimage construction.

Classic QC votes and VRF epoch-randomness messages are signed by
`vote_preimage(...)`, `vrf_commit_preimage(...)`, and `vrf_reveal_preimage(...)`.
This model captures the fields that must be bound into those preimages:
consensus-domain material, message type, vote subject roots and round context,
chain-order context, optional highest-QC context for NewView votes, and VRF
epoch/signer/secret material. Vote, VRF, and aggregate signatures are mutable
transport or certificate material and must stay outside the signing preimage.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Set(Str);
  fields

\* @type: <<Str, Set(Str)>>;
vars == <<candidate, fields>>

Cases == {
  "vote_no_highest",
  "vote_with_highest",
  "vrf_commit",
  "vrf_reveal"
}

VoteCases == {"vote_no_highest", "vote_with_highest"}

VrfCases == {"vrf_commit", "vrf_reveal"}

DomainFields == {
  "domain_protocol",
  "chain_id",
  "mode_tag",
  "proto_version",
  "v1"
}

VoteTypeField == {"type_vote"}

VrfCommitTypeField == {"type_vrf_commit"}

VrfRevealTypeField == {"type_vrf_reveal"}

VoteSubjectFields == {
  "block_hash",
  "parent_state_root",
  "post_state_root",
  "height",
  "view",
  "epoch",
  "chain_order_hash",
  "rechain_seq",
  "phase"
}

HighestAbsentField == {"highest_absent_flag"}

HighestBodyFields == {
  "highest_height",
  "highest_view",
  "highest_epoch",
  "highest_subject_block_hash",
  "highest_phase"
}

HighestPresentFields == {"highest_present_flag"} \union HighestBodyFields

VrfCommitFields == {"epoch", "commit_signer", "commitment"}

VrfRevealFields == {"epoch", "reveal_signer", "reveal"}

SignatureFields == {
  "vote_signature",
  "vrf_commit_signature",
  "vrf_reveal_signature",
  "aggregate_signature",
  "signer_bitmap"
}

SpecFields(c) ==
  CASE c = "vote_no_highest" ->
      DomainFields \union VoteTypeField \union VoteSubjectFields \union HighestAbsentField
    [] c = "vote_with_highest" ->
      DomainFields \union VoteTypeField \union VoteSubjectFields \union HighestPresentFields
    [] c = "vrf_commit" ->
      DomainFields \union VrfCommitTypeField \union VrfCommitFields
    [] c = "vrf_reveal" ->
      DomainFields \union VrfRevealTypeField \union VrfRevealFields
    [] OTHER -> {}

ActualFields(c) ==
  CASE c \in Cases /\ Bug = "drop_chain_id" ->
      SpecFields(c) \ {"chain_id"}
    [] c \in Cases /\ Bug = "drop_mode_tag" ->
      SpecFields(c) \ {"mode_tag"}
    [] c \in Cases /\ Bug = "drop_proto_version" ->
      SpecFields(c) \ {"proto_version"}
    [] c \in Cases /\ Bug = "drop_domain_protocol" ->
      SpecFields(c) \ {"domain_protocol"}
    [] c \in Cases /\ Bug = "drop_version" ->
      SpecFields(c) \ {"v1"}
    [] c \in VoteCases /\ Bug = "vote_uses_vrf_commit_type" ->
      (SpecFields(c) \ VoteTypeField) \union VrfCommitTypeField
    [] c = "vrf_commit" /\ Bug = "vrf_commit_uses_vote_type" ->
      (SpecFields(c) \ VrfCommitTypeField) \union VoteTypeField
    [] c = "vrf_reveal" /\ Bug = "vrf_reveal_uses_commit_type" ->
      (SpecFields(c) \ VrfRevealTypeField) \union VrfCommitTypeField
    [] c \in VoteCases /\ Bug = "drop_block_hash" ->
      SpecFields(c) \ {"block_hash"}
    [] c \in VoteCases /\ Bug = "drop_parent_state_root" ->
      SpecFields(c) \ {"parent_state_root"}
    [] c \in VoteCases /\ Bug = "drop_post_state_root" ->
      SpecFields(c) \ {"post_state_root"}
    [] c \in Cases /\ Bug = "drop_height" ->
      SpecFields(c) \ {"height"}
    [] c \in Cases /\ Bug = "drop_view" ->
      SpecFields(c) \ {"view"}
    [] c \in Cases /\ Bug = "drop_epoch" ->
      SpecFields(c) \ {"epoch"}
    [] c \in VoteCases /\ Bug = "drop_chain_order_hash" ->
      SpecFields(c) \ {"chain_order_hash"}
    [] c \in VoteCases /\ Bug = "drop_rechain_seq" ->
      SpecFields(c) \ {"rechain_seq"}
    [] c \in VoteCases /\ Bug = "drop_phase" ->
      SpecFields(c) \ {"phase"}
    [] c = "vote_no_highest" /\ Bug = "vote_without_highest_omits_absent_flag" ->
      SpecFields(c) \ HighestAbsentField
    [] c = "vote_no_highest" /\ Bug = "vote_without_highest_includes_highest_body" ->
      SpecFields(c) \union HighestBodyFields
    [] c = "vote_with_highest" /\ Bug = "vote_with_highest_omits_present_flag" ->
      SpecFields(c) \ {"highest_present_flag"}
    [] c = "vote_with_highest" /\ Bug = "drop_highest_height" ->
      SpecFields(c) \ {"highest_height"}
    [] c = "vote_with_highest" /\ Bug = "drop_highest_view" ->
      SpecFields(c) \ {"highest_view"}
    [] c = "vote_with_highest" /\ Bug = "drop_highest_epoch" ->
      SpecFields(c) \ {"highest_epoch"}
    [] c = "vote_with_highest" /\ Bug = "drop_highest_subject_block_hash" ->
      SpecFields(c) \ {"highest_subject_block_hash"}
    [] c = "vote_with_highest" /\ Bug = "drop_highest_phase" ->
      SpecFields(c) \ {"highest_phase"}
    [] c = "vrf_commit" /\ Bug = "vrf_commit_drops_signer" ->
      SpecFields(c) \ {"commit_signer"}
    [] c = "vrf_commit" /\ Bug = "vrf_commit_drops_commitment" ->
      SpecFields(c) \ {"commitment"}
    [] c = "vrf_reveal" /\ Bug = "vrf_reveal_drops_signer" ->
      SpecFields(c) \ {"reveal_signer"}
    [] c = "vrf_reveal" /\ Bug = "vrf_reveal_drops_reveal" ->
      SpecFields(c) \ {"reveal"}
    [] c \in VoteCases /\ Bug = "vote_includes_signature" ->
      SpecFields(c) \union {"vote_signature"}
    [] c = "vrf_commit" /\ Bug = "vrf_commit_includes_signature" ->
      SpecFields(c) \union {"vrf_commit_signature"}
    [] c = "vrf_reveal" /\ Bug = "vrf_reveal_includes_signature" ->
      SpecFields(c) \union {"vrf_reveal_signature"}
    [] OTHER -> SpecFields(c)

BugModes == {
  "none",
  "drop_chain_id",
  "drop_mode_tag",
  "drop_proto_version",
  "drop_domain_protocol",
  "drop_version",
  "vote_uses_vrf_commit_type",
  "vrf_commit_uses_vote_type",
  "vrf_reveal_uses_commit_type",
  "drop_block_hash",
  "drop_parent_state_root",
  "drop_post_state_root",
  "drop_height",
  "drop_view",
  "drop_epoch",
  "drop_chain_order_hash",
  "drop_rechain_seq",
  "drop_phase",
  "vote_without_highest_omits_absent_flag",
  "vote_without_highest_includes_highest_body",
  "vote_with_highest_omits_present_flag",
  "drop_highest_height",
  "drop_highest_view",
  "drop_highest_epoch",
  "drop_highest_subject_block_hash",
  "drop_highest_phase",
  "vrf_commit_drops_signer",
  "vrf_commit_drops_commitment",
  "vrf_reveal_drops_signer",
  "vrf_reveal_drops_reveal",
  "vote_includes_signature",
  "vrf_commit_includes_signature",
  "vrf_reveal_includes_signature"
}

AllFields ==
  DomainFields
    \union VoteTypeField
    \union VrfCommitTypeField
    \union VrfRevealTypeField
    \union VoteSubjectFields
    \union HighestAbsentField
    \union HighestPresentFields
    \union VrfCommitFields
    \union VrfRevealFields
    \union SignatureFields

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ fields \subseteq AllFields

Init ==
  /\ candidate = "none"
  /\ fields = {}

Apply(c) ==
  /\ candidate' = c
  /\ fields' = ActualFields(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

FieldsMatchSpec ==
  candidate = "none" \/ fields = SpecFields(candidate)

PreimagesBindDomain ==
  candidate \in Cases => DomainFields \subseteq fields

VoteUsesVoteTypeOnly ==
  candidate \in VoteCases =>
    /\ VoteTypeField \subseteq fields
    /\ fields \cap (VrfCommitTypeField \union VrfRevealTypeField) = {}

VrfCommitUsesCommitTypeOnly ==
  candidate = "vrf_commit" =>
    /\ VrfCommitTypeField \subseteq fields
    /\ fields \cap (VoteTypeField \union VrfRevealTypeField) = {}

VrfRevealUsesRevealTypeOnly ==
  candidate = "vrf_reveal" =>
    /\ VrfRevealTypeField \subseteq fields
    /\ fields \cap (VoteTypeField \union VrfCommitTypeField) = {}

VoteBindsSubject ==
  candidate \in VoteCases => VoteSubjectFields \subseteq fields

VoteWithoutHighestBindsAbsenceOnly ==
  candidate = "vote_no_highest" =>
    /\ HighestAbsentField \subseteq fields
    /\ fields \cap HighestPresentFields = {}

VoteWithHighestBindsReference ==
  candidate = "vote_with_highest" =>
    /\ HighestPresentFields \subseteq fields
    /\ fields \cap HighestAbsentField = {}

VrfCommitBindsBody ==
  candidate = "vrf_commit" => VrfCommitFields \subseteq fields

VrfRevealBindsBody ==
  candidate = "vrf_reveal" => VrfRevealFields \subseteq fields

PreimagesExcludeMutableSignatures ==
  candidate \in Cases => fields \cap SignatureFields = {}

Safety ==
  /\ FieldsMatchSpec
  /\ PreimagesBindDomain
  /\ VoteUsesVoteTypeOnly
  /\ VrfCommitUsesCommitTypeOnly
  /\ VrfRevealUsesRevealTypeOnly
  /\ VoteBindsSubject
  /\ VoteWithoutHighestBindsAbsenceOnly
  /\ VoteWithHighestBindsReference
  /\ VrfCommitBindsBody
  /\ VrfRevealBindsBody
  /\ PreimagesExcludeMutableSignatures

AllConcretePreimagesMatchSpec ==
  \A c \in Cases:
    ActualFields(c) = SpecFields(c)

AllConcretePreimagesBindDomain ==
  \A c \in Cases:
    DomainFields \subseteq ActualFields(c)

ConcreteVoteTypeAnchors ==
  \A c \in VoteCases:
    /\ VoteTypeField \subseteq ActualFields(c)
    /\ ActualFields(c) \cap (VrfCommitTypeField \union VrfRevealTypeField) = {}

ConcreteVrfCommitTypeAnchors ==
  /\ VrfCommitTypeField \subseteq ActualFields("vrf_commit")
  /\ ActualFields("vrf_commit") \cap (VoteTypeField \union VrfRevealTypeField) = {}

ConcreteVrfRevealTypeAnchors ==
  /\ VrfRevealTypeField \subseteq ActualFields("vrf_reveal")
  /\ ActualFields("vrf_reveal") \cap (VoteTypeField \union VrfCommitTypeField) = {}

AllConcreteVotesBindSubject ==
  \A c \in VoteCases:
    VoteSubjectFields \subseteq ActualFields(c)

VoteWithoutHighestConcreteAnchors ==
  /\ HighestAbsentField \subseteq ActualFields("vote_no_highest")
  /\ ActualFields("vote_no_highest") \cap HighestPresentFields = {}

VoteWithHighestConcreteAnchors ==
  /\ HighestPresentFields \subseteq ActualFields("vote_with_highest")
  /\ ActualFields("vote_with_highest") \cap HighestAbsentField = {}

VrfConcreteBodyAnchors ==
  /\ VrfCommitFields \subseteq ActualFields("vrf_commit")
  /\ VrfRevealFields \subseteq ActualFields("vrf_reveal")

AllConcretePreimagesExcludeMutableSignatures ==
  \A c \in Cases:
    ActualFields(c) \cap SignatureFields = {}

ClassicPreimageSafetyAnchors ==
  /\ AllConcretePreimagesMatchSpec
  /\ AllConcretePreimagesBindDomain
  /\ ConcreteVoteTypeAnchors
  /\ ConcreteVrfCommitTypeAnchors
  /\ ConcreteVrfRevealTypeAnchors
  /\ AllConcreteVotesBindSubject
  /\ VoteWithoutHighestConcreteAnchors
  /\ VoteWithHighestConcreteAnchors
  /\ VrfConcreteBodyAnchors
  /\ AllConcretePreimagesExcludeMutableSignatures

====
