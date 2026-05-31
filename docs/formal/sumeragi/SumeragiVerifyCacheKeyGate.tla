---- MODULE SumeragiVerifyCacheKeyGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for vote and QC verification cache keys.

This slice captures `QcVerifyKey::from_qc(...)`,
`QcVerifyCacheKey::from_qc(...)`,
`VoteVerifyKey::from_vote_with_signer_public_key(...)`, and
`VoteVerifyCacheKey::from_vote_with_signer_public_key(...)` from
`main_loop.rs`. Hashes, signatures, validators, and public keys are finite
integers. The model preserves the observable identity contract: QC keys bind
phase, height, view, epoch, chain-order hash, rechain sequence, and subject
block hash; QC cache keys additionally bind the validator-set hash/version,
signers bitmap hash, and aggregate signature hash; vote keys bind phase,
height, view, epoch, signer, chain-order hash, rechain sequence, BLS signature
hash, block hash, parent state root, and post state root; and both vote key
constructors intentionally ignore the optional signer public key.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

QcKeyPhase == 1
QcKeyHeight == 2
QcKeyView == 3
QcKeyEpoch == 4
QcKeyChainOrder == 5
QcKeyRechain == 6
QcKeyBlockHash == 7
QcCacheInnerKey == 8
QcCacheValidatorSetHash == 9
QcCacheValidatorSetHashVersion == 10
QcCacheSignersBitmapHash == 11
QcCacheAggregateSignatureHash == 12
VoteKeyPhase == 13
VoteKeyHeight == 14
VoteKeyView == 15
VoteKeyEpoch == 16
VoteKeySigner == 17
VoteKeyChainOrder == 18
VoteKeyRechain == 19
VoteKeySignatureHash == 20
VoteKeyBlockHash == 21
VoteKeyParentStateRoot == 22
VoteKeyPostStateRoot == 23
VoteKeyIgnoresSignerPublicKey == 24
VoteCacheInnerKey == 25
VoteCacheOuterSignatureHash == 26
VoteCacheIgnoresSignerPublicKey == 27

Candidates == 1..27
QcKeyCandidates == 1..7
QcCacheCandidates == 8..12
VoteKeyCandidates == 13..24
VoteCacheCandidates == 25..27

QcPhaseA == 1
QcPhaseB == 2
QcHeightA == 10
QcHeightB == 11
QcViewA == 3
QcViewB == 4
QcEpochA == 5
QcEpochB == 6
QcChainA == 100
QcChainB == 101
QcRechainA == 7
QcRechainB == 8
QcBlockA == 200
QcBlockB == 201
QcValidatorSetHashA == 300
QcValidatorSetHashB == 301
QcValidatorSetHashVersionA == 1
QcValidatorSetHashVersionB == 2
QcSignersBitmapHashA == 400
QcSignersBitmapHashB == 401
QcAggregateSignatureHashA == 500
QcAggregateSignatureHashB == 501

VotePhaseA == 1
VotePhaseB == 2
VoteHeightA == 20
VoteHeightB == 21
VoteViewA == 4
VoteViewB == 5
VoteEpochA == 6
VoteEpochB == 7
VoteSignerA == 2
VoteSignerB == 3
VoteChainA == 600
VoteChainB == 601
VoteRechainA == 9
VoteRechainB == 10
VotePubA == 700
VotePubB == 701
VoteSignatureHashA == 800
VoteSignatureHashB == 801
VoteBlockA == 900
VoteBlockB == 901
VoteParentRootA == 1000
VoteParentRootB == 1001
VotePostRootA == 1100
VotePostRootB == 1101
VoteOuterSignatureHashA == VoteSignatureHashA
VoteOuterSignatureHashB == 1200

