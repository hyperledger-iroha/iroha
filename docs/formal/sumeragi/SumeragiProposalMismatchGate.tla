---- MODULE SumeragiProposalMismatchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `detect_proposal_mismatch(...)`.

The helper compares proposal metadata against the block header and recomputed
payload hash. It must return the first mismatch in implementation order:
height, view, parent hash, transaction root, state root, then payload hash.
Missing parent and transaction roots default to the zero hash. A proposal state
root equal to the zero hash is accepted as a compatibility value even when the
block header carries a non-zero execution-result root.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  height_matches,
  \* @type: Bool;
  view_matches,
  \* @type: Bool;
  block_has_parent,
  \* @type: Bool;
  proposal_parent_zero,
  \* @type: Bool;
  parent_matches,
  \* @type: Bool;
  block_has_tx_root,
  \* @type: Bool;
  proposal_tx_root_zero,
  \* @type: Bool;
  tx_root_matches,
  \* @type: Bool;
  block_has_state_root,
  \* @type: Bool;
  proposal_state_root_zero,
  \* @type: Bool;
  state_root_matches,
  \* @type: Bool;
  payload_hash_matches,
  \* @type: Bool;
  mismatch_found,
  \* @type: Str;
  mismatch_kind,
  \* @type: Int;
  mismatch_rank

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Str, Int>>;
vars ==
  <<candidate, height_matches, view_matches, block_has_parent,
    proposal_parent_zero, parent_matches, block_has_tx_root,
    proposal_tx_root_zero, tx_root_matches, block_has_state_root,
    proposal_state_root_zero, state_root_matches, payload_hash_matches,
    mismatch_found, mismatch_kind, mismatch_rank>>

Kinds == {"None", "Height", "View", "Parent", "TxRoot", "StateRoot", "PayloadHash"}

Cases == {
  "matching_full",
  "genesis_parent_zero_ok",
  "genesis_parent_mismatch",
  "missing_tx_root_zero_ok",
  "missing_tx_root_mismatch",
  "state_root_zero_compat",
  "state_zero_and_payload_mismatch",
  "height_mismatch",
  "view_mismatch",
  "parent_mismatch",
  "tx_root_mismatch",
  "state_root_mismatch",
  "payload_hash_mismatch",
  "height_priority_over_payload",
  "view_priority_over_parent",
  "parent_priority_over_tx",
  "tx_priority_over_state",
  "state_priority_over_payload"
}

RankValues == 0..6

KindRank(kind) ==
  CASE kind = "None" -> 0
    [] kind = "Height" -> 1
    [] kind = "View" -> 2
    [] kind = "Parent" -> 3
    [] kind = "TxRoot" -> 4
    [] kind = "StateRoot" -> 5
    [] kind = "PayloadHash" -> 6

SpecHeightMatches(c) ==
  c # "height_mismatch" /\ c # "height_priority_over_payload"

SpecViewMatches(c) ==
  c # "view_mismatch" /\ c # "view_priority_over_parent"

SpecBlockHasParent(c) ==
  c # "genesis_parent_zero_ok" /\ c # "genesis_parent_mismatch"

SpecProposalParentZero(c) ==
  c = "genesis_parent_zero_ok"

SpecParentMatches(c) ==
  c # "genesis_parent_mismatch" /\
  c # "parent_mismatch" /\
  c # "view_priority_over_parent" /\
  c # "parent_priority_over_tx"

SpecBlockHasTxRoot(c) ==
  c # "missing_tx_root_zero_ok" /\ c # "missing_tx_root_mismatch"

SpecProposalTxRootZero(c) ==
  c = "missing_tx_root_zero_ok"

SpecTxRootMatches(c) ==
  c # "missing_tx_root_mismatch" /\
  c # "tx_root_mismatch" /\
  c # "parent_priority_over_tx" /\
  c # "tx_priority_over_state"

SpecBlockHasStateRoot(c) ==
  c \in {
    "state_root_zero_compat",
    "state_zero_and_payload_mismatch",
    "state_root_mismatch",
    "tx_priority_over_state",
    "state_priority_over_payload"
  }

SpecProposalStateRootZero(c) ==
  c = "state_root_zero_compat" \/ c = "state_zero_and_payload_mismatch"

SpecStateRootMatches(c) ==
  c # "state_root_zero_compat" /\
  c # "state_zero_and_payload_mismatch" /\
  c # "state_root_mismatch" /\
  c # "tx_priority_over_state" /\
  c # "state_priority_over_payload"

SpecPayloadHashMatches(c) ==
  c # "payload_hash_mismatch" /\
  c # "height_priority_over_payload" /\
  c # "state_zero_and_payload_mismatch" /\
  c # "state_priority_over_payload"

