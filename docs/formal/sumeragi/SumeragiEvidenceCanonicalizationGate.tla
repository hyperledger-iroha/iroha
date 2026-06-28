---- MODULE SumeragiEvidenceCanonicalizationGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi evidence canonicalization and
deduplication helpers.

This slice pins:
- `evidence_key(...)` / `canonicalize_evidence(...)`, including
  order-insensitive double-vote and censorship-receipt keys while preserving
  evidence kind and payload bytes,
- `evidence_subject_height_view(...)` for double votes, invalid QCs, invalid
  proposals, and censorship receipts,
- `evidence_block_refs(...)`, including the same-hash double-commit root
  conflict case that should not duplicate the same block reference,
- `EvidenceStore::insert(...)` and `persist_record(...)` deduplication over the
  canonical evidence key, validation-before-insert, canonical storage, subject
  height/view defaults, and unset penalty flags.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

KeyCases == {
  "double_ordered",
  "double_swapped",
  "double_phase_order",
  "double_block_order",
  "double_root_order",
  "censorship_ordered",
  "censorship_swapped",
  "censorship_duplicate_receipts",
  "invalid_qc",
  "invalid_proposal",
  "same_payload_different_kind"
}

SubjectCases == {
  "double_vote",
  "invalid_qc",
  "invalid_proposal",
  "censorship_empty",
  "censorship_one",
  "censorship_many"
}

RefCases == {
  "double_vote_distinct",
  "double_vote_same_hash_root_conflict",
  "invalid_qc",
  "invalid_proposal",
  "censorship"
}

StoreCases == {
  "valid_new",
  "valid_duplicate",
  "valid_swapped_duplicate",
  "invalid_new",
  "second_distinct"
}

PersistCases == {
  "valid_new",
  "existing_duplicate",
  "invalid",
  "missing_subject",
  "censorship_subject",
  "swapped_duplicate"
}

SpecKey(c) ==
  CASE c \in {"double_ordered", "double_swapped"} -> "kind_double_prepare|vote_a|vote_b"
    [] c = "double_phase_order" -> "kind_double_commit|prepare_vote|commit_vote"
    [] c = "double_block_order" -> "kind_double_prepare|block_1|block_2"
    [] c = "double_root_order" -> "kind_double_commit|root_1|root_2"
    [] c \in {"censorship_ordered", "censorship_swapped"} ->
         "kind_censorship|receipt_a|receipt_b"
    [] c = "censorship_duplicate_receipts" ->
         "kind_censorship|receipt_a|receipt_a|receipt_b"
    [] c = "invalid_qc" -> "kind_invalid_qc|qc_payload"
    [] c = "invalid_proposal" -> "kind_invalid_proposal|proposal_payload"
    [] c = "same_payload_different_kind" -> "kind_invalid_qc|shared_payload"
    [] OTHER -> "unknown"

ActualKey(c) ==
  CASE Bug = "double_swapped_not_canonical"
       /\ c = "double_swapped" -> "kind_double_prepare|vote_b|vote_a"
    [] Bug = "double_phase_order_wrong"
       /\ c = "double_phase_order" -> "kind_double_commit|commit_vote|prepare_vote"
    [] Bug = "double_block_order_wrong"
       /\ c = "double_block_order" -> "kind_double_prepare|block_2|block_1"
    [] Bug = "double_root_order_wrong"
       /\ c = "double_root_order" -> "kind_double_commit|root_2|root_1"
    [] Bug = "censorship_swapped_not_canonical"
       /\ c = "censorship_swapped" -> "kind_censorship|receipt_b|receipt_a"
    [] Bug = "censorship_dedups_receipts"
       /\ c = "censorship_duplicate_receipts" -> "kind_censorship|receipt_a|receipt_b"
    [] Bug = "key_omits_kind"
       /\ c = "same_payload_different_kind" -> "shared_payload"
    [] Bug = "key_omits_payload"
       /\ c = "invalid_qc" -> "kind_invalid_qc"
    [] Bug = "canonical_mutates_invalid_qc"
       /\ c = "invalid_qc" -> "kind_invalid_qc|rewritten_qc_payload"
    [] Bug = "canonical_mutates_invalid_proposal"
       /\ c = "invalid_proposal" -> "kind_invalid_proposal|rewritten_proposal_payload"
    [] OTHER -> SpecKey(c)

