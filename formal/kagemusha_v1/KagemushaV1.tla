---- MODULE KagemushaV1 ----
EXTENDS Naturals, FiniteSets, Sequences

(***************************************************************************
Finite-state safety model for the sole first-release Kagemusha protocol.

Devices, identifiers, amounts, counters, and byte capacities are TLC
exploration bounds only. They are not protocol admission limits. In
particular, the real protocol carries one aggregate recursive balance and
constant-size public state; it carries no hop, ancestry, origin, fan-in,
receipt-count, transition-count, or proof-depth field.

Cryptographic digests and proofs are abstracted by identifiers. A candidate
identifier denotes the exact sealed bytes recovered after a crash. Private
predecessors occur only in the hardware-side state below, never in the
abstract public payment envelope.
***************************************************************************)

CONSTANTS
  Devices,
  CreditIds,
  IntentIds,
  TicketIds,
  RequestIds,
  VoucherIds,
  SuiteIds,
  ProfileIds,
  InitialActiveSuite,
  InitialOnlinePerDevice,
  MaxAmount,
  MaxSequence,
  InboxCapacity,
  OutboxCapacity,
  ModelDevice1,
  ModelCredit1,
  ModelCredit2,
  ModelCredit3,
  ModelCredit4,
  ModelIntent1,
  ModelIntent2,
  ModelIntent3,
  ModelIntent4,
  ModelTicket1,
  ModelTicket2,
  ModelTicket3,
  ModelTicket4,
  ModelRequest1,
  ModelProfile1,
  ModelProfile2

IntentStates ==
  {"Absent", "Offered", "Authorized", "Ticketed", "ClosedNoCommit"}
TicketStates ==
  {"Absent", "Reserved", "Locked", "Consumed", "RecoveryPending", "Released"}
PaymentPhases ==
  {"Idle", "Prepared", "Candidate", "HardwareCommitted",
   "WrapperGenerated", "Installed", "Exposed", "Recovery", "Cancelled"}
RedemptionPhases == PaymentPhases
RecoverySources ==
  {"None", "Prepared", "Candidate", "HardwareCommitted",
   "WrapperGenerated", "Installed", "Exposed"}
CreditStates == {"Absent", "Committed", "Available", "Staged", "Consumed"}
VoucherStates == {"Absent", "Pending", "Applied"}
OutboxStates == {"Absent", "Reserved", "Committed", "Released"}
SuiteStates == {"Pending", "Active", "Retained"}
ProfileStates == {"Active", "Suspended"}

(***************************************************************************
Concrete model functions are defined in the module because TLC configuration
files accept scalar model values but do not evaluate general TLA+ function
expressions. All four distinguished intent/ticket pairs target the same exact
request so TLC explores multiple independent valid payments against it.
***************************************************************************)
InitialOnline == [d \in Devices |-> InitialOnlinePerDevice]
RequestOwner == [q \in RequestIds |-> ModelDevice1]
RequestAmount == [q \in RequestIds |-> 2]
IntentRequest == [i \in IntentIds |-> ModelRequest1]
IntentAmount == [i \in IntentIds |-> RequestAmount[IntentRequest[i]]]
TicketIntent ==
  [t \in TicketIds |->
    CASE t = ModelTicket1 -> ModelIntent1
      [] t = ModelTicket2 -> ModelIntent2
      [] t = ModelTicket3 -> ModelIntent3
      [] OTHER -> ModelIntent4]
TicketRequest ==
  [t \in TicketIds |-> IntentRequest[TicketIntent[t]]]
RandomSenderCommitment(i, sender) ==
  [randomIntentNonce |-> i, committedSender |-> sender]
SenderCommitmentValues ==
  {RandomSenderCommitment(i, d) : i \in IntentIds, d \in Devices}
TicketBytes == [t \in TicketIds |-> 1]
TicketIssuedAt == [t \in TicketIds |-> 1]
TicketExpiresAt == [t \in TicketIds |-> 8]
TicketSuite == [t \in TicketIds |-> InitialActiveSuite]
TicketProfile ==
  [t \in TicketIds |->
    IF t = ModelTicket3 THEN ModelProfile2 ELSE ModelProfile1]
IntentAuthorizationProfile == [i \in IntentIds |-> ModelProfile1]
IntentAuthorizationSuite == [i \in IntentIds |-> InitialActiveSuite]
PaymentTicket ==
  [c \in CreditIds |->
    CASE c = ModelCredit1 -> ModelTicket1
      [] c = ModelCredit2 -> ModelTicket2
      [] c = ModelCredit3 -> ModelTicket3
      [] OTHER -> ModelTicket4]
PaymentOutboxBytes == [c \in CreditIds |-> 1]
PaymentSuite == [c \in CreditIds |-> InitialActiveSuite]
PaymentProfile ==
  [c \in CreditIds |->
    IF c = ModelCredit2 THEN ModelProfile2 ELSE ModelProfile1]
SenderCommitTime == [c \in CreditIds |-> 3]
RedemptionOutboxBytes == [v \in VoucherIds |-> 1]
RedemptionSuite == [v \in VoucherIds |-> InitialActiveSuite]
RedemptionProfile == [v \in VoucherIds |-> ModelProfile1]
HardwareQualified ==
  [p \in ProfileIds |-> p \in {ModelProfile1, ModelProfile2}]
NoSoftwareFallback ==
  [p \in ProfileIds |-> p \in {ModelProfile1, ModelProfile2}]
DigestValues == {"canonical-digest", "conflicting-digest"}
CanonicalEnvelopeDigest(c) == "canonical-digest"
ConflictRecordType ==
  [creditId : CreditIds, ticketId : TicketIds, digest : DigestValues]

VARIABLES
  balance,
  sequence,
  epoch,
  online,
  reserve,
  totalTopups,
  totalRedemptions,
  spentPredecessors,
  ticket,
  payment,
  redemption,
  suiteState,
  profileState,
  profilePolicyEpoch,
  paymentEvidence,
  redemptionEvidence

ledgerVars ==
  <<balance, sequence, epoch, online, reserve, totalTopups, totalRedemptions,
    spentPredecessors>>
lifecycleVars == <<suiteState, profileState, profilePolicyEpoch>>
evidenceVars == <<paymentEvidence, redemptionEvidence>>
vars == <<ledgerVars, ticket, payment, redemption, lifecycleVars, evidenceVars>>

DefaultDevice == CHOOSE d \in Devices : TRUE
DefaultCredit == CHOOSE c \in CreditIds : TRUE
DefaultIntent == CHOOSE i \in IntentIds : TRUE
DefaultRequest == CHOOSE q \in RequestIds : TRUE
DefaultPredecessor ==
  [device |-> DefaultDevice, sequence |-> 0, epoch |-> 1]

PredecessorType ==
  [device : Devices, sequence : 0..MaxSequence, epoch : 1..MaxSequence]
TicketRecordType ==
  [state : TicketStates,
   intentState : IntentStates,
   intentId : IntentIds,
   intentRequest : RequestIds,
   intentDigest : IntentIds,
   exactAmount : 0..MaxAmount,
   senderCommitment : SenderCommitmentValues,
   intentSender : Devices,
   intentPredecessor : PredecessorType,
   authorizationProfile : ProfileIds,
   authorizationSuite : SuiteIds,
   authorizationPolicyEpoch : 0..MaxSequence,
   authorizationReleaseDigest : SuiteIds,
   authorizationProof : IntentIds,
   boundCredit : CreditIds,
   policyEpoch : 0..MaxSequence]
PaymentRecordType ==
  [phase : PaymentPhases,
   recoveryFrom : RecoverySources,
   sender : Devices,
   predecessor : PredecessorType,
   policyEpoch : 0..MaxSequence,
   creditState : CreditStates,
   amount : 0..MaxAmount,
   outboxState : OutboxStates]
RedemptionRecordType ==
  [phase : RedemptionPhases,
   recoveryFrom : RecoverySources,
   owner : Devices,
   predecessor : PredecessorType,
   policyEpoch : 0..MaxSequence,
   voucherState : VoucherStates,
   amount : 0..MaxAmount,
   outboxState : OutboxStates]
PaymentEvidenceType ==
  [sealed : SUBSET CreditIds,
   candidates : SUBSET CreditIds,
   wrappers : SUBSET CreditIds,
   canonical : SUBSET CreditIds,
   exposed : SUBSET CreditIds,
   committed : SUBSET CreditIds,
   inbox : SUBSET CreditIds,
   acknowledgements : SUBSET CreditIds,
   consumed : SUBSET CreditIds,
   conflicts : SUBSET ConflictRecordType,
   expiredTickets : SUBSET TicketIds,
   noCommitRecovery : SUBSET TicketIds,
   noCommitClosures : SUBSET TicketIds]
RedemptionEvidenceType ==
  [sealed : SUBSET VoucherIds,
   candidates : SUBSET VoucherIds,
   wrappers : SUBSET VoucherIds,
   canonical : SUBSET VoucherIds,
   exposed : SUBSET VoucherIds,
   committed : SUBSET VoucherIds,
   consumedNullifiers : SUBSET VoucherIds]

RECURSIVE SumDeviceSet(_, _)
SumDeviceSet(f, S) ==
  IF S = {}
    THEN 0
    ELSE LET d == CHOOSE x \in S : TRUE
         IN f[d] + SumDeviceSet(f, S \ {d})

RECURSIVE SumCreditSet(_)
SumCreditSet(S) ==
  IF S = {}
    THEN 0
    ELSE LET c == CHOOSE x \in S : TRUE
         IN payment[c].amount + SumCreditSet(S \ {c})

RECURSIVE SumVoucherSet(_)
SumVoucherSet(S) ==
  IF S = {}
    THEN 0
    ELSE LET v == CHOOSE x \in S : TRUE
         IN redemption[v].amount + SumVoucherSet(S \ {v})

RECURSIVE SumTicketBytes(_)
SumTicketBytes(S) ==
  IF S = {}
    THEN 0
    ELSE LET t == CHOOSE x \in S : TRUE
         IN TicketBytes[t] + SumTicketBytes(S \ {t})

RECURSIVE SumCreditInboxBytes(_)
SumCreditInboxBytes(S) ==
  IF S = {}
    THEN 0
    ELSE LET c == CHOOSE x \in S : TRUE
         IN TicketBytes[PaymentTicket[c]] + SumCreditInboxBytes(S \ {c})

RECURSIVE SumPaymentOutboxBytes(_)
SumPaymentOutboxBytes(S) ==
  IF S = {}
    THEN 0
    ELSE LET c == CHOOSE x \in S : TRUE
         IN PaymentOutboxBytes[c] + SumPaymentOutboxBytes(S \ {c})

RECURSIVE SumRedemptionOutboxBytes(_)
SumRedemptionOutboxBytes(S) ==
  IF S = {}
    THEN 0
    ELSE LET v == CHOOSE x \in S : TRUE
         IN RedemptionOutboxBytes[v] + SumRedemptionOutboxBytes(S \ {v})

