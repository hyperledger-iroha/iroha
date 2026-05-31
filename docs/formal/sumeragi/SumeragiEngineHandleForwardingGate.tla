---- MODULE SumeragiEngineHandleForwardingGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for top-level pure-engine input argument forwarding.

This slice models the argument values preserved by
`ConsensusEngine::handle(...)` when it routes `ConsensusInput` variants to
their handlers. `SumeragiEngineHandleDispatchGate.tla` proves that each input
variant reaches exactly one matching handler; this companion model proves that
the handler receives the exact proposal, certificate, payload, validation, and
committed-block fields from the input.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugProposalMutatesRound,
  \* @type: Bool;
  BugProposalMutatesSubject,
  \* @type: Bool;
  BugProposalMutatesHighest,
  \* @type: Bool;
  BugCertificateMutatesPhase,
  \* @type: Bool;
  BugCertificateMutatesRound,
  \* @type: Bool;
  BugCertificateMutatesSubject,
  \* @type: Bool;
  BugCertificateMutatesHighest,
  \* @type: Bool;
  BugPayloadMutatesSubject,
  \* @type: Bool;
  BugValidationMutatesRound,
  \* @type: Bool;
  BugValidationMutatesBlock,
  \* @type: Bool;
  BugValidationMutatesValidity,
  \* @type: Bool;
  BugCommittedMutatesRound,
  \* @type: Bool;
  BugCommittedMutatesBlock,
  \* @type: Bool;
  BugCommittedMutatesReconfiguration

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Inputs == {
  "tick",
  "proposal_no_highest",
  "proposal_with_highest",
  "certificate_prepare_no_highest",
  "certificate_commit_no_highest",
  "certificate_new_view_with_highest",
  "payload_available",
  "validation_valid",
  "validation_invalid",
  "committed_plain",
  "committed_reconfiguration"
}

Rounds == {
  "none",
  "proposal_round",
  "certificate_round",
  "validation_round",
  "committed_round",
  "wrong_round"
}

Subjects == {
  "none",
  "proposal_subject",
  "certificate_subject",
  "payload_subject",
  "wrong_subject"
}

Blocks == {"none", "validation_block", "committed_block", "wrong_block"}
HighestValues == {"none", "highest_a", "highest_b"}
Phases == {"none", "Prepare", "Commit", "NewView"}
ValidityValues == {"none", "valid", "invalid"}
ReconfigurationValues == {"none", "change_a", "change_b"}

ProposalInput(input) ==
  input \in {"proposal_no_highest", "proposal_with_highest"}

CertificateInput(input) ==
  input \in {
    "certificate_prepare_no_highest",
    "certificate_commit_no_highest",
    "certificate_new_view_with_highest"
  }

ValidationInput(input) ==
  input \in {"validation_valid", "validation_invalid"}

CommittedInput(input) ==
  input \in {"committed_plain", "committed_reconfiguration"}

SpecProposalRound(input) ==
  IF ProposalInput(input) THEN "proposal_round" ELSE "none"

SpecProposalSubject(input) ==
  IF ProposalInput(input) THEN "proposal_subject" ELSE "none"

SpecProposalHighest(input) ==
  IF input = "proposal_with_highest" THEN "highest_a" ELSE "none"

SpecCertificatePhase(input) ==
  CASE input = "certificate_prepare_no_highest" -> "Prepare"
    [] input = "certificate_commit_no_highest" -> "Commit"
    [] input = "certificate_new_view_with_highest" -> "NewView"
    [] OTHER -> "none"

SpecCertificateRound(input) ==
  IF CertificateInput(input) THEN "certificate_round" ELSE "none"

SpecCertificateSubject(input) ==
  IF CertificateInput(input) THEN "certificate_subject" ELSE "none"

SpecCertificateHighest(input) ==
  IF input = "certificate_new_view_with_highest" THEN "highest_a" ELSE "none"

SpecPayloadSubject(input) ==
  IF input = "payload_available" THEN "payload_subject" ELSE "none"

