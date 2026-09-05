---- MODULE KagemushaV1 ----
EXTENDS Naturals, FiniteSets, Sequences

(***************************************************************************
Finite-state safety model for the sole first-release KAGEMUSHA protocol.

KAGEMUSHA carries one recursively proved aggregate balance per device lane
and asset.  Nothing in this module gives monetary meaning to hops, notes,
origins, ancestry, fan-in, receipt count, or proof depth.  The finite sets and
integer ceilings supplied to TLC are exploration bounds only; they are not
protocol fields or admission rules.

Cryptographic hashes, commitments, recursive proofs, attestation, finality,
authenticated storage, and trusted time are symbolic here.  The model checks
the state-machine obligations that those mechanisms must enforce.
***************************************************************************)

CONSTANTS
  Devices,
  RequestIds,
  CreditIds,
  MintIds,
  VoucherIds,
  RotationIds,
  InitialOnlinePerDevice,
  MaxAmount,
  MaxLogicalSequence,
  MaxHardwareCounter,
  MaxEpoch,
  MaxTime,
  NoState,
  NoDigest,
  NoAck,
  ConflictingDigest,
  ModelDevice1,
  ModelDevice2,
  ModelRequest1,
  ModelRequest2,
  ModelCredit1,
  ModelCredit2,
  ModelCredit3,
  ModelMint1,
  ModelMint2,
  ModelVoucher1,
  ModelVoucher2,
  ModelRotation1,
  ModelRotation2

PeerMessageOrder == <<"Request", "Payment", "Acknowledgement">>
MintStates == {"Absent", "Finalized", "Folded"}
PaymentPhases ==
  {"Absent", "HardwareCommitted", "Proofed", "Canonical", "Exposed",
   "Staged", "Folded"}
RedemptionPhases ==
  {"Absent", "HardwareCommitted", "Proofed", "Canonical", "Exposed",
   "Applied"}
TransitionKinds ==
  {"MintFold", "SendSplit", "ReceiveFold", "RedeemSplit", "Rotate"}

(***************************************************************************
The concrete functions below are sample exploration data.  ModelCredit1 and
ModelCredit2 are deliberately distinct payments for ModelRequest1.  There is
no mutable per-request use, amount-total, count, or reservation state.
***************************************************************************)

RequestAmount ==
  [q \in RequestIds |-> IF q = ModelRequest2 THEN 2 ELSE 1]
RequestRecipient == [q \in RequestIds |-> ModelDevice2]
RequestIssuedAt ==
  [q \in RequestIds |-> IF q = ModelRequest2 THEN 20 ELSE 10]
RequestExpiresAt ==
  [q \in RequestIds |-> IF q = ModelRequest2 THEN 30 ELSE 20]

CreditRequest ==
  [c \in CreditIds |->
    IF c \in {ModelCredit1, ModelCredit2}
      THEN ModelRequest1
      ELSE ModelRequest2]
CreditSender == [c \in CreditIds |-> ModelDevice1]
CreditRecipient == [c \in CreditIds |-> RequestRecipient[CreditRequest[c]]]
CreditAmount == [c \in CreditIds |-> RequestAmount[CreditRequest[c]]]
CreditCommitTime ==
  [c \in CreditIds |->
    CASE c = ModelCredit1 -> 11
      [] c = ModelCredit2 -> 12
      [] OTHER -> 31]

RotationDevice ==
  [r \in RotationIds |->
    IF r = ModelRotation1 THEN ModelDevice1 ELSE ModelDevice2]

SendKey(c) == <<"SendSplit", c>>
MintKey(m) == <<"MintFold", m>>
ReceiveKey(c) == <<"ReceiveFold", c>>
RedeemKey(v) == <<"RedeemSplit", v>>
RotateKey(r) == <<"Rotate", r>>

OperationKeys ==
  {SendKey(c) : c \in CreditIds}
    \cup {MintKey(m) : m \in MintIds}
    \cup {ReceiveKey(c) : c \in CreditIds}
    \cup {RedeemKey(v) : v \in VoucherIds}
    \cup {RotateKey(r) : r \in RotationIds}

MaxValue == Cardinality(Devices) * InitialOnlinePerDevice
InitialOnline == [d \in Devices |-> InitialOnlinePerDevice]

DefaultDevice == CHOOSE d \in Devices : TRUE
DefaultRequest == CHOOSE q \in RequestIds : TRUE

StateHeadType ==
  [deviceLane : Devices,
   hardwareEpoch : 0..MaxEpoch,
   logicalSequence : 0..MaxLogicalSequence,
   hardwareCounter : 0..MaxHardwareCounter,
   stateNonce : 0..MaxLogicalSequence]

DeviceStateType ==
  [bootstrapped : BOOLEAN,
   balance : 0..MaxValue,
   hardwareEpoch : 0..MaxEpoch,
   logicalSequence : 0..MaxLogicalSequence,
   hardwareCounter : 0..MaxHardwareCounter,
   stateNonce : 0..MaxLogicalSequence,
   replayRoot : SUBSET CreditIds]

MintRecordType ==
  [state : MintStates,
   recipient : Devices,
   amount : 0..MaxAmount,
   authorizationVerified : BOOLEAN,
   finalityVerified : BOOLEAN]

PaymentRecordType ==
  [phase : PaymentPhases,
   request : RequestIds,
   sender : Devices,
   recipient : Devices,
   amount : 0..MaxAmount,
   commitTime : 0..MaxTime,
   before : StateHeadType \cup {NoState},
   after : StateHeadType \cup {NoState},
   beforeBalance : 0..MaxValue,
   afterBalance : 0..MaxValue,
   hardwareCertificate : BOOLEAN,
   recursiveProof : BOOLEAN,
   outboxEnvelope : CreditIds \cup {NoDigest},
   acceptedEnvelope : CreditIds \cup {NoDigest, ConflictingDigest},
   durableAck : CreditIds \cup {NoAck},
   outboxRetained : BOOLEAN,
   crashed : BOOLEAN,
   recovered : BOOLEAN]

