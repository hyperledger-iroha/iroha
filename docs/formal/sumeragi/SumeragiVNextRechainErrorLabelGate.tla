---- MODULE SumeragiVNextRechainErrorLabelGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `rechain_error_label(...)`.

Re-chain validation labels are observable through vNext control/status paths.
Each `RechainError` variant must map to one stable string label. Variants that
carry payload fields, such as the expected successor or untainted-validator
counts, must not include those dynamic payload values in the label.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WrongLabel == "wrong_label"

EmptyEvidence == "EmptyEvidence"
CanonicalEvidenceEncoding == "CanonicalEvidenceEncoding"
DuplicateEvidence == "DuplicateEvidence"
SlotMismatch == "SlotMismatch"
ChainOrderHashMismatch == "ChainOrderHashMismatch"
RechainSequenceMismatch == "RechainSequenceMismatch"
AccuserHasNoCriticalSuccessor == "AccuserHasNoCriticalSuccessor"
AccusedIsNotSuccessorA == "AccusedIsNotSuccessorA"
AccusedIsNotSuccessorB == "AccusedIsNotSuccessorB"
InsufficientUntaintedValidatorsLow == "InsufficientUntaintedValidatorsLow"
InsufficientUntaintedValidatorsHigh == "InsufficientUntaintedValidatorsHigh"
InsufficientQuorumAfterQuarantine == "InsufficientQuorumAfterQuarantine"
RechainSequenceExhausted == "RechainSequenceExhausted"

Cases == {
  EmptyEvidence,
  CanonicalEvidenceEncoding,
  DuplicateEvidence,
  SlotMismatch,
  ChainOrderHashMismatch,
  RechainSequenceMismatch,
  AccuserHasNoCriticalSuccessor,
  AccusedIsNotSuccessorA,
  AccusedIsNotSuccessorB,
  InsufficientUntaintedValidatorsLow,
  InsufficientUntaintedValidatorsHigh,
  InsufficientQuorumAfterQuarantine,
  RechainSequenceExhausted
}

LabelValues == {
  "empty_evidence",
  "canonical_evidence_encoding",
  "duplicate_evidence",
  "slot_mismatch",
  "chain_order_hash_mismatch",
  "rechain_sequence_mismatch",
  "accuser_has_no_critical_successor",
  "accused_is_not_successor",
  "insufficient_untainted_validators",
  "insufficient_quorum_after_quarantine",
  "rechain_sequence_exhausted"
}

AllLabelValues == LabelValues \cup {
  WrongLabel,
  "accused_is_not_successor_a",
  "accused_is_not_successor_b",
  "insufficient_untainted_validators_low",
  "insufficient_untainted_validators_high"
}

\* @type: Str => Str;
SpecLabel(c) ==
  CASE c = EmptyEvidence -> "empty_evidence"
    [] c = CanonicalEvidenceEncoding -> "canonical_evidence_encoding"
    [] c = DuplicateEvidence -> "duplicate_evidence"
    [] c = SlotMismatch -> "slot_mismatch"
    [] c = ChainOrderHashMismatch -> "chain_order_hash_mismatch"
    [] c = RechainSequenceMismatch -> "rechain_sequence_mismatch"
    [] c = AccuserHasNoCriticalSuccessor -> "accuser_has_no_critical_successor"
    [] c \in {AccusedIsNotSuccessorA, AccusedIsNotSuccessorB} ->
       "accused_is_not_successor"
    [] c \in {
         InsufficientUntaintedValidatorsLow,
         InsufficientUntaintedValidatorsHigh
       } -> "insufficient_untainted_validators"
    [] c = InsufficientQuorumAfterQuarantine ->
       "insufficient_quorum_after_quarantine"
    [] c = RechainSequenceExhausted -> "rechain_sequence_exhausted"
    [] OTHER -> WrongLabel