SpecValidationRound(input) ==
  IF ValidationInput(input) THEN "validation_round" ELSE "none"

SpecValidationBlock(input) ==
  IF ValidationInput(input) THEN "validation_block" ELSE "none"

SpecValidationValidity(input) ==
  CASE input = "validation_valid" -> "valid"
    [] input = "validation_invalid" -> "invalid"
    [] OTHER -> "none"

SpecCommittedRound(input) ==
  IF CommittedInput(input) THEN "committed_round" ELSE "none"

SpecCommittedBlock(input) ==
  IF CommittedInput(input) THEN "committed_block" ELSE "none"

SpecCommittedReconfiguration(input) ==
  IF input = "committed_reconfiguration" THEN "change_a" ELSE "none"

MutatedHighest(value) ==
  IF value = "none" THEN "highest_b" ELSE "none"

MutatedPhase(value) ==
  IF value = "Commit" THEN "Prepare" ELSE "Commit"

MutatedValidity(value) ==
  IF value = "valid" THEN "invalid" ELSE "valid"

MutatedReconfiguration(value) ==
  IF value = "none" THEN "change_b" ELSE "none"

ImplementationProposalRound(input) ==
  IF ProposalInput(input) /\ BugProposalMutatesRound
  THEN "wrong_round"
  ELSE SpecProposalRound(input)

ImplementationProposalSubject(input) ==
  IF ProposalInput(input) /\ BugProposalMutatesSubject
  THEN "wrong_subject"
  ELSE SpecProposalSubject(input)

ImplementationProposalHighest(input) ==
  IF ProposalInput(input) /\ BugProposalMutatesHighest
  THEN MutatedHighest(SpecProposalHighest(input))
  ELSE SpecProposalHighest(input)

ImplementationCertificatePhase(input) ==
  IF CertificateInput(input) /\ BugCertificateMutatesPhase
  THEN MutatedPhase(SpecCertificatePhase(input))
  ELSE SpecCertificatePhase(input)

ImplementationCertificateRound(input) ==
  IF CertificateInput(input) /\ BugCertificateMutatesRound
  THEN "wrong_round"
  ELSE SpecCertificateRound(input)

ImplementationCertificateSubject(input) ==
  IF CertificateInput(input) /\ BugCertificateMutatesSubject
  THEN "wrong_subject"
  ELSE SpecCertificateSubject(input)

ImplementationCertificateHighest(input) ==
  IF CertificateInput(input) /\ BugCertificateMutatesHighest
  THEN MutatedHighest(SpecCertificateHighest(input))
  ELSE SpecCertificateHighest(input)

ImplementationPayloadSubject(input) ==
  IF input = "payload_available" /\ BugPayloadMutatesSubject
  THEN "wrong_subject"
  ELSE SpecPayloadSubject(input)

ImplementationValidationRound(input) ==
  IF ValidationInput(input) /\ BugValidationMutatesRound
  THEN "wrong_round"
  ELSE SpecValidationRound(input)

ImplementationValidationBlock(input) ==
  IF ValidationInput(input) /\ BugValidationMutatesBlock
  THEN "wrong_block"
  ELSE SpecValidationBlock(input)

ImplementationValidationValidity(input) ==
  IF ValidationInput(input) /\ BugValidationMutatesValidity
  THEN MutatedValidity(SpecValidationValidity(input))
  ELSE SpecValidationValidity(input)

ImplementationCommittedRound(input) ==
  IF CommittedInput(input) /\ BugCommittedMutatesRound
  THEN "wrong_round"
  ELSE SpecCommittedRound(input)

ImplementationCommittedBlock(input) ==
  IF CommittedInput(input) /\ BugCommittedMutatesBlock
  THEN "wrong_block"
  ELSE SpecCommittedBlock(input)

ImplementationCommittedReconfiguration(input) ==
  IF CommittedInput(input) /\ BugCommittedMutatesReconfiguration
  THEN MutatedReconfiguration(SpecCommittedReconfiguration(input))
  ELSE SpecCommittedReconfiguration(input)

