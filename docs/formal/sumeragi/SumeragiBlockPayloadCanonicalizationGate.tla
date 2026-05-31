---- MODULE SumeragiBlockPayloadCanonicalizationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `block_payload_bytes(...)`.

The helper rebuilds the proposal payload from a signed block by clearing the
execution-result Merkle root, recomputing the header transaction Merkle root
from external entrypoints, dropping signatures, and preserving every field that
belongs to the canonical proposal payload. Proposal payload bytes must remain
stable across execution results, signature collection, missing leader
signatures, and stale header roots while still changing whenever canonical
payload fields change.
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
IncludeResultRoot == 1
IncludeSignatures == 2
RequireLeaderSignature == 3
KeepHeaderMerkleRoot == 4
OmitTransactions == 5
OmitExternalEntrypoints == 6
OmitExecutionContext == 7
OmitDaCommitments == 8
OmitDaProofPolicies == 9
OmitDaPinIntents == 10
OmitPreviousRosterEvidence == 11
OmitNposEffects == 12

Bugs == 0..12

Cases == {
  "base",
  "extra_signature",
  "missing_leader_signature",
  "result_root_changed",
  "stale_header_merkle",
  "transactions_changed",
  "external_entrypoints_changed",
  "execution_context_changed",
  "da_commitments_changed",
  "da_proof_policies_changed",
  "da_pin_intents_changed",
  "previous_roster_evidence_changed",
  "npos_effects_changed"
}

StableAsBaseCases == {
  "extra_signature",
  "missing_leader_signature",
  "result_root_changed",
  "stale_header_merkle"
}

FieldChangeCases == Cases \ ({"base"} \cup StableAsBaseCases)

\* @type: Seq(Str);
EmptySeq == <<>>

\* @type: Set(Str);
NoSignatures == {}

\* @type: Str => Seq(Str);
Transactions(c) ==
  IF c = "transactions_changed" THEN <<"txB">> ELSE <<"txA">>

\* @type: Str => Seq(Str);
ExternalEntrypoints(c) ==
  IF c = "external_entrypoints_changed" THEN <<"entryB">> ELSE <<"entryA">>

\* @type: Seq(Str) => Str;
EntryRoot(entries) ==
  CASE entries = <<"entryA">> -> "rootA"
    [] entries = <<"entryB">> -> "rootB"
    [] entries = <<"entryA", "entryB">> -> "rootAB"
    [] entries = EmptySeq -> "rootEmpty"
    [] OTHER -> "rootUnknown"

\* @type: Str => Str;
HeaderMerkleRoot(c) ==
  IF c = "stale_header_merkle" \/ c = "external_entrypoints_changed" THEN
    "staleRoot"
  ELSE
    EntryRoot(ExternalEntrypoints(c))

\* @type: Str => Str;
HeaderResultRoot(c) ==
  IF c = "result_root_changed" THEN "resultRootB" ELSE "resultRootA"

\* @type: Str => Set(Str);
Signatures(c) ==
  IF c = "extra_signature" THEN {"leader", "extra"}
  ELSE IF c = "missing_leader_signature" THEN {"extra"}
  ELSE {"leader"}

\* @type: Str => Bool;
HasLeaderSignature(c) ==
  "leader" \in Signatures(c)

\* @type: Str => Str;
ExecutionContext(c) ==
  IF c = "execution_context_changed" THEN "ctxB" ELSE "ctxA"

\* @type: Str => Seq(Str);
DaCommitments(c) ==
  IF c = "da_commitments_changed" THEN <<"daB">> ELSE <<"daA">>

\* @type: Str => Seq(Str);
DaProofPolicies(c) ==
  IF c = "da_proof_policies_changed" THEN <<"policyB">> ELSE <<"policyA">>

\* @type: Str => Seq(Str);
DaPinIntents(c) ==
  IF c = "da_pin_intents_changed" THEN <<"pinB">> ELSE <<"pinA">>

\* @type: Str => Seq(Str);
PreviousRosterEvidence(c) ==
  IF c = "previous_roster_evidence_changed" THEN <<"rosterB">> ELSE <<"rosterA">>

\* @type: Str => Seq(Str);
NposEffects(c) ==
  IF c = "npos_effects_changed" THEN <<"nposB">> ELSE <<"nposA">>

