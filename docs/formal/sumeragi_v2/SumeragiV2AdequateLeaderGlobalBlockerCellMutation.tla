---- MODULE SumeragiV2AdequateLeaderGlobalBlockerCellMutation ----
EXTENDS Naturals

(***************************************************************************
This mutation isolates the equal-rank witness-swap defect in the
adequate-leader global-blocker closure.

The rank-only specification may replace the frozen source cell with an
unrelated cell at the same service rank.  Fairly servicing and replenishing
that replacement does not release the original cell, so the original-cell
progress property has a fair lasso.

The exact-cell specification carries the frozen cell identity through
selection.  Its fair service action therefore releases the original cell
before unrelated equal-rank replenishment can continue.
***************************************************************************)

VARIABLES originalOwned, replacementGeneration, selectedCell

vars == <<originalOwned, replacementGeneration, selectedCell>>

Cells == {"Original", "Replacement"}

CellRank(cell) ==
  IF cell \in Cells
  THEN <<2, 1>>
  ELSE <<3, 1>>

TypeInvariant ==
  /\ originalOwned \in BOOLEAN
  /\ replacementGeneration \in 0..1
  /\ selectedCell \in Cells

SelectedCellHasOriginalRank ==
  CellRank(selectedCell) = CellRank("Original")

RankOnlyInit ==
  /\ originalOwned = TRUE
  /\ replacementGeneration = 0
  /\ selectedCell = "Original"

SelectEqualRankReplacement ==
  /\ originalOwned
  /\ selectedCell = "Original"
  /\ selectedCell' = "Replacement"
  /\ UNCHANGED <<originalOwned, replacementGeneration>>

ServiceAndReplenishReplacement ==
  /\ originalOwned
  /\ selectedCell = "Replacement"
  /\ replacementGeneration' = 1 - replacementGeneration
  /\ UNCHANGED <<originalOwned, selectedCell>>

RankOnlyNext ==
  SelectEqualRankReplacement \/ ServiceAndReplenishReplacement

RankOnlySpec ==
  /\ RankOnlyInit
  /\ [][RankOnlyNext]_vars
  /\ WF_vars(SelectEqualRankReplacement)
  /\ WF_vars(ServiceAndReplenishReplacement)

ExactCellInit == RankOnlyInit

ServiceExactOriginalCell ==
  /\ originalOwned
  /\ selectedCell = "Original"
  /\ originalOwned' = FALSE
  /\ UNCHANGED <<replacementGeneration, selectedCell>>

ChurnReplacementAfterOriginalService ==
  /\ ~originalOwned
  /\ replacementGeneration' = 1 - replacementGeneration
  /\ selectedCell' = "Replacement"
  /\ UNCHANGED originalOwned

ExactCellNext ==
  ServiceExactOriginalCell \/ ChurnReplacementAfterOriginalService

ExactCellSpec ==
  /\ ExactCellInit
  /\ [][ExactCellNext]_vars
  /\ WF_vars(ServiceExactOriginalCell)
  /\ WF_vars(ChurnReplacementAfterOriginalService)

OriginalCellEventuallyReleased ==
  originalOwned ~> ~originalOwned

=============================================================================
