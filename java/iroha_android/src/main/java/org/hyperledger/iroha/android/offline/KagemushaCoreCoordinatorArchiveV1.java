// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import org.hyperledger.iroha.sdk.offline.KagemushaDeviceRedemptionTerminalReceiptV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderPublicInputsV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderWalletContextV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderCandidateV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderPreparationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderRecoveryV1;

/** Java mirror of canonical public projections; shape decoding grants no native Core capability. */
public final class KagemushaCoreCoordinatorArchiveV1 {
  private KagemushaCoreCoordinatorArchiveV1() {}

  /** Encode a bounded canonical preparation selector. */
  public static byte[] encodePreparationShape(final KagemushaNativeSenderPreparationV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(value);
  }
  /** Decode exact shape without authenticating native preparation. */
  public static KagemushaNativeSenderPreparationV1 decodePreparationShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(bytes);
  }
  /** Encode a candidate selector without granting proof authority. */
  public static byte[] encodeCandidateShape(final KagemushaNativeSenderCandidateV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(value);
  }
  /** Decode candidate shape; the native boundary must authenticate its signature and journal. */
  public static KagemushaNativeSenderCandidateV1 decodeCandidateShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(bytes);
  }
  /** Encode one recovery selector. */
  public static byte[] encodeRecoveryShape(final KagemushaNativeSenderRecoveryV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(value);
  }
  /** Decode exact recovery shape without asserting terminal state exists. */
  public static KagemushaNativeSenderRecoveryV1 decodeRecoveryShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(bytes);
  }
  /** Encode the complete canonical redemption receipt archive. */
  public static byte[] encodeRedemptionReceiptShape(final KagemushaDeviceRedemptionTerminalReceiptV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.encodeRedemptionReceiptShape(value);
  }
  /** Decode receipt shape; this does not recover verified redemption-release authority. */
  public static KagemushaDeviceRedemptionTerminalReceiptV1 decodeRedemptionReceiptShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.decodeRedemptionReceiptShapeExact(bytes);
  }
  /** Compute the exact public input binding retained by native Core. */
  public static byte[] inputsDigestShape(final byte[] operationId,
      final KagemushaDeviceSenderWalletContextV1 context, final KagemushaDeviceSenderPublicInputsV1 inputs) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(operationId, context, inputs);
  }
  /** Compute the byte-exact terminal envelope binding. */
  public static byte[] terminalEnvelopeDigestShape(final byte[] envelope) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope);
  }
}
