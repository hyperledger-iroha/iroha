---- MODULE SumeragiEngineHandleOutputRelayGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for top-level pure-engine output relaying.

This slice models the return-value boundary of `ConsensusEngine::handle(...)`.
Dispatch and argument forwarding are covered by companion models; this model
proves that the top-level match returns exactly the selected handler's output
list without dropping, inventing, duplicating, substituting, or reordering
adapter commands.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugDropNonEmptyOutput,
  \* @type: Bool;
  BugEmitOnEmptyOutput,
  \* @type: Bool;
  BugDropFirstOutput,
  \* @type: Bool;
  BugDropSecondOutput,
  \* @type: Bool;
  BugDuplicateOutput,
  \* @type: Bool;
  BugReorderTwoOutputs,
  \* @type: Bool;
  BugSubstituteOutput,
  \* @type: Bool;
  BugTickUsesProposalOutput,
  \* @type: Bool;
  BugProposalUsesTickOutput,
  \* @type: Bool;
  BugCertificateUsesPayloadOutput,
  \* @type: Bool;
  BugPayloadUsesCertificateOutput,
  \* @type: Bool;
  BugValidationUsesCommittedOutput,
  \* @type: Bool;
  BugCommittedUsesValidationOutput

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Inputs == {
  "tick_new_view",
  "proposal_accepted",
  "proposal_rejected",
  "certificate_prepare",
  "certificate_commit_fetch",
  "certificate_commit_commit",
  "certificate_new_view",
  "certificate_rejected",
  "payload_matching_pending",
  "payload_ignored",
  "validation_valid",
  "validation_invalid",
  "committed_plain",
  "committed_reconfiguration",
  "committed_duplicate"
}

OutputTraces == {
  "none",
  "new_view_then_advance",
  "advance_then_new_view",
  "new_view_only",
  "advance_only",
  "validate_then_prepare",
  "prepare_then_validate",
  "validate_only",
  "prepare_only",
  "commit_vote",
  "fetch_payload",
  "commit_block",
  "advance_view",
  "activate_validator_set",
  "duplicated_output",
  "wrong_output",
  "spurious_output"
}

TickInput(input) ==
  input = "tick_new_view"

ProposalInput(input) ==
  input \in {"proposal_accepted", "proposal_rejected"}

CertificateInput(input) ==
  input \in {
    "certificate_prepare",
    "certificate_commit_fetch",
    "certificate_commit_commit",
    "certificate_new_view",
    "certificate_rejected"
  }

PayloadInput(input) ==
  input \in {"payload_matching_pending", "payload_ignored"}

ValidationInput(input) ==
  input \in {"validation_valid", "validation_invalid"}

CommittedInput(input) ==
  input \in {
    "committed_plain",
    "committed_reconfiguration",
    "committed_duplicate"
  }

SpecHandlerOutput(input) ==
  CASE input = "tick_new_view" -> "new_view_then_advance"
    [] input = "proposal_accepted" -> "validate_then_prepare"
    [] input = "proposal_rejected" -> "none"
    [] input = "certificate_prepare" -> "commit_vote"
    [] input = "certificate_commit_fetch" -> "fetch_payload"
    [] input = "certificate_commit_commit" -> "commit_block"
    [] input = "certificate_new_view" -> "advance_view"
    [] input = "certificate_rejected" -> "none"
    [] input = "payload_matching_pending" -> "commit_block"
    [] input = "payload_ignored" -> "none"
    [] input = "validation_valid" -> "none"
    [] input = "validation_invalid" -> "new_view_then_advance"
    [] input = "committed_plain" -> "none"
    [] input = "committed_reconfiguration" -> "activate_validator_set"
    [] input = "committed_duplicate" -> "none"
    [] OTHER -> "none"

CrossHandlerOutput(input) ==
  CASE TickInput(input) -> "validate_then_prepare"
    [] ProposalInput(input) -> "new_view_then_advance"
    [] CertificateInput(input) -> "commit_block"
    [] PayloadInput(input) -> "commit_vote"
    [] ValidationInput(input) -> "activate_validator_set"
    [] CommittedInput(input) -> "new_view_then_advance"
    [] OTHER -> "wrong_output"

ImplementationBaseOutput(input) ==
  IF TickInput(input) /\ BugTickUsesProposalOutput
  THEN CrossHandlerOutput(input)
  ELSE IF ProposalInput(input) /\ BugProposalUsesTickOutput
       THEN CrossHandlerOutput(input)
       ELSE IF CertificateInput(input) /\ BugCertificateUsesPayloadOutput
            THEN CrossHandlerOutput(input)
            ELSE IF PayloadInput(input) /\ BugPayloadUsesCertificateOutput
                 THEN CrossHandlerOutput(input)
                 ELSE IF ValidationInput(input) /\
                         BugValidationUsesCommittedOutput
                      THEN CrossHandlerOutput(input)
                      ELSE IF CommittedInput(input) /\
                              BugCommittedUsesValidationOutput
                           THEN CrossHandlerOutput(input)
                           ELSE SpecHandlerOutput(input)