StateRootCompatible(c) ==
  SpecStateRootMatches(c) \/ SpecProposalStateRootZero(c)

SpecMismatchKind(c) ==
  IF ~SpecHeightMatches(c) THEN "Height"
  ELSE IF ~SpecViewMatches(c) THEN "View"
  ELSE IF ~SpecParentMatches(c) THEN "Parent"
  ELSE IF ~SpecTxRootMatches(c) THEN "TxRoot"
  ELSE IF ~SpecStateRootMatches(c) /\ ~SpecProposalStateRootZero(c) THEN "StateRoot"
  ELSE IF ~SpecPayloadHashMatches(c) THEN "PayloadHash"
  ELSE "None"

ActualMismatchKind(c) ==
  CASE Bug = "accept_height_mismatch" /\ c = "height_mismatch" -> "None"
    [] Bug = "accept_view_mismatch" /\ c = "view_mismatch" -> "None"
    [] Bug = "accept_parent_mismatch" /\ c = "parent_mismatch" -> "None"
    [] Bug = "accept_tx_root_mismatch" /\ c = "tx_root_mismatch" -> "None"
    [] Bug = "accept_state_root_mismatch" /\ c = "state_root_mismatch" -> "None"
    [] Bug = "accept_payload_hash_mismatch" /\ c = "payload_hash_mismatch" -> "None"
    [] Bug = "reject_state_zero_compat" /\ c = "state_root_zero_compat" -> "StateRoot"
    [] Bug = "parent_default_not_zero" /\ c = "genesis_parent_zero_ok" -> "Parent"
    [] Bug = "tx_default_not_zero" /\ c = "missing_tx_root_zero_ok" -> "TxRoot"
    [] Bug = "payload_checked_before_height" /\ c = "height_priority_over_payload" ->
         "PayloadHash"
    [] Bug = "parent_checked_before_view" /\ c = "view_priority_over_parent" ->
         "Parent"
    [] Bug = "tx_checked_before_parent" /\ c = "parent_priority_over_tx" ->
         "TxRoot"
    [] Bug = "state_checked_before_tx" /\ c = "tx_priority_over_state" ->
         "StateRoot"
    [] Bug = "payload_checked_before_state" /\ c = "state_priority_over_payload" ->
         "PayloadHash"
    [] Bug = "state_zero_skips_payload" /\ c = "state_zero_and_payload_mismatch" ->
         "None"
    [] OTHER -> SpecMismatchKind(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "accept_height_mismatch",
       "accept_view_mismatch",
       "accept_parent_mismatch",
       "accept_tx_root_mismatch",
       "accept_state_root_mismatch",
       "accept_payload_hash_mismatch",
       "reject_state_zero_compat",
       "parent_default_not_zero",
       "tx_default_not_zero",
       "payload_checked_before_height",
       "parent_checked_before_view",
       "tx_checked_before_parent",
       "state_checked_before_tx",
       "payload_checked_before_state",
       "state_zero_skips_payload"
     }
  /\ candidate \in Cases
  /\ height_matches \in BOOLEAN
  /\ view_matches \in BOOLEAN
  /\ block_has_parent \in BOOLEAN
  /\ proposal_parent_zero \in BOOLEAN
  /\ parent_matches \in BOOLEAN
  /\ block_has_tx_root \in BOOLEAN
  /\ proposal_tx_root_zero \in BOOLEAN
  /\ tx_root_matches \in BOOLEAN
  /\ block_has_state_root \in BOOLEAN
  /\ proposal_state_root_zero \in BOOLEAN
  /\ state_root_matches \in BOOLEAN
  /\ payload_hash_matches \in BOOLEAN
  /\ mismatch_found \in BOOLEAN
  /\ mismatch_kind \in Kinds
  /\ mismatch_rank \in RankValues

