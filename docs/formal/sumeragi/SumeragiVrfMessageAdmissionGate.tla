---- MODULE SumeragiVrfMessageAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi VRF commit/reveal message admission.

`handle_vrf_commit(...)` and `handle_vrf_reveal(...)` first require a supported
consensus mode, active topology, valid signer index, non-empty BLS signature,
and signature verification over the canonical VRF preimage. Accepted messages
then pass through `EpochManager::try_note_commit_at_height(...)` or
`EpochManager::try_note_reveal_at_height(...)`, which enforces epoch, roster,
window, duplicate, and commitment/reveal consistency before staging a VRF epoch
snapshot. External accepted messages are rebroadcast; network-originated
messages are not. Local VRF state changes only when the accepted signer is the
local validator. Late reveals are accepted as penalty-clearing observations but
must not refresh the current PRF context.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Bool;
  late,
  \* @type: Bool;
  staged,
  \* @type: Bool;
  broadcast,
  \* @type: Bool;
  local_updated,
  \* @type: Bool;
  prf_refreshed

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, accepted, late, staged, broadcast, local_updated, prf_refreshed>>

Cases == {
  "valid_commit_external",
  "valid_commit_network",
  "duplicate_commit_same",
  "valid_reveal_external",
  "valid_reveal_network",
  "duplicate_reveal_same",
  "valid_late_reveal_external",
  "valid_late_reveal_network",
  "unsupported_mode",
  "missing_manager",
  "signer_oob",
  "missing_signature",
  "bad_signature",
  "epoch_mismatch",
  "unknown_signer",
  "commit_out_of_window",
  "commit_rewrite",
  "reveal_in_commit_window",
  "reveal_without_commit",
  "reveal_commit_mismatch",
  "reveal_rewrite"
}

CommitCases == {
  "valid_commit_external",
  "valid_commit_network",
  "duplicate_commit_same",
  "unsupported_mode",
  "missing_manager",
  "signer_oob",
  "missing_signature",
  "bad_signature",
  "epoch_mismatch",
  "unknown_signer",
  "commit_out_of_window",
  "commit_rewrite"
}

RevealCases == (Cases \ CommitCases) \ {
  "unsupported_mode",
  "missing_manager",
  "signer_oob",
  "missing_signature",
  "bad_signature",
  "epoch_mismatch",
  "unknown_signer"
}

ValidCases == {
  "valid_commit_external",
  "valid_commit_network",
  "duplicate_commit_same",
  "valid_reveal_external",
  "valid_reveal_network",
  "duplicate_reveal_same",
  "valid_late_reveal_external",
  "valid_late_reveal_network"
}

LateRevealCases == {"valid_late_reveal_external", "valid_late_reveal_network"}

NormalRevealCases == {
  "valid_reveal_external",
  "valid_reveal_network",
  "duplicate_reveal_same",
  "reveal_without_commit",
  "reveal_commit_mismatch",
  "reveal_rewrite"
}

InvalidCases == Cases \ ValidCases

SenderExternal(c) ==
  c \in {
    "valid_commit_external",
    "duplicate_commit_same",
    "valid_reveal_external",
    "duplicate_reveal_same",
    "valid_late_reveal_external"
  }

SignerIsLocal(c) ==
  c \in {
    "valid_commit_external",
    "duplicate_commit_same",
    "valid_reveal_external",
    "duplicate_reveal_same",
    "valid_late_reveal_external"
  }

ValidMode(c) == c # "unsupported_mode"

ManagerAvailable(c) == c # "missing_manager"

SignerInTopology(c) == c # "signer_oob"

SignaturePresent(c) == c # "missing_signature"

SignatureValid(c) == c # "bad_signature"

EpochMatches(c) == c # "epoch_mismatch"

KnownSigner(c) == c # "unknown_signer"

CommitWindowOk(c) == c # "commit_out_of_window"

CommitNoRewrite(c) == c # "commit_rewrite"

RevealTimingOk(c) == c \in NormalRevealCases \union LateRevealCases

RevealCommitExists(c) == c # "reveal_without_commit"

RevealMatchesCommit(c) == c # "reveal_commit_mismatch"

