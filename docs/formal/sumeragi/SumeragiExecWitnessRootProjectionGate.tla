---- MODULE SumeragiExecWitnessRootProjectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi execution-witness root projection.

This slice captures the state-selection contract around
`post_state_from_witness(...)`, `parent_state_from_witness(...)`,
`prevalidated_roots_match_witness(...)`, and the FASTPQ public-input root
projection. The concrete Rust hashes use a deterministic sparse Merkle tree;
this model abstracts that tree to the input projection that must be identical
across peers:

- post roots bind all reads only for pure-read witnesses,
- post roots bind writes and ignore reads whenever writes exist,
- parent roots bind read pre-values, filtering to written keys when writes
  exist,
- root input canonicalization is order-independent and key-deduplicating,
- FASTPQ transcript/batch payloads do not perturb parent/post roots,
- commit prevalidation accepts only when a witness exists and both roots match.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PostEmpty == "post_empty"
PostPureReads == "post_pure_reads"
PostWrites == "post_writes"
PostWriteConflict == "post_write_conflict"
PostOrderIndependence == "post_order_independence"
ParentEmpty == "parent_empty"
ParentReadOnly == "parent_read_only"
ParentWrites == "parent_writes"
ParentOrderIndependence == "parent_order_independence"
RootIgnoresFastpqPayloads == "root_ignores_fastpq_payloads"
PrevalidatedNoWitness == "prevalidated_no_witness"
PrevalidatedParentMismatch == "prevalidated_parent_mismatch"
PrevalidatedPostMismatch == "prevalidated_post_mismatch"
PrevalidatedMatch == "prevalidated_match"
FastpqTemplate == "fastpq_template"

Cases == {
  PostEmpty,
  PostPureReads,
  PostWrites,
  PostWriteConflict,
  PostOrderIndependence,
  ParentEmpty,
  ParentReadOnly,
  ParentWrites,
  ParentOrderIndependence,
  RootIgnoresFastpqPayloads,
  PrevalidatedNoWitness,
  PrevalidatedParentMismatch,
  PrevalidatedPostMismatch,
  PrevalidatedMatch,
  FastpqTemplate
}

ReadPairsConverted == 1
WritePairsConverted == 2
PostUsesEmpty == 3
PostUsesReadsWhenNoWrites == 4
PostUsesWritesWhenWrites == 5
PostIgnoresReadsWhenWrites == 6
PostWritesOverrideReads == 7
RootInputSortedByKey == 8
RootDedupsByKey == 9
ParentUsesEmpty == 10
ParentUsesAllReadsWhenNoWrites == 11
ParentFiltersReadsToWrittenKeys == 12
ParentIgnoresIncidentalReads == 13
ParentIgnoresWriteValues == 14
ParentDoesNotUseWritesAsPrevalues == 15
ParentOrderIndependent == 16
PrevalidatedRequiresWitness == 17
PrevalidatedChecksParentRoot == 18
PrevalidatedChecksPostRoot == 19
PrevalidatedAcceptsMatching == 20
PrevalidatedRejectsMismatch == 21
FastpqTemplateOldRootParent == 22
FastpqTemplateNewRootPost == 23
IgnoreFastpqPayloads == 24
FastpqTemplateOldRootPost == 25
FastpqTemplateNewRootParent == 26
RootUsesFastpqPayloads == 27

Actions == 1..27

RootCanonical ==
  {RootInputSortedByKey, RootDedupsByKey}

PostWritesBase ==
  {WritePairsConverted, PostUsesWritesWhenWrites,
   PostIgnoresReadsWhenWrites} \cup RootCanonical

ParentWritesBase ==
  {ReadPairsConverted, WritePairsConverted, ParentFiltersReadsToWrittenKeys,
   ParentIgnoresIncidentalReads, ParentIgnoresWriteValues,
   ParentDoesNotUseWritesAsPrevalues} \cup RootCanonical

