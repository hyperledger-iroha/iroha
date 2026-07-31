---- MODULE SumeragiV2ReplayLockedBodyCarrierMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Compact mutation model for the locked-body carrier at replay Finish.

The recovering validator owns one historical locked Prepare and one unrelated
current signature.  Fixed restart reconstruction admits the exact FetchBody
candidate before that signature.  The mutant admits only the signature.  The
signature may finish while the body owner advances through the normal
certified-request, Store, Validate, Commit, or terminal handoffs.

Recovery authority is deliberately not a non-authority carrier.  The fixed
model proves that a concrete carrier is reachable throughout Replaying and
survives Finish.  The bug configuration omits the stronger Replaying check so
TLC reaches Finish and shows that changing to Recovered drops the last owner.
This is bounded mutation evidence, not a proof of the full asynchronous spec.
***************************************************************************)

CONSTANT Mode

Modes == {"Fixed", "DropLockedFetch"}

ASSUME Mode \in Modes

RecoveringNode == "validator-0"
RecoveryContext == [height |-> 7, epoch |-> 2]
LockedView == 3
CurrentView == 4
RecoveryGeneration == 2
LockedSubject == "locked-block-7"

LockedPrepareEvidence ==
  [kind |-> "PrepareQC",
   context |-> RecoveryContext,
   view |-> LockedView,
   subject |-> LockedSubject]

CurrentSignatureEvidence ==
  [kind |-> "TimeoutIntent",
   context |-> RecoveryContext,
   view |-> CurrentView,
   subject |-> "no-subject"]

Candidate(candidateKind, candidateView, candidateEvidence) ==
  [class |-> "Completion",
   kind |-> candidateKind,
   node |-> RecoveringNode,
   context |-> RecoveryContext,
   view |-> candidateView,
   generation |-> RecoveryGeneration,
   subject |-> LockedSubject,
   evidence |-> candidateEvidence]

LockedFetchCandidate ==
  Candidate("FetchBody", LockedView, LockedPrepareEvidence)

LockedStoreCandidate ==
  Candidate("StoreBody", LockedView, LockedPrepareEvidence)

LockedValidateCandidate ==
  Candidate("ValidateBody", LockedView, LockedPrepareEvidence)

ReplaySignatureCandidate ==
  Candidate("SignTimeout", CurrentView, CurrentSignatureEvidence)

CandidateSet ==
  {LockedFetchCandidate, LockedStoreCandidate,
   LockedValidateCandidate, ReplaySignatureCandidate}

BodyPipelineKinds ==
  {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody",
   "StoreBody", "ValidateBody"}

Phases == {"ReplayRequired", "Replaying", "Recovered"}

VARIABLES phase,
          scheduled,
          signatureActive,
          signatureReady,
          activeCertifiedRequest,
          lockedCommitWitness,
          terminalValidation,
          finished

vars ==
  <<phase, scheduled, signatureActive, signatureReady,
    activeCertifiedRequest, lockedCommitWitness, terminalValidation,
    finished>>

SequenceSet(sequence) ==
  {sequence[index]: index \in 1..Len(sequence)}

SignatureSuffix ==
  IF signatureActive THEN <<ReplaySignatureCandidate>> ELSE <<>>

BodyOnly(sequence) ==
  IF LockedFetchCandidate \in SequenceSet(sequence)
  THEN <<LockedFetchCandidate>>
  ELSE IF LockedStoreCandidate \in SequenceSet(sequence)
       THEN <<LockedStoreCandidate>>
       ELSE IF LockedValidateCandidate \in SequenceSet(sequence)
            THEN <<LockedValidateCandidate>>
            ELSE <<>>

RestartReplay ==
  IF Mode = "Fixed"
  THEN <<LockedFetchCandidate, ReplaySignatureCandidate>>
  ELSE <<ReplaySignatureCandidate>>

Init ==
  /\ phase = "ReplayRequired"
  /\ scheduled = <<>>
  /\ signatureActive = FALSE
  /\ signatureReady = FALSE
  /\ activeCertifiedRequest = FALSE
  /\ lockedCommitWitness = FALSE
  /\ terminalValidation = FALSE
  /\ finished = FALSE

ResetSchedulerForReplay ==
  /\ phase = "ReplayRequired"
  /\ phase' = "Replaying"
  /\ scheduled' = RestartReplay
  /\ signatureActive' = TRUE
  /\ signatureReady' = FALSE
  /\ activeCertifiedRequest' = FALSE
  /\ lockedCommitWitness' = FALSE
  /\ terminalValidation' = FALSE
  /\ finished' = FALSE

BeginCertifiedRequest ==
  /\ phase \in {"Replaying", "Recovered"}
  /\ LockedFetchCandidate \in SequenceSet(scheduled)
  /\ scheduled' = SignatureSuffix
  /\ activeCertifiedRequest' = TRUE
  /\ UNCHANGED <<phase, signatureActive, signatureReady,
                  lockedCommitWitness, terminalValidation, finished>>

ReceiveCertifiedBody ==
  /\ phase \in {"Replaying", "Recovered"}
  /\ activeCertifiedRequest
  /\ scheduled' = <<LockedStoreCandidate>> \o SignatureSuffix
  /\ activeCertifiedRequest' = FALSE
  /\ UNCHANGED <<phase, signatureActive, signatureReady,
                  lockedCommitWitness, terminalValidation, finished>>