RevealNoRewrite(c) == c # "reveal_rewrite"

SpecSignatureOk(c) ==
  /\ SignerInTopology(c)
  /\ SignaturePresent(c)
  /\ SignatureValid(c)

ActualSignatureOk(c) ==
  /\ (SignerInTopology(c) \/ Bug = "ignore_signer_out_of_topology")
  /\ (SignaturePresent(c) \/ Bug = "accept_missing_signature")
  /\ (SignatureValid(c) \/ Bug = "accept_bad_signature")

SpecCommitNoteOk(c) ==
  /\ EpochMatches(c)
  /\ KnownSigner(c)
  /\ CommitWindowOk(c)
  /\ CommitNoRewrite(c)

ActualCommitNoteOk(c) ==
  /\ (EpochMatches(c) \/ Bug = "accept_epoch_mismatch")
  /\ (KnownSigner(c) \/ Bug = "accept_unknown_signer")
  /\ (CommitWindowOk(c) \/ Bug = "accept_commit_out_of_window")
  /\ (CommitNoRewrite(c) \/ Bug = "accept_commitment_rewrite")

SpecRevealNoteOk(c) ==
  /\ EpochMatches(c)
  /\ KnownSigner(c)
  /\ RevealTimingOk(c)
  /\ RevealCommitExists(c)
  /\ RevealMatchesCommit(c)
  /\ RevealNoRewrite(c)

ActualRevealNoteOk(c) ==
  /\ (EpochMatches(c) \/ Bug = "accept_epoch_mismatch")
  /\ (KnownSigner(c) \/ Bug = "accept_unknown_signer")
  /\ (RevealTimingOk(c) \/ Bug = "accept_reveal_in_commit_window")
  /\ (RevealCommitExists(c) \/ Bug = "accept_reveal_without_commit")
  /\ (RevealMatchesCommit(c) \/ Bug = "accept_reveal_commit_mismatch")
  /\ (RevealNoRewrite(c) \/ Bug = "accept_reveal_rewrite")

SpecNoteOk(c) ==
  IF c \in CommitCases THEN SpecCommitNoteOk(c) ELSE SpecRevealNoteOk(c)

ActualNoteOk(c) ==
  IF c \in CommitCases THEN ActualCommitNoteOk(c) ELSE ActualRevealNoteOk(c)

SpecAccept(c) ==
  /\ ValidMode(c)
  /\ ManagerAvailable(c)
  /\ SpecSignatureOk(c)
  /\ SpecNoteOk(c)

ActualAccept(c) ==
  /\ (ValidMode(c) \/ Bug = "accept_unsupported_mode")
  /\ (ManagerAvailable(c) \/ Bug = "accept_missing_manager")
  /\ ActualSignatureOk(c)
  /\ ActualNoteOk(c)

SpecLate(c) ==
  SpecAccept(c) /\ c \in LateRevealCases

ActualLate(c) ==
  ActualAccept(c) /\ c \in LateRevealCases

SpecStaged(c) == SpecAccept(c)

ActualStaged(c) ==
  IF ActualAccept(c) THEN Bug # "skip_stage_on_accept" ELSE FALSE

SpecBroadcast(c) == SpecAccept(c) /\ SenderExternal(c)

ActualBroadcast(c) ==
  IF ActualAccept(c)
  THEN /\ (SenderExternal(c) \/ Bug = "rebroadcast_network_origin")
       /\ Bug # "skip_external_broadcast"
  ELSE Bug = "broadcast_on_reject"

SpecLocalUpdated(c) == SpecAccept(c) /\ SignerIsLocal(c)

ActualLocalUpdated(c) ==
  IF ActualAccept(c)
  THEN /\ (SignerIsLocal(c) \/ Bug = "update_local_for_remote_signer")
       /\ Bug # "skip_local_update"
  ELSE FALSE

SpecPrfRefreshed(c) == SpecAccept(c) /\ c \in NormalRevealCases

ActualPrfRefreshed(c) ==
  IF ActualAccept(c)
  THEN
    CASE c \in NormalRevealCases -> Bug # "skip_prf_refresh_on_reveal"
      [] c \in LateRevealCases -> Bug = "refresh_prf_on_late_reveal"
      [] OTHER -> FALSE
  ELSE FALSE

