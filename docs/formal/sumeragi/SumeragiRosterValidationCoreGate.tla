---- MODULE SumeragiRosterValidationCoreGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for core roster validation.

This slice captures the deterministic validation policy in
`validate_commit_qc_roster(...)` and `validate_checkpoint_roster(...)` from
`main_loop.rs`, after the wrapper-level cache mechanics are abstracted away.
BLS arithmetic and hash bytes are represented by action labels, but the model
pins the observable acceptance gates: roster emptiness, validator-set hash
version and hash binding, signer bitmap length and bounds, genesis-stub
conditions, missing aggregate rejection, permissioned and NPoS quorum rules,
stake-snapshot matching, proof-of-possession lookup, checkpoint expiry and root
binding, preimage field selection, aggregate verification inputs, and returning
the full validated roster rather than the signer subset.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommitRejectEmptyRoster == 1
CommitHashVersionV1 == 2
CommitRosterHashBindsValidatorSet == 3
CommitBitmapLengthCeil == 4
CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap == 5
CommitMissingSignatureRejected == 6
CommitGenesisStubReturnsRoster == 7
CommitBitmapRejectsOutOfRange == 8
CommitSignerSetFromBitmap == 9
CommitPermissionedRequiresCommitQuorum == 10
CommitNposRequiresMatchingStakeSnapshot == 11
CommitNposStakeQuorumResult == 12
CommitRequiresPopForSigner == 13
CommitBlsPreimageUsesQcChainMode == 14
CommitBlsVerifiesSignerKeysPops == 15
CommitReturnsValidatorSet == 16
CheckpointHashVersionV1 == 17
CheckpointRejectExpiredAtBoundary == 18
CheckpointRejectEmptyRoster == 19
CheckpointRosterHashBindsValidatorSet == 20
CheckpointBitmapLengthCeil == 21
CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap == 22
CheckpointMissingSignatureRejected == 23
CheckpointBitmapRejectsOutOfRange == 24
CheckpointPermissionedRequiresCommitQuorum == 25
CheckpointNposRequiresMatchingStakeSnapshot == 26
CheckpointRootsMatchWhenProvided == 27
CheckpointVotePreimageFields == 28
CheckpointBlsVerifiesSignerKeysPops == 29
CheckpointReturnsValidatorSet == 30

Candidates == 1..30

CommitAdmissionCases == {
  CommitRejectEmptyRoster,
  CommitHashVersionV1,
  CommitRosterHashBindsValidatorSet,
  CommitBitmapLengthCeil
}

CommitGenesisSignatureCases == {
  CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap,
  CommitMissingSignatureRejected,
  CommitGenesisStubReturnsRoster
}

CommitSignerBitmapCases == {
  CommitBitmapRejectsOutOfRange,
  CommitSignerSetFromBitmap
}

CommitQuorumStakeCases == {
  CommitPermissionedRequiresCommitQuorum,
  CommitNposRequiresMatchingStakeSnapshot,
  CommitNposStakeQuorumResult
}

CommitBlsOutputCases == {
  CommitRequiresPopForSigner,
  CommitBlsPreimageUsesQcChainMode,
  CommitBlsVerifiesSignerKeysPops,
  CommitReturnsValidatorSet
}

CheckpointAdmissionCases == {
  CheckpointHashVersionV1,
  CheckpointRejectExpiredAtBoundary,
  CheckpointRejectEmptyRoster,
  CheckpointRosterHashBindsValidatorSet,
  CheckpointBitmapLengthCeil
}

CheckpointGenesisSignatureCases == {
  CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap,
  CheckpointMissingSignatureRejected
}

CheckpointSignerBitmapCases == {
  CheckpointBitmapRejectsOutOfRange
}

CheckpointQuorumStakeCases == {
  CheckpointPermissionedRequiresCommitQuorum,
  CheckpointNposRequiresMatchingStakeSnapshot
}

CheckpointRootPreimageCases == {
  CheckpointRootsMatchWhenProvided,
  CheckpointVotePreimageFields
}

