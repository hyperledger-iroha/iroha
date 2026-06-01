---- MODULE SumeragiCollectorSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi collector fanout/selection helpers.

This slice covers `collector_fanout_floor(...)`,
`collector_indices_k(...)`, `collector_indices_k_fallback(...)`,
`collector_indices_k_prf(...)`, and the `deterministic_collectors(...)`
wrapper. It abstracts concrete peer ids into numeric indices and abstracts the
hash-based PRF into its required return contract: distinct in-range collectors
that exclude the leader in multi-peer topologies.
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
  "empty_k_positive",
  "multi_zero_k",
  "single_k_zero",
  "single_k_positive",
  "len4_k1",
  "len4_k10",
  "len6_k2",
  "len7_k2",
  "len7_k10",
  "perm_no_seed",
  "perm_seed",
  "npos_no_seed",
  "npos_seed"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

RosterLen(c) ==
  CASE c = "empty_k_positive" -> 0
    [] c \in {"single_k_zero", "single_k_positive"} -> 1
    [] c \in {"len6_k2"} -> 6
    [] c \in {"len7_k2", "len7_k10"} -> 7
    [] OTHER -> 4

RequestedK(c) ==
  CASE c \in {"multi_zero_k", "single_k_zero"} -> 0
    [] c \in {"len4_k10", "len7_k10"} -> 10
    [] c = "len6_k2" -> 2
    [] c \in {"len7_k2"} -> 2
    [] OTHER -> 1

Mode(c) ==
  IF c \in {"npos_no_seed", "npos_seed"} THEN "npos" ELSE "permissioned"

SeedPresent(c) ==
  c \in {"perm_seed", "npos_seed"}

SpecCommitQuorum(n) ==
  IF n <= 3 THEN n ELSE (n * 2) \div 3 + 1

SpecProxyTail(c) ==
  IF SpecCommitQuorum(RosterLen(c)) = 0
  THEN 0
  ELSE SpecCommitQuorum(RosterLen(c)) - 1

SpecEffectiveK(c) ==
  LET n == RosterLen(c) IN
  LET k == RequestedK(c) IN
  IF n = 0 \/ k = 0 THEN 0
  ELSE IF n = 1 THEN 1
  ELSE Min(Max(k, SpecCommitQuorum(n)), n - 1)

SpecDefaultLen(c) ==
  LET n == RosterLen(c) IN
  LET k == RequestedK(c) IN
  LET start == SpecProxyTail(c) IN
  LET finish == Min(start + k, n) IN
  IF n = 0 \/ k = 0 THEN 0
  ELSE IF n = 1 THEN 1
  ELSE IF finish <= start THEN 0
  ELSE finish - start

SpecDefaultFirst(c) ==
  IF SpecDefaultLen(c) = 0 THEN 0 ELSE SpecProxyTail(c)

SpecDefaultSecond(c) ==
  IF SpecDefaultLen(c) <= 1 THEN 0 ELSE SpecProxyTail(c) + 1

SpecDefaultLast(c) ==
  IF SpecDefaultLen(c) = 0 THEN 0 ELSE SpecProxyTail(c) + SpecDefaultLen(c) - 1

SpecDefaultHasLeaderForMulti(c) == FALSE

SpecDefaultWraps(c) == FALSE

SpecFallbackLen(c) == SpecEffectiveK(c)

SpecFallbackFirst(c) ==
  CASE SpecFallbackLen(c) = 0 -> 0
    [] RosterLen(c) = 1 -> 0
    [] OTHER -> SpecProxyTail(c)

SpecFallbackSecond(c) ==
  CASE SpecFallbackLen(c) <= 1 -> 0
    [] RosterLen(c) = 4 -> 3
    [] RosterLen(c) = 6 -> 5
    [] RosterLen(c) = 7 -> 5
    [] OTHER -> 0

SpecFallbackLast(c) ==
  CASE SpecFallbackLen(c) = 0 -> 0
    [] RosterLen(c) = 1 -> 0
    [] RosterLen(c) = 4 -> 1
    [] RosterLen(c) = 6 -> 3
    [] RosterLen(c) = 7 /\ SpecFallbackLen(c) = 5 -> 2
    [] RosterLen(c) = 7 /\ SpecFallbackLen(c) = 6 -> 3
    [] OTHER -> 0