\* @type: (Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
SpecQcKey(phase, height, view, epoch, chain, rechain, block) ==
  <<10, phase, height, view, epoch, chain, rechain, block>>

\* @type: (Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplQcKey(phase, height, view, epoch, chain, rechain, block) ==
  CASE Bug = "qc_key_omits_phase" ->
      <<10, height, view, epoch, chain, rechain, block>>
    [] Bug = "qc_key_omits_height" ->
      <<10, phase, view, epoch, chain, rechain, block>>
    [] Bug = "qc_key_omits_view" ->
      <<10, phase, height, epoch, chain, rechain, block>>
    [] Bug = "qc_key_omits_epoch" ->
      <<10, phase, height, view, chain, rechain, block>>
    [] Bug = "qc_key_omits_chain_order" ->
      <<10, phase, height, view, epoch, rechain, block>>
    [] Bug = "qc_key_omits_rechain" ->
      <<10, phase, height, view, epoch, chain, block>>
    [] Bug = "qc_key_omits_block_hash" ->
      <<10, phase, height, view, epoch, chain, rechain>>
    [] OTHER ->
      SpecQcKey(phase, height, view, epoch, chain, rechain, block)

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
SpecQcCacheKey(phase, height, view, epoch, chain, rechain, block,
               validator_set_hash, validator_set_hash_version,
               signers_bitmap_hash, aggregate_signature_hash) ==
  <<20>> \o
  SpecQcKey(phase, height, view, epoch, chain, rechain, block) \o
  <<validator_set_hash, validator_set_hash_version, signers_bitmap_hash,
    aggregate_signature_hash>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplQcCacheKey(phase, height, view, epoch, chain, rechain, block,
               validator_set_hash, validator_set_hash_version,
               signers_bitmap_hash, aggregate_signature_hash) ==
  CASE Bug = "qc_cache_omits_key" ->
      <<20, validator_set_hash, validator_set_hash_version,
        signers_bitmap_hash, aggregate_signature_hash>>
    [] Bug = "qc_cache_omits_validator_set_hash" ->
      <<20>> \o
      ImplQcKey(phase, height, view, epoch, chain, rechain, block) \o
      <<validator_set_hash_version, signers_bitmap_hash,
        aggregate_signature_hash>>
    [] Bug = "qc_cache_omits_validator_set_hash_version" ->
      <<20>> \o
      ImplQcKey(phase, height, view, epoch, chain, rechain, block) \o
      <<validator_set_hash, signers_bitmap_hash, aggregate_signature_hash>>
    [] Bug = "qc_cache_omits_signers_bitmap_hash" ->
      <<20>> \o
      ImplQcKey(phase, height, view, epoch, chain, rechain, block) \o
      <<validator_set_hash, validator_set_hash_version,
        aggregate_signature_hash>>
    [] Bug = "qc_cache_omits_aggregate_signature_hash" ->
      <<20>> \o
      ImplQcKey(phase, height, view, epoch, chain, rechain, block) \o
      <<validator_set_hash, validator_set_hash_version,
        signers_bitmap_hash>>
    [] OTHER ->
      <<20>> \o
      ImplQcKey(phase, height, view, epoch, chain, rechain, block) \o
      <<validator_set_hash, validator_set_hash_version, signers_bitmap_hash,
        aggregate_signature_hash>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
SpecVoteKey(phase, height, view, epoch, signer, chain, rechain,
            signer_public_key, signature_hash, block, parent_root, post_root) ==
  <<30, phase, height, view, epoch, signer, chain, rechain,
    signature_hash, block, parent_root, post_root>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplVoteKey(phase, height, view, epoch, signer, chain, rechain,
            signer_public_key, signature_hash, block, parent_root, post_root) ==
  CASE Bug = "vote_key_omits_phase" ->
      <<30, height, view, epoch, signer, chain, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_height" ->
      <<30, phase, view, epoch, signer, chain, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_view" ->
      <<30, phase, height, epoch, signer, chain, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_epoch" ->
      <<30, phase, height, view, signer, chain, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_signer" ->
      <<30, phase, height, view, epoch, chain, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_chain_order" ->
      <<30, phase, height, view, epoch, signer, rechain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_rechain" ->
      <<30, phase, height, view, epoch, signer, chain,
        signature_hash, block, parent_root, post_root>>
    [] Bug = "vote_key_omits_signature_hash" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        block, parent_root, post_root>>
    [] Bug = "vote_key_omits_block_hash" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        signature_hash, parent_root, post_root>>
    [] Bug = "vote_key_omits_parent_state_root" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        signature_hash, block, post_root>>
    [] Bug = "vote_key_omits_post_state_root" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        signature_hash, block, parent_root>>
    [] Bug = "vote_key_binds_signer_public_key" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        signer_public_key, signature_hash, block, parent_root, post_root>>
    [] OTHER ->
      SpecVoteKey(phase, height, view, epoch, signer, chain, rechain,
                  signer_public_key, signature_hash, block, parent_root,
                  post_root)

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplVoteCacheInnerKey(phase, height, view, epoch, signer, chain, rechain,
                      signer_public_key, signature_hash, block, parent_root,
                      post_root) ==
  CASE Bug = "vote_cache_binds_signer_public_key" ->
      <<30, phase, height, view, epoch, signer, chain, rechain,
        signer_public_key, signature_hash, block, parent_root, post_root>>
    [] OTHER ->
      ImplVoteKey(phase, height, view, epoch, signer, chain, rechain,
                  signer_public_key, signature_hash, block, parent_root,
                  post_root)

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
SpecVoteCacheKey(phase, height, view, epoch, signer, chain, rechain,
                 signer_public_key, signature_hash, block, parent_root,
                 post_root, outer_signature_hash) ==
  <<40>> \o
  SpecVoteKey(phase, height, view, epoch, signer, chain, rechain,
              signer_public_key, signature_hash, block, parent_root,
              post_root) \o
  <<outer_signature_hash>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplVoteCacheKey(phase, height, view, epoch, signer, chain, rechain,
                 signer_public_key, signature_hash, block, parent_root,
                 post_root, outer_signature_hash) ==
  CASE Bug = "vote_cache_omits_key" ->
      <<40, outer_signature_hash>>
    [] Bug = "vote_cache_omits_outer_signature_hash" ->
      <<40>> \o
      ImplVoteCacheInnerKey(phase, height, view, epoch, signer, chain,
                            rechain, signer_public_key, signature_hash,
                            block, parent_root, post_root)
    [] OTHER ->
      <<40>> \o
      ImplVoteCacheInnerKey(phase, height, view, epoch, signer, chain,
                            rechain, signer_public_key, signature_hash,
                            block, parent_root, post_root) \o
      <<outer_signature_hash>>

QcVarPhase(candidate) ==
  IF candidate = QcKeyPhase THEN QcPhaseB ELSE QcPhaseA

QcVarHeight(candidate) ==
  IF candidate = QcKeyHeight THEN QcHeightB ELSE QcHeightA

QcVarView(candidate) ==
  IF candidate = QcKeyView THEN QcViewB ELSE QcViewA

QcVarEpoch(candidate) ==
  IF candidate = QcKeyEpoch THEN QcEpochB ELSE QcEpochA

QcVarChain(candidate) ==
  IF candidate = QcKeyChainOrder THEN QcChainB ELSE QcChainA

QcVarRechain(candidate) ==
  IF candidate = QcKeyRechain THEN QcRechainB ELSE QcRechainA

QcVarBlock(candidate) ==
  IF candidate \in {QcKeyBlockHash, QcCacheInnerKey}
  THEN QcBlockB
  ELSE QcBlockA

QcVarValidatorSetHash(candidate) ==
  IF candidate = QcCacheValidatorSetHash
  THEN QcValidatorSetHashB
  ELSE QcValidatorSetHashA

QcVarValidatorSetHashVersion(candidate) ==
  IF candidate = QcCacheValidatorSetHashVersion
  THEN QcValidatorSetHashVersionB
  ELSE QcValidatorSetHashVersionA

QcVarSignersBitmapHash(candidate) ==
  IF candidate = QcCacheSignersBitmapHash
  THEN QcSignersBitmapHashB
  ELSE QcSignersBitmapHashA

QcVarAggregateSignatureHash(candidate) ==
  IF candidate = QcCacheAggregateSignatureHash
  THEN QcAggregateSignatureHashB
  ELSE QcAggregateSignatureHashA

VoteVarPhase(candidate) ==
  IF candidate = VoteKeyPhase THEN VotePhaseB ELSE VotePhaseA

VoteVarHeight(candidate) ==
  IF candidate = VoteKeyHeight THEN VoteHeightB ELSE VoteHeightA

VoteVarView(candidate) ==
  IF candidate = VoteKeyView THEN VoteViewB ELSE VoteViewA

VoteVarEpoch(candidate) ==
  IF candidate = VoteKeyEpoch THEN VoteEpochB ELSE VoteEpochA

VoteVarSigner(candidate) ==
  IF candidate = VoteKeySigner THEN VoteSignerB ELSE VoteSignerA

VoteVarChain(candidate) ==
  IF candidate = VoteKeyChainOrder THEN VoteChainB ELSE VoteChainA

VoteVarRechain(candidate) ==
  IF candidate = VoteKeyRechain THEN VoteRechainB ELSE VoteRechainA

VoteVarSignerPublicKey(candidate) ==
  IF candidate \in {VoteKeyIgnoresSignerPublicKey,
                    VoteCacheIgnoresSignerPublicKey}
  THEN VotePubB
  ELSE VotePubA

VoteVarSignatureHash(candidate) ==
  IF candidate = VoteKeySignatureHash
  THEN VoteSignatureHashB
  ELSE VoteSignatureHashA

VoteVarBlock(candidate) ==
  IF candidate \in {VoteKeyBlockHash, VoteCacheInnerKey}
  THEN VoteBlockB
  ELSE VoteBlockA

VoteVarParentRoot(candidate) ==
  IF candidate = VoteKeyParentStateRoot
  THEN VoteParentRootB
  ELSE VoteParentRootA

VoteVarPostRoot(candidate) ==
  IF candidate = VoteKeyPostStateRoot
  THEN VotePostRootB
  ELSE VotePostRootA

VoteVarOuterSignatureHash(candidate) ==
  IF candidate = VoteCacheOuterSignatureHash
  THEN VoteOuterSignatureHashB
  ELSE VoteOuterSignatureHashA

SpecQcKeyDistinct(candidate) ==
  SpecQcKey(QcPhaseA, QcHeightA, QcViewA, QcEpochA, QcChainA,
            QcRechainA, QcBlockA) #
  SpecQcKey(QcVarPhase(candidate), QcVarHeight(candidate),
            QcVarView(candidate), QcVarEpoch(candidate),
            QcVarChain(candidate), QcVarRechain(candidate),
            QcVarBlock(candidate))

ImplQcKeyDistinct(candidate) ==
  ImplQcKey(QcPhaseA, QcHeightA, QcViewA, QcEpochA, QcChainA,
            QcRechainA, QcBlockA) #
  ImplQcKey(QcVarPhase(candidate), QcVarHeight(candidate),
            QcVarView(candidate), QcVarEpoch(candidate),
            QcVarChain(candidate), QcVarRechain(candidate),
            QcVarBlock(candidate))

SpecQcCacheDistinct(candidate) ==
  SpecQcCacheKey(QcPhaseA, QcHeightA, QcViewA, QcEpochA, QcChainA,
                 QcRechainA, QcBlockA, QcValidatorSetHashA,
                 QcValidatorSetHashVersionA, QcSignersBitmapHashA,
                 QcAggregateSignatureHashA) #
  SpecQcCacheKey(QcVarPhase(candidate), QcVarHeight(candidate),
                 QcVarView(candidate), QcVarEpoch(candidate),
                 QcVarChain(candidate), QcVarRechain(candidate),
                 QcVarBlock(candidate), QcVarValidatorSetHash(candidate),
                 QcVarValidatorSetHashVersion(candidate),
                 QcVarSignersBitmapHash(candidate),
                 QcVarAggregateSignatureHash(candidate))

ImplQcCacheDistinct(candidate) ==
  ImplQcCacheKey(QcPhaseA, QcHeightA, QcViewA, QcEpochA, QcChainA,
                 QcRechainA, QcBlockA, QcValidatorSetHashA,
                 QcValidatorSetHashVersionA, QcSignersBitmapHashA,
                 QcAggregateSignatureHashA) #
  ImplQcCacheKey(QcVarPhase(candidate), QcVarHeight(candidate),
                 QcVarView(candidate), QcVarEpoch(candidate),
                 QcVarChain(candidate), QcVarRechain(candidate),
                 QcVarBlock(candidate), QcVarValidatorSetHash(candidate),
                 QcVarValidatorSetHashVersion(candidate),
                 QcVarSignersBitmapHash(candidate),
                 QcVarAggregateSignatureHash(candidate))

SpecVoteKeyDistinct(candidate) ==
  SpecVoteKey(VotePhaseA, VoteHeightA, VoteViewA, VoteEpochA,
              VoteSignerA, VoteChainA, VoteRechainA, VotePubA,
              VoteSignatureHashA, VoteBlockA, VoteParentRootA,
              VotePostRootA) #
  SpecVoteKey(VoteVarPhase(candidate), VoteVarHeight(candidate),
              VoteVarView(candidate), VoteVarEpoch(candidate),
              VoteVarSigner(candidate), VoteVarChain(candidate),
              VoteVarRechain(candidate), VoteVarSignerPublicKey(candidate),
              VoteVarSignatureHash(candidate), VoteVarBlock(candidate),
              VoteVarParentRoot(candidate), VoteVarPostRoot(candidate))

ImplVoteKeyDistinct(candidate) ==
  ImplVoteKey(VotePhaseA, VoteHeightA, VoteViewA, VoteEpochA,
              VoteSignerA, VoteChainA, VoteRechainA, VotePubA,
              VoteSignatureHashA, VoteBlockA, VoteParentRootA,
              VotePostRootA) #
  ImplVoteKey(VoteVarPhase(candidate), VoteVarHeight(candidate),
              VoteVarView(candidate), VoteVarEpoch(candidate),
              VoteVarSigner(candidate), VoteVarChain(candidate),
              VoteVarRechain(candidate), VoteVarSignerPublicKey(candidate),
              VoteVarSignatureHash(candidate), VoteVarBlock(candidate),
              VoteVarParentRoot(candidate), VoteVarPostRoot(candidate))

SpecVoteCacheDistinct(candidate) ==
  SpecVoteCacheKey(VotePhaseA, VoteHeightA, VoteViewA, VoteEpochA,
                   VoteSignerA, VoteChainA, VoteRechainA, VotePubA,
                   VoteSignatureHashA, VoteBlockA, VoteParentRootA,
                   VotePostRootA, VoteOuterSignatureHashA) #
  SpecVoteCacheKey(VoteVarPhase(candidate), VoteVarHeight(candidate),
                   VoteVarView(candidate), VoteVarEpoch(candidate),
                   VoteVarSigner(candidate), VoteVarChain(candidate),
                   VoteVarRechain(candidate), VoteVarSignerPublicKey(candidate),
                   VoteVarSignatureHash(candidate), VoteVarBlock(candidate),
                   VoteVarParentRoot(candidate), VoteVarPostRoot(candidate),
                   VoteVarOuterSignatureHash(candidate))

ImplVoteCacheDistinct(candidate) ==
  ImplVoteCacheKey(VotePhaseA, VoteHeightA, VoteViewA, VoteEpochA,
                   VoteSignerA, VoteChainA, VoteRechainA, VotePubA,
                   VoteSignatureHashA, VoteBlockA, VoteParentRootA,
                   VotePostRootA, VoteOuterSignatureHashA) #
  ImplVoteCacheKey(VoteVarPhase(candidate), VoteVarHeight(candidate),
                   VoteVarView(candidate), VoteVarEpoch(candidate),
                   VoteVarSigner(candidate), VoteVarChain(candidate),
                   VoteVarRechain(candidate), VoteVarSignerPublicKey(candidate),
                   VoteVarSignatureHash(candidate), VoteVarBlock(candidate),
                   VoteVarParentRoot(candidate), VoteVarPostRoot(candidate),
                   VoteVarOuterSignatureHash(candidate))

SpecDistinct(candidate) ==
  CASE candidate \in QcKeyCandidates -> SpecQcKeyDistinct(candidate)
    [] candidate \in QcCacheCandidates -> SpecQcCacheDistinct(candidate)
    [] candidate \in VoteKeyCandidates -> SpecVoteKeyDistinct(candidate)
    [] candidate \in VoteCacheCandidates -> SpecVoteCacheDistinct(candidate)
    [] OTHER -> FALSE

ImplDistinct(candidate) ==
  CASE candidate \in QcKeyCandidates -> ImplQcKeyDistinct(candidate)
    [] candidate \in QcCacheCandidates -> ImplQcCacheDistinct(candidate)
    [] candidate \in VoteKeyCandidates -> ImplVoteKeyDistinct(candidate)
    [] candidate \in VoteCacheCandidates -> ImplVoteCacheDistinct(candidate)
    [] OTHER -> FALSE

Init == checked \in Candidates

Next == UNCHANGED vars

TypeInvariant == checked \in Candidates

Safety ==
  \A candidate \in Candidates:
    ImplDistinct(candidate) = SpecDistinct(candidate)

BugQcKeyOmitsPhase ==
  ImplDistinct(QcKeyPhase) = SpecDistinct(QcKeyPhase)

BugQcKeyOmitsHeight ==
  ImplDistinct(QcKeyHeight) = SpecDistinct(QcKeyHeight)

BugQcKeyOmitsView ==
  ImplDistinct(QcKeyView) = SpecDistinct(QcKeyView)

BugQcKeyOmitsEpoch ==
  ImplDistinct(QcKeyEpoch) = SpecDistinct(QcKeyEpoch)

BugQcKeyOmitsChainOrder ==
  ImplDistinct(QcKeyChainOrder) = SpecDistinct(QcKeyChainOrder)

BugQcKeyOmitsRechain ==
  ImplDistinct(QcKeyRechain) = SpecDistinct(QcKeyRechain)

BugQcKeyOmitsBlockHash ==
  ImplDistinct(QcKeyBlockHash) = SpecDistinct(QcKeyBlockHash)

BugQcCacheOmitsKey ==
  ImplDistinct(QcCacheInnerKey) = SpecDistinct(QcCacheInnerKey)

BugQcCacheOmitsValidatorSetHash ==
  ImplDistinct(QcCacheValidatorSetHash) = SpecDistinct(QcCacheValidatorSetHash)

BugQcCacheOmitsValidatorSetHashVersion ==
  ImplDistinct(QcCacheValidatorSetHashVersion) =
    SpecDistinct(QcCacheValidatorSetHashVersion)

BugQcCacheOmitsSignersBitmapHash ==
  ImplDistinct(QcCacheSignersBitmapHash) =
    SpecDistinct(QcCacheSignersBitmapHash)

BugQcCacheOmitsAggregateSignatureHash ==
  ImplDistinct(QcCacheAggregateSignatureHash) =
    SpecDistinct(QcCacheAggregateSignatureHash)

BugVoteKeyOmitsPhase ==
  ImplDistinct(VoteKeyPhase) = SpecDistinct(VoteKeyPhase)

BugVoteKeyOmitsHeight ==
  ImplDistinct(VoteKeyHeight) = SpecDistinct(VoteKeyHeight)

BugVoteKeyOmitsView ==
  ImplDistinct(VoteKeyView) = SpecDistinct(VoteKeyView)

BugVoteKeyOmitsEpoch ==
  ImplDistinct(VoteKeyEpoch) = SpecDistinct(VoteKeyEpoch)

BugVoteKeyOmitsSigner ==
  ImplDistinct(VoteKeySigner) = SpecDistinct(VoteKeySigner)

BugVoteKeyOmitsChainOrder ==
  ImplDistinct(VoteKeyChainOrder) = SpecDistinct(VoteKeyChainOrder)

BugVoteKeyOmitsRechain ==
  ImplDistinct(VoteKeyRechain) = SpecDistinct(VoteKeyRechain)

BugVoteKeyOmitsSignatureHash ==
  ImplDistinct(VoteKeySignatureHash) = SpecDistinct(VoteKeySignatureHash)

BugVoteKeyOmitsBlockHash ==
  ImplDistinct(VoteKeyBlockHash) = SpecDistinct(VoteKeyBlockHash)

BugVoteKeyOmitsParentStateRoot ==
  ImplDistinct(VoteKeyParentStateRoot) =
    SpecDistinct(VoteKeyParentStateRoot)

BugVoteKeyOmitsPostStateRoot ==
  ImplDistinct(VoteKeyPostStateRoot) = SpecDistinct(VoteKeyPostStateRoot)

BugVoteKeyBindsSignerPublicKey ==
  ImplDistinct(VoteKeyIgnoresSignerPublicKey) =
    SpecDistinct(VoteKeyIgnoresSignerPublicKey)

BugVoteCacheOmitsKey ==
  ImplDistinct(VoteCacheInnerKey) = SpecDistinct(VoteCacheInnerKey)

BugVoteCacheOmitsOuterSignatureHash ==
  ImplDistinct(VoteCacheOuterSignatureHash) =
    SpecDistinct(VoteCacheOuterSignatureHash)

BugVoteCacheBindsSignerPublicKey ==
  ImplDistinct(VoteCacheIgnoresSignerPublicKey) =
    SpecDistinct(VoteCacheIgnoresSignerPublicKey)

====