CheckpointBlsOutputCases == {
  CheckpointBlsVerifiesSignerKeysPops,
  CheckpointReturnsValidatorSet
}

CheckNonEmptyRoster == 1
CheckHashVersion == 2
CheckRosterHash == 3
CheckBitmapLenCeil == 4
CheckBitmapLenFloor == 5
CheckGenesisFlag == 6
CheckGenesisHeight == 7
CheckGenesisView == 8
CheckEmptySignature == 9
CheckZeroBitmap == 10
RejectMissingSignature == 11
SkipSignatureVerify == 12
IterateBitmap == 13
RejectOutOfRange == 14
SignerSetFromBitmap == 15
DedupSigners == 16
PermissionedCommitQuorum == 17
RequireOneVote == 18
NposRequireStakeSnapshot == 19
NposRequireRosterMatch == 20
NposStakeQuorumTrue == 21
NposStakeQuorumMissingError == 22
RequirePopForSigner == 23
BuildQcPreimage == 24
BuildVotePreimage == 25
UseChainId == 26
UseModeTag == 27
UseQcFields == 28
UseBlockHash == 29
UseHeight == 30
UseViewFallback == 31
UseEpoch == 32
UseChainOrderHash == 33
UseRechainSeq == 34
UseCommitPhase == 35
UseNoHighestQc == 36
UseSignerPublicKeys == 37
UseSignerPops == 38
VerifyBlsAggregate == 39
UseAllRoster == 40
CheckExpiryBoundary == 41
CheckRootsWhenProvided == 42
ReturnRoster == 43
ReturnSigners == 44
ReturnError == 45

Actions == 1..45

GenesisStubChecks ==
  {CheckGenesisFlag, CheckGenesisHeight, CheckGenesisView,
   CheckEmptySignature, CheckZeroBitmap}

PermissionedQuorum ==
  {PermissionedCommitQuorum, ReturnError}

NposMatchingStake ==
  {NposRequireStakeSnapshot, NposRequireRosterMatch, ReturnError}

BlsSignerInputs ==
  {VerifyBlsAggregate, UseSignerPublicKeys, UseSignerPops}

