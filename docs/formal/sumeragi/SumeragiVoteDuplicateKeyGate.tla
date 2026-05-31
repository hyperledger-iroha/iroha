---- MODULE SumeragiVoteDuplicateKeyGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for vote duplicate-key handling.

This slice captures `vote_key(...)`, `vote_identity_key(...)`,
`raw_vote_key_from_identity_key(...)`, `vote_duplicate(...)`, and
`same_recorded_vote_is_duplicate(...)` from `main_loop/votes.rs`. It abstracts
concrete hashes and public keys as finite integers while preserving the
deterministic contract: the vote log is keyed only by phase, height, view,
epoch, signer, chain-order hash, and rechain sequence; block hash is checked
after raw-key lookup; non-NEW_VIEW duplicates ignore highest-QC references;
NEW_VIEW duplicates require identical highest-QC references; and identity keys
bind the signer public key while raw-key projection strips it back out.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Prepare == 1
Commit == 2
NewView == 3

NoQc == 0
QcA == 1
QcB == 2

BasePhase == Prepare
BaseHeight == 10
BaseView == 2
BaseEpoch == 1
BaseSigner == 4
BaseChain == 100
BaseRechain == 3
BaseBlock == 50
BaseHighest == NoQc
ExistingPub == 7
IncomingPub == 9

PresentPrepareSameHash == 1
PresentPrepareDifferentHash == 2
PresentPrepareSameHashDifferentHighestQc == 3
PresentCommitSameHash == 4
PresentNewViewSameHashSameQc == 5
PresentNewViewSameHashDifferentQc == 6
PresentNewViewSameHashBothNoQc == 7
PresentNewViewExistingNoQcVoteSome == 8
PresentNewViewExistingSomeVoteNoQc == 9
PresentNewViewDifferentHashSameQc == 10
MissingLogSameKey == 11
PhaseMismatchSameHash == 12
HeightMismatchSameHash == 13
ViewMismatchSameHash == 14
EpochMismatchSameHash == 15
SignerMismatchSameHash == 16
ChainOrderMismatchSameHash == 17
RechainMismatchSameHash == 18
RawProjectionStripsPublicKey == 19
IdentityKeyBindsPublicKey == 20

Candidates == 1..20
VoteDuplicateCandidates == 1..18

NewViewCases ==
  {PresentNewViewSameHashSameQc,
   PresentNewViewSameHashDifferentQc,
   PresentNewViewSameHashBothNoQc,
   PresentNewViewExistingNoQcVoteSome,
   PresentNewViewExistingSomeVoteNoQc,
   PresentNewViewDifferentHashSameQc}

LogPresent(candidate) == candidate # MissingLogSameKey

ExistingPhase(candidate) ==
  CASE candidate = PresentCommitSameHash -> Commit
    [] candidate \in NewViewCases -> NewView
    [] OTHER -> BasePhase

IncomingPhase(candidate) ==
  CASE candidate = PresentCommitSameHash -> Commit
    [] candidate \in NewViewCases -> NewView
    [] candidate = PhaseMismatchSameHash -> Commit
    [] OTHER -> BasePhase

ExistingHeight(candidate) == BaseHeight
IncomingHeight(candidate) ==
  IF candidate = HeightMismatchSameHash THEN 11 ELSE BaseHeight

ExistingView(candidate) == BaseView
IncomingView(candidate) ==
  IF candidate = ViewMismatchSameHash THEN 3 ELSE BaseView

ExistingEpoch(candidate) == BaseEpoch
IncomingEpoch(candidate) ==
  IF candidate = EpochMismatchSameHash THEN 2 ELSE BaseEpoch

ExistingSigner(candidate) == BaseSigner
IncomingSigner(candidate) ==
  IF candidate = SignerMismatchSameHash THEN 5 ELSE BaseSigner

ExistingChain(candidate) == BaseChain
IncomingChain(candidate) ==
  IF candidate = ChainOrderMismatchSameHash THEN 101 ELSE BaseChain

ExistingRechain(candidate) == BaseRechain
IncomingRechain(candidate) ==
  IF candidate = RechainMismatchSameHash THEN 4 ELSE BaseRechain

ExistingBlock(candidate) == BaseBlock
IncomingBlock(candidate) ==
  IF candidate \in {PresentPrepareDifferentHash,
                    PresentNewViewDifferentHashSameQc}
  THEN 51
  ELSE BaseBlock

ExistingHighest(candidate) ==
  CASE candidate \in {PresentNewViewSameHashSameQc,
                      PresentNewViewSameHashDifferentQc,
                      PresentNewViewDifferentHashSameQc,
                      PresentNewViewExistingSomeVoteNoQc,
                      PresentPrepareSameHashDifferentHighestQc} -> QcA
    [] OTHER -> BaseHighest

IncomingHighest(candidate) ==
  CASE candidate \in {PresentNewViewSameHashSameQc,
                      PresentNewViewDifferentHashSameQc,
                      PresentNewViewExistingNoQcVoteSome} -> QcA
    [] candidate \in {PresentNewViewSameHashDifferentQc,
                      PresentPrepareSameHashDifferentHighestQc} -> QcB
    [] OTHER -> BaseHighest

