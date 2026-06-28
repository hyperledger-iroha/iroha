---- MODULE SumeragiRbcSamplingGate ----
EXTENDS Integers, Sequences

(***************************************************************************
A bounded abstract model for RBC chunk sampling from persisted session state.

This slice pins `rbc_sampling::sample_from_store(...)`:
- absent stores return `Ok(None)` while I/O and persisted-session validation
  failures stay explicit errors,
- zero/oversized sample requests fail before proof generation,
- incomplete sessions, missing chunk material, missing proofs, and excessive
  proof depth fail closed,
- successful samples use sorted, unique, in-range indices and return metadata
  bound to the requested session key plus the session's total chunk count,
  chunk root, and optional payload hash.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "absent",
  "io_error",
  "persisted_invalid",
  "zero_total",
  "count_zero",
  "count_over_total",
  "incomplete_digests",
  "missing_bytes",
  "missing_digest",
  "proof_missing",
  "proof_too_deep",
  "missing_root",
  "valid_one_seeded",
  "valid_two_seeded",
  "valid_all_seeded",
  "valid_payload_absent"
}

OkCases == {
  "valid_one_seeded",
  "valid_two_seeded",
  "valid_all_seeded",
  "valid_payload_absent"
}

SpecKind(c) ==
  CASE c = "absent" -> "none"
    [] c = "io_error" -> "io_error"
    [] c = "persisted_invalid" -> "persisted_error"
    [] c \in {"zero_total", "count_zero", "count_over_total"} -> "invalid_count"
    [] c \in {"incomplete_digests", "missing_root"} -> "incomplete"
    [] c \in {"missing_bytes", "missing_digest", "proof_missing", "proof_too_deep"} -> "proof_error"
    [] OTHER -> "ok"

ActualKind(c) ==
  CASE Bug = "load_absent_as_error"
       /\ c = "absent" -> "io_error"
    [] Bug = "load_io_error_as_none"
       /\ c = "io_error" -> "none"
    [] Bug = "persisted_invalid_as_none"
       /\ c = "persisted_invalid" -> "none"
    [] Bug = "zero_total_ok"
       /\ c = "zero_total" -> "ok"
    [] Bug = "count_zero_ok"
       /\ c = "count_zero" -> "ok"
    [] Bug = "count_over_total_ok"
       /\ c = "count_over_total" -> "ok"
    [] Bug = "incomplete_digests_ok"
       /\ c = "incomplete_digests" -> "ok"
    [] Bug = "missing_bytes_ok"
       /\ c = "missing_bytes" -> "ok"
    [] Bug = "missing_digest_ok"
       /\ c = "missing_digest" -> "ok"
    [] Bug = "proof_missing_ok"
       /\ c = "proof_missing" -> "ok"
    [] Bug = "proof_too_deep_ok"
       /\ c = "proof_too_deep" -> "ok"
    [] Bug = "missing_root_ok"
       /\ c = "missing_root" -> "ok"
    [] Bug = "valid_returns_none"
       /\ c = "valid_one_seeded" -> "none"
    [] OTHER -> SpecKind(c)

SampleCount(c) ==
  CASE c = "valid_all_seeded" -> 3
    [] c = "valid_two_seeded" -> 2
    [] OTHER -> 1

TotalChunks(c) ==
  CASE c = "valid_all_seeded" -> 3
    [] c = "valid_two_seeded" -> 4
    [] OTHER -> 2

SpecSampleSeq(c) ==
  CASE c = "valid_one_seeded" -> <<1>>
    [] c = "valid_two_seeded" -> <<1, 3>>
    [] c = "valid_all_seeded" -> <<0, 1, 2>>
    [] c = "valid_payload_absent" -> <<0>>
    [] OTHER -> <<>>

ActualSampleSeq(c) ==
  CASE Bug = "sample_unsorted"
       /\ c = "valid_two_seeded" -> <<3, 1>>
    [] Bug = "sample_duplicate"
       /\ c = "valid_two_seeded" -> <<1, 1>>
    [] Bug = "sample_out_of_range"
       /\ c = "valid_two_seeded" -> <<1, 4>>
    [] Bug = "sample_underselects"
       /\ c = "valid_two_seeded" -> <<1>>
    [] Bug = "sample_overselects"
       /\ c = "valid_two_seeded" -> <<0, 1, 3>>
    [] OTHER -> SpecSampleSeq(c)

SeqInRange(seq, total) ==
  \A i \in 1..Len(seq):
    /\ seq[i] >= 0
    /\ seq[i] < total

SeqStrictlySorted(seq) ==
  \A i \in 1..(Len(seq) - 1):
    seq[i] < seq[i + 1]