SpecActions(candidate) ==
  CASE candidate = CommitRejectEmptyRoster ->
      {CheckNonEmptyRoster, ReturnError}
    [] candidate = CommitHashVersionV1 ->
      {CheckHashVersion, ReturnError}
    [] candidate = CommitRosterHashBindsValidatorSet ->
      {CheckRosterHash, ReturnError}
    [] candidate = CommitBitmapLengthCeil ->
      {CheckBitmapLenCeil, ReturnError}
    [] candidate = CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap ->
      GenesisStubChecks
    [] candidate = CommitMissingSignatureRejected ->
      {CheckEmptySignature, RejectMissingSignature, ReturnError}
    [] candidate = CommitGenesisStubReturnsRoster ->
      GenesisStubChecks \cup {ReturnRoster, SkipSignatureVerify}
    [] candidate = CommitBitmapRejectsOutOfRange ->
      {IterateBitmap, RejectOutOfRange, ReturnError}
    [] candidate = CommitSignerSetFromBitmap ->
      {IterateBitmap, SignerSetFromBitmap, DedupSigners}
    [] candidate = CommitPermissionedRequiresCommitQuorum ->
      PermissionedQuorum
    [] candidate = CommitNposRequiresMatchingStakeSnapshot ->
      NposMatchingStake
    [] candidate = CommitNposStakeQuorumResult ->
      {NposStakeQuorumTrue, NposStakeQuorumMissingError}
    [] candidate = CommitRequiresPopForSigner ->
      {RequirePopForSigner, ReturnError}
    [] candidate = CommitBlsPreimageUsesQcChainMode ->
      {BuildQcPreimage, UseChainId, UseModeTag, UseQcFields}
    [] candidate = CommitBlsVerifiesSignerKeysPops ->
      BlsSignerInputs
    [] candidate = CommitReturnsValidatorSet ->
      {ReturnRoster}
    [] candidate = CheckpointHashVersionV1 ->
      {CheckHashVersion, ReturnError}
    [] candidate = CheckpointRejectExpiredAtBoundary ->
      {CheckExpiryBoundary, ReturnError}
    [] candidate = CheckpointRejectEmptyRoster ->
      {CheckNonEmptyRoster, ReturnError}
    [] candidate = CheckpointRosterHashBindsValidatorSet ->
      {CheckRosterHash, ReturnError}
    [] candidate = CheckpointBitmapLengthCeil ->
      {CheckBitmapLenCeil, ReturnError}
    [] candidate = CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap ->
      GenesisStubChecks
    [] candidate = CheckpointMissingSignatureRejected ->
      {CheckEmptySignature, RejectMissingSignature, ReturnError}
    [] candidate = CheckpointBitmapRejectsOutOfRange ->
      {IterateBitmap, RejectOutOfRange, ReturnError}
    [] candidate = CheckpointPermissionedRequiresCommitQuorum ->
      PermissionedQuorum
    [] candidate = CheckpointNposRequiresMatchingStakeSnapshot ->
      NposMatchingStake
    [] candidate = CheckpointRootsMatchWhenProvided ->
      {CheckRootsWhenProvided, ReturnError}
    [] candidate = CheckpointVotePreimageFields ->
      {BuildVotePreimage, UseChainId, UseModeTag, UseBlockHash, UseHeight,
       UseViewFallback, UseEpoch, UseChainOrderHash, UseRechainSeq,
       UseCommitPhase, UseNoHighestQc}
    [] candidate = CheckpointBlsVerifiesSignerKeysPops ->
      BlsSignerInputs
    [] candidate = CheckpointReturnsValidatorSet ->
      {ReturnRoster}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = CommitRejectEmptyRoster /\
          Bug = "commit_accepts_empty_roster" ->
      spec \ {CheckNonEmptyRoster, ReturnError}
    [] candidate = CommitHashVersionV1 /\
          Bug = "commit_accepts_wrong_hash_version" ->
      spec \ {CheckHashVersion, ReturnError}
    [] candidate = CommitRosterHashBindsValidatorSet /\
          Bug = "commit_accepts_roster_hash_mismatch" ->
      spec \ {CheckRosterHash, ReturnError}
    [] candidate = CommitBitmapLengthCeil /\
          Bug = "commit_uses_floor_bitmap_len" ->
      (spec \ {CheckBitmapLenCeil}) \cup {CheckBitmapLenFloor}
    [] candidate = CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap /\
          Bug = "commit_unsigned_genesis_without_flag" ->
      spec \ {CheckGenesisFlag}
    [] candidate = CommitMissingSignatureRejected /\
          Bug = "commit_unsigned_non_genesis" ->
      (spec \ {RejectMissingSignature, ReturnError}) \cup {ReturnRoster}
    [] candidate = CommitGenesisStubReturnsRoster /\
          Bug = "commit_genesis_stub_verifies_signature" ->
      (spec \ {SkipSignatureVerify}) \cup {VerifyBlsAggregate}
    [] candidate = CommitBitmapRejectsOutOfRange /\
          Bug = "commit_accepts_bitmap_out_of_range" ->
      spec \ {RejectOutOfRange, ReturnError}
    [] candidate = CommitSignerSetFromBitmap /\
          Bug = "commit_signer_set_uses_all_roster" ->
      (spec \ {SignerSetFromBitmap}) \cup {UseAllRoster}
    [] candidate = CommitPermissionedRequiresCommitQuorum /\
          Bug = "commit_permissioned_uses_one_vote" ->
      (spec \ {PermissionedCommitQuorum}) \cup {RequireOneVote}
    [] candidate = CommitNposRequiresMatchingStakeSnapshot /\
          Bug = "commit_npos_ignores_snapshot_match" ->
      spec \ {NposRequireRosterMatch}
    [] candidate = CommitNposStakeQuorumResult /\
          Bug = "commit_npos_ignores_stake_quorum" ->
      (spec \ {NposStakeQuorumMissingError}) \cup {ReturnRoster}
    [] candidate = CommitRequiresPopForSigner /\
          Bug = "commit_skips_pop_lookup" ->
      spec \ {RequirePopForSigner}
    [] candidate = CommitBlsPreimageUsesQcChainMode /\
          Bug = "commit_preimage_drops_chain_id" ->
      spec \ {UseChainId}
    [] candidate = CommitBlsVerifiesSignerKeysPops /\
          Bug = "commit_bls_verifies_all_roster" ->
      (spec \ {UseSignerPublicKeys}) \cup {UseAllRoster}
    [] candidate = CommitReturnsValidatorSet /\
          Bug = "commit_returns_signers" ->
      (spec \ {ReturnRoster}) \cup {ReturnSigners}
    [] candidate = CheckpointHashVersionV1 /\
          Bug = "checkpoint_accepts_wrong_hash_version" ->
      spec \ {CheckHashVersion, ReturnError}
    [] candidate = CheckpointRejectExpiredAtBoundary /\
          Bug = "checkpoint_ignores_expiry_boundary" ->
      spec \ {CheckExpiryBoundary, ReturnError}
    [] candidate = CheckpointRejectEmptyRoster /\
          Bug = "checkpoint_accepts_empty_roster" ->
      spec \ {CheckNonEmptyRoster, ReturnError}
    [] candidate = CheckpointRosterHashBindsValidatorSet /\
          Bug = "checkpoint_accepts_roster_hash_mismatch" ->
      spec \ {CheckRosterHash, ReturnError}
    [] candidate = CheckpointBitmapLengthCeil /\
          Bug = "checkpoint_uses_floor_bitmap_len" ->
      (spec \ {CheckBitmapLenCeil}) \cup {CheckBitmapLenFloor}
    [] candidate = CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap /\
          Bug = "checkpoint_unsigned_genesis_without_zero_bitmap" ->
      spec \ {CheckZeroBitmap}
    [] candidate = CheckpointMissingSignatureRejected /\
          Bug = "checkpoint_unsigned_non_genesis" ->
      (spec \ {RejectMissingSignature, ReturnError}) \cup {ReturnRoster}
    [] candidate = CheckpointBitmapRejectsOutOfRange /\
          Bug = "checkpoint_accepts_bitmap_out_of_range" ->
      spec \ {RejectOutOfRange, ReturnError}
    [] candidate = CheckpointPermissionedRequiresCommitQuorum /\
          Bug = "checkpoint_permissioned_uses_one_vote" ->
      (spec \ {PermissionedCommitQuorum}) \cup {RequireOneVote}
    [] candidate = CheckpointNposRequiresMatchingStakeSnapshot /\
          Bug = "checkpoint_npos_ignores_snapshot_match" ->
      spec \ {NposRequireRosterMatch}
    [] candidate = CheckpointRootsMatchWhenProvided /\
          Bug = "checkpoint_roots_not_bound" ->
      spec \ {CheckRootsWhenProvided, ReturnError}
    [] candidate = CheckpointVotePreimageFields /\
          Bug = "checkpoint_preimage_drops_chain_order" ->
      spec \ {UseChainOrderHash}
    [] candidate = CheckpointBlsVerifiesSignerKeysPops /\
          Bug = "checkpoint_bls_verifies_all_roster" ->
      (spec \ {UseSignerPublicKeys}) \cup {UseAllRoster}
    [] candidate = CheckpointReturnsValidatorSet /\
          Bug = "checkpoint_returns_signers" ->
      (spec \ {ReturnRoster}) \cup {ReturnSigners}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "commit_accepts_empty_roster",
       "commit_accepts_wrong_hash_version",
       "commit_accepts_roster_hash_mismatch",
       "commit_uses_floor_bitmap_len",
       "commit_unsigned_genesis_without_flag",
       "commit_unsigned_non_genesis",
       "commit_genesis_stub_verifies_signature",
       "commit_accepts_bitmap_out_of_range",
       "commit_signer_set_uses_all_roster",
       "commit_permissioned_uses_one_vote",
       "commit_npos_ignores_snapshot_match",
       "commit_npos_ignores_stake_quorum",
       "commit_skips_pop_lookup",
       "commit_preimage_drops_chain_id",
       "commit_bls_verifies_all_roster",
       "commit_returns_signers",
       "checkpoint_accepts_wrong_hash_version",
       "checkpoint_ignores_expiry_boundary",
       "checkpoint_accepts_empty_roster",
       "checkpoint_accepts_roster_hash_mismatch",
       "checkpoint_uses_floor_bitmap_len",
       "checkpoint_unsigned_genesis_without_zero_bitmap",
       "checkpoint_unsigned_non_genesis",
       "checkpoint_accepts_bitmap_out_of_range",
       "checkpoint_permissioned_uses_one_vote",
       "checkpoint_npos_ignores_snapshot_match",
       "checkpoint_roots_not_bound",
       "checkpoint_preimage_drops_chain_order",
       "checkpoint_bls_verifies_all_roster",
       "checkpoint_returns_signers"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCommitAdmissionExact ==
  \A c \in CommitAdmissionCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCommitGenesisSignatureExact ==
  \A c \in CommitGenesisSignatureCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCommitSignerBitmapExact ==
  \A c \in CommitSignerBitmapCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCommitQuorumStakeExact ==
  \A c \in CommitQuorumStakeCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCommitBlsOutputExact ==
  \A c \in CommitBlsOutputCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointAdmissionExact ==
  \A c \in CheckpointAdmissionCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointGenesisSignatureExact ==
  \A c \in CheckpointGenesisSignatureCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointSignerBitmapExact ==
  \A c \in CheckpointSignerBitmapCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointQuorumStakeExact ==
  \A c \in CheckpointQuorumStakeCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointRootPreimageExact ==
  \A c \in CheckpointRootPreimageCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreCheckpointBlsOutputExact ==
  \A c \in CheckpointBlsOutputCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCoreExactness ==
  /\ RosterValidationCoreCommitAdmissionExact
  /\ RosterValidationCoreCommitGenesisSignatureExact
  /\ RosterValidationCoreCommitSignerBitmapExact
  /\ RosterValidationCoreCommitQuorumStakeExact
  /\ RosterValidationCoreCommitBlsOutputExact
  /\ RosterValidationCoreCheckpointAdmissionExact
  /\ RosterValidationCoreCheckpointGenesisSignatureExact
  /\ RosterValidationCoreCheckpointSignerBitmapExact
  /\ RosterValidationCoreCheckpointQuorumStakeExact
  /\ RosterValidationCoreCheckpointRootPreimageExact
  /\ RosterValidationCoreCheckpointBlsOutputExact

