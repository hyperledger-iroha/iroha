---- MODULE SumeragiSignatureIndexRecoveryGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for commit signature-index recovery.

This slice models `remap_block_signature_indices_to_topology(...)`, used by
the commit validation retry path when a block fails signature verification.
Each signature is mapped to a topology index by first trusting a raw index only
when it names an eligible BLS peer that verifies the block hash; otherwise the
helper scans eligible peers and accepts exactly one verifier. Unknown,
ambiguous, duplicate, and empty remaps fail closed before the recovered block
can be retried.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

TopologyIndices == 0..2

SignatureValues == {"sig_a", "sig_b", "same"}

EmptyMappedPairs ==
  {p \in TopologyIndices \X SignatureValues: FALSE}

Cases == {
  "empty",
  "raw_valid",
  "raw_wrong_fallback",
  "raw_out_of_range_fallback",
  "raw_ineligible_only",
  "raw_ineligible_fallback",
  "no_match",
  "ambiguous_scan",
  "identical_duplicate_pair",
  "distinct_duplicate_index",
  "two_distinct",
  "raw_priority_extra_match"
}

Signatures(c) ==
  CASE c = "empty" -> {}
    [] c \in {
         "identical_duplicate_pair",
         "distinct_duplicate_index",
         "two_distinct"
       } -> {"a", "b"}
    [] OTHER -> {"a"}

SignatureValue(c, s) ==
  CASE c = "identical_duplicate_pair" -> "same"
    [] s = "b" -> "sig_b"
    [] OTHER -> "sig_a"

RawIndex(c, s) ==
  CASE c \in {"raw_valid", "raw_priority_extra_match"} -> 1
    [] c \in {
         "raw_wrong_fallback",
         "raw_ineligible_only",
         "raw_ineligible_fallback",
         "no_match",
         "ambiguous_scan"
       } -> 0
    [] c = "raw_out_of_range_fallback" -> 9
    [] c \in {"identical_duplicate_pair", "distinct_duplicate_index"} -> 1
    [] c = "two_distinct" /\ s = "a" -> 1
    [] c = "two_distinct" /\ s = "b" -> 2
    [] OTHER -> -1

Eligible(c, i) ==
  CASE c \in {"raw_ineligible_only", "raw_ineligible_fallback"} /\ i = 0 -> FALSE
    [] OTHER -> i \in TopologyIndices

Verifies(c, s, i) ==
  CASE c = "raw_valid" -> i = 1
    [] c = "raw_priority_extra_match" -> i \in {1, 2}
    [] c \in {"raw_wrong_fallback", "raw_out_of_range_fallback"} -> i = 1
    [] c = "raw_ineligible_only" -> i = 0
    [] c = "raw_ineligible_fallback" -> i \in {0, 1}
    [] c = "ambiguous_scan" -> i \in {1, 2}
    [] c \in {"identical_duplicate_pair", "distinct_duplicate_index"} -> i = 1
    [] c = "two_distinct" /\ s = "a" -> i = 1
    [] c = "two_distinct" /\ s = "b" -> i = 2
    [] OTHER -> FALSE

RawHit(c, s) ==
  RawIndex(c, s) \in TopologyIndices
  /\ Eligible(c, RawIndex(c, s))
  /\ Verifies(c, s, RawIndex(c, s))

ScanMatches(c, s) ==
  {i \in TopologyIndices: Eligible(c, i) /\ Verifies(c, s, i)}

SpecSignatureStatus(c, s) ==
  IF RawHit(c, s) THEN
    "Ok"
  ELSE IF Cardinality(ScanMatches(c, s)) = 1 THEN
    "Ok"
  ELSE
    "Unknown"

SpecMappedIndex(c, s) ==
  IF RawHit(c, s) THEN
    RawIndex(c, s)
  ELSE IF Cardinality(ScanMatches(c, s)) = 1 THEN
    CHOOSE i \in ScanMatches(c, s): TRUE
  ELSE
    -1

SpecMappedPairs(c) ==
  {<<SpecMappedIndex(c, s), SignatureValue(c, s)>>: s \in Signatures(c)}

SpecMappedIndices(c) ==
  {SpecMappedIndex(c, s): s \in Signatures(c)}

