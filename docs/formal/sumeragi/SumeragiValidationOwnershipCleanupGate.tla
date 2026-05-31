---- MODULE SumeragiValidationOwnershipCleanupGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for `clear_validation_ownership_for_block(...)`.

The Rust helper clears all validation ownership tied to one block hash: legacy
in-flight ownership, vNext in-flight ownership, superseded-result markers, and
the matching slot in every vNext round. After slot removal, rounds with no
remaining slots are pruned. Unrelated hashes and still-nonempty rounds must be
left intact.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Set(Str);
  inflight,
  \* @type: Set(Str);
  vnextInflight,
  \* @type: Set(Str);
  superseded,
  \* @type: Set(<<Str, Str>>);
  vnextSlots,
  \* @type: Set(Str);
  retainedRounds

\* @type: <<Str, Set(Str), Set(Str), Set(Str), Set(<<Str, Str>>), Set(Str)>>;
vars == <<candidate, inflight, vnextInflight, superseded, vnextSlots, retainedRounds>>

Target == "target"
Other == "other"
Third == "third"
RoundA == "round_a"
RoundB == "round_b"
RoundC == "round_c"
EmptyRound == "empty_round"

Hashes == {Target, Other, Third}
Rounds == {RoundA, RoundB, RoundC, EmptyRound}
RoundHashPairs == Rounds \X Hashes

Cases == {
  "all_ownership",
  "legacy_only",
  "vnext_inflight_only",
  "superseded_only",
  "slot_only_mixed_rounds",
  "target_absent",
  "preexisting_empty_round",
  "all_target_rounds",
  "other_only_round",
  "target_and_other_same_round"
}

\* @type: Str => Set(Str);
PreInflight(c) ==
  CASE c = "all_ownership" -> {Target, Other}
    [] c = "legacy_only" -> {Target}
    [] c = "target_absent" -> {Other}
    [] OTHER -> {}

\* @type: Str => Set(Str);
PreVNextInflight(c) ==
  CASE c = "all_ownership" -> {Target, Other}
    [] c = "vnext_inflight_only" -> {Target}
    [] c = "target_absent" -> {Other}
    [] OTHER -> {}

\* @type: Str => Set(Str);
PreSuperseded(c) ==
  CASE c = "all_ownership" -> {Target, Other}
    [] c = "superseded_only" -> {Target}
    [] c = "target_absent" -> {Other}
    [] OTHER -> {}

\* @type: Str => Set(<<Str, Str>>);
PreSlots(c) ==
  CASE c = "all_ownership" ->
       {<<RoundA, Target>>, <<RoundB, Target>>, <<RoundB, Other>>}
    [] c = "slot_only_mixed_rounds" ->
       {<<RoundA, Target>>, <<RoundB, Other>>, <<RoundC, Third>>}
    [] c = "target_absent" ->
       {<<RoundA, Other>>, <<RoundB, Third>>}
    [] c = "preexisting_empty_round" ->
       {<<RoundA, Target>>, <<RoundB, Other>>}
    [] c = "all_target_rounds" ->
       {<<RoundA, Target>>, <<RoundB, Target>>}
    [] c = "other_only_round" ->
       {<<RoundA, Other>>}
    [] c = "target_and_other_same_round" ->
       {<<RoundA, Target>>, <<RoundA, Other>>}
    [] OTHER -> {}

\* @type: Str => Set(Str);
PreRounds(c) ==
  CASE c = "all_ownership" -> {RoundA, RoundB}
    [] c = "slot_only_mixed_rounds" -> {RoundA, RoundB, RoundC}
    [] c = "target_absent" -> {RoundA, RoundB}
    [] c = "preexisting_empty_round" -> {RoundA, RoundB, EmptyRound}
    [] c = "all_target_rounds" -> {RoundA, RoundB}
    [] c = "other_only_round" -> {RoundA}
    [] c = "target_and_other_same_round" -> {RoundA}
    [] OTHER -> {}

\* @type: Str => Set(Str);
SpecInflight(c) ==
  PreInflight(c) \ {Target}

\* @type: Str => Set(Str);
SpecVNextInflight(c) ==
  PreVNextInflight(c) \ {Target}

\* @type: Str => Set(Str);
SpecSuperseded(c) ==
  PreSuperseded(c) \ {Target}