BugCommitAcceptsEmptyRoster ==
  ImplementationActions(CommitRejectEmptyRoster) =
    SpecActions(CommitRejectEmptyRoster)

BugCommitAcceptsWrongHashVersion ==
  ImplementationActions(CommitHashVersionV1) = SpecActions(CommitHashVersionV1)

BugCommitAcceptsRosterHashMismatch ==
  ImplementationActions(CommitRosterHashBindsValidatorSet) =
    SpecActions(CommitRosterHashBindsValidatorSet)

BugCommitUsesFloorBitmapLen ==
  ImplementationActions(CommitBitmapLengthCeil) =
    SpecActions(CommitBitmapLengthCeil)

BugCommitUnsignedGenesisWithoutFlag ==
  ImplementationActions(CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap) =
    SpecActions(CommitGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap)

BugCommitUnsignedNonGenesis ==
  ImplementationActions(CommitMissingSignatureRejected) =
    SpecActions(CommitMissingSignatureRejected)

BugCommitGenesisStubVerifiesSignature ==
  ImplementationActions(CommitGenesisStubReturnsRoster) =
    SpecActions(CommitGenesisStubReturnsRoster)

BugCommitAcceptsBitmapOutOfRange ==
  ImplementationActions(CommitBitmapRejectsOutOfRange) =
    SpecActions(CommitBitmapRejectsOutOfRange)