\* @type: (Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
SpecVoteKey(phase, height, view, epoch, signer, chain, rechain) ==
  <<phase, height, view, epoch, signer, chain, rechain>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int) => Seq(Int);
ImplVoteKey(phase, height, view, epoch, signer, chain, rechain, pub) ==
  CASE Bug = "key_includes_public_key" ->
      <<phase, height, view, epoch, signer, chain, rechain, pub>>
    [] Bug = "key_omits_phase" ->
      <<height, view, epoch, signer, chain, rechain>>
    [] Bug = "key_omits_height" ->
      <<phase, view, epoch, signer, chain, rechain>>
    [] Bug = "key_omits_view" ->
      <<phase, height, epoch, signer, chain, rechain>>
    [] Bug = "key_omits_epoch" ->
      <<phase, height, view, signer, chain, rechain>>
    [] Bug = "key_omits_signer" ->
      <<phase, height, view, epoch, chain, rechain>>
    [] Bug = "key_omits_chain_order" ->
      <<phase, height, view, epoch, signer, rechain>>
    [] Bug = "key_omits_rechain" ->
      <<phase, height, view, epoch, signer, chain>>
    [] OTHER -> SpecVoteKey(phase, height, view, epoch, signer, chain, rechain)

\* @type: (Int) => Seq(Int);
SpecExistingKey(candidate) ==
  SpecVoteKey(
    ExistingPhase(candidate),
    ExistingHeight(candidate),
    ExistingView(candidate),
    ExistingEpoch(candidate),
    ExistingSigner(candidate),
    ExistingChain(candidate),
    ExistingRechain(candidate))

\* @type: (Int) => Seq(Int);
SpecIncomingKey(candidate) ==
  SpecVoteKey(
    IncomingPhase(candidate),
    IncomingHeight(candidate),
    IncomingView(candidate),
    IncomingEpoch(candidate),
    IncomingSigner(candidate),
    IncomingChain(candidate),
    IncomingRechain(candidate))

\* @type: (Int) => Seq(Int);
ImplExistingKey(candidate) ==
  ImplVoteKey(
    ExistingPhase(candidate),
    ExistingHeight(candidate),
    ExistingView(candidate),
    ExistingEpoch(candidate),
    ExistingSigner(candidate),
    ExistingChain(candidate),
    ExistingRechain(candidate),
    ExistingPub)

\* @type: (Int) => Seq(Int);
ImplIncomingKey(candidate) ==
  ImplVoteKey(
    IncomingPhase(candidate),
    IncomingHeight(candidate),
    IncomingView(candidate),
    IncomingEpoch(candidate),
    IncomingSigner(candidate),
    IncomingChain(candidate),
    IncomingRechain(candidate),
    IncomingPub)

\* @type: (Int) => Seq(Int);
SpecIdentityKey(pub) ==
  <<BasePhase, BaseHeight, BaseView, BaseEpoch, BaseSigner, BaseChain,
    BaseRechain, pub>>

\* @type: (Int) => Seq(Int);
ImplIdentityKey(pub) ==
  IF Bug = "identity_key_omits_public_key"
  THEN SpecVoteKey(BasePhase, BaseHeight, BaseView, BaseEpoch, BaseSigner,
                   BaseChain, BaseRechain)
  ELSE SpecIdentityKey(pub)

\* @type: Seq(Int);
SpecRawFromIdentity == SpecVoteKey(BasePhase, BaseHeight, BaseView, BaseEpoch,
                                   BaseSigner, BaseChain, BaseRechain)

\* @type: Seq(Int);
ImplRawFromIdentity ==
  IF Bug = "raw_projection_keeps_public_key"
  THEN ImplIdentityKey(IncomingPub)
  ELSE SpecRawFromIdentity

SpecSameRecordedVoteDuplicate(candidate) ==
  /\ ExistingBlock(candidate) = IncomingBlock(candidate)
  /\ IF IncomingPhase(candidate) = NewView
     THEN ExistingHighest(candidate) = IncomingHighest(candidate)
     ELSE TRUE

ImplSameRecordedVoteDuplicate(candidate) ==
  CASE Bug = "skip_block_hash_compare" ->
      IF IncomingPhase(candidate) = NewView
      THEN ExistingHighest(candidate) = IncomingHighest(candidate)
      ELSE TRUE
    [] Bug = "new_view_ignores_highest_qc" ->
      ExistingBlock(candidate) = IncomingBlock(candidate)
    [] Bug = "non_new_view_checks_highest_qc" ->
      /\ ExistingBlock(candidate) = IncomingBlock(candidate)
      /\ ExistingHighest(candidate) = IncomingHighest(candidate)
    [] Bug = "new_view_absent_highest_wildcard" ->
      /\ ExistingBlock(candidate) = IncomingBlock(candidate)
      /\ IF IncomingPhase(candidate) = NewView
         THEN ExistingHighest(candidate) = IncomingHighest(candidate)
              \/ ExistingHighest(candidate) = NoQc
              \/ IncomingHighest(candidate) = NoQc
         ELSE TRUE
    [] OTHER -> SpecSameRecordedVoteDuplicate(candidate)