Predecessor(d) ==
  [device |-> d, sequence |-> sequence[d], epoch |-> epoch[d]]

PaymentRecipient(c) ==
  RequestOwner[TicketRequest[PaymentTicket[c]]]

PrivatePaymentSuccessor(c) ==
  [device |-> payment[c].sender,
   sequence |-> payment[c].predecessor.sequence + 1,
   epoch |-> payment[c].predecessor.epoch]

(***************************************************************************
The sender authorization is a proof-bearing, release-pinned envelope produced
after the intent and before ticket issuance. Its authority is selected from
the sender authorization profile, never inferred from the receiver's ticket
profile. The proof and artifact manifest are abstract digests in this model.
***************************************************************************)
AcceptanceIntentAuthorizationStatement(t) ==
  [requestId |-> ticket[t].intentRequest,
   intentDigest |-> ticket[t].intentDigest,
   exactAmount |-> ticket[t].exactAmount,
   senderCommitment |-> ticket[t].senderCommitment,
   senderProfileId |-> ticket[t].authorizationProfile,
   suiteId |-> ticket[t].authorizationSuite,
   policyEpoch |-> ticket[t].authorizationPolicyEpoch,
   releaseDigest |-> ticket[t].authorizationReleaseDigest]

AcceptanceIntentAuthorizationEnvelope(t) ==
  [statement |-> AcceptanceIntentAuthorizationStatement(t),
   pairedProof |->
     [statementDigest |-> ticket[t].intentDigest,
      proofDigest |-> ticket[t].authorizationProof],
   artifactManifest |->
     [senderProfileId |-> ticket[t].authorizationProfile,
      suiteId |-> ticket[t].authorizationSuite,
      policyEpoch |-> ticket[t].authorizationPolicyEpoch,
      releaseDigest |-> ticket[t].authorizationReleaseDigest]]

AcceptanceIntentAuthorizationBound(t) ==
  LET i == ticket[t].intentId
      p == ticket[t].authorizationProfile
      s == ticket[t].authorizationSuite
      envelope == AcceptanceIntentAuthorizationEnvelope(t)
  IN /\ p = IntentAuthorizationProfile[i]
     /\ s = IntentAuthorizationSuite[i]
     /\ ticket[t].authorizationPolicyEpoch > 0
     /\ ticket[t].authorizationReleaseDigest = s
     /\ ticket[t].authorizationProof = i
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ DOMAIN envelope = {"statement", "pairedProof", "artifactManifest"}
     /\ DOMAIN envelope.statement =
          {"requestId", "intentDigest", "exactAmount", "senderCommitment",
           "senderProfileId", "suiteId", "policyEpoch", "releaseDigest"}
     /\ DOMAIN envelope.pairedProof = {"statementDigest", "proofDigest"}
     /\ DOMAIN envelope.artifactManifest =
          {"senderProfileId", "suiteId", "policyEpoch", "releaseDigest"}
     /\ envelope.pairedProof.statementDigest = ticket[t].intentDigest
     /\ envelope.pairedProof.proofDigest = ticket[t].authorizationProof
     /\ envelope.artifactManifest.senderProfileId = p
     /\ envelope.artifactManifest.suiteId = s
     /\ envelope.artifactManifest.policyEpoch =
          ticket[t].authorizationPolicyEpoch
     /\ envelope.artifactManifest.releaseDigest =
          ticket[t].authorizationReleaseDigest

AcceptanceIntentAuthorizationValid(t) ==
  LET p == ticket[t].authorizationProfile
      s == ticket[t].authorizationSuite
  IN /\ AcceptanceIntentAuthorizationBound(t)
     /\ ticket[t].authorizationPolicyEpoch = profilePolicyEpoch[p]
     /\ profileState[p] = "Active"
     /\ suiteState[s] = "Active"

(***************************************************************************
MintAuthorization is checked before TopUp debits online value. Its statement
binds the exact recipient, typed asset incarnation, amount/context, randomized
credential commitment, ID-independent credit commitment, and recipient KEM.
Derived credit IDs and ciphertext are deliberately absent, preserving the
acyclic pre-ID authorization -> debit/mint construction order.
***************************************************************************)
AxtAssetIncarnationV1 ==
  [assetDefinitionId |-> "kagemusha-asset", incarnationId |-> 1]

ActiveSuite == CHOOSE s \in SuiteIds : suiteState[s] = "Active"
MintAuthorizationProfile(d) == ModelProfile1
MintRecipientCredentialCommitment(d) ==
  [randomCredentialNonce |-> Predecessor(d), recipientDevice |-> d]
MintCreditCommitment(d, amount) ==
  [preIdCreditCommitment |->
    [recipientDevice |-> d,
     amount |-> amount,
     aggregatePredecessor |-> Predecessor(d)]]
MintRecipientKem(d) == [recipientKem |-> d]

MintCreditBinding(d, amount) ==
  [recipientCredentialCommitment |-> MintRecipientCredentialCommitment(d),
   creditCommitment |-> MintCreditCommitment(d, amount),
   recipientKem |-> MintRecipientKem(d)]

MintAuthorizationStatement(d, amount) ==
  LET p == MintAuthorizationProfile(d)
      s == ActiveSuite
  IN [recipientAccount |-> d,
      assetIncarnation |-> AxtAssetIncarnationV1,
      amount |-> amount,
      context |-> "OnlineTopUp",
      creditBinding |-> MintCreditBinding(d, amount),
      recipientProfileId |-> p,
      suiteId |-> s,
      policyEpoch |-> profilePolicyEpoch[p],
      releaseDigest |-> s]

MintAuthorizationEnvelope(d, amount) ==
  LET statement == MintAuthorizationStatement(d, amount)
  IN [statement |-> statement,
      pairedProof |-> [statementDigest |-> statement],
      artifactManifest |->
        [recipientProfileId |-> statement.recipientProfileId,
         suiteId |-> statement.suiteId,
         policyEpoch |-> statement.policyEpoch,
         releaseDigest |-> statement.releaseDigest]]

MintAuthorizationDigest(d, amount) ==
  [mintAuthorizationDigest |-> MintAuthorizationEnvelope(d, amount)]

MintRecursiveHelperBinding(d, amount) ==
  [mintAuthorizationDigest |-> MintAuthorizationDigest(d, amount),
   creditCommitment |-> MintCreditCommitment(d, amount),
   aggregatePredecessor |-> Predecessor(d)]

MintAuthorizationShape(d, amount) ==
  LET statement == MintAuthorizationStatement(d, amount)
      binding == MintCreditBinding(d, amount)
      envelope == MintAuthorizationEnvelope(d, amount)
      helper == MintRecursiveHelperBinding(d, amount)
      forbidden ==
        {"creditId", "ciphertext", "ciphertextDigest", "encryptedCredit"}
  IN /\ DOMAIN statement =
          {"recipientAccount", "assetIncarnation", "amount", "context",
           "creditBinding", "recipientProfileId", "suiteId", "policyEpoch",
           "releaseDigest"}
     /\ DOMAIN binding =
          {"recipientCredentialCommitment", "creditCommitment", "recipientKem"}
     /\ DOMAIN binding.creditCommitment = {"preIdCreditCommitment"}
     /\ DOMAIN envelope = {"statement", "pairedProof", "artifactManifest"}
     /\ DOMAIN envelope.artifactManifest =
          {"recipientProfileId", "suiteId", "policyEpoch", "releaseDigest"}
     /\ forbidden \cap DOMAIN statement = {}
     /\ forbidden \cap DOMAIN binding = {}
     /\ forbidden \cap DOMAIN binding.creditCommitment = {}
     /\ statement.recipientAccount = d
     /\ statement.assetIncarnation = AxtAssetIncarnationV1
     /\ statement.amount = amount
     /\ statement.creditBinding = MintCreditBinding(d, amount)
     /\ helper.mintAuthorizationDigest = MintAuthorizationDigest(d, amount)
     /\ helper.creditCommitment = statement.creditBinding.creditCommitment

MintAuthorizationValid(d, amount) ==
  LET p == MintAuthorizationProfile(d)
      s == ActiveSuite
  IN /\ MintAuthorizationShape(d, amount)
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ suiteState[s] = "Active"

LifecycleBindingFields ==
  {"networkId", "protocolVersion", "suiteId", "vkDigest", "assetId",
   "assetIncarnation", "assetScale", "hardwareProfileId", "policyEpoch",
   "credentialExpiry", "laneCommitment", "hardwareEpoch", "operationKind",
   "requestId", "acceptanceIntentAuthorizationDigest", "acceptanceTicketId",
   "creditId", "ciphertextDigest",
   "predecessorStateHead", "successorStateHead"}

PaymentLifecycleBinding(c) ==
  LET t == PaymentTicket[c]
  IN [networkId |-> "kagemusha-v1-network",
      protocolVersion |-> 1,
      suiteId |-> PaymentSuite[c],
      vkDigest |-> PaymentSuite[c],
      assetId |-> "kagemusha-asset",
      assetIncarnation |-> AxtAssetIncarnationV1,
      assetScale |-> 2,
      hardwareProfileId |-> PaymentProfile[c],
      policyEpoch |-> payment[c].policyEpoch,
      credentialExpiry |-> TicketExpiresAt[t],
      laneCommitment |-> payment[c].predecessor,
      hardwareEpoch |-> payment[c].predecessor.epoch,
      operationKind |-> "PeerPayment",
      requestId |-> TicketRequest[t],
      acceptanceIntentAuthorizationDigest |->
        AcceptanceIntentAuthorizationEnvelope(t),
      acceptanceTicketId |-> t,
      creditId |-> c,
      ciphertextDigest |-> CanonicalEnvelopeDigest(c),
      predecessorStateHead |-> payment[c].predecessor,
      successorStateHead |-> PrivatePaymentSuccessor(c)]

PaymentCandidateRecord(c) ==
  LET t == PaymentTicket[c]
  IN [candidateDigest |-> c,
      lifecycleBinding |-> PaymentLifecycleBinding(c),
      sealedAmount |-> payment[c].amount,
      privateIntentOpening |->
        [intentId |-> ticket[t].intentId,
         sender |-> ticket[t].intentSender,
         predecessor |-> ticket[t].intentPredecessor]]

(***************************************************************************
The canonical terminal body is constructed first and deliberately contains no
certificate ID. Only then is its digest computed, and only that digest derives
the terminal certificate ID. This functional order models a self-free preimage.
***************************************************************************)
TerminalBodyFieldOrder ==
  <<"candidateDigest", "lifecycleBindingDigest", "commitEvidence">>

PaymentTerminalCertificateBody(c) ==
  [canonicalFieldOrder |-> TerminalBodyFieldOrder,
   candidateDigest |-> PaymentCandidateRecord(c).candidateDigest,
   lifecycleBindingDigest |-> c,
   commitEvidence |-> c]

PaymentTerminalCertificateBodyDigest(c) ==
  [terminalBodyDigest |-> PaymentTerminalCertificateBody(c)]