BugCommitSignerSetUsesAllRoster ==
  ImplementationActions(CommitSignerSetFromBitmap) =
    SpecActions(CommitSignerSetFromBitmap)

BugCommitPermissionedUsesOneVote ==
  ImplementationActions(CommitPermissionedRequiresCommitQuorum) =
    SpecActions(CommitPermissionedRequiresCommitQuorum)

BugCommitNposIgnoresSnapshotMatch ==
  ImplementationActions(CommitNposRequiresMatchingStakeSnapshot) =
    SpecActions(CommitNposRequiresMatchingStakeSnapshot)

BugCommitNposIgnoresStakeQuorum ==
  ImplementationActions(CommitNposStakeQuorumResult) =
    SpecActions(CommitNposStakeQuorumResult)

BugCommitSkipsPopLookup ==
  ImplementationActions(CommitRequiresPopForSigner) =
    SpecActions(CommitRequiresPopForSigner)

BugCommitPreimageDropsChainId ==
  ImplementationActions(CommitBlsPreimageUsesQcChainMode) =
    SpecActions(CommitBlsPreimageUsesQcChainMode)

BugCommitBlsVerifiesAllRoster ==
  ImplementationActions(CommitBlsVerifiesSignerKeysPops) =
    SpecActions(CommitBlsVerifiesSignerKeysPops)

