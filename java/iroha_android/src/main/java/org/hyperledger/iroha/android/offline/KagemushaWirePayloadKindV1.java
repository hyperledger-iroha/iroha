// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

/**
 * Canonical KAGEMUSHA V1 value whose opaque Norito bytes are transported.
 *
 * <p>Request, payment, and acknowledgement form the complete IPM1 payment exchange.
 */
public enum KagemushaWirePayloadKindV1 {
  PAYMENT_REQUEST(
      KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
      KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES),
  PAYMENT(KagemushaWireV1.MAXIMUM_PAYMENT_BYTES, KagemushaWireV1.MAXIMUM_PAYMENT_TEXT_BYTES),
  ACKNOWLEDGEMENT(
      KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES,
      KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES),
  MINT_AUTHORIZATION(
      KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES,
      KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES),
  MINT_CREDIT(
      KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES,
      KagemushaWireV1.MAXIMUM_MINT_CREDIT_TEXT_BYTES),
  REDEMPTION_VOUCHER(
      KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES,
      KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES);

  private final int maximumRawBytes;
  private final int maximumTextBytes;

  KagemushaWirePayloadKindV1(final int maximumRawBytes, final int maximumTextBytes) {
    this.maximumRawBytes = maximumRawBytes;
    this.maximumTextBytes = maximumTextBytes;
  }

  /** Return the maximum canonical Norito bytes for this value. */
  public int maximumRawBytes() {
    return maximumRawBytes;
  }

  /** Return the maximum complete {@code kgm1:} text bytes for this value. */
  public int maximumTextBytes() {
    return maximumTextBytes;
  }

  org.hyperledger.iroha.sdk.offline.KagemushaWirePayloadKindV1 canonicalKind() {
    return org.hyperledger.iroha.sdk.offline.KagemushaWirePayloadKindV1.valueOf(name());
  }
}