SampleWellFormed(c, seq) ==
  /\ Len(seq) = SampleCount(c)
  /\ SeqInRange(seq, TotalChunks(c))
  /\ SeqStrictlySorted(seq)

SpecBlock(c) == "key_block"

ActualBlock(c) ==
  CASE Bug = "metadata_uses_persisted_key"
       /\ c = "valid_one_seeded" -> "persisted_block"
    [] OTHER -> SpecBlock(c)

SpecHeight(c) == 5

ActualHeight(c) ==
  CASE Bug = "metadata_wrong_height"
       /\ c = "valid_one_seeded" -> 6
    [] OTHER -> SpecHeight(c)

SpecView(c) == 2

ActualView(c) ==
  CASE Bug = "metadata_wrong_view"
       /\ c = "valid_one_seeded" -> 3
    [] OTHER -> SpecView(c)

SpecTotal(c) == TotalChunks(c)

ActualTotal(c) ==
  CASE Bug = "metadata_wrong_total"
       /\ c = "valid_two_seeded" -> 2
    [] OTHER -> SpecTotal(c)

SpecRootPresent(c) == TRUE

ActualRootPresent(c) ==
  CASE Bug = "metadata_drops_chunk_root"
       /\ c = "valid_one_seeded" -> FALSE
    [] OTHER -> SpecRootPresent(c)

SpecPayloadHash(c) ==
  c # "valid_payload_absent"

ActualPayloadHash(c) ==
  CASE Bug = "metadata_drops_payload_hash"
       /\ c = "valid_one_seeded" -> FALSE
    [] Bug = "payload_none_synthesizes_hash"
       /\ c = "valid_payload_absent" -> TRUE
    [] OTHER -> SpecPayloadHash(c)

MetadataMatches(c) ==
  /\ ActualBlock(c) = SpecBlock(c)
  /\ ActualHeight(c) = SpecHeight(c)
  /\ ActualView(c) = SpecView(c)
  /\ ActualTotal(c) = SpecTotal(c)
  /\ ActualRootPresent(c) = SpecRootPresent(c)
  /\ ActualPayloadHash(c) = SpecPayloadHash(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "load_absent_as_error",
       "load_io_error_as_none",
       "persisted_invalid_as_none",
       "zero_total_ok",
       "count_zero_ok",
       "count_over_total_ok",
       "incomplete_digests_ok",
       "missing_bytes_ok",
       "missing_digest_ok",
       "proof_missing_ok",
       "proof_too_deep_ok",
       "missing_root_ok",
       "valid_returns_none",
       "sample_unsorted",
       "sample_duplicate",
       "sample_out_of_range",
       "sample_underselects",
       "sample_overselects",
       "metadata_uses_persisted_key",
       "metadata_wrong_height",
       "metadata_wrong_view",
       "metadata_wrong_total",
       "metadata_drops_chunk_root",
       "metadata_drops_payload_hash",
       "payload_none_synthesizes_hash"
     }
  /\ checked \in 0..25

RbcSamplingCoreSafety ==
  /\ \A c \in Cases:
       ActualKind(c) = SpecKind(c)
  /\ \A c \in OkCases:
       /\ SampleWellFormed(c, ActualSampleSeq(c))
       /\ ActualSampleSeq(c) = SpecSampleSeq(c)
       /\ MetadataMatches(c)

SafetyFast ==
  RbcSamplingCoreSafety

AllKindsMatchSpec ==
  \A c \in Cases:
    ActualKind(c) = SpecKind(c)

AllOkSamplesWellFormed ==
  \A c \in OkCases:
    SampleWellFormed(c, ActualSampleSeq(c))

AllOkSamplesMatchSpec ==
  \A c \in OkCases:
    ActualSampleSeq(c) = SpecSampleSeq(c)

AllOkMetadataMatches ==
  \A c \in OkCases:
    MetadataMatches(c)

LoadOutcomeAnchors ==
  /\ ActualKind("absent") = "none"
  /\ ActualKind("io_error") = "io_error"
  /\ ActualKind("persisted_invalid") = "persisted_error"

InvalidRequestAnchors ==
  /\ ActualKind("zero_total") = "invalid_count"
  /\ ActualKind("count_zero") = "invalid_count"
  /\ ActualKind("count_over_total") = "invalid_count"

ProofFailureAnchors ==
  /\ ActualKind("incomplete_digests") = "incomplete"
  /\ ActualKind("missing_bytes") = "proof_error"
  /\ ActualKind("missing_digest") = "proof_error"
  /\ ActualKind("proof_missing") = "proof_error"
  /\ ActualKind("proof_too_deep") = "proof_error"
  /\ ActualKind("missing_root") = "incomplete"