BugCommitReturnsSigners ==
  ImplementationActions(CommitReturnsValidatorSet) =
    SpecActions(CommitReturnsValidatorSet)

BugCheckpointAcceptsWrongHashVersion ==
  ImplementationActions(CheckpointHashVersionV1) =
    SpecActions(CheckpointHashVersionV1)

BugCheckpointIgnoresExpiryBoundary ==
  ImplementationActions(CheckpointRejectExpiredAtBoundary) =
    SpecActions(CheckpointRejectExpiredAtBoundary)

BugCheckpointAcceptsEmptyRoster ==
  ImplementationActions(CheckpointRejectEmptyRoster) =
    SpecActions(CheckpointRejectEmptyRoster)

BugCheckpointAcceptsRosterHashMismatch ==
  ImplementationActions(CheckpointRosterHashBindsValidatorSet) =
    SpecActions(CheckpointRosterHashBindsValidatorSet)

BugCheckpointUsesFloorBitmapLen ==
  ImplementationActions(CheckpointBitmapLengthCeil) =
    SpecActions(CheckpointBitmapLengthCeil)

BugCheckpointUnsignedGenesisWithoutZeroBitmap ==
  ImplementationActions(CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap) =
    SpecActions(CheckpointGenesisStubRequiresFlagHeightViewEmptySigZeroBitmap)

BugCheckpointUnsignedNonGenesis ==
  ImplementationActions(CheckpointMissingSignatureRejected) =
    SpecActions(CheckpointMissingSignatureRejected)

BugCheckpointAcceptsBitmapOutOfRange ==
  ImplementationActions(CheckpointBitmapRejectsOutOfRange) =
    SpecActions(CheckpointBitmapRejectsOutOfRange)

BugCheckpointPermissionedUsesOneVote ==
  ImplementationActions(CheckpointPermissionedRequiresCommitQuorum) =
    SpecActions(CheckpointPermissionedRequiresCommitQuorum)

BugCheckpointNposIgnoresSnapshotMatch ==
  ImplementationActions(CheckpointNposRequiresMatchingStakeSnapshot) =
    SpecActions(CheckpointNposRequiresMatchingStakeSnapshot)

BugCheckpointRootsNotBound ==
  ImplementationActions(CheckpointRootsMatchWhenProvided) =
    SpecActions(CheckpointRootsMatchWhenProvided)

BugCheckpointPreimageDropsChainOrder ==
  ImplementationActions(CheckpointVotePreimageFields) =
    SpecActions(CheckpointVotePreimageFields)

BugCheckpointBlsVerifiesAllRoster ==
  ImplementationActions(CheckpointBlsVerifiesSignerKeysPops) =
    SpecActions(CheckpointBlsVerifiesSignerKeysPops)

BugCheckpointReturnsSigners ==
  ImplementationActions(CheckpointReturnsValidatorSet) =
    SpecActions(CheckpointReturnsValidatorSet)

====