RedemptionRecordType ==
  [phase : RedemptionPhases,
   owner : Devices,
   amount : 0..MaxAmount,
   before : StateHeadType \cup {NoState},
   after : StateHeadType \cup {NoState},
   beforeBalance : 0..MaxValue,
   afterBalance : 0..MaxValue,
   hardwareCertificate : BOOLEAN,
   recursiveProof : BOOLEAN,
   outboxEnvelope : VoucherIds \cup {NoDigest},
   outboxRetained : BOOLEAN,
   crashed : BOOLEAN,
   recovered : BOOLEAN]

TransitionRecordType ==
  [kind : TransitionKinds,
   operation : OperationKeys,
   device : Devices,
   before : StateHeadType,
   after : StateHeadType,
   beforeBalance : 0..MaxValue,
   afterBalance : 0..MaxValue,
   beforeReplayRoot : SUBSET CreditIds,
   afterReplayRoot : SUBSET CreditIds]

ConflictRecordType ==
  [creditId : CreditIds, digest : {ConflictingDigest}]

VARIABLES
  device,
  online,
  reserve,
  totalTopups,
  totalRedemptions,
  consumedHeads,
  transitionLog,
  preparedHead,
  mint,
  payment,
  redemption,
  conflictingDeliveries,
  duplicateAckCredits,
  senderObservedAcks,
  trustedTime

ledgerVars ==
  <<device, online, reserve, totalTopups, totalRedemptions,
    consumedHeads, transitionLog, preparedHead>>
transportVars ==
  <<payment, conflictingDeliveries, duplicateAckCredits, senderObservedAcks>>
vars == <<ledgerVars, mint, transportVars, redemption, trustedTime>>

StateHead(d) ==
  [deviceLane |-> d,
   hardwareEpoch |-> device[d].hardwareEpoch,
   logicalSequence |-> device[d].logicalSequence,
   hardwareCounter |-> device[d].hardwareCounter,
   stateNonce |-> device[d].stateNonce]

NormalSuccessor(h) ==
  [deviceLane |-> h.deviceLane,
   hardwareEpoch |-> h.hardwareEpoch,
   logicalSequence |-> h.logicalSequence + 1,
   hardwareCounter |-> h.hardwareCounter + 1,
   stateNonce |-> h.stateNonce + 1]

RotationSuccessor(h) ==
  [deviceLane |-> h.deviceLane,
   hardwareEpoch |-> h.hardwareEpoch + 1,
   logicalSequence |-> h.logicalSequence + 1,
   hardwareCounter |-> 0,
   stateNonce |-> h.stateNonce + 1]

Transition(kind, operation, d, before, after,
           beforeBalance, afterBalance, beforeRoot, afterRoot) ==
  [kind |-> kind,
   operation |-> operation,
   device |-> d,
   before |-> before,
   after |-> after,
   beforeBalance |-> beforeBalance,
   afterBalance |-> afterBalance,
   beforeReplayRoot |-> beforeRoot,
   afterReplayRoot |-> afterRoot]

CanAdvance(d) ==
  /\ device[d].bootstrapped
  /\ device[d].logicalSequence < MaxLogicalSequence
  /\ device[d].hardwareCounter < MaxHardwareCounter
  /\ StateHead(d) \notin consumedHeads

CanRotate(d) ==
  /\ device[d].bootstrapped
  /\ device[d].logicalSequence < MaxLogicalSequence
  /\ device[d].hardwareEpoch < MaxEpoch
  /\ StateHead(d) \notin consumedHeads

HardwareQualified(d) == d \in Devices
NoSoftwareFallback(d) == d \in Devices

PaymentRequestV1(q) ==
  [protocolVersion |-> 1,
   network |-> "kagemusha-v1-network",
   asset |-> <<"kagemusha-asset", 1>>,
   amount |-> RequestAmount[q],
   recipientAccount |-> RequestRecipient[q],
   recipientLane |-> <<"lane", RequestRecipient[q]>>,
   recipientEncryptionKey |-> <<"kem", q>>,
   hardwarePolicy |-> "non-forking-no-fallback",
   requestId |-> q,
   issuedAt |-> RequestIssuedAt[q],
   expiresAt |-> RequestExpiresAt[q]]

RequestDigest(q) == <<"request-digest", PaymentRequestV1(q)>>

OpaqueStateCommitment(head, hiddenBalance) ==
  <<"state-commitment", head, hiddenBalance>>

HardwareTransitionCommitment(c) ==
  <<"hardware-transition", payment[c].before, payment[c].after,
    payment[c].request, payment[c].amount>>

GuardBundle(c) ==
  [exactBefore |-> payment[c].before,
   exactAfter |-> payment[c].after,
   requestDigest |-> RequestDigest(payment[c].request),
   recipientLane |-> PaymentRequestV1(payment[c].request).recipientLane,
   hardwareEpoch |-> payment[c].before.hardwareEpoch,
   commitTime |-> payment[c].commitTime,
   transitionCommitment |-> HardwareTransitionCommitment(c)]

GuardBundleValid(c) ==
  /\ payment[c].hardwareCertificate
  /\ payment[c].before # NoState
  /\ payment[c].after = NormalSuccessor(payment[c].before)
  /\ payment[c].request = CreditRequest[c]
  /\ payment[c].recipient = CreditRecipient[c]
  /\ payment[c].amount = CreditAmount[c]
  /\ GuardBundle(c).requestDigest = RequestDigest(CreditRequest[c])
  /\ GuardBundle(c).recipientLane =
       PaymentRequestV1(CreditRequest[c]).recipientLane