SpecSubjectHeight(c) ==
  CASE c = "double_vote" -> 10
    [] c = "invalid_qc" -> 8
    [] c = "invalid_proposal" -> 11
    [] c = "censorship_empty" -> -1
    [] c = "censorship_one" -> 6
    [] c = "censorship_many" -> 12
    [] OTHER -> -1

SpecSubjectView(c) ==
  CASE c = "double_vote" -> 3
    [] c = "invalid_qc" -> 2
    [] c = "invalid_proposal" -> 5
    [] OTHER -> -1

ActualSubjectHeight(c) ==
  CASE Bug = "subject_double_uses_second"
       /\ c = "double_vote" -> 9
    [] Bug = "subject_invalid_qc_missing"
       /\ c = "invalid_qc" -> -1
    [] Bug = "subject_proposal_uses_qc"
       /\ c = "invalid_proposal" -> 10
    [] Bug = "subject_censorship_uses_min"
       /\ c = "censorship_many" -> 4
    [] Bug = "subject_empty_censorship_zero"
       /\ c = "censorship_empty" -> 0
    [] OTHER -> SpecSubjectHeight(c)

ActualSubjectView(c) ==
  CASE Bug = "subject_censorship_sets_view"
       /\ c = "censorship_one" -> 1
    [] Bug = "subject_double_uses_second"
       /\ c = "double_vote" -> 4
    [] OTHER -> SpecSubjectView(c)

SpecRefs(c) ==
  CASE c = "double_vote_distinct" -> {"h10_a", "h10_b"}
    [] c = "double_vote_same_hash_root_conflict" -> {"h10_a"}
    [] c = "invalid_qc" -> {"h8_qc"}
    [] OTHER -> {}

ActualRefs(c) ==
  CASE Bug = "refs_double_drops_second"
       /\ c = "double_vote_distinct" -> {"h10_a"}
    [] Bug = "refs_double_duplicates_same_hash"
       /\ c = "double_vote_same_hash_root_conflict" -> {"h10_a", "h10_a_dup"}
    [] Bug = "refs_invalid_qc_empty"
       /\ c = "invalid_qc" -> {}
    [] Bug = "refs_proposal_included"
       /\ c = "invalid_proposal" -> {"proposal_parent"}
    [] Bug = "refs_censorship_included"
       /\ c = "censorship" -> {"tx_receipt"}
    [] OTHER -> SpecRefs(c)

SpecStoreInserted(c) ==
  c \in {"valid_new", "second_distinct"}

ActualStoreInserted(c) ==
  CASE Bug = "store_accepts_invalid"
       /\ c = "invalid_new" -> TRUE
    [] Bug = "store_duplicate_inserted"
       /\ c = "valid_duplicate" -> TRUE
    [] Bug = "store_swapped_inserted"
       /\ c = "valid_swapped_duplicate" -> TRUE
    [] Bug = "store_rejects_new_valid"
       /\ c = "valid_new" -> FALSE
    [] OTHER -> SpecStoreInserted(c)

SpecStoreKey(c) ==
  CASE c = "valid_swapped_duplicate" -> "kind_double_prepare|vote_a|vote_b"
    [] c = "valid_duplicate" -> "kind_double_prepare|vote_a|vote_b"
    [] c = "valid_new" -> "kind_double_prepare|vote_a|vote_b"
    [] c = "second_distinct" -> "kind_double_prepare|vote_a|vote_c"
    [] OTHER -> "none"

ActualStoreKey(c) ==
  CASE Bug = "store_skips_canonicalization"
       /\ c = "valid_swapped_duplicate" -> "kind_double_prepare|vote_b|vote_a"
    [] Bug = "store_omits_kind"
       /\ c = "valid_new" -> "vote_a|vote_b"
    [] OTHER -> SpecStoreKey(c)

SpecPersistInserted(c) ==
  c \in {"valid_new", "missing_subject", "censorship_subject"}

ActualPersistInserted(c) ==
  CASE Bug = "persist_accepts_invalid"
       /\ c = "invalid" -> TRUE
    [] Bug = "persist_duplicate_inserted"
       /\ c = "existing_duplicate" -> TRUE
    [] Bug = "persist_swapped_duplicate_inserted"
       /\ c = "swapped_duplicate" -> TRUE
    [] Bug = "persist_rejects_missing_subject"
       /\ c = "missing_subject" -> FALSE
    [] OTHER -> SpecPersistInserted(c)

SpecRecordedHeight(c) ==
  CASE c = "valid_new" -> 10
    [] c = "missing_subject" -> 20
    [] c = "censorship_subject" -> 12
    [] OTHER -> -1