SpecFallbackWraps(c) ==
  RosterLen(c) > 1 /\ SpecFallbackLen(c) > 0
    /\ SpecProxyTail(c) + SpecFallbackLen(c) > RosterLen(c)

SpecFallbackHasLeaderForMulti(c) == FALSE

SpecFallbackDistinct(c) == TRUE

SpecPrfLen(c) == SpecEffectiveK(c)

SpecPrfHasLeaderForMulti(c) == FALSE

SpecPrfDistinct(c) == TRUE

SpecPrfInRange(c) == TRUE

SpecDeterministicSource(c) ==
  IF SeedPresent(c) THEN "prf" ELSE "fallback"

SpecDeterministicLen(c) ==
  IF SpecDeterministicSource(c) = "prf"
  THEN SpecPrfLen(c)
  ELSE SpecFallbackLen(c)

SpecDeterministicHasLeaderForMulti(c) == FALSE

SpecDeterministicDistinct(c) ==
  IF SpecDeterministicSource(c) = "prf"
  THEN TRUE
  ELSE TRUE

SpecOutput(c) ==
  <<RosterLen(c), RequestedK(c), SpecCommitQuorum(RosterLen(c)),
    SpecProxyTail(c), SpecEffectiveK(c), SpecDefaultLen(c),
    SpecDefaultFirst(c), SpecDefaultSecond(c), SpecDefaultLast(c),
    SpecDefaultHasLeaderForMulti(c), SpecDefaultWraps(c),
    SpecFallbackLen(c), SpecFallbackFirst(c), SpecFallbackSecond(c),
    SpecFallbackLast(c), SpecFallbackHasLeaderForMulti(c),
    SpecFallbackDistinct(c), SpecFallbackWraps(c), SpecPrfLen(c),
    SpecPrfHasLeaderForMulti(c), SpecPrfDistinct(c), SpecPrfInRange(c),
    Mode(c), SeedPresent(c), SpecDeterministicSource(c),
    SpecDeterministicLen(c), SpecDeterministicHasLeaderForMulti(c),
    SpecDeterministicDistinct(c)>>

ActualCommitQuorum(c) ==
  CASE Bug = "quorum_underestimates_len4" /\ RosterLen(c) = 4 -> 2
    [] OTHER -> SpecCommitQuorum(RosterLen(c))

ActualProxyTail(c) ==
  CASE Bug = "proxy_tail_is_leader" /\ c = "len4_k1" -> 0
    [] OTHER ->
         IF ActualCommitQuorum(c) = 0 THEN 0 ELSE ActualCommitQuorum(c) - 1

ActualEffectiveK(c) ==
  CASE Bug = "fanout_allows_zero_k" /\ c = "multi_zero_k" ->
         SpecCommitQuorum(RosterLen(c))
    [] Bug = "fanout_ignores_quorum_floor" /\ c = "len4_k1" ->
         RequestedK(c)
    [] Bug = "fanout_exceeds_nonleader" /\ c = "len7_k10" ->
         RosterLen(c)
    [] Bug = "single_peer_empty" /\ c = "single_k_positive" -> 0
    [] OTHER ->
         LET n == RosterLen(c) IN
         LET k == RequestedK(c) IN
         IF n = 0 \/ k = 0 THEN 0
         ELSE IF n = 1 THEN 1
         ELSE Min(Max(k, ActualCommitQuorum(c)), n - 1)

ActualDefaultLen(c) ==
  CASE Bug = "default_wraps" /\ c = "len4_k10" -> 3
    [] OTHER -> SpecDefaultLen(c)

ActualDefaultFirst(c) ==
  CASE Bug = "default_skips_proxy" /\ c = "len4_k1" -> 3
    [] Bug = "proxy_tail_is_leader" /\ c = "len4_k1" -> ActualProxyTail(c)
    [] OTHER -> SpecDefaultFirst(c)

ActualDefaultSecond(c) ==
  IF ActualDefaultLen(c) <= 1 THEN 0 ELSE ActualDefaultFirst(c) + 1

ActualDefaultLast(c) ==
  CASE Bug = "default_wraps" /\ c = "len4_k10" -> 1
    [] OTHER -> SpecDefaultLast(c)

ActualDefaultHasLeaderForMulti(c) ==
  CASE Bug = "default_wraps" /\ c = "len4_k10" -> TRUE
    [] Bug = "proxy_tail_is_leader" /\ c = "len4_k1" -> TRUE
    [] OTHER -> FALSE