PaymentV1(c) ==
  [protocolVersion |-> 1,
   creditId |-> c,
   requestDigest |-> RequestDigest(payment[c].request),
   amount |-> payment[c].amount,
   senderBeforeCommitment |->
     OpaqueStateCommitment(payment[c].before, payment[c].beforeBalance),
   senderAfterCommitment |->
     OpaqueStateCommitment(payment[c].after, payment[c].afterBalance),
   receiverBinding |->
     <<payment[c].recipient,
       PaymentRequestV1(payment[c].request).recipientLane,
       PaymentRequestV1(payment[c].request).recipientEncryptionKey>>,
   trustedSenderCommitTime |-> payment[c].commitTime,
   encryptedCreditOpening |-> <<"encrypted-credit", c>>,
   hardwareTransitionCommitment |-> HardwareTransitionCommitment(c),
   recursiveProof |-> <<"paired-pasta-proof", c>>]

AcknowledgementV1(c) ==
  [protocolVersion |-> 1,
   requestDigest |-> RequestDigest(payment[c].request),
   paymentDigest |-> c,
   creditId |-> c,
   inboxReceipt |-> <<"rollback-resistant-inbox-receipt", c>>]

RequestValidAt(q, t) ==
  /\ RequestIssuedAt[q] <= t
  /\ t < RequestExpiresAt[q]

PaymentStageEligible(c) ==
  /\ payment[c].phase = "Exposed"
  /\ payment[c].recursiveProof
  /\ payment[c].outboxEnvelope = c
  /\ payment[c].recipient = CreditRecipient[c]
  /\ payment[c].request = CreditRequest[c]
  /\ payment[c].amount = CreditAmount[c]

ReceiveCreditSemanticallyValid(c) ==
  LET d == CreditRecipient[c]
  IN /\ payment[c].phase = "Staged"
     /\ payment[c].acceptedEnvelope = c
     /\ payment[c].durableAck = c
     /\ c \notin device[d].replayRoot

RECURSIVE SumDeviceBalances(_)
SumDeviceBalances(S) ==
  IF S = {}
    THEN 0
    ELSE LET d == CHOOSE x \in S : TRUE
         IN device[d].balance + SumDeviceBalances(S \ {d})

RECURSIVE SumOnline(_)
SumOnline(S) ==
  IF S = {}
    THEN 0
    ELSE LET d == CHOOSE x \in S : TRUE
         IN online[d] + SumOnline(S \ {d})

RECURSIVE SumMintValue(_)
SumMintValue(S) ==
  IF S = {}
    THEN 0
    ELSE LET m == CHOOSE x \in S : TRUE
         IN mint[m].amount + SumMintValue(S \ {m})

RECURSIVE SumCreditValue(_)
SumCreditValue(S) ==
  IF S = {}
    THEN 0
    ELSE LET c == CHOOSE x \in S : TRUE
         IN payment[c].amount + SumCreditValue(S \ {c})

RECURSIVE SumVoucherValue(_)
SumVoucherValue(S) ==
  IF S = {}
    THEN 0
    ELSE LET v == CHOOSE x \in S : TRUE
         IN redemption[v].amount + SumVoucherValue(S \ {v})