\* @type: Str => Set(<<Str, Str>>);
SpecSlots(c) ==
  {pair \in PreSlots(c): pair[2] # Target}

\* @type: Str => Set(Str);
SpecRounds(c) ==
  {pair[1] : pair \in SpecSlots(c)}

\* @type: Str => Set(Str);
ActualInflight(c) ==
  CASE Bug = "skip_inflight_remove" -> PreInflight(c)
    [] Bug = "drop_other_inflight" -> SpecInflight(c) \ {Other}
    [] OTHER -> SpecInflight(c)

\* @type: Str => Set(Str);
ActualVNextInflight(c) ==
  CASE Bug = "skip_vnext_inflight_remove" -> PreVNextInflight(c)
    [] Bug = "drop_other_vnext_inflight" -> SpecVNextInflight(c) \ {Other}
    [] OTHER -> SpecVNextInflight(c)

\* @type: Str => Set(Str);
ActualSuperseded(c) ==
  CASE Bug = "skip_superseded_remove" -> PreSuperseded(c)
    [] Bug = "drop_other_superseded" -> SpecSuperseded(c) \ {Other}
    [] OTHER -> SpecSuperseded(c)

\* @type: Str => Set(<<Str, Str>>);
ActualSlots(c) ==
  CASE Bug = "skip_slot_remove" -> PreSlots(c)
    [] Bug = "drop_other_slot" ->
       SpecSlots(c) \ {pair \in SpecSlots(c): pair[2] = Other}
    [] OTHER -> SpecSlots(c)

\* @type: Str => Set(Str);
ActualRounds(c) ==
  CASE Bug = "keep_empty_round" -> PreRounds(c)
    [] Bug = "drop_nonempty_round" -> SpecRounds(c) \ {RoundB}
    [] OTHER -> SpecRounds(c)

Init ==
  /\ candidate = "none"
  /\ inflight = {}
  /\ vnextInflight = {}
  /\ superseded = {}
  /\ vnextSlots = {}
  /\ retainedRounds = {}

Next ==
  \E c \in Cases:
    /\ candidate' = c
    /\ inflight' = ActualInflight(c)
    /\ vnextInflight' = ActualVNextInflight(c)
    /\ superseded' = ActualSuperseded(c)
    /\ vnextSlots' = ActualSlots(c)
    /\ retainedRounds' = ActualRounds(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_inflight_remove",
       "skip_vnext_inflight_remove",
       "skip_superseded_remove",
       "skip_slot_remove",
       "drop_other_inflight",
       "drop_other_vnext_inflight",
       "drop_other_superseded",
       "drop_other_slot",
       "keep_empty_round",
       "drop_nonempty_round"
     }
  /\ candidate \in Cases \cup {"none"}
  /\ inflight \subseteq Hashes
  /\ vnextInflight \subseteq Hashes
  /\ superseded \subseteq Hashes
  /\ vnextSlots \subseteq RoundHashPairs
  /\ retainedRounds \subseteq Rounds

Safety ==
  candidate = "none" \/
    /\ inflight = SpecInflight(candidate)
    /\ vnextInflight = SpecVNextInflight(candidate)
    /\ superseded = SpecSuperseded(candidate)
    /\ vnextSlots = SpecSlots(candidate)
    /\ retainedRounds = SpecRounds(candidate)

SpecTargetRemoved ==
  candidate = "none" \/
    /\ Target \notin inflight
    /\ Target \notin vnextInflight
    /\ Target \notin superseded
    /\ \A pair \in vnextSlots: pair[2] # Target

SpecUnrelatedOwnershipPreserved ==
  candidate = "none" \/
    /\ \A hash \in Hashes \ {Target}:
        /\ ((hash \in inflight) <=> (hash \in PreInflight(candidate)))
        /\ ((hash \in vnextInflight) <=> (hash \in PreVNextInflight(candidate)))
        /\ ((hash \in superseded) <=> (hash \in PreSuperseded(candidate)))
    /\ \A pair \in RoundHashPairs:
        pair[2] # Target =>
          ((pair \in vnextSlots) <=> (pair \in PreSlots(candidate)))

SpecRoundProjection ==
  candidate = "none" \/
    retainedRounds = {pair[1] : pair \in vnextSlots}

SpecNoEmptyRounds ==
  candidate = "none" \/
    /\ \A round \in retainedRounds:
        \E pair \in vnextSlots: pair[1] = round
    /\ \A round \in Rounds:
        (\E pair \in vnextSlots: pair[1] = round) => round \in retainedRounds

====