PaymentTerminalCertificateId(c) ==
  [terminalCertificateId |-> PaymentTerminalCertificateBodyDigest(c)]

PaymentCommitCertificate(c) ==
  [terminalBody |-> PaymentTerminalCertificateBody(c),
   terminalBodyDigest |-> PaymentTerminalCertificateBodyDigest(c),
   terminalCertificateId |-> PaymentTerminalCertificateId(c)]

PublicAcceptanceIntent(t) ==
  [requestId |-> ticket[t].intentRequest,
   intentId |-> ticket[t].intentId,
   exactAmount |-> ticket[t].exactAmount,
   senderCommitment |-> ticket[t].senderCommitment]

PublicAcceptanceTicket(t) ==
  [requestId |-> TicketRequest[t],
   acceptanceTicketId |-> t,
   intentDigest |-> ticket[t].intentDigest,
   exactAmount |-> ticket[t].exactAmount]

AllowedPublicPaymentFields ==
  {"transitionNullifier", "acceptanceIntent", "acceptanceTicket",
   "recipientOneTimeKey", "amountCiphertextCommitment",
   "hardwareProfileId", "policyEpoch", "commitEvidence",
   "proof", "commitCertificate"}

PublicPayment(c) ==
  LET t == PaymentTicket[c]
  IN [transitionNullifier |-> c,
      acceptanceIntent |-> PublicAcceptanceIntent(t),
      acceptanceTicket |-> PublicAcceptanceTicket(t),
      recipientOneTimeKey |-> t,
      amountCiphertextCommitment |-> CanonicalEnvelopeDigest(c),
      hardwareProfileId |-> PaymentProfile[c],
      policyEpoch |-> payment[c].policyEpoch,
      commitEvidence |-> c,
      proof |-> c,
      commitCertificate |-> PaymentCommitCertificate(c)]

PaymentEnvelopeRecord(c) ==
  [candidateDigest |-> PaymentCandidateRecord(c).candidateDigest,
   lifecycleBindingDigest |-> c,
   publicPayment |-> PublicPayment(c)]

PrivateRedemptionSuccessor(v) ==
  [device |-> redemption[v].owner,
   sequence |-> redemption[v].predecessor.sequence + 1,
   epoch |-> redemption[v].predecessor.epoch]

RedemptionLifecycleBinding(v) ==
  [networkId |-> "kagemusha-v1-network",
   protocolVersion |-> 1,
   suiteId |-> RedemptionSuite[v],
   vkDigest |-> RedemptionSuite[v],
   assetId |-> "kagemusha-asset",
   assetIncarnation |-> AxtAssetIncarnationV1,
   assetScale |-> 2,
   hardwareProfileId |-> RedemptionProfile[v],
   policyEpoch |-> redemption[v].policyEpoch,
   laneCommitment |-> redemption[v].predecessor,
   hardwareEpoch |-> redemption[v].predecessor.epoch,
   operationKind |-> "Redemption",
   voucherId |-> v,
   ciphertextDigest |-> v,
   predecessorStateHead |-> redemption[v].predecessor,
   successorStateHead |-> PrivateRedemptionSuccessor(v)]

ReservedTicketsFor(d) ==
  {t \in TicketIds :
     ticket[t].state \in {"Reserved", "Locked", "RecoveryPending"}
       /\ RequestOwner[TicketRequest[t]] = d}
StagedCreditsFor(d) ==
  {c \in CreditIds :
     payment[c].creditState = "Staged" /\ PaymentRecipient(c) = d}
InboxUse(d) ==
  SumTicketBytes(ReservedTicketsFor(d))
    + SumCreditInboxBytes(StagedCreditsFor(d))

PaymentAmountAllowed(t, amount) ==
  /\ amount \in 1..MaxAmount
  /\ amount = ticket[t].exactAmount
  /\ amount = RequestAmount[TicketRequest[t]]

NoPaymentStartedForTicket(t) ==
  \A c \in CreditIds :
    PaymentTicket[c] = t => payment[c].phase = "Idle"

CancelledBoundPayment(t) ==
  LET c == ticket[t].boundCredit
  IN /\ PaymentTicket[c] = t
     /\ payment[c].phase = "Cancelled"
     /\ payment[c].creditState = "Absent"
     /\ c \notin paymentEvidence.committed

StagedCreditValueFits(c) ==
  /\ payment[c].creditState = "Staged"
  /\ balance[PaymentRecipient(c)] + payment[c].amount <= MaxAmount

StagedCreditCanEnterFold(c) ==
  LET d == PaymentRecipient(c)
  IN /\ StagedCreditValueFits(c)
     /\ c \in StagedCreditsFor(d)
     /\ c \notin paymentEvidence.consumed

LivePaymentOutboxes(d) ==
  {c \in CreditIds :
     payment[c].sender = d
       /\ payment[c].outboxState \in {"Reserved", "Committed"}}
LiveRedemptionOutboxes(d) ==
  {v \in VoucherIds :
     redemption[v].owner = d
       /\ redemption[v].outboxState \in {"Reserved", "Committed"}}
OutboxUse(d) ==
  SumPaymentOutboxBytes(LivePaymentOutboxes(d))
    + SumRedemptionOutboxBytes(LiveRedemptionOutboxes(d))

PaymentHoldsPredecessor(c) ==
  \/ payment[c].phase \in {"Prepared", "Candidate"}
  \/ /\ payment[c].phase = "Recovery"
     /\ payment[c].recoveryFrom \in {"Prepared", "Candidate"}
RedemptionHoldsPredecessor(v) ==
  \/ redemption[v].phase \in {"Prepared", "Candidate"}
  \/ /\ redemption[v].phase = "Recovery"
     /\ redemption[v].recoveryFrom \in {"Prepared", "Candidate"}
LockedPredecessors ==
  {payment[c].predecessor :
     c \in {x \in CreditIds : PaymentHoldsPredecessor(x)}}
    \cup
  {redemption[v].predecessor :
     v \in {x \in VoucherIds : RedemptionHoldsPredecessor(x)}}
    \cup
  {ticket[t].intentPredecessor :
     t \in {x \in TicketIds : ticket[x].state = "RecoveryPending"}}

LiveCredits ==
  {c \in CreditIds :
     payment[c].creditState \in {"Committed", "Available", "Staged"}}
PendingVouchers ==
  {v \in VoucherIds : redemption[v].voucherState = "Pending"}
OfflineLiability ==
  SumDeviceSet(balance, Devices)
    + SumCreditSet(LiveCredits)
    + SumVoucherSet(PendingVouchers)
InitialOnlineTotal == SumDeviceSet(InitialOnline, Devices)

CanStage(c) ==
  LET t == PaymentTicket[c]
  IN /\ c \in paymentEvidence.committed
     /\ c \in paymentEvidence.canonical
     /\ c \in paymentEvidence.exposed
     /\ \/ payment[c].phase = "Exposed"
        \/ /\ payment[c].phase = "Recovery"
           /\ payment[c].recoveryFrom = "Exposed"
     /\ payment[c].creditState = "Available"
     /\ payment[c].outboxState = "Committed"
     /\ ticket[t].state = "Locked"
     /\ ticket[t].intentState = "Ticketed"
     /\ ticket[t].boundCredit = c
     /\ payment[c].amount = ticket[t].exactAmount
     /\ payment[c].sender = ticket[t].intentSender
     /\ payment[c].predecessor = ticket[t].intentPredecessor

Init ==
  /\ Devices # {}
  /\ CreditIds # {}
  /\ IntentIds # {}
  /\ TicketIds # {}
  /\ RequestIds # {}
  /\ VoucherIds # {}
  /\ SuiteIds # {}
  /\ ProfileIds # {}
  /\ InitialActiveSuite \in SuiteIds
  /\ MaxAmount \in Nat \ {0}
  /\ MaxSequence \in Nat \ {0}
  /\ InboxCapacity \in Nat \ {0}
  /\ OutboxCapacity \in Nat \ {0}
  /\ InitialOnlinePerDevice \in 0..MaxAmount
  /\ ModelDevice1 \in Devices
  /\ {ModelCredit1, ModelCredit2, ModelCredit3, ModelCredit4}
       \subseteq CreditIds
  /\ Cardinality({ModelCredit1, ModelCredit2, ModelCredit3, ModelCredit4}) = 4
  /\ {ModelIntent1, ModelIntent2, ModelIntent3, ModelIntent4}
       \subseteq IntentIds
  /\ Cardinality({ModelIntent1, ModelIntent2, ModelIntent3, ModelIntent4}) = 4
  /\ {ModelTicket1, ModelTicket2, ModelTicket3, ModelTicket4}
       \subseteq TicketIds
  /\ Cardinality({ModelTicket1, ModelTicket2, ModelTicket3, ModelTicket4}) = 4
  /\ ModelRequest1 \in RequestIds
  /\ {ModelProfile1, ModelProfile2} \subseteq ProfileIds
  /\ ModelProfile1 # ModelProfile2
  /\ InitialOnline \in [Devices -> 0..MaxAmount]
  /\ InitialOnlineTotal <= MaxAmount
  /\ RequestOwner \in [RequestIds -> Devices]
  /\ RequestAmount \in [RequestIds -> 1..MaxAmount]
  /\ IntentRequest \in [IntentIds -> RequestIds]
  /\ IntentAmount \in [IntentIds -> 1..MaxAmount]
  /\ IntentAuthorizationProfile \in [IntentIds -> ProfileIds]
  /\ IntentAuthorizationSuite \in [IntentIds -> SuiteIds]
  /\ TicketIntent \in [TicketIds -> IntentIds]
  /\ \A t1 \in TicketIds, t2 \in TicketIds :
       TicketIntent[t1] = TicketIntent[t2] => t1 = t2
  /\ TicketRequest \in [TicketIds -> RequestIds]
  /\ TicketBytes \in [TicketIds -> 1..InboxCapacity]
  /\ TicketIssuedAt \in [TicketIds -> Nat]
  /\ TicketExpiresAt \in [TicketIds -> Nat]
  /\ TicketSuite \in [TicketIds -> SuiteIds]
  /\ TicketProfile \in [TicketIds -> ProfileIds]
  /\ PaymentTicket \in [CreditIds -> TicketIds]
  /\ PaymentOutboxBytes \in [CreditIds -> 1..OutboxCapacity]
  /\ PaymentSuite \in [CreditIds -> SuiteIds]
  /\ PaymentProfile \in [CreditIds -> ProfileIds]
  /\ SenderCommitTime \in [CreditIds -> Nat]
  /\ RedemptionOutboxBytes \in [VoucherIds -> 1..OutboxCapacity]
  /\ RedemptionSuite \in [VoucherIds -> SuiteIds]
  /\ RedemptionProfile \in [VoucherIds -> ProfileIds]
  /\ HardwareQualified \in [ProfileIds -> BOOLEAN]
  /\ NoSoftwareFallback \in [ProfileIds -> BOOLEAN]
  /\ balance = [d \in Devices |-> 0]
  /\ sequence = [d \in Devices |-> 0]
  /\ epoch = [d \in Devices |-> 1]
  /\ online = InitialOnline
  /\ reserve = 0
  /\ totalTopups = 0
  /\ totalRedemptions = 0
  /\ spentPredecessors = {}
  /\ ticket =
       [t \in TicketIds |->
         [state |-> "Absent",
          intentState |-> "Absent",
          intentId |-> DefaultIntent,
          intentRequest |-> DefaultRequest,
          intentDigest |-> DefaultIntent,
          exactAmount |-> 0,
          senderCommitment |->
            RandomSenderCommitment(DefaultIntent, DefaultDevice),
          intentSender |-> DefaultDevice,
          intentPredecessor |-> DefaultPredecessor,
          authorizationProfile |-> ModelProfile1,
          authorizationSuite |-> InitialActiveSuite,
          authorizationPolicyEpoch |-> 0,
          authorizationReleaseDigest |-> InitialActiveSuite,
          authorizationProof |-> DefaultIntent,
          boundCredit |-> DefaultCredit,
          policyEpoch |-> 0]]
  /\ payment =
       [c \in CreditIds |->
         [phase |-> "Idle",
          recoveryFrom |-> "None",
          sender |-> DefaultDevice,
          predecessor |-> DefaultPredecessor,
          policyEpoch |-> 0,
          creditState |-> "Absent",
          amount |-> 0,
          outboxState |-> "Absent"]]
  /\ redemption =
       [v \in VoucherIds |->
         [phase |-> "Idle",
          recoveryFrom |-> "None",
          owner |-> DefaultDevice,
          predecessor |-> DefaultPredecessor,
          policyEpoch |-> 0,
          voucherState |-> "Absent",
          amount |-> 0,
          outboxState |-> "Absent"]]
  /\ suiteState =
       [s \in SuiteIds |->
         IF s = InitialActiveSuite THEN "Active" ELSE "Pending"]
  /\ profileState = [p \in ProfileIds |-> "Active"]
  /\ profilePolicyEpoch = [p \in ProfileIds |-> 1]
  /\ paymentEvidence =
       [sealed |-> {},
        candidates |-> {},
        wrappers |-> {},
        canonical |-> {},
        exposed |-> {},
        committed |-> {},
        inbox |-> {},
        acknowledgements |-> {},
        consumed |-> {},
        conflicts |-> {},
        expiredTickets |-> {},
        noCommitRecovery |-> {},
        noCommitClosures |-> {}]
  /\ redemptionEvidence =
       [sealed |-> {},
        candidates |-> {},
        wrappers |-> {},
        canonical |-> {},
        exposed |-> {},
        committed |-> {},
        consumedNullifiers |-> {}]

