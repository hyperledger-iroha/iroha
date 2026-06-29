---- MODULE SumeragiVrfMaterialDerivationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for VRF material derivation.

`derive_vrf_material_from_key(...)` builds a deterministic message from the
VRF input domain, chain hash, epoch, and validator index; signs that message
with the local private key; derives the reveal by hashing the signature
payload; derives the commitment by hashing the reveal; and returns the pair as
`(reveal, commitment)`. This model captures the construction contract without
modeling the cryptographic primitives themselves.
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

Cases == {"derive"}

MessageRequiredFields == {
  "domain_separator",
  "chain_hash",
  "epoch_be",
  "signer_u64_be"
}

MessageOrderFields == {
  "domain_before_chain",
  "chain_before_epoch",
  "epoch_before_signer"
}

SignatureFields == {
  "signature_from_private_key",
  "signature_over_message"
}

RevealFields == {"reveal_hash_signature_payload"}

CommitmentFields == {"commitment_hash_reveal"}

ReturnFields == {
  "return_reveal_first",
  "return_commitment_second"
}

BadFields == {
  "epoch_le",
  "signer_u64_le",
  "signer_without_u64",
  "signer_before_epoch",
  "signature_over_chain_only",
  "reveal_hash_message",
  "reveal_raw_signature",
  "commitment_hash_signature",
  "commitment_hash_message",
  "return_commitment_first",
  "return_reveal_second",
  "raw_signature_returned",
  "message_returned"
}

SpecFields(c) ==
  CASE c = "derive" ->
      MessageRequiredFields
        \union MessageOrderFields
        \union SignatureFields
        \union RevealFields
        \union CommitmentFields
        \union ReturnFields
    [] OTHER -> {}

ActualFields(c) ==
  CASE c = "derive" /\ Bug = "drop_domain_separator" ->
      SpecFields(c) \ {"domain_separator"}
    [] c = "derive" /\ Bug = "drop_chain_hash" ->
      SpecFields(c) \ {"chain_hash"}
    [] c = "derive" /\ Bug = "drop_epoch" ->
      SpecFields(c) \ {"epoch_be"}
    [] c = "derive" /\ Bug = "drop_signer" ->
      SpecFields(c) \ {"signer_u64_be"}
    [] c = "derive" /\ Bug = "epoch_little_endian" ->
      (SpecFields(c) \ {"epoch_be"}) \union {"epoch_le"}
    [] c = "derive" /\ Bug = "signer_little_endian" ->
      (SpecFields(c) \ {"signer_u64_be"}) \union {"signer_u64_le"}
    [] c = "derive" /\ Bug = "signer_without_u64" ->
      (SpecFields(c) \ {"signer_u64_be"}) \union {"signer_without_u64"}
    [] c = "derive" /\ Bug = "swap_epoch_signer_order" ->
      (SpecFields(c) \ {"epoch_before_signer"}) \union {"signer_before_epoch"}
    [] c = "derive" /\ Bug = "signature_over_chain_only" ->
      (SpecFields(c) \ {"signature_over_message"}) \union {"signature_over_chain_only"}
    [] c = "derive" /\ Bug = "reveal_hashes_message" ->
      (SpecFields(c) \ RevealFields) \union {"reveal_hash_message"}
    [] c = "derive" /\ Bug = "reveal_returns_signature_payload" ->
      (SpecFields(c) \ RevealFields) \union {"reveal_raw_signature"}
    [] c = "derive" /\ Bug = "commitment_hashes_signature" ->
      (SpecFields(c) \ CommitmentFields) \union {"commitment_hash_signature"}
    [] c = "derive" /\ Bug = "commitment_hashes_message" ->
      (SpecFields(c) \ CommitmentFields) \union {"commitment_hash_message"}
    [] c = "derive" /\ Bug = "return_swapped_material" ->
      (SpecFields(c) \ ReturnFields) \union {"return_commitment_first", "return_reveal_second"}
    [] c = "derive" /\ Bug = "return_omits_commitment" ->
      SpecFields(c) \ {"return_commitment_second"}
    [] c = "derive" /\ Bug = "include_raw_signature_output" ->
      SpecFields(c) \union {"raw_signature_returned"}
    [] c = "derive" /\ Bug = "include_message_output" ->
      SpecFields(c) \union {"message_returned"}
    [] OTHER -> SpecFields(c)

BugModes == {
  "none",
  "drop_domain_separator",
  "drop_chain_hash",
  "drop_epoch",
  "drop_signer",
  "epoch_little_endian",
  "signer_little_endian",
  "signer_without_u64",
  "swap_epoch_signer_order",
  "signature_over_chain_only",
  "reveal_hashes_message",
  "reveal_returns_signature_payload",
  "commitment_hashes_signature",
  "commitment_hashes_message",
  "return_swapped_material",
  "return_omits_commitment",
  "include_raw_signature_output",
  "include_message_output"
}

AllFields ==
  MessageRequiredFields
    \union MessageOrderFields
    \union SignatureFields
    \union RevealFields
    \union CommitmentFields
    \union ReturnFields
    \union BadFields

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

MessageBindsRequiredInputs ==
  candidate = "derive" => MessageRequiredFields \subseteq fields

MessageUsesBigEndianContext ==
  candidate = "derive" =>
    /\ "epoch_be" \in fields
    /\ "signer_u64_be" \in fields
    /\ fields \cap {"epoch_le", "signer_u64_le", "signer_without_u64"} = {}

MessageOrderStable ==
  candidate = "derive" =>
    /\ MessageOrderFields \subseteq fields
    /\ fields \cap {"signer_before_epoch"} = {}

SignatureBindsPrivateKeyAndMessage ==
  candidate = "derive" =>
    /\ SignatureFields \subseteq fields
    /\ fields \cap {"signature_over_chain_only"} = {}

RevealHashesSignaturePayload ==
  candidate = "derive" =>
    /\ RevealFields \subseteq fields
    /\ fields \cap {"reveal_hash_message", "reveal_raw_signature"} = {}

CommitmentHashesReveal ==
  candidate = "derive" =>
    /\ CommitmentFields \subseteq fields
    /\ fields \cap {"commitment_hash_signature", "commitment_hash_message"} = {}

ReturnShapeStable ==
  candidate = "derive" =>
    /\ ReturnFields \subseteq fields
    /\ fields \cap {"return_commitment_first", "return_reveal_second"} = {}

NoRawIntermediateOutputs ==
  candidate = "derive" =>
    fields \cap {"raw_signature_returned", "message_returned"} = {}

VrfMaterialDerivationExactness ==
  /\ FieldsMatchSpec
  /\ MessageBindsRequiredInputs
  /\ MessageUsesBigEndianContext
  /\ MessageOrderStable
  /\ SignatureBindsPrivateKeyAndMessage
  /\ RevealHashesSignaturePayload
  /\ CommitmentHashesReveal
  /\ ReturnShapeStable
  /\ NoRawIntermediateOutputs

VrfMaterialDerivationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VrfMaterialDerivationExactness

NoBugInvariant == VrfMaterialDerivationExactness

Safety ==
  VrfMaterialDerivationExactness

====