ActualDefaultWraps(c) ==
  CASE Bug = "default_wraps" /\ c = "len4_k10" -> TRUE
    [] OTHER -> FALSE

ActualFallbackLen(c) ==
  CASE Bug = "fallback_no_wrap" /\ c = "len4_k10" -> SpecDefaultLen(c)
    [] OTHER -> ActualEffectiveK(c)

ActualFallbackFirst(c) ==
  CASE Bug = "fallback_wrong_start" /\ c = "len4_k1" -> 3
    [] Bug = "fallback_includes_leader" /\ c = "len4_k1" -> 0
    [] OTHER -> SpecFallbackFirst(c)

ActualFallbackSecond(c) ==
  CASE Bug = "fallback_duplicates" /\ c = "len4_k1" -> ActualFallbackFirst(c)
    [] Bug = "fallback_includes_leader" /\ c = "len4_k1" -> 2
    [] Bug = "fallback_no_wrap" /\ c = "len4_k10" -> 3
    [] OTHER -> SpecFallbackSecond(c)

ActualFallbackLast(c) ==
  CASE Bug = "fallback_no_wrap" /\ c = "len4_k10" -> 3
    [] Bug = "fallback_includes_leader" /\ c = "len4_k1" -> 3
    [] Bug = "fallback_wrong_start" /\ c = "len4_k1" -> 2
    [] OTHER -> SpecFallbackLast(c)

ActualFallbackHasLeaderForMulti(c) ==
  CASE Bug = "fallback_includes_leader" /\ c = "len4_k1" -> TRUE
    [] OTHER -> FALSE

ActualFallbackDistinct(c) ==
  CASE Bug = "fallback_duplicates" /\ c = "len4_k1" -> FALSE
    [] OTHER -> TRUE

ActualFallbackWraps(c) ==
  CASE Bug = "fallback_no_wrap" /\ c = "len4_k10" -> FALSE
    [] OTHER -> SpecFallbackWraps(c)

ActualPrfLen(c) ==
  CASE Bug = "prf_underselects" /\ c = "perm_seed" ->
         ActualEffectiveK(c) - 1
    [] OTHER -> ActualEffectiveK(c)

ActualPrfHasLeaderForMulti(c) ==
  CASE Bug = "prf_includes_leader" /\ c = "perm_seed" -> TRUE
    [] OTHER -> FALSE

ActualPrfDistinct(c) ==
  CASE Bug = "prf_duplicates" /\ c = "perm_seed" -> FALSE
    [] OTHER -> TRUE

ActualPrfInRange(c) ==
  CASE Bug = "prf_out_of_range" /\ c = "perm_seed" -> FALSE
    [] OTHER -> TRUE

ActualDeterministicSource(c) ==
  CASE Bug = "seed_none_uses_prf" /\ c = "perm_no_seed" -> "prf"
    [] Bug = "seed_some_uses_fallback" /\ c = "perm_seed" -> "fallback"
    [] OTHER -> SpecDeterministicSource(c)

ActualDeterministicLen(c) ==
  IF ActualDeterministicSource(c) = "prf"
  THEN ActualPrfLen(c)
  ELSE ActualFallbackLen(c)

ActualDeterministicHasLeaderForMulti(c) ==
  IF ActualDeterministicSource(c) = "prf"
  THEN ActualPrfHasLeaderForMulti(c)
  ELSE ActualFallbackHasLeaderForMulti(c)

ActualDeterministicDistinct(c) ==
  IF ActualDeterministicSource(c) = "prf"
  THEN ActualPrfDistinct(c)
  ELSE ActualFallbackDistinct(c)

ActualOutput(c) ==
  <<RosterLen(c), RequestedK(c), ActualCommitQuorum(c), ActualProxyTail(c),
    ActualEffectiveK(c), ActualDefaultLen(c), ActualDefaultFirst(c),
    ActualDefaultSecond(c), ActualDefaultLast(c),
    ActualDefaultHasLeaderForMulti(c), ActualDefaultWraps(c),
    ActualFallbackLen(c), ActualFallbackFirst(c), ActualFallbackSecond(c),
    ActualFallbackLast(c), ActualFallbackHasLeaderForMulti(c),
    ActualFallbackDistinct(c), ActualFallbackWraps(c), ActualPrfLen(c),
    ActualPrfHasLeaderForMulti(c), ActualPrfDistinct(c),
    ActualPrfInRange(c), Mode(c), SeedPresent(c),
    ActualDeterministicSource(c), ActualDeterministicLen(c),
    ActualDeterministicHasLeaderForMulti(c),
    ActualDeterministicDistinct(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "quorum_underestimates_len4",
       "proxy_tail_is_leader",
       "fanout_allows_zero_k",
       "fanout_ignores_quorum_floor",
       "fanout_exceeds_nonleader",
       "single_peer_empty",
       "default_wraps",
       "default_skips_proxy",
       "fallback_no_wrap",
       "fallback_includes_leader",
       "fallback_duplicates",
       "fallback_wrong_start",
       "seed_none_uses_prf",
       "seed_some_uses_fallback",
       "prf_includes_leader",
       "prf_underselects",
       "prf_duplicates",
       "prf_out_of_range"
     }
  /\ checked = 0