SpecStatus(c) ==
  IF Signatures(c) = {} THEN
    "NotEnough"
  ELSE IF \E s \in Signatures(c): SpecSignatureStatus(c, s) = "Unknown" THEN
    "Unknown"
  ELSE IF Cardinality(SpecMappedPairs(c)) < Cardinality(Signatures(c)) THEN
    "Duplicate"
  ELSE IF Cardinality(SpecMappedIndices(c)) < Cardinality(Signatures(c)) THEN
    "Other"
  ELSE
    "Ok"

\* @type: Str => <<Str, Set(<<Int, Str>>)>>;
SpecResult(c) ==
  <<SpecStatus(c), IF SpecStatus(c) = "Ok" THEN SpecMappedPairs(c) ELSE EmptyMappedPairs>>

\* @type: Str => <<Str, Set(<<Int, Str>>)>>;
ActualResult(c) ==
  CASE Bug = "accept_empty"
       /\ c = "empty" -> <<"Ok", EmptyMappedPairs>>
    [] Bug = "skip_raw_hit"
       /\ c = "raw_valid" -> <<"Unknown", EmptyMappedPairs>>
    [] Bug = "skip_scan_fallback"
       /\ c = "raw_wrong_fallback" -> <<"Unknown", EmptyMappedPairs>>
    [] Bug = "skip_out_of_range_fallback"
       /\ c = "raw_out_of_range_fallback" -> <<"Unknown", EmptyMappedPairs>>
    [] Bug = "skip_ineligible_fallback"
       /\ c = "raw_ineligible_fallback" -> <<"Unknown", EmptyMappedPairs>>
    [] Bug = "accept_ineligible_raw"
       /\ c = "raw_ineligible_only" -> <<"Ok", {<<0, SignatureValue(c, "a")>>}>>
    [] Bug = "accept_no_match"
       /\ c = "no_match" -> <<"Ok", {<<0, SignatureValue(c, "a")>>}>>
    [] Bug = "accept_ambiguous"
       /\ c = "ambiguous_scan" -> <<"Ok", {<<1, SignatureValue(c, "a")>>}>>
    [] Bug = "accept_identical_duplicate"
       /\ c = "identical_duplicate_pair" -> <<"Ok", {<<1, "same">>}>>
    [] Bug = "accept_distinct_duplicate_index"
       /\ c = "distinct_duplicate_index" -> <<"Ok", SpecMappedPairs(c)>>
    [] Bug = "wrong_duplicate_error"
       /\ c = "distinct_duplicate_index" -> <<"Duplicate", EmptyMappedPairs>>
    [] Bug = "lose_raw_priority"
       /\ c = "raw_priority_extra_match" -> <<"Unknown", EmptyMappedPairs>>
    [] Bug = "wrong_fallback_index"
       /\ c = "raw_wrong_fallback" -> <<"Ok", {<<0, SignatureValue(c, "a")>>}>>
    [] OTHER -> SpecResult(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

SafetyFast ==
  \A c \in Cases: ActualResult(c) = SpecResult(c)

BugAcceptEmpty ==
  ActualResult("empty") = SpecResult("empty")

BugSkipRawHit ==
  ActualResult("raw_valid") = SpecResult("raw_valid")

BugSkipScanFallback ==
  ActualResult("raw_wrong_fallback") = SpecResult("raw_wrong_fallback")

BugSkipOutOfRangeFallback ==
  ActualResult("raw_out_of_range_fallback") =
    SpecResult("raw_out_of_range_fallback")

BugSkipIneligibleFallback ==
  ActualResult("raw_ineligible_fallback") = SpecResult("raw_ineligible_fallback")

BugAcceptIneligibleRaw ==
  ActualResult("raw_ineligible_only") = SpecResult("raw_ineligible_only")

BugAcceptNoMatch ==
  ActualResult("no_match") = SpecResult("no_match")

BugAcceptAmbiguous ==
  ActualResult("ambiguous_scan") = SpecResult("ambiguous_scan")

BugAcceptIdenticalDuplicate ==
  ActualResult("identical_duplicate_pair") =
    SpecResult("identical_duplicate_pair")

BugAcceptDistinctDuplicateIndex ==
  ActualResult("distinct_duplicate_index") =
    SpecResult("distinct_duplicate_index")

BugWrongDuplicateError ==
  ActualResult("distinct_duplicate_index") =
    SpecResult("distinct_duplicate_index")

BugLoseRawPriority ==
  ActualResult("raw_priority_extra_match") = SpecResult("raw_priority_extra_match")

BugWrongFallbackIndex ==
  ActualResult("raw_wrong_fallback") = SpecResult("raw_wrong_fallback")

====