SpecPayload(c) == [
  available |-> TRUE,
  header_result_root |-> "none",
  header_merkle_root |-> EntryRoot(ExternalEntrypoints(c)),
  transactions |-> Transactions(c),
  external_entrypoints |-> ExternalEntrypoints(c),
  execution_context |-> ExecutionContext(c),
  da_commitments |-> DaCommitments(c),
  da_proof_policies |-> DaProofPolicies(c),
  da_pin_intents |-> DaPinIntents(c),
  previous_roster_evidence |-> PreviousRosterEvidence(c),
  npos_consensus_effects |-> NposEffects(c),
  signatures |-> NoSignatures
]

ActualPayload(c) ==
  CASE Bug = IncludeResultRoot ->
       [SpecPayload(c) EXCEPT !.header_result_root = HeaderResultRoot(c)]
    [] Bug = IncludeSignatures ->
       [SpecPayload(c) EXCEPT !.signatures = Signatures(c)]
    [] Bug = RequireLeaderSignature ->
       [SpecPayload(c) EXCEPT !.available = HasLeaderSignature(c)]
    [] Bug = KeepHeaderMerkleRoot ->
       [SpecPayload(c) EXCEPT !.header_merkle_root = HeaderMerkleRoot(c)]
    [] Bug = OmitTransactions ->
       [SpecPayload(c) EXCEPT !.transactions = EmptySeq]
    [] Bug = OmitExternalEntrypoints ->
       [SpecPayload(c) EXCEPT
          !.external_entrypoints = EmptySeq,
          !.header_merkle_root = EntryRoot(EmptySeq)]
    [] Bug = OmitExecutionContext ->
       [SpecPayload(c) EXCEPT !.execution_context = "none"]
    [] Bug = OmitDaCommitments ->
       [SpecPayload(c) EXCEPT !.da_commitments = EmptySeq]
    [] Bug = OmitDaProofPolicies ->
       [SpecPayload(c) EXCEPT !.da_proof_policies = EmptySeq]
    [] Bug = OmitDaPinIntents ->
       [SpecPayload(c) EXCEPT !.da_pin_intents = EmptySeq]
    [] Bug = OmitPreviousRosterEvidence ->
       [SpecPayload(c) EXCEPT !.previous_roster_evidence = EmptySeq]
    [] Bug = OmitNposEffects ->
       [SpecPayload(c) EXCEPT !.npos_consensus_effects = EmptySeq]
    [] OTHER -> SpecPayload(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ StableAsBaseCases \subseteq Cases
  /\ FieldChangeCases \subseteq Cases
  /\ \A c \in Cases:
       /\ ActualPayload(c).available \in BOOLEAN
       /\ ActualPayload(c).header_result_root \in {
            "none",
            "resultRootA",
            "resultRootB"
          }
       /\ ActualPayload(c).header_merkle_root \in {
            "rootA",
            "rootB",
            "rootAB",
            "rootEmpty",
            "rootUnknown",
            "staleRoot"
          }
       /\ ActualPayload(c).transactions \in {EmptySeq, <<"txA">>, <<"txB">>}
       /\ ActualPayload(c).external_entrypoints \in {
            EmptySeq,
            <<"entryA">>,
            <<"entryB">>
          }
       /\ ActualPayload(c).execution_context \in {"none", "ctxA", "ctxB"}
       /\ ActualPayload(c).da_commitments \in {EmptySeq, <<"daA">>, <<"daB">>}
       /\ ActualPayload(c).da_proof_policies \in {
            EmptySeq,
            <<"policyA">>,
            <<"policyB">>
          }
       /\ ActualPayload(c).da_pin_intents \in {EmptySeq, <<"pinA">>, <<"pinB">>}
       /\ ActualPayload(c).previous_roster_evidence \in {
            EmptySeq,
            <<"rosterA">>,
            <<"rosterB">>
          }
       /\ ActualPayload(c).npos_consensus_effects \in {
            EmptySeq,
            <<"nposA">>,
            <<"nposB">>
          }
       /\ ActualPayload(c).signatures \in {
            NoSignatures,
            {"leader"},
            {"extra"},
            {"leader", "extra"}
          }

PayloadMatchesSpec ==
  \A c \in Cases:
    ActualPayload(c) = SpecPayload(c)

ResultsSignaturesAndHeaderRootIgnored ==
  \A c \in StableAsBaseCases:
    ActualPayload(c) = ActualPayload("base")

PayloadFieldsAreBound ==
  \A c \in FieldChangeCases:
    ActualPayload(c) # ActualPayload("base")

SafetyFast ==
  /\ PayloadMatchesSpec
  /\ ResultsSignaturesAndHeaderRootIgnored
  /\ PayloadFieldsAreBound

====