DropFirst(output) ==
  CASE output = "new_view_then_advance" -> "advance_only"
    [] output = "validate_then_prepare" -> "prepare_only"
    [] OTHER -> output

DropSecond(output) ==
  CASE output = "new_view_then_advance" -> "new_view_only"
    [] output = "validate_then_prepare" -> "validate_only"
    [] OTHER -> output

ReorderOutput(output) ==
  CASE output = "new_view_then_advance" -> "advance_then_new_view"
    [] output = "validate_then_prepare" -> "prepare_then_validate"
    [] OTHER -> output

ImplementationOutput(input) ==
  IF BugDropNonEmptyOutput /\ ImplementationBaseOutput(input) # "none"
  THEN "none"
  ELSE IF BugEmitOnEmptyOutput /\ ImplementationBaseOutput(input) = "none"
       THEN "spurious_output"
       ELSE IF BugDropFirstOutput
            THEN DropFirst(ImplementationBaseOutput(input))
            ELSE IF BugDropSecondOutput
                 THEN DropSecond(ImplementationBaseOutput(input))
                 ELSE IF BugDuplicateOutput /\
                         ImplementationBaseOutput(input) # "none"
                      THEN "duplicated_output"
                      ELSE IF BugReorderTwoOutputs
                           THEN ReorderOutput(ImplementationBaseOutput(input))
                           ELSE IF BugSubstituteOutput /\
                                   ImplementationBaseOutput(input) # "none"
                                THEN "wrong_output"
                                ELSE ImplementationBaseOutput(input)

TypeInvariant ==
  /\ BugDropNonEmptyOutput \in BOOLEAN
  /\ BugEmitOnEmptyOutput \in BOOLEAN
  /\ BugDropFirstOutput \in BOOLEAN
  /\ BugDropSecondOutput \in BOOLEAN
  /\ BugDuplicateOutput \in BOOLEAN
  /\ BugReorderTwoOutputs \in BOOLEAN
  /\ BugSubstituteOutput \in BOOLEAN
  /\ BugTickUsesProposalOutput \in BOOLEAN
  /\ BugProposalUsesTickOutput \in BOOLEAN
  /\ BugCertificateUsesPayloadOutput \in BOOLEAN
  /\ BugPayloadUsesCertificateOutput \in BOOLEAN
  /\ BugValidationUsesCommittedOutput \in BOOLEAN
  /\ BugCommittedUsesValidationOutput \in BOOLEAN
  /\ tried \subseteq Inputs
  /\ \A input \in tried:
    /\ SpecHandlerOutput(input) \in OutputTraces
    /\ CrossHandlerOutput(input) \in OutputTraces
    /\ ImplementationBaseOutput(input) \in OutputTraces
    /\ DropFirst(ImplementationBaseOutput(input)) \in OutputTraces
    /\ DropSecond(ImplementationBaseOutput(input)) \in OutputTraces
    /\ ReorderOutput(ImplementationBaseOutput(input)) \in OutputTraces
    /\ ImplementationOutput(input) \in OutputTraces

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

HandleRelaysHandlerOutputExactly ==
  \A input \in tried:
    ImplementationOutput(input) = SpecHandlerOutput(input)

EmptyHandlerOutputsRemainEmpty ==
  \A input \in tried:
    SpecHandlerOutput(input) = "none" => ImplementationOutput(input) = "none"

NonEmptyHandlerOutputsAreNotDropped ==
  \A input \in tried:
    SpecHandlerOutput(input) # "none" => ImplementationOutput(input) # "none"

TwoOutputOrderPreserved ==
  \A input \in tried:
    /\ SpecHandlerOutput(input) = "new_view_then_advance" =>
      ImplementationOutput(input) = "new_view_then_advance"
    /\ SpecHandlerOutput(input) = "validate_then_prepare" =>
      ImplementationOutput(input) = "validate_then_prepare"

TickOutputsRelayed ==
  \A input \in tried:
    TickInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

ProposalOutputsRelayed ==
  \A input \in tried:
    ProposalInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

CertificateOutputsRelayed ==
  \A input \in tried:
    CertificateInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

PayloadOutputsRelayed ==
  \A input \in tried:
    PayloadInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

ValidationOutputsRelayed ==
  \A input \in tried:
    ValidationInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

CommittedOutputsRelayed ==
  \A input \in tried:
    CommittedInput(input) =>
      ImplementationOutput(input) = SpecHandlerOutput(input)

ValuesStayInDomain ==
  \A input \in tried:
    /\ SpecHandlerOutput(input) \in OutputTraces
    /\ ImplementationBaseOutput(input) \in OutputTraces
    /\ ImplementationOutput(input) \in OutputTraces

Safety ==
  /\ HandleRelaysHandlerOutputExactly
  /\ EmptyHandlerOutputsRemainEmpty
  /\ NonEmptyHandlerOutputsAreNotDropped
  /\ TwoOutputOrderPreserved
  /\ TickOutputsRelayed
  /\ ProposalOutputsRelayed
  /\ CertificateOutputsRelayed
  /\ PayloadOutputsRelayed
  /\ ValidationOutputsRelayed
  /\ CommittedOutputsRelayed
  /\ ValuesStayInDomain

=============================================================================
