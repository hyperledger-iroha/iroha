---- MODULE SumeragiV2IngressClassMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded mutation witness for the two production ingress boundaries.

The outer FairV2Ingress classification reserves every Commit vote, QC, TC,
TimeoutVote, payload chunk, certified-body request/response, and
Commit-certificate request/response as Progress.  At the serialized runtime
boundary only QCs, TCs, TimeoutVote, and an authenticated exact historical
locked Commit use Progress, together with an authenticated exact current
Prepare for a locally bound unchanged-lock reproposal.  Ordinary proposal/vote
traffic is Normal, while chunk and recovery transport payloads bypass
that runtime queue.
The repaired mode checks both exact partitions.  Each mutant removes or
promotes one source-exact case while preserving the outer/runtime distinction.
***************************************************************************)

CONSTANT Mode

OuterKinds ==
  {"Proposal", "PrepareVote", "CommitVote", "PrepareQC", "CommitQC",
   "TimeoutVote", "TimeoutCertificate", "Chunk",
   "CertifiedRequest", "CertifiedResponse", "CommitCertificateRequest",
   "CommitCertificateResponse"}

RequiredOuterProgressKinds ==
  {"CommitVote", "PrepareQC", "CommitQC", "TimeoutVote",
   "TimeoutCertificate", "Chunk", "CertifiedRequest", "CertifiedResponse",
   "CommitCertificateRequest", "CommitCertificateResponse"}

RequiredOuterAuxiliaryKinds ==
  {"Proposal", "PrepareVote"}

DroppedOuterKind ==
  CASE Mode = "DropOuterCommitVote" -> "CommitVote"
    [] Mode = "DropOuterPrepareQC" -> "PrepareQC"
    [] Mode = "DropOuterCommitQC" -> "CommitQC"
    [] Mode = "DropOuterTimeout" -> "TimeoutVote"
    [] Mode = "DropOuterTimeoutCertificate" -> "TimeoutCertificate"
    [] Mode = "DropOuterChunk" -> "Chunk"
    [] Mode = "DropOuterCertified" -> "CertifiedRequest"
    [] Mode = "DropOuterCertifiedResponse" -> "CertifiedResponse"
    [] Mode = "DropOuterCommit" -> "CommitCertificateRequest"
    [] Mode = "DropOuterCommitResponse" -> "CommitCertificateResponse"
    [] OTHER -> "None"

OuterProgressKinds == RequiredOuterProgressKinds \ {DroppedOuterKind}
OuterAuxiliaryKinds == OuterKinds \ OuterProgressKinds

RuntimeKinds ==
  {"Proposal", "PrepareVote", "CommitVote", "HistoricalLockedCommitVote",
   "CurrentLockedReproposalPrepareVote", "PrepareQC", "CommitQC",
   "TimeoutVote", "TimeoutCertificate",
   "Chunk", "CertifiedRequest", "CertifiedResponse",
   "CommitCertificateRequest", "CommitCertificateResponse"}

RequiredRuntimeProgressKinds ==
  {"HistoricalLockedCommitVote", "CurrentLockedReproposalPrepareVote",
   "PrepareQC", "CommitQC", "TimeoutVote", "TimeoutCertificate"}

RequiredRuntimeNormalKinds ==
  {"Proposal", "PrepareVote", "CommitVote"}

RequiredRuntimeBypassKinds ==
  {"Chunk", "CertifiedRequest", "CertifiedResponse",
   "CommitCertificateRequest", "CommitCertificateResponse"}

DroppedRuntimeProgressKind ==
  CASE Mode = "DropRuntimeLockedCommit" -> "HistoricalLockedCommitVote"
    [] Mode = "DropRuntimeLockedReproposalPrepare" ->
         "CurrentLockedReproposalPrepareVote"
    [] Mode = "DropRuntimePrepareQC" -> "PrepareQC"
    [] Mode = "DropRuntimeCommitQC" -> "CommitQC"
    [] Mode = "DropRuntimeTimeout" -> "TimeoutVote"
    [] Mode = "DropRuntimeTimeoutCertificate" -> "TimeoutCertificate"
    [] OTHER -> "None"

PromotedRuntimeKind ==
  CASE Mode = "PromoteRuntimeProposal" -> "Proposal"
    [] Mode = "PromoteRuntimePrepareVote" -> "PrepareVote"
    [] Mode = "PromoteRuntimeCommitVote" -> "CommitVote"
    [] Mode = "PromoteRuntimeChunk" -> "Chunk"
    [] Mode = "PromoteRuntimeCertified" -> "CertifiedRequest"
    [] Mode = "PromoteRuntimeCertifiedResponse" -> "CertifiedResponse"
    [] Mode = "PromoteRuntimeCommit" -> "CommitCertificateRequest"
    [] Mode = "PromoteRuntimeCommitResponse" ->
         "CommitCertificateResponse"
    [] OTHER -> "None"

RuntimeProgressKinds ==
  (RequiredRuntimeProgressKinds \ {DroppedRuntimeProgressKind})
    \cup ({PromotedRuntimeKind} \ {"None"})

RuntimeNormalKinds == RequiredRuntimeNormalKinds
RuntimeBypassKinds == RequiredRuntimeBypassKinds

VARIABLE observed

vars == <<observed>>

Init == observed = FALSE

Observe ==
  /\ ~observed
  /\ observed' = TRUE

Done == observed /\ UNCHANGED vars

Next == Observe \/ Done

Spec == Init /\ [][Next]_vars

OuterProgressClassAligned ==
  ~observed
    \/ /\ OuterProgressKinds = RequiredOuterProgressKinds
       /\ OuterAuxiliaryKinds = RequiredOuterAuxiliaryKinds
       /\ OuterProgressKinds \cap OuterAuxiliaryKinds = {}
       /\ OuterProgressKinds \cup OuterAuxiliaryKinds = OuterKinds

RuntimeProgressClassAligned ==
  ~observed
    \/ /\ RuntimeProgressKinds = RequiredRuntimeProgressKinds
       /\ RuntimeNormalKinds = RequiredRuntimeNormalKinds
       /\ RuntimeBypassKinds = RequiredRuntimeBypassKinds
       /\ RuntimeProgressKinds \cap RuntimeNormalKinds = {}
       /\ RuntimeProgressKinds \cap RuntimeBypassKinds = {}
       /\ RuntimeNormalKinds \cap RuntimeBypassKinds = {}
       /\ RuntimeProgressKinds \cup RuntimeNormalKinds
            \cup RuntimeBypassKinds = RuntimeKinds

====
