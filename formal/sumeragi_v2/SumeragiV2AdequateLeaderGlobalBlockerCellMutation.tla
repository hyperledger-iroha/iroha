---- MODULE SumeragiV2AdequateLeaderGlobalBlockerCellMutation ----
EXTENDS Naturals

(***************************************************************************
This mutation isolates the equal-rank witness-swap defect in the
adequate-leader global-blocker closure.

The rank-only specification may select an unrelated cell at the same service
rank as the frozen source cell.  Fairly servicing and replenishing that
replacement does not release the original cell, so the original-cell progress
property has a fair lasso.

The exact-cell specification selects the frozen cell identity.  Both
specifications use the same service action, so selection identity is the only
red/green semantic difference.
***************************************************************************)

VARIABLES originalOwned, replacementGeneration, selectedCell

vars == <<originalOwned, replacementGeneration, selectedCell>>

Cells == {"Unselected", "Original", "Replacement"}

CellRank(cell) ==
  IF cell \in {"Original", "Replacement"}
  THEN <<2, 1>>
  ELSE <<0, 0>>

TypeInvariant ==
  /\ originalOwned \in BOOLEAN
  /\ replacementGeneration \in 0..1
  /\ selectedCell \in Cells

SelectedCellHasOriginalRank ==
  \/ selectedCell = "Unselected"
  \/ CellRank(selectedCell) = CellRank("Original")

Init ==
  /\ originalOwned = TRUE
  /\ replacementGeneration = 0
  /\ selectedCell = "Unselected"

SelectEqualRankReplacement ==
  /\ originalOwned
  /\ selectedCell = "Unselected"
  /\ selectedCell' = "Replacement"
  /\ UNCHANGED <<originalOwned, replacementGeneration>>

SelectFrozenOriginal ==
  /\ originalOwned
  /\ selectedCell = "Unselected"
  /\ selectedCell' = "Original"
  /\ UNCHANGED <<originalOwned, replacementGeneration>>

ServiceSelectedCell ==
  /\ originalOwned
  /\ selectedCell \in {"Original", "Replacement"}
  /\ IF selectedCell = "Original"
        THEN /\ originalOwned' = FALSE
             /\ UNCHANGED replacementGeneration
        ELSE /\ originalOwned' = TRUE
             /\ replacementGeneration' = 1 - replacementGeneration
  /\ UNCHANGED selectedCell

RankOnlyNext ==
  SelectEqualRankReplacement \/ ServiceSelectedCell

RankOnlySpec ==
  /\ Init
  /\ [][RankOnlyNext]_vars
  /\ WF_vars(SelectEqualRankReplacement)
  /\ WF_vars(ServiceSelectedCell)

ExactCellNext ==
  SelectFrozenOriginal \/ ServiceSelectedCell

ExactCellSpec ==
  /\ Init
  /\ [][ExactCellNext]_vars
  /\ WF_vars(SelectFrozenOriginal)
  /\ WF_vars(ServiceSelectedCell)

OriginalCellEventuallyReleased ==
  originalOwned ~> ~originalOwned

=============================================================================