TypeInvariant ==
  /\ BugProposalMutatesRound \in BOOLEAN
  /\ BugProposalMutatesSubject \in BOOLEAN
  /\ BugProposalMutatesHighest \in BOOLEAN
  /\ BugCertificateMutatesPhase \in BOOLEAN
  /\ BugCertificateMutatesRound \in BOOLEAN
  /\ BugCertificateMutatesSubject \in BOOLEAN
  /\ BugCertificateMutatesHighest \in BOOLEAN
  /\ BugPayloadMutatesSubject \in BOOLEAN
  /\ BugValidationMutatesRound \in BOOLEAN
  /\ BugValidationMutatesBlock \in BOOLEAN
  /\ BugValidationMutatesValidity \in BOOLEAN
  /\ BugCommittedMutatesRound \in BOOLEAN
  /\ BugCommittedMutatesBlock \in BOOLEAN
  /\ BugCommittedMutatesReconfiguration \in BOOLEAN
  /\ tried \subseteq Inputs
  /\ \A input \in tried:
    /\ SpecProposalRound(input) \in Rounds
    /\ ImplementationProposalRound(input) \in Rounds
    /\ SpecProposalSubject(input) \in Subjects
    /\ ImplementationProposalSubject(input) \in Subjects
    /\ SpecProposalHighest(input) \in HighestValues
    /\ ImplementationProposalHighest(input) \in HighestValues
    /\ SpecCertificatePhase(input) \in Phases
    /\ ImplementationCertificatePhase(input) \in Phases
    /\ SpecCertificateRound(input) \in Rounds
    /\ ImplementationCertificateRound(input) \in Rounds
    /\ SpecCertificateSubject(input) \in Subjects
    /\ ImplementationCertificateSubject(input) \in Subjects
    /\ SpecCertificateHighest(input) \in HighestValues
    /\ ImplementationCertificateHighest(input) \in HighestValues
    /\ SpecPayloadSubject(input) \in Subjects
    /\ ImplementationPayloadSubject(input) \in Subjects
    /\ SpecValidationRound(input) \in Rounds
    /\ ImplementationValidationRound(input) \in Rounds
    /\ SpecValidationBlock(input) \in Blocks
    /\ ImplementationValidationBlock(input) \in Blocks
    /\ SpecValidationValidity(input) \in ValidityValues
    /\ ImplementationValidationValidity(input) \in ValidityValues
    /\ SpecCommittedRound(input) \in Rounds
    /\ ImplementationCommittedRound(input) \in Rounds
    /\ SpecCommittedBlock(input) \in Blocks
    /\ ImplementationCommittedBlock(input) \in Blocks
    /\ SpecCommittedReconfiguration(input) \in ReconfigurationValues
    /\ ImplementationCommittedReconfiguration(input) \in ReconfigurationValues

Init ==
  tried = {}

TryInput(input) ==
  /\ input \in Inputs \ tried
  /\ tried' = tried \cup {input}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E input \in Inputs: TryInput(input)
  \/ Stable

ProposalRoundForwarded ==
  \A input \in tried:
    ImplementationProposalRound(input) = SpecProposalRound(input)

ProposalSubjectForwarded ==
  \A input \in tried:
    ImplementationProposalSubject(input) = SpecProposalSubject(input)

ProposalHighestForwarded ==
  \A input \in tried:
    ImplementationProposalHighest(input) = SpecProposalHighest(input)

CertificatePhaseForwarded ==
  \A input \in tried:
    ImplementationCertificatePhase(input) = SpecCertificatePhase(input)

CertificateRoundForwarded ==
  \A input \in tried:
    ImplementationCertificateRound(input) = SpecCertificateRound(input)

CertificateSubjectForwarded ==
  \A input \in tried:
    ImplementationCertificateSubject(input) = SpecCertificateSubject(input)