ValidKindAnchors ==
  /\ ActualKind("valid_one_seeded") = "ok"
  /\ ActualKind("valid_two_seeded") = "ok"
  /\ ActualKind("valid_all_seeded") = "ok"
  /\ ActualKind("valid_payload_absent") = "ok"

SampleSelectionAnchors ==
  /\ ActualSampleSeq("valid_one_seeded") = <<1>>
  /\ ActualSampleSeq("valid_two_seeded") = <<1, 3>>
  /\ ActualSampleSeq("valid_all_seeded") = <<0, 1, 2>>
  /\ ActualSampleSeq("valid_payload_absent") = <<0>>

MetadataAnchors ==
  /\ ActualBlock("valid_one_seeded") = "key_block"
  /\ ActualHeight("valid_one_seeded") = 5
  /\ ActualView("valid_one_seeded") = 2
  /\ ActualTotal("valid_two_seeded") = 4
  /\ ActualRootPresent("valid_one_seeded")
  /\ ActualPayloadHash("valid_one_seeded")
  /\ ~ActualPayloadHash("valid_payload_absent")

SafetyAnchors ==
  /\ AllKindsMatchSpec
  /\ AllOkSamplesWellFormed
  /\ AllOkSamplesMatchSpec
  /\ AllOkMetadataMatches
  /\ LoadOutcomeAnchors
  /\ InvalidRequestAnchors
  /\ ProofFailureAnchors
  /\ ValidKindAnchors
  /\ SampleSelectionAnchors
  /\ MetadataAnchors

RbcSamplingExactness == SafetyAnchors

RbcSamplingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcSamplingExactness

BugLoadAbsentAsError ==
  ActualKind("absent") = SpecKind("absent")

BugLoadIoErrorAsNone ==
  ActualKind("io_error") = SpecKind("io_error")

BugPersistedInvalidAsNone ==
  ActualKind("persisted_invalid") = SpecKind("persisted_invalid")

BugZeroTotalOk ==
  ActualKind("zero_total") = SpecKind("zero_total")

BugCountZeroOk ==
  ActualKind("count_zero") = SpecKind("count_zero")

BugCountOverTotalOk ==
  ActualKind("count_over_total") = SpecKind("count_over_total")

BugIncompleteDigestsOk ==
  ActualKind("incomplete_digests") = SpecKind("incomplete_digests")

BugMissingBytesOk ==
  ActualKind("missing_bytes") = SpecKind("missing_bytes")

BugMissingDigestOk ==
  ActualKind("missing_digest") = SpecKind("missing_digest")

BugProofMissingOk ==
  ActualKind("proof_missing") = SpecKind("proof_missing")

BugProofTooDeepOk ==
  ActualKind("proof_too_deep") = SpecKind("proof_too_deep")

BugMissingRootOk ==
  ActualKind("missing_root") = SpecKind("missing_root")

BugValidReturnsNone ==
  ActualKind("valid_one_seeded") = SpecKind("valid_one_seeded")

BugSampleUnsorted ==
  ActualSampleSeq("valid_two_seeded") = SpecSampleSeq("valid_two_seeded")

BugSampleDuplicate ==
  ActualSampleSeq("valid_two_seeded") = SpecSampleSeq("valid_two_seeded")

BugSampleOutOfRange ==
  /\ SampleWellFormed("valid_two_seeded", ActualSampleSeq("valid_two_seeded"))
  /\ ActualSampleSeq("valid_two_seeded") = SpecSampleSeq("valid_two_seeded")

BugSampleUnderselects ==
  /\ SampleWellFormed("valid_two_seeded", ActualSampleSeq("valid_two_seeded"))
  /\ ActualSampleSeq("valid_two_seeded") = SpecSampleSeq("valid_two_seeded")

BugSampleOverselects ==
  /\ SampleWellFormed("valid_two_seeded", ActualSampleSeq("valid_two_seeded"))
  /\ ActualSampleSeq("valid_two_seeded") = SpecSampleSeq("valid_two_seeded")

BugMetadataUsesPersistedKey ==
  MetadataMatches("valid_one_seeded")

BugMetadataWrongHeight ==
  MetadataMatches("valid_one_seeded")

BugMetadataWrongView ==
  MetadataMatches("valid_one_seeded")

BugMetadataWrongTotal ==
  MetadataMatches("valid_two_seeded")

BugMetadataDropsChunkRoot ==
  MetadataMatches("valid_one_seeded")

BugMetadataDropsPayloadHash ==
  MetadataMatches("valid_one_seeded")

BugPayloadNoneSynthesizesHash ==
  MetadataMatches("valid_payload_absent")

====