SpecVoteDuplicate(candidate) ==
  /\ LogPresent(candidate)
  /\ SpecExistingKey(candidate) = SpecIncomingKey(candidate)
  /\ SpecSameRecordedVoteDuplicate(candidate)

ImplVoteDuplicate(candidate) ==
  IF Bug = "missing_log_accepts" /\ candidate = MissingLogSameKey
  THEN ImplSameRecordedVoteDuplicate(candidate)
  ELSE
    /\ LogPresent(candidate)
    /\ ImplExistingKey(candidate) = ImplIncomingKey(candidate)
    /\ ImplSameRecordedVoteDuplicate(candidate)

SpecProperty(candidate) ==
  CASE candidate \in VoteDuplicateCandidates ->
      SpecVoteDuplicate(candidate)
    [] candidate = RawProjectionStripsPublicKey ->
      SpecRawFromIdentity = SpecVoteKey(BasePhase, BaseHeight, BaseView,
                                        BaseEpoch, BaseSigner, BaseChain,
                                        BaseRechain)
    [] candidate = IdentityKeyBindsPublicKey ->
      SpecIdentityKey(ExistingPub) # SpecIdentityKey(IncomingPub)
    [] OTHER -> FALSE

ImplProperty(candidate) ==
  CASE candidate \in VoteDuplicateCandidates ->
      ImplVoteDuplicate(candidate)
    [] candidate = RawProjectionStripsPublicKey ->
      ImplRawFromIdentity = SpecVoteKey(BasePhase, BaseHeight, BaseView,
                                        BaseEpoch, BaseSigner, BaseChain,
                                        BaseRechain)
    [] candidate = IdentityKeyBindsPublicKey ->
      ImplIdentityKey(ExistingPub) # ImplIdentityKey(IncomingPub)
    [] OTHER -> FALSE

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "missing_log_accepts",
       "key_includes_public_key",
       "key_omits_phase",
       "key_omits_height",
       "key_omits_view",
       "key_omits_epoch",
       "key_omits_signer",
       "key_omits_chain_order",
       "key_omits_rechain",
       "skip_block_hash_compare",
       "new_view_ignores_highest_qc",
       "non_new_view_checks_highest_qc",
       "new_view_absent_highest_wildcard",
       "raw_projection_keeps_public_key",
       "identity_key_omits_public_key"
     }
  /\ checked = 0
  /\ \A c \in Candidates: SpecProperty(c) \in BOOLEAN
  /\ \A c \in Candidates: ImplProperty(c) \in BOOLEAN

Safety ==
  \A c \in Candidates:
    ImplProperty(c) = SpecProperty(c)

BugMissingLogAccepts ==
  ImplProperty(MissingLogSameKey) = SpecProperty(MissingLogSameKey)

BugKeyIncludesPublicKey ==
  ImplProperty(PresentPrepareSameHash) = SpecProperty(PresentPrepareSameHash)

BugKeyOmitsPhase ==
  ImplProperty(PhaseMismatchSameHash) = SpecProperty(PhaseMismatchSameHash)

BugKeyOmitsHeight ==
  ImplProperty(HeightMismatchSameHash) = SpecProperty(HeightMismatchSameHash)

BugKeyOmitsView ==
  ImplProperty(ViewMismatchSameHash) = SpecProperty(ViewMismatchSameHash)

BugKeyOmitsEpoch ==
  ImplProperty(EpochMismatchSameHash) = SpecProperty(EpochMismatchSameHash)

BugKeyOmitsSigner ==
  ImplProperty(SignerMismatchSameHash) = SpecProperty(SignerMismatchSameHash)

BugKeyOmitsChainOrder ==
  ImplProperty(ChainOrderMismatchSameHash) =
    SpecProperty(ChainOrderMismatchSameHash)

BugKeyOmitsRechain ==
  ImplProperty(RechainMismatchSameHash) =
    SpecProperty(RechainMismatchSameHash)

BugSkipBlockHashCompare ==
  ImplProperty(PresentPrepareDifferentHash) =
    SpecProperty(PresentPrepareDifferentHash)

BugNewViewIgnoresHighestQc ==
  ImplProperty(PresentNewViewSameHashDifferentQc) =
    SpecProperty(PresentNewViewSameHashDifferentQc)

BugNonNewViewChecksHighestQc ==
  ImplProperty(PresentPrepareSameHashDifferentHighestQc) =
    SpecProperty(PresentPrepareSameHashDifferentHighestQc)

BugNewViewAbsentHighestWildcard ==
  ImplProperty(PresentNewViewExistingNoQcVoteSome) =
    SpecProperty(PresentNewViewExistingNoQcVoteSome)

BugRawProjectionKeepsPublicKey ==
  ImplProperty(RawProjectionStripsPublicKey) =
    SpecProperty(RawProjectionStripsPublicKey)

BugIdentityKeyOmitsPublicKey ==
  ImplProperty(IdentityKeyBindsPublicKey) =
    SpecProperty(IdentityKeyBindsPublicKey)

====
