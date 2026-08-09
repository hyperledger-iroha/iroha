---- MODULE SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Bounded mutation witness for the selected-lifecycle episode.

Selected A owns occurrence 2, immutable token T, lifecycle ordinal 4, and
physical rank 2.  Lower semantic occurrence B already coexists, but its
physical rank is 3 and therefore cannot witness strict physical descent.
Draining A first carries T/4 through one exact pre-candidate route at the
unchanged physical rank; only servicing that route reaches physical rank 1.

The repaired model keeps A's selected-lifecycle episode active despite B.
The mutation reinstates the old `~StrictOccurrenceDescentGoal` gate, so the
episode disappears as soon as selection occurs even though no lower physical
rank exists.  No production fairness is modeled here.
***************************************************************************)

CONSTANT PreserveSelectedLifecycleEpisode

ASSUME PreserveSelectedLifecycleEpisode \in BOOLEAN

SelectedToken == "T"
SourceOrdinal == 4
SourceOccurrenceRank == 2
LowerOccurrenceRank == 1
SourcePhysicalRank == 2
LowerPhysicalRank == 3

Stages == {"Fresh", "Selected", "Route", "Done"}
NoToken == "None"

VARIABLES stage, routeToken, routeOrdinal

vars == <<stage, routeToken, routeOrdinal>>

TypeInvariant ==
  /\ stage \in Stages
  /\ routeToken \in {NoToken, SelectedToken}
  /\ routeOrdinal \in 0..SourceOrdinal

LowerOccurrenceCoexists ==
  /\ LowerOccurrenceRank < SourceOccurrenceRank
  /\ LowerPhysicalRank >= SourcePhysicalRank

PhysicalRank ==
  IF stage = "Done" THEN SourcePhysicalRank - 1
  ELSE SourcePhysicalRank

PhysicalStrictRankGoal ==
  PhysicalRank < SourcePhysicalRank

SelectedLifecycleCarrier ==
  \/ stage = "Selected"
  \/ /\ stage = "Route"
     /\ routeToken = SelectedToken
     /\ routeOrdinal = SourceOrdinal

SelectedLifecycleEpisodeActive ==
  /\ SelectedLifecycleCarrier
  /\ IF PreserveSelectedLifecycleEpisode
     THEN TRUE
     ELSE ~LowerOccurrenceCoexists

\* Coexisting semantic descent is not physical progress.  Until the exact
\* selected token/cut reaches a lower physical rank, its episode must remain
\* active.
SelectedLifecycleEpisodeOrPhysicalDescent ==
  \/ stage = "Fresh"
  \/ PhysicalStrictRankGoal
  \/ SelectedLifecycleEpisodeActive

ExactSelectedTokenCutCarry ==
  stage = "Route"
    => /\ routeToken = SelectedToken
       /\ routeOrdinal = SourceOrdinal

Init ==
  /\ stage = "Fresh"
  /\ routeToken = NoToken
  /\ routeOrdinal = 0

SelectLifecycleEpisode ==
  /\ stage = "Fresh"
  /\ stage' = "Selected"
  /\ UNCHANGED <<routeToken, routeOrdinal>>

DrainSelectedToExactRoute ==
  /\ stage = "Selected"
  /\ stage' = "Route"
  /\ routeToken' = SelectedToken
  /\ routeOrdinal' = SourceOrdinal

ServiceExactRoute ==
  /\ stage = "Route"
  /\ stage' = "Done"
  /\ UNCHANGED <<routeToken, routeOrdinal>>

Next ==
  \/ SelectLifecycleEpisode
  \/ DrainSelectedToExactRoute
  \/ ServiceExactRoute

Spec ==
  /\ Init
  /\ [][Next]_vars

=============================================================================
