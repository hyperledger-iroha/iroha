// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceIntentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAccountIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareRecoveryV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareStageDispositionV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintCreditV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestModeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaReceiveFoldBatchResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.KagemushaStagedPaymentV1;

/** Java facade over the canonical Kotlin/native-core KAGEMUSHA V1 orchestration. */
public final class KagemushaWalletV1 {
  private final org.hyperledger.iroha.sdk.offline.KagemushaWalletV1 delegate;

  private KagemushaWalletV1(
      final org.hyperledger.iroha.sdk.offline.KagemushaWalletV1 delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  /** Open only when the complete non-forking native hardware contract is present. */
  public static KagemushaWalletV1 open(final KagemushaHardwareProviderV1 provider) {
    return new KagemushaWalletV1(
        org.hyperledger.iroha.sdk.offline.KagemushaWalletV1.open(
            Objects.requireNonNull(provider, "provider")));
  }

  public KagemushaHardwareCredentialV1 hardwareCredential() {
    return delegate.hardwareCredential();
  }

  public KagemushaAggregateStateCommitmentV1 aggregateState() {
    return delegate.aggregateState();
  }

  public BigInteger journalRevision() {
    return delegate.journalRevision();
  }

  public KagemushaHardwareRecoveryV1 recover() {
    return delegate.recover();
  }

  public KagemushaPaymentRequestV1 createPaymentRequest(
      final KagemushaAccountIdV1 recipient,
      final KagemushaPaymentRequestModeV1 requestMode,
      final long validityWindowMillis) {
    return delegate.createPaymentRequest(
        Objects.requireNonNull(recipient, "recipient"),
        Objects.requireNonNull(requestMode, "requestMode"),
        validityWindowMillis);
  }

  public KagemushaAcceptanceIntentV1 prepareAcceptanceIntent(
      final KagemushaPaymentRequestV1 request, final BigInteger exactAmount) {
    return delegate.prepareAcceptanceIntent(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(exactAmount, "exactAmount"));
  }

  public KagemushaAcceptanceIntentV1 recoverAcceptanceIntent(
      final KagemushaPaymentRequestV1 request, final byte[] intentId) {
    final byte[] copy = Objects.requireNonNull(intentId, "intentId").clone();
    try {
      return delegate.recoverAcceptanceIntent(
          Objects.requireNonNull(request, "request"), copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public KagemushaAcceptanceTicketV1 issueAcceptanceTicket(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return delegate.issueAcceptanceTicket(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  public KagemushaAcceptanceTicketV1 recoverAcceptanceTicket(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final byte[] acceptanceTicketId) {
    final byte[] copy =
        Objects.requireNonNull(acceptanceTicketId, "acceptanceTicketId").clone();
    try {
      return delegate.recoverAcceptanceTicket(
          Objects.requireNonNull(request, "request"),
          Objects.requireNonNull(intent, "intent"),
          copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public KagemushaPaymentV1 send(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return delegate.send(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public KagemushaStagedPaymentV1 stagePayment(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final KagemushaPaymentV1 payment) {
    return delegate.stagePayment(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"),
        Objects.requireNonNull(payment, "payment"));
  }

  public KagemushaHardwareStageDispositionV1 stageMintCredit(
      final KagemushaMintAuthorizationV1 authorization,
      final KagemushaMintCreditV1 mintCredit) {
    return delegate.stageMintCredit(
        Objects.requireNonNull(authorization, "authorization"),
        Objects.requireNonNull(mintCredit, "mintCredit"));
  }

  public KagemushaReceiveFoldBatchResultV1 foldReceiveBatch() {
    return delegate.foldReceiveBatch();
  }

  public BigInteger drainPendingCredits() {
    return delegate.drainPendingCredits();
  }

  public KagemushaPaymentV1 recoverPayment(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final byte[] creditId) {
    final byte[] copy = Objects.requireNonNull(creditId, "creditId").clone();
    try {
      return delegate.recoverPayment(
          Objects.requireNonNull(request, "request"),
          Objects.requireNonNull(intent, "intent"),
          Objects.requireNonNull(ticket, "ticket"),
          copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public void recordAcknowledgement(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final KagemushaPaymentV1 payment,
      final KagemushaAcknowledgementV1 acknowledgement) {
    delegate.recordAcknowledgement(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(acknowledgement, "acknowledgement"));
  }

  public KagemushaRedemptionVoucherV1 redeem(
      final BigInteger amount, final KagemushaAccountIdV1 beneficiary) {
    return delegate.redeem(
        Objects.requireNonNull(amount, "amount"),
        Objects.requireNonNull(beneficiary, "beneficiary"));
  }

  public KagemushaRedemptionVoucherV1 recoverRedemption(final byte[] redemptionId) {
    final byte[] copy = Objects.requireNonNull(redemptionId, "redemptionId").clone();
    try {
      return delegate.recoverRedemption(copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public KagemushaAggregateStateCommitmentV1 rotateHardwareEpoch() {
    return delegate.rotateHardwareEpoch();
  }
}