SpecActions(c) ==
  CASE c = PostEmpty ->
      {ReadPairsConverted, WritePairsConverted, PostUsesEmpty}
    [] c = PostPureReads ->
      {ReadPairsConverted, PostUsesReadsWhenNoWrites} \cup RootCanonical
    [] c = PostWrites ->
      PostWritesBase
    [] c = PostWriteConflict ->
      PostWritesBase \cup {PostWritesOverrideReads}
    [] c = PostOrderIndependence ->
      {PostUsesWritesWhenWrites, PostIgnoresReadsWhenWrites} \cup RootCanonical
    [] c = ParentEmpty ->
      {ReadPairsConverted, ParentUsesEmpty}
    [] c = ParentReadOnly ->
      {ReadPairsConverted, ParentUsesAllReadsWhenNoWrites} \cup RootCanonical
    [] c = ParentWrites ->
      ParentWritesBase
    [] c = ParentOrderIndependence ->
      {ParentOrderIndependent} \cup RootCanonical
    [] c = RootIgnoresFastpqPayloads ->
      {IgnoreFastpqPayloads, PostUsesWritesWhenWrites,
       ParentFiltersReadsToWrittenKeys}
    [] c = PrevalidatedNoWitness ->
      {PrevalidatedRequiresWitness, PrevalidatedRejectsMismatch}
    [] c = PrevalidatedParentMismatch ->
      {PrevalidatedRequiresWitness, PrevalidatedChecksParentRoot,
       PrevalidatedRejectsMismatch}
    [] c = PrevalidatedPostMismatch ->
      {PrevalidatedRequiresWitness, PrevalidatedChecksParentRoot,
       PrevalidatedChecksPostRoot, PrevalidatedRejectsMismatch}
    [] c = PrevalidatedMatch ->
      {PrevalidatedRequiresWitness, PrevalidatedChecksParentRoot,
       PrevalidatedChecksPostRoot, PrevalidatedAcceptsMatching}
    [] c = FastpqTemplate ->
      {FastpqTemplateOldRootParent, FastpqTemplateNewRootPost}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "post_empty_uses_reads"
       /\ c = PostEmpty ->
      (spec \ {PostUsesEmpty}) \cup {PostUsesReadsWhenNoWrites}
    [] Bug = "post_pure_reads_ignored"
       /\ c = PostPureReads ->
      (spec \ {PostUsesReadsWhenNoWrites}) \cup {PostUsesEmpty}
    [] Bug = "post_writes_include_reads"
       /\ c = PostWrites ->
      (spec \ {PostIgnoresReadsWhenWrites}) \cup {PostUsesReadsWhenNoWrites}
    [] Bug = "post_writes_use_reads_only"
       /\ c = PostWrites ->
      (spec \ {PostUsesWritesWhenWrites, PostIgnoresReadsWhenWrites}) \cup
        {PostUsesReadsWhenNoWrites}
    [] Bug = "post_write_conflict_read_wins"
       /\ c = PostWriteConflict ->
      (spec \ {PostWritesOverrideReads, PostUsesWritesWhenWrites}) \cup
        {PostUsesReadsWhenNoWrites}
    [] Bug = "post_not_sorted"
       /\ c \in {PostPureReads, PostWrites, PostWriteConflict,
                 PostOrderIndependence} ->
      spec \ {RootInputSortedByKey}
    [] Bug = "post_no_dedup"
       /\ c \in {PostPureReads, PostWrites, PostWriteConflict,
                 PostOrderIndependence} ->
      spec \ {RootDedupsByKey}
    [] Bug = "parent_empty_uses_writes"
       /\ c = ParentEmpty ->
      (spec \ {ParentUsesEmpty}) \cup {ParentDoesNotUseWritesAsPrevalues}
    [] Bug = "parent_read_only_ignored"
       /\ c = ParentReadOnly ->
      (spec \ {ParentUsesAllReadsWhenNoWrites}) \cup {ParentUsesEmpty}
    [] Bug = "parent_writes_include_incidental_reads"
       /\ c = ParentWrites ->
      spec \ {ParentIgnoresIncidentalReads}
    [] Bug = "parent_writes_use_write_values"
       /\ c = ParentWrites ->
      (spec \ {ParentIgnoresWriteValues, ParentDoesNotUseWritesAsPrevalues}) \cup
        {PostUsesWritesWhenWrites}
    [] Bug = "parent_writes_use_all_writes"
       /\ c = ParentWrites ->
      (spec \ {ParentFiltersReadsToWrittenKeys,
               ParentDoesNotUseWritesAsPrevalues}) \cup
        {PostUsesWritesWhenWrites}
    [] Bug = "parent_not_sorted"
       /\ c \in {ParentReadOnly, ParentWrites, ParentOrderIndependence} ->
      spec \ {RootInputSortedByKey}
    [] Bug = "parent_no_dedup"
       /\ c \in {ParentReadOnly, ParentWrites, ParentOrderIndependence} ->
      spec \ {RootDedupsByKey}
    [] Bug = "prevalidated_missing_witness_accepts"
       /\ c = PrevalidatedNoWitness ->
      (spec \ {PrevalidatedRejectsMismatch}) \cup {PrevalidatedAcceptsMatching}
    [] Bug = "prevalidated_skips_parent_root"
       /\ c \in {PrevalidatedParentMismatch, PrevalidatedPostMismatch,
                 PrevalidatedMatch} ->
      spec \ {PrevalidatedChecksParentRoot}
    [] Bug = "prevalidated_skips_post_root"
       /\ c \in {PrevalidatedPostMismatch, PrevalidatedMatch} ->
      spec \ {PrevalidatedChecksPostRoot}
    [] Bug = "prevalidated_match_rejects"
       /\ c = PrevalidatedMatch ->
      (spec \ {PrevalidatedAcceptsMatching}) \cup {PrevalidatedRejectsMismatch}
    [] Bug = "fastpq_swaps_roots"
       /\ c = FastpqTemplate ->
      {FastpqTemplateOldRootPost, FastpqTemplateNewRootParent}
    [] Bug = "roots_include_fastpq_payloads"
       /\ c = RootIgnoresFastpqPayloads ->
      (spec \ {IgnoreFastpqPayloads}) \cup {RootUsesFastpqPayloads}
    [] OTHER -> spec

