---- MODULE KagemushaV1ProofGates ----
EXTENDS KagemushaV1

(***************************************************************************
Deterministic positive traces and deliberately faulty transitions for the
three-message KAGEMUSHA state machine.  The runner retains every invariant
and temporal property from KagemushaV1.cfg while substituting HarnessSpec.
***************************************************************************)

CONSTANT Mutation
VARIABLES scenario, step

HarnessVars == <<vars, scenario, step>>

PaymentMutations ==
  {"nonrecursive-payment", "fork", "ack-before-stage",
   "conflict-accepted", "replay"}
RedemptionMutations == {"reserve-accounting", "rotation-loss"}

GenerateProofWithoutRecursiveGuard(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "HardwareCommitted"
  /\ payment[c].hardwareCertificate
  /\ ~payment[c].crashed
  /\ payment' = [payment EXCEPT ![c].phase = "Proofed"]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

ForkPaymentFromConsumedHead(c, source) ==
  LET d == CreditSender[c]
      amount == CreditAmount[c]
      op == SendKey(c)
      forgedBefore == payment[source].before
      forgedAfter == payment[source].after
      actualBefore == StateHead(d)
      actualAfter == NormalSuccessor(actualBefore)
      transition ==
        Transition("SendSplit", op, d, forgedBefore, forgedAfter,
                   device[d].balance, device[d].balance - amount,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ c \in CreditIds
     /\ source \in CreditIds
     /\ payment[source].phase # "Absent"
     /\ payment[c].phase = "Absent"
     /\ preparedHead[op] = actualBefore
     /\ CanAdvance(d)
     /\ forgedBefore \in consumedHeads
     /\ amount > 0
     /\ device[d].balance >= amount
     /\ trustedTime = CreditCommitTime[c]
     /\ RequestValidAt(CreditRequest[c], trustedTime)
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = @ - amount,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = @ + 1,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {forgedBefore}
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
                !.before = forgedBefore,
                !.after = forgedAfter,
                !.beforeBalance = device[d].balance,
                !.afterBalance = device[d].balance - amount,
                !.hardwareCertificate = TRUE,
                !.outboxRetained = TRUE]]
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, conflictingDeliveries,
                     duplicateAckCredits, senderObservedAcks, redemption,
                     trustedTime>>

AcknowledgeBeforeDurableStage(c) ==
  /\ c \in CreditIds
  /\ payment[c].phase = "Exposed"
  /\ payment[c].durableAck = NoAck
  /\ payment' = [payment EXCEPT ![c].durableAck = c]
  /\ UNCHANGED <<ledgerVars, mint, conflictingDeliveries,
                  duplicateAckCredits, senderObservedAcks, redemption,
                  trustedTime>>

AcceptConflictingBytes(c) ==
  LET conflict == [creditId |-> c, digest |-> ConflictingDigest]
  IN /\ c \in CreditIds
     /\ payment[c].phase = "Staged"
     /\ payment[c].acceptedEnvelope = c
     /\ conflictingDeliveries' = conflictingDeliveries \cup {conflict}
     /\ payment' = [payment EXCEPT ![c].acceptedEnvelope = ConflictingDigest]
     /\ UNCHANGED <<ledgerVars, mint, duplicateAckCredits,
                     senderObservedAcks, redemption, trustedTime>>

FoldAlreadyReplayedCredit(c) ==
  LET d == CreditRecipient[c]
      amount == payment[c].amount
      op == ReceiveKey(c)
      before == StateHead(d)
      after == NormalSuccessor(before)
      transition ==
        Transition("ReceiveFold", op, d, before, after,
                   device[d].balance, device[d].balance + amount,
                   device[d].replayRoot, device[d].replayRoot)
  IN /\ c \in CreditIds
     /\ payment[c].phase = "Folded"
     /\ c \in device[d].replayRoot
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
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, transportVars, redemption,
                     trustedTime>>