Init ==
  /\ candidate \in Cases
  /\ height_matches = SpecHeightMatches(candidate)
  /\ view_matches = SpecViewMatches(candidate)
  /\ block_has_parent = SpecBlockHasParent(candidate)
  /\ proposal_parent_zero = SpecProposalParentZero(candidate)
  /\ parent_matches = SpecParentMatches(candidate)
  /\ block_has_tx_root = SpecBlockHasTxRoot(candidate)
  /\ proposal_tx_root_zero = SpecProposalTxRootZero(candidate)
  /\ tx_root_matches = SpecTxRootMatches(candidate)
  /\ block_has_state_root = SpecBlockHasStateRoot(candidate)
  /\ proposal_state_root_zero = SpecProposalStateRootZero(candidate)
  /\ state_root_matches = SpecStateRootMatches(candidate)
  /\ payload_hash_matches = SpecPayloadHashMatches(candidate)
  /\ mismatch_kind = ActualMismatchKind(candidate)
  /\ mismatch_found = (ActualMismatchKind(candidate) # "None")
  /\ mismatch_rank = KindRank(ActualMismatchKind(candidate))

Next ==
  UNCHANGED vars

MismatchKindMatchesSpec ==
  mismatch_kind = SpecMismatchKind(candidate)

MismatchFoundMatchesSpec ==
  mismatch_found = (SpecMismatchKind(candidate) # "None")

MismatchRankMatchesSpec ==
  mismatch_rank = KindRank(SpecMismatchKind(candidate))

HeightMismatchHasPriority ==
  ~height_matches => mismatch_kind = "Height"

ViewMismatchAfterHeight ==
  height_matches /\ ~view_matches => mismatch_kind = "View"

ParentMismatchAfterView ==
  height_matches /\ view_matches /\ ~parent_matches =>
    mismatch_kind = "Parent"

TxRootMismatchAfterParent ==
  height_matches /\ view_matches /\ parent_matches /\ ~tx_root_matches =>
    mismatch_kind = "TxRoot"

StateRootMismatchAfterTx ==
  height_matches /\ view_matches /\ parent_matches /\ tx_root_matches /\
  ~state_root_matches /\ ~proposal_state_root_zero =>
    mismatch_kind = "StateRoot"

PayloadMismatchAfterCompatibleHeader ==
  height_matches /\ view_matches /\ parent_matches /\ tx_root_matches /\
  (state_root_matches \/ proposal_state_root_zero) /\ ~payload_hash_matches =>
    mismatch_kind = "PayloadHash"

ZeroStateRootCompatDoesNotReject ==
  height_matches /\ view_matches /\ parent_matches /\ tx_root_matches /\
  ~state_root_matches /\ proposal_state_root_zero /\ payload_hash_matches =>
    ~mismatch_found

ZeroStateRootCompatStillChecksPayload ==
  height_matches /\ view_matches /\ parent_matches /\ tx_root_matches /\
  ~state_root_matches /\ proposal_state_root_zero /\ ~payload_hash_matches =>
    mismatch_kind = "PayloadHash"

MatchingProposalAccepted ==
  candidate = "matching_full" => ~mismatch_found

GenesisParentDefaultZeroAccepted ==
  ~block_has_parent /\ proposal_parent_zero /\ height_matches /\ view_matches /\
  parent_matches /\ tx_root_matches /\ state_root_matches /\ payload_hash_matches =>
    ~mismatch_found

MissingTxRootDefaultZeroAccepted ==
  ~block_has_tx_root /\ proposal_tx_root_zero /\ height_matches /\ view_matches /\
  parent_matches /\ tx_root_matches /\ state_root_matches /\ payload_hash_matches =>
    ~mismatch_found

NoMismatchOnlyWhenCompatible ==
  ~mismatch_found =>
    /\ height_matches
    /\ view_matches
    /\ parent_matches
    /\ tx_root_matches
    /\ (state_root_matches \/ proposal_state_root_zero)
    /\ payload_hash_matches

ProposalMismatchCoreSafety ==
  /\ MismatchKindMatchesSpec
  /\ MismatchFoundMatchesSpec
  /\ MismatchRankMatchesSpec
  /\ HeightMismatchHasPriority
  /\ ViewMismatchAfterHeight
  /\ ParentMismatchAfterView
  /\ TxRootMismatchAfterParent
  /\ StateRootMismatchAfterTx
  /\ PayloadMismatchAfterCompatibleHeader
  /\ ZeroStateRootCompatDoesNotReject
  /\ ZeroStateRootCompatStillChecksPayload
  /\ MatchingProposalAccepted
  /\ GenesisParentDefaultZeroAccepted
  /\ MissingTxRootDefaultZeroAccepted
  /\ NoMismatchOnlyWhenCompatible

ProposalMismatchExactness ==
  /\ MismatchKindMatchesSpec
  /\ MismatchFoundMatchesSpec
  /\ MismatchRankMatchesSpec
  /\ HeightMismatchHasPriority
  /\ ViewMismatchAfterHeight
  /\ ParentMismatchAfterView
  /\ TxRootMismatchAfterParent
  /\ StateRootMismatchAfterTx
  /\ PayloadMismatchAfterCompatibleHeader
  /\ ZeroStateRootCompatDoesNotReject
  /\ ZeroStateRootCompatStillChecksPayload
  /\ MatchingProposalAccepted
  /\ GenesisParentDefaultZeroAccepted
  /\ MissingTxRootDefaultZeroAccepted
  /\ NoMismatchOnlyWhenCompatible
ProposalMismatchCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalMismatchExactness

Safety == ProposalMismatchExactness

====