Bugs == {
  "none",
  "post_empty_uses_reads",
  "post_pure_reads_ignored",
  "post_writes_include_reads",
  "post_writes_use_reads_only",
  "post_write_conflict_read_wins",
  "post_not_sorted",
  "post_no_dedup",
  "parent_empty_uses_writes",
  "parent_read_only_ignored",
  "parent_writes_include_incidental_reads",
  "parent_writes_use_write_values",
  "parent_writes_use_all_writes",
  "parent_not_sorted",
  "parent_no_dedup",
  "prevalidated_missing_witness_accepts",
  "prevalidated_skips_parent_root",
  "prevalidated_skips_post_root",
  "prevalidated_match_rejects",
  "fastpq_swaps_roots",
  "roots_include_fastpq_payloads"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 15
     /\ checked' = checked + 1
  \/ /\ checked = 15
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..15
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

PostRootActionsMatchSpec ==
  \A c \in {
    PostEmpty,
    PostPureReads,
    PostWrites,
    PostWriteConflict,
    PostOrderIndependence
  }:
    ImplementationActions(c) = SpecActions(c)

ParentRootActionsMatchSpec ==
  \A c \in {
    ParentEmpty,
    ParentReadOnly,
    ParentWrites,
    ParentOrderIndependence
  }:
    ImplementationActions(c) = SpecActions(c)

PrevalidationActionsMatchSpec ==
  \A c \in {
    PrevalidatedNoWitness,
    PrevalidatedParentMismatch,
    PrevalidatedPostMismatch,
    PrevalidatedMatch
  }:
    ImplementationActions(c) = SpecActions(c)

FastpqRootActionsMatchSpec ==
  \A c \in {RootIgnoresFastpqPayloads, FastpqTemplate}:
    ImplementationActions(c) = SpecActions(c)

PostRootProjectionAnchors ==
  /\ ReadPairsConverted \in ImplementationActions(PostEmpty)
  /\ WritePairsConverted \in ImplementationActions(PostEmpty)
  /\ PostUsesEmpty \in ImplementationActions(PostEmpty)
  /\ ReadPairsConverted \in ImplementationActions(PostPureReads)
  /\ PostUsesReadsWhenNoWrites \in ImplementationActions(PostPureReads)
  /\ RootCanonical \subseteq ImplementationActions(PostPureReads)
  /\ WritePairsConverted \in ImplementationActions(PostWrites)
  /\ PostUsesWritesWhenWrites \in ImplementationActions(PostWrites)
  /\ PostIgnoresReadsWhenWrites \in ImplementationActions(PostWrites)
  /\ RootCanonical \subseteq ImplementationActions(PostWrites)
  /\ PostWritesOverrideReads \in ImplementationActions(PostWriteConflict)
  /\ RootCanonical \subseteq ImplementationActions(PostWriteConflict)
  /\ PostUsesWritesWhenWrites \in
       ImplementationActions(PostOrderIndependence)
  /\ PostIgnoresReadsWhenWrites \in
       ImplementationActions(PostOrderIndependence)
  /\ RootCanonical \subseteq ImplementationActions(PostOrderIndependence)

ParentRootProjectionAnchors ==
  /\ ReadPairsConverted \in ImplementationActions(ParentEmpty)
  /\ ParentUsesEmpty \in ImplementationActions(ParentEmpty)
  /\ ReadPairsConverted \in ImplementationActions(ParentReadOnly)
  /\ ParentUsesAllReadsWhenNoWrites \in ImplementationActions(ParentReadOnly)
  /\ RootCanonical \subseteq ImplementationActions(ParentReadOnly)
  /\ ReadPairsConverted \in ImplementationActions(ParentWrites)
  /\ WritePairsConverted \in ImplementationActions(ParentWrites)
  /\ ParentFiltersReadsToWrittenKeys \in ImplementationActions(ParentWrites)
  /\ ParentIgnoresIncidentalReads \in ImplementationActions(ParentWrites)
  /\ ParentIgnoresWriteValues \in ImplementationActions(ParentWrites)
  /\ ParentDoesNotUseWritesAsPrevalues \in
       ImplementationActions(ParentWrites)
  /\ RootCanonical \subseteq ImplementationActions(ParentWrites)
  /\ ParentOrderIndependent \in
       ImplementationActions(ParentOrderIndependence)
  /\ RootCanonical \subseteq ImplementationActions(ParentOrderIndependence)

PrevalidationAnchors ==
  /\ PrevalidatedRequiresWitness \in
       ImplementationActions(PrevalidatedNoWitness)
  /\ PrevalidatedRejectsMismatch \in
       ImplementationActions(PrevalidatedNoWitness)
  /\ PrevalidatedRequiresWitness \in
       ImplementationActions(PrevalidatedParentMismatch)
  /\ PrevalidatedChecksParentRoot \in
       ImplementationActions(PrevalidatedParentMismatch)
  /\ PrevalidatedRejectsMismatch \in
       ImplementationActions(PrevalidatedParentMismatch)
  /\ PrevalidatedRequiresWitness \in
       ImplementationActions(PrevalidatedPostMismatch)
  /\ PrevalidatedChecksParentRoot \in
       ImplementationActions(PrevalidatedPostMismatch)
  /\ PrevalidatedChecksPostRoot \in
       ImplementationActions(PrevalidatedPostMismatch)
  /\ PrevalidatedRejectsMismatch \in
       ImplementationActions(PrevalidatedPostMismatch)
  /\ PrevalidatedRequiresWitness \in ImplementationActions(PrevalidatedMatch)
  /\ PrevalidatedChecksParentRoot \in
       ImplementationActions(PrevalidatedMatch)
  /\ PrevalidatedChecksPostRoot \in ImplementationActions(PrevalidatedMatch)
  /\ PrevalidatedAcceptsMatching \in ImplementationActions(PrevalidatedMatch)

FastpqRootAnchors ==
  /\ IgnoreFastpqPayloads \in ImplementationActions(RootIgnoresFastpqPayloads)
  /\ PostUsesWritesWhenWrites \in
       ImplementationActions(RootIgnoresFastpqPayloads)
  /\ ParentFiltersReadsToWrittenKeys \in
       ImplementationActions(RootIgnoresFastpqPayloads)
  /\ FastpqTemplateOldRootParent \in ImplementationActions(FastpqTemplate)
  /\ FastpqTemplateNewRootPost \in ImplementationActions(FastpqTemplate)

ExecWitnessRootSafetyAnchors ==
  /\ PostRootActionsMatchSpec
  /\ ParentRootActionsMatchSpec
  /\ PrevalidationActionsMatchSpec
  /\ FastpqRootActionsMatchSpec
  /\ PostRootProjectionAnchors
  /\ ParentRootProjectionAnchors
  /\ PrevalidationAnchors
  /\ FastpqRootAnchors

SafetyFast == Safety

ExecWitnessRootProjectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ ExecWitnessRootSafetyAnchors

====