(***************************************************************************
Online top-up enters the one global asset reserve and advances the exact-next
aggregate state. It is shown atomically because this model concentrates its
crash boundary exploration on offline sends and redemptions.
***************************************************************************)
TopUp(d, amount) ==
  /\ d \in Devices
  /\ amount \in 1..MaxAmount
  /\ MintAuthorizationValid(d, amount)
  /\ online[d] >= amount
  /\ balance[d] + amount <= MaxAmount
  /\ reserve + amount <= MaxAmount
  /\ totalTopups + amount <= MaxAmount
  /\ sequence[d] < MaxSequence
  /\ Predecessor(d) \notin spentPredecessors
  /\ Predecessor(d) \notin LockedPredecessors
  /\ balance' = [balance EXCEPT ![d] = @ + amount]
  /\ sequence' = [sequence EXCEPT ![d] = @ + 1]
  /\ online' = [online EXCEPT ![d] = @ - amount]
  /\ reserve' = reserve + amount
  /\ totalTopups' = totalTopups + amount
  /\ spentPredecessors' = spentPredecessors \cup {Predecessor(d)}
  /\ UNCHANGED <<epoch, totalRedemptions, ticket, payment, redemption,
                  lifecycleVars, evidenceVars>>

(***************************************************************************
The sender first creates one unique acceptance intent with an exact positive
amount and an opaque randomized commitment to its private sender/predecessor
opening. This does not yet reserve receiver bytes.
The private opening fields below are specification ghosts and never enter the
public intent or ticket records.
***************************************************************************)
CreateAcceptanceIntent(sender, t) ==
  LET i == TicketIntent[t]
      q == TicketRequest[t]
  IN /\ sender \in Devices
     /\ t \in TicketIds
     /\ sender # RequestOwner[q]
     /\ ticket[t].state = "Absent"
     /\ ticket[t].intentState = "Absent"
     /\ IntentRequest[i] = q
     /\ IntentAmount[i] = RequestAmount[q]
     /\ Predecessor(sender) \notin spentPredecessors
     /\ Predecessor(sender) \notin LockedPredecessors
     /\ ticket' =
          [ticket EXCEPT
            ![t].intentState = "Offered",
            ![t].intentId = i,
            ![t].intentRequest = q,
            ![t].intentDigest = i,
            ![t].exactAmount = IntentAmount[i],
            ![t].senderCommitment = RandomSenderCommitment(i, sender),
            ![t].intentSender = sender,
            ![t].intentPredecessor = Predecessor(sender)]
     /\ UNCHANGED <<ledgerVars, payment, redemption, lifecycleVars,
                     evidenceVars>>

(***************************************************************************
This explicit pre-ticket step verifies the proof-bearing sender authorization
against its own profile, suite, policy epoch, and release. TicketProfile[t] is
the receiver authority and is intentionally not used as the sender authority.
***************************************************************************)
AuthorizeAcceptanceIntent(t) ==
  LET i == ticket[t].intentId
      p == IntentAuthorizationProfile[i]
      s == IntentAuthorizationSuite[i]
  IN /\ t \in TicketIds
     /\ ticket[t].state = "Absent"
     /\ ticket[t].intentState = "Offered"
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ suiteState[s] = "Active"
     /\ ticket' =
          [ticket EXCEPT
            ![t].intentState = "Authorized",
            ![t].authorizationProfile = p,
            ![t].authorizationSuite = s,
            ![t].authorizationPolicyEpoch = profilePolicyEpoch[p],
            ![t].authorizationReleaseDigest = s,
            ![t].authorizationProof = i]
     /\ UNCHANGED <<ledgerVars, payment, redemption, lifecycleVars,
                     evidenceVars>>

(***************************************************************************
Issuance atomically consumes the unique intent and reserves receiver bytes.
Every distinct valid intent for the same request may receive an exact-amount
ticket; there is no invoice-level amount or count ledger. Ticket IDs remain
one-use, and hardware/durable capacity must be reserved for every payment.
***************************************************************************)
IssueAcceptanceTicket(t) ==
  LET q == TicketRequest[t]
      d == RequestOwner[q]
      p == TicketProfile[t]
      s == TicketSuite[t]
  IN /\ t \in TicketIds
     /\ ticket[t].state = "Absent"
     /\ ticket[t].intentState = "Authorized"
     /\ AcceptanceIntentAuthorizationValid(t)
     /\ ticket[t].intentId = TicketIntent[t]
     /\ ticket[t].intentRequest = q
     /\ ticket[t].intentDigest = ticket[t].intentId
     /\ ticket[t].senderCommitment =
          RandomSenderCommitment(
            ticket[t].intentId, ticket[t].intentSender)
     /\ ticket[t].exactAmount = RequestAmount[q]
     /\ TicketIssuedAt[t] < TicketExpiresAt[t]
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ suiteState[s] = "Active"
     /\ InboxUse(d) + TicketBytes[t] <= InboxCapacity
     /\ ticket' =
          [ticket EXCEPT
            ![t].state = "Reserved",
            ![t].intentState = "Ticketed",
            ![t].policyEpoch = profilePolicyEpoch[p]]
     /\ UNCHANGED <<ledgerVars, payment, redemption, lifecycleVars,
                     evidenceVars>>

(***************************************************************************
Send: prepare seals the exact predecessor, inputs, and randomness and reserves
sender outbox bytes. Candidate persistence precedes the atomic hardware
commit. Wrapper generation, canonical installation, and exposure are separate
crash boundaries; recovery resumes the same c-bound bytes and cannot create
another successor.
***************************************************************************)
PreparePayment(c) ==
  LET t == PaymentTicket[c]
      sender == ticket[t].intentSender
      amount == ticket[t].exactAmount
      recipient == PaymentRecipient(c)
      senderProfile == PaymentProfile[c]
      receiverProfile == TicketProfile[t]
      s == PaymentSuite[c]
  IN /\ sender \in Devices
     /\ c \in CreditIds
     /\ sender # recipient
     /\ payment[c].phase = "Idle"
     /\ payment[c].creditState = "Absent"
     /\ ticket[t].state = "Reserved"
     /\ ticket[t].intentState = "Ticketed"
     /\ PaymentAmountAllowed(t, amount)
     /\ PaymentSuite[c] = TicketSuite[t]
     /\ HardwareQualified[senderProfile]
     /\ NoSoftwareFallback[senderProfile]
     /\ HardwareQualified[receiverProfile]
     /\ NoSoftwareFallback[receiverProfile]
     /\ profileState[senderProfile] = "Active"
     /\ profileState[receiverProfile] = "Active"
     /\ suiteState[s] = "Active"
     /\ balance[sender] >= amount
     /\ StagedCreditsFor(sender) = {}
     /\ sequence[sender] < MaxSequence
     /\ ticket[t].intentPredecessor = Predecessor(sender)
     /\ Predecessor(sender) \notin spentPredecessors
     /\ Predecessor(sender) \notin LockedPredecessors
     /\ OutboxUse(sender) + PaymentOutboxBytes[c] <= OutboxCapacity
     /\ ticket' =
          [ticket EXCEPT
            ![t].state = "Locked",
            ![t].boundCredit = c]
     /\ payment' =
          [payment EXCEPT
            ![c].phase = "Prepared",
            ![c].recoveryFrom = "None",
            ![c].sender = sender,
            ![c].predecessor = Predecessor(sender),
            ![c].policyEpoch = profilePolicyEpoch[senderProfile],
            ![c].amount = amount,
            ![c].outboxState = "Reserved"]
     /\ paymentEvidence' =
          [paymentEvidence EXCEPT !.sealed = @ \cup {c}]
     /\ UNCHANGED <<ledgerVars, redemption, lifecycleVars, redemptionEvidence>>