CertificateHighestForwarded ==
  \A input \in tried:
    ImplementationCertificateHighest(input) = SpecCertificateHighest(input)

PayloadSubjectForwarded ==
  \A input \in tried:
    ImplementationPayloadSubject(input) = SpecPayloadSubject(input)

ValidationRoundForwarded ==
  \A input \in tried:
    ImplementationValidationRound(input) = SpecValidationRound(input)

ValidationBlockForwarded ==
  \A input \in tried:
    ImplementationValidationBlock(input) = SpecValidationBlock(input)

ValidationValidityForwarded ==
  \A input \in tried:
    ImplementationValidationValidity(input) = SpecValidationValidity(input)

CommittedRoundForwarded ==
  \A input \in tried:
    ImplementationCommittedRound(input) = SpecCommittedRound(input)

CommittedBlockForwarded ==
  \A input \in tried:
    ImplementationCommittedBlock(input) = SpecCommittedBlock(input)

CommittedReconfigurationForwarded ==
  \A input \in tried:
    ImplementationCommittedReconfiguration(input) =
      SpecCommittedReconfiguration(input)

AllPayloadsForwarded ==
  \A input \in tried:
    /\ ImplementationProposalRound(input) = SpecProposalRound(input)
    /\ ImplementationProposalSubject(input) = SpecProposalSubject(input)
    /\ ImplementationProposalHighest(input) = SpecProposalHighest(input)
    /\ ImplementationCertificatePhase(input) = SpecCertificatePhase(input)
    /\ ImplementationCertificateRound(input) = SpecCertificateRound(input)
    /\ ImplementationCertificateSubject(input) = SpecCertificateSubject(input)
    /\ ImplementationCertificateHighest(input) = SpecCertificateHighest(input)
    /\ ImplementationPayloadSubject(input) = SpecPayloadSubject(input)
    /\ ImplementationValidationRound(input) = SpecValidationRound(input)
    /\ ImplementationValidationBlock(input) = SpecValidationBlock(input)
    /\ ImplementationValidationValidity(input) = SpecValidationValidity(input)
    /\ ImplementationCommittedRound(input) = SpecCommittedRound(input)
    /\ ImplementationCommittedBlock(input) = SpecCommittedBlock(input)
    /\ ImplementationCommittedReconfiguration(input) =
      SpecCommittedReconfiguration(input)

ValuesStayInDomain ==
  \A input \in tried:
    /\ ImplementationProposalRound(input) \in Rounds
    /\ ImplementationProposalSubject(input) \in Subjects
    /\ ImplementationProposalHighest(input) \in HighestValues
    /\ ImplementationCertificatePhase(input) \in Phases
    /\ ImplementationCertificateRound(input) \in Rounds
    /\ ImplementationCertificateSubject(input) \in Subjects
    /\ ImplementationCertificateHighest(input) \in HighestValues
    /\ ImplementationPayloadSubject(input) \in Subjects
    /\ ImplementationValidationRound(input) \in Rounds
    /\ ImplementationValidationBlock(input) \in Blocks
    /\ ImplementationValidationValidity(input) \in ValidityValues
    /\ ImplementationCommittedRound(input) \in Rounds
    /\ ImplementationCommittedBlock(input) \in Blocks
    /\ ImplementationCommittedReconfiguration(input) \in ReconfigurationValues

Safety ==
  /\ ProposalRoundForwarded
  /\ ProposalSubjectForwarded
  /\ ProposalHighestForwarded
  /\ CertificatePhaseForwarded
  /\ CertificateRoundForwarded
  /\ CertificateSubjectForwarded
  /\ CertificateHighestForwarded
  /\ PayloadSubjectForwarded
  /\ ValidationRoundForwarded
  /\ ValidationBlockForwarded
  /\ ValidationValidityForwarded
  /\ CommittedRoundForwarded
  /\ CommittedBlockForwarded
  /\ CommittedReconfigurationForwarded
  /\ AllPayloadsForwarded
  /\ ValuesStayInDomain

=============================================================================