SafetyFast ==
  /\ ActualOutput("empty_k_positive") = SpecOutput("empty_k_positive")
  /\ ActualOutput("multi_zero_k") = SpecOutput("multi_zero_k")
  /\ ActualOutput("single_k_zero") = SpecOutput("single_k_zero")
  /\ ActualOutput("single_k_positive") = SpecOutput("single_k_positive")
  /\ ActualOutput("len4_k1") = SpecOutput("len4_k1")
  /\ ActualOutput("len4_k10") = SpecOutput("len4_k10")
  /\ ActualOutput("len6_k2") = SpecOutput("len6_k2")
  /\ ActualOutput("len7_k2") = SpecOutput("len7_k2")
  /\ ActualOutput("len7_k10") = SpecOutput("len7_k10")
  /\ ActualOutput("perm_no_seed") = SpecOutput("perm_no_seed")
  /\ ActualOutput("perm_seed") = SpecOutput("perm_seed")
  /\ ActualOutput("npos_no_seed") = SpecOutput("npos_no_seed")
  /\ ActualOutput("npos_seed") = SpecOutput("npos_seed")

BugQuorumUnderestimatesLen4 ==
  ActualCommitQuorum("len4_k1") = SpecCommitQuorum(4)

BugProxyTailIsLeader ==
  ActualProxyTail("len4_k1") = SpecProxyTail("len4_k1")

BugFanoutAllowsZeroK ==
  ActualEffectiveK("multi_zero_k") = SpecEffectiveK("multi_zero_k")

BugFanoutIgnoresQuorumFloor ==
  ActualEffectiveK("len4_k1") = SpecEffectiveK("len4_k1")

BugFanoutExceedsNonleader ==
  ActualEffectiveK("len7_k10") = SpecEffectiveK("len7_k10")

BugSinglePeerEmpty ==
  ActualEffectiveK("single_k_positive") = SpecEffectiveK("single_k_positive")

BugDefaultWraps ==
  ActualDefaultWraps("len4_k10") = SpecDefaultWraps("len4_k10")

BugDefaultSkipsProxy ==
  ActualDefaultFirst("len4_k1") = SpecDefaultFirst("len4_k1")

BugFallbackNoWrap ==
  ActualFallbackLen("len4_k10") = SpecFallbackLen("len4_k10")

BugFallbackIncludesLeader ==
  ActualFallbackHasLeaderForMulti("len4_k1") =
    SpecFallbackHasLeaderForMulti("len4_k1")

BugFallbackDuplicates ==
  ActualFallbackDistinct("len4_k1") = SpecFallbackDistinct("len4_k1")

BugFallbackWrongStart ==
  ActualFallbackFirst("len4_k1") = SpecFallbackFirst("len4_k1")

BugSeedNoneUsesPrf ==
  ActualDeterministicSource("perm_no_seed") =
    SpecDeterministicSource("perm_no_seed")

BugSeedSomeUsesFallback ==
  ActualDeterministicSource("perm_seed") = SpecDeterministicSource("perm_seed")

BugPrfIncludesLeader ==
  ActualPrfHasLeaderForMulti("perm_seed") =
    SpecPrfHasLeaderForMulti("perm_seed")

BugPrfUnderselects ==
  ActualPrfLen("perm_seed") = SpecPrfLen("perm_seed")

BugPrfDuplicates ==
  ActualPrfDistinct("perm_seed") = SpecPrfDistinct("perm_seed")

BugPrfOutOfRange ==
  ActualPrfInRange("perm_seed") = SpecPrfInRange("perm_seed")

Safety ==
  SafetyFast

=============================================================================
====