PendingMints == {m \in MintIds : mint[m].state = "Finalized"}
CommittedCredits == {c \in CreditIds : payment[c].phase # "Absent"}
PendingCredits ==
  {c \in CreditIds :
    payment[c].phase \in
      {"HardwareCommitted", "Proofed", "Canonical", "Exposed", "Staged"}}
StagedCredits ==
  {c \in CreditIds : payment[c].phase \in {"Staged", "Folded"}}
FoldedCredits == {c \in CreditIds : payment[c].phase = "Folded"}
PendingVouchers ==
  {v \in VoucherIds :
    redemption[v].phase \in
      {"HardwareCommitted", "Proofed", "Canonical", "Exposed"}}

OfflineLiability ==
  SumDeviceBalances(Devices)
    + SumMintValue(PendingMints)
    + SumCreditValue(PendingCredits)
    + SumVoucherValue(PendingVouchers)

InitialOnlineTotal == Cardinality(Devices) * InitialOnlinePerDevice

Init ==
  /\ Devices # {}
  /\ RequestIds # {}
  /\ ModelDevice1 \in Devices
  /\ ModelDevice2 \in Devices
  /\ ModelDevice1 # ModelDevice2
  /\ {ModelRequest1, ModelRequest2} \subseteq RequestIds
  /\ ModelRequest1 # ModelRequest2
  /\ {ModelCredit1, ModelCredit2, ModelCredit3} \subseteq CreditIds
  /\ Cardinality({ModelCredit1, ModelCredit2, ModelCredit3}) = 3
  /\ {ModelMint1, ModelMint2} \subseteq MintIds
  /\ ModelMint1 # ModelMint2
  /\ {ModelVoucher1, ModelVoucher2} \subseteq VoucherIds
  /\ ModelVoucher1 # ModelVoucher2
  /\ {ModelRotation1, ModelRotation2} \subseteq RotationIds
  /\ ModelRotation1 # ModelRotation2
  /\ NoState \notin StateHeadType
  /\ NoDigest \notin CreditIds
  /\ NoDigest \notin VoucherIds
  /\ NoAck \notin CreditIds
  /\ ConflictingDigest \notin CreditIds
  /\ MaxAmount > 0
  /\ MaxLogicalSequence > 0
  /\ MaxHardwareCounter > 0
  /\ MaxEpoch > 0
  /\ MaxTime >= 31
  /\ device =
       [d \in Devices |->
         [bootstrapped |-> FALSE,
          balance |-> 0,
          hardwareEpoch |-> 0,
          logicalSequence |-> 0,
          hardwareCounter |-> 0,
          stateNonce |-> 0,
          replayRoot |-> {}]]
  /\ online = InitialOnline
  /\ reserve = 0
  /\ totalTopups = 0
  /\ totalRedemptions = 0
  /\ consumedHeads = {}
  /\ transitionLog = {}
  /\ preparedHead = [op \in OperationKeys |-> NoState]
  /\ mint =
       [m \in MintIds |->
         [state |-> "Absent",
          recipient |-> DefaultDevice,
          amount |-> 0,
          authorizationVerified |-> FALSE,
          finalityVerified |-> FALSE]]
  /\ payment =
       [c \in CreditIds |->
         [phase |-> "Absent",
          request |-> DefaultRequest,
          sender |-> DefaultDevice,
          recipient |-> DefaultDevice,
          amount |-> 0,
          commitTime |-> 0,
          before |-> NoState,
          after |-> NoState,
          beforeBalance |-> 0,
          afterBalance |-> 0,
          hardwareCertificate |-> FALSE,
          recursiveProof |-> FALSE,
          outboxEnvelope |-> NoDigest,
          acceptedEnvelope |-> NoDigest,
          durableAck |-> NoAck,
          outboxRetained |-> FALSE,
          crashed |-> FALSE,
          recovered |-> FALSE]]
  /\ redemption =
       [v \in VoucherIds |->
         [phase |-> "Absent",
          owner |-> DefaultDevice,
          amount |-> 0,
          before |-> NoState,
          after |-> NoState,
          beforeBalance |-> 0,
          afterBalance |-> 0,
          hardwareCertificate |-> FALSE,
          recursiveProof |-> FALSE,
          outboxEnvelope |-> NoDigest,
          outboxRetained |-> FALSE,
          crashed |-> FALSE,
          recovered |-> FALSE]]
  /\ conflictingDeliveries = {}
  /\ duplicateAckCredits = {}
  /\ senderObservedAcks = {}
  /\ trustedTime = 0

Bootstrap(d) ==
  /\ d \in Devices
  /\ ~device[d].bootstrapped
  /\ device[d].balance = 0
  /\ device' = [device EXCEPT ![d].bootstrapped = TRUE]
  /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                  consumedHeads, transitionLog, preparedHead, mint,
                  transportVars, redemption, trustedTime>>

AdvanceTrustedTime(t) ==
  /\ t \in 0..MaxTime
  /\ trustedTime < t
  /\ trustedTime' = t
  /\ UNCHANGED <<ledgerVars, mint, transportVars, redemption>>

TopUp(d, m, amount) ==
  /\ d \in Devices
  /\ m \in MintIds
  /\ amount \in 1..MaxAmount
  /\ mint[m].state = "Absent"
  /\ online[d] >= amount
  /\ HardwareQualified(d)
  /\ NoSoftwareFallback(d)
  /\ online' = [online EXCEPT ![d] = @ - amount]
  /\ reserve' = reserve + amount
  /\ totalTopups' = totalTopups + amount
  /\ mint' =
       [mint EXCEPT
         ![m] =
           [@ EXCEPT
             !.state = "Finalized",
             !.recipient = d,
             !.amount = amount,
             !.authorizationVerified = TRUE,
             !.finalityVerified = TRUE]]
  /\ UNCHANGED <<device, totalRedemptions, consumedHeads, transitionLog,
                  preparedHead, transportVars, redemption, trustedTime>>

PrepareMintFold(m) ==
  LET d == mint[m].recipient
      op == MintKey(m)
  IN /\ m \in MintIds
     /\ mint[m].state = "Finalized"
     /\ preparedHead[op] = NoState
     /\ device[d].bootstrapped
     /\ preparedHead' = [preparedHead EXCEPT ![op] = StateHead(d)]
     /\ UNCHANGED <<device, online, reserve, totalTopups,
                     totalRedemptions, consumedHeads, transitionLog, mint,
                     transportVars, redemption, trustedTime>>

MintFold(m) ==
  LET d == mint[m].recipient
      amount == mint[m].amount
      op == MintKey(m)
      before == StateHead(d)
      after == NormalSuccessor(before)
      transition ==
        Transition("MintFold", op, d, before, after,
                   device[d].balance, device[d].balance + amount,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ m \in MintIds
     /\ mint[m].state = "Finalized"
     /\ mint[m].authorizationVerified
     /\ mint[m].finalityVerified
     /\ preparedHead[op] = before
     /\ CanAdvance(d)
     /\ device[d].balance + amount <= MaxValue
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = @ + amount,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = @ + 1,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ mint' = [mint EXCEPT ![m].state = "Folded"]
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, transportVars, redemption, trustedTime>>

PrepareSend(c) ==
  LET d == CreditSender[c]
      op == SendKey(c)
  IN /\ c \in CreditIds
     /\ payment[c].phase = "Absent"
     /\ preparedHead[op] = NoState
     /\ device[d].bootstrapped
     /\ CreditAmount[c] > 0
     /\ preparedHead' = [preparedHead EXCEPT ![op] = StateHead(d)]
     /\ UNCHANGED <<device, online, reserve, totalTopups,
                     totalRedemptions, consumedHeads, transitionLog, mint,
                     transportVars, redemption, trustedTime>>

SendSplit(c) ==
  LET d == CreditSender[c]
      amount == CreditAmount[c]
      op == SendKey(c)
      before == StateHead(d)
      after == NormalSuccessor(before)
      transition ==
        Transition("SendSplit", op, d, before, after,
                   device[d].balance, device[d].balance - amount,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ c \in CreditIds
     /\ payment[c].phase = "Absent"
     /\ preparedHead[op] = before
     /\ CanAdvance(d)
     /\ amount > 0
     /\ device[d].balance >= amount
     /\ trustedTime = CreditCommitTime[c]
     /\ RequestValidAt(CreditRequest[c], trustedTime)
     /\ HardwareQualified(d)
     /\ NoSoftwareFallback(d)
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = @ - amount,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = @ + 1,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ payment' =
          [payment EXCEPT
            ![c] =
              [@ EXCEPT
                !.phase = "HardwareCommitted",
                !.request = CreditRequest[c],
                !.sender = d,
                !.recipient = CreditRecipient[c],
                !.amount = amount,
                !.commitTime = trustedTime,
                !.before = before,
                !.after = after,
                !.beforeBalance = device[d].balance,
                !.afterBalance = device[d].balance - amount,
                !.hardwareCertificate = TRUE,
                !.outboxRetained = TRUE]]
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, conflictingDeliveries,
                     duplicateAckCredits, senderObservedAcks, redemption,
                     trustedTime>>

GeneratePaymentProof(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "HardwareCommitted"
  /\ ~payment[c].crashed
  /\ GuardBundleValid(c)
  /\ payment' =
       [payment EXCEPT
         ![c] = [@ EXCEPT !.phase = "Proofed", !.recursiveProof = TRUE]]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

PersistPaymentEnvelope(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Proofed"
  /\ payment[c].recursiveProof
  /\ ~payment[c].crashed
  /\ payment' =
       [payment EXCEPT
         ![c] = [@ EXCEPT !.phase = "Canonical", !.outboxEnvelope = c]]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

ExposePayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Canonical"
  /\ payment[c].outboxEnvelope = c
  /\ ~payment[c].crashed
  /\ payment' = [payment EXCEPT ![c].phase = "Exposed"]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

CrashPayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase # "Absent"
  /\ ~payment[c].crashed
  /\ payment' = [payment EXCEPT ![c].crashed = TRUE]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

RecoverPayment(c) ==
  /\ c \in CreditIds
  /\ payment[c].crashed
  /\ payment' =
       [payment EXCEPT
         ![c] = [@ EXCEPT !.crashed = FALSE, !.recovered = TRUE]]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

StagePayment(c) ==
  /\ c \in CreditIds
  /\ PaymentStageEligible(c)
  /\ payment' =
       [payment EXCEPT
         ![c] =
           [@ EXCEPT
             !.phase = "Staged",
             !.acceptedEnvelope = c,
             !.durableAck = c]]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

RedeliverExactDuplicate(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase \in {"Staged", "Folded"}
  /\ payment[c].acceptedEnvelope = c
  /\ payment[c].durableAck = c
  /\ duplicateAckCredits' = duplicateAckCredits \cup {c}
  /\ UNCHANGED <<ledgerVars, mint, payment, conflictingDeliveries,
                  senderObservedAcks, redemption, trustedTime>>

RejectConflictingDelivery(c) ==
  LET conflict == [creditId |-> c, digest |-> ConflictingDigest]
  IN /\ c \in CreditIds
     /\ payment[c].phase \in {"Exposed", "Staged", "Folded"}
     /\ payment[c].outboxEnvelope = c
     /\ ConflictingDigest # c
     /\ conflictingDeliveries' = conflictingDeliveries \cup {conflict}
     /\ UNCHANGED <<ledgerVars, mint, payment, duplicateAckCredits,
                     senderObservedAcks, redemption, trustedTime>>

PrepareReceiveFold(c) ==
  LET d == CreditRecipient[c]
      op == ReceiveKey(c)
  IN /\ c \in CreditIds
     /\ ReceiveCreditSemanticallyValid(c)
     /\ preparedHead[op] = NoState
     /\ preparedHead' = [preparedHead EXCEPT ![op] = StateHead(d)]
     /\ UNCHANGED <<device, online, reserve, totalTopups,
                     totalRedemptions, consumedHeads, transitionLog, mint,
                     transportVars, redemption, trustedTime>>

ReceiveFold(c) ==
  LET d == CreditRecipient[c]
      amount == payment[c].amount
      op == ReceiveKey(c)
      before == StateHead(d)
      after == NormalSuccessor(before)
      newRoot == device[d].replayRoot \cup {c}
      transition ==
        Transition("ReceiveFold", op, d, before, after,
                   device[d].balance, device[d].balance + amount,
                   device[d].replayRoot, newRoot)
  IN /\ c \in CreditIds
     /\ ReceiveCreditSemanticallyValid(c)
     /\ preparedHead[op] = before
     /\ CanAdvance(d)
     /\ device[d].balance + amount <= MaxValue
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = @ + amount,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = @ + 1,
                !.stateNonce = @ + 1,
                !.replayRoot = newRoot]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ payment' = [payment EXCEPT ![c].phase = "Folded"]
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, conflictingDeliveries,
                     duplicateAckCredits, senderObservedAcks, redemption,
                     trustedTime>>

ObserveAcknowledgement(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase \in {"Staged", "Folded"}
  /\ payment[c].durableAck = c
  /\ payment[c].outboxRetained
  /\ payment' = [payment EXCEPT ![c].outboxRetained = FALSE]
  /\ senderObservedAcks' = senderObservedAcks \cup {c}
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, redemption, trustedTime>>

PrepareRedemption(v, d, amount) ==
  LET op == RedeemKey(v)
  IN /\ v \in VoucherIds
     /\ d \in Devices
     /\ amount \in 1..MaxAmount
     /\ redemption[v].phase = "Absent"
     /\ preparedHead[op] = NoState
     /\ device[d].bootstrapped
     /\ device[d].balance >= amount
     /\ preparedHead' = [preparedHead EXCEPT ![op] = StateHead(d)]
     /\ redemption' =
          [redemption EXCEPT
            ![v] = [@ EXCEPT !.owner = d, !.amount = amount]]
     /\ UNCHANGED <<device, online, reserve, totalTopups,
                     totalRedemptions, consumedHeads, transitionLog, mint,
                     transportVars, trustedTime>>

RedeemSplit(v) ==
  LET d == redemption[v].owner
      amount == redemption[v].amount
      op == RedeemKey(v)
      before == StateHead(d)
      after == NormalSuccessor(before)
      transition ==
        Transition("RedeemSplit", op, d, before, after,
                   device[d].balance, device[d].balance - amount,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ v \in VoucherIds
     /\ redemption[v].phase = "Absent"
     /\ preparedHead[op] = before
     /\ CanAdvance(d)
     /\ amount > 0
     /\ device[d].balance >= amount
     /\ HardwareQualified(d)
     /\ NoSoftwareFallback(d)
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = @ - amount,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = @ + 1,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ redemption' =
          [redemption EXCEPT
            ![v] =
              [@ EXCEPT
                !.phase = "HardwareCommitted",
                !.before = before,
                !.after = after,
                !.beforeBalance = device[d].balance,
                !.afterBalance = device[d].balance - amount,
                !.hardwareCertificate = TRUE,
                !.outboxRetained = TRUE]]
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, transportVars, trustedTime>>

GenerateRedemptionProof(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "HardwareCommitted"
  /\ redemption[v].hardwareCertificate
  /\ ~redemption[v].crashed
  /\ redemption[v].after = NormalSuccessor(redemption[v].before)
  /\ redemption' =
       [redemption EXCEPT
         ![v] = [@ EXCEPT !.phase = "Proofed", !.recursiveProof = TRUE]]
  /\ UNCHANGED <<ledgerVars, mint, transportVars, trustedTime>>

PersistRedemptionEnvelope(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Proofed"
  /\ redemption[v].recursiveProof
  /\ ~redemption[v].crashed
  /\ redemption' =
       [redemption EXCEPT
         ![v] = [@ EXCEPT !.phase = "Canonical", !.outboxEnvelope = v]]
  /\ UNCHANGED <<ledgerVars, mint, transportVars, trustedTime>>

ExposeRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase = "Canonical"
  /\ redemption[v].outboxEnvelope = v
  /\ ~redemption[v].crashed
  /\ redemption' = [redemption EXCEPT ![v].phase = "Exposed"]
  /\ UNCHANGED <<ledgerVars, mint, transportVars, trustedTime>>

CrashRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].phase # "Absent"
  /\ ~redemption[v].crashed
  /\ redemption' = [redemption EXCEPT ![v].crashed = TRUE]
  /\ UNCHANGED <<ledgerVars, mint, transportVars, trustedTime>>

RecoverRedemption(v) ==
  /\ v \in VoucherIds
  /\ redemption[v].crashed
  /\ redemption' =
       [redemption EXCEPT
         ![v] = [@ EXCEPT !.crashed = FALSE, !.recovered = TRUE]]
  /\ UNCHANGED <<ledgerVars, mint, transportVars, trustedTime>>

ApplyRedemption(v) ==
  LET d == redemption[v].owner
      amount == redemption[v].amount
  IN /\ v \in VoucherIds
     /\ redemption[v].phase = "Exposed"
     /\ redemption[v].recursiveProof
     /\ redemption[v].outboxEnvelope = v
     /\ reserve >= amount
     /\ online[d] + amount <= MaxValue
     /\ reserve' = reserve - amount
     /\ online' = [online EXCEPT ![d] = @ + amount]
     /\ totalRedemptions' = totalRedemptions + amount
     /\ redemption' =
          [redemption EXCEPT
            ![v] =
              [@ EXCEPT !.phase = "Applied", !.outboxRetained = FALSE]]
     /\ UNCHANGED <<device, totalTopups, consumedHeads, transitionLog,
                     preparedHead, mint, transportVars, trustedTime>>

PrepareRotate(r) ==
  LET d == RotationDevice[r]
      op == RotateKey(r)
  IN /\ r \in RotationIds
     /\ preparedHead[op] = NoState
     /\ device[d].bootstrapped
     /\ preparedHead' = [preparedHead EXCEPT ![op] = StateHead(d)]
     /\ UNCHANGED <<device, online, reserve, totalTopups,
                     totalRedemptions, consumedHeads, transitionLog, mint,
                     transportVars, redemption, trustedTime>>

Rotate(r) ==
  LET d == RotationDevice[r]
      op == RotateKey(r)
      before == StateHead(d)
      after == RotationSuccessor(before)
      transition ==
        Transition("Rotate", op, d, before, after,
                   device[d].balance, device[d].balance,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ r \in RotationIds
     /\ preparedHead[op] = before
     /\ CanRotate(d)
     /\ HardwareQualified(d)
     /\ NoSoftwareFallback(d)
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.hardwareEpoch = @ + 1,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = 0,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, transportVars, redemption,
                     trustedTime>>

Next ==
  \/ \E d \in Devices : Bootstrap(d)
  \/ \E t \in 0..MaxTime : AdvanceTrustedTime(t)
  \/ \E d \in Devices, m \in MintIds, amount \in 1..MaxAmount :
       TopUp(d, m, amount)
  \/ \E m \in MintIds : PrepareMintFold(m)
  \/ \E m \in MintIds : MintFold(m)
  \/ \E c \in CreditIds : PrepareSend(c)
  \/ \E c \in CreditIds : SendSplit(c)
  \/ \E c \in CreditIds : GeneratePaymentProof(c)
  \/ \E c \in CreditIds : PersistPaymentEnvelope(c)
  \/ \E c \in CreditIds : ExposePayment(c)
  \/ \E c \in CreditIds : CrashPayment(c)
  \/ \E c \in CreditIds : RecoverPayment(c)
  \/ \E c \in CreditIds : StagePayment(c)
  \/ \E c \in CreditIds : RedeliverExactDuplicate(c)
  \/ \E c \in CreditIds : RejectConflictingDelivery(c)
  \/ \E c \in CreditIds : PrepareReceiveFold(c)
  \/ \E c \in CreditIds : ReceiveFold(c)
  \/ \E c \in CreditIds : ObserveAcknowledgement(c)
  \/ \E v \in VoucherIds, d \in Devices, amount \in 1..MaxAmount :
       PrepareRedemption(v, d, amount)
  \/ \E v \in VoucherIds : RedeemSplit(v)
  \/ \E v \in VoucherIds : GenerateRedemptionProof(v)
  \/ \E v \in VoucherIds : PersistRedemptionEnvelope(v)
  \/ \E v \in VoucherIds : ExposeRedemption(v)
  \/ \E v \in VoucherIds : CrashRedemption(v)
  \/ \E v \in VoucherIds : RecoverRedemption(v)
  \/ \E v \in VoucherIds : ApplyRedemption(v)
  \/ \E r \in RotationIds : PrepareRotate(r)
  \/ \E r \in RotationIds : Rotate(r)

Spec == Init /\ [][Next]_vars

(***************************************************************************
Safety invariants.
***************************************************************************)

TypeOK ==
  /\ device \in [Devices -> DeviceStateType]
  /\ online \in [Devices -> 0..MaxValue]
  /\ reserve \in 0..MaxValue
  /\ totalTopups \in 0..MaxValue
  /\ totalRedemptions \in 0..MaxValue
  /\ consumedHeads \subseteq StateHeadType
  /\ transitionLog \subseteq TransitionRecordType
  /\ preparedHead \in [OperationKeys -> StateHeadType \cup {NoState}]
  /\ mint \in [MintIds -> MintRecordType]
  /\ payment \in [CreditIds -> PaymentRecordType]
  /\ redemption \in [VoucherIds -> RedemptionRecordType]
  /\ conflictingDeliveries \subseteq ConflictRecordType
  /\ duplicateAckCredits \subseteq CreditIds
  /\ senderObservedAcks \subseteq CreditIds
  /\ trustedTime \in 0..MaxTime

ReserveEquation == reserve = totalTopups - totalRedemptions

LiabilityConservation == reserve = OfflineLiability

TotalValueConservation == SumOnline(Devices) + reserve = InitialOnlineTotal

ThreeMessageWireShape ==
  /\ PeerMessageOrder = <<"Request", "Payment", "Acknowledgement">>
  /\ \A q \in RequestIds :
       DOMAIN PaymentRequestV1(q) =
         {"protocolVersion", "network", "asset", "amount",
          "recipientAccount", "recipientLane", "recipientEncryptionKey",
          "hardwarePolicy", "requestId", "issuedAt", "expiresAt"}
  /\ \A c \in {x \in CreditIds : payment[x].phase # "Absent"} :
       DOMAIN PaymentV1(c) =
         {"protocolVersion", "creditId", "requestDigest", "amount",
          "senderBeforeCommitment", "senderAfterCommitment",
          "receiverBinding", "trustedSenderCommitTime",
          "encryptedCreditOpening", "hardwareTransitionCommitment",
          "recursiveProof"}
  /\ \A c \in StagedCredits :
       DOMAIN AcknowledgementV1(c) =
         {"protocolVersion", "requestDigest", "paymentDigest", "creditId",
          "inboxReceipt"}

RequestsNeverBindReceiverState ==
  \A q \in RequestIds :
    /\ "receiverBalanceHead" \notin DOMAIN PaymentRequestV1(q)
    /\ "receiverStateCommitment" \notin DOMAIN PaymentRequestV1(q)
    /\ PaymentRequestV1(q).amount > 0
    /\ PaymentRequestV1(q).issuedAt < PaymentRequestV1(q).expiresAt

DistinctPaymentsMayShareARequest ==
  /\ ModelCredit1 # ModelCredit2
  /\ CreditRequest[ModelCredit1] = ModelRequest1
  /\ CreditRequest[ModelCredit2] = ModelRequest1
  /\ CreditAmount[ModelCredit1] = RequestAmount[ModelRequest1]
  /\ CreditAmount[ModelCredit2] = RequestAmount[ModelRequest1]

SenderCommitWithinRequestWindow ==
  \A c \in CommittedCredits :
    /\ RequestValidAt(payment[c].request, payment[c].commitTime)
    /\ payment[c].commitTime = CreditCommitTime[c]
    /\ payment[c].request = CreditRequest[c]

PaymentsBindExactRequests ==
  \A c \in CommittedCredits :
    /\ payment[c].amount = RequestAmount[payment[c].request]
    /\ payment[c].amount > 0
    /\ payment[c].recipient = RequestRecipient[payment[c].request]
    /\ PaymentV1(c).requestDigest = RequestDigest(payment[c].request)

HardwareAuthorityIsRecursive ==
  \A c \in CreditIds :
    payment[c].phase \in {"Proofed", "Canonical", "Exposed", "Staged", "Folded"}
      => /\ payment[c].hardwareCertificate
         /\ payment[c].recursiveProof
         /\ GuardBundleValid(c)

EveryTransitionWasPrepared ==
  \A t \in transitionLog : preparedHead[t.operation] = t.before

ExactNextNonForking ==
  /\ consumedHeads = {t.before : t \in transitionLog}
  /\ \A h \in consumedHeads :
       Cardinality({t \in transitionLog : t.before = h}) = 1
  /\ \A op \in OperationKeys :
       Cardinality({t \in transitionLog : t.operation = op}) <= 1
  /\ \A t \in transitionLog :
       /\ t.before.deviceLane = t.device
       /\ t.after.deviceLane = t.device
       /\ IF t.kind = "Rotate"
            THEN t.after = RotationSuccessor(t.before)
            ELSE t.after = NormalSuccessor(t.before)

SenderSplitIsConservative ==
  \A c \in CommittedCredits :
    /\ payment[c].beforeBalance >= payment[c].amount
    /\ payment[c].afterBalance =
         payment[c].beforeBalance - payment[c].amount
    /\ payment[c].after = NormalSuccessor(payment[c].before)

AcknowledgementsFollowDurableStaging ==
  \A c \in CreditIds :
    /\ (payment[c].durableAck # NoAck) =
         (payment[c].phase \in {"Staged", "Folded"})
    /\ payment[c].durableAck # NoAck =>
         /\ payment[c].durableAck = c
         /\ payment[c].acceptedEnvelope = c
         /\ AcknowledgementV1(c).requestDigest =
              RequestDigest(payment[c].request)
         /\ AcknowledgementV1(c).paymentDigest = c

ExactDuplicateReturnsSameDurableAcknowledgement ==
  \A c \in duplicateAckCredits :
    /\ payment[c].phase \in {"Staged", "Folded"}
    /\ payment[c].acceptedEnvelope = c
    /\ payment[c].durableAck = c

SameIdDifferentBytesAreRejected ==
  \A conflict \in conflictingDeliveries :
    LET c == conflict.creditId
    IN /\ conflict.digest = ConflictingDigest
       /\ conflict.digest # c
       /\ payment[c].outboxEnvelope = c
       /\ payment[c].acceptedEnvelope # ConflictingDigest
       /\ payment[c].durableAck # ConflictingDigest

NoCountBasedReceiveRejection ==
  \A c \in CreditIds :
    payment[c].phase = "Staged" => ReceiveCreditSemanticallyValid(c)

ReceiveFoldUsesReplayNonmembership ==
  /\ \A c \in FoldedCredits : c \in device[CreditRecipient[c]].replayRoot
  /\ \A t \in {x \in transitionLog : x.kind = "ReceiveFold"} :
       /\ t.afterReplayRoot = t.beforeReplayRoot \cup {t.operation[2]}
       /\ t.operation[2] \notin t.beforeReplayRoot
  /\ \A d \in Devices :
       device[d].replayRoot =
         {c \in FoldedCredits : CreditRecipient[c] = d}

CommittedPaymentsRemainReceivable ==
  \A c \in CommittedCredits :
    CASE payment[c].phase = "HardwareCommitted" ->
           /\ payment[c].hardwareCertificate
           /\ payment[c].outboxRetained
      [] payment[c].phase = "Proofed" ->
           /\ payment[c].recursiveProof
           /\ payment[c].outboxRetained
      [] payment[c].phase = "Canonical" ->
           /\ payment[c].outboxEnvelope = c
           /\ payment[c].outboxRetained
      [] payment[c].phase = "Exposed" -> PaymentStageEligible(c)
      [] payment[c].phase = "Staged" ->
           /\ payment[c].acceptedEnvelope = c
           /\ payment[c].durableAck = c
           /\ c \notin device[CreditRecipient[c]].replayRoot
      [] payment[c].phase = "Folded" ->
           /\ payment[c].acceptedEnvelope = c
           /\ payment[c].durableAck = c
           /\ c \in device[CreditRecipient[c]].replayRoot
      [] OTHER -> FALSE

OutboxNeverFreezesSenderRemainder ==
  \A c \in CommittedCredits :
    /\ payment[c].afterBalance =
         payment[c].beforeBalance - payment[c].amount
    /\ payment[c].outboxRetained => payment[c].after # NoState

MintRequiresFinalityAndAuthorization ==
  \A m \in MintIds :
    mint[m].state # "Absent" =>
      /\ mint[m].amount > 0
      /\ mint[m].authorizationVerified
      /\ mint[m].finalityVerified

RedemptionSplitsAreFullOrPartial ==
  \A v \in VoucherIds :
    redemption[v].phase # "Absent" =>
      /\ redemption[v].amount > 0
      /\ redemption[v].beforeBalance >= redemption[v].amount
      /\ redemption[v].afterBalance =
           redemption[v].beforeBalance - redemption[v].amount
      /\ redemption[v].after = NormalSuccessor(redemption[v].before)

AppliedRedemptionNullifiersAreUnique ==
  /\ totalRedemptions <= totalTopups
  /\ \A v \in VoucherIds :
       redemption[v].phase = "Applied" =>
         /\ redemption[v].recursiveProof
         /\ redemption[v].outboxEnvelope = v
         /\ ~redemption[v].outboxRetained

RotationCarriesCompleteState ==
  \A t \in {x \in transitionLog : x.kind = "Rotate"} :
    /\ t.afterBalance = t.beforeBalance
    /\ t.afterReplayRoot = t.beforeReplayRoot
    /\ t.after = RotationSuccessor(t.before)

PublicStateAndProofShapeIsHistoryIndependent ==
  /\ \A d \in Devices :
       DOMAIN OpaqueStateCommitment(StateHead(d), device[d].balance) = 1..3
  /\ \A c \in {x \in CreditIds : payment[x].phase \in
                    {"Proofed", "Canonical", "Exposed", "Staged", "Folded"}} :
       PaymentV1(c).recursiveProof = <<"paired-pasta-proof", c>>

CumulativeEvidenceStep ==
  /\ consumedHeads \subseteq consumedHeads'
  /\ transitionLog \subseteq transitionLog'
  /\ conflictingDeliveries \subseteq conflictingDeliveries'
  /\ duplicateAckCredits \subseteq duplicateAckCredits'
  /\ senderObservedAcks \subseteq senderObservedAcks'
  /\ \A c \in CreditIds :
       payment[c].phase # "Absent" => payment'[c].phase # "Absent"
  /\ \A c \in CreditIds :
       c \in device[CreditRecipient[c]].replayRoot =>
         c \in device'[CreditRecipient[c]].replayRoot

CumulativeEvidenceNeverShrinks == [] [CumulativeEvidenceStep]_vars

PostCommitArtifactStep ==
  /\ \A c \in CreditIds :
       payment[c].outboxEnvelope # NoDigest =>
         payment'[c].outboxEnvelope = payment[c].outboxEnvelope
  /\ \A c \in CreditIds :
       payment[c].durableAck # NoAck =>
         payment'[c].durableAck = payment[c].durableAck
  /\ \A c \in CreditIds :
       payment[c].acceptedEnvelope # NoDigest =>
         payment'[c].acceptedEnvelope = payment[c].acceptedEnvelope
  /\ \A v \in VoucherIds :
       redemption[v].outboxEnvelope # NoDigest =>
         redemption'[v].outboxEnvelope = redemption[v].outboxEnvelope

PostCommitArtifactsNeverChange == [] [PostCommitArtifactStep]_vars

=============================================================================