ProveAndPersistPaymentCandidate(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Prepared"
  /\ c \in paymentEvidence.sealed
  /\ payment' = [payment EXCEPT ![c].phase = "Candidate"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.candidates = @ \cup {c}]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars,
                  redemptionEvidence>>

HardwareCommitPayment(c) ==
  LET d == payment[c].sender
      t == PaymentTicket[c]
      p == PaymentProfile[c]
  IN /\ c \in CreditIds
     /\ payment[c].phase = "Candidate"
     /\ payment[c].creditState = "Absent"
     /\ payment[c].outboxState = "Reserved"
     /\ c \in paymentEvidence.sealed
     /\ c \in paymentEvidence.candidates
     /\ ticket[t].state = "Locked"
     /\ ticket[t].intentState = "Ticketed"
     /\ ticket[t].boundCredit = c
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ SenderCommitTime[c] >= TicketIssuedAt[t]
     /\ SenderCommitTime[c] < TicketExpiresAt[t]
     /\ payment[c].amount = ticket[t].exactAmount
     /\ payment[c].sender = ticket[t].intentSender
     /\ payment[c].predecessor = ticket[t].intentPredecessor
     /\ payment[c].predecessor = Predecessor(d)
     /\ Predecessor(d) \notin spentPredecessors
     /\ balance[d] >= payment[c].amount
     /\ sequence[d] < MaxSequence
     /\ balance' = [balance EXCEPT ![d] = @ - payment[c].amount]
     /\ sequence' = [sequence EXCEPT ![d] = @ + 1]
     /\ spentPredecessors' = spentPredecessors \cup {Predecessor(d)}
     /\ payment' =
          [payment EXCEPT
            ![c].phase = "HardwareCommitted",
            ![c].creditState = "Committed",
            ![c].outboxState = "Committed"]
     /\ paymentEvidence' =
          [paymentEvidence EXCEPT !.committed = @ \cup {c}]
     /\ UNCHANGED <<epoch, online, reserve, totalTopups, totalRedemptions,
                     ticket, redemption, lifecycleVars, redemptionEvidence>>

GeneratePaymentWrapper(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "HardwareCommitted"
  /\ payment[c].creditState = "Committed"
  /\ c \in paymentEvidence.candidates
  /\ payment' =
       [payment EXCEPT ![c].phase = "WrapperGenerated"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.wrappers = @ \cup {c}]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars,
                  redemptionEvidence>>

InstallPaymentEnvelope(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "WrapperGenerated"
  /\ payment[c].creditState = "Committed"
  /\ c \in paymentEvidence.wrappers
  /\ payment' = [payment EXCEPT ![c].phase = "Installed"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.canonical = @ \cup {c}]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars,
                  redemptionEvidence>>

ExposePaymentEnvelope(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Installed"
  /\ payment[c].creditState = "Committed"
  /\ c \in paymentEvidence.canonical
  /\ payment' =
       [payment EXCEPT
         ![c].phase = "Exposed",
         ![c].creditState = "Available"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.exposed = @ \cup {c}]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars,
                  redemptionEvidence>>

CrashPayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase \in
       {"Prepared", "Candidate", "HardwareCommitted",
        "WrapperGenerated", "Installed", "Exposed"}
  /\ payment' =
       [payment EXCEPT
         ![c].recoveryFrom = payment[c].phase,
         ![c].phase = "Recovery"]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars, evidenceVars>>

ResumePrecommitPayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Recovery"
  /\ payment[c].recoveryFrom \in {"Prepared", "Candidate"}
  /\ payment' =
       [payment EXCEPT
         ![c].phase = payment[c].recoveryFrom,
         ![c].recoveryFrom = "None"]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars, evidenceVars>>

ResumeCommittedPayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Recovery"
  /\ payment[c].recoveryFrom \in
       {"HardwareCommitted", "WrapperGenerated", "Installed", "Exposed"}
  /\ c \in paymentEvidence.candidates
  /\ c \in paymentEvidence.committed
  /\ payment' =
       [payment EXCEPT
         ![c].phase = payment[c].recoveryFrom,
         ![c].recoveryFrom = "None"]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars, evidenceVars>>

(***************************************************************************
Expiry is only observed evidence. It changes no ticket, intent, or capacity
state and therefore cannot reclaim anything by itself.
***************************************************************************)
ObserveTicketExpiry(t) ==
  /\ t \in TicketIds
  /\ ticket[t].state # "Absent"
  /\ t \notin paymentEvidence.expiredTickets
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.expiredTickets = @ \cup {t}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, lifecycleVars,
                  redemptionEvidence>>

(***************************************************************************
These actions abstract an authenticated proof that the bound sender intent did
not reach terminal hardware commit. Opening recovery preserves the ticket's
receiver bytes in RecoveryPending. Only the distinct closure action may release
physical capacity. Intent and ticket identities remain closed forever, and a
consumed ticket can never enter this corridor.
***************************************************************************)
BeginPaymentNoCommitRecovery(c) ==
  LET t == PaymentTicket[c]
  IN /\ c \in CreditIds
     /\ \/ payment[c].phase \in {"Prepared", "Candidate"}
        \/ /\ payment[c].phase = "Recovery"
           /\ payment[c].recoveryFrom \in {"Prepared", "Candidate"}
     /\ payment[c].creditState = "Absent"
     /\ payment[c].outboxState = "Reserved"
     /\ ticket[t].state = "Locked"
     /\ ticket[t].intentState = "Ticketed"
     /\ ticket[t].boundCredit = c
     /\ ticket[t].intentPredecessor \notin spentPredecessors
     /\ ticket' = [ticket EXCEPT ![t].state = "RecoveryPending"]
     /\ payment' =
          [payment EXCEPT
            ![c].phase = "Cancelled",
            ![c].recoveryFrom = "None",
            ![c].outboxState = "Released"]
     /\ paymentEvidence' =
          [paymentEvidence EXCEPT !.noCommitRecovery = @ \cup {t}]
     /\ UNCHANGED <<ledgerVars, redemption, lifecycleVars,
                     redemptionEvidence>>

BeginUnusedTicketNoCommitRecovery(t) ==
  /\ t \in TicketIds
  /\ ticket[t].state = "Reserved"
  /\ ticket[t].intentState = "Ticketed"
  /\ NoPaymentStartedForTicket(t)
  /\ ticket[t].intentPredecessor \notin spentPredecessors
  /\ ticket[t].intentPredecessor \notin LockedPredecessors
  /\ ticket' = [ticket EXCEPT ![t].state = "RecoveryPending"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.noCommitRecovery = @ \cup {t}]
  /\ UNCHANGED <<ledgerVars, payment, redemption, lifecycleVars,
                  redemptionEvidence>>

CloseAuthenticatedNoCommitRecovery(t) ==
  /\ t \in TicketIds
  /\ ticket[t].state = "RecoveryPending"
  /\ ticket[t].intentState = "Ticketed"
  /\ t \in paymentEvidence.noCommitRecovery
  /\ t \notin paymentEvidence.noCommitClosures
  /\ ticket[t].intentPredecessor \notin spentPredecessors
  /\ \A c \in paymentEvidence.committed : PaymentTicket[c] # t
  /\ \/ NoPaymentStartedForTicket(t)
     \/ CancelledBoundPayment(t)
  /\ ticket' =
       [ticket EXCEPT
         ![t].state = "Released",
         ![t].intentState = "ClosedNoCommit"]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT !.noCommitClosures = @ \cup {t}]
  /\ UNCHANGED <<ledgerVars, payment, redemption, lifecycleVars,
                  redemptionEvidence>>

(***************************************************************************
No capacity, expiry, suite, profile, or policy check appears here. The locked
ticket owns the exact receiver bytes, so every sender-committed canonical
payment is stageable even after later traffic, suite rotation, or suspension.
The acknowledgement is made durable in the same abstract transition.
***************************************************************************)
StagePayment(c) ==
  LET t == PaymentTicket[c]
  IN /\ c \in CreditIds
     /\ CanStage(c)
     /\ ticket' = [ticket EXCEPT ![t].state = "Consumed"]
     /\ payment' =
          [payment EXCEPT
            ![c].phase = "Exposed",
            ![c].recoveryFrom = "None",
            ![c].creditState = "Staged"]
     /\ paymentEvidence' =
          [paymentEvidence EXCEPT
            !.inbox = @ \cup {c},
            !.acknowledgements = @ \cup {c}]
     /\ UNCHANGED <<ledgerVars, redemption, lifecycleVars,
                     redemptionEvidence>>

ObserveAcknowledgement(c) ==
  /\ c \in CreditIds
  /\ c \in paymentEvidence.acknowledgements
  /\ payment[c].outboxState = "Committed"
  /\ payment' =
       [payment EXCEPT ![c].outboxState = "Released"]
  /\ UNCHANGED <<ledgerVars, ticket, redemption, lifecycleVars, evidenceVars>>

RejectConflictingPayment(c, claimedDigest) ==
  /\ c \in CreditIds
  /\ claimedDigest \in DigestValues
  /\ c \in paymentEvidence.canonical
  /\ claimedDigest # CanonicalEnvelopeDigest(c)
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT
         !.conflicts =
           @ \cup
             {[creditId |-> c,
               ticketId |-> PaymentTicket[c],
               digest |-> claimedDigest]}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, lifecycleVars,
                  redemptionEvidence>>

(* One fixed-shape receive transition consumes exactly one staged credit. *)
FoldReceive(d, credit) ==
  /\ d \in Devices
  /\ credit \in StagedCreditsFor(d)
  /\ credit \notin paymentEvidence.consumed
  /\ balance[d] + payment[credit].amount <= MaxAmount
  /\ sequence[d] < MaxSequence
  /\ Predecessor(d) \notin spentPredecessors
  /\ Predecessor(d) \notin LockedPredecessors
  /\ balance' =
       [balance EXCEPT
         ![d] = @ + payment[credit].amount]
  /\ sequence' = [sequence EXCEPT ![d] = @ + 1]
  /\ spentPredecessors' = spentPredecessors \cup {Predecessor(d)}
  /\ payment' =
       [c \in CreditIds |->
         IF c = credit
           THEN [payment[c] EXCEPT !.creditState = "Consumed"]
           ELSE payment[c]]
  /\ paymentEvidence' =
       [paymentEvidence EXCEPT
         !.consumed = @ \cup {credit}]
  /\ UNCHANGED <<epoch, online, reserve, totalTopups, totalRedemptions,
                  ticket, redemption, lifecycleVars, redemptionEvidence>>

(***************************************************************************
Redemption uses the same prepare/candidate/hardware-commit/finalize/recovery
shape and shares the sender outbox capacity pool with payments.
***************************************************************************)
PrepareRedemption(d, v, amount) ==
  LET p == RedemptionProfile[v]
      s == RedemptionSuite[v]
  IN /\ d \in Devices
     /\ v \in VoucherIds
     /\ amount \in 1..MaxAmount
     /\ redemption[v].phase = "Idle"
     /\ redemption[v].voucherState = "Absent"
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ suiteState[s] = "Active"
     /\ balance[d] >= amount
     /\ StagedCreditsFor(d) = {}
     /\ sequence[d] < MaxSequence
     /\ Predecessor(d) \notin spentPredecessors
     /\ Predecessor(d) \notin LockedPredecessors
     /\ OutboxUse(d) + RedemptionOutboxBytes[v] <= OutboxCapacity
     /\ redemption' =
          [redemption EXCEPT
            ![v].phase = "Prepared",
            ![v].recoveryFrom = "None",
            ![v].owner = d,
            ![v].predecessor = Predecessor(d),
            ![v].policyEpoch = profilePolicyEpoch[p],
            ![v].amount = amount,
            ![v].outboxState = "Reserved"]
     /\ redemptionEvidence' =
          [redemptionEvidence EXCEPT !.sealed = @ \cup {v}]
     /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars,
                     paymentEvidence>>

ProveAndPersistRedemptionCandidate(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Prepared"
  /\ v \in redemptionEvidence.sealed
  /\ redemption' = [redemption EXCEPT ![v].phase = "Candidate"]
  /\ redemptionEvidence' =
       [redemptionEvidence EXCEPT !.candidates = @ \cup {v}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, paymentEvidence>>

HardwareCommitRedemption(v) ==
  LET d == redemption[v].owner
      p == RedemptionProfile[v]
  IN /\ v \in VoucherIds
     /\ redemption[v].phase = "Candidate"
     /\ redemption[v].voucherState = "Absent"
     /\ redemption[v].outboxState = "Reserved"
     /\ v \in redemptionEvidence.sealed
     /\ v \in redemptionEvidence.candidates
     /\ HardwareQualified[p]
     /\ NoSoftwareFallback[p]
     /\ profileState[p] = "Active"
     /\ redemption[v].predecessor = Predecessor(d)
     /\ Predecessor(d) \notin spentPredecessors
     /\ balance[d] >= redemption[v].amount
     /\ sequence[d] < MaxSequence
     /\ balance' = [balance EXCEPT ![d] = @ - redemption[v].amount]
     /\ sequence' = [sequence EXCEPT ![d] = @ + 1]
     /\ spentPredecessors' = spentPredecessors \cup {Predecessor(d)}
     /\ redemption' =
          [redemption EXCEPT
            ![v].phase = "HardwareCommitted",
            ![v].voucherState = "Pending",
            ![v].outboxState = "Committed"]
     /\ redemptionEvidence' =
          [redemptionEvidence EXCEPT !.committed = @ \cup {v}]
     /\ UNCHANGED <<epoch, online, reserve, totalTopups, totalRedemptions,
                     ticket, payment, lifecycleVars, paymentEvidence>>

GenerateRedemptionWrapper(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "HardwareCommitted"
  /\ redemption[v].voucherState = "Pending"
  /\ v \in redemptionEvidence.candidates
  /\ redemption' =
       [redemption EXCEPT ![v].phase = "WrapperGenerated"]
  /\ redemptionEvidence' =
       [redemptionEvidence EXCEPT !.wrappers = @ \cup {v}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, paymentEvidence>>

InstallRedemptionEnvelope(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "WrapperGenerated"
  /\ redemption[v].voucherState = "Pending"
  /\ v \in redemptionEvidence.wrappers
  /\ redemption' = [redemption EXCEPT ![v].phase = "Installed"]
  /\ redemptionEvidence' =
       [redemptionEvidence EXCEPT !.canonical = @ \cup {v}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, paymentEvidence>>

ExposeRedemptionEnvelope(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Installed"
  /\ redemption[v].voucherState = "Pending"
  /\ v \in redemptionEvidence.canonical
  /\ redemption' = [redemption EXCEPT ![v].phase = "Exposed"]
  /\ redemptionEvidence' =
       [redemptionEvidence EXCEPT !.exposed = @ \cup {v}]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, paymentEvidence>>

CrashRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase \in
       {"Prepared", "Candidate", "HardwareCommitted",
        "WrapperGenerated", "Installed", "Exposed"}
  /\ redemption' =
       [redemption EXCEPT
         ![v].recoveryFrom = redemption[v].phase,
         ![v].phase = "Recovery"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, evidenceVars>>

ResumePrecommitRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Recovery"
  /\ redemption[v].recoveryFrom \in {"Prepared", "Candidate"}
  /\ redemption' =
       [redemption EXCEPT
         ![v].phase = redemption[v].recoveryFrom,
         ![v].recoveryFrom = "None"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, evidenceVars>>

ResumeCommittedRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Recovery"
  /\ redemption[v].recoveryFrom \in
       {"HardwareCommitted", "WrapperGenerated", "Installed", "Exposed"}
  /\ v \in redemptionEvidence.candidates
  /\ v \in redemptionEvidence.committed
  /\ redemption' =
       [redemption EXCEPT
         ![v].phase = redemption[v].recoveryFrom,
         ![v].recoveryFrom = "None"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, evidenceVars>>

CancelPrecommitRedemption(v) ==
  /\ v \in VoucherIds
  /\ \/ redemption[v].phase \in {"Prepared", "Candidate"}
     \/ /\ redemption[v].phase = "Recovery"
        /\ redemption[v].recoveryFrom \in {"Prepared", "Candidate"}
  /\ redemption[v].voucherState = "Absent"
  /\ redemption[v].outboxState = "Reserved"
  /\ redemption' =
       [redemption EXCEPT
         ![v].phase = "Cancelled",
         ![v].recoveryFrom = "None",
         ![v].outboxState = "Released"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, lifecycleVars, evidenceVars>>

(***************************************************************************
Server application deliberately has no current suite/profile/policy guard.
Thus a voucher legitimately hardware-committed before an emergency profile
suspension retains its online redemption/recovery path.
***************************************************************************)
ApplyRedemption(v) ==
  LET d == redemption[v].owner
  IN /\ v \in VoucherIds
     /\ v \in redemptionEvidence.exposed
     /\ \/ redemption[v].phase = "Exposed"
        \/ /\ redemption[v].phase = "Recovery"
           /\ redemption[v].recoveryFrom = "Exposed"
     /\ redemption[v].voucherState = "Pending"
     /\ redemption[v].outboxState = "Committed"
     /\ v \in redemptionEvidence.canonical
     /\ v \notin redemptionEvidence.consumedNullifiers
     /\ reserve >= redemption[v].amount
     /\ totalRedemptions + redemption[v].amount <= totalTopups
     /\ online[d] + redemption[v].amount <= MaxAmount
     /\ online' = [online EXCEPT ![d] = @ + redemption[v].amount]
     /\ reserve' = reserve - redemption[v].amount
     /\ totalRedemptions' = totalRedemptions + redemption[v].amount
     /\ redemption' =
          [redemption EXCEPT
            ![v].phase = "Exposed",
            ![v].recoveryFrom = "None",
            ![v].voucherState = "Applied",
            ![v].outboxState = "Released"]
     /\ redemptionEvidence' =
          [redemptionEvidence EXCEPT
            !.consumedNullifiers = @ \cup {v}]
     /\ UNCHANGED <<balance, sequence, epoch, totalTopups, spentPredecessors,
                     ticket, payment, lifecycleVars, paymentEvidence>>

(***************************************************************************
Counter rollover installs a fresh hardware epoch and resets the bounded model
counter. MaxSequence and the finite epoch range are exploration bounds, not a
production transition count or cumulative history limit.
***************************************************************************)
RotateHardwareEpoch(d) ==
  /\ d \in Devices
  /\ epoch[d] < MaxSequence
  /\ Predecessor(d) \notin spentPredecessors
  /\ Predecessor(d) \notin LockedPredecessors
  /\ sequence' = [sequence EXCEPT ![d] = 0]
  /\ epoch' = [epoch EXCEPT ![d] = @ + 1]
  /\ spentPredecessors' = spentPredecessors \cup {Predecessor(d)}
  /\ UNCHANGED <<balance, online, reserve, totalTopups, totalRedemptions,
                  ticket, payment, redemption, lifecycleVars, evidenceVars>>

(***************************************************************************
Ordinary verifier rotation retains the old verifier. Suspension is
prospective: it stops new tickets and hardware commits, while post-commit
recovery, staging, acknowledgement, and online redemption remain available.
***************************************************************************)
RotateVerifierSuite(nextSuite) ==
  /\ nextSuite \in SuiteIds
  /\ suiteState[nextSuite] = "Pending"
  /\ suiteState' =
       [s \in SuiteIds |->
         IF s = nextSuite
           THEN "Active"
           ELSE IF suiteState[s] = "Active"
             THEN "Retained"
             ELSE suiteState[s]]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, profileState,
                  profilePolicyEpoch, evidenceVars>>

SuspendHardwareProfile(p) ==
  /\ p \in ProfileIds
  /\ profileState[p] = "Active"
  /\ profileState' = [profileState EXCEPT ![p] = "Suspended"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, suiteState,
                  profilePolicyEpoch, evidenceVars>>

ReinstateHardwareProfile(p) ==
  /\ p \in ProfileIds
  /\ profileState[p] = "Suspended"
  /\ HardwareQualified[p]
  /\ NoSoftwareFallback[p]
  /\ profileState' = [profileState EXCEPT ![p] = "Active"]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, suiteState,
                  profilePolicyEpoch, evidenceVars>>

AdvanceProfilePolicyEpoch(p) ==
  /\ p \in ProfileIds
  /\ profilePolicyEpoch[p] < MaxSequence
  /\ profilePolicyEpoch' =
       [profilePolicyEpoch EXCEPT ![p] = @ + 1]
  /\ UNCHANGED <<ledgerVars, ticket, payment, redemption, suiteState,
                  profileState, evidenceVars>>

Next ==
  \/ \E d \in Devices, amount \in 1..MaxAmount : TopUp(d, amount)
  \/ \E d \in Devices, t \in TicketIds : CreateAcceptanceIntent(d, t)
  \/ \E t \in TicketIds : AuthorizeAcceptanceIntent(t)
  \/ \E t \in TicketIds : IssueAcceptanceTicket(t)
  \/ \E c \in CreditIds : PreparePayment(c)
  \/ \E c \in CreditIds : ProveAndPersistPaymentCandidate(c)
  \/ \E c \in CreditIds : HardwareCommitPayment(c)
  \/ \E c \in CreditIds : GeneratePaymentWrapper(c)
  \/ \E c \in CreditIds : InstallPaymentEnvelope(c)
  \/ \E c \in CreditIds : ExposePaymentEnvelope(c)
  \/ \E c \in CreditIds : CrashPayment(c)
  \/ \E c \in CreditIds : ResumePrecommitPayment(c)
  \/ \E c \in CreditIds : ResumeCommittedPayment(c)
  \/ \E t \in TicketIds : ObserveTicketExpiry(t)
  \/ \E c \in CreditIds : BeginPaymentNoCommitRecovery(c)
  \/ \E t \in TicketIds : BeginUnusedTicketNoCommitRecovery(t)
  \/ \E t \in TicketIds : CloseAuthenticatedNoCommitRecovery(t)
  \/ \E c \in CreditIds : StagePayment(c)
  \/ \E c \in CreditIds : ObserveAcknowledgement(c)
  \/ \E c \in CreditIds, claimedDigest \in DigestValues :
       RejectConflictingPayment(c, claimedDigest)
  \/ \E d \in Devices, c \in CreditIds : FoldReceive(d, c)
  \/ \E d \in Devices, v \in VoucherIds, amount \in 1..MaxAmount :
       PrepareRedemption(d, v, amount)
  \/ \E v \in VoucherIds : ProveAndPersistRedemptionCandidate(v)
  \/ \E v \in VoucherIds : HardwareCommitRedemption(v)
  \/ \E v \in VoucherIds : GenerateRedemptionWrapper(v)
  \/ \E v \in VoucherIds : InstallRedemptionEnvelope(v)
  \/ \E v \in VoucherIds : ExposeRedemptionEnvelope(v)
  \/ \E v \in VoucherIds : CrashRedemption(v)
  \/ \E v \in VoucherIds : ResumePrecommitRedemption(v)
  \/ \E v \in VoucherIds : ResumeCommittedRedemption(v)
  \/ \E v \in VoucherIds : CancelPrecommitRedemption(v)
  \/ \E v \in VoucherIds : ApplyRedemption(v)
  \/ \E d \in Devices : RotateHardwareEpoch(d)
  \/ \E s \in SuiteIds : RotateVerifierSuite(s)
  \/ \E p \in ProfileIds : SuspendHardwareProfile(p)
  \/ \E p \in ProfileIds : ReinstateHardwareProfile(p)
  \/ \E p \in ProfileIds : AdvanceProfilePolicyEpoch(p)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ balance \in [Devices -> 0..MaxAmount]
  /\ sequence \in [Devices -> 0..MaxSequence]
  /\ epoch \in [Devices -> 1..MaxSequence]
  /\ online \in [Devices -> 0..MaxAmount]
  /\ reserve \in 0..MaxAmount
  /\ totalTopups \in 0..MaxAmount
  /\ totalRedemptions \in 0..MaxAmount
  /\ spentPredecessors \subseteq PredecessorType
  /\ ticket \in [TicketIds -> TicketRecordType]
  /\ payment \in [CreditIds -> PaymentRecordType]
  /\ redemption \in [VoucherIds -> RedemptionRecordType]
  /\ suiteState \in [SuiteIds -> SuiteStates]
  /\ profileState \in [ProfileIds -> ProfileStates]
  /\ profilePolicyEpoch \in [ProfileIds -> 1..MaxSequence]
  /\ paymentEvidence \in PaymentEvidenceType
  /\ redemptionEvidence \in RedemptionEvidenceType

ReserveEquation == totalTopups = reserve + totalRedemptions

LiabilityConservation == reserve = OfflineLiability

TotalValueConservation ==
  InitialOnlineTotal = SumDeviceSet(online, Devices) + reserve

AcknowledgementsAreDurable ==
  paymentEvidence.acknowledgements \subseteq paymentEvidence.inbox

ConsumedCreditsWereStaged ==
  /\ paymentEvidence.consumed =
       {c \in CreditIds : payment[c].creditState = "Consumed"}
  /\ paymentEvidence.consumed \subseteq paymentEvidence.inbox

AppliedNullifiersAreUnique ==
  redemptionEvidence.consumedNullifiers =
    {v \in VoucherIds : redemption[v].voucherState = "Applied"}

TerminalCommitRespectsTicketDeadline ==
  \A c \in paymentEvidence.committed :
    LET t == PaymentTicket[c]
    IN /\ SenderCommitTime[c] >= TicketIssuedAt[t]
       /\ SenderCommitTime[c] < TicketExpiresAt[t]

AcceptanceIntentTicketBinding ==
  /\ \A t \in TicketIds :
       /\ CASE ticket[t].intentState = "Absent" ->
                 /\ ticket[t].state = "Absent"
                 /\ ticket[t].exactAmount = 0
            [] ticket[t].intentState = "Offered" ->
                 ticket[t].state = "Absent"
            [] ticket[t].intentState = "Authorized" ->
                 /\ ticket[t].state = "Absent"
                 /\ AcceptanceIntentAuthorizationBound(t)
            [] ticket[t].intentState = "Ticketed" ->
                 ticket[t].state \in
                   {"Reserved", "Locked", "Consumed", "RecoveryPending"}
            [] ticket[t].intentState = "ClosedNoCommit" ->
                 ticket[t].state = "Released"
            [] OTHER -> FALSE
       /\ (ticket[t].intentState # "Absent") =>
            /\ ticket[t].intentId = TicketIntent[t]
            /\ ticket[t].intentRequest = TicketRequest[t]
            /\ ticket[t].intentDigest = ticket[t].intentId
            /\ ticket[t].exactAmount = IntentAmount[ticket[t].intentId]
            /\ ticket[t].exactAmount =
                 RequestAmount[ticket[t].intentRequest]
            /\ ticket[t].senderCommitment =
                 RandomSenderCommitment(
                   ticket[t].intentId, ticket[t].intentSender)
            /\ ticket[t].intentSender #
                 RequestOwner[ticket[t].intentRequest]
  /\ \A t1 \in TicketIds, t2 \in TicketIds :
       /\ ticket[t1].intentState # "Absent"
       /\ ticket[t2].intentState # "Absent"
       /\ ticket[t1].intentId = ticket[t2].intentId
       => t1 = t2

AcceptanceIntentAuthorizationGate ==
  /\ \A t \in TicketIds :
       ticket[t].intentState \in
         {"Authorized", "Ticketed", "ClosedNoCommit"} =>
           /\ AcceptanceIntentAuthorizationBound(t)
           /\ ticket[t].authorizationProfile =
                IntentAuthorizationProfile[ticket[t].intentId]
  /\ \A t \in TicketIds :
       ticket[t].intentState = "Offered" =>
         ticket[t].authorizationPolicyEpoch = 0
  /\ ticket[ModelTicket3].intentState \in
       {"Authorized", "Ticketed", "ClosedNoCommit"} =>
         ticket[ModelTicket3].authorizationProfile #
           TicketProfile[ModelTicket3]

MintAuthorizationIsPreDebitAndAcyclic ==
  \A d \in Devices, amount \in 1..MaxAmount :
    /\ MintAuthorizationShape(d, amount)
    /\ MintAuthorizationStatement(d, amount).creditBinding =
         MintCreditBinding(d, amount)
    /\ MintAuthorizationStatement(d, amount).creditBinding.creditCommitment =
         MintCreditCommitment(d, amount)

TerminalCertificateConstructionIsSelfFree ==
  \A c \in paymentEvidence.committed :
    LET body == PaymentTerminalCertificateBody(c)
        bodyDigest == PaymentTerminalCertificateBodyDigest(c)
        certificateId == PaymentTerminalCertificateId(c)
        certificate == PaymentCommitCertificate(c)
    IN /\ DOMAIN body =
             {"canonicalFieldOrder", "candidateDigest",
              "lifecycleBindingDigest", "commitEvidence"}
       /\ body.canonicalFieldOrder = TerminalBodyFieldOrder
       /\ "terminalCertificateId" \notin DOMAIN body
       /\ certificate.terminalBody = body
       /\ certificate.terminalBodyDigest = bodyDigest
       /\ certificate.terminalCertificateId = certificateId
       /\ certificateId.terminalCertificateId = bodyDigest

ExpiryAloneNeverReclaims ==
  \A t \in paymentEvidence.expiredTickets :
    \/ /\ t \notin paymentEvidence.noCommitClosures
       /\ ticket[t].state \in
            {"Reserved", "Locked", "Consumed", "RecoveryPending"}
    \/ /\ t \in paymentEvidence.noCommitClosures
       /\ ticket[t].state = "Released"

AuthenticatedNoCommitRecoveryIntegrity ==
  /\ paymentEvidence.noCommitClosures
       \subseteq paymentEvidence.noCommitRecovery
  /\ \A t \in
       (paymentEvidence.noCommitRecovery \ paymentEvidence.noCommitClosures) :
       /\ ticket[t].state = "RecoveryPending"
       /\ ticket[t].intentState = "Ticketed"
       /\ t \in ReservedTicketsFor(RequestOwner[TicketRequest[t]])
       /\ ticket[t].intentPredecessor \notin spentPredecessors
       /\ \A c \in paymentEvidence.committed : PaymentTicket[c] # t
  /\ \A t \in paymentEvidence.noCommitClosures :
       /\ ticket[t].state = "Released"
       /\ ticket[t].intentState = "ClosedNoCommit"
       /\ \A c \in paymentEvidence.committed : PaymentTicket[c] # t
  /\ \A t \in TicketIds :
       ticket[t].state = "Released" <=>
         t \in paymentEvidence.noCommitClosures
  /\ \A t \in TicketIds :
       ticket[t].state = "Consumed" =>
         t \notin paymentEvidence.noCommitRecovery

CapacityReservationsHold ==
  /\ \A d \in Devices : InboxUse(d) <= InboxCapacity
  /\ \A d \in Devices : OutboxUse(d) <= OutboxCapacity

PaymentsMatchExactTicketAmount ==
  \A c \in CreditIds :
    payment[c].phase # "Idle" =>
      LET t == PaymentTicket[c]
      IN /\ PaymentAmountAllowed(t, payment[c].amount)
         /\ payment[c].amount = ticket[t].exactAmount
         /\ payment[c].sender = ticket[t].intentSender
         /\ payment[c].predecessor = ticket[t].intentPredecessor

TicketAmountsAreExactPositive ==
  /\ RequestAmount \in [RequestIds -> 1..MaxAmount]
  /\ \A i \in IntentIds :
       IntentAmount[i] = RequestAmount[IntentRequest[i]]
  /\ \A t \in TicketIds :
       ticket[t].intentState # "Absent" =>
         /\ ticket[t].exactAmount > 0
         /\ ticket[t].exactAmount =
              RequestAmount[TicketRequest[t]]
         /\ ticket[t].exactAmount =
              PublicAcceptanceIntent(t).exactAmount
         /\ ticket[t].exactAmount =
              PublicAcceptanceTicket(t).exactAmount
  /\ \A t \in {ModelTicket1, ModelTicket2, ModelTicket3, ModelTicket4} :
       TicketRequest[t] = ModelRequest1

ConservationMakesStagedCreditsFoldable ==
  \A c \in CreditIds :
    payment[c].creditState = "Staged" =>
      LET d == PaymentRecipient(c)
      IN /\ balance[d] + payment[c].amount <= OfflineLiability
         /\ OfflineLiability = reserve
         /\ reserve <= MaxAmount

StagedCreditsRemainValueFoldable ==
  \A c \in CreditIds :
    payment[c].creditState = "Staged" => StagedCreditCanEnterFold(c)

SameIdDifferentDigestConflictsAreRejected ==
  \A conflict \in paymentEvidence.conflicts :
    LET c == conflict.creditId
    IN /\ conflict.ticketId = PaymentTicket[c]
       /\ conflict.digest \in DigestValues
       /\ conflict.digest # CanonicalEnvelopeDigest(c)
       /\ c \in paymentEvidence.canonical
       /\ PublicPayment(c).amountCiphertextCommitment =
            CanonicalEnvelopeDigest(c)

TicketReservationIntegrity ==
  \A t \in TicketIds :
    /\ (ticket[t].state = "Locked") =>
         LET c == ticket[t].boundCredit
         IN /\ PaymentTicket[c] = t
            /\ payment[c].phase \in
                 {"Prepared", "Candidate", "HardwareCommitted",
                  "WrapperGenerated", "Installed", "Exposed", "Recovery"}
            /\ payment[c].creditState \in
                 {"Absent", "Committed", "Available"}
    /\ (ticket[t].state = "Consumed") =>
         LET c == ticket[t].boundCredit
         IN /\ PaymentTicket[c] = t
            /\ c \in paymentEvidence.committed
            /\ payment[c].phase \in {"Exposed", "Recovery"}
            /\ payment[c].creditState \in {"Staged", "Consumed"}
    /\ (ticket[t].state = "RecoveryPending") =>
         /\ t \in paymentEvidence.noCommitRecovery
         /\ t \notin paymentEvidence.noCommitClosures
         /\ \A c \in paymentEvidence.committed : PaymentTicket[c] # t
         /\ \/ NoPaymentStartedForTicket(t)
            \/ CancelledBoundPayment(t)
    /\ (ticket[t].state = "Released") =>
         /\ t \in paymentEvidence.noCommitClosures
         /\ ticket[t].intentState = "ClosedNoCommit"
         /\ \A c \in paymentEvidence.committed : PaymentTicket[c] # t

TicketsAreOneUse ==
  \A c1 \in paymentEvidence.committed,
     c2 \in paymentEvidence.committed :
    PaymentTicket[c1] = PaymentTicket[c2] => c1 = c2

CommittedOperationsUseOnePredecessor ==
  /\ \A c \in paymentEvidence.committed :
       payment[c].predecessor \in spentPredecessors
  /\ \A v \in redemptionEvidence.committed :
       redemption[v].predecessor \in spentPredecessors
  /\ \A c1 \in paymentEvidence.committed,
         c2 \in paymentEvidence.committed :
       payment[c1].predecessor = payment[c2].predecessor => c1 = c2
  /\ \A v1 \in redemptionEvidence.committed,
         v2 \in redemptionEvidence.committed :
       redemption[v1].predecessor = redemption[v2].predecessor => v1 = v2
  /\ \A c \in paymentEvidence.committed,
         v \in redemptionEvidence.committed :
       payment[c].predecessor # redemption[v].predecessor

CommittedPaymentsRemainReceivable ==
  \A c \in paymentEvidence.committed :
    LET t == PaymentTicket[c]
    IN CASE payment[c].creditState = "Committed" ->
              /\ c \in paymentEvidence.sealed
              /\ c \in paymentEvidence.candidates
              /\ payment[c].outboxState = "Committed"
              /\ ticket[t].state = "Locked"
              /\ ticket[t].boundCredit = c
              /\ \/ payment[c].phase \in
                       {"HardwareCommitted", "WrapperGenerated", "Installed"}
                 \/ /\ payment[c].phase = "Recovery"
                    /\ payment[c].recoveryFrom \in
                         {"HardwareCommitted", "WrapperGenerated", "Installed"}
         [] payment[c].creditState = "Available" -> CanStage(c)
         [] payment[c].creditState = "Staged" ->
              /\ ticket[t].state = "Consumed"
              /\ ticket[t].boundCredit = c
              /\ c \in paymentEvidence.canonical
              /\ c \in paymentEvidence.exposed
              /\ c \in paymentEvidence.inbox
              /\ c \in paymentEvidence.acknowledgements
              /\ StagedCreditCanEnterFold(c)
         [] payment[c].creditState = "Consumed" ->
              /\ ticket[t].state = "Consumed"
              /\ ticket[t].boundCredit = c
              /\ c \in paymentEvidence.canonical
              /\ c \in paymentEvidence.exposed
              /\ c \in paymentEvidence.inbox
              /\ c \in paymentEvidence.acknowledgements
         [] OTHER -> FALSE

CommittedRedemptionsRemainRecoverable ==
  \A v \in redemptionEvidence.committed :
    /\ v \in redemptionEvidence.sealed
    /\ v \in redemptionEvidence.candidates
    /\ CASE redemption[v].voucherState = "Pending" ->
              /\ redemption[v].outboxState = "Committed"
              /\ \/ redemption[v].phase \in
                       {"HardwareCommitted", "WrapperGenerated",
                        "Installed", "Exposed"}
                 \/ /\ redemption[v].phase = "Recovery"
                    /\ redemption[v].recoveryFrom \in
                         {"HardwareCommitted", "WrapperGenerated",
                          "Installed", "Exposed"}
          [] redemption[v].voucherState = "Applied" ->
              /\ \/ redemption[v].phase = "Exposed"
                 \/ /\ redemption[v].phase = "Recovery"
                    /\ redemption[v].recoveryFrom = "Exposed"
              /\ redemption[v].outboxState = "Released"
              /\ v \in redemptionEvidence.canonical
              /\ v \in redemptionEvidence.exposed
              /\ v \in redemptionEvidence.consumedNullifiers
          [] OTHER -> FALSE

CanonicalArtifactsWereHardwareCommitted ==
  /\ paymentEvidence.wrappers \subseteq
       (paymentEvidence.candidates \cap paymentEvidence.committed)
  /\ paymentEvidence.canonical \subseteq paymentEvidence.wrappers
  /\ paymentEvidence.exposed \subseteq paymentEvidence.canonical
  /\ redemptionEvidence.wrappers \subseteq
       (redemptionEvidence.candidates \cap redemptionEvidence.committed)
  /\ redemptionEvidence.canonical \subseteq redemptionEvidence.wrappers
  /\ redemptionEvidence.exposed \subseteq redemptionEvidence.canonical

LifecycleBindingsAreComplete ==
  /\ \A c \in paymentEvidence.committed :
       /\ DOMAIN PaymentLifecycleBinding(c) = LifecycleBindingFields
       /\ PaymentCandidateRecord(c).candidateDigest = c
       /\ PaymentCandidateRecord(c).lifecycleBinding =
            PaymentLifecycleBinding(c)
       /\ PaymentCandidateRecord(c).privateIntentOpening =
            [intentId |-> ticket[PaymentTicket[c]].intentId,
             sender |-> ticket[PaymentTicket[c]].intentSender,
             predecessor |-> ticket[PaymentTicket[c]].intentPredecessor]
       /\ PaymentCommitCertificate(c).terminalBody.candidateDigest =
            PaymentCandidateRecord(c).candidateDigest
       /\ PaymentEnvelopeRecord(c).candidateDigest =
            PaymentCandidateRecord(c).candidateDigest
       /\ PaymentEnvelopeRecord(c).publicPayment = PublicPayment(c)
  /\ \A v \in redemptionEvidence.committed :
       DOMAIN RedemptionLifecycleBinding(v) =
         {"networkId", "protocolVersion", "suiteId", "vkDigest", "assetId",
          "assetIncarnation", "assetScale", "hardwareProfileId",
          "policyEpoch", "laneCommitment", "hardwareEpoch", "operationKind",
          "voucherId", "ciphertextDigest", "predecessorStateHead",
          "successorStateHead"}

PublicPaymentTranscriptBoundary ==
  \A c \in paymentEvidence.exposed :
    LET t == PaymentTicket[c]
        forbidden ==
          {"sender", "intentPredecessor", "laneCommitment", "hardwareEpoch",
           "predecessorStateHead", "successorStateHead", "issuedAt",
           "expiresAt", "deadline", "commitTime", "leaseId",
           "authorizationCounter", "clockEpoch"}
    IN /\ DOMAIN PublicPayment(c) = AllowedPublicPaymentFields
       /\ DOMAIN PublicAcceptanceIntent(t) =
            {"requestId", "intentId", "exactAmount", "senderCommitment"}
       /\ DOMAIN PublicAcceptanceTicket(t) =
            {"requestId", "acceptanceTicketId", "intentDigest", "exactAmount"}
       /\ forbidden \cap DOMAIN PublicPayment(c) = {}
       /\ forbidden \cap DOMAIN PublicAcceptanceIntent(t) = {}
       /\ forbidden \cap DOMAIN PublicAcceptanceTicket(t) = {}
       /\ PublicAcceptanceTicket(t).intentDigest =
            PublicAcceptanceIntent(t).intentId
       /\ PublicAcceptanceTicket(t).exactAmount = payment[c].amount
       /\ PublicPayment(c).commitEvidence = c

PostCommitArtifactIdentityIsStable ==
  /\ \A c \in paymentEvidence.committed :
       /\ (payment[c].phase = "WrapperGenerated"
             \/ (payment[c].phase = "Recovery"
                   /\ payment[c].recoveryFrom = "WrapperGenerated"))
            => c \in paymentEvidence.wrappers
       /\ (payment[c].phase = "Installed"
             \/ (payment[c].phase = "Recovery"
                   /\ payment[c].recoveryFrom = "Installed"))
            => c \in paymentEvidence.canonical
       /\ (payment[c].phase = "Exposed"
             \/ (payment[c].phase = "Recovery"
                   /\ payment[c].recoveryFrom = "Exposed"))
            => c \in paymentEvidence.exposed
  /\ \A v \in redemptionEvidence.committed :
       /\ (redemption[v].phase = "WrapperGenerated"
             \/ (redemption[v].phase = "Recovery"
                   /\ redemption[v].recoveryFrom = "WrapperGenerated"))
            => v \in redemptionEvidence.wrappers
       /\ (redemption[v].phase = "Installed"
             \/ (redemption[v].phase = "Recovery"
                   /\ redemption[v].recoveryFrom = "Installed"))
            => v \in redemptionEvidence.canonical
       /\ (redemption[v].phase = "Exposed"
             \/ (redemption[v].phase = "Recovery"
                   /\ redemption[v].recoveryFrom = "Exposed"))
            => v \in redemptionEvidence.exposed

QualifiedHardwareOnly ==
  /\ \A t \in TicketIds :
       ticket[t].intentState \in
         {"Authorized", "Ticketed", "ClosedNoCommit"} =>
         /\ HardwareQualified[ticket[t].authorizationProfile]
         /\ NoSoftwareFallback[ticket[t].authorizationProfile]
         /\ ticket[t].authorizationPolicyEpoch > 0
  /\ \A t \in TicketIds :
       ticket[t].state # "Absent" =>
         /\ HardwareQualified[TicketProfile[t]]
         /\ NoSoftwareFallback[TicketProfile[t]]
         /\ ticket[t].policyEpoch > 0
  /\ \A c \in CreditIds :
       payment[c].phase # "Idle" =>
         /\ HardwareQualified[PaymentProfile[c]]
         /\ NoSoftwareFallback[PaymentProfile[c]]
         /\ payment[c].policyEpoch > 0
  /\ \A v \in VoucherIds :
       redemption[v].phase # "Idle" =>
         /\ HardwareQualified[RedemptionProfile[v]]
         /\ NoSoftwareFallback[RedemptionProfile[v]]
         /\ redemption[v].policyEpoch > 0

ExactlyOneActiveSuite ==
  Cardinality({s \in SuiteIds : suiteState[s] = "Active"}) = 1

=============================================================================