ApplyWithoutReserveDebit(v) ==
  LET d == redemption[v].owner
      amount == redemption[v].amount
  IN /\ v \in VoucherIds
     /\ redemption[v].phase = "Exposed"
     /\ redemption[v].recursiveProof
     /\ redemption[v].outboxEnvelope = v
     /\ reserve >= amount
     /\ online[d] + amount <= MaxValue
     /\ online' = [online EXCEPT ![d] = @ + amount]
     /\ totalRedemptions' = totalRedemptions + amount
     /\ redemption' =
          [redemption EXCEPT
            ![v] =
              [@ EXCEPT !.phase = "Applied", !.outboxRetained = FALSE]]
     /\ UNCHANGED <<device, reserve, totalTopups, consumedHeads,
                     transitionLog, preparedHead, mint, transportVars,
                     trustedTime>>

RotateDroppingBalance(r) ==
  LET d == RotationDevice[r]
      op == RotateKey(r)
      before == StateHead(d)
      after == RotationSuccessor(before)
      transition ==
        Transition("Rotate", op, d, before, after,
                   device[d].balance, 0,
                   device[d].replayRoot, {})
  IN /\ r \in RotationIds
     /\ preparedHead[op] = before
     /\ CanRotate(d)
     /\ device' =
          [device EXCEPT
            ![d] =
              [@ EXCEPT
                !.balance = 0,
                !.replayRoot = {},
                !.hardwareEpoch = @ + 1,
                !.logicalSequence = @ + 1,
                !.hardwareCounter = 0,
                !.stateNonce = @ + 1]]
     /\ consumedHeads' = consumedHeads \cup {before}
     /\ transitionLog' = transitionLog \cup {transition}
     /\ UNCHANGED <<online, reserve, totalTopups, totalRedemptions,
                     preparedHead, mint, transportVars, redemption,
                     trustedTime>>

CommitOutsideRequestWindow(c) ==
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
     /\ ~RequestValidAt(CreditRequest[c], trustedTime)
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

PaymentTrace ==
  CASE step = 0 -> Bootstrap(ModelDevice1)
    [] step = 1 -> Bootstrap(ModelDevice2)
    [] step = 2 -> TopUp(ModelDevice1, ModelMint1, 3)
    [] step = 3 -> PrepareMintFold(ModelMint1)
    [] step = 4 -> MintFold(ModelMint1)
    [] step = 5 -> AdvanceTrustedTime(11)
    [] step = 6 -> PrepareSend(ModelCredit1)
    [] step = 7 -> SendSplit(ModelCredit1)
    [] step = 8 -> CrashPayment(ModelCredit1)
    [] step = 9 -> RecoverPayment(ModelCredit1)
    [] step = 10 ->
         IF Mutation = "nonrecursive-payment"
           THEN GenerateProofWithoutRecursiveGuard(ModelCredit1)
           ELSE GeneratePaymentProof(ModelCredit1)
    [] step = 11 -> PersistPaymentEnvelope(ModelCredit1)
    [] step = 12 -> ExposePayment(ModelCredit1)
    [] step = 13 -> AdvanceTrustedTime(12)
    [] step = 14 -> PrepareSend(ModelCredit2)
    [] step = 15 ->
         IF Mutation = "fork"
           THEN ForkPaymentFromConsumedHead(ModelCredit2, ModelCredit1)
           ELSE SendSplit(ModelCredit2)
    [] step = 16 -> GeneratePaymentProof(ModelCredit2)
    [] step = 17 -> PersistPaymentEnvelope(ModelCredit2)
    [] step = 18 -> ExposePayment(ModelCredit2)
    [] step = 19 -> AdvanceTrustedTime(30)
    [] step = 20 ->
         IF Mutation = "ack-before-stage"
           THEN AcknowledgeBeforeDurableStage(ModelCredit1)
           ELSE StagePayment(ModelCredit1)
    [] step = 21 -> StagePayment(ModelCredit2)
    [] step = 22 -> RedeliverExactDuplicate(ModelCredit1)
    [] step = 23 ->
         IF Mutation = "conflict-accepted"
           THEN AcceptConflictingBytes(ModelCredit1)
           ELSE RejectConflictingDelivery(ModelCredit1)
    [] step = 24 -> PrepareReceiveFold(ModelCredit1)
    [] step = 25 -> ReceiveFold(ModelCredit1)
    [] step = 26 ->
         IF Mutation = "replay"
           THEN FoldAlreadyReplayedCredit(ModelCredit1)
           ELSE PrepareReceiveFold(ModelCredit2)
    [] step = 27 -> ReceiveFold(ModelCredit2)
    [] step = 28 -> ObserveAcknowledgement(ModelCredit1)
    [] step = 29 -> ObserveAcknowledgement(ModelCredit2)
    [] OTHER -> FALSE

