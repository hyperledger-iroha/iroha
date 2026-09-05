// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderPublicInputsV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderTerminalReceiptV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareQualificationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareTerminalResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeOutboxReleaseV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderCandidateV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderKindV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderPreparationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderRecoveryV1;

/** Java typed mirror using Kotlin's real JNI coordinator and identical context binding checks. */
public final class KagemushaNativeCoreCoordinatorAdapterV1 implements KagemushaNativeCoreCoordinatorV1 {
  private final org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorAdapterV1 delegate;

  private KagemushaNativeCoreCoordinatorAdapterV1(
      final org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorAdapterV1 delegate) {
    this.delegate = delegate;
  }

  /** Open only an installed qualified native backend; no software fallback or stock factory. */
  public static KagemushaNativeCoreCoordinatorAdapterV1 open(final String storagePath) {
    return new KagemushaNativeCoreCoordinatorAdapterV1(
        org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorAdapterV1.open(storagePath));
  }

  @Override public byte[] reserveOperationId(final int operation, final byte[] operationId, final byte[] publicBinding) {
    return delegate.reserveOperationId(operation, operationId, publicBinding);
  }
  @Override public void acceptQualification(final KagemushaHardwareQualificationV1 qualification, final byte[] hardwarePolicyDigest) {
    delegate.acceptQualification(qualification, hardwarePolicyDigest);
  }
  @Override public void acceptAuthenticatedDeviceReply(final int operation, final byte[] requestId,
      final byte[] canonicalCommand, final byte[] canonicalReply, final byte[] responseAuthenticator,
      final KagemushaHardwareQualificationV1 qualification) {
    delegate.acceptAuthenticatedDeviceReply(operation, requestId, canonicalCommand, canonicalReply, responseAuthenticator, qualification);
  }
  @Override public KagemushaNativeSenderPreparationV1 beginSenderTransition(final byte[] operationId,
      final KagemushaDeviceSenderPublicInputsV1 inputs, final KagemushaHardwareQualificationV1 qualification) {
    return delegate.beginSenderTransition(operationId, inputs, qualification);
  }
  @Override public KagemushaNativeSenderCandidateV1 provePreparedSenderTransition(
      final KagemushaNativeSenderPreparationV1 preparation, final byte[] authenticatedPreparationReply) {
    return delegate.provePreparedSenderTransition(preparation, authenticatedPreparationReply);
  }
  @Override public byte[] terminalEnvelope(final KagemushaNativeSenderCandidateV1 candidate, final byte[] authenticatedCommitReply) {
    return delegate.terminalEnvelope(candidate, authenticatedCommitReply);
  }
  @Override public KagemushaHardwareTerminalResultV1 acceptInstalledTerminal(final KagemushaNativeSenderCandidateV1 candidate,
      final byte[] canonicalEnvelope, final byte[] authenticatedInstallReply, final byte[] authenticatedInstalledReply,
      final byte[] authenticatedWalletSnapshotReply) {
    return delegate.acceptInstalledTerminal(candidate, canonicalEnvelope, authenticatedInstallReply,
        authenticatedInstalledReply, authenticatedWalletSnapshotReply);
  }
  @Override public KagemushaNativeSenderRecoveryV1 senderRecovery(final KagemushaNativeSenderKindV1 kind,
      final byte[] terminalId, final KagemushaHardwareQualificationV1 qualification) {
    return delegate.senderRecovery(kind, terminalId, qualification);
  }
  @Override public KagemushaNativeSenderRecoveryV1 senderRecoveryByOperationId(final KagemushaNativeSenderKindV1 kind,
      final byte[] operationId, final KagemushaHardwareQualificationV1 qualification) {
    return delegate.senderRecoveryByOperationId(kind, operationId, qualification);
  }
  @Override public byte[] recoverTerminalEnvelope(final KagemushaNativeSenderRecoveryV1 recovery, final byte[] authenticatedInstalledReply) {
    return delegate.recoverTerminalEnvelope(recovery, authenticatedInstalledReply);
  }
  @Override public KagemushaNativeOutboxReleaseV1 outboxRelease(final byte[] creditId, final KagemushaDeviceSenderPublicInputsV1 inputs,
      final byte[] canonicalPayment, final KagemushaDeviceSenderTerminalReceiptV1 terminalReceipt,
      final KagemushaHardwareQualificationV1 qualification) {
    return delegate.outboxRelease(creditId, inputs, canonicalPayment, terminalReceipt, qualification);
  }
}
