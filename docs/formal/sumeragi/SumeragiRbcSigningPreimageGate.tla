---- MODULE SumeragiRbcSigningPreimageGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC signing preimage construction.

RBC READY and DELIVER messages are signed independently from ordinary
prepare/commit votes. The concrete helpers are
`rbc_ready_preimage(...)` and `rbc_deliver_preimage(...)`. This model captures
the fields that must be bound into those preimages: chain id, Sumeragi mode
tag, v1 message domain, block hash, height, view, epoch, roster hash, chunk
root, sender index, and DELIVER's embedded READY-signature bundle. The message's
own signature is mutable transport material and must stay outside the preimage.
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

Cases == {"ready_preimage", "deliver_empty", "deliver_bundle"}

DeliverCases == {"deliver_empty", "deliver_bundle"}

DomainFields == {"chain_id", "mode_tag", "v1"}

ReadyTypeField == {"type_rbc_ready"}

DeliverTypeField == {"type_rbc_deliver"}

SubjectFields == {
  "block_hash",
  "height",
  "view",
  "epoch",
  "roster_hash",
  "chunk_root",
  "sender"
}

ReadyBundleFields == {
  "ready_count",
  "ready_entry_order",
  "ready_entry_sender",
  "ready_entry_sig_len",
  "ready_entry_signature"
}

SelfSignatureFields == {"ready_signature", "deliver_signature"}

SpecFields(c) ==
  CASE c = "ready_preimage" ->
      DomainFields \union ReadyTypeField \union SubjectFields
    [] c = "deliver_empty" ->
      DomainFields \union DeliverTypeField \union SubjectFields \union {"ready_count"}
    [] c = "deliver_bundle" ->
      DomainFields \union DeliverTypeField \union SubjectFields \union ReadyBundleFields
    [] OTHER -> {}

ActualFields(c) ==
  CASE c \in Cases /\ Bug = "drop_chain_id" ->
      SpecFields(c) \ {"chain_id"}
    [] c \in Cases /\ Bug = "drop_mode_tag" ->
      SpecFields(c) \ {"mode_tag"}
    [] c \in Cases /\ Bug = "drop_version" ->
      SpecFields(c) \ {"v1"}
    [] c = "ready_preimage" /\ Bug = "ready_uses_deliver_type" ->
      (SpecFields(c) \ ReadyTypeField) \union DeliverTypeField
    [] c \in DeliverCases /\ Bug = "deliver_uses_ready_type" ->
      (SpecFields(c) \ DeliverTypeField) \union ReadyTypeField
    [] c \in Cases /\ Bug = "drop_block_hash" ->
      SpecFields(c) \ {"block_hash"}
    [] c \in Cases /\ Bug = "drop_height" ->
      SpecFields(c) \ {"height"}
    [] c \in Cases /\ Bug = "drop_view" ->
      SpecFields(c) \ {"view"}
    [] c \in Cases /\ Bug = "drop_epoch" ->
      SpecFields(c) \ {"epoch"}
    [] c \in Cases /\ Bug = "drop_roster_hash" ->
      SpecFields(c) \ {"roster_hash"}
    [] c \in Cases /\ Bug = "drop_chunk_root" ->
      SpecFields(c) \ {"chunk_root"}
    [] c \in Cases /\ Bug = "drop_sender" ->
      SpecFields(c) \ {"sender"}
    [] c = "ready_preimage" /\ Bug = "ready_includes_signature" ->
      SpecFields(c) \union {"ready_signature"}
    [] c \in DeliverCases /\ Bug = "deliver_includes_signature" ->
      SpecFields(c) \union {"deliver_signature"}
    [] c \in DeliverCases /\ Bug = "deliver_omits_ready_count" ->
      SpecFields(c) \ {"ready_count"}
    [] c = "deliver_bundle" /\ Bug = "deliver_omits_ready_bundle" ->
      SpecFields(c) \ ReadyBundleFields
    [] c = "deliver_bundle" /\ Bug = "deliver_omits_entry_order" ->
      SpecFields(c) \ {"ready_entry_order"}
    [] c = "deliver_bundle" /\ Bug = "deliver_omits_entry_sender" ->
      SpecFields(c) \ {"ready_entry_sender"}
    [] c = "deliver_bundle" /\ Bug = "deliver_omits_entry_sig_len" ->
      SpecFields(c) \ {"ready_entry_sig_len"}
    [] c = "deliver_bundle" /\ Bug = "deliver_omits_entry_signature" ->
      SpecFields(c) \ {"ready_entry_signature"}
    [] OTHER -> SpecFields(c)