ActualRecordedHeight(c) ==
  CASE Bug = "persist_missing_subject_zero_height"
       /\ c = "missing_subject" -> 0
    [] Bug = "persist_censorship_uses_current_height"
       /\ c = "censorship_subject" -> 20
    [] OTHER -> SpecRecordedHeight(c)

SpecRecordedView(c) ==
  CASE c = "valid_new" -> 3
    [] OTHER -> 0

ActualRecordedView(c) ==
  CASE Bug = "persist_missing_subject_nonzero_view"
       /\ c = "missing_subject" -> 7
    [] Bug = "persist_censorship_nonzero_view"
       /\ c = "censorship_subject" -> 5
    [] OTHER -> SpecRecordedView(c)

SpecPenaltyFlagsClear(c) == TRUE

ActualPenaltyFlagsClear(c) ==
  CASE Bug = "persist_sets_penalty_applied"
       /\ c = "valid_new" -> FALSE
    [] Bug = "persist_sets_penalty_cancelled"
       /\ c = "valid_new" -> FALSE
    [] OTHER -> TRUE

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "double_swapped_not_canonical",
       "double_phase_order_wrong",
       "double_block_order_wrong",
       "double_root_order_wrong",
       "censorship_swapped_not_canonical",
       "censorship_dedups_receipts",
       "key_omits_kind",
       "key_omits_payload",
       "canonical_mutates_invalid_qc",
       "canonical_mutates_invalid_proposal",
       "subject_double_uses_second",
       "subject_invalid_qc_missing",
       "subject_proposal_uses_qc",
       "subject_censorship_uses_min",
       "subject_empty_censorship_zero",
       "subject_censorship_sets_view",
       "refs_double_drops_second",
       "refs_double_duplicates_same_hash",
       "refs_invalid_qc_empty",
       "refs_proposal_included",
       "refs_censorship_included",
       "store_accepts_invalid",
       "store_duplicate_inserted",
       "store_swapped_inserted",
       "store_rejects_new_valid",
       "store_skips_canonicalization",
       "store_omits_kind",
       "persist_accepts_invalid",
       "persist_duplicate_inserted",
       "persist_swapped_duplicate_inserted",
       "persist_rejects_missing_subject",
       "persist_missing_subject_zero_height",
       "persist_censorship_uses_current_height",
       "persist_missing_subject_nonzero_view",
       "persist_censorship_nonzero_view",
       "persist_sets_penalty_applied",
       "persist_sets_penalty_cancelled"
     }
  /\ checked = 0

EvidenceCanonicalizationMatchesSpec ==
  /\ \A c \in KeyCases:
       ActualKey(c) = SpecKey(c)
  /\ \A c \in SubjectCases:
       /\ ActualSubjectHeight(c) = SpecSubjectHeight(c)
       /\ ActualSubjectView(c) = SpecSubjectView(c)
  /\ \A c \in RefCases:
       ActualRefs(c) = SpecRefs(c)
  /\ \A c \in StoreCases:
       /\ ActualStoreInserted(c) = SpecStoreInserted(c)
       /\ ActualStoreKey(c) = SpecStoreKey(c)
  /\ \A c \in PersistCases:
       /\ ActualPersistInserted(c) = SpecPersistInserted(c)
       /\ ActualRecordedHeight(c) = SpecRecordedHeight(c)
       /\ ActualRecordedView(c) = SpecRecordedView(c)
       /\ ActualPenaltyFlagsClear(c) = SpecPenaltyFlagsClear(c)

EvidenceCanonicalizationExactness ==
  /\ EvidenceCanonicalizationMatchesSpec
EvidenceCanonicalizationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EvidenceCanonicalizationExactness

SafetyFast ==
  EvidenceCanonicalizationExactness

BugDoubleSwappedNotCanonical ==
  ActualKey("double_swapped") = SpecKey("double_swapped")

BugDoublePhaseOrderWrong ==
  ActualKey("double_phase_order") = SpecKey("double_phase_order")

BugDoubleBlockOrderWrong ==
  ActualKey("double_block_order") = SpecKey("double_block_order")

BugDoubleRootOrderWrong ==
  ActualKey("double_root_order") = SpecKey("double_root_order")

BugCensorshipSwappedNotCanonical ==
  ActualKey("censorship_swapped") = SpecKey("censorship_swapped")