CompleteStore ==
  /\ phase \in {"Replaying", "Recovered"}
  /\ LockedStoreCandidate \in SequenceSet(scheduled)
  /\ scheduled' = <<LockedValidateCandidate>> \o SignatureSuffix
  /\ UNCHANGED <<phase, signatureActive, signatureReady,
                  activeCertifiedRequest, lockedCommitWitness,
                  terminalValidation, finished>>

CompleteValidationWithCommitWitness ==
  /\ phase \in {"Replaying", "Recovered"}
  /\ LockedValidateCandidate \in SequenceSet(scheduled)
  /\ scheduled' = SignatureSuffix
  /\ lockedCommitWitness' = TRUE
  /\ UNCHANGED <<phase, signatureActive, signatureReady,
                  activeCertifiedRequest, terminalValidation, finished>>

CompleteValidationWithTerminalFence ==
  /\ phase \in {"Replaying", "Recovered"}
  /\ LockedValidateCandidate \in SequenceSet(scheduled)
  /\ scheduled' = SignatureSuffix
  /\ terminalValidation' = TRUE
  /\ UNCHANGED <<phase, signatureActive, signatureReady,
                  activeCertifiedRequest, lockedCommitWitness, finished>>

CompleteReplaySignature ==
  /\ phase = "Replaying"
  /\ signatureActive
  /\ ReplaySignatureCandidate \in SequenceSet(scheduled)
  /\ scheduled' = BodyOnly(scheduled)
  /\ signatureActive' = FALSE
  /\ signatureReady' = TRUE
  /\ UNCHANGED <<phase, activeCertifiedRequest, lockedCommitWitness,
                  terminalValidation, finished>>

FinishResponsiveReplay ==
  /\ phase = "Replaying"
  /\ ~signatureActive
  /\ signatureReady
  /\ phase' = "Recovered"
  /\ finished' = TRUE
  /\ UNCHANGED <<scheduled, signatureActive, signatureReady,
                  activeCertifiedRequest, lockedCommitWitness,
                  terminalValidation>>

Next ==
  \/ ResetSchedulerForReplay
  \/ BeginCertifiedRequest
  \/ ReceiveCertifiedBody
  \/ CompleteStore
  \/ CompleteValidationWithCommitWitness
  \/ CompleteValidationWithTerminalFence
  \/ CompleteReplaySignature
  \/ FinishResponsiveReplay

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ResetSchedulerForReplay)
  /\ WF_vars(CompleteReplaySignature)
  /\ WF_vars(FinishResponsiveReplay)

TypeInvariant ==
  /\ phase \in Phases
  /\ scheduled \in Seq(CandidateSet)
  /\ Len(scheduled) <= 2
  /\ signatureActive \in BOOLEAN
  /\ signatureReady \in BOOLEAN
  /\ activeCertifiedRequest \in BOOLEAN
  /\ lockedCommitWitness \in BOOLEAN
  /\ terminalValidation \in BOOLEAN
  /\ finished \in BOOLEAN
  /\ (signatureActive
        <=> ReplaySignatureCandidate \in SequenceSet(scheduled))
  /\ (signatureReady => ~signatureActive)
  /\ (finished <=> phase = "Recovered")

HistoricalLockedPrepareSource == TRUE

HistoricalLockedCommitRecoveryWitness ==
  lockedCommitWitness

HistoricalLockedCertifiedRequestActive ==
  activeCertifiedRequest

HistoricalLockedBodyPipelineCandidate(candidate) ==
  /\ candidate.node = RecoveringNode
  /\ candidate.context = RecoveryContext
  /\ candidate.view = LockedView
  /\ candidate.generation = RecoveryGeneration
  /\ candidate.subject = LockedSubject
  /\ candidate.kind \in BodyPipelineKinds

HistoricalLockedBodyRecoveryTerminal ==
  terminalValidation

HistoricalLockedBodyNonAuthorityCarrier ==
  \/ HistoricalLockedCommitRecoveryWitness
  \/ HistoricalLockedCertifiedRequestActive
  \/ \E candidate \in SequenceSet(scheduled):
       HistoricalLockedBodyPipelineCandidate(candidate)
  \/ HistoricalLockedBodyRecoveryTerminal

ReplayLockedBodyCarrierReady ==
  HistoricalLockedPrepareSource =>
    HistoricalLockedBodyNonAuthorityCarrier

ReplayLockedBodyCarrierInvariant ==
  phase = "Replaying" => ReplayLockedBodyCarrierReady

HistoricalLockedBodyRecoveryAuthority ==
  phase \in {"ReplayRequired", "Replaying"}

HistoricalLockedBodyRecoveryStage ==
  \/ HistoricalLockedBodyRecoveryAuthority
  \/ HistoricalLockedBodyNonAuthorityCarrier

HistoricalLockedBodyRecoveryStageInvariant ==
  HistoricalLockedPrepareSource =>
    HistoricalLockedBodyRecoveryStage

FinishDoesNotDropLastLockedBodyOwner ==
  finished => HistoricalLockedBodyNonAuthorityCarrier

ReplayEventuallyFinishes ==
  <>finished

=============================================================================