BugModes == {
  "none",
  "drop_chain_id",
  "drop_mode_tag",
  "drop_version",
  "ready_uses_deliver_type",
  "deliver_uses_ready_type",
  "drop_block_hash",
  "drop_height",
  "drop_view",
  "drop_epoch",
  "drop_roster_hash",
  "drop_chunk_root",
  "drop_sender",
  "ready_includes_signature",
  "deliver_includes_signature",
  "deliver_omits_ready_count",
  "deliver_omits_ready_bundle",
  "deliver_omits_entry_order",
  "deliver_omits_entry_sender",
  "deliver_omits_entry_sig_len",
  "deliver_omits_entry_signature"
}

AllFields ==
  DomainFields
    \union ReadyTypeField
    \union DeliverTypeField
    \union SubjectFields
    \union ReadyBundleFields
    \union SelfSignatureFields

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

ReadyUsesReadyTypeOnly ==
  candidate = "ready_preimage" =>
    /\ ReadyTypeField \subseteq fields
    /\ fields \cap DeliverTypeField = {}

DeliverUsesDeliverTypeOnly ==
  candidate \in DeliverCases =>
    /\ DeliverTypeField \subseteq fields
    /\ fields \cap ReadyTypeField = {}

PreimagesBindSubject ==
  candidate \in Cases => SubjectFields \subseteq fields

PreimagesExcludeSelfSignatures ==
  candidate \in Cases => fields \cap SelfSignatureFields = {}

DeliverBindsReadyCount ==
  candidate \in DeliverCases => "ready_count" \in fields

EmptyDeliverHasNoReadyEntries ==
  candidate = "deliver_empty" =>
    fields \cap (ReadyBundleFields \ {"ready_count"}) = {}

BundledDeliverBindsReadyEntries ==
  candidate = "deliver_bundle" => ReadyBundleFields \subseteq fields

Safety ==
  /\ FieldsMatchSpec
  /\ PreimagesBindDomain
  /\ ReadyUsesReadyTypeOnly
  /\ DeliverUsesDeliverTypeOnly
  /\ PreimagesBindSubject
  /\ PreimagesExcludeSelfSignatures
  /\ DeliverBindsReadyCount
  /\ EmptyDeliverHasNoReadyEntries
  /\ BundledDeliverBindsReadyEntries

AllConcretePreimagesMatchSpec ==
  \A c \in Cases:
    ActualFields(c) = SpecFields(c)

AllConcretePreimagesBindDomain ==
  \A c \in Cases:
    DomainFields \subseteq ActualFields(c)

ConcreteReadyTypeAnchors ==
  /\ ReadyTypeField \subseteq ActualFields("ready_preimage")
  /\ ActualFields("ready_preimage") \cap DeliverTypeField = {}

ConcreteDeliverTypeAnchors ==
  \A c \in DeliverCases:
    /\ DeliverTypeField \subseteq ActualFields(c)
    /\ ActualFields(c) \cap ReadyTypeField = {}

AllConcretePreimagesBindSubject ==
  \A c \in Cases:
    SubjectFields \subseteq ActualFields(c)

AllConcretePreimagesExcludeSelfSignatures ==
  \A c \in Cases:
    ActualFields(c) \cap SelfSignatureFields = {}

DeliverReadyCountAnchors ==
  \A c \in DeliverCases:
    "ready_count" \in ActualFields(c)

EmptyDeliverEntryAnchors ==
  ActualFields("deliver_empty") \cap (ReadyBundleFields \ {"ready_count"}) = {}

BundledDeliverEntryAnchors ==
  ReadyBundleFields \subseteq ActualFields("deliver_bundle")

RbcPreimageSafetyAnchors ==
  /\ AllConcretePreimagesMatchSpec
  /\ AllConcretePreimagesBindDomain
  /\ ConcreteReadyTypeAnchors
  /\ ConcreteDeliverTypeAnchors
  /\ AllConcretePreimagesBindSubject
  /\ AllConcretePreimagesExcludeSelfSignatures
  /\ DeliverReadyCountAnchors
  /\ EmptyDeliverEntryAnchors
  /\ BundledDeliverEntryAnchors

RbcSigningPreimageCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ RbcPreimageSafetyAnchors

====
