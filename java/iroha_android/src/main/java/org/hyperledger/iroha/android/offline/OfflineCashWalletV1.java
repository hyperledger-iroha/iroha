// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceIntentAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAccountIdV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareRecoveryV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareStageDispositionV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintCreditV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashStagedPaymentV1;

/** Java facade over the canonical Kotlin/native-core Offline Cash V1 orchestration. */
public final class OfflineCashWalletV1 {
  private final org.hyperledger.iroha.sdk.offline.OfflineCashWalletV1 delegate;

  private OfflineCashWalletV1(
      final org.hyperledger.iroha.sdk.offline.OfflineCashWalletV1 delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  /** Open only when the complete non-forking native hardware contract is present. */
  public static OfflineCashWalletV1 open(final OfflineCashHardwareProviderV1 provider) {
    return new OfflineCashWalletV1(
        org.hyperledger.iroha.sdk.offline.OfflineCashWalletV1.open(
            Objects.requireNonNull(provider, "provider")));
  }

  public OfflineCashHardwareCredentialV1 hardwareCredential() {
    return delegate.hardwareCredential();
  }

  public OfflineCashAggregateStateCommitmentV1 aggregateState() {
    return delegate.aggregateState();
  }

  public BigInteger journalRevision() {
    return delegate.journalRevision();
  }

  public OfflineCashHardwareRecoveryV1 recover() {
    return delegate.recover();
  }

  public OfflineCashPaymentRequestV1 createPaymentRequest(
      final OfflineCashAccountIdV1 recipient,
      final BigInteger amount,
      final long validityWindowMillis) {
    return delegate.createPaymentRequest(
        Objects.requireNonNull(recipient, "recipient"),
        Objects.requireNonNull(amount, "amount"),
        validityWindowMillis);
  }

  public OfflineCashAcceptanceIntentAuthorizationV1 authorizeAcceptanceIntent(
      final OfflineCashPaymentRequestV1 request) {
    return delegate.authorizeAcceptanceIntent(Objects.requireNonNull(request, "request"));
  }

  public OfflineCashAcceptanceTicketV1 issueAcceptanceTicket(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return delegate.issueAcceptanceTicket(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public OfflineCashPaymentV1 send(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization,
      final OfflineCashAcceptanceTicketV1 ticket) {
    return delegate.send(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public OfflineCashStagedPaymentV1 stagePayment(
      final OfflineCashPaymentRequestV1 request, final OfflineCashPaymentV1 payment) {
    return delegate.stagePayment(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public OfflineCashHardwareStageDispositionV1 stageMintCredit(
      final OfflineCashMintAuthorizationV1 authorization,
      final OfflineCashMintCreditV1 mintCredit) {
    return delegate.stageMintCredit(
        Objects.requireNonNull(authorization, "authorization"),
        Objects.requireNonNull(mintCredit, "mintCredit"));
  }

  public boolean foldPendingCredit() {
    return delegate.foldPendingCredit();
  }

  public BigInteger drainPendingCredits() {
    return delegate.drainPendingCredits();
  }

  public OfflineCashPaymentV1 recoverPayment(
      final OfflineCashPaymentRequestV1 request, final byte[] creditId) {
    final byte[] copy = Objects.requireNonNull(creditId, "creditId").clone();
    try {
      return delegate.recoverPayment(Objects.requireNonNull(request, "request"), copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public void recordAcknowledgement(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment,
      final OfflineCashAcknowledgementV1 acknowledgement) {
    delegate.recordAcknowledgement(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(acknowledgement, "acknowledgement"));
  }

  public OfflineCashRedemptionVoucherV1 redeem(
      final BigInteger amount, final OfflineCashAccountIdV1 beneficiary) {
    return delegate.redeem(
        Objects.requireNonNull(amount, "amount"),
        Objects.requireNonNull(beneficiary, "beneficiary"));
  }

  public OfflineCashRedemptionVoucherV1 recoverRedemption(final byte[] redemptionId) {
    final byte[] copy = Objects.requireNonNull(redemptionId, "redemptionId").clone();
    try {
      return delegate.recoverRedemption(copy);
    } finally {
      Arrays.fill(copy, (byte) 0);
    }
  }

  public OfflineCashAggregateStateCommitmentV1 rotateHardwareEpoch() {
    return delegate.rotateHardwareEpoch();
  }
}