\* @type: Str => Str;
ActualLabel(c) ==
  CASE Bug = "empty_evidence_label_wrong"
       /\ c = EmptyEvidence -> WrongLabel
    [] Bug = "canonical_evidence_label_wrong"
       /\ c = CanonicalEvidenceEncoding -> WrongLabel
    [] Bug = "duplicate_evidence_label_wrong"
       /\ c = DuplicateEvidence -> WrongLabel
    [] Bug = "slot_mismatch_label_wrong"
       /\ c = SlotMismatch -> WrongLabel
    [] Bug = "chain_order_hash_label_wrong"
       /\ c = ChainOrderHashMismatch -> WrongLabel
    [] Bug = "rechain_sequence_label_wrong"
       /\ c = RechainSequenceMismatch -> WrongLabel
    [] Bug = "accuser_no_successor_label_wrong"
       /\ c = AccuserHasNoCriticalSuccessor -> WrongLabel
    [] Bug = "accused_not_successor_label_wrong"
       /\ c = AccusedIsNotSuccessorA -> WrongLabel
    [] Bug = "insufficient_untainted_label_wrong"
       /\ c = InsufficientUntaintedValidatorsLow -> WrongLabel
    [] Bug = "insufficient_quorum_label_wrong"
       /\ c = InsufficientQuorumAfterQuarantine -> WrongLabel
    [] Bug = "sequence_exhausted_label_wrong"
       /\ c = RechainSequenceExhausted -> WrongLabel
    [] Bug = "accused_payload_leaks_label"
       /\ c = AccusedIsNotSuccessorA -> "accused_is_not_successor_a"
    [] Bug = "accused_payload_leaks_label"
       /\ c = AccusedIsNotSuccessorB -> "accused_is_not_successor_b"
    [] Bug = "untainted_payload_leaks_label"
       /\ c = InsufficientUntaintedValidatorsLow ->
       "insufficient_untainted_validators_low"
    [] Bug = "untainted_payload_leaks_label"
       /\ c = InsufficientUntaintedValidatorsHigh ->
       "insufficient_untainted_validators_high"
    [] OTHER -> SpecLabel(c)

Matches(c) ==
  ActualLabel(c) = SpecLabel(c)

PayloadLabelsStable ==
  /\ SpecLabel(AccusedIsNotSuccessorA) = SpecLabel(AccusedIsNotSuccessorB)
  /\ SpecLabel(InsufficientUntaintedValidatorsLow)
     = SpecLabel(InsufficientUntaintedValidatorsHigh)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_evidence_label_wrong",
       "canonical_evidence_label_wrong",
       "duplicate_evidence_label_wrong",
       "slot_mismatch_label_wrong",
       "chain_order_hash_label_wrong",
       "rechain_sequence_label_wrong",
       "accuser_no_successor_label_wrong",
       "accused_not_successor_label_wrong",
       "insufficient_untainted_label_wrong",
       "insufficient_quorum_label_wrong",
       "sequence_exhausted_label_wrong",
       "accused_payload_leaks_label",
       "untainted_payload_leaks_label"
     }
  /\ checked = 0
  /\ \A c \in Cases: SpecLabel(c) \in LabelValues
  /\ \A c \in Cases: ActualLabel(c) \in AllLabelValues

VNextRechainErrorLabelCoreSafety ==
  /\ \A c \in Cases: Matches(c)
  /\ PayloadLabelsStable

VNextRechainErrorLabelExactness ==
  /\ \A c \in Cases: Matches(c)
  /\ PayloadLabelsStable
VNextRechainErrorLabelCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextRechainErrorLabelExactness

SafetyFast ==
  VNextRechainErrorLabelExactness

BugEmptyEvidenceLabelWrong ==
  Matches(EmptyEvidence)

BugCanonicalEvidenceLabelWrong ==
  Matches(CanonicalEvidenceEncoding)

BugDuplicateEvidenceLabelWrong ==
  Matches(DuplicateEvidence)

BugSlotMismatchLabelWrong ==
  Matches(SlotMismatch)

BugChainOrderHashLabelWrong ==
  Matches(ChainOrderHashMismatch)

BugRechainSequenceLabelWrong ==
  Matches(RechainSequenceMismatch)

BugAccuserNoSuccessorLabelWrong ==
  Matches(AccuserHasNoCriticalSuccessor)

BugAccusedNotSuccessorLabelWrong ==
  Matches(AccusedIsNotSuccessorA)

BugInsufficientUntaintedLabelWrong ==
  Matches(InsufficientUntaintedValidatorsLow)

BugInsufficientQuorumLabelWrong ==
  Matches(InsufficientQuorumAfterQuarantine)

BugSequenceExhaustedLabelWrong ==
  Matches(RechainSequenceExhausted)

BugAccusedPayloadLeaksLabel ==
  /\ Matches(AccusedIsNotSuccessorA)
  /\ Matches(AccusedIsNotSuccessorB)

BugUntaintedPayloadLeaksLabel ==
  /\ Matches(InsufficientUntaintedValidatorsLow)
  /\ Matches(InsufficientUntaintedValidatorsHigh)

====