BugModes == {
  "none",
  "accept_unsupported_mode",
  "accept_missing_manager",
  "ignore_signer_out_of_topology",
  "accept_missing_signature",
  "accept_bad_signature",
  "accept_epoch_mismatch",
  "accept_unknown_signer",
  "accept_commit_out_of_window",
  "accept_commitment_rewrite",
  "accept_reveal_in_commit_window",
  "accept_reveal_without_commit",
  "accept_reveal_commit_mismatch",
  "accept_reveal_rewrite",
  "broadcast_on_reject",
  "rebroadcast_network_origin",
  "skip_external_broadcast",
  "skip_stage_on_accept",
  "update_local_for_remote_signer",
  "skip_local_update",
  "skip_prf_refresh_on_reveal",
  "refresh_prf_on_late_reveal"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ accepted \in BOOLEAN
  /\ late \in BOOLEAN
  /\ staged \in BOOLEAN
  /\ broadcast \in BOOLEAN
  /\ local_updated \in BOOLEAN
  /\ prf_refreshed \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ accepted = FALSE
  /\ late = FALSE
  /\ staged = FALSE
  /\ broadcast = FALSE
  /\ local_updated = FALSE
  /\ prf_refreshed = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ accepted' = ActualAccept(c)
  /\ late' = ActualLate(c)
  /\ staged' = ActualStaged(c)
  /\ broadcast' = ActualBroadcast(c)
  /\ local_updated' = ActualLocalUpdated(c)
  /\ prf_refreshed' = ActualPrfRefreshed(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

AcceptMatchesSpec ==
  candidate = "none" \/ accepted = SpecAccept(candidate)

LateMatchesSpec ==
  candidate = "none" \/ late = SpecLate(candidate)

StagingMatchesSpec ==
  candidate = "none" \/ staged = SpecStaged(candidate)

BroadcastMatchesSpec ==
  candidate = "none" \/ broadcast = SpecBroadcast(candidate)

LocalUpdateMatchesSpec ==
  candidate = "none" \/ local_updated = SpecLocalUpdated(candidate)

PrfRefreshMatchesSpec ==
  candidate = "none" \/ prf_refreshed = SpecPrfRefreshed(candidate)

ValidCasesAccepted ==
  candidate \in ValidCases => accepted

InvalidCasesRejected ==
  candidate \in InvalidCases => ~accepted

RejectedHasNoSideEffects ==
  candidate \in InvalidCases =>
    /\ ~staged
    /\ ~broadcast
    /\ ~local_updated
    /\ ~prf_refreshed

NetworkOriginDoesNotBroadcast ==
  candidate # "none" /\ ~SenderExternal(candidate) => ~broadcast

ExternalAcceptedBroadcasts ==
  candidate # "none" /\ SpecAccept(candidate) /\ SenderExternal(candidate) => broadcast

LocalStateOnlyForLocalSigner ==
  candidate # "none" /\ ~SignerIsLocal(candidate) => ~local_updated

LateRevealDoesNotRefreshPrf ==
  candidate \in LateRevealCases => ~prf_refreshed

NormalRevealRefreshesPrf ==
  candidate \in NormalRevealCases /\ SpecAccept(candidate) => prf_refreshed

CommitDoesNotRefreshPrf ==
  candidate \in CommitCases => ~prf_refreshed

Safety ==
  /\ AcceptMatchesSpec
  /\ LateMatchesSpec
  /\ StagingMatchesSpec
  /\ BroadcastMatchesSpec
  /\ LocalUpdateMatchesSpec
  /\ PrfRefreshMatchesSpec
  /\ ValidCasesAccepted
  /\ InvalidCasesRejected
  /\ RejectedHasNoSideEffects
  /\ NetworkOriginDoesNotBroadcast
  /\ ExternalAcceptedBroadcasts
  /\ LocalStateOnlyForLocalSigner
  /\ LateRevealDoesNotRefreshPrf
  /\ NormalRevealRefreshesPrf
  /\ CommitDoesNotRefreshPrf

====
