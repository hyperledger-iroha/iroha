---- MODULE SumeragiEngineHandleDispatchGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for top-level pure-engine input dispatch.

This slice models `ConsensusEngine::handle(...)`. Each `ConsensusInput`
variant must be routed to exactly one matching handler. Certificate phase
variants all dispatch to `on_certificate(...)`; phase-specific certificate
prefiltering is covered by `SumeragiEngineCertificateDispatchGate.tla`.
Validation outcomes and committed-block reconfiguration forms likewise keep
their top-level handler identity and leave branch semantics to their dedicated
models.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugDropTick,
  \* @type: Bool;
  BugDropProposal,
  \* @type: Bool;
  BugDropCertificate,
  \* @type: Bool;
  BugDropPayload,
  \* @type: Bool;
  BugDropValidation,
  \* @type: Bool;
  BugDropCommitted,
  \* @type: Bool;
  BugTickAsProposal,
  \* @type: Bool;
  BugProposalAsTick,
  \* @type: Bool;
  BugCertificateAsPayload,
  \* @type: Bool;
  BugPayloadAsCertificate,
  \* @type: Bool;
  BugValidationAsCommitted,
  \* @type: Bool;
  BugCommittedAsValidation,
  \* @type: Bool;
  BugDispatchTwice

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Inputs == {
  "tick",
  "proposal",
  "certificate_prepare",
  "certificate_commit",
  "certificate_new_view",
  "payload_available",
  "validation_valid",
  "validation_invalid",
  "committed_plain",
  "committed_reconfiguration"
}

Handlers == {
  "tick",
  "proposal",
  "certificate",
  "payload",
  "validation",
  "committed"
}

CertificateInput(input) ==
  input \in {
    "certificate_prepare",
    "certificate_commit",
    "certificate_new_view"
  }

ValidationInput(input) ==
  input \in {"validation_valid", "validation_invalid"}

CommittedInput(input) ==
  input \in {"committed_plain", "committed_reconfiguration"}

SpecHandler(input) ==
  CASE input = "tick" -> "tick"
    [] input = "proposal" -> "proposal"
    [] CertificateInput(input) -> "certificate"
    [] input = "payload_available" -> "payload"
    [] ValidationInput(input) -> "validation"
    [] CommittedInput(input) -> "committed"

ExtraHandler(handler) ==
  CASE handler = "tick" -> "proposal"
    [] handler = "proposal" -> "tick"
    [] handler = "certificate" -> "payload"
    [] handler = "payload" -> "certificate"
    [] handler = "validation" -> "committed"
    [] OTHER -> "validation"

PrimaryHandlers(input) ==
  IF input = "tick" /\ BugDropTick
  THEN {}
  ELSE IF input = "proposal" /\ BugDropProposal
       THEN {}
       ELSE IF CertificateInput(input) /\ BugDropCertificate
            THEN {}
            ELSE IF input = "payload_available" /\ BugDropPayload
                 THEN {}
                 ELSE IF ValidationInput(input) /\ BugDropValidation
                      THEN {}
                      ELSE IF CommittedInput(input) /\ BugDropCommitted
                           THEN {}
                           ELSE IF input = "tick" /\ BugTickAsProposal
                                THEN {"proposal"}
                                ELSE IF input = "proposal" /\ BugProposalAsTick
                                     THEN {"tick"}
                                     ELSE IF CertificateInput(input) /\
                                             BugCertificateAsPayload
                                          THEN {"payload"}
                                          ELSE IF input = "payload_available" /\
                                                  BugPayloadAsCertificate
                                               THEN {"certificate"}
                                               ELSE IF ValidationInput(input) /\
                                                       BugValidationAsCommitted
                                                    THEN {"committed"}
                                                    ELSE IF CommittedInput(input) /\
                                                            BugCommittedAsValidation
                                                         THEN {"validation"}
                                                         ELSE {SpecHandler(input)}

ImplementationHandlers(input) ==
  IF BugDispatchTwice
  THEN PrimaryHandlers(input) \cup {ExtraHandler(SpecHandler(input))}
  ELSE PrimaryHandlers(input)

TypeInvariant ==
  /\ BugDropTick \in BOOLEAN
  /\ BugDropProposal \in BOOLEAN
  /\ BugDropCertificate \in BOOLEAN
  /\ BugDropPayload \in BOOLEAN
  /\ BugDropValidation \in BOOLEAN
  /\ BugDropCommitted \in BOOLEAN
  /\ BugTickAsProposal \in BOOLEAN
  /\ BugProposalAsTick \in BOOLEAN
  /\ BugCertificateAsPayload \in BOOLEAN
  /\ BugPayloadAsCertificate \in BOOLEAN
  /\ BugValidationAsCommitted \in BOOLEAN
  /\ BugCommittedAsValidation \in BOOLEAN
  /\ BugDispatchTwice \in BOOLEAN
  /\ tried \subseteq Inputs
  /\ \A input \in tried:
    /\ SpecHandler(input) \in Handlers
    /\ ExtraHandler(SpecHandler(input)) \in Handlers
    /\ PrimaryHandlers(input) \subseteq Handlers
    /\ ImplementationHandlers(input) \subseteq Handlers

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

DispatchMatchesSpec ==
  \A input \in tried:
    ImplementationHandlers(input) = {SpecHandler(input)}

EveryInputDispatchesOnce ==
  \A input \in tried:
    Cardinality(ImplementationHandlers(input)) = 1

NoInputDropped ==
  \A input \in tried:
    ImplementationHandlers(input) # {}

NoCrossDispatch ==
  \A input \in tried:
    ImplementationHandlers(input) \subseteq {SpecHandler(input)}

TickDispatchesOnlyToTick ==
  "tick" \in tried =>
    ImplementationHandlers("tick") = {"tick"}

ProposalDispatchesOnlyToProposal ==
  "proposal" \in tried =>
    ImplementationHandlers("proposal") = {"proposal"}

CertificatesDispatchOnlyToCertificate ==
  \A input \in tried:
    CertificateInput(input) =>
      ImplementationHandlers(input) = {"certificate"}

PayloadDispatchesOnlyToPayload ==
  "payload_available" \in tried =>
    ImplementationHandlers("payload_available") = {"payload"}

ValidationResultsDispatchOnlyToValidation ==
  \A input \in tried:
    ValidationInput(input) =>
      ImplementationHandlers(input) = {"validation"}

CommittedBlocksDispatchOnlyToCommitted ==
  \A input \in tried:
    CommittedInput(input) =>
      ImplementationHandlers(input) = {"committed"}

ValuesStayInDomain ==
  \A input \in tried:
    /\ SpecHandler(input) \in Handlers
    /\ PrimaryHandlers(input) \subseteq Handlers
    /\ ImplementationHandlers(input) \subseteq Handlers

Safety ==
  /\ DispatchMatchesSpec
  /\ EveryInputDispatchesOnce
  /\ NoInputDropped
  /\ NoCrossDispatch
  /\ TickDispatchesOnlyToTick
  /\ ProposalDispatchesOnlyToProposal
  /\ CertificatesDispatchOnlyToCertificate
  /\ PayloadDispatchesOnlyToPayload
  /\ ValidationResultsDispatchOnlyToValidation
  /\ CommittedBlocksDispatchOnlyToCommitted
  /\ ValuesStayInDomain

====