BugCensorshipDedupsReceipts ==
  ActualKey("censorship_duplicate_receipts") = SpecKey("censorship_duplicate_receipts")

BugKeyOmitsKind ==
  ActualKey("same_payload_different_kind") = SpecKey("same_payload_different_kind")

BugKeyOmitsPayload ==
  ActualKey("invalid_qc") = SpecKey("invalid_qc")

BugCanonicalMutatesInvalidQc ==
  ActualKey("invalid_qc") = SpecKey("invalid_qc")

BugCanonicalMutatesInvalidProposal ==
  ActualKey("invalid_proposal") = SpecKey("invalid_proposal")

BugSubjectDoubleUsesSecond ==
  /\ ActualSubjectHeight("double_vote") = SpecSubjectHeight("double_vote")
  /\ ActualSubjectView("double_vote") = SpecSubjectView("double_vote")

BugSubjectInvalidQcMissing ==
  ActualSubjectHeight("invalid_qc") = SpecSubjectHeight("invalid_qc")

BugSubjectProposalUsesQc ==
  ActualSubjectHeight("invalid_proposal") = SpecSubjectHeight("invalid_proposal")

BugSubjectCensorshipUsesMin ==
  ActualSubjectHeight("censorship_many") = SpecSubjectHeight("censorship_many")

BugSubjectEmptyCensorshipZero ==
  ActualSubjectHeight("censorship_empty") = SpecSubjectHeight("censorship_empty")

BugSubjectCensorshipSetsView ==
  ActualSubjectView("censorship_one") = SpecSubjectView("censorship_one")

BugRefsDoubleDropsSecond ==
  ActualRefs("double_vote_distinct") = SpecRefs("double_vote_distinct")

BugRefsDoubleDuplicatesSameHash ==
  ActualRefs("double_vote_same_hash_root_conflict") =
    SpecRefs("double_vote_same_hash_root_conflict")

BugRefsInvalidQcEmpty ==
  ActualRefs("invalid_qc") = SpecRefs("invalid_qc")

BugRefsProposalIncluded ==
  ActualRefs("invalid_proposal") = SpecRefs("invalid_proposal")

BugRefsCensorshipIncluded ==
  ActualRefs("censorship") = SpecRefs("censorship")

BugStoreAcceptsInvalid ==
  ActualStoreInserted("invalid_new") = SpecStoreInserted("invalid_new")

BugStoreDuplicateInserted ==
  ActualStoreInserted("valid_duplicate") = SpecStoreInserted("valid_duplicate")

BugStoreSwappedInserted ==
  ActualStoreInserted("valid_swapped_duplicate") =
    SpecStoreInserted("valid_swapped_duplicate")

BugStoreRejectsNewValid ==
  ActualStoreInserted("valid_new") = SpecStoreInserted("valid_new")

BugStoreSkipsCanonicalization ==
  ActualStoreKey("valid_swapped_duplicate") = SpecStoreKey("valid_swapped_duplicate")

BugStoreOmitsKind ==
  ActualStoreKey("valid_new") = SpecStoreKey("valid_new")

BugPersistAcceptsInvalid ==
  ActualPersistInserted("invalid") = SpecPersistInserted("invalid")

BugPersistDuplicateInserted ==
  ActualPersistInserted("existing_duplicate") = SpecPersistInserted("existing_duplicate")

BugPersistSwappedDuplicateInserted ==
  ActualPersistInserted("swapped_duplicate") = SpecPersistInserted("swapped_duplicate")

BugPersistRejectsMissingSubject ==
  ActualPersistInserted("missing_subject") = SpecPersistInserted("missing_subject")

BugPersistMissingSubjectZeroHeight ==
  ActualRecordedHeight("missing_subject") = SpecRecordedHeight("missing_subject")

BugPersistCensorshipUsesCurrentHeight ==
  ActualRecordedHeight("censorship_subject") = SpecRecordedHeight("censorship_subject")

BugPersistMissingSubjectNonzeroView ==
  ActualRecordedView("missing_subject") = SpecRecordedView("missing_subject")

BugPersistCensorshipNonzeroView ==
  ActualRecordedView("censorship_subject") = SpecRecordedView("censorship_subject")

BugPersistSetsPenaltyApplied ==
  ActualPenaltyFlagsClear("valid_new") = SpecPenaltyFlagsClear("valid_new")

BugPersistSetsPenaltyCancelled ==
  ActualPenaltyFlagsClear("valid_new") = SpecPenaltyFlagsClear("valid_new")

====
