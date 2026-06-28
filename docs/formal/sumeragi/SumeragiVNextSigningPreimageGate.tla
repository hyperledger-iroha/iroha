---- MODULE SumeragiVNextSigningPreimageGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for vNext signing preimage construction.

The vNext vote and certificate helpers intentionally share aggregate signing
preimages. Re-chain votes and certificates sign the same re-chain certificate
body, while view-change votes and certificates sign the same view-change body.
Every preimage is domain separated by chain id, message type, vNext version,
and Sumeragi mode tag. Signatures and signer bitmaps are not part of the
aggregate-signing body.

Suspicion evidence is canonicalized by a signing-body hash that includes the
slot, accuser, accused, missed obligation, chain-order hash, re-chain sequence,
and observed delay, while excluding the signature itself. This keeps duplicate
evidence detection deterministic and prevents signature malleability from
changing evidence identity.
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
  "rechain_vote_unsigned",
  "view_vote_unsigned",
  "rechain_vote_preimage",
  "rechain_cert_preimage",
  "view_vote_preimage",
  "view_cert_preimage",
  "suspect_hash"
}

RechainPreimageCases == {"rechain_vote_preimage", "rechain_cert_preimage"}

ViewPreimageCases == {"view_vote_preimage", "view_cert_preimage"}

PreimageCases == RechainPreimageCases \union ViewPreimageCases

UnsignedVoteCases == {"rechain_vote_unsigned", "view_vote_unsigned"}

DomainFields == {"chain_id", "mode_tag", "vnext_v1"}

RechainTypeField == {"type_rechain_certificate"}

ViewTypeField == {"type_view_change_certificate"}

RechainBodyFields == {
  "slot",
  "previous_chain_order_hash",
  "new_chain_order_hash",
  "new_order",
  "rechain_seq",
  "tainted",
  "suspicions"
}

ViewBodyFields == {"new_view", "highest_slot", "chain_order_hash"}

SignerFields == {"signer", "empty_signature"}

SignatureAndBitmapFields == {
  "vote_signature",
  "aggregate_signature",
  "signer_bitmap"
}

SuspectBodyFields == {
  "slot",
  "accuser",
  "accused",
  "obligation",
  "chain_order_hash",
  "rechain_seq",
  "observed_delay_ms"
}

SpecFields(c) ==
  CASE c = "rechain_vote_unsigned" ->
      RechainBodyFields \union SignerFields
    [] c = "view_vote_unsigned" ->
      ViewBodyFields \union SignerFields
    [] c \in RechainPreimageCases ->
      DomainFields \union RechainTypeField \union RechainBodyFields
    [] c \in ViewPreimageCases ->
      DomainFields \union ViewTypeField \union ViewBodyFields
    [] c = "suspect_hash" -> SuspectBodyFields
    [] OTHER -> {}