RedemptionTrace ==
  CASE step = 0 -> Bootstrap(ModelDevice2)
    [] step = 1 -> TopUp(ModelDevice2, ModelMint2, 2)
    [] step = 2 -> PrepareMintFold(ModelMint2)
    [] step = 3 -> MintFold(ModelMint2)
    [] step = 4 -> PrepareRedemption(ModelVoucher1, ModelDevice2, 1)
    [] step = 5 -> RedeemSplit(ModelVoucher1)
    [] step = 6 -> CrashRedemption(ModelVoucher1)
    [] step = 7 -> RecoverRedemption(ModelVoucher1)
    [] step = 8 -> GenerateRedemptionProof(ModelVoucher1)
    [] step = 9 -> PersistRedemptionEnvelope(ModelVoucher1)
    [] step = 10 -> ExposeRedemption(ModelVoucher1)
    [] step = 11 ->
         IF Mutation = "reserve-accounting"
           THEN ApplyWithoutReserveDebit(ModelVoucher1)
           ELSE ApplyRedemption(ModelVoucher1)
    [] step = 12 -> PrepareRotate(ModelRotation2)
    [] step = 13 ->
         IF Mutation = "rotation-loss"
           THEN RotateDroppingBalance(ModelRotation2)
           ELSE Rotate(ModelRotation2)
    [] step = 14 -> PrepareRedemption(ModelVoucher2, ModelDevice2, 1)
    [] step = 15 -> RedeemSplit(ModelVoucher2)
    [] step = 16 -> GenerateRedemptionProof(ModelVoucher2)
    [] step = 17 -> PersistRedemptionEnvelope(ModelVoucher2)
    [] step = 18 -> ExposeRedemption(ModelVoucher2)
    [] step = 19 -> ApplyRedemption(ModelVoucher2)
    [] OTHER -> FALSE

ExpiryTrace ==
  CASE step = 0 -> Bootstrap(ModelDevice1)
    [] step = 1 -> TopUp(ModelDevice1, ModelMint1, 3)
    [] step = 2 -> PrepareMintFold(ModelMint1)
    [] step = 3 -> MintFold(ModelMint1)
    [] step = 4 -> AdvanceTrustedTime(31)
    [] step = 5 -> PrepareSend(ModelCredit3)
    [] step = 6 -> CommitOutsideRequestWindow(ModelCredit3)
    [] OTHER -> FALSE

HarnessInit ==
  /\ Init
  /\ scenario \in
       CASE Mutation = "none" -> {"Payment", "Redemption"}
         [] Mutation = "expired-commit" -> {"Expiry"}
         [] Mutation \in PaymentMutations -> {"Payment"}
         [] OTHER -> {"Redemption"}
  /\ step = 0

HarnessNext ==
  /\ CASE scenario = "Payment" -> PaymentTrace
       [] scenario = "Redemption" -> RedemptionTrace
       [] OTHER -> ExpiryTrace
  /\ step' = step + 1
  /\ UNCHANGED scenario

HarnessSpec == HarnessInit /\ [][HarnessNext]_HarnessVars

HarnessCompletion ==
  /\ scenario = "Payment" /\ step = 30 =>
       /\ payment[ModelCredit1].phase = "Folded"
       /\ payment[ModelCredit2].phase = "Folded"
       /\ ModelCredit1 \in duplicateAckCredits
       /\ [creditId |-> ModelCredit1, digest |-> ConflictingDigest]
            \in conflictingDeliveries
       /\ ModelCredit1 \in senderObservedAcks
       /\ ModelCredit2 \in senderObservedAcks
       /\ trustedTime >= RequestExpiresAt[ModelRequest1]
       /\ device[ModelDevice1].balance = 1
       /\ device[ModelDevice2].balance = 2
  /\ scenario = "Redemption" /\ step = 20 =>
       /\ redemption[ModelVoucher1].phase = "Applied"
       /\ redemption[ModelVoucher2].phase = "Applied"
       /\ device[ModelDevice2].balance = 0
       /\ device[ModelDevice2].hardwareEpoch = 1
       /\ reserve = 0

=============================================================================