ActualFields(c) ==
  CASE c \in PreimageCases /\ Bug = "drop_chain_id" ->
      SpecFields(c) \ {"chain_id"}
    [] c \in PreimageCases /\ Bug = "drop_mode_tag" ->
      SpecFields(c) \ {"mode_tag"}
    [] c \in PreimageCases /\ Bug = "drop_vnext_version" ->
      SpecFields(c) \ {"vnext_v1"}
    [] c \in RechainPreimageCases /\ Bug = "use_view_type_for_rechain" ->
      (SpecFields(c) \ RechainTypeField) \union ViewTypeField
    [] c \in ViewPreimageCases /\ Bug = "use_rechain_type_for_view" ->
      (SpecFields(c) \ ViewTypeField) \union RechainTypeField
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_slot" ->
      SpecFields(c) \ {"slot"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_previous_hash" ->
      SpecFields(c) \ {"previous_chain_order_hash"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_new_hash" ->
      SpecFields(c) \ {"new_chain_order_hash"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_new_order" ->
      SpecFields(c) \ {"new_order"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_sequence" ->
      SpecFields(c) \ {"rechain_seq"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_tainted" ->
      SpecFields(c) \ {"tainted"}
    [] c \in RechainPreimageCases /\ Bug = "drop_rechain_suspicions" ->
      SpecFields(c) \ {"suspicions"}
    [] c \in RechainPreimageCases /\ Bug = "include_rechain_signature" ->
      SpecFields(c) \union {"aggregate_signature"}
    [] c \in RechainPreimageCases /\ Bug = "include_rechain_bitmap" ->
      SpecFields(c) \union {"signer_bitmap"}
    [] c = "rechain_vote_unsigned" /\ Bug = "rechain_vote_drops_body" ->
      SignerFields
    [] c = "rechain_vote_unsigned" /\ Bug = "rechain_vote_keeps_signature" ->
      SpecFields(c) \union {"vote_signature"}
    [] c \in ViewPreimageCases /\ Bug = "drop_view_new_view" ->
      SpecFields(c) \ {"new_view"}
    [] c \in ViewPreimageCases /\ Bug = "drop_view_highest_slot" ->
      SpecFields(c) \ {"highest_slot"}
    [] c \in ViewPreimageCases /\ Bug = "drop_view_chain_order_hash" ->
      SpecFields(c) \ {"chain_order_hash"}
    [] c \in ViewPreimageCases /\ Bug = "include_view_signature" ->
      SpecFields(c) \union {"aggregate_signature"}
    [] c \in ViewPreimageCases /\ Bug = "include_view_bitmap" ->
      SpecFields(c) \union {"signer_bitmap"}
    [] c = "view_vote_unsigned" /\ Bug = "view_vote_drops_body" ->
      SignerFields
    [] c = "view_vote_unsigned" /\ Bug = "view_vote_keeps_signature" ->
      SpecFields(c) \union {"vote_signature"}
    [] c = "suspect_hash" /\ Bug = "suspect_hash_drops_accuser" ->
      SpecFields(c) \ {"accuser"}
    [] c = "suspect_hash" /\ Bug = "suspect_hash_drops_accused" ->
      SpecFields(c) \ {"accused"}
    [] c = "suspect_hash" /\ Bug = "suspect_hash_drops_obligation" ->
      SpecFields(c) \ {"obligation"}
    [] c = "suspect_hash" /\ Bug = "suspect_hash_includes_signature" ->
      SpecFields(c) \union {"vote_signature"}
    [] OTHER -> SpecFields(c)

BugModes == {
  "none",
  "drop_chain_id",
  "drop_mode_tag",
  "drop_vnext_version",
  "use_view_type_for_rechain",
  "use_rechain_type_for_view",
  "drop_rechain_slot",
  "drop_rechain_previous_hash",
  "drop_rechain_new_hash",
  "drop_rechain_new_order",
  "drop_rechain_sequence",
  "drop_rechain_tainted",
  "drop_rechain_suspicions",
  "include_rechain_signature",
  "include_rechain_bitmap",
  "rechain_vote_drops_body",
  "rechain_vote_keeps_signature",
  "drop_view_new_view",
  "drop_view_highest_slot",
  "drop_view_chain_order_hash",
  "include_view_signature",
  "include_view_bitmap",
  "view_vote_drops_body",
  "view_vote_keeps_signature",
  "suspect_hash_drops_accuser",
  "suspect_hash_drops_accused",
  "suspect_hash_drops_obligation",
  "suspect_hash_includes_signature"
}

AllFields ==
  DomainFields
    \union RechainTypeField
    \union ViewTypeField
    \union RechainBodyFields
    \union ViewBodyFields
    \union SignerFields
    \union SignatureAndBitmapFields
    \union SuspectBodyFields

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

PreimageBindsDomain ==
  candidate \in PreimageCases => DomainFields \subseteq fields

RechainPreimageUsesRechainTypeOnly ==
  candidate \in RechainPreimageCases =>
    /\ RechainTypeField \subseteq fields
    /\ fields \cap ViewTypeField = {}

ViewPreimageUsesViewTypeOnly ==
  candidate \in ViewPreimageCases =>
    /\ ViewTypeField \subseteq fields
    /\ fields \cap RechainTypeField = {}

RechainPreimageBindsBody ==
  candidate \in RechainPreimageCases => RechainBodyFields \subseteq fields

ViewPreimageBindsBody ==
  candidate \in ViewPreimageCases => ViewBodyFields \subseteq fields

PreimagesExcludeMutableSignatureMaterial ==
  candidate \in PreimageCases => fields \cap SignatureAndBitmapFields = {}

RechainVoteAndCertificatePreimagesAgree ==
  candidate = "rechain_vote_preimage" =>
    fields = SpecFields("rechain_cert_preimage")

ViewVoteAndCertificatePreimagesAgree ==
  candidate = "view_vote_preimage" =>
    fields = SpecFields("view_cert_preimage")

UnsignedVotesProjectBodyAndSigner ==
  candidate = "rechain_vote_unsigned" =>
    fields = RechainBodyFields \union SignerFields

UnsignedViewVotesProjectBodyAndSigner ==
  candidate = "view_vote_unsigned" =>
    fields = ViewBodyFields \union SignerFields

UnsignedVotesStartWithoutSignature ==
  candidate \in UnsignedVoteCases =>
    /\ "empty_signature" \in fields
    /\ "vote_signature" \notin fields

SuspectHashBindsBody ==
  candidate = "suspect_hash" => fields = SuspectBodyFields

SuspectHashExcludesSignature ==
  candidate = "suspect_hash" => "vote_signature" \notin fields

VNextSigningPreimageExactness ==
  /\ FieldsMatchSpec
  /\ PreimageBindsDomain
  /\ RechainPreimageUsesRechainTypeOnly
  /\ ViewPreimageUsesViewTypeOnly
  /\ RechainPreimageBindsBody
  /\ ViewPreimageBindsBody
  /\ PreimagesExcludeMutableSignatureMaterial
  /\ RechainVoteAndCertificatePreimagesAgree
  /\ ViewVoteAndCertificatePreimagesAgree
  /\ UnsignedVotesProjectBodyAndSigner
  /\ UnsignedViewVotesProjectBodyAndSigner
  /\ UnsignedVotesStartWithoutSignature
  /\ SuspectHashBindsBody
  /\ SuspectHashExcludesSignature

Safety ==
  VNextSigningPreimageExactness

VNextSigningPreimageCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextSigningPreimageExactness

SafetyFast ==
  VNextSigningPreimageExactness

====
